use markdown_parser::{FormattedText, FormattedTextFragment, FormattedTextLine};
use warp_core::ui::appearance::Appearance;
use warpui::{
    elements::{
        ConstrainedBox, Container, CrossAxisAlignment, Flex, FormattedTextElement,
        HighlightedHyperlink, HyperlinkLens, MainAxisAlignment, MainAxisSize, ParentElement,
    },
    AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext,
};

use crate::{
    ai::{blocklist::error_color, AIRequestUsageModel},
    auth::AuthStateProvider,
    network::NetworkStatus,
    settings_view::SettingsSection,
    ui_components::icons::Icon,
    workspace::WorkspaceAction,
    workspaces::user_workspaces::UserWorkspaces,
};
use ai::api_keys::ApiKeyManager;

const ANONYMOUS_USER_REQUEST_LIMIT_SOFT_GATE_PERCENTAGE: f32 = 0.5;

const NO_CONNECTION_PRIMARY_TEXT: &str = "No internet connection";
const ANONYMOUS_USER_REQUEST_LIMIT_SOFT_GATE_PRIMARY_TEXT: &str = "";
const ANONYMOUS_USER_REQUEST_LIMIT_HARD_GATE_PRIMARY_TEXT: &str = "At Limit -";
const OUT_OF_REQUESTS_PRIMARY_TEXT: &str = "Out of credits";

const ANONYMOUS_USER_REQUEST_LIMIT_ACTION_TEXT: &str = "Configure local AI provider";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptAlertAction {}

/// The alert state of the chip that appears to the right of certain parts of the prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptAlertState {
    /// The user is offline (no connection).
    NoConnection,
    /// An anonymous user has reached a certain percentage of requests used.
    /// This doesn't use a primary text to avoid being too in-your-face.
    AnonymousUserRequestLimitSoftGate,
    /// An anonymous user has reached the request limit.
    AnonymousUserRequestLimitHardGate,
    /// The user has reached the request limit.
    RequestLimitReached,
    /// No alert should be displayed.
    NoAlert,
}

pub struct PromptAlertView {
    state: PromptAlertState,
    action_hyperlink: HighlightedHyperlink,
}

impl PromptAlertView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let request_usage_model = AIRequestUsageModel::handle(ctx);
        let user_workspaces = UserWorkspaces::handle(ctx);
        let network_status = NetworkStatus::handle(ctx);
        let api_key_manager = ApiKeyManager::handle(ctx);

        ctx.subscribe_to_model(&request_usage_model, |me, _, _, ctx| {
            me.state = Self::determine_state(ctx);
            ctx.notify();
        });

        ctx.subscribe_to_model(&user_workspaces, |me, _, _, ctx| {
            me.state = Self::determine_state(ctx);
            ctx.notify();
        });

        ctx.subscribe_to_model(&network_status, |me, _, _, ctx| {
            me.state = Self::determine_state(ctx);
            ctx.notify();
        });

        ctx.subscribe_to_model(&api_key_manager, |me, _, _, ctx| {
            me.state = Self::determine_state(ctx);
            ctx.notify();
        });

        Self {
            state: Self::determine_state(ctx),
            action_hyperlink: Default::default(),
        }
    }

    pub fn determine_state(app: &AppContext) -> PromptAlertState {
        if UserWorkspaces::as_ref(app).is_byo_api_key_enabled() {
            return PromptAlertState::NoAlert;
        }

        // OpenWarp: BYOP/本地 provider 自行处理连接状态,包括 Ollama 这类 localhost
        // provider。全局离线状态只阻止内置云端用量。
        if !NetworkStatus::as_ref(app).is_online() {
            return PromptAlertState::NoConnection;
        }

        let request_usage_model = AIRequestUsageModel::as_ref(app);
        // OpenWarp(Phase 3c A1):`has_requests_remaining` 本地化后恒为 true,
        // 原有的 if/else 二分只有 SoftGate 分支可达达,直接则使用 true 分支。
        let auth_state = AuthStateProvider::as_ref(app).get();

        // Next, if the user is anonymous, we check if they have reached a certain percentage of requests used.
        if auth_state
            .is_anonymous_user_feature_gated()
            .unwrap_or_default()
        {
            let percentage_used = request_usage_model.request_percentage_used();

            if percentage_used >= ANONYMOUS_USER_REQUEST_LIMIT_SOFT_GATE_PERCENTAGE {
                return PromptAlertState::AnonymousUserRequestLimitSoftGate;
            }
        }

        // If there is ever any ai remaining, no alert
        if request_usage_model.has_any_ai_remaining(app) {
            return PromptAlertState::NoAlert;
        }

        // 本地版没有云端 overages / 计费升级路径。
        PromptAlertState::RequestLimitReached
    }

    pub fn is_no_alert(&self) -> bool {
        matches!(self.state, PromptAlertState::NoAlert)
    }

    pub fn state(&self) -> &PromptAlertState {
        &self.state
    }

    pub fn does_alert_block_ai_requests(app: &AppContext) -> bool {
        does_alert_block_ai_requests(&Self::determine_state(app))
    }

    fn primary_text(
        &self,
        state: &PromptAlertState,
        text_fragments: &mut Vec<FormattedTextFragment>,
    ) {
        // Add leading space to separate text from icon.
        //
        // Use this instead of hardcoded margin so it scales with font size and is consistent
        // with the space between this primary fragment and the option hyperlink fragment.
        text_fragments.push(FormattedTextFragment::plain_text("  "));
        match state {
            PromptAlertState::NoConnection => {
                text_fragments.push(FormattedTextFragment::plain_text(
                    NO_CONNECTION_PRIMARY_TEXT,
                ));
            }
            PromptAlertState::AnonymousUserRequestLimitSoftGate => {
                text_fragments.push(FormattedTextFragment::plain_text(
                    ANONYMOUS_USER_REQUEST_LIMIT_SOFT_GATE_PRIMARY_TEXT,
                ));
            }
            PromptAlertState::AnonymousUserRequestLimitHardGate => {
                text_fragments.push(FormattedTextFragment::plain_text(
                    ANONYMOUS_USER_REQUEST_LIMIT_HARD_GATE_PRIMARY_TEXT,
                ));
            }
            PromptAlertState::RequestLimitReached => {
                text_fragments.push(FormattedTextFragment::plain_text(
                    OUT_OF_REQUESTS_PRIMARY_TEXT,
                ));
            }
            PromptAlertState::NoAlert => {}
        }
    }

    fn action_hyperlink(
        &self,
        state: &PromptAlertState,
        text_fragments: &mut Vec<FormattedTextFragment>,
        app: &AppContext,
    ) {
        match state {
            PromptAlertState::NoConnection => {}
            PromptAlertState::AnonymousUserRequestLimitSoftGate
            | PromptAlertState::AnonymousUserRequestLimitHardGate => {
                text_fragments.push(FormattedTextFragment::plain_text("  "));
                text_fragments.push(FormattedTextFragment::hyperlink_action(
                    ANONYMOUS_USER_REQUEST_LIMIT_ACTION_TEXT,
                    WorkspaceAction::ShowSettingsPageWithSearch {
                        search_query: "api".to_string(),
                        section: Some(SettingsSection::WarpAgent),
                    },
                ));
            }
            PromptAlertState::RequestLimitReached => {
                text_fragments.push(FormattedTextFragment::plain_text("  "));
                if UserWorkspaces::as_ref(app).is_byo_api_key_enabled() {
                    text_fragments.push(FormattedTextFragment::hyperlink_action(
                        "use your own API keys",
                        WorkspaceAction::ShowSettingsPageWithSearch {
                            search_query: "api".to_string(),
                            section: Some(SettingsSection::WarpAgent),
                        },
                    ));
                }
            }
            PromptAlertState::NoAlert => {}
        }
    }
}

