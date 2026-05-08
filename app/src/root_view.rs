use crate::ai::blocklist::SerializedBlockListItem;
use crate::appearance::Appearance;
use crate::interval_timer::IntervalTimer;
use crate::launch_configs::launch_config;
use crate::linear::LinearIssueWork;

use crate::persistence::ModelEvent;
use crate::settings::QuakeModeSettings;
use crate::settings_view::flags;
use crate::settings_view::mcp_servers_page::MCPServersSettingsPage;
use crate::settings_view::SettingsSection;
use crate::terminal::available_shells::AvailableShell;
use crate::terminal::general_settings::GeneralSettings;
use crate::terminal::keys_settings::KeysSettings;
use crate::terminal::shell::ShellType;
use crate::terminal::view::cell_size_and_padding;
use crate::themes::theme::AnsiColorIdentifier;
use crate::uri::OpenMCPSettingsArgs;
use crate::util::bindings::{self, is_binding_pty_compliant};
use crate::window_settings::WindowSettings;
use crate::workspace::metadata::LaunchConfigUiLocation;
use crate::workspace::WorkspaceAction;
use crate::workspace::{PaneViewLocator, Workspace};
use crate::ChannelState;
use crate::{
    app_state::{AppState, PaneUuid, WindowSnapshot},
    pane_group::{NewTerminalOptions, PanesLayout},
    UpdateQuakeModeEventArg,
};
use crate::{GlobalResourceHandles, GlobalResourceHandlesProvider};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{vec2f, Vector2F};
use serde::{Deserialize, Serialize};
use settings::Setting as _;
use std::path::Path;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::{collections::HashMap, path::PathBuf};
use warp_core::context_flag::ContextFlag;
use warpui::keymap::{EditableBinding, FixedBinding};
use warpui::windowing::WindowManager;

use warpui::elements::{ParentElement, Stack};
use warpui::rendering::OnGPUDeviceSelected;
use warpui::{id, AddWindowOptions, DisplayId, SingletonEntity};
use warpui::{
    platform::{WindowBounds, WindowStyle},
    presenter::ChildView,
    AppContext, Element, Entity, EntityId, TypedActionView, View, ViewContext, ViewHandle,
    WindowId,
};
use warpui::{FocusContext, NextNewWindowsHasThisWindowsBoundsUponClose};

const WINDOW_TITLE: &str = "Warper";

lazy_static! {
    static ref FALLBACK_WINDOW_SIZE: Vector2F = vec2f(800.0, 600.0);
    static ref QUAKE_STATE: Arc<Mutex<Option<QuakeModeState>>> = Arc::new(Mutex::new(None));
}

#[derive(Debug, Clone)]
enum WindowState {
    /// Quake mode window is open and visible on the screen.
    Open,
    /// Quake mode window is opening but has not become the key window yet.
    /// This happens when the app is not focused when the quake mode window
    /// is opened.
    PendingOpen,
    /// Quake mode window is open but hidden away from the screen.
    /// In this state, toggling quake mode will show the hidden window rather
    /// than creating a new one.
    Hidden,
}

#[derive(Debug, Clone)]
pub struct QuakeModeState {
    /// State of the opened quake mode window.
    window_state: WindowState,
    window_id: WindowId,
    /// ID of the active screen when we last positioned the quake mode window.
    /// Note that this is not necessarily the screen quake mode lives in if user
    /// set a specific pinned screen.
    active_display_id: DisplayId,
}

/// Configuration for the new quake mode window including the active screen id and the window bound.
struct QuakeModeFrameConfig {
    display_id: DisplayId,
    window_bounds: RectF,
}

/// Trigger of a potential quake window move.
#[derive(Debug)]
enum QuakeModeMoveTrigger {
    /// The screen configuration changed (plug / unplug monitor). We need
    /// to reposition quake mode as it might be in an invalid position.
    ScreenConfigurationChange,
    /// User set "active screen" as the screen to pin to. In this case,
    /// we will attempt to move the quake window if the active screen dimension
    /// changed. If it hasn't change, we will keep the window as is to avoid
    /// meaningless resizing.
    ActiveScreenSetting,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Hash,
    Eq,
    PartialEq,
    Deserialize,
    Serialize,
    Default,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Screen edge to pin the hotkey window to.",
    rename_all = "snake_case"
)]
pub enum QuakeModePinPosition {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

pub struct OpenFromRestoredArg {
    pub app_state: Option<AppState>,
}

pub struct OpenLaunchConfigArg {
    pub launch_config: launch_config::LaunchConfig,
    pub ui_location: LaunchConfigUiLocation,

