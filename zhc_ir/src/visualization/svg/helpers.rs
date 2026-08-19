use super::*;
use crate::visualization::composition::{CardShape, Style};
use zhc_utils::graphics::{Color, Frame, Position, Thickness, X, Y};

/// Background rect for a styled container, in the literal `fill`/`stroke`
/// colors from `style` (so a `StyleModifier` override from `VisualAnnotation`
/// always shows — nothing here can suppress it). No class attribute: types
/// that need one for a CSS/JS selector tag it themselves via `tag_background`.
pub(crate) fn background_rect(style: &Style, frame: &Frame) -> Option<SvgElement> {
    if style.fill_color == Color::TRANSPARENT && style.border_color == Color::TRANSPARENT {
        return None;
    }
    Some(SvgElement::Rect {
        x: frame.position.x.as_f64(),
        y: frame.position.y.as_f64(),
        width: frame.size.width.as_f64(),
        height: frame.size.height.as_f64(),
        rx: (style.corner_radius > Thickness::ZERO).then(|| style.corner_radius.as_f64()),
        fill: Some(style.fill_color.to_string()),
        stroke: Some(style.border_color.to_string()),
        stroke_width: Some(style.border_width.as_f64()),
        class: None,
        id: None,
        data_val: None,
    })
}

/// A hairline separator between stacked children, in the container's own
/// border color. Inset by the border width on each side, so it stops at the
/// border stroke's inner edge instead of painting over it.
pub(crate) fn separator_rect(style: &Style, x: f64, y_mid: f64, width: f64) -> SvgElement {
    let inset = style.border_width.as_f64();
    SvgElement::Rect {
        x: x + inset,
        y: y_mid - inset / 2.0,
        width: (width - 2.0 * inset).max(0.0),
        height: inset,
        rx: None,
        fill: Some(style.border_color.to_string()),
        stroke: None,
        stroke_width: None,
        class: None,
        id: None,
        data_val: None,
    }
}

/// A hairline separator between horizontally stacked children, in the
/// container's own border color. Inset by the border width on each side, so it
/// stops at the border stroke's inner edge instead of painting over it.
pub(crate) fn vseparator_rect(style: &Style, x_mid: f64, y: f64, height: f64) -> SvgElement {
    let inset = style.border_width.as_f64();
    SvgElement::Rect {
        x: x_mid - inset / 2.0,
        y: y + inset,
        width: inset,
        height: (height - 2.0 * inset).max(0.0),
        rx: None,
        fill: Some(style.border_color.to_string()),
        stroke: None,
        stroke_width: None,
        class: None,
        id: None,
        data_val: None,
    }
}

/// Card background: a plain rounded rect, or a ticket-stub zigzag on the
/// free edge for `CardShape::SawtoothTop/Bottom` — purely decorative, the
/// solver still treats the card as its ordinary rectangular frame.
pub(crate) fn card_background(style: &Style, frame: &Frame) -> Option<SvgElement> {
    match style.card_shape {
        CardShape::Rect => background_rect(style, frame),
        CardShape::SawtoothTop => Some(sawtooth_path(style, frame, true)),
        CardShape::SawtoothBottom => Some(sawtooth_path(style, frame, false)),
    }
}

/// Small colored bars at the card's left/right edges signaling its op-kind,
/// driven by `Style::accent_color` (`Color::TRANSPARENT` draws none) — not
/// part of the layout, just a render-time decoration.
pub(crate) fn rail_rects(style: &Style, frame: &Frame) -> Vec<SvgElement> {
    if style.accent_color == Color::TRANSPARENT {
        return vec![];
    }
    let inset = 6.0;
    let rail_width = 4.0;
    let x0 = frame.position.x.as_f64();
    let x1 = x0 + frame.size.width.as_f64();
    let y = frame.position.y.as_f64() + inset;
    let height = (frame.size.height.as_f64() - 2.0 * inset).max(0.0);
    let fill = Some(style.accent_color.to_string());
    [x0 + 5.0, x1 - rail_width - 5.0]
        .into_iter()
        .map(|x| SvgElement::Rect {
            x,
            y,
            width: rail_width,
            height,
            rx: Some(rail_width / 2.0),
            fill: fill.clone(),
            stroke: None,
            stroke_width: None,
            class: None,
            id: None,
            data_val: None,
        })
        .collect()
}

