use warpui::{
    AppContext, ClipBounds, Element, Event, EventContext, SizeConstraint,
    elements::{
        Axis, Border, CornerRadius, DEFAULT_SCROLL_WHEEL_PIXELS_PER_LINE, Empty, Radius,
        ScrollData, ScrollbarAppearance, ScrollbarGeometry, ScrollbarWidth,
        compute_scrollbar_geometry, project_scroll_delta_by_sensitivity,
        scroll_delta_for_pointer_movement,
    },
    event::DispatchedEvent,
    geometry::{
        rect::RectF,
        vector::{Vector2F, vec2f},
    },
    units::{IntoPixels, Pixels},
};

use crate::{
    editor::RunnableCommandModel,
    extract_block,
    render::{
        BLOCK_FOOTER_HEIGHT,
        model::{BlockItem, RenderState, viewport::ViewportItem},
    },
};

use super::{RenderContext, RenderableBlock};

const CODE_SCROLL_SENSITIVITY: f32 = 1.0;

struct CodeBlockScrollDrag {
    start_position_x: Pixels,
    start_scroll_left: f32,
    scroll_data: ScrollData,
}

/// [`RenderableBlock`] implementation for runnable command blocks.
pub struct RenderableRunnableCommand {
    viewport_item: ViewportItem,
    footer: Box<dyn Element>,
    border: Option<Border>,
    /// Current horizontal scroll offset in pixels.
    scroll_left: f32,
    /// Natural (intrinsic) width of the code content, updated each paint.
    natural_width: f32,
    /// Clip/viewport bounds set during paint and reused in dispatch_event.
    viewport_bounds: Option<RectF>,
    /// Scrollbar geometry computed during paint.
    scrollbar: Option<ScrollbarGeometry>,
    scrollbar_hovered: bool,
    scrollbar_drag: Option<CodeBlockScrollDrag>,
}

impl RenderableRunnableCommand {
    pub fn new(
        viewport_item: ViewportItem,
        model: Option<&dyn RunnableCommandModel>,
        editor_is_focused: bool,
        ctx: &AppContext,
    ) -> Self {
        let border = model.as_ref().and_then(|model| model.border(ctx));
        let footer = match model {
            Some(model) => model.render_block_footer(editor_is_focused, ctx),
            None => Empty::new().finish(),
        };

        Self {
            viewport_item,
            footer,
            border,
            scroll_left: 0.0,
            natural_width: 0.0,
            viewport_bounds: None,
            scrollbar: None,
            scrollbar_hovered: false,
            scrollbar_drag: None,
        }
    }
}

/// Maximum reachable scroll position for a code block.
pub(crate) fn code_block_max_scroll(natural_width: f32, container_width: f32) -> f32 {
    (natural_width - container_width).max(0.0)
}

/// Scroll data describing the current scroll state for scrollbar geometry calculations.
pub(crate) fn code_block_scroll_data(
    natural_width: f32,
    container_width: f32,
    scroll_left: f32,
) -> ScrollData {
    ScrollData {
        scroll_start: scroll_left.into_pixels(),
        visible_px: container_width.into_pixels(),
        total_size: natural_width.into_pixels(),
    }
}

/// Extracts a horizontal scroll delta from a scroll-wheel event, matching the table behaviour:
/// fires only when the user explicitly scrolls horizontally (shift-scroll or a horizontal
/// trackpad gesture).
pub(crate) fn code_block_horizontal_scroll_delta(
    delta: Vector2F,
    precise: bool,
    shift: bool,
) -> Option<f32> {
    let delta = if shift && delta.x().abs() <= f32::EPSILON {
        vec2f(delta.y(), 0.0)
    } else {
        delta
    };
    let projected_delta = project_scroll_delta_by_sensitivity(delta, CODE_SCROLL_SENSITIVITY);
    let horizontal = projected_delta.x();
    if horizontal.abs() <= f32::EPSILON {
        return None;
    }
    Some(if precise {
        horizontal
    } else {
        horizontal * DEFAULT_SCROLL_WHEEL_PIXELS_PER_LINE
    })
}

impl RenderableBlock for RenderableRunnableCommand {
    fn viewport_item(&self) -> &ViewportItem {
        &self.viewport_item
    }

    fn layout(&mut self, _model: &RenderState, ctx: &mut warpui::LayoutContext, app: &AppContext) {
        self.footer.layout(
            SizeConstraint::strict(vec2f(
                self.viewport_item.content_size.x(),
                BLOCK_FOOTER_HEIGHT,
            )),
            ctx,
            app,
        );
    }