    /// Tries to open the launch config into the active window, if any.
    ///
    /// Currently, this is only supported by single-window launch configs
    /// and will open the window tabs into the existing window when true.
    pub open_in_active_window: bool,
}

pub struct OpenPath {
    pub path: PathBuf,
}

// Arguments for actions that run a command that should start a subshell.
pub struct SubshellCommandArg {
    pub command: String,
    pub shell_type: Option<ShellType>,
}

/// Arguments for the immediate tab detach action dispatched during drag.
/// This contains minimal info needed to identify which tab to detach.
pub struct DetachTabImmediateArg {
    /// Index of the tab to detach
    pub tab_index: usize,
    /// Pre-calculated window position for the new window (in screen coordinates).
    /// This is calculated to position the window so the mouse is in the tab bar region.
    pub window_position: Option<Vector2F>,
    /// Source window ID - the window containing the tab to detach.
    /// We need this because the active window might be the preview window.
    pub source_window_id: WindowId,
}

/// Pre-gathered information for creating a transferred window.
/// This is used when the caller already has access to the workspace (e.g., from within a view method)
/// and cannot rely on workspace lookup (which fails during view updates).
pub struct TabTransferInfo {
    pub transferred_tab: crate::workspace::view::TransferredTab,
    pub window_size: Vector2F,
    pub window_position: Vector2F,
    pub source_window_id: WindowId,
}

pub fn init(app: &mut AppContext) {
    app.register_binding_validator::<RootView>(is_binding_pty_compliant);

    app.add_global_action("root_view:open_from_restored", open_from_restored);
    app.add_global_action("root_view:open_new", open_new);
    app.add_global_action("root_view:open_new_with_shell", open_new_with_shell);
    app.add_global_action("root_view:open_new_from_path", |arg, ctx| {
        let _ = open_new_from_path(arg, ctx);
    });
    app.add_global_action(
        "root_view:open_new_tab_insert_subshell_command_and_bootstrap_if_supported",
        open_new_tab_insert_subshell_command_and_bootstrap_if_supported,
    );
    app.add_global_action("root_view:open_launch_config", open_launch_config);
    app.add_global_action("root_view:send_feedback", send_feedback);
    app.add_global_action("root_view:detach_tab_immediate", |arg, ctx| {
        let _ = detach_tab_with_transfer(arg, ctx);
    });
    app.add_global_action(
        "root_view:toggle_quake_mode_window",
        toggle_quake_mode_window,
    );
    app.add_global_action(
        "root_view:show_or_hide_non_quake_mode_windows",
        show_or_hide_non_quake_mode_windows,
    );
    app.add_global_action("root_view:update_quake_mode_state", update_quake_mode_state);
    app.add_global_action(
        "root_view:move_quake_mode_window_from_screen_change",
        move_quake_mode_window_from_screen_change,
    );
    #[cfg(feature = "voice_input")]
    app.add_global_action("root_view:abort_voice_input", abort_voice_input);
    #[cfg(feature = "voice_input")]
    app.add_action(
        "root_view:maybe_stop_active_voice_input",
        RootView::maybe_stop_active_voice_input,
    );
    app.add_action(
        "root_view:add_session_at_path",
        RootView::add_session_at_path,
    );
    app.add_action(
        "root_view:handle_notification_click",
        RootView::handle_notification_click,
    );
    app.add_action(
        "root_view:handle_pane_navigation_event",
        RootView::focus_pane,
    );
    app.add_action("root_view:close_window", RootView::close_window);
    app.add_action("root_view:minimize_window", RootView::minimize_window);
    app.add_action(
        "root_view:toggle_maximize_window",
        RootView::toggle_maximize_window,
    );
    app.add_action("root_view:toggle_fullscreen", RootView::toggle_fullscreen);

    app.add_global_action(
        "root_view:open_settings_page_in_new_window",
        open_settings_page_in_new_window,
    );
    app.add_action(
        "root_view:open_settings_page_in_existing_window",
        RootView::open_settings_page_in_existing_window,
    );

    app.add_global_action(
        "root_view:open_mcp_settings_in_new_window",
        open_mcp_settings_in_new_window,
    );
    app.add_action(
        "root_view:open_mcp_settings_in_existing_window",
        RootView::open_mcp_settings_in_existing_window,
    );

    app.add_global_action(
        "root_view:open_linear_issue_work_in_new_window",
        open_linear_issue_work_in_new_window,
    );
    app.add_action(
        "root_view:open_linear_issue_work_in_existing_window",
        RootView::open_linear_issue_work_in_existing_window,
    );

    app.add_action("root_view:add_file_pane", RootView::add_file_pane);
    app.add_global_action(
        "root_view:open_new_with_file_notebook",
        open_new_with_file_notebook,
    );

    app.register_fixed_bindings([
        FixedBinding::empty(
            "Hide All Windows",
            RootViewAction::ShowOrHideNonQuakeModeWindows,
            id!("RootView") & id!(flags::ACTIVATION_HOTKEY_FLAG),
        ),
        FixedBinding::empty(
            "Show Dedicated Hotkey Window",
            RootViewAction::ToggleQuakeModeWindow,
            id!("RootView")
                & id!(flags::QUAKE_MODE_ENABLED_CONTEXT_FLAG)
                & !id!(flags::QUAKE_WINDOW_OPEN_FLAG),
        ),
        FixedBinding::empty(
            "Hide Dedicated Hotkey Window",
            RootViewAction::ToggleQuakeModeWindow,
            id!("RootView")
                & id!(flags::QUAKE_MODE_ENABLED_CONTEXT_FLAG)
                & id!(flags::QUAKE_WINDOW_OPEN_FLAG),
        ),
    ]);

    app.register_editable_bindings([
        // Register a binding to toggle fullscreen on Linux and Windows.
        EditableBinding::new(
            "root_view:toggle_fullscreen",
            "Toggle fullscreen",
            RootViewAction::ToggleFullscreen,
        )
        .with_group(bindings::BindingGroup::Navigation.as_str())
        .with_context_predicate(id!("RootView"))
        .with_linux_or_windows_key_binding("f11"),
    ])
}

fn maybe_register_global_window_shortcuts(
    global_resource_handles: GlobalResourceHandles,
    ctx: &mut AppContext,
) {
    // let keys_settings = KeysSettings::handle(ctx).as_ref(ctx);
    if let Some(key) = KeysSettings::as_ref(ctx)
        .quake_mode_settings
        .keybinding
        .clone()
        .filter(|_| *KeysSettings::as_ref(ctx).quake_mode_enabled)
    {
        ctx.register_global_shortcut(
            key.clone(),
            "root_view:toggle_quake_mode_window",
            global_resource_handles,
        );
    }

    if let Some(key) = KeysSettings::as_ref(ctx)
        .activation_hotkey_keybinding
        .clone()
        .filter(|_| *KeysSettings::as_ref(ctx).activation_hotkey_enabled)
    {
        ctx.register_global_shortcut(
            key.clone(),
            "root_view:show_or_hide_non_quake_mode_windows",
            (),
        )
    }
}

/// Find the root [`Workspace`] view for the active window.
fn active_workspace(ctx: &mut AppContext) -> Option<ViewHandle<Workspace>> {
    let window_id = ctx.windows().active_window()?;
    ctx.views_of_type::<Workspace>(window_id)
        .and_then(|views| views.first().cloned())
}

/// Find the root [`Workspace`] view for a specific window.
pub fn workspace_for_window(
    window_id: WindowId,
    ctx: &mut AppContext,
) -> Option<ViewHandle<Workspace>> {
    ctx.views_of_type::<Workspace>(window_id)
        .and_then(|views| views.first().cloned())
}

fn open_launch_config(arg: &OpenLaunchConfigArg, ctx: &mut AppContext) {
    let active_window_workspace = active_workspace(ctx);
    if arg.launch_config.windows.is_empty() {
        open_new(&(), ctx);
    } else if arg.open_in_active_window
        && arg.launch_config.windows.len() == 1
        && active_window_workspace.is_some()
    {
        active_window_workspace
            .expect("already checked if there is a workspace for the active window")
            .update(ctx, |workspace, ctx| {
                workspace.open_launch_config_window(arg.launch_config.windows[0].clone(), ctx)
            });
    } else {
        let mut active_index = None;
        for (idx, window_template) in arg.launch_config.windows.iter().enumerate() {
            if arg
                .launch_config
                .active_window_index
                .map(|window_idx| window_idx == idx)
                .unwrap_or(false)
            {
                active_index = Some(idx);
            } else {
                open_new_with_workspace_source(
                    NewWorkspaceSource::FromTemplate {
                        window_template: window_template.clone(),
                    },
                    ctx,
                );
            }
        }

        if let Some(idx) = active_index {
            let window_template = arg
                .launch_config
                .windows
                .get(idx)
                .expect("Window should exist at idx");

            open_new_with_workspace_source(
                NewWorkspaceSource::FromTemplate {
                    window_template: window_template.clone(),
                },
                ctx,
            );
        }
    }
}

fn send_feedback(_: &(), ctx: &mut AppContext) {
    if let Some(workspace) = active_workspace(ctx) {
        workspace.update(ctx, |workspace, ctx| {
            workspace.handle_action(&WorkspaceAction::SendFeedback, ctx);
        });
    } else {
        ctx.open_url(&crate::util::links::feedback_form_url());
    }
}

/// Handler for tab detachment using the transferable views framework.
/// Instead of extracting and recreating views, this transfers the PaneGroup view tree directly.
/// Returns the new window ID if successful.
pub fn detach_tab_with_transfer(
    arg: &DetachTabImmediateArg,
    ctx: &mut AppContext,
) -> Option<WindowId> {
    let Some(source_workspace) = workspace_for_window(arg.source_window_id, ctx) else {
        log::warn!(
            "No workspace found for source window {:?}",
            arg.source_window_id
        );
        return None;
    };

    let transferred_tab = source_workspace.read(ctx, |workspace, ctx| {
        workspace.get_tab_transfer_info(arg.tab_index, ctx)
    })?;

    let window_size = ctx
        .windows()
        .platform_window(arg.source_window_id)
        .map(|window| window.as_ctx().size())
        .unwrap_or(*FALLBACK_WINDOW_SIZE);

    let window_position = arg.window_position.unwrap_or_default();

    let info = TabTransferInfo {
        transferred_tab,
        window_size,
        window_position,
        source_window_id: arg.source_window_id,
    };

    let (new_window_id, _transferred_view_ids) = create_transferred_window(info, false, ctx);

    source_workspace.update(ctx, |workspace, ctx| {
        workspace.remove_tab_without_undo(arg.tab_index, ctx);
    });

    Some(new_window_id)
}

/// Creates a new window with the transferred pane group.
/// This function takes pre-gathered TabTransferInfo, allowing it to be called
/// from within a view method where workspace lookup would fail.
///
/// If `for_drag` is true, the window is created without stealing focus (for drag preview).
///
/// Returns the new window ID and the list of transferred view entity IDs.
/// The transferred view IDs are needed by `tab_drag::on_tab_drag` to track which
/// views must follow the tab during subsequent handoff/reverse-handoff cycles.
pub fn create_transferred_window(
    info: TabTransferInfo,
    for_drag: bool,
    ctx: &mut AppContext,
) -> (WindowId, Vec<EntityId>) {
    let global_resource_handles = GlobalResourceHandlesProvider::handle(ctx)
        .as_ref(ctx)
        .get()
        .clone();
    let window_settings = WindowSettings::handle(ctx).as_ref(ctx);

    let window_bounds =
        WindowBounds::ExactPosition(RectF::new(info.window_position, info.window_size));

    let window_style = if for_drag {
        WindowStyle::PositionedNoFocus
    } else {
        WindowStyle::Normal
    };

    let (new_window_id, _) = ctx.add_window(
        AddWindowOptions {
            window_style,
            window_bounds,
            title: Some(WINDOW_TITLE.to_owned()),
            background_blur_radius_pixels: Some(*window_settings.background_blur_radius),
            background_blur_texture: *window_settings.background_blur_texture,
            on_gpu_driver_selected: on_gpu_driver_selected_callback(),
            ..Default::default()
        },
        |ctx| {
            let mut view = RootView::new(
                global_resource_handles.clone(),
                NewWorkspaceSource::TransferredTab {
                    tab_color: info.transferred_tab.color,
                    custom_title: info.transferred_tab.custom_title.clone(),
                    left_panel_open: info.transferred_tab.left_panel_open,
                    vertical_tabs_panel_open: info.transferred_tab.vertical_tabs_panel_open,
                    right_panel_open: info.transferred_tab.right_panel_open,
                    is_right_panel_maximized: info.transferred_tab.is_right_panel_maximized,
                    for_drag_preview: for_drag,
                },
                ctx,
            );
            if !for_drag {
                view.focus(ctx);
            }
            view
        },
    );

    let pane_group_id = info.transferred_tab.pane_group.id();
    let transferred_view_ids =
        ctx.transfer_view_tree_to_window(pane_group_id, info.source_window_id, new_window_id);

    if let Some(new_workspace) = workspace_for_window(new_window_id, ctx) {
        new_workspace.update(ctx, |workspace, ctx| {
            workspace.adopt_transferred_pane_group(info.transferred_tab.pane_group.clone(), ctx);
        });
    } else {
        log::warn!("Failed to find workspace in newly created window {new_window_id:?}");
    }
    (new_window_id, transferred_view_ids)
}

fn on_gpu_driver_selected_callback() -> Option<Box<OnGPUDeviceSelected>> {
    None
}

fn open_from_restored(arg: &OpenFromRestoredArg, ctx: &mut AppContext) {
    let global_resource_handles = GlobalResourceHandlesProvider::as_ref(ctx).get().clone();
    IntervalTimer::handle(ctx).update(ctx, |timer, _| {
        timer.mark_interval_end("HANDLING_OPEN_ACTION");
    });

    if let Some(app_state) = &arg.app_state {
        maybe_register_global_window_shortcuts(global_resource_handles.clone(), ctx);

        let (background_blur_radius_pixels, background_blur_texture) = {
            let window_settings = WindowSettings::as_ref(ctx);
            (
                Some(*window_settings.background_blur_radius),
                *window_settings.background_blur_texture,
            )
        };

        // Check whether user has enabled session restoration.
        if *GeneralSettings::as_ref(ctx).restore_session {
            let mut active_index = None;
            let mut normal_window_count = 0;
            for (idx, window) in app_state.windows.iter().enumerate() {
                // If this window is a quake window, hide it by default.
                if window.quake_mode {
                    // If this is Windows, skip restoring the quake window. Creating a hidden window
                    // is not supported on Windows. We can't have the quake window visible on
                    // startup or else it will get mistaken for a normal window.
                    if cfg!(windows) {
                        continue;
                    }
                    let frame_args = quake_mode_config(
                        &KeysSettings::as_ref(ctx)
                            .quake_mode_settings
                            .value()
                            .clone(),
                        ctx,
                    );

                    let (id, _) = ctx.add_window(
                        AddWindowOptions {
                            window_style: WindowStyle::Pin,
                            window_bounds: WindowBounds::ExactPosition(frame_args.window_bounds),
                            title: Some("Warper".to_owned()),
                            fullscreen_state: window.fullscreen_state,
                            background_blur_radius_pixels,
                            background_blur_texture,
                            // Don't use the quake window for positioning new windows.
                            anchor_new_windows_from_closed_position:
                                NextNewWindowsHasThisWindowsBoundsUponClose::No,
                            on_gpu_driver_selected: on_gpu_driver_selected_callback(),
                            window_instance: Some(ChannelState::app_id().to_string() + "-hotkey"),
                        },
                        |ctx| {
                            let mut view = RootView::new(
                                global_resource_handles.clone(),
                                NewWorkspaceSource::Restored {
                                    window_snapshot: window.clone(),
                                    block_lists: app_state.block_lists.clone(),
                                },
                                ctx,
                            );
                            view.focus(ctx);
                            view
                        },
                    );
                    ctx.windows().hide_window(id);

                    let mut quake_mode_state = QUAKE_STATE.lock();
                    *quake_mode_state = Some(QuakeModeState {
                        window_state: WindowState::Hidden,
                        window_id: id,
                        active_display_id: frame_args.display_id,
                    });
                } else {
                    normal_window_count += 1;
                    if app_state
                        .active_window_index
                        .map(|window_idx| window_idx == idx)
                        .unwrap_or(false)
                    {
                        active_index = Some(idx);
                    } else {
                        ctx.add_window(
                            AddWindowOptions {
                                window_bounds: WindowBounds::new(window.bounds),
                                title: Some("Warper".to_owned()),
                                fullscreen_state: window.fullscreen_state,
                                background_blur_radius_pixels,
                                background_blur_texture,
                                on_gpu_driver_selected: on_gpu_driver_selected_callback(),
                                ..Default::default()
                            },
                            |ctx| {
                                let mut view = RootView::new(
                                    global_resource_handles.clone(),
                                    NewWorkspaceSource::Restored {
                                        window_snapshot: window.clone(),
                                        block_lists: app_state.block_lists.clone(),
                                    },
                                    ctx,
                                );
                                view.focus(ctx);
                                view
                            },
                        );
                    }
                }
            }

            // If only the quake mode window was restored (which starts hidden), create a new normal
            // window so that something visible is created on startup.
            if normal_window_count == 0 {
                let window_settings = WindowSettings::as_ref(ctx);
                let options = default_window_options(window_settings, ctx);
                ctx.add_window(options, |ctx| {
                    let mut view = RootView::new(
                        global_resource_handles.clone(),
                        NewWorkspaceSource::Empty {
                            previous_active_window: None,
                            shell: None,
                        },
                        ctx,
                    );
                    view.focus(ctx);
                    view
                });
            }

            // Create the active window last to make sure it is focused on startup.
            if let Some(idx) = active_index {
                let window = app_state
                    .windows
                    .get(idx)
                    .expect("Window should exist at idx");
                ctx.add_window(
                    AddWindowOptions {
                        window_bounds: WindowBounds::new(window.bounds),
                        title: Some("Warper".to_owned()),
                        fullscreen_state: window.fullscreen_state,
                        background_blur_radius_pixels,
                        background_blur_texture,
                        on_gpu_driver_selected: on_gpu_driver_selected_callback(),
                        ..Default::default()
                    },
                    |ctx| {
                        let mut view = RootView::new(
                            global_resource_handles,
                            NewWorkspaceSource::Restored {
                                window_snapshot: window.clone(),
                                block_lists: app_state.block_lists.clone(),
                            },
                            ctx,
                        );
                        view.focus(ctx);
                        view
                    },
                );
            }
        }
    }
}

fn path_if_directory(path: &Path) -> Option<&Path> {
    path.is_dir().then_some(path)
}

/// Opens a new window with the workspace configured according to `source`. Returns the
/// newly-opened window ID and a handle to the root view in that window.
///
/// This is the canonical way to open a new Warp window - all other entrypoints should delegate to
/// it if possible.
pub(crate) fn open_new_with_workspace_source(
    source: NewWorkspaceSource,
    ctx: &mut AppContext,
) -> (WindowId, ViewHandle<RootView>) {
    let global_resource_handles = GlobalResourceHandlesProvider::as_ref(ctx).get().clone();
    let window_settings = WindowSettings::as_ref(ctx);
    let options = default_window_options(window_settings, ctx);
    ctx.add_window(options, |ctx| {
        let mut view = RootView::new(global_resource_handles, source, ctx);
        view.focus(ctx);
        view
    })
}

pub(crate) fn open_new_from_path(
    arg: &OpenPath,
    ctx: &mut AppContext,
) -> (WindowId, ViewHandle<RootView>) {
    open_new_with_workspace_source(
        NewWorkspaceSource::Session {
            options: Box::new(
                NewTerminalOptions::default()
                    .with_initial_directory_opt(path_if_directory(&arg.path).map(Into::into)),
            ),
        },
        ctx,
    )
}

fn open_settings_page_in_new_window(section: &SettingsSection, ctx: &mut AppContext) {
    if section.is_removed_hosted_surface() {
        return;
    }

    let root_handle = open_new_window_get_handles(None, ctx).1;
    root_handle.update(ctx, |root_view, ctx| {
        let window_id = ctx.window_id();
        ctx.dispatch_typed_action_for_view(
            window_id,
            root_view.workspace.id(),
            &WorkspaceAction::ShowSettingsPage(*section),
        );
    });
}

/// MCP servers need to wait for initial load to complete, so we have this action in addition
/// to the general-purpose [`open_settings_page_in_new_window`].
fn open_mcp_settings_in_new_window(args: &OpenMCPSettingsArgs, ctx: &mut AppContext) {
    let _ = args;
    let root_handle = open_new_window_get_handles(None, ctx).1;
    root_handle.update(ctx, |root_view, ctx| {
        root_view.workspace.update(ctx, |_, ctx| {
            ctx.dispatch_typed_action(&WorkspaceAction::ShowSettingsPage(
                SettingsSection::AgentMCPServers,
            ));
        });
    });
}

/// Opens a new window and enters agent view with the Linear issue work prompt.
fn open_linear_issue_work_in_new_window(args: &LinearIssueWork, ctx: &mut AppContext) {
    let (_, root_handle) = open_new_window_get_handles(None, ctx);
    let args = args.clone();
    root_handle.update(ctx, |root_view, ctx| {
        root_view.workspace.update(ctx, |workspace, ctx| {
            workspace.open_linear_issue_work(&args, ctx);
        });
    });
}

/// Opens a new window with a file-based notebook open.
fn open_new_with_file_notebook(arg: &PathBuf, ctx: &mut AppContext) {
    open_new_with_workspace_source(
        NewWorkspaceSource::NotebookFromFilePath {
            file_path: Some(arg.to_owned()),
        },
        ctx,
    );
}

/// Creates a new window and returns its [`WindowId`] and root view's [`ViewHandle`].
pub(crate) fn open_new_window_get_handles(
    shell: Option<AvailableShell>,
    ctx: &mut AppContext,
) -> (WindowId, ViewHandle<RootView>) {
    let active_window_id = ctx.windows().active_window();
    open_new_with_workspace_source(
        NewWorkspaceSource::Empty {
            previous_active_window: active_window_id,
            shell,
        },
        ctx,
    )
}

/// Opens a new window.
fn open_new(_: &(), ctx: &mut AppContext) {
    open_new_window_get_handles(None, ctx);
}

/// Opens a new window with a specific shell
fn open_new_with_shell(shell: &Option<AvailableShell>, ctx: &mut AppContext) {
    open_new_window_get_handles(shell.to_owned(), ctx);
}

/// Global action that performs a few steps:
/// 1. Open a new tab, or open a window if there is none.
/// 2. Set the terminal input buffer to a command that should open a subshell
/// 3. Set a flag that we should automatically bootstrap that subshell if its we can boostrap its
/// [`ShellType`].
fn open_new_tab_insert_subshell_command_and_bootstrap_if_supported(
    arg: &SubshellCommandArg,
    ctx: &mut AppContext,
) {
    let root_view_handle: Option<ViewHandle<RootView>> = ctx
        .windows()
        .frontmost_window_id()
        .and_then(|window_id| ctx.root_view(window_id));

    let root_view_handle = match root_view_handle {
        Some(root_view_handle) => {
            root_view_handle.update(ctx, |root_view, ctx| {
                root_view.workspace.update(ctx, |workspace, ctx| {
                    workspace.add_terminal_tab(false /* hide_homepage */, ctx);
                });
            });
            root_view_handle
        }
        None => open_new_window_get_handles(None, ctx).1,
    };

    root_view_handle.update(ctx, |root_view, ctx| {
        root_view.insert_subshell_command_and_bootstrap_if_supported(arg, ctx);
    });
}

/// Returns the common configuration for a new "regular" window (not Quake Mode).
fn default_window_options(window_settings: &WindowSettings, ctx: &AppContext) -> AddWindowOptions {
    let (inherited_bounds, window_style) = ctx.next_window_bounds_and_style();
    let next_bounds =
        bounds_for_opening_at_custom_window_size(inherited_bounds, window_settings, ctx);

    AddWindowOptions {
        window_style,
        window_bounds: next_bounds,
        title: Some("Warper".to_owned()),
        background_blur_radius_pixels: Some(*window_settings.background_blur_radius),
        background_blur_texture: *window_settings.background_blur_texture,
        on_gpu_driver_selected: on_gpu_driver_selected_callback(),
        ..Default::default()
    }
}

/// Returns the bounds to open the next window at taking into account whether
/// the user has configured their settings to open windows at a custom size
/// and whether that feature is flagged on.
fn bounds_for_opening_at_custom_window_size(
    bounds: WindowBounds,
    window_settings: &WindowSettings,
    app: &AppContext,
) -> WindowBounds {
    if *window_settings.open_windows_at_custom_size.value() {
        let font_cache = app.font_cache();
        let appearance = Appearance::as_ref(app);

        let cell_size_and_padding = cell_size_and_padding(
            font_cache,
            appearance.monospace_font_family(),
            appearance.monospace_font_size(),
            appearance.ui_builder().line_height_ratio(),
        );
        let window_size = vec2f(
            *window_settings.new_windows_num_columns.value() as f32
                * cell_size_and_padding.cell_width_px.as_f32()
                + 2. * cell_size_and_padding.padding_x_px.as_f32(),
            *window_settings.new_windows_num_rows.value() as f32
                * cell_size_and_padding.cell_height_px.as_f32()
                + 2. * cell_size_and_padding.padding_y_px.as_f32(),
        );

        match bounds {
            WindowBounds::ExactPosition(rect) => {
                WindowBounds::ExactPosition(RectF::new(rect.origin(), window_size))
            }
            WindowBounds::ExactSize(_) | WindowBounds::Default => {
                WindowBounds::ExactSize(window_size)
            }
        }
    } else {
        bounds
    }
}

pub fn quake_mode_window_is_open() -> bool {
    let quake_mode_state = QUAKE_STATE.lock();

    quake_mode_state
        .as_ref()
        .map(|state| {
            matches!(
                state.window_state,
                WindowState::Open | WindowState::PendingOpen
            )
        })
        .unwrap_or_default()
}

pub fn quake_mode_window_id() -> Option<WindowId> {
    let quake_mode_state = QUAKE_STATE.lock();

    quake_mode_state.as_ref().map(|state| state.window_id)
}

pub fn set_quake_mode(new_state: Option<QuakeModeState>) {
    let mut quake_mode_state = QUAKE_STATE.lock();
    *quake_mode_state = new_state;
}

fn move_quake_mode_window_from_screen_change(settings: &QuakeModeSettings, ctx: &mut AppContext) {
    fit_quake_mode_window_within_active_screen(
        settings,
        QuakeModeMoveTrigger::ScreenConfigurationChange,
        ctx,
    )
}

/// If there exists a quake window, mutate its size and position, i.e. its bounds, to match the
/// bounds specified by the [`QuakeModeSettings`].
pub fn update_quake_window_bounds(quake_settings: &QuakeModeSettings, ctx: &mut AppContext) {
    let config = quake_mode_config(quake_settings, ctx);
    let Some(ref state) = *QUAKE_STATE.lock() else {
        return;
    };
    ctx.windows()
        .set_window_bounds(state.window_id, config.window_bounds);
}

/// Move Quake Mode window to the active screen if it is already open or hidden.
fn fit_quake_mode_window_within_active_screen(
    settings: &QuakeModeSettings,
    trigger: QuakeModeMoveTrigger,
    ctx: &mut AppContext,
) {
    let mut quake_mode_state = QUAKE_STATE.lock();

    if let Some(state) = quake_mode_state.as_mut() {
        let active_id = ctx.windows().active_display_id();

        // When there is no screen config and active screen id change, we don't need to reposition
        // the quake mode window as its position should still be valid.
        if matches!(trigger, QuakeModeMoveTrigger::ActiveScreenSetting)
            && active_id == state.active_display_id
        {
            return;
        }

        let window_bound = settings.resolve_quake_mode_bounds(ctx);
        ctx.windows()
            .set_window_bounds(state.window_id, window_bound);
        state.active_display_id = active_id;
    }
}

fn update_quake_mode_state(arg: &UpdateQuakeModeEventArg, ctx: &mut AppContext) {
    if !KeysSettings::as_ref(ctx)
        .quake_mode_settings
        .hide_window_when_unfocused
    {
        return;
    }

    {
        let mut quake_mode_state = QUAKE_STATE.lock();

        if let Some(state) = quake_mode_state.as_mut() {
            state.window_state = match state.window_state {
                WindowState::PendingOpen => WindowState::Open,
                WindowState::Open => {
                    if arg.active_window_id.is_some_and(|id| id == state.window_id) {
                        WindowState::Open
                    } else {
                        ctx.windows().hide_window(state.window_id);
                        WindowState::Hidden
                    }
                }
                WindowState::Hidden => WindowState::Hidden,
            }
        }
    }
}

// Configuration of the next positioning of the quake mode window.
fn quake_mode_config(settings: &QuakeModeSettings, ctx: &mut AppContext) -> QuakeModeFrameConfig {
    QuakeModeFrameConfig {
        display_id: ctx.windows().active_display_id(),
        window_bounds: settings.resolve_quake_mode_bounds(ctx),
    }
}

fn get_quake_mode_state(ctx: &mut AppContext) -> Option<QuakeModeState> {
    let quake_mode_state = QUAKE_STATE.lock();

    match quake_mode_state.as_ref() {
        Some(state) if ctx.is_window_open(state.window_id) => Some(state.clone()),
        _ => None,
    }
}

fn toggle_quake_mode_window(global_resource_handles: &GlobalResourceHandles, ctx: &mut AppContext) {
    // Get the current state of quake mode.
    let state = get_quake_mode_state(ctx);
    match state {
        None => {
            let config = quake_mode_config(
                &KeysSettings::as_ref(ctx)
                    .quake_mode_settings
                    .value()
                    .clone(),
                ctx,
            );

            let window_settings = WindowSettings::as_ref(ctx);

            let active_window_id = ctx.windows().active_window();
            let (id, _) = ctx.add_window(
                AddWindowOptions {
                    window_style: WindowStyle::Pin,
                    window_bounds: WindowBounds::ExactPosition(config.window_bounds),
                    title: Some("Warper".to_owned()),
                    background_blur_radius_pixels: Some(*window_settings.background_blur_radius),
                    background_blur_texture: *window_settings.background_blur_texture,
                    // Ignore the quake window for positioning the next window
                    anchor_new_windows_from_closed_position:
                        warpui::NextNewWindowsHasThisWindowsBoundsUponClose::No,
                    on_gpu_driver_selected: on_gpu_driver_selected_callback(),
                    window_instance: Some(ChannelState::app_id().to_string() + "-hotkey"),
                    ..Default::default()
                },
                |ctx| {
                    let mut view = RootView::new(
                        global_resource_handles.clone(),
                        NewWorkspaceSource::Empty {
                            previous_active_window: active_window_id,
                            shell: None,
                        },
                        ctx,
                    );
                    view.focus(ctx);
                    view
                },
            );

            // Update quake mode state after the call to prevent deadlocking.
            let mut quake_mode_state = QUAKE_STATE.lock();
            *quake_mode_state = Some(QuakeModeState {
                window_state: WindowState::PendingOpen,
                window_id: id,
                active_display_id: config.display_id,
            });
        }
        Some(state) if matches!(state.window_state, WindowState::Hidden) => {
            // If quake mode does not have a set pin screen -- move it to the current active screen.
            if KeysSettings::as_ref(ctx)
                .quake_mode_settings
                .pin_screen
                .is_none()
            {
                fit_quake_mode_window_within_active_screen(
                    &KeysSettings::as_ref(ctx)
                        .quake_mode_settings
                        .value()
                        .clone(),
                    QuakeModeMoveTrigger::ActiveScreenSetting,
                    ctx,
                );
            }
            ctx.windows().show_window_and_focus_app(state.window_id);

            // Update quake mode state after the call to prevent deadlocking.
            let mut quake_mode_state = QUAKE_STATE.lock();

            if let Some(state) = quake_mode_state.as_mut() {
                state.window_state = WindowState::PendingOpen;
            }
        }
        Some(state) => {
            ctx.windows().hide_window(state.window_id);

            // Update quake mode state after the call to prevent deadlocking.
            let mut quake_mode_state = QUAKE_STATE.lock();

            if let Some(state) = quake_mode_state.as_mut() {
                state.window_state = WindowState::Hidden;
            }
        }
    };
}

/// This action will show or hide all of Warp's windows except the quake window
///
/// - If Warp is active and has any windows, hide those windows.
/// - If Warp is hidden, show all windows.
/// - If Warp is active but has 0 normal windows, create a new window with a new session.
fn show_or_hide_non_quake_mode_windows(_: &(), ctx: &mut AppContext) {
    let quake_window_id = get_quake_mode_state(ctx).map(|state| state.window_id);
    let non_quake_mode_window_ids = ctx
        .window_ids()
        .filter(|window_id| Some(window_id) != quake_window_id.as_ref());
    if non_quake_mode_window_ids.count() == 0 {
        // If there are no normal windows, this action should create one.
        open_new(&(), ctx);
    }
    let windowing_model = ctx.windows();
    // Now there is at least one window. If a Warp window is active, hide the app.
    // Otherwise, show activate the app to show it in front.
    let active_window_id = windowing_model.active_window();
    match active_window_id {
        Some(_) => windowing_model.hide_app(),
        None => {
            windowing_model.activate_app();
        }
    };
}

#[cfg(feature = "voice_input")]
fn abort_voice_input(_: &(), ctx: &mut AppContext) {
    let voice_input = voice_input::VoiceInput::handle(ctx);
    if voice_input.as_ref(ctx).is_listening() {
        voice_input.update(ctx, |voice_input, _| {
            voice_input.abort_listening();
        });
    }
}

#[derive(Clone)]
pub enum NewWorkspaceSource {
    Empty {
        previous_active_window: Option<WindowId>,
        shell: Option<AvailableShell>,
    },
    FromTemplate {
        window_template: launch_config::WindowTemplate,
    },
    Restored {
        window_snapshot: WindowSnapshot,
        block_lists: Arc<HashMap<PaneUuid, Vec<SerializedBlockListItem>>>,
    },
    Session {
        options: Box<NewTerminalOptions>,
    },
    NotebookFromFilePath {
        file_path: Option<PathBuf>,
    },
    AgentSession {
        options: Box<NewTerminalOptions>,
        initial_query: Option<String>,
    },
    /// A tab is being transferred from another window via the transferable views framework.
    /// The workspace will create a placeholder tab, which will be replaced by the transferred
    /// PaneGroup after window creation.
    TransferredTab {
        /// Tab color from the source tab
        tab_color: Option<AnsiColorIdentifier>,
        /// Custom title from the source tab
        custom_title: Option<String>,
        /// Whether the left panel was open in the source tab
        left_panel_open: bool,
        /// Captured from the source window so detached tabs inherit the panel state.
        vertical_tabs_panel_open: bool,
        /// Whether the right panel was open in the source tab
        right_panel_open: bool,
        /// Whether the right panel was maximized in the source tab
        is_right_panel_maximized: bool,
        /// Whether this transferred tab window is currently being used as a drag preview.
        for_drag_preview: bool,
    },
}

impl NewWorkspaceSource {
    pub fn has_horizontal_split(&self) -> bool {
        match self {
            NewWorkspaceSource::Restored {
                window_snapshot, ..
            } => {
                if window_snapshot.tabs.is_empty() {
                    false
                } else {
                    let active_index = window_snapshot.active_tab_index;
                    let active_tab = window_snapshot
                        .tabs
                        .get(active_index)
                        .unwrap_or(&window_snapshot.tabs[0]);
                    active_tab.root.has_horizontal_split()
                }
            }
            _ => false,
        }
    }
}

/// Args needed to construct a `Workspace`.
#[derive(Clone)]
struct WorkspaceArgs {
    global_resource_handles: GlobalResourceHandles,
    workspace_setting: NewWorkspaceSource,
}

pub struct RootView {
    workspace: ViewHandle<Workspace>,
    pub model_event_sender: Option<SyncSender<ModelEvent>>,
}

impl RootView {
    pub fn new(
        global_resource_handles: GlobalResourceHandles,
        workspace_setting: NewWorkspaceSource,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let model_event_sender = global_resource_handles.model_event_sender.clone();
        let workspace_args = WorkspaceArgs {
            global_resource_handles,
            workspace_setting,
        };

        Self {
            workspace: workspace_args.create_workspace(ctx),
            model_event_sender,
        }
    }

