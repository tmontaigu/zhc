use std::fmt::Display;

use crate::DialectTypeSystem;

/// Type system for the layout dialect.
///
/// Single-typed: every edge carries the sole `Value` variant. Layout IRs only encode the shape
/// of the dataflow to draw, so no further type distinction is needed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LayoutTypeSystem {
    Value,
}

impl Display for LayoutTypeSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutTypeSystem::Value => write!(f, "Value"),
        }
    }
}

impl DialectTypeSystem for LayoutTypeSystem {}
