use ::ai::local_models::{LocalModelClient, LocalModelProvider, ModelInfo};
use ::ai::local_models::config::{ConfiguredModel, LocalModelConfig, ModelParams};
use ::ai::local_models::provider::ProviderFactory;
use settings::Setting;
use warpui::{
    elements::{ChildView, Container, CrossAxisAlignment, Flex, ParentElement, Shrinkable, Text},
    ui_components::components::{Coords, UiComponent, UiComponentStyles},
    AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use crate::{
    appearance::Appearance,
    editor::{EditorView, Event as EditorEvent, SingleLineEditorOptions, TextOptions},
    report_if_error,
    settings::LocalModelSettings,
    settings_view::{
        settings_page::{
            render_page_title, MatchData, PageType, SettingsPageEvent, SettingsPageMeta,
            SettingsPageViewHandle, SettingsWidget, CONTENT_FONT_SIZE, HEADER_FONT_SIZE,
        },
        SettingsSection,
    },
    view_components::{
        action_button::{ActionButton, SecondaryTheme},
        dropdown::TOP_MENU_BAR_HEIGHT,
        Dropdown, DropdownItem,
    },
};

#[derive(Clone, Debug, PartialEq)]
enum ConnectionStatus {
    NotTested,
    Testing,
    Connected,
    Failed(String),
}

impl ConnectionStatus {
    fn display_text(&self) -> String {
        match self {
            Self::NotTested => "Not tested".to_string(),
            Self::Testing => "Testing connection...".to_string(),
            Self::Connected => "Connected".to_string(),
            Self::Failed(err) => format!("Connection failed: {err}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestState {
    Idle,
    Pending,
}

#[derive(Clone, Copy)]
enum UrlEditorField {
    Ollama,
    LMStudio,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LocalModelsPageAction {
    SetProvider(LocalModelProvider),
    SelectModel(String),
    TestConnection,
    RefreshModels,
}

enum LocalModelsRequestResult {
    Connected,
    Models(Vec<ModelInfo>),
}

pub struct LocalModelsSettingsPageView {
    page: PageType<Self>,
    provider_dropdown: ViewHandle<Dropdown<LocalModelsPageAction>>,
    model_dropdown: ViewHandle<Dropdown<LocalModelsPageAction>>,
    ollama_url_editor: ViewHandle<EditorView>,
    lmstudio_url_editor: ViewHandle<EditorView>,
    test_connection_button: ViewHandle<ActionButton>,
    refresh_models_button: ViewHandle<ActionButton>,
    available_models: Vec<ModelInfo>,
    request_state: RequestState,
    connection_status: ConnectionStatus,
}

impl LocalModelsSettingsPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let settings = LocalModelSettings::as_ref(ctx);
        let selected_provider = settings.selected_provider();
        let selected_model = settings.selected_model_name();
        let ollama_url = settings.ollama_base_url.value().clone();
        let lmstudio_url = settings.lmstudio_base_url.value().clone();
        let appearance_handle = Appearance::handle(ctx);

        let provider_dropdown = ctx.add_typed_action_view(|ctx| {
            let mut dropdown = Dropdown::new(ctx);
            dropdown.set_top_bar_max_width(220.);
            dropdown
        });

        let model_dropdown = ctx.add_typed_action_view(|ctx| {
            let mut dropdown = Dropdown::new(ctx);
            dropdown.set_top_bar_max_width(340.);
            dropdown
        });

        let ollama_url_editor = {
            let appearance_handle = appearance_handle.clone();
            let ollama_url = ollama_url.clone();
            ctx.add_typed_action_view(move |ctx| {
                let options = SingleLineEditorOptions {
                    text: TextOptions::ui_font_size(appearance_handle.as_ref(ctx)),
                    ..Default::default()
                };
                let mut editor = EditorView::single_line(options, ctx);
                editor.set_placeholder_text("http://localhost:11434", ctx);
                editor.set_buffer_text(&ollama_url, ctx);
                editor
            })
        };

        let lmstudio_url_editor = {
            let appearance_handle = appearance_handle.clone();
            let lmstudio_url = lmstudio_url.clone();
            ctx.add_typed_action_view(move |ctx| {
                let options = SingleLineEditorOptions {
                    text: TextOptions::ui_font_size(appearance_handle.as_ref(ctx)),
                    ..Default::default()
                };
                let mut editor = EditorView::single_line(options, ctx);
                editor.set_placeholder_text("http://localhost:1234/v1", ctx);
                editor.set_buffer_text(&lmstudio_url, ctx);
                editor
            })
        };

        let test_connection_button = ctx.add_typed_action_view(|_| {
            ActionButton::new("Test connection", SecondaryTheme).on_click(|ctx| {
                ctx.dispatch_typed_action(LocalModelsPageAction::TestConnection);
            })
        });

        let refresh_models_button = ctx.add_typed_action_view(|_| {
            ActionButton::new("Refresh models", SecondaryTheme).on_click(|ctx| {
                ctx.dispatch_typed_action(LocalModelsPageAction::RefreshModels);
            })
        });

        let view = Self {
            page: PageType::new_monolith(LocalModelsWidget, None, false),
            provider_dropdown,
            model_dropdown,
            ollama_url_editor,
            lmstudio_url_editor,
            test_connection_button,
            refresh_models_button,
            available_models: Vec::new(),
            request_state: RequestState::Idle,
            connection_status: ConnectionStatus::NotTested,
        };

        view.populate_provider_dropdown(selected_provider, ctx);
        view.populate_model_dropdown(selected_model, ctx);
        view.update_button_states(ctx);

        ctx.subscribe_to_view(&view.ollama_url_editor, |me, _, event, ctx| {
            me.handle_url_editor_event(UrlEditorField::Ollama, event, ctx);
        });
        ctx.subscribe_to_view(&view.lmstudio_url_editor, |me, _, event, ctx| {
            me.handle_url_editor_event(UrlEditorField::LMStudio, event, ctx);
        });

        view
    }

    fn populate_provider_dropdown(
        &self,
        selected_provider: LocalModelProvider,
        ctx: &mut ViewContext<Self>,
    ) {
        let items = vec![
            DropdownItem::new(
                "Disabled",
                LocalModelsPageAction::SetProvider(LocalModelProvider::None),
            ),
            DropdownItem::new(
                "Ollama",
                LocalModelsPageAction::SetProvider(LocalModelProvider::Ollama),
            ),
            DropdownItem::new(
                "LM Studio",
                LocalModelsPageAction::SetProvider(LocalModelProvider::LMStudio),
            ),
        ];

        self.provider_dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_items(items, ctx);
            dropdown
                .set_selected_by_action(LocalModelsPageAction::SetProvider(selected_provider), ctx);
        });
    }

    fn populate_model_dropdown(&self, selected_model: Option<String>, ctx: &mut ViewContext<Self>) {
        let provider = LocalModelSettings::as_ref(ctx).selected_provider();
        self.model_dropdown.update(ctx, |dropdown, ctx| {
            if provider == LocalModelProvider::None {
                dropdown.set_items(
                    vec![DropdownItem::new(
                        "Select a provider first",
                        LocalModelsPageAction::SelectModel(String::new()),
                    )],
                    ctx,
                );
                dropdown.set_selected_by_index(0, ctx);
                dropdown.set_disabled(ctx);
                return;
            }

            if self.available_models.is_empty() {
                dropdown.set_items(
                    vec![DropdownItem::new(
                        "No models loaded",
                        LocalModelsPageAction::SelectModel(String::new()),
                    )],
                    ctx,
                );
                dropdown.set_selected_by_index(0, ctx);
                dropdown.set_disabled(ctx);
                return;
            }

            let items: Vec<DropdownItem<LocalModelsPageAction>> = self
                .available_models
                .iter()
                .map(|model| {
                    DropdownItem::new(
                        model.name.clone(),
                        LocalModelsPageAction::SelectModel(model.name.clone()),
                    )
                })
                .collect();
            dropdown.set_items(items, ctx);
            dropdown.set_enabled(ctx);

            if let Some(selected_model) = selected_model {
                dropdown.set_selected_by_action(
                    LocalModelsPageAction::SelectModel(selected_model),
                    ctx,
                );
            } else {
                dropdown.set_selected_by_index(0, ctx);
            }
        });
    }

    fn persist_provider_selection(provider: LocalModelProvider, ctx: &mut ViewContext<Self>) {
        LocalModelSettings::handle(ctx).update(ctx, |settings, ctx| {
            report_if_error!(settings
                .enabled
                .set_value(provider != LocalModelProvider::None, ctx));
            report_if_error!(settings
                .provider
                .set_value(LocalModelSettings::provider_storage_value(provider), ctx));
            if provider == LocalModelProvider::None {
                report_if_error!(settings.selected_model.set_value(String::new(), ctx));
            }
        });
    }

    fn persist_url(field: UrlEditorField, value: String, ctx: &mut ViewContext<Self>) {
        LocalModelSettings::handle(ctx).update(ctx, |settings, ctx| match field {
            UrlEditorField::Ollama => {
                report_if_error!(settings.ollama_base_url.set_value(value, ctx));
            }
            UrlEditorField::LMStudio => {
                report_if_error!(settings.lmstudio_base_url.set_value(value, ctx));
            }
        });
    }

    fn update_button_states(&self, ctx: &mut ViewContext<Self>) {
        let disable = self.request_state == RequestState::Pending
            || LocalModelSettings::as_ref(ctx).selected_provider() == LocalModelProvider::None;
        self.test_connection_button.update(ctx, |button, ctx| {
            button.set_disabled(disable, ctx);
        });
        self.refresh_models_button.update(ctx, |button, ctx| {
            button.set_disabled(disable, ctx);
        });
    }

    fn current_provider_urls(app: &AppContext) -> (String, String, LocalModelProvider) {
        let settings = LocalModelSettings::as_ref(app);
        (
            settings.ollama_base_url.value().clone(),
            settings.lmstudio_base_url.value().clone(),
            settings.selected_provider(),
        )
    }

    fn run_provider_request(&mut self, refresh_models: bool, ctx: &mut ViewContext<Self>) {
        if self.request_state == RequestState::Pending {
            return;
        }

        let (ollama_url, lmstudio_url, provider) = Self::current_provider_urls(ctx);
        if provider == LocalModelProvider::None {
            self.connection_status = ConnectionStatus::Failed("No provider selected".to_string());
            ctx.notify();
            return;
        }

        self.request_state = RequestState::Pending;
        self.connection_status = ConnectionStatus::Testing;
        self.update_button_states(ctx);
        ctx.notify();

        ctx.spawn(
            async move {
                let base_url = match provider {
                    LocalModelProvider::Ollama => &ollama_url,
                    LocalModelProvider::LMStudio => &lmstudio_url,
                    _ => return Err("Unsupported or unconfigured provider".to_string()),
                };

                let config = LocalModelConfig {
                    active_model_id: Some("_probe".to_string()),
                    configured_models: vec![ConfiguredModel {
                        id: "_probe".to_string(),
                        display_name: "probe".to_string(),
                        provider,
                        base_url: base_url.clone(),
                        params: ModelParams::default(),
                        max_context_tokens: None,
                        tags: vec![],
                    }],
                    ..Default::default()
                };

                #[allow(deprecated)]
                let client: Box<dyn LocalModelClient> = ProviderFactory::create_client(&config)
                    .map_err(|e: Box<dyn std::error::Error + Send + Sync>| e.to_string())?;

                if refresh_models {
                    let models: Vec<ModelInfo> = client
                        .list_models()
                        .await
                        .map_err(|e: Box<dyn std::error::Error + Send + Sync>| e.to_string())?;
                    Ok(LocalModelsRequestResult::Models(models))
                } else {
                    client
                        .check_connection()
                        .await
                        .map_err(|e: Box<dyn std::error::Error + Send + Sync>| e.to_string())?;
                    Ok(LocalModelsRequestResult::Connected)
                }
            },
            move |me, result, ctx| {
                me.request_state = RequestState::Idle;
                match result {
                    Ok(LocalModelsRequestResult::Connected) => {
                        me.connection_status = ConnectionStatus::Connected;
                    }
                    Ok(LocalModelsRequestResult::Models(models)) => {
                        me.connection_status = ConnectionStatus::Connected;
                        me.available_models = models;
                        let selected_model = LocalModelSettings::as_ref(ctx)
                            .selected_model_name()
                            .filter(|selected_model| {
                                me.available_models
                                    .iter()
                                    .any(|model| model.name == *selected_model)
                            })
                            .or_else(|| {
                                me.available_models.first().map(|model| model.name.clone())
                            });
                        if let Some(selected_model) = &selected_model {
                            LocalModelSettings::handle(ctx).update(ctx, |settings, ctx| {
                                report_if_error!(settings
                                    .selected_model
                                    .set_value(selected_model.clone(), ctx));
                            });
                        }
                        me.populate_model_dropdown(selected_model, ctx);
                    }
                    Err(err) => {
                        me.connection_status = ConnectionStatus::Failed(err);
                        if refresh_models {
                            me.available_models.clear();
                            me.populate_model_dropdown(None, ctx);
                        }
                    }
                }
                me.update_button_states(ctx);
                ctx.notify();
            },
        );
    }

    fn handle_url_editor_event(
        &mut self,
        field: UrlEditorField,
        event: &EditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if !matches!(event, EditorEvent::Blurred | EditorEvent::Enter) {
            return;
        }

        let value = match field {
            UrlEditorField::Ollama => self.ollama_url_editor.as_ref(ctx).buffer_text(ctx),
            UrlEditorField::LMStudio => self.lmstudio_url_editor.as_ref(ctx).buffer_text(ctx),
        };
        Self::persist_url(field, value.trim().to_string(), ctx);
    }
}

impl Entity for LocalModelsSettingsPageView {
    type Event = SettingsPageEvent;
}

impl TypedActionView for LocalModelsSettingsPageView {
    type Action = LocalModelsPageAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            LocalModelsPageAction::SetProvider(provider) => {
                Self::persist_provider_selection(*provider, ctx);
                self.connection_status = ConnectionStatus::NotTested;
                self.available_models.clear();
                self.populate_provider_dropdown(*provider, ctx);
                self.populate_model_dropdown(None, ctx);
                self.update_button_states(ctx);
                ctx.notify();
            }
            LocalModelsPageAction::SelectModel(model) => {
                if model.trim().is_empty() {
                    return;
                }
                LocalModelSettings::handle(ctx).update(ctx, |settings, ctx| {
                    report_if_error!(settings.selected_model.set_value(model.clone(), ctx));
                });
            }
            LocalModelsPageAction::TestConnection => self.run_provider_request(false, ctx),
            LocalModelsPageAction::RefreshModels => self.run_provider_request(true, ctx),
        }
    }
}

