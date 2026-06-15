//! On-demand compilation of homomorphic circuits into HPU programs.
//!
//! This module exposes [`Pipeline`], the entry point for turning a circuit — a `Builder` from
//! the `zhc_builder` crate — into everything the compiler can derive from it: intermediate
//! representations at every abstraction level, binary instruction streams, assembly listings,
//! performance metrics, execution traces, and interactive graph drawings.
//!
//! # Pull-Based Compilation
//!
//! The compilation flow is described once and for all, as a fixed graph whose nodes are
//! compilation steps — lowering, scheduling, register allocation, code generation, measuring,
//! tracing — and whose edges are the artifacts they exchange. A [`Pipeline`] walks that graph
//! *lazily*: creating one costs nothing, and requesting an artifact runs only the steps that this
//! particular artifact depends on. Steps that have already run are never run again, so requesting
//! the device-level IR and then the instruction stream derived from it performs the work they
//! share exactly once.
//!
//! The API therefore comes in two halves. The `with_*` methods declare the inputs of the
//! compilation and are meant to be chained right after [`Pipeline::new`], while the `get_*`
//! methods request artifacts, in any order and as many times as needed.
//!
//! # Single-Board and Multi-Board Flows
//!
//! Two families of artifacts coexist. The single-board flow compiles the circuit for one HPU
//! board and is driven by an `HpuConfig`, whereas the multi-board flow spreads the circuit over
//! the boards of a multi-HPU system — inserting the inter-board transfers this requires — and is
//! driven by a `MultiHpuConfig`; both configurations come from the `zhc_config` crate.
//! Multi-board artifacts are exposed by the `get_multi_*` methods and are usually collections,
//! holding one entry per board.
//!
//! The two configurations are mutually exclusive, so a given pipeline serves either the
//! single-board methods or the multi-board ones, never both.
//!
//! # Examples
//!
//! Compiling a circuit for a single board, then reading the instruction stream and the latency it
//! is expected to take:
//!
//! ```rust,no_run
//! # use zhc_pipeline::Pipeline;
//! # use zhc_builder::{Builder, CiphertextBlockSpec};
//! # use zhc_config::hpu::HpuConfig;
//! # let builder = Builder::new(CiphertextBlockSpec(2, 2));
//! let mut pipeline = Pipeline::new()
//!     .with_builder(builder)
//!     .with_hpu_config(HpuConfig::default());
//!
//! let stream = pipeline.get_hpu_stream().clone();
//! println!("{} instructions, {}", stream[0], pipeline.get_hpu_metrics().latency);
//!
//! // Both artifacts came from the same intermediate representations, which the pipeline
//! // computed once and kept.
//! println!("{} device operations", pipeline.get_doplang().n_ops());
//! ```
//!
//! Compiling the same circuit for a multi-board system, then opening the trace of its simulated
//! execution:
//!
//! ```rust,no_run
//! # use zhc_pipeline::Pipeline;
//! # use zhc_builder::{Builder, CiphertextBlockSpec};
//! # use zhc_config::multi_hpu::MultiHpuConfig;
//! # let builder = Builder::new(CiphertextBlockSpec(2, 2));
//! let mut pipeline = Pipeline::new()
//!     .with_builder(builder)
//!     .with_multi_hpu_config(MultiHpuConfig::default());
//!
//! // One instruction stream per board of the system.
//! for (board, stream) in pipeline.get_multi_hpu_stream().iter().enumerate() {
//!     println!("board {board}: {} instructions", stream[0]);
//! }
//!
//! pipeline.get_multi_hpu_trace().open().unwrap();
//! ```

use std::sync::LazyLock;

use zhc_builder::{Builder, Type};
use zhc_config::{hpu::HpuConfig, multi_hpu::MultiHpuConfig};
use zhc_ir::{IR, OpMap, Signature, ValId, evaluation::LazyEvaluator, partition::PartitionId};
use zhc_langs::{
    doplang::DopLang,
    hpulang::{HpuLang, HpuLocality},
    ioplang::IopLang,
    pipelinelang::{PipelineInstructionSet, PipelineLang},
};
use zhc_utils::{
    files::{Extension, FileHandle, PerfettoTrace, random_path},
    svec,
};

use crate::{
    hpu::{metrics::HpuMetrics, translation_table::DOpRepr},
    misc::PbsMetrics,
};

struct ArtifactsValids {
    builder: ValId,
    ioplang: ValId,
    slack_drawing: ValId,
    pbs_metrics: ValId,
    partitions: ValId,
    prototype: ValId,
    hpu_config: ValId,
    hpulang_translated: ValId,
    hpulang_scheduled: ValId,
    doplang: ValId,
    hpu_stream: ValId,
    hpu_metrics: ValId,
    hpu_trace: ValId,
    hpu_assembly: ValId,
    multi_hpu_config: ValId,
    multi_hpulang_translated: ValId,
    multi_hpu_localities: ValId,
    multi_hpulang_scheduled: ValId,
    multi_doplang: ValId,
    multi_hpu_trace: ValId,
    multi_hpu_stream: ValId,
    multi_hpu_assembly: ValId,
}

