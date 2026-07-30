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
}

impl Run {
    pub fn generate(plan: &VmExecutionPlan, inputs: &[Value], outputs: &mut [ValueMut]) -> Run {
        let locks: Vec<AtomicU8> = plan.locks_table.iter().map(|a| (*a).into()).collect();
        let successors: Vec<SmallVec<OpIdRaw>> = plan.successors_table.clone();
        Run {
            bytecodes: plan.irs.iter().cloned().collect(),
            inputs: inputs.iter().cloned().collect(),
            outputs: outputs.iter().cloned().collect(),
            successors,
            locks,
        }
    }
}
