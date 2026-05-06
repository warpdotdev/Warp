//! Renders a horizontal pill bar in the agent view header containing the
//! orchestrator agent and any child agents spawned by it. Clicking a pill
//! switches the active pane to that agent's conversation.
//!
//! V1 scope: avatars (deterministic color + initial), pill labels, click to
//! switch. Hover popover, pin / unpin, and the 3-dot menu (Open in new pane /
//! Open in new tab / Stop agent / Kill agent) are tracked as follow-ups.

use std::cell::RefCell;
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::time::Duration;

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warp_cli::agent::Harness;
use warp_core::ui::color::coloru_with_opacity;
use warp_core::ui::theme::Fill;
use warp_core::ui::{appearance::Appearance, theme::WarpTheme};
use warpui::elements::new_scrollable::{NewScrollable, ScrollableAppearance, SingleAxisConfig};
use warpui::elements::{
    AnchorPair, ChildAnchor, ChildView, Clipped, ClippedScrollStateHandle, ConstrainedBox,
    Container, CornerRadius, CrossAxisAlignment, Element, Empty, Fill as ElementFill, Flex,
    Hoverable, MainAxisAlignment, MainAxisSize, MouseStateHandle, OffsetPositioning, OffsetType,
    ParentAnchor, ParentElement, ParentOffsetBounds, PositionedElementOffsetBounds,
    PositioningAxis, Radius, SavePosition, ScrollbarWidth, Stack, Text, XAxisAnchor, YAxisAnchor,
};
use warpui::fonts::{Properties, Weight};
use warpui::platform::Cursor;
use warpui::text_layout::ClipConfig;
use warpui::{
    AppContext, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use crate::ai::active_agent_views_model::ActiveAgentViewsModel;
use crate::ai::agent::conversation::{AIConversation, AIConversationId, ConversationStatus};
use crate::ai::artifacts::Artifact;
use crate::ai::blocklist::agent_view::orchestration_conversation_links::parent_conversation_id;
use crate::ai::blocklist::agent_view::{AgentViewController, AgentViewControllerEvent};
use crate::ai::blocklist::{BlocklistAIHistoryEvent, BlocklistAIHistoryModel};
use crate::ai::harness_display;
use crate::features::FeatureFlag;
use crate::menu::{Event as MenuEvent, Menu, MenuItem, MenuItemFields};
use crate::pane_group::pane::view::PaneHeaderAction;
use crate::terminal::view::TerminalAction;
use crate::ui_components::icons::Icon;
use crate::workspace::{WorkspaceAction, WorkspaceRegistry};
use warp_core::ui::theme::color::internal_colors;
use warpui::EntityId;

const PILL_HEIGHT: f32 = 22.;
const PILL_RADIUS: f32 = PILL_HEIGHT / 2.;
const AVATAR_SIZE: f32 = 16.;
const PILL_LABEL_MAX_WIDTH: f32 = 110.;
const PILL_GAP: f32 = 6.;
const PILL_HORIZONTAL_PADDING_LEFT: f32 = 4.;
const PILL_HORIZONTAL_PADDING_RIGHT: f32 = 10.;

/// Stable palette used to color child agent avatars deterministically by name.
fn pill_palette(theme: &WarpTheme) -> [ColorU; 6] {
    [
        theme.ansi_fg_blue(),
        theme.ansi_fg_magenta(),
        theme.ansi_fg_cyan(),
        theme.ansi_fg_green(),
        theme.ansi_fg_yellow(),
        theme.ansi_fg_red(),
    ]
}

fn pill_avatar_color(name: &str, theme: &WarpTheme) -> ColorU {
    let palette = pill_palette(theme);
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    let idx = (hasher.finish() as usize) % palette.len();
    palette[idx]
}

fn pill_initial(name: &str) -> char {
    name.trim()
        .chars()
        .next()
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('A')
}

/// What kind of pill we are rendering, which determines click behavior.
#[derive(Clone, Copy)]
enum PillKind {
    Orchestrator,
    Child,
}

/// Whether this pill's conversation is currently "pinned" — i.e. living in
/// another visible terminal view (a separate pane or tab from this one). The
/// orchestrator pane displays a pin glyph in front of pinned children's
/// labels; clicking a pinned pill focuses that other pane/tab instead of
/// switching this pane in place.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // PinnedInOtherPane is wired up but currently never
                    // constructed — pin detection is disabled until pane-visibility plumbing
                    // lands. See `pill_specs` for context.
enum PillPinState {
    Unpinned,
    PinnedInOtherPane,
}

/// Pre-computed data for one pill in the bar.
struct PillSpec {
    conversation_id: AIConversationId,
    label: String,
    avatar_color: ColorU,
    avatar_glyph: AvatarGlyph,
    is_selected: bool,
    kind: PillKind,
    pin_state: PillPinState,
}

#[derive(Clone, Copy)]
enum AvatarGlyph {
    Letter(char),
    Icon(Icon),
}

/// Width of the per-pill 3-dot overflow menu when expanded.
const OVERFLOW_MENU_WIDTH: f32 = 200.;
/// Size in logical pixels of the 3-dot button at the trailing edge of each
/// child pill.
const OVERFLOW_BUTTON_SIZE: f32 = 16.;

/// Returns the saved-position id used to anchor the 3-dot menu to a
/// specific child pill's overflow button. The id is global within the
/// position cache, so we include the conversation id to keep it unique
/// across multiple sibling pills.
fn overflow_button_position_id(conversation_id: AIConversationId) -> String {
    format!("orchestration-pill-overflow-{conversation_id}")
}

/// Returns the saved-position id used to anchor the hover details card
/// to a specific pill's body. Unique per conversation so neighbouring
/// pills don't fight over the same id.
fn pill_body_position_id(conversation_id: AIConversationId) -> String {
    format!("orchestration-pill-body-{conversation_id}")
}

/// Width of the per-pill hover details card.
const HOVER_CARD_WIDTH: f32 = 280.;
/// Delay before the hover card appears after the cursor first lands on a
/// pill. Matches the standard tooltip / popover delay so a quick scrub
/// across the bar doesn't pop a card per pill.
const HOVER_CARD_IN_DELAY: Duration = Duration::from_millis(300);
/// Delay before the card disappears after the cursor leaves the pill.
/// Small but nonzero so a single-pixel gap between pill and card doesn't
/// instantly dismiss it. (V1 doesn't yet share hover state across the pill
/// and the card itself, so the card disappears as soon as the pointer
/// leaves the pill body — the hover-out delay just smooths that.)
const HOVER_CARD_OUT_DELAY: Duration = Duration::from_millis(80);

/// Typed actions dispatched by the pill bar's own widgets (the 3-dot
/// overflow button and the items in its dropdown menu). Each action carries
/// the `AIConversationId` of the child pill it targets so a single
/// `Menu<OrchestrationPillBarAction>` instance can serve every child by being
/// rebuilt with the right ids each time it opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrchestrationPillBarAction {
    /// Open the 3-dot menu for the given child conversation. Dispatched by
    /// the trailing `⋯` button on a child pill.
    OpenMenu(AIConversationId),
    /// Close the open menu. Dispatched by the `Menu`'s `Close` event so
    /// click-outside dismissal is handled the same way as keyboard ESC.
    CloseMenu,
    /// Menu item: split the orchestrator pane right and host this child
    /// agent in the new pane.
    OpenInNewPane(AIConversationId),
    /// Menu item: open this child agent in a new tab in the same window.
    OpenInNewTab(AIConversationId),
    /// Menu item: stop the in-progress agent task without removing the
    /// conversation from history.
    ///
    /// Currently unconstructed — the menu item is hidden because the
    /// underlying behaviour isn't reliable yet (see
    /// `open_menu_for`). The handler arm and the corresponding
    /// `TerminalAction::StopAgentConversation` wiring stay in place
    /// so re-adding the menu entry only requires uncommenting the
    /// item.
    #[allow(dead_code)]
    Stop(AIConversationId),
    /// Menu item: cancel any in-flight task and remove the conversation
    /// from local history (no server delete).
    ///
    /// Currently unconstructed — see the comment on `Stop` above.
    #[allow(dead_code)]
    Kill(AIConversationId),
    /// Set or clear which pill the user is currently hovering. Dispatched
    /// from the pill body's `on_hover` handler after the configured
    /// hover-in delay so the details card can be rendered as an overlay.
    /// `None` clears the hovered pill (cursor left the bar).
    SetHoveredPill(Option<AIConversationId>),
    /// Menu item: focus the existing pane/tab that already owns the
    /// child agent's transcript instead of splitting/opening a new one.
    /// Used when the conversation is open elsewhere; mirrors the
    /// breadcrumb parent click's `RestoreOrNavigateToConversation`
    /// dispatch.
    FocusOpenedConversation(AIConversationId),
}

/// View that renders the orchestration pill bar above the agent view content.
///
/// Shows one pill for the orchestrator (parent of the active conversation, or
/// the active conversation itself if it has no parent) and one pill per child
/// agent spawned by that orchestrator. Clicking a non-active pill switches
/// to that agent's pane.
pub struct OrchestrationPillBar {
    agent_view_controller: ModelHandle<AgentViewController>,
    /// Hover state per conversation id, persisted across renders so hover
    /// effects work correctly. Wrapped in a `RefCell` so `render` (which only
    /// has `&self`) can lazily insert handles for ids that aren't yet
    /// populated by `ensure_mouse_states`. Synthesizing a fresh
    /// `MouseStateHandle::default()` in the render hot path is *not*
    /// equivalent: the next render after a mouse-down would produce yet
    /// another fresh handle, losing the down-state before mouse-up arrives
    /// and silently swallowing the click (per the WarpUI mouse-state rule).
    mouse_states: RefCell<HashMap<AIConversationId, MouseStateHandle>>,
    /// Hover state per child pill's trailing 3-dot button. Kept separate
    /// from `mouse_states` so the button has its own hover highlight
    /// independent of the pill body.
    overflow_button_mouse_states: RefCell<HashMap<AIConversationId, MouseStateHandle>>,
    /// The single shared dropdown menu rendered when a child pill's 3-dot
    /// button is clicked. Items are rebuilt per-open with the relevant
    /// child id baked into each `on_select_action` so we don't need a
    /// separate menu instance per pill.
    menu: ViewHandle<Menu<OrchestrationPillBarAction>>,
    /// `Some(id)` when the 3-dot menu is currently open and targeting the
    /// child conversation `id`, `None` when no menu is open. Used both to
    /// gate whether the menu overlay renders and to highlight the active
    /// pill while the menu is open.
    menu_open_for: Option<AIConversationId>,
    /// `Some(id)` while the cursor is hovering the pill for that
    /// conversation (after the configured hover-in delay) and the 3-dot
    /// menu is not also open. Drives the per-pill details card overlay.
    hovered_pill: Option<AIConversationId>,
}

