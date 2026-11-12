//! Surfaces lofted through section curves, and their preview meshes.

use crate::curve::Curve;
use crate::field::Field;
use crate::geom::{
    P2, P3, Xform, add3, catmull_rom, cross3, dist3, dot3, lerp3, norm3, scale3, signed_area_xy,
    sub3, unit3,
};
use std::rc::Rc;

/// The most vertices one preview mesh may have; denser lofts get fewer rings.
const MAX_MESH_VERTICES: usize = 600_000;

/// A lofted surface: sections bottom to top, smoothly interpolated in between.
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    /// All sections have the same number of points, and point `j` of every section lies on
    /// the same line along the surface (that's how the loft pairs them up).
    pub sections: Vec<Vec<P3>>,
    /// The sections are closed loops (a tube rather than a strip).
    pub closed: bool,
    /// Both ends are closed with flat caps, making a solid.
    pub capped: bool,
    /// Drawn straight from section to section rather than smoothly (a surface of revolution:
    /// its sections are its profile's points, and a smooth curve through them would overshoot
    /// where a long straight span meets short ones). Only the 3D view's mesh reads it.
    pub straight: bool,
    /// The last section joins back onto the first: a wall with a thickness (`thicken`), a solid
    /// whose sections run up its outside and back down its inside.
    pub looped: bool,
    /// For a surface of revolution, its profile and axis, so a STEP file can carry its exact
    /// round faces. Kept through moves, turns, mirrors and uniform scales; [`Surface::revolution`]
    /// checks it against the sections, which stay the truth.
    pub revolution: Option<Rc<Revolution>>,
}

/// A surface of revolution's exact shape: `profile` as (distance from the axis, height along
/// it), revolved about the axis through `origin` along `axis`. Angle 0 is along `xdir`, the
/// angle growing counter-clockwise seen from the axis' tip.
#[derive(Debug, Clone, PartialEq)]
pub struct Revolution {
    pub profile: Vec<P2>,
    /// The profile is a closed loop, counter-clockwise in (r, z): it encloses a solid ring.
    pub closed: bool,
    pub origin: P3,
    /// Unit length.
    pub axis: P3,
    /// Unit length, perpendicular to `axis`.
    pub xdir: P3,
}

impl Revolution {
    /// The point at distance `r` from the axis, height `z` along it, `angle` radians round.
    pub fn point(&self, r: f64, z: f64, angle: f64) -> P3 {
        let y = cross3(self.axis, self.xdir);
        let (s, c) = angle.sin_cos();
        add3(
            add3(self.origin, scale3(self.axis, z)),
            add3(scale3(self.xdir, r * c), scale3(y, r * s)),
        )
    }

    /// The rings of the surface: one per profile point, `samples` points round.
    fn rings(&self, samples: usize) -> Vec<Vec<P3>> {
        self.profile
            .iter()
            .map(|&[r, z]| {
                (0..samples)
                    .map(|j| self.point(r, z, std::f64::consts::TAU * j as f64 / samples as f64))
                    .collect()
            })
            .collect()
    }

    /// Moved by `x`, if that keeps it a surface of revolution (a move, turn, mirror or uniform
    /// scale). A mirror maps it onto itself turned the other way round, the same shape, so the
    /// frame stays right-handed.
    fn transformed(&self, x: &Xform) -> Option<Revolution> {
        let y = cross3(self.axis, self.xdir);
        let (a, xd, yd) = (
            x.apply_vector(self.axis),
            x.apply_vector(self.xdir),
            x.apply_vector(y),
        );
        let s = norm3(a);
        if !(s.is_finite() && s > 1e-12) {
            return None;
        }
        let tol = 1e-9 * s;
        let similar = (norm3(xd) - s).abs() <= tol
            && (norm3(yd) - s).abs() <= tol
            && dot3(a, xd).abs() <= tol * s
            && dot3(a, yd).abs() <= tol * s
            && dot3(xd, yd).abs() <= tol * s;
        if !similar {
            return None;
        }
        Some(Revolution {
            profile: self.profile.iter().map(|p| [p[0] * s, p[1] * s]).collect(),
            closed: self.closed,
            origin: x.apply(self.origin),
            axis: unit3(a)?,
            xdir: unit3(xd)?,
        })
    }

    /// Whether every point of `sections` lies on this revolution: section `i` at the profile's
    /// point `i` (its distance from the axis and its height along it).
    fn matches(&self, sections: &[Vec<P3>]) -> bool {
        if sections.len() != self.profile.len() {
            return false;
        }
        let size = self
            .profile
            .iter()
            .fold(1.0f64, |m, p| m.max(p[0].abs()).max(p[1].abs()))
            .max(norm3(self.origin));
        let tol = 1e-6 * size;
        sections.iter().zip(&self.profile).all(|(ring, &[r, z])| {
            ring.iter().all(|p| {
                let d = sub3(*p, self.origin);
                let h = dot3(d, self.axis);
                let radial = norm3(sub3(d, scale3(self.axis, h)));
                (h - z).abs() <= tol && (radial - r).abs() <= tol
            })
        })
    }
}

