//! The node library: every step a recipe can call, with its inputs and docs.
//!
//! The catalog the app shows (and the agent will read) is generated from this table, so the
//! docs can't drift from the code. Units: lengths in mm, angles in degrees, speeds in mm/s.

use crate::curve::Curve;
use crate::eval::Ctx;
use crate::field::{self, Field, MathFn};
use crate::geom::{P3, Plane, Xform};
use crate::spiral;
use crate::surface::Surface;
use crate::toolpath::{self, MAX_PATH_POINTS, Settings, Toolpath};
use crate::value::Value;
use std::rc::Rc;

mod curve_tools;
mod printing;
mod toolbox;

/// How an input takes a list. An *item* input that receives a list runs the node once per item
/// (Grasshopper's list matching); a *list* input takes the whole list. A *point* input is an
/// item input that also reads a list of 2 or 3 numbers as one point (`[0, 0, 1]`). A list input
/// given a list of lists runs once per inner list, like a Grasshopper component once per branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Item,
    List,
    Point,
    /// The whole value as it comes, nested lists and all (never run once per item).
    Tree,
}

#[derive(Debug)]
pub struct Input {
    pub name: &'static str,
    /// For the docs: `Number`, `Curve`, `Geometry`, …
    pub kind: &'static str,
    pub access: Access,
    /// The default when it's a number. Neither this nor `point`: the input is required.
    pub default: Option<f64>,
    /// The default when it's a point or vector.
    pub point: Option<P3>,
    pub doc: &'static str,
}

impl Input {
    pub fn required(&self) -> bool {
        self.default.is_none() && self.point.is_none()
    }

    /// The value an input takes when the recipe leaves it out.
    pub fn default_value(&self) -> Value {
        match (self.point, self.default) {
            (Some(p), _) => Value::Point(p),
            (None, d) => Value::Number(d.unwrap_or(0.0)),
        }
    }
}

/// What a node does with its inputs.
pub enum Run {
    /// Works on whole values.
    Op(fn(&mut Ctx, &Args) -> Result<Value, String>),
    /// Pure math on numbers. Given a field anywhere, it returns a field that applies the math
    /// point by point.
    Math(MathFn),
}

pub struct Node {
    pub name: &'static str,
    pub group: &'static str,
    pub inputs: &'static [Input],
    pub output: &'static str,
    pub doc: &'static str,
    pub run: Run,
}

const fn req(name: &'static str, kind: &'static str, doc: &'static str) -> Input {
    Input {
        name,
        kind,
        access: Access::Item,
        default: None,
        point: None,
        doc,
    }
}

const fn opt(name: &'static str, kind: &'static str, default: f64, doc: &'static str) -> Input {
    Input {
        name,
        kind,
        access: Access::Item,
        default: Some(default),
        point: None,
        doc,
    }
}

const fn all(name: &'static str, kind: &'static str, doc: &'static str) -> Input {
    Input {
        name,
        kind,
        access: Access::List,
        default: None,
        point: None,
        doc,
    }
}

/// A required input that takes a nested list whole.
const fn tree(name: &'static str, kind: &'static str, doc: &'static str) -> Input {
    Input {
        name,
        kind,
        access: Access::Tree,
        default: None,
        point: None,
        doc,
    }
}

/// A required point or vector input.
const fn pt(name: &'static str, kind: &'static str, doc: &'static str) -> Input {
    Input {
        name,
        kind,
        access: Access::Point,
        default: None,
        point: None,
        doc,
    }
}

/// An optional point or vector input.
const fn opt_pt(name: &'static str, kind: &'static str, default: P3, doc: &'static str) -> Input {
    Input {
        name,
        kind,
        access: Access::Point,
        default: None,
        point: Some(default),
        doc,
    }
}

const GEOMETRY: &str = "Geometry";
const NUM: &str = "Number | Field";