impl Entity for OrchestrationPillBar {
    type Event = ();
}

impl OrchestrationPillBar {
    pub fn new(
        agent_view_controller: ModelHandle<AgentViewController>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let history_model = BlocklistAIHistoryModel::handle(ctx);
        ctx.subscribe_to_model(&history_model, |this, _, event, ctx| match event {
            BlocklistAIHistoryEvent::UpdatedConversationStatus { .. }
            | BlocklistAIHistoryEvent::AppendedExchange { .. }
            | BlocklistAIHistoryEvent::SetActiveConversation { .. }
            | BlocklistAIHistoryEvent::StartedNewConversation { .. } => {
                this.ensure_mouse_states(ctx);
                ctx.notify();
            }
            BlocklistAIHistoryEvent::RemoveConversation {
                conversation_id, ..
            }
            | BlocklistAIHistoryEvent::DeletedConversation {
                conversation_id, ..
            } => {
                this.mouse_states.borrow_mut().remove(conversation_id);
                this.overflow_button_mouse_states
                    .borrow_mut()
                    .remove(conversation_id);
                // If the menu was open for a child that just disappeared,
                // close it so we don't leave a dangling menu pointing at a
                // dead conversation id.
                if this.menu_open_for == Some(*conversation_id) {
                    this.menu_open_for = None;
                }
                ctx.notify();
            }
            _ => {}
        });
        ctx.subscribe_to_model(&agent_view_controller, |this, _, event, ctx| {
            if matches!(
                event,
                AgentViewControllerEvent::EnteredAgentView { .. }
                    | AgentViewControllerEvent::ExitedAgentView { .. }
            ) {
                this.mouse_states.borrow_mut().clear();
                this.overflow_button_mouse_states.borrow_mut().clear();
                this.menu_open_for = None;
            }
            this.ensure_mouse_states(ctx);
            ctx.notify();
        });

        let menu = ctx.add_typed_action_view(|_ctx| {
            Menu::new()
                .with_width(OVERFLOW_MENU_WIDTH)
                .with_drop_shadow()
                .prevent_interaction_with_other_elements()
        });

        // The menu emits `Close { .. }` when the user clicks outside the
        // menu or presses ESC. Forward that into our typed action surface
        // so menu_open_for stays in sync with what's actually visible.
        ctx.subscribe_to_view(&menu, |this, _, event, ctx| match event {
            MenuEvent::Close { .. } => {
                this.handle_action(&OrchestrationPillBarAction::CloseMenu, ctx);
            }
            MenuEvent::ItemSelected | MenuEvent::ItemHovered => {}
        });

        Self {
            agent_view_controller,
            mouse_states: RefCell::new(HashMap::new()),
            overflow_button_mouse_states: RefCell::new(HashMap::new()),
            menu,
            menu_open_for: None,
            hovered_pill: None,
        }
    }

    /// Rebuilds the menu items for the given child conversation id and
    /// flips `menu_open_for` to that id. Called each time the user clicks
    /// a child's 3-dot button so the menu items dispatch actions targeting
    /// the right child.
    fn open_menu_for(&mut self, conversation_id: AIConversationId, ctx: &mut ViewContext<Self>) {
        let appearance = Appearance::as_ref(ctx);
        let theme = appearance.theme();
        let hover_background: Fill = internal_colors::neutral_4(theme).into();

        let item = |label: &'static str,
                    icon: Icon,
                    action: OrchestrationPillBarAction|
         -> MenuItem<OrchestrationPillBarAction> {
            MenuItem::Item(
                MenuItemFields::new(label)
                    .with_icon(icon)
                    .with_override_hover_background_color(hover_background)
                    .with_on_select_action(action),
            )
        };

        // If this child conversation is already open in a *different*
        // visible terminal view — whether that's a separate pane in this
        // tab, another tab, or another window — collapse the create-new
        // entries into a single "Focus pane" item that routes the user to
        // the existing owner. The conversation has a single source of
        // truth (one terminal view renders its AI blocks; see
        // `ConversationOwnershipTransferred` in `BlocklistAIHistoryModel`),
        // so offering separate "Open pane" / "Open tab" entries here
        // would imply more than one destination exists.
        //
        // We deliberately do NOT use
        // `ActiveAgentViewsModel::terminal_view_id_for_conversation` here:
        // every child agent has a hidden child-agent pane registered in
        // that model (its `AgentViewController` keeps the child as its
        // `active_conversation_id` to receive events), which would falsely
        // flag every child as "open elsewhere" even when the orchestrator
        // is the only visible owner.
        let self_terminal_view_id = self.agent_view_controller.as_ref(ctx).terminal_view_id();
        let is_open_elsewhere =
            is_conversation_open_in_other_visible_view(conversation_id, self_terminal_view_id, ctx);

        // Stop / Kill items are intentionally omitted for now — their
        // wiring is still in place (see `OrchestrationPillBarAction::Stop`
        // / `Kill` and the matching `TerminalAction` handlers) but the
        // current behaviour isn't reliable enough to ship. Re-add the
        // menu items here when that's fixed.
        let items = if is_open_elsewhere {
            vec![item(
                "Focus pane",
                Icon::ArrowSplit,
                OrchestrationPillBarAction::FocusOpenedConversation(conversation_id),
            )]
        } else {
            vec![
                item(
                    "Open in new pane",
                    Icon::ArrowSplit,
                    OrchestrationPillBarAction::OpenInNewPane(conversation_id),
                ),
                item(
                    "Open in new tab",
                    Icon::Plus,
                    OrchestrationPillBarAction::OpenInNewTab(conversation_id),
                ),
            ]
        };

        self.menu.update(ctx, |menu, ctx| {
            menu.set_items(items, ctx);
        });
        self.menu_open_for = Some(conversation_id);
        // The hover card and the open 3-dot menu both anchor to the same
        // pill, so suppress the card while the menu is shown to avoid
        // overlapping overlays.
        self.hovered_pill = None;
        ctx.focus(&self.menu);
        ctx.notify();
    }

    fn close_menu(&mut self, ctx: &mut ViewContext<Self>) {
        if self.menu_open_for.is_none() {
            return;
        }
        self.menu_open_for = None;
        ctx.notify();
    }

    fn set_hovered_pill(
        &mut self,
        conversation_id: Option<AIConversationId>,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.hovered_pill == conversation_id {
            return;
        }
        self.hovered_pill = conversation_id;
        ctx.notify();
    }

    fn ensure_mouse_states(&mut self, ctx: &AppContext) {
        let Some(active_id) = self
            .agent_view_controller
            .as_ref(ctx)
            .agent_view_state()
            .active_conversation_id()
        else {
            return;
        };
        let history = BlocklistAIHistoryModel::as_ref(ctx);
        let Some(active_conversation) = history.conversation(&active_id) else {
            return;
        };
        let orchestrator_id = parent_conversation_id(active_conversation, ctx).unwrap_or(active_id);
        // Build the set of conversation ids that should still have hover state
        // tracked, then insert any missing entries and drop the rest. Without
        // the retain step, switching between orchestrators within the same
        // view would leak `MouseStateHandle`s for old orchestrators / their
        // children indefinitely.
        let mut alive: HashSet<AIConversationId> = HashSet::new();
        alive.insert(orchestrator_id);
        // Include the parent id when the active conversation is a child, so
        // the breadcrumb's parent crumb has a stable handle even before the
        // parent `AIConversation` is loaded into history.
        if let Some(parent_id) = parent_conversation_id(active_conversation, ctx) {
            alive.insert(parent_id);
        }
        for child in history.child_conversations_of(orchestrator_id) {
            alive.insert(child.id());
        }
        let mut mouse_states = self.mouse_states.borrow_mut();
        let mut overflow_states = self.overflow_button_mouse_states.borrow_mut();
        for id in &alive {
            mouse_states.entry(*id).or_default();
            overflow_states.entry(*id).or_default();
        }
        mouse_states.retain(|id, _| alive.contains(id));
        overflow_states.retain(|id, _| alive.contains(id));
    }

