use crate::{
    hpu::{metrics::HpuMetrics, translation_table::DOpRepr},
    misc::PbsMetrics,
};
use zhc_builder::{Builder, Type};
use zhc_config::{hpu::HpuConfig, multi_hpu::MultiHpuConfig};
use zhc_ir::{IR, OpMap, Signature, evaluation::Evaluation, partition::PartitionId};
use zhc_langs::{
    doplang::DopLang,
    hpulang::{HpuLang, HpuLocality},
    ioplang::IopLang,
};
use zhc_utils::existential_enum;
use zhc_utils::files::{FileHandle, PerfettoTrace};

#[derive(Debug, Clone, PartialEq, Eq)]
#[existential_enum]
pub enum PipelineArtifact {
    // Commons
    Builder(Builder),
    IopLang(IR<IopLang>),
    PbsMetrics(PbsMetrics),
    SlackDrawing(FileHandle),
    Partitions(OpMap<PartitionId>),
    Prototype(Signature<Type>),
    // Hpu
    HpuConfig(HpuConfig),
    HpuLangTranslated(IR<HpuLang>),
    HpuLangScheduled(IR<HpuLang>),
    DopLang(IR<DopLang>),
    HpuStream(Vec<DOpRepr>),
    HpuMetrics(HpuMetrics),
    HpuTrace(PerfettoTrace),
    HpuAssembly(FileHandle),
    // MultiHpu
    MultiHpuConfig(MultiHpuConfig),
    MultiHpuLangTranslated(IR<HpuLang>),
    MultiHpuLocalities(OpMap<HpuLocality>),
    MultiHpuLangScheduled(Vec<IR<HpuLang>>),
    MultiDopLang(Vec<IR<DopLang>>),
    MultiHpuTrace(PerfettoTrace),
    MultiHpuStream(Vec<Vec<DOpRepr>>),
    MultiHpuAssembly(Vec<FileHandle>),
}

impl Evaluation for PipelineArtifact {}
