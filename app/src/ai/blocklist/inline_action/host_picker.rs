//! Custom host picker used in the orchestration UI.
//!
//! Two display modes:
//! - **List mode (default):** behaves like a standard orchestration picker —
//!   a styled top bar opens a `Menu` of known options (workspace default
//!   first when set and badged "Default", then `warp`, then the most-
//!   recent custom host as a plain slug, plus a `Custom host…` entry).
//! - **Custom mode:** swaps the top bar for an inline [`EditorView`] so the
//!   user can type an arbitrary self-hosted worker slug. Enter or blur
//!   commits the trimmed value; the small `×` button or Escape reverts to
//!   list mode.
//!
//! Layout mirrors the Oz webapp's `HostSelector`: workspace default sits
//! at the top with a "Default" badge so admin-configured teams get their
//! preferred host pre-selected. Recent custom hosts render as plain
//! slugs (no "Recent" badge) because the compact dropdown doesn't have
//! room for the extra chip.
//!
//! The picker is non-generic and reports value changes via
//! [`HostPickerEvent::HostChanged`]. Card views subscribe and re-dispatch
//! their own `WorkerHostChanged` action so the existing edit-state plumbing
//! continues to work unchanged.

use warpui::elements::{
    Border, ChildAnchor, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    Expanded, Flex, Hoverable, MainAxisAlignment, MainAxisSize, MouseStateHandle, ParentElement,
    PositionedElementAnchor, Radius,
};
use warpui::platform::Cursor;
use warpui::{
    AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use warp_core::ui::theme::Fill;

use crate::ai::blocklist::inline_action::orchestration_controls::{
    self as oc, ORCHESTRATION_PICKER_BORDER_WIDTH, ORCHESTRATION_PICKER_FONT_SIZE,
    ORCHESTRATION_PICKER_HEIGHT, ORCHESTRATION_PICKER_RADIUS, ORCHESTRATION_WARP_WORKER_HOST,
};
use crate::appearance::Appearance;
use crate::editor::{
    EditorView, Event as EditorEvent, PropagateAndNoOpNavigationKeys, SingleLineEditorOptions,
    TextOptions,
};
use crate::menu::{MenuItem, MenuItemFields};
use crate::ui_components::blended_colors;
use crate::ui_components::icons::Icon;
use crate::view_components::dropdown::{
    Dropdown, DropdownAction, DropdownEvent, DropdownStyle, DROPDOWN_PADDING,
};

// ── Public API types ────────────────────────────────────────────────

/// Public events emitted by [`HostPicker`].
#[derive(Debug, Clone)]
pub enum HostPickerEvent {
    /// User selected a value from the menu or committed a custom entry.
    /// `slug` is non-empty and trimmed.
    HostChanged { slug: String },
    /// The menu closed (selection made, dismissed) or the custom-mode
    /// editor blurred. Parent views use this to refocus their input.
    Closed,
}

const CUSTOM_HOST_LABEL: &str = "Custom host…";
const DEFAULT_BADGE: &str = "Default";
const EDITOR_PLACEHOLDER: &str = "my-worker-host";

// ── Internal action plumbing ────────────────────────────────────────

/// Action dispatched by the inner `Dropdown<InternalAction>` items and
/// the inline `×` button. Handled by [`HostPicker`] itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternalAction {
    /// Pick a known host (Warp, workspace default, recent slug).
    SelectKnown(String),
    /// Switch to custom-mode text input.
    EnterCustomMode,
    /// Exit custom mode without committing the editor contents.
    CancelCustom,
}

// ── View ────────────────────────────────────────────────────────────