    /// Builds the ordered list of pills to render. Returns `None` when the
    /// pill bar should not be shown at all.
    fn pill_specs(&self, app: &AppContext) -> Option<Vec<PillSpec>> {
        let active_id = self
            .agent_view_controller
            .as_ref(app)
            .agent_view_state()
            .active_conversation_id()?;
        let history = BlocklistAIHistoryModel::as_ref(app);
        let active_conversation = history.conversation(&active_id)?;

        // V2 same-pane semantics: render pills for both the orchestrator and
        // any same-pane child. When the active conversation is a child,
        // resolve its orchestrator (parent) so the pill set is anchored on
        // the orchestrator regardless of which conversation is selected.
        //
        // Split-off child views render breadcrumbs only (no pill bar). Bail
        // here so the pane header renders just the breadcrumbs returned by
        // `render_orchestration_breadcrumbs` and not a redundant pill row
        // below it. Without this short-circuit, the breadcrumbs in the title
        // and the pill bar in the column would both render, producing
        // duplicate orchestration chrome in the split-off pane/tab.
        if is_split_off_child(self.agent_view_controller.as_ref(app), app) {
            return None;
        }
        let orchestrator_id = parent_conversation_id(active_conversation, app).unwrap_or(active_id);
        let orchestrator = history.conversation(&orchestrator_id)?;

        // `child_conversations_of` returns children in registration order
        // (i.e. the order they were spawned by the orchestrator), which is
        // the stable ordering we want for the pills. Don't re-sort by
        // `first_exchange().start_time`: children whose first exchange
        // hasn't started yet would otherwise sort to the front (because
        // `Option::None < Option::Some`) and pop into their time-based
        // position once they begin streaming, reshuffling the bar.
        let children = history.child_conversations_of(orchestrator_id);

        // Nothing to show if the orchestrator has no children yet.
        if children.is_empty() {
            return None;
        }

        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let mut specs = Vec::with_capacity(1 + children.len());

        // Orchestrator pill first. The orchestrator never carries a pin
        // glyph: it represents the home view of the orchestration tree, not
        // a sibling that has been split off.
        specs.push(PillSpec {
            conversation_id: orchestrator_id,
            label: orchestrator_label(orchestrator),
            avatar_color: theme.ansi_fg_cyan(),
            avatar_glyph: AvatarGlyph::Icon(Icon::Oz),
            is_selected: orchestrator_id == active_id,
            kind: PillKind::Orchestrator,
            pin_state: PillPinState::Unpinned,
        });

        // Then a pill per child agent.
        //
        // V2-of-V2 NOTE: pin detection is intentionally disabled here. The
        // intended signal was "this child has an active agent view in some
        // *other* visible terminal view than the orchestrator pane" (using
        // `ActiveAgentViewsModel::terminal_view_id_for_conversation`), but
        // every child agent already has a hidden terminal view registered in
        // `ActiveAgentViewsModel` (via `TerminalPane::attach` when the
        // orchestrator's `StartAgentExecutor` creates the hidden child pane
        // through `create_hidden_child_agent_conversation`). That makes the
        // naive check fire for every child — swapping every avatar for the
        // pin glyph and routing every click through `RevealChildAgent`
        // instead of `SwitchAgentViewToConversation`, which broke the
        // expected in-place switching.
        //
        // Restoring real pin detection requires plumbing pane visibility
        // (visible vs. hidden-for-child-agent) into `ActiveAgentViewsModel`,
        // or exposing a `is_child_agent_pane_visible(conversation_id)` accessor
        // off `PaneGroup`. Tracked as a follow-up.
        for child in children {
            let name = child
                .agent_name()
                .filter(|n| !n.is_empty())
                .unwrap_or("Agent");
            specs.push(PillSpec {
                conversation_id: child.id(),
                label: name.to_string(),
                avatar_color: pill_avatar_color(name, theme),
                avatar_glyph: AvatarGlyph::Letter(pill_initial(name)),
                is_selected: child.id() == active_id,
                kind: PillKind::Child,
                pin_state: PillPinState::Unpinned,
            });
        }

        Some(specs)
    }
}

/// Renders a non-interactive agent pill using the same deterministic-color
/// + initial-letter avatar as the live pill bar.
pub fn render_static_agent_pill(name: &str, app: &AppContext) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let avatar_color = pill_avatar_color(name, theme);
    let avatar_glyph = AvatarGlyph::Letter(pill_initial(name));
    let avatar = render_avatar_disc(avatar_color, avatar_glyph, theme, appearance);
    let label_text = Text::new(
        name.to_string(),
        appearance.ui_font_family(),
        appearance.monospace_font_size() - 1.,
    )
    .with_color(internal_colors::text_main(theme, theme.background()))
    .soft_wrap(false)
    .with_clip(ClipConfig::ellipsis())
    .finish();

    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min)
        .with_spacing(6.)
        .with_child(avatar)
        .with_child(
            ConstrainedBox::new(label_text)
                .with_max_width(PILL_LABEL_MAX_WIDTH)
                .finish(),
        )
        .finish();

    ConstrainedBox::new(
        Container::new(row)
            .with_padding_left(PILL_HORIZONTAL_PADDING_LEFT)
            .with_padding_right(PILL_HORIZONTAL_PADDING_RIGHT)
            .with_background_color(internal_colors::fg_overlay_2(theme).into())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(PILL_RADIUS)))
            .finish(),
    )
    .with_height(PILL_HEIGHT)
    .finish()
}

/// Returns the label to use for the orchestrator pill. Prefers the explicitly
/// set agent name, falling back to "Orchestrator" so the pill is meaningful
/// even before any naming has happened.
fn orchestrator_label(orchestrator: &AIConversation) -> String {
    orchestrator
        .agent_name()
        .filter(|n| !n.is_empty())
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Orchestrator".to_string())
}

impl TypedActionView for OrchestrationPillBar {
    type Action = OrchestrationPillBarAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            OrchestrationPillBarAction::OpenMenu(id) => {
                self.open_menu_for(*id, ctx);
            }
            OrchestrationPillBarAction::CloseMenu => {
                self.close_menu(ctx);
            }
            OrchestrationPillBarAction::OpenInNewPane(id) => {
                // Defer the actual pane split / tab open / cancel logic to
                // `TerminalView::handle_action`, which already owns the
                // wiring added in Phase C. We just translate the typed
                // pill-bar action into the existing `TerminalAction` and
                // dispatch it through the pane header action surface so
                // it bubbles up the standard way (mirrors the pill-click
                // path in `render_pill`).
                self.close_menu(ctx);
                ctx.dispatch_typed_action(
                    &PaneHeaderAction::<TerminalAction, TerminalAction>::CustomAction(
                        TerminalAction::OpenChildAgentInNewPane {
                            conversation_id: *id,
                        },
                    ),
                );
            }
            OrchestrationPillBarAction::OpenInNewTab(id) => {
                self.close_menu(ctx);
                ctx.dispatch_typed_action(
                    &PaneHeaderAction::<TerminalAction, TerminalAction>::CustomAction(
                        TerminalAction::OpenChildAgentInNewTab {
                            conversation_id: *id,
                        },
                    ),
                );
            }
            OrchestrationPillBarAction::Stop(id) => {
                self.close_menu(ctx);
                ctx.dispatch_typed_action(
                    &PaneHeaderAction::<TerminalAction, TerminalAction>::CustomAction(
                        TerminalAction::StopAgentConversation {
                            conversation_id: *id,
                        },
                    ),
                );
            }
            OrchestrationPillBarAction::Kill(id) => {
                self.close_menu(ctx);
                ctx.dispatch_typed_action(
                    &PaneHeaderAction::<TerminalAction, TerminalAction>::CustomAction(
                        TerminalAction::KillAgentConversation {
                            conversation_id: *id,
                        },
                    ),
                );
            }
            OrchestrationPillBarAction::SetHoveredPill(id) => {
                self.set_hovered_pill(*id, ctx);
            }
            OrchestrationPillBarAction::FocusOpenedConversation(id) => {
                self.close_menu(ctx);
                // "Focus pane" is purely a focus operation: the
                // conversation already lives in some other visible
                // terminal view (verified by
                // `is_conversation_open_in_other_visible_view` before we
                // surface this menu item) and we just want to move the
                // user's cursor there. We deliberately do *not* go
                // through `RestoreOrNavigateToConversation`: that path
                // calls `set_active_conversation_id` with whichever
                // `terminal_view_id` it receives, which would either
                // re-transfer ownership to a stale id pulled from
                // `AgentConversationsModel::nav_data` or, worse, blank
                // out the real owner pane while the conversation pops
                // back into the orchestrator.
                //
                // Resolve the canonical owner directly from
                // `BlocklistAIHistoryModel` (the single source of truth)
                // and pick the appropriate focus action based on whether
                // the owner pane lives in the same pane group as us:
                //   * Same pane group (sibling pane in this tab) —
                //     dispatch `TerminalAction::RevealChildAgent`. The
                //     pane group's handler walks visible terminal panes
                //     and calls `group.focus_pane(.., true, ctx)` from
                //     its own `ViewContext<PaneGroup>`, which actually
                //     shifts focus to the sibling pane. Going through
                //     the workspace's `focus_pane` from a different
                //     `ViewContext` doesn't reliably move focus when the
                //     destination is in the same pane group.
                //   * Different pane group (other tab / window) —
                //     dispatch `WorkspaceAction::FocusTerminalViewInWorkspace`,
                //     which walks all tabs/windows and activates the
                //     containing tab as needed.
                let owner_view_id =
                    BlocklistAIHistoryModel::as_ref(ctx).terminal_view_id_for_conversation(id);
                let Some(owner_view_id) = owner_view_id else {
                    log::warn!(
                        "FocusOpenedConversation: no canonical owner for {id:?}; falling back to switch-in-place"
                    );
                    ctx.dispatch_typed_action(
                        &PaneHeaderAction::<TerminalAction, TerminalAction>::CustomAction(
                            TerminalAction::SwitchAgentViewToConversation {
                                conversation_id: *id,
                            },
                        ),
                    );
                    return;
                };
                let self_pane_group_id = self.agent_view_controller.as_ref(ctx).pane_group_id();
                let owner_pane_group_id =
                    pane_group_id_containing_terminal_view(owner_view_id, ctx);
                if owner_pane_group_id.is_some() && owner_pane_group_id == self_pane_group_id {
                    ctx.dispatch_typed_action(
                        &PaneHeaderAction::<TerminalAction, TerminalAction>::CustomAction(
                            TerminalAction::RevealChildAgent {
                                conversation_id: *id,
                            },
                        ),
                    );
                } else {
                    ctx.dispatch_typed_action(&WorkspaceAction::FocusTerminalViewInWorkspace {
                        terminal_view_id: owner_view_id,
                    });
                }
            }
        }
    }
}

