use crate::{
    Dialect,
    visualization::layoutlang::{LayoutInstructionSet, LayoutTypeSystem},
};

/// Dialect tag for the layout language.
///
/// Unit struct binding [`LayoutTypeSystem`] and [`LayoutInstructionSet`] into a concrete
/// [`Dialect`] implementation.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct LayoutDialect;

impl Dialect for LayoutDialect {
    type TypeSystem = LayoutTypeSystem;
    type InstructionSet = LayoutInstructionSet;
}
