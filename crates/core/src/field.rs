//! Fields: numbers that vary over space, the way Grasshopper's attractor patterns work.
//!
//! A field is evaluated at a point. `distance(circle(35))` is how far a point is from that
//! circle (negative inside it); `fade(…)` turns a distance into a smooth 0..1 weight; and
//! arithmetic and math steps combine fields point by point, so
//! `line * (1 + (open - 1) * near)` is a spacing that grows near the ring. Steps that take a
//! field (a spiral's spacing, a wave's size) evaluate it wherever they need a value.

use crate::curve::{Circle, Curve};
use crate::geom::P3;
use easycad_lang::BinOp;
use std::rc::Rc;

/// The deepest fields may nest (each operation adds one level). Real recipes use a handful.
pub const MAX_FIELD_DEPTH: u16 = 64;
/// The most inputs a math step applied to fields may take.
const MAX_ARITY: usize = 4;

/// A pure math step: numbers in, a number (or a message) out.
pub type MathFn = fn(&[f64]) -> Result<f64, String>;

#[derive(Debug, Clone)]
pub struct Field {
    kind: Kind,
    depth: u16,
}

/// Fields are functions; two fields are equal only if they are the same one.
impl PartialEq for Field {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

#[derive(Debug, Clone)]
enum Kind {
    Const(f64),
    /// The x (0), y (1) or z (2) coordinate of the point.
    Coord(usize),
    /// Distance in the XY plane to a curve; negative inside a closed curve.
    Distance(Rc<CurveDistance>),
    Binary(BinOp, Rc<Field>, Rc<Field>),
    Neg(Rc<Field>),
    /// A math step applied point by point.
    Map(MathFn, Vec<Rc<Field>>),
}

impl Field {
    pub fn constant(v: f64) -> Field {
        Field {
            kind: Kind::Const(v),
            depth: 1,
        }
    }

    /// The x (`axis` 0), y (1) or z (2) coordinate of each point: `move(z: 2 * x)` lifts
    /// points by twice their x.
    pub fn coordinate(axis: usize) -> Field {
        Field {
            kind: Kind::Coord(axis.min(2)),
            depth: 1,
        }
    }

    pub fn distance(curve: &Curve) -> Field {
        Field {
            kind: Kind::Distance(Rc::new(CurveDistance::new(curve))),
            depth: 1,
        }
    }

    pub fn binary(op: BinOp, a: Rc<Field>, b: Rc<Field>) -> Result<Field, String> {
        let depth = deeper(&[&a, &b])?;
        Ok(Field {
            kind: Kind::Binary(op, a, b),
            depth,
        })
    }

    pub fn negated(a: Rc<Field>) -> Result<Field, String> {
        let depth = deeper(&[&a])?;
        Ok(Field {
            kind: Kind::Neg(a),
            depth,
        })
    }

    pub fn map(f: MathFn, args: Vec<Rc<Field>>) -> Result<Field, String> {
        if args.len() > MAX_ARITY {
            return Err(format!(
                "this step can't take more than {MAX_ARITY} inputs on fields"
            ));
        }
        let depth = deeper(&args.iter().collect::<Vec<_>>())?;
        Ok(Field {
            kind: Kind::Map(f, args),
            depth,
        })
    }

