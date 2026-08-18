use zhc_ir::Dialect;

/// Dialect tag for the pipeline meta-language.
///
/// Unit struct binding [`PipelineTypeSystem`](super::PipelineTypeSystem) and
/// [`PipelineInstructionSet`](super::PipelineInstructionSet) into a concrete [`Dialect`]
/// implementation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PipelineLang;

impl Dialect for PipelineLang {
    type TypeSystem = super::PipelineTypeSystem;
    type InstructionSet = super::PipelineInstructionSet;
}