static PIPELINE: LazyLock<(IR<PipelineLang>, ArtifactsValids)> = LazyLock::new(|| {
    use PipelineInstructionSet::*;
    let mut ir = IR::<PipelineLang>::empty();

    // Commons
    let (_, rets) = ir.add_op(InputBuilder, svec![]);
    let builder = rets[0];
    let (_, rets) = ir.add_op(InputHpuConfig, svec![]);
    let hpu_config = rets[0];
    let (_, rets) = ir.add_op(BuilderToIopLang, svec![builder]);
    let ioplang = rets[0];
    let (_, rets) = ir.add_op(BuilderToPartitions, svec![builder]);
    let partitions = rets[0];
    let (_, rets) = ir.add_op(BuilderToPrototype, svec![builder]);
    let prototype = rets[0];
    let (_, rets) = ir.add_op(DrawSlack, svec![ioplang]);
    let slack_drawing = rets[0];

    // Hpu
    let (_, rets) = ir.add_op(ComputePbsMetrics, svec![ioplang]);
    let pbs_metrics = rets[0];
    let (_, rets) = ir.add_op(IopLangToHpuLang, svec![ioplang]);
    let hpulang_translated = rets[0];
    let (_, rets) = ir.add_op(ScheduleHpuLang, svec![hpulang_translated, hpu_config]);
    let hpulang_scheduled = rets[0];
    let (_, rets) = ir.add_op(AllocateDopLang, svec![hpulang_scheduled, hpu_config]);
    let doplang = rets[0];
    let (_, rets) = ir.add_op(GenerateHpuStream, svec![doplang]);
    let hpu_stream = rets[0];
    let (_, rets) = ir.add_op(TraceHpuExecution, svec![doplang, hpu_config]);
    let hpu_trace = rets[0];
    let (_, rets) = ir.add_op(ComputeHpuMetrics, svec![doplang, hpulang_scheduled]);
    let hpu_metrics = rets[0];
    let (_, rets) = ir.add_op(GenerateHpuAssembly, svec![doplang]);
    let hpu_assembly = rets[0];

    // Multi-Hpu
    let (_, rets) = ir.add_op(InputMultiHpuConfig, svec![]);
    let multi_hpu_config = rets[0];
    let (_, rets) = ir.add_op(IopLangToMultiHpu, svec![ioplang, partitions]);
    let multi_hpulang_translated = rets[0];
    let multi_hpu_localities = rets[1];
    let (_, rets) = ir.add_op(
        ScheduleMultiHpuLang,
        svec![
            multi_hpulang_translated,
            multi_hpu_localities,
            multi_hpu_config
        ],
    );
    let multi_hpulang_scheduled = rets[0];
    let (_, rets) = ir.add_op(
        AllocateMultiDopLang,
        svec![multi_hpulang_scheduled, multi_hpu_config],
    );
    let multi_doplang = rets[0];
    let (_, rets) = ir.add_op(
        TraceMultiHpuExecution,
        svec![multi_doplang, multi_hpu_config],
    );
    let multi_hpu_trace = rets[0];
    let (_, rets) = ir.add_op(GenerateMultiHpuStream, svec![multi_doplang]);
    let multi_hpu_stream = rets[0];
    let (_, rets) = ir.add_op(GenerateMultiHpuAssembly, svec![multi_doplang]);
    let multi_hpu_assembly = rets[0];

    (
        ir,
        ArtifactsValids {
            builder,
            ioplang,
            pbs_metrics,
            slack_drawing,
            partitions,
            prototype,
            hpu_config,
            hpulang_translated,
            hpulang_scheduled,
            doplang,
            hpu_stream,
            hpu_metrics,
            hpu_trace,
            hpu_assembly,
            multi_hpu_config,
            multi_hpulang_translated,
            multi_hpu_localities,
            multi_hpulang_scheduled,
            multi_doplang,
            multi_hpu_trace,
            multi_hpu_stream,
            multi_hpu_assembly,
        },
    )
});

#[allow(non_snake_case)]
fn IR() -> &'static IR<PipelineLang> {
    &PIPELINE.0
}
#[allow(non_snake_case)]
fn VALIDS() -> &'static ArtifactsValids {
    &PIPELINE.1
}

mod artifacts;
mod context;
mod evaluation;

use artifacts::*;

use crate::pipeline::context::PipelineContext;

