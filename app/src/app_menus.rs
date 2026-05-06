use std::borrow::Cow;
use std::fs::File;
use std::path::PathBuf;

use crate::ai::persisted_workspace::PersistedWorkspace;
use crate::auth::AuthStateProvider;
use crate::default_terminal::DefaultTerminal;
use crate::features::{runtime_flags_menu_items, FeatureFlag};
use crate::root_view::OpenLaunchConfigArg;
use crate::server::telemetry::LaunchConfigUiLocation;
use crate::settings::{
    AISettings, BlockVisibilitySettings, DebugSettings, DefaultSessionMode, SelectionSettings,
};
use crate::terminal::alt_screen_reporting::AltScreenReporting;
use crate::terminal::session_settings::SessionSettings;
use crate::terminal::settings::{SpacingMode, TerminalSettings};
use crate::undo_close::UndoCloseStack;
use crate::user_config::WarpConfig;
use crate::util::bindings::{self, trigger_to_keystroke, CustomAction};
use crate::util::links;
use crate::workspace::sync_inputs::SyncedInputState;
use crate::{auth, i18n, report_if_error};
use crate::settings::LanguageSettings;
use warpui::SingletonEntity as _;
use ai::workspace::WorkspaceMetadata;
use csv::Writer;
use enclose::enclose;
use itertools::Itertools;
use settings::manager::SettingsManager;
use settings::Setting as _;
use warp_core::context_flag::ContextFlag;
use warp_util::path::user_friendly_path;
use warpui::actions::StandardAction;
use warpui::keymap::{Keystroke, Trigger};
use warpui::platform::menu::{
    CustomMenuItem, Menu, MenuBar, MenuItem, MenuItemProperties, MenuItemPropertyChanges,
};
use warpui::windowing::WindowManager;
use warpui::AppContext;

type CheckmarkStatusGetter = dyn 'static + Fn(&mut AppContext) -> bool;

/// Translate a menu string key using the current language setting.
fn menu_t(key: &'static str, ctx: &AppContext) -> String {
    use settings::Setting as _;
    let lang = i18n::lang_code(*LanguageSettings::as_ref(ctx).ui_language.value());
    i18n::translate(key, lang).to_owned()
}

const ENABLE_SHELL_DEBUG_MODE_MENU_ITEM_NAME: &str =
    "Enable Shell Debug Mode (-x) for New Sessions";
const DISABLE_SHELL_DEBUG_MODE_MENU_ITEM_NAME: &str =
    "Disable Shell Debug Mode (-x) for New Sessions";
const ENABLE_IN_BAND_GENERATORS_MENU_ITEM_NAME: &str = "Enable In-band Generators for New Sessions";
const DISABLE_IN_BAND_GENERATORS_MENU_ITEM_NAME: &str =
    "Disable in-band generators for new sessions";
const ENABLE_PTY_RECORDING: &str = "Enable PTY Recording Mode (warp.pty.recording)";
const DISABLE_PTY_RECORDING: &str = "Disable PTY Recording Mode (warp.pty.recording)";
const SHOW_BOOTSTRAP_BLOCK_MENU_ITEM_NAME: &str = "Show Initialization Block";
const HIDE_BOOTSTRAP_BLOCK_MENU_ITEM_NAME: &str = "Hide Initialization Block";
const SHOW_IN_BAND_COMMAND_BLOCKS_MENU_ITEM_NAME: &str = "Show In-band Command Blocks";
const HIDE_IN_BAND_COMMAND_BLOCKS_MENU_ITEM_NAME: &str = "Hide In-band Command Blocks";
const SHOW_SSH_COMMAND_BLOCKS_MENU_ITEM_NAME: &str = "Show Warpified SSH Blocks";
const HIDE_SSH_COMMAND_BLOCKS_MENU_ITEM_NAME: &str = "Hide Warpified SSH Blocks";
const EXPORT_DEFAULT_SETTINGS_CSV_MENU_ITEM_NAME: &str =
    "Export Default Settings as CSV to home dir";

const SETTINGS_CSV_FILE_NAME: &str = "warp_default_settings.csv";
const MAX_RECENT_REPOS_IN_MENU: usize = 10;

/// Creates the root app menu bar
pub fn menu_bar(ctx: &mut AppContext) -> MenuBar {
    MenuBar::new(vec![
        make_new_app_menu(ctx),
        make_new_file_menu(ctx),
        make_new_edit_menu(ctx),
        make_new_view_menu(ctx),
        make_new_tab_menu(ctx),
        make_new_blocks_menu(ctx),
        make_new_ai_menu(ctx),
        make_new_drive_menu(ctx),
        make_new_window_menu(ctx),
        make_new_help_menu(ctx),
    ])
}

// Creates the app dock menu
// Menu here instead of MenuBar, since we only need one vec<MenuItem> in the dock
// To create submenus, we could use MenuItem::Custom(CustomMenuItem::new_with_submenu(...))
pub fn dock_menu() -> Menu {
    Menu::new(
        "New Window",
        vec![MenuItem::Custom(CustomMenuItem::new(
            "New Window",
            move |ctx| {
                ctx.dispatch_global_action("root_view:open_new", &());
                ctx.dispatch_global_action("workspace:save_app", &());
            },
            no_updates,
            Some(Keystroke::parse("cmd-n").expect("Valid keystroke")),
        ))],
    )
}

fn custom_shortcut(action: CustomAction) -> Option<Keystroke> {
    trigger_to_keystroke(&Trigger::Custom(action.into()))
}

fn default_name(action: CustomAction, ctx: &AppContext) -> String {
    ctx.description_for_custom_action(action.into(), bindings::MAC_MENUS_CONTEXT)
        .unwrap_or_else(|| {
            debug_assert!(false, "action should have a name: {action:?}");
            "<NO DESCRIPTION>".into()
        })
}

fn non_updateable_custom_item(action: CustomAction, ctx: &AppContext) -> MenuItem {
    MenuItem::Custom(CustomMenuItem::new(
        &default_name(action, ctx),
        custom_action_dispatcher(action),
        no_updates,
        custom_shortcut(action),
    ))
}

/// Return a Custom Menu Item whose CustomAction can be updated and
/// whose checkmark status is determined by
/// the bool result of should_be_checked, a provided closure
fn updateable_custom_item_with_checkmark(
    action: CustomAction,
    ctx: &AppContext,
    should_be_checked: Box<CheckmarkStatusGetter>,
) -> MenuItem {
    MenuItem::Custom(CustomMenuItem::new(
        &default_name(action, ctx),
        custom_action_dispatcher(action),
        custom_action_updater(action, should_be_checked),
        custom_shortcut(action),
    ))
}

/// Return a Custom Menu Item whose CustomAction can be updated
/// and is always unchecked
fn updateable_custom_item_without_checkmark(action: CustomAction, ctx: &AppContext) -> MenuItem {
    updateable_custom_item_with_checkmark(action, ctx, Box::new(|_| false))
}

