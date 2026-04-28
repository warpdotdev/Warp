use std::fmt::Debug;

use pathfinder_color::ColorU;
use warpui::{
    elements::{
        Border, ChildAnchor, ChildView, ConstrainedBox, Container, Element, Icon,
        MainAxisAlignment, MainAxisSize, MouseStateHandle, OffsetPositioning, ParentElement,
        PositionedElementAnchor, PositionedElementOffsetBounds, SavePosition, Stack,
    },
    fonts::FamilyId,
    geometry::vector::vec2f,
    scene::DropShadow,
    ui_components::{
        button::{ButtonVariant, TextAndIcon, TextAndIconAlignment},
        components::{Coords, UiComponent, UiComponentStyles},
    },
    Action, AppContext, BlurContext, Entity, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle, WeakViewHandle,
};

use crate::{
    appearance::Appearance,
    menu::{Event as MenuEvent, Menu, MenuItem, MenuItemFields, MenuVariant},
};

pub const TOP_MENU_BAR_HEIGHT: f32 = 30.;
pub const TOP_MENU_BAR_MAX_WIDTH: f32 = 190.;
pub const DROPDOWN_PADDING: f32 = 6.;

pub type MenuHeaderTextFormatter = Box<dyn Fn(&str) -> String>;

#[derive(Clone, Default)]
pub enum DropdownStyle {
    #[default]
    Secondary,
    /// No border, smaller text, smaller padding
    #[allow(dead_code)]
    Naked,
    /// Similar to Secondary but with ActionButton-like hover behavior:
    /// background fill on hover instead of border color change.
    /// TODO this should probably replace the default `Secondary` theme
    ActionButtonSecondary,
}

impl DropdownStyle {
    fn ui_component_styles(&self) -> UiComponentStyles {
        match self {
            DropdownStyle::Secondary | DropdownStyle::ActionButtonSecondary => UiComponentStyles {
                padding: Some(Coords {
                    top: 5.,
                    bottom: 5.,
                    left: 8.,
                    right: 8.,
                }),
                ..Default::default()
            },
            DropdownStyle::Naked => UiComponentStyles {
                ..Default::default()
            },
        }
    }
}

/// A dropdown menu view. The view renders each DropdownItem. When a menu item is clicked,
/// on_click_action_name is dispatched, with the value of the corresponding menu item.
pub struct Dropdown<A: Action + Clone> {
    is_expanded: bool,
    disabled: bool,
    top_bar_mouse_state: MouseStateHandle,
    top_bar_max_width: f32,
    element_anchor: PositionedElementAnchor,
    child_anchor: ChildAnchor,
    main_axis_size: MainAxisSize,

    dropdown: ViewHandle<Menu<DropdownAction<A>>>,
    selected_item: Option<MenuItem<DropdownAction<A>>>,
    // Function for overriding the default closed-state text (the selected item)
    menu_header_text_override: Option<MenuHeaderTextFormatter>,
    self_handle: WeakViewHandle<Self>,
    style: DropdownStyle,
    use_drop_shadow: bool,
    font_color: Option<ColorU>,
    font_size: Option<f32>,
    padding: Option<Coords>,
    vertical_margin: f32,
    top_bar_height: f32,
}

#[derive(Clone)]
pub struct DropdownItem<A: Action + Clone> {
    /// Text to display for the item
    pub display_text: String,
    /// Constructor for the typed action object
    action: A,
    /// Custom font for the dropdown item
    family_id: Option<FamilyId>,
}

impl<A> DropdownItem<A>
where
    A: Action + Clone,
{
    pub fn new<S>(display_text: S, action: A) -> Self
    where
        S: Into<String>,
    {
        Self {
            display_text: display_text.into(),
            action,
            family_id: None,
        }
    }

    // Override the font of the drop down item. If this is not set, the default will
    // be the ui_font_family.
    pub fn with_font_override(mut self, family_id: FamilyId) -> Self {
        self.family_id = Some(family_id);
        self
    }
}

impl<A> From<&DropdownItem<A>> for MenuItem<DropdownAction<A>>
where
    A: Action + Clone,
{
    fn from(dropdown_item: &DropdownItem<A>) -> MenuItem<DropdownAction<A>> {
        let menu_item = MenuItemFields::new(dropdown_item.display_text.clone())
            .with_on_select_action(DropdownAction::SelectActionAndClose(
                dropdown_item.action.clone(),
            ));
        if let Some(family_id) = dropdown_item.family_id {
            menu_item.with_font_override(family_id).into_item()
        } else {
            menu_item.into_item()
        }
    }
}

