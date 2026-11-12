//! The files a recipe makes: G-code for each toolpath it prints (each part of the project), and
//! the file of each `export` line (STL or STEP for shapes, G-code for a toolpath). The app asks
//! for them through the WebAssembly facade; the command line and MCP will ask the same way.

use crate::eval::{Evaluation, ExportKind, ParamInfo};
use crate::gcode::{self, Profile};
use crate::surface::{Mesh, Surface};
use crate::value::Value;
use crate::{step, stl};
use easycad_lang::{Literal, format_number};

/// Which file to make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    /// G-code for the toolpath of one of the recipe's `print` lines, by its index in
    /// `Evaluation::prints` (0: the first).
    Print(usize),
    /// The file of one of the recipe's `export` lines, by its index in `Evaluation::exports`.
    Export(usize),
}

/// A file to save. `name` is empty when the recipe doesn't name it (the printed G-code; with
/// several parts, the part's name).
#[derive(Debug, Clone, PartialEq)]
pub struct File {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Makes a file from a recipe's evaluation. A broken recipe makes nothing: half a shape or half
/// a toolpath is worse than a clear message.
pub fn make(e: &Evaluation, which: Which, profile: &Profile) -> Result<File, String> {
    if let Some(d) = e.errors().next() {
        return Err(format!(
            "fix the recipe first (line {}: {})",
            d.line, d.message
        ));
    }
    let title = e.project.as_deref().unwrap_or("project");
    match which {
        Which::Print(i) => {
            if e.prints.is_empty() {
                return Err(
                    "this recipe doesn't print anything: add a line like `print path`".into(),
                );
            }
            let print = e
                .prints
                .get(i)
                .ok_or("this recipe has no such part to print")?;
            // One part: the project's file. Several: each named after its part.
            let (name, text) = if e.prints.len() == 1 {
                (
                    String::new(),
                    gcode::write(&print.path, profile, title)?.text,
                )
            } else {
                let title = format!("{title} — {}", print.name);
                (
                    print.name.clone(),
                    gcode::write(&print.path, profile, &title)?.text,
                )
            };
            Ok(File {
                name,
                bytes: text.into_bytes(),
            })
        }
        Which::Export(i) => {
            let export = e.exports.get(i).ok_or("this recipe has no such export")?;
            let bytes = match (&export.kind, &export.value) {
                (ExportKind::Gcode, Value::Toolpath(p)) => {
                    gcode::write(p, profile, title)?.text.into_bytes()
                }
                (ExportKind::Stl, value) => {
                    let mut mesh = Mesh::default();
                    add_surfaces(value, &mut mesh);
                    stl::binary(&mesh, title)
                }
                (ExportKind::Step, value) => {
                    let mut surfaces = Vec::new();
                    collect_surfaces(value, &mut surfaces);
                    let mut description = vec![format!("EasyCAD: {title}"), "units: mm".into()];
                    description.extend(settings(&e.params));
                    step::write(&surfaces, title, &export.file, &description)?.into_bytes()
                }
                _ => return Err("this export doesn't match its file type".into()),
            };
            Ok(File {
                name: export.file.clone(),
                bytes,
            })
        }
    }
}

/// Every surface and solid in a value.
fn collect_surfaces<'a>(value: &'a Value, out: &mut Vec<&'a Surface>) {
    match value {
        Value::Surface(s) => out.push(s),
        Value::List(l) => l.items.iter().for_each(|v| collect_surfaces(v, out)),
        _ => {}
    }
}

/// The settings a file was made with, as `Label: value`, for its header: the ones that apply
/// (a setting shown only for another choice is left out). A thread's pitch, for one, is printed
/// by the bead rather than modelled, so whoever slices the shape needs to read it here.
fn settings(params: &[ParamInfo]) -> Vec<String> {
    let value_of = |name: &str| params.iter().find(|p| p.name == name).map(|p| &p.value);
    let applies = |p: &ParamInfo| {
        let Some(c) = &p.when else { return true };
        let Some(v) = value_of(&c.name) else {
            return true;
        };
        let hit = if c.values.is_empty() {
            match v {
                Literal::Bool(b) => *b,
                Literal::Number(n) => *n != 0.0,
                Literal::Text(t) => !t.is_empty(),
                Literal::List(l) => !l.is_empty(),
            }
        } else {
            c.values.contains(v)
        };
        hit != c.not
    };
    params
        .iter()
        .filter(|p| applies(p))
        .map(|p| {
            let value = literal(&p.value);
            let unit = if p.unit.is_empty() || matches!(p.value, Literal::List(_)) {
                String::new()
            } else {
                format!(" {}", p.unit)
            };
            let label = if p.label.is_empty() {
                &p.name
            } else {
                &p.label
            };
            match &p.part {
                Some(part) => format!("{part}: {label}: {value}{unit}"),
                None => format!("{label}: {value}{unit}"),
            }
        })
        .collect()
}