    /// Used for integration tests.
    pub fn workspace_view(&self) -> Option<&ViewHandle<Workspace>> {
        Some(&self.workspace)
    }

    fn close_window(&mut self, _: &(), ctx: &mut ViewContext<Self>) -> bool {
        if ContextFlag::CloseWindow.is_enabled() {
            ctx.close_window();
        }
        true
    }

    fn toggle_maximize_window(&mut self, _: &(), ctx: &mut ViewContext<Self>) -> bool {
        ctx.toggle_maximized_window();
        true
    }

    fn toggle_fullscreen(&mut self, _: &(), ctx: &mut ViewContext<Self>) -> bool {
        let window_id = ctx.window_id();
        WindowManager::handle(ctx).update(ctx, |state, ctx| {
            state.toggle_fullscreen(window_id, ctx);
        });
        true
    }

    fn minimize_window(&mut self, _: &(), ctx: &mut ViewContext<Self>) -> bool {
        ctx.minimize_window();
        true
    }

    fn focus_pane(
        &mut self,
        pane_view_locator: &PaneViewLocator,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        // Focus the appropriate window.
        let window_id = ctx.window_id();

        let mut quake_mode_state = QUAKE_STATE.lock();
        // If the window we are focusing is the Quake Mode window, then update the QuakeModeState.
        if let Some(mode) = quake_mode_state.as_mut() {
            if mode.window_id == window_id {
                mode.window_state = WindowState::Open;
            }
        }

        ctx.windows().show_window_and_focus_app(window_id);

        // Focus the appropriate tab/pane.
        self.workspace.update(ctx, |view, ctx| {
            view.focus_pane(*pane_view_locator, ctx);
        });
        true
    }

