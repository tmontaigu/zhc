use std::path::Path;

use zhc_config::multi_hpu::MultiHpuConfig;
use zhc_ir::IR;
use zhc_langs::doplang::DopLang;
use zhc_sim::{
    Simulator, TracingLevel,
    hpu::{DOp, DOpId},
    multi_hpu::{Events, MultiHpu},
};

pub fn trace_execution(
    irs: &[IR<DopLang>],
    config: &MultiHpuConfig,
    trace_events: bool,
    path: impl AsRef<Path>,
) {
    let streams: Vec<Vec<DOp>> = irs
        .iter()
        .map(|ir| {
            ir.walk_ops_linear()
                .map(|a| DOp {
                    raw: a.get_instruction(),
                    id: DOpId(a.get_id().into()),
                })
                .collect()
        })
        .collect();
    let mut simulator = Simulator::from_simulatable(
        config.hpu_config.freq,
        MultiHpu::new(&config),
        if trace_events {
            TracingLevel::Events
        } else {
            TracingLevel::Load
        },
    );
    let event = zhc_sim::multi_hpu::Events::PushDOps(streams);
    simulator.dispatch(event);
    simulator.play_until_event(Events::ProcessOver);
    simulator.dump_trace(path.as_ref());
}
