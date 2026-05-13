use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ai::skills::SkillProvider;
use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::color::internal_colors;
use warpui::{
    elements::{
        Border, ChildView, Clipped, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox,
        Container, CornerRadius, CrossAxisAlignment, Element, Fill as ElementFill, Flex, Hoverable,
        MainAxisSize, MouseStateHandle, Padding, ParentElement, Radius, SavePosition, ScrollTarget,
        ScrollToPositionMode, ScrollbarWidth, Shrinkable, Text,
    },
    platform::Cursor,
    text_layout::ClipConfig,
    ui_components::{
        components::UiComponentStyles,
        segmented_control::{
            LabelConfig, RenderableOptionConfig, SegmentedControl, SegmentedControlEvent,
        },
    },
    AppContext, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use crate::ai::skills::{
    SkillInventoryDuplicate, SkillInventoryItem, SkillManager, SkillManagerEvent,
};
use crate::editor::{
    EditorOptions, EditorView, Event as EditorEvent, PropagateAndNoOpNavigationKeys,
    PropagateHorizontalNavigationKeys, TextOptions,
};

const PANEL_PADDING: f32 = 8.0;
const ROW_PADDING_VERTICAL: f32 = 5.0;
const ROW_PADDING_HORIZONTAL: f32 = 8.0;
const FONT_SIZE: f32 = 13.0;
const SEARCH_FONT_SIZE: f32 = 14.0;
const META_FONT_SIZE: f32 = 11.0;
const FILTER_BUTTON_HEIGHT: f32 = 24.0;
const FILTER_LABEL_WIDTH: f32 = 44.0;
const FILTER_ALL_WIDTH: f32 = 44.0;
const FILTER_PROVIDER_WIDTH: f32 = 66.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProviderFilterOption {
    All,
    Provider(SkillProvider),
}

impl ProviderFilterOption {
    fn from_provider(provider: Option<SkillProvider>) -> Self {
        match provider {
            Some(provider) => Self::Provider(provider),
            None => Self::All,
        }
    }

    fn provider(self) -> Option<SkillProvider> {
        match self {
            Self::All => None,
            Self::Provider(provider) => Some(provider),
        }
    }
}

#[derive(Clone, Debug)]
pub enum SkillManagerPanelAction {
    SelectProviderFilter(Option<SkillProvider>),
    EditSkill(PathBuf),
}

#[derive(Clone, Debug)]
pub enum SkillManagerPanelEvent {
    OpenSkillFile { path: PathBuf },
}

pub struct SkillManagerPanel {
    selected_path: Option<PathBuf>,
    provider_filter: Option<SkillProvider>,
    query_editor: ViewHandle<EditorView>,
    provider_filter_control: ViewHandle<SegmentedControl<ProviderFilterOption>>,
    row_mouse_states: RefCell<HashMap<PathBuf, MouseStateHandle>>,
    list_scroll_state: ClippedScrollStateHandle,
}

impl SkillManagerPanel {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let query_editor = ctx.add_typed_action_view(|ctx| {
            let options = EditorOptions {
                text: TextOptions::ui_text(Some(SEARCH_FONT_SIZE), Appearance::as_ref(ctx)),
                propagate_and_no_op_vertical_navigation_keys:
                    PropagateAndNoOpNavigationKeys::AtBoundary,
                propagate_horizontal_navigation_keys: PropagateHorizontalNavigationKeys::Always,
                single_line: true,
                clear_selections_on_blur: true,
                convert_newline_to_space: true,
                ..Default::default()
            };
            let mut editor = EditorView::new(options, ctx);
            editor.set_placeholder_text(crate::t!("skill-manager-search-placeholder"), ctx);
            editor
        });

        let provider_filter_options = Self::provider_filter_options(ctx);
        let provider_filter_control = ctx.add_typed_action_view(move |ctx| {
            SegmentedControl::new(
                provider_filter_options,
                Self::render_provider_filter_option,
                ProviderFilterOption::All,
                Self::provider_filter_control_styles(ctx),
            )
        });

        ctx.subscribe_to_view(&query_editor, |me, _, event, ctx| {
            if matches!(
                event,
                EditorEvent::Edited(_)
                    | EditorEvent::BufferReplaced
                    | EditorEvent::BufferReinitialized
            ) {
                me.scroll_selected_path_into_view(ctx);
                ctx.notify();
            }
        });

        ctx.subscribe_to_view(&provider_filter_control, |_, _handle, event, ctx| {
            let SegmentedControlEvent::OptionSelected(filter) = event;
            ctx.dispatch_typed_action(&SkillManagerPanelAction::SelectProviderFilter(
                filter.provider(),
            ));
        });

        ctx.subscribe_to_model(&Appearance::handle(ctx), |me, _, _, ctx| {
            me.provider_filter_control.update(ctx, |control, ctx| {
                control.set_styles(Self::provider_filter_control_styles(ctx), ctx)
            });
            ctx.notify();
        });

        let skill_manager = SkillManager::handle(ctx);
        ctx.subscribe_to_model(&skill_manager, |me, _manager, event, ctx| match event {
            SkillManagerEvent::InventoryChanged => {
                me.update_provider_filter_options(ctx);
                me.scroll_selected_path_into_view(ctx);
                ctx.notify();
            }
        });

        Self {
            selected_path: None,
            provider_filter: None,
            query_editor,
            provider_filter_control,
            row_mouse_states: RefCell::new(HashMap::new()),
            list_scroll_state: ClippedScrollStateHandle::default(),
        }
    }

    fn query(&self, app: &AppContext) -> String {
        self.query_editor
            .as_ref(app)
            .buffer_text(app)
            .trim()
            .to_lowercase()
    }

    fn filtered_items(&self, app: &AppContext) -> Vec<SkillInventoryItem> {
        let query = self.query(app);
        SkillManager::as_ref(app)
            .list_skill_inventory(app)
            .into_iter()
            .filter_map(|item| {
                let duplicates = item
                    .duplicates
                    .into_iter()
                    .filter(|duplicate| {
                        self.provider_filter
                            .is_none_or(|provider| duplicate.provider == provider)
                            && (query.is_empty()
                                || duplicate.name.to_lowercase().contains(&query)
                                || duplicate.description.to_lowercase().contains(&query)
                                || duplicate
                                    .path
                                    .display()
                                    .to_string()
                                    .to_lowercase()
                                    .contains(&query))
                    })
                    .collect::<Vec<_>>();

                let default_skill = duplicates.first()?.clone();
                Some(SkillInventoryItem {
                    name: item.name,
                    default_skill,
                    duplicates,
                })
            })
            .collect()
    }

    fn selected_path_is_visible(&self, path: &Path, app: &AppContext) -> bool {
        self.filtered_items(app)
            .iter()
            .flat_map(|item| item.duplicates.iter())
            .any(|duplicate| duplicate.path.as_path() == path)
    }

    fn skill_row_position_id(path: &Path) -> String {
        format!("skill-manager-row:{}", path.to_string_lossy())
    }

    fn scroll_selected_path_into_view(&self, app: &AppContext) {
        let Some(path) = self.selected_path.as_deref() else {
            return;
        };
        if !self.selected_path_is_visible(path, app) {
            return;
        }

        self.list_scroll_state.scroll_to_position(ScrollTarget {
            position_id: Self::skill_row_position_id(path),
            mode: ScrollToPositionMode::FullyIntoView,
        });
    }

    fn row_mouse_state_for(&self, path: &Path) -> MouseStateHandle {
        self.row_mouse_states
            .borrow_mut()
            .entry(path.to_path_buf())
            .or_default()
            .clone()
    }

    fn render_label(
        text: impl Into<String>,
        appearance: &Appearance,
        font_size: f32,
        color: impl Into<pathfinder_color::ColorU>,
    ) -> Box<dyn Element> {
        Text::new_inline(text.into(), appearance.ui_font_family(), font_size)
            .with_color(color.into())
            .with_clip(ClipConfig::ellipsis())
            .finish()
    }

    fn provider_filter_options(app: &AppContext) -> Vec<ProviderFilterOption> {
        let mut providers = SkillManager::as_ref(app)
            .list_skill_inventory(app)
            .iter()
            .flat_map(|item| item.duplicates.iter().map(|duplicate| duplicate.provider))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        providers.sort_by_key(|provider| provider.to_string());

        let mut options = vec![ProviderFilterOption::All];
        options.extend(providers.into_iter().map(ProviderFilterOption::Provider));
        options
    }

    fn provider_filter_control_styles(app: &AppContext) -> UiComponentStyles {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        UiComponentStyles {
            font_family_id: Some(appearance.ui_font_family()),
            font_size: Some(META_FONT_SIZE + 1.0),
            height: Some(FILTER_BUTTON_HEIGHT),
            border_radius: Some(CornerRadius::with_all(Radius::Pixels(6.0))),
            border_width: Some(1.0),
            border_color: Some(ElementFill::Solid(theme.surface_3().into())),
            background: Some(ElementFill::Solid(
                internal_colors::fg_overlay_1(theme).into(),
            )),
            ..Default::default()
        }
    }

    fn render_provider_filter_option(
        option: ProviderFilterOption,
        is_selected: bool,
        app: &AppContext,
    ) -> Option<RenderableOptionConfig> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let text_color = if is_selected {
            theme.main_text_color(theme.background())
        } else {
            theme.sub_text_color(theme.background())
        };
        let (label, width_override) = match option {
            ProviderFilterOption::All => (
                crate::t!("skill-manager-filter-all").into(),
                FILTER_ALL_WIDTH,
            ),
            ProviderFilterOption::Provider(provider) => {
                (provider.to_string().into(), FILTER_PROVIDER_WIDTH)
            }
        };

        Some(RenderableOptionConfig {
            icon_path: "",
            icon_color: text_color.into(),
            label: Some(LabelConfig {
                label,
                width_override: Some(width_override),
                color: text_color.into(),
            }),
            tooltip: None,
            background: if is_selected {
                ElementFill::Solid(internal_colors::fg_overlay_3(theme).into())
            } else {
                ElementFill::None
            },
        })
    }

    fn update_provider_filter_options(&mut self, ctx: &mut ViewContext<Self>) {
        let options = Self::provider_filter_options(ctx);
        let selected_filter = ProviderFilterOption::from_provider(self.provider_filter);
        if !options.contains(&selected_filter) {
            self.provider_filter = None;
        }
        let selected_filter = ProviderFilterOption::from_provider(self.provider_filter);

        self.provider_filter_control.update(ctx, |control, ctx| {
            control.update_options(options, ctx);
            control.set_selected_option(selected_filter, ctx);
        });
    }

    fn render_search_input(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let search_row = Shrinkable::new(
            1.0,
            Clipped::new(ChildView::new(&self.query_editor).finish()).finish(),
        )
        .finish();

        Container::new(search_row)
            .with_padding(Padding::uniform(6.0).with_left(12.0).with_right(12.0))
            .with_border(Border::all(1.0).with_border_fill(theme.surface_3()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .finish()
    }

    fn render_filter_rows(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(8.0)
            .with_child(
                ConstrainedBox::new(Self::render_label(
                    crate::t!("skill-manager-filter-provider"),
                    appearance,
                    META_FONT_SIZE,
                    theme.sub_text_color(theme.background()),
                ))
                .with_width(FILTER_LABEL_WIDTH)
                .finish(),
            )
            .with_child(ChildView::new(&self.provider_filter_control).finish())
            .finish()
    }

    fn render_skill_row(
        &self,
        duplicate: &SkillInventoryDuplicate,
        is_selected: bool,
        is_default: bool,
        has_duplicates: bool,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let path = duplicate.path.display().to_string();
        let mut meta = format!("{} · {}", duplicate.provider, duplicate.scope);
        if has_duplicates {
            if is_default {
                meta.push_str(" · ");
                meta.push_str(&crate::t!("skill-manager-meta-default"));
            } else {
                meta.push_str(" · ");
                meta.push_str(&crate::t!("skill-manager-meta-duplicate"));
            }
        }

        let title = Self::render_label(
            duplicate.name.clone(),
            appearance,
            FONT_SIZE,
            theme.main_text_color(theme.background()),
        );
        let description = Self::render_label(
            duplicate.description.clone(),
            appearance,
            META_FONT_SIZE,
            theme.sub_text_color(theme.background()),
        );
        let meta = Self::render_label(
            meta,
            appearance,
            META_FONT_SIZE,
            theme.sub_text_color(theme.background()),
        );
        let path = Self::render_label(
            path,
            appearance,
            META_FONT_SIZE,
            theme.sub_text_color(theme.background()),
        );

        let action = SkillManagerPanelAction::EditSkill(duplicate.path.clone());
        let position_id = Self::skill_row_position_id(&duplicate.path);
        let state = self.row_mouse_state_for(&duplicate.path);
        let row = Hoverable::new(state, move |mouse| {
            let background = if is_selected && mouse.is_hovered() {
                Some(internal_colors::fg_overlay_4(theme))
            } else if is_selected {
                Some(internal_colors::fg_overlay_3(theme))
            } else if mouse.is_hovered() {
                Some(internal_colors::fg_overlay_2(theme))
            } else {
                None
            };
            let mut row = Container::new(
                Flex::column()
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_spacing(2.0)
                    .with_child(title)
                    .with_child(description)
                    .with_child(meta)
                    .with_child(path)
                    .finish(),
            )
            .with_padding_top(ROW_PADDING_VERTICAL)
            .with_padding_bottom(ROW_PADDING_VERTICAL)
            .with_padding_left(ROW_PADDING_HORIZONTAL)
            .with_padding_right(ROW_PADDING_HORIZONTAL)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)));
            if let Some(background) = background {
                row = row.with_background(background);
            }
            row.finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
        })
        .finish();

        SavePosition::new(row, &position_id).finish()
    }

    fn render_skill_list(
        &self,
        items: &[SkillInventoryItem],
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        if items.is_empty() {
            return Container::new(Self::render_label(
                crate::t!("skill-manager-empty"),
                appearance,
                FONT_SIZE,
                appearance
                    .theme()
                    .sub_text_color(appearance.theme().background()),
            ))
            .with_uniform_padding(12.0)
            .finish();
        }

        let mut rows = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(2.0);
        for item in items {
            let has_duplicates = item.has_duplicates();
            for duplicate in &item.duplicates {
                let is_selected = self
                    .selected_path
                    .as_deref()
                    .is_some_and(|path| path == duplicate.path.as_path());
                let is_default = duplicate.path == item.default_skill.path;
                rows.add_child(self.render_skill_row(
                    duplicate,
                    is_selected,
                    is_default,
                    has_duplicates,
                    appearance,
                ));
            }
        }

        let theme = appearance.theme();
        ClippedScrollable::vertical(
            self.list_scroll_state.clone(),
            rows.finish(),
            ScrollbarWidth::Auto,
            theme.disabled_text_color(theme.background()).into(),
            theme.main_text_color(theme.background()).into(),
            ElementFill::None,
        )
        .with_overlayed_scrollbar()
        .finish()
    }
}

