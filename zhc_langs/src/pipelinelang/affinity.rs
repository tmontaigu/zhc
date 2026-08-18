/// Pipeline branch a compilation step belongs to.
///
/// `Commons` marks the frontend steps shared by every target; `Hpu`, `MultiHpu`, and `Vm` mark
/// the steps specific to the single-HPU, multi-HPU, and software-VM backends respectively.
/// Obtained from an instruction via
/// [`get_affinity`](super::PipelineInstructionSet::get_affinity).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Affinity {
    Commons,
    Hpu,
    MultiHpu,
    Vm,
}