/// Build a menu item that shows the standard action name but overrides it with a
/// JSON-keyed translation.  The key is looked up on every update so language
/// switches are reflected immediately.
fn translated_tab_item(
    action: CustomAction,
    translation_key: &'static str,
    ctx: &AppContext,
) -> MenuItem {
    MenuItem::Custom(CustomMenuItem::new(
        &menu_t(translation_key, ctx),
        custom_action_dispatcher(action),
        move |_props, ctx| {
            let mut changes = MenuItemPropertyChanges {
                name: Some(menu_t(translation_key, ctx)),
                ..Default::default()
            };
            ctx.update_custom_action_binding(action.into(), |binding| {
                changes.disabled = Some(binding.is_none());
                if let Some(binding) = binding {
                    changes.keystroke = Some(bindings::trigger_to_keystroke(binding.trigger));
                }
            });
            changes
        },
        custom_shortcut(action),
    ))
}

fn make_new_app_menu(ctx: &AppContext) -> Menu {
    let mut menu_items = vec![MenuItem::Custom(CustomMenuItem::new(
        &menu_t("menu.app.about_warp", ctx),
        custom_action_dispatcher(CustomAction::ShowAboutWarp),
        |_props, ctx| MenuItemPropertyChanges {
            name: Some(menu_t("menu.app.about_warp", ctx)),
            ..Default::default()
        },
        custom_shortcut(CustomAction::ShowAboutWarp),
    ))];

    if !FeatureFlag::AvatarInTabBar.is_enabled() {
        menu_items.push(updateable_custom_item_without_checkmark(
            CustomAction::ToggleResourceCenter,
            ctx,
        ))
    }

    menu_items.extend([
        MenuItem::Separator,
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.app.invite_people", ctx),
            custom_action_dispatcher(CustomAction::ReferAFriend),
            |_props, ctx| MenuItemPropertyChanges {
                name: Some(menu_t("menu.app.invite_people", ctx)),
                ..Default::default()
            },
            custom_shortcut(CustomAction::ReferAFriend),
        )),
        MenuItem::Separator,
    ]);

    let preferences_menu_items = vec![
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.app.settings", ctx),
            custom_action_dispatcher(CustomAction::ShowSettings),
            |_props, ctx| MenuItemPropertyChanges {
                name: Some(menu_t("menu.app.settings", ctx)),
                ..Default::default()
            },
            custom_shortcut(CustomAction::ShowSettings),
        )),
        MenuItem::Separator,
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.app.toggle_keyboard_shortcuts", ctx),
            custom_action_dispatcher(CustomAction::ToggleKeybindingsPage),
            |_props, ctx| MenuItemPropertyChanges {
                name: Some(menu_t("menu.app.toggle_keyboard_shortcuts", ctx)),
                ..Default::default()
            },
            custom_shortcut(CustomAction::ToggleKeybindingsPage),
        )),
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.app.configure_keyboard_shortcuts", ctx),
            custom_action_dispatcher(CustomAction::ConfigureKeybindings),
            |_props, ctx| MenuItemPropertyChanges {
                name: Some(menu_t("menu.app.configure_keyboard_shortcuts", ctx)),
                ..Default::default()
            },
            custom_shortcut(CustomAction::ConfigureKeybindings),
        )),
        MenuItem::Separator,
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.app.appearance", ctx),
            custom_action_dispatcher(CustomAction::ShowAppearance),
            |_props, ctx| MenuItemPropertyChanges {
                name: Some(menu_t("menu.app.appearance", ctx)),
                ..Default::default()
            },
            custom_shortcut(CustomAction::ShowAppearance),
        )),
        MenuItem::Separator,
    ];

    menu_items.push(MenuItem::Custom(CustomMenuItem::new_with_submenu(
        menu_t("menu.app.preferences", ctx).as_str(),
        |_| (),
        |_props, ctx| MenuItemPropertyChanges { name: Some(menu_t("menu.app.preferences", ctx)), ..Default::default() },
        None,
        preferences_menu_items,
    )));

    if FeatureFlag::Changelog.is_enabled() {
        menu_items.push(updateable_custom_item_without_checkmark(
            CustomAction::ViewChangelog,
            ctx,
        ));
    }

    #[cfg(target_os = "macos")]
    {
        menu_items.push(MenuItem::Services);
    }

    menu_items.push(MenuItem::Separator);
    menu_items.push(MenuItem::Custom(CustomMenuItem::new(
        &menu_t("menu.app.privacy_policy", ctx),
        |ctx| ctx.open_url(links::PRIVACY_POLICY_URL),
        |_props, ctx| MenuItemPropertyChanges {
            name: Some(menu_t("menu.app.privacy_policy", ctx)),
            ..Default::default()
        },
        None,
    )));

    let debug_menu_items = debug_menu_items();
    if !debug_menu_items.is_empty() {
        menu_items.push(MenuItem::Custom(CustomMenuItem::new_with_submenu(
            menu_t("menu.app.debug", ctx).as_str(),
            |_| (),
            |_props, ctx| MenuItemPropertyChanges { name: Some(menu_t("menu.app.debug", ctx)), ..Default::default() },
            None,
            debug_menu_items,
        )));
    }

    menu_items.push(MenuItem::Separator);
    menu_items.push(MenuItem::Standard(StandardAction::Hide));
    menu_items.push(MenuItem::Standard(StandardAction::HideOtherApps));
    menu_items.push(MenuItem::Standard(StandardAction::ShowAllApps));
    menu_items.push(MenuItem::Separator);
    menu_items.push(MenuItem::Custom(CustomMenuItem::new(
        &menu_t("menu.app.set_default_terminal", ctx),
        move |ctx| {
            DefaultTerminal::handle(ctx).update(ctx, |default_terminal, ctx| {
                default_terminal.make_warp_default(ctx)
            });
        },
        move |_props, ctx| {
            let default_terminal = DefaultTerminal::handle(ctx).as_ref(ctx);
            MenuItemPropertyChanges {
                name: Some(menu_t("menu.app.set_default_terminal", ctx)),
                disabled: Some(
                    !DefaultTerminal::can_warp_become_default()
                        || default_terminal.is_warp_default(),
                ),
                ..Default::default()
            }
        },
        None,
    )));
    menu_items.push(MenuItem::Separator);
    menu_items.push(MenuItem::Custom(CustomMenuItem::new(
        &menu_t("menu.app.log_out", ctx),
        auth::maybe_log_out,
        move |_, ctx| {
            let is_anonymous = AuthStateProvider::handle(ctx)
                .as_ref(ctx)
                .get()
                .is_anonymous_or_logged_out();
            MenuItemPropertyChanges {
                name: Some(menu_t("menu.app.log_out", ctx)),
                disabled: Some(is_anonymous),
                ..Default::default()
            }
        },
        None,
    )));
    menu_items.push(MenuItem::Standard(StandardAction::Quit));
    Menu::new("Warp", menu_items)
}

