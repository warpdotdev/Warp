//! Rendering functions for orchestration-related output items (messaging & agent management).

use pathfinder_color::ColorU;
use warpui::elements::{
    ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty, Flex, Hoverable,
    MouseStateHandle, ParentElement, Radius, Text,
};
use warpui::platform::Cursor;
use warpui::{AppContext, Element, SingletonEntity};

use markdown_parser::{FormattedText, FormattedTextFragment, FormattedTextLine};
use warpui::elements::FormattedTextElement;

use crate::ai::agent::conversation::{AIConversation, AIConversationId, ConversationStatus};
use crate::ai::agent::{
    AIAgentActionId, AIAgentActionResultType, MessageId, OrchestrateActionResult,
    OrchestrateAgentOutcome, OrchestrateAgentRunConfig, OrchestrateExecutionMode,
    ReceivedMessageDisplay, SendMessageToAgentResult, StartAgentExecutionMode, StartAgentResult,
};
use crate::ai::blocklist::action_model::AIActionStatus;
use crate::ai::blocklist::action_model::OrchestrateDecision;
use crate::ai::blocklist::agent_view::orchestration_conversation_links::{
    conversation_id_for_agent_id, conversation_navigation_card_with_icon,
};
use crate::ai::blocklist::block::model::AIBlockModelHelper;
use crate::ai::blocklist::block::{
    AIBlockAction, CollapsibleExpansionState, OrchestrateButtonHandles, OrchestrateConfigDropdown,
};
use crate::ai::blocklist::inline_action::inline_action_header::{
    ICON_MARGIN, INLINE_ACTION_HEADER_VERTICAL_PADDING, INLINE_ACTION_HORIZONTAL_PADDING,
};
use crate::ai::blocklist::inline_action::inline_action_icons::{self, icon_size};
use crate::ai::blocklist::inline_action::requested_action::{
    render_requested_action_row, render_requested_action_row_for_text,
};
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::ai::cloud_environments::CloudAmbientAgentEnvironment;
use crate::appearance::Appearance;
use crate::cloud_object::model::generic_string_model::StringModel;
use crate::terminal::view::TerminalAction;
use crate::ui_components::blended_colors;
use crate::ui_components::icons::Icon;
use warp_core::ui::theme::Fill;

use super::common::render_scrollable_collapsible_content;
use super::output::{action_icon, Props};
use super::WithContentItemSpacing;

const GENERATING_TITLE_PLACEHOLDER: &str = "Generating title...";
const ORCHESTRATION_COLLAPSED_MAX_HEIGHT: f32 = 200.;

fn agent_display_name_from_id(
    agent_id: &str,
    orchestrator_agent_id: Option<&str>,
    app: &AppContext,
) -> String {
    if orchestrator_agent_id.is_some_and(|id| id == agent_id) {
        return "Orchestrator agent".to_string();
    }
    if let Some(conversation_id) = conversation_id_for_agent_id(agent_id, app) {
        if let Some(conversation) =
            BlocklistAIHistoryModel::as_ref(app).conversation(&conversation_id)
        {
            if let Some(agent_name) = conversation.agent_name() {
                return agent_name.to_string();
            }
        }
    }
    "Unknown agent".to_string()
}

fn orchestrator_agent_id_for_conversation(
    conversation: &AIConversation,
    app: &AppContext,
) -> Option<String> {
    match conversation.parent_conversation_id() {
        Some(parent_id) => BlocklistAIHistoryModel::as_ref(app)
            .conversation(&parent_id)
            .and_then(|parent| parent.orchestration_agent_id()),
        None => conversation.orchestration_agent_id(),
    }
}

fn render_message_fields(
    fields: &[(&str, &str)],
    body: &str,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.monospace_font_size();
    let label_color = blended_colors::text_disabled(theme, theme.surface_2());
    let value_color: ColorU = theme.main_text_color(theme.background()).into();

    let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    for (label, value) in fields {
        let line = Flex::row()
            .with_child(
                Text::new(label.to_string(), font_family, font_size)
                    .with_color(label_color)
                    .finish(),
            )
            .with_child(
                Text::new(value.to_string(), font_family, font_size)
                    .with_color(value_color)
                    .finish(),
            )
            .finish();
        column.add_child(line);
    }

    if !body.is_empty() {
        column.add_child(
            Container::new(
                Text::new(body.to_string(), font_family, font_size)
                    .with_color(value_color)
                    .finish(),
            )
            .with_margin_top(4.)
            .finish(),
        );
    }

    column.finish()
}

pub(super) fn render_messages_received_from_agents(
    messages: &[ReceivedMessageDisplay],
    props: Props,
    message_id: &MessageId,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();

    let status_icon = inline_action_icons::green_check_icon(appearance).finish();
    let chevron = render_collapse_chevron(message_id, props, app);

    let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    // Header row with icon and collapse chevron
    let header = render_requested_action_row_for_text(
        format!("Messages received ({})", messages.len()).into(),
        appearance.ui_font_family(),
        Some(status_icon),
        chevron,
        false,
        false,
        app,
    );
    column.add_child(header);

    let orchestrator_agent_id = props
        .model
        .conversation(app)
        .and_then(|conversation| orchestrator_agent_id_for_conversation(conversation, app));

    // Collect all messages into a single collapsible body.
    let mut messages_column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    for msg in messages {
        let sender_name =
            agent_display_name_from_id(&msg.sender_agent_id, orchestrator_agent_id.as_deref(), app);
        let recipients = msg
            .addresses
            .iter()
            .map(|agent_id| {
                agent_display_name_from_id(agent_id, orchestrator_agent_id.as_deref(), app)
            })
            .collect::<Vec<_>>()
            .join(", ");
        let fields = [
            ("From: ", sender_name.as_str()),
            ("To: ", recipients.as_str()),
            ("Subject: ", msg.subject.as_str()),
        ];
        let message_block = Container::new(render_message_fields(&fields, &msg.message_body, app))
            .with_margin_top(8.)
            .with_margin_left(8.)
            .finish();
        messages_column.add_child(message_block);
    }

    if let Some(body) = render_collapsible_body(message_id, messages_column.finish(), false, props)
    {
        column.add_child(body);
    }

    Container::new(column.finish())
        .with_horizontal_padding(8.)
        .with_vertical_padding(8.)
        .with_background_color(blended_colors::neutral_2(theme))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
        .finish()
        .with_agent_output_item_spacing(app)
        .finish()
}