    /// The field's value at `p`. A math step that fails there gives NaN, which the step using
    /// the field reports.
    pub fn at(&self, p: P3) -> f64 {
        match &self.kind {
            Kind::Const(v) => *v,
            Kind::Coord(axis) => p.get(*axis).copied().unwrap_or(0.0),
            Kind::Distance(d) => d.at([p[0], p[1]]),
            Kind::Binary(op, a, b) => {
                let (x, y) = (a.at(p), b.at(p));
                match op {
                    BinOp::Add => x + y,
                    BinOp::Sub => x - y,
                    BinOp::Mul => x * y,
                    BinOp::Div => x / y,
                    BinOp::Pow => x.powf(y),
                }
            }
            Kind::Neg(a) => -a.at(p),
            Kind::Map(f, args) => {
                let mut values = [0.0; MAX_ARITY];
                for (slot, arg) in values.iter_mut().zip(args) {
                    *slot = arg.at(p);
                }
                f(values.get(..args.len()).unwrap_or(&[])).unwrap_or(f64::NAN)
            }
        }
    }
}

fn deeper(args: &[&Rc<Field>]) -> Result<u16, String> {
    let depth = args.iter().map(|a| a.depth).max().unwrap_or(0) + 1;
    if depth > MAX_FIELD_DEPTH {
        return Err(format!(
            "fields are combined more than {MAX_FIELD_DEPTH} levels deep"
        ));
    }
    Ok(depth)
}

/// Distance from a point to a curve in the XY plane, signed for closed curves (negative
/// inside). Exact and constant-time for circles; otherwise from the curve's segments, with
/// groups of segments skipped when their bounding box is already farther than the best match.
#[derive(Debug, Clone, PartialEq)]
pub struct CurveDistance {
    circle: Option<Circle>,
    points: Vec<[f64; 2]>,
    closed: bool,
    /// Bounding boxes of consecutive groups of `CHUNK` segments: [min x, min y, max x, max y].
    boxes: Vec<[f64; 4]>,
}

const CHUNK: usize = 16;

impl CurveDistance {
    fn new(curve: &Curve) -> Self {
        let mut points: Vec<[f64; 2]> = curve.points.iter().map(|p| [p[0], p[1]]).collect();
        if curve.closed
            && let Some(first) = points.first().copied()
        {
            points.push(first);
        }
        let boxes = points
            .windows(2)
            .collect::<Vec<_>>()
            .chunks(CHUNK)
            .map(|segs| {
                let mut b = [
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ];
                for s in segs {
                    for p in s.iter() {
                        b = [
                            b[0].min(p[0]),
                            b[1].min(p[1]),
                            b[2].max(p[0]),
                            b[3].max(p[1]),
                        ];
                    }
                }
                b
            })
            .collect();
        CurveDistance {
            circle: curve.circle,
            points,
            closed: curve.closed,
            boxes,
        }
    }

