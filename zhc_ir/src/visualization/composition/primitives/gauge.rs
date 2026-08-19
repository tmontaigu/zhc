use super::*;
use crate::visualization::svg::{DominantBaseline, TextAnchor};
use zhc_utils::graphics::{Color, ColorScale, Frame, Height, Size, Width};

/// A horizontal gauge displaying a percentage as a rounded bar filling from left to right.
///
/// The fill color follows a green-to-yellow-to-red ramp as the value approaches 100%. Values
/// above 100% saturate: the bar stays full and turns purple. Values at or below 0% draw the
/// empty track only. The percentage is written in the middle of the gauge, using the class
/// style's font settings.
pub struct Gauge<C: Class = NoClass> {
    percentage: f32,
    styler: Styler<C>,
    variable: VariableCell,
}

impl<C: Class> Gauge<C> {
    /// Intrinsic size of the gauge track, before padding.
    const TRACK_SIZE: Size = Size {
        width: Width::new(60.),
        height: Height::new(14.),
    };
    /// Fill color of the empty part of the track.
    const TRACK_COLOR: Color = Color::rgb(226, 229, 234);
    /// Fill color of the bar when the gauge saturates above 100%.
    const SATURATED_COLOR: Color = Color::MEDIUMPURPLE;
    /// Corner radius of the track and fill rects.
    const CORNER_RADIUS: f64 = 3.;

    /// Creates a gauge displaying the given percentage (0 to 100, saturating above).
    pub fn new(modifier: Option<StyleModifier>, percentage: f32) -> Self {
        Self {
            percentage,
            styler: Styler::new(modifier),
            variable: VariableCell::fresh(),
        }
    }
}

impl<C: Class> SceneElement for Gauge<C> {
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

impl<C: Class> SceneSolver for Gauge<C> {
    fn solve_size(&mut self) {
        let style = self.styler.get();
        let size = Self::TRACK_SIZE.pad(style.padding);
        self.variable.set_size(size);
    }

    fn solve_frame(&mut self, available: Frame) {
        let style = self.styler.get();
        let frame = available.resize(&self.get_size(), style.halign, style.valign);
        self.variable.set_frame(frame);
    }
}

impl<C: Class> Renderable for Gauge<C> {
    fn render(&self) -> Vec<SvgElement> {
        let style = self.styler.get();
        let frame = self.get_frame();
        let x = frame.position.x.as_f64() + style.padding.as_f64();
        let y = frame.position.y.as_f64() + style.padding.as_f64();
        let width = (frame.size.width.as_f64() - 2. * style.padding.as_f64()).max(0.);
        let height = (frame.size.height.as_f64() - 2. * style.padding.as_f64()).max(0.);
        let rect = |width: f64, color: Color| SvgElement::Rect {
            x,
            y,
            width,
            height,
            rx: Some(Self::CORNER_RADIUS),
            fill: Some(color.to_string()),
            stroke: None,
            stroke_width: None,
            class: None,
            id: None,
            data_val: None,
        };
        let mut elements = vec![rect(width, Self::TRACK_COLOR)];
        if self.percentage > 0. {
            let (ratio, color) = if self.percentage > 100. {
                (1., Self::SATURATED_COLOR)
            } else {
                let ratio = f64::from(self.percentage) / 100.;
                (ratio, ColorScale::TRAFFIC_LIGHT.interpolate(ratio))
            };
            // The fill is kept at least as wide as its rounded corners, so that small
            // percentages don't render as a degenerate sliver.
            elements.push(rect((width * ratio).max(2. * Self::CORNER_RADIUS), color));
        }
        elements.push(SvgElement::Text {
            x: x + width / 2.,
            y: y + height / 2.,
            content: format!("{:.0}%", self.percentage),
            font_size: style.font_size.as_f64(),
            font_family: Some(style.font.0.to_string()),
            fill: Some(style.font_color.to_string()),
            text_anchor: TextAnchor::Middle,
            dominant_baseline: DominantBaseline::Middle,
            class: None,
            id: None,
        });
        elements
    }
}
