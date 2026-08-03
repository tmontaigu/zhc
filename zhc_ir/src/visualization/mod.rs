use crate::{AnnIR, AnnIRView, Annotation, Dialect, IR, OpMap, visualization::svg::Svg};
use zhc_utils::files::{Extension, FileHandle};

mod composition;
mod hierarchy;
mod html;
mod layer_map;
mod layoutlang;
mod placement;
mod svg;
mod visual_annotation;

pub use composition::*;
pub use hierarchy::*;
pub use layer_map::*;
pub use layoutlang::*;
pub use visual_annotation::*;

#[cfg(test)]
mod test;

fn draw_ir<D: Dialect>(ir: &IR<D>, hierarchy_ann: Option<OpMap<Hierarchy>>) -> Svg {
    let hierarchy_ann = hierarchy_ann.unwrap_or(ir.filled_opmap(Hierarchy::new()));
    let ann_ir = AnnIR::new(ir, hierarchy_ann, ir.filled_valmap(()));
    let layout_ir = generate_layout_ir(&ann_ir);
    let placed_ir = placement::place(&layout_ir);
    let scene = composition::compose(&placed_ir);
    svg::draw(&scene)
}

fn draw_ann_ir<D: Dialect, OpAnn: Annotation + VisualAnnotation, ValAnn: Annotation>(
    ir: &AnnIRView<D, OpAnn, ValAnn>,
    hierarchy_ann: Option<OpMap<Hierarchy>>,
) -> Svg {
    let hierarchy_ann = hierarchy_ann.unwrap_or(ir.filled_opmap(Hierarchy::new()));
    let ann_ir = AnnIR::new(ir, hierarchy_ann, ir.filled_valmap(()));
    let mut layout_ir = generate_layout_ir(&ann_ir);
    annotate_layout(&mut layout_ir, ir.op_annotations());
    let placed_ir = placement::place(&layout_ir);
    let scene = composition::compose(&placed_ir);
    svg::draw(&scene)
}

/// Renders an IR graph as a static SVG file.
///
/// The visualization displays operations as nodes and data dependencies as edges.
/// Operations sharing the same hierarchy annotation are grouped together visually,
/// making the logical structure of the program easier to follow.
///
/// For an interactive version with zoom and pan, use [`draw_ir_to_html`]. To include
/// custom per-operation visual annotations (e.g., computed values), use
/// [`draw_ann_ir_to_svg`] instead.
///
/// The returned handle points at a freshly created temporary file.
///
/// # Panics
///
/// Panics if the file cannot be written.
pub fn draw_ir_to_svg<D: Dialect>(
    ir: &IR<D>,
    hierarchy_ann: Option<OpMap<Hierarchy>>,
) -> FileHandle {
    let svg_output = draw_ir(ir, hierarchy_ann);
    let svg_content = format!("{}", svg_output);
    let handle = FileHandle::random(Extension::Svg);
    std::fs::write(&handle, svg_content).expect("Failed to write SVG file");
    handle
}

/// Renders an annotated IR graph as a static SVG file.
///
/// Like [`draw_ir_to_svg`], but accepts an [`AnnIRView`] whose operation annotations implement
/// [`VisualAnnotation`]. Each operation's annotation can provide a custom widget (displayed
/// inside the node) and a style modifier (affecting the node's appearance). This is useful
/// for visualizing interpreter results, optimization metadata, or any per-operation data.
///
/// For an interactive version with zoom and pan, use [`draw_ann_ir_to_html`].
///
/// The returned handle points at a freshly created temporary file.
///
/// # Panics
///
/// Panics if the file cannot be written.
pub fn draw_ann_ir_to_svg<D: Dialect, OpAnn: Annotation + VisualAnnotation, ValAnn: Annotation>(
    ir: &AnnIRView<D, OpAnn, ValAnn>,
    hierarchy_ann: Option<OpMap<Hierarchy>>,
) -> FileHandle {
    let svg_output = draw_ann_ir(ir, hierarchy_ann);
    let svg_content = format!("{}", svg_output);
    let handle = FileHandle::random(Extension::Svg);
    std::fs::write(&handle, svg_content).expect("Failed to write SVG file");
    handle
}

/// Renders an IR graph as an interactive HTML file.
///
/// The visualization displays operations as nodes and data dependencies as edges.
/// Operations sharing the same hierarchy annotation are grouped together visually,
/// making the logical structure of the program easier to follow. The resulting HTML
/// file supports interactive features such as zooming and panning.
///
/// For a static SVG without interactivity, use [`draw_ir_to_svg`]. To include custom
/// per-operation visual annotations (e.g., computed values), use [`draw_ann_ir_to_html`]
/// instead.
///
/// The returned handle points at a freshly created temporary file.
///
/// # Panics
///
/// Panics if the file cannot be written.
pub fn draw_ir_to_html<D: Dialect>(
    ir: &IR<D>,
    hierarchy_ann: Option<OpMap<Hierarchy>>,
) -> FileHandle {
    let svg_output = draw_ir(ir, hierarchy_ann);
    let html_output = html::wrap_svg(svg_output);
    let html_content = format!("{}", html_output);
    let handle = FileHandle::random(Extension::Html);
    std::fs::write(&handle, html_content).expect("Failed to write HTML file");
    handle
}

/// Renders an annotated IR graph as an interactive HTML file.
///
/// Like [`draw_ir_to_html`], but accepts an [`AnnIRView`] whose operation annotations implement
/// [`VisualAnnotation`]. Each operation's annotation can provide a custom widget (displayed
/// inside the node) and a style modifier (affecting the node's appearance). This is useful
/// for visualizing interpreter results, optimization metadata, or any per-operation data.
/// The resulting HTML file supports interactive features such as zooming and panning.
///
/// For a static SVG without interactivity, use [`draw_ann_ir_to_svg`].
///
/// The returned handle points at a freshly created temporary file.
///
/// # Panics
///
/// Panics if the file cannot be written.
pub fn draw_ann_ir_to_html<D: Dialect, OpAnn: Annotation + VisualAnnotation, ValAnn: Annotation>(
    ir: &AnnIRView<D, OpAnn, ValAnn>,
    hierarchy_ann: Option<OpMap<Hierarchy>>,
) -> FileHandle {
    let svg_output = draw_ann_ir(ir, hierarchy_ann);
    let html_output = html::wrap_svg(svg_output);
    let html_content = format!("{}", html_output);
    let handle = FileHandle::random(Extension::Html);
    std::fs::write(&handle, html_content).expect("Failed to write HTML file");
    handle
}
