fn main() {
    for tol in [1e-3, 1e-2, 5e-2, 1e-1] {
        let t = std::time::Instant::now();
        let r = probe::run_with(tol).expect("probe failed");
        println!(
            "norm_tol {:>6}: loft {:>7} faces | solid {} tris, vol {:.0} mm^3 | total {:.1} ms",
            tol, r.loft_faces, r.solid_tris, r.solid_volume, t.elapsed().as_secs_f64() * 1000.0
        );
    }
    // solid-only timing: run the manifold part alone via a huge tolerance loft (cheap)
    let t = std::time::Instant::now();
    for _ in 0..10 { let _ = probe::run_with(1.0); }
    println!("10x run at coarse loft: {:.1} ms each", t.elapsed().as_secs_f64() * 100.0);
}
