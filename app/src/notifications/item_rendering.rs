use std::sync::Arc;

use pathfinder_color::ColorU;
use warp_core::ui::icons::Icon;
use warp_core::ui::theme::{Fill, WarpTheme};
use warpui::clipboard::ClipboardContent;
use warpui::elements::{
    ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    Element, EventHandler, Flex, MainAxisAlignment, MainAxisSize, ParentElement, Radius, Rect,
    Shrinkable,
};
use warpui::fonts::Weight;
use warpui::ui_components::components::{UiComponent, UiComponentStyles};
use warpui::{SingletonEntity, View, ViewContext, ViewHandle};

use warp_core::ui::appearance::Appearance as CoreAppearance;
use warp_core::ui::theme::color::internal_colors;

use crate::ai::agent::conversation::ConversationStatus;
use crate::ai::artifacts::{Artifact, ArtifactButtonsRow, ArtifactButtonsRowEvent};
use crate::appearance::Appearance;
use crate::notifications::item::NotificationSourceAgent;
use crate::notifications::telemetry::{ArtifactType, NotificationsTelemetryEvent};
use crate::notifications::{NotificationCategory, NotificationItem};
use crate::send_telemetry_from_ctx;
use crate::ui_components::icon_with_status::{
    render_icon_with_status, IconWithStatusSizing, IconWithStatusVariant,
};
use crate::util::time_format::format_elapsed_since;
use crate::view_components::action_button::ActionButtonTheme;
use crate::workspace::WorkspaceAction;

const COLLAPSED_MAX_CHARS: usize = 100;
const EXPANDED_MAX_CHARS: usize = 500;

fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() > max_chars - 3 {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{truncated}…")
    } else {
        text.to_owned()
    }
}

/// title 或 message 是否会被 `truncate_text` 截断。
fn content_is_truncated(title: &str, message: &str) -> bool {
    title.chars().count() > COLLAPSED_MAX_CHARS - 3
        || message.chars().count() > COLLAPSED_MAX_CHARS - 3
}

/// 区分 toast vs mailbox 渲染差异。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotificationRenderContext {
    Toast,
    Mailbox,
}

/// 通知里 artifact chip 的按钮主题。
/// 使用 `outline` 边框,以便在 `surface_2` 背景上仍可见。
pub(crate) struct NotificationArtifactButtonTheme;

impl ActionButtonTheme for NotificationArtifactButtonTheme {
    fn background(&self, hovered: bool, appearance: &CoreAppearance) -> Option<Fill> {
        if hovered {
            Some(internal_colors::fg_overlay_2(appearance.theme()))
        } else {
            None
        }
    }

    fn text_color(
        &self,
        _hovered: bool,
        _background: Option<Fill>,
        appearance: &CoreAppearance,
    ) -> ColorU {
        appearance.theme().foreground().into_solid()
    }

    fn border(&self, appearance: &CoreAppearance) -> Option<ColorU> {
        Some(appearance.theme().outline().into_solid())
    }
}

/// 用户在被截断的消息上点击展开/折叠时回调。
pub(crate) type OnExpandClick = Box<dyn Fn(&mut warpui::EventContext)>;

/// 渲染通知项的内层内容。
/// 根据 `item.branch` 是否存在分发到 rich(带 branch 行) 或 simple 布局。
pub(crate) fn render_notification_item_content(
    item: &NotificationItem,
    artifact_buttons: Option<&ViewHandle<ArtifactButtonsRow>>,
    context: NotificationRenderContext,
    message_expanded: bool,
    on_expand_click: OnExpandClick,
    extra_content: Option<Box<dyn Element>>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();

    let text_column = if item.branch.is_some() {
        render_rich_text_column(
            item,
            artifact_buttons,
            context,
            message_expanded,
            on_expand_click,
            extra_content,
            appearance,
        )
    } else {
        render_simple_text_column(
            item,
            artifact_buttons,
            context,
            message_expanded,
            on_expand_click,
            extra_content,
            appearance,
        )
    };

    Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(
            Container::new(render_agent_avatar(item.agent, item.category, theme))
                .with_margin_right(8.)
                .with_margin_top(2.)
                .finish(),
        )
        .with_child(Shrinkable::new(1.0, text_column).finish())
        .finish()
}

/// Rich 布局:branch 行 + clamped title + clamped message + artifact 按钮。
fn render_rich_text_column(
    item: &NotificationItem,
    artifact_buttons: Option<&ViewHandle<ArtifactButtonsRow>>,
    context: NotificationRenderContext,
    message_expanded: bool,
    on_expand_click: OnExpandClick,
    extra_content: Option<Box<dyn Element>>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let branch = item.branch.as_deref().unwrap_or_default();

    let branch_left = render_branch_label(branch, appearance);
    let is_truncated = content_is_truncated(&item.title, &item.message);

    let branch_right: Box<dyn Element> = match context {
        NotificationRenderContext::Toast if is_truncated || message_expanded => {
            render_expand_chevron(message_expanded, on_expand_click, theme)
        }
        NotificationRenderContext::Toast => {
            // 内容能完整显示时不渲染 chevron。
            Flex::row().finish()
        }
        NotificationRenderContext::Mailbox => render_timestamp_with_dot(item, appearance),
    };

    let branch_row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(branch_left)
        .with_child(branch_right)
        .finish();

    let title = render_clamped_title(&item.title, message_expanded, appearance);
    let message = render_message_text(&item.message, message_expanded, appearance);

    let mut content = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(branch_row)
        .with_child(title)
        .with_child(Container::new(message).with_margin_top(2.).finish());

    append_trailing_content(&mut content, artifact_buttons, extra_content);
    content.finish()
}