/// A lazily-evaluated compilation of a circuit into HPU artifacts.
///
/// Holds the inputs of a compilation — the circuit and the configuration of the target — together
/// with every artifact computed so far. Artifacts are requested one at a time with the `get_*`
/// methods, which run the compilation steps still missing and keep their results, so the same
/// pipeline can be queried again and again without ever redoing work.
///
/// Instances are configured by chaining the `with_*` methods on a fresh [`new`](Self::new)
/// pipeline. Requesting an artifact takes `&mut self`, since the call may compile, and hands back
/// a reference into the pipeline's own storage.
pub struct Pipeline {
    eval: LazyEvaluator<'static, PipelineLang, PipelineArtifact>,
    context: PipelineContext,
}

impl Pipeline {
    /// Creates a pipeline with no circuit and no target configuration.
    ///
    /// Nothing is compiled at this point: every step of the flow starts out pending. The inputs
    /// must be declared with [`with_builder`](Self::with_builder) and one of
    /// [`with_hpu_config`](Self::with_hpu_config) or
    /// [`with_multi_hpu_config`](Self::with_multi_hpu_config) before an artifact that needs them
    /// is requested.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::hpu::HpuConfig;
    /// # let builder = Builder::new(CiphertextBlockSpec(2, 2));
    /// let pipeline = Pipeline::new()
    ///     .with_builder(builder)
    ///     .with_hpu_config(HpuConfig::default());
    /// ```
    pub fn new() -> Self {
        Pipeline {
            eval: LazyEvaluator::from_ir(IR()),
            context: PipelineContext::new(),
        }
    }

    fn eventually_report_failure(&self) {
        if !self.eval.is_ok() {
            let failed = self
                .eval
                .as_view()
                .walk_ops_linear()
                .find(|op| op.get_annotation().is_panic())
                .unwrap();
            panic!(
                "Failed to evaluate pipeline. Panic occured evaluating step:\n{}",
                failed.format()
            )
        }
    }

    /// Renders the current state of the compilation as an interactive HTML graph.
    ///
    /// Draws the graph of compilation steps as it stands, annotated with the state of each of
    /// them: which artifacts have been computed, which are still pending, and which ones failed.
    /// The call itself compiles nothing, which makes it a convenient way to see what a series of
    /// `get_*` calls actually triggered — or, on a fresh pipeline, to discover the compilation
    /// flow itself.
    ///
    /// The returned handle points at a freshly created temporary file, which can be displayed in
    /// the default browser with its `open` method.
    ///
    /// # Panics
    ///
    /// Panics if the HTML file cannot be written.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// // Displays the whole compilation flow, with every step still pending.
    /// Pipeline::new().draw_state().open().unwrap();
    /// ```
    pub fn draw_state(&self) -> FileHandle {
        let path = random_path(Extension::Html);
        self.eval.as_view().draw_to_html(None, &path);
        FileHandle::from(path)
    }

    /// Sets the circuit to compile.
    ///
    /// The `builder` argument holds the integer-level circuit as recorded by the `zhc_builder`
    /// crate: its inputs, the integer operations applied to them, and its outputs. Every artifact
    /// but the target configurations descends from it. The circuit is optimized on the way in, so
    /// the IR returned by [`get_ioplang`](Self::get_ioplang) is the optimized form of what is
    /// given here.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # let builder = Builder::new(CiphertextBlockSpec(2, 2));
    /// let pipeline = Pipeline::new().with_builder(builder);
    /// ```
    pub fn with_builder(mut self, builder: Builder) -> Self {
        self.context.builder = Some(builder);
        self
    }

    /// Sets the configuration of the single HPU board to compile for.
    ///
    /// The `config` argument describes the target hardware — clock frequency, register file size,
    /// bootstrapping batch bounds, memory and ALU latencies — and drives scheduling, register
    /// allocation, and the timing model behind metrics and traces. Setting it selects the
    /// single-board flow, that is, every `get_*` method without a `multi_` in its name.
    ///
    /// # Panics
    ///
    /// Panics if a multi-board configuration was already set with
    /// [`with_multi_hpu_config`](Self::with_multi_hpu_config), as the two flows are mutually
    /// exclusive.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_config::hpu::HpuConfig;
    /// let pipeline = Pipeline::new().with_hpu_config(HpuConfig::default());
    /// ```
    pub fn with_hpu_config(mut self, config: HpuConfig) -> Self {
        assert!(self.context.multi_hpu_config.is_none());
        self.context.hpu_config = Some(config);
        self
    }

    /// Sets the configuration of the multi-HPU system to compile for.
    ///
    /// The `config` argument describes the target system: the board configuration its boards
    /// share, and how many of them there are. Setting it selects the multi-board flow, that is,
    /// every `get_multi_*` method, which spreads the circuit over the boards and inserts the
    /// transfers needed to move ciphertexts between them.
    ///
    /// # Panics
    ///
    /// Panics if a single-board configuration was already set with
    /// [`with_hpu_config`](Self::with_hpu_config), as the two flows are mutually exclusive.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_config::multi_hpu::MultiHpuConfig;
    /// let pipeline = Pipeline::new().with_multi_hpu_config(MultiHpuConfig::default());
    /// ```
    pub fn with_multi_hpu_config(mut self, config: MultiHpuConfig) -> Self {
        assert!(self.context.hpu_config.is_none());
        self.context.multi_hpu_config = Some(config);
        self
    }