/// Every node, in catalog order.
pub static NODES: &[Node] = &[
    // ------------------------------------------------------------------ numbers & lists
    Node {
        name: "range",
        group: "Numbers & lists",
        inputs: &[
            req("start", "Number", "first number"),
            req("end", "Number", "last number"),
            req("count", "Count", "how many numbers (rounded)"),
        ],
        output: "List<Number>",
        doc: "`count` evenly spaced numbers from `start` to `end`, both included. `range(0, 1, count: 5)` is [0, 0.25, 0.5, 0.75, 1].",
        run: Run::Op(range),
    },
    Node {
        name: "series",
        group: "Numbers & lists",
        inputs: &[
            req("start", "Number", "first number"),
            req("step", "Number", "added each time"),
            req("count", "Count", "how many numbers"),
        ],
        output: "List<Number>",
        doc: "`count` numbers from `start`, `step` apart. (Grasshopper: Series)",
        run: Run::Op(toolbox::series),
    },
    Node {
        name: "random",
        group: "Numbers & lists",
        inputs: &[
            req("count", "Count", "how many numbers"),
            opt("low", "Number", 0.0, ""),
            opt("high", "Number", 1.0, ""),
            opt("seed", "Number", 1.0, "another seed gives other numbers"),
        ],
        output: "List<Number>",
        doc: "Random numbers between `low` and `high`, the same every time for the same seed. (Grasshopper: Random)",
        run: Run::Op(toolbox::random),
    },
    Node {
        name: "count",
        group: "Numbers & lists",
        inputs: &[all("list", "List", "any list")],
        output: "Number",
        doc: "How many items a list has. (Grasshopper: List Length)",
        run: Run::Op(count),
    },
    Node {
        name: "item",
        group: "Numbers & lists",
        inputs: &[
            all("list", "List", "any list"),
            req(
                "index",
                "Number",
                "0 is the first; it wraps around, so -1 is the last",
            ),
        ],
        output: "Value",
        doc: "One item of a list; a list of indices gives a list of items. (Grasshopper: List Item, with wrap)",
        run: Run::Op(toolbox::item),
    },
    Node {
        name: "reverse",
        group: "Numbers & lists",
        inputs: &[all("list", "List", "any list")],
        output: "List",
        doc: "The list in reverse order. (Grasshopper: Reverse List)",
        run: Run::Op(toolbox::reverse),
    },
    Node {
        name: "shift",
        group: "Numbers & lists",
        inputs: &[
            all("list", "List", "any list"),
            req("offset", "Number", "how many places; items wrap around"),
        ],
        output: "List",
        doc: "The list rotated: `shift(list, 1)` starts at the second item. (Grasshopper: Shift List, with wrap)",
        run: Run::Op(toolbox::shift),
    },
    Node {
        name: "flatten",
        group: "Numbers & lists",
        inputs: &[tree("list", "List", "a list of lists (of lists…)")],
        output: "List",
        doc: "All items of all nested lists in one plain list. (Grasshopper: Flatten)",
        run: Run::Op(toolbox::flatten),
    },
    Node {
        name: "graft",
        group: "Numbers & lists",
        inputs: &[all("list", "List", "any list")],
        output: "List",
        doc: "Each item in a list of its own, so a step that takes a whole list runs once per item: `toolpath(graft(rings), …)` makes a toolpath per ring, for `join` to hop between. (Grasshopper: Graft Tree)",
        run: Run::Op(toolbox::graft),
    },
    Node {
        name: "pick",
        group: "Numbers & lists",
        inputs: &[
            req("test", "Bool | Number | Field", "true, or any number but 0"),
            tree("yes", "Any", "the result when `test` is true"),
            tree("no", "Any", "the result otherwise"),
        ],
        output: "Any",
        doc: "`yes` when `test` is true (or a number other than 0), `no` otherwise: `pick(line, line, 2.4 * pitch)` is the line width, or 2.4 × pitch when it's 0 (auto). With a field as test it picks point by point. (Grasshopper: Pick'n'Choose)",
        run: Run::Op(curve_tools::pick),
    },
    Node {
        name: "index_of",
        group: "Numbers & lists",
        inputs: &[
            all("list", "List", "numbers, texts or true/false"),
            tree("value", "Any", "what to look for"),
        ],
        output: "Number",
        doc: "Where `value` first appears in the list, counting from 0 like `item`: `item(pitches, index_of(sizes, size))` looks a dropdown's choice up in a table. (Grasshopper: Member Index)",
        run: Run::Op(curve_tools::index_of),
    },
    Node {
        name: "sum",
        group: "Numbers & lists",
        inputs: &[all("list", "List<Number>", "")],
        output: "Number",
        doc: "All the numbers added up. (Grasshopper: Mass Addition)",
        run: Run::Op(curve_tools::sum),
    },
    Node {
        name: "bounds",
        group: "Numbers & lists",
        inputs: &[all("list", "List<Number>", "")],
        output: "List<Number>",
        doc: "The smallest and the largest number of a list, as [smallest, largest]: `item(bounds(xs), 1)` is the largest. (Grasshopper: Bounds)",
        run: Run::Op(curve_tools::bounds),
    },
    Node {
        name: "graph",
        group: "Numbers & lists",
        inputs: &[
            all("xs", "List<Number>", "the graph's x values, in any order"),
            all("ys", "List<Number>", "its y values, one for each x"),
            req(
                "x",
                "Number | Field",
                "where to read it; a field gives a field",
            ),
            opt(
                "smooth",
                "Bool",
                0.0,
                "false: straight between the points; true: a smooth curve that never overshoots them and is level at both ends",
            ),
        ],
        output: "Number | Field",
        doc: "A graph through the points (x, y), read at `x`, and flat beyond its ends: a table that blends between its rows. `graph(heights, angles, z)` is a field of height. With `smooth: true` the curve is smooth through every point, never past them, and level at both ends: a radius profile through a few points that starts and ends with a straight wall. (Grasshopper: Graph Mapper)",
        run: Run::Op(curve_tools::graph),
    },
    Node {
        name: "accumulate",
        group: "Numbers & lists",
        inputs: &[all("list", "List<Number>", "")],
        output: "List<Number>",
        doc: "Running totals: [a, a + b, a + b + c, …]. Layer heights in, layer positions out. (Grasshopper: Mass Addition, partial results)",
        run: Run::Op(curve_tools::accumulate),
    },
    Node {
        name: "lerp",
        group: "Numbers & lists",
        inputs: &[
            req("a", NUM, "value at t = 0"),
            req("b", NUM, "value at t = 1"),
            req("t", NUM, "fraction"),
        ],
        output: NUM,
        doc: "Blends from `a` to `b`: a + (b − a)·t.",
        run: Run::Math(|a| Ok(arg(a, 0) + (arg(a, 1) - arg(a, 0)) * arg(a, 2))),
    },
    Node {
        name: "remap",
        group: "Numbers & lists",
        inputs: &[
            req("value", NUM, ""),
            req("from_low", NUM, ""),
            req("from_high", NUM, ""),
            opt("to_low", NUM, 0.0, ""),
            opt("to_high", NUM, 1.0, ""),
        ],
        output: NUM,
        doc: "Maps `value` from the range from_low..from_high onto to_low..to_high (not clamped). (Grasshopper: Remap Numbers)",
        run: Run::Math(toolbox::remap),
    },
    Node {
        name: "min",
        group: "Numbers & lists",
        inputs: &[req("a", NUM, ""), req("b", NUM, "")],
        output: NUM,
        doc: "The smaller of two values.",
        run: Run::Math(|a| Ok(arg(a, 0).min(arg(a, 1)))),
    },
    Node {
        name: "max",
        group: "Numbers & lists",
        inputs: &[req("a", NUM, ""), req("b", NUM, "")],
        output: NUM,
        doc: "The larger of two values.",
        run: Run::Math(|a| Ok(arg(a, 0).max(arg(a, 1)))),
    },
    Node {
        name: "clamp",
        group: "Numbers & lists",
        inputs: &[
            req("value", NUM, ""),
            req("low", NUM, ""),
            req("high", NUM, ""),
        ],
        output: NUM,
        doc: "Keeps `value` between `low` and `high`.",
        run: Run::Math(clamp),
    },
    Node {
        name: "mod",
        group: "Numbers & lists",
        inputs: &[req("a", NUM, ""), req("b", NUM, "")],
        output: NUM,
        doc: "Remainder of a ÷ b, always between 0 and b (for b > 0).",
        run: Run::Math(modulo),
    },
    Node {
        name: "abs",
        group: "Numbers & lists",
        inputs: &[req("x", NUM, "")],
        output: NUM,
        doc: "The value without its sign.",
        run: Run::Math(|a| Ok(arg(a, 0).abs())),
    },
    Node {
        name: "sqrt",
        group: "Numbers & lists",
        inputs: &[req("x", NUM, "0 or more")],
        output: NUM,
        doc: "Square root.",
        run: Run::Math(sqrt),
    },
    Node {
        name: "round",
        group: "Numbers & lists",
        inputs: &[req("x", NUM, "")],
        output: NUM,
        doc: "Nearest whole number.",
        run: Run::Math(|a| Ok(arg(a, 0).round())),
    },
    Node {
        name: "floor",
        group: "Numbers & lists",
        inputs: &[req("x", NUM, "")],
        output: NUM,
        doc: "Whole number at or below x.",
        run: Run::Math(|a| Ok(arg(a, 0).floor())),
    },
    Node {
        name: "ceil",
        group: "Numbers & lists",
        inputs: &[req("x", NUM, "")],
        output: NUM,
        doc: "Whole number at or above x.",
        run: Run::Math(|a| Ok(arg(a, 0).ceil())),
    },
    Node {
        name: "sin",
        group: "Numbers & lists",
        inputs: &[req("angle", NUM, "degrees")],
        output: NUM,
        doc: "Sine of an angle in degrees: sin(90) is 1.",
        run: Run::Math(|a| Ok(arg(a, 0).to_radians().sin())),
    },
    Node {
        name: "cos",
        group: "Numbers & lists",
        inputs: &[req("angle", NUM, "degrees")],
        output: NUM,
        doc: "Cosine of an angle in degrees.",
        run: Run::Math(|a| Ok(arg(a, 0).to_radians().cos())),
    },
    Node {
        name: "tan",
        group: "Numbers & lists",
        inputs: &[req("angle", NUM, "degrees")],
        output: NUM,
        doc: "Tangent of an angle in degrees.",
        run: Run::Math(|a| Ok(arg(a, 0).to_radians().tan())),
    },
    Node {
        name: "asin",
        group: "Numbers & lists",
        inputs: &[req("x", NUM, "-1..1")],
        output: NUM,
        doc: "The angle (degrees) whose sine is x.",
        run: Run::Math(|a| inverse_trig(arg(a, 0), f64::asin)),
    },
    Node {
        name: "acos",
        group: "Numbers & lists",
        inputs: &[req("x", NUM, "-1..1")],
        output: NUM,
        doc: "The angle (degrees) whose cosine is x.",
        run: Run::Math(|a| inverse_trig(arg(a, 0), f64::acos)),
    },
    Node {
        name: "atan",
        group: "Numbers & lists",
        inputs: &[req("x", NUM, "")],
        output: NUM,
        doc: "The angle (degrees) whose tangent is x.",
        run: Run::Math(|a| Ok(arg(a, 0).atan().to_degrees())),
    },
    Node {
        name: "atan2",
        group: "Numbers & lists",
        inputs: &[req("y", NUM, ""), req("x", NUM, "")],
        output: NUM,
        doc: "The angle (degrees, -180..180) of the direction (x, y) from +X; `atan2(y, x)` is each point's angle around the Z axis.",
        run: Run::Math(|a| Ok(arg(a, 0).atan2(arg(a, 1)).to_degrees())),
    },
    Node {
        name: "exp",
        group: "Numbers & lists",
        inputs: &[req("x", NUM, "")],
        output: NUM,
        doc: "e to the power x.",
        run: Run::Math(|a| Ok(arg(a, 0).exp())),
    },
    Node {
        name: "ln",
        group: "Numbers & lists",
        inputs: &[req("x", NUM, "more than 0")],
        output: NUM,
        doc: "The natural logarithm.",
        run: Run::Math(|a| {
            let x = arg(a, 0);
            if x > 0.0 {
                Ok(x.ln())
            } else {
                Err(format!("can't take the logarithm of {x}"))
            }
        }),
    },
    Node {
        name: "sign",
        group: "Numbers & lists",
        inputs: &[req("x", NUM, "")],
        output: NUM,
        doc: "-1, 0 or 1.",
        run: Run::Math(|a| {
            let x = arg(a, 0);
            Ok(if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            })
        }),
    },
    Node {
        name: "smoothstep",
        group: "Numbers & lists",
        inputs: &[req("t", NUM, "0..1 (clamped)")],
        output: NUM,
        doc: "An S-curve from 0 to 1: 3t² − 2t³. Eases in and out instead of kinking.",
        run: Run::Math(|a| {
            let t = arg(a, 0).clamp(0.0, 1.0);
            Ok(t * t * (3.0 - 2.0 * t))
        }),
    },
    Node {
        name: "greater",
        group: "Numbers & lists",
        inputs: &[req("a", NUM, ""), req("b", NUM, "")],
        output: "Number | Field",
        doc: "1 where `a` is more than `b`, else 0: a mask to multiply by, or a test for `pick`. `greater(z, 2)` is 1 above 2 mm. (Grasshopper: Larger Than)",
        run: Run::Math(|a| Ok(if arg(a, 0) > arg(a, 1) { 1.0 } else { 0.0 })),
    },
    Node {
        name: "less",
        group: "Numbers & lists",
        inputs: &[req("a", NUM, ""), req("b", NUM, "")],
        output: "Number | Field",
        doc: "1 where `a` is less than `b`, else 0. (Grasshopper: Smaller Than)",
        run: Run::Math(|a| Ok(if arg(a, 0) < arg(a, 1) { 1.0 } else { 0.0 })),
    },
    // ------------------------------------------------------------------ fields
    Node {
        name: "distance",
        group: "Fields",
        inputs: &[pt(
            "to",
            "Curve | Point",
            "a curve or a point to measure from",
        )],
        output: "Field",
        doc: "A field: how far each point is from the curve, in mm (in plan view; negative inside a closed curve), or from a point. Exact for circles.",
        run: Run::Op(distance),
    },
    Node {
        name: "fade",
        group: "Fields",
        inputs: &[
            req("distance", NUM, "a distance, usually a `distance(…)` field"),
            req("inside", NUM, "mm inside the curve where it reaches 0"),
            req("outside", NUM, "mm outside the curve where it reaches 0"),
            opt("band", NUM, 0.0, "width held at 1, centred on the curve"),
        ],
        output: NUM,
        doc: "1 on the curve, easing smoothly to 0 `inside` mm inside and `outside` mm outside it (EasyGCode's spiral-disc falloff). Turns a distance into a 0..1 weight.",
        run: Run::Math(field::fade),
    },
    Node {
        name: "lean",
        group: "Fields",
        inputs: &[req(
            "wall",
            "Surface",
            "a loft of closed sections stacked upward",
        )],
        output: "Field",
        doc: "How far the wall leans from vertical at each point, in degrees (positive leaning out, an overhang): `adjust(path, extrude: 1 / max(0.05, cos(lean(wall))))` prints wider beads where it overhangs (EasyGCode's lampshade compensation).",
        run: Run::Op(printing::lean),
    },
    // ------------------------------------------------------------------ points & planes
    Node {
        name: "point",
        group: "Points & planes",
        inputs: &[
            opt("x", "Length", 0.0, ""),
            opt("y", "Length", 0.0, ""),
            opt("z", "Length", 0.0, ""),
        ],
        output: "Point",
        doc: "A point. Lists of coordinates make lists of points. `[x, y, z]` also works wherever a point is asked for. (Grasshopper: Construct Point)",
        run: Run::Op(toolbox::point),
    },
    Node {
        name: "vector",
        group: "Points & planes",
        inputs: &[
            opt("x", "Length", 0.0, ""),
            opt("y", "Length", 0.0, ""),
            opt("z", "Length", 0.0, ""),
        ],
        output: "Vector",
        doc: "A direction and length (an arrow from the origin), for axes and moves; the same value as a point, so + - and * work on both. (Grasshopper: Vector XYZ)",
        run: Run::Op(toolbox::point),
    },
    Node {
        name: "plane",
        group: "Points & planes",
        inputs: &[
            opt_pt("origin", "Point", [0.0, 0.0, 0.0], ""),
            opt_pt(
                "normal",
                "Vector",
                [0.0, 0.0, 1.0],
                "the direction it faces",
            ),
        ],
        output: "Plane",
        doc: "A plane through `origin` facing `normal`. Move, rotate and mirror it like geometry. (Grasshopper: Plane Normal)",
        run: Run::Op(toolbox::plane),
    },
    Node {
        name: "sample",
        group: "Points & planes",
        inputs: &[
            req("field", NUM, "a field, or x, y or z"),
            pt("at", "Point", ""),
        ],
        output: "Number",
        doc: "A field's value at a point: `sample(z, p)` is the height of point p, `sample(distance(c), p)` its distance to curve c. (Grasshopper: Deconstruct Point)",
        run: Run::Op(toolbox::sample),
    },
    Node {
        name: "scatter",
        group: "Points & planes",
        inputs: &[
            req("width", "Length", "x from −width/2 to width/2"),
            req("height", "Length", "y from 0 to height"),
            req("count", "Count", "how many points"),
            opt("seed", "Number", 1.0, "another seed gives other points"),
            opt("relax", "Bool", 1.0, "push points that are too close apart"),
        ],
        output: "List<Point>",
        doc: "Points spread evenly but irregularly over a rectangle (blue noise), the same every time for the same seed: EasyGCode's spike placement. Map them onto a wall with `evaluate` and `move`. (Grasshopper: Populate 2D)",
        run: Run::Op(printing::scatter),
    },
    // ------------------------------------------------------------------ curves
    Node {
        name: "circle",
        group: "Curves",
        inputs: &[
            req("r", "Length", "radius"),
            opt(
                "tolerance",
                "Length",
                0.0,
                "0: points by quality; otherwise enough points that no flat strays more than this from the true circle (at least 24, as EasyGCode)",
            ),
        ],
        output: "Curve",
        doc: "A circle around the origin, flat on the bed, starting on +X and running counter-clockwise. Stays an exact circle through moves, turns and even scaling, which `helix` and `distance` use. (Grasshopper: Circle)",
        run: Run::Op(curve_tools::circle),
    },
    Node {
        name: "ellipse",
        group: "Curves",
        inputs: &[
            req("rx", "Length", "radius along X"),
            req("ry", "Length", "radius along Y"),
        ],
        output: "Curve",
        doc: "An ellipse around the origin, counter-clockwise from +X. (Grasshopper: Ellipse)",
        run: Run::Op(toolbox::ellipse),
    },
    Node {
        name: "superellipse",
        group: "Curves",
        inputs: &[
            req("rx", "Length", "radius along X"),
            req("ry", "Length", "radius along Y"),
            opt(
                "exponent",
                "Number",
                4.0,
                "2 is an ellipse, about 4 a squircle, more squarer, 1 a diamond",
            ),
        ],
        output: "Curve",
        doc: "|x/rx|ⁿ + |y/ry|ⁿ = 1 around the origin, counter-clockwise from +X: EasyGCode's squircle. (Grasshopper: no direct equivalent)",
        run: Run::Op(curve_tools::superellipse),
    },
    Node {
        name: "rectangle",
        group: "Curves",
        inputs: &[
            req("width", "Length", "along X"),
            req("height", "Length", "along Y"),
            opt("radius", "Length", 0.0, "corner rounding"),
        ],
        output: "Curve",
        doc: "A rectangle around the origin, counter-clockwise from the middle of its +X side, with rounded corners if `radius` > 0. (Grasshopper: Rectangle)",
        run: Run::Op(toolbox::rectangle),
    },
    Node {
        name: "arc",
        group: "Curves",
        inputs: &[
            req("r", "Length", "radius"),
            opt("start", "Angle", 0.0, "degrees from +X"),
            opt("end", "Angle", 180.0, "degrees from +X"),
            opt(
                "tolerance",
                "Length",
                0.0,
                "0: points by quality; otherwise enough points that no chord strays more than this from the true arc (at least 12 segments, as EasyGCode)",
            ),
        ],
        output: "Curve",
        doc: "An open arc around the origin, from `start` to `end` (counter-clockwise when `end` is larger). (Grasshopper: Arc)",
        run: Run::Op(toolbox::arc),
    },
    Node {
        name: "vertices",
        group: "Curves",
        inputs: &[req("curve", "Curve", "")],
        output: "List<Point>",
        doc: "The curve's own points, in order (a closed curve's first point isn't repeated): to reorder, drop or add points, then `polyline` them again. `flatten(vertices(rings))` strings rings together point by point. (Grasshopper: Control Points)",
        run: Run::Op(toolbox::vertices),
    },
    Node {
        name: "line",
        group: "Curves",
        inputs: &[pt("start", "Point", ""), pt("end", "Point", "")],
        output: "Curve",
        doc: "A straight line. (Grasshopper: Line)",
        run: Run::Op(toolbox::line),
    },
    Node {
        name: "polyline",
        group: "Curves",
        inputs: &[
            all("points", "List<Point>", "in order"),
            opt(
                "closed",
                "Bool",
                0.0,
                "true joins the last point back to the first",
            ),
        ],
        output: "Curve",
        doc: "Straight segments through the points. (Grasshopper: Polyline)",
        run: Run::Op(toolbox::polyline),
    },
    Node {
        name: "interpolate",
        group: "Curves",
        inputs: &[
            all("points", "List<Point>", "in order"),
            opt("closed", "Bool", 0.0, ""),
        ],
        output: "Curve",
        doc: "A smooth curve through the points (a cubic NURBS). (Grasshopper: Interpolate)",
        run: Run::Op(toolbox::interpolate),
    },
    Node {
        name: "catmull_rom",
        group: "Curves",
        inputs: &[
            all("points", "List<Point>", "in order"),
            opt("closed", "Bool", 0.0, ""),
            opt(
                "samples",
                "Count",
                16.0,
                "points per span between two points",
            ),
        ],
        output: "Curve",
        doc: "A Catmull-Rom spline through the points, the spline EasyGCode draws rounded stars and vessel profiles with (an open one starts heading straight for the second point). Unlike `interpolate`, each span depends only on its four nearest points.",
        run: Run::Op(curve_tools::catmull_rom),
    },
    Node {
        name: "polygon",
        group: "Curves",
        inputs: &[
            req("sides", "Count", "3 or more"),
            req("r", "Length", "corner radius"),
        ],
        output: "Curve",
        doc: "A regular polygon around the origin with a corner on +X.",
        run: Run::Op(polygon),
    },
    Node {
        name: "star",
        group: "Curves",
        inputs: &[
            req("points", "Count", "number of tips"),
            req("outer", "Length", "tip radius"),
            req("inner", "Length", "inner corner radius"),
        ],
        output: "Curve",
        doc: "A star's corners around the origin, alternating tips and inner corners, first tip on +X. Sharp; add `|> smooth` to round it.",
        run: Run::Op(star),
    },
    Node {
        name: "smooth",
        group: "Curves",
        inputs: &[req("curve", "Curve", "a curve with corners")],
        output: "Curve",
        doc: "A smooth curve through every point of `curve` (a cubic NURBS, closed if the curve is closed).",
        run: Run::Op(smooth),
    },
    Node {
        name: "spiral",
        group: "Curves",
        inputs: &[
            opt(
                "outer",
                "Length",
                0.0,
                "radius to end near (on a whole turn); or give `turns`",
            ),
            req(
                "spacing",
                "Length | Field",
                "distance from one turn to the next; a field makes it vary",
            ),
            opt(
                "tolerance",
                "Length",
                0.05,
                "how closely the points follow the true curve",
            ),
            opt(
                "min_samples",
                "Count",
                0.0,
                "at least this many points per turn",
            ),
            opt("start", "Length", 0.0, "radius it starts at, with `turns`"),
            opt(
                "turns",
                "Number",
                0.0,
                "exactly this many turns (fractions too) instead of ending near `outer`",
            ),
        ],
        output: "Curve",
        doc: "A flat spiral around the origin, starting on +X and turning counter-clockwise, each turn one `spacing` outside the last. From the centre out to the whole turn nearest `outer`; with a field as spacing, turns spread apart where the field is larger (EasyGCode's spiral disc). Or exactly `turns` turns from radius `start`, points spread evenly over the whole spiral (EasyGCode's spoon).",
        run: Run::Op(spiral),
    },
    Node {
        name: "interlock",
        group: "Curves",
        inputs: &[
            req("spiral", "Curve", "a spiral (from `spiral`)"),
            req(
                "bumps",
                "Count",
                "bumps per turn, counting outward and inward ones; odd",
            ),
            req("width", "Length", "line width"),
            opt(
                "stretch",
                "Length",
                0.0,
                "after the wiggles, stretch the whole spiral a little (every turn alike) to end exactly this far from its centre, to run straight into a wall that starts there; 0 leaves it",
            ),
        ],
        output: "Curve",
        doc: "Wiggles a spiral's turns so that wherever they are spread wider than `width`, each outward bump meets the next turn's inward bump with one line width between them: the turns stay joined at those points and leave holes in between. Turns that touch stay straight. A spiral snapped to whole turns ends near the radius you asked for, not on it; `stretch` fits it on exactly. It is done after the wiggles are solved, never to the spiral before: stretching first opens a sliver of gap between touching turns and the wiggles then alternate big and small from one turn to the next.",
        run: Run::Op(interlock),
    },
    Node {
        name: "tween",
        group: "Curves",
        inputs: &[
            req("a", "Curve", "at t = 0"),
            req("b", "Curve", "at t = 1"),
            req("t", "Number", "0..1; a list gives a list of curves"),
        ],
        output: "Curve",
        doc: "A curve between `a` and `b`, point by point; `tween(bottom, top, range(0, 1, count: n))` makes n layers that change gradually from one to the other. Both closed or both open; different point counts are resampled to match. (Grasshopper: Tween Curve)",
        run: Run::Op(toolbox::tween),
    },
    Node {
        name: "offset",
        group: "Curves",
        inputs: &[
            req("curve", "Curve", ""),
            req(
                "distance",
                "Length",
                "outward for a closed curve, to the right of travel for an open one; negative goes the other way",
            ),
        ],
        output: "Curve",
        doc: "The curve moved sideways in plan, corners mitred; good for smooth curves and distances smaller than their tightest bend. (Grasshopper: Offset Curve)",
        run: Run::Op(toolbox::offset),
    },
    Node {
        name: "divide",
        group: "Curves",
        inputs: &[
            req("curve", "Curve", ""),
            req("count", "Count", "number of equal pieces"),
        ],
        output: "List<Point>",
        doc: "Points dividing the curve into `count` pieces of equal length: count + 1 points on an open curve, count on a closed one. (Grasshopper: Divide Curve)",
        run: Run::Op(toolbox::divide),
    },
    Node {
        name: "evaluate",
        group: "Curves",
        inputs: &[
            req("curve", "Curve", ""),
            req("t", "Number", "0..1 along the length"),
        ],
        output: "Point",
        doc: "The point a fraction `t` of the way along the curve. (Grasshopper: Evaluate Length, normalized)",
        run: Run::Op(toolbox::evaluate),
    },
    Node {
        name: "length",
        group: "Curves",
        inputs: &[req("curve", "Curve", "")],
        output: "Number",
        doc: "The curve's length in mm. (Grasshopper: Length)",
        run: Run::Op(toolbox::length),
    },
    Node {
        name: "flip",
        group: "Curves",
        inputs: &[req("curve", "Curve", "")],
        output: "Curve",
        doc: "The curve run the other way (a closed curve keeps its start point). (Grasshopper: Flip Curve)",
        run: Run::Op(toolbox::flip),
    },
    Node {
        name: "seam",
        group: "Curves",
        inputs: &[
            req("curve", "Curve", "a closed curve"),
            req("t", "Number", "0..1 along the length"),
        ],
        output: "Curve",
        doc: "A closed curve starting at another point, a fraction `t` of the way along it: where a vase spiral starts and its layers change. (Grasshopper: Seam)",
        run: Run::Op(toolbox::seam),
    },
    Node {
        name: "resample",
        group: "Curves",
        inputs: &[
            req("curve", "Curve", ""),
            req("count", "Count", "number of points"),
        ],
        output: "Curve",
        doc: "The same curve with `count` points spaced evenly along it. (Grasshopper: Rebuild)",
        run: Run::Op(toolbox::resample),
    },
    Node {
        name: "fillet",
        group: "Curves",
        inputs: &[
            req("curve", "Curve", "with corners (a polyline, a polygon)"),
            req("radius", "Length", "arc radius"),
        ],
        output: "Curve",
        doc: "Every corner rounded with an arc of `radius` tangent to both sides; where a side is too short, the arc gets smaller (it uses at most half of each side). (Grasshopper: Fillet)",
        run: Run::Op(curve_tools::fillet),
    },
    Node {
        name: "simplify",
        group: "Curves",
        inputs: &[
            req("curve", "Curve", ""),
            req("tolerance", "Length", "how far the result may stray"),
        ],
        output: "Curve",
        doc: "The same curve with fewer points: every point it drops is within `tolerance` of the result (Douglas–Peucker). EasyGCode simplifies its dense outlines this way. (Grasshopper: Reduce)",
        run: Run::Op(curve_tools::simplify),
    },
    Node {
        name: "subcurve",
        group: "Curves",
        inputs: &[
            req("curve", "Curve", ""),
            req("start", "Number", "0..1 along the length"),
            req("end", "Number", "0..1 along the length"),
        ],
        output: "Curve",
        doc: "The piece between two places along the curve, as an open curve (from `start` to `end`, backwards if `end` comes first). On a closed curve, an `end` before `start` goes on through the seam. (Grasshopper: Sub Curve)",
        run: Run::Op(curve_tools::subcurve),
    },
    Node {
        name: "join_curves",
        group: "Curves",
        inputs: &[
            all(
                "curves",
                "List<Curve>",
                "pieces that meet end to end, in order",
            ),
            opt(
                "tolerance",
                "Length",
                0.001,
                "how close two ends must be to count as meeting",
            ),
        ],
        output: "Curve",
        doc: "One curve from pieces: each piece is turned round if needed to carry on from the one before; closed if the last piece ends where the first began. (Grasshopper: Join Curves)",
        run: Run::Op(curve_tools::join_curves),
    },
    Node {
        name: "tangent",
        group: "Curves",
        inputs: &[
            req("curve", "Curve", ""),
            req("t", "Number", "0..1 along the length"),
        ],
        output: "Vector",
        doc: "The direction of travel at a place along the curve, as a vector 1 long. (Grasshopper: Evaluate Curve, tangent)",
        run: Run::Op(curve_tools::tangent),
    },
    Node {
        name: "normal",
        group: "Curves",
        inputs: &[
            req("curve", "Curve", ""),
            req("t", "Number", "0..1 along the length"),
        ],
        output: "Vector",
        doc: "The sideways direction in plan at a place along the curve, 1 long: outward for a closed curve (either way round), to the right of travel for an open one. `move(evaluate(c, t), x: d * …)` places points along it; with `offset` it agrees.",
        run: Run::Op(curve_tools::normal),
    },
    Node {
        name: "closest",
        group: "Curves",
        inputs: &[req("curve", "Curve", ""), pt("point", "Point", "")],
        output: "Number",
        doc: "Where along the curve (0..1 by length) it comes closest to the point; `evaluate(c, closest(c, p))` is that closest point. (Grasshopper: Curve Closest Point)",
        run: Run::Op(curve_tools::closest),
    },
    Node {
        name: "centroid",
        group: "Curves",
        inputs: &[req("curve", "Curve", "")],
        output: "Point",
        doc: "The centre of the area a closed curve encloses (in plan, at its mean height); for an open curve, the middle of its length. (Grasshopper: Area, centroid)",
        run: Run::Op(curve_tools::centroid),
    },
    Node {
        name: "rings",
        group: "Curves",
        inputs: &[
            req("curve", "Curve", "a closed outline"),
            req(
                "spacing",
                "Length",
                "from one ring to the next: the line width",
            ),
            opt(
                "inset",
                "Bool",
                0.0,
                "false: scaled toward the centre (exact for circles); true: true insets, every side one spacing in",
            ),
        ],
        output: "List<Curve>",
        doc: "Rings inside an outline, from the outline itself inward, one spacing apart, until there's no room: EasyGCode's ring fill for bottoms and seats. Link them into one line with `link_rings`.",
        run: Run::Op(printing::rings),
    },
    Node {
        name: "link_rings",
        group: "Curves",
        inputs: &[
            all(
                "rings",
                "List<Curve>",
                "closed rings in print order (`reverse` them to go from the centre out)",
            ),
            req("style", "Text", "\"staircase\", \"zipper\" or \"spiral\""),
            req("width", "Length", "line width"),
        ],
        output: "Curve",
        doc: "The rings joined into one continuous line, as EasyGCode joins them: a staircase (each ring stops a line width short and steps across), a zipper (each ring cut open at a slot and run the other way) or a spiral (each ring blending into the next, opening out of the middle). The seam is on the ray from the centre through the largest ring's first point.",
        run: Run::Op(printing::link_rings),
    },
    // ------------------------------------------------------------------ transforms
    Node {
        name: "scale",
        group: "Transforms",
        inputs: &[
            req("geometry", GEOMETRY, "what to scale"),
            opt("factor", NUM, 1.0, "uniform scale"),
            opt("x", NUM, 1.0, "extra scale in X"),
            opt("y", NUM, 1.0, "extra scale in Y"),
            opt("z", NUM, 1.0, "extra scale in Z"),
            opt_pt("center", "Point", [0.0, 0.0, 0.0], ""),
        ],
        output: GEOMETRY,
        doc: "Scales about `center`. `scale(c, 2)` doubles everything; `scale(c, y: 0.5)` squashes Y only. A field scales each point by its own value: `scale(wall, 1 + 0.002 * z)` flares a wall upward. (Grasshopper: Scale, Scale NU)",
        run: Run::Op(toolbox::scale),
    },
    Node {
        name: "rotate",
        group: "Transforms",
        inputs: &[
            req("geometry", GEOMETRY, "what to rotate"),
            req(
                "angle",
                "Angle | Field",
                "degrees, counter-clockwise looking against the axis",
            ),
            opt_pt(
                "axis",
                "Vector",
                [0.0, 0.0, 1.0],
                "the direction to turn about",
            ),
            opt_pt("center", "Point", [0.0, 0.0, 0.0], "a point on the axis"),
        ],
        output: GEOMETRY,
        doc: "Rotates about an axis through `center` (the vertical one through the origin by default). `rotate(c, 30, axis: [0, 1, 0])` tilts about Y. A field turns each point by its own angle: `rotate(wall, z * 0.5)` twists 0.5° per mm of height. (Grasshopper: Rotate 3D)",
        run: Run::Op(toolbox::rotate),
    },
    Node {
        name: "move",
        group: "Transforms",
        inputs: &[
            req("geometry", GEOMETRY, "what to move"),
            opt("x", "Length | Field", 0.0, ""),
            opt("y", "Length | Field", 0.0, ""),
            opt("z", "Length | Field", 0.0, ""),
        ],
        output: GEOMETRY,
        doc: "Moves by x, y and z. `move(c, z: 10)` lifts by 10 mm. A field moves each point by its own value: `move(c, z: 0.3 * x)` tilts a flat curve into a slope, as if projected onto a tilted plane. (Grasshopper: Move)",
        run: Run::Op(toolbox::translate),
    },
    Node {
        name: "mirror",
        group: "Transforms",
        inputs: &[
            req("geometry", GEOMETRY, ""),
            req("plane", "Plane", "the mirror"),
        ],
        output: GEOMETRY,
        doc: "Mirrors through a plane; `mirror(c, plane(normal: [1, 0, 0]))` flips X. (Grasshopper: Mirror)",
        run: Run::Op(toolbox::mirror),
    },
    Node {
        name: "project_along",
        group: "Transforms",
        inputs: &[
            req("geometry", GEOMETRY, ""),
            req("plane", "Plane", "where the points land"),
            opt_pt(
                "direction",
                "Vector",
                [0.0, 0.0, 1.0],
                "the direction they travel (straight up and down by default)",
            ),
        ],
        output: GEOMETRY,
        doc: "Moves every point along `direction` until it lies in the plane: a circle projected onto a tilted plane is the top edge of a cylinder cut at that angle. (Grasshopper: Project Along)",
        run: Run::Op(toolbox::project_along),
    },
    Node {
        name: "center",
        group: "Transforms",
        inputs: &[all(
            "geometry",
            "Geometry | List",
            "one thing, or a list moved together",
        )],
        output: GEOMETRY,
        doc: "Moved in X and Y so the bounding box of all of it is centred on the origin, which prints at the bed's centre: `center(rotate(c, 45))` is EasyGCode's \"rotate onto the diagonal and recentre\". A list is moved together, by one offset. (Grasshopper: Move to bounding box centre)",
        run: Run::Op(curve_tools::center),
    },
    // ------------------------------------------------------------------ surfaces
    Node {
        name: "loft",
        group: "Surfaces",
        inputs: &[all(
            "sections",
            "List<Curve>",
            "two or more curves, bottom to top",
        )],
        output: "Surface",
        doc: "A smooth surface through the section curves, in order. Each section's first point is its seam.",
        run: Run::Op(loft),
    },
    Node {
        name: "cap",
        group: "Surfaces",
        inputs: &[req("surface", "Surface", "a loft of closed sections")],
        output: "Solid",
        doc: "Closes both ends of a lofted tube with flat caps, making a solid. (Grasshopper: Cap Holes)",
        run: Run::Op(cap),
    },
    Node {
        name: "extrude",
        group: "Surfaces",
        inputs: &[
            req("curve", "Curve", ""),
            req("height", "Length", "up along Z"),
        ],
        output: "Surface",
        doc: "The curve swept straight up. (Grasshopper: Extrude)",
        run: Run::Op(toolbox::extrude),
    },
    Node {
        name: "revolve",
        group: "Surfaces",
        inputs: &[req(
            "profile",
            "Curve",
            "drawn in the XZ plane: x is the distance from the Z axis, z the height",
        )],
        output: "Surface",
        doc: "A surface of revolution about the Z axis, built from horizontal circles, so it caps and prints in vase mode like a loft. (Grasshopper: Revolution)",
        run: Run::Op(toolbox::revolve),
    },
    Node {
        name: "contour",
        group: "Surfaces",
        inputs: &[
            req("surface", "Surface", "sections stacked upward"),
            req(
                "layer",
                "Length | Field",
                "layer height; a field (of height, read on the seam) changes it from layer to layer",
            ),
            opt(
                "pace",
                "Number",
                0.0,
                "0..1: closer layers where the wall leans (1: each step is layer × cos(lean), EasyGCode's overhang-paced vessel)",
            ),
        ],
        output: "List<Curve>",
        doc: "Flat layer curves through the surface, `layer` apart from bottom to top (height snapped to whole layers), ready for `spiralize` or `toolpath`, or to bend with fields first. With `pace`, layers come closer where the wall leans out or in, so the beads stay one layer apart along the wall, and the steps are fitted to end exactly at the top. A `layer` field (e.g. of `z`) makes each layer as tall as the field halfway up it: thin layers at the bottom easing into thick ones. (Grasshopper: Contour)",
        run: Run::Op(toolbox::contour),
    },
    // ------------------------------------------------------------------ toolpaths
    Node {
        name: "toolpath",
        group: "Toolpaths",
        inputs: &[
            all("curves", "List<Curve>", "printed in order"),
            req("width", "Length", "line width"),
            req("layer", "Length", "layer height"),
            opt("speed", "Speed", 40.0, "print speed in mm/s"),
            opt(
                "flow",
                "Number",
                0.0,
                "volumetric flow in mm³/s; when set, it sets the speed (flow ÷ bead area)",
            ),
        ],
        output: "Toolpath",
        doc: "Prints curves exactly as drawn, in order, with a travel move to the start of each. A curve drawn at z = 0 prints on the bed (the nozzle one layer up).",
        run: Run::Op(toolpath),
    },
    Node {
        name: "vase_spiral",
        group: "Toolpaths",
        inputs: &[
            req(
                "wall",
                "Surface",
                "a loft of closed sections stacked upward",
            ),
            req("layer", "Length", "layer height: the rise per turn"),
            req("width", "Length", "line width"),
            opt("speed", "Speed", 40.0, "print speed in mm/s"),
            opt(
                "flow",
                "Number",
                0.0,
                "volumetric flow in mm³/s; when set, it sets the speed (flow ÷ bead area)",
            ),
            opt(
                "flat_top",
                "Bool",
                1.0,
                "true: a flat closing turn for a level rim; false: an open top",
            ),
        ],
        output: "Toolpath",
        doc: "One continuous spiral up the wall (vase mode, as in EasyGCode): starts on the bed with extrusion ramping up, snaps to whole layers, and ends with a flat turn that ramps extrusion down (or, with `flat_top: false`, stops at full flow as it reaches the top: EasyGCode's open top).",
        run: Run::Op(vase_spiral),
    },
    Node {
        name: "vase_revolve",
        group: "Toolpaths",
        inputs: &[
            req(
                "profile",
                "Curve",
                "radius (x) against height (z), drawn in the XZ plane from the bottom up",
            ),
            req(
                "layer",
                "Length | Field",
                "rise per turn; a field is read once per turn, at its middle",
            ),
            req(
                "width",
                "Length | Field",
                "line width; a field is read once per turn, at its middle",
            ),
            opt("speed", "Speed", 40.0, "print speed in mm/s"),
            opt(
                "flow",
                "Number | Field",
                0.0,
                "volumetric flow in mm³/s (sets the speed); a field is read once per turn",
            ),
            opt(
                "bead_layer",
                "Length",
                0.0,
                "extrude every turn for this layer height instead of its rise (0: its rise)",
            ),
            opt(
                "tolerance",
                "Length",
                0.05,
                "how far a chord may stray from the true circle (at least 24 points per turn)",
            ),
            opt(
                "seam",
                "Angle",
                90.0,
                "where each turn starts, degrees from +X (90: the back)",
            ),
        ],
        output: "Toolpath",
        doc: "Vase mode around the Z axis following a profile, turn by turn: EasyGCode's lampshade. Each turn has as many points as its size needs and reads the radius from the profile at each point; the rise, width and flow can change from turn to turn (fields of height, e.g. from `graph`), which is how overhang compensation and flow ramps work. The first turn ramps extrusion up on the bed, a flat closing turn ramps it down.",
        run: Run::Op(vase_revolve),
    },
    Node {
        name: "spiralize",
        group: "Toolpaths",
        inputs: &[
            all(
                "layers",
                "List<Curve>",
                "closed curves from the bottom (on the bed) up; flat or not",
            ),
            req("width", "Length", "line width"),
            opt("speed", "Speed", 40.0, "print speed in mm/s"),
            opt(
                "flow",
                "Number",
                0.0,
                "volumetric flow in mm³/s; when set, it sets each move's speed (flow ÷ bead area)",
            ),
            opt(
                "flat_top",
                "Bool",
                1.0,
                "true: a flat closing turn for a level rim; false: an open top",
            ),
            opt(
                "ramp",
                "Bool",
                1.0,
                "true: start on the bed, extrusion ramping up; false: carry on at full extrusion from something printed before (a flat bottom)",
            ),
        ],
        output: "Toolpath",
        doc: "Vase mode through any stack of layers, flat or tilted: one continuous spiral that blends each layer into the next, each move extruded for its own local layer height (thin where layers are close, thick where they're apart). Same start ramp and closing turn as vase_spiral; flat, evenly spaced layers print exactly like it. `bead(path, layer: …)` extrudes it for one layer height throughout instead (EasyGCode's squeeze, where layers come closer on overhangs).",
        run: Run::Op(toolbox::spiralize),
    },
    Node {
        name: "helix",
        group: "Toolpaths",
        inputs: &[
            req(
                "circle",
                "Curve",
                "from `circle`: its centre and radius, where the helix starts and which way it turns",
            ),
            req("pitch", "Length", "rise per turn; also the layer height"),
            req("height", "Length", "where it ends; more than one pitch"),
            req("width", "Length", "line width"),
            opt("speed", "Speed", 40.0, "print speed in mm/s"),
            opt(
                "flow",
                "Number",
                0.0,
                "volumetric flow in mm³/s; when set, it sets the speed (flow ÷ bead area)",
            ),
        ],
        output: "Toolpath",
        doc: "Vase mode around a circle, rising exactly `pitch` per turn to exactly `height` (any fraction of a turn included, nothing snapped): EasyGCode's thread connector. Starts on the circle's first point with extrusion ramping up over the first turn, ends with a flat turn that ramps it down. One move per point of the circle: `circle(r, tolerance: 0.01)` keeps within 0.01 mm of the true circle. Counter-clockwise rising (a right-hand thread) for a circle as `circle` makes it.",
        run: Run::Op(helix),
    },
    Node {
        name: "stack",
        group: "Toolpaths",
        inputs: &[
            req("path", "Toolpath", "one layer (or more)"),
            req("count", "Count", "how many copies (0: none)"),
            opt(
                "lift",
                "Number",
                1.0,
                "layers above the new copy that the nozzle travels back to the start at; 0 goes straight over at the new layer's height",
            ),
            opt(
                "wipe",
                "Length",
                0.0,
                "instead: one straight move to this far above the new copy's start, then down onto it",
            ),
        ],
        output: "Toolpath",
        doc: "Prints the toolpath `count` times, each copy one layer higher. Between copies the nozzle travels back to the start clear of the print, like EasyGCode: lift 1 (its spiral disc), 0 (its spoon), or a wipe of two layers (its bend stool, which wipes clear without stopping to lift).",
        run: Run::Op(stack),
    },
    Node {
        name: "join",
        group: "Toolpaths",
        inputs: &[
            all("paths", "List<Toolpath>", "printed in order"),
            opt(
                "clear",
                "Length",
                0.0,
                "travel at least this high above the bed between parts (EasyGCode: twice the brim's layer height after a brim); 0 is one layer above the higher end",
            ),
            opt(
                "wipe",
                "Length",
                0.0,
                "instead: one straight move to this far above the next part's start, then down onto it",
            ),
        ],
        output: "Toolpath",
        doc: "Toolpaths printed one after another (a brim, then a wall), each with its own width, layer and speed. Between parts the nozzle goes up, over and down (or wipes across, with `wipe`); a part that starts where the last one ended carries straight on (no travel), and empty parts are skipped (their machine events still happen). One-layer parts at the start belong to the first layer, so the fan comes on after the brim *and* the first layer printed inside it.",
        run: Run::Op(join),
    },
    Node {
        name: "bead",
        group: "Toolpaths",
        inputs: &[
            req("path", "Toolpath", ""),
            opt(
                "width",
                "Length | Field",
                0.0,
                "line width to extrude for; 0 keeps it",
            ),
            opt(
                "layer",
                "Length | Field",
                0.0,
                "layer height to extrude for; 0 keeps it",
            ),
        ],
        output: "Toolpath",
        doc: "The same moves extruded for another bead (EasyGCode's bead overrides, like its spoon's stick): nothing moves, only the plastic each move gets, and at a set flow its speed. Join it to the rest to print part of a path thicker or thinner. Fields are read at the middle of every move, so the bead can blend from one size to another a little at each move: `bead(width: 3 + (2 - 3) * smoothstep(z / 8))` eases from 3 mm wide on the bed to 2 mm 8 mm up.",
        run: Run::Op(curve_tools::bead),
    },
    Node {
        name: "adjust",
        group: "Toolpaths",
        inputs: &[
            req("path", "Toolpath", ""),
            opt(
                "speed",
                NUM,
                1.0,
                "speed factor (at a set flow, the flow scales with it)",
            ),
            opt("extrude", NUM, 1.0, "extrusion factor"),
        ],
        output: "Toolpath",
        doc: "Every printing move's speed and extrusion multiplied by the fields at its middle: a slower band (`speed: 1 - 0.5 * greater(z, 50)`), a flow that ramps up over a fillet, wider beads where a wall overhangs. Travels are left alone.",
        run: Run::Op(printing::adjust),
    },
    Node {
        name: "accelerate",
        group: "Toolpaths",
        inputs: &[
            req("path", "Toolpath", ""),
            req("acceleration", "Number", "mm/s²"),
        ],
        output: "Toolpath",
        doc: "Speed that only rises as fast as `acceleration` allows: wherever a move is faster than the one before, the stretch after it is cut into 8 pieces that speed up step by step (EasyGCode's acceleration ramps after its spikes and bridges).",
        run: Run::Op(printing::accelerate),
    },
    Node {
        name: "fan",
        group: "Toolpaths",
        inputs: &[
            req("path", "Toolpath", ""),
            req("level", NUM, "0–100 %; a field sets it zone by zone"),
        ],
        output: "Toolpath",
        doc: "Part fan levels along the toolpath, read at each move: the fan changes wherever the level does (EasyGCode's fan on the bumps only). Replaces the automatic \"fan on after the first layer\"; like it, only written with the printer's start and end blocks.",
        run: Run::Op(printing::fan),
    },
    Node {
        name: "dwell",
        group: "Toolpaths",
        inputs: &[
            req("path", "Toolpath", ""),
            all(
                "at",
                "List<Point>",
                "the printing move ending nearest each point gets a pause",
            ),
            req("seconds", "Number", "0..600"),
        ],
        output: "Toolpath",
        doc: "A pause with the nozzle in place (`G4`) after the move nearest each point, as EasyGCode pauses at a spike's tip to let it set.",
        run: Run::Op(printing::dwell),
    },
    Node {
        name: "temperature",
        group: "Toolpaths",
        inputs: &[
            req("path", "Toolpath", ""),
            opt(
                "nozzle",
                NUM,
                0.0,
                "°C: a filament printer's nozzle, and any pellet zone not given; 0 is the printer's own",
            ),
            opt(
                "up",
                NUM,
                0.0,
                "°C: the pellet printer's upper zone; 0 is `nozzle` or the printer's own",
            ),
            opt("mid", NUM, 0.0, "°C: its middle zone"),
            opt(
                "down",
                NUM,
                0.0,
                "°C: its lower zone, at the nozzle (a filament printer uses it when `nozzle` is 0)",
            ),
            opt(
                "at_end",
                "Bool",
                0.0,
                "after the toolpath's last move, instead of before its first printing move",
            ),
        ],
        output: "Toolpath",
        doc: "New heater temperatures, set right before the toolpath prints (after the travel to its start) or right after it; printing carries on while they change. With nothing given it goes back to the printer's own (the Print tab's). Written as the printer's own commands: the pellet printer's three zone macros, the filament printer's `M104`. EasyGCode's foaming heats this way after the first layer.",
        run: Run::Op(printing::temperature),
    },
    Node {
        name: "wait_temperature",
        group: "Toolpaths",
        inputs: &[
            req("path", "Toolpath", ""),
            opt("nozzle", NUM, 0.0, "°C to wait for; 0 is the printer's own"),
            opt("up", NUM, 0.0, "°C: the pellet printer's upper zone"),
            opt("mid", NUM, 0.0, "°C: its middle zone"),
            opt("down", NUM, 0.0, "°C: its lower zone, at the nozzle"),
            opt(
                "cool",
                "Bool",
                0.0,
                "wait until cool enough instead of hot enough (pellet printer only)",
            ),
            opt(
                "at_end",
                "Bool",
                0.0,
                "after the toolpath's last move, instead of before its first printing move",
            ),
        ],
        output: "Toolpath",
        doc: "Waits, with the nozzle in place, until the heaters reach the temperatures: hot enough (the zone at the nozzle within 2 °C below; `M109` on a filament printer) or, with `cool`, cool enough (every zone within 2 °C above). Tolerant, as EasyGCode waits, because the zones settle near their target, never exactly on it. Put it on a primer line, so what oozes while waiting goes there.",
        run: Run::Op(printing::wait_temperature),
    },
    Node {
        name: "speed_override",
        group: "Toolpaths",
        inputs: &[
            req("path", "Toolpath", ""),
            req("percent", NUM, "1..1000; 100 is as written"),
            opt(
                "at_end",
                "Bool",
                0.0,
                "after the toolpath's last move, instead of before its first printing move",
            ),
        ],
        output: "Toolpath",
        doc: "The printer's speed override (`M220`, whole percent): every move from here on runs this much faster or slower, until the next override. With `extrusion_override` at 100 × 100 ÷ that percent, the material flow stays the same: EasyGCode's foaming prints foamed layers at 38 % extrusion and 263 % speed.",
        run: Run::Op(printing::speed_override),
    },
    Node {
        name: "extrusion_override",
        group: "Toolpaths",
        inputs: &[
            req("path", "Toolpath", ""),
            req("percent", NUM, "1..1000; 100 is as written"),
            opt(
                "at_end",
                "Bool",
                0.0,
                "after the toolpath's last move, instead of before its first printing move",
            ),
        ],
        output: "Toolpath",
        doc: "The printer's extrusion override (`M221`, whole percent): every move from here on extrudes this share of what it's written with, until the next override. Foaming material expands, so it needs less.",
        run: Run::Op(printing::extrusion_override),
    },
    Node {
        name: "spikes",
        group: "Toolpaths",
        inputs: &[
            req("path", "Toolpath", "usually a vase spiral"),
            all(
                "at",
                "List<Point>",
                "where the spikes go (the nearest place on the path)",
            ),
            req("width", "Length", "between the way out and the way back"),
            req(
                "length",
                "Length",
                "how far a spike sticks out; negative goes in",
            ),
            opt("angle", "Angle", 0.0, "tilts the spike up (degrees)"),
            opt("out_speed", "Number", 1.0, "speed factor on the way out"),
            opt("tip_speed", "Number", 1.0, "speed factor across the tip"),
            opt("in_speed", "Number", 1.0, "speed factor on the way back"),
            opt("dwell", "Number", 0.0, "pause at the tip, seconds"),
        ],
        output: "Toolpath",
        doc: "Spikes pulled out of the toolpath (EasyGCode's staples): at each place the path leaves the wall, runs out `length`, across the tip, pauses, and comes back `width` further on. Out is the right of travel: outward on a counter-clockwise wall. `scatter` makes good places.",
        run: Run::Op(printing::spikes),
    },
    Node {
        name: "wave",
        group: "Toolpaths",
        inputs: &[
            req("geometry", GEOMETRY, "a toolpath, curve or surface"),
            req(
                "amplitude",
                "Length | Field",
                "how far the wave pushes in and out; a field makes it vary",
            ),
            req(
                "count",
                "Number",
                "waves per turn (toolpath, spiral) or per loop (curve)",
            ),
            opt("phase", "Number", 0.0, "shift, in waves"),
        ],
        output: GEOMETRY,
        doc: "Pushes points in and out sideways in a sine wave. On a toolpath, whole counts stack into straight ribs, a fraction makes each layer shift (0.5 alternates, for a weave).",
        run: Run::Op(wave),
    },
];