    fn paint(&mut self, model: &RenderState, ctx: &mut RenderContext, app: &AppContext) {
        let content = model.content();
        let code_block = extract_block!(
            self.viewport_item,
            content,
            (block, BlockItem::RunnableCodeBlock { paragraph_block, .. }) => block.code_block(paragraph_block)
        );

        let styles = model.styles();
        let code_style = &styles.code_text;

        let border = if ctx.focused {
            self.border.unwrap_or(styles.code_border)
        } else {
            styles.code_border
        };

        // Background is drawn before the clip layer so rounded corners aren't clipped.
        let background_rect = self.viewport_item.visible_bounds(ctx);
        ctx.paint
            .scene
            .draw_rect_without_hit_recording(background_rect)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
            .with_border(border)
            .with_background(model.styles().code_background);

        // Compute natural vs container widths for horizontal scrolling.
        let container_width = self.viewport_item.content_size.x();
        let natural_width = code_block.item.content_size().x();
        self.natural_width = natural_width;

        let max_scroll = code_block_max_scroll(natural_width, container_width);
        self.scroll_left = self.scroll_left.clamp(0.0, max_scroll);

        // Viewport bounds used for clipping text content and the scrollbar.
        let content_origin_screen = ctx.content_to_screen(code_block.content_origin());
        let viewport_size = vec2f(container_width, code_block.item.height().as_f32());
        let viewport_bounds = RectF::new(content_origin_screen, viewport_size);
        self.viewport_bounds = Some(viewport_bounds);

        // Compute scrollbar geometry when there is content to scroll.
        self.scrollbar = if max_scroll > 0.0 {
            let scroll_data =
                code_block_scroll_data(natural_width, container_width, self.scroll_left);
            let scrollbar = compute_scrollbar_geometry(
                Axis::Horizontal,
                viewport_bounds.origin(),
                viewport_bounds.size(),
                scroll_data,
                ScrollbarAppearance::new(ScrollbarWidth::Auto, true),
            );
            scrollbar.has_thumb().then_some(scrollbar)
        } else {
            self.scrollbar_drag = None;
            self.scrollbar_hovered = false;
            None
        };

        // Clip text content to the container width and paint with scroll offset.
        ctx.paint
            .scene
            .start_layer(ClipBounds::BoundedByActiveLayerAnd(viewport_bounds));

        for paragraph in code_block.paragraphs() {
            ctx.draw_paragraph_scrolled(&paragraph, code_style, model, self.scroll_left);
        }

        // Paint scrollbar thumb inside the clip layer.
        if let Some(scrollbar) = self.scrollbar {
            let active = self.scrollbar_hovered || self.scrollbar_drag.is_some();
            ctx.paint
                .scene
                .draw_rect_without_hit_recording(scrollbar.thumb_bounds)
                .with_background(if active {
                    styles.table_style.scrollbar_active_thumb_color
                } else {
                    styles.table_style.scrollbar_nonactive_thumb_color
                })
                .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.0)));
        }

        ctx.paint.scene.stop_layer();

        // Place the footer at a higher z-index outside the clip layer.
        ctx.paint.scene.start_layer(ClipBounds::ActiveLayer);
        let content_rect = self.viewport_item.content_bounds(ctx);
        let button_origin = content_rect.lower_right()
            - vec2f(
                self.footer.size().expect("Footer should be laid out").x(),
                0.,
            );
        self.footer.paint(button_origin, ctx.paint, app);
        ctx.paint.scene.stop_layer();
    }

    fn after_layout(&mut self, ctx: &mut warpui::AfterLayoutContext, app: &warpui::AppContext) {
        self.footer.after_layout(ctx, app);
    }

    fn dispatch_event(
        &mut self,
        _model: &RenderState,
        event: &DispatchedEvent,
        ctx: &mut EventContext,
        app: &AppContext,
    ) -> bool {
        if self.footer.dispatch_event(event, ctx, app) {
            return true;
        }

        let container_width = self.viewport_item.content_size.x();

        match event.raw_event() {
            Event::LeftMouseDown { position, .. } => {
                let Some(scrollbar) = self.scrollbar else {
                    return false;
                };
                let thumb_hit = scrollbar.thumb_bounds.contains_point(*position);
                let track_hit = scrollbar.track_bounds.contains_point(*position);

                if thumb_hit {
                    let scroll_data = code_block_scroll_data(
                        self.natural_width,
                        container_width,
                        self.scroll_left,
                    );
                    self.scrollbar_drag = Some(CodeBlockScrollDrag {
                        start_position_x: position.x().into_pixels(),
                        start_scroll_left: self.scroll_left,
                        scroll_data,
                    });
                    ctx.notify();
                    return true;
                }

                if track_hit {
                    let scroll_data = code_block_scroll_data(
                        self.natural_width,
                        container_width,
                        self.scroll_left,
                    );
                    let delta = scroll_delta_for_pointer_movement(
                        scrollbar.thumb_center_along(Axis::Horizontal),
                        position.x().into_pixels(),
                        scroll_data,
                    );
                    let new_scroll = (self.scroll_left - delta.as_f32()).clamp(
                        0.0,
                        code_block_max_scroll(self.natural_width, container_width),
                    );
                    if (new_scroll - self.scroll_left).abs() > f32::EPSILON {
                        self.scroll_left = new_scroll;
                        ctx.notify();
                    }
                    return true;
                }

                false
            }
            Event::LeftMouseDragged { position, .. } => {
                let Some(drag) = &self.scrollbar_drag else {
                    return false;
                };
                let delta = scroll_delta_for_pointer_movement(
                    drag.start_position_x,
                    position.x().into_pixels(),
                    drag.scroll_data,
                );
                let new_scroll = (drag.start_scroll_left - delta.as_f32()).clamp(
                    0.0,
                    code_block_max_scroll(self.natural_width, container_width),
                );
                if (new_scroll - self.scroll_left).abs() > f32::EPSILON {
                    self.scroll_left = new_scroll;
                    ctx.notify();
                }
                true
            }
            Event::LeftMouseUp { .. } => {
                let had_drag = self.scrollbar_drag.take().is_some();
                if had_drag {
                    ctx.notify();
                }
                had_drag
            }
            Event::MouseMoved { position, .. } => {
                let hovered = self
                    .scrollbar
                    .is_some_and(|sb| sb.thumb_bounds.contains_point(*position));
                if hovered != self.scrollbar_hovered {
                    self.scrollbar_hovered = hovered;
                    ctx.notify();
                }
                // Never consume MouseMoved so downstream handlers (hover-link, cursor) still fire.
                false
            }
            Event::ScrollWheel {
                position,
                delta,
                precise,
                modifiers,
            } if !modifiers.ctrl => {
                let Some(bounds) = self.viewport_bounds else {
                    return false;
                };
                if !bounds.contains_point(*position)
                    || code_block_max_scroll(self.natural_width, container_width) <= 0.0
                {
                    return false;
                }
                let Some(horizontal_delta) =
                    code_block_horizontal_scroll_delta(*delta, *precise, modifiers.shift)
                else {
                    return false;
                };
                let new_scroll = (self.scroll_left - horizontal_delta).clamp(
                    0.0,
                    code_block_max_scroll(self.natural_width, container_width),
                );
                let changed = (new_scroll - self.scroll_left).abs() > f32::EPSILON;
                if changed {
                    self.scroll_left = new_scroll;
                    ctx.notify();
                }
                changed
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod code_block_scroll_tests {
    use warpui::geometry::vector::vec2f;

    use super::*;

    #[test]
    fn max_scroll_zero_when_content_fits() {
        assert_eq!(code_block_max_scroll(100.0, 200.0), 0.0);
    }

    #[test]
    fn max_scroll_zero_when_content_exactly_fits() {
        assert_eq!(code_block_max_scroll(200.0, 200.0), 0.0);
    }

    #[test]
    fn max_scroll_positive_when_content_overflows() {
        assert_eq!(code_block_max_scroll(350.0, 200.0), 150.0);
    }

    #[test]
    fn scroll_data_reflects_current_state() {
        let data = code_block_scroll_data(400.0, 200.0, 50.0);
        assert_eq!(data.scroll_start, 50.0.into_pixels());
        assert_eq!(data.visible_px, 200.0.into_pixels());
        assert_eq!(data.total_size, 400.0.into_pixels());
    }

    #[test]
    fn horizontal_delta_returns_none_for_plain_vertical_scroll() {
        let delta = vec2f(0.0, -3.0);
        assert!(code_block_horizontal_scroll_delta(delta, false, false).is_none());
    }

    #[test]
    fn horizontal_delta_fires_on_shift_scroll() {
        let delta = vec2f(0.0, -3.0);
        let result = code_block_horizontal_scroll_delta(delta, false, true);
        assert!(result.is_some());
        // shift maps y → x, so negative y (scroll down) = positive horizontal delta
        assert!(result.unwrap() < 0.0);
    }

    #[test]
    fn horizontal_delta_fires_on_horizontal_trackpad_gesture() {
        let delta = vec2f(-5.0, 0.0);
        let result = code_block_horizontal_scroll_delta(delta, true, false);
        assert!(result.is_some());
        assert!(result.unwrap() < 0.0);
    }

    #[test]
    fn horizontal_delta_precise_skips_line_multiplier() {
        let delta = vec2f(-1.0, 0.0);
        let precise = code_block_horizontal_scroll_delta(delta, true, false).unwrap();
        let imprecise = code_block_horizontal_scroll_delta(delta, false, false).unwrap();
        assert!(imprecise.abs() > precise.abs());
    }

    #[test]
    fn scroll_left_clamps_to_zero() {
        // Simulate clamping logic used in paint().
        let natural = 300.0_f32;
        let container = 200.0_f32;
        let raw = -50.0_f32;
        let clamped = raw.clamp(0.0, code_block_max_scroll(natural, container));
        assert_eq!(clamped, 0.0);
    }

    #[test]
    fn scroll_left_clamps_to_max_scroll() {
        let natural = 300.0_f32;
        let container = 200.0_f32;
        let raw = 500.0_f32;
        let clamped = raw.clamp(0.0, code_block_max_scroll(natural, container));
        assert_eq!(clamped, 100.0);
    }
}
