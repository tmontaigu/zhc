//! VM dialect for the experimental software VM.
//!
//! This dialect models computation as the software VM executes it. It
//! is reached directly from the block-level IOP dialect, bypassing
//! [`hpulang`](crate::hpulang) and [`doplang`](crate::doplang), and it
//! terminates that lowering chain: rather than lowering to a further
//! dialect, an `IR<VmLang>` is scheduled and register-allocated into a
//! flat [`VmByteCode`] stream.
//!
//! Values live in virtual ciphertext registers
//! ([`CtRegister`](VmTypeSystem::CtRegister)) and plaintext immediates
//! ([`PtImmediate`](VmTypeSystem::PtImmediate)). There is no heap
//! storage class: the VM never spills, so a ciphertext occupies a
//! register from its definition to its last use. Keyswitching is
//! explicit — [`Ks`](VmInstructionSet::Ks) is a standalone instruction
//! that must feed every [`Pbs`](VmInstructionSet::Pbs) and
//! [`Pbs2`](VmInstructionSet::Pbs2), which consume the reduced
//! ciphertext width it produces.
//!
//! [`VmByteCode`] restates the same operations in executable form,
//! with three differences. SSA values become physical `u16` register
//! indices. Plaintext operands are inlined into the instructions that
//! read them, so [`ImmLd`](VmInstructionSet::ImmLd) has no bytecode
//! counterpart. And every instruction carries the `OpIdRaw` of the
//! operation it was lowered from, which the executor uses as the key
//! to its per-instruction dependency bookkeeping — this is what allows
//! a stream split across worker threads to preserve the dataflow order
//! of the original IR.

mod bytecode;
mod dialect;
mod instruction_set;
mod type_system;

pub use bytecode::*;
pub use dialect::*;
pub use instruction_set::*;
pub use type_system::*;
