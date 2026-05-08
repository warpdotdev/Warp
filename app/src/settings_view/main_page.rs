use super::{
    settings_page::{
        MatchData, PageType, SettingsPageMeta, SettingsPageViewHandle, SettingsWidget,
    },
    SettingsSection,
};
use crate::editor::{
    EditorView, Event as EditorEvent, SingleLineEditorOptions, TextColors, TextOptions,
};
use crate::{appearance::Appearance, workspace::WorkspaceAction};
use ::ai::api_keys::{ApiKeyManager, ApiKeys};
use warp_core::channel::ChannelState;
use warpui::keymap::ContextPredicate;
use warpui::{
    elements::{
        Align, Container, CrossAxisAlignment, Element, Flex, MouseStateHandle, ParentElement,
        Shrinkable, Text,
    },
    ui_components::components::{Coords, UiComponent, UiComponentStyles},
};
use warpui::{
    elements::{Border, Empty},
    platform::Cursor,
};
use warpui::{AppContext, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle};

const REGULAR_TEXT_FONT_SIZE: f32 = 12.;
const VERTICAL_MARGIN: f32 = 24.;

pub fn init_actions_from_parent_view<T>(
    app: &mut AppContext,
    context: &ContextPredicate,
    builder: fn(super::SettingsAction) -> T,
) {
    let _ = (app, context, builder);
}

pub fn handle_experiment_change(app: &mut AppContext) {
    let _ = app;
}

#[derive(Debug, Clone)]
pub enum MainPageAction {}

#[derive(Clone, Copy)]
pub enum MainSettingsPageEvent {}

pub struct MainSettingsPageView {
    page: PageType<Self>,
}

impl Entity for MainSettingsPageView {
    type Event = MainSettingsPageEvent;
}

impl TypedActionView for MainSettingsPageView {
    type Action = MainPageAction;

    fn handle_action(&mut self, action: &Self::Action, _ctx: &mut ViewContext<Self>) {
        match *action {}
    }
}

impl View for MainSettingsPageView {
    fn ui_name() -> &'static str {
        "MainSettingsPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

impl MainSettingsPageView {
    pub fn new(ctx: &mut ViewContext<MainSettingsPageView>) -> Self {
        let mut widgets: Vec<Box<dyn SettingsWidget<View = Self>>> = vec![
            Box::new(OpenRouterSettingsWidget::new(ctx)),
            Box::new(DividerWidget {}),
        ];

        if ChannelState::app_version().is_some() {
            widgets.push(Box::new(VersionInfoWidget::default()));
        }

        let page = PageType::new_uncategorized(widgets, Some("OpenRouter"));

        MainSettingsPageView { page }
    }
}

struct OpenRouterSettingsWidget {
    api_key_editor: ViewHandle<EditorView>,
    model_editor: ViewHandle<EditorView>,
}

impl OpenRouterSettingsWidget {
    fn new(ctx: &mut ViewContext<MainSettingsPageView>) -> Self {
        ApiKeyManager::handle(ctx).update(ctx, |manager, ctx| {
            manager.load_keys_from_secure_storage_if_needed(ctx);
        });

        let ApiKeys {
            open_router,
            open_router_model,
            ..
        } = ApiKeyManager::as_ref(ctx).keys().clone();

        let api_key_editor = Self::create_editor(ctx, open_router, "sk-or-v1-...", true);
        ctx.subscribe_to_view(&api_key_editor, |_, editor, event, ctx| {
            if matches!(event, EditorEvent::Blurred | EditorEvent::Enter) {
                let buffer_text = editor.as_ref(ctx).buffer_text(ctx);
                let value = if buffer_text.is_empty() {
                    None
                } else {
                    Some(buffer_text)
                };
                ApiKeyManager::handle(ctx).update(ctx, |model, ctx| {
                    model.set_open_router_key(value, ctx);
                });
            }
        });

        let model_editor = Self::create_editor(ctx, open_router_model, "openrouter/auto", false);
        ctx.subscribe_to_view(&model_editor, |_, editor, event, ctx| {
            if matches!(event, EditorEvent::Blurred | EditorEvent::Enter) {
                let buffer_text = editor.as_ref(ctx).buffer_text(ctx);
                let value = if buffer_text.is_empty() {
                    None
                } else {
                    Some(buffer_text)
                };
                ApiKeyManager::handle(ctx).update(ctx, |model, ctx| {
                    model.set_open_router_model(value, ctx);
                });
            }
        });

        Self {
            api_key_editor,
            model_editor,
        }
    }