fn make_new_file_menu(ctx: &AppContext) -> Menu {
    let mut file_menu_options = make_new_elements_menu_items(ctx);
    file_menu_options.extend([
        MenuItem::Separator,
        updateable_custom_item_without_checkmark(CustomAction::OpenRepository, ctx),
        MenuItem::Custom(CustomMenuItem::new_with_submenu(
            &menu_t("menu.file.open_recent", ctx),
            |_| (),
            |_props, ctx| {
                let recent_repos = generate_recent_repos_for_menu(ctx);
                MenuItemPropertyChanges {
                    name: Some(menu_t("menu.file.open_recent", ctx)),
                    submenu: Some(Some(make_recent_repos_menu_items(ctx))),
                    disabled: Some(recent_repos.is_empty()),
                    ..Default::default()
                }
            },
            None,
            vec![],
        )),
        MenuItem::Separator,
        updateable_custom_item_without_checkmark(CustomAction::CloseCurrentSession, ctx),
        updateable_custom_item_without_checkmark(CustomAction::CloseWindow, ctx),
    ]);

    Menu::new(menu_t("menu.file", ctx), file_menu_options)
}

fn make_new_edit_menu(ctx: &AppContext) -> Menu {
    let mut edit_menu_items = vec![];

    let group_1 = vec![
        translated_tab_item(CustomAction::Undo, "menu.edit.undo", ctx),
        translated_tab_item(CustomAction::Redo, "menu.edit.redo", ctx),
    ];
    let group_2 = vec![
        translated_tab_item(CustomAction::Cut, "menu.edit.cut", ctx),
        translated_tab_item(CustomAction::Copy, "menu.edit.copy", ctx),
        translated_tab_item(CustomAction::Paste, "menu.edit.paste", ctx),
        translated_tab_item(CustomAction::SelectAll, "menu.edit.select_all", ctx),
        translated_tab_item(CustomAction::ClearEditor, "menu.edit.clear_editor", ctx),
    ];
    let group_3 = vec![
        translated_tab_item(CustomAction::AddNextOccurrence, "menu.edit.add_next_occurrence", ctx),
        translated_tab_item(CustomAction::AddCursorAbove, "menu.edit.add_cursor_above", ctx),
        translated_tab_item(CustomAction::AddCursorBelow, "menu.edit.add_cursor_below", ctx),
    ];
    let group_4 = vec![
        translated_tab_item(CustomAction::Find, "menu.edit.find_in_terminal", ctx),
        translated_tab_item(CustomAction::GoToLine, "menu.edit.go_to_line", ctx),
        translated_tab_item(CustomAction::FocusInput, "menu.edit.focus_terminal_input", ctx),
    ];
    let group_5 = vec![
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.edit.use_warps_prompt", ctx),
            move |ctx| ctx.dispatch_global_action("app:toggle_user_ps1", &()),
            move |_props, ctx| MenuItemPropertyChanges {
                name: Some(menu_t("menu.edit.use_warps_prompt", ctx)),
                checked: Some(
                    SessionSettings::handle(ctx).read(ctx, |session_settings, _ctx| {
                        !session_settings.honor_ps1.value()
                    }),
                ),
                ..Default::default()
            },
            None,
        )),
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.edit.copy_on_select", ctx),
            move |ctx| {
                ctx.dispatch_global_action("app:toggle_copy_on_select", &());
            },
            move |_props, ctx| MenuItemPropertyChanges {
                name: Some(menu_t("menu.edit.copy_on_select", ctx)),
                checked: Some(
                    SelectionSettings::handle(ctx)
                        .as_ref(ctx)
                        .copy_on_select_enabled(),
                ),
                ..Default::default()
            },
            None,
        )),
    ];

    edit_menu_items.extend(group_1);
    edit_menu_items.push(MenuItem::Separator);
    edit_menu_items.extend(group_2);
    edit_menu_items.push(MenuItem::Separator);
    edit_menu_items.extend(group_3);
    edit_menu_items.push(MenuItem::Separator);
    edit_menu_items.extend(group_4);
    edit_menu_items.push(MenuItem::Separator);

    edit_menu_items.push(MenuItem::Custom(CustomMenuItem::new_with_submenu(
        &menu_t("menu.edit.synchronize_inputs", ctx),
        |_| (),
        |_props, ctx| MenuItemPropertyChanges { name: Some(menu_t("menu.edit.synchronize_inputs", ctx)), ..Default::default() },
        None,
        vec![
            updateable_custom_item_without_checkmark(
                CustomAction::ToggleSyncAllTerminalInputsInAllTabs,
                ctx,
            ),
            updateable_custom_item_without_checkmark(
                CustomAction::ToggleSyncTerminalInputsInCurrentTab,
                ctx,
            ),
            updateable_custom_item_with_checkmark(
                CustomAction::DisableSyncTerminalInputs,
                ctx,
                Box::new(|ctx| {
                    if let Some(window_id) = WindowManager::handle(ctx).as_ref(ctx).active_window()
                    {
                        SyncedInputState::handle(ctx)
                            .read(ctx, |status, _| !status.is_syncing_any_inputs(window_id))
                    } else {
                        false
                    }
                }),
            ),
        ],
    )));
    edit_menu_items.push(MenuItem::Separator);

    edit_menu_items.extend(group_5);

    Menu::new(menu_t("menu.edit", ctx), edit_menu_items)
}