    fn handle_notification_click(
        &mut self,
        pane_view_locator: &PaneViewLocator,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        // Focus the pane that the notification originated from.
        self.focus_pane(pane_view_locator, ctx);
        true
    }

    #[allow(clippy::ptr_arg)]
    fn add_session_at_path(&mut self, path: &PathBuf, ctx: &mut ViewContext<Self>) -> bool {
        let window_id = ctx.window_id();
        self.workspace.update(ctx, |view, ctx| {
            view.add_tab_with_pane_layout(
                PanesLayout::SingleTerminal(Box::new(
                    NewTerminalOptions::default()
                        .with_initial_directory_opt(path_if_directory(path).map(Into::into)),
                )),
                Arc::new(HashMap::new()),
                None,
                ctx,
            );
            ctx.windows().show_window_and_focus_app(window_id);
            ctx.notify();
        });
        true
    }

    pub fn add_file_pane(&mut self, path: &PathBuf, ctx: &mut ViewContext<Self>) -> bool {
        self.workspace.update(ctx, |workspace, ctx| {
            workspace.add_tab_for_file_notebook(Some(path.to_owned()), ctx);
        });
        let window_id = ctx.window_id();
        ctx.windows().show_window_and_focus_app(window_id);
        ctx.notify();
        true
    }

