use std::ops::Deref;

use zhc_utils::Dumpable;
use zhc_utils::files::FileHandle;

use super::*;
use crate::{
    AnnOpRef, AnnValRef, AsOpId, AsValId, Dialect, Formatted, IR, OpId, OpMap, ValId, ValMap,
    visualization::{Hierarchy, VisualAnnotation, draw_ann_ir_to_html},
};

/// A read-only, annotated view over an [`IR`], pairing operations and values with associated
/// data.
///
/// Combines an [`IR`] with per-operation and per-value annotation maps, giving each active
/// operation and value an attached [`Annotation`] alongside the underlying IR data. Constructed
/// via [`new`](Self::new), which requires both annotation maps to be fully populated.
#[derive(Debug, Clone)]
pub struct AnnIRView<'ir, 'ann, D: Dialect, OpAnn: Annotation, ValAnn: Annotation> {
    pub(crate) ir: &'ir IR<D>,
    pub(crate) op_annotations: &'ann OpMap<OpAnn>,
    pub(crate) val_annotations: &'ann ValMap<ValAnn>,
}

impl<'ir, 'ann, D: Dialect, OpAnn: Annotation, ValAnn: Annotation>
    AnnIRView<'ir, 'ann, D, OpAnn, ValAnn>
{
    /// Creates a view combining the IR with its operation and value annotations.
    ///
    /// Associates `ir` with `op_annotations` and `val_annotations`, exposing each active
    /// operation and value alongside its corresponding annotation.
    ///
    /// # Panics
    ///
    /// Panics if `op_annotations` or `val_annotations` does not have an annotation stored for
    /// every active operation or value, respectively, in `ir`.
    pub fn new(
        ir: &'ir IR<D>,
        op_annotations: &'ann OpMap<OpAnn>,
        val_annotations: &'ann ValMap<ValAnn>,
    ) -> Self {
        assert!(
            op_annotations.is_filled(),
            "Operation annotations map must be filled for all active operations"
        );
        assert!(
            val_annotations.is_filled(),
            "Value annotations map must be filled for all active values"
        );
        Self {
            ir,
            op_annotations,
            val_annotations,
        }
    }

    /// Returns a reference to the operation annotations map.
    pub fn op_annotations(&self) -> &OpMap<OpAnn> {
        &self.op_annotations
    }

    /// Returns a reference to the value annotations map.
    pub fn val_annotations(&self) -> &ValMap<ValAnn> {
        &self.val_annotations
    }

    /// Returns an annotated operation reference for the specified operation.
    ///
    /// # Panics
    ///
    /// Panics if the operation ID does not exist or refers to an inactive operation.
    pub fn get_op(&self, opid: impl AsOpId) -> AnnOpRef<'ir, 'ann, D, OpAnn, ValAnn> {
        let opid = opid.op_id();
        let opref = self.ir.get_op(opid);
        let ann = &self.op_annotations[opid];
        AnnOpRef {
            ir: self.clone(),
            opref,
            ann,
        }
    }

    /// Returns an annotated value reference for the specified value.
    ///
    /// # Panics
    ///
    /// Panics if the value ID does not exist, refers to an inactive value.
    pub fn get_val(&self, valid: impl AsValId) -> AnnValRef<'ir, 'ann, D, OpAnn, ValAnn> {
        let valid = valid.val_id();
        let valref = self.ir.get_val(valid);
        let ann = &self.val_annotations[valid];
        AnnValRef {
            ir: self.clone(),
            valref,
            ann,
        }
    }

    /// Returns an iterator over all active operations with annotations in linear order.
    pub fn walk_ops_linear(
        &self,
    ) -> impl DoubleEndedIterator<Item = AnnOpRef<'ir, 'ann, D, OpAnn, ValAnn>>
    + use<'ir, 'ann, D, OpAnn, ValAnn> {
        let op_anns = self.op_annotations;
        let ir = self.clone();
        self.ir.walk_ops_linear().map(move |opref| {
            let ann = &op_anns[&opref];
            AnnOpRef {
                ir: ir.clone(),
                opref,
                ann,
            }
        })
    }

    /// Returns an iterator over all active operations with annotations in topological order.
    pub fn walk_ops_topological(
        &self,
    ) -> impl DoubleEndedIterator<Item = AnnOpRef<'ir, 'ann, D, OpAnn, ValAnn>>
    + use<'ir, 'ann, D, OpAnn, ValAnn> {
        let op_anns = self.op_annotations;
        let ir = self.clone();
        self.ir.walk_ops_topological().map(move |opref| {
            let ann = &op_anns[&opref];
            AnnOpRef {
                ir: ir.clone(),
                opref,
                ann,
            }
        })
    }

    /// Returns an iterator over operations with annotations using a custom walker.
    pub fn walk_ops_with<W: Iterator<Item = OpId>>(
        &self,
        walker: W,
    ) -> impl Iterator<Item = AnnOpRef<'ir, 'ann, D, OpAnn, ValAnn>> + use<'ir, 'ann, W, D, OpAnn, ValAnn>
    {
        let op_anns = self.op_annotations;
        let ir = self.clone();
        self.ir.walk_ops_with(walker).map(move |opref| {
            let ann = &op_anns[&opref];
            AnnOpRef {
                ir: ir.clone(),
                opref,
                ann,
            }
        })
    }

    /// Returns an iterator over all active values with annotations in linear order.
    pub fn walk_vals_linear(
        &self,
    ) -> impl DoubleEndedIterator<Item = AnnValRef<'ir, 'ann, D, OpAnn, ValAnn>>
    + use<'ir, 'ann, D, OpAnn, ValAnn> {
        let val_anns = self.val_annotations;
        let ir = self.clone();
        self.ir.walk_vals_linear().map(move |valref| {
            let ann = &val_anns[&valref];
            AnnValRef {
                ir: ir.clone(),
                valref,
                ann,
            }
        })
    }

    /// Returns an iterator over values with annotations using a custom walker.
    pub fn walk_vals_with<W: Iterator<Item = ValId>>(
        &self,
        walker: W,
    ) -> impl Iterator<Item = AnnValRef<'ir, 'ann, D, OpAnn, ValAnn>> + use<'ir, 'ann, W, D, OpAnn, ValAnn>
    {
        let val_anns = self.val_annotations;
        let ir = self.clone();
        self.ir.walk_vals_with(walker).map(move |valref| {
            let ann = &val_anns[&valref];
            AnnValRef {
                ir: ir.clone(),
                valref,
                ann,
            }
        })
    }

    /// Renders this annotated view as an interactive HTML file.
    ///
    /// Equivalent to [`draw_ann_ir_to_html`], requiring `OpAnn` to implement
    /// [`VisualAnnotation`]. The returned handle points at a freshly created temporary file.
    ///
    /// # Panics
    ///
    /// Panics if the file cannot be written.
    pub fn draw_to_html(&self, hierarchy_ann: Option<OpMap<Hierarchy>>) -> FileHandle
    where
        OpAnn: VisualAnnotation,
    {
        draw_ann_ir_to_html(self, hierarchy_ann)
    }

    /// Creates a configurable formatter for the annotated IR.
    pub fn format(&self) -> Formatted<'_, Self> {
        Formatted::new(self)
    }
}

impl<'ir, 'ann, D: Dialect, OpAnn: Annotation, ValAnn: Annotation> Deref
    for AnnIRView<'ir, 'ann, D, OpAnn, ValAnn>
{
    type Target = IR<D>;

    fn deref(&self) -> &Self::Target {
        self.ir
    }
}

impl<'ir, 'ann, D: Dialect, OpAnn: Annotation, ValAnn: Annotation> Dumpable
    for AnnIRView<'ir, 'ann, D, OpAnn, ValAnn>
{
    fn dump_to_string(&self) -> String {
        format!(
            "{}",
            self.format()
                .with_walker(crate::PrintWalker::Linear)
                .show_types(false)
                .show_opid(true)
                .show_comments(true)
        )
    }
}
