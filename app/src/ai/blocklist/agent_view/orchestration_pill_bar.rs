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

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warp_core::ui::theme::Fill;
use warp_core::ui::{appearance::Appearance, theme::WarpTheme};
use warpui::elements::{
    ChildAnchor, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Element,
    Empty, Flex, Hoverable, MainAxisAlignment, MainAxisSize, MouseStateHandle, OffsetPositioning,
    ParentAnchor, ParentElement, ParentOffsetBounds, Radius, Stack, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::platform::Cursor;
use warpui::text_layout::ClipConfig;
use warpui::{
    AppContext, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use crate::ai::active_agent_views_model::ActiveAgentViewsModel;
use crate::ai::agent::conversation::{AIConversation, AIConversationId};
use crate::ai::blocklist::agent_view::orchestration_conversation_links::parent_conversation_id;
use crate::ai::blocklist::agent_view::{AgentViewController, AgentViewControllerEvent};
use crate::ai::blocklist::{BlocklistAIHistoryEvent, BlocklistAIHistoryModel};
use crate::features::FeatureFlag;
use crate::menu::{Event as MenuEvent, Menu, MenuItem, MenuItemFields};
use crate::pane_group::pane::view::PaneHeaderAction;
use crate::terminal::view::TerminalAction;
use crate::ui_components::icons::Icon;
use warp_core::ui::theme::color::internal_colors;

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
    Stop(AIConversationId),
    /// Menu item: cancel any in-flight task and remove the conversation
    /// from local history (no server delete).
    Kill(AIConversationId),
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

        let items = vec![
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
            MenuItem::Separator,
            item(
                "Stop agent",
                Icon::Stop,
                OrchestrationPillBarAction::Stop(conversation_id),
            ),
            item(
                "Kill agent",
                Icon::Trash,
                OrchestrationPillBarAction::Kill(conversation_id),
            ),
        ];

        self.menu.update(ctx, |menu, ctx| {
            menu.set_items(items, ctx);
        });
        self.menu_open_for = Some(conversation_id);
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
        for id in &alive {
            mouse_states.entry(*id).or_default();
        }
        mouse_states.retain(|id, _| alive.contains(id));
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

        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
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
                app,
            ));
        }
        drop(mouse_states);
        drop(overflow_states);

        // Wrap in a container with a touch of horizontal padding so the bar
        // doesn't sit flush against the pane edges, and with the same overlay
        // background as the rest of the agent view header so it merges visually.
        let bar = Container::new(row.finish())
            .with_padding_left(12.)
            .with_padding_right(12.)
            .with_padding_top(4.)
            .with_padding_bottom(4.)
            .with_background(theme.surface_overlay_1())
            .finish();

        // When the 3-dot menu is open, overlay it beneath the pill bar
        // anchored to the bar's bottom-left. Precise alignment under the
        // specific clicked pill would require capturing per-pill layout
        // bounds, which we don't have plumbed through yet; landing the menu
        // beneath the bar keeps interaction working and the menu visible
        // until that polish lands. Tracked as a follow-up.
        if self.menu_open_for.is_some() {
            let mut stack = Stack::new();
            stack.add_child(bar);
            stack.add_positioned_overlay_child(
                ChildView::new(&self.menu).finish(),
                OffsetPositioning::offset_from_parent(
                    vec2f(12., 4.),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::BottomLeft,
                    ChildAnchor::TopLeft,
                ),
            );
            stack.finish()
        } else {
            bar
        }
    }
}

fn render_pill(
    spec: PillSpec,
    mouse_state: MouseStateHandle,
    overflow_mouse_state: MouseStateHandle,
    menu_is_open_for_this: bool,
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
        let (background, text_color) = if is_selected || menu_is_open_for_this {
            (
                theme.foreground().into_solid(),
                theme.background().into_solid(),
            )
        } else if hover_state.is_hovered() || hover_state.is_clicked() {
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

        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
            .with_spacing(6.)
            .with_child(leading)
            .with_child(
                ConstrainedBox::new(label_text)
                    .with_max_width(PILL_LABEL_MAX_WIDTH)
                    .finish(),
            );
        if show_overflow_button {
            // Render the 3-dot button as part of the pill body so the
            // pill's own pill-shaped background and corner radius extend
            // visually past the trailing edge of the label. Extra left
            // gap keeps the button visually distinct from the label.
            row = row.with_child(render_overflow_button(
                overflow_mouse_state.clone(),
                conversation_id,
                text_color.into(),
                theme,
            ));
        }
        let row = row.finish();

        // Constrain pill to a fixed height so the half-stadium corner radius
        // renders as a clean continuous shape rather than awkwardly clamping.
        // Use a tighter trailing pad when the overflow button is present so
        // the button doesn't sit flush against the pill's curved edge.
        let trailing_pad = if show_overflow_button {
            4.
        } else {
            PILL_HORIZONTAL_PADDING_RIGHT
        };
        ConstrainedBox::new(
            Container::new(row)
                .with_padding_left(PILL_HORIZONTAL_PADDING_LEFT)
                .with_padding_right(trailing_pad)
                .with_background_color(background)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(PILL_RADIUS)))
                .finish(),
        )
        .with_height(PILL_HEIGHT)
        .finish()
    })
    .with_cursor(if is_selected {
        Cursor::Arrow
    } else {
        Cursor::PointingHand
    })
    .on_click(move |ctx, _app, _| {
        if is_selected {
            return;
        }
        let _ = kind;
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

    pill_body
}

/// Renders the trailing 3-dot button on a child pill. Click dispatches
/// `OrchestrationPillBarAction::OpenMenu(conversation_id)` as a typed action
/// up to the pill bar's `handle_action`, which rebuilds the menu items for
/// that child id and toggles `menu_open_for` on. We use a separate inner
/// `Hoverable` so the button has its own hover highlight independent of the
/// surrounding pill body.
fn render_overflow_button(
    mouse_state: MouseStateHandle,
    conversation_id: AIConversationId,
    text_color: Fill,
    theme: &WarpTheme,
) -> Box<dyn Element> {
    Hoverable::new(mouse_state, move |hover_state| {
        let bg = if hover_state.is_hovered() || hover_state.is_clicked() {
            Some(internal_colors::fg_overlay_1(theme))
        } else {
            None
        };
        let icon = ConstrainedBox::new(Icon::DotsHorizontal.to_warpui_icon(text_color).finish())
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
        // The 3-dot button is rendered inside the pill's outer click
        // surface, so we need to ensure clicks here don't *also* trigger
        // the pill body's `SwitchAgentViewToConversation` click. We do that
        // by dispatching the typed action and relying on warpui's event
        // bubbling — the inner Hoverable consumes the click before it can
        // reach the outer one. (If this proves wrong empirically, we'll
        // need an explicit `stop_propagation` hook, but that's not in the
        // public Hoverable API today.)
        ctx.dispatch_typed_action(OrchestrationPillBarAction::OpenMenu(conversation_id));
    })
    .finish()
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

    let mut row = Flex::row()
        .with_main_axis_alignment(MainAxisAlignment::Center)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max)
        .with_spacing(4.);
    row.add_child(render_crumb(
        parent_spec,
        Some(parent_crumb_mouse_state),
        theme,
        appearance,
    ));
    row.add_child(chevron);
    row.add_child(render_crumb(child_spec, None, theme, appearance));
    Some(row.finish())
}

fn render_crumb(
    spec: CrumbSpec,
    mouse_state: Option<MouseStateHandle>,
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
    .on_click(move |ctx, _, _| {
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