impl View for LocalModelsSettingsPageView {
    fn ui_name() -> &'static str {
        "LocalModelsSettingsPageView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

impl SettingsPageMeta for LocalModelsSettingsPageView {
    fn section() -> SettingsSection {
        SettingsSection::LocalModels
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

impl From<ViewHandle<LocalModelsSettingsPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<LocalModelsSettingsPageView>) -> Self {
        SettingsPageViewHandle::LocalModels(view_handle)
    }
}

struct LocalModelsWidget;

impl SettingsWidget for LocalModelsWidget {
    type View = LocalModelsSettingsPageView;

    fn search_terms(&self) -> &str {
        "local models ollama lmstudio lm studio offline provider"
    }

    fn render(
        &self,
        view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let provider = LocalModelSettings::as_ref(app).selected_provider();
        let status_text = view.connection_status.display_text();

        let mut content = Flex::column()
            .with_child(render_page_title("Local models", HEADER_FONT_SIZE, appearance))
            .with_child(
                appearance
                    .ui_builder()
                    .paragraph(
                        "Configure Ollama or LM Studio to run local models directly on this machine.",
                    )
                    .with_style(UiComponentStyles {
                        font_size: Some(CONTENT_FONT_SIZE),
                        margin: Some(Coords::default().bottom(12.)),
                        ..Default::default()
                    })
                    .build()
                    .finish(),
            )
            .with_child(
                labeled_row(
                    "Provider",
                    ChildView::new(&view.provider_dropdown).finish(),
                    appearance,
                ),
            );

        if provider == LocalModelProvider::Ollama {
            content.add_child(labeled_row(
                "Ollama URL",
                render_url_editor(&view.ollama_url_editor, appearance),
                appearance,
            ));
        } else if provider == LocalModelProvider::LMStudio {
            content.add_child(labeled_row(
                "LM Studio URL",
                render_url_editor(&view.lmstudio_url_editor, appearance),
                appearance,
            ));
        }

        content.add_child(labeled_row(
            "Model",
            ChildView::new(&view.model_dropdown).finish(),
            appearance,
        ));

        content.add_child(
            Container::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(ChildView::new(&view.test_connection_button).finish())
                    .with_child(
                        Container::new(ChildView::new(&view.refresh_models_button).finish())
                            .with_margin_left(8.)
                            .finish(),
                    )
                    .finish(),
            )
            .with_margin_bottom(12.)
            .finish(),
        );