/// Looks a node up by name.
pub fn find(name: &str) -> Option<&'static Node> {
    NODES.iter().find(|n| n.name == name)
}

/// `name(input: Kind, …) -> Output`, for messages and the catalog.
pub fn signature(node: &Node) -> String {
    let inputs: Vec<String> = node
        .inputs
        .iter()
        .map(|i| match (i.default, i.point) {
            (_, Some(p)) => format!(
                "{}: {} = [{}, {}, {}]",
                i.name,
                i.kind,
                easycad_lang::format_number(p[0]),
                easycad_lang::format_number(p[1]),
                easycad_lang::format_number(p[2])
            ),
            (Some(d), None) if i.kind == "Bool" => {
                format!("{}: {} = {}", i.name, i.kind, d != 0.0)
            }
            (Some(d), None) => format!(
                "{}: {} = {}",
                i.name,
                i.kind,
                easycad_lang::format_number(d)
            ),
            (None, None) => format!("{}: {}", i.name, i.kind),
        })
        .collect();
    format!("{}({}) -> {}", node.name, inputs.join(", "), node.output)
}

/// The node catalog as plain text: one signature, description and input notes per node.
pub fn catalog() -> String {
    let mut out = String::new();
    let mut group = "";
    for node in NODES {
        if node.group != group {
            group = node.group;
            out.push_str(&format!("\n## {group}\n\n"));
        }
        out.push_str(&signature(node));
        out.push('\n');
        out.push_str(&format!("    {}\n", node.doc));
        let notes: Vec<String> = node
            .inputs
            .iter()
            .filter(|i| !i.doc.is_empty())
            .map(|i| format!("{}: {}", i.name, i.doc))
            .collect();
        if !notes.is_empty() {
            out.push_str(&format!("    ({})\n", notes.join("; ")));
        }
    }
    out.trim_start().to_string()
}

