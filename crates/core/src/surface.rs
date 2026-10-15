//! Surfaces lofted through section curves, and their preview meshes.

use crate::curve::Curve;
use crate::geom::{P3, Xform, catmull_rom, signed_area_xy};

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
        let sections = profile
            .points
            .iter()
            .map(|p| {
                (0..n)
                    .map(|j| {
                        let a = std::f64::consts::TAU * j as f64 / n as f64;
                        [p[0] * a.cos(), p[0] * a.sin(), p[2]]
                    })
                    .collect()
            })
            .collect();
        Ok(Surface {
            sections,
            closed: true,
            capped: false,
        })
    }

    /// Flat layers through the surface (Grasshopper's Contour), `layer` apart: the first at its
    /// bottom, then one per layer up to its top, the height snapped to whole layers with the
    /// surface stretched to fit, exactly as `vase_spiral` prints it. Sections must stack upward.
    pub fn contour(&self, layer: f64, max_points: usize) -> Result<Vec<Curve>, String> {
        if !(layer > 0.0 && layer.is_finite()) {
            return Err(format!("`layer` must be more than 0, got {layer}"));
        }
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
        let layers = (height / layer).round().max(1.0) as usize;
        if (layers + 1).saturating_mul(m) > max_points {
            return Err(format!(
                "this needs too many points ({} layers of {m}); use a larger layer height",
                layers + 1
            ));
        }
        let v_at = |z: f64| -> f64 {
            let i = levels
                .partition_point(|l| *l <= z)
                .saturating_sub(1)
                .min(n - 2);
            let (lo, hi) = (levels[i], levels[i + 1]);
            i as f64 + ((z - lo) / (hi - lo)).clamp(0.0, 1.0)
        };
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
                mesh.positions.extend(self.point_at(v, j).map(|c| c as f32));
            }
        }
        let at = |ring: usize, j: usize| (ring * m + j % m) as u32;
        let around = if self.closed { m } else { m - 1 };
        for r in 0..rings - 1 {
            for j in 0..around {
                let (a, b, c, d) = (at(r, j), at(r, j + 1), at(r + 1, j + 1), at(r + 1, j));
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
        mesh
    }
}

/// `a` mirrored through `b`: the point as far past `b` as `a` is before it.
fn reflect(a: P3, b: P3) -> P3 {
    [2.0 * b[0] - a[0], 2.0 * b[1] - a[1], 2.0 * b[2] - a[2]]
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
        let layers = tube.contour(0.3, 1_000_000).unwrap();
        assert_eq!(layers.len(), 68);
        assert!((layers[67].points[0][2] - 20.1).abs() < 1e-9);
        assert!(layers.iter().all(|l| l.closed && l.points.len() == 32));
        assert!(tube.contour(0.0, 1000).is_err());
        assert!(tube.contour(0.001, 1000).is_err());
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