    /// Insert a command that should create a subshell. If we support bootstrapping AKA
    /// "warpifying" its [`ShellType`], set a flag to automatically bootstrap it when the command's
    /// block receives the [`AfterBlockStarted`] event.
    pub fn insert_subshell_command_and_bootstrap_if_supported(
        &mut self,
        arg: &SubshellCommandArg,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        let window_id = ctx.window_id();
        self.workspace.update(ctx, |workspace, ctx| {
            workspace.insert_subshell_command_and_bootstrap_if_supported(
                &arg.command,
                arg.shell_type,
                ctx,
            );
            ctx.windows().show_window_and_focus_app(window_id);
        });
        true
    }

    pub fn open_settings_page_in_existing_window(
        &mut self,
        section: &SettingsSection,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        if !ChannelState::is_warp_server_available() && section.requires_hosted_services() {
            return false;
        }

        let window_id = ctx.window_id();
        ctx.dispatch_typed_action_for_view(
            window_id,
            self.workspace.id(),
            &WorkspaceAction::ShowSettingsPage(*section),
        );
        ctx.windows().show_window_and_focus_app(window_id);
        true
    }

    /// Opens the MCP servers settings page in an existing window.
    pub fn open_mcp_settings_in_existing_window(
        &mut self,
        args: &OpenMCPSettingsArgs,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        let _ = args;
        self.workspace.update(ctx, |workspace, ctx| {
            workspace.open_mcp_servers_page(MCPServersSettingsPage::List, ctx);
        });
        let window_id = ctx.window_id();
        ctx.windows().show_window_and_focus_app(window_id);
        true
    }

