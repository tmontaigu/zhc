//! Pipeline meta-dialect for the ZHC compiler IR.
//!
//! Unlike the other dialects in this crate, which model FHE computations,
//! this dialect models the compilation flow itself: values are compilation
//! artifacts (source circuit, dialect IRs, target configurations, output
//! streams, metrics) and instructions are the compilation steps that
//! produce them. An `IR<PipelineLang>` program is thus a dataflow graph of
//! the whole compilation pipeline, which downstream crates can evaluate
//! step by step or render for inspection.
//!
//! [`PipelineLang`] is the dialect tag binding [`PipelineTypeSystem`] (the
//! artifact kinds) to [`PipelineInstructionSet`] (the compilation steps).
//! Every step also carries an [`Affinity`] identifying the pipeline branch
//! it belongs to: the frontend shared by every target, or one of the
//! single-HPU, multi-HPU, and software-VM backends.

mod affinity;
mod dialect;
mod instruction_set;
mod type_system;

pub use affinity::*;
pub use dialect::*;
pub use instruction_set::*;
pub use type_system::*;