fn make_new_view_menu(ctx: &AppContext) -> Menu {
    let mut items = vec![
        translated_tab_item(CustomAction::ToggleWarpDrive, "menu.drive.toggle_drive", ctx),
        MenuItem::Separator,
        translated_tab_item(CustomAction::CommandPalette, "menu.view.command_palette", ctx),
        translated_tab_item(CustomAction::NavigationPalette, "menu.view.navigation_palette", ctx),
        translated_tab_item(CustomAction::LaunchConfigPalette, "menu.view.launch_config_palette", ctx),
        translated_tab_item(CustomAction::FilesPalette, "menu.view.files_palette", ctx),
        translated_tab_item(CustomAction::ToggleConversationListView, "menu.view.agent_conversations", ctx),
        translated_tab_item(CustomAction::ToggleProjectExplorer, "menu.view.project_explorer_panel", ctx),
        translated_tab_item(CustomAction::ToggleGlobalSearch, "menu.view.global_search_panel", ctx),
        MenuItem::Separator,
        translated_tab_item(CustomAction::History, "menu.view.show_history", ctx),
        translated_tab_item(CustomAction::CommandSearch, "menu.view.command_search", ctx),
        translated_tab_item(CustomAction::Workflows, "menu.view.workflows", ctx),
        MenuItem::Separator,
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.view.toggle_mouse_reporting", ctx),
            move |ctx| {
                ctx.dispatch_global_action("workspace:toggle_mouse_reporting", &());
            },
            move |_props, ctx| {
                let mouse_reporting_enabled = AltScreenReporting::handle(ctx)
                    .as_ref(ctx)
                    .mouse_reporting_enabled
                    .value();
                MenuItemPropertyChanges {
                    name: Some(menu_t("menu.view.toggle_mouse_reporting", ctx)),
                checked: Some(*mouse_reporting_enabled),
                    ..Default::default()
                }
            },
            None,
        )),
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.view.toggle_scroll_reporting", ctx),
            move |ctx| {
                ctx.dispatch_global_action("workspace:toggle_scroll_reporting", &());
            },
            move |_props, ctx| {
                let reporting = AltScreenReporting::handle(ctx).as_ref(ctx);
                MenuItemPropertyChanges {
                    name: Some(menu_t("menu.view.toggle_scroll_reporting", ctx)),
                    disabled: Some(!*reporting.mouse_reporting_enabled.value()),
                    checked: Some(*reporting.scroll_reporting_enabled.value()),
                    ..Default::default()
                }
            },
            None,
        )),
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.view.toggle_focus_reporting", ctx),
            move |ctx| {
                ctx.dispatch_global_action("workspace:toggle_focus_reporting", &());
            },
            move |_props, ctx| {
                let reporting = AltScreenReporting::handle(ctx).as_ref(ctx);
                MenuItemPropertyChanges {
                    name: Some(menu_t("menu.view.toggle_focus_reporting", ctx)),
                    checked: Some(*reporting.focus_reporting_enabled.value()),
                    ..Default::default()
                }
            },
            None,
        )),
    ];

    let is_compact_mode = matches!(
        TerminalSettings::handle(ctx)
            .as_ref(ctx)
            .spacing_mode
            .value(),
        SpacingMode::Compact
    );

    items.extend([
        MenuItem::Separator,
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.view.compact_mode", ctx),
            move |ctx| {
                TerminalSettings::handle(ctx).update(ctx, |terminal_settings, ctx| {
                    let current_value = *terminal_settings.spacing_mode;
                    report_if_error!(terminal_settings
                        .spacing_mode
                        .set_value(current_value.other_mode(), ctx));
                });
            },
            move |_props, ctx| MenuItemPropertyChanges {
                name: Some(menu_t("menu.view.compact_mode", ctx)),
                checked: Some(is_compact_mode),
                ..Default::default()
            },
            None,
        )),
        MenuItem::Separator,
    ]);

    if FeatureFlag::UIZoom.is_enabled() {
        items.extend([
            translated_tab_item(CustomAction::IncreaseZoom, "menu.view.zoom_in", ctx),
            translated_tab_item(CustomAction::DecreaseZoom, "menu.view.zoom_out", ctx),
            translated_tab_item(CustomAction::ResetZoom, "menu.view.reset_zoom_level", ctx),
            MenuItem::Separator,
        ]);
    } else {
        items.extend([
            translated_tab_item(CustomAction::IncreaseFontSize, "menu.view.increase_font_size", ctx),
            translated_tab_item(CustomAction::DecreaseFontSize, "menu.view.decrease_font_size", ctx),
            translated_tab_item(CustomAction::ResetFontSize, "menu.view.reset_font_size", ctx),
            MenuItem::Separator,
        ]);
    }

    Menu::new(menu_t("menu.view", ctx), items)
}

fn make_new_tab_menu(ctx: &AppContext) -> Menu {
    let items = vec![
        translated_tab_item(CustomAction::RenameTab, "menu.tab.rename", ctx),
        MenuItem::Separator,
        translated_tab_item(CustomAction::SplitPaneRight, "menu.tab.split_pane_right", ctx),
        translated_tab_item(CustomAction::SplitPaneLeft, "menu.tab.split_pane_left", ctx),
        translated_tab_item(CustomAction::SplitPaneDown, "menu.tab.split_pane_down", ctx),
        translated_tab_item(CustomAction::SplitPaneUp, "menu.tab.split_pane_up", ctx),
        MenuItem::Separator,
        // MoveTabLeft/Right: vertical-tabs mode shows "Up"/"Down" — pick key dynamically
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.tab.move_tab_left", ctx),
            custom_action_dispatcher(CustomAction::MoveTabLeft),
            |_props, ctx| {
                let key = if crate::tab::uses_vertical_tabs(ctx) {
                    "menu.tab.move_tab_up"
                } else {
                    "menu.tab.move_tab_left"
                };
                let mut changes = MenuItemPropertyChanges {
                    name: Some(menu_t(key, ctx)),
                    ..Default::default()
                };
                ctx.update_custom_action_binding(CustomAction::MoveTabLeft.into(), |b| {
                    changes.disabled = Some(b.is_none());
                    if let Some(b) = b {
                        changes.keystroke = Some(bindings::trigger_to_keystroke(b.trigger));
                    }
                });
                changes
            },
            custom_shortcut(CustomAction::MoveTabLeft),
        )),
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.tab.move_tab_right", ctx),
            custom_action_dispatcher(CustomAction::MoveTabRight),
            |_props, ctx| {
                let key = if crate::tab::uses_vertical_tabs(ctx) {
                    "menu.tab.move_tab_down"
                } else {
                    "menu.tab.move_tab_right"
                };
                let mut changes = MenuItemPropertyChanges {
                    name: Some(menu_t(key, ctx)),
                    ..Default::default()
                };
                ctx.update_custom_action_binding(CustomAction::MoveTabRight.into(), |b| {
                    changes.disabled = Some(b.is_none());
                    if let Some(b) = b {
                        changes.keystroke = Some(bindings::trigger_to_keystroke(b.trigger));
                    }
                });
                changes
            },
            custom_shortcut(CustomAction::MoveTabRight),
        )),
        MenuItem::Separator,
        translated_tab_item(CustomAction::CycleNextSession, "menu.tab.switch_to_next", ctx),
        translated_tab_item(CustomAction::CyclePrevSession, "menu.tab.switch_to_previous", ctx),
        MenuItem::Separator,
        translated_tab_item(CustomAction::ActivateNextPane, "menu.tab.activate_next_pane", ctx),
        translated_tab_item(CustomAction::ActivatePreviousPane, "menu.tab.activate_previous_pane", ctx),
        MenuItem::Separator,
        translated_tab_item(CustomAction::ToggleMaximizePane, "menu.tab.toggle_maximize_pane", ctx),
        MenuItem::Separator,
        translated_tab_item(CustomAction::CloseTab, "menu.tab.close_current", ctx),
        translated_tab_item(CustomAction::CloseOtherTabs, "menu.tab.close_other", ctx),
        // CloseTabsRight: vertical-tabs shows "Close Tabs Below"
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.tab.close_tabs_right", ctx),
            custom_action_dispatcher(CustomAction::CloseTabsRight),
            |_props, ctx| {
                let key = if crate::tab::uses_vertical_tabs(ctx) {
                    "menu.tab.close_tabs_below"
                } else {
                    "menu.tab.close_tabs_right"
                };
                let mut changes = MenuItemPropertyChanges {
                    name: Some(menu_t(key, ctx)),
                    ..Default::default()
                };
                ctx.update_custom_action_binding(CustomAction::CloseTabsRight.into(), |b| {
                    changes.disabled = Some(b.is_none());
                    if let Some(b) = b {
                        changes.keystroke = Some(bindings::trigger_to_keystroke(b.trigger));
                    }
                });
                changes
            },
            custom_shortcut(CustomAction::CloseTabsRight),
        )),
    ];
    Menu::new(menu_t("menu.tab", ctx), items)
}