    /// Opens the Codex modal in an existing window.
    pub fn open_codex_in_existing_window(&mut self, _: &(), ctx: &mut ViewContext<Self>) -> bool {
        let window_id = ctx.window_id();
        self.workspace.update(ctx, |workspace, ctx| {
            workspace.open_codex_modal(ctx);
        });
        ctx.windows().show_window_and_focus_app(window_id);
        true
    }

    /// Opens a new tab with agent view for a Linear issue work deeplink.
    pub fn open_linear_issue_work_in_existing_window(
        &mut self,
        args: &LinearIssueWork,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        let window_id = ctx.window_id();
        let args = args.clone();
        self.workspace.update(ctx, |workspace, ctx| {
            workspace.open_linear_issue_work(&args, ctx);
        });
        ctx.windows().show_window_and_focus_app(window_id);
        true
    }

    pub fn focus(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        ctx.focus(&self.workspace);
        ctx.notify();
        true
    }

    /// Stops active voice input, if the configured voice input toggle key is released.
    #[cfg(feature = "voice_input")]
    fn maybe_stop_active_voice_input(
        &mut self,
        key_code: &warpui::platform::keyboard::KeyCode,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        use crate::settings::AISettings;
        use voice_input::{VoiceInput, VoiceInputState, VoiceInputToggledFrom};
        use warpui::event::KeyState;

        // Check that the released key matches the configured voice input toggle key.
        let ai_settings = AISettings::as_ref(ctx);
        if let Some(configured_key_code) = ai_settings.voice_input_toggle_key.value().to_key_code()
        {
            if configured_key_code == *key_code {
                let voice_input = VoiceInput::handle(ctx);
                // Check if we're actively listening and it was started from a key press.
                if let VoiceInputState::Listening { enabled_from, .. } =
                    voice_input.as_ref(ctx).state()
                {
                    if matches!(
                        enabled_from,
                        VoiceInputToggledFrom::Key {
                            state: KeyState::Pressed
                        }
                    ) {
                        log::debug!("Voice input key release detected: {key_code:?}");
                        // Stop listening and proceed to transcription (don't abort).
                        voice_input.update(ctx, |voice_input, ctx| {
                            if let Err(e) = voice_input.stop_listening(ctx) {
                                log::error!("Failed to stop voice input on key release: {e:?}");
                            }
                        });
                    }
                }
            }
        }
        true
    }
}

#[derive(Clone, Debug)]
pub enum RootViewEvent {
    WorkspaceReady,
}

impl Entity for RootView {
    type Event = RootViewEvent;
}

impl View for RootView {
    fn ui_name() -> &'static str {
        "RootView"
    }

