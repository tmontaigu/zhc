use super::*;
use crate::visualization::svg::{card_background, rail_rects};
use zhc_utils::graphics::{Frame, Height, Remaining, Size, Taken, Width};

macro_rules! vstack_fixed {
    ($name:ident, $n:literal, [$($etype:ident, $efield:ident),*]) => {
        /// Fixed vertical stack of exactly $n elements with spacing.
        pub struct $name<$($etype),*, C: Class = NoClass> {
            $(pub $efield: $etype,)*
            styler: Styler<C>,
            variable: VariableCell,
        }

        impl<$($etype),*, C: Class> $name<$($etype),*, C> {
            /// Creates a new vertical stack.
            pub fn new(modifier: Option<StyleModifier>, $($efield: $etype),*) -> Self {
                Self {
                    $($efield,)*
                    styler: Styler::new(modifier),
                    variable: VariableCell::fresh(),
                }
            }
        }

        impl<$($etype),*, C: Class> SceneElement for $name<$($etype),*, C> {
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

        impl<$($etype: SceneSolver),*, C: Class> SceneSolver for $name<$($etype),*, C> {
            fn solve_size(&mut self) {
                let style = self.styler.get();
                $(self.$efield.solve_size();)*

                let mut size = Size::ZERO.pad_top(style.padding);
                let mut has_content = false;
                $(
                    let child_size = self.$efield.get_size();
                    if child_size.height > Height::ZERO {
                        if has_content {
                            size = size.pad_bottom(style.spacing);
                        }
                        size = size.stack_vertical(child_size);
                        #[allow(unused_assignments)]
                        { has_content = true; }
                    }
                )*
                size = size.pad_bottom(style.padding);
                size = size.pad_horizontal(style.padding);
                self.variable.set_size(size);
            }

            fn solve_frame(&mut self, available: Frame) {
                let style = self.styler.get();
                let size = self.get_size();
                let frame = available.resize(&size, style.halign, style.valign);
                self.variable.set_frame(frame.clone());

                let mut remaining = frame
                    .crop_top(Height(style.padding))
                    .crop_left(Width(style.padding))
                    .crop_right(Width(style.padding));
                let mut has_content = false;
                $(
                    let child_height = self.$efield.get_size().height;
                    if child_height > Height::ZERO {
                        if has_content {
                            remaining = remaining.crop_top(Height(style.spacing));
                        }
                        let (Taken(child_frame), Remaining(new_remaining)) =
                            remaining.take_top(child_height);
                        self.$efield.solve_frame(child_frame);
                        remaining = new_remaining;
                        #[allow(unused_assignments)]
                        { has_content = true; }
                    } else {
                        // Zero-height child still needs a frame (collapsed)
                        self.$efield.solve_frame(remaining.take_top(Height::ZERO).0.0);
                    }
                )*
                let remaining = remaining.crop_top(Height(style.padding));
                remaining.assert_collapsed();
            }
        }

        impl<$($etype: Renderable),*, C: Class> Renderable for $name<$($etype),*, C> {
            fn render(&self) -> Vec<SvgElement> {
                let style = self.styler.get();
                let frame = self.get_frame();
                let mut elements = Vec::new();

                elements.extend(card_background(&style, &frame));

                // Collect non-zero-height child frames for separator rendering
                let child_frames: Vec<Frame> = vec![$(self.$efield.get_frame()),*]
                    .into_iter()
                    .filter(|f| f.size.height > Height::ZERO)
                    .collect();

                // Render separators between non-empty children if enabled
                if style.draw_separators && child_frames.len() > 1 {
                    for i in 0..child_frames.len() - 1 {
                        let sep_y = (child_frames[i].bottom_left().y.as_f64() + child_frames[i + 1].top_left().y.as_f64()) / 2.0;
                        elements.push(separator_rect(
                            &style,
                            frame.position.x.as_f64(),
                            sep_y,
                            frame.size.width.as_f64(),
                        ));
                    }
                }

                // Render children
                $(elements.extend(self.$efield.render());)*

                // Rendered last so it sits on top of the separators/children
                // instead of getting cut across by them.
                elements.extend(rail_rects(&style, &frame));

                elements
            }
        }
    };
}

vstack_fixed!(V2, 2, [E1, e1, E2, e2]);
vstack_fixed!(V4, 4, [E1, e1, E2, e2, E3, e3, E4, e4]);
vstack_fixed!(V5, 5, [E1, e1, E2, e2, E3, e3, E4, e4, E5, e5]);
