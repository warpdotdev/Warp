use std::fmt::Display;

use markdown_parser::{FormattedText, FormattedTextFragment, FormattedTextLine};
use regex::Regex;
use warpui::elements::{FormattedTextElement, HighlightedHyperlink};
use warpui::keymap::ContextPredicate;
use warpui::{
    elements::{Container, Flex, MouseStateHandle, ParentElement},
    presenter::ChildView,
    ui_components::components::{UiComponent, UiComponentStyles},
    Action, AppContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View,
    ViewContext, ViewHandle,
};

use crate::ui_components::blended_colors;
use crate::{
    appearance::Appearance,
    terminal::warpify::settings::WarpifySettings,
    view_components::{SubmittableTextInput, SubmittableTextInputEvent},
};

use super::settings_page::{
    render_page_title, Category, MatchData, PageType, SettingsPageEvent, SettingsWidget,
    HEADER_FONT_SIZE,
};
use super::SettingsSection;
use super::{
    settings_page::{render_alternating_color_list, SettingsPageMeta, SettingsPageViewHandle},
    SettingsAction, ToggleSettingActionPair,
};

pub fn init_actions_from_parent_view<T: Action + Clone>(
    app: &mut AppContext,
    context: &ContextPredicate,
    builder: fn(SettingsAction) -> T,
) {
    let _ = (context, builder);
    ToggleSettingActionPair::<T>::add_toggle_setting_action_pairs_as_bindings(Vec::new(), app);
}

const CONTENT_FONT_SIZE: f32 = 12.;
const ITEM_VERTICAL_SPACING: f32 = 24.;
/// There's a built-in 10px margin below the text input.
const BUILT_IN_TEXT_INPUT_MARGIN: f32 = 10.;
const SPACE_AFTER_TEXT_INPUT: f32 = ITEM_VERTICAL_SPACING - BUILT_IN_TEXT_INPUT_MARGIN;

/// This page lets users configure when they get asked to warpify a session. Some shell commands
/// are recognized by default. Users can add new shell commands, or prevent the default ones from
/// asking.
pub struct WarpifyPageView {
    page: PageType<Self>,
    /// This needs to mirror the length of SubshellSettings::added_remove_button_states.
    remove_added_command_button_states: Vec<MouseStateHandle>,
    add_added_commands_editor: ViewHandle<SubmittableTextInput>,
    /// This needs to mirror the length of SubshellSettings::denylisted_remove_button_states.
    remove_denylisted_command_button_states: Vec<MouseStateHandle>,
    add_denylisted_commands_editor: ViewHandle<SubmittableTextInput>,
}

