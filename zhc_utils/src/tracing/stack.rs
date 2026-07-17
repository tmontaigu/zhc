use serde::Serialize;

pub type StackId = usize;
pub type StackMember = String;

#[derive(Serialize, Debug, Clone, Default)]
#[serde(untagged)]
pub enum MaybeStackTrace {
    Stack {
        stack: Vec<StackMember>,
    },
    StackFrame {
        sf: StackId,
    },
    #[default]
    None,
}
