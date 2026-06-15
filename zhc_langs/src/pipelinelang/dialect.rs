use zhc_ir::Dialect;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PipelineLang;

impl Dialect for PipelineLang {
    type TypeSystem = super::PipelineTypeSystem;
    type InstructionSet = super::PipelineInstructionSet;
}
