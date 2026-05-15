//! "Network" 设置页:全局 HTTP 代理配置(见 Issue #72)。
//!
//! 设计原则:每个输入框始终显示当前已保存值,可直接编辑(包括清空),用旁边的
//! "保存"按钮提交。密码字段用 `is_password: true` mask 显示。System / Off 模式
//! 下输入框禁用 + 显示提示;Custom 模式才可编辑。

use std::sync::Arc;
use std::time::{Duration, Instant};

use settings::Setting;
use warpui::{
    elements::{
        Align, ConstrainedBox, Container, CrossAxisAlignment, Element, Flex, MainAxisAlignment,
        MainAxisSize, MouseStateHandle, ParentElement, Text,
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
use crate::editor::{EditorView, InteractionState, SingleLineEditorOptions, TextOptions};
use crate::report_if_error;
use crate::settings::network::{NetworkSettings, ProxyMode};
use crate::settings::network_secrets::ProxyCredentials;
use crate::view_components::dropdown::{Dropdown, DropdownItem};

/// System / Off 模式下用于“出网连通”探测的公网 URL。
/// `generate_204` 对代理友好,无 body,固定返回 204。
const PUBLIC_PROBE_URL: &str = "https://www.google.com/generate_204";

/// 单次测试连接的最长等待时间。
const TEST_CONNECTION_TIMEOUT_SECS: u64 = 8;

/// 输入框区域(editor + 两个按钮)的最大宽度,与字段标签右侧的槽位约束对齐。
const INPUT_AREA_MAX_WIDTH: f32 = 420.0;

/// 从环境变量读取系统代理(跨平台最小集):返回 (https_proxy, http_proxy, no_proxy)。
/// Windows WinINET / macOS SCDynamicStore 的深入读取留作后续 PR。
fn read_system_proxy_env() -> (String, String, String) {
    fn read(name_upper: &str) -> String {
        std::env::var(name_upper)
            .ok()
            .or_else(|| std::env::var(name_upper.to_lowercase()).ok())
            .unwrap_or_default()
    }
    (read("HTTPS_PROXY"), read("HTTP_PROXY"), read("NO_PROXY"))
}

#[derive(Debug, Clone)]
pub enum NetworkPageAction {
    /// dropdown 选择了某个 ProxyMode 项,持久化到 settings。
    SetProxyMode(ProxyMode),
    /// 点击 URL 字段的"保存"按钮。
    SaveProxyUrl,
    /// 点击 URL 字段的"清除"按钮。
    ClearProxyUrl,
    SaveProxyUsername,
    ClearProxyUsername,
    SaveProxyPassword,
    ClearProxyPassword,
    SaveProxyNoProxy,
    ClearProxyNoProxy,
    /// 点击“测试连接”按钮。
    TestConnection,
    /// 测试连接完成。
    TestConnectionResult(TestOutcome),
}

/// 本次测试选择的探测方式。供结果文案选择合适的描述。
#[derive(Debug, Clone, Copy)]
enum TestKind {
    /// TCP 探测代理 host:port(验证代理本身可达,适合企业内网 / VPN 代理)。
    /// 用于 Custom 模式与能从环境变量探测到系统代理的 System 模式。
    Tcp,
    /// HTTP GET 公网探测 URL。仅用于 Off 模式 或 System 模式但未能探测
    /// 到系统代理时的退化。
    Http,
}

/// 测试结果(从 async 任务返回给 main 线程的 handle_action)。
#[derive(Debug, Clone)]
pub struct TestOutcome {
    kind: TestKind,
    result: Result<u128, String>,
}

/// 测试连接的当前状态。
#[derive(Debug, Clone, Default)]
enum TestState {
    #[default]
    Idle,
    Running,
    Success {
        kind: TestKind,
        latency_ms: u128,
    },
    Failed {
        kind: TestKind,
        message: String,
    },
}

pub struct NetworkPageView {
    page: PageType<Self>,
    /// 代理模式下拉。
    mode_dropdown: ViewHandle<Dropdown<NetworkPageAction>>,
    /// 各字段的 editor(密码字段开了 `is_password` mask)。
    url_editor: ViewHandle<EditorView>,
    username_editor: ViewHandle<EditorView>,
    password_editor: ViewHandle<EditorView>,
    no_proxy_editor: ViewHandle<EditorView>,
    /// 每个字段对应的两个按钮(保存 + 清除)的 mouse state。
    url_save_state: MouseStateHandle,
    url_clear_state: MouseStateHandle,
    username_save_state: MouseStateHandle,
    username_clear_state: MouseStateHandle,
    password_save_state: MouseStateHandle,
    password_clear_state: MouseStateHandle,
    no_proxy_save_state: MouseStateHandle,
    no_proxy_clear_state: MouseStateHandle,
    /// 测试连接按钮的 mouse state 与状态。
    test_button_state: MouseStateHandle,
    test_state: TestState,
}

impl NetworkPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let mode_dropdown = ctx.add_typed_action_view(Dropdown::<NetworkPageAction>::new);
        mode_dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_items(
                vec![
                    DropdownItem::new(
                        crate::t!("settings-network-mode-off"),
                        NetworkPageAction::SetProxyMode(ProxyMode::Off),
                    ),
                    DropdownItem::new(
                        crate::t!("settings-network-mode-system"),
                        NetworkPageAction::SetProxyMode(ProxyMode::System),
                    ),
                    DropdownItem::new(
                        crate::t!("settings-network-mode-custom"),
                        NetworkPageAction::SetProxyMode(ProxyMode::Custom),
                    ),
                ],
                ctx,
            );
        });

        let url_editor =
            build_text_editor(ctx, false, crate::t!("settings-network-url-placeholder"));
        let username_editor =
            build_text_editor(ctx, false, crate::t!("settings-network-username-placeholder"));
        let password_editor =
            build_text_editor(ctx, true, crate::t!("settings-network-password-placeholder"));
        let no_proxy_editor = build_text_editor(
            ctx,
            false,
            crate::t!("settings-network-no-proxy-placeholder"),
        );

        // 订阅 settings / credentials 变更 — 任何字段或 mode 外部变更后,
        // 把最新值灌回各 editor 的 buffer,并同步 dropdown 选项。
        ctx.subscribe_to_model(
            &NetworkSettings::handle(ctx),
            |me: &mut Self, _, _event, ctx| {
                Self::sync_all_from_settings(me, ctx);
                ctx.notify();
            },
        );
        ctx.subscribe_to_model(
            &ProxyCredentials::handle(ctx),
            |me: &mut Self, _, _event, ctx| {
                Self::sync_password_from_credentials(me, ctx);
                ctx.notify();
            },
        );

        let mut me = Self {
            page: PageType::new_monolith(NetworkPageWidget::default(), None, false),
            mode_dropdown,
            url_editor,
            username_editor,
            password_editor,
            no_proxy_editor,
            url_save_state: MouseStateHandle::default(),
            url_clear_state: MouseStateHandle::default(),
            username_save_state: MouseStateHandle::default(),
            username_clear_state: MouseStateHandle::default(),
            password_save_state: MouseStateHandle::default(),
            password_clear_state: MouseStateHandle::default(),
            no_proxy_save_state: MouseStateHandle::default(),
            no_proxy_clear_state: MouseStateHandle::default(),
            test_button_state: MouseStateHandle::default(),
            test_state: TestState::Idle,
        };

        // 初始同步一次,让 dropdown 与各 editor 显示当前已保存值。
        Self::sync_all_from_settings(&mut me, ctx);
        Self::sync_password_from_credentials(&mut me, ctx);
        me
    }

    /// 把当前 NetworkSettings 的值灌进 dropdown 与三个非密码 editor。
    fn sync_all_from_settings(me: &mut Self, ctx: &mut ViewContext<Self>) {
        let net = NetworkSettings::as_ref(ctx);
        let mode = *net.proxy_mode.value();
        let url = net.proxy_url.value().clone();
        let username = net.proxy_username.value().clone();
        let no_proxy = net.proxy_no_proxy.value().clone();

        // dropdown 选项跟随 mode。
        let label: String = match mode {
            ProxyMode::Off => crate::t!("settings-network-mode-off"),
            ProxyMode::System => crate::t!("settings-network-mode-system"),
            ProxyMode::Custom => crate::t!("settings-network-mode-custom"),
        };
        me.mode_dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_selected_by_name(label, ctx);
        });

        // editor buffer 跟随 setting 值;同时按 mode 切换 InteractionState。
        let editable = matches!(mode, ProxyMode::Custom);
        set_editor_text_and_state(&me.url_editor, &url, editable, ctx);
        set_editor_text_and_state(&me.username_editor, &username, editable, ctx);
        set_editor_text_and_state(&me.no_proxy_editor, &no_proxy, editable, ctx);

        // 密码也跟随 mode 切换交互态(buffer 由 ProxyCredentials 订阅单独刷)。
        me.password_editor.update(ctx, |editor, ctx| {
            editor.set_interaction_state(
                if editable {
                    InteractionState::Editable
                } else {
                    InteractionState::Disabled
                },
                ctx,
            );
        });
    }

    /// 把当前密码灌进 password editor(由 ProxyCredentials 单独管理)。
    fn sync_password_from_credentials(me: &mut Self, ctx: &mut ViewContext<Self>) {
        let pw = ProxyCredentials::as_ref(ctx).password().to_string();
        me.password_editor.update(ctx, |editor, ctx| {
            editor.set_buffer_text(&pw, ctx);
        });
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
            NetworkPageAction::SaveProxyUrl => {
                let value = self.url_editor.as_ref(ctx).buffer_text(ctx);
                NetworkSettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.proxy_url.set_value(value, ctx));
                });
                ctx.notify();
            }
            NetworkPageAction::ClearProxyUrl => {
                NetworkSettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.proxy_url.set_value(String::new(), ctx));
                });
                ctx.notify();
            }
            NetworkPageAction::SaveProxyUsername => {
                let value = self.username_editor.as_ref(ctx).buffer_text(ctx);
                NetworkSettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.proxy_username.set_value(value, ctx));
                });
                ctx.notify();
            }
            NetworkPageAction::ClearProxyUsername => {
                NetworkSettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.proxy_username.set_value(String::new(), ctx));
                });
                ctx.notify();
            }
            NetworkPageAction::SaveProxyPassword => {
                let value = self.password_editor.as_ref(ctx).buffer_text(ctx);
                ProxyCredentials::handle(ctx).update(ctx, |creds, ctx| {
                    creds.set_password(value, ctx);
                });
                ctx.notify();
            }
            NetworkPageAction::ClearProxyPassword => {
                ProxyCredentials::handle(ctx).update(ctx, |creds, ctx| {
                    creds.set_password(String::new(), ctx);
                });
                ctx.notify();
            }
            NetworkPageAction::SaveProxyNoProxy => {
                let value = self.no_proxy_editor.as_ref(ctx).buffer_text(ctx);
                NetworkSettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.proxy_no_proxy.set_value(value, ctx));
                });
                ctx.notify();
            }
            NetworkPageAction::ClearProxyNoProxy => {
                NetworkSettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.proxy_no_proxy.set_value(String::new(), ctx));
                });
                ctx.notify();
            }
            NetworkPageAction::TestConnection => {
                self.test_state = TestState::Running;
                ctx.notify();

                // 根据当前 mode 决定测试策略:
                //   Custom → TCP 探测代理 host:port (代理连通性,与出网无关,适合企业内网代理)
                //   System / Off → HTTP GET 公网探测 URL (出网连通性)
                let mode = *NetworkSettings::as_ref(ctx).proxy_mode.value();
                let proxy_url = NetworkSettings::as_ref(ctx).proxy_url.value().clone();
                spawn_test_connection(self, mode, proxy_url, ctx);
            }
            NetworkPageAction::TestConnectionResult(outcome) => {
                self.test_state = match &outcome.result {
                    Ok(latency_ms) => TestState::Success {
                        kind: outcome.kind,
                        latency_ms: *latency_ms,
                    },
                    Err(msg) => TestState::Failed {
                        kind: outcome.kind,
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

/// 根据模式选择探测方式,spawn 到后台运行,结果通过 action 回到主线程。
fn spawn_test_connection(
    _view: &NetworkPageView,
    mode: ProxyMode,
    proxy_url: String,
    ctx: &mut ViewContext<NetworkPageView>,
) {
    let timeout = Duration::from_secs(TEST_CONNECTION_TIMEOUT_SECS);

    match mode {
        ProxyMode::Custom => {
            // 用户填的代理:解析后 TCP 探测 host:port。
            let Some((host, port)) = parse_host_port(&proxy_url) else {
                ctx.spawn(
                    async move {
                        TestOutcome {
                            kind: TestKind::Tcp,
                            result: Err("invalid proxy URL".to_string()),
                        }
                    },
                    |me, outcome, ctx| {
                        me.handle_action(
                            &NetworkPageAction::TestConnectionResult(outcome),
                            ctx,
                        );
                    },
                );
                return;
            };
            spawn_tcp_probe(host, port, timeout, ctx);
        }
        ProxyMode::System => {
            // 优先从环境变量读系统代理(跨平台的最小集),能读到则走 TCP
            // 探测;读不到(macOS SCDynamicStore / Windows WinINET 仅 reqwest 内部
            // 使用)则退化 HTTP 探测公网。
            let (sys_https, sys_http, _) = read_system_proxy_env();
            let sys_proxy = if !sys_https.is_empty() {
                sys_https
            } else {
                sys_http
            };
            if let Some((host, port)) = parse_host_port(&sys_proxy) {
                spawn_tcp_probe(host, port, timeout, ctx);
            } else {
                spawn_http_probe(timeout, ctx);
            }
        }
        ProxyMode::Off => {
            // Off 模式没有代理可测,测一下“直连出网”可不可。
            spawn_http_probe(timeout, ctx);
        }
    }
}

/// 同步 TCP 探测逻辑抽出为 helper,顺便可重用 Custom 与 System 两路。
fn spawn_tcp_probe(
    host: String,
    port: u16,
    timeout: Duration,
    ctx: &mut ViewContext<NetworkPageView>,
) {
    ctx.spawn(
        async move {
            let start = Instant::now();
            let addr = format!("{host}:{port}");
            let result =
                tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr)).await;
            let outcome_result = match result {
                Ok(Ok(_stream)) => Ok(start.elapsed().as_millis()),
                Ok(Err(e)) => Err(format!("{e}")),
                Err(_) => Err(format!("timeout after {}s", timeout.as_secs())),
            };
            TestOutcome {
                kind: TestKind::Tcp,
                result: outcome_result,
            }
        },
        |me, outcome, ctx| {
            me.handle_action(&NetworkPageAction::TestConnectionResult(outcome), ctx);
        },
    );
}

/// HTTP 探测逻辑(走 reqwest 全局代理设置)。仅用于 Off 或 System 退化场景。
fn spawn_http_probe(timeout: Duration, ctx: &mut ViewContext<NetworkPageView>) {
    let client = Arc::new(http_client::Client::new());
    let target = PUBLIC_PROBE_URL.to_string();
    ctx.spawn(
        async move {
            let start = Instant::now();
            let outcome_result = match client.get(&target).timeout(timeout).send().await {
                Ok(resp) => {
                    if resp.status().is_success() || resp.status().as_u16() == 204 {
                        Ok(start.elapsed().as_millis())
                    } else {
                        Err(format!("HTTP {}", resp.status().as_u16()))
                    }
                }
                Err(err) => Err(format!("{err:#}")),
            };
            TestOutcome {
                kind: TestKind::Http,
                result: outcome_result,
            }
        },
        |me, outcome, ctx| {
            me.handle_action(&NetworkPageAction::TestConnectionResult(outcome), ctx);
        },
    );
}

/// 从一个“粗略”代理 URL 中抽取 host + port。
/// 支持以下输入:
///   - `http://host:port`
///   - `https://host:port`
///   - `socks5://host:port`
///   - `host:port`(无 scheme)
/// 返回 `None` 表示无法解析。
fn parse_host_port(raw: &str) -> Option<(String, u16)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // 如果有 scheme,先走 url::Url 解析;否则补上 `http://` 再解析。
    let normalized: String = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let url = url::Url::parse(&normalized).ok()?;
    let host = url.host_str()?.to_string();
    let port = url.port_or_known_default()?;
    Some((host, port))
}

