#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedPolicy {
    AsSoonAsPossible,
    AsLateAsPossible,
}