pub(super) fn render_send_message(
    props: Props,
    action_id: &AIAgentActionId,
    address: &[String],
    subject: &str,
    message: &str,
    message_id: &MessageId,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let status = props.action_model.as_ref(app).get_action_status(action_id);
    let orchestrator_agent_id = props
        .model
        .conversation(app)
        .and_then(|conversation| orchestrator_agent_id_for_conversation(conversation, app));
    let recipients = address
        .iter()
        .map(|agent_id| agent_display_name_from_id(agent_id, orchestrator_agent_id.as_deref(), app))
        .collect::<Vec<_>>()
        .join(", ");

    if let Some(AIActionStatus::Finished(result)) = &status {
        let AIAgentActionResultType::SendMessageToAgent(result) = &result.result else {
            log::error!(
                "Unexpected action result type for send message action: {:?}",
                result.result
            );
            return Empty::new().finish();
        };
        match result {
            SendMessageToAgentResult::Success { .. } => {
                let status_icon = inline_action_icons::green_check_icon(appearance).finish();
                let chevron = render_collapse_chevron(message_id, props, app);
                let header = render_requested_action_row_for_text(
                    format!("Sent message to {recipients}: {subject}").into(),
                    appearance.ui_font_family(),
                    Some(status_icon),
                    chevron,
                    false,
                    false,
                    app,
                );

                let fields = [("To: ", recipients.as_str()), ("Subject: ", subject)];
                let body_element = Container::new(render_message_fields(&fields, message, app))
                    .with_margin_top(4.)
                    .with_margin_left(8.)
                    .finish();

                let mut column =
                    Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
                column.add_child(header);
                if let Some(body) = render_collapsible_body(message_id, body_element, false, props)
                {
                    column.add_child(body);
                }

                return Container::new(column.finish())
                    .with_horizontal_padding(8.)
                    .with_vertical_padding(8.)
                    .with_background_color(blended_colors::neutral_2(theme))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
                    .finish()
                    .with_agent_output_item_spacing(app)
                    .finish();
            }
            SendMessageToAgentResult::Error(error) => {
                let label = format!("Failed to send message to {recipients}: {error}");
                let status_icon = inline_action_icons::red_x_icon(appearance).finish();
                return render_requested_action_row_for_text(
                    label.into(),
                    appearance.ui_font_family(),
                    Some(status_icon),
                    None,
                    false,
                    false,
                    app,
                )
                .with_agent_output_item_spacing(app)
                .with_background_color(blended_colors::neutral_2(theme))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
                .finish();
            }
            SendMessageToAgentResult::Cancelled => {
                let label = format!("Send message to {recipients} cancelled.");
                let status_icon = inline_action_icons::cancelled_icon(appearance).finish();
                return render_requested_action_row_for_text(
                    label.into(),
                    appearance.ui_font_family(),
                    Some(status_icon),
                    None,
                    false,
                    false,
                    app,
                )
                .with_agent_output_item_spacing(app)
                .with_background_color(blended_colors::neutral_2(theme))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
                .finish();
            }
        };
    }

    // Non-finished (streaming/queued) state.
    let dimmed_text_color = blended_colors::text_disabled(theme, theme.surface_2());
    let should_dim_text = (props.model.status(app).is_streaming()
        && !props.model.is_first_action_in_output(action_id, app))
        || status.as_ref().is_some_and(|s| s.is_queued());

    let label_fragments = vec![
        FormattedTextFragment::plain_text("Sending message to "),
        FormattedTextFragment::bold(&recipients),
        FormattedTextFragment::plain_text(format!(": {subject}")),
    ];
    let mut header_text = render_formatted_text_element(label_fragments, app);
    if should_dim_text {
        header_text = header_text.with_color(dimmed_text_color);
    }

    let has_message = !message.is_empty();
    let chevron = if has_message {
        render_collapse_chevron(message_id, props, app)
    } else {
        None
    };

    let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    column.add_child(render_requested_action_row(
        header_text.into(),
        Some(action_icon(action_id, props.action_model, props.model, app).finish()),
        chevron,
        false,
        false,
        app,
    ));

    // Collapsible body: message text with max height
    if has_message {
        let message_color = if should_dim_text {
            dimmed_text_color
        } else {
            blended_colors::text_disabled(theme, theme.surface_2())
        };
        let message_element = render_collapsible_text_body(message, message_color, true, app);
        if let Some(body) = render_collapsible_body(
            message_id,
            message_element,
            props.model.status(app).is_streaming(),
            props,
        ) {
            column.add_child(body);
        }
    }

    column
        .finish()
        .with_agent_output_item_spacing(app)
        .with_background_color(blended_colors::neutral_2(theme))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
        .finish()
}