impl View for OrchestrationPillBar {
    fn ui_name() -> &'static str {
        "OrchestrationPillBar"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let Some(specs) = self.pill_specs(app) else {
            return Empty::new().finish();
        };

        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        // The row uses `MainAxisSize::Max` so the bar's intrinsic width
        // is the parent's available width (i.e. the pane width passed in
        // by the wrapping `Flex::column` in `pane_impl.rs`), not the sum
        // of the children. With `MainAxisSize::Min` the row reports its
        // full intrinsic width upward and the surrounding `Clipped`
        // wrapper has nothing tighter to clip against, so the trailing
        // pills paint into whichever pane sits to the right. Children
        // remain left-packed via `MainAxisAlignment::Start`; any pills
        // that overflow to the right of the available width get clipped
        // by the `Clipped` element below.
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::Start)
            .with_spacing(PILL_GAP);

        // Resolve a persistent `MouseStateHandle` for each pill. If `ensure_mouse_states`
        // has not yet seen this id (e.g. mid-event-propagation race), insert a
        // freshly defaulted handle into our `mouse_states` map and reuse it on
        // subsequent renders. Falling back to a transient
        // `MouseStateHandle::default()` here would silently break clicks: the
        // mouse-down notify would re-enter `render` with yet another fresh
        // handle, and mouse-up would land on a different handle than
        // mouse-down.
        let mut mouse_states = self.mouse_states.borrow_mut();
        let mut overflow_states = self.overflow_button_mouse_states.borrow_mut();
        let menu_open_for = self.menu_open_for;
        // Cache this view's terminal_view_id once so each pill click can
        // cheaply check whether its target conversation is currently
        // owned by *another* terminal view. The pill bar renders inside
        // the orchestrator pane, so any child whose owner differs from
        // this id has been split off into another pane/tab.
        let self_terminal_view_id = self.agent_view_controller.as_ref(app).terminal_view_id();
        for spec in specs {
            let mouse_state = mouse_states
                .entry(spec.conversation_id)
                .or_default()
                .clone();
            // Each child pill gets its own dedicated 3-dot button mouse
            // state so hover highlight on the button is independent of the
            // pill body. Orchestrator pills don't get a 3-dot button (no
            // overflow actions apply to the home view), so we still create
            // the entry for layout symmetry but won't render the button.
            let overflow_mouse_state = overflow_states
                .entry(spec.conversation_id)
                .or_default()
                .clone();
            let menu_is_open_for_this = menu_open_for == Some(spec.conversation_id);
            row.add_child(render_pill(
                spec,
                mouse_state,
                overflow_mouse_state,
                menu_is_open_for_this,
                self_terminal_view_id,
                app,
            ));
        }
        drop(mouse_states);
        drop(overflow_states);

        // Wrap in a container with a touch of horizontal padding so the bar
        // doesn't sit flush against the pane edges, and with the same overlay
        // background as the rest of the agent view header so it merges visually.
        //
        // Wrap the whole thing in a `Clipped` so when the orchestrator's
        // pane is narrower than the natural width of the pill row
        // (orchestrator + N child pills), the pills get clipped at the pane
        // boundary instead of bleeding into whichever pane sits to the
        // right. Without this clip the row's `MainAxisSize::Min` reports
        // its full intrinsic width upward and the parent doesn't enforce
        // a horizontal bound, so the trailing pills paint outside the
        // pane (visible in split layouts).
        let bar = Clipped::new(
            Container::new(row.finish())
                .with_padding_left(12.)
                .with_padding_right(12.)
                .with_padding_top(4.)
                .with_padding_bottom(4.)
                .with_background(theme.surface_overlay_1())
                .finish(),
        )
        .finish();

        // When the 3-dot menu is open, overlay it directly beneath the
        // clicked pill's overflow button. We anchor to the saved position id
        // associated with that button (`overflow_button_position_id(id)`,
        // registered via `SavePosition` in `render_overflow_button`), aligning
        // the menu's top-right corner to the button's bottom-right so the menu
        // tucks neatly under the trailing edge of the pill, regardless of how
        // far across the bar that pill happens to be rendered.
        //
        // Otherwise, when no menu is open but a pill is hovered, we overlay
        // the hover details card under that pill instead. The two overlays
        // are mutually exclusive by design: opening the menu clears
        // `hovered_pill` (see `open_menu_for`).
        //
        // Defensive: only render the hover card if the cursor is still
        // genuinely over the pill. The `SetHoveredPill(None)` action that
        // the pill's `on_hover` callback dispatches when the cursor leaves
        // can be missed in edge cases (window-focus changes, layout drops
        // mid-hover, the cursor exiting the app entirely, or a re-entry
        // into a stale Hoverable that suppresses the synthetic
        // hover-out), leaving `hovered_pill` stuck on a stale id and the
        // card visible until something else triggers a re-render. Reading
        // `MouseState::is_mouse_over_element` directly at render time
        // makes the overlay strictly track the cursor: as soon as the
        // pointer moves off the pill, the next render hides the card,
        // regardless of whether the typed-action callback fired.
        let overlay = if let Some(target_id) = self.menu_open_for {
            Some(MenuOrCard::Menu(target_id))
        } else {
            self.hovered_pill.and_then(|id| {
                let mouse_states = self.mouse_states.borrow();
                let still_over_pill = mouse_states
                    .get(&id)
                    .and_then(|handle| handle.lock().ok().map(|s| s.is_mouse_over_element()))
                    .unwrap_or(false);
                drop(mouse_states);
                if !still_over_pill {
                    return None;
                }
                render_hover_card(id, self.agent_view_controller.as_ref(app), app)
                    .map(|card| MenuOrCard::Card { id, card })
            })
        };

        match overlay {
            Some(MenuOrCard::Menu(target_id)) => {
                let mut stack = Stack::new();
                stack.add_child(bar);
                let position_id = overflow_button_position_id(target_id);
                stack.add_positioned_overlay_child(
                    ChildView::new(&self.menu).finish(),
                    OffsetPositioning::from_axes(
                        PositioningAxis::relative_to_stack_child(
                            &position_id,
                            PositionedElementOffsetBounds::WindowByPosition,
                            OffsetType::Pixel(0.),
                            AnchorPair::new(XAxisAnchor::Right, XAxisAnchor::Right),
                        )
                        .with_conditional_anchor(),
                        PositioningAxis::relative_to_stack_child(
                            &position_id,
                            PositionedElementOffsetBounds::WindowByPosition,
                            OffsetType::Pixel(4.),
                            AnchorPair::new(YAxisAnchor::Bottom, YAxisAnchor::Top),
                        )
                        .with_conditional_anchor(),
                    ),
                );
                stack.finish()
            }
            Some(MenuOrCard::Card { id, card }) => {
                let mut stack = Stack::new();
                stack.add_child(bar);
                let position_id = pill_body_position_id(id);
                stack.add_positioned_overlay_child(
                    card,
                    OffsetPositioning::from_axes(
                        PositioningAxis::relative_to_stack_child(
                            &position_id,
                            PositionedElementOffsetBounds::WindowByPosition,
                            OffsetType::Pixel(0.),
                            AnchorPair::new(XAxisAnchor::Left, XAxisAnchor::Left),
                        )
                        .with_conditional_anchor(),
                        PositioningAxis::relative_to_stack_child(
                            &position_id,
                            PositionedElementOffsetBounds::WindowByPosition,
                            OffsetType::Pixel(6.),
                            AnchorPair::new(YAxisAnchor::Bottom, YAxisAnchor::Top),
                        )
                        .with_conditional_anchor(),
                    ),
                );
                stack.finish()
            }
            None => bar,
        }
    }
}

/// Local enum used by `View::render` to model the at-most-one overlay
/// rendered on top of the pill bar (the 3-dot menu *or* the hover details
/// card). Wrapping these in one enum keeps the positioning logic in a
/// single match arm rather than two near-duplicate `if let` branches.
enum MenuOrCard {
    Menu(AIConversationId),
    Card {
        id: AIConversationId,
        card: Box<dyn Element>,
    },
}

