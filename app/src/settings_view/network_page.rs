//! "Network" 设置页:全局 HTTP 代理配置(见 Issue #72)。
//!
//! 提供三档代理模式(System / Custom / Off)、Custom URL / 用户名 / 密码 / no_proxy
//! 列表,以及一个"测试连接"按钮。用户改完任一字段后回车(或点 enter)即保存,
//! 设置变更后 `app/src/settings/init.rs` 的订阅会立即把新值推到
//! `http_client` 与 `websocket` 两处的全局 slot。
//!
//! 测试连接按钮:用当前(已保存的)代理配置新建一个 `http_client::Client`,
//! 发一次 GET 到固定 URL(默认 `https://www.google.com/generate_204`,
//! 该地址命中 HTTP 204 且对代理友好),根据返回结果显示成功 / 失败文案。
//! 测试发起在 `ctx.spawn` 中,结果通过专用 action 回到 view。

use std::sync::Arc;

use settings::Setting;
use warpui::{
    elements::{
        Container, CrossAxisAlignment, Element, Flex, MainAxisAlignment, MouseStateHandle,
        ParentElement, Text,
    },
    fonts::{Properties, Weight},
    ui_components::{
        button::ButtonVariant,
        components::{Coords, UiComponent, UiComponentStyles},
    },
    AppContext, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use super::settings_page::{
    render_body_item, render_page_title, render_sub_header_with_description, AdditionalInfo,
    LocalOnlyIconState, MatchData, PageType, SettingsPageEvent, SettingsPageMeta,
    SettingsPageViewHandle, SettingsWidget, ToggleState, HEADER_FONT_SIZE,
};
use super::SettingsSection;
use crate::appearance::Appearance;
use crate::report_if_error;
use crate::settings::network::{NetworkSettings, ProxyMode};
use crate::settings::network_secrets::ProxyCredentials;
use crate::view_components::dropdown::{Dropdown, DropdownItem};
use crate::view_components::{SubmittableTextInput, SubmittableTextInputEvent};

/// 用于测试连接的目标 URL。Google 的 `generate_204` 是无 body / 200 状态码的轻量探针;
/// 通过代理时如果失败,我们能确认是代理配置 / 网络 / DNS 等链路问题。
const TEST_CONNECTION_URL: &str = "https://www.google.com/generate_204";

/// 单次测试连接的最长等待时间。
const TEST_CONNECTION_TIMEOUT_SECS: u64 = 8;

#[derive(Debug, Clone)]
pub enum NetworkPageAction {
    /// dropdown 选择了某个 ProxyMode 项,持久化到 settings。
    SetProxyMode(ProxyMode),
    /// SubmittableTextInput 提交了新 proxy_url。
    SetProxyUrl(String),
    /// 提交了新 proxy_username。
    SetProxyUsername(String),
    /// 提交了新 代理密码(走 OS 密钥库)。
    SetProxyPassword(String),
    /// 提交了新 no_proxy 列表。
    SetProxyNoProxy(String),
    /// 点击"测试连接"按钮:发起一次 GET 请求。
    TestConnection,
    /// 测试连接完成,把结果显示到 UI。`Ok(status_code)` / `Err(error_string)`。
    TestConnectionResult(Result<u16, String>),
}

/// "测试连接"按钮的当前状态。
#[derive(Debug, Clone, Default)]
enum TestState {
    #[default]
    Idle,
    Running,
    Success {
        status: u16,
    },
    Failed {
        message: String,
    },
}

pub struct NetworkPageView {
    page: PageType<Self>,
    /// 代理模式下拉。
    mode_dropdown: ViewHandle<Dropdown<NetworkPageAction>>,
    /// 代理 URL 输入(回车提交)。
    url_input: ViewHandle<SubmittableTextInput>,
    /// 用户名输入。
    username_input: ViewHandle<SubmittableTextInput>,
    /// 密码输入。因为 `SubmittableTextInput` 未提供 mask 选项,输入时仍会明文显示;
    /// 提交后走 OS 密钥库保存,不写入 settings.toml。
    password_input: ViewHandle<SubmittableTextInput>,
    /// no_proxy 列表输入。
    no_proxy_input: ViewHandle<SubmittableTextInput>,
    /// 测试连接按钮的 mouse state(WarpUI 习惯单独保留)。
    test_button_state: MouseStateHandle,
    /// 测试连接当前状态。
    test_state: TestState,
}

impl NetworkPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let mode_dropdown = ctx.add_typed_action_view(Dropdown::<NetworkPageAction>::new);

        // 装配 ProxyMode 三个选项。
        mode_dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_items(
                vec![
                    DropdownItem::new(
                        crate::t!("settings-network-mode-system"),
                        NetworkPageAction::SetProxyMode(ProxyMode::System),
                    ),
                    DropdownItem::new(
                        crate::t!("settings-network-mode-custom"),
                        NetworkPageAction::SetProxyMode(ProxyMode::Custom),
                    ),
                    DropdownItem::new(
                        crate::t!("settings-network-mode-off"),
                        NetworkPageAction::SetProxyMode(ProxyMode::Off),
                    ),
                ],
                ctx,
            );
        });

        // 四个文本输入(每个独立 SubmittableTextInput;submit 时由 view 持久化)。
        let url_input = ctx.add_typed_action_view(|ctx| {
            let mut input = SubmittableTextInput::new(ctx);
            input.set_placeholder_text(crate::t!("settings-network-url-placeholder"), ctx);
            input
        });
        ctx.subscribe_to_view(&url_input, |me: &mut Self, _, event, ctx| {
            if let SubmittableTextInputEvent::Submit(text) = event {
                me.handle_action(&NetworkPageAction::SetProxyUrl(text.clone()), ctx);
            }
        });

        let username_input = ctx.add_typed_action_view(|ctx| {
            let mut input = SubmittableTextInput::new(ctx);
            input.set_placeholder_text(crate::t!("settings-network-username-placeholder"), ctx);
            input
        });
        ctx.subscribe_to_view(&username_input, |me: &mut Self, _, event, ctx| {
            if let SubmittableTextInputEvent::Submit(text) = event {
                me.handle_action(&NetworkPageAction::SetProxyUsername(text.clone()), ctx);
            }
        });

        let password_input = ctx.add_typed_action_view(|ctx| {
            let mut input = SubmittableTextInput::new(ctx);
            input.set_placeholder_text(crate::t!("settings-network-password-placeholder"), ctx);
            input
        });
        ctx.subscribe_to_view(&password_input, |me: &mut Self, _, event, ctx| {
            if let SubmittableTextInputEvent::Submit(text) = event {
                me.handle_action(&NetworkPageAction::SetProxyPassword(text.clone()), ctx);
            }
        });

        let no_proxy_input = ctx.add_typed_action_view(|ctx| {
            let mut input = SubmittableTextInput::new(ctx);
            input.set_placeholder_text(crate::t!("settings-network-no-proxy-placeholder"), ctx);
            input
        });
        ctx.subscribe_to_view(&no_proxy_input, |me: &mut Self, _, event, ctx| {
            if let SubmittableTextInputEvent::Submit(text) = event {
                me.handle_action(&NetworkPageAction::SetProxyNoProxy(text.clone()), ctx);
            }
        });

        Self {
            page: PageType::new_monolith(NetworkPageWidget::default(), None, false),
            mode_dropdown,
            url_input,
            username_input,
            password_input,
            no_proxy_input,
            test_button_state: MouseStateHandle::default(),
            test_state: TestState::Idle,
        }
    }
}