fn make_new_ai_menu(ctx: &AppContext) -> Menu {
    let mut items = vec![translated_tab_item(CustomAction::NewAgentModePane, "menu.ai.new_agent_pane", ctx)];

    items.push(translated_tab_item(CustomAction::AttachSelectionAsAgentModeContext, "menu.ai.attach_selection", ctx));

    items.extend([
        MenuItem::Separator,
        translated_tab_item(CustomAction::AISearch, "menu.ai.ai_command_suggestions", ctx),
    ]);

    if FeatureFlag::AIRules.is_enabled() {
        items.extend([
            MenuItem::Separator,
            translated_tab_item(CustomAction::OpenAIFactCollection, "menu.ai.open_ai_rules", ctx),
        ]);
    }

    if FeatureFlag::McpServer.is_enabled() && ContextFlag::ShowMCPServers.is_enabled() {
        items.push(updateable_custom_item_without_checkmark(
            CustomAction::OpenMCPServerCollection,
            ctx,
        ));
    }

    Menu::new(menu_t("menu.ai", ctx), items)
}

fn make_new_blocks_menu(ctx: &AppContext) -> Menu {
    let mut items = vec![
        translated_tab_item(CustomAction::ClearBlocks, "menu.blocks.clear", ctx),
        MenuItem::Separator,
        translated_tab_item(CustomAction::SelectBlockAbove, "menu.blocks.select_previous", ctx),
        translated_tab_item(CustomAction::SelectBlockBelow, "menu.blocks.select_next", ctx),
        translated_tab_item(CustomAction::SelectAllBlocks, "menu.blocks.select_all", ctx),
        MenuItem::Separator,
    ];
    items.push(translated_tab_item(CustomAction::ScrollToTopOfSelectedBlocks, "menu.blocks.scroll_to_top", ctx));
    items.push(translated_tab_item(CustomAction::ScrollToBottomOfSelectedBlocks, "menu.blocks.scroll_to_bottom", ctx));
    items.push(MenuItem::Separator);
    items.extend([
        translated_tab_item(CustomAction::CreateBlockPermalink, "menu.blocks.share", ctx),
        translated_tab_item(CustomAction::ViewSharedBlocks, "menu.blocks.view_shared", ctx),
        translated_tab_item(CustomAction::ToggleBookmarkBlock, "menu.blocks.bookmark", ctx),
        translated_tab_item(CustomAction::FindWithinBlock, "menu.blocks.find_within", ctx),
        MenuItem::Separator,
        translated_tab_item(CustomAction::CopyBlock, "menu.blocks.copy_all", ctx),
        translated_tab_item(CustomAction::CopyBlockCommand, "menu.blocks.copy_command", ctx),
        translated_tab_item(CustomAction::CopyBlockOutput, "menu.blocks.copy_output", ctx),
    ]);

    let debug_items = block_menu_debug_items(ctx);
    if !debug_items.is_empty() {
        items.push(MenuItem::Separator);
        items.extend(debug_items);
    }

    Menu::new(menu_t("menu.blocks", ctx), items)
}

fn make_new_drive_menu(ctx: &AppContext) -> Menu {
    let mut items = vec![
        translated_tab_item(CustomAction::NewPersonalWorkflow, "menu.drive.new_personal_workflow", ctx),
        translated_tab_item(CustomAction::NewPersonalNotebook, "menu.drive.new_personal_notebook", ctx),
        translated_tab_item(CustomAction::NewPersonalAIPrompt, "menu.drive.new_personal_prompt", ctx),
    ];
    items.push(translated_tab_item(CustomAction::NewPersonalEnvVars, "menu.drive.new_personal_env_vars", ctx));
    items.extend([
        MenuItem::Separator,
        translated_tab_item(CustomAction::NewTeamWorkflow, "menu.drive.new_team_workflow", ctx),
        translated_tab_item(CustomAction::NewTeamNotebook, "menu.drive.new_team_notebook", ctx),
        translated_tab_item(CustomAction::NewTeamAIPrompt, "menu.drive.new_team_prompt", ctx),
    ]);
    items.push(translated_tab_item(CustomAction::NewTeamEnvVars, "menu.drive.new_team_env_vars", ctx));
    items.extend([
        MenuItem::Separator,
        translated_tab_item(CustomAction::ToggleWarpDrive, "menu.drive.toggle_drive", ctx),
        translated_tab_item(CustomAction::SearchDrive, "menu.drive.search", ctx),
        translated_tab_item(CustomAction::OpenTeamSettings, "menu.drive.open_team_settings", ctx),
        translated_tab_item(CustomAction::OpenAIFactCollection, "menu.drive.open_ai_rules", ctx),
        translated_tab_item(CustomAction::OpenMCPServerCollection, "menu.drive.open_mcp_servers", ctx),
    ]);

    items.push(translated_tab_item(CustomAction::SharePaneContents, "menu.drive.share_pane", ctx));

    if FeatureFlag::CreatingSharedSessions.is_enabled() {
        items.extend([
            MenuItem::Separator,
            translated_tab_item(CustomAction::ShareCurrentSession, "menu.drive.share_session", ctx),
        ])
    }

    Menu::new(menu_t("menu.drive", ctx), items)
}