/// Builds the hover details card overlay for the given conversation, or
/// returns `None` if there's no conversation to summarise (e.g. the id
/// has just been removed from history). Hidden by `View::render` until the
/// hover-in delay elapses.
///
/// V1 scope keeps the card pragmatic: title + description + a compact
/// chips row showing the agent's harness (placeholder for now), branch
/// (from any PR artifact), and a clickable-looking PR chip. We hide chips
/// whose data is not available rather than showing empty placeholders.
fn render_hover_card(
    conversation_id: AIConversationId,
    _agent_view_controller: &AgentViewController,
    app: &AppContext,
) -> Option<Box<dyn Element>> {
    let history = BlocklistAIHistoryModel::as_ref(app);
    let conversation = history.conversation(&conversation_id)?;

    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let bg = theme.surface_2();
    let main_text = internal_colors::text_main(theme, bg);
    let sub_text = internal_colors::text_sub(theme, bg);
    let outline = theme.outline();

    let name = conversation
        .agent_name()
        .filter(|n| !n.is_empty())
        .map(|n| n.to_string())
        .or_else(|| conversation.title())
        .unwrap_or_else(|| "Agent".to_string());

    // Header: small avatar disc + bold agent name on the left, status
    // badge right-aligned. We use the conversation's `ConversationStatus`
    // (mapped to icon+color via `status_icon_and_color`) to drive the
    // badge so the card matches the colors used elsewhere in the agent
    // details panel.
    let avatar_color = pill_avatar_color(&name, theme);
    let avatar_glyph = AvatarGlyph::Letter(pill_initial(&name));
    let avatar = render_avatar_disc(avatar_color, avatar_glyph, theme, appearance);
    let name_text = Text::new(
        name,
        appearance.ui_font_family(),
        appearance.monospace_font_size(),
    )
    .with_color(main_text)
    .with_style(Properties {
        weight: Weight::Semibold,
        ..Default::default()
    })
    .with_clip(ClipConfig::ellipsis())
    .soft_wrap(false)
    .finish();
    // The orchestrator's `ConversationStatus` reflects its own last
    // exchange's outcome (often `Cancelled` after the user cancels to
    // delegate to subagents, or `Success` once the orchestrator's own
    // streaming finishes), which doesn't usefully describe the state of
    // the orchestration as a whole. Until we plumb an aggregated
    // child-status accessor we hide the badge for the orchestrator pill
    // — child pills still show the (per-child accurate) badge.
    let is_orchestrator = conversation.parent_conversation_id().is_none();
    // Cap the badge at a fixed width so it can't shove the name out of
    // the card. Slightly larger than the longest expected status label
    // ("In progress") plus its icon and padding.
    const STATUS_BADGE_MAX_WIDTH: f32 = 96.;
    let status_badge: Option<Box<dyn Element>> = (!is_orchestrator).then(|| {
        ConstrainedBox::new(render_status_badge(
            conversation.status(),
            theme,
            appearance,
        ))
        .with_max_width(STATUS_BADGE_MAX_WIDTH)
        .finish()
    });
    // Compute the name's max width by subtracting all of the surrounding
    // chrome from the card width: card horizontal padding (12+12), the
    // 16px avatar, the 8px avatar→name gap, an 8px name→badge gap, and
    // the reserved badge slot when one is shown. Without this fixed
    // budget, `MainAxisAlignment::SpaceBetween` would happily push the
    // badge off the right edge of the card whenever the name is long
    // enough to fill the available space (this happened on the
    // orchestrator pill, whose title falls back to the conversation's
    // multi-word title rather than a short agent name).
    let name_max_width = if status_badge.is_some() {
        HOVER_CARD_WIDTH - 24. - 16. - 8. - 8. - STATUS_BADGE_MAX_WIDTH
    } else {
        HOVER_CARD_WIDTH - 24. - 16. - 8.
    };
    let mut header_row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max)
        .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
        .with_child(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min)
                .with_spacing(8.)
                .with_child(avatar)
                .with_child(
                    ConstrainedBox::new(name_text)
                        .with_max_width(name_max_width)
                        .finish(),
                )
                .finish(),
        );
    if let Some(status_badge) = status_badge {
        header_row = header_row.with_child(status_badge);
    }
    let header = header_row.finish();

    // Working directory line: pulled from the root task's first exchange
    // when available, falling back to the most recent exchange. Hidden
    // entirely when neither is populated (e.g. cloud agents whose CWD
    // hasn't synced yet).
    //
    // Use `dirs::home_dir()` (cross-platform: `$HOME` on unix,
    // `%USERPROFILE%` on Windows) to find the home prefix, then defer to
    // the shared `warp_util::path::user_friendly_path` helper so the cwd
    // displays as `~/foo` regardless of OS — and matches the same
    // tilde-substitution behaviour used by the tab title, prompt header,
    // and pwd chip.
    let home_dir = dirs::home_dir();
    let home_dir_str = home_dir.as_deref().and_then(|p| p.to_str());
    let cwd_line: Option<Box<dyn Element>> = conversation
        .initial_working_directory()
        .or_else(|| conversation.current_working_directory())
        .filter(|s| !s.is_empty())
        .map(|cwd| {
            Text::new(
                warp_util::path::user_friendly_path(&cwd, home_dir_str).into_owned(),
                appearance.ui_font_family(),
                appearance.monospace_font_size() - 1.,
            )
            .with_color(main_text)
            .with_clip(ClipConfig::ellipsis())
            .soft_wrap(false)
            .finish()
        });

    // Description: title or initial query, truncated visually via wrapping
    // inside a constrained box.
    let description_text = conversation
        .title()
        .filter(|s| !s.is_empty())
        .or_else(|| conversation.initial_query())
        .filter(|s| !s.is_empty());
    let description: Option<Box<dyn Element>> = description_text.map(|description| {
        let trimmed = if description.chars().count() > 200 {
            let truncated: String = description.chars().take(197).collect();
            format!("{truncated}\u{2026}")
        } else {
            description
        };
        Text::new(
            trimmed,
            appearance.ui_font_family(),
            appearance.monospace_font_size() - 1.,
        )
        .with_color(sub_text)
        .soft_wrap(true)
        .finish()
    });

    // Chips row: branch (if known via a PR artifact) + PR (if known) +
    // harness (always when known). Hidden entirely when no chip applies.
    let mut chips: Vec<Box<dyn Element>> = Vec::new();

    // Harness chip: defaults to Warp Agent (Oz) when server metadata
    // hasn't loaded yet so the chip slot stays useful for in-progress
    // local conversations. The brand color matches `harness_display`
    // (e.g. orange for Claude Code, blue for Gemini CLI).
    let harness = conversation
        .server_metadata()
        .map(|m| Harness::from(m.harness))
        .unwrap_or(Harness::Oz);
    let harness_icon = harness_display::icon_for(harness);
    let harness_label = harness_display::display_name(harness).to_string();
    let harness_color = harness_display::brand_color(harness).unwrap_or(sub_text);
    chips.push(render_chip(
        harness_icon,
        harness_label,
        harness_color,
        main_text,
        theme,
        appearance,
    ));

    for artifact in conversation.artifacts() {
        if let Artifact::PullRequest {
            url: _,
            branch,
            repo,
            number,
        } = artifact
        {
            if !branch.is_empty() {
                chips.push(render_chip(
                    Icon::GitBranch,
                    branch.clone(),
                    sub_text,
                    main_text,
                    theme,
                    appearance,
                ));
            }
            if let (Some(repo), Some(number)) = (repo, number) {
                chips.push(render_chip(
                    Icon::Github,
                    format!("{repo}#{number}"),
                    sub_text,
                    main_text,
                    theme,
                    appearance,
                ));
            }
            // Only one PR artifact is meaningful per conversation; bail.
            break;
        }
    }

    // Assemble.
    let mut column = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_main_axis_size(MainAxisSize::Min)
        .with_spacing(8.)
        .with_child(header);
    if let Some(cwd_line) = cwd_line {
        column = column.with_child(
            ConstrainedBox::new(cwd_line)
                .with_max_width(HOVER_CARD_WIDTH - 24.)
                .finish(),
        );
    }
    if let Some(description) = description {
        column = column.with_child(
            ConstrainedBox::new(description)
                .with_max_width(HOVER_CARD_WIDTH - 24.)
                .finish(),
        );
    }
    if !chips.is_empty() {
        let mut chip_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
            .with_spacing(6.);
        for chip in chips {
            chip_row = chip_row.with_child(chip);
        }
        column = column.with_child(chip_row.finish());
    }

    let card = Container::new(column.finish())
        .with_padding_left(12.)
        .with_padding_right(12.)
        .with_padding_top(10.)
        .with_padding_bottom(10.)
        .with_background(bg)
        .with_border(warpui::elements::Border::all(1.).with_border_fill(outline))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
        .finish();

    Some(
        ConstrainedBox::new(card)
            .with_width(HOVER_CARD_WIDTH)
            .finish(),
    )
}

/// Renders the colored "Working / Done / Error / Cancelled / Blocked"
/// status badge that sits in the top-right of the hover card. Mirrors
/// the visual treatment in `conversation_details_panel::render_status_section`
/// (icon + label, tinted with the same opacity•10 chip background) so
/// the card and the side panel can't drift.
fn render_status_badge(
    status: &ConversationStatus,
    theme: &WarpTheme,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let (icon, color) = status.status_icon_and_color(theme);
    let icon_el = ConstrainedBox::new(icon.to_warpui_icon(color.into()).finish())
        .with_width(12.)
        .with_height(12.)
        .finish();
    let label = Text::new(
        status.to_string(),
        appearance.ui_font_family(),
        appearance.monospace_font_size() - 2.,
    )
    .with_color(color)
    .soft_wrap(false)
    .finish();
    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min)
        .with_spacing(4.)
        .with_child(icon_el)
        .with_child(label)
        .finish();
    Container::new(row)
        .with_padding_left(6.)
        .with_padding_right(6.)
        .with_padding_top(2.)
        .with_padding_bottom(2.)
        .with_background_color(coloru_with_opacity(color, 10))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .finish()
}

/// Renders a small icon + label chip used inside the hover details card.
fn render_chip(
    icon: Icon,
    label: String,
    icon_color: ColorU,
    text_color: ColorU,
    theme: &WarpTheme,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let icon_el = ConstrainedBox::new(icon.to_warpui_icon(icon_color.into()).finish())
        .with_width(12.)
        .with_height(12.)
        .finish();
    let text = Text::new(
        label,
        appearance.ui_font_family(),
        appearance.monospace_font_size() - 2.,
    )
    .with_color(text_color)
    .soft_wrap(false)
    .with_clip(ClipConfig::ellipsis())
    .finish();
    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min)
        .with_spacing(4.)
        .with_child(icon_el)
        .with_child(
            ConstrainedBox::new(text)
                .with_max_width(HOVER_CARD_WIDTH - 60.)
                .finish(),
        )
        .finish();
    Container::new(row)
        .with_padding_left(6.)
        .with_padding_right(6.)
        .with_padding_top(2.)
        .with_padding_bottom(2.)
        .with_background(internal_colors::neutral_2(theme))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .finish()
}