fn literal(v: &Literal) -> String {
    match v {
        Literal::Number(n) => format_number(*n),
        Literal::Bool(b) => if *b { "on" } else { "off" }.into(),
        Literal::Text(t) => t.clone(),
        Literal::List(l) => format!("[{}]", l.iter().map(literal).collect::<Vec<_>>().join(", ")),
    }
}

/// Every surface and solid in a value, meshed at full quality.
fn add_surfaces(value: &Value, mesh: &mut Mesh) {
    match value {
        Value::Surface(s) => mesh.append(&s.mesh(2)),
        Value::List(l) => l.items.iter().for_each(|v| add_surfaces(v, mesh)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{Quality, evaluate};
    use crate::gcode::Mode;
    use std::collections::HashMap;

    const VASE: &str = include_str!("../../../fixtures/recipes/twisted-vase.easycad");

    fn p1p() -> Profile {
        Profile {
            mode: Mode::Filament,
            bed: [256.0, 256.0, 256.0],
            ..Profile::default()
        }
    }

    /// Triangles of a binary STL, as corner positions.
    fn triangles(stl: &[u8]) -> Vec<[[f32; 3]; 3]> {
        let f = |at: usize| f32::from_le_bytes([stl[at], stl[at + 1], stl[at + 2], stl[at + 3]]);
        let count = u32::from_le_bytes([stl[80], stl[81], stl[82], stl[83]]) as usize;
        assert_eq!(stl.len(), 84 + 50 * count);
        (0..count)
            .map(|t| {
                let at = 84 + 50 * t + 12; // after the normal
                [0, 1, 2].map(|c| [0, 1, 2].map(|k| f(at + 12 * c + 4 * k)))
            })
            .collect()
    }

    #[test]
    fn the_vase_template_makes_gcode_and_a_closed_stl() {
        let e = evaluate(VASE, Quality::Final);
        let gcode = make(&e, Which::Print(0), &p1p()).unwrap();
        let text = String::from_utf8(gcode.bytes).unwrap();
        assert!(gcode.name.is_empty());
        assert!(text.starts_with("; EasyCAD — Twisted star vase\n"));
        assert!(text.contains("; --- start G-code (filament / Marlin) ---"));
        assert!(text.trim_end().ends_with("M84 ; disable steppers"));

        assert_eq!(e.exports.len(), 1);
        let stl = make(&e, Which::Export(0), &p1p()).unwrap();
        assert_eq!(stl.name, "twisted-vase.stl");
        let tris = triangles(&stl.bytes);
        assert!(tris.len() > 1000);
        // Watertight with consistent winding: every edge is used once in each direction.
        let key = |p: [f32; 3]| p.map(f32::to_bits);
        let mut edges: HashMap<([u32; 3], [u32; 3]), i32> = HashMap::new();
        let mut volume = 0.0f64;
        for t in &tris {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *edges.entry((key(a), key(b))).or_default() += 1;
                *edges.entry((key(b), key(a))).or_default() -= 1;
            }
            let [a, b, c] = t.map(|p| p.map(f64::from));
            volume += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0;
        }
        assert!(
            edges.values().all(|&n| n == 0),
            "the mesh has holes or flipped triangles"
        );
        // Facing outward: a positive volume, about that of a 160 mm tall star of radius ≤ 45 mm.
        assert!(
            volume > 200_000.0 && volume < std::f64::consts::PI * 45.0 * 45.0 * 160.0,
            "{volume}"
        );
    }

    #[test]
    fn nothing_is_made_from_a_broken_or_empty_recipe() {
        let broken = evaluate("x = (1 +", Quality::Final);
        assert!(
            make(&broken, Which::Print(0), &p1p())
                .unwrap_err()
                .starts_with("fix the recipe first (line 1")
        );
        let empty = evaluate("c = circle(5)\nshow c", Quality::Final);
        assert!(
            make(&empty, Which::Print(0), &p1p())
                .unwrap_err()
                .contains("print path")
        );
        assert_eq!(
            make(&empty, Which::Export(0), &p1p()).unwrap_err(),
            "this recipe has no such export"
        );
        // The printer's bed is checked: a 256 mm bed can't hold a 400 mm disc.
        let wide = "w = loft([circle(200), circle(200) |> move(z: 2)])\np = vase_spiral(w, layer: 1, width: 2)\nprint p";
        let wide = evaluate(wide, Quality::Final);
        assert!(
            make(&wide, Which::Print(0), &p1p())
                .unwrap_err()
                .contains("leaves the bed")
        );
    }

    #[test]
    fn each_part_is_its_own_gcode_file() {
        let src = "project \"Shaker\"\nw = loft([circle(10), circle(10) |> move(z: 3)])\nbase = vase_spiral(w, layer: 1, width: 2)\nlid = vase_spiral(w |> scale(1.2), layer: 1, width: 2)\nprint base\nprint lid";
        let e = evaluate(src, Quality::Final);
        let base = make(&e, Which::Print(0), &p1p()).unwrap();
        let lid = make(&e, Which::Print(1), &p1p()).unwrap();
        assert_eq!((base.name.as_str(), lid.name.as_str()), ("base", "lid"));
        let text = String::from_utf8(lid.bytes).unwrap();
        assert!(
            text.starts_with("; EasyCAD — Shaker — lid\n"),
            "{}",
            &text[..40]
        );
        assert_eq!(
            make(&e, Which::Print(2), &p1p()).unwrap_err(),
            "this recipe has no such part to print"
        );
        // One part: the project's own file, unnamed.
        let one = evaluate(&src.replace("\nprint lid", ""), Quality::Final);
        assert_eq!(make(&one, Which::Print(0), &p1p()).unwrap().name, "");
    }

    #[test]
    fn a_toolpath_exports_as_gcode() {
        let src =
            "c = circle(10)\np = toolpath(c, width: 1, layer: 0.5)\nexport p as \"ring.gcode\"";
        let e = evaluate(src, Quality::Final);
        let file = make(&e, Which::Export(0), &Profile::default()).unwrap();
        assert_eq!(file.name, "ring.gcode");
        assert!(
            String::from_utf8(file.bytes)
                .unwrap()
                .contains("; --- start G-code (pellet / Klipper) ---")
        );
    }

    /// The signed volume of a binary STL's triangles: positive when they face outward.
    fn stl_volume(stl: &[u8]) -> f64 {
        triangles(stl)
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|p| p.map(f64::from));
                (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]))
                    / 6.0
            })
            .sum()
    }

    /// The lampshade's wall, a line thick, as an exact STEP solid of cones, cylinders and
    /// planes; its header says what it was made with (the socket gives the thread's pitch).
    #[test]
    fn the_lampshade_exports_its_wall_as_a_step_solid() {
        let src = include_str!("../../../web/templates/lampshade.easycad");
        let e = evaluate(src, Quality::Final);
        let i = e
            .exports
            .iter()
            .position(|x| x.kind == ExportKind::Step)
            .unwrap();
        assert_eq!(e.exports[i].file, "lampshade.step");
        let file = make(&e, Which::Export(i), &p1p()).unwrap();
        assert_eq!(file.name, "lampshade.step");
        let text = String::from_utf8(file.bytes).unwrap();
        assert!(text.starts_with("ISO-10303-21;"));
        let counts = crate::step::tests::check(&text);
        assert_eq!(counts["MANIFOLD_SOLID_BREP"], 1);
        assert_eq!(counts["CLOSED_SHELL"], 1);
        assert!(counts["CONICAL_SURFACE"] > 10, "{counts:?}");
        assert!(counts["CYLINDRICAL_SURFACE"] >= 2, "{counts:?}");
        assert!(!counts.contains_key("OPEN_SHELL"));
        assert!(text.contains("'EasyCAD: Lampshade'"));
        assert!(text.contains("'Socket: E27'"), "{}", &text[..2000]);
        assert!(text.contains("'Line width: 6 mm'"));
        // The custom pitch only applies to a custom socket: left out.
        assert!(!text.contains("Custom thread pitch"));
        // The same solid as STL: closed, facing out, about the wall's volume.
        let as_stl = evaluate(
            &src.replace("\"lampshade.step\"", "\"lampshade.stl\""),
            Quality::Final,
        );
        let stl = make(&as_stl, Which::Export(i), &p1p()).unwrap();
        let volume = stl_volume(&stl.bytes);
        assert!(volume > 50_000.0, "{volume}");
        // The G-code doesn't change for it: the export is a side branch.
        let without = evaluate(
            &src.replace("export wall_solid as \"lampshade.step\"\n", ""),
            Quality::Final,
        );
        assert!(without.exports.is_empty());
        assert_eq!(
            make(&without, Which::Print(0), &p1p()).unwrap().bytes,
            make(&e, Which::Print(0), &p1p()).unwrap().bytes
        );
    }

    /// Each of the lamp's shades exports its own STEP file, named after its part, where it goes
    /// in the lamp.
    #[test]
    fn each_lamp_shade_exports_its_own_step_file() {
        let lamp = include_str!("../../../web/templates/lamp.easycad");
        let src = easycad_lang::add_part(lamp, "shade", "top", &[]).unwrap();
        let e = evaluate(&src, Quality::Final);
        let steps: Vec<(usize, &str)> = e
            .exports
            .iter()
            .enumerate()
            .filter(|(_, x)| x.kind == ExportKind::Step)
            .map(|(i, x)| (i, x.file.as_str()))
            .collect();
        assert_eq!(
            steps.iter().map(|s| s.1).collect::<Vec<_>>(),
            ["main-shade.step", "top-shade.step"]
        );
        for (i, name) in steps {
            let file = make(&e, Which::Export(i), &p1p()).unwrap();
            assert_eq!(file.name, name);
            let text = String::from_utf8(file.bytes).unwrap();
            let counts = crate::step::tests::check(&text);
            assert_eq!(counts["MANIFOLD_SOLID_BREP"], 1, "{name}");
        }
        // Closed at the end: the disc reaches the axis, still one solid.
        let closed = easycad_lang::set_values(
            lamp,
            &[("main.closed".into(), easycad_lang::Literal::Bool(true))],
        )
        .unwrap();
        let e = evaluate(&closed, Quality::Final);
        let i = e
            .exports
            .iter()
            .position(|x| x.kind == ExportKind::Step)
            .unwrap();
        let text = String::from_utf8(make(&e, Which::Export(i), &p1p()).unwrap().bytes).unwrap();
        assert_eq!(crate::step::tests::check(&text)["MANIFOLD_SOLID_BREP"], 1);
    }

    #[test]
    fn step_and_stp_are_export_kinds_and_others_are_named() {
        let src = "w = revolve(polyline([[10, 0, 0], [12, 0, 30]]))\nexport w as \"a.STP\"\nexport w as \"b.obj\"";
        let e = evaluate(src, Quality::Final);
        assert_eq!(e.exports.len(), 1);
        assert_eq!(e.exports[0].kind, ExportKind::Step);
        assert!(
            e.diagnostics
                .iter()
                .any(|d| d.message.contains(".stl or .step (for shapes)"))
        );
        // A surface without thickness is written as a surface model.
        let text = String::from_utf8(make(&e, Which::Export(0), &p1p()).unwrap().bytes).unwrap();
        assert_eq!(crate::step::tests::check(&text)["OPEN_SHELL"], 1);
    }

    /// Thickened, a solid can't be sliced, capped or thickened again: each says so.
    #[test]
    fn a_thickened_wall_is_for_export_only() {
        let base = "w = revolve(polyline([[10, 0, 0], [12, 0, 30]]))\ns = thicken(w, 2)\n";
        for (tail, says) in [
            ("p = vase_spiral(s, layer: 1, width: 2)", "thicken"),
            ("c = cap(s)", "already a solid"),
            ("t = thicken(s, 1)", "already a solid"),
            ("t = thicken(w, 0)", "more than 0"),
            (
                "z = revolve(polyline([[20, 0, 0], [20, 0, 10], [22, 0, 10], [22, 0, 0]]))\nt = thicken(z, 6)",
                "folds over itself",
            ),
        ] {
            let e = evaluate(&format!("{base}{tail}"), Quality::Final);
            assert!(
                e.errors().any(|d| d.message.contains(says)),
                "{tail}: {:?}",
                e.diagnostics
            );
        }
    }
}