impl WarpifyPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let warpify_settings_handle = WarpifySettings::handle(ctx);

        ctx.observe(&warpify_settings_handle, Self::update_button_states);
        ctx.subscribe_to_model(&warpify_settings_handle, move |me, model, event, ctx| {
            me.update_button_states(model, ctx);
            let _ = event;
            ctx.notify();
        });

        // Added commands can be specified by regex, while denied commands are strictly exact
        // match.
        let add_added_commands_editor = ctx.add_typed_action_view(|ctx| {
            let mut input =
                SubmittableTextInput::new(ctx).validate_on_edit(|regex| Regex::new(regex).is_ok());
            input.set_placeholder_text("command (supports regex)", ctx);
            input
        });

        ctx.subscribe_to_view(
            &add_added_commands_editor,
            Self::handle_added_command_editor_event,
        );

        let add_denylisted_commands_editor = ctx.add_typed_action_view(|ctx| {
            let mut input = SubmittableTextInput::new(ctx);
            input.set_placeholder_text("command (supports regex)", ctx);
            input
        });

        ctx.subscribe_to_view(
            &add_denylisted_commands_editor,
            Self::handle_denylisted_command_editor_event,
        );

        let mut instance = Self {
            page: Self::build_page(ctx),
            remove_added_command_button_states: Default::default(),
            add_added_commands_editor,
            remove_denylisted_command_button_states: Default::default(),
            add_denylisted_commands_editor,
        };

        instance.update_button_states(warpify_settings_handle, ctx);
        instance
    }

    fn build_page(_ctx: &mut ViewContext<Self>) -> PageType<Self> {
        let categories = vec![
            Category::new("", vec![Box::new(TitleWidget::default())]),
            Category::new("Subshells", vec![Box::new(SubshellsWidget::default())])
                .with_subtitle("Subshells supported: bash, zsh, and fish."),
        ];

        PageType::new_categorized(categories, None)
    }

    /// This method ensures each command in the SubshellSettings has a matching button state for
    /// its delete button in the View.
    fn update_button_states(
        &mut self,
        warpify_settings_handle: ModelHandle<WarpifySettings>,
        ctx: &mut ViewContext<Self>,
    ) {
        let warpify_settings = warpify_settings_handle.as_ref(ctx);
        self.remove_denylisted_command_button_states = warpify_settings
            .subshell_command_denylist
            .iter()
            .map(|_| Default::default())
            .collect();
        self.remove_added_command_button_states = warpify_settings
            .added_subshell_commands
            .iter()
            .map(|_| Default::default())
            .collect();
        ctx.notify();
    }

    fn handle_added_command_editor_event(
        &mut self,
        _handle: ViewHandle<SubmittableTextInput>,
        event: &SubmittableTextInputEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            SubmittableTextInputEvent::Submit(new_command) => {
                WarpifySettings::handle(ctx).update(ctx, |warpify_settings, ctx| {
                    warpify_settings.add_subshell_command(new_command, ctx);
                });
            }
            SubmittableTextInputEvent::Escape => ctx.emit(SettingsPageEvent::FocusModal),
        }
    }

    fn handle_denylisted_command_editor_event(
        &mut self,
        _handle: ViewHandle<SubmittableTextInput>,
        event: &SubmittableTextInputEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            SubmittableTextInputEvent::Submit(new_command) => {
                WarpifySettings::handle(ctx).update(ctx, |warpify_settings, ctx| {
                    warpify_settings.denylist_subshell_command(new_command, ctx);
                });
            }
            SubmittableTextInputEvent::Escape => ctx.emit(SettingsPageEvent::FocusModal),
        }
    }

    fn remove_denylisted_command(&self, index: usize, ctx: &mut ViewContext<Self>) {
        WarpifySettings::handle(ctx).update(ctx, |warpify, ctx| {
            warpify.remove_denylisted_subshell_command(index, ctx)
        });
    }

    fn remove_added_command(&self, index: usize, ctx: &mut ViewContext<Self>) {
        WarpifySettings::handle(ctx).update(ctx, |warpify, ctx| {
            warpify.remove_added_subshell_command(index, ctx)
        });
    }
}

impl Entity for WarpifyPageView {
    type Event = SettingsPageEvent;
}

fn build_sub_sub_title(title: &str, appearance: &Appearance) -> Container {
    appearance
        .ui_builder()
        .span(title.to_string())
        .with_style(UiComponentStyles {
            font_size: Some(CONTENT_FONT_SIZE),
            ..Default::default()
        })
        .build()
}

impl WarpifyPageView {
    /// Renders a title, a list of items that can be removed, and an input field to add new items.
    fn build_input_list<
        ListItem: Display,
        SettingsPageAction: Action + Clone,
        F: Fn(usize) -> SettingsPageAction,
        T: View,
    >(
        &self,
        title: &str,
        patterns: &[ListItem],
        mouse_states: &[MouseStateHandle],
        create_action: F,
        handle: &ViewHandle<T>,
        appearance: &Appearance,
    ) -> Container {
        let mut column = Flex::column();
        let mut title = build_sub_sub_title(title, appearance);

        if !patterns.is_empty() {
            title = title.with_padding_bottom(BUILT_IN_TEXT_INPUT_MARGIN);
        }

        column.add_child(title.finish());

        render_alternating_color_list(
            &mut column,
            patterns,
            mouse_states,
            create_action,
            appearance,
        );

        Container::new(
            column
                .with_child(
                    Container::new(ChildView::new(handle).finish())
                        .with_margin_bottom(SPACE_AFTER_TEXT_INPUT)
                        .finish(),
                )
                .finish(),
        )
    }
}