fn render_pill(
    spec: PillSpec,
    mouse_state: MouseStateHandle,
    overflow_mouse_state: MouseStateHandle,
    menu_is_open_for_this: bool,
    self_terminal_view_id: warpui::EntityId,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let conversation_id = spec.conversation_id;
    let kind = spec.kind;
    let is_selected = spec.is_selected;
    let pin_state = spec.pin_state;
    let is_pinned = matches!(pin_state, PillPinState::PinnedInOtherPane);
    let show_overflow_button = matches!(kind, PillKind::Child);
    // `spec` is owned by value, so we can move `label` directly into the
    // build closure below without cloning.
    let label = spec.label;
    let avatar_color = spec.avatar_color;
    let avatar_glyph = spec.avatar_glyph;

    // `Hoverable::new`'s build closure is `FnOnce` (see
    // `crates/warpui_core/src/elements/hoverable.rs`). We can therefore move
    // `label` into the closure by value rather than cloning it on every
    // build.
    let pill_body = Hoverable::new(mouse_state, move |hover_state| {
        // Highlight the pill only when it's the currently active
        // conversation. Opening the 3-dot menu on a *non-active* pill
        // should not change that pill's appearance — the menu itself is
        // a separate overlay and the user expects only the truly
        // selected agent's pill to read as selected.
        let (background, text_color) = if is_selected {
            (
                theme.foreground().into_solid(),
                theme.background().into_solid(),
            )
        } else if hover_state.is_hovered() || hover_state.is_clicked() || menu_is_open_for_this {
            (
                warp_core::ui::theme::color::internal_colors::neutral_3(theme),
                warp_core::ui::theme::color::internal_colors::text_main(theme, theme.background()),
            )
        } else {
            (
                warp_core::ui::theme::color::internal_colors::neutral_2(theme),
                warp_core::ui::theme::color::internal_colors::text_main(theme, theme.background()),
            )
        };

        // Reserve room for the 3-dot button on every child pill, even
        // at rest. Switching label_max_width based on hover would cause
        // the pill to *shrink* when the dots appear (the label would
        // suddenly clip earlier, which propagates outward through Min
        // sizing), making sibling pills shift. By always using the
        // shorter budget for child pills we get a stable pill width
        // independent of hover state: short labels are well under either
        // budget so they don't grow the pill, and labels near the limit
        // always clip to the same width so the dots overlay never
        // overlaps text. Orchestrator pills don't host a 3-dot button
        // so they keep the full label budget.
        let label_max_width = if show_overflow_button {
            (PILL_LABEL_MAX_WIDTH - OVERFLOW_BUTTON_SIZE - 2.).max(0.)
        } else {
            PILL_LABEL_MAX_WIDTH
        };
        let show_dots = show_overflow_button && (hover_state.is_hovered() || menu_is_open_for_this);

        let label_text = Text::new(
            label,
            appearance.ui_font_family(),
            appearance.monospace_font_size() - 1.,
        )
        .with_color(text_color)
        .soft_wrap(false)
        .with_clip(ClipConfig::ellipsis())
        .with_style(Properties {
            weight: if is_selected {
                Weight::Semibold
            } else {
                Weight::Normal
            },
            ..Default::default()
        })
        .finish();

        // Pinned pills swap the avatar disc for a pin glyph (per Figma) so
        // the user can spot at a glance that this child is currently living
        // in a separate pane/tab. Unpinned pills keep the avatar disc.
        let leading: Box<dyn Element> = if is_pinned {
            ConstrainedBox::new(Icon::Pin.to_warpui_icon(text_color.into()).finish())
                .with_width(AVATAR_SIZE)
                .with_height(AVATAR_SIZE)
                .finish()
        } else {
            render_avatar_disc(avatar_color, avatar_glyph, theme, appearance)
        };

        // Body row contains just the avatar + label — the 3-dot button
        // is rendered as a positioned overlay (below) so it doesn't take
        // a slot in this row. That means the pill's intrinsic width is
        // determined by the label alone, and the dots can visually clip
        // the trailing edge of the text when shown without making the
        // pill itself wider or shifting siblings.
        let row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
            .with_spacing(6.)
            .with_child(leading)
            .with_child(
                ConstrainedBox::new(label_text)
                    .with_max_width(label_max_width)
                    .finish(),
            )
            .finish();

        // Constrain pill to a fixed height so the half-stadium corner radius
        // renders as a clean continuous shape rather than awkwardly clamping.
        let pill_inner = ConstrainedBox::new(
            Container::new(row)
                .with_padding_left(PILL_HORIZONTAL_PADDING_LEFT)
                .with_padding_right(PILL_HORIZONTAL_PADDING_RIGHT)
                .with_background_color(background)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(PILL_RADIUS)))
                .finish(),
        )
        .with_height(PILL_HEIGHT)
        .finish();

        // Render the 3-dot button as a positioned overlay only when the
        // pill is being hovered (or its 3-dot menu is already open). The
        // overlay sits at the trailing edge of the pill; the label above
        // already shortens its max width when `show_dots` is true so the
        // ellipsis truncates before reaching the dots rather than running
        // underneath them. The pill's outer width still doesn't change
        // between rest and hover.
        if show_dots {
            let mut stack = Stack::new();
            stack.add_child(pill_inner);
            stack.add_positioned_child(
                render_overflow_button(
                    overflow_mouse_state.clone(),
                    conversation_id,
                    text_color.into(),
                    theme,
                ),
                OffsetPositioning::offset_from_parent(
                    vec2f(-PILL_HORIZONTAL_PADDING_RIGHT + 4., 0.),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::MiddleRight,
                    ChildAnchor::MiddleRight,
                ),
            );
            stack.finish()
        } else {
            pill_inner
        }
    })
    .with_cursor(if is_selected {
        Cursor::Arrow
    } else {
        Cursor::PointingHand
    })
    // The 3-dot overflow button is a child Hoverable on top of this one.
    // Without `defer_events_to_children`, both the inner and outer click
    // handlers fire for the same mouse-up — the overflow button opens the
    // menu *and* the pill body switches the agent view in place. Defer
    // skips the outer click whenever a child already handled it so the
    // 3-dot click only opens the menu.
    .with_defer_events_to_children()
    .with_hover_in_delay(HOVER_CARD_IN_DELAY)
    .with_hover_out_delay(HOVER_CARD_OUT_DELAY)
    .on_hover(move |is_hovered, ctx, _app, _pos| {
        // Drive the hover-details-card overlay via a typed action so the
        // pill bar's `handle_action` can update its `hovered_pill` field
        // and re-render. We pass the conversation id on hover-in and
        // `None` on hover-out; if the pointer scrubs from one pill to
        // another the new pill's hover-in arrives after this pill's
        // hover-out, so the action surface still ends up with the
        // correct id.
        let payload = if is_hovered {
            Some(conversation_id)
        } else {
            None
        };
        ctx.dispatch_typed_action(OrchestrationPillBarAction::SetHoveredPill(payload));
    })
    .on_click(move |ctx, app, _| {
        if is_selected {
            return;
        }
        let _ = kind;
        // Single source of truth: if the conversation is currently owned
        // by a *different* visible terminal view than this orchestrator
        // pane (because it was split off into a separate pane or tab),
        // the pill should focus that existing pane rather than re-render
        // the conversation in place. Route through the pill bar's own
        // `FocusOpenedConversation` action so this path and the 3-dot
        // menu's "Focus pane" item share a single implementation — the
        // pill bar's `handle_action` then dispatches
        // `WorkspaceAction::FocusTerminalViewInWorkspace` from a
        // `ViewContext<Self>`, which reliably reaches the workspace.
        let is_open_elsewhere =
            is_conversation_open_in_other_visible_view(conversation_id, self_terminal_view_id, app);
        if is_open_elsewhere {
            ctx.dispatch_typed_action(OrchestrationPillBarAction::FocusOpenedConversation(
                conversation_id,
            ));
            return;
        }
        // Pinned pills focus the existing pane/tab that already hosts this
        // child agent (via `RevealChildAgent`, which the pane group treats
        // as a request to show + focus an existing child pane). Unpinned
        // pills navigate the *current* pane in place via
        // `SwitchAgentViewToConversation`. Both paths bubble through
        // `PaneHeaderAction::CustomAction` because the pill bar lives
        // inside the pane header chrome (mirrors `agent_view_back_button`).
        let action = if is_pinned {
            TerminalAction::RevealChildAgent { conversation_id }
        } else {
            TerminalAction::SwitchAgentViewToConversation { conversation_id }
        };
        ctx.dispatch_typed_action(
            PaneHeaderAction::<TerminalAction, TerminalAction>::CustomAction(action),
        );
    })
    .finish();

    // Cache the painted rect of this pill body under a stable id so the
    // hover details card overlay (rendered as a positioned overlay sibling
    // of the bar in `View::render`) can anchor relative to it without
    // having to know which index this pill ended up at in the row.
    SavePosition::new(pill_body, &pill_body_position_id(conversation_id)).finish()
}

/// Renders the trailing 3-dot button on a child pill. Click dispatches
/// `OrchestrationPillBarAction::OpenMenu(conversation_id)` as a typed action
/// up to the pill bar's `handle_action`, which rebuilds the menu items for
/// that child id and toggles `menu_open_for` on. We use a separate inner
/// `Hoverable` so the button has its own hover highlight independent of the
/// surrounding pill body.
///
/// The button is wrapped in a `SavePosition` so the open menu can anchor
/// itself directly beneath this specific pill's button (see
/// `View::render`); without the saved position, the menu would have to fall
/// back to a bar-relative offset which doesn't track which pill is active.
fn render_overflow_button(
    mouse_state: MouseStateHandle,
    conversation_id: AIConversationId,
    text_color: Fill,
    theme: &WarpTheme,
) -> Box<dyn Element> {
    let button = Hoverable::new(mouse_state, move |hover_state| {
        // The button's own surface gets a subtle filled background when
        // hovered or pressed so it reads as a discrete clickable target
        // even though it sits on top of the pill body's own highlight.
        let bg = if hover_state.is_hovered() || hover_state.is_clicked() {
            Some(internal_colors::fg_overlay_1(theme))
        } else {
            None
        };
        let icon = ConstrainedBox::new(Icon::DotsVertical.to_warpui_icon(text_color).finish())
            .with_width(OVERFLOW_BUTTON_SIZE)
            .with_height(OVERFLOW_BUTTON_SIZE)
            .finish();
        let mut container =
            Container::new(icon).with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)));
        if let Some(bg) = bg {
            container = container.with_background(bg);
        }
        ConstrainedBox::new(container.finish())
            .with_height(OVERFLOW_BUTTON_SIZE + 2.)
            .finish()
    })
    .with_cursor(Cursor::PointingHand)
    .on_click(move |ctx, _app, _| {
        // The outer pill body is configured with
        // `with_defer_events_to_children`, so this child Hoverable is
        // allowed to consume the click event and the outer pill's
        // `SwitchAgentViewToConversation` handler is skipped for the
        // same mouse-up. That keeps a click on the dots strictly to
        // "open the menu" without also switching the agent view.
        ctx.dispatch_typed_action(OrchestrationPillBarAction::OpenMenu(conversation_id));
    })
    .finish();

    // Cache this button's painted rect under a stable id so the open menu
    // (rendered as a positioned overlay sibling of the bar in `View::render`)
    // can anchor relative to it.
    SavePosition::new(button, &overflow_button_position_id(conversation_id)).finish()
}