impl Entity for NetworkPageView {
    type Event = SettingsPageEvent;
}

impl TypedActionView for NetworkPageView {
    type Action = NetworkPageAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            NetworkPageAction::SetProxyMode(mode) => {
                let mode = *mode;
                NetworkSettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.proxy_mode.set_value(mode, ctx));
                });
                ctx.notify();
            }
            NetworkPageAction::SetProxyUrl(url) => {
                let url = url.clone();
                NetworkSettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.proxy_url.set_value(url, ctx));
                });
                ctx.notify();
            }
            NetworkPageAction::SetProxyUsername(username) => {
                let username = username.clone();
                NetworkSettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.proxy_username.set_value(username, ctx));
                });
                ctx.notify();
            }
            NetworkPageAction::SetProxyPassword(password) => {
                let password = password.clone();
                ProxyCredentials::handle(ctx).update(ctx, |creds, ctx| {
                    creds.set_password(password, ctx);
                });
                ctx.notify();
            }
            NetworkPageAction::SetProxyNoProxy(no_proxy) => {
                let no_proxy = no_proxy.clone();
                NetworkSettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.proxy_no_proxy.set_value(no_proxy, ctx));
                });
                ctx.notify();
            }
            NetworkPageAction::TestConnection => {
                // 切到 Running 显示进度;真正发起请求时使用当前全局代理配置
                // (init.rs 的订阅会在每次 settings 变更时刷新全局 slot)。
                self.test_state = TestState::Running;
                ctx.notify();

                let client = Arc::new(http_client::Client::new());
                let target = TEST_CONNECTION_URL.to_string();
                ctx.spawn(
                    async move {
                        let req = client.get(&target).timeout(std::time::Duration::from_secs(
                            TEST_CONNECTION_TIMEOUT_SECS,
                        ));
                        match req.send().await {
                            Ok(resp) => Ok(resp.status().as_u16()),
                            Err(err) => Err(format!("{err:#}")),
                        }
                    },
                    |me, result, ctx| {
                        me.handle_action(&NetworkPageAction::TestConnectionResult(result), ctx);
                    },
                );
            }
            NetworkPageAction::TestConnectionResult(result) => {
                self.test_state = match result {
                    Ok(status) => TestState::Success { status: *status },
                    Err(msg) => TestState::Failed {
                        message: msg.clone(),
                    },
                };
                ctx.notify();
            }
        }
    }
}