/// Simple 布局:title (+ 可选 chevron) | timestamp 行 + message + artifact 按钮。
fn render_simple_text_column(
    item: &NotificationItem,
    artifact_buttons: Option<&ViewHandle<ArtifactButtonsRow>>,
    context: NotificationRenderContext,
    message_expanded: bool,
    on_expand_click: OnExpandClick,
    extra_content: Option<Box<dyn Element>>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let is_truncated = content_is_truncated(&item.title, &item.message);
    let title_text = render_clamped_title(&item.title, message_expanded, appearance);

    let title_row: Box<dyn Element> = if context == NotificationRenderContext::Mailbox {
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(Shrinkable::new(1.0, title_text).finish())
            .with_child(render_timestamp_with_dot(item, appearance))
            .finish()
    } else if is_truncated || message_expanded {
        let chevron = render_expand_chevron(message_expanded, on_expand_click, theme);
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(Shrinkable::new(1.0, title_text).finish())
            .with_child(Container::new(chevron).with_margin_top(2.).finish())
            .finish()
    } else {
        title_text
    };

    let message = render_message_text(&item.message, message_expanded, appearance);

    let mut content = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(title_row)
        .with_child(Container::new(message).with_margin_top(2.).finish());

    append_trailing_content(&mut content, artifact_buttons, extra_content);
    content.finish()
}

/// 把 artifact 按钮和 extra content 追加到 text column。
fn append_trailing_content(
    content: &mut Flex,
    artifact_buttons: Option<&ViewHandle<ArtifactButtonsRow>>,
    extra_content: Option<Box<dyn Element>>,
) {
    if let Some(artifact_buttons) = artifact_buttons {
        content.add_child(
            Container::new(ChildView::new(artifact_buttons).finish())
                .with_margin_top(8.)
                .finish(),
        );
    }
    if let Some(extra) = extra_content {
        content.add_child(extra);
    }
}

/// git-branch 图标 + branch 名标签。
fn render_branch_label(branch: &str, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let color = theme.sub_text_color(theme.surface_1());

    Shrinkable::new(
        1.0,
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(2.)
            .with_child(
                ConstrainedBox::new(Icon::GitBranch.to_warpui_icon(color).finish())
                    .with_width(10.)
                    .with_height(10.)
                    .finish(),
            )
            .with_child(
                Shrinkable::new(
                    1.0,
                    appearance
                        .ui_builder()
                        .wrappable_text(branch.to_owned(), false)
                        .with_style(UiComponentStyles {
                            font_size: Some(12.),
                            font_color: Some(color.into()),
                            font_family_id: Some(appearance.ui_font_family()),
                            ..Default::default()
                        })
                        .build()
                        .finish(),
                )
                .finish(),
            )
            .finish(),
    )
    .finish()
}

/// 时间戳文本 + 可选未读小红点。
fn render_timestamp_with_dot(item: &NotificationItem, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();

    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min)
        .with_child(
            appearance
                .ui_builder()
                .wrappable_text(format_elapsed_since(item.created_at), false)
                .with_style(UiComponentStyles {
                    font_size: Some(12.),
                    font_color: Some(theme.disabled_text_color(theme.surface_1()).into()),
                    font_family_id: Some(appearance.ui_font_family()),
                    ..Default::default()
                })
                .build()
                .finish(),
        );

    if !item.is_read {
        row.add_child(
            Container::new(
                ConstrainedBox::new(
                    Rect::new()
                        .with_background(theme.accent())
                        .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
                        .finish(),
                )
                .with_width(10.)
                .with_height(10.)
                .finish(),
            )
            .with_margin_left(8.)
            .finish(),
        );
    }

    row.finish()
}

/// 可点击的展开/折叠 chevron。
fn render_expand_chevron(
    expanded: bool,
    on_click: OnExpandClick,
    theme: &WarpTheme,
) -> Box<dyn Element> {
    let icon = if expanded {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    };
    let chevron = ConstrainedBox::new(
        icon.to_warpui_icon(theme.disabled_text_color(theme.surface_1()))
            .finish(),
    )
    .with_width(12.)
    .with_height(12.)
    .finish();

    EventHandler::new(chevron)
        .on_left_mouse_down(move |ctx, _, _| {
            on_click(ctx);
            DispatchEventResult::StopPropagation
        })
        .finish()
}

