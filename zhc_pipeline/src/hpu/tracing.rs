use std::path::Path;

use zhc_config::hpu::HpuConfig;
use zhc_ir::IR;
use zhc_langs::{doplang::DopLang, hpulang::HpuId};
use zhc_sim::{
    Simulator, TracingLevel,
    hpu::{DOp, DOpId, Events, Hpu},
};

pub fn trace_execution(
    ir: &IR<DopLang>,
    config: &HpuConfig,
    trace_events: bool,
    path: impl AsRef<Path>,
) {
    let mut simulator = Simulator::from_simulatable(
        config.freq,
        Hpu::new(&config, HpuId(0)),
        if trace_events {
            TracingLevel::Events
        } else {
            TracingLevel::Load
        },
    );
    let dops = ir
        .walk_ops_linear()
        .map(|a| DOp {
            raw: a.get_instruction().clone(),
            id: DOpId(a.get_id().into()),
        })
        .collect();
    let event = Events::UCorePushDOps(dops);
    simulator.dispatch(event);
    simulator.play_until_event(Events::UCoreStarved);
    simulator.dump_trace(path.as_ref());
}