impl View for NetworkPageView {
    fn ui_name() -> &'static str {
        "NetworkPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

impl SettingsPageMeta for NetworkPageView {
    fn section() -> SettingsSection {
        SettingsSection::Network
    }

    fn should_render(&self, _ctx: &AppContext) -> bool {
        // 与 nav 一同受 FeatureFlag::HttpProxySettings 门控,nav 已过滤,这里恒真即可。
        true
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        self.page.update_filter(query, ctx)
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.page.scroll_to_widget(widget_id);
    }

    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}

impl From<ViewHandle<NetworkPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<NetworkPageView>) -> Self {
        SettingsPageViewHandle::Network(view_handle)
    }
}

#[derive(Default)]
struct NetworkPageWidget;

impl SettingsWidget for NetworkPageWidget {
    type View = NetworkPageView;

    fn search_terms(&self) -> &str {
        "network proxy http https 代理 网络 vpn 公司 corporate system custom off no_proxy 测试连接"
    }

    fn render(
        &self,
        view: &NetworkPageView,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let net = NetworkSettings::as_ref(app);
        let mode = *net.proxy_mode.value();
        let url_value = net.proxy_url.value().clone();
        let username_value = net.proxy_username.value().clone();
        let no_proxy_value = net.proxy_no_proxy.value().clone();
        // 密码不明文显示;仅提示"已设置 / 未设置"。
        let has_password = !ProxyCredentials::as_ref(app).password().is_empty();

        let page_title = crate::t!("settings-network-page-title");
        let header = crate::t!("settings-network-header");
        let description = crate::t!("settings-network-description");

        let mut content = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(render_page_title(&page_title, HEADER_FONT_SIZE, appearance))
            .with_child(render_sub_header_with_description(
                appearance,
                header,
                description,
            ));

        // 1. 模式 dropdown
        content.add_child(render_body_item::<NetworkPageAction>(
            crate::t!("settings-network-mode-label"),
            None::<AdditionalInfo<NetworkPageAction>>,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            warpui::elements::ChildView::new(&view.mode_dropdown).finish(),
            Some(crate::t!("settings-network-mode-description")),
        ));

        // 仅当 mode == Custom 时后续字段才可用。由于 `ToggleState` 未实现 `Clone`,
        // 每次使用时都重新从 bool 转出一个新值(该枚举已 impl `From<bool>`)。
        let custom_enabled = matches!(mode, ProxyMode::Custom);

        // 2. URL
        let url_description = if url_value.is_empty() {
            crate::t!("settings-network-url-description")
        } else {
            format!(
                "{} — {}",
                crate::t!("settings-network-url-description"),
                crate::t!("settings-network-url-current", value = url_value.clone())
            )
        };
        content.add_child(render_body_item::<NetworkPageAction>(
            crate::t!("settings-network-url-label"),
            None::<AdditionalInfo<NetworkPageAction>>,
            LocalOnlyIconState::Hidden,
            ToggleState::from(custom_enabled),
            appearance,
            warpui::elements::ChildView::new(&view.url_input).finish(),
            Some(url_description),
        ));

        // 3. 用户名
        let username_current = if username_value.is_empty() {
            crate::t!("settings-network-empty")
        } else {
            username_value.clone()
        };
        let username_description = format!(
            "{} {}",
            crate::t!("settings-network-username-description"),
            crate::t!("settings-network-username-current", value = username_current)
        );
        content.add_child(render_body_item::<NetworkPageAction>(
            crate::t!("settings-network-username-label"),
            None::<AdditionalInfo<NetworkPageAction>>,
            LocalOnlyIconState::Hidden,
            ToggleState::from(custom_enabled),
            appearance,
            warpui::elements::ChildView::new(&view.username_input).finish(),
            Some(username_description),
        ));

        // 4. 密码。提交后密码存入 OS 密钥库,不出现在 settings.toml 中。
        let password_status = if has_password {
            crate::t!("settings-network-password-set")
        } else {
            crate::t!("settings-network-password-unset")
        };
        content.add_child(render_body_item::<NetworkPageAction>(
            crate::t!("settings-network-password-label"),
            None::<AdditionalInfo<NetworkPageAction>>,
            LocalOnlyIconState::Hidden,
            ToggleState::from(custom_enabled),
            appearance,
            warpui::elements::ChildView::new(&view.password_input).finish(),
            Some(crate::t!(
                "settings-network-password-description",
                value = password_status
            )),
        ));

        // 5. no_proxy
        let no_proxy_current = if no_proxy_value.is_empty() {
            crate::t!("settings-network-empty")
        } else {
            no_proxy_value.clone()
        };
        content.add_child(render_body_item::<NetworkPageAction>(
            crate::t!("settings-network-no-proxy-label"),
            None::<AdditionalInfo<NetworkPageAction>>,
            LocalOnlyIconState::Hidden,
            ToggleState::from(custom_enabled),
            appearance,
            warpui::elements::ChildView::new(&view.no_proxy_input).finish(),
            Some(crate::t!(
                "settings-network-no-proxy-description",
                value = no_proxy_current
            )),
        ));

        // 6. Test connection 按钮 + 结果文本
        let test_button = appearance
            .ui_builder()
            .button(ButtonVariant::Secondary, view.test_button_state.clone())
            .with_text_label(crate::t!("settings-network-test-button"))
            .with_style(
                UiComponentStyles::default()
                    .set_padding(Coords::uniform(8.))
                    .set_margin(Coords::default().top(12.)),
            )
            .build()
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(NetworkPageAction::TestConnection);
            });

        let result_text: String = match &view.test_state {
            TestState::Idle => crate::t!("settings-network-test-idle", url = TEST_CONNECTION_URL),
            TestState::Running => crate::t!("settings-network-test-running"),
            TestState::Success { status } => {
                crate::t!("settings-network-test-success", status = (*status as i64))
            }
            TestState::Failed { message } => {
                crate::t!("settings-network-test-failed", error = message.clone())
            }
        };

        let result_text_element = Text::new(
            result_text,
            appearance.ui_font_family(),
            appearance.ui_font_size(),
        )
        .with_color(appearance.theme().nonactive_ui_text_color().into())
        .with_style(Properties::default().weight(Weight::Normal))
        .finish();

        content.add_child(
            Container::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_alignment(MainAxisAlignment::Start)
                    .with_child(test_button.finish())
                    .with_child(
                        Container::new(result_text_element)
                            .with_padding_left(12.)
                            .finish(),
                    )
                    .finish(),
            )
            .with_margin_top(16.)
            .finish(),
        );

        content.finish()
    }
}
