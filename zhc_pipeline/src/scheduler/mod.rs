pub mod two_step;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingDirection {
    Forward,
    Backward,
}