pub(super) fn render_start_agent(
    props: Props,
    action_id: &AIAgentActionId,
    name: &str,
    prompt: &str,
    execution_mode: &StartAgentExecutionMode,
    message_id: &MessageId,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let status = props.action_model.as_ref(app).get_action_status(action_id);

    if let Some(AIActionStatus::Finished(result)) = &status {
        let AIAgentActionResultType::StartAgent(result) = &result.result else {
            log::error!(
                "Unexpected action result type for start agent action: {:?}",
                result.result
            );
            return Empty::new().finish();
        };
        let child_conversation_card_data = child_conversation_card_data_for_result(result, app);
        let (label_fragments, status_icon) = match result {
            StartAgentResult::Success { .. } => (
                vec![
                    FormattedTextFragment::plain_text("Started agent "),
                    FormattedTextFragment::bold(name),
                    FormattedTextFragment::plain_text(start_agent_success_suffix(execution_mode)),
                ],
                inline_action_icons::green_check_icon(appearance).finish(),
            ),
            StartAgentResult::Error { error, .. } => (
                vec![
                    FormattedTextFragment::plain_text(start_agent_error_prefix(execution_mode)),
                    FormattedTextFragment::bold(name),
                    FormattedTextFragment::plain_text(format!(": {error}")),
                ],
                inline_action_icons::red_x_icon(appearance).finish(),
            ),
            StartAgentResult::Cancelled { .. } => (
                vec![
                    FormattedTextFragment::plain_text(start_agent_cancelled_prefix(execution_mode)),
                    FormattedTextFragment::bold(name),
                    FormattedTextFragment::plain_text(" cancelled."),
                ],
                inline_action_icons::cancelled_icon(appearance).finish(),
            ),
        };

        let has_prompt = !prompt.is_empty();
        let chevron = if has_prompt {
            render_collapse_chevron(message_id, props, app)
        } else {
            None
        };

        let header_text = render_formatted_text_element(label_fragments, app);
        let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        column.add_child(render_requested_action_row(
            header_text.into(),
            Some(status_icon),
            chevron,
            false,
            false,
            app,
        ));

        if has_prompt {
            let prompt_element = render_collapsible_text_body(
                prompt,
                blended_colors::text_disabled(theme, theme.surface_2()),
                true,
                app,
            );
            if let Some(body) = render_collapsible_body(message_id, prompt_element, false, props) {
                column.add_child(body);
            }
        }
        if let Some(card_data) = child_conversation_card_data {
            let navigation_card_handle = props
                .state_handles
                .orchestration_navigation_card_handles
                .get(action_id)
                .cloned()
                .unwrap_or_else(|| {
                    log::error!(
                        "Missing orchestration navigation card handle for StartAgent action {:?}",
                        action_id
                    );
                    MouseStateHandle::default()
                });
            let status_icon = card_data.status.status_icon_and_color(theme);
            column.add_child(render_conversation_navigation_card_row(
                &card_data.agent_name,
                Some(&card_data.title),
                Some(status_icon),
                card_data.conversation_id,
                navigation_card_handle,
                true,
                app,
            ));
        }

        return column
            .finish()
            .with_agent_output_item_spacing(app)
            .with_background_color(blended_colors::neutral_2(theme))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
            .finish();
    }

    // Non-finished (streaming/queued) state.
    let dimmed_text_color = blended_colors::text_disabled(theme, theme.surface_2());
    let should_dim_text = (props.model.status(app).is_streaming()
        && !props.model.is_first_action_in_output(action_id, app))
        || status.as_ref().is_some_and(|s| s.is_queued());

    let label_fragments = vec![
        FormattedTextFragment::plain_text(start_agent_in_progress_prefix(execution_mode)),
        FormattedTextFragment::bold(name),
        FormattedTextFragment::plain_text(" ..."),
    ];
    let mut header_text = render_formatted_text_element(label_fragments, app);
    if should_dim_text {
        header_text = header_text.with_color(dimmed_text_color);
    }

    let has_prompt = !prompt.is_empty();
    let chevron = if has_prompt {
        render_collapse_chevron(message_id, props, app)
    } else {
        None
    };

    let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    column.add_child(render_requested_action_row(
        header_text.into(),
        Some(action_icon(action_id, props.action_model, props.model, app).finish()),
        chevron,
        false,
        false,
        app,
    ));

    // Collapsible body: prompt text with max height
    if has_prompt {
        let prompt_color = if should_dim_text {
            dimmed_text_color
        } else {
            blended_colors::text_disabled(theme, theme.surface_2())
        };
        let prompt_element = render_collapsible_text_body(prompt, prompt_color, true, app);
        if let Some(body) = render_collapsible_body(
            message_id,
            prompt_element,
            props.model.status(app).is_streaming(),
            props,
        ) {
            column.add_child(body);
        }
    }

    column
        .finish()
        .with_agent_output_item_spacing(app)
        .with_background_color(blended_colors::neutral_2(theme))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
        .finish()
}

fn start_agent_success_suffix(execution_mode: &StartAgentExecutionMode) -> &'static str {
    match execution_mode {
        StartAgentExecutionMode::Local { .. } => " locally.",
        StartAgentExecutionMode::Remote { .. } => " remotely.",
    }
}

fn start_agent_error_prefix(execution_mode: &StartAgentExecutionMode) -> &'static str {
    match execution_mode {
        StartAgentExecutionMode::Local { .. } => "Failed to start agent ",
        StartAgentExecutionMode::Remote { .. } => "Failed to start remote agent ",
    }
}

fn start_agent_cancelled_prefix(execution_mode: &StartAgentExecutionMode) -> &'static str {
    match execution_mode {
        StartAgentExecutionMode::Local { .. } => "Start agent ",
        StartAgentExecutionMode::Remote { .. } => "Start remote agent ",
    }
}

fn start_agent_in_progress_prefix(execution_mode: &StartAgentExecutionMode) -> &'static str {
    match execution_mode {
        StartAgentExecutionMode::Local { .. } => "Starting agent ",
        StartAgentExecutionMode::Remote { .. } => "Starting remote agent ",
    }
}

/// Renders a selectable text block below an orchestration action header, using a muted color.
/// Used for both StartAgent prompts and SendMessageToAgent message bodies.
fn render_collapsible_text_body(
    text: &str,
    text_color: ColorU,
    align_with_status_row_text: bool,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let mut container = Container::new(
        Text::new(
            text.to_string(),
            appearance.ui_font_family(),
            appearance.monospace_font_size(),
        )
        .with_color(text_color)
        .with_selectable(true)
        .finish(),
    )
    .with_margin_top(4.);

    if align_with_status_row_text {
        container = container
            .with_margin_left(INLINE_ACTION_HORIZONTAL_PADDING + icon_size(app) + ICON_MARGIN)
            .with_margin_right(INLINE_ACTION_HORIZONTAL_PADDING)
            .with_margin_bottom(INLINE_ACTION_HEADER_VERTICAL_PADDING);
    }

    container.finish()
}