/// Triangle mesh for the 3D view: `positions` as x,y,z triples, `indices` as triangles.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mesh {
    pub positions: Vec<f32>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn vertex_count(&self) -> usize {
        self.positions.len() / 3
    }

    /// Adds another mesh's triangles to this one.
    pub fn append(&mut self, other: &Mesh) {
        let offset = self.vertex_count() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.indices
            .extend(other.indices.iter().map(|i| i + offset));
    }
}

impl Surface {
    /// Skins a surface through `curves` in order. Sections with different point counts are
    /// resampled evenly to the largest count, starting at each section's first point (its seam).
    pub fn loft(curves: &[&Curve]) -> Result<Surface, String> {
        if curves.len() < 2 {
            return Err(format!("needs at least 2 sections, got {}", curves.len()));
        }
        let closed = curves.first().is_some_and(|c| c.closed);
        if curves.iter().any(|c| c.closed != closed) {
            return Err("the sections must be all closed or all open".into());
        }
        if let Some(i) = curves.iter().position(|c| c.points.len() < 2) {
            return Err(format!("section {} has fewer than 2 points", i + 1));
        }
        let m = curves.iter().map(|c| c.points.len()).max().unwrap_or(0);
        let sections = curves
            .iter()
            .map(|c| {
                if c.points.len() == m {
                    c.points.clone()
                } else {
                    c.resample(m).points
                }
            })
            .collect();
        Ok(Surface {
            sections,
            closed,
            capped: false,
            straight: false,
            looped: false,
            revolution: None,
        })
    }

    /// The curve swept straight up by `height` (Grasshopper's Extrude along Z).
    pub fn extrude(curve: &Curve, height: f64) -> Result<Surface, String> {
        if curve.points.len() < 2 {
            return Err("the curve needs at least 2 points".into());
        }
        let top = curve.transformed(&Xform::translate(0.0, 0.0, height));
        Surface::loft(&[curve, &top])
    }

    /// A surface of revolution about the Z axis (Grasshopper's Revolution): `profile` is drawn
    /// in the XZ plane, x being the distance from the axis and z the height. Each profile
    /// point becomes a horizontal circle of `samples` points, so the result lofts, caps and
    /// prints in vase mode like any other wall.
    pub fn revolve(profile: &Curve, samples: usize) -> Result<Surface, String> {
        if profile.closed {
            return Err("the profile must be an open curve (from the axis side to the rim)".into());
        }
        if profile.points.len() < 2 {
            return Err("the profile needs at least 2 points".into());
        }
        if let Some(p) = profile.points.iter().find(|p| p[0] < 0.0) {
            return Err(format!(
                "the profile must stay on the +X side of the axis (x ≥ 0), but reaches x = {}",
                easycad_lang::format_number((p[0] * 1000.0).round() / 1000.0)
            ));
        }
        let n = samples.max(3);
        // A point repeating the one before adds a span of no height: left out.
        let mut points: Vec<P3> = Vec::with_capacity(profile.points.len());
        for p in &profile.points {
            if points.last().is_none_or(|q| dist3(*q, *p) > 1e-9) {
                points.push(*p);
            }
        }
        if points.len() < 2 {
            return Err("the profile needs at least 2 different points".into());
        }
        let revolution = Revolution {
            profile: points.iter().map(|p| [p[0], p[2]]).collect(),
            closed: false,
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            xdir: [1.0, 0.0, 0.0],
        };
        Ok(Surface {
            sections: revolution.rings(n),
            closed: true,
            capped: false,
            straight: true,
            looped: false,
            revolution: Some(Rc::new(revolution)),
        })
    }

    /// The exact surface of revolution this is, if it is one (checked against the sections, so
    /// a step that reshaped them can't leave a stale one behind).
    pub fn revolution(&self) -> Option<&Revolution> {
        self.revolution
            .as_deref()
            .filter(|r| r.matches(&self.sections))
    }