/// Renders the avatar circle as a colored disc with a centered glyph (letter
/// or icon) on top. Uses `Stack` so the disc is a clean rounded square that
/// composites cleanly over the pill's own background without visual seams.
fn render_avatar_disc(
    avatar_color: ColorU,
    glyph: AvatarGlyph,
    theme: &WarpTheme,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let disc = ConstrainedBox::new(
        Container::new(Empty::new().finish())
            .with_background_color(avatar_color)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(AVATAR_SIZE / 2.)))
            .finish(),
    )
    .with_width(AVATAR_SIZE)
    .with_height(AVATAR_SIZE)
    .finish();

    let glyph_element: Box<dyn Element> = match glyph {
        AvatarGlyph::Letter(letter) => Text::new(
            letter.to_string(),
            appearance.ui_font_family(),
            (appearance.monospace_font_size() - 2.).max(9.),
        )
        .with_color(theme.background().into_solid())
        .with_style(Properties {
            weight: Weight::Bold,
            ..Default::default()
        })
        .finish(),
        AvatarGlyph::Icon(icon) => {
            ConstrainedBox::new(icon.to_warpui_icon(theme.background()).finish())
                .with_width(10.)
                .with_height(10.)
                .finish()
        }
    };

    // Center the glyph on top of the disc both horizontally and vertically by
    // using `MainAxisAlignment::Center` (along axis) and
    // `CrossAxisAlignment::Center` (perpendicular) on both Flex containers.
    let glyph_centered = ConstrainedBox::new(
        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(glyph_element)
                    .finish(),
            )
            .finish(),
    )
    .with_width(AVATAR_SIZE)
    .with_height(AVATAR_SIZE)
    .finish();

    Stack::new()
        .with_child(disc)
        .with_child(glyph_centered)
        .finish()
}

/// Pre-computed data for a single breadcrumb in the orchestration breadcrumb row.
struct CrumbSpec {
    conversation_id: AIConversationId,
    label: String,
    avatar_color: ColorU,
    avatar_glyph: AvatarGlyph,
    /// `true` for the trailing crumb (the conversation currently being
    /// viewed). The trailing crumb is rendered with a brighter text color
    /// and is non-interactive.
    is_active: bool,
}

const CRUMB_HEIGHT: f32 = 24.;
const CRUMB_RADIUS: f32 = 4.;
const CRUMB_HORIZONTAL_PADDING: f32 = 6.;