/// Returns [`MenuItem`]s that aid debugging to be included in the Block menu.
fn block_menu_debug_items(ctx: &AppContext) -> Vec<MenuItem> {
    let mut items = vec![];
    if FeatureFlag::ToggleBootstrapBlock.is_enabled() {
        items.push(toggle_bootstrap_block_menu_item());
    }

    items.push(MenuItem::Custom(CustomMenuItem::new(
        &menu_t("menu.blocks.show_in_band", ctx),
        move |ctx| {
            let handle = BlockVisibilitySettings::handle(ctx);
            handle.update(ctx, |block_visibility_settings, ctx| {
                let new_value = !block_visibility_settings
                    .should_show_in_band_command_blocks
                    .value();
                if let Err(e) = block_visibility_settings
                    .should_show_in_band_command_blocks
                    .set_value(new_value, ctx)
                {
                    log::error!("Failed to persist 'Show in-band command blocks' setting: {e}");
                }
            });
        },
        move |_props, ctx| {
            let name = if BlockVisibilitySettings::handle(ctx).read(ctx, |settings, _ctx| {
                *settings.should_show_in_band_command_blocks.value()
            }) {
                menu_t("menu.blocks.hide_in_band", ctx)
            } else {
                menu_t("menu.blocks.show_in_band", ctx)
            };

            MenuItemPropertyChanges {
                name: Some(name),
                ..Default::default()
            }
        },
        None,
    )));

    items.push(MenuItem::Custom(CustomMenuItem::new(
        &menu_t("menu.blocks.show_ssh", ctx),
        move |ctx| {
            let handle = BlockVisibilitySettings::handle(ctx);
            handle.update(ctx, |block_visibility_settings, ctx| {
                let new_value = !block_visibility_settings.should_show_ssh_block.value();
                if let Err(e) = block_visibility_settings
                    .should_show_ssh_block
                    .set_value(new_value, ctx)
                {
                    log::error!("Failed to persist 'Show ssh command blocks' setting: {e}");
                }
            });
        },
        move |_props, ctx| {
            let name = if BlockVisibilitySettings::handle(ctx).read(ctx, |settings, _ctx| {
                *settings.should_show_ssh_block.value()
            }) {
                menu_t("menu.blocks.hide_ssh", ctx)
            } else {
                menu_t("menu.blocks.show_ssh", ctx)
            };

            MenuItemPropertyChanges {
                name: Some(name),
                ..Default::default()
            }
        },
        None,
    )));

    items
}

fn toggle_bootstrap_block_menu_item() -> MenuItem {
    MenuItem::Custom(CustomMenuItem::new(
        SHOW_BOOTSTRAP_BLOCK_MENU_ITEM_NAME,
        move |ctx| {
            BlockVisibilitySettings::handle(ctx).update(ctx, |block_visibility_settings, ctx| {
                let new_value = !block_visibility_settings
                    .should_show_bootstrap_block
                    .value();
                if let Err(e) = block_visibility_settings
                    .should_show_bootstrap_block
                    .set_value(new_value, ctx)
                {
                    log::error!("Failed to persist 'Show bootstrap block' setting: {e}");
                }
            });
        },
        move |_props, ctx| {
            let name = if BlockVisibilitySettings::handle(ctx).read(ctx, |settings, _ctx| {
                *settings.should_show_bootstrap_block.value()
            }) {
                HIDE_BOOTSTRAP_BLOCK_MENU_ITEM_NAME.to_owned()
            } else {
                SHOW_BOOTSTRAP_BLOCK_MENU_ITEM_NAME.to_owned()
            };

            MenuItemPropertyChanges {
                name: Some(name),
                ..Default::default()
            }
        },
        None,
    ))
}

fn make_new_window_menu(ctx: &AppContext) -> Menu {
    Menu::new(
        menu_t("menu.window", ctx),
        vec![
            MenuItem::Standard(StandardAction::Minimize),
            MenuItem::Standard(StandardAction::Zoom),
            MenuItem::Standard(StandardAction::ToggleFullScreen),
            MenuItem::Separator,
            MenuItem::Standard(StandardAction::BringAllToFront),
        ],
    )
}

fn debug_menu_items() -> Vec<MenuItem> {
    let mut debug_menu_items = vec![];

    if FeatureFlag::DebugMode.is_enabled() {
        debug_menu_items.push(MenuItem::Custom(CustomMenuItem::new(
            ENABLE_SHELL_DEBUG_MODE_MENU_ITEM_NAME,
            move |ctx| {
                DebugSettings::handle(ctx).update(ctx, |debug_settings, ctx| {
                    let new_value = !debug_settings.is_shell_debug_mode_enabled.value();
                    if let Err(e) = debug_settings
                        .is_shell_debug_mode_enabled
                        .set_value(new_value, ctx)
                    {
                        log::error!("Failed to persist 'Debug mode' setting: {e}");
                    }
                });
            },
            move |_props, ctx| {
                let name = if DebugSettings::handle(ctx).read(ctx, |settings, _ctx| {
                    *settings.is_shell_debug_mode_enabled.value()
                }) {
                    DISABLE_SHELL_DEBUG_MODE_MENU_ITEM_NAME.to_owned()
                } else {
                    ENABLE_SHELL_DEBUG_MODE_MENU_ITEM_NAME.to_owned()
                };

                MenuItemPropertyChanges {
                    name: Some(name),
                    ..Default::default()
                }
            },
            None,
        )));

        debug_menu_items.push(MenuItem::Custom(CustomMenuItem::new(
            ENABLE_PTY_RECORDING,
            move |ctx| {
                DebugSettings::handle(ctx).update(ctx, |debug_settings, ctx| {
                    let new_value = !debug_settings.recording_mode.value();
                    let _ = debug_settings.recording_mode.set_value(new_value, ctx);
                });
            },
            move |_props, ctx| {
                let name = if DebugSettings::handle(ctx)
                    .read(ctx, |settings, _ctx| *settings.recording_mode.value())
                {
                    DISABLE_PTY_RECORDING.to_owned()
                } else {
                    ENABLE_PTY_RECORDING.to_owned()
                };

                MenuItemPropertyChanges {
                    name: Some(name),
                    ..Default::default()
                }
            },
            None,
        )));

        debug_menu_items.push(MenuItem::Custom(CustomMenuItem::new(
            ENABLE_IN_BAND_GENERATORS_MENU_ITEM_NAME,
            move |ctx| {
                DebugSettings::handle(ctx).update(ctx, |debug_settings, ctx| {
                    let new_value = !debug_settings
                        .are_in_band_generators_for_all_sessions_enabled
                        .value();
                    if let Err(e) = debug_settings
                        .are_in_band_generators_for_all_sessions_enabled
                        .set_value(new_value, ctx)
                    {
                        log::error!("Failed to persist 'Enable in-band generators' setting: {e}");
                    }
                });
            },
            move |_props, ctx| {
                let name = if DebugSettings::handle(ctx).read(ctx, |settings, _ctx| {
                    *settings
                        .are_in_band_generators_for_all_sessions_enabled
                        .value()
                }) {
                    DISABLE_IN_BAND_GENERATORS_MENU_ITEM_NAME.to_owned()
                } else {
                    ENABLE_IN_BAND_GENERATORS_MENU_ITEM_NAME.to_owned()
                };

                MenuItemPropertyChanges {
                    name: Some(name),
                    ..Default::default()
                }
            },
            None,
        )));

        if !FeatureFlag::ToggleBootstrapBlock.is_enabled() {
            debug_menu_items.push(toggle_bootstrap_block_menu_item());
        }

        debug_menu_items.push(MenuItem::Custom(CustomMenuItem::new(
            "Manually Toggle Network Status",
            move |ctx| ctx.dispatch_global_action("workspace:toggle_debug_network_status", &()),
            no_updates,
            None,
        )));

        debug_menu_items.push(MenuItem::Custom(CustomMenuItem::new(
            EXPORT_DEFAULT_SETTINGS_CSV_MENU_ITEM_NAME,
            move |ctx| {
                let default_settings = SettingsManager::handle(ctx).as_ref(ctx).default_values();
                let mut writer = Writer::from_writer(
                    File::create(
                        dirs::home_dir()
                            .unwrap_or_default()
                            .join(SETTINGS_CSV_FILE_NAME),
                    )
                    .expect("Failed to create settings csv file"),
                );
                writer
                    .write_record(["storage_key", "value"])
                    .expect("Failed to write header record");
                for (storage_key, value) in default_settings {
                    log::debug!("Writing setting: {storage_key} = {value}");
                    writer
                        .write_record(&[storage_key, value])
                        .expect("Failed to write settings record");
                }
                let _ = writer.flush();
            },
            no_updates,
            None,
        )));

        debug_menu_items.push(MenuItem::Custom(CustomMenuItem::new(
            "Create anonymous user",
            move |ctx| ctx.dispatch_global_action("workspace:debug_create_anonymous_user", &()),
            no_updates,
            None,
        )));
    }

    if FeatureFlag::RuntimeFeatureFlags.is_enabled() {
        debug_menu_items.extend(runtime_flags_menu_items());
    }

    debug_menu_items
}