// ---------------------------------------------------------------------------------------------
// Calling a node: list replication and typed access to the inputs
// ---------------------------------------------------------------------------------------------

/// Runs `node` on `args` (one per input), replicating over lists.
pub fn call(ctx: &mut Ctx, node: &'static Node, args: Vec<Value>) -> Result<Value, String> {
    let replicable = |i: usize, v: &Value| -> bool {
        match (node.inputs.get(i).map(|x| x.access), v) {
            (Some(Access::Item), Value::List(_)) => true,
            // `[0, 0, 1]` is one point; a list of points (or of such lists) runs once per point.
            (Some(Access::Point), Value::List(l)) => as_point(l).is_none(),
            // A list of lists runs a list input once per inner list, except that a list of
            // `[x, y, z]` lists is one list of points for an input that takes points.
            (Some(Access::List), Value::List(l)) => {
                let points = node
                    .inputs
                    .get(i)
                    .is_some_and(|x| x.kind.starts_with("List<Point"))
                    && l.items
                        .iter()
                        .all(|x| matches!(x, Value::List(p) if as_point(p).is_some()));
                !points
                    && !l.items.is_empty()
                    && l.items.iter().all(|x| matches!(x, Value::List(_)))
            }
            _ => false,
        }
    };
    replicate(ctx, &args, &replicable, &mut |ctx, values| {
        let out = match node.run {
            Run::Op(f) => f(ctx, &Args { node, values })?,
            Run::Math(f) => math(node, f, values)?,
        };
        if !out.is_finite() {
            return Err("the result has invalid coordinates (infinite or not a number); check the values going in".into());
        }
        Ok(out)
    })
}

