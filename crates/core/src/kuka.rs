//! The KUKA KR 50 R2500's kinematics, as the SPHR handover models it
//! (`SPHR_Full_Print_Handover-2/Globe_E27_30_Layers/kinematics.py`): Denavit–Hartenberg
//! parameters, the joint limits, and the closed-form inverse for its one arm configuration
//! (elbow up, the wrist bent one way: A5 ≤ 0). Ported line for line, and tested against
//! numbers from the Python (`kinematics.py`) in the tests below.
//!
//! Positions are world millimetres with the robot's base at the world origin; a pose is the tool
//! frame (the nozzle tip, its z into the tool) in world coordinates, and `tool` is TR_TOOL (the
//! tool frame on the flange).

use std::f64::consts::FRAC_PI_2;

/// DH link lengths a, offsets d (mm) and twists α (radians), A1 to A6.
const A: [f64; 6] = [175.0, 1290.0, 50.0, 0.0, 0.0, 0.0];
const D: [f64; 6] = [575.0, 0.0, 0.0, 1035.0, 0.0, 185.0];
const ALPHA: [f64; 6] = [FRAC_PI_2, 0.0, FRAC_PI_2, -FRAC_PI_2, FRAC_PI_2, 0.0];
/// Joint limits (degrees), A1 to A6 (A6 is continuous in the handover's model).
pub const LIMITS: [[f64; 2]; 6] = [
    [-185.0, 185.0],
    [-175.0, 60.0],
    [-120.0, 170.0],
    [-180.0, 180.0],
    [-125.0, 125.0],
    [-450.0, 450.0],
];

/// The handover's start pose (degrees): the continuous joints (A1, A4, A6) are counted from it,
/// as kinematics.py does.
pub const SEED: [f64; 6] = [-10.7617, -14.3847, 124.2567, -100.1345, -86.3609, 160.4508];

pub type M3 = [[f64; 3]; 3];

/// KUKA's ZYX angles A, B, C (degrees) as a rotation matrix.
pub fn rot(a: f64, b: f64, c: f64) -> M3 {
    let (sa, ca) = a.to_radians().sin_cos();
    let (sb, cb) = b.to_radians().sin_cos();
    let (sc, cc) = c.to_radians().sin_cos();
    [
        [ca * cb, ca * sb * sc - sa * cc, ca * sb * cc + sa * sc],
        [sa * cb, sa * sb * sc + ca * cc, sa * sb * cc - ca * sc],
        [-sb, cb * sc, cb * cc],
    ]
}

fn mul(a: &M3, b: &M3) -> M3 {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

fn transpose(a: &M3) -> M3 {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = a[j][i];
        }
    }
    out
}

/// The rotation of the first `n` links for joint angles `j` (degrees), as `forward` in
/// kinematics.py (only its rotation part is needed here).
fn links_rotation(j: &[f64; 6], n: usize) -> M3 {
    let mut m: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for i in 0..n {
        let q = -j[i].to_radians() + if i == 2 { FRAC_PI_2 } else { 0.0 };
        let (s, c) = q.sin_cos();
        let (sa, ca) = ALPHA[i].sin_cos();
        let t = [[c, -s * ca, s * sa], [s, c * ca, -c * sa], [0.0, sa, ca]];
        m = mul(&m, &t);
    }
    m
}