    fn create_editor(
        ctx: &mut ViewContext<MainSettingsPageView>,
        value: Option<String>,
        placeholder: &'static str,
        is_password: bool,
    ) -> ViewHandle<EditorView> {
        ctx.add_typed_action_view(move |ctx| {
            let appearance = Appearance::handle(ctx).as_ref(ctx);
            let options = SingleLineEditorOptions {
                is_password,
                text: TextOptions {
                    font_size_override: Some(appearance.ui_font_size()),
                    font_family_override: Some(appearance.monospace_font_family()),
                    text_colors_override: Some(TextColors {
                        default_color: appearance.theme().active_ui_text_color(),
                        disabled_color: appearance.theme().disabled_ui_text_color(),
                        hint_color: appearance.theme().disabled_ui_text_color(),
                    }),
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut editor = EditorView::single_line(options, ctx);
            editor.set_placeholder_text(placeholder, ctx);
            if let Some(value) = value.as_ref() {
                editor.set_buffer_text(value, ctx);
            }
            editor
        })
    }

    fn render_input(
        label: &'static str,
        editor: ViewHandle<EditorView>,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let label = Text::new_inline(label, appearance.ui_font_family(), REGULAR_TEXT_FONT_SIZE)
            .with_color(appearance.theme().active_ui_text_color().into())
            .finish();

        let input = appearance
            .ui_builder()
            .text_input(editor)
            .with_style(UiComponentStyles {
                padding: Some(Coords {
                    top: 10.,
                    bottom: 10.,
                    left: 16.,
                    right: 16.,
                }),
                background: Some(appearance.theme().surface_2().into()),
                ..Default::default()
            })
            .build()
            .finish();

        Flex::column()
            .with_spacing(8.)
            .with_child(label)
            .with_child(input)
            .finish()
    }
}

impl SettingsWidget for OpenRouterSettingsWidget {
    type View = MainSettingsPageView;

    fn search_terms(&self) -> &str {
        "openrouter open router api key model agent free"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        let description = appearance
            .ui_builder()
            .paragraph(
                "Warper runs the agent directly through OpenRouter. No Warp account or plan is required.",
            )
            .with_style(UiComponentStyles {
                font_color: Some(
                    appearance
                        .theme()
                        .active_ui_text_color()
                        .with_opacity(60)
                        .into(),
                ),
                font_size: Some(REGULAR_TEXT_FONT_SIZE),
                margin: Some(Coords {
                    top: 0.,
                    bottom: 0.,
                    left: 0.,
                    right: 0.,
                }),
                ..Default::default()
            })
            .build()
            .finish();

        Container::new(
            Flex::column()
                .with_spacing(16.)
                .with_child(description)
                .with_child(Self::render_input(
                    "OpenRouter API Key",
                    self.api_key_editor.clone(),
                    appearance,
                ))
                .with_child(Self::render_input(
                    "OpenRouter Model",
                    self.model_editor.clone(),
                    appearance,
                ))
                .finish(),
        )
        .with_margin_top(VERTICAL_MARGIN)
        .finish()
    }
}

struct DividerWidget {}

impl SettingsWidget for DividerWidget {
    type View = MainSettingsPageView;

    fn search_terms(&self) -> &str {
        ""
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        Container::new(
            Container::new(Empty::new().finish())
                .with_border(Border::bottom(1.).with_border_fill(appearance.theme().outline()))
                .finish(),
        )
        .with_margin_top(VERTICAL_MARGIN)
        .finish()
    }
}

#[derive(Default)]
struct VersionInfoWidget {
    copy_version_button_mouse_state: MouseStateHandle,
}

impl VersionInfoWidget {
    fn render_version_info(
        &self,
        version: &'static str,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        let faded_text_color = appearance
            .theme()
            .active_ui_text_color()
            .with_opacity(60)
            .into();

        let first_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(
                Shrinkable::new(
                    1.0,
                    Align::new(
                        Text::new_inline(
                            "Version".to_string(),
                            appearance.ui_font_family(),
                            REGULAR_TEXT_FONT_SIZE,
                        )
                        .with_color(faded_text_color)
                        .finish(),
                    )
                    .left()
                    .finish(),
                )
                .finish(),
            );

        let second_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(
                Shrinkable::new(
                    1.0,
                    Align::new(
                        Flex::row()
                            .with_cross_axis_alignment(CrossAxisAlignment::Start)
                            .with_child(
                                appearance
                                    .ui_builder()
                                    .copy_button(16., self.copy_version_button_mouse_state.clone())
                                    .build()
                                    .with_cursor(Cursor::PointingHand)
                                    .on_click(move |ctx, _, _| {
                                        ctx.dispatch_typed_action(WorkspaceAction::CopyVersion(
                                            version,
                                        ));
                                    })
                                    .finish(),
                            )
                            .with_child(
                                Container::new(
                                    Text::new_inline(
                                        version.to_string(),
                                        appearance.ui_font_family(),
                                        REGULAR_TEXT_FONT_SIZE,
                                    )
                                    .with_color(appearance.theme().active_ui_text_color().into())
                                    .finish(),
                                )
                                .with_margin_left(8.)
                                .finish(),
                            )
                            .finish(),
                    )
                    .left()
                    .finish(),
                )
                .finish(),
            );

        let mut version_info = Flex::column();
        version_info.add_child(first_row.finish());
        version_info.add_child(
            Container::new(second_row.finish())
                .with_margin_top(5.)
                .finish(),
        );
        version_info.finish()
    }
}

impl SettingsWidget for VersionInfoWidget {
    type View = MainSettingsPageView;

    fn search_terms(&self) -> &str {
        "version update"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        if let Some(version) = ChannelState::app_version() {
            Container::new(self.render_version_info(version, appearance, app))
                .with_margin_top(VERTICAL_MARGIN)
                .finish()
        } else {
            log::error!("Shouldn't render VersionInfoWidget without GIT_RELEASE_TAG");
            Empty::new().finish()
        }
    }
}

impl SettingsPageMeta for MainSettingsPageView {
    fn section() -> SettingsSection {
        SettingsSection::Account
    }

    fn should_render(&self, _ctx: &AppContext) -> bool {
        true
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        self.page.update_filter(query, ctx)
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.page.scroll_to_widget(widget_id)
    }

    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}

impl From<ViewHandle<MainSettingsPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<MainSettingsPageView>) -> Self {
        SettingsPageViewHandle::Main(view_handle)
    }
}