    /// Selects the legacy scheduler for the single-board flow.
    ///
    /// The default scheduler orders operations under a single as-late-as-possible step, whereas
    /// the legacy one is two-step and more often than not gives worse results. The
    /// choice changes the execution order picked for the circuit, and with it everything derived
    /// from [`get_scheduled_hpulang`](Self::get_scheduled_hpulang) — batching, register pressure,
    /// instruction stream, and measured latency. It has no effect on the multi-board flow.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::hpu::HpuConfig;
    /// # let builder = Builder::new(CiphertextBlockSpec(2, 2));
    /// let mut pipeline = Pipeline::new()
    ///     .with_builder(builder)
    ///     .with_hpu_config(HpuConfig::default())
    ///     .with_legacy_hpu_scheduler();
    ///
    /// // Scheduling, and everything downstream of it, now goes through the legacy scheduler.
    /// println!("{}", pipeline.get_hpu_metrics().latency);
    /// ```
    pub fn with_legacy_hpu_scheduler(mut self) -> Self {
        self.context.legacy_hpu_scheduler = true;
        self
    }

    /// Records the individual events of the simulated device in the execution traces.
    ///
    /// By default, a trace holds the successive states of the units of the device and the load
    /// counters measured on them, which is what following an execution along its timeline calls
    /// for. This turns the simulation to its most verbose tracing mode, which adds one section to
    /// the trace: every event the simulated device goes through, each on its own track and carrying
    /// its payload — an operation being issued to a processing element, a dependency being
    /// unlocked, a bootstrapping batch being launched or landing, a unit becoming unavailable. This
    /// is the level of detail that answers *why* the device did something, where the states and
    /// counters answer *when*.
    ///
    /// The extra events cost simulation time and trace size, hence the opt-in. The setting applies
    /// to both flows, that is to the traces returned by [`get_hpu_trace`](Self::get_hpu_trace) and
    /// [`get_multi_hpu_trace`](Self::get_multi_hpu_trace), and to nothing else: the other
    /// artifacts, metrics included, are computed identically with or without it.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::hpu::HpuConfig;
    /// # let builder = Builder::new(CiphertextBlockSpec(2, 2));
    /// let mut pipeline = Pipeline::new()
    ///     .with_builder(builder)
    ///     .with_hpu_config(HpuConfig::default())
    ///     .with_trace_hpu_events();
    ///
    /// // The trace now carries the event tracks, on top of the states and the counters.
    /// pipeline.get_hpu_trace().open().unwrap();
    /// ```
    pub fn with_trace_hpu_events(mut self) -> Self {
        self.context.hpu_trace_events = true;
        self
    }