/// Card data for a child conversation navigation link.
#[derive(Debug, PartialEq)]
struct ChildConversationCardData {
    conversation_id: AIConversationId,
    agent_name: String,
    title: String,
    status: ConversationStatus,
}

fn child_conversation_card_data_for_result(
    result: &StartAgentResult,
    app: &AppContext,
) -> Option<ChildConversationCardData> {
    match result {
        StartAgentResult::Success { agent_id, .. } => {
            let conversation_id = conversation_id_for_agent_id(agent_id, app)?;
            let conversation =
                BlocklistAIHistoryModel::as_ref(app).conversation(&conversation_id)?;
            let agent_name = conversation.agent_name().unwrap_or("Agent").to_string();
            let status = conversation.status().clone();
            let title = available_conversation_title_for_id(conversation_id, app)?;
            Some(ChildConversationCardData {
                conversation_id,
                agent_name,
                title,
                status,
            })
        }
        StartAgentResult::Error { .. } | StartAgentResult::Cancelled { .. } => None,
    }
}

fn available_conversation_title_for_id(
    conversation_id: AIConversationId,
    app: &AppContext,
) -> Option<String> {
    let conversation = BlocklistAIHistoryModel::as_ref(app).conversation(&conversation_id)?;
    let title = conversation.title().filter(|title| !title.is_empty());
    match title {
        Some(title) if conversation.initial_query().as_deref() != Some(title.as_str()) => {
            Some(title)
        }
        _ => Some(GENERATING_TITLE_PLACEHOLDER.to_string()),
    }
}

/// Renders a chevron toggle for collapsing/expanding orchestration block bodies.
fn render_collapse_chevron(
    message_id: &MessageId,
    props: Props,
    app: &AppContext,
) -> Option<Box<dyn Element>> {
    let state = props.collapsible_block_states.get(message_id)?;
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let text_color = theme.foreground();
    let icon_sz = icon_size(app);

    let is_expanded = matches!(
        state.expansion_state,
        CollapsibleExpansionState::Expanded { .. }
    );
    let chevron_icon = if is_expanded {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    };

    let toggle_mouse_state = state.expansion_toggle_mouse_state.clone();
    let message_id_clone = message_id.clone();

    Some(
        Hoverable::new(toggle_mouse_state, move |_| {
            ConstrainedBox::new(chevron_icon.to_warpui_icon(text_color).finish())
                .with_width(icon_sz)
                .with_height(icon_sz)
                .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(AIBlockAction::ToggleCollapsibleBlockExpanded(
                message_id_clone.clone(),
            ));
        })
        .finish(),
    )
}

/// Renders the collapsible body content with max height and scroll, or None if collapsed.
fn render_collapsible_body(
    message_id: &MessageId,
    body: Box<dyn Element>,
    is_streaming: bool,
    props: Props,
) -> Option<Box<dyn Element>> {
    let Some(state) = props.collapsible_block_states.get(message_id) else {
        log::error!(
            "Missing collapsible state for orchestration message {:?}",
            message_id
        );
        return None;
    };
    render_scrollable_collapsible_content(
        message_id,
        state,
        body,
        is_streaming,
        ORCHESTRATION_COLLAPSED_MAX_HEIGHT,
    )
}

/// Builds a `FormattedTextElement` from a list of mixed plain/bold fragments.
fn render_formatted_text_element(
    fragments: Vec<FormattedTextFragment>,
    app: &AppContext,
) -> FormattedTextElement {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let formatted_text = FormattedText::new(vec![FormattedTextLine::Line(fragments)]);
    FormattedTextElement::new(
        formatted_text,
        appearance.monospace_font_size(),
        appearance.ui_font_family(),
        appearance.ui_font_family(),
        blended_colors::text_main(theme, theme.background()),
        Default::default(),
    )
    .set_selectable(true)
}

/// Client-facing model IDs offered in the Model dropdown on the
/// `OrchestrateConfigCard`. Per PRODUCT.md §configuration-block, the dropdown
/// surfaces the same client-facing IDs the rest of the product uses, with
/// `auto` as the always-available option. The list is intentionally
/// concise; if the LLM proposes a model not in this list, it is still kept
/// as the active selection (rendered in the dropdown header) and the user
/// can pick one of these as a replacement.
const MODEL_OPTIONS: &[(&str, &str)] = &[
    ("auto", "auto"),
    ("claude-4-6-opus-high", "Claude 4.6 Opus (high)"),
    ("claude-4-6-opus-max", "Claude 4.6 Opus (max)"),
    ("gpt-5-4-high", "GPT-5.4 (high)"),
    ("gpt-5-4-xhigh", "GPT-5.4 (xhigh)"),
];

/// Harness options exposed in the Harness dropdown. OpenCode is included
/// because it is selectable in Local mode; the `OrchestrateSelectExecutionMode`
/// handler auto-resets it to `oz` when the user toggles to Remote.
const HARNESS_OPTIONS: &[(&str, &str)] = &[
    ("oz", "Oz (Warp Agent)"),
    ("claude", "Claude Code"),
    ("gemini", "Gemini CLI"),
    ("opencode", "OpenCode (local-only)"),
];