fn link_menu_item(title: &'static str, link: Cow<'static, str>) -> MenuItem {
    MenuItem::Custom(CustomMenuItem::new(
        title,
        move |ctx| {
            ctx.open_url(&link);
        },
        no_updates,
        None,
    ))
}

fn feedback_menu_item() -> MenuItem {
    MenuItem::Custom(CustomMenuItem::new(
        "Send Feedback...",
        move |ctx| {
            // Route through the root-view action so workspace windows can open the
            // guided AI flow, while non-workspace windows still fall back to the
            // browser-based feedback form.
            ctx.dispatch_global_action("root_view:send_feedback", &());
        },
        no_updates,
        None,
    ))
}

fn make_new_help_menu(ctx: &AppContext) -> Menu {
    Menu::new(
        menu_t("menu.help", ctx),
        vec![
            MenuItem::Custom(CustomMenuItem::new(
                &menu_t("menu.help.send_feedback", ctx),
                |ctx| { ctx.dispatch_global_action("root_view:send_feedback", &()); },
                |_props, ctx| MenuItemPropertyChanges { name: Some(menu_t("menu.help.send_feedback", ctx)), ..Default::default() },
                None,
            )),
            MenuItem::Custom(CustomMenuItem::new(
                &menu_t("menu.help.documentation", ctx),
                |ctx| ctx.open_url(links::USER_DOCS_URL),
                |_props, ctx| MenuItemPropertyChanges { name: Some(menu_t("menu.help.documentation", ctx)), ..Default::default() },
                None,
            )),
            MenuItem::Custom(CustomMenuItem::new(
                &menu_t("menu.help.github_issues", ctx),
                |ctx| ctx.open_url(links::GITHUB_ISSUES_URL),
                |_props, ctx| MenuItemPropertyChanges { name: Some(menu_t("menu.help.github_issues", ctx)), ..Default::default() },
                None,
            )),
            MenuItem::Custom(CustomMenuItem::new(
                &menu_t("menu.help.slack_community", ctx),
                |ctx| ctx.open_url(links::SLACK_URL),
                |_props, ctx| MenuItemPropertyChanges { name: Some(menu_t("menu.help.slack_community", ctx)), ..Default::default() },
                None,
            )),
        ],
    )
}

fn make_launch_config_menu_items(ctx: &mut AppContext) -> Vec<MenuItem> {
    let mut launch_config_menu_items = vec![];

    let launch_configs = WarpConfig::handle(ctx).as_ref(ctx).launch_configs();
    for config in launch_configs {
        launch_config_menu_items.push(MenuItem::Custom(CustomMenuItem::new(
            &config.name,
            enclose!((config) move |ctx| {
                ctx.dispatch_global_action(
                    "root_view:open_launch_config",
                    &OpenLaunchConfigArg {
                        launch_config: config.clone(),
                        ui_location: LaunchConfigUiLocation::AppMenu,
                        open_in_active_window: false,
                    }
                );
                ctx.dispatch_global_action("workspace:save_app", &());
            }),
            no_updates,
            None,
        )));
    }

    if !launch_config_menu_items.is_empty() {
        launch_config_menu_items.push(MenuItem::Separator);
    }

    // TODO(vorporeal): use non_updateable_custom_item() here instead
    launch_config_menu_items.push(MenuItem::Custom(CustomMenuItem::new(
        menu_t("menu.file.save_new", ctx).as_str(),
        custom_action_dispatcher(CustomAction::SaveCurrentConfig),
        no_updates,
        custom_shortcut(CustomAction::SaveCurrentConfig),
    )));

    launch_config_menu_items
}