impl<A> From<A> for DropdownAction<A>
where
    A: Action + Clone,
{
    fn from(action: A) -> DropdownAction<A> {
        DropdownAction::SelectActionAndClose(action)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DropdownAction<A: Action + Clone> {
    Focus(usize),
    Close,
    SelectActionAndClose(A),
    ToggleExpanded,
}

pub enum DropdownEvent {
    ToggleExpanded,
    Close,
}

impl<A> Dropdown<A>
where
    A: Action + Clone,
{
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let dropdown = ctx.add_typed_action_view(|ctx| {
            let theme = Appearance::as_ref(ctx).theme();
            Menu::new()
                .with_menu_variant(MenuVariant::scrollable())
                .with_border(Border::all(1.).with_border_fill(theme.outline()))
                .prevent_interaction_with_other_elements()
        });
        ctx.subscribe_to_view(&dropdown, move |me, _, event, ctx| {
            me.handle_menu_event(event, ctx);
        });

        Self {
            main_axis_size: MainAxisSize::Max,
            is_expanded: false,
            disabled: false,
            dropdown,
            top_bar_mouse_state: Default::default(),
            top_bar_max_width: TOP_MENU_BAR_MAX_WIDTH,
            selected_item: None,
            menu_header_text_override: None,
            self_handle: ctx.handle(),
            style: Default::default(),
            element_anchor: PositionedElementAnchor::BottomLeft,
            child_anchor: ChildAnchor::TopLeft,
            use_drop_shadow: false,
            font_color: None,
            font_size: None,
            padding: None,
            vertical_margin: DROPDOWN_PADDING,
            top_bar_height: TOP_MENU_BAR_HEIGHT,
        }
    }

    pub fn with_drop_shadow(mut self) -> Self {
        self.use_drop_shadow = true;
        self
    }

    pub fn set_font_color(&mut self, color: ColorU, ctx: &mut ViewContext<Self>) {
        self.font_color = Some(color);
        ctx.notify();
    }

    pub fn set_font_size(&mut self, size: f32, ctx: &mut ViewContext<Self>) {
        self.font_size = Some(size);
        ctx.notify();
    }

    pub fn set_vertical_margin(&mut self, margin: f32, ctx: &mut ViewContext<Self>) {
        self.vertical_margin = margin;
        ctx.notify();
    }

    pub fn set_top_bar_height(&mut self, height: f32, ctx: &mut ViewContext<Self>) {
        self.top_bar_height = height;
        ctx.notify();
    }

    pub fn set_padding(&mut self, padding: Coords, ctx: &mut ViewContext<Self>) {
        self.padding = Some(padding);
        ctx.notify();
    }

    #[allow(dead_code)]
    pub fn set_style(&mut self, style: DropdownStyle, ctx: &mut ViewContext<Self>) {
        self.style = style;
        ctx.notify();
    }

    /// Set the main_axis_size behavior for the dropdown header button.
    ///
    /// Default is MainAxisSize::Max, set to MainAxisSize::Min if you want to wrap the dropdown to
    /// the text that's filling it.
    pub fn set_main_axis_size(
        &mut self,
        main_axis_size: MainAxisSize,
        ctx: &mut ViewContext<Self>,
    ) {
        self.main_axis_size = main_axis_size;
        ctx.notify();
    }

    pub fn set_menu_header_text_override<F>(&mut self, formatter: F)
    where
        F: Fn(&str) -> String + 'static,
    {
        self.menu_header_text_override = Some(Box::new(formatter));
    }

    pub fn set_menu_position(
        &mut self,
        element_anchor: PositionedElementAnchor,
        child_anchor: ChildAnchor,
        ctx: &mut ViewContext<Self>,
    ) {
        self.element_anchor = element_anchor;
        self.child_anchor = child_anchor;
        ctx.notify();
    }

    pub fn add_items(&mut self, items: Vec<DropdownItem<A>>, ctx: &mut ViewContext<Self>) {
        self.dropdown.update(ctx, |dropdown, ctx| {
            dropdown.add_items(items.iter().map(|item| item.into()));
            ctx.notify();
        });
        ctx.notify();
    }

    pub fn is_focused(&self, ctx: &AppContext) -> bool {
        let Some(handle) = self.self_handle.upgrade(ctx) else {
            return false;
        };

        if handle.is_focused(ctx) {
            return true;
        }

        if self.dropdown.is_focused(ctx) {
            return true;
        }

        false
    }

    pub fn set_items(&mut self, items: Vec<DropdownItem<A>>, ctx: &mut ViewContext<Self>) {
        self.dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_items(items.iter().map(|item| item.into()), ctx);
        });
        ctx.notify();
    }

    // Most dropdowns don't need to use rich menu features like separators, indents, and submenus.
    // But some do and, for those, we expose a "rich" item API.
    pub fn set_rich_items(
        &mut self,
        items: impl IntoIterator<Item = MenuItem<DropdownAction<A>>>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_items(items, ctx);
        });
        ctx.notify();
    }

    pub fn set_disabled(&mut self, ctx: &mut ViewContext<Self>) {
        self.disabled = true;
        ctx.notify();
    }

    pub fn set_enabled(&mut self, ctx: &mut ViewContext<Self>) {
        self.disabled = false;
        ctx.notify();
    }

    /// Select the item with the given name. If no such item exists, this clears the selection.
    pub fn set_selected_by_name(
        &mut self,
        selected_item: impl AsRef<str>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_selected_by_name(selected_item, ctx);
            ctx.notify();
        });
        self.selected_item = self.selected_item(ctx);
        ctx.notify();
    }

    /// Select the item at the given index. If the index is out of bounds, this clears the selection.
    pub fn set_selected_by_index(&mut self, selected_index: usize, ctx: &mut ViewContext<Self>) {
        self.dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_selected_by_index(selected_index, ctx);
            ctx.notify();
        });
        self.selected_item = self.selected_item(ctx);
        ctx.notify();
    }

    /// Select the dropdown item whose on-select action equals the given action. If no such item exists,
    /// this clears the selection.
    ///
    /// This is primarily useful when items are dynamically generated and correspond to some backing data that's captured by the action.
    pub fn set_selected_by_action(&mut self, action: A, ctx: &mut ViewContext<Self>)
    where
        A: PartialEq,
    {
        self.dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_selected_by_action(&DropdownAction::SelectActionAndClose(action), ctx);
            ctx.notify();
        });
        self.selected_item = self.selected_item(ctx);
        ctx.notify();
    }

    pub fn set_selected_to_none(&mut self, ctx: &mut ViewContext<Self>) {
        self.selected_item = None;
        ctx.notify();
    }

    pub fn set_top_bar_max_width(&mut self, max_width: f32) {
        self.top_bar_max_width = max_width;
    }

    pub fn set_menu_width(&mut self, width: f32, ctx: &mut ViewContext<Self>) {
        self.dropdown.update(ctx, |menu, ctx| {
            menu.set_width(width);
            ctx.notify();
        })
    }

    pub fn set_menu_max_height(&mut self, height: f32, ctx: &mut ViewContext<Self>) {
        self.dropdown.update(ctx, |menu, ctx| {
            menu.set_height(height);
            ctx.notify();
        })
    }

    fn selected_item(&self, ctx: &mut ViewContext<Self>) -> Option<MenuItem<DropdownAction<A>>> {
        self.dropdown
            .read(ctx, |dropdown, _| dropdown.selected_item())
    }

    fn focus(&mut self, _delta: usize, ctx: &mut ViewContext<Self>) {
        ctx.focus(&self.dropdown);
        ctx.notify();
    }

    fn select_action_and_close(&mut self, action: &A, ctx: &mut ViewContext<Self>) {
        ctx.dispatch_typed_action(action);
        self.close(ctx);
    }

    fn close(&mut self, ctx: &mut ViewContext<Self>) {
        self.is_expanded = false;
        ctx.emit(DropdownEvent::Close);
        ctx.notify();
    }

    pub fn toggle_expanded(&mut self, ctx: &mut ViewContext<Self>) {
        self.is_expanded = !self.is_expanded;
        if self.is_expanded {
            ctx.focus(&self.dropdown);
            ctx.emit(DropdownEvent::ToggleExpanded);
        }
        ctx.notify();
    }

    fn render_top_bar(&self, appearance: &Appearance) -> Box<dyn Element> {
        let icon_path = "bundled/svg/chevron-down.svg";

        let (selected_item_text, font_family_id) = match self.selected_item.clone() {
            Some(MenuItem::Item(fields)) => {
                let label = fields.label();
                let text = if let Some(formatter) = &self.menu_header_text_override {
                    formatter(label)
                } else {
                    label.to_string()
                };
                (text, fields.override_font_family())
            }
            _ => (String::new(), None),
        };
        let mut top_bar = appearance
            .ui_builder()
            .button(
                match self.style {
                    DropdownStyle::Secondary => ButtonVariant::Outlined,
                    DropdownStyle::Naked => ButtonVariant::Text,
                    DropdownStyle::ActionButtonSecondary => ButtonVariant::Secondary,
                },
                self.top_bar_mouse_state.clone(),
            )
            .with_text_and_icon_label(
                TextAndIcon::new(
                    TextAndIconAlignment::TextFirst,
                    selected_item_text,
                    Icon::new(
                        icon_path,
                        self.font_color
                            .unwrap_or_else(|| appearance.theme().active_ui_text_color().into()),
                    ),
                    self.main_axis_size,
                    MainAxisAlignment::SpaceBetween,
                    vec2f(15., 15.),
                )
                .with_inner_padding(match self.style {
                    DropdownStyle::Secondary | DropdownStyle::ActionButtonSecondary => 10.,
                    DropdownStyle::Naked => 6.,
                }),
            )
            .with_style(self.style.ui_component_styles())
            .with_style(UiComponentStyles {
                font_color: self.font_color,
                font_size: self.font_size,
                padding: self.padding,
                ..Default::default()
            })
            .set_clicked_styles(None);

        if self.disabled {
            top_bar = top_bar.disabled();
        }

        if let Some(font_family_id) = font_family_id {
            top_bar =
                top_bar.with_style(UiComponentStyles::default().set_font_family_id(font_family_id))
        }

        let top_bar_element = top_bar.build().on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(DropdownAction::<A>::ToggleExpanded);
        });

        SavePosition::new(
            Container::new(
                ConstrainedBox::new(top_bar_element.finish())
                    .with_max_width(self.top_bar_max_width)
                    .with_height(self.top_bar_height)
                    .finish(),
            )
            .finish(),
            &self.top_bar_label(),
        )
        .finish()
    }

    fn top_bar_label(&self) -> String {
        format!("dropdown_top_bar_{}", self.dropdown.id())
    }

    fn handle_menu_event(&mut self, event: &MenuEvent, ctx: &mut ViewContext<Self>) {
        match event {
            MenuEvent::Close { via_select_item: _ } => self.close(ctx),
            MenuEvent::ItemSelected => {
                self.selected_item = self.selected_item(ctx);
                ctx.notify();
            }
            MenuEvent::ItemHovered => {}
        }
    }
}

