//! EasyCAD engine core: runs `.easycad` recipes.
//!
//! A recipe (parsed by `easycad-lang`) is a small dataflow program. [`eval::evaluate`] runs its
//! steps in dependency order through the node library ([`nodes`]): curves ([`curve`]), lofted
//! surfaces ([`surface`]) and toolpaths with EasyGCode's vase-mode conventions and bead model
//! ([`toolpath`], [`bead`]). The result is a drawing for the 3D view, print statistics, the
//! settings window's contents and diagnostics.
//!
//! Rules (see `CLAUDE.md`): no UI, no I/O, no panics. Bad input of any kind becomes a message.

pub mod bead;
pub mod curve;
pub mod eval;
pub mod geom;
pub mod nodes;
pub mod surface;
pub mod toolpath;
pub mod value;

pub use eval::{
    Diagnostic, Evaluation, ParamInfo, PrintInfo, Quality, Severity, StepInfo, evaluate,
};

#[cfg(test)]
mod eval_tests;