pub struct HostPicker {
    /// Currently displayed slug. Always equal to what would be sent to
    /// the server if the picker were dispatched right now.
    current_slug: String,
    /// Workspace-configured default, when set. Shown badged as "Default"
    /// and surfaced as the top row, matching the Oz webapp.
    default_host: Option<String>,
    /// User's most-recent custom host (excluding warp / default).
    recent_host: Option<String>,
    /// Inner menu-based dropdown rendered in list mode.
    dropdown: ViewHandle<Dropdown<InternalAction>>,
    /// Inline editor used in custom mode.
    editor: ViewHandle<EditorView>,
    /// Mouse state for the small `×` clear button.
    clear_mouse_state: MouseStateHandle,
    /// Whether we're currently showing the inline editor.
    is_custom_mode: bool,
    /// Snapshot of `current_slug` taken when the editor was opened, so
    /// Escape / `×` can revert without committing.
    slug_before_edit: Option<String>,
}

impl HostPicker {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let (_styles, colors) = oc::picker_styles(Appearance::as_ref(ctx));

        // Inner dropdown — styled identically to the other orchestration
        // pickers so the row stays visually uniform.
        let dropdown = ctx.add_typed_action_view(|ctx_dropdown| {
            let mut dropdown = Dropdown::<InternalAction>::new(ctx_dropdown);
            dropdown.set_use_overlay_layer(false, ctx_dropdown);
            dropdown.set_main_axis_size(MainAxisSize::Max, ctx_dropdown);
            dropdown.set_style(DropdownStyle::ActionButtonSecondary, ctx_dropdown);
            dropdown.set_top_bar_height(ORCHESTRATION_PICKER_HEIGHT, ctx_dropdown);
            dropdown.set_top_bar_max_width(f32::INFINITY);
            dropdown.set_padding(colors.padding, ctx_dropdown);
            dropdown.set_border_radius(colors.corner_radius, ctx_dropdown);
            dropdown.set_background(colors.background, ctx_dropdown);
            dropdown.set_border_width(ORCHESTRATION_PICKER_BORDER_WIDTH, ctx_dropdown);
            dropdown.set_font_size(ORCHESTRATION_PICKER_FONT_SIZE, ctx_dropdown);
            dropdown
        });
        ctx.subscribe_to_view(&dropdown, |me, _, event, ctx| {
            if let DropdownEvent::Close = event {
                // Suppress the propagated Closed event when we're
                // transitioning into custom mode. Otherwise the parent's
                // `refocus_after_picker_close` would steal focus from the
                // editor we just focused, the editor would fire `Blurred`,
                // and `commit_custom` would immediately revert us out of
                // custom mode — making the "Custom host…" menu item
                // look like a no-op to the user.
                if me.is_custom_mode {
                    return;
                }
                ctx.emit(HostPickerEvent::Closed);
                ctx.notify();
            }
        });

        // Inline editor for custom mode.
        let editor = ctx.add_typed_action_view(|ctx_editor| {
            let appearance = Appearance::as_ref(ctx_editor);
            let mut editor = EditorView::single_line(
                SingleLineEditorOptions {
                    text: TextOptions::ui_text(Some(appearance.ui_font_size()), appearance),
                    propagate_and_no_op_vertical_navigation_keys:
                        PropagateAndNoOpNavigationKeys::Always,
                    select_all_on_focus: true,
                    ..Default::default()
                },
                ctx_editor,
            );
            editor.set_placeholder_text(EDITOR_PLACEHOLDER, ctx_editor);
            editor
        });
        ctx.subscribe_to_view(&editor, |me, _, event, ctx| {
            me.handle_editor_event(event, ctx);
        });