    /// The surface given a thickness: a wall `thickness` thick, centred on the surface (as a
    /// printed bead is on its path), closed across its ends: a solid. A surface of revolution is
    /// thickened exactly, its profile offset to both sides; any other tube is offset point by
    /// point along the surface's normal.
    pub fn thicken(&self, thickness: f64) -> Result<Surface, String> {
        if !(thickness > 0.0 && thickness.is_finite()) {
            return Err(format!("`thickness` must be more than 0, got {thickness}"));
        }
        if self.capped || self.looped {
            return Err("is already a solid; thicken the surface it was made from".into());
        }
        if !self.closed {
            return Err(
                "needs a surface of closed sections (a wall all the way round, from loft or revolve)"
                    .into(),
            );
        }
        let m = self.samples();
        if self.sections.len() < 2 || m < 3 {
            return Err("needs a surface with at least 2 sections of 3 or more points".into());
        }
        if let Some(rev) = self.revolution().filter(|r| !r.closed) {
            let revolution = Revolution {
                profile: offset_profile(&rev.profile, thickness / 2.0)?,
                closed: true,
                ..rev.clone()
            };
            return Ok(Surface {
                sections: revolution.rings(m),
                closed: true,
                capped: false,
                straight: true,
                looped: true,
                revolution: Some(Rc::new(revolution)),
            });
        }
        // Any other tube: each point moved half the thickness out and in along the normal, the
        // smooth loft first taken at 4 rings a span (the solid is drawn straight between them).
        let dense: Vec<Vec<P3>>;
        let sections = if self.straight {
            &self.sections
        } else {
            let n = self.sections.len();
            let rings = (n - 1) * 4 + 1;
            if rings.saturating_mul(m) > MAX_MESH_VERTICES {
                return Err("the surface is too detailed to thicken".into());
            }
            dense = (0..rings)
                .map(|r| {
                    let v = r as f64 / 4.0;
                    (0..m).map(|j| self.point_at(v, j)).collect()
                })
                .collect();
            &dense
        };
        let n = sections.len();
        let get = |i: usize, j: usize| {
            sections
                .get(i)
                .and_then(|s| s.get(j % m))
                .copied()
                .unwrap_or([0.0; 3])
        };
        // Which side is out doesn't matter: both are offset, and the mesh faces out by volume.
        let half = 0.5 * thickness;
        let mut outer = Vec::with_capacity(2 * n);
        let mut inner = Vec::with_capacity(n);
        for i in 0..n {
            let (lo, hi) = (i.saturating_sub(1), (i + 1).min(n - 1));
            let mut out_ring = Vec::with_capacity(m);
            let mut in_ring = Vec::with_capacity(m);
            for j in 0..m {
                let along = sub3(get(i, j + 1), get(i, j + m - 1));
                let across = sub3(get(hi, j), get(lo, j));
                let normal = unit3(cross3(along, across)).ok_or_else(|| {
                    format!(
                        "the surface has no direction at section {}, point {}",
                        i + 1,
                        j + 1
                    )
                })?;
                let p = get(i, j);
                out_ring.push(add3(p, scale3(normal, half)));
                in_ring.push(sub3(p, scale3(normal, half)));
            }
            outer.push(out_ring);
            inner.push(in_ring);
        }
        inner.reverse();
        outer.extend(inner);
        Ok(Surface {
            sections: outer,
            closed: true,
            capped: false,
            straight: true,
            looped: true,
            revolution: None,
        })
    }

    /// Flat layers through the surface (Grasshopper's Contour), `layer` apart: the first at its
    /// bottom, then one per layer up to its top, the height snapped to whole layers with the
    /// surface stretched to fit, exactly as `vase_spiral` prints it. Sections must stack upward.
    /// A `layer` that varies (a field, read on the seam at each height) steps up layer by layer
    /// instead, each step as tall as the field halfway up it, the steps fitted to end at the top.
    pub fn contour(
        &self,
        layer: &Field,
        pace: Pace,
        max_points: usize,
    ) -> Result<Vec<Curve>, String> {
        let fixed = layer.as_constant();
        if let Some(layer) = fixed
            && !(layer > 0.0 && layer.is_finite())
        {
            return Err(format!("`layer` must be more than 0, got {layer}"));
        }
        if self.looped {
            return Err(LOOPED.into());
        }
        pace.check()?;
        let n = self.sections.len();
        let m = self.samples();
        if n < 2 || m < 2 {
            return Err("needs a surface with at least 2 sections".into());
        }
        let levels: Vec<f64> = self
            .sections
            .iter()
            .map(|s| s.iter().map(|p| p[2]).sum::<f64>() / s.len().max(1) as f64)
            .collect();
        if levels.iter().any(|z| !z.is_finite()) {
            return Err("the surface has points at an invalid height".into());
        }
        if let Some(i) = levels.windows(2).position(|w| w[1] <= w[0]) {
            return Err(format!(
                "the sections must be stacked upward, but section {} is not above section {}",
                i + 2,
                i + 1
            ));
        }
        let (z0, z1) = (levels[0], levels[n - 1]);
        let height = z1 - z0;
        let v_at = |z: f64| -> f64 {
            let i = levels
                .partition_point(|l| *l <= z)
                .saturating_sub(1)
                .min(n - 2);
            let (lo, hi) = (levels[i], levels[i + 1]);
            i as f64 + ((z - lo) / (hi - lo)).clamp(0.0, 1.0)
        };
        let Some(layer) = fixed.filter(|_| pace.amount == 0.0) else {
            // The layer height at height z: the field on the seam there.
            let layer_at = |z: f64| -> Result<f64, String> {
                if let Some(l) = fixed {
                    return Ok(l);
                }
                let p = self.point_at(v_at(z.clamp(z0, z1)), 0);
                let l = layer.at([p[0], p[1], z]);
                if l > 0.0 && l.is_finite() {
                    Ok(l)
                } else {
                    Err(format!(
                        "`layer` must be more than 0 everywhere, but it's {} at height {}",
                        easycad_lang::format_number(l),
                        easycad_lang::format_number((z * 100.0).round() / 100.0)
                    ))
                }
            };
            return self.stepped_contour(&layer_at, pace, &v_at, z0, z1, max_points);
        };
        let layers = (height / layer).round().max(1.0) as usize;
        if (layers + 1).saturating_mul(m) > max_points {
            return Err(format!(
                "this needs too many points ({} layers of {m}); use a larger layer height",
                layers + 1
            ));
        }
        Ok((0..=layers)
            .map(|k| {
                let v = v_at(z0 + height * k as f64 / layers as f64);
                let z = z0 + layer * k as f64;
                let points = (0..m)
                    .map(|j| {
                        let p = self.point_at(v, j);
                        [p[0], p[1], z]
                    })
                    .collect();
                Curve::new(points, self.closed)
            })
            .collect())
    }

