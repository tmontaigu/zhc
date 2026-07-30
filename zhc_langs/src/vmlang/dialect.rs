use zhc_ir::{Dialect, cse::AllowCse};

/// Dialect tag for the experimental software VM language.
///
/// Unit struct binding [`VmTypeSystem`](super::VmTypeSystem) and
/// [`VmInstructionSet`](super::VmInstructionSet) into a concrete
/// [`Dialect`] implementation. The dialect opts into common
/// subexpression elimination through [`AllowCse`] with the default
/// argument handling, so commuted forms of the commutative
/// instructions are not recognized as equivalent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VmLang;

impl Dialect for VmLang {
    type TypeSystem = super::VmTypeSystem;
    type InstructionSet = super::VmInstructionSet;
}

impl AllowCse for VmLang {}