/// A math node on numbers, or on fields point by point.
fn math(node: &Node, f: MathFn, values: &[Value]) -> Result<Value, String> {
    let mut numbers = Vec::with_capacity(values.len());
    let mut fields = Vec::with_capacity(values.len());
    for (i, v) in values.iter().enumerate() {
        match v {
            Value::Number(n) => {
                numbers.push(*n);
                fields.push(Rc::new(Field::constant(*n)));
            }
            Value::Field(fl) => fields.push(fl.clone()),
            other => {
                let name = node.inputs.get(i).map_or("?", |x| x.name);
                return Err(format!(
                    "`{name}` must be a number or a field, got {}",
                    other.kind()
                ));
            }
        }
    }
    if numbers.len() == values.len() {
        f(&numbers).map(Value::Number)
    } else {
        Ok(Value::Field(Rc::new(Field::map(f, fields)?)))
    }
}

fn arg(a: &[f64], i: usize) -> f64 {
    a.get(i).copied().unwrap_or(0.0)
}

/// What [`replicate`] runs once per set of items.
pub type Apply<'a> = dyn FnMut(&mut Ctx, &[Value]) -> Result<Value, String> + 'a;

/// Runs `f` once per item wherever `replicable` says an argument's list should be split up,
/// matching lists by the longest-list rule (shorter lists repeat their last item). Nested
/// lists replicate level by level.
pub fn replicate(
    ctx: &mut Ctx,
    args: &[Value],
    replicable: &dyn Fn(usize, &Value) -> bool,
    f: &mut Apply<'_>,
) -> Result<Value, String> {
    let lists: Vec<(usize, Rc<crate::value::List>)> = args
        .iter()
        .enumerate()
        .filter(|(i, v)| replicable(*i, v))
        .filter_map(|(i, v)| {
            if let Value::List(l) = v {
                Some((i, l.clone()))
            } else {
                None
            }
        })
        .collect();
    if lists.is_empty() {
        return f(ctx, args);
    }
    if lists.iter().any(|(_, l)| l.items.is_empty()) {
        return Value::list(Vec::new());
    }
    let n = lists.iter().map(|(_, l)| l.items.len()).max().unwrap_or(0);
    ctx.spend(n)?;
    let mut out = Vec::with_capacity(n);
    let mut sub = args.to_vec();
    for k in 0..n {
        for (i, l) in &lists {
            if let (Some(slot), Some(item)) =
                (sub.get_mut(*i), l.items.get(k.min(l.items.len() - 1)))
            {
                *slot = item.clone();
            }
        }
        out.push(replicate(ctx, &sub, replicable, f)?);
    }
    Value::list(out)
}