/// 构造单行 EditorView,可选 password mask。
fn build_text_editor(
    ctx: &mut ViewContext<NetworkPageView>,
    is_password: bool,
    placeholder: String,
) -> ViewHandle<EditorView> {
    ctx.add_typed_action_view(move |ctx| {
        let appearance = Appearance::as_ref(ctx);
        let options = SingleLineEditorOptions {
            is_password,
            text: TextOptions {
                font_size_override: Some(appearance.ui_font_size()),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut editor = EditorView::single_line(options, ctx);
        editor.set_placeholder_text(placeholder, ctx);
        editor
    })
}

/// 把当前值写入 editor buffer,并按 `editable` 切换 InteractionState。
/// 注意:`set_buffer_text` 会重置光标,不该在用户聚焦编辑时调用 —— 本函数仅
/// 在 settings 外部变更时使用。
fn set_editor_text_and_state(
    editor: &ViewHandle<EditorView>,
    value: &str,
    editable: bool,
    ctx: &mut ViewContext<NetworkPageView>,
) {
    editor.update(ctx, |editor, ctx| {
        // 若 buffer 已经等于目标值,跳过 set 以避免不必要的 cursor 重置。
        if editor.buffer_text(ctx) != value {
            editor.set_buffer_text(value, ctx);
        }
        editor.set_interaction_state(
            if editable {
                InteractionState::Editable
            } else {
                InteractionState::Disabled
            },
            ctx,
        );
    });
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
        _app: &AppContext,
    ) -> Box<dyn Element> {
        // 注: SettingsWidget::render 传入的 `_app` 是渲染时的 AppContext;读当前 mode
        // 需要用到。这里暂不改参数名以避免全文修改,下文直接用 `_app`。
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

        // 1. 模式 dropdown — 始终 enabled
        content.add_child(render_body_item::<NetworkPageAction>(
            crate::t!("settings-network-mode-label"),
            None::<AdditionalInfo<NetworkPageAction>>,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            warpui::elements::ChildView::new(&view.mode_dropdown).finish(),
            Some(crate::t!("settings-network-mode-description")),
        ));

        // 字段渲染辅助:一个 editor + 保存按钮 + 清除按钮,统一宽度对齐。
        let render_field = |label: String,
                            description: String,
                            editor: &ViewHandle<EditorView>,
                            save_state: &MouseStateHandle,
                            clear_state: &MouseStateHandle,
                            save_action: NetworkPageAction,
                            clear_action: NetworkPageAction|
         -> Box<dyn Element> {
            let editor_element = warpui::elements::ChildView::new(editor).finish();
            // 设固定宽度 + 小 padding,以实现表单列整齐 + 文字与按钮宽度区匹配。
            // 不用 with_centered_text_label——当前 Button 实现中 `CenteredText` 路径未明确
            // 指定 Align 方向,会造成文字偏右;用固定宽 + Text 默认左对齐 +
            // 左右对称 padding,视觉上即居中。
            const ACTION_BUTTON_WIDTH: f32 = 56.0;
            let save_button = appearance
                .ui_builder()
                .button(ButtonVariant::Accent, save_state.clone())
                .with_text_label(crate::t!("settings-network-save"))
                .with_style(
                    UiComponentStyles::default()
                        .set_padding(Coords {
                            top: 5.,
                            bottom: 5.,
                            left: 14.,
                            right: 14.,
                        })
                        .set_margin(Coords::default().left(6.))
                        .set_width(ACTION_BUTTON_WIDTH),
                )
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(save_action.clone());
                })
                .finish();
            let clear_button = appearance
                .ui_builder()
                .button(ButtonVariant::Text, clear_state.clone())
                .with_text_label(crate::t!("settings-network-clear"))
                .with_style(
                    UiComponentStyles::default()
                        .set_padding(Coords {
                            top: 5.,
                            bottom: 5.,
                            left: 12.,
                            right: 12.,
                        })
                        .set_margin(Coords::default().left(4.))
                        .set_width(ACTION_BUTTON_WIDTH),
                )
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(clear_action.clone());
                })
                .finish();

            let input_area = ConstrainedBox::new(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Min)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(
                        // editor 占据剩余空间,放进一个有限宽度的容器里(避免内部 flex 在
                        // 无限约束下出问题)。
                        ConstrainedBox::new(editor_element)
                            .with_max_width(INPUT_AREA_MAX_WIDTH - 120.0)
                            .finish(),
                    )
                    .with_child(save_button)
                    .with_child(clear_button)
                    .finish(),
            )
            .with_max_width(INPUT_AREA_MAX_WIDTH)
            .finish();

            render_body_item::<NetworkPageAction>(
                label,
                None::<AdditionalInfo<NetworkPageAction>>,
                LocalOnlyIconState::Hidden,
                ToggleState::Enabled,
                appearance,
                input_area,
                Some(description),
            )
        };

        // 2. URL
        content.add_child(render_field(
            crate::t!("settings-network-url-label"),
            crate::t!("settings-network-url-description"),
            &view.url_editor,
            &view.url_save_state,
            &view.url_clear_state,
            NetworkPageAction::SaveProxyUrl,
            NetworkPageAction::ClearProxyUrl,
        ));

        // 3. 用户名
        content.add_child(render_field(
            crate::t!("settings-network-username-label"),
            crate::t!("settings-network-username-description"),
            &view.username_editor,
            &view.username_save_state,
            &view.username_clear_state,
            NetworkPageAction::SaveProxyUsername,
            NetworkPageAction::ClearProxyUsername,
        ));

        // 4. 密码
        content.add_child(render_field(
            crate::t!("settings-network-password-label"),
            crate::t!("settings-network-password-description"),
            &view.password_editor,
            &view.password_save_state,
            &view.password_clear_state,
            NetworkPageAction::SaveProxyPassword,
            NetworkPageAction::ClearProxyPassword,
        ));

        // 5. no_proxy
        content.add_child(render_field(
            crate::t!("settings-network-no-proxy-label"),
            crate::t!("settings-network-no-proxy-description"),
            &view.no_proxy_editor,
            &view.no_proxy_save_state,
            &view.no_proxy_clear_state,
            NetworkPageAction::SaveProxyNoProxy,
            NetworkPageAction::ClearProxyNoProxy,
        ));

        // 6. 测试连接 — 同上:固定宽 + 左右对称 padding,避开 CenteredText 偏右问题。
        const TEST_BUTTON_WIDTH: f32 = 100.0;
        let mut test_button = appearance
            .ui_builder()
            .button(ButtonVariant::Accent, view.test_button_state.clone())
            .with_text_label(crate::t!("settings-network-test-button"))
            .with_style(
                UiComponentStyles::default()
                    .set_padding(Coords {
                        top: 6.,
                        bottom: 6.,
                        left: 14.,
                        right: 14.,
                    })
                    .set_width(TEST_BUTTON_WIDTH),
            )
            .build()
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(NetworkPageAction::TestConnection);
            });
        if matches!(view.test_state, TestState::Running) {
            test_button = test_button.disable();
        }

        // Idle 提示文案需要与当前模式匹配:Custom 测代理连通性,System/Off 测出网连通性。
        let mode = *NetworkSettings::as_ref(_app).proxy_mode.value();
        let result_text: String = match &view.test_state {
            TestState::Idle => match mode {
                ProxyMode::Custom => crate::t!("settings-network-test-idle-tcp"),
                ProxyMode::System | ProxyMode::Off => {
                    crate::t!("settings-network-test-idle-http", url = PUBLIC_PROBE_URL)
                }
            },
            TestState::Running => crate::t!("settings-network-test-running"),
            TestState::Success { kind, latency_ms } => match kind {
                TestKind::Tcp => crate::t!(
                    "settings-network-test-success-tcp",
                    latency = (*latency_ms as i64)
                ),
                TestKind::Http => crate::t!(
                    "settings-network-test-success-http",
                    latency = (*latency_ms as i64)
                ),
            },
            TestState::Failed { kind, message } => match kind {
                TestKind::Tcp => crate::t!(
                    "settings-network-test-failed-tcp",
                    error = message.clone()
                ),
                TestKind::Http => crate::t!(
                    "settings-network-test-failed-http",
                    error = message.clone()
                ),
            },
        };
        let result_element = Text::new(
            result_text,
            appearance.ui_font_family(),
            appearance.ui_font_size(),
        )
        .with_color(appearance.theme().nonactive_ui_text_color().into())
        .with_style(Properties::default().weight(Weight::Normal))
        .finish();

        // 外层用 Align(left)包裹,防止父级 Flex 在 cross-axis 上 stretch 把按钮拉高;
        // 内层 Flex::row 带 MainAxisSize::Min,只占据实际安全需要的宽度。
        content.add_child(
            Container::new(
                Align::new(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Min)
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_main_axis_alignment(MainAxisAlignment::Start)
                        .with_child(test_button.finish())
                        .with_child(
                            Container::new(result_element)
                                .with_padding_left(12.)
                                .finish(),
                        )
                        .finish(),
                )
                .left()
                .finish(),
            )
            .with_margin_top(20.)
            .finish(),
        );

        content.finish()
    }
}