        content.add_child(
            appearance
                .ui_builder()
                .span(status_text)
                .with_style(UiComponentStyles {
                    font_size: Some(CONTENT_FONT_SIZE),
                    ..Default::default()
                })
                .build()
                .finish(),
        );

        Container::new(Shrinkable::new(1., content.finish()).finish())
            .with_padding_top(4.)
            .finish()
    }
}

fn labeled_row(
    label: &str,
    control: Box<dyn Element>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    Container::new(
        Flex::column()
            .with_child(
                Text::new(
                    label.to_string(),
                    appearance.ui_font_family(),
                    CONTENT_FONT_SIZE,
                )
                .with_color(appearance.theme().active_ui_text_color().into())
                .finish(),
            )
            .with_child(Container::new(control).with_margin_top(6.).finish())
            .finish(),
    )
    .with_margin_bottom(12.)
    .finish()
}

fn render_url_editor(editor: &ViewHandle<EditorView>, appearance: &Appearance) -> Box<dyn Element> {
    appearance
        .ui_builder()
        .text_input(editor.clone())
        .with_style(UiComponentStyles {
            height: Some(TOP_MENU_BAR_HEIGHT),
            padding: Some(Coords::uniform(7.)),
            margin: Some(Coords::default()),
            font_size: Some(appearance.ui_font_size()),
            background: Some(appearance.theme().surface_2().into()),
            ..Default::default()
        })
        .build()
        .finish()
}