impl<A> Entity for Dropdown<A>
where
    A: Action + Clone,
{
    type Event = DropdownEvent;
}

impl<A> TypedActionView for Dropdown<A>
where
    A: Action + Clone,
{
    type Action = DropdownAction<A>;

    fn handle_action(&mut self, action: &DropdownAction<A>, ctx: &mut ViewContext<Self>) {
        match action {
            DropdownAction::Focus(delta) => self.focus(*delta, ctx),
            DropdownAction::Close => self.close(ctx),
            DropdownAction::SelectActionAndClose(action) => {
                self.select_action_and_close(action, ctx)
            }
            DropdownAction::ToggleExpanded => self.toggle_expanded(ctx),
        }
    }
}

impl<A> View for Dropdown<A>
where
    A: Action + Clone,
{
    fn ui_name() -> &'static str {
        "Dropdown"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let mut dropdown_stack = Stack::new().with_child(self.render_top_bar(appearance));
        if self.is_expanded {
            let mut menu = ChildView::new(&self.dropdown).finish();
            if self.use_drop_shadow {
                menu = Container::new(menu)
                    .with_drop_shadow(DropShadow::default())
                    .finish();
            }
            dropdown_stack.add_positioned_overlay_child(
                menu,
                OffsetPositioning::offset_from_save_position_element(
                    self.top_bar_label(),
                    vec2f(0., 0.),
                    PositionedElementOffsetBounds::WindowByPosition,
                    self.element_anchor,
                    self.child_anchor,
                ),
            );
        }
        Container::new(dropdown_stack.finish())
            .with_margin_top(self.vertical_margin)
            .with_margin_bottom(self.vertical_margin)
            .finish()
    }

    fn on_blur(&mut self, blur_ctx: &BlurContext, ctx: &mut ViewContext<Self>) {
        if blur_ctx.is_self_blurred() {
            ctx.emit(DropdownEvent::Close);
        }
    }
}
