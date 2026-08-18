//! Layout dialect for IR visualization.
//!
//! This dialect is the first stage of the drawing pipeline behind
//! [`draw_ir_to_svg`](super::draw_ir_to_svg) and its variants: an IR of any dialect, annotated
//! with a [`Hierarchy`](super::Hierarchy) per operation, is restructured into an
//! `IR<LayoutDialect>` describing what to draw rather than what to compute. Operations become
//! generic display nodes carrying their pre-formatted text ([`OpContent`]), and hierarchy
//! branches become [`Group`](LayoutInstructionSet::Group) nodes holding a nested sub-IR, with
//! explicit boundary nodes where values cross into or out of a group.
//!
//! [`LayoutDialect`] is the dialect tag binding [`LayoutTypeSystem`] to
//! [`LayoutInstructionSet`]. The type system is trivial — every edge carries the single
//! [`Value`](LayoutTypeSystem::Value) type — since only the shape of the dataflow matters for
//! drawing.
//!
//! [`generate_layout_ir`] performs the restructuring, and [`annotate_layout`] attaches
//! per-operation [`VisualAnnotation`](super::VisualAnnotation)s afterwards, keyed by the
//! original operation ids that the layout nodes preserve.

mod annotate;
mod dialect;
mod generate;
mod instruction_set;
mod type_system;

pub use annotate::*;
pub use dialect::*;
pub use generate::*;
pub use instruction_set::*;
pub use type_system::*;
