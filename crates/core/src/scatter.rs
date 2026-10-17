//! Points scattered evenly but irregularly over a rectangle (blue noise), the way EasyGCode
//! places its coat hanger's spikes: Mitchell's best candidate, then an optional relaxation that
//! pushes points closer than the ideal spacing apart.

use crate::geom::P3;

/// The generator EasyGCode uses (mulberry32): the same seed always gives the same numbers.
struct Mulberry32(u32);

impl Mulberry32 {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6D2B_79F5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(1 | t);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

/// The most points one scatter may make.
pub const MAX_SCATTER: usize = 5000;

/// `count` points over x in −width/2..width/2 and y in 0..height (z = 0), each the best of 15
/// random candidates (the one farthest from the points so far). With `relax`, 30 rounds push
/// any two points closer than the ideal spacing for this many points apart.
pub fn scatter(
    width: f64,
    height: f64,
    count: usize,
    seed: f64,
    relax: bool,
) -> Result<Vec<P3>, String> {
    if !(width.is_finite() && width > 0.0 && height.is_finite() && height > 0.0) {
        return Err("`width` and `height` must be more than 0".into());
    }
    if count > MAX_SCATTER {
        return Err(format!("at most {MAX_SCATTER} points, got {count}"));
    }
    let seed = if seed.is_finite() {
        seed as i64 as u32
    } else {
        1
    };
    let mut rng = Mulberry32(if seed == 0 { 1 } else { seed });
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(count);
    for i in 0..count {
        let tries = if i == 0 { 1 } else { 15 };
        let mut best = [0.0; 2];
        let mut best_d = -1.0;
        for _ in 0..tries {
            let x = -width / 2.0 + rng.next() * width;
            let y = rng.next() * height;
            let d = pts
                .iter()
                .map(|q| (q[0] - x).hypot(q[1] - y))
                .fold(f64::INFINITY, f64::min);
            if d > best_d {
                (best, best_d) = ([x, y], d);
            }
        }
        pts.push(best);
    }
    if relax && count > 1 {
        let target = ((2.0 / 3f64.sqrt()) * width * height / count as f64).sqrt();
        for _ in 0..30 {
            for a in 0..count {
                for b in a + 1..count {
                    let (p, q) = (pts[a], pts[b]);
                    let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
                    let d = dx.hypot(dy);
                    if d >= target {
                        continue;
                    }
                    let (ux, uy) = if d > 1e-12 {
                        (dx / d, dy / d)
                    } else {
                        (1.0, 0.0)
                    };
                    let push = 0.25 * (target - d).min(target);
                    let clamp = |v: [f64; 2]| {
                        [
                            v[0].clamp(-width / 2.0, width / 2.0),
                            v[1].clamp(0.0, height),
                        ]
                    };
                    pts[a] = clamp([p[0] - ux * push, p[1] - uy * push]);
                    pts[b] = clamp([q[0] + ux * push, q[1] + uy * push]);
                }
            }
        }
    }
    Ok(pts.into_iter().map(|p| [p[0], p[1], 0.0]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mulberry32_matches_javascript() {
        // From Node.js: mulberry32(1) gives these first three numbers.
        let mut r = Mulberry32(1);
        let got: Vec<f64> = (0..3).map(|_| r.next()).collect();
        assert_eq!(
            got,
            vec![0.6270739405881613, 0.002735721180215478, 0.5274470399599522]
        );
    }

    #[test]
    fn scattered_points_spread_out() {
        let a = scatter(100.0, 50.0, 60, 7.0, true).unwrap();
        assert_eq!(a.len(), 60);
        assert_eq!(a, scatter(100.0, 50.0, 60, 7.0, true).unwrap());
        assert!(
            a.iter()
                .all(|p| p[0].abs() <= 50.0 && (0.0..=50.0).contains(&p[1]))
        );
        let closest = (0..a.len())
            .flat_map(|i| (i + 1..a.len()).map(move |j| (i, j)))
            .map(|(i, j)| (a[i][0] - a[j][0]).hypot(a[i][1] - a[j][1]))
            .fold(f64::INFINITY, f64::min);
        assert!(closest > 3.0, "{closest}");
        assert_ne!(a, scatter(100.0, 50.0, 60, 8.0, true).unwrap());
        assert!(scatter(0.0, 1.0, 3, 1.0, false).is_err());
    }
}
