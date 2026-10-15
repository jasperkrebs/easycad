//! EasyCAD engine core: runs `.easycad` recipes.
//!
//! A recipe (parsed by `easycad-lang`) is a small dataflow program. [`eval::evaluate`] runs its
//! steps in dependency order through the node library ([`nodes`]): curves ([`curve`],
//! [`spiral`]), values that vary over space ([`field`]), lofted surfaces ([`surface`]) and
//! toolpaths with EasyGCode's conventions and bead model ([`toolpath`], [`bead`]). The result is
//! a drawing for the 3D view, print statistics, the settings window's contents and diagnostics.
//! [`files`] turns it into files to save: G-code ([`gcode`]) and STL ([`stl`]).
//!
//! Rules (see `CLAUDE.md`): no UI, no I/O, no panics. Bad input of any kind becomes a message.

pub mod bead;
pub mod curve;
pub mod eval;
pub mod field;
pub mod files;
pub mod gcode;
pub mod geom;
pub mod nodes;
pub mod spiral;
pub mod stl;
pub mod surface;
pub mod toolpath;
pub mod value;

pub use eval::{
    Diagnostic, Evaluation, Export, ExportKind, ParamInfo, PrintInfo, Quality, Severity, StepInfo,
    evaluate,
};

#[cfg(test)]
mod eval_tests;
#[cfg(test)]
mod golden_tests;
