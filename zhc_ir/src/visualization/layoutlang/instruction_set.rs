use std::{hash::Hash, rc::Rc};

use zhc_utils::{small::SmallVec, svec};

use crate::{
    Dialect, DialectInstructionSet, Format, FormatContext, IR, OpId, OpRef, Signature, ValId, sig,
    visualization::{
        layoutlang::{LayoutDialect, LayoutTypeSystem},
        visual_annotation::VisualAnnotation,
    },
};

/// Renderable content of an [`Operation`](LayoutInstructionSet::Operation) node.
///
/// Holds the pre-formatted strings to display for one original operation, plus optional
/// decorations. Equality and hashing compare the textual fields only; the annotation fields are
/// intentionally excluded, so two contents differing only in their annotations compare equal.
#[derive(Debug)]
pub struct OpContent {
    /// Formatted argument values, one string per input edge.
    pub args: SmallVec<String>,
    /// Formatted return values, one string per output edge.
    pub returns: SmallVec<String>,
    /// Formatted operation call, without types or comments.
    pub call: String,
    /// Optional comment line displayed alongside the node.
    pub comment: Option<String>,
    /// Optional visual annotation, attached by [`annotate_layout`](super::annotate_layout).
    pub annotation: Option<Rc<dyn VisualAnnotation>>,
    /// Optional per-input visual annotations, attached by
    /// [`annotate_layout_vals`](super::annotate_layout_vals).
    pub arg_annotations: SmallVec<Option<Rc<dyn VisualAnnotation>>>,
    /// Optional per-output visual annotations, attached by
    /// [`annotate_layout_vals`](super::annotate_layout_vals).
    pub return_annotations: SmallVec<Option<Rc<dyn VisualAnnotation>>>,
}

impl Clone for OpContent {
    fn clone(&self) -> Self {
        Self {
            args: self.args.clone(),
            returns: self.returns.clone(),
            call: self.call.clone(),
            comment: self.comment.clone(),
            annotation: self.annotation.clone(),
            arg_annotations: self.arg_annotations.clone(),
            return_annotations: self.return_annotations.clone(),
        }
    }
}

impl PartialEq for OpContent {
    fn eq(&self, other: &Self) -> bool {
        // Annotation is intentionally excluded from equality
        self.args == other.args
            && self.returns == other.returns
            && self.call == other.call
            && self.comment == other.comment
    }
}

impl Eq for OpContent {}

impl Hash for OpContent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Annotation is intentionally excluded from hashing
        self.args.hash(state);
        self.returns.hash(state);
        self.call.hash(state);
        self.comment.hash(state);
    }
}

impl OpContent {
    /// Builds the display content of an operation of any dialect.
    ///
    /// The arguments and returns of the operation behind `opref` are formatted with `ctx`, while
    /// the call itself is formatted with types and comments disabled. `comment` and all the
    /// annotation fields start out as `None`/empty.
    pub fn from_op<'ir, D: Dialect>(opref: &OpRef<'ir, D>, ctx: &FormatContext) -> Self {
        OpContent {
            args: opref
                .get_args_iter()
                .map(|a| a.fmt_to_string(ctx))
                .collect(),
            returns: opref
                .get_returns_iter()
                .map(|a| a.fmt_to_string(ctx))
                .collect(),
            call: opref.fmt_to_string(&ctx.clone().show_comments(false).show_types(false)),
            comment: None,
            annotation: None,
            arg_annotations: svec![],
            return_annotations: svec![],
        }
    }
}

/// Instruction set for the layout dialect.
///
/// Each instruction is a node to draw, and its signature (exposed by the
/// [`DialectInstructionSet`] impl) gives its edge arity in
/// [`Value`](LayoutTypeSystem::Value)s:
///
/// **`Operation`** is a plain display node standing for one operation of the original IR. It
/// carries the original [`OpId`], the pre-formatted [`OpContent`] to render, and the original
/// argument and return [`ValId`]s; its arity follows the argument and return counts of its
/// content.
///
/// **`Group`** holds a whole nested `IR<LayoutDialect>` to draw as a named box. Its arity is
/// derived from that nested IR: one input per `GroupInput` op and one output per `GroupOutput`
/// op it contains. **`GroupInput`** (no input, one output) and **`GroupOutput`** (one input, no
/// output) mark, inside the nested IR, where the group's `pos`-th argument enters and where its
/// `pos`-th result leaves.
///
/// **`Dummy`** forwards a value unchanged (one input, one output), padding an edge so that it
/// spans one rendering layer at a time.
///
/// `Dummy`, `GroupInput`, and `GroupOutput` record in `valid` the original IR value they carry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LayoutInstructionSet {
    Operation {
        opid: OpId,
        op: OpContent,
        args: SmallVec<ValId>,
        returns: SmallVec<ValId>,
    },
    Dummy {
        valid: ValId,
    },
    Group {
        ir: IR<LayoutDialect>,
        name: String,
    },
    GroupInput {
        pos: u16,
        valid: ValId,
    },
    GroupOutput {
        pos: u16,
        valid: ValId,
    },
}

impl Format for LayoutInstructionSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, ctx: &crate::FormatContext) -> std::fmt::Result {
        match self {
            LayoutInstructionSet::Operation { opid, .. } => write!(f, "operation<{opid}>"),
            LayoutInstructionSet::Dummy { valid, .. } => write!(f, "dummy<{valid}>"),
            LayoutInstructionSet::Group { ir, name, .. } => {
                let inner_ctx = ctx.with_prefix("    ").with_next_nested_prefix();
                writeln!(f, "group<\"{}\"> {{", name)?;
                Format::fmt(ir, f, &inner_ctx)?;
                write!(f, "\n{}}}", ctx.prefix())
            }
            LayoutInstructionSet::GroupInput { pos, .. } => write!(f, "group_input<{pos}>"),
            LayoutInstructionSet::GroupOutput { pos, .. } => write!(f, "group_output<{pos}>"),
        }
    }
}

impl DialectInstructionSet for LayoutInstructionSet {
    type TypeSystem = LayoutTypeSystem;

    fn get_signature(&self) -> crate::Signature<Self::TypeSystem> {
        match self {
            LayoutInstructionSet::Operation { op, .. } => Signature(
                svec![LayoutTypeSystem::Value; op.args.len()],
                svec![LayoutTypeSystem::Value; op.returns.len()],
            ),
            LayoutInstructionSet::Dummy { .. } => {
                sig![(LayoutTypeSystem::Value) -> (LayoutTypeSystem::Value)]
            }
            LayoutInstructionSet::Group { ir, .. } => {
                let n_inputs = ir
                    .walk_ops_linear()
                    .filter(|op| {
                        matches!(
                            op.get_instruction(),
                            LayoutInstructionSet::GroupInput { .. }
                        )
                    })
                    .count();
                let n_outputs = ir
                    .walk_ops_linear()
                    .filter(|op| {
                        matches!(
                            op.get_instruction(),
                            LayoutInstructionSet::GroupOutput { .. }
                        )
                    })
                    .count();
                Signature(
                    svec![LayoutTypeSystem::Value; n_inputs],
                    svec![LayoutTypeSystem::Value; n_outputs],
                )
            }
            LayoutInstructionSet::GroupInput { .. } => sig![() -> (LayoutTypeSystem::Value)],
            LayoutInstructionSet::GroupOutput { .. } => sig![(LayoutTypeSystem::Value) -> ()],
        }
    }
}