fn does_alert_block_ai_requests(state: &PromptAlertState) -> bool {
    match state {
        PromptAlertState::AnonymousUserRequestLimitSoftGate | PromptAlertState::NoAlert => false,
        PromptAlertState::NoConnection
        | PromptAlertState::AnonymousUserRequestLimitHardGate
        | PromptAlertState::RequestLimitReached => true,
    }
}

impl Entity for PromptAlertView {
    type Event = ();
}

impl View for PromptAlertView {
    fn ui_name() -> &'static str {
        "PromptAlertView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let state = Self::determine_state(app);
        let mut text_fragments = vec![];

        self.primary_text(&state, &mut text_fragments);

        self.action_hyperlink(&state, &mut text_fragments, app);

        let formatted_text_element = FormattedTextElement::new(
            FormattedText::new([FormattedTextLine::Line(text_fragments)]),
            appearance.ui_font_size(),
            appearance.ui_font_family(),
            appearance.ui_font_family(),
            error_color(appearance.theme()),
            self.action_hyperlink.clone(),
        )
        .with_line_height_ratio(1.)
        .with_hyperlink_font_color(appearance.theme().ansi_fg_blue())
        .with_no_text_wrapping()
        .register_default_click_handlers_with_action_support(|hyperlink_lens, event, ctx| {
            match hyperlink_lens {
                HyperlinkLens::Url(url) => {
                    ctx.open_url(url);
                }
                HyperlinkLens::Action(action_ref) => {
                    if let Some(action) = action_ref.as_any().downcast_ref::<WorkspaceAction>() {
                        event.dispatch_typed_action(action.clone());
                    }
                }
            }
        })
        .finish();

        let icon_size = appearance.ui_font_size();

        let mut chip_row = Flex::row()
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::End);
        if does_alert_block_ai_requests(&self.state) {
            chip_row.add_child(
                ConstrainedBox::new(
                    Icon::AlertTriangle
                        .to_warpui_icon(error_color(appearance.theme()).into())
                        .finish(),
                )
                .with_width(icon_size)
                .with_height(icon_size)
                .finish(),
            )
        }

        chip_row.add_child(formatted_text_element);

        Container::new(chip_row.finish())
            .with_margin_right(16.)
            .finish()
    }
}

impl TypedActionView for PromptAlertView {
    type Action = PromptAlertAction;

    fn handle_action(&mut self, action: &Self::Action, _ctx: &mut ViewContext<Self>) {
        match *action {}
    }
}
