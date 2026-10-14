//! EasyCAD engine core.
//!
//! Phase 0 holds one hard-coded "project", the twisted star vase ([`vase`]), built the way a
//! Grasshopper definition would build it. It proves the pipeline end to end: a NURBS outline
//! (via `curvo`), a preview mesh, a continuous vase-mode toolpath, and print statistics from
//! EasyGCode's bead model. In Phase 1 the same steps become nodes driven by an `.easycad`
//! project file.
//!
//! Rules (see `CLAUDE.md`): no UI, no I/O, no panics. Every failure is an [`Error`].

pub mod bead;
pub mod error;
pub mod geom;
pub mod vase;

pub use error::Error;
