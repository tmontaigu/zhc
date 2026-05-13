use super::*;

mod batcher;
mod scheduler;

use zhc_ir::IR;
use zhc_langs::hpulang::HpuLang;
use zhc_sim::hpu::HpuConfig;

pub fn schedule(
    ir: &IR<HpuLang>,
    config: &HpuConfig,
    direction: SchedulingDirection,
) -> IR<HpuLang> {
    let batched = batcher::batch(ir, config, direction);
    scheduler::schedule(&batched, config, direction)
}