    /// Layers stepped up one at a time, each step `layer(z)` tall, or with `pace` closer where
    /// the wall leans (EasyGCode's overhang-paced Z): `layer(z) · (1 − pace · (1 − cos a))`, `a`
    /// being the lean of the seam (point 0) at `z` (cos a no less than 0.05), the pace scaled by
    /// `(a / pivot)^steep` when `steep` is set. Each step is taken as tall as it is halfway up it,
    /// and the steps are then scaled to end exactly at the top.
    fn stepped_contour(
        &self,
        layer: &dyn Fn(f64) -> Result<f64, String>,
        pace: Pace,
        v_at: &dyn Fn(f64) -> f64,
        z0: f64,
        z1: f64,
        max_points: usize,
    ) -> Result<Vec<Curve>, String> {
        let m = self.samples();
        let seam = |z: f64| self.point_at(v_at(z.clamp(z0, z1)), 0);
        let cos_at = |z: f64, layer: f64| -> f64 {
            let eps = (0.02 * layer).max(1e-3);
            let (a, b) = (seam(z - eps), seam(z + eps));
            let run = (b[0] - a[0]).hypot(b[1] - a[1]);
            (2.0 * eps).atan2(run).sin().max(0.05) // cos of the angle from vertical
        };
        let step = |z: f64| -> Result<f64, String> {
            let layer = layer(z)?;
            Ok(if pace.amount > 0.0 {
                let c = cos_at(z, layer);
                (layer * (1.0 - pace.at(c) * (1.0 - c))).max(0.05 * layer)
            } else {
                layer
            })
        };
        let mut zs = vec![z0];
        let mut z = z0;
        while z < z1 - 1e-9 {
            let g = step(z)?;
            z += step(z + g / 2.0)?;
            zs.push(z);
            if zs.len().saturating_mul(m) > max_points {
                return Err(format!(
                    "this needs too many points (over {} layers of {m}); use a larger layer height",
                    zs.len()
                ));
            }
        }
        // Scaled to end exactly at the top.
        let k = (z1 - z0) / (z - z0).max(1e-12);
        Ok(zs
            .into_iter()
            .map(|z| {
                let z = z0 + (z - z0) * k;
                let v = v_at(z);
                let points = (0..m)
                    .map(|j| {
                        let p = self.point_at(v, j);
                        [p[0], p[1], z]
                    })
                    .collect();
                Curve::new(points, self.closed)
            })
            .collect())
    }

    /// Points per section.
    pub fn samples(&self) -> usize {
        self.sections.first().map_or(0, Vec::len)
    }

    pub fn point_count(&self) -> usize {
        self.sections.len() * self.samples()
    }

    /// Point `j` of the section at `v` (0 = first section, n − 1 = last), passing smoothly
    /// through every section (Catmull-Rom across the sections).
    pub fn point_at(&self, v: f64, j: usize) -> P3 {
        let n = self.sections.len();
        let get = |i: usize| {
            self.sections
                .get(i)
                .and_then(|s| s.get(j))
                .copied()
                .unwrap_or([0.0; 3])
        };
        if n < 2 {
            return get(0);
        }
        let v = if v.is_finite() {
            v.clamp(0.0, (n - 1) as f64)
        } else {
            0.0
        };
        let i = (v.floor() as usize).min(n - 2);
        let t = v - i as f64;
        let (p1, p2) = (get(i), get(i + 1));
        // Past the ends, continue the first and last spans straight.
        let p0 = if i == 0 { reflect(p2, p1) } else { get(i - 1) };
        let p3 = if i + 2 >= n {
            reflect(p1, p2)
        } else {
            get(i + 2)
        };
        catmull_rom(p0, p1, p2, p3, t)
    }