/// Renders the `OrchestrateConfigCard` for an `orchestrate` tool call.
///
/// The card has three logical states keyed off the action's status:
///
/// 1. **Pre-terminal (Pending/Blocked/Queued)**: render interactive
///    Model/Harness/Execution-mode/Environment dropdowns, inline validation
///    messages, the OpenCode→Oz auto-reset notice, and the three terminal
///    buttons (Reject / Launch without orchestration / Launch).
/// 2. **In-flight (RunningAsync)**: the user has clicked Launch and the
///    parallel `CreateAgentTask` flow in `OrchestrateExecutor` is dispatching.
///    Render "Launching N agents" with a spinner per PRODUCT.md
///    §post-action; dropdowns are read-only and buttons are removed.
/// 3. **Finished**: render one of the six terminal post-action states
///    (Started N / Started M of N / Failed / Launch denied / Cancelled).
///    The card stays in conversation history per PRODUCT.md §configuration-
///    block.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_orchestrate_config_card(
    props: Props,
    action_id: &AIAgentActionId,
    summary: &str,
    model_id: &str,
    harness: &str,
    execution_mode: &OrchestrateExecutionMode,
    agents: &[OrchestrateAgentRunConfig],
    message_id: &MessageId,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let status = props.action_model.as_ref(app).get_action_status(action_id);

    // Read user-edited selections (model/harness/execution-mode/environment).
    // Falls back to the LLM-proposed action values when not present (e.g.
    // for restored conversations where `handle_updated_output` has not yet
    // run for this action). In that fallback, the dropdowns will not be
    // interactive (no handles) but the static display still works.
    let selections = props
        .state_handles
        .orchestrate_config_selections
        .get(action_id);
    let dropdown_handles = props
        .state_handles
        .orchestrate_dropdown_handles
        .get(action_id);

    // Resolve the values to render. User selections always win over the
    // LLM-proposed values once the action has been observed.
    let resolved_model_id = selections.map(|s| s.model_id.clone()).unwrap_or_else(|| {
        if model_id.is_empty() {
            "auto".to_string()
        } else {
            model_id.to_string()
        }
    });
    let resolved_harness = selections.map(|s| s.harness.clone()).unwrap_or_else(|| {
        if harness.is_empty() {
            "oz".to_string()
        } else {
            harness.to_string()
        }
    });
    let (resolved_is_remote, resolved_environment_id) = match selections {
        Some(s) => (s.is_remote, s.environment_id.clone()),
        None => match execution_mode {
            OrchestrateExecutionMode::Local => (false, String::new()),
            OrchestrateExecutionMode::Remote { environment_id } => (true, environment_id.clone()),
        },
    };
    // Determine the card phase: Pre-terminal (Pending/Blocked/Queued/
    // Preprocessing), In-flight (RunningAsync after Launch click), or
    // Finished. We compute the heading and status icon up front so the
    // header is uniform across phases.
    let is_finished = matches!(&status, Some(AIActionStatus::Finished(_)));
    let is_in_flight = matches!(&status, Some(AIActionStatus::RunningAsync));

    let (heading, status_icon) = match &status {
        Some(AIActionStatus::Finished(result)) => {
            let AIAgentActionResultType::Orchestrate(orchestrate_result) = &result.result else {
                log::error!(
                    "Unexpected action result type for orchestrate action: {:?}",
                    result.result
                );
                return Empty::new().finish();
            };
            match orchestrate_result {
                OrchestrateActionResult::Launched {
                    agents: outcomes, ..
                } => {
                    let succeeded = outcomes
                        .iter()
                        .filter(|entry| {
                            matches!(entry.outcome, OrchestrateAgentOutcome::Launched { .. })
                        })
                        .count();
                    let total = outcomes.len();
                    if succeeded == total {
                        (
                            format!("Started {total} agent(s)"),
                            inline_action_icons::green_check_icon(appearance).finish(),
                        )
                    } else {
                        // The M=0 case (every per-agent dispatch failed) renders
                        // here per spec — not under "Failed to start
                        // orchestration" — because the run-wide configuration
                        // was resolved and the Launched result still carries it,
                        // just with all `failed` outcomes.
                        (
                            format!("Started {succeeded} of {total} agent(s)"),
                            inline_action_icons::red_x_icon(appearance).finish(),
                        )
                    }
                }
                OrchestrateActionResult::LaunchDenied => (
                    "Launch denied".to_string(),
                    inline_action_icons::cancelled_icon(appearance).finish(),
                ),
                OrchestrateActionResult::Failure { error } => (
                    format!("Failed to start orchestration: {error}"),
                    inline_action_icons::red_x_icon(appearance).finish(),
                ),
                OrchestrateActionResult::Cancelled => (
                    "Cancelled".to_string(),
                    inline_action_icons::cancelled_icon(appearance).finish(),
                ),
            }
        }
        _ => (
            // Pre-terminal and in-flight share the "Launching N" heading
            // per PRODUCT.md §post-action; the in-flight state simply locks
            // the dropdowns and removes the buttons.
            format!("Launching {} agent(s)", agents.len()),
            action_icon(action_id, props.action_model, props.model, app).finish(),
        ),
    };

    let header_text = render_formatted_text_element(
        vec![
            FormattedTextFragment::bold(heading),
            FormattedTextFragment::plain_text(if summary.is_empty() {
                String::new()
            } else {
                format!(" — {summary}")
            }),
        ],
        app,
    );
    let chevron = render_collapse_chevron(message_id, props, app);

    let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    column.add_child(render_requested_action_row(
        header_text.into(),
        Some(status_icon),
        chevron,
        false,
        false,
        app,
    ));

    let mut body_column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    let dimmed_text_color = blended_colors::text_disabled(theme, theme.surface_2());
    let value_color: ColorU = theme.main_text_color(theme.background()).into();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.monospace_font_size();

    // Pre-terminal state: render interactive dropdowns. In-flight and
    // Finished states render the dropdowns as read-only static rows.
    let is_interactive = !is_finished && !is_in_flight && dropdown_handles.is_some();

    if is_interactive {
        let handles = dropdown_handles.expect("checked above");
        let open_dropdown = selections.and_then(|s| s.open_dropdown);

        body_column.add_child(render_dropdown_row(
            "Model",
            &display_label_for_option(&resolved_model_id, MODEL_OPTIONS),
            handles.model.clone(),
            open_dropdown == Some(OrchestrateConfigDropdown::Model),
            AIBlockAction::OrchestrateToggleDropdown {
                action_id: action_id.clone(),
                dropdown: OrchestrateConfigDropdown::Model,
            },
            MODEL_OPTIONS
                .iter()
                .map(|(value, label)| {
                    let action = AIBlockAction::OrchestrateSelectModel {
                        action_id: action_id.clone(),
                        model_id: (*value).to_string(),
                    };
                    (label.to_string(), action)
                })
                .collect(),
            app,
        ));

        body_column.add_child(render_dropdown_row(
            "Harness",
            &display_label_for_option(&resolved_harness, HARNESS_OPTIONS),
            handles.harness.clone(),
            open_dropdown == Some(OrchestrateConfigDropdown::Harness),
            AIBlockAction::OrchestrateToggleDropdown {
                action_id: action_id.clone(),
                dropdown: OrchestrateConfigDropdown::Harness,
            },
            HARNESS_OPTIONS
                .iter()
                .map(|(value, label)| {
                    let action = AIBlockAction::OrchestrateSelectHarness {
                        action_id: action_id.clone(),
                        harness: (*value).to_string(),
                    };
                    (label.to_string(), action)
                })
                .collect(),
            app,
        ));

        body_column.add_child(render_dropdown_row(
            "Execution mode",
            if resolved_is_remote {
                "Remote"
            } else {
                "Local"
            },
            handles.execution_mode.clone(),
            open_dropdown == Some(OrchestrateConfigDropdown::ExecutionMode),
            AIBlockAction::OrchestrateToggleDropdown {
                action_id: action_id.clone(),
                dropdown: OrchestrateConfigDropdown::ExecutionMode,
            },
            vec![
                (
                    "Local".to_string(),
                    AIBlockAction::OrchestrateSelectExecutionMode {
                        action_id: action_id.clone(),
                        is_remote: false,
                    },
                ),
                (
                    "Remote".to_string(),
                    AIBlockAction::OrchestrateSelectExecutionMode {
                        action_id: action_id.clone(),
                        is_remote: true,
                    },
                ),
            ],
            app,
        ));

        if resolved_is_remote {
            // PRODUCT.md §configuration-block: the Environment dropdown is
            // visible only when execution mode is Remote.
            let environments = CloudAmbientAgentEnvironment::get_all(app);
            let environment_options: Vec<(String, AIBlockAction)> = environments
                .iter()
                .map(|env| {
                    let env_id = env.id.to_string();
                    let label = env.model().string_model.display_name();
                    let action = AIBlockAction::OrchestrateSelectEnvironment {
                        action_id: action_id.clone(),
                        environment_id: env_id,
                    };
                    (label, action)
                })
                .collect();
            let env_label = if resolved_environment_id.is_empty() {
                "Choose environment…".to_string()
            } else {
                environments
                    .iter()
                    .find(|e| e.id.to_string() == resolved_environment_id)
                    .map(|e| e.model().string_model.display_name())
                    .unwrap_or(resolved_environment_id.clone())
            };
            body_column.add_child(render_dropdown_row(
                "Environment",
                &env_label,
                handles.environment.clone(),
                open_dropdown == Some(OrchestrateConfigDropdown::Environment),
                AIBlockAction::OrchestrateToggleDropdown {
                    action_id: action_id.clone(),
                    dropdown: OrchestrateConfigDropdown::Environment,
                },
                environment_options,
                app,
            ));
        }

        // Render the OpenCode→Oz auto-reset notice if it's currently active.
        if selections
            .map(|s| s.did_auto_reset_opencode)
            .unwrap_or(false)
        {
            body_column.add_child(render_opencode_reset_notice(
                action_id,
                handles.dismiss_opencode_notice.clone(),
                app,
            ));
        }
    } else {
        // Read-only summary for in-flight and Finished states.
        let mode_label = if resolved_is_remote {
            if resolved_environment_id.is_empty() {
                "Remote (no environment)".to_string()
            } else {
                format!("Remote ({resolved_environment_id})")
            }
        } else {
            "Local".to_string()
        };
        let config_fields = [
            ("Model: ", resolved_model_id.as_str()),
            ("Harness: ", resolved_harness.as_str()),
            ("Execution mode: ", mode_label.as_str()),
        ];
        for (label, value) in config_fields {
            let line = Flex::row()
                .with_child(
                    Text::new(label.to_string(), font_family, font_size)
                        .with_color(dimmed_text_color)
                        .finish(),
                )
                .with_child(
                    Text::new(value.to_string(), font_family, font_size)
                        .with_color(value_color)
                        .finish(),
                )
                .finish();
            body_column.add_child(line);
        }
    }

    if !agents.is_empty() {
        body_column.add_child(
            Container::new(
                Text::new("Agents:".to_string(), font_family, font_size)
                    .with_color(dimmed_text_color)
                    .finish(),
            )
            .with_margin_top(4.)
            .finish(),
        );
        for agent in agents {
            body_column.add_child(
                Container::new(
                    Text::new(format!("  • {}", agent.name), font_family, font_size)
                        .with_color(value_color)
                        .finish(),
                )
                .finish(),
            );
        }
    }

    if is_interactive {
        // Inline validation messages (TECH.md §6):
        //   * "Choose an environment before launching" when Remote and the
        //     environment_id is empty.
        //   * "OpenCode is not supported in remote mode" when the LLM
        //     directly proposes the OpenCode + Remote combination. Note that
        //     toggling Local→Remote auto-resets OpenCode→Oz, so this only
        //     fires when the action arrived in the OpenCode+Remote state
        //     and the user hasn't picked a different harness yet.
        let validation_errors = compute_validation_errors(
            &resolved_harness,
            resolved_is_remote,
            &resolved_environment_id,
        );
        if !validation_errors.is_empty() {
            let error_color = blended_colors::text_main(theme, theme.background());
            for error in &validation_errors {
                body_column.add_child(
                    Container::new(
                        Text::new(error.to_string(), font_family, font_size)
                            .with_color(error_color)
                            .finish(),
                    )
                    .with_margin_top(6.)
                    .finish(),
                );
            }
        }

        if let Some(handles) = props
            .state_handles
            .orchestrate_button_handles
            .get(action_id)
        {
            let launch_disabled = !validation_errors.is_empty();
            body_column.add_child(render_orchestrate_buttons(
                action_id,
                handles,
                launch_disabled,
                app,
            ));
        }
    } else if is_in_flight {
        // PRODUCT.md §post-action: in-flight state shows a simple status
        // line indicating the parallel CreateAgentTask flow is dispatching.
        body_column.add_child(
            Container::new(
                Text::new(
                    format!("Launching {} agent(s)…", agents.len()),
                    font_family,
                    font_size,
                )
                .with_color(dimmed_text_color)
                .finish(),
            )
            .with_margin_top(8.)
            .finish(),
        );
    }

    let body = Container::new(body_column.finish())
        .with_margin_top(4.)
        .with_margin_left(INLINE_ACTION_HORIZONTAL_PADDING + icon_size(app) + ICON_MARGIN)
        .with_margin_right(INLINE_ACTION_HORIZONTAL_PADDING)
        .with_margin_bottom(INLINE_ACTION_HEADER_VERTICAL_PADDING)
        .finish();

    if let Some(rendered_body) = render_collapsible_body(
        message_id,
        body,
        props.model.status(app).is_streaming(),
        props,
    ) {
        column.add_child(rendered_body);
    }

    // Suppress an unused-variable warning when the only remaining use of
    // `model_id` / `harness` / `execution_mode` is the read-only static
    // summary path. They remain part of the function signature because
    // restored conversations may not have selections yet.
    let _ = (model_id, harness, execution_mode);

    column
        .finish()
        .with_agent_output_item_spacing(app)
        .with_background_color(blended_colors::neutral_2(theme))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
        .finish()
}