impl TypedActionView for SkillManagerPanel {
    type Action = SkillManagerPanelAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            SkillManagerPanelAction::SelectProviderFilter(provider) => {
                self.provider_filter = *provider;
                self.provider_filter_control.update(ctx, |control, ctx| {
                    control
                        .set_selected_option(ProviderFilterOption::from_provider(*provider), ctx);
                });
                self.scroll_selected_path_into_view(ctx);
                ctx.notify();
            }
            SkillManagerPanelAction::EditSkill(path) => {
                self.selected_path = Some(path.clone());
                self.scroll_selected_path_into_view(ctx);
                ctx.emit(SkillManagerPanelEvent::OpenSkillFile { path: path.clone() });
                ctx.notify();
            }
        }
    }
}

impl View for SkillManagerPanel {
    fn ui_name() -> &'static str {
        "SkillManagerPanel"
    }

    fn on_focus(&mut self, _focus_ctx: &warpui::FocusContext, ctx: &mut ViewContext<Self>) {
        ctx.focus(&self.query_editor);
        self.scroll_selected_path_into_view(ctx);
        ctx.notify();
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let items = self.filtered_items(app);

        Container::new(
            Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_spacing(8.0)
                .with_child(self.render_search_input(appearance))
                .with_child(self.render_filter_rows(appearance))
                .with_child(
                    Shrinkable::new(1.0, self.render_skill_list(&items, appearance)).finish(),
                )
                .finish(),
        )
        .with_uniform_padding(PANEL_PADDING)
        .finish()
    }
}

impl Entity for SkillManagerPanel {
    type Event = SkillManagerPanelEvent;
}
