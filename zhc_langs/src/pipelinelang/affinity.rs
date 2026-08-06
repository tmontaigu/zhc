#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Affinity {
    Commons,
    Hpu,
    MultiHpu,
    Vm,
}
