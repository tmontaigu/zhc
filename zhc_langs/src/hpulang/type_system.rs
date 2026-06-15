use zhc_ir::DialectTypeSystem;
use zhc_utils::DisplayVariant;

/// Type system for the HPU dialect.
///
/// Models the three storage classes visible at the HPU register level:
/// ciphertext registers, plaintext immediates, and heap-spilled
/// ciphertexts.
#[derive(DisplayVariant, Debug, Clone, PartialEq, Eq, Hash)]
pub enum HpuTypeSystem {
    /// Ciphertext block held in a virtual register.
    CtRegister,
    /// Plaintext scalar loaded from an input slot or inlined as a
    /// constant.
    PtImmediate,
    /// Ciphertext block spilled to the heap.
    CtHeap,
}

impl DialectTypeSystem for HpuTypeSystem {}
