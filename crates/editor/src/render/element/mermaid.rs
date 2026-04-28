use warpui::{
    AppContext, Element, SizeConstraint,
    elements::{Align, CacheOption, CornerRadius, Image, Radius, Text},
    geometry::vector::vec2f,
};

use crate::{
    extract_block,
    render::{
        element::paint::CursorData,
        model::{BlockItem, RenderState, viewport::ViewportItem},
    },
};

use super::{CursorDisplayType, RenderContext, RenderableBlock};

pub struct RenderableMermaidDiagram {
    viewport_item: ViewportItem,
    image_element: Option<Box<dyn Element>>,
}

impl RenderableMermaidDiagram {
    pub fn new(viewport_item: ViewportItem) -> Self {
        Self {
            viewport_item,
            image_element: None,
        }
    }
}

impl RenderableBlock for RenderableMermaidDiagram {
    fn viewport_item(&self) -> &ViewportItem {
        &self.viewport_item
    }

    fn layout(&mut self, model: &RenderState, ctx: &mut warpui::LayoutContext, app: &AppContext) {
        let content = model.content();
        let (asset_source, config) = extract_block!(
            self.viewport_item,
            content,
            (_block, BlockItem::MermaidDiagram { asset_source, config, .. }) => (asset_source.clone(), *config)
        );

        let code_text = model.styles().code_text;
        let placeholder = Align::new(
            Text::new(
                "Rendering Mermaid diagram…",
                code_text.font_family,
                code_text.font_size,
            )
            .with_color(model.styles().placeholder_color)
            .with_line_height_ratio(code_text.line_height_ratio)
            .soft_wrap(false)
            .finish(),
        )
        .finish();

        let size = vec2f(config.width.as_f32(), config.height.as_f32());
        let mut image = Image::new(asset_source, CacheOption::BySize)
            .contain()
            .before_load(placeholder);
        image.layout(SizeConstraint::strict(size), ctx, app);

        self.image_element = Some(Box::new(image));
    }

    fn paint(&mut self, model: &RenderState, ctx: &mut RenderContext, app: &AppContext) {
        let content = model.content();
        let (start_offset, end_offset) = extract_block!(
            self.viewport_item,
            content,
            (block, BlockItem::MermaidDiagram { .. }) => (block.start_char_offset, block.end_char_offset())
        );

        let visible_rect = self.viewport_item.visible_bounds(ctx);
        let content_rect = self.viewport_item.content_bounds(ctx);

        ctx.paint
            .scene
            .draw_rect_without_hit_recording(visible_rect)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
            .with_border(model.styles().code_border)
            .with_background(model.styles().code_background);

        if let Some(ref mut image_element) = self.image_element {
            image_element.paint(content_rect.origin(), ctx.paint, app);
        }

        let selected = model
            .selections()
            .iter()
            .any(|selection| selection.start() < end_offset && selection.end() + 1 > start_offset);
        if selected {
            ctx.paint
                .scene
                .draw_rect_with_hit_recording(content_rect)
                .with_background(model.styles().selection_fill);
        }

        if model.is_selection_head(start_offset) {
            ctx.draw_and_save_cursor(
                CursorDisplayType::Bar,
                content_rect.origin(),
                vec2f(
                    model.styles().cursor_width,
                    self.viewport_item.content_size.y(),
                ),
                CursorData::default(),
                model.styles(),
            );
        }
    }

    fn after_layout(&mut self, ctx: &mut warpui::AfterLayoutContext, app: &warpui::AppContext) {
        if let Some(ref mut image_element) = self.image_element {
            image_element.after_layout(ctx, app);
        }
    }
}
