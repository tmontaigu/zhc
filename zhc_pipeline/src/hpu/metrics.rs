//! HPU-level performance metrics.
//!
//! This module provides metrics computed after the full compilation pipeline, including
//! a decomposition of the simulated latency into ideal PBS time, batching overhead, and
//! PBS starvation, along with batch size statistics.

use zhc_config::hpu::HpuConfig;
use zhc_ir::IR;
use zhc_langs::{
    doplang::DopLang,
    hpulang::{HpuId, HpuLang, get_batch_statistics},
};
use zhc_sim::{
    Simulator,
    hpu::{DOp, DOpId, Events, FlatLinLatency, Hpu, Statistics},
};
use zhc_utils::{
    Dumpable,
    data_visulization::Histogram,
    units::{Cycle, Microseconds},
};

/// HPU execution performance metrics.
///
/// Contains timing information and batch statistics computed by simulating the
/// compiled IR on the HPU model. The simulated latency decomposes exactly as
/// `latency = lower_bound + batching_overhead + starvation`, since the PBS
/// processing element executes batches serially. All timing values are in
/// microseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HpuMetrics {
    /// Total simulated execution latency (µs).
    pub latency: Microseconds,
    /// Theoretical lower bound assuming perfect batching and linear-op hiding (µs).
    pub lower_bound: Microseconds,
    /// Extra PBS processing time paid for launching under-filled batches (µs).
    pub batching_overhead: Microseconds,
    /// Time the PBS processing element spent waiting for work (µs).
    pub starvation: Microseconds,
    /// Number of PBS batches launched during simulation.
    pub batch_count: usize,
    /// Number of PBSes processed across all batches.
    pub slots_filled: usize,
    /// Total PBS slots available across all launched batches.
    pub slots_total: usize,
    /// Number of batches launched by timeout rather than filling up.
    pub timeout_launches: u16,
    /// Distribution of PBS batch sizes.
    pub batch_stats: Histogram<u16>,
}

impl Dumpable for HpuMetrics {
    fn dump_to_string(&self) -> String {
        let occupancy = if self.slots_total > 0 {
            100.0 * self.slots_filled as f64 / self.slots_total as f64
        } else {
            0.0
        };
        let batch_hist_str = self.batch_stats.dump_to_string();
        let batch_hist_lines: Vec<&str> = batch_hist_str.lines().collect();

        let mut result = format!(
            "╔══════════════════════════════════════════════════════════════════════════════
║ HPU Metrics
║──────────────────────────────────────────────────────────────────────────────
║   Latency          : {}
║     PBS ideal      : {}  (lower bound)
║     Batch overhead : {}  (under-filled batches)
║     Starvation     : {}  (PE waiting for work)
║   Slot occupancy   : {:.1}%
║   Timeout launches : {} / {} batches
║──────────────────────────────────────────────────────────────────────────────
║   Batch Size Histogram:",
            self.latency,
            self.lower_bound,
            self.batching_overhead,
            self.starvation,
            occupancy,
            self.timeout_launches,
            self.batch_count,
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
    let period = config.freq.period();
    let lower_bound = compute_lower_bound(dop_ir, &config).as_ts(period);
    let (latency, stats) = simulate(dop_ir, &config);
    let latency = latency.as_ts(period);
    let pbs_busy = stats.pbs_busy.as_ts(period);
    let batch_stats = get_batch_statistics(hpu_ir);
    HpuMetrics {
        latency,
        lower_bound,
        batching_overhead: Microseconds(pbs_busy.0 - lower_bound.0),
        starvation: Microseconds(latency.0 - pbs_busy.0),
        batch_count: stats.pbs_batches,
        slots_filled: stats.pbs_slots_filled,
        slots_total: stats.pbs_batches * config.pbs_max_batch_size,
        timeout_launches: stats.timeouts,
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
    let mut cycles = model.compute_latency(config.pbs_max_batch_size) * n_full;
    if last_batch_length > 0 {
        cycles = cycles + model.compute_latency(last_batch_length);
    }
    cycles
}

fn simulate(ir: &IR<DopLang>, config: &HpuConfig) -> (Cycle, Statistics) {
    let mut simulator = Simulator::from_simulatable(
        config.freq,
        Hpu::new(config, HpuId(0)),
        zhc_sim::TracingLevel::None,
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
    let latency = simulator.now();
    let stats = std::mem::take(&mut simulator.simulatable.statistics);
    (latency, stats)
}
