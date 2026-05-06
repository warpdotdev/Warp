use std::{cell::RefCell, collections::HashMap};

use crate::t;

use super::{
    settings_page::{
        render_dropdown_item, LocalOnlyIconState, MatchData, PageType, SettingsPageEvent,
        SettingsPageMeta, SettingsPageViewHandle, SettingsWidget,
    },
    SettingsSection,
};
use crate::{
    appearance::Appearance,
    report_if_error,
    settings::{LanguageSettings, UILanguage, UILanguageSetting},
    view_components::{Dropdown, DropdownItem},
};
use settings::Setting as _;
use warpui::{
    elements::{Element, MouseStateHandle},
    AppContext, Entity, SingletonEntity, View, ViewContext, ViewHandle,
};

const LANGUAGE_DROPDOWN_WIDTH: f32 = 200.;

#[derive(Clone, Debug)]
pub enum LanguagePageAction {
    SetUILanguage(UILanguage),
}

pub struct LanguageSettingsPageView {
    page: PageType<Self>,
    ui_language_dropdown: ViewHandle<Dropdown<LanguagePageAction>>,
    local_only_icon_tooltip_states: RefCell<HashMap<String, MouseStateHandle>>,
}

impl LanguageSettingsPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let ui_language_dropdown = ctx.add_typed_action_view(|ctx| {
            let mut dropdown = Dropdown::new(ctx);
            dropdown.set_top_bar_max_width(LANGUAGE_DROPDOWN_WIDTH);
            dropdown.set_menu_width(LANGUAGE_DROPDOWN_WIDTH, ctx);

            let values = vec![UILanguage::English, UILanguage::ChineseSimplified];
            let current_value = *<LanguageSettings as SingletonEntity>::as_ref(ctx).ui_language.value();
            let selected_index = values
                .iter()
                .position(|v| *v == current_value)
                .unwrap_or(0);

            dropdown.add_items(
                values
                    .into_iter()
                    .map(|val| {
                        DropdownItem::new(val.label(), LanguagePageAction::SetUILanguage(val))
                    })
                    .collect(),
                ctx,
            );
            dropdown.set_selected_by_index(selected_index, ctx);
            dropdown
        });

        LanguageSettingsPageView {
            page: PageType::new_monolith(LanguageWidget::default(), None, false),
            ui_language_dropdown,
            local_only_icon_tooltip_states: RefCell::new(HashMap::new()),
        }
    }

    fn set_ui_language(&mut self, lang: UILanguage, ctx: &mut ViewContext<Self>) {
        <LanguageSettings as SingletonEntity>::handle(ctx).update(ctx, |settings, ctx| {
            report_if_error!(settings.ui_language.set_value(lang, ctx));
        });
        self.ui_language_dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_selected_by_name(lang.label(), ctx);
        });
    }
}

impl Entity for LanguageSettingsPageView {
    type Event = SettingsPageEvent;
}

impl View for LanguageSettingsPageView {
    fn ui_name() -> &'static str {
        "LanguageSettingsPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

impl warpui::TypedActionView for LanguageSettingsPageView {
    type Action = LanguagePageAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            LanguagePageAction::SetUILanguage(lang) => self.set_ui_language(*lang, ctx),
        }
    }
}

impl SettingsPageMeta for LanguageSettingsPageView {
    fn section() -> SettingsSection {
        SettingsSection::Language
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

impl From<ViewHandle<LanguageSettingsPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<LanguageSettingsPageView>) -> Self {
        SettingsPageViewHandle::Language(view_handle)
    }
}

#[derive(Default)]
struct LanguageWidget;

impl SettingsWidget for LanguageWidget {
    type View = LanguageSettingsPageView;

    fn search_terms(&self) -> &str {
        "language ui display chinese english 中文 语言"
    }

    fn render(
        &self,
        view: &LanguageSettingsPageView,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let label = t!(app, "Language", "界面语言");
        let description = t!(app, "UI display language", "选择 Warp 界面的显示语言");
        render_dropdown_item(
            appearance,
            label,
            Some(description),
            None,
            LocalOnlyIconState::for_setting(
                UILanguageSetting::storage_key(),
                UILanguageSetting::sync_to_cloud(),
                &mut view.local_only_icon_tooltip_states.borrow_mut(),
                app,
            ),
            None,
            &view.ui_language_dropdown,
        )
    }
}