        let mut me = Self {
            current_slug: ORCHESTRATION_WARP_WORKER_HOST.to_string(),
            default_host: None,
            recent_host: None,
            dropdown,
            editor,
            clear_mouse_state: MouseStateHandle::default(),
            is_custom_mode: false,
            slug_before_edit: None,
        };
        me.repopulate_menu(ctx);
        me.sync_dropdown_selection(ctx);
        me
    }

    // ── Public API ──────────────────────────────────────────────────

    /// Replaces the workspace default and recent-host options shown in
    /// the menu. Pass `None` to omit a given row.
    pub fn set_options(
        &mut self,
        default_host: Option<String>,
        recent_host: Option<String>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.default_host = default_host.filter(|s| !s.trim().is_empty());
        self.recent_host = recent_host.filter(|s| !s.trim().is_empty());
        self.repopulate_menu(ctx);
        self.sync_dropdown_selection(ctx);
        ctx.notify();
    }

    /// Forwards to the inner dropdown's [`Dropdown::set_use_overlay_layer`].
    /// Callers in the plan card pass `true` so the open menu paints in the
    /// overlay layer above sibling pickers (matching the other orchestration
    /// pickers in that view); callers in the confirmation card leave it at
    /// the default `false` and instead flip the menu upward via
    /// [`Self::set_menu_position`].
    pub fn set_use_overlay_layer(&mut self, use_overlay_layer: bool, ctx: &mut ViewContext<Self>) {
        self.dropdown.update(ctx, |dropdown, ctx_dropdown| {
            dropdown.set_use_overlay_layer(use_overlay_layer, ctx_dropdown);
        });
    }

    /// Forwards to the inner dropdown's [`Dropdown::set_menu_position`].
    /// The confirmation card uses this to open the menu upward, avoiding
    /// visual overlap with the Environment / Base model pickers rendered
    /// directly below the host picker.
    pub fn set_menu_position(
        &mut self,
        element_anchor: PositionedElementAnchor,
        child_anchor: ChildAnchor,
        ctx: &mut ViewContext<Self>,
    ) {
        self.dropdown.update(ctx, |dropdown, ctx_dropdown| {
            dropdown.set_menu_position(element_anchor, child_anchor, ctx_dropdown);
        });
    }

    /// Sets the currently-displayed slug. If the slug doesn't match any
    /// known menu option (Warp, default, recent), the picker switches to
    /// custom mode pre-filled with the slug. Empty input falls back to
    /// `"warp"`.
    pub fn set_selected(&mut self, slug: &str, ctx: &mut ViewContext<Self>) {
        let effective = {
            let trimmed = slug.trim();
            if trimmed.is_empty() {
                ORCHESTRATION_WARP_WORKER_HOST.to_string()
            } else {
                trimmed.to_string()
            }
        };
        let is_known = self.is_known_option(&effective);
        self.current_slug = effective.clone();
        if is_known {
            self.is_custom_mode = false;
            self.sync_dropdown_selection(ctx);
        } else {
            self.enter_custom_mode_with_slug(&effective, ctx);
        }
        ctx.notify();
    }

    // ── Internals ───────────────────────────────────────────────────

    fn is_known_option(&self, slug: &str) -> bool {
        if slug.eq_ignore_ascii_case(ORCHESTRATION_WARP_WORKER_HOST) {
            return true;
        }
        if self.default_host.as_deref() == Some(slug) {
            return true;
        }
        if self.recent_host.as_deref() == Some(slug) {
            return true;
        }
        false
    }

    fn repopulate_menu(&mut self, ctx: &mut ViewContext<Self>) {
        let items = build_menu_items(self.default_host.as_deref(), self.recent_host.as_deref());
        self.dropdown.update(ctx, |dropdown, ctx_dropdown| {
            dropdown.set_rich_items(items, ctx_dropdown);
        });
    }

    fn sync_dropdown_selection(&mut self, ctx: &mut ViewContext<Self>) {
        let label = menu_label_for(
            &self.current_slug,
            self.default_host.as_deref(),
            self.recent_host.as_deref(),
        );
        self.dropdown.update(ctx, |dropdown, ctx_dropdown| {
            dropdown.set_selected_by_name(&label, ctx_dropdown);
        });
    }

    fn enter_custom_mode_with_slug(&mut self, slug: &str, ctx: &mut ViewContext<Self>) {
        self.is_custom_mode = true;
        self.slug_before_edit = Some(self.current_slug.clone());
        let initial = if slug.eq_ignore_ascii_case(ORCHESTRATION_WARP_WORKER_HOST) {
            String::new()
        } else {
            slug.to_string()
        };
        self.editor.update(ctx, |editor, editor_ctx| {
            editor.set_buffer_text_ignoring_undo(&initial, editor_ctx);
        });
        ctx.focus(&self.editor);
    }

    /// Commits whatever is in the editor right now. Empty input is
    /// treated as a no-op revert (stays at the previous slug).
    fn commit_custom(&mut self, ctx: &mut ViewContext<Self>) {
        let raw = self.editor.as_ref(ctx).buffer_text(ctx).trim().to_string();
        if raw.is_empty() {
            self.cancel_custom(ctx);
            return;
        }
        self.current_slug = raw.clone();
        self.is_custom_mode = false;
        self.slug_before_edit = None;
        // If the committed slug isn't already one of the known options,
        // surface it as the new "Recent" entry so it's available in the
        // list on the next paint. The parent will also persist it
        // out-of-band so it survives across cards.
        if !self.is_known_option(&raw) {
            self.recent_host = Some(raw.clone());
            self.repopulate_menu(ctx);
        }
        self.sync_dropdown_selection(ctx);
        ctx.emit(HostPickerEvent::HostChanged { slug: raw });
        ctx.emit(HostPickerEvent::Closed);
        ctx.notify();
    }

    fn cancel_custom(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.is_custom_mode {
            return;
        }
        if let Some(prev) = self.slug_before_edit.take() {
            self.current_slug = prev;
        }
        self.is_custom_mode = false;
        self.sync_dropdown_selection(ctx);
        ctx.emit(HostPickerEvent::Closed);
        ctx.notify();
    }

    fn handle_editor_event(&mut self, event: &EditorEvent, ctx: &mut ViewContext<Self>) {
        match event {
            EditorEvent::Enter => self.commit_custom(ctx),
            EditorEvent::Escape => self.cancel_custom(ctx),
            EditorEvent::Blurred => {
                if self.is_custom_mode {
                    self.commit_custom(ctx);
                }
            }
            _ => {}
        }
    }

    fn render_custom_mode(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let background: Fill = theme.surface_overlay_1();
        let border_color = theme.outline();

        // Wrap the editor in a column with `MainAxisAlignment::Center` so its
        // text baseline sits at the vertical center of the picker box. Without
        // this, the surrounding row's tight cross-axis constraint stretches the
        // editor to fill the full content height and the glyphs render flush
        // to the top.
        let centered_editor = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(ChildView::new(&self.editor).finish())
            .finish();
        let cancel_button = self.render_cancel_button(appearance);

        let row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(Expanded::new(1.0, centered_editor).finish())
            .with_child(cancel_button)
            .finish();

        // The standard `Dropdown` view (used by every other picker in the
        // row) wraps its top bar in a Container with `DROPDOWN_PADDING`
        // top/bottom margins. Mirror that here so custom mode sits at the
        // same y offset as the other pickers instead of riding ~6px higher.
        Container::new(
            ConstrainedBox::new(
                Container::new(row)
                    .with_horizontal_padding(12.)
                    .with_vertical_padding(6.)
                    .with_background(background)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                        ORCHESTRATION_PICKER_RADIUS,
                    )))
                    .with_border(
                        Border::all(ORCHESTRATION_PICKER_BORDER_WIDTH)
                            .with_border_fill(border_color),
                    )
                    .finish(),
            )
            .with_height(ORCHESTRATION_PICKER_HEIGHT)
            .finish(),
        )
        .with_margin_top(DROPDOWN_PADDING)
        .with_margin_bottom(DROPDOWN_PADDING)
        .finish()
    }

    fn render_cancel_button(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let icon_fill = Fill::Solid(blended_colors::text_disabled(theme, theme.surface_1()));
        let mouse_state = self.clear_mouse_state.clone();
        Hoverable::new(mouse_state, move |_| {
            ConstrainedBox::new(
                Container::new(Icon::X.to_warpui_icon(icon_fill).finish())
                    .with_uniform_padding(2.)
                    .finish(),
            )
            .with_width(16.)
            .with_height(16.)
            .finish()
        })
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(InternalAction::CancelCustom);
        })
        .with_cursor(Cursor::PointingHand)
        .finish()
    }
}