/// Returns the human-readable label for an option whose canonical value is
/// `value`, falling back to the value itself when no matching option exists
/// (e.g. when the LLM proposes a custom model_id not in the static list).
fn display_label_for_option(value: &str, options: &[(&str, &str)]) -> String {
    options
        .iter()
        .find_map(|(v, label)| (*v == value).then(|| label.to_string()))
        .unwrap_or_else(|| value.to_string())
}

/// Computes the inline validation errors for the current resolved
/// configuration. Empty vec means Launch is enabled.
fn compute_validation_errors(
    harness: &str,
    is_remote: bool,
    environment_id: &str,
) -> Vec<&'static str> {
    let mut errors = Vec::new();
    if is_remote && environment_id.is_empty() {
        errors.push("Choose an environment before launching.");
    }
    if is_remote && harness == "opencode" {
        errors.push("OpenCode is not supported in remote mode.");
    }
    errors
}

/// Renders one interactive dropdown row on the `OrchestrateConfigCard`. The
/// row is a label + clickable header showing the current selection; when
/// `is_open` is true, the option list is rendered inline below the header.
fn render_dropdown_row(
    label: &str,
    selected_label: &str,
    header_mouse_state: MouseStateHandle,
    is_open: bool,
    toggle_action: AIBlockAction,
    options: Vec<(String, AIBlockAction)>,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.monospace_font_size();
    let label_color = blended_colors::text_disabled(theme, theme.surface_2());
    let value_color: ColorU = theme.main_text_color(theme.background()).into();
    let header_bg: ColorU = blended_colors::neutral_3(theme);
    let option_bg: ColorU = blended_colors::neutral_2(theme);
    let option_hover_bg: ColorU = blended_colors::neutral_4(theme);

    let label_owned = label.to_string();
    let selected_owned = selected_label.to_string();
    let chevron = if is_open {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    };

    let header = Hoverable::new(header_mouse_state, move |_| {
        let row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Text::new(selected_owned.clone(), font_family, font_size)
                    .with_color(value_color)
                    .finish(),
            )
            .with_child(
                Container::new(
                    ConstrainedBox::new(chevron.to_warpui_icon(Fill::Solid(value_color)).finish())
                        .with_width(font_size)
                        .with_height(font_size)
                        .finish(),
                )
                .with_margin_left(6.)
                .finish(),
            )
            .finish();
        Container::new(row)
            .with_horizontal_padding(8.)
            .with_vertical_padding(4.)
            .with_background_color(header_bg)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
            .finish()
    })
    .with_cursor(Cursor::PointingHand)
    .on_click(move |ctx, _, _| {
        ctx.dispatch_typed_action(toggle_action.clone());
    })
    .finish();

    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(
            ConstrainedBox::new(
                Text::new(format!("{label_owned}: "), font_family, font_size)
                    .with_color(label_color)
                    .finish(),
            )
            .with_width(140.)
            .finish(),
        )
        .with_child(header);

    let header_row_element = row.finish();

    if !is_open {
        return header_row_element;
    }

    // Render the option list inline below the header when the dropdown is
    // expanded. Each option is a clickable row that dispatches the matching
    // selection action.
    let mut option_column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    for (option_label, option_action) in options {
        let option_label_owned = option_label.clone();
        let option_handle = MouseStateHandle::default();
        let element = Hoverable::new(option_handle, move |state| {
            let bg = if state.is_hovered() {
                option_hover_bg
            } else {
                option_bg
            };
            Container::new(
                Text::new(option_label_owned.clone(), font_family, font_size)
                    .with_color(value_color)
                    .finish(),
            )
            .with_horizontal_padding(8.)
            .with_vertical_padding(4.)
            .with_background_color(bg)
            .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(option_action.clone());
        })
        .finish();
        option_column.add_child(element);
    }

    let mut wrapper = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    wrapper.add_child(header_row_element);
    wrapper.add_child(
        Container::new(option_column.finish())
            .with_margin_top(2.)
            .with_margin_left(140.)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
            .finish(),
    );
    wrapper.finish()
}

