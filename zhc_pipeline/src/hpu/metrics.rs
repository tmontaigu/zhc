//! HPU-level performance metrics.
//!
//! This module provides metrics computed after the full compilation pipeline, including
//! latency bounds, PE idle time, and batch size distribution.

use zhc_config::hpu::HpuConfig;
use zhc_ir::IR;
use zhc_langs::{
    doplang::DopLang,
    hpulang::{HpuId, HpuLang, get_batch_statistics},
};
use zhc_sim::{
    Simulator,
    hpu::{DOp, DOpId, Events, FlatLinLatency, Hpu},
};
use zhc_utils::{
    Dumpable,
    data_visulization::Histogram,
    tracing::Event,
    units::{Cycle, MHz, Microseconds},
};

/// HPU execution performance metrics.
///
/// Contains timing information and batch statistics computed by simulating the
/// compiled IR on the HPU model. All timing values are in microseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HpuMetrics {
    /// Theoretical lower bound assuming perfect batching and linear-op hiding (µs).
    pub lower_bound: Microseconds,
    /// Time the PBS processing element spent idle (µs).
    pub pep_idle: Microseconds,
    /// Total simulated execution latency (µs).
    pub latency: Microseconds,
    /// Distribution of PBS batch sizes.
    pub batch_stats: Histogram<u16>,
}

impl Dumpable for HpuMetrics {
    fn dump_to_string(&self) -> String {
        let batch_hist_str = self.batch_stats.dump_to_string();
        let batch_hist_lines: Vec<&str> = batch_hist_str.lines().collect();

        let mut result = format!(
            "╔══════════════════════════════════════════════════════════════════════════════
║ HPU Metrics
║──────────────────────────────────────────────────────────────────────────────
║   Lower Bound : {}
║   Latency     : {}
║   PePbs Idle  : {}
║   Efficiency  : {:.1}%
║──────────────────────────────────────────────────────────────────────────────
║   Batch Size Histogram:",
            self.lower_bound,
            self.latency,
            self.pep_idle,
            100.0 * self.lower_bound.0 / self.latency.0
        );
        for line in batch_hist_lines {
            result.push_str(&format!("\n║     {}", line));
        }
        result.push_str(
            "\n╚══════════════════════════════════════════════════════════════════════════════",
        );
        result
    }
}

pub(crate) fn compute_hpu_metrics(dop_ir: &IR<DopLang>, hpu_ir: &IR<HpuLang>) -> HpuMetrics {
    let config = HpuConfig::default();
    let lower_bound = compute_lower_bound(&dop_ir, &config);
    let lower_bound = lower_bound.as_ts(MHz::default().period());
    let (latency, pep_idle) = compute_latency(&dop_ir, &config);
    let latency = latency.as_ts(MHz::default().period());
    let pep_idle = pep_idle.as_ts(MHz::default().period());
    let batch_stats = get_batch_statistics(hpu_ir);
    HpuMetrics {
        lower_bound,
        pep_idle,
        latency,
        batch_stats,
    }
}

fn compute_lower_bound(ir: &IR<DopLang>, config: &HpuConfig) -> Cycle {
    let pbses_count = ir
        .walk_ops_linear()
        .filter(|op| op.get_instruction().is_pbs())
        .count();
    let n_full = pbses_count.div_euclid(config.pbs_max_batch_size);
    let last_batch_length = pbses_count.rem_euclid(config.pbs_max_batch_size);
    let model = FlatLinLatency::new(
        config.pbs_processing_latency_a,
        config.pbs_processing_latency_b,
        config.pbs_processing_latency_m,
    );
    model.compute_latency(config.pbs_max_batch_size) * n_full
        + model.compute_latency(last_batch_length)
}

fn compute_latency(ir: &IR<DopLang>, config: &HpuConfig) -> (Cycle, Cycle) {
    let mut simulator = Simulator::from_simulatable(
        config.freq,
        Hpu::new(&config, HpuId(0)),
        zhc_sim::TracingLevel::None,
    );
    let dops = ir
        .walk_ops_linear()
        .map(|a| DOp {
            raw: a.get_instruction(),
            id: DOpId(a.get_id().into()),
        })
        .collect();
    let event = Events::UCorePushDOps(dops);
    simulator.dispatch(event);
    simulator.play_until_event(Events::UCoreStarved);
    let idle_duration = compute_pe_pbs_idle_duration(&simulator);
    (simulator.now(), idle_duration)
}

fn compute_pe_pbs_idle_duration(simulator: &Simulator<Hpu>) -> Cycle {
    let end_time = simulator.now().0;

    let mut events: Vec<(usize, f64)> = simulator
        .get_tracer()
        .trace()
        .trace_events
        .iter()
        .filter_map(|e| {
            if let Event::Counter(c) = e {
                if c.name == "pe_pbs_working" {
                    let state = c.args.as_ref()?.get("state")?.as_f64()?;
                    // Timestamp is stored as cycle * MHz(400).period(), convert back to cycles
                    let cycle = (c.timestamp / MHz::default().period().0).round() as usize;
                    return Some((cycle, state));
                }
            }
            None
        })
        .collect();

    // Sort by timestamp
    events.sort_by_key(|(ts, _)| *ts);

    // Integrate idle time (state = 0.0)
    let mut idle_duration: usize = 0;
    let mut last_ts: usize = 0;
    let mut last_state = 0.0; // Assume idle at start

    for (ts, state) in events {
        if last_state == 0.0 {
            idle_duration += ts - last_ts;
        }
        last_ts = ts;
        last_state = state;
    }

    // Account for final period up to end_time
    if last_state == 0.0 {
        idle_duration += end_time - last_ts;
    }

    Cycle(idle_duration)
}
