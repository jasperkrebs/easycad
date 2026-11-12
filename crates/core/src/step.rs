//! STEP (ISO 10303-21), for a manufacturer who slices the shape their own way, or for CAD.
//!
//! The schema is AP214 (`AUTOMOTIVE_DESIGN`), the one CAD programs and slicers built on Open
//! CASCADE (FreeCAD, PrusaSlicer, Bambu Studio, OrcaSlicer, …) read most widely; AP242 readers
//! read it too. Units are mm and radians.
//!
//! A surface of revolution keeps its exact shape: each span of its profile becomes one face, a
//! plane, a cylinder or a cone round the axis, bounded by true circles (with a seam line where
//! the face wraps all the way round). A solid wall (`thicken`, or a capped revolve) is a closed
//! shell: a `MANIFOLD_SOLID_BREP`. Any other shape is written as its mesh, one flat triangle a
//! face, sharing edges, still a solid when the mesh is closed. Open surfaces go in as surface
//! models (`SHELL_BASED_SURFACE_MODEL`).
//!
//! The text is deterministic: the same shape always gives the same file (no time stamp).

use crate::geom::{P2, P3, cross3, dist3, norm3, scale3, sub3, unit3};
use crate::surface::{Mesh, Revolution, Surface, signed_volume};
use std::collections::HashMap;
use std::fmt::Write as _;

/// The most triangles a meshed (not exact) shape may have: STEP spends about a kilobyte on
/// each, so more is a file no one wants to open.
pub const MAX_FACETS: usize = 40_000;

/// The schema line: AP214's.
pub const SCHEMA: &str = "AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }";