fn make_new_elements_menu_items(ctx: &AppContext) -> Vec<MenuItem> {
    // Dynamically assign the workspace:new_tab keystroke (cmd-t) to whichever item
    // matches the user's "Default mode for new sessions" setting. The non-default item
    // shows its dedicated keystroke instead.
    let mut new_elements_menu = vec![
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.file.new_window", ctx),
            open_new_window,
            |_props, ctx| MenuItemPropertyChanges {
                name: Some(menu_t("menu.file.new_window", ctx)),
                ..Default::default()
            },
            Some(Keystroke::parse("cmd-n").expect("Valid keystroke")),
        )),
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.file.new_terminal_tab", ctx),
            open_new_default_tab_or_window,
            move |_props: &MenuItemProperties, ctx: &mut AppContext| {
                let mut changes = MenuItemPropertyChanges {
                    name: Some(menu_t("menu.file.new_terminal_tab", ctx)),
                    ..Default::default()
                };
                let is_default_session_mode_agent =
                    AISettings::handle(ctx).read(ctx, |ai_settings, ctx| {
                        ai_settings.is_any_ai_enabled(ctx)
                            && ai_settings.default_session_mode(ctx) == DefaultSessionMode::Agent
                    });
                let trigger = if is_default_session_mode_agent {
                    Trigger::Custom(CustomAction::NewTerminalTab.into())
                } else {
                    Trigger::Custom(CustomAction::NewTab.into())
                };
                let binding = ctx
                    .get_key_bindings()
                    .find(|b| b.trigger == &trigger || b.original_trigger == Some(&trigger));
                if let Some(binding) = binding {
                    changes.keystroke = Some(bindings::trigger_to_keystroke(binding.trigger));
                }
                changes
            },
            Some(Keystroke::parse("cmd-t").expect("Valid keystroke")),
        )),
        MenuItem::Custom(CustomMenuItem::new(
            &menu_t("menu.file.new_agent_tab", ctx),
            open_new_agent_tab_or_window,
            move |_props: &MenuItemProperties, ctx: &mut AppContext| {
                let mut changes = MenuItemPropertyChanges {
                    name: Some(menu_t("menu.file.new_agent_tab", ctx)),
                    ..Default::default()
                };
                let (is_any_ai_enabled, is_default_session_mode_agent) = AISettings::handle(ctx)
                    .read(ctx, |ai_settings, ctx| {
                        let enabled = ai_settings.is_any_ai_enabled(ctx);
                        let agent = enabled
                            && ai_settings.default_session_mode(ctx) == DefaultSessionMode::Agent;
                        (enabled, agent)
                    });
                if !is_any_ai_enabled {
                    changes.disabled = Some(true);
                    return changes;
                }
                let trigger = if is_default_session_mode_agent {
                    Trigger::Custom(CustomAction::NewTab.into())
                } else {
                    Trigger::Custom(CustomAction::NewAgentTab.into())
                };
                let binding = ctx
                    .get_key_bindings()
                    .find(|b| b.trigger == &trigger || b.original_trigger == Some(&trigger));
                if let Some(binding) = binding {
                    changes.keystroke = Some(bindings::trigger_to_keystroke(binding.trigger));
                }
                changes
            },
            None,
        )),
        non_updateable_custom_item(CustomAction::NewFile, ctx),
    ];

    let reopen_session_action_updater =
        custom_action_updater(CustomAction::ReopenClosedSession, Box::new(|_| false));
    new_elements_menu.push(MenuItem::Custom(CustomMenuItem::new(
        &menu_t("menu.file.reopen_closed_session", ctx),
        |ctx| {
            UndoCloseStack::handle(ctx).update(ctx, |stack, ctx| {
                stack.undo_close(ctx);
            });
        },
        move |props, ctx| {
            let mut changes = reopen_session_action_updater(props, ctx);
            changes.name = Some(menu_t("menu.file.reopen_closed_session", ctx));
            changes.disabled = Some(UndoCloseStack::handle(ctx).as_ref(ctx).is_empty());
            changes
        },
        Some(Keystroke::parse("cmd-shift-T").expect("Valid keystroke")),
    )));

    new_elements_menu.push(MenuItem::Custom(CustomMenuItem::new_with_submenu(
        &menu_t("menu.file.launch_configurations", ctx),
        |_| (),
        |_props, ctx| MenuItemPropertyChanges {
            name: Some(menu_t("menu.file.launch_configurations", ctx)),
            submenu: Some(Some(make_launch_config_menu_items(ctx))),
            ..Default::default()
        },
        None,
        vec![],
    )));

    new_elements_menu
}

/// \return a callback that dispatches a CustomAction, appropriate for plugging into a CustomMenuItem.
fn custom_action_dispatcher(action: CustomAction) -> impl Fn(&mut AppContext) + 'static {
    move |ctx| {
        if let Some(wid) = WindowManager::handle(ctx).as_ref(ctx).active_window() {
            ctx.dispatch_custom_action(action, wid)
        }
    }
}

/// Dispatch events to open the user's default tab type in the active window
/// or make a new window if there is no active window.
fn open_new_default_tab_or_window(ctx: &mut AppContext) {
    if let Some(wid) = WindowManager::handle(ctx).as_ref(ctx).active_window() {
        ctx.dispatch_custom_action(CustomAction::NewTab, wid)
    } else {
        open_new_window(ctx)
    }
}

/// Dispatch events to open an agent tab in the active window
/// or make a new window if there is no active window.
fn open_new_agent_tab_or_window(ctx: &mut AppContext) {
    if let Some(wid) = WindowManager::handle(ctx).as_ref(ctx).active_window() {
        ctx.dispatch_custom_action(CustomAction::NewAgentTab, wid)
    } else {
        open_new_window(ctx)
    }
}

/// Dispatch event to open a new Warp window
fn open_new_window(ctx: &mut AppContext) {
    ctx.dispatch_global_action("root_view:open_new", &());
    ctx.dispatch_global_action("workspace:save_app", &());
}

/// No-op updater function for custom menu items that never change.
fn no_updates(_: &MenuItemProperties, _: &mut AppContext) -> MenuItemPropertyChanges {
    Default::default()
}

fn make_recent_repos_menu_items(ctx: &AppContext) -> Vec<MenuItem> {
    let recent_repos = generate_recent_repos_for_menu(ctx);

    if recent_repos.is_empty() {
        return vec![];
    }

    let home = dirs::home_dir().map(|p| p.display().to_string());

    recent_repos
        .into_iter()
        .map(|path| {
            let full_path = path.display().to_string();
            let display_path = user_friendly_path(&full_path, home.as_deref()).into_owned();

            MenuItem::Custom(CustomMenuItem::new(
                &display_path,
                move |ctx| {
                    ctx.dispatch_global_action("workspace:open_repository", &full_path);
                },
                no_updates,
                None,
            ))
        })
        .collect()
}

fn generate_recent_repos_for_menu(ctx: &AppContext) -> Vec<PathBuf> {
    PersistedWorkspace::handle(ctx)
        .as_ref(ctx)
        .workspaces()
        .sorted_by(WorkspaceMetadata::most_recently_navigated)
        .take(MAX_RECENT_REPOS_IN_MENU)
        .map(|cbm| cbm.path)
        .collect::<Vec<_>>()
}

/// \return a callback that updates a custom action based menu item based on the
/// current keybinding context:
/// 1) Whether the item should be enabled / disabled
/// 2) The current keystroke bound to the item (either the default or a custom one)
/// 3) The name of the item
///
/// This updated is called by the mac menu system before the menu is opened.
///
/// Note, that correctly disabling the key binding is important not just for limiting
/// what a user can do in a given context, but also because if the keybinding is not
/// disabled correctly, the mac menu system will capture key events when they should
/// be flowing into the underlying views (e.g. ctrl-c needs to flow directly to the
/// alt grid in emacs, whereas if our editor is active it's ok to dispatch a custom action)
fn custom_action_updater(
    action: CustomAction,
    checkmark_status: Box<CheckmarkStatusGetter>,
) -> impl Fn(&MenuItemProperties, &mut AppContext) -> MenuItemPropertyChanges + 'static {
    move |_props, ctx| {
        let mut changes = MenuItemPropertyChanges::default();
        ctx.update_custom_action_binding(action.into(), |binding| {
            changes.disabled = Some(binding.is_none());
            if let Some(binding) = binding {
                if let Some(description) = binding.description {
                    changes.name = Some(
                        description
                            .resolve(ctx, bindings::MAC_MENUS_CONTEXT)
                            .into_owned(),
                    );
                }
                changes.keystroke = Some(bindings::trigger_to_keystroke(binding.trigger));
            }
        });

        changes.checked = Some(checkmark_status(ctx));

        changes
    }
}