/// Returns `true` if `conversation_id` is canonically owned (per
/// `BlocklistAIHistoryModel::live_conversation_ids_for_terminal_view`) by
/// some *visible* terminal view that is not `self_terminal_view_id`. Used
/// by the orchestration pill bar to decide between "open in new pane / new
/// tab" (when no other visible pane shows the conversation) and "focus
/// pane" (when one does).
///
/// `BlocklistAIHistoryModel` is the single source of truth for which
/// terminal view renders a given conversation's AI blocks (see
/// `ConversationOwnershipTransferred`), so it correctly reflects the
/// orchestrator after an in-place switch and the new pane after a split.
/// However, before any user interaction the canonical owner is the
/// hidden child-agent pane created by
/// `create_hidden_child_agent_conversation` — a real terminal view that
/// we deliberately keep off-screen. Using only the history-model owner
/// here would treat that hidden pane as "elsewhere" and falsely surface
/// "Focus pane" for every child the user has not yet opened.
///
/// The visible-pane filter resolves both edge cases: walking
/// `Workspace::tab_views()` and consulting `PaneGroup::visible_pane_ids()`
/// (which excludes hidden-for-child-agent / hidden-for-close / etc.
/// panes) confirms the owner is a pane the user can actually navigate to.
fn is_conversation_open_in_other_visible_view(
    conversation_id: AIConversationId,
    self_terminal_view_id: EntityId,
    app: &AppContext,
) -> bool {
    let Some(owner) =
        BlocklistAIHistoryModel::as_ref(app).terminal_view_id_for_conversation(&conversation_id)
    else {
        return false;
    };
    if owner == self_terminal_view_id {
        return false;
    }
    let registry = WorkspaceRegistry::as_ref(app);
    for (_, workspace_handle) in registry.all_workspaces(app) {
        let workspace = workspace_handle.as_ref(app);
        for pane_group_handle in workspace.tab_views() {
            let pane_group = pane_group_handle.as_ref(app);
            for pane_id in pane_group.visible_pane_ids() {
                if let Some(terminal_view) = pane_group.terminal_view_from_pane_id(pane_id, app) {
                    if terminal_view.id() == owner {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Walks every visible terminal pane across every workspace/tab and
/// returns the `EntityId` of the `PaneGroup` that contains the given
/// `terminal_view_id`, if any. Used by the pill bar to decide between
/// the same-pane-group focus path (`RevealChildAgent`) and the
/// cross-pane-group path (`FocusTerminalViewInWorkspace`).
fn pane_group_id_containing_terminal_view(
    terminal_view_id: EntityId,
    app: &AppContext,
) -> Option<EntityId> {
    let registry = WorkspaceRegistry::as_ref(app);
    for (_, workspace_handle) in registry.all_workspaces(app) {
        let workspace = workspace_handle.as_ref(app);
        for pane_group_handle in workspace.tab_views() {
            let pane_group = pane_group_handle.as_ref(app);
            for pane_id in pane_group.visible_pane_ids() {
                if let Some(terminal_view) = pane_group.terminal_view_from_pane_id(pane_id, app) {
                    if terminal_view.id() == terminal_view_id {
                        return Some(pane_group_handle.id());
                    }
                }
            }
        }
    }
    None
}

/// Returns `true` if the active conversation in `agent_view_controller` is a
/// child agent that has been opened in a *different* terminal view than this
/// one — i.e. "Open in new pane" or "Open in new tab" was used to split it
/// off from its orchestrator. We use this to decide whether the pane header
/// should render breadcrumbs (split-off) vs. the pill bar (same-pane).
///
/// Specifically: the active conversation is a child AND the *parent*
/// (orchestrator) is currently the active conversation in some other
/// terminal view (per `ActiveAgentViewsModel`). When the orchestrator pane
/// switches in place to a child, the parent is no longer the active
/// conversation in any pane, so this returns `false` and the pill bar keeps
/// rendering with the active child highlighted.
pub fn is_split_off_child(agent_view_controller: &AgentViewController, app: &AppContext) -> bool {
    let Some(active_id) = agent_view_controller
        .agent_view_state()
        .active_conversation_id()
    else {
        return false;
    };
    let history = BlocklistAIHistoryModel::as_ref(app);
    let Some(active_conversation) = history.conversation(&active_id) else {
        return false;
    };
    let Some(parent_id) = parent_conversation_id(active_conversation, app) else {
        return false;
    };
    let Some(parent_view_id) =
        ActiveAgentViewsModel::as_ref(app).terminal_view_id_for_conversation(parent_id, app)
    else {
        return false;
    };
    parent_view_id != agent_view_controller.terminal_view_id()
}

/// Renders a `[Parent Avatar] [Parent Title] > [Child Avatar] [Child Name]`
/// breadcrumb row when the active conversation is a child agent under an
/// orchestrator that has been split off into another pane/tab. Returns
/// `None` for same-pane child views — those render the pill bar with the
/// active child highlighted instead.
///
/// We render this manually rather than going through
/// `crate::ui_components::breadcrumb::render_breadcrumbs` because we need a
/// chevron separator (per the Figma) and per-crumb avatars, neither of which
/// the shared helper supports today.
///
/// `parent_crumb_mouse_state` must be a `MouseStateHandle` owned by the caller
/// (e.g. on a TerminalView field) so hover and click events persist across
/// renders. Inline `MouseStateHandle::default()` would zero state every frame
/// and silently break clicks (per the WarpUI mouse-state guidance).
pub fn render_orchestration_breadcrumbs(
    agent_view_controller: &AgentViewController,
    parent_crumb_mouse_state: MouseStateHandle,
    horizontal_scroll_state: ClippedScrollStateHandle,
    app: &AppContext,
) -> Option<Box<dyn Element>> {
    // Mirror the gating used by `maybe_add_parent_navigation_card` in
    // `pane_impl.rs` so the breadcrumb path can't accidentally render in a
    // non-AgentView build / state.
    if !FeatureFlag::AgentView.is_enabled() {
        return None;
    }
    if !FeatureFlag::OrchestrationPillBar.is_enabled() {
        return None;
    }
    if !agent_view_controller.is_fullscreen() {
        return None;
    }
    // V2: only render breadcrumbs from a *split-off* pane/tab. Same-pane
    // child views render the pill bar with the active child highlighted.
    if !is_split_off_child(agent_view_controller, app) {
        return None;
    }
    let active_id = agent_view_controller
        .agent_view_state()
        .active_conversation_id()?;
    let history = BlocklistAIHistoryModel::as_ref(app);
    let active = history.conversation(&active_id)?;
    let parent_id = parent_conversation_id(active, app)?;
    // The parent's `AIConversation` may not yet be loaded into
    // `conversations_by_id` (e.g. a child agent restored on startup whose
    // parent is only known via the `children_by_parent` index — see
    // `pane_group/mod.rs`'s `TODO(QUALITY-378)`). In that case we still want
    // to render a clickable parent crumb so the user can navigate back to
    // the orchestrator: `SwitchAgentViewToConversation` will load the parent
    // through the normal `enter_agent_view_for_conversation` path. Bailing
    // out here would otherwise leave the user with no "back to parent"
    // affordance, since the new flag also suppresses the legacy parent card.
    let parent = history.conversation(&parent_id);

    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();

    // Prefer the parent's user-visible title; fall back to its agent name,
    // and finally to a generic "Orchestrator" label so the breadcrumb is
    // always meaningful even before titles have been generated (or before
    // the parent conversation itself has been loaded).
    let parent_label = parent
        .and_then(|p| {
            p.title()
                .filter(|t| !t.is_empty())
                .or_else(|| p.agent_name().map(str::to_string))
        })
        .unwrap_or_else(|| "Orchestrator".to_string());

    // Treat empty `agent_name` as missing so the label, avatar color, and
    // initial all consistently fall back to "Agent". Without the
    // `.filter(|n| !n.is_empty())` on `child_name`, an unnamed agent would
    // show "Agent" as the label but be hashed/initialed against the empty
    // string, producing a different color/letter from a real "Agent".
    let child_name = active
        .agent_name()
        .filter(|n| !n.is_empty())
        .unwrap_or("Agent");
    let child_label = child_name.to_string();

    // Parent crumb uses the Oz glyph on a neutral disc to match the
    // orchestrator pill in the pill bar.
    let parent_spec = CrumbSpec {
        conversation_id: parent_id,
        label: parent_label,
        avatar_color: theme.ansi_fg_cyan(),
        avatar_glyph: AvatarGlyph::Icon(Icon::Oz),
        is_active: false,
    };

    // Child crumb uses the same deterministic colored disc + initial letter
    // we render in the pill bar.
    let child_spec = CrumbSpec {
        conversation_id: active_id,
        label: child_label,
        avatar_color: pill_avatar_color(child_name, theme),
        avatar_glyph: AvatarGlyph::Letter(pill_initial(child_name)),
        is_active: true,
    };

    let chevron_color = internal_colors::text_sub(theme, theme.background());
    let chevron = ConstrainedBox::new(
        Icon::ChevronRight
            .to_warpui_icon(chevron_color.into())
            .finish(),
    )
    .with_width(16.)
    .with_height(16.)
    .finish();

    // The row uses `MainAxisSize::Min` so its intrinsic width is the sum
    // of the crumbs (avatar + label per crumb plus spacing). Wrapping that
    // row in a horizontal `NewScrollable` lets the user pan through the
    // breadcrumbs whenever the title slot is too narrow to fit them —
    // common when the orchestrator was opened in a split-off pane that's
    // been resized down. With `MainAxisSize::Max` the row would always
    // try to fill the title slot which makes the inner content unscrollable.
    let mut row = Flex::row()
        .with_main_axis_alignment(MainAxisAlignment::Start)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min)
        .with_spacing(4.);
    let self_terminal_view_id = agent_view_controller.terminal_view_id();
    row.add_child(render_crumb(
        parent_spec,
        Some(parent_crumb_mouse_state),
        self_terminal_view_id,
        theme,
        appearance,
    ));
    row.add_child(chevron);
    row.add_child(render_crumb(
        child_spec,
        None,
        self_terminal_view_id,
        theme,
        appearance,
    ));

    let scrollable = NewScrollable::horizontal(
        SingleAxisConfig::Clipped {
            handle: horizontal_scroll_state,
            child: row.finish(),
        },
        theme.nonactive_ui_detail().into(),
        theme.active_ui_detail().into(),
        ElementFill::None,
    )
    // Pass `true` for `overlaid_scrollbar` so the horizontal scrollbar
    // paints on top of the row instead of stealing vertical space below
    // it. Reserving space pushes the breadcrumbs upward (off-center)
    // whenever the row overflows; overlaying keeps the row vertically
    // centered in the title slot at the cost of the scrollbar briefly
    // crossing through the bottom edge of the labels — which the user
    // explicitly accepted as a fine trade-off.
    .with_horizontal_scrollbar(ScrollableAppearance::new(ScrollbarWidth::Auto, true))
    .with_propagate_mousewheel_if_not_handled(true)
    .finish();
    Some(scrollable)
}

fn render_crumb(
    spec: CrumbSpec,
    mouse_state: Option<MouseStateHandle>,
    self_terminal_view_id: EntityId,
    theme: &WarpTheme,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let conversation_id = spec.conversation_id;
    let is_active = spec.is_active;
    let label = spec.label;
    let avatar_color = spec.avatar_color;
    let avatar_glyph = spec.avatar_glyph;

    // Active (trailing) crumb: bright text, no hover/click. Use the same
    // height + padding as the interactive crumb so the row is uniform.
    if is_active {
        let inner = build_crumb_inner(
            label,
            avatar_color,
            avatar_glyph,
            true,  /* is_active */
            false, /* is_hovered */
            theme,
            appearance,
        );
        return ConstrainedBox::new(inner)
            .with_height(CRUMB_HEIGHT)
            .finish();
    }

    // Interactive (parent) crumb: hover highlight + click handler. The
    // `Hoverable::new` build closure is `FnOnce`, so `label` can move into
    // the closure by value instead of cloning on every build.
    let mouse_state = mouse_state.unwrap_or_default();
    Hoverable::new(mouse_state, move |hover_state| {
        let inner = build_crumb_inner(
            label,
            avatar_color,
            avatar_glyph,
            false, /* is_active */
            hover_state.is_hovered() || hover_state.is_clicked(),
            theme,
            appearance,
        );
        ConstrainedBox::new(inner)
            .with_height(CRUMB_HEIGHT)
            .finish()
    })
    .with_cursor(Cursor::PointingHand)
    .on_click(move |ctx, app, _| {
        // Focus the pane that already hosts the parent conversation
        // rather than switching this (split-off child) pane to it.
        //
        // Pick the focus path based on where the parent's canonical
        // owner pane lives, mirroring the orchestration pill bar's
        // "Focus pane" handler:
        //   * Same pane group as us (sibling pane in this tab) —
        //     dispatch `TerminalAction::RevealChildAgent`, which the
        //     pane group handles by walking visible terminal panes and
        //     focusing the one whose active conversation matches.
        //     Going through the workspace's `focus_pane` from a
        //     different `ViewContext` doesn't reliably move focus when
        //     the destination is in the same pane group.
        //   * Different pane group (other tab / window) — dispatch
        //     `WorkspaceAction::FocusTerminalViewInWorkspace`, which
        //     walks all tabs/windows and activates the containing tab
        //     as needed.
        //   * No canonical owner anywhere — fall back to
        //     `SwitchAgentViewToConversation` so the breadcrumb stays
        //     useful even after the orchestrator pane has been closed
        //     and the parent conversation only persists in history.
        if let Some(owner_view_id) =
            BlocklistAIHistoryModel::as_ref(app).terminal_view_id_for_conversation(&conversation_id)
        {
            let self_pane_group_id =
                pane_group_id_containing_terminal_view(self_terminal_view_id, app);
            let owner_pane_group_id = pane_group_id_containing_terminal_view(owner_view_id, app);
            if owner_pane_group_id.is_some() && owner_pane_group_id == self_pane_group_id {
                ctx.dispatch_typed_action(
                    PaneHeaderAction::<TerminalAction, TerminalAction>::CustomAction(
                        TerminalAction::RevealChildAgent { conversation_id },
                    ),
                );
                return;
            }
            ctx.dispatch_typed_action(WorkspaceAction::FocusTerminalViewInWorkspace {
                terminal_view_id: owner_view_id,
            });
            return;
        }
        ctx.dispatch_typed_action(
            PaneHeaderAction::<TerminalAction, TerminalAction>::CustomAction(
                TerminalAction::SwitchAgentViewToConversation { conversation_id },
            ),
        );
    })
    .finish()
}

/// Builds the inner content (background + padding + avatar + label row) for a
/// single crumb. Shared between active (non-interactive) and interactive paths
/// so both render at the same height with consistent padding.
fn build_crumb_inner(
    label: String,
    avatar_color: ColorU,
    avatar_glyph: AvatarGlyph,
    is_active: bool,
    is_hovered: bool,
    theme: &WarpTheme,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let text_color = if is_active || is_hovered {
        internal_colors::text_main(theme, theme.background())
    } else {
        internal_colors::text_sub(theme, theme.background())
    };

    let label_text = Text::new(
        label,
        appearance.ui_font_family(),
        appearance.monospace_font_size(),
    )
    .with_color(text_color)
    .soft_wrap(false)
    .with_clip(ClipConfig::ellipsis())
    .finish();

    let avatar = render_avatar_disc(avatar_color, avatar_glyph, theme, appearance);

    let row = Flex::row()
        .with_main_axis_alignment(MainAxisAlignment::Center)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min)
        .with_spacing(6.)
        .with_child(avatar)
        .with_child(
            ConstrainedBox::new(label_text)
                .with_max_width(220.)
                .finish(),
        )
        .finish();

    let mut container = Container::new(row)
        .with_padding_left(CRUMB_HORIZONTAL_PADDING)
        .with_padding_right(CRUMB_HORIZONTAL_PADDING)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(CRUMB_RADIUS)));
    if is_hovered && !is_active {
        container = container.with_background_color(internal_colors::neutral_2(theme));
    }
    container.finish()
}