/// The STEP file of `surfaces`. `product` names the part inside it, `file` is its file name,
/// and `description` goes in the header (what it is, the settings it was made with).
pub fn write(
    surfaces: &[&Surface],
    product: &str,
    file: &str,
    description: &[String],
) -> Result<String, String> {
    if surfaces.is_empty() {
        return Err("there's no shape to write".into());
    }
    let mut out = Out::default();
    let mut shells: Vec<(usize, bool)> = Vec::new();
    for s in surfaces {
        let (faces, closed) = match s.revolution() {
            Some(rev) => revolution_faces(&mut out, rev, s.capped)?,
            None => mesh_faces(&mut out, s)?,
        };
        if faces.is_empty() {
            continue;
        }
        let kind = if closed { "CLOSED_SHELL" } else { "OPEN_SHELL" };
        let shell = out.add(&format!("{kind}('',{})", refs(&faces)));
        shells.push((shell, closed));
    }
    if shells.is_empty() {
        return Err("the shape has no faces to write".into());
    }

    // Units (mm, radians) and the 3D context.
    let mm = out.add("( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) )");
    let rad = out.add("( NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.) )");
    let sr = out.add("( NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT() )");
    let accuracy = out.add(&format!(
        "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-06),#{mm},'distance_accuracy_value','confusion accuracy')"
    ));
    let context = out.add(&format!(
        "( GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{accuracy})) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{mm},#{rad},#{sr})) REPRESENTATION_CONTEXT('Context #1','3D Context with UNIT and UNCERTAINTY') )"
    ));
    let origin = out.placement([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
    let name = text(product);
    let representation = if shells.iter().all(|s| s.1) {
        let mut items = vec![origin];
        for (shell, _) in &shells {
            items.push(out.add(&format!("MANIFOLD_SOLID_BREP({name},#{shell})")));
        }
        out.add(&format!(
            "ADVANCED_BREP_SHAPE_REPRESENTATION({name},{},#{context})",
            refs(&items)
        ))
    } else {
        let ids: Vec<usize> = shells.iter().map(|s| s.0).collect();
        let model = out.add(&format!("SHELL_BASED_SURFACE_MODEL({name},{})", refs(&ids)));
        out.add(&format!(
            "MANIFOLD_SURFACE_SHAPE_REPRESENTATION({name},(#{origin},#{model}),#{context})"
        ))
    };

    // The product: one part, with that shape.
    let app =
        out.add("APPLICATION_CONTEXT('core data for automotive mechanical design processes')");
    out.add(&format!(
        "APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{app})"
    ));
    let pc = out.add(&format!("PRODUCT_CONTEXT('',#{app},'mechanical')"));
    let prod = out.add(&format!("PRODUCT({name},{name},'',(#{pc}))"));
    out.add(&format!(
        "PRODUCT_RELATED_PRODUCT_CATEGORY('part',$,(#{prod}))"
    ));
    let pdf = out.add(&format!("PRODUCT_DEFINITION_FORMATION('','',#{prod})"));
    let pdc = out.add(&format!(
        "PRODUCT_DEFINITION_CONTEXT('part definition',#{app},'design')"
    ));
    let pd = out.add(&format!("PRODUCT_DEFINITION('design','',#{pdf},#{pdc})"));
    let pds = out.add(&format!("PRODUCT_DEFINITION_SHAPE('','',#{pd})"));
    out.add(&format!(
        "SHAPE_DEFINITION_REPRESENTATION(#{pds},#{representation})"
    ));

    let description: Vec<String> = description.iter().map(|d| text(d)).collect();
    let description = if description.is_empty() {
        "('')".to_string()
    } else {
        format!("({})", description.join(","))
    };
    let mut file_text = String::with_capacity(out.text.len() + 512);
    file_text.push_str("ISO-10303-21;\nHEADER;\n");
    let _ = writeln!(file_text, "FILE_DESCRIPTION({description},'2;1');");
    let _ = writeln!(
        file_text,
        "FILE_NAME({},'',(''),(''),'EasyCAD','EasyCAD','');",
        text(file)
    );
    let _ = writeln!(file_text, "FILE_SCHEMA(('{SCHEMA}'));");
    file_text.push_str("ENDSEC;\nDATA;\n");
    file_text.push_str(&out.text);
    file_text.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    Ok(file_text)
}

/// The DATA section being written: entities numbered from #1 in the order they're added, each
/// after everything it refers to.
#[derive(Default)]
struct Out {
    text: String,
    count: usize,
}

impl Out {
    fn add(&mut self, entity: &str) -> usize {
        self.count += 1;
        let _ = writeln!(self.text, "#{}={entity};", self.count);
        self.count
    }

    fn point(&mut self, p: P3) -> usize {
        self.add(&format!(
            "CARTESIAN_POINT('',({},{},{}))",
            real(p[0]),
            real(p[1]),
            real(p[2])
        ))
    }

    fn direction(&mut self, v: P3) -> usize {
        self.add(&format!(
            "DIRECTION('',({},{},{}))",
            real(v[0]),
            real(v[1]),
            real(v[2])
        ))
    }

    /// A frame at `origin`: `z` its axis (a surface's normal), `x` its reference direction.
    fn placement(&mut self, origin: P3, z: P3, x: P3) -> usize {
        let (o, z, x) = (self.point(origin), self.direction(z), self.direction(x));
        self.add(&format!("AXIS2_PLACEMENT_3D('',#{o},#{z},#{x})"))
    }

    fn vertex(&mut self, p: P3) -> usize {
        let p = self.point(p);
        self.add(&format!("VERTEX_POINT('',#{p})"))
    }

    /// A straight edge between two vertices at `a` and `b`.
    fn line_edge(&mut self, va: usize, vb: usize, a: P3, b: P3) -> usize {
        let d = unit3(sub3(b, a)).unwrap_or([1.0, 0.0, 0.0]);
        let start = self.point(a);
        let dir = self.direction(d);
        let vector = self.add(&format!("VECTOR('',#{dir},{})", real(dist3(a, b))));
        let line = self.add(&format!("LINE('',#{start},#{vector})"));
        self.add(&format!("EDGE_CURVE('',#{va},#{vb},#{line},.T.)"))
    }

    /// A loop of edges, each `.T.` along its curve or `.F.` against it.
    fn edge_loop(&mut self, edges: &[(usize, bool)]) -> usize {
        let oriented: Vec<usize> = edges
            .iter()
            .map(|&(e, along)| self.add(&format!("ORIENTED_EDGE('',*,*,#{e},{})", flag(along))))
            .collect();
        self.add(&format!("EDGE_LOOP('',{})", refs(&oriented)))
    }

    /// A face of `surface`, bounded by `loops` (the first the outer one). `same_sense`: the
    /// face's outward side is the surface's own normal.
    fn face(&mut self, loops: &[usize], surface: usize, same_sense: bool) -> usize {
        let bounds: Vec<usize> = loops
            .iter()
            .enumerate()
            .map(|(k, &l)| {
                let kind = if k == 0 {
                    "FACE_OUTER_BOUND"
                } else {
                    "FACE_BOUND"
                };
                self.add(&format!("{kind}('',#{l},.T.)"))
            })
            .collect();
        self.add(&format!(
            "ADVANCED_FACE('',{},#{surface},{})",
            refs(&bounds),
            flag(same_sense)
        ))
    }
}

/// The exact faces of a surface of revolution, and whether they close up into a solid.
///
/// Each span of the profile from (r0, z0) to (r1, z1) sweeps one face, parametrised by the
/// angle round and the distance along the span. Its boundary runs round the circle at the
/// span's start, up the seam (the span itself, at angle 0), back round the circle at its end and
/// down the seam: counter-clockwise in those parameters, so the face's outward side is
/// (dz, −dr), to the right of the profile. A solid's profile runs counter-clockwise in (r, z),
/// which puts that side outside the material. Neighbouring faces use each circle in opposite
/// directions, as a closed shell needs. Spans on the axis make no face; a circle of no radius
/// (a point on the axis) is left out of the boundary.
fn revolution_faces(
    out: &mut Out,
    rev: &Revolution,
    capped: bool,
) -> Result<(Vec<usize>, bool), String> {
    let mut profile: Vec<P2> = rev.profile.clone();
    let closed = rev.closed || capped;
    if capped && !rev.closed {
        // Flat caps: the profile closed along the axis from end to end.
        if let (Some(first), Some(last)) = (profile.first().copied(), profile.last().copied()) {
            if last[0] > 0.0 {
                profile.push([0.0, last[1]]);
            }
            if first[0] > 0.0 {
                profile.insert(0, [0.0, first[1]]);
            }
        }
        if signed_area(&profile) < 0.0 {
            profile.reverse();
        }
    }
    let n = profile.len();
    if n < 2 {
        return Err("the profile needs at least 2 points".into());
    }
    let size = profile
        .iter()
        .fold(1.0f64, |m, p| m.max(p[0].abs()).max(p[1].abs()));
    let eps = 1e-9 * size;
    let at = |p: P2| rev.point(p[0], p[1], 0.0);
    let on_axis = |p: P2| p[0] <= eps;

    let mut vertices: Vec<Option<usize>> = vec![None; n];
    let mut circles: Vec<Option<usize>> = vec![None; n];
    let mut vertex = |out: &mut Out, i: usize| -> usize {
        if let Some(v) = vertices[i] {
            return v;
        }
        let v = out.vertex(at(profile[i]));
        vertices[i] = Some(v);
        v
    };
    let spans = if closed { n } else { n - 1 };
    let mut faces = Vec::with_capacity(spans);
    for i in 0..spans {
        let j = (i + 1) % n;
        let (p0, p1) = (profile[i], profile[j]);
        let (dr, dz) = (p1[0] - p0[0], p1[1] - p0[1]);
        if (on_axis(p0) && on_axis(p1)) || dr.hypot(dz) <= eps {
            continue;
        }
        // The circles at each end (none on the axis): round counter-clockwise from the seam.
        let mut circle = |out: &mut Out, k: usize| -> Option<usize> {
            let p = profile[k];
            if on_axis(p) {
                return None;
            }
            if let Some(c) = circles[k] {
                return Some(c);
            }
            let v = vertex(out, k);
            let frame = out.placement(rev.point(0.0, p[1], 0.0), rev.axis, rev.xdir);
            let curve = out.add(&format!("CIRCLE('',#{frame},{})", real(p[0])));
            let edge = out.add(&format!("EDGE_CURVE('',#{v},#{v},#{curve},.T.)"));
            circles[k] = Some(edge);
            Some(edge)
        };
        let (c0, c1) = (circle(out, i), circle(out, j));
        let face = if dz.abs() <= eps {
            // Flat: a disc or a ring, its outward side down where the span runs outward.
            let plane_at = out.placement(rev.point(0.0, p0[1], 0.0), rev.axis, rev.xdir);
            let plane = out.add(&format!("PLANE('',#{plane_at})"));
            let mut loops: Vec<(f64, usize)> = Vec::with_capacity(2);
            if let Some(c) = c0 {
                loops.push((p0[0], out.edge_loop(&[(c, true)])));
            }
            if let Some(c) = c1 {
                loops.push((p1[0], out.edge_loop(&[(c, false)])));
            }
            loops.sort_by(|a, b| b.0.total_cmp(&a.0));
            let loops: Vec<usize> = loops.into_iter().map(|l| l.1).collect();
            out.face(&loops, plane, dr < 0.0)
        } else {
            let (v0, v1) = (vertex(out, i), vertex(out, j));
            let seam = out.line_edge(v0, v1, at(p0), at(p1));
            let mut edges = Vec::with_capacity(4);
            if let Some(c) = c0 {
                edges.push((c, true));
            }
            edges.push((seam, true));
            if let Some(c) = c1 {
                edges.push((c, false));
            }
            edges.push((seam, false));
            let edge_loop = out.edge_loop(&edges);
            let surface = if dr.abs() <= eps {
                let frame = out.placement(rev.point(0.0, p0[1], 0.0), rev.axis, rev.xdir);
                out.add(&format!(
                    "CYLINDRICAL_SURFACE('',#{frame},{})",
                    real((p0[0] + p1[0]) / 2.0)
                ))
            } else {
                // A cone: placed at its narrow end, its axis pointing to the wide end.
                let (narrow, wide) = if p0[0] < p1[0] { (p0, p1) } else { (p1, p0) };
                let towards = if wide[1] > narrow[1] { 1.0 } else { -1.0 };
                let frame = out.placement(
                    rev.point(0.0, narrow[1], 0.0),
                    scale3(rev.axis, towards),
                    rev.xdir,
                );
                let angle = dr.abs().atan2(dz.abs());
                out.add(&format!(
                    "CONICAL_SURFACE('',#{frame},{},{})",
                    real(narrow[0].max(0.0)),
                    real(angle)
                ))
            };
            // Cylinders and cones face away from their axis; the face's outward side does where
            // the span runs up the axis.
            out.face(&[edge_loop], surface, dz > 0.0)
        };
        faces.push(face);
    }
    Ok((faces, closed))
}

/// A shape written as its mesh: one flat triangle a face, edges shared. Closed (every edge used
/// once each way) it's a solid, turned to face outward.
fn mesh_faces(out: &mut Out, s: &Surface) -> Result<(Vec<usize>, bool), String> {
    let mesh: Mesh = s.mesh(1);
    // Corners at the same place are one vertex.
    let mut ids: HashMap<[u32; 3], usize> = HashMap::new();
    let mut points: Vec<P3> = Vec::new();
    let index: Vec<usize> = mesh
        .positions
        .chunks_exact(3)
        .map(|p| {
            let key = [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
            *ids.entry(key).or_insert_with(|| {
                points.push([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]);
                points.len() - 1
            })
        })
        .collect();
    let mut triangles: Vec<[usize; 3]> = Vec::with_capacity(mesh.indices.len() / 3);
    for t in mesh.indices.chunks_exact(3) {
        let Some(t) = t
            .iter()
            .map(|&i| index.get(i as usize).copied())
            .collect::<Option<Vec<usize>>>()
        else {
            continue;
        };
        let (a, b, c) = (t[0], t[1], t[2]);
        if a == b || b == c || a == c {
            continue;
        }
        triangles.push([a, b, c]);
    }
    if triangles.len() > MAX_FACETS {
        return Err(format!(
            "this shape is too detailed for a STEP file ({} triangles, at most {MAX_FACETS}); export it as .stl",
            triangles.len()
        ));
    }
    let mut directed: HashMap<(usize, usize), i32> = HashMap::new();
    for t in &triangles {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *directed.entry((a, b)).or_default() += 1;
        }
    }
    let closed = !triangles.is_empty()
        && directed
            .iter()
            .all(|(&(a, b), &n)| n == 1 && directed.get(&(b, a)) == Some(&1));
    let flip = closed && signed_volume(&mesh) < 0.0;

    let mut vertices: Vec<Option<usize>> = vec![None; points.len()];
    let mut edges: HashMap<(usize, usize), usize> = HashMap::new();
    let mut faces = Vec::with_capacity(triangles.len());
    for t in &triangles {
        let [a, b, c] = if flip { [t[0], t[2], t[1]] } else { *t };
        let (pa, pb, pc) = (points[a], points[b], points[c]);
        let normal = cross3(sub3(pb, pa), sub3(pc, pa));
        let (Some(normal), Some(along)) = (unit3(normal), unit3(sub3(pb, pa))) else {
            continue;
        };
        if norm3(cross3(sub3(pb, pa), sub3(pc, pa))) <= 1e-12 {
            continue;
        }
        let mut loop_edges = Vec::with_capacity(3);
        for (u, v) in [(a, b), (b, c), (c, a)] {
            let key = (u.min(v), u.max(v));
            let edge = match edges.get(&key) {
                Some(&e) => e,
                None => {
                    let mut vertex = |k: usize| -> usize {
                        if let Some(v) = vertices[k] {
                            return v;
                        }
                        let v = out.vertex(points[k]);
                        vertices[k] = Some(v);
                        v
                    };
                    let (v0, v1) = (vertex(key.0), vertex(key.1));
                    let e = out.line_edge(v0, v1, points[key.0], points[key.1]);
                    edges.insert(key, e);
                    e
                }
            };
            loop_edges.push((edge, u < v));
        }
        let edge_loop = out.edge_loop(&loop_edges);
        let frame = out.placement(pa, normal, along);
        let plane = out.add(&format!("PLANE('',#{frame})"));
        faces.push(out.face(&[edge_loop], plane, true));
    }
    Ok((faces, closed))
}

/// Twice the area a closed (r, z) polygon encloses, counter-clockwise positive.
fn signed_area(ring: &[P2]) -> f64 {
    (0..ring.len())
        .map(|k| {
            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum()
}

fn refs(ids: &[usize]) -> String {
    let list: Vec<String> = ids.iter().map(|i| format!("#{i}")).collect();
    format!("({})", list.join(","))
}

fn flag(b: bool) -> &'static str {
    if b { ".T." } else { ".F." }
}

/// A STEP real: always with a decimal point (`3.`, `-0.25`), to the nanometre / 1e-10 radian.
fn real(v: f64) -> String {
    if !v.is_finite() {
        return "0.".into();
    }
    let s = format!("{v:.10}");
    let s = s.trim_end_matches('0');
    match s {
        "-." | "." | "-0." => "0.".into(),
        _ => s.to_string(),
    }
}

/// A STEP string: quoted, `'` doubled, `\` escaped, anything not plain ASCII as `\X2\…\X0\`.
fn text(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\'' => out.push_str("''"),
            '\\' => out.push_str("\\\\"),
            ' '..='~' => out.push(c),
            c if c.is_control() => {}
            c => {
                out.push_str("\\X2\\");
                let mut units = [0u16; 2];
                for u in c.encode_utf16(&mut units) {
                    let _ = write!(out, "{u:04X}");
                }
                out.push_str("\\X0\\");
            }
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::curve::Curve;
    use crate::geom::Xform;
    use std::collections::HashSet;

    /// Checks the file's structure: header, data section, every entity defined once, every
    /// reference defined. Returns how many of each entity type there are.
    pub(crate) fn check(step: &str) -> HashMap<String, usize> {
        assert!(step.starts_with("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(("));
        assert!(step.contains(&format!("FILE_SCHEMA(('{SCHEMA}'));")));
        assert!(step.ends_with("ENDSEC;\nEND-ISO-10303-21;\n"));
        let data = step
            .split_once("DATA;\n")
            .unwrap()
            .1
            .strip_suffix("ENDSEC;\nEND-ISO-10303-21;\n")
            .unwrap();
        let mut defined = HashSet::new();
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut used = Vec::new();
        for line in data.lines() {
            assert!(line.ends_with(';'), "{line}");
            let (id, body) = line.split_once('=').unwrap();
            let id: usize = id.strip_prefix('#').unwrap().parse().unwrap();
            assert!(defined.insert(id), "#{id} defined twice");
            let name: String = body
                .trim_start_matches("( ")
                .chars()
                .take_while(|c| c.is_ascii_uppercase() || *c == '_' || c.is_ascii_digit())
                .collect();
            *counts.entry(name).or_default() += 1;
            // References are `#` and digits outside strings.
            let mut in_string = false;
            let mut chars = body.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\'' {
                    in_string = !in_string;
                } else if c == '#' && !in_string {
                    let mut n = String::new();
                    while let Some(d) = chars.peek().filter(|d| d.is_ascii_digit()) {
                        n.push(*d);
                        chars.next();
                    }
                    if !n.is_empty() {
                        used.push(n.parse::<usize>().unwrap());
                    }
                }
            }
            assert!(!in_string, "unclosed string: {line}");
        }
        for r in used {
            assert!(defined.contains(&r), "#{r} is used but not defined");
        }
        counts
    }

    /// Every cartesian point in the file.
    fn points(step: &str) -> Vec<P3> {
        step.lines()
            .filter_map(|l| l.split_once("CARTESIAN_POINT('',(")?.1.strip_suffix("));"))
            .map(|xyz| {
                let v: Vec<f64> = xyz.split(',').map(|c| c.parse().unwrap()).collect();
                [v[0], v[1], v[2]]
            })
            .collect()
    }

    fn revolved(points: &[[f64; 2]]) -> Surface {
        let profile = Curve::new(points.iter().map(|p| [p[0], 0.0, p[1]]).collect(), false);
        Surface::revolve(&profile, 32).unwrap()
    }

    #[test]
    fn reals_and_strings_are_written_the_step_way() {
        assert_eq!(real(3.0), "3.");
        assert_eq!(real(-0.25), "-0.25");
        assert_eq!(real(-0.0), "0.");
        assert_eq!(real(1e-12), "0.");
        assert_eq!(real(f64::NAN), "0.");
        assert_eq!(real(12.3456789012), "12.3456789012");
        assert_eq!(text("it's"), "'it''s'");
        assert_eq!(text("a\\b"), "'a\\\\b'");
        assert_eq!(text("Lamp — main"), "'Lamp \\X2\\2014\\X0\\ main'");
        assert_eq!(text("ø"), "'\\X2\\00F8\\X0\\'");
    }

    #[test]
    fn a_tube_wall_is_an_exact_solid_of_two_cylinders_and_two_rings() {
        // A straight tube: radius 20, 50 tall, 2 thick.
        let tube = revolved(&[[20.0, 0.0], [20.0, 50.0]]).thicken(2.0).unwrap();
        let step = write(&[&tube], "Tube", "tube.step", &["EasyCAD: tube".into()]).unwrap();
        let counts = check(&step);
        assert_eq!(counts["CYLINDRICAL_SURFACE"], 2);
        assert_eq!(counts["PLANE"], 2);
        assert_eq!(counts["ADVANCED_FACE"], 4);
        assert_eq!(counts["CIRCLE"], 4);
        assert_eq!(counts["CLOSED_SHELL"], 1);
        assert_eq!(counts["MANIFOLD_SOLID_BREP"], 1);
        assert_eq!(counts["ADVANCED_BREP_SHAPE_REPRESENTATION"], 1);
        assert!(!counts.contains_key("OPEN_SHELL"));
        assert!(step.contains("CYLINDRICAL_SURFACE('',#") && step.contains(",21.);"));
        assert!(step.contains(",19.);"));
        // Bounds: radius 19 to 21, height 0 to 50 (the end faces square across the wall).
        let pts = points(&step);
        let r = |p: &P3| p[0].hypot(p[1]);
        assert!(pts.iter().all(|p| p[2] >= -1e-9 && p[2] <= 50.0 + 1e-9));
        assert!(pts.iter().any(|p| (r(p) - 21.0).abs() < 1e-9));
        assert!(pts.iter().any(|p| (r(p) - 19.0).abs() < 1e-9));
        assert!(pts.iter().all(|p| r(p) <= 21.0 + 1e-9));
        // Deterministic.
        assert_eq!(
            step,
            write(&[&tube], "Tube", "tube.step", &["EasyCAD: tube".into()]).unwrap()
        );
    }

    #[test]
    fn a_cone_wall_has_cones_with_their_true_angle() {
        // From radius 10 at the bottom to 30 at 20 up: 45° out.
        let cone = revolved(&[[10.0, 0.0], [30.0, 20.0]]).thicken(1.0).unwrap();
        let step = write(&[&cone], "Cone", "cone.step", &[]).unwrap();
        let counts = check(&step);
        // Inside and outside, and the ends, square across the wall: cones at 45° too.
        assert_eq!(counts["CONICAL_SURFACE"], 4);
        assert_eq!(counts["ADVANCED_FACE"], 4);
        let angle = std::f64::consts::FRAC_PI_4;
        assert_eq!(step.matches(&format!(",{})", real(angle))).count(), 4);
        // The ends are square across the wall, so it reaches half the thickness past each.
        let pts = points(&step);
        let h = 0.5 / 2f64.sqrt();
        let (lo, hi) = pts
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[2]), b.max(p[2])));
        assert!(
            (lo + h).abs() < 1e-9 && (hi - 20.0 - h).abs() < 1e-9,
            "{lo} {hi}"
        );
    }

    #[test]
    fn a_closed_end_reaching_the_axis_is_a_disc() {
        // A cup: a flat bottom from the axis out to 10, then straight up to 20.
        let cup = revolved(&[[0.0, 0.0], [10.0, 0.0], [10.0, 20.0]])
            .thicken(2.0)
            .unwrap();
        let rev = cup.revolution().unwrap();
        assert!(rev.closed && rev.profile.iter().all(|p| p[0] >= 0.0));
        let step = write(&[&cup], "Cup", "cup.step", &[]).unwrap();
        let counts = check(&step);
        // Bottom and inside floor (discs), the rim (a ring), outside and inside (cylinders).
        assert_eq!(counts["PLANE"], 3);
        assert_eq!(counts["CYLINDRICAL_SURFACE"], 2);
        assert_eq!(counts["CLOSED_SHELL"], 1);
    }

    #[test]
    fn a_surface_without_thickness_is_an_open_surface_model() {
        let wall = revolved(&[[10.0, 0.0], [10.0, 5.0], [20.0, 15.0]]);
        let step = write(&[&wall], "Wall", "wall.step", &[]).unwrap();
        let counts = check(&step);
        assert_eq!(counts["OPEN_SHELL"], 1);
        assert_eq!(counts["SHELL_BASED_SURFACE_MODEL"], 1);
        assert_eq!(counts["MANIFOLD_SURFACE_SHAPE_REPRESENTATION"], 1);
        assert_eq!(counts["ADVANCED_FACE"], 2);
        // Capped, it's a solid: two discs close it.
        let mut capped = wall.clone();
        capped.capped = true;
        let counts = check(&write(&[&capped], "Wall", "wall.step", &[]).unwrap());
        assert_eq!(counts["ADVANCED_FACE"], 4);
        assert_eq!(counts["MANIFOLD_SOLID_BREP"], 1);
    }

    #[test]
    fn moved_turned_and_mirrored_it_stays_exact() {
        let wall = revolved(&[[10.0, 0.0], [20.0, 15.0]]).thicken(1.0).unwrap();
        let moved = wall
            .transformed(&Xform::translate(5.0, -3.0, 2.0))
            .transformed(&Xform::placed(40.0, true))
            .transformed(&Xform::rotate_axis(30.0, [1.0, 1.0, 0.0], [0.0; 3]).unwrap())
            .transformed(&Xform::mirror([0.0; 3], [1.0, 0.0, 0.0]).unwrap())
            .transformed(&Xform::scale(2.0, 2.0, 2.0));
        let rev = moved.revolution().expect("still a revolution");
        assert!((rev.profile[0][0] - wall.revolution().unwrap().profile[0][0] * 2.0).abs() < 1e-9);
        let counts = check(&write(&[&moved], "Moved", "m.step", &[]).unwrap());
        assert_eq!(counts["CONICAL_SURFACE"], 4);
        // Stretched one way only, it isn't round any more: written as its mesh.
        let squashed = wall.transformed(&Xform::scale(1.0, 2.0, 1.0));
        assert!(squashed.revolution().is_none());
        let counts = check(&write(&[&squashed], "Squashed", "s.step", &[]).unwrap());
        assert!(!counts.contains_key("CONICAL_SURFACE"));
        assert_eq!(counts["CLOSED_SHELL"], 1);
        assert_eq!(counts["ADVANCED_FACE"], squashed.mesh(1).indices.len() / 3);
    }

    #[test]
    fn a_loft_is_written_as_its_mesh_and_closed_when_solid() {
        let square = Curve::polygon(4, 10.0);
        let top = square.transformed(&Xform::translate(0.0, 0.0, 10.0));
        let mut box_ = Surface::loft(&[&square, &top]).unwrap();
        let open = check(&write(&[&box_], "Box", "b.step", &[]).unwrap());
        assert_eq!(open["OPEN_SHELL"], 1);
        box_.capped = true;
        let step = write(&[&box_], "Box", "b.step", &[]).unwrap();
        let counts = check(&step);
        assert_eq!(counts["CLOSED_SHELL"], 1);
        assert_eq!(counts["MANIFOLD_SOLID_BREP"], 1);
        // 4 sides of 2 triangles, 2 caps of 4 (a fan round the centre).
        assert_eq!(counts["ADVANCED_FACE"], 16);
        // A thickened loft: a closed solid too.
        let wall = Surface::loft(&[&square, &top])
            .unwrap()
            .thicken(1.0)
            .unwrap();
        let counts = check(&write(&[&wall], "Wall", "w.step", &[]).unwrap());
        assert_eq!(counts["CLOSED_SHELL"], 1);
    }

    #[test]
    fn nothing_to_write_is_an_error_not_a_crash() {
        assert!(write(&[], "x", "x.step", &[]).is_err());
        let empty = Surface {
            sections: vec![],
            closed: true,
            capped: false,
            straight: false,
            looped: false,
            revolution: None,
        };
        assert!(write(&[&empty], "x", "x.step", &[]).is_err());
        // Too many triangles for a file anyone wants: a message, not a 100 MB string.
        let ring = |z: f64| -> Vec<P3> {
            (0..400)
                .map(|j| {
                    let a = std::f64::consts::TAU * j as f64 / 400.0;
                    [10.0 * a.cos(), 10.0 * a.sin(), z]
                })
                .collect()
        };
        let dense = Surface {
            sections: (0..100).map(|i| ring(i as f64)).collect(),
            ..empty
        };
        assert!(
            write(&[&dense], "x", "x.step", &[])
                .unwrap_err()
                .contains("too detailed")
        );
        // A stale revolution (sections reshaped without it) is ignored, not trusted.
        let mut wall = revolved(&[[10.0, 0.0], [10.0, 20.0]]);
        wall.sections[1][3][0] += 1.0;
        assert!(wall.revolution().is_none());
    }
}