/// Renders the inline notice that the Harness was auto-reset from OpenCode
/// to Oz when the user toggled the execution mode from Local to Remote
/// (TECH.md Risks mitigation: "client resets harness to Oz when toggling to
/// Remote with a notice"). The user dismisses it by clicking the inline X.
fn render_opencode_reset_notice(
    action_id: &AIAgentActionId,
    dismiss_handle: MouseStateHandle,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.monospace_font_size();
    let text_color: ColorU = theme.main_text_color(theme.background()).into();
    let bg: ColorU = blended_colors::neutral_3(theme);
    let icon_sz = icon_size(app);

    let action_id_clone = action_id.clone();
    let dismiss = Hoverable::new(dismiss_handle, move |_| {
        ConstrainedBox::new(Icon::X.to_warpui_icon(Fill::Solid(text_color)).finish())
            .with_width(icon_sz)
            .with_height(icon_sz)
            .finish()
    })
    .with_cursor(Cursor::PointingHand)
    .on_click(move |ctx, _, _| {
        ctx.dispatch_typed_action(AIBlockAction::OrchestrateDismissOpenCodeNotice {
            action_id: action_id_clone.clone(),
        });
    })
    .finish();

    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(
            Text::new(
                "Harness reset to Oz: OpenCode is not supported in remote mode.".to_string(),
                font_family,
                font_size,
            )
            .with_color(text_color)
            .finish(),
        )
        .with_child(Container::new(dismiss).with_margin_left(8.).finish())
        .finish();

    Container::new(row)
        .with_margin_top(6.)
        .with_horizontal_padding(8.)
        .with_vertical_padding(4.)
        .with_background_color(bg)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .finish()
}

