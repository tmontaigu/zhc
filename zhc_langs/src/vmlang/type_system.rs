use zhc_ir::DialectTypeSystem;
use zhc_utils::DisplayVariant;

/// Type system for the VM dialect.
///
/// Models the two storage classes the software VM exposes: virtual
/// ciphertext registers and plaintext immediates. Unlike
/// [`HpuTypeSystem`](crate::hpulang::HpuTypeSystem) there is no heap
/// class, since the VM never spills a ciphertext out of its register.
#[derive(DisplayVariant, Debug, Clone, PartialEq, Eq, Hash)]
pub enum VmTypeSystem {
    /// Ciphertext block held in a virtual register. The class does not
    /// distinguish the two ciphertext widths the VM manipulates: a
    /// register carries the reduced form only between a
    /// [`Ks`](super::VmInstructionSet::Ks) and the PBS consuming it.
    CtRegister,
    /// Plaintext scalar loaded from a positional input slot.
    PtImmediate,
}

impl DialectTypeSystem for VmTypeSystem {}
