use zhc_utils::graphics::{Frame, Size, VAlign};

use super::*;
use crate::visualization::svg::{DominantBaseline, TextAnchor};

/// Text element that renders string content with typography styling.
pub struct TextBox<C: Class = NoClass> {
    pub content: String,
    styler: Styler<C>,
    variable: VariableCell,
}

impl<C: Class> TextBox<C> {
    /// Creates a new text box with the given content string.
    pub fn new(modifier: Option<StyleModifier>, content: String) -> Self {
        Self {
            content,
            styler: Styler::new(modifier),
            variable: VariableCell::fresh(),
        }
    }
}

impl<C: Class> SceneElement for TextBox<C> {
    fn get_size(&self) -> Size {
        self.variable.get_size()
    }

    fn get_frame(&self) -> Frame {
        self.variable.get_frame()
    }

    fn get_variable_cell(&self) -> VariableCell {
        self.variable.clone()
    }
}

impl<C: Class> SceneSolver for TextBox<C> {
    fn solve_size(&mut self) {
        let style = self.styler.get();
        let size = style
            .font_size
            .get_text_size(&self.content)
            .pad(style.padding);
        self.variable.set_size(size);
    }

    fn solve_frame(&mut self, available: Frame) {
        let style = self.styler.get();
        let frame = available.resize(&self.get_size(), style.halign, style.valign);
        self.variable.set_frame(frame);
    }
}

impl<C: Class> Renderable for TextBox<C> {
    fn render(&self) -> Vec<SvgElement> {
        let style = self.styler.get();
        let frame = self.get_frame();
        let mut elements = Vec::new();

        elements.extend(background_rect(&style, &frame));

        // The anchor y and its paired dominant-baseline both come from
        // `font_valign` — they need to agree, or text renders off from
        // where its box says it should sit.
        let line_height = style.font_size.as_f64() * 1.2;
        let n_lines = self.content.lines().count().max(1) as f64;
        let block_height = line_height * n_lines;

        let (first_line_y, dominant_baseline) = match style.font_valign {
            VAlign::Top => (
                frame.position.y.as_f64() + style.padding.as_f64(),
                DominantBaseline::Hanging,
            ),
            VAlign::Center => (
                frame.position.y.as_f64()
                    + (frame.size.height.as_f64() - block_height) / 2.0
                    + line_height / 2.0,
                DominantBaseline::Middle,
            ),
            VAlign::Bottom => (
                frame.position.y.as_f64() + frame.size.height.as_f64()
                    - style.padding.as_f64()
                    - block_height
                    + line_height,
                DominantBaseline::Auto,
            ),
        };

        for (line_index, line) in self.content.lines().enumerate() {
            elements.push(SvgElement::Text {
                x: frame.position.x.as_f64() + style.padding.as_f64(),
                y: first_line_y + line_index as f64 * line_height,
                content: line.to_string(),
                font_size: style.font_size.as_f64(),
                font_family: Some(style.font.0.to_string()),
                fill: Some(style.font_color.to_string()),
                text_anchor: TextAnchor::from(style.font_halign),
                dominant_baseline: dominant_baseline.clone(),
                class: None,
                id: None,
            });
        }

        elements
    }
}