/// The joint angles (degrees) that put the tool at world `p` with world rotation `r`, for the
/// handover's configuration; `None` out of reach. A1, A4 and A6 come out in −180..180 (the
/// handover unwraps them along a path; nothing here needs that).
pub fn inverse(p: [f64; 3], r: &M3, tool: &[f64; 6]) -> Option<[f64; 6]> {
    let tr = rot(tool[3], tool[4], tool[5]);
    let tt = [tool[0], tool[1], tool[2]];
    // The flange's rotation, and the wrist centre: the tool point less the tool offset, less
    // the flange's length along its own axis.
    let fr = mul(r, &transpose(&tr));
    let mut w = p;
    for k in 0..3 {
        w[k] -= (0..3).map(|m| fr[k][m] * tt[m]).sum::<f64>() + D[5] * fr[k][2];
    }
    let rr = w[0].hypot(w[1]) - A[0];
    let z = w[2] - D[0];
    let l1 = A[1];
    let l2 = A[2].hypot(D[3]);
    let delta = (-D[3]).atan2(A[2]);
    let co = (rr * rr + z * z - l1 * l1 - l2 * l2) / (2.0 * l1 * l2);
    if !co.is_finite() || co.abs() > 1.0 + 1e-9 {
        return None;
    }
    let phi = -co.clamp(-1.0, 1.0).acos();
    let q2 = phi - delta;
    let q1 = z.atan2(rr) - (l2 * phi.sin()).atan2(l1 + l2 * phi.cos());
    let mut j = [0.0; 6];
    j[0] = -(w[1].atan2(w[0])).to_degrees();
    j[1] = -q1.to_degrees();
    j[2] = 90.0 - q2.to_degrees();
    let r3 = mul(&transpose(&links_rotation(&j, 3)), &fr);
    j[3] = -(r3[1][2].atan2(r3[0][2])).to_degrees();
    j[4] = -(r3[2][2].clamp(-1.0, 1.0).acos()).to_degrees();
    j[5] = -(r3[2][1].atan2(-r3[2][0])).to_degrees();
    Some(j)
}

/// `j` with A1, A4 and A6 turned by whole turns to lie nearest `near` (the joints a moment
/// before): the robot turns them continuously, so −179° after 179° is 181°.
pub fn unwrap_near(mut j: [f64; 6], near: &[f64; 6]) -> [f64; 6] {
    for k in [0, 3, 5] {
        j[k] += 360.0 * ((near[k] - j[k]) / 360.0).round();
    }
    j
}

/// How far the joints A1 to A6 are inside their limits (degrees; negative: past one), the
/// least of them, and which (0 for A1). A6's limits are the handover's ±450°, so it hardly
/// ever counts.
pub fn margin(j: &[f64; 6]) -> (f64, usize) {
    (0..6)
        .map(|k| ((j[k] - LIMITS[k][0]).min(LIMITS[k][1] - j[k]), k))
        .fold((f64::INFINITY, 0), |a, b| if b.0 < a.0 { b } else { a })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOOL: [f64; 6] = [-508.0, 3.5, 139.34, -180.0, -90.0, 0.0];

    /// Numbers from the handover's kinematics.py (`inverse`, before it unwraps), for three
    /// poses of the SPHR cell: upright over the print centre, tilted 30° and 45°.
    #[test]
    fn matches_the_handover_kinematics() {
        let cases: [([f64; 3], [f64; 3], [f64; 6]); 3] = [
            (
                [1088.30, -65.14, -656.50],
                [-90.0, 0.0, 0.0],
                [
                    -13.355127,
                    -11.784364,
                    122.286323,
                    -102.536814,
                    -85.359678,
                    -19.991977,
                ],
            ),
            (
                [1088.30, -65.14, -653.70],
                [-90.0, 0.0, -30.0],
                [
                    -10.905341,
                    -10.867198,
                    107.605850,
                    -100.831792,
                    -88.727967,
                    23.381951,
                ],
            ),
            (
                [1050.00, -200.00, -500.00],
                [-95.0, -20.0, 40.0],
                [
                    -19.105867,
                    0.663433,
                    131.285249,
                    -120.935531,
                    -89.875286,
                    -77.601864,
                ],
            ),
        ];
        for (p, abc, expected) in cases {
            let j = inverse(p, &rot(abc[0], abc[1], abc[2]), &TOOL).unwrap();
            for k in 0..6 {
                assert!(
                    (j[k] - expected[k]).abs() < 1e-4,
                    "A{}: {} vs {}",
                    k + 1,
                    j[k],
                    expected[k]
                );
            }
        }
    }

    #[test]
    fn out_of_reach_is_none() {
        assert!(inverse([5000.0, 0.0, 0.0], &rot(-90.0, 0.0, 0.0), &TOOL).is_none());
    }
}
