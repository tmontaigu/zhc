use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SchedPolicy {
    AsSoonAsPossible,
    AsLateAsPossible,
}