    fn on_focus(&mut self, focus_ctx: &FocusContext, ctx: &mut ViewContext<Self>) {
        if focus_ctx.is_self_focused() {
            self.focus(ctx);
        }
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let child = ChildView::new(&self.workspace).finish();

        let mut stack = Stack::new();
        stack.add_child(child);

        cfg_if::cfg_if! {
            if #[cfg(feature = "voice_input")] {
                use warpui::elements::{EventHandler, DispatchEventResult};
                EventHandler::new(stack.finish())
                    .on_modifier_state_changed(|ctx, _app, key_code, key_state| {
                        if matches!(key_state, warpui::event::KeyState::Released) {
                            ctx.dispatch_action("root_view:maybe_stop_active_voice_input", *key_code);
                        }
                        DispatchEventResult::PropagateToParent
                    })
                    .finish()
            } else {
                stack.finish()
            }
        }
    }

    fn keymap_context(&self, app: &AppContext) -> warpui::keymap::Context {
        let mut context = Self::default_keymap_context();
        if quake_mode_window_is_open() {
            context.set.insert(flags::QUAKE_WINDOW_OPEN_FLAG);
        }
        if *KeysSettings::as_ref(app).quake_mode_enabled {
            context.set.insert(flags::QUAKE_MODE_ENABLED_CONTEXT_FLAG);
        }
        if *KeysSettings::as_ref(app).activation_hotkey_enabled.value() {
            context.set.insert(flags::ACTIVATION_HOTKEY_FLAG);
        }
        context
    }
}

