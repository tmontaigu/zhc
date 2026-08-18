use std::rc::Rc;

use crate::{
    IR, OpMap,
    visualization::{LayoutDialect, LayoutInstructionSet, visual_annotation::VisualAnnotation},
};

/// Attaches visual annotations to every operation node of a layout IR.
///
/// Recursively walks `ir`, including nested groups, and sets each
/// [`Operation`](LayoutInstructionSet::Operation) node's annotation to a copy of the
/// `annotations` entry keyed by the node's original [`OpId`](crate::OpId). Other node kinds are
/// left untouched.
///
/// # Panics
///
/// Panics if `annotations` has no entry for the original operation id of some operation node.
pub fn annotate_layout<OpAnn: VisualAnnotation + Clone>(
    ir: &mut IR<LayoutDialect>,
    annotations: &OpMap<OpAnn>,
) {
    ir.mutate_ops_linear(|op| match op {
        LayoutInstructionSet::Operation { opid, op, .. } => {
            op.annotation = Some(Rc::new(annotations.get(opid).unwrap().clone()));
        }
        LayoutInstructionSet::Group { ir, .. } => {
            annotate_layout(ir, annotations);
        }
        _ => {}
    });
}
