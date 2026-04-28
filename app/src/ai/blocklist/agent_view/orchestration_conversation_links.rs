use pathfinder_color::ColorU;
use pathfinder_geometry::vector::Vector2F;
use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::Fill;
use warpui::{
    elements::{
        ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty, Expanded, Flex,
        Hoverable, MainAxisSize, MouseStateHandle, ParentElement, Radius, Shrinkable, Text,
    },
    fonts::{Properties, Weight::Bold},
    platform::Cursor,
    text_layout::ClipConfig,
    AppContext, Element, EventContext, SingletonEntity,
};

use crate::{
    ai::{
        agent::{
            api::ServerConversationToken,
            conversation::{AIConversation, AIConversationId},
        },
        agent_conversations_model::AgentConversationsModel,
        blocklist::BlocklistAIHistoryModel,
    },
    ui_components::{blended_colors, icons::Icon},
    workspace::{RestoreConversationLayout, WorkspaceAction},
};

pub(crate) fn conversation_id_for_agent_id(
    agent_id: &str,
    app: &AppContext,
) -> Option<AIConversationId> {
    let history_model = BlocklistAIHistoryModel::as_ref(app);
    history_model
        .conversation_id_for_agent_id(agent_id)
        .or_else(|| {
            history_model.find_conversation_id_by_server_token(&ServerConversationToken::new(
                agent_id.to_string(),
            ))
        })
}

pub(crate) fn parent_conversation_id(
    active_conversation: &AIConversation,
    app: &AppContext,
) -> Option<AIConversationId> {
    active_conversation.parent_conversation_id().or_else(|| {
        active_conversation
            .parent_agent_id()
            .and_then(|id| conversation_id_for_agent_id(id, app))
    })
}

pub(crate) fn conversation_navigation_action(
    conversation_id: AIConversationId,
    app: &AppContext,
) -> WorkspaceAction {
    AgentConversationsModel::as_ref(app)
        .get_conversation(&conversation_id)
        .and_then(|conversation| {
            conversation.get_open_action(Some(RestoreConversationLayout::SplitPane), app)
        })
        .unwrap_or(WorkspaceAction::RestoreOrNavigateToConversation {
            pane_view_locator: None,
            window_id: None,
            conversation_id,
            terminal_view_id: None,
            restore_layout: Some(RestoreConversationLayout::SplitPane),
        })
}

pub(crate) fn parent_conversation_navigation_card(
    active_conversation: &AIConversation,
    mouse_state: MouseStateHandle,
    app: &AppContext,
) -> Option<Box<dyn Element>> {
    let parent_conversation_id = parent_conversation_id(active_conversation, app)?;
    let parent_title = BlocklistAIHistoryModel::as_ref(app)
        .conversation(&parent_conversation_id)
        .and_then(|conversation| conversation.title())
        .unwrap_or_else(|| "Parent conversation".to_string());
    let action = conversation_navigation_action(parent_conversation_id, app);
    Some(conversation_navigation_card(
        parent_title,
        Some("Back to parent conversation".to_string()),
        move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
        },
        mouse_state,
        false,
        app,
    ))
}

pub(crate) fn conversation_navigation_card(
    title: String,
    subtitle: Option<String>,
    on_click: impl FnMut(&mut EventContext, &AppContext, Vector2F) + 'static,
    mouse_state: MouseStateHandle,
    expands_to_max_width: bool,
    app: &AppContext,
) -> Box<dyn Element> {
    conversation_navigation_card_with_icon(
        None,
        title,
        subtitle,
        on_click,
        mouse_state,
        expands_to_max_width,
        None,
        app,
    )
}

/// Renders a clickable card with an optional leading icon, title/subtitle,
/// a trailing chevron, and an optional extra trailing element (e.g. a dismiss
/// button). When `extra_trailing` is provided, the card's Hoverable uses
/// `defer_events_to_children` so the trailing element can handle its own
/// click without also triggering the card's `on_click`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn conversation_navigation_card_with_icon(
    icon: Option<(Icon, ColorU)>,
    title: String,
    subtitle: Option<String>,
    on_click: impl FnMut(&mut EventContext, &AppContext, Vector2F) + 'static,
    mouse_state: MouseStateHandle,
    expands_to_max_width: bool,
    extra_trailing: Option<Box<dyn Element>>,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let has_extra_trailing = extra_trailing.is_some();

    let mut hoverable = Hoverable::new(mouse_state, move |hover_state| {
        let background = if hover_state.is_hovered() {
            blended_colors::fg_overlay_2(theme)
        } else {
            blended_colors::fg_overlay_1(theme)
        };

        let mut text_column = Flex::column().with_child(
            Text::new(
                title.clone(),
                appearance.ui_font_family(),
                appearance.monospace_font_size(),
            )
            .soft_wrap(false)
            .with_clip(ClipConfig::ellipsis())
            .with_style(Properties {
                weight: Bold,
                ..Default::default()
            })
            .with_color(blended_colors::text_main(
                theme,
                appearance.theme().background(),
            ))
            .finish(),
        );
        if let Some(subtitle) = subtitle.as_ref() {
            text_column.add_child(
                Text::new(
                    subtitle.clone(),
                    appearance.ui_font_family(),
                    (appearance.monospace_font_size() - 2.).max(10.),
                )
                .soft_wrap(false)
                .with_clip(ClipConfig::ellipsis())
                .with_color(blended_colors::text_sub(
                    theme,
                    appearance.theme().background(),
                ))
                .finish(),
            );
        }
        let text_column = text_column.finish();

        let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        if let Some((icon, color)) = icon {
            row.add_child(
                Container::new(
                    ConstrainedBox::new(icon.to_warpui_icon(Fill::Solid(color)).finish())
                        .with_width(16.)
                        .with_height(16.)
                        .finish(),
                )
                .with_margin_right(6.)
                .finish(),
            );
        }
        if expands_to_max_width {
            row = row
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(Shrinkable::new(1., text_column).finish());
        } else {
            row = row.with_child(text_column);
        }
        row.add_child(
            Container::new(
                ConstrainedBox::new(
                    Icon::ChevronRight
                        .to_warpui_icon(blended_colors::text_sub(theme, theme.background()).into())
                        .finish(),
                )
                .with_height(20.)
                .with_width(20.)
                .finish(),
            )
            .with_margin_left(8.)
            .finish(),
        );
        if let Some(trailing) = extra_trailing {
            // Spacer pushes the trailing element to the far right edge.
            row.add_child(Expanded::new(1., Empty::new().finish()).finish());
            row.add_child(trailing);
        }
        let row = row.finish();

        Container::new(row)
            .with_background_color(background.into())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
            .with_horizontal_padding(10.)
            .with_vertical_padding(8.)
            .finish()
    })
    .with_cursor(Cursor::PointingHand)
    .on_click(on_click);

    // When an extra trailing element is present (e.g. dismiss button), defer
    // click events to children so the trailing element's click handler takes
    // precedence over the card's navigation handler.
    if has_extra_trailing {
        hoverable = hoverable.with_defer_events_to_children();
    }

    hoverable.finish()
}