/// A node's inputs, after replication: item inputs hold single values.
pub struct Args<'a> {
    node: &'static Node,
    values: &'a [Value],
}

impl Args<'_> {
    fn name(&self, i: usize) -> &'static str {
        self.node.inputs.get(i).map_or("?", |x| x.name)
    }

    fn get(&self, i: usize) -> Result<&Value, String> {
        self.values
            .get(i)
            .ok_or_else(|| format!("missing `{}`", self.name(i)))
    }

    fn wrong(&self, i: usize, wanted: &str, got: &Value) -> String {
        format!("`{}` must be {wanted}, got {}", self.name(i), got.kind())
    }

    pub fn num(&self, i: usize) -> Result<f64, String> {
        match self.get(i)? {
            Value::Number(n) if n.is_finite() => Ok(*n),
            Value::Number(_) => Err(format!("`{}` is not a finite number", self.name(i))),
            other => Err(self.wrong(i, "a number", other)),
        }
    }

    /// A number that must be more than zero.
    pub fn positive(&self, i: usize) -> Result<f64, String> {
        let v = self.num(i)?;
        if v > 0.0 {
            Ok(v)
        } else {
            Err(format!("`{}` must be more than 0, got {v}", self.name(i)))
        }
    }

    /// A count: rounded to a whole number, which must be within `min..=max`.
    pub fn count(&self, i: usize, min: usize, max: usize) -> Result<usize, String> {
        let v = self.num(i)?.round();
        if v < min as f64 || v > max as f64 {
            return Err(format!(
                "`{}` must be between {min} and {max}, got {v}",
                self.name(i)
            ));
        }
        Ok(v as usize)
    }

    /// A number or a field, as a field.
    pub fn field(&self, i: usize) -> Result<Rc<Field>, String> {
        match self.get(i)? {
            Value::Number(n) if n.is_finite() => Ok(Rc::new(Field::constant(*n))),
            Value::Field(f) => Ok(f.clone()),
            other => Err(self.wrong(i, "a number or a field", other)),
        }
    }

    pub fn curve(&self, i: usize) -> Result<&Rc<Curve>, String> {
        match self.get(i)? {
            Value::Curve(c) => Ok(c),
            other => Err(self.wrong(i, "a curve", other)),
        }
    }

    pub fn surface(&self, i: usize) -> Result<&Rc<Surface>, String> {
        match self.get(i)? {
            Value::Surface(s) => Ok(s),
            other => Err(self.wrong(i, "a surface (from loft)", other)),
        }
    }

    pub fn toolpath(&self, i: usize) -> Result<&Rc<Toolpath>, String> {
        match self.get(i)? {
            Value::Toolpath(p) => Ok(p),
            other => Err(self.wrong(i, "a toolpath", other)),
        }
    }

    /// A list input's items (a single value counts as a list of one).
    pub fn items(&self, i: usize) -> Result<&[Value], String> {
        match self.get(i)? {
            Value::List(l) => Ok(&l.items),
            single => Ok(std::slice::from_ref(single)),
        }
    }

    /// A point or vector: a point value, or a list of 2 or 3 numbers.
    pub fn point(&self, i: usize) -> Result<P3, String> {
        match self.get(i)? {
            Value::Point(p) => Ok(*p),
            Value::List(l) => as_point(l).ok_or_else(|| {
                format!(
                    "`{}` must be a point: point(x, y, z) or [x, y, z]",
                    self.name(i)
                )
            }),
            other => Err(self.wrong(i, "a point (point(x, y, z) or [x, y, z])", other)),
        }
    }

    pub fn plane(&self, i: usize) -> Result<Plane, String> {
        match self.get(i)? {
            Value::Plane(p) => Ok(*p),
            other => Err(self.wrong(i, "a plane (from plane)", other)),
        }
    }

    /// Yes or no: `true`/`false`, or a number (0 is no).
    pub fn flag(&self, i: usize) -> Result<bool, String> {
        match self.get(i)? {
            Value::Bool(b) => Ok(*b),
            Value::Number(n) => Ok(*n != 0.0),
            other => Err(self.wrong(i, "true or false", other)),
        }
    }

    /// A text, like a dropdown's choice.
    pub fn text(&self, i: usize) -> Result<&str, String> {
        match self.get(i)? {
            Value::Text(t) => Ok(t),
            other => Err(self.wrong(i, "a text in quotes", other)),
        }
    }

    /// Whether input `i` holds a field (rather than a plain number).
    pub fn is_field(&self, i: usize) -> bool {
        matches!(self.values.get(i), Some(Value::Field(_)))
    }
}