    fn at(&self, p: [f64; 2]) -> f64 {
        if let Some(c) = self.circle {
            return (p[0] - c.center[0]).hypot(p[1] - c.center[1]) - c.r;
        }
        if self.points.len() == 1 {
            return self
                .points
                .first()
                .map_or(f64::NAN, |q| (p[0] - q[0]).hypot(p[1] - q[1]));
        }
        let mut best = f64::INFINITY;
        let mut inside = false;
        for (k, b) in self.boxes.iter().enumerate() {
            let segs = self
                .points
                .get(k * CHUNK..((k + 1) * CHUNK + 1).min(self.points.len()))
                .unwrap_or(&[]);
            // Even-odd crossings of a ray towards +X: only boxes the ray can hit matter.
            let ray_hits = self.closed && b[1] <= p[1] && p[1] <= b[3] && b[2] >= p[0];
            let dx = (b[0] - p[0]).max(p[0] - b[2]).max(0.0);
            let dy = (b[1] - p[1]).max(p[1] - b[3]).max(0.0);
            let near = dx.hypot(dy) < best;
            if !ray_hits && !near {
                continue;
            }
            for s in segs.windows(2) {
                let (a, c) = (s[0], s[1]);
                if ray_hits && (a[1] > p[1]) != (c[1] > p[1]) {
                    let x = a[0] + (p[1] - a[1]) * (c[0] - a[0]) / (c[1] - a[1]);
                    if p[0] < x {
                        inside = !inside;
                    }
                }
                if near {
                    best = best.min(segment_distance(p, a, c));
                }
            }
        }
        if inside { -best } else { best }
    }
}

fn segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

/// EasyGCode's spiral-disc falloff (`spiralDiscSpec` in its gcode.js): 1 within `band / 2` of
/// the curve, easing to 0 at `inside` mm inside it and `outside` mm outside it, with a
/// smoothstep (3t² − 2t³) so it never kinks. Arguments: distance, inside, outside, band.
pub fn fade(a: &[f64]) -> Result<f64, String> {
    let get = |i: usize| a.get(i).copied().unwrap_or(0.0);
    let (d, inside, outside, band) = (get(0), get(1), get(2), get(3));
    if !d.is_finite() {
        return Err("the distance is not a number".into());
    }
    if inside < 0.0 || outside < 0.0 || band < 0.0 {
        return Err("`inside`, `outside` and `band` must be 0 or more".into());
    }
    let half = band / 2.0;
    let d = if d < 0.0 {
        (d + half).min(0.0)
    } else {
        (d - half).max(0.0)
    };
    let t = if d < 0.0 {
        if inside > 0.0 { 1.0 + d / inside } else { 0.0 }
    } else if outside > 0.0 {
        1.0 - d / outside
    } else if d == 0.0 {
        1.0
    } else {
        0.0
    };
    let t = t.clamp(0.0, 1.0);
    Ok(t * t * (3.0 - 2.0 * t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Xform;

    fn rc(f: Field) -> Rc<Field> {
        Rc::new(f)
    }

    #[test]
    fn distance_to_a_circle_is_exact_and_signed() {
        let d = Field::distance(&Curve::circle(10.0, 8));
        assert_eq!(d.at([3.0, 4.0, 0.0]), -5.0);
        assert_eq!(d.at([0.0, 15.0, 7.0]), 5.0);
        let moved = Curve::circle(10.0, 8).transformed(&Xform::translate(100.0, 0.0, 0.0));
        assert_eq!(Field::distance(&moved).at([100.0, 0.0, 0.0]), -10.0);
    }

    #[test]
    fn distance_to_other_curves_comes_from_their_segments() {
        let square = Curve::polygon(4, 10.0).transformed(&Xform::rotate_z(45.0)); // sides at ±7.07
        let d = Field::distance(&square);
        let half = 10.0 / 2f64.sqrt();
        assert!((d.at([0.0, 0.0, 0.0]) + half).abs() < 1e-9);
        assert!((d.at([half + 3.0, 0.0, 0.0]) - 3.0).abs() < 1e-9);
        let open = Curve::new(vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]], false);
        assert_eq!(Field::distance(&open).at([5.0, -2.0, 0.0]), 2.0);
        // Many segments: the box skipping gives the same answer as the plain search.
        let star = Curve::star(40, 20.0, 12.0).transformed(&Xform::scale(1.0, 0.7, 1.0));
        let fast = Field::distance(&star);
        for p in [[0.0, 0.0], [25.0, 3.0], [-14.0, 6.0], [3.0, -13.0]] {
            let plain = star
                .segments()
                .map(|(a, b)| segment_distance(p, [a[0], a[1]], [b[0], b[1]]))
                .fold(f64::INFINITY, f64::min);
            assert!((fast.at([p[0], p[1], 0.0]).abs() - plain).abs() < 1e-12);
        }
    }

    #[test]
    fn fade_matches_easygcode() {
        let f = |d: f64| fade(&[d, 15.0, 10.0, 4.0]).unwrap();
        assert_eq!(f(0.0), 1.0);
        assert_eq!(f(-2.0), 1.0); // inside the band
        assert_eq!(f(2.0), 1.0);
        assert_eq!(f(-17.0), 0.0); // 2 (half band) + 15 inside
        assert_eq!(f(12.0), 0.0); // 2 + 10 outside
        let t: f64 = 1.0 - 5.0 / 10.0;
        assert_eq!(f(7.0), t * t * (3.0 - 2.0 * t));
        assert_eq!(fade(&[0.0, 0.0, 0.0, 0.0]).unwrap(), 1.0);
        assert_eq!(fade(&[0.1, 0.0, 0.0, 0.0]).unwrap(), 0.0);
        assert!(fade(&[f64::NAN, 1.0, 1.0, 0.0]).is_err());
        assert!(fade(&[1.0, -1.0, 1.0, 0.0]).is_err());
    }

    #[test]
    fn arithmetic_and_math_work_point_by_point() {
        let d = rc(Field::distance(&Curve::circle(10.0, 8)));
        let near = rc(Field::map(
            fade,
            vec![
                d,
                rc(Field::constant(5.0)),
                rc(Field::constant(5.0)),
                rc(Field::constant(0.0)),
            ],
        )
        .unwrap());
        let spacing = Field::binary(BinOp::Mul, rc(Field::constant(2.0)), near).unwrap();
        assert_eq!(spacing.at([10.0, 0.0, 0.0]), 2.0);
        assert_eq!(spacing.at([30.0, 0.0, 0.0]), 0.0);
        let bad = Field::map(|_| Err("no".into()), vec![]).unwrap();
        assert!(bad.at([0.0; 3]).is_nan());
    }

    #[test]
    fn nesting_is_limited() {
        let mut f = rc(Field::constant(1.0));
        for _ in 1..MAX_FIELD_DEPTH {
            f = rc(Field::negated(f).unwrap());
        }
        assert!(Field::negated(f).is_err());
    }
}