// ── Pure helpers (also exercised by unit tests) ─────────────────────

/// Builds the menu items shown in list mode. Items appear in this order:
/// workspace default (if set, badged "Default"), `warp`, recent custom
/// host (if set and not a duplicate, plain slug), then "Custom host…".
/// Mirrors the Oz webapp's `HostSelector` layout.
pub(crate) fn build_menu_items(
    default_host: Option<&str>,
    recent_host: Option<&str>,
) -> Vec<MenuItem<DropdownAction<InternalAction>>> {
    let mut items: Vec<MenuItem<DropdownAction<InternalAction>>> = Vec::new();

    if let Some(slug) = default_host {
        items.push(menu_item_for_known(
            slug,
            Some(DEFAULT_BADGE),
            InternalAction::SelectKnown(slug.to_string()),
        ));
    }
    items.push(menu_item_for_known(
        ORCHESTRATION_WARP_WORKER_HOST,
        None,
        InternalAction::SelectKnown(ORCHESTRATION_WARP_WORKER_HOST.to_string()),
    ));
    if let Some(slug) = recent_host {
        if default_host != Some(slug) && !slug.eq_ignore_ascii_case(ORCHESTRATION_WARP_WORKER_HOST)
        {
            // Recent custom hosts render as plain slugs — no "(Recent)"
            // suffix. The "Default" badge stays because it carries a
            // distinct admin-policy meaning.
            items.push(menu_item_for_known(
                slug,
                None,
                InternalAction::SelectKnown(slug.to_string()),
            ));
        }
    }
    items.push(MenuItem::Item(
        MenuItemFields::new(CUSTOM_HOST_LABEL).with_on_select_action(
            DropdownAction::SelectActionAndClose(InternalAction::EnterCustomMode),
        ),
    ));

    items
}