/// 标题文本,按 expanded 状态决定截断长度。
fn render_clamped_title(title: &str, expanded: bool, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let max = if expanded {
        EXPANDED_MAX_CHARS
    } else {
        COLLAPSED_MAX_CHARS
    };

    appearance
        .ui_builder()
        .wrappable_text(truncate_text(title, max), true)
        .with_style(UiComponentStyles {
            font_size: Some(14.),
            font_weight: Some(Weight::Semibold),
            font_color: Some(theme.main_text_color(theme.surface_1()).into()),
            font_family_id: Some(appearance.ui_font_family()),
            ..Default::default()
        })
        .build()
        .finish()
}

/// 消息正文,按 expanded 状态决定截断长度。
fn render_message_text(message: &str, expanded: bool, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let max = if expanded {
        EXPANDED_MAX_CHARS
    } else {
        COLLAPSED_MAX_CHARS
    };

    appearance
        .ui_builder()
        .wrappable_text(truncate_text(message, max), true)
        .with_style(UiComponentStyles {
            font_size: Some(14.),
            font_color: Some(theme.sub_text_color(theme.surface_1()).into()),
            font_family_id: Some(appearance.ui_font_family()),
            ..Default::default()
        })
        .build()
        .finish()
}

const NOTIFICATION_AVATAR_SIZING: IconWithStatusSizing = IconWithStatusSizing {
    icon_size: 16.,
    padding: 8.,
    badge_icon_size: 12.,
    badge_padding: 2.,
    overall_size_override: None,
    badge_offset: (6., 6.),
};

fn render_agent_avatar(
    agent: NotificationSourceAgent,
    category: NotificationCategory,
    theme: &WarpTheme,
) -> Box<dyn Element> {
    let status = notification_category_to_conversation_status(category);
    let variant = match agent {
        NotificationSourceAgent::Oz => IconWithStatusVariant::OzAgent {
            status: Some(status),
            is_ambient: false,
        },
        NotificationSourceAgent::CLI(cli) => IconWithStatusVariant::CLIAgent {
            agent: cli,
            status: Some(status),
        },
    };
    render_icon_with_status(
        variant,
        &NOTIFICATION_AVATAR_SIZING,
        theme,
        theme.surface_2(),
    )
}

fn notification_category_to_conversation_status(
    category: NotificationCategory,
) -> ConversationStatus {
    match category {
        NotificationCategory::Complete => ConversationStatus::Success,
        NotificationCategory::Request => ConversationStatus::Blocked {
            blocked_action: String::new(),
        },
        NotificationCategory::Error => ConversationStatus::Error,
    }
}

/// 创建带通知专用主题的 `ArtifactButtonsRow` view。
/// 调用方需要自己订阅返回 view 的事件。
pub(crate) fn create_notification_artifact_buttons_view(
    artifacts: &[Artifact],
    ctx: &mut ViewContext<impl View>,
) -> Option<ViewHandle<ArtifactButtonsRow>> {
    if artifacts.is_empty() {
        return None;
    }
    let theme = Arc::new(NotificationArtifactButtonTheme);
    Some(ctx.add_typed_action_view(|ctx| ArtifactButtonsRow::with_theme(artifacts, theme, ctx)))
}

/// 处理通知 view (toast / mailbox) 里的 artifact 按钮事件。
pub(crate) fn handle_notification_artifact_buttons_event(
    event: &ArtifactButtonsRowEvent,
    ctx: &mut ViewContext<impl View>,
) {
    match event {
        ArtifactButtonsRowEvent::OpenPlan { document_uid } => {
            send_telemetry_from_ctx!(
                NotificationsTelemetryEvent::ArtifactClicked {
                    artifact_type: ArtifactType::Plan
                },
                ctx
            );
            // openWarp 本地化:点击 plan 按钮打开本地 AIDocument pane,
            // 不再跳到云 notebook 镜像。
            let document_version =
                crate::ai::document::ai_document_model::AIDocumentModel::as_ref(ctx)
                    .get_current_document(document_uid)
                    .map(|doc| doc.version)
                    .unwrap_or_default();
            ctx.dispatch_typed_action(&WorkspaceAction::OpenAIDocumentPane {
                document_id: *document_uid,
                document_version,
            });
        }
        ArtifactButtonsRowEvent::CopyBranch { branch } => {
            send_telemetry_from_ctx!(
                NotificationsTelemetryEvent::ArtifactClicked {
                    artifact_type: ArtifactType::Branch
                },
                ctx
            );
            ctx.clipboard()
                .write(ClipboardContent::plain_text(branch.clone()));
        }
        ArtifactButtonsRowEvent::OpenPullRequest { url } => {
            send_telemetry_from_ctx!(
                NotificationsTelemetryEvent::ArtifactClicked {
                    artifact_type: ArtifactType::PullRequest
                },
                ctx
            );
            ctx.open_url(url);
        }
    }
}
