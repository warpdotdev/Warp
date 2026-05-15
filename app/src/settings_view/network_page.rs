//! "Network" 设置页:全局 HTTP 代理配置(见 Issue #72)。
//!
//! 设计原则:每个输入框始终显示当前已保存值,可直接编辑(包括清空),用旁边的
//! "保存"按钮提交。密码字段用 `is_password: true` mask 显示。System / Off 模式
//! 下输入框禁用 + 显示提示;Custom 模式才可编辑。

use std::sync::Arc;
use std::time::Duration;

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

/// 测试连接目标 URL。`generate_204` 对代理友好,无 body,固定返回 204。
const TEST_CONNECTION_URL: &str = "https://www.google.com/generate_204";

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
    /// 点击"测试连接"按钮。
    TestConnection,
    /// 测试连接完成。
    TestConnectionResult(Result<u16, String>),
}

/// 测试连接的当前状态。
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
                let client = Arc::new(http_client::Client::new());
                let target = TEST_CONNECTION_URL.to_string();
                ctx.spawn(
                    async move {
                        match client
                            .get(&target)
                            .timeout(Duration::from_secs(TEST_CONNECTION_TIMEOUT_SECS))
                            .send()
                            .await
                        {
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
            // 使用 with_centered_text_label 让按钮文字水平居中。
            let save_button = appearance
                .ui_builder()
                .button(ButtonVariant::Accent, save_state.clone())
                .with_centered_text_label(crate::t!("settings-network-save"))
                .with_style(
                    UiComponentStyles::default()
                        .set_padding(Coords {
                            top: 5.,
                            bottom: 5.,
                            left: 10.,
                            right: 10.,
                        })
                        .set_margin(Coords::default().left(6.)),
                )
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(save_action.clone());
                })
                .finish();
            let clear_button = appearance
                .ui_builder()
                .button(ButtonVariant::Text, clear_state.clone())
                .with_centered_text_label(crate::t!("settings-network-clear"))
                .with_style(
                    UiComponentStyles::default()
                        .set_padding(Coords {
                            top: 5.,
                            bottom: 5.,
                            left: 8.,
                            right: 8.,
                        })
                        .set_margin(Coords::default().left(4.)),
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

        // 6. 测试连接 — 文字水平居中。用 with_centered_text_label 避免默认左对齐。
        let mut test_button = appearance
            .ui_builder()
            .button(ButtonVariant::Accent, view.test_button_state.clone())
            .with_centered_text_label(crate::t!("settings-network-test-button"))
            .with_style(
                UiComponentStyles::default()
                    .set_padding(Coords {
                        top: 6.,
                        bottom: 6.,
                        left: 14.,
                        right: 14.,
                    }),
            )
            .build()
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(NetworkPageAction::TestConnection);
            });
        if matches!(view.test_state, TestState::Running) {
            test_button = test_button.disable();
        }

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
