use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warp_core::ui::icons::Icon as WarpIcon;
use warp_core::ui::theme::color::internal_colors;
use warp_core::ui::theme::{Fill as WarpThemeFill, WarpTheme};
use warpui::elements::{
    ChildAnchor, ConstrainedBox, Container, CornerRadius, Element, OffsetPositioning, ParentAnchor,
    ParentElement, ParentOffsetBounds, Radius, Stack,
};

use crate::ai::agent::conversation::ConversationStatus;
use crate::terminal::CLIAgent;
use crate::themes::theme::Fill as ThemeFill;

/// Sizing configuration for the icon circle and its status badge.
pub(crate) struct IconWithStatusSizing {
    pub(crate) icon_size: f32,
    pub(crate) padding: f32,
    pub(crate) badge_icon_size: f32,
    pub(crate) badge_padding: f32,
    /// The overall constrained size for the stack.
    /// When set, overrides the default `icon_size + padding * 2`.
    pub(crate) overall_size_override: Option<f32>,
    /// Offset of the status badge from the bottom-right corner of the circle.
    /// Positive x pushes right, positive y pushes down.
    pub(crate) badge_offset: (f32, f32),
}

/// What to render inside the circle.
pub(crate) enum IconWithStatusVariant {
    /// A generic icon with a given color on an overlay background.
    Neutral {
        icon: WarpIcon,
        icon_color: WarpThemeFill,
    },
    /// A pre-built icon element on an overlay background.
    NeutralElement { icon_element: Box<dyn Element> },
    /// An Oz agent icon on the theme background.
    OzAgent {
        status: Option<ConversationStatus>,
        is_ambient: bool,
    },
    /// A CLI agent icon on the agent's brand color background.
    CLIAgent {
        agent: CLIAgent,
        status: Option<ConversationStatus>,
    },
}

/// Renders an icon inside a circle with an optional status badge overlay.
pub(crate) fn render_icon_with_status(
    variant: IconWithStatusVariant,
    sizing: &IconWithStatusSizing,
    theme: &WarpTheme,
    badge_ring_background: WarpThemeFill,
) -> Box<dyn Element> {
    let sub_text = theme.sub_text_color(theme.background());

    match variant {
        IconWithStatusVariant::Neutral { icon, icon_color } => {
            let inner = ConstrainedBox::new(icon.to_warpui_icon(icon_color).finish())
                .with_width(sizing.icon_size)
                .with_height(sizing.icon_size)
                .finish();
            Container::new(inner)
                .with_uniform_padding(sizing.padding)
                .with_background(internal_colors::fg_overlay_2(theme))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                    (sizing.icon_size + sizing.padding * 2.) / 2.,
                )))
                .finish()
        }
        IconWithStatusVariant::NeutralElement { icon_element } => {
            let inner = ConstrainedBox::new(icon_element)
                .with_width(sizing.icon_size)
                .with_height(sizing.icon_size)
                .finish();
            Container::new(inner)
                .with_uniform_padding(sizing.padding)
                .with_background(internal_colors::fg_overlay_2(theme))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                    (sizing.icon_size + sizing.padding * 2.) / 2.,
                )))
                .finish()
        }
        IconWithStatusVariant::OzAgent { status, is_ambient } => {
            let icon = if is_ambient {
                WarpIcon::OzCloud
            } else {
                WarpIcon::Oz
            };
            let inner = ConstrainedBox::new(
                icon.to_warpui_icon(theme.main_text_color(theme.background()))
                    .finish(),
            )
            .with_width(sizing.icon_size)
            .with_height(sizing.icon_size)
            .finish();
            let circle = Container::new(inner)
                .with_uniform_padding(sizing.padding)
                .with_background(theme.background())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                    (sizing.icon_size + sizing.padding * 2.) / 2.,
                )))
                .finish();
            render_with_optional_status_badge(
                circle,
                status.as_ref(),
                sizing,
                theme,
                badge_ring_background,
            )
        }
        IconWithStatusVariant::CLIAgent { agent, status } => {
            let brand_color = agent
                .brand_color()
                .unwrap_or(ColorU::new(100, 100, 100, 255));
            let icon_color = agent.brand_icon_color();
            let icon_element = agent
                .icon()
                .map(|icon| {
                    icon.to_warpui_icon(WarpThemeFill::Solid(icon_color))
                        .finish()
                })
                .unwrap_or_else(|| WarpIcon::Terminal.to_warpui_icon(sub_text).finish());
            let inner = ConstrainedBox::new(icon_element)
                .with_width(sizing.icon_size)
                .with_height(sizing.icon_size)
                .finish();
            let circle = Container::new(inner)
                .with_uniform_padding(sizing.padding)
                .with_background(ThemeFill::Solid(brand_color))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                    (sizing.icon_size + sizing.padding * 2.) / 2.,
                )))
                .finish();
            render_with_optional_status_badge(
                circle,
                status.as_ref(),
                sizing,
                theme,
                badge_ring_background,
            )
        }
    }
}

/// Adds a status badge with a cutout ring to the bottom-right of the circle.
fn render_with_optional_status_badge(
    circle: Box<dyn Element>,
    status: Option<&ConversationStatus>,
    sizing: &IconWithStatusSizing,
    theme: &WarpTheme,
    badge_ring_background: WarpThemeFill,
) -> Box<dyn Element> {
    let Some(status) = status else {
        return circle;
    };
    let (icon, color) = status.status_icon_and_color(theme);
    let badge_icon = ConstrainedBox::new(icon.to_warpui_icon(WarpThemeFill::Solid(color)).finish())
        .with_width(sizing.badge_icon_size)
        .with_height(sizing.badge_icon_size)
        .finish();
    let badge = Container::new(badge_icon)
        .with_uniform_padding(sizing.badge_padding)
        .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
        .finish();
    // Cutout ring that visually separates the badge from the circle.
    let badge_with_ring = Container::new(badge)
        .with_uniform_padding(sizing.badge_padding)
        .with_background(badge_ring_background)
        .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
        .finish();

    let circle_size = sizing.icon_size + sizing.padding * 2.;
    let overall_size = sizing.overall_size_override.unwrap_or(circle_size);
    let mut stack = Stack::new().with_child(
        ConstrainedBox::new(circle)
            .with_width(overall_size)
            .with_height(overall_size)
            .finish(),
    );
    stack.add_positioned_child(
        badge_with_ring,
        OffsetPositioning::offset_from_parent(
            vec2f(sizing.badge_offset.0, sizing.badge_offset.1),
            ParentOffsetBounds::ParentBySize,
            ParentAnchor::BottomRight,
            ChildAnchor::BottomRight,
        ),
    );
    ConstrainedBox::new(stack.finish())
        .with_width(overall_size)
        .with_height(overall_size)
        .finish()
}