impl View for WarpifyPageView {
    fn ui_name() -> &'static str {
        "WarpifyPageView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum WarpifyPageAction {
    RemoveAddedCommand(usize),
    RemoveDenylistedCommand(usize),
    OpenUrl(String),
}

impl TypedActionView for WarpifyPageView {
    type Action = WarpifyPageAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        use WarpifyPageAction::*;
        match action {
            RemoveDenylistedCommand(index) => self.remove_denylisted_command(*index, ctx),
            RemoveAddedCommand(index) => self.remove_added_command(*index, ctx),
            OpenUrl(url) => {
                ctx.open_url(url.as_str());
            }
        }
    }
}

impl SettingsPageMeta for WarpifyPageView {
    fn section() -> SettingsSection {
        SettingsSection::Warpify
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

impl From<ViewHandle<WarpifyPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<WarpifyPageView>) -> Self {
        SettingsPageViewHandle::Warpify(view_handle)
    }
}

#[derive(Default)]
struct TitleWidget {
    learn_more_highlight_index: HighlightedHyperlink,
}

impl TitleWidget {
    fn render_top_of_page(&self, appearance: &Appearance, _app: &AppContext) -> Box<dyn Element> {
        let warpify_description = vec![
            FormattedTextFragment::plain_text(
                "Configure whether Warper attempts to “Warpify” (add support for blocks, \
                    input modes, etc) certain shells. ",
            ),
            FormattedTextFragment::hyperlink("Learn more", "about:blank"),
        ];

        let warpify_description = FormattedTextElement::new(
            FormattedText::new([FormattedTextLine::Line(warpify_description)]),
            CONTENT_FONT_SIZE,
            appearance.ui_font_family(),
            appearance.ui_font_family(),
            blended_colors::text_sub(appearance.theme(), appearance.theme().surface_1()),
            self.learn_more_highlight_index.clone(),
        )
        .with_hyperlink_font_color(appearance.theme().accent().into_solid())
        .register_default_click_handlers(|url, _, ctx| {
            ctx.open_url(&url.url);
        })
        .finish();

        Flex::column()
            .with_child(render_page_title("Warpify", HEADER_FONT_SIZE, appearance))
            .with_child(warpify_description)
            .finish()
    }
}

impl SettingsWidget for TitleWidget {
    type View = WarpifyPageView;

    fn search_terms(&self) -> &str {
        "ssh subshell warpify session"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        Container::new(self.render_top_of_page(appearance, app))
            .with_margin_bottom(ITEM_VERTICAL_SPACING)
            .finish()
    }
}

#[derive(Default)]
struct SubshellsWidget {}

impl SubshellsWidget {
    fn render_subshells_section(
        &self,
        view: &WarpifyPageView,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let mut column = Flex::column();

        let warpify_settings = WarpifySettings::as_ref(app);

        column.add_child(
            view.build_input_list(
                "Added commands",
                &warpify_settings.added_subshell_commands,
                &view.remove_added_command_button_states,
                WarpifyPageAction::RemoveAddedCommand,
                &view.add_added_commands_editor,
                appearance,
            )
            .finish(),
        );

        column.add_child(
            view.build_input_list(
                "Denylisted commands",
                &warpify_settings.subshell_command_denylist,
                &view.remove_denylisted_command_button_states,
                WarpifyPageAction::RemoveDenylistedCommand,
                &view.add_denylisted_commands_editor,
                appearance,
            )
            .with_margin_bottom(-BUILT_IN_TEXT_INPUT_MARGIN)
            .finish(),
        );

        column.finish()
    }
}

impl SettingsWidget for SubshellsWidget {
    type View = WarpifyPageView;

    fn search_terms(&self) -> &str {
        "warpify subshell"
    }

    fn render(
        &self,
        view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        Container::new(self.render_subshells_section(view, appearance, app))
            .with_margin_bottom(ITEM_VERTICAL_SPACING)
            .finish()
    }
}