const TOOTH_HALF: f64 = 7.0;
const TOOTH_AMPLITUDE: f64 = 7.0;

/// Zigzag `LineTo` commands from `x0` to `x1`, centered, with flat filler
/// segments on each side for any remainder that doesn't divide evenly into
/// a tooth. Peaks sit at `base_y + outward`; valleys stay flush with
/// `base_y` (so the tooth reads as an outward notch, not a symmetric wave).
/// `outward` is negative for the top edge, since screen y grows downward.
fn zigzag_edge(x0: f64, x1: f64, base_y: f64, outward: f64) -> Vec<PathCommand> {
    let pos = |x: f64, y: f64| Position {
        x: X::new(x),
        y: Y::new(y),
    };
    let span = x1 - x0;
    let dir = span.signum();
    let total = span.abs();
    let period = TOOTH_HALF * 2.0;
    let teeth = (total / period).floor();
    let lead = (total - teeth * period) / 2.0;

    let mut commands = Vec::new();
    let mut x = x0 + dir * lead;
    commands.push(PathCommand::LineTo(pos(x, base_y)));
    for _ in 0..(teeth as usize) {
        x += dir * TOOTH_HALF;
        commands.push(PathCommand::LineTo(pos(x, base_y + outward)));
        x += dir * TOOTH_HALF;
        commands.push(PathCommand::LineTo(pos(x, base_y)));
    }
    commands.push(PathCommand::LineTo(pos(x1, base_y)));
    commands
}

fn sawtooth_path(style: &Style, frame: &Frame, teeth_on_top: bool) -> SvgElement {
    let x0 = frame.position.x.as_f64();
    let y0 = frame.position.y.as_f64();
    let x1 = x0 + frame.size.width.as_f64();
    let y1 = y0 + frame.size.height.as_f64();
    let r = style.corner_radius.as_f64();
    let pos = |x: f64, y: f64| Position {
        x: X::new(x),
        y: Y::new(y),
    };
    let arc_to = |end_x: f64, end_y: f64| PathCommand::EllipticalArc {
        rx: r,
        ry: r,
        x_axis_rotation: 0.0,
        large_arc: false,
        sweep: true,
        end: pos(end_x, end_y),
    };

    let mut commands = Vec::new();
    if teeth_on_top {
        // Square top corners with a zigzag top edge, rounded bottom corners.
        commands.push(PathCommand::MoveTo(pos(x0, y0)));
        commands.extend(zigzag_edge(x0, x1, y0, -TOOTH_AMPLITUDE));
        commands.push(PathCommand::LineTo(pos(x1, y1 - r)));
        commands.push(arc_to(x1 - r, y1));
        commands.push(PathCommand::LineTo(pos(x0 + r, y1)));
        commands.push(arc_to(x0, y1 - r));
        commands.push(PathCommand::ClosePath);
    } else {
        // Rounded top corners, a zigzag bottom edge with square bottom corners.
        commands.push(PathCommand::MoveTo(pos(x0, y0 + r)));
        commands.push(arc_to(x0 + r, y0));
        commands.push(PathCommand::LineTo(pos(x1 - r, y0)));
        commands.push(arc_to(x1, y0 + r));
        commands.push(PathCommand::LineTo(pos(x1, y1)));
        commands.extend(zigzag_edge(x1, x0, y1, TOOTH_AMPLITUDE));
        commands.push(PathCommand::ClosePath);
    }

    SvgElement::Path {
        commands,
        fill: Some(style.fill_color.to_string()),
        stroke: Some(style.border_color.to_string()),
        stroke_width: Some(style.border_width.as_f64()),
        class: None,
        id: None,
        title: None,
        data_val: None,
    }
}

/// Tags the first element (the card/group background, always pushed first)
/// with a structural class, for the CSS elevation selector and the
/// color-picker's node lookup in script.js — it never drives color.
pub(crate) fn tag_background(mut elements: Vec<SvgElement>, class: &str) -> Vec<SvgElement> {
    if let Some(SvgElement::Rect { class: c, .. } | SvgElement::Path { class: c, .. }) =
        elements.first_mut()
    {
        *c = Some(class.to_string());
    }
    elements
}
