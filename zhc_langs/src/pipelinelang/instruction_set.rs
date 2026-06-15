use std::fmt::{Debug, Display};
use zhc_ir::{DialectInstructionSet, Format, FormatContext, Signature, sig};

use super::PipelineTypeSystem;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PipelineInstructionSet {
    // Commons
    InputBuilder,
    BuilderToIopLang,
    BuilderToPartitions,
    BuilderToPrototype,
    ComputePbsMetrics,
    DrawSlack,
    // Hpu
    InputHpuConfig,
    IopLangToHpuLang,
    ScheduleHpuLang,
    AllocateDopLang,
    GenerateHpuStream,
    ComputeHpuMetrics,
    TraceHpuExecution,
    GenerateHpuAssembly,
    // MultiHpu
    InputMultiHpuConfig,
    IopLangToMultiHpu,
    ScheduleMultiHpuLang,
    AllocateMultiDopLang,
    GenerateMultiHpuStream,
    TraceMultiHpuExecution,
    GenerateMultiHpuAssembly,
}

impl Format for PipelineInstructionSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, _ctx: &FormatContext) -> std::fmt::Result {
        use PipelineInstructionSet::*;
        match self {
            InputBuilder => write!(f, "input_builder"),
            InputHpuConfig => write!(f, "input_hpu_config"),
            BuilderToIopLang => write!(f, "builder_to_iop_lang"),
            BuilderToPrototype => write!(f, "builder_to_prototype"),
            ComputePbsMetrics => write!(f, "compute_pbs_metrics"),
            IopLangToHpuLang => write!(f, "iop_lang_to_hpu_lang"),
            ScheduleHpuLang => write!(f, "schedule_hpu_lang"),
            AllocateDopLang => write!(f, "allocate_dop_lang"),
            GenerateHpuStream => write!(f, "generate_hpu_stream"),
            ComputeHpuMetrics => write!(f, "compute_hpu_metrics"),
            TraceHpuExecution => write!(f, "trace_hpu_execution"),
            DrawSlack => write!(f, "draw_slack"),
            BuilderToPartitions => write!(f, "builder_to_partitions"),
            GenerateHpuAssembly => write!(f, "generate_hpu_assembly"),
            InputMultiHpuConfig => write!(f, "input_multi_hpu_config"),
            IopLangToMultiHpu => write!(f, "iop_lang_to_multi_hpu"),
            ScheduleMultiHpuLang => write!(f, "schedule_multi_hpu_lang"),
            AllocateMultiDopLang => write!(f, "allocate_multi_dop_lang"),
            GenerateMultiHpuStream => write!(f, "generate_multi_hpu_stream"),
            TraceMultiHpuExecution => write!(f, "trace_multi_hpu_execution"),
            GenerateMultiHpuAssembly => write!(f, "generate_multi_hpu_assembly"),
        }
    }
}

impl Display for PipelineInstructionSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Format::fmt(self, f, &FormatContext::default())
    }
}

impl DialectInstructionSet for PipelineInstructionSet {
    type TypeSystem = PipelineTypeSystem;

    fn get_signature(&self) -> Signature<Self::TypeSystem> {
        use PipelineInstructionSet::*;
        use PipelineTypeSystem::*;
        match self {
            InputBuilder => sig![() -> (Builder)],
            InputHpuConfig => sig![() -> (HpuConfig)],
            BuilderToIopLang => sig![(Builder) -> (IopLang)],
            BuilderToPrototype => sig![(Builder) -> (Prototype)],
            ComputePbsMetrics => sig![(IopLang) -> (PbsMetrics)],
            IopLangToHpuLang => sig![(IopLang) -> (HpuLangTranslated)],
            ScheduleHpuLang => sig![(HpuLangTranslated, HpuConfig) -> (HpuLangScheduled)],
            AllocateDopLang => sig![(HpuLangScheduled, HpuConfig) -> (DopLang)],
            GenerateHpuStream => sig![(DopLang) -> (HpuStream)],
            ComputeHpuMetrics => sig![(DopLang, HpuLangScheduled) -> (HpuMetrics)],
            TraceHpuExecution => sig![(DopLang, HpuConfig) -> (HpuTrace)],
            DrawSlack => sig![(IopLang) -> (SlackDrawing)],
            BuilderToPartitions => sig![(Builder) -> (Partitions)],
            GenerateHpuAssembly => sig![(DopLang) -> (HpuAssembly)],
            InputMultiHpuConfig => sig![() -> (MultiHpuConfig)],
            IopLangToMultiHpu => {
                sig![(IopLang, Partitions) -> (MultiHpuLangTranslated, MultiHpuLocalities)]
            }
            ScheduleMultiHpuLang => {
                sig![(MultiHpuLangTranslated, MultiHpuLocalities, MultiHpuConfig) -> (MultiHpuLangScheduled)]
            }
            AllocateMultiDopLang => sig![(MultiHpuLangScheduled, MultiHpuConfig) -> (MultiDopLang)],
            GenerateMultiHpuStream => sig![(MultiDopLang) -> (MultiHpuStream)],
            TraceMultiHpuExecution => sig![(MultiDopLang, MultiHpuConfig) -> (MultiHpuTrace)],
            GenerateMultiHpuAssembly => sig![(MultiDopLang) -> (MultiHpuAssembly)],
        }
    }
}
