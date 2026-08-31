use std::sync::atomic::AtomicU8;
use zhc::prelude::VmExecutionPlan;
use zhc_ir::OpIdRaw;
use zhc_langs::vmlang::VmByteCode;
use zhc_utils::{Store, small::SmallVec};

use super::*;

/// The instantiation of an Execution Plan.
pub struct Run {
    pub bytecodes: Store<WorkerId, Vec<VmByteCode>>,
    pub inputs: SmallVec<Value>,
    pub outputs: SmallVec<ValueMut>,
    pub locks: Vec<AtomicU8>,
    pub successors: Vec<SmallVec<OpIdRaw>>,
    /// Flat `inputs` index of the i-th ciphertext input.
    ///
    /// The VmLang lowering numbers ciphertext and plaintext inputs with
    /// separate dense counters (`LD.src_id` is ciphertext-local,
    /// `ADDS/MULS/... s_id` is plaintext-local), so the per-kind ids must be
    /// remapped onto the single `inputs` slice.
    pub ct_index: Vec<usize>,
    /// Flat `inputs` index of the i-th plaintext input. See [`Self::ct_index`].
    pub pt_index: Vec<usize>,
}

impl Run {
    pub fn generate(plan: &VmExecutionPlan, inputs: &[Value], outputs: &mut [ValueMut]) -> Run {
        let locks: Vec<AtomicU8> = plan.locks_table.iter().map(|a| (*a).into()).collect();
        let successors: Vec<SmallVec<OpIdRaw>> = plan.successors_table.clone();
        let mut ct_index = Vec::new();
        let mut pt_index = Vec::new();
        for (i, input) in inputs.iter().enumerate() {
            match input {
                Value::FheUint(_) => ct_index.push(i),
                Value::Uint(_) => pt_index.push(i),
            }
        }
        Run {
            bytecodes: plan.irs.iter().cloned().collect(),
            inputs: inputs.iter().cloned().collect(),
            outputs: outputs.iter().cloned().collect(),
            successors,
            locks,
            ct_index,
            pt_index,
        }
    }
}