#[derive(Clone, Debug)]
pub enum RootViewAction {
    ToggleQuakeModeWindow,
    ShowOrHideNonQuakeModeWindows,
    ToggleFullscreen,
}

impl TypedActionView for RootView {
    type Action = RootViewAction;
    fn handle_action(&mut self, action: &RootViewAction, ctx: &mut ViewContext<Self>) {
        match action {
            RootViewAction::ToggleQuakeModeWindow => {
                let global_resource_handles =
                    GlobalResourceHandlesProvider::as_ref(ctx).get().clone();
                toggle_quake_mode_window(&global_resource_handles, ctx)
            }
            RootViewAction::ShowOrHideNonQuakeModeWindows => {
                show_or_hide_non_quake_mode_windows(&(), ctx)
            }
            RootViewAction::ToggleFullscreen => {
                let window_id = ctx.window_id();
                WindowManager::handle(ctx).update(ctx, |state, ctx| {
                    state.toggle_fullscreen(window_id, ctx);
                });
            }
        }
    }
}

impl WorkspaceArgs {
    fn create_workspace(self, ctx: &mut ViewContext<RootView>) -> ViewHandle<Workspace> {
        ctx.add_typed_action_view(|ctx| {
            Workspace::new(self.global_resource_handles, self.workspace_setting, ctx)
        })
    }
}