    /// Like `point_at`, but straight from section to section.
    fn straight_at(&self, v: f64, j: usize) -> P3 {
        let n = self.sections.len();
        let get = |i: usize| {
            self.sections
                .get(i)
                .and_then(|s| s.get(j))
                .copied()
                .unwrap_or([0.0; 3])
        };
        if n < 2 || !v.is_finite() {
            return get(0);
        }
        let v = v.clamp(0.0, (n - 1) as f64);
        let i = (v.floor() as usize).min(n - 2);
        lerp3(get(i), get(i + 1), v - i as f64)
    }

    pub fn transformed(&self, x: &Xform) -> Surface {
        let mirrors = x.mirrors() && self.closed;
        let sections = self
            .sections
            .iter()
            .map(|s| {
                let mut out: Vec<P3> = s.iter().map(|p| x.apply(*p)).collect();
                if mirrors && let Some(rest) = out.get_mut(1..) {
                    rest.reverse();
                }
                out
            })
            .collect();
        Surface {
            sections,
            closed: self.closed,
            capped: self.capped,
            straight: self.straight,
            looped: self.looped,
            revolution: self
                .revolution
                .as_ref()
                .and_then(|r| r.transformed(x))
                .map(Rc::new),
        }
    }

    /// The preview mesh: `per_span` rings between neighbouring sections, plus flat caps on a
    /// solid. Triangles face outward.
    pub fn mesh(&self, per_span: usize) -> Mesh {
        let n = self.sections.len();
        let m = self.samples();
        if n < 2 || m < 2 {
            return Mesh::default();
        }
        let mut rings = (n - 1) * per_span.max(1) + 1;
        if rings * m > MAX_MESH_VERTICES {
            rings = (MAX_MESH_VERTICES / m).max(2);
        }
        let outward = !self.closed
            || self
                .sections
                .first()
                .is_none_or(|s| signed_area_xy(s) >= 0.0);

        let mut mesh = Mesh::default();
        mesh.positions.reserve(rings * m * 3);
        for r in 0..rings {
            let v = (n - 1) as f64 * r as f64 / (rings - 1) as f64;
            for j in 0..m {
                let p = if self.straight {
                    self.straight_at(v, j)
                } else {
                    self.point_at(v, j)
                };
                mesh.positions.extend(p.map(|c| c as f32));
            }
        }
        let at = |ring: usize, j: usize| (ring * m + j % m) as u32;
        let around = if self.closed { m } else { m - 1 };
        // A looped solid also joins its last ring back onto its first.
        let spans = if self.looped { rings } else { rings - 1 };
        for r in 0..spans {
            let next = (r + 1) % rings;
            for j in 0..around {
                let (a, b, c, d) = (at(r, j), at(r, j + 1), at(next, j + 1), at(next, j));
                if outward {
                    mesh.indices.extend([a, b, c, a, c, d]);
                } else {
                    mesh.indices.extend([a, c, b, a, d, c]);
                }
            }
        }
        if self.capped && self.closed {
            for (ring, top) in [(0, false), (rings - 1, true)] {
                // Separate rim vertices, so the caps shade flat instead of rounding the edge.
                let start = mesh.vertex_count() as u32;
                let Some(rim) = mesh
                    .positions
                    .get(ring * m * 3..(ring + 1) * m * 3)
                    .map(<[f32]>::to_vec)
                else {
                    continue;
                };
                let mut center = [0.0f64; 3];
                for p in rim.chunks_exact(3) {
                    for k in 0..3 {
                        center[k] += f64::from(p[k]) / m as f64;
                    }
                }
                mesh.positions.extend(rim);
                mesh.positions.extend(center.map(|c| c as f32));
                let mid = start + m as u32;
                for j in 0..m {
                    let (p, q) = (start + j as u32, start + ((j + 1) % m) as u32);
                    if top == outward {
                        mesh.indices.extend([mid, p, q]);
                    } else {
                        mesh.indices.extend([mid, q, p]);
                    }
                }
            }
        }
        // A looped solid faces outward whichever way it was turned: its volume is positive.
        if self.looped && signed_volume(&mesh) < 0.0 {
            for t in mesh.indices.chunks_exact_mut(3) {
                t.swap(1, 2);
            }
        }
        mesh
    }
}

/// What `contour` and `vase_spiral` say about a solid wall.
pub const LOOPED: &str =
    "is a solid wall (from thicken); slice the surface it was thickened from instead";

/// The volume a closed mesh encloses: positive when its triangles face outward.
pub fn signed_volume(mesh: &Mesh) -> f64 {
    let p = |i: u32| -> P3 {
        let k = i as usize * 3;
        match mesh.positions.get(k..k + 3) {
            Some(q) => [f64::from(q[0]), f64::from(q[1]), f64::from(q[2])],
            None => [0.0; 3],
        }
    };
    mesh.indices
        .chunks_exact(3)
        .map(|t| dot3(p(t[0]), cross3(p(t[1]), p(t[2]))) / 6.0)
        .sum()
}

