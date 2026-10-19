//! The files a recipe makes: G-code for each toolpath it prints (each part of the project), and
//! the file of each `export` line (STL for shapes, G-code for a toolpath). The app asks for them through the WebAssembly
//! facade; the command line and MCP will ask the same way.

use crate::eval::{Evaluation, ExportKind};
use crate::gcode::{self, Profile};
use crate::stl;
use crate::surface::Mesh;
use crate::value::Value;

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
                _ => return Err("this export doesn't match its file type".into()),
            };
            Ok(File {
                name: export.file.clone(),
                bytes,
            })
        }
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

    const VASE: &str = include_str!("../../../web/templates/twisted-vase.easycad");

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
}
