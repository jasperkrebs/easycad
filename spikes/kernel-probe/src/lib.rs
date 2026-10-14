//! Throwaway probe: can curvo (NURBS) + manifold-rust (mesh solids) do a
//! Grasshopper-style "twisted star vase" natively and in wasm32?
use curvo::prelude::*;
use manifold_rust::linalg::{Vec2, Vec3};
use manifold_rust::manifold::Manifold;
use nalgebra::{Point3, Rotation3, Translation3, Vector3};

pub struct Report {
    pub loft_verts: usize,
    pub loft_faces: usize,
    pub solid_tris: usize,
    pub solid_volume: f64,
}

fn star(points: usize, r_out: f64, r_in: f64) -> Vec<Point3<f64>> {
    (0..points * 2)
        .map(|i| {
            let a = i as f64 * std::f64::consts::PI / points as f64;
            let r = if i % 2 == 0 { r_out } else { r_in };
            Point3::new(r * a.cos(), r * a.sin(), 0.0)
        })
        .collect()
}

/// Runs the probe at the preview-quality tessellation used in docs/research.md.
pub fn run() -> Option<Report> {
    run_with(1e-1)
}

pub fn run_with(norm_tol: f64) -> Option<Report> {
    // 1) NURBS: smooth periodic star, 24 twisted/scaled sections, lofted surface.
    let base = NurbsCurve3D::<f64>::try_periodic(&star(7, 45.0, 37.0), 3).ok()?;
    let sections: Vec<_> = (0..24)
        .map(|i| {
            let t = i as f64 / 23.0;
            let s = 1.0 - 0.2 * t;
            let rot = Rotation3::from_axis_angle(&Vector3::z_axis(), t * std::f64::consts::FRAC_PI_2);
            let m = Translation3::new(0.0, 0.0, 160.0 * t).to_homogeneous()
                * rot.to_homogeneous()
                * nalgebra::Matrix4::new_nonuniform_scaling(&Vector3::new(s, s, 1.0));
            base.transformed(&m)
        })
        .collect();
    let wall = NurbsSurface::try_loft(&sections, Some(3)).ok()?;
    let opts: AdaptiveTessellationOptions<f64, nalgebra::U4, curvo::prelude::DefaultDivider<f64, nalgebra::U4>> =
        AdaptiveTessellationOptions::default().with_norm_tolerance(norm_tol);
    let tess = wall.tessellate(Some(opts));

    // 2) Mesh solid: twisted extrusion of the same star, hollowed by a smaller copy.
    let poly: Vec<Vec2> = star(7, 45.0, 37.0).iter().map(|p| Vec2::new(p.x, p.y)).collect();
    let inner: Vec<Vec2> = star(7, 43.0, 35.0).iter().map(|p| Vec2::new(p.x, p.y)).collect();
    let outer = Manifold::extrude(&vec![poly], 160.0, 64, 90.0, Vec2::new(0.8, 0.8));
    let cavity = Manifold::extrude(&vec![inner], 160.0, 64, 90.0, Vec2::new(0.8, 0.8))
        .translate(Vec3::new(0.0, 0.0, 2.0));
    let vase = outer.difference(&cavity);

    Some(Report {
        loft_verts: tess.points().len(),
        loft_faces: tess.faces().len(),
        solid_tris: vase.num_tri(),
        solid_volume: vase.volume(),
    })
}

/// Exported so the wasm build keeps all the geometry code (no dead-stripping).
#[no_mangle]
pub extern "C" fn probe_run() -> f64 {
    match run() {
        Some(r) => r.solid_volume + r.loft_faces as f64,
        None => -1.0,
    }
}