/// Renders the row of three terminal buttons on an `OrchestrateConfigCard`:
/// Reject (secondary, leftmost), Launch without orchestration (secondary), and
/// Launch (primary). The Launch button is rendered in a disabled visual
/// treatment when `launch_disabled` is true (e.g. validation errors are
/// active).
///
/// Reject and Launch-without-orchestration dispatch
/// `AIBlockAction::OrchestrateActionDecision` directly. Launch dispatches
/// `AIBlockAction::OrchestrateLaunchClicked` so the click handler can read
/// the user's resolved selections from `state_handles.orchestrate_config_
/// selections` (the closure cannot read state at render time).
fn render_orchestrate_buttons(
    action_id: &AIAgentActionId,
    handles: &OrchestrateButtonHandles,
    launch_disabled: bool,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.monospace_font_size();
    let primary_text_color: ColorU = theme.main_text_color(theme.accent()).into_solid();
    let secondary_text_color: ColorU = theme.main_text_color(theme.surface_2()).into_solid();
    let primary_bg: ColorU = theme.accent().into_solid();
    let secondary_bg: ColorU = blended_colors::neutral_3(theme);
    let disabled_bg: ColorU = blended_colors::neutral_2(theme);
    let disabled_text_color = blended_colors::text_disabled(theme, theme.surface_2());

    let make_button = |label: &str,
                       mouse_state: MouseStateHandle,
                       background: ColorU,
                       text_color: ColorU,
                       on_click_action: Option<AIBlockAction>|
     -> Box<dyn Element> {
        let label_owned = label.to_string();
        let cursor = if on_click_action.is_some() {
            Cursor::PointingHand
        } else {
            Cursor::Arrow
        };
        let mut hoverable = Hoverable::new(mouse_state, move |_| {
            Container::new(
                Text::new(label_owned.clone(), font_family, font_size)
                    .with_color(text_color)
                    .finish(),
            )
            .with_horizontal_padding(10.)
            .with_vertical_padding(6.)
            .with_background_color(background)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
            .finish()
        })
        .with_cursor(cursor);
        if let Some(action) = on_click_action {
            hoverable = hoverable.on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
            });
        }
        hoverable.finish()
    };

    let reject_button = make_button(
        "Reject",
        handles.reject.clone(),
        secondary_bg,
        secondary_text_color,
        Some(AIBlockAction::OrchestrateActionDecision {
            action_id: action_id.clone(),
            decision: OrchestrateDecision::Reject,
        }),
    );
    let launch_without_button = make_button(
        "Launch without orchestration",
        handles.launch_without_orchestration.clone(),
        secondary_bg,
        secondary_text_color,
        Some(AIBlockAction::OrchestrateActionDecision {
            action_id: action_id.clone(),
            decision: OrchestrateDecision::LaunchWithoutOrchestration,
        }),
    );
    let (launch_bg, launch_text, launch_action) = if launch_disabled {
        (disabled_bg, disabled_text_color, None)
    } else {
        (
            primary_bg,
            primary_text_color,
            Some(AIBlockAction::OrchestrateLaunchClicked {
                action_id: action_id.clone(),
            }),
        )
    };
    let launch_button = make_button(
        "Launch",
        handles.launch.clone(),
        launch_bg,
        launch_text,
        launch_action,
    );

    Container::new(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(Container::new(reject_button).with_margin_right(8.).finish())
            .with_child(
                Container::new(launch_without_button)
                    .with_margin_right(8.)
                    .finish(),
            )
            .with_child(launch_button)
            .finish(),
    )
    .with_margin_top(8.)
    .finish()
}

fn render_conversation_navigation_card_row(
    title: &str,
    subtitle: Option<&str>,
    icon: Option<(Icon, pathfinder_color::ColorU)>,
    conversation_id: AIConversationId,
    mouse_state: MouseStateHandle,
    align_with_status_row_text: bool,
    app: &AppContext,
) -> Box<dyn Element> {
    let card = conversation_navigation_card_with_icon(
        icon,
        title.to_string(),
        subtitle.map(|s| s.to_string()),
        move |ctx, _, _| {
            ctx.dispatch_typed_action(TerminalAction::RevealChildAgent { conversation_id });
        },
        mouse_state,
        true,
        None,
        app,
    );

    let mut container = Container::new(card).with_margin_top(6.);

    if align_with_status_row_text {
        container = container
            .with_margin_left(INLINE_ACTION_HORIZONTAL_PADDING + icon_size(app) + ICON_MARGIN)
            .with_margin_right(INLINE_ACTION_HORIZONTAL_PADDING)
            .with_margin_bottom(INLINE_ACTION_HEADER_VERTICAL_PADDING);
    }

    container.finish()
}

#[cfg(test)]
#[path = "orchestration_tests.rs"]
mod tests;