    /// Returns the circuit being compiled.
    ///
    /// Hands back the circuit given to [`with_builder`](Self::with_builder), which is convenient
    /// when the circuit was produced elsewhere and only the pipeline holds on to it.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)));
    /// let builder: &Builder = pipeline.get_builder();
    /// ```
    pub fn get_builder(&mut self) -> &Builder {
        self.eval.pull_val(&mut self.context, VALIDS().builder);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().builder)
            .unwrap()
            .unwrap_builder_ref()
    }

    /// Returns the input and output types of the circuit.
    ///
    /// The prototype is the I/O signature of the circuit: the type of every argument it takes and
    /// of every value it returns, in declaration order. Each of these types is either an encrypted
    /// or a plaintext integer, of a given bit width and block layout. This is what a caller of the
    /// compiled program needs to know to run it — which values to encrypt, in which order to hand
    /// them over, and what to expect back.
    ///
    /// The signature is read straight from the circuit, so asking for it triggers no compilation
    /// work.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), or if a step this
    /// artifact depends on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)));
    /// let prototype = pipeline.get_prototype();
    /// println!("{:?} -> {:?}", prototype.get_args(), prototype.get_returns());
    /// ```
    pub fn get_prototype(&mut self) -> &Signature<Type> {
        self.eval.pull_val(&mut self.context, VALIDS().prototype);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().prototype)
            .unwrap()
            .unwrap_prototype_ref()
    }

    /// Returns the configuration of the target HPU board.
    ///
    /// Hands back the configuration given to [`with_hpu_config`](Self::with_hpu_config), which is
    /// useful to re-read the hardware parameters the single-board artifacts were compiled against.
    ///
    /// # Panics
    ///
    /// Panics if no single-board configuration was set with
    /// [`with_hpu_config`](Self::with_hpu_config).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_config::hpu::HpuConfig;
    /// # let mut pipeline = Pipeline::new().with_hpu_config(HpuConfig::default());
    /// println!("{} registers", pipeline.get_hpu_config().regf_size);
    /// ```
    pub fn get_hpu_config(&mut self) -> &HpuConfig {
        self.eval.pull_val(&mut self.context, VALIDS().hpu_config);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().hpu_config)
            .unwrap()
            .unwrap_hpu_config_ref()
    }

    /// Returns the optimized integer-level IR of the circuit.
    ///
    /// This is the first artifact derived from the circuit: an IR in the IOP language, whose
    /// operations still work on whole encrypted integers rather than on radix blocks, as left by
    /// the optimization passes of the builder. Every other artifact of the pipeline is compiled
    /// from it, so it is the right place to look at what is actually being compiled.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), or if a step this
    /// artifact depends on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)));
    /// println!("{} integer operations", pipeline.get_ioplang().n_ops());
    /// ```
    pub fn get_ioplang(&mut self) -> &IR<IopLang> {
        self.eval.pull_val(&mut self.context, VALIDS().ioplang);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().ioplang)
            .unwrap()
            .unwrap_iop_lang_ref()
    }

    /// Returns the block-level HPU IR of the circuit, before scheduling.
    ///
    /// Lowering the integer-level IR replaces each integer operation by the block-level operations
    /// and programmable bootstrapping lookups that implement it, giving an IR in the HPU language.
    /// Its operations are still in translation order — no execution order has been picked yet, and
    /// no register has been assigned — which makes this artifact the one to inspect when checking
    /// how an integer operation is expressed in terms of block operations.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), or if a step this
    /// artifact depends on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)));
    /// println!("{} block operations", pipeline.get_translated_hpulang().n_ops());
    /// ```
    pub fn get_translated_hpulang(&mut self) -> &IR<HpuLang> {
        self.eval
            .pull_val(&mut self.context, VALIDS().hpulang_translated);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().hpulang_translated)
            .unwrap()
            .unwrap_hpu_lang_translated_ref()
    }

    /// Returns the block-level HPU IR of the circuit, after scheduling.
    ///
    /// Scheduling picks an execution order for the block-level operations and groups programmable
    /// bootstrappings into the batches the device processes in one go, within the resources the
    /// board configuration declares. Operands are still symbolic values at this point; they are
    /// bound to physical locations later, by [`get_doplang`](Self::get_doplang).
    ///
    /// Which scheduler produces this artifact can be changed with
    /// [`with_legacy_hpu_scheduler`](Self::with_legacy_hpu_scheduler).
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), if no configuration
    /// was set with [`with_hpu_config`](Self::with_hpu_config), or if a step this artifact depends
    /// on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::hpu::HpuConfig;
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)))
    /// #     .with_hpu_config(HpuConfig::default());
    /// // Draws the scheduled program, whose operations are laid out in execution order.
    /// pipeline.get_scheduled_hpulang().draw_to_html(None, "schedule.html");
    /// ```
    pub fn get_scheduled_hpulang(&mut self) -> &IR<HpuLang> {
        self.eval
            .pull_val(&mut self.context, VALIDS().hpulang_scheduled);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().hpulang_scheduled)
            .unwrap()
            .unwrap_hpu_lang_scheduled_ref()
    }

    /// Returns the device-level IR of the circuit.
    ///
    /// Register allocation rewrites the scheduled block-level IR into the DOP language, where
    /// every operand is a physical location of the board — a register of the register file or a
    /// memory slot — and where the loads and stores needed to spill values are explicit. This is
    /// the last intermediate representation before code generation, and therefore the one the
    /// instruction stream, the assembly listing, the metrics, and the trace are all read from.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), if no configuration
    /// was set with [`with_hpu_config`](Self::with_hpu_config), or if a step this artifact depends
    /// on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::hpu::HpuConfig;
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)))
    /// #     .with_hpu_config(HpuConfig::default());
    /// println!("{} device operations", pipeline.get_doplang().n_ops());
    /// ```
    pub fn get_doplang(&mut self) -> &IR<DopLang> {
        self.eval.pull_val(&mut self.context, VALIDS().doplang);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().doplang)
            .unwrap()
            .unwrap_dop_lang_ref()
    }

    /// Returns the binary instruction stream to be sent to the device.
    ///
    /// Encodes each operation of the device-level IR into the machine word the board decodes, in
    /// execution order. The first word of the stream is a header holding the number of
    /// instructions that follow, so a stream carrying `n` instructions is `n + 1` words long.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), if no configuration
    /// was set with [`with_hpu_config`](Self::with_hpu_config), or if a step this artifact depends
    /// on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::hpu::HpuConfig;
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)))
    /// #     .with_hpu_config(HpuConfig::default());
    /// let stream = pipeline.get_hpu_stream();
    /// println!("{} instructions in {} words", stream[0], stream.len());
    /// ```
    pub fn get_hpu_stream(&mut self) -> &Vec<DOpRepr> {
        self.eval.pull_val(&mut self.context, VALIDS().hpu_stream);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().hpu_stream)
            .unwrap()
            .unwrap_hpu_stream_ref()
    }

    /// Returns the bootstrapping metrics of the circuit.
    ///
    /// Characterizes the circuit before it is compiled for any particular board: how many
    /// programmable bootstrappings it performs, how long its longest chain of dependent
    /// bootstrappings is, and how much freedom the remaining ones have in time. These figures
    /// depend on the circuit alone, not on a target configuration, which makes them the right way
    /// to gauge the intrinsic cost and the available parallelism of a circuit.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), or if a step this
    /// artifact depends on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)));
    /// let metrics = pipeline.get_pbs_metrics();
    /// println!("{} bootstrappings, {} deep", metrics.count, metrics.critical_length);
    /// ```
    pub fn get_pbs_metrics(&mut self) -> &PbsMetrics {
        self.eval.pull_val(&mut self.context, VALIDS().pbs_metrics);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().pbs_metrics)
            .unwrap()
            .unwrap_pbs_metrics_ref()
    }

    /// Returns the performance metrics of the compiled program.
    ///
    /// Runs the device-level program through the timing model of the board and reports the latency
    /// it takes, the theoretical lower bound it is worth comparing against, the time the
    /// bootstrapping unit spent idle, and the distribution of the batch sizes the scheduler
    /// achieved. This is the artifact to look at when judging how well a circuit was compiled.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), if no configuration
    /// was set with [`with_hpu_config`](Self::with_hpu_config), or if a step this artifact depends
    /// on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::hpu::HpuConfig;
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)))
    /// #     .with_hpu_config(HpuConfig::default());
    /// let metrics = pipeline.get_hpu_metrics();
    /// println!("{} (lower bound {})", metrics.latency, metrics.lower_bound);
    /// ```
    pub fn get_hpu_metrics(&mut self) -> &HpuMetrics {
        self.eval.pull_val(&mut self.context, VALIDS().hpu_metrics);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().hpu_metrics)
            .unwrap()
            .unwrap_hpu_metrics_ref()
    }

    /// Returns a trace of the simulated execution of the compiled program.
    ///
    /// Replays the device-level program on the timing model of the board and records what each
    /// unit of the device does over time. Where [`get_hpu_metrics`](Self::get_hpu_metrics) sums
    /// the execution up in a handful of numbers, this shows it instruction by instruction, which
    /// is how a stall or an unexpectedly small batch is tracked down. The returned handle points
    /// at a trace file that can be displayed in the Perfetto UI with its `open` method.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), if no configuration
    /// was set with [`with_hpu_config`](Self::with_hpu_config), or if a step this artifact depends
    /// on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::hpu::HpuConfig;
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)))
    /// #     .with_hpu_config(HpuConfig::default());
    /// pipeline.get_hpu_trace().open().unwrap();
    /// ```
    pub fn get_hpu_trace(&mut self) -> &PerfettoTrace {
        self.eval.pull_val(&mut self.context, VALIDS().hpu_trace);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().hpu_trace)
            .unwrap()
            .unwrap_hpu_trace_ref()
    }

    /// Returns an interactive drawing of the circuit's scheduling slack.
    ///
    /// Draws the integer-level IR as a graph whose operations are coloured by their slack — how
    /// much they can be moved in time without delaying the circuit — on a traffic-light scale, so
    /// that the critical path stands out from the operations that can wait. The returned handle
    /// points at an HTML file that can be displayed in the default browser with its `open` method.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), or if a step this
    /// artifact depends on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)));
    /// pipeline.get_slack_drawing().open().unwrap();
    /// ```
    pub fn get_slack_drawing(&mut self) -> &FileHandle {
        self.eval
            .pull_val(&mut self.context, VALIDS().slack_drawing);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().slack_drawing)
            .unwrap()
            .unwrap_slack_drawing_ref()
    }

    /// Returns the partition each operation of the circuit belongs to.
    ///
    /// A partition is a labelled cluster of neighbouring operations, declared while building the
    /// circuit, that the compiler treats as a single unit of work; the returned map associates
    /// every operation of the optimized integer-level IR with its own. The multi-board flow reads
    /// this map to decide which board runs what, so partitioning a circuit is how its placement is
    /// steered.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), or if a step this
    /// artifact depends on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)));
    /// let partitions = pipeline.get_partitions();
    /// println!("{} operations placed in partitions", partitions.iter().count());
    /// ```
    pub fn get_partitions(&mut self) -> &OpMap<PartitionId> {
        self.eval.pull_val(&mut self.context, VALIDS().partitions);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().partitions)
            .unwrap()
            .unwrap_partitions_ref()
    }

    /// Returns a human-readable assembly listing of the compiled program.
    ///
    /// Emits the device-level program as assembly text and writes it to a file. The content is the
    /// same program as the one encoded by [`get_hpu_stream`](Self::get_hpu_stream), in a form
    /// meant to be read rather than executed. The returned handle can be displayed with its `open`
    /// method.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), if no configuration
    /// was set with [`with_hpu_config`](Self::with_hpu_config), or if a step this artifact depends
    /// on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::hpu::HpuConfig;
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)))
    /// #     .with_hpu_config(HpuConfig::default());
    /// pipeline.get_hpu_assembly().open().unwrap();
    /// ```
    pub fn get_hpu_assembly(&mut self) -> &FileHandle {
        self.eval.pull_val(&mut self.context, VALIDS().hpu_assembly);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().hpu_assembly)
            .unwrap()
            .unwrap_hpu_assembly_ref()
    }

    /// Returns the configuration of the target multi-HPU system.
    ///
    /// Hands back the configuration given to
    /// [`with_multi_hpu_config`](Self::with_multi_hpu_config), which is useful to re-read the
    /// board configuration and the board count the multi-board artifacts were compiled against.
    ///
    /// # Panics
    ///
    /// Panics if no multi-board configuration was set with
    /// [`with_multi_hpu_config`](Self::with_multi_hpu_config).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_config::multi_hpu::MultiHpuConfig;
    /// # let mut pipeline = Pipeline::new().with_multi_hpu_config(MultiHpuConfig::default());
    /// println!("{} boards", pipeline.get_multi_hpu_config().n_hpus);
    /// ```
    pub fn get_multi_hpu_config(&mut self) -> &MultiHpuConfig {
        self.eval
            .pull_val(&mut self.context, VALIDS().multi_hpu_config);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().multi_hpu_config)
            .unwrap()
            .unwrap_multi_hpu_config_ref()
    }

    /// Returns the block-level HPU IR of the whole system, before scheduling.
    ///
    /// Lowers the integer-level IR the way the single-board flow does, then assigns every
    /// operation to a board following the circuit's partitions, and inserts an explicit transfer
    /// wherever an operation consumes a value that lives on another board. The result is one IR
    /// covering the whole system: where each of its operations runs is told by
    /// [`get_multi_hpu_localities`](Self::get_multi_hpu_localities), and it is cut into per-board
    /// programs by [`get_scheduled_multi_hpulang`](Self::get_scheduled_multi_hpulang).
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), or if a step this
    /// artifact depends on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)));
    /// let ir = pipeline.get_translated_multi_hpulang();
    /// println!("{} block operations, transfers included", ir.n_ops());
    /// ```
    pub fn get_translated_multi_hpulang(&mut self) -> &IR<HpuLang> {
        self.eval
            .pull_val(&mut self.context, VALIDS().multi_hpulang_translated);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().multi_hpulang_translated)
            .unwrap()
            .unwrap_multi_hpu_lang_translated_ref()
    }

    /// Returns the placement of each operation over the boards of the system.
    ///
    /// Associates every operation of the IR returned by
    /// [`get_translated_multi_hpulang`](Self::get_translated_multi_hpulang) with the board it runs
    /// on, with the pair of boards a transfer moves data between, or with the set of boards it is
    /// replicated on. Boards are numbered in the order the circuit's partitions are first met, so
    /// this map is what to read to know where the compiler decided to put the work.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), or if a step this
    /// artifact depends on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_langs::hpulang::HpuLocality;
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)));
    /// let transfers = pipeline
    ///     .get_multi_hpu_localities()
    ///     .iter()
    ///     .filter(|(_, locality)| matches!(**locality, HpuLocality::Transfer { .. }))
    ///     .count();
    /// println!("{transfers} inter-board transfers");
    /// ```
    pub fn get_multi_hpu_localities(&mut self) -> &OpMap<HpuLocality> {
        self.eval
            .pull_val(&mut self.context, VALIDS().multi_hpu_localities);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().multi_hpu_localities)
            .unwrap()
            .unwrap_multi_hpu_localities_ref()
    }

    /// Returns the scheduled block-level HPU IR of each board.
    ///
    /// Schedules the operations of the whole system at once — accounting for each board's own
    /// resources and for the transfers the boards wait on — then splits the outcome into one IR
    /// per board, in board order. Each of them is a block-level program of the same shape the
    /// single-board flow produces, restricted to the operations its board runs.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), if no configuration
    /// was set with [`with_multi_hpu_config`](Self::with_multi_hpu_config), or if a step this
    /// artifact depends on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::multi_hpu::MultiHpuConfig;
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)))
    /// #     .with_multi_hpu_config(MultiHpuConfig::default());
    /// for (board, ir) in pipeline.get_scheduled_multi_hpulang().iter().enumerate() {
    ///     println!("board {board}: {} block operations", ir.n_ops());
    /// }
    /// ```
    pub fn get_scheduled_multi_hpulang(&mut self) -> &Vec<IR<HpuLang>> {
        self.eval
            .pull_val(&mut self.context, VALIDS().multi_hpulang_scheduled);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().multi_hpulang_scheduled)
            .unwrap()
            .unwrap_multi_hpu_lang_scheduled_ref()
    }

    /// Returns the device-level IR of each board.
    ///
    /// Allocates registers separately for every board's scheduled program, against the board
    /// configuration the system shares, giving one device-level IR per board in board order. Each
    /// of them is what its board will actually run, and is the program the per-board streams,
    /// listings, and traces are generated from.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), if no configuration
    /// was set with [`with_multi_hpu_config`](Self::with_multi_hpu_config), or if a step this
    /// artifact depends on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::multi_hpu::MultiHpuConfig;
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)))
    /// #     .with_multi_hpu_config(MultiHpuConfig::default());
    /// for (board, ir) in pipeline.get_multi_doplang().iter().enumerate() {
    ///     println!("board {board}: {} device operations", ir.n_ops());
    /// }
    /// ```
    pub fn get_multi_doplang(&mut self) -> &Vec<IR<DopLang>> {
        self.eval
            .pull_val(&mut self.context, VALIDS().multi_doplang);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().multi_doplang)
            .unwrap()
            .unwrap_multi_dop_lang_ref()
    }

    /// Returns a trace of the simulated execution of the whole system.
    ///
    /// Replays the per-board programs together on the timing model of the system, so that the
    /// activity of every board and the transfers between them appear side by side in a single
    /// trace. This is where the cost of splitting a circuit across boards becomes visible: boards
    /// idling while a transfer completes show up as gaps. The returned handle points at a trace
    /// file that can be displayed in the Perfetto UI with its `open` method.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), if no configuration
    /// was set with [`with_multi_hpu_config`](Self::with_multi_hpu_config), or if a step this
    /// artifact depends on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::multi_hpu::MultiHpuConfig;
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)))
    /// #     .with_multi_hpu_config(MultiHpuConfig::default());
    /// pipeline.get_multi_hpu_trace().open().unwrap();
    /// ```
    pub fn get_multi_hpu_trace(&mut self) -> &PerfettoTrace {
        self.eval
            .pull_val(&mut self.context, VALIDS().multi_hpu_trace);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().multi_hpu_trace)
            .unwrap()
            .unwrap_multi_hpu_trace_ref()
    }

    /// Returns the binary instruction stream of each board.
    ///
    /// Encodes every board's device-level program into machine words, giving one stream per board
    /// in board order. Each stream is laid out exactly like the single-board one returned by
    /// [`get_hpu_stream`](Self::get_hpu_stream): a header word holding the instruction count,
    /// followed by the instructions themselves.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), if no configuration
    /// was set with [`with_multi_hpu_config`](Self::with_multi_hpu_config), or if a step this
    /// artifact depends on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::multi_hpu::MultiHpuConfig;
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)))
    /// #     .with_multi_hpu_config(MultiHpuConfig::default());
    /// for (board, stream) in pipeline.get_multi_hpu_stream().iter().enumerate() {
    ///     println!("board {board}: {} instructions", stream[0]);
    /// }
    /// ```
    pub fn get_multi_hpu_stream(&mut self) -> &Vec<Vec<DOpRepr>> {
        self.eval
            .pull_val(&mut self.context, VALIDS().multi_hpu_stream);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().multi_hpu_stream)
            .unwrap()
            .unwrap_multi_hpu_stream_ref()
    }

    /// Returns a human-readable assembly listing for each board.
    ///
    /// Emits every board's device-level program as assembly text and writes it to its own file,
    /// giving one handle per board in board order. The content is the same as what
    /// [`get_multi_hpu_stream`](Self::get_multi_hpu_stream) encodes, in a form meant to be read
    /// rather than executed, and each file can be displayed with its `open` method.
    ///
    /// # Panics
    ///
    /// Panics if no circuit was set with [`with_builder`](Self::with_builder), if no configuration
    /// was set with [`with_multi_hpu_config`](Self::with_multi_hpu_config), or if a step this
    /// artifact depends on panics, in which case the message names the failing step.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use zhc_pipeline::Pipeline;
    /// # use zhc_builder::{Builder, CiphertextBlockSpec};
    /// # use zhc_config::multi_hpu::MultiHpuConfig;
    /// # let mut pipeline = Pipeline::new()
    /// #     .with_builder(Builder::new(CiphertextBlockSpec(2, 2)))
    /// #     .with_multi_hpu_config(MultiHpuConfig::default());
    /// for listing in pipeline.get_multi_hpu_assembly() {
    ///     println!("{listing:?}");
    /// }
    /// ```
    pub fn get_multi_hpu_assembly(&mut self) -> &Vec<FileHandle> {
        self.eval
            .pull_val(&mut self.context, VALIDS().multi_hpu_assembly);
        self.eventually_report_failure();
        self.eval
            .get_val(VALIDS().multi_hpu_assembly)
            .unwrap()
            .unwrap_multi_hpu_assembly_ref()
    }
}