/// A list of 2 or 3 numbers read as a point (z = 0 when missing).
pub fn as_point(l: &crate::value::List) -> Option<P3> {
    let n = |k: usize| match l.items.get(k) {
        Some(Value::Number(v)) => Some(*v),
        _ => None,
    };
    match l.items.len() {
        2 => Some([n(0)?, n(1)?, 0.0]),
        3 => Some([n(0)?, n(1)?, n(2)?]),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Node implementations
// ---------------------------------------------------------------------------------------------

/// The most numbers one `range` may make.
const MAX_RANGE: usize = 100_000;

fn range(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (start, end) = (a.num(0)?, a.num(1)?);
    let n = a.count(2, 1, MAX_RANGE)?;
    ctx.spend(n)?;
    let step = if n > 1 {
        (end - start) / (n - 1) as f64
    } else {
        0.0
    };
    // The last value is exactly `end`, without rounding drift.
    Value::list(
        (0..n)
            .map(|i| {
                Value::Number(if i + 1 == n && n > 1 {
                    end
                } else {
                    start + step * i as f64
                })
            })
            .collect(),
    )
}

fn count(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    Ok(Value::Number(a.items(0)?.len() as f64))
}

fn clamp(a: &[f64]) -> Result<f64, String> {
    let (v, lo, hi) = (arg(a, 0), arg(a, 1), arg(a, 2));
    if lo > hi {
        return Err(format!("`low` ({lo}) is above `high` ({hi})"));
    }
    Ok(v.clamp(lo, hi))
}

fn modulo(a: &[f64]) -> Result<f64, String> {
    let (x, y) = (arg(a, 0), arg(a, 1));
    if y == 0.0 {
        return Err("`b` must not be 0".into());
    }
    Ok(x.rem_euclid(y))
}

fn sqrt(a: &[f64]) -> Result<f64, String> {
    let x = arg(a, 0);
    if x < 0.0 {
        return Err(format!("can't take the square root of {x}"));
    }
    Ok(x.sqrt())
}

fn distance(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    if let Ok(p) = a.point(0) {
        return Ok(Value::Field(Rc::new(Field::distance(&Curve::new(
            vec![p],
            false,
        )))));
    }
    let c = a.curve(0)?;
    if c.points.is_empty() {
        return Err("the curve has no points".into());
    }
    Ok(Value::Field(Rc::new(Field::distance(c))))
}

fn polygon(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let sides = a.count(0, 3, 1000)?;
    let r = a.positive(1)?;
    ctx.spend(sides)?;
    Ok(Value::Curve(Rc::new(Curve::polygon(sides, r))))
}

fn star(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let tips = a.count(0, 2, 500)?;
    let outer = a.positive(1)?;
    let inner = a.num(2)?;
    if inner < 0.0 {
        return Err(format!("`inner` must be 0 or more, got {inner}"));
    }
    ctx.spend(2 * tips)?;
    Ok(Value::Curve(Rc::new(Curve::star(tips, outer, inner))))
}

fn smooth(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    let per = ctx.samples(8, 16);
    ctx.spend(c.points.len().saturating_mul(per))?;
    Ok(Value::Curve(Rc::new(c.smooth(per)?)))
}

fn spiral(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let outer = a.num(0)?;
    let spacing = a.field(1)?;
    let tolerance = a.num(2)?;
    let min_samples = a.count(3, 0, 100_000)?;
    let (start, turns) = (a.num(4)?, a.num(5)?);
    let max = ctx.remaining().min(MAX_PATH_POINTS);
    let curve = if turns != 0.0 {
        if outer != 0.0 {
            ctx.warn("`outer` is ignored when `turns` is given".into());
        }
        let constant = if a.is_field(1) { None } else { Some(a.num(1)?) };
        spiral::spiral_turns(
            start,
            turns,
            &spacing,
            constant,
            tolerance,
            min_samples,
            max,
        )?
    } else {
        if start != 0.0 {
            return Err("`start` needs `turns`: how many turns to make from there".into());
        }
        if outer == 0.0 {
            return Err("give `outer` (the radius to end near) or `turns`".into());
        }
        spiral::spiral(outer, &spacing, tolerance, min_samples, max)?
    };
    ctx.spend(curve.points.len())?;
    Ok(Value::Curve(Rc::new(curve)))
}

fn interlock(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    let bumps = a.count(1, 1, 999)?;
    let width = a.positive(2)?;
    let stretch = a.num(3)?;
    if stretch < 0.0 {
        return Err("`stretch` must be 0 (leave it) or a radius".into());
    }
    ctx.spend(c.points.len())?;
    let (curve, info) = spiral::interlock(c, bumps, width)?;
    let curve = if stretch > 0.0 {
        spiral::end_exactly_at(curve, stretch)
    } else {
        curve
    };
    if info.bumps != bumps {
        ctx.warn(format!(
            "bumps per turn must be odd so a peak always faces the next turn's dip; using {}",
            info.bumps
        ));
    }
    if info.overlap > 0.25 * width {
        ctx.warn(format!(
            "the spread is too short for the wiggles to meet exactly: neighbouring turns overlap by up to {} mm; widen the fade",
            easycad_lang::format_number((info.overlap * 1000.0).round() / 1000.0)
        ));
    }
    Ok(Value::Curve(Rc::new(curve)))
}

fn transform(ctx: &mut Ctx, g: &Value, x: &Xform, z_scale: f64) -> Result<Value, String> {
    match g {
        Value::Curve(c) => {
            ctx.spend(c.points.len())?;
            Ok(Value::Curve(Rc::new(c.transformed(x))))
        }
        Value::Surface(s) => {
            ctx.spend(s.point_count())?;
            Ok(Value::Surface(Rc::new(s.transformed(x))))
        }
        Value::Toolpath(p) => {
            ctx.spend(p.points.len())?;
            Ok(Value::Toolpath(Rc::new(p.transformed(x, z_scale)?)))
        }
        Value::Point(p) => Ok(Value::Point(x.apply(*p))),
        Value::Plane(p) => p
            .transformed(x)
            .map(Value::Plane)
            .ok_or_else(|| "this transform flattens the plane".into()),
        other => Err(format!(
            "works on curves, surfaces, toolpaths, points and planes, got {}",
            other.kind()
        )),
    }
}

/// asin/acos in degrees, for inputs within -1..1.
fn inverse_trig(x: f64, f: fn(f64) -> f64) -> Result<f64, String> {
    if (-1.0..=1.0).contains(&x) {
        Ok(f(x).to_degrees())
    } else {
        Err(format!("needs a value between -1 and 1, got {x}"))
    }
}

fn loft(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let curves = curves_of(a.items(0)?, "section")?;
    let m = curves.iter().map(|c| c.points.len()).max().unwrap_or(0);
    ctx.spend(curves.len().saturating_mul(m))?;
    Ok(Value::Surface(Rc::new(Surface::loft(&curves)?)))
}

/// The curves in a list input, or a message naming the first item that isn't one.
fn curves_of<'v>(items: &'v [Value], what: &str) -> Result<Vec<&'v Curve>, String> {
    items
        .iter()
        .enumerate()
        .map(|(i, v)| match v {
            Value::Curve(c) => Ok(c.as_ref()),
            other => Err(format!("{what} {} is {}, not a curve", i + 1, other.kind())),
        })
        .collect()
}