/// A profile (distance from the axis, height) given a thickness: offset `half` to each side,
/// along its normals (mitred at its corners), closed straight across both ends, cut at the axis,
/// and turned counter-clockwise. Refuses a thickness that would fold the wall over itself (a
/// bend tighter than half the thickness).
pub fn offset_profile(profile: &[P2], half: f64) -> Result<Vec<P2>, String> {
    let mut p: Vec<P2> = Vec::with_capacity(profile.len());
    for q in profile {
        if !(q[0].is_finite() && q[1].is_finite()) {
            return Err("the profile has a point that isn't a number".into());
        }
        if p.last()
            .is_none_or(|l| (l[0] - q[0]).hypot(l[1] - q[1]) > 1e-9)
        {
            p.push(*q);
        }
    }
    if p.len() < 2 {
        return Err("the profile needs at least 2 different points".into());
    }
    // Each span's normal, to its right going along it.
    let normals: Vec<P2> = p
        .windows(2)
        .map(|w| {
            let (dr, dz) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
            let len = dr.hypot(dz).max(1e-300);
            [dz / len, -dr / len]
        })
        .collect();
    let last = p.len() - 1;
    let side = |h: f64, out: &mut Vec<P2>, i: usize| {
        let at = |n: P2, k: f64| [p[i][0] + n[0] * k, p[i][1] + n[1] * k];
        if i == 0 || i == last {
            out.push(at(normals[i.min(last - 1)], h));
            return;
        }
        let (a, b) = (normals[i - 1], normals[i]);
        let mid = [a[0] + b[0], a[1] + b[1]];
        let len = mid[0].hypot(mid[1]);
        let cos = if len > 1e-9 {
            (mid[0] * b[0] + mid[1] * b[1]) / len
        } else {
            0.0
        };
        if cos > 0.25 {
            out.push(at([mid[0] / len, mid[1] / len], h / cos));
        } else {
            // Nearly turning back on itself: a bevel instead of a mitre miles out.
            out.push(at(a, h));
            out.push(at(b, h));
        }
    };
    let mut ring: Vec<P2> = Vec::with_capacity(2 * p.len() + 4);
    for i in 0..p.len() {
        side(half, &mut ring, i);
    }
    for i in (0..p.len()).rev() {
        side(-half, &mut ring, i);
    }
    // Cut at the axis: nothing reaches past it.
    let mut cut: Vec<P2> = Vec::with_capacity(ring.len() + 4);
    for (k, a) in ring.iter().enumerate() {
        let b = ring[(k + 1) % ring.len()];
        if a[0] >= 0.0 {
            cut.push(*a);
        }
        if (a[0] >= 0.0) != (b[0] >= 0.0) {
            let t = a[0] / (a[0] - b[0]);
            cut.push([0.0, a[1] + (b[1] - a[1]) * t]);
        }
    }
    let mut out: Vec<P2> = Vec::with_capacity(cut.len());
    for q in cut {
        let q = [if q[0].abs() < 1e-9 { 0.0 } else { q[0] }, q[1]];
        if out
            .last()
            .is_none_or(|l| (l[0] - q[0]).hypot(l[1] - q[1]) > 1e-9)
        {
            out.push(q);
        }
    }
    while out.len() > 1
        && out
            .first()
            .zip(out.last())
            .is_some_and(|(a, b)| (a[0] - b[0]).hypot(a[1] - b[1]) <= 1e-9)
    {
        out.pop();
    }
    let area: f64 = (0..out.len())
        .map(|k| {
            let (a, b) = (out[k], out[(k + 1) % out.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        / 2.0;
    if out.len() < 3 || area.abs() < 1e-9 {
        return Err("the wall has no area left once cut at the axis".into());
    }
    if area < 0.0 {
        out.reverse();
    }
    if let Some(k) = self_crossing(&out) {
        let q = out[k];
        return Err(format!(
            "a wall this thick folds over itself near r = {}, z = {} (a bend tighter than half the thickness); make it thinner or round that bend more",
            easycad_lang::format_number((q[0] * 10.0).round() / 10.0),
            easycad_lang::format_number((q[1] * 10.0).round() / 10.0)
        ));
    }
    Ok(out)
}

/// Where a closed polygon crosses itself (the index of a span that crosses another), if it does.
/// Spans lying on the axis (r = 0) are left out: they make no surface.
fn self_crossing(ring: &[P2]) -> Option<usize> {
    let n = ring.len();
    let span = |k: usize| (ring[k], ring[(k + 1) % n]);
    let on_axis = |k: usize| {
        let (a, b) = span(k);
        a[0] == 0.0 && b[0] == 0.0
    };
    // Swept bottom to top: only spans whose heights overlap are compared.
    let mut order: Vec<(f64, f64, usize)> = (0..n)
        .map(|k| {
            let (a, b) = span(k);
            (a[1].min(b[1]), a[1].max(b[1]), k)
        })
        .collect();
    order.sort_by(|x, y| x.0.total_cmp(&y.0));
    let cross = |o: P2, a: P2, b: P2| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    for (x, &(_, top, i)) in order.iter().enumerate() {
        for &(bottom, _, j) in &order[x + 1..] {
            if bottom > top {
                break;
            }
            let adjacent = (i + 1) % n == j || (j + 1) % n == i;
            if adjacent || i == j || (on_axis(i) && on_axis(j)) {
                continue;
            }
            let ((a, b), (c, d)) = (span(i), span(j));
            let scale = (b[0] - a[0]).hypot(b[1] - a[1]) * (d[0] - c[0]).hypot(d[1] - c[1]);
            let eps = 1e-12 * scale.max(1e-12);
            let (d1, d2) = (cross(a, b, c), cross(a, b, d));
            let (d3, d4) = (cross(c, d, a), cross(c, d, b));
            let crossing = ((d1 > eps && d2 < -eps) || (d1 < -eps && d2 > eps))
                && ((d3 > eps && d4 < -eps) || (d3 < -eps && d4 > eps));
            // Or touching: an end of one lying on the other (overlapping ends of a U-turn).
            let touch = 1e-9 * scale.max(1e-12);
            let on = |p: P2, q0: P2, q1: P2, side: f64| {
                let along = (p[0] - q0[0]) * (q1[0] - q0[0]) + (p[1] - q0[1]) * (q1[1] - q0[1]);
                let len2 = (q1[0] - q0[0]).powi(2) + (q1[1] - q0[1]).powi(2);
                side.abs() <= touch && (0.0..=len2).contains(&along)
            };
            if crossing || on(c, a, b, d1) || on(d, a, b, d2) || on(a, c, d, d3) || on(b, c, d, d4)
            {
                return Some(i.min(j));
            }
        }
    }
    None
}

/// `a` mirrored through `b`: the point as far past `b` as `a` is before it.
fn reflect(a: P3, b: P3) -> P3 {
    [2.0 * b[0] - a[0], 2.0 * b[1] - a[1], 2.0 * b[2] - a[2]]
}

/// How much closer layers come where a wall leans (`contour`'s `pace`): `amount` of EasyGCode's
/// `1 − cos a`, scaled by `(a / pivot)^steep` so it can do less on shallow overhangs and more on
/// steep ones (`steep` 0: the same everywhere, EasyGCode's).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pace {
    pub amount: f64,
    pub steep: f64,
    /// Degrees from vertical where the curve gives `amount`.
    pub pivot: f64,
}

impl Pace {
    /// The same pace at every lean.
    pub fn even(amount: f64) -> Pace {
        Pace {
            amount,
            steep: 0.0,
            pivot: 45.0,
        }
    }

    fn check(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.amount) {
            return Err(format!(
                "`pace` must be between 0 and 1, got {}",
                self.amount
            ));
        }
        if !(self.steep.is_finite() && (0.0..=10.0).contains(&self.steep)) {
            return Err(format!(
                "`steep` must be between 0 and 10, got {}",
                self.steep
            ));
        }
        if !(self.pivot.is_finite() && self.pivot > 0.0 && self.pivot < 90.0) {
            return Err(format!(
                "`pivot` must be between 0 and 90 degrees, got {}",
                self.pivot
            ));
        }
        Ok(())
    }

    /// The pace where the wall leans so that cos(lean) is `c`.
    fn at(&self, c: f64) -> f64 {
        if self.steep == 0.0 {
            return self.amount;
        }
        let lean = c.clamp(-1.0, 1.0).acos().to_degrees().min(89.0);
        self.amount * (lean / self.pivot).powf(self.steep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::dist3;

    fn tube(sections: usize) -> Surface {
        let c = Curve::circle(10.0, 32);
        let curves: Vec<Curve> = (0..sections)
            .map(|i| c.transformed(&Xform::translate(0.0, 0.0, 10.0 * i as f64)))
            .collect();
        Surface::loft(&curves.iter().collect::<Vec<_>>()).unwrap()
    }

    /// A cone 20 mm tall whose wall leans `lean` degrees out from vertical.
    fn cone(lean: f64) -> Surface {
        let mut s = Surface::extrude(&Curve::circle(10.0, 32), 20.0).unwrap();
        let grow = (10.0 + 20.0 * lean.to_radians().tan()) / 10.0;
        for p in &mut s.sections[1] {
            p[0] *= grow;
            p[1] *= grow;
        }
        s
    }

    #[test]
    fn the_steep_curve_paces_steep_walls_more_and_shallow_ones_less() {
        let count = |lean: f64, steep: f64| {
            let pace = Pace {
                amount: 1.0,
                steep,
                pivot: 45.0,
            };
            cone(lean)
                .contour(&Field::constant(1.0), pace, 1_000_000)
                .unwrap()
                .len()
        };
        // EasyGCode's pace: steps of cos(lean).
        assert_eq!(count(60.0, 0.0), 41); // 20 / cos 60° = 40 steps
        // Steeper than the pivot: closer layers; shallower: further apart; at the pivot: as set.
        assert!(count(60.0, 2.0) > count(60.0, 0.0));
        assert!(count(20.0, 2.0) < count(20.0, 0.0));
        assert_eq!(count(45.0, 2.0), count(45.0, 0.0));
        // Never closer than 5 % of a layer, however steep the curve.
        assert!(count(80.0, 10.0) <= 401);
        let bad = |steep: f64, pivot: f64| {
            cone(30.0)
                .contour(
                    &Field::constant(1.0),
                    Pace {
                        amount: 1.0,
                        steep,
                        pivot,
                    },
                    1_000_000,
                )
                .is_err()
        };
        assert!(bad(-1.0, 45.0) && bad(1.0, 0.0) && bad(1.0, 90.0));
    }

    #[test]
    fn extrude_revolve_and_contour() {
        let c = Curve::circle(10.0, 32);
        let tube = Surface::extrude(&c, 20.0).unwrap();
        assert_eq!(tube.sections.len(), 2);
        assert_eq!(tube.sections[1][0], [10.0, 0.0, 20.0]);

        // A cone from radius 10 at the bottom to 5 at z = 10.
        let profile = Curve::new(vec![[10.0, 0.0, 0.0], [5.0, 0.0, 10.0]], false);
        let cone = Surface::revolve(&profile, 16).unwrap();
        assert_eq!((cone.sections.len(), cone.samples()), (2, 16));
        assert!(dist3(cone.sections[1][4], [0.0, 5.0, 10.0]) < 1e-12);
        let bad = Curve::new(vec![[-1.0, 0.0, 0.0], [5.0, 0.0, 10.0]], false);
        assert!(Surface::revolve(&bad, 16).is_err());
        assert!(Surface::revolve(&Curve::circle(3.0, 8), 16).is_err());

        // 20 mm at 0.3 mm snaps to 67 layers (20.1 mm), 68 contours from the bottom up.
        let layers = tube
            .contour(&Field::constant(0.3), Pace::even(0.0), 1_000_000)
            .unwrap();
        assert_eq!(layers.len(), 68);
        assert!((layers[67].points[0][2] - 20.1).abs() < 1e-9);
        assert!(layers.iter().all(|l| l.closed && l.points.len() == 32));
        assert!(
            tube.contour(&Field::constant(0.0), Pace::even(0.0), 1000)
                .is_err()
        );
        assert!(
            tube.contour(&Field::constant(0.001), Pace::even(0.0), 1000)
                .is_err()
        );
    }

    #[test]
    fn loft_passes_through_its_sections() {
        let s = tube(4);
        for i in 0..4 {
            assert!(dist3(s.point_at(i as f64, 3), s.sections[i][3]) < 1e-12);
        }
        // A straight tube stays straight in between.
        let mid = s.point_at(1.5, 0);
        assert!(dist3(mid, [10.0, 0.0, 15.0]) < 1e-9);
    }

    #[test]
    fn loft_needs_two_matching_sections() {
        let c = Curve::circle(10.0, 8);
        assert!(Surface::loft(&[&c]).is_err());
        let open = Curve::new(vec![[0.0; 3], [1.0, 0.0, 0.0]], false);
        assert!(Surface::loft(&[&c, &open]).is_err());
        // Different point counts are resampled to match.
        let square = Curve::polygon(4, 10.0).transformed(&Xform::translate(0.0, 0.0, 5.0));
        let s = Surface::loft(&[&c, &square]).unwrap();
        assert_eq!(s.sections[1].len(), 8);
    }

    #[test]
    fn mesh_is_watertight_and_in_range() {
        let mut s = tube(3);
        s.capped = true;
        let mesh = s.mesh(2);
        let verts = mesh.vertex_count() as u32;
        assert!(mesh.indices.iter().all(|&i| i < verts));
        // 5 rings × 32 quads × 2 + 2 caps × 32 triangles
        assert_eq!(mesh.indices.len() / 3, 4 * 32 * 2 + 2 * 32);
    }

    #[test]
    fn mesh_faces_outward_either_way_round() {
        // Signed volume via the divergence theorem is positive when triangles face outward.
        let volume = |mesh: &Mesh| -> f64 {
            let p = |i: u32| {
                let k = i as usize * 3;
                [
                    mesh.positions[k] as f64,
                    mesh.positions[k + 1] as f64,
                    mesh.positions[k + 2] as f64,
                ]
            };
            mesh.indices
                .chunks_exact(3)
                .map(|t| {
                    let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                    (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                        + a[2] * (b[0] * c[1] - b[1] * c[0]))
                        / 6.0
                })
                .sum()
        };
        let mut s = tube(2);
        s.capped = true;
        let v = volume(&s.mesh(1));
        let exact = std::f64::consts::PI * 100.0 * 10.0;
        assert!(v > 0.95 * exact && v < exact, "{v}");
        let mut reversed = s.clone();
        for sec in &mut reversed.sections {
            sec.reverse();
        }
        assert!(volume(&reversed.mesh(1)) > 0.95 * exact);
    }
}