/// Returns the menu label that corresponds to `slug` (so callers can
/// re-select it via `set_selected_by_name`). The label includes the
/// "Default" badge when the slug matches the workspace default; recent
/// custom hosts render as plain slugs.
pub(crate) fn menu_label_for(
    slug: &str,
    default_host: Option<&str>,
    _recent_host: Option<&str>,
) -> String {
    if default_host == Some(slug) {
        format_known_label(slug, Some(DEFAULT_BADGE))
    } else {
        format_known_label(slug, None)
    }
}

fn format_known_label(slug: &str, badge: Option<&str>) -> String {
    match badge {
        Some(badge) => format!("{slug}  ({badge})"),
        None => slug.to_string(),
    }
}

fn menu_item_for_known(
    slug: &str,
    badge: Option<&str>,
    action: InternalAction,
) -> MenuItem<DropdownAction<InternalAction>> {
    MenuItem::Item(
        MenuItemFields::new(format_known_label(slug, badge))
            .with_on_select_action(DropdownAction::SelectActionAndClose(action)),
    )
}

// ── Entity / View impls ─────────────────────────────────────────────

impl Entity for HostPicker {
    type Event = HostPickerEvent;
}

impl TypedActionView for HostPicker {
    type Action = InternalAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            InternalAction::SelectKnown(slug) => {
                let slug = slug.clone();
                self.current_slug = slug.clone();
                self.is_custom_mode = false;
                self.slug_before_edit = None;
                // Intentionally NOT calling sync_dropdown_selection: this
                // action was dispatched FROM the inner dropdown, which
                // already updated its own `selected_item` via
                // `MenuEvent::ItemSelected`. Re-entering its view from
                // here would panic with `Circular view update`.
                ctx.emit(HostPickerEvent::HostChanged { slug });
                ctx.notify();
            }
            InternalAction::EnterCustomMode => {
                let current = self.current_slug.clone();
                self.enter_custom_mode_with_slug(&current, ctx);
                ctx.notify();
            }
            InternalAction::CancelCustom => {
                self.cancel_custom(ctx);
            }
        }
    }
}

impl View for HostPicker {
    fn ui_name() -> &'static str {
        "HostPicker"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        if self.is_custom_mode {
            self.render_custom_mode(appearance)
        } else {
            ChildView::new(&self.dropdown).finish()
        }
    }
}

#[cfg(test)]
#[path = "host_picker_tests.rs"]
mod tests;