fn cap(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let s = a.surface(0)?;
    if !s.closed {
        return Err("needs a surface made of closed sections".into());
    }
    ctx.spend(s.point_count())?;
    Ok(Value::Surface(Rc::new(Surface {
        capped: true,
        ..s.as_ref().clone()
    })))
}

fn toolpath(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let curves = curves_of(a.items(0)?, "item")?;
    let settings = Settings::from_inputs(a.num(1)?, a.num(2)?, a.num(3)?, flow(a, 4)?)?;
    let path = toolpath::from_curves(&curves, settings, ctx.remaining().min(MAX_PATH_POINTS))?;
    ctx.spend(path.points.len())?;
    Ok(Value::Toolpath(Rc::new(path)))
}

fn vase_spiral(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let wall = a.surface(0)?;
    let settings = Settings::from_inputs(a.num(2)?, a.num(1)?, a.num(3)?, flow(a, 4)?)?;
    let path = toolpath::vase_spiral(
        wall,
        settings,
        a.flag(5)?,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    ctx.spend(path.points.len())?;
    Ok(Value::Toolpath(Rc::new(path)))
}

fn vase_revolve(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let profile = a.curve(0)?;
    let (rise, width, flow) = (a.field(1)?, a.field(2)?, a.field(4)?);
    let read = |f: &Rc<Field>, what: &'static str| {
        let f = f.clone();
        move |p: P3| -> Result<f64, String> {
            let v = f.at(p);
            if v.is_finite() {
                Ok(v)
            } else {
                Err(format!("`{what}` has no value at height {:.2}", p[2]))
            }
        }
    };
    let (rise, width, flow) = (
        read(&rise, "layer"),
        read(&width, "width"),
        read(&flow, "flow"),
    );
    let path = toolpath::vase_revolve(
        profile,
        &toolpath::PerTurn {
            rise: &rise,
            width: &width,
            flow: &flow,
        },
        a.num(3)?,
        a.num(5)?,
        a.num(6)?,
        a.num(7)?,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    ctx.spend(path.points.len())?;
    Ok(Value::Toolpath(Rc::new(path)))
}

/// A `flow` input: 0 (not set) or a volumetric flow in mm³/s.
fn flow(a: &Args, i: usize) -> Result<f64, String> {
    let q = a.num(i)?;
    if q < 0.0 {
        return Err(format!("`flow` must be 0 (not set) or more, got {q}"));
    }
    Ok(q)
}

fn helix(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let circle = a.curve(0)?;
    let (pitch, height) = (a.num(1)?, a.num(2)?);
    let settings = Settings::from_inputs(a.num(3)?, pitch, a.num(4)?, flow(a, 5)?)?;
    let path = toolpath::helix(
        circle,
        pitch,
        height,
        settings,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    ctx.spend(path.points.len())?;
    Ok(Value::Toolpath(Rc::new(path)))
}

fn stack(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let path = a.toolpath(0)?;
    let n = a.count(1, 0, 10_000)?;
    let out = toolpath::stack(
        path,
        n,
        a.num(2)?,
        a.num(3)?,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    ctx.spend(out.points.len())?;
    Ok(Value::Toolpath(Rc::new(out)))
}

fn join(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let paths = a
        .items(0)?
        .iter()
        .enumerate()
        .map(|(i, v)| match v {
            Value::Toolpath(p) => Ok(p.as_ref()),
            other => Err(format!(
                "item {} is {}, not a toolpath",
                i + 1,
                other.kind()
            )),
        })
        .collect::<Result<Vec<_>, String>>()?;
    if paths.is_empty() {
        return Err("needs at least one toolpath".into());
    }
    let out = toolpath::join(
        &paths,
        a.num(1)?,
        a.num(2)?,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    ctx.spend(out.points.len())?;
    Ok(Value::Toolpath(Rc::new(out)))
}

fn wave(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (amplitude, count, phase) = (a.field(1)?, a.num(2)?, a.num(3)?);
    let max = ctx.remaining().min(MAX_PATH_POINTS);
    let out = match a.get(0)? {
        Value::Toolpath(p) => Value::Toolpath(Rc::new(toolpath::wave_path(
            p, &amplitude, count, phase, max,
        )?)),
        Value::Curve(c) => Value::Curve(Rc::new(toolpath::wave_curve(
            c, &amplitude, count, phase, max,
        )?)),
        Value::Surface(s) => {
            let mut sections = Vec::with_capacity(s.sections.len());
            for sec in &s.sections {
                let ring = Curve::new(sec.clone(), s.closed);
                let waved = toolpath::wave_curve(&ring, &amplitude, count, phase, ctx.remaining())?;
                ctx.spend(waved.points.len())?;
                sections.push(waved.points);
            }
            Value::Surface(Rc::new(Surface {
                sections,
                ..s.as_ref().clone()
            }))
        }
        other => {
            return Err(format!(
                "works on toolpaths, curves and surfaces, got {}",
                other.kind()
            ));
        }
    };
    if let Value::Toolpath(p) = &out {
        ctx.spend(p.points.len())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::Quality;

    #[test]
    fn names_are_unique_and_documented() {
        for (i, n) in NODES.iter().enumerate() {
            assert!(
                NODES[..i].iter().all(|m| m.name != n.name),
                "duplicate node {}",
                n.name
            );
            assert!(!n.doc.is_empty(), "{} has no doc", n.name);
            assert!(
                !easycad_lang::RESERVED.contains(&n.name),
                "{} is a reserved word",
                n.name
            );
        }
        let cat = catalog();
        assert!(cat.starts_with("## Numbers & lists"));
        assert!(cat.contains("vase_spiral(wall: Surface, layer: Length, width: Length, speed: Speed = 40, flow: Number = 0, flat_top: Bool = true) -> Toolpath"));
        assert!(cat.contains("fade(distance: Number | Field, inside: Number | Field, outside: Number | Field, band: Number | Field = 0) -> Number | Field"));
    }

    #[test]
    fn replication_follows_the_longest_list() {
        let mut ctx = Ctx::new(Quality::Preview);
        let node = find("lerp").unwrap();
        let n = |x: f64| Value::Number(x);
        let ts = Value::list(vec![n(0.0), n(0.5), n(1.0)]).unwrap();
        let ends = Value::list(vec![n(10.0), n(20.0)]).unwrap();
        let out = call(&mut ctx, node, vec![n(0.0), ends, ts]).unwrap();
        // b: [10, 20, 20 (repeats its last item)], t: [0, 0.5, 1]
        assert_eq!(out, Value::list(vec![n(0.0), n(10.0), n(20.0)]).unwrap());
        let empty = Value::list(vec![]).unwrap();
        assert_eq!(
            call(&mut ctx, node, vec![n(0.0), n(1.0), empty.clone()]).unwrap(),
            empty
        );
    }

    #[test]
    fn math_on_fields_gives_fields() {
        let mut ctx = Ctx::new(Quality::Preview);
        let c = Value::Curve(Rc::new(Curve::circle(10.0, 8)));
        let d = call(&mut ctx, find("distance").unwrap(), vec![c]).unwrap();
        let out = call(
            &mut ctx,
            find("clamp").unwrap(),
            vec![d, Value::Number(-1.0), Value::Number(1.0)],
        )
        .unwrap();
        let Value::Field(f) = out else {
            panic!("expected a field")
        };
        assert_eq!(f.at([0.0, 0.0, 0.0]), -1.0);
        assert_eq!(f.at([10.5, 0.0, 0.0]), 0.5);
        let curve = Value::Curve(Rc::new(Curve::circle(1.0, 8)));
        assert_eq!(
            call(&mut ctx, find("sin").unwrap(), vec![curve]).unwrap_err(),
            "`angle` must be a number or a field, got a curve"
        );
    }

    #[test]
    fn lists_of_lists_loft_once_per_inner_list() {
        let mut ctx = Ctx::new(Quality::Preview);
        let c = |z: f64| {
            Value::Curve(Rc::new(
                Curve::circle(5.0, 8).transformed(&Xform::translate(0.0, 0.0, z)),
            ))
        };
        let pair = || Value::list(vec![c(0.0), c(10.0)]).unwrap();
        let out = call(
            &mut ctx,
            find("loft").unwrap(),
            vec![Value::list(vec![pair(), pair()]).unwrap()],
        )
        .unwrap();
        let Value::List(l) = out else {
            panic!("expected a list")
        };
        assert_eq!(l.items.len(), 2);
        assert!(matches!(l.items[0], Value::Surface(_)));
    }

    #[test]
    fn helpful_errors() {
        let mut ctx = Ctx::new(Quality::Preview);
        let n = |x: f64| Value::Number(x);
        let star = find("star").unwrap();
        assert_eq!(
            call(&mut ctx, star, vec![n(7.0), n(-1.0), n(1.0)]).unwrap_err(),
            "`outer` must be more than 0, got -1"
        );
        assert_eq!(
            call(&mut ctx, star, vec![n(1.0), n(5.0), n(1.0)]).unwrap_err(),
            "`points` must be between 2 and 500, got 1"
        );
        let curve = Value::Curve(Rc::new(Curve::circle(1.0, 8)));
        assert_eq!(
            call(&mut ctx, star, vec![curve, n(5.0), n(1.0)]).unwrap_err(),
            "`points` must be a number, got a curve"
        );
        assert!(
            call(
                &mut ctx,
                find("range").unwrap(),
                vec![n(0.0), n(1.0), n(1e9)]
            )
            .is_err()
        );
        assert_eq!(
            call(
                &mut ctx,
                find("scale").unwrap(),
                vec![
                    n(1.0),
                    n(2.0),
                    n(1.0),
                    n(1.0),
                    n(1.0),
                    Value::Point([0.0; 3])
                ]
            )
            .unwrap_err(),
            "works on curves, surfaces, toolpaths, points and planes, got a number"
        );
    }

    #[test]
    fn range_hits_both_ends() {
        let mut ctx = Ctx::new(Quality::Preview);
        let out = call(
            &mut ctx,
            find("range").unwrap(),
            vec![Value::Number(0.0), Value::Number(0.3), Value::Number(4.0)],
        )
        .unwrap();
        let Value::List(l) = out else {
            panic!("expected a list")
        };
        assert_eq!(l.items.last(), Some(&Value::Number(0.3)));
        assert_eq!(l.items.len(), 4);
    }
}
