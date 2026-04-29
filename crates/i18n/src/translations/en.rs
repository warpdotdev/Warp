use std::collections::HashMap;

lazy_static::lazy_static! {
    pub static ref TRANSLATIONS: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();

        // Navigation sidebar
        m.insert("nav.account", "Account");
        m.insert("nav.appearance", "Appearance");
        m.insert("nav.features", "Features");
        m.insert("nav.keybindings", "Keyboard shortcuts");
        m.insert("nav.privacy", "Privacy");
        m.insert("nav.teams", "Teams");
        m.insert("nav.about", "About");
        m.insert("nav.ai", "AI");
        m.insert("nav.code", "Code");
        m.insert("nav.cloud_platform", "Cloud platform");
        m.insert("nav.agents", "Agents");
        m.insert("nav.billing_and_usage", "Billing and usage");
        m.insert("nav.shared_blocks", "Shared blocks");
        m.insert("nav.mcp_servers", "MCP Servers");
        m.insert("nav.warp_drive", "Warp Drive");
        m.insert("nav.warp_agent", "Warp Agent");
        m.insert("nav.agent_profiles", "Profiles");
        m.insert("nav.agent_mcp_servers", "MCP servers");
        m.insert("nav.knowledge", "Knowledge");
        m.insert("nav.third_party_cli_agents", "Third party CLI agents");
        m.insert("nav.code_indexing", "Indexing and projects");
        m.insert("nav.editor_and_code_review", "Editor and Code Review");
        m.insert("nav.cloud_environments", "Environments");
        m.insert("nav.oz_cloud_api_keys", "Oz Cloud API Keys");

        // Settings shell
        m.insert("settings.title", "Settings");
        m.insert("settings.search", "Search");

        // Settings shell - Context menu items
        m.insert("settings.shell.split_right", "Split pane right");
        m.insert("settings.shell.split_left", "Split pane left");
        m.insert("settings.shell.split_down", "Split pane down");
        m.insert("settings.shell.split_up", "Split pane up");
        m.insert("settings.shell.close_pane", "Close pane");

        // Settings shell - Toggle pair descriptions
        m.insert("settings.shell.toggle.code_review_show", "Show code review button in tab bar");
        m.insert("settings.shell.toggle.code_review_hide", "Hide code review button in tab bar");
        m.insert("settings.shell.toggle.init_block_show", "Show initialization block");
        m.insert("settings.shell.toggle.init_block_hide", "Hide initialization block");
        m.insert("settings.shell.toggle.in_band_show", "Show in-band command blocks");
        m.insert("settings.shell.toggle.in_band_hide", "Hide in-band command blocks");
        m.insert("settings.shell.toggle.tab_bar_show", "Always show tab bar");
        m.insert("settings.shell.toggle.tab_bar_hide_fullscreen", "Hide tab bar if fullscreen");
        m.insert("settings.shell.toggle.tab_bar_hover", "Only show tab bar on hover");

        // Appearance page - Language
        m.insert("settings.appearance.language.label", "Language");
        m.insert("settings.appearance.language.subtitle", "Set the display language for Warp's interface.");

        // Appearance page - Themes
        m.insert("settings.appearance.themes", "Themes");

        // Appearance page - Window
        m.insert("settings.appearance.window", "Window");
        m.insert("settings.appearance.icon", "Icon");
        m.insert("settings.appearance.window.opacity", "Window opacity");
        m.insert("settings.appearance.window.blur", "Background blur");
        m.insert("settings.appearance.window.blur_texture", "Background blur texture");

        // Appearance page - Input
        m.insert("settings.appearance.input", "Input");
        m.insert("settings.appearance.input.type", "Input type");
        m.insert("settings.appearance.input.mode", "Input position");

        // Appearance page - Blocks
        m.insert("settings.appearance.blocks", "Blocks");
        m.insert("settings.appearance.panes", "Panes");
        m.insert("settings.appearance.cursor", "Cursor");

        // Appearance page - Text
        m.insert("settings.appearance.text", "Text");
        m.insert("settings.appearance.text.font_size", "Font size");
        m.insert("settings.appearance.text.font_family", "Font family");

        // Appearance page - Full-screen Apps
        m.insert("settings.appearance.full_screen_apps", "Full-screen Apps");

        // Appearance page - Tabs
        m.insert("settings.appearance.tabs", "Tabs");

        // Appearance page - Themes (extended)
        m.insert("settings.appearance.themes.create_custom", "Create your own custom theme");
        m.insert("settings.appearance.themes.light", "Light");
        m.insert("settings.appearance.themes.dark", "Dark");
        m.insert("settings.appearance.themes.current", "Current theme");
        m.insert("settings.appearance.themes.sync_with_os", "Sync with OS");
        m.insert("settings.appearance.themes.sync_with_os.description", "Automatically switch between light and dark themes when your system does.");

        // Appearance page - Icon (extended)
        m.insert("settings.appearance.icon.customize", "Customize your app icon");
        m.insert("settings.appearance.icon.bundle_warning", "Changing the app icon requires the app to be bundled.");
        m.insert("settings.appearance.icon.restart_warning", "You may need to restart Warp for MacOS to apply the preferred icon style.");

        // Appearance page - Window (extended)
        m.insert("settings.appearance.window.custom_size", "Open new windows with custom size");
        m.insert("settings.appearance.window.columns", "Columns");
        m.insert("settings.appearance.window.rows", "Rows");
        m.insert("settings.appearance.window.opacity.label", "Window Opacity:");
        m.insert("settings.appearance.window.opacity.unsupported", "Transparency is not supported with your graphics drivers.");
        m.insert("settings.appearance.window.opacity.value", "Window Opacity: {opacity_value}");
        m.insert("settings.appearance.window.opacity.graphics_warning", "The selected graphics settings may not support rendering transparent windows.");
        m.insert("settings.appearance.window.opacity.graphics_hint", " Try changing the settings for the graphics backend or integrated GPU in Features > System.");
        m.insert("settings.appearance.window.blur.value", "Window Blur Radius: {blur_value}");
        m.insert("settings.appearance.window.blur.use_acrylic", "Use Window Blur (Acrylic texture)");
        m.insert("settings.appearance.window.blur.hardware_warning", "The selected hardware may not support rendering transparent windows.");

        // Appearance page - Panes
        m.insert("settings.appearance.panes.consistent_tools", "Tools panel visibility is consistent across tabs");
        m.insert("settings.appearance.panes.dim_inactive", "Dim inactive panes");
        m.insert("settings.appearance.panes.focus_follows_mouse", "Focus follows mouse");
        m.insert("settings.appearance.panes.compact_mode", "Compact mode");

        // Appearance page - Input (extended)
        m.insert("settings.appearance.input.warp", "Warp");
        m.insert("settings.appearance.input.shell_ps1", "Shell (PS1)");
        m.insert("settings.appearance.input.mode.warp", "Pin to the bottom (Warp mode)");
        m.insert("settings.appearance.input.mode.reverse", "Pin to the top (Reverse mode)");
        m.insert("settings.appearance.input.mode.classic", "Start at the top (Classic mode)");

        // Appearance page - Blocks (extended)
        m.insert("settings.appearance.blocks.jump_to_bottom", "Show Jump to Bottom of Block button");
        m.insert("settings.appearance.blocks.dividers", "Show block dividers");

        // Appearance page - Text (extended)
        m.insert("settings.appearance.text.agent_font", "Agent font");
        m.insert("settings.appearance.text.match_terminal", "Match terminal");
        m.insert("settings.appearance.text.line_height", "Line height");
        m.insert("settings.appearance.text.reset_default", "Reset to default");
        m.insert("settings.appearance.text.terminal_font", "Terminal font");
        m.insert("settings.appearance.text.view_system_fonts", "View all available system fonts");
        m.insert("settings.appearance.text.font_weight", "Font weight");
        m.insert("settings.appearance.text.font_size_px", "Font size (px)");
        m.insert("settings.appearance.text.notebook_font_size", "Notebook font size");
        m.insert("settings.appearance.text.thin_strokes", "Use thin strokes");
        m.insert("settings.appearance.text.min_contrast", "Enforce minimum contrast");
        m.insert("settings.appearance.text.ligatures", "Show ligatures in terminal");
        m.insert("settings.appearance.text.ligatures.warning", "Ligatures may reduce performance");

        // Appearance page - Cursor
        m.insert("settings.appearance.cursor.type", "Cursor type");
        m.insert("settings.appearance.cursor.type.disabled_vim", "Cursor type is disabled in Vim mode");
        m.insert("settings.appearance.cursor.blink", "Blinking cursor");

        // Appearance page - Tabs (extended)
        m.insert("settings.appearance.tabs.close_position", "Tab close button position");
        m.insert("settings.appearance.tabs.indicators", "Show tab indicators");
        m.insert("settings.appearance.tabs.code_review_button", "Show code review button");
        m.insert("settings.appearance.tabs.preserve_color", "Preserve active tab color for new tabs");
        m.insert("settings.appearance.tabs.vertical_layout", "Use vertical tab layout");
        m.insert("settings.appearance.tabs.prompt_as_title", "Use latest user prompt as conversation title in tab names");
        m.insert("settings.appearance.tabs.prompt_as_title.description", "Show the latest user prompt instead of the generated conversation title for Oz and third-party agent sessions in vertical tabs.");
        m.insert("settings.appearance.tabs.header_layout", "Header toolbar layout");
        m.insert("settings.appearance.tabs.directory_colors", "Directory tab colors");
        m.insert("settings.appearance.tabs.directory_colors.description", "Automatically color tabs based on the directory or repo you're working in.");
        m.insert("settings.appearance.tabs.directory_colors.default", "Default (no color)");
        m.insert("settings.appearance.tabs.show_tab_bar", "Show the tab bar");
        m.insert("settings.appearance.tabs.alt_screen_padding", "Use custom padding in alt-screen");
        m.insert("settings.appearance.tabs.uniform_padding", "Uniform padding (px)");

        // Appearance page - Zoom
        m.insert("settings.appearance.zoom", "Zoom");
        m.insert("settings.appearance.zoom.description", "Adjusts the default zoom level across all windows");

        // Appearance page - Dropdown option keys
        m.insert("settings.appearance.option.never", "Never");
        m.insert("settings.appearance.option.always", "Always");
        m.insert("settings.appearance.option.left", "Left");
        m.insert("settings.appearance.option.right", "Right");
        m.insert("settings.appearance.option.on_low_dpi", "On low-DPI displays");
        m.insert("settings.appearance.option.on_high_dpi", "On high-DPI displays");
        m.insert("settings.appearance.option.only_named_colors", "Only for named colors");
        m.insert("settings.appearance.option.when_windowed", "When windowed");
        m.insert("settings.appearance.option.only_on_hover", "Only on hover");

        // Appearance page - Input binding descriptions
        m.insert("settings.appearance.input.binding.start_top", "Start Input at the Top");
        m.insert("settings.appearance.input.binding.pin_top", "Pin Input to the Top");
        m.insert("settings.appearance.input.binding.pin_bottom", "Pin Input to the Bottom");
        m.insert("settings.appearance.input.binding.toggle", "Toggle Input Mode (Warp/Classic)");

        // Common buttons
        m.insert("button.reset", "Reset");
        m.insert("button.add", "Add");
        m.insert("button.remove", "Remove");
        m.insert("button.save", "Save");
        m.insert("button.cancel", "Cancel");
        m.insert("button.close", "Close");
        m.insert("button.sign_up", "Sign up");
        m.insert("button.apply", "Apply");

        // Features page
        m.insert("settings.features", "Features");
        m.insert("settings.features.copy_on_select", "Copy on select");

        // Features page - Categories
        m.insert("settings.features.category.general", "General");
        m.insert("settings.features.category.session", "Session");
        m.insert("settings.features.category.keys", "Keys");
        m.insert("settings.features.category.text_editing", "Text Editing");
        m.insert("settings.features.category.terminal_input", "Terminal Input");
        m.insert("settings.features.category.terminal", "Terminal");
        m.insert("settings.features.category.notifications", "Notifications");
        m.insert("settings.features.category.workflows", "Workflows");
        m.insert("settings.features.category.system", "System");

        // Features page - General
        m.insert("settings.features.open_links_in_desktop_app", "Open links in desktop app");
        m.insert("settings.features.open_links_in_desktop_app.description", "Automatically open links in desktop app whenever possible.");
        m.insert("settings.features.restore_on_startup", "Restore windows, tabs, and panes on startup");
        m.insert("settings.features.wayland_positions_warning", "Window positions won't be restored on Wayland. ");
        m.insert("settings.features.see_docs", "See docs.");
        m.insert("settings.features.sticky_command_header", "Show sticky command header");
        m.insert("settings.features.link_tooltip", "Show tooltip on click on links");
        m.insert("settings.features.quit_warning", "Show warning before quitting/logging out");
        m.insert("settings.features.login_item_macos", "Start Warp at login (requires macOS 13+)");
        m.insert("settings.features.login_item", "Start Warp at login");
        m.insert("settings.features.quit_when_all_closed", "Quit when all windows are closed");
        m.insert("settings.features.changelog_after_updates", "Show changelog toast after updates");
        m.insert("settings.features.mouse_scroll_interval", "Lines scrolled by mouse wheel interval");
        m.insert("settings.features.mouse_scroll_interval.description", "Supports floating point values between 1 and 20.");
        m.insert("settings.features.mouse_scroll_interval.allowed_values", "Allowed Values: 1-20");
        m.insert("settings.features.auto_open_code_review", "Auto open code review panel");
        m.insert("settings.features.auto_open_code_review.description", "When this setting is on, the code review panel will open on the first accepted diff of a conversation");
        m.insert("settings.features.warp_is_default_terminal", "Warp is the default terminal");
        m.insert("settings.features.make_default_terminal", "Make Warp the default terminal");
        m.insert("settings.features.max_rows", "Maximum rows in a block");
        m.insert("settings.features.max_rows.description", "Setting the limit above 100k lines may impact performance. Maximum rows supported is {max_rows}.");
        m.insert("settings.features.ssh_wrapper", "Warp SSH Wrapper");
        m.insert("settings.features.new_sessions_effect", "This change will take effect in new sessions");
        m.insert("settings.features.default", "Default");

        // Features page - Notifications
        m.insert("settings.features.desktop_notifications", "Receive desktop notifications from Warp");
        m.insert("settings.features.notify_agent_task_completed", "Notify when an agent completes a task");
        m.insert("settings.features.notify_needs_attention", "Notify when a command or agent needs your attention to continue");
        m.insert("settings.features.notification_sounds", "Play notification sounds");
        m.insert("settings.features.in_app_agent_notifications", "Show in-app agent notifications");
        m.insert("settings.features.toast_duration", "Toast notifications stay visible for");
        m.insert("settings.features.seconds", "seconds");
        m.insert("settings.features.command_longer_than", "When a command takes longer than");
        m.insert("settings.features.seconds_to_complete", "seconds to complete");

        // Features page - Session
        m.insert("settings.features.default_shell", "Default shell for new sessions");
        m.insert("settings.features.working_directory", "Working directory for new sessions");
        m.insert("settings.features.confirm_close_shared", "Confirm before closing shared session");
        m.insert("settings.features.new_tab_placement", "New tab placement");
        m.insert("settings.features.after_all_tabs", "After all tabs");
        m.insert("settings.features.after_current_tab", "After current tab");
        m.insert("settings.features.default_session_mode", "Default mode for new sessions");
        m.insert("settings.features.global_workflows", "Show Global Workflows in Command Search (ctrl-r)");

        // Features page - Keys
        m.insert("settings.features.global_hotkey", "Global hotkey:");
        m.insert("settings.features.configure_global_hotkey", "Configure Global Hotkey");
        m.insert("settings.features.wayland_not_supported", "Not supported on Wayland. ");
        m.insert("settings.features.keybinding", "Keybinding");
        m.insert("settings.features.click_to_set_hotkey", "Click to set global hotkey");
        m.insert("settings.features.press_new_shortcut", "Press new keyboard shortcut");
        m.insert("settings.features.change_keybinding", "Change keybinding");
        m.insert("settings.features.pin_to_top", "Pin to top");
        m.insert("settings.features.pin_to_bottom", "Pin to bottom");
        m.insert("settings.features.pin_to_left", "Pin to left");
        m.insert("settings.features.pin_to_right", "Pin to right");
        m.insert("settings.features.active_screen", "Active Screen");
        m.insert("settings.features.width_percent", "Width %");
        m.insert("settings.features.height_percent", "Height %");
        m.insert("settings.features.autohide_keyboard_focus", "Autohides on loss of keyboard focus");
        m.insert("settings.features.meta_key_left.option", "Left Option key is Meta");
        m.insert("settings.features.meta_key_right.option", "Right Option key is Meta");
        m.insert("settings.features.meta_key_left.alt", "Left Alt key is Meta");
        m.insert("settings.features.meta_key_right.alt", "Right Alt key is Meta");

        // Features page - Text Editing
        m.insert("settings.features.autocomplete_symbols", "Autocomplete quotes, parentheses, and brackets");
        m.insert("settings.features.error_underlining", "Error underlining for commands");
        m.insert("settings.features.syntax_highlighting", "Syntax highlighting for commands");
        m.insert("settings.features.completions_while_typing", "Open completions menu as you type");
        m.insert("settings.features.suggest_corrections", "Suggest corrected commands");
        m.insert("settings.features.expand_aliases", "Expand aliases as you type");
        m.insert("settings.features.middle_click_paste", "Middle-click to paste");
        m.insert("settings.features.vim_mode", "Edit code and commands with Vim keybindings");
        m.insert("settings.features.vim_unnamed_clipboard", "Set unnamed register as system clipboard");
        m.insert("settings.features.vim_status_bar", "Show Vim status bar");

        // Features page - Terminal Input
        m.insert("settings.features.at_context_menu", "Enable '@' context menu in terminal mode");
        m.insert("settings.features.slash_commands", "Enable slash commands in terminal mode");
        m.insert("settings.features.outline_codebase_symbols", "Outline codebase symbols for '@' context menu");
        m.insert("settings.features.terminal_input_message", "Show terminal input message line");
        m.insert("settings.features.autosuggestion_keybinding_hint", "Show autosuggestion keybinding hint");
        m.insert("settings.features.autosuggestion_ignore_button", "Show autosuggestion ignore button");
        m.insert("settings.features.tab_key_behavior", "Tab key behavior");
        m.insert("settings.features.ctrl_tab_behavior", "Ctrl+Tab behavior:");
        m.insert("settings.features.arrow_accepts_autosuggestions", "\u{2192} accepts autosuggestions.");
        m.insert("settings.features.keystroke_accepts_autosuggestions", "{keystroke} accepts autosuggestions.");
        m.insert("settings.features.completions_open_as_you_type", "Completions open as you type.");
        m.insert("settings.features.completions_open_as_you_type_or", "Completions open as you type (or {keystroke}).");
        m.insert("settings.features.completion_menu_unbound", "Opening the completion menu is unbound.");
        m.insert("settings.features.keystroke_opens_completion_menu", "{keystroke} opens completion menu.");
        m.insert("settings.features.accept_autosuggestion", "Accept Autosuggestion");
        m.insert("settings.features.open_completions_menu", "Open Completions Menu");
        m.insert("settings.features.word_char_config", "Characters considered part of a word");

        // Features page - Terminal
        m.insert("settings.features.mouse_reporting", "Enable Mouse Reporting");
        m.insert("settings.features.scroll_reporting", "Enable Scroll Reporting");
        m.insert("settings.features.focus_reporting", "Enable Focus Reporting");
        m.insert("settings.features.audible_bell", "Use Audible Bell");
        m.insert("settings.features.smart_selection", "Double-click smart selection");
        m.insert("settings.features.show_help_block", "Show help block in new sessions");
        m.insert("settings.features.linux_selection_clipboard", "Honor linux selection clipboard");
        m.insert("settings.features.linux_selection_clipboard.description", "Whether the Linux primary clipboard should be supported.");

        // Features page - System
        m.insert("settings.features.prefer_low_power_gpu", "Prefer rendering new windows with integrated GPU (low power)");
        m.insert("settings.features.changes_new_windows", "Changes will apply to new windows.");
        m.insert("settings.features.wayland_window_management", "Use Wayland for window management");
        m.insert("settings.features.wayland_window_management.description", "Enables the use of Wayland");
        m.insert("settings.features.wayland_hotkey_warning", "Enabling this setting disables global hotkey support. When disabled, text may be blurry if your Wayland compositor is using fraction scaling (ex: 125%).");
        m.insert("settings.features.restart_warp_effect", "Restart Warp for changes to take effect.");
        m.insert("settings.features.preferred_graphics_backend", "Preferred graphics backend");
        m.insert("settings.features.current_backend", "Current backend: {backend}");

        // AI page
        m.insert("settings.ai", "AI");
        m.insert("settings.ai.warp_agent", "Warp Agent");
        m.insert("settings.ai.active_ai", "Active AI");
        m.insert("settings.ai.usage", "Usage");
        m.insert("settings.ai.credits", "Credits");
        m.insert("settings.ai.unlimited", "Unlimited");
        m.insert("settings.ai.restricted_billing", "Restricted due to billing issue");
        m.insert("settings.ai.resets", "Resets {formatted_next_refresh_time}");
        m.insert("settings.ai.credits_limit_description", "This is the {0} limit of AI credits for your account.");
        m.insert("settings.ai.upgrade", "Upgrade");
        m.insert("settings.ai.get_more_usage", " to get more AI usage.");
        m.insert("settings.ai.compare_plans", "Compare plans");
        m.insert("settings.ai.more_usage", " for more AI usage.");
        m.insert("settings.ai.contact_support", "Contact support");
        m.insert("settings.ai.contact_sales", "Contact sales");
        m.insert("settings.ai.enable_byo_enterprise", " to enable bringing your own API keys on your Enterprise plan.");
        m.insert("settings.ai.upgrade_build_plan", "Upgrade to the Build plan");
        m.insert("settings.ai.use_own_api_keys", " to use your own API keys.");
        m.insert("settings.ai.ask_admin_upgrade", "Ask your team's admin to upgrade to the Build plan to use your own API keys.");
        m.insert("settings.ai.org_disallows_remote_ai", "Your organization disallows AI when the active pane contains content from a remote session");
        m.insert("settings.ai.create_account_prompt", "To use AI features, please create an account.");
        m.insert("settings.ai.org_enforced_setting", "This option is enforced by your organization's settings and cannot be customized.");
        m.insert("settings.ai.learn_more", "Learn more");

        // AI page - Agents section
        m.insert("settings.ai.agents", "Agents");
        m.insert("settings.ai.agents_description", "Set the boundaries for how your Agent operates. Choose what it can access, how much autonomy it has, and when it must ask for your approval. You can also fine-tune behavior around natural language input, codebase awareness, and more.");
        m.insert("settings.ai.profiles", "Profiles");
        m.insert("settings.ai.profiles_description", "Profiles let you define how your Agent operates \u{2014} from the actions it can take and when it needs approval, to the models it uses for tasks like coding and planning. You can also scope them to individual projects.");
        m.insert("settings.ai.add_profile", "Add Profile");
        m.insert("settings.ai.models", "Models");
        m.insert("settings.ai.permissions", "Permissions");

        // AI page - Permissions
        m.insert("settings.ai.apply_code_diffs", "Apply code diffs");
        m.insert("settings.ai.read_files", "Read files");
        m.insert("settings.ai.execute_commands", "Execute commands");
        m.insert("settings.ai.interact_with_running_commands", "Interact with running commands");
        m.insert("settings.ai.workspace_managed_permissions", "Some of your permissions are managed by your workspace.");
        m.insert("settings.ai.call_mcp_servers", "Call MCP servers");
        m.insert("settings.ai.command_denylist", "Command denylist");
        m.insert("settings.ai.command_denylist.description", "Regular expressions to match commands that the Warp Agent should always ask permission to execute.");
        m.insert("settings.ai.command_allowlist", "Command allowlist");
        m.insert("settings.ai.command_allowlist.description", "Regular expressions to match commands that can be automatically executed by the Warp Agent.");
        m.insert("settings.ai.directory_allowlist", "Directory allowlist");
        m.insert("settings.ai.directory_allowlist.description", "Give the agent file access to certain directories.");
        m.insert("settings.ai.mcp_allowlist", "MCP allowlist");
        m.insert("settings.ai.mcp_allowlist.description", "Allow the Warp Agent to call these MCP servers.");
        m.insert("settings.ai.mcp_denylist", "MCP denylist");
        m.insert("settings.ai.mcp_denylist.description", "The Warp Agent will always ask for permission before calling any MCP servers on this list.");
        m.insert("settings.ai.mcp_zero_state_description", "You haven't added any MCP servers yet. Once you do, you'll be able to control how much autonomy the Warp Agent has when interacting with them. ");
        m.insert("settings.ai.add_a_server", "Add a server");
        m.insert("settings.ai.or", " or ");
        m.insert("settings.ai.learn_more_mcps", "learn more about MCPs.");
        m.insert("settings.ai.show_model_picker_in_prompt", "Show model picker in prompt");
        m.insert("settings.ai.base_model", "Base model");
        m.insert("settings.ai.base_model.description", "This model serves as the primary engine behind the Warp Agent. It powers most interactions and invokes other models for tasks like planning or code generation when necessary. Warp may automatically switch to alternate models based on model availability or for auxiliary tasks such as conversation summarization.");
        m.insert("settings.ai.codebase_context", "Codebase Context");
        m.insert("settings.ai.codebase_context.description", "Allow the Warp Agent to generate an outline of your codebase that can be used for context. No code is ever stored on our servers. ");
        m.insert("settings.ai.toolbar_layout", "Toolbar layout");

        // AI page - Permission options
        m.insert("settings.ai.option.agent_decides", "Agent decides");
        m.insert("settings.ai.option.always_allow", "Always allow");
        m.insert("settings.ai.option.always_ask", "Always ask");
        m.insert("settings.ai.option.ask_on_first_write", "Ask on first write");
        m.insert("settings.ai.option.read_only", "Read only");
        m.insert("settings.ai.option.supervised", "Supervised");
        m.insert("settings.ai.option.allow_in_specific_directories", "Allow in specific directories");
        m.insert("settings.ai.option.new_tab", "New Tab");
        m.insert("settings.ai.option.split_pane", "Split Pane");
        m.insert("settings.ai.select_mcp_servers", "Select MCP servers");
        m.insert("settings.ai.select_coding_agent", "Select coding agent");

        // AI page - Active AI section
        m.insert("settings.ai.next_command", "Next Command");
        m.insert("settings.ai.next_command.description", "Let AI suggest the next command to run based on your command history, outputs, and common workflows.");
        m.insert("settings.ai.prompt_suggestions", "Prompt Suggestions");
        m.insert("settings.ai.prompt_suggestions.description", "Let AI suggest natural language prompts, as inline banners in the input, based on recent commands and their outputs.");
        m.insert("settings.ai.suggested_code_banners", "Suggested Code Banners");
        m.insert("settings.ai.suggested_code_banners.description", "Let AI suggest code diffs and queries as inline banners in the blocklist, based on recent commands and their outputs.");
        m.insert("settings.ai.natural_language_autosuggestions", "Natural Language Autosuggestions");
        m.insert("settings.ai.natural_language_autosuggestions.description", "Let AI suggest natural language autosuggestions, based on recent commands and their outputs.");
        m.insert("settings.ai.shared_block_title_generation", "Shared Block Title Generation");
        m.insert("settings.ai.shared_block_title_generation.description", "Let AI generate a title for your shared block based on the command and output.");
        m.insert("settings.ai.commit_pull_request_generation", "Commit & Pull Request Generation");
        m.insert("settings.ai.git_operations_autogen.description", "Let AI generate commit messages and pull request titles and descriptions.");

        // AI page - Input section
        m.insert("settings.ai.input", "Input");
        m.insert("settings.ai.show_input_hint_text", "Show input hint text");
        m.insert("settings.ai.show_agent_tips", "Show agent tips");
        m.insert("settings.ai.include_agent_commands_in_history", "Include agent-executed commands in history");
        m.insert("settings.ai.natural_language_detection", "Natural language detection");
        m.insert("settings.ai.autodetect_agent_prompts", "Autodetect agent prompts in terminal input");
        m.insert("settings.ai.autodetect_terminal_commands", "Autodetect terminal commands in agent input");
        m.insert("settings.ai.incorrect_detection", "Encountered an incorrect detection? ");
        m.insert("settings.ai.incorrect_input_detection", " Encountered an incorrect input detection? ");
        m.insert("settings.ai.let_us_know", "Let us know");
        m.insert("settings.ai.nld_description", "Enabling natural language detection will detect when natural language is written in the terminal input, and then automatically switch to Agent Mode for AI queries.");
        m.insert("settings.ai.natural_language_denylist", "Natural language denylist");
        m.insert("settings.ai.natural_language_denylist.description", "Commands listed here will never trigger natural language detection.");

        // AI page - MCP Servers section
        m.insert("settings.ai.mcp_servers", "MCP Servers");
        m.insert("settings.ai.mcp_description", "Add MCP servers to extend the Warp Agent's capabilities. MCP servers expose data sources or tools to agents through a standardized interface, essentially acting like plugins. ");
        m.insert("settings.ai.auto_spawn_mcp_servers", "Auto-spawn servers from third-party agents");
        m.insert("settings.ai.auto_spawn_mcp.description", "Automatically detect and spawn MCP servers from globally-scoped third-party AI agent configuration files (e.g. in your home directory). Servers detected inside a repository are never spawned automatically and must be enabled individually from the MCP settings page. ");
        m.insert("settings.ai.see_supported_providers", "See supported providers.");
        m.insert("settings.ai.manage_mcp_servers", "Manage MCP servers");

        // AI page - Knowledge section
        m.insert("settings.ai.knowledge", "Knowledge");
        m.insert("settings.ai.rules", "Rules");
        m.insert("settings.ai.rules.description", "Rules help the Warp Agent follow your conventions, whether for codebases or specific workflows. ");
        m.insert("settings.ai.suggested_rules", "Suggested Rules");
        m.insert("settings.ai.suggested_rules.description", "Let AI suggest rules to save based on your interactions.");
        m.insert("settings.ai.manage_rules", "Manage rules");
        m.insert("settings.ai.warp_drive_as_agent_context", "Warp Drive as agent context");
        m.insert("settings.ai.warp_drive_context.description", "The Warp Agent can leverage your Warp Drive Contents to tailor responses to your personal and team developer workflows and environments. This includes any Workflows, Notebooks, and Environment Variables.");

        // AI page - Voice section
        m.insert("settings.ai.voice", "Voice");
        m.insert("settings.ai.voice_input", "Voice Input");
        m.insert("settings.ai.voice_input.description", "Voice input allows you to control Warp by speaking directly to your terminal (powered by ");
        m.insert("settings.ai.voice_input_key", "Key for Activating Voice Input");
        m.insert("settings.ai.voice_input.press_hold", "Press and hold to activate.");

        // AI page - Other section
        m.insert("settings.ai.other", "Other");
        m.insert("settings.ai.show_oz_changelog", "Show Oz changelog in new conversation view");
        m.insert("settings.ai.show_use_agent_footer", "Show \"Use Agent\" footer");
        m.insert("settings.ai.use_agent_footer.description", "Shows hint to use the \"Full Terminal Use\"-enabled agent in long running commands.");
        m.insert("settings.ai.show_conversation_history", "Show conversation history in tools panel");
        m.insert("settings.ai.agent_thinking_display", "Agent thinking display");
        m.insert("settings.ai.thinking_display.description", "Controls how reasoning/thinking traces are displayed.");
        m.insert("settings.ai.preferred_conversation_layout", "Preferred layout when opening existing agent conversations");

        // AI page - Third party CLI agents section
        m.insert("settings.ai.third_party_cli_agents", "Third party CLI agents");
        m.insert("settings.ai.show_coding_agent_toolbar", "Show coding agent toolbar");
        m.insert("settings.ai.coding_agent_toolbar.description", "Show a toolbar with quick actions when running coding agents like ");
        m.insert("settings.ai.auto_toggle_rich_input", "Auto show/hide Rich Input based on agent status");
        m.insert("settings.ai.requires_warp_plugin", "Requires the Warp plugin for your coding agent");
        m.insert("settings.ai.auto_open_rich_input", "Auto open Rich Input when a coding agent session starts");
        m.insert("settings.ai.auto_dismiss_rich_input", "Auto dismiss Rich Input after prompt submission");
        m.insert("settings.ai.commands_enable_toolbar", "Commands that enable the toolbar");
        m.insert("settings.ai.toolbar_command_patterns.description", "Add regex patterns to show the coding agent toolbar for matching commands.");

        // AI page - Agent Attribution section
        m.insert("settings.ai.agent_attribution", "Agent Attribution");
        m.insert("settings.ai.enable_agent_attribution", "Enable agent attribution");
        m.insert("settings.ai.agent_attribution.description", "Oz can add attribution to commit messages and pull requests it creates");

        // AI page - Experimental section
        m.insert("settings.ai.experimental", "Experimental");
        m.insert("settings.ai.computer_use_cloud_agents", "Computer use in Cloud Agents");
        m.insert("settings.ai.computer_use.description", "Enable computer use in cloud agent conversations started from the Warp app.");
        m.insert("settings.ai.orchestration", "Orchestration");
        m.insert("settings.ai.orchestration.description", "Enable multi-agent orchestration, allowing the agent to spawn and coordinate parallel sub-agents.");

        // AI page - API Keys section
        m.insert("settings.ai.api_keys", "API Keys");
        m.insert("settings.ai.api_keys.description", "Use your own API keys from model providers for the Warp Agent to use. API keys are stored locally and never synced to the cloud. Using auto models or models from providers you have not provided API keys for will consume Warp credits.");
        m.insert("settings.ai.warp_credit_fallback", "Warp credit fallback");
        m.insert("settings.ai.warp_credit_fallback.description", "When enabled, agent requests may be routed to one of Warp's provided models in the event of an error. Warp will prioritize using your API keys over your Warp credits.");

        // AI page - AWS Bedrock section
        m.insert("settings.ai.aws_bedrock", "AWS Bedrock");
        m.insert("settings.ai.use_aws_bedrock_credentials", "Use AWS Bedrock credentials");
        m.insert("settings.ai.aws_bedrock_credentials.description", "Warp loads and sends local AWS CLI credentials for Bedrock-supported models.");
        m.insert("settings.ai.aws_bedrock_credentials.admin_description", "Warp loads and sends local AWS CLI credentials for Bedrock-supported models. This setting is managed by your organization.");
        m.insert("settings.ai.login_command", "Login Command");
        m.insert("settings.ai.aws_profile", "AWS Profile");
        m.insert("settings.ai.auto_run_login_command", "Automatically run login command");
        m.insert("settings.ai.auto_login.description", "When enabled, the login command will run automatically when AWS Bedrock credentials expire.");

        // AI page - Placeholder hints
        m.insert("settings.ai.placeholder.code_repo", "e.g. ~/code-repos/repo");
        m.insert("settings.ai.placeholder.commands_comma", "Commands, comma separated");
        m.insert("settings.ai.placeholder.regex_ls", "e.g. ls .*");
        m.insert("settings.ai.placeholder.regex_rm", "e.g. rm .*");
        m.insert("settings.ai.placeholder.command_regex", "command (supports regex)");

        // Code page - Indexing (extended)
        m.insert("settings.code.indexing.init_settings", "Initialization Settings");
        m.insert("settings.code.indexing.description", "Warp can automatically index code repositories for AI-powered features like codebase search and context-aware suggestions.");
        m.insert("settings.code.indexing.exclude_description", "To exclude specific files or directories from indexing, add them to your .warpindexignore file.");
        m.insert("settings.code.indexing.index_new_folder.description", "When set to true, Warp will automatically index newly discovered folders.");
        m.insert("settings.code.indexing.disabled_by_admin", "Team admins have disabled codebase indexing.");
        m.insert("settings.code.indexing.enabled_by_admin", "Team admins have enabled codebase indexing.");
        m.insert("settings.code.indexing.ai_required", "AI Features must be enabled to use codebase indexing.");
        m.insert("settings.code.indexing.max_indices", "You have reached the maximum number of codebase indices. Please remove an existing index before adding a new one.");
        m.insert("settings.code.indexing.initialized_folders", "Initialized / indexed folders");
        m.insert("settings.code.indexing.no_folders", "No folders have been initialized yet.");
        m.insert("settings.code.indexing.open_project_rules", "Open project rules");
        m.insert("settings.code.indexing.status_label", "INDEXING");
        m.insert("settings.code.indexing.no_index", "No index created");
        m.insert("settings.code.indexing.discovered_chunks", "Discovered {total_nodes} chunks");
        m.insert("settings.code.indexing.syncing_progress", "Syncing - {completed_nodes} / {total_nodes}");
        m.insert("settings.code.indexing.syncing", "Syncing...");
        m.insert("settings.code.indexing.synced", "Synced");
        m.insert("settings.code.indexing.too_large", "Codebase too large");
        m.insert("settings.code.indexing.stale", "Stale");
        m.insert("settings.code.indexing.failed", "Failed");
        m.insert("settings.code.indexing.no_index_built", "No index built");

        // Code page - Indexing
        m.insert("settings.code.indexing.title", "Codebase Indexing");
        m.insert("settings.code.indexing.index_new_folder", "Index new folder");

        // Code page - LSP Servers
        m.insert("settings.code.lsp.title", "LSP SERVERS");
        m.insert("settings.code.lsp.installed", "Installed");
        m.insert("settings.code.lsp.installing", "Installing...");
        m.insert("settings.code.lsp.checking", "Checking...");
        m.insert("settings.code.lsp.available_download", "Available for download");
        m.insert("settings.code.lsp.restart", "Restart server");
        m.insert("settings.code.lsp.view_logs", "View logs");
        m.insert("settings.code.lsp.status_available", "Available");
        m.insert("settings.code.lsp.status_busy", "Busy");
        m.insert("settings.code.lsp.status_failed", "Failed");
        m.insert("settings.code.lsp.status_stopped", "Stopped");
        m.insert("settings.code.lsp.status_not_running", "Not running");
        m.insert("settings.code.editor.title", "Editor and Code Review");
        m.insert("settings.code.editor.category", "Code Editor and Review");
        m.insert("settings.code.editor.default_app", "Default App");
        m.insert("settings.code.editor.layout.split_pane", "Split Pane");
        m.insert("settings.code.editor.layout.new_tab", "New Tab");
        m.insert("settings.code.editor.open_file_links", "Choose an editor to open file links");
        m.insert("settings.code.editor.open_code_panel_files", "Choose an editor to open files from the code review panel, project explorer, and global search");
        m.insert("settings.code.editor.open_files_layout", "Choose a layout to open files in Warp");
        m.insert("settings.code.editor.group_files", "Group files into single editor pane");
        m.insert("settings.code.editor.group_files.description", "When this setting is on, any files opened in the same tab will be automatically grouped into a single editor pane.");
        m.insert("settings.code.editor.open_markdown_in_viewer", "Open Markdown files in Warp's Markdown Viewer by default");
        m.insert("settings.code.editor.auto_open_code_review_panel", "Auto open code review panel");
        m.insert("settings.code.editor.auto_open_code_review_panel.description", "When this setting is on, the code review panel will open on the first accepted diff of a conversation");
        m.insert("settings.code.editor.show_code_review_button", "Show code review button");
        m.insert("settings.code.editor.show_code_review_button.description", "Show a button in the top right of the window to toggle the code review panel.");
        m.insert("settings.code.editor.show_diff_stats", "Show diff stats on code review button");
        m.insert("settings.code.editor.show_diff_stats.description", "Show lines added and removed counts on the code review button.");
        m.insert("settings.code.editor.project_explorer", "Project explorer");
        m.insert("settings.code.editor.project_explorer.description", "Adds an IDE-style project explorer / file tree to the left side tools panel.");
        m.insert("settings.code.editor.global_file_search", "Global file search");
        m.insert("settings.code.editor.global_file_search.description", "Adds global file search to the left side tools panel.");

        // Keybindings page
        m.insert("settings.keybindings", "Keyboard shortcuts");
        m.insert("settings.keybindings.search_placeholder", "Search by name or by keys (ex. \"cmd d\")");
        m.insert("settings.keybindings.conflict_warning", "This shortcut conflicts with other keybinds");
        m.insert("settings.keybindings.default", "Default");
        m.insert("settings.keybindings.clear", "Clear");
        m.insert("settings.keybindings.press_new_shortcut", "Press new keyboard shortcut");
        m.insert("settings.keybindings.add_custom", "Add your own custom keybindings to existing actions below.");
        m.insert("settings.keybindings.use", "Use");
        m.insert("settings.keybindings.not_synced", "Keyboard shortcuts are not synced to the cloud");
        m.insert("settings.keybindings.configure", "Configure keyboard shortcuts");
        m.insert("settings.keybindings.command", "Command");

        // Privacy page
        m.insert("settings.privacy", "Privacy");
        m.insert("settings.privacy.safe_mode", "Secret redaction");
        m.insert("settings.privacy.safe_mode.description", "When this setting is enabled, Warp will scan blocks, the contents of Warp Drive objects, and Oz prompts for potential sensitive information and prevent saving or sending this data to any servers. You can customize this list via regexes.");
        m.insert("settings.privacy.user_secret_regex", "Custom secret redaction");
        m.insert("settings.privacy.user_secret_regex.description", "Use regex to define additional secrets or data you'd like to redact. This will take effect when the next command runs. You can use the inline (?i) flag as a prefix to your regex to make it case-insensitive.");
        m.insert("settings.privacy.telemetry", "Help improve Warp");
        m.insert("settings.privacy.telemetry.description", "App analytics help us make the product better for you. We may collect certain console interactions to improve Warp's AI capabilities.");
        m.insert("settings.privacy.telemetry.description_old", "App analytics help us make the product better for you. We only collect app usage metadata, never console input or output.");
        m.insert("settings.privacy.telemetry.free_tier_note", "On the free tier, analytics must be enabled to use AI features.");
        m.insert("settings.privacy.telemetry.read_more", "Read more about Warp's use of data");
        m.insert("settings.privacy.data_management", "Manage your data");
        m.insert("settings.privacy.data_management.description", "At any time, you may choose to delete your Warp account permanently. You will no longer be able to use Warp.");
        m.insert("settings.privacy.data_management.link", "Visit the data management page");
        m.insert("settings.privacy.privacy_policy", "Privacy policy");
        m.insert("settings.privacy.privacy_policy.link", "Read Warp's privacy policy");
        m.insert("settings.privacy.personal", "Personal");
        m.insert("settings.privacy.enterprise", "Enterprise");
        m.insert("settings.privacy.enterprise.cannot_modify", "Enterprise secret redaction cannot be modified.");
        m.insert("settings.privacy.enterprise.no_regexes", "No enterprise regexes have been configured by your organization.");
        m.insert("settings.privacy.managed_by_org", "Enabled by your organization.");
        m.insert("settings.privacy.managed_by_org.tooltip", "This setting is managed by your organization.");
        m.insert("settings.privacy.secret_visual_mode", "Secret visual redaction mode");
        m.insert("settings.privacy.secret_visual_mode.description", "Choose how secrets are visually presented in the block list while keeping them searchable. This setting only affects what you see in the block list.");
        m.insert("settings.privacy.recommended", "Recommended");
        m.insert("settings.privacy.add_all", "Add all");
        m.insert("settings.privacy.add_regex", "Add regex");
        m.insert("settings.privacy.add_regex_pattern", "Add regex pattern");
        m.insert("settings.privacy.send_crash_reports", "Send crash reports");
        m.insert("settings.privacy.send_crash_reports.description", "Crash reports assist with debugging and stability improvements.");
        m.insert("settings.privacy.store_ai_conversations", "Store AI conversations in the cloud");
        m.insert("settings.privacy.store_ai_conversations.enabled_description", "Agent conversations can be shared with others and are retained when you log in on different devices. This data is only stored for product functionality, and Warp will not use it for analytics.");
        m.insert("settings.privacy.store_ai_conversations.disabled_description", "Agent conversations are only stored locally on your machine, are lost upon logout, and cannot be shared. Note: conversation data for ambient agents are still stored in the cloud.");
        m.insert("settings.privacy.network_log_console", "Network log console");
        m.insert("settings.privacy.network_log_console.description", "We've built a native console that allows you to view all communications from Warp to external servers to ensure you feel comfortable that your work is always kept safe.");
        m.insert("settings.privacy.network_log_console.link", "View network logging");
        m.insert("settings.privacy.zero_data_retention", "Your administrator has enabled zero data retention for your team. User generated content will never be collected.");

        // About page
        m.insert("settings.about", "About");

        // Environments page
        m.insert("settings.environments.title", "Environments");
        m.insert("settings.environments.description", "Environments define where your ambient agents run. Set one up in minutes via GitHub (recommended), Warp-assisted setup, or manual configuration.");
        m.insert("settings.environments.search_placeholder", "Search environments...");
        m.insert("settings.environments.no_matches", "No environments match your search.");
        m.insert("settings.environments.section.personal", "Personal");
        m.insert("settings.environments.section.shared_by_team", "Shared by Warp and {team_name}");
        m.insert("settings.environments.section.shared_by_default", "Shared by Warp and your team");
        m.insert("settings.environments.empty.loading", "Loading...");
        m.insert("settings.environments.empty.retry", "Retry");
        m.insert("settings.environments.empty.authorize", "Authorize");
        m.insert("settings.environments.empty.get_started", "Get started");
        m.insert("settings.environments.empty.launch_agent", "Launch agent");
        m.insert("settings.environments.empty.quick_setup", "Quick setup");
        m.insert("settings.environments.empty.suggested", "Suggested");
        m.insert("settings.environments.empty.github_subtitle", "Select the GitHub repositories you\u{2019}d like to work with and we\u{2019}ll suggest a base image and config");
        m.insert("settings.environments.empty.use_agent", "Use the agent");
        m.insert("settings.environments.empty.agent_subtitle", "Choose a locally set up project and we\u{2019}ll help you set up an environment based on it");
        m.insert("settings.environments.empty.no_envs_header", "You haven\u{2019}t set up any environments yet.");
        m.insert("settings.environments.empty.no_envs_subheader", "Choose how you\u{2019}d like to set up your environment:");
        m.insert("settings.environments.card.env_id", "Env ID: {env_id}");
        m.insert("settings.environments.card.image", "Image: {image}");
        m.insert("settings.environments.card.repos", "Repos: {repos}");
        m.insert("settings.environments.card.setup_commands", "Setup commands: {commands}");
        m.insert("settings.environments.card.view_runs", "View my runs");
        m.insert("settings.environments.card.share", "Share");
        m.insert("settings.environments.card.edit", "Edit");
        m.insert("settings.environments.timestamp.last_edited", "Last edited: {duration}");
        m.insert("settings.environments.timestamp.last_used", "Last used: {duration}");
        m.insert("settings.environments.timestamp.last_used_never", "Last used: never");
        m.insert("settings.environments.toast.updated", "Successfully updated environment");
        m.insert("settings.environments.toast.created", "Successfully created environment");
        m.insert("settings.environments.toast.deleted", "Environment deleted successfully");
        m.insert("settings.environments.toast.shared", "Successfully shared environment");
        m.insert("settings.environments.toast.share_failed", "Failed to share environment with team");
        m.insert("settings.environments.toast.create_not_logged_in", "Unable to create environment: not logged in.");
        m.insert("settings.environments.toast.save_not_found", "Unable to save: environment no longer exists.");
        m.insert("settings.environments.toast.share_no_team", "Unable to share environment: you are not currently on a team.");
        m.insert("settings.environments.toast.share_not_synced", "Unable to share environment: environment is not yet synced.");

        // Warp Drive page
        m.insert("settings.warp_drive.create_account_prompt", "To use Warp Drive, please create an account.");
        m.insert("settings.warp_drive.title", "Warp Drive");
        m.insert("settings.warp_drive.description", "Warp Drive is a workspace in your terminal where you can save Workflows, Notebooks, Prompts, and Environment Variables for personal use or to share with a team.");

        // Settings footer
        m.insert("settings.footer.open_settings_file", "Open settings file");
        m.insert("settings.footer.open_file", "Open file");
        m.insert("settings.footer.fix_with_oz", "Fix with Oz");

        // Transfer ownership confirmation
        m.insert("settings.transfer.confirm_message", "Are you sure you want to transfer team ownership to {email}? This action cannot be undone. You will lose admin privileges.");
        m.insert("settings.transfer.button", "Transfer");

        // Main page (Account)
        m.insert("settings.main.referral_cta", "Earn rewards by sharing Warp with friends & colleagues");
        m.insert("settings.main.log_out", "Log out");
        m.insert("settings.main.free_plan", "Free");
        m.insert("settings.main.compare_plans", "Compare plans");
        m.insert("settings.main.contact_support", "Contact support");
        m.insert("settings.main.manage_billing", "Manage billing");
        m.insert("settings.main.upgrade_turbo", "Upgrade to Turbo plan");
        m.insert("settings.main.upgrade_lightspeed", "Upgrade to Lightspeed plan");
        m.insert("settings.main.refer_friend", "Refer a friend");
        m.insert("settings.main.version", "Version");
        m.insert("settings.main.up_to_date", "Up to date");
        m.insert("settings.main.check_updates", "Check for updates");
        m.insert("settings.main.checking_update", "checking for update...");
        m.insert("settings.main.downloading_update", "downloading update...");
        m.insert("settings.main.update_available", "Update available");
        m.insert("settings.main.relaunch_warp", "Relaunch Warp");
        m.insert("settings.main.updating", "Updating...");
        m.insert("settings.main.installed_update", "Installed update");
        m.insert("settings.main.update_unavailable", "A new version of Warp is available but can't be installed");
        m.insert("settings.main.update_manually", "Update Warp manually");
        m.insert("settings.main.update_launch_error", "A new version of Warp is installed but can't be launched.");
        m.insert("settings.main.settings_sync", "Settings sync");

        // Directory color picker
        m.insert("settings.dir_color.add_directory", "+ Add directory\u{2026}");
        m.insert("settings.dir_color.add_button", "Add directory color");

        // Show blocks view (Shared blocks)
        m.insert("settings.show_blocks.unshare_confirm", "Are you sure you want to unshare this block?\n\nIt will no longer be accessible by link and will be permanently deleted from Warp servers.");
        m.insert("settings.show_blocks.no_shared_blocks", "You don't have any shared blocks yet.");
        m.insert("settings.show_blocks.getting_blocks", "Getting blocks...");
        m.insert("settings.show_blocks.load_failed", "Failed to load blocks. Please try again.");
        m.insert("settings.show_blocks.executed_on", "Executed on: {timestamp}");
        m.insert("settings.show_blocks.link_copied", "Link copied.");
        m.insert("settings.show_blocks.unshare_success", "Block was successfully unshared.");
        m.insert("settings.show_blocks.unshare_failed", "Failed to unshare block. Please try again.");
        m.insert("settings.show_blocks.unshare_title", "Unshare block");
        m.insert("settings.show_blocks.deleting", "Deleting...");
        m.insert("settings.show_blocks.copy_link", "Copy link");

        // Execution profile view
        m.insert("settings.execution_profile.edit", "Edit");
        m.insert("settings.execution_profile.models", "MODELS");
        m.insert("settings.execution_profile.base_model", "Base model:");
        m.insert("settings.execution_profile.full_terminal_use", "Full terminal use:");
        m.insert("settings.execution_profile.computer_use", "Computer use:");
        m.insert("settings.execution_profile.permissions", "PERMISSIONS");
        m.insert("settings.execution_profile.apply_code_diffs", "Apply code diffs:");
        m.insert("settings.execution_profile.read_files", "Read files:");
        m.insert("settings.execution_profile.execute_commands", "Execute commands:");
        m.insert("settings.execution_profile.interact_running", "Interact with running commands:");
        m.insert("settings.execution_profile.ask_questions", "Ask questions:");
        m.insert("settings.execution_profile.call_mcp_servers", "Call MCP servers:");
        m.insert("settings.execution_profile.call_web_tools", "Call web tools:");
        m.insert("settings.execution_profile.auto_sync_plans", "Auto-sync plans to Warp Drive:");
        m.insert("settings.execution_profile.directory_allowlist", "Directory allowlist:");
        m.insert("settings.execution_profile.command_allowlist", "Command allowlist:");
        m.insert("settings.execution_profile.command_denylist", "Command denylist:");
        m.insert("settings.execution_profile.mcp_allowlist", "MCP allowlist:");
        m.insert("settings.execution_profile.mcp_denylist", "MCP denylist:");
        m.insert("settings.execution_profile.none", "None");
        m.insert("settings.execution_profile.agent_decides", "Agent decides");
        m.insert("settings.execution_profile.always_allow", "Always allow");
        m.insert("settings.execution_profile.always_ask", "Always ask");
        m.insert("settings.execution_profile.unknown", "Unknown");
        m.insert("settings.execution_profile.ask_on_first_write", "Ask on first write");
        m.insert("settings.execution_profile.never", "Never");
        m.insert("settings.execution_profile.never_ask", "Never ask");
        m.insert("settings.execution_profile.ask_unless_auto_approve", "Ask unless auto-approve");
        m.insert("settings.execution_profile.on", "On");
        m.insert("settings.execution_profile.off", "Off");

        // Delete environment confirmation dialog
        m.insert("settings.delete_env.title", "Delete environment?");
        m.insert("settings.delete_env.description", "Are you sure you want to remove the {env_name} environment?");
        m.insert("settings.delete_env.confirm", "Delete environment");

        // Environment form
        m.insert("settings.env_form.create", "Create");
        m.insert("settings.env_form.save", "Save");
        m.insert("settings.env_form.delete_env", "Delete environment");
        m.insert("settings.env_form.create_env", "Create environment");
        m.insert("settings.env_form.edit_env", "Edit environment");
        m.insert("settings.env_form.save_env", "Save environment");
        m.insert("settings.env_form.share_with_team", "Share with team");
        m.insert("settings.env_form.name_placeholder", "Environment name");
        m.insert("settings.env_form.description_label", "Description");
        m.insert("settings.env_form.char_count", "{count} / {max} characters");
        m.insert("settings.env_form.repos_label", "Repo(s)");
        m.insert("settings.env_form.docker_image_label", "Docker image reference");
        m.insert("settings.env_form.suggest_image", "Suggest image");
        m.insert("settings.env_form.launch_agent", "Launch agent");
        m.insert("settings.env_form.authenticate", "Authenticate");
        m.insert("settings.env_form.auth_github", "Auth with GitHub");
        m.insert("settings.env_form.retry", "Retry");
        m.insert("settings.env_form.no_repos_found", "No repositories found");
        m.insert("settings.env_form.configure_github", "Configure access on GitHub");
        m.insert("settings.env_form.grant_access_hint", "You need to grant access to your GitHub repos to suggest a Docker image");
        m.insert("settings.env_form.share_warning", "Personal environments cannot be used with external integrations or team API keys. For the best experience, use shared environments.");

        // Environment modal
        m.insert("settings.env_modal.title", "Select repos for your environment");
        m.insert("settings.env_modal.description.indexed", "Select locally indexed repos to provide context for the environment creation agent.");
        m.insert("settings.env_modal.description.default", "Select repos to provide context for the environment creation agent.");
        m.insert("settings.env_modal.section.selected_repos", "Selected repos");
        m.insert("settings.env_modal.section.available_repos", "Available indexed repos");
        m.insert("settings.env_modal.empty.no_selected", "No repos selected yet");
        m.insert("settings.env_modal.empty.all_selected", "All locally indexed repos are already selected.");
        m.insert("settings.env_modal.empty.no_indexed", "No locally indexed repos found yet. Index a repo, then try again.");
        m.insert("settings.env_modal.empty.unavailable", "Local repo selection is unavailable in this build.");
        m.insert("settings.env_modal.loading", "Loading locally indexed repos\u{2026}");
        m.insert("settings.env_modal.button.add_repo", "Add repo");
        m.insert("settings.env_modal.button.create_environment", "Create environment");
        m.insert("settings.env_modal.toast.not_git_repo", "Selected folder is not a Git repository: {path}");

        // Platform page
        m.insert("settings.platform.new_api_key", "New API key");
        m.insert("settings.platform.save_your_key", "Save your key");
        m.insert("settings.platform.api_key_deleted", "API key deleted");
        m.insert("settings.platform.oz_cloud_api_keys", "Oz Cloud API Keys");
        m.insert("settings.platform.create_api_key", "+ Create API Key");
        m.insert("settings.platform.description", "Create and manage API keys to allow other Oz cloud agents to access your Warp account.\nFor more information, visit the ");
        m.insert("settings.platform.documentation", "Documentation.");
        m.insert("settings.platform.header.name", "Name");
        m.insert("settings.platform.header.key", "Key");
        m.insert("settings.platform.header.scope", "Scope");
        m.insert("settings.platform.header.created", "Created");
        m.insert("settings.platform.header.last_used", "Last used");
        m.insert("settings.platform.header.expires_at", "Expires at");
        m.insert("settings.platform.never", "Never");
        m.insert("settings.platform.scope.personal", "Personal");
        m.insert("settings.platform.scope.team", "Team");
        m.insert("settings.platform.no_api_keys", "No API Keys");
        m.insert("settings.platform.no_api_keys_description", "Create a key to manage external access to Warp");

        // MCP Servers page
        m.insert("settings.mcp.page_title", "MCP Servers");
        m.insert("settings.mcp.logout_success_with_name", "Successfully logged out of {name} MCP server");
        m.insert("settings.mcp.logout_success", "Successfully logged out of MCP server");
        m.insert("settings.mcp.finish_current_install", "Finish the current MCP install before opening another install link.");
        m.insert("settings.mcp.unknown_server", "Unknown MCP server '{autoinstall_param}'");
        m.insert("settings.mcp.cannot_install_from_link", "MCP server '{gallery_title}' cannot be installed from this link.");

        // Warpify page
        m.insert("settings.warpify.title", "Warpify");
        m.insert("settings.warpify.description", "Configure whether Warp attempts to \u{201c}Warpify\u{201d} (add support for blocks, input modes, etc) certain shells. ");
        m.insert("settings.warpify.learn_more", "Learn more");
        m.insert("settings.warpify.subshells", "Subshells");
        m.insert("settings.warpify.subshells_description", "Subshells supported: bash, zsh, and fish.");
        m.insert("settings.warpify.ssh", "SSH");
        m.insert("settings.warpify.ssh_description", "Warpify your interactive SSH sessions.");
        m.insert("settings.warpify.ssh_session_detection", "SSH session detection for Warpification");
        m.insert("settings.warpify.placeholder_command", "command (supports regex)");
        m.insert("settings.warpify.placeholder_host", "host (supports regex)");
        m.insert("settings.warpify.added_commands", "Added commands");
        m.insert("settings.warpify.denylisted_commands", "Denylisted commands");
        m.insert("settings.warpify.warpify_ssh_sessions", "Warpify SSH Sessions");
        m.insert("settings.warpify.install_ssh_extension", "Install SSH extension");
        m.insert("settings.warpify.ssh_extension_install_mode_description", "Controls the installation behavior for Warp's SSH extension when a remote host doesn't have it installed.");
        m.insert("settings.warpify.use_tmux_warpification", "Use Tmux Warpification");
        m.insert("settings.warpify.ssh_tmux_warpification_description", "The tmux ssh wrapper works in many situations where the default one does not, but may require you to hit a button to warpify. Takes effect in new tabs.");
        m.insert("settings.warpify.denylisted_hosts", "Denylisted hosts");

        // Referrals page
        m.insert("settings.referrals.header", "Invite a friend to Warp");
        m.insert("settings.referrals.anonymous_header", "Sign up to participate in Warp's referral program");
        m.insert("settings.referrals.link_error", "Failed to load referral code.");
        m.insert("settings.referrals.copy_link", "Copy link");
        m.insert("settings.referrals.send_email", "Send");
        m.insert("settings.referrals.sending", "Sending...");
        m.insert("settings.referrals.loading", "Loading...");
        m.insert("settings.referrals.link_copied", "Link copied.");
        m.insert("settings.referrals.email_success", "Successfully sent emails.");
        m.insert("settings.referrals.email_failure", "Failed to send emails. Please try again.");
        m.insert("settings.referrals.reward_intro", "Get exclusive Warp goodies when you refer someone*");
        m.insert("settings.referrals.terms_link", "Certain restrictions apply.");
        m.insert("settings.referrals.terms_contact", " If you have any questions about the referral program, please contact referrals@warp.dev.");
        m.insert("settings.referrals.current_referral_singular", "Current referral");
        m.insert("settings.referrals.current_referral_plural", "Current referrals");
        m.insert("settings.referrals.link_label", "Link");
        m.insert("settings.referrals.email_label", "Email");
        m.insert("settings.referrals.sign_up", "Sign up");
        m.insert("settings.referrals.enter_email_error", "Please enter an email.");
        m.insert("settings.referrals.invalid_email_error", "Please ensure the following email is valid: {invalid_email}");
        m.insert("settings.referrals.reward_exclusive_theme", "Exclusive theme");
        m.insert("settings.referrals.reward_keycaps_stickers", "Keycaps + stickers");
        m.insert("settings.referrals.reward_tshirt", "T-shirt");
        m.insert("settings.referrals.reward_notebook", "Notebook");
        m.insert("settings.referrals.reward_baseball_cap", "Baseball cap");
        m.insert("settings.referrals.reward_hoodie", "Hoodie");
        m.insert("settings.referrals.reward_hydro_flask", "Premium Hydro Flask");
        m.insert("settings.referrals.reward_backpack", "Backpack");

        // Billing and usage page - Overage section
        m.insert("settings.billing.overage.admin_header", "Enable premium model usage overages");
        m.insert("settings.billing.overage.user_header_enabled", "Premium model usage overages are enabled");
        m.insert("settings.billing.overage.user_header_disabled", "Premium model usage overages are not enabled");
        m.insert("settings.billing.overage.description", "Continue using premium models beyond your plan\u{2019}s limits. Usage is charged in $20 increments up to your spending limit, with any remaining balance charged on your scheduled billing date.");
        m.insert("settings.billing.overage.user_description", "Ask a team admin to enable overages for more AI usage.");
        m.insert("settings.billing.overage.link_text", "View details on overage usage");
        m.insert("settings.billing.overage.monthly_spending_limit", "Monthly overage spending limit");
        m.insert("settings.billing.overage.monthly_spending_limit.tooltip", "Sets the monthly overage spending limit beyond the plan amount");
        m.insert("settings.billing.overage.not_set", "Not set");
        m.insert("settings.billing.overage.total", "Total overages");
        m.insert("settings.billing.overage.resets_on", "Usage resets on {date}");
        m.insert("settings.billing.overage.one_credit", "1 credit");
        m.insert("settings.billing.overage.credits", "{count} credits");

        // Billing and usage page - Sort options
        m.insert("settings.billing.sort.a_to_z", "A to Z");
        m.insert("settings.billing.sort.z_to_a", "Z to A");
        m.insert("settings.billing.sort.ascending", "Usage ascending");
        m.insert("settings.billing.sort.descending", "Usage descending");
        m.insert("settings.billing.sort.label", "Sort by");

        // Billing and usage page - Auto reload / restricted warnings
        m.insert("settings.billing.auto_reload.exceed_limit_warning", "Auto reload is disabled, as the next reload would exceed your monthly spend limit. Increase your limit to use auto reload.");
        m.insert("settings.billing.auto_reload.delinquent_warning", "Restricted due to billing issue. Update your payment method to purchase add-on credits.");
        m.insert("settings.billing.auto_reload.restricted_warning", "Auto reload is disabled due to recent failed reload. Please update your payment method and try again.");
        m.insert("settings.billing.auto_reload.exceed_limit_with_link", "Reloading would exceed your monthly limit. ");
        m.insert("settings.billing.auto_reload.increase_limit_link", "Increase your limit");
        m.insert("settings.billing.auto_reload.to_continue", " to continue.");
        m.insert("settings.billing.restricted.billing_issue", "Restricted due to billing issue");

        // Billing and usage page - Tabs
        m.insert("settings.billing.tab.overview", "Overview");
        m.insert("settings.billing.tab.usage_history", "Usage History");

        // Billing and usage page - Enterprise usage callout
        m.insert("settings.billing.enterprise.callout_header", "Usage reporting is currently limited");
        m.insert("settings.billing.enterprise.callout_admin_prefix", "Enterprise credit usage isn\u{2019}t fully available in this view yet. For the most accurate spend tracking, ");
        m.insert("settings.billing.enterprise.callout_admin_link", "visit the admin panel");
        m.insert("settings.billing.enterprise.callout_admin_suffix", ".");
        m.insert("settings.billing.enterprise.callout_non_admin", "Enterprise credit usage isn\u{2019}t fully available in this view yet. Contact a team admin for detailed usage reporting.");

        // Billing and usage page - Add-on credits
        m.insert("settings.billing.addon.title", "Add-on credits");
        m.insert("settings.billing.addon.description", "Add-on credits are purchased in prepaid packages that roll over each billing cycle and expire after one year. The more you purchase, the better the per-credit rate. Once your base plan credits are used, add-on credits will be consumed.");
        m.insert("settings.billing.addon.description_team", "Purchased add-on credits are shared across your team.");
        m.insert("settings.billing.addon.monthly_spend_limit", "Monthly spend limit");
        m.insert("settings.billing.addon.monthly_spend_limit.tooltip", "Sets the monthly limit spent on add-on credits");
        m.insert("settings.billing.addon.purchased_this_month", "Purchased this month");
        m.insert("settings.billing.addon.auto_reload", "Auto reload");
        m.insert("settings.billing.addon.auto_reload.description", "When enabled, auto reload will automatically purchase {amount} credits when your add-on credit balance reaches 100 credits remaining.");
        m.insert("settings.billing.addon.one_time_purchase", "One-time purchase");
        m.insert("settings.billing.addon.buy", "Buy");
        m.insert("settings.billing.addon.buying", "Buying\u{2026}");
        m.insert("settings.billing.addon.one_credit", "1 credit");
        m.insert("settings.billing.addon.credits", "{count} credits");
        m.insert("settings.billing.addon.zero_credits", "0 credits");
        m.insert("settings.billing.addon.contact_account_executive", "Contact your Account Executive for more add-on credits.");
        m.insert("settings.billing.addon.contact_admin", "Contact a team admin to purchase add-on credits.");
        m.insert("settings.billing.addon.switch_build", "Switch to the Build plan");
        m.insert("settings.billing.addon.upgrade_build", "Upgrade to the Build plan");
        m.insert("settings.billing.addon.to_purchase_suffix", " to purchase add-on credits.");

        // Billing and usage page - Cloud agent trial
        m.insert("settings.billing.ambient_trial.title", "Cloud agent trial");
        m.insert("settings.billing.ambient_trial.one_credit_remaining", "1 credit remaining");
        m.insert("settings.billing.ambient_trial.credits_remaining", "{count} credits remaining");
        m.insert("settings.billing.ambient_trial.new_agent", "New agent");
        m.insert("settings.billing.ambient_trial.buy_more", "Buy more");

        // Billing and usage page - Usage history
        m.insert("settings.billing.usage_history.last_30_days", "Last 30 days");
        m.insert("settings.billing.usage_history.load_more", "Load more");
        m.insert("settings.billing.usage_history.empty_title", "No usage history");
        m.insert("settings.billing.usage_history.empty_description", "Kick off an agent task to view usage history here.");

        // Billing and usage page - Usage section
        m.insert("settings.billing.usage.title", "Usage");
        m.insert("settings.billing.usage.credits", "Credits");
        m.insert("settings.billing.usage.resets", "Resets {time}");
        m.insert("settings.billing.usage.limit_description", "This is the {duration} limit of AI credits for your account.");
        m.insert("settings.billing.usage.team_total", "Team total");

        // Billing and usage page - Modals
        m.insert("settings.billing.overage.modal_title", "Overage spending limit");
        m.insert("settings.billing.addon.modal_title", "Monthly spending limit");

        // Billing and usage page - Prorated limits
        m.insert("settings.billing.prorated.tooltip_current_user", "Your credit limit is prorated because you joined midway through the billing cycle.");
        m.insert("settings.billing.prorated.tooltip_other_user", "This credit limit is prorated because this user joined midway through the billing cycle.");

        // Billing and usage page - Toast messages
        m.insert("settings.billing.toast.update_settings_failed", "Failed to update workspace settings");
        m.insert("settings.billing.toast.purchase_success", "Successfully purchased add-on credits");

        // Billing and usage page - Plan section
        m.insert("settings.billing.plan.title", "Plan");
        m.insert("settings.billing.plan.free", "Free");
        m.insert("settings.billing.plan.sign_up", "Sign up");
        m.insert("settings.billing.plan.compare_plans", "Compare plans");
        m.insert("settings.billing.plan.manage_billing", "Manage billing");
        m.insert("settings.billing.plan.open_admin_panel", "Open admin panel");

        // Billing and usage page - Upgrade CTA
        m.insert("settings.billing.upgrade.manage_billing_regain", "Manage billing");
        m.insert("settings.billing.upgrade.to_regain_access", " to regain access to AI features.");
        m.insert("settings.billing.upgrade.contact_admin_billing", "Contact your team admin to resolve billing issues.");
        m.insert("settings.billing.upgrade.switch_build", "Switch to the Build plan");
        m.insert("settings.billing.upgrade.for_flexible_pricing", " for a more flexible pricing model.");
        m.insert("settings.billing.upgrade.upgrade_build", "Upgrade to the Build plan");
        m.insert("settings.billing.upgrade.bring_your_own_key", "bring your own key");
        m.insert("settings.billing.upgrade.or", " or ");
        m.insert("settings.billing.upgrade.for_increased_access", " for increased access to AI features.");
        m.insert("settings.billing.upgrade.to_turbo", "Upgrade to Turbo plan");
        m.insert("settings.billing.upgrade.to_lightspeed", "Upgrade to Lightspeed plan");
        m.insert("settings.billing.upgrade.generic", "Upgrade");
        m.insert("settings.billing.upgrade.to_get_more_usage", " to get more AI usage.");
        m.insert("settings.billing.upgrade.to_max", "Upgrade to Max");
        m.insert("settings.billing.upgrade.for_more_ai_credits", " for more AI credits.");
        m.insert("settings.billing.upgrade.switch_business", "Switch to Business");
        m.insert("settings.billing.upgrade.for_security_features", " for security features like SSO and automatically applied zero data retention.");
        m.insert("settings.billing.upgrade.to_enterprise", "Upgrade to Enterprise");
        m.insert("settings.billing.upgrade.for_custom_limits", " for custom limits and dedicated support.");
        m.insert("settings.billing.upgrade.contact_support", "Contact support");
        m.insert("settings.billing.upgrade.for_more_ai_usage_generic", " for more AI usage.");
        m.insert("settings.billing.upgrade.for_more_credits_models", " for more credits and access to more models.");

        // Teams page - Header
        m.insert("settings.teams.header", "Teams");

        // Teams page - Create team
        m.insert("settings.teams.create.title", "Create a team");
        m.insert("settings.teams.create.description", "When you create a team, you can collaborate on agent-driven development by sharing cloud agent runs, environments, automations, and artifacts. You can also create a shared knowledge store for teammates and agents alike.");
        m.insert("settings.teams.create.team_name_placeholder", "Team name");
        m.insert("settings.teams.create.button", "Create");
        m.insert("settings.teams.create.discoverable_checkbox_domain", "Allow Warp users with an @{domain} email to find and join the team.");
        m.insert("settings.teams.create.discoverable_checkbox_generic", "Allow Warp users with the same email domain as you to find and join the team.");
        m.insert("settings.teams.create.join_existing", "Or, join an existing team within your company");

        // Teams page - Team management
        m.insert("settings.teams.manage.leave_team", "Leave team");
        m.insert("settings.teams.manage.delete_team", "Delete team");
        m.insert("settings.teams.manage.rename_placeholder", "Your new team name");
        m.insert("settings.teams.manage.transfer_ownership_title", "Transfer team ownership?");
        m.insert("settings.teams.manage.contact_support", "Contact support");
        m.insert("settings.teams.manage.manage_billing", "Manage billing");
        m.insert("settings.teams.manage.open_admin_panel", "Open admin panel");
        m.insert("settings.teams.manage.manage_plan", "Manage plan");

        // Teams page - Invite section
        m.insert("settings.teams.invite.by_link", "Invite by Link");
        m.insert("settings.teams.invite.link_toggle_instructions", "As an admin, you can choose whether to enable or disable the ability for team members to invite others by invitation link.");
        m.insert("settings.teams.invite.reset_links", "Reset links");
        m.insert("settings.teams.invite.restrict_by_domain", "Restrict by domain");
        m.insert("settings.teams.invite.domain_restrictions_instructions", "Only allow users with emails at specific domains to join your team through the invite link.");
        m.insert("settings.teams.invite.domains_placeholder", "Domains, comma separated");
        m.insert("settings.teams.invite.set_button", "Set");
        m.insert("settings.teams.invite.invalid_domains", "Some of the provided domains are invalid, or have already been added.");
        m.insert("settings.teams.invite.failed_load_link", "Failed to load invite link.");
        m.insert("settings.teams.invite.by_email", "Invite by Email");
        m.insert("settings.teams.invite.email_expiry_instructions", "Email invitations are valid for 7 days.");
        m.insert("settings.teams.invite.emails_placeholder", "Emails, comma separated");
        m.insert("settings.teams.invite.invite_button", "Invite");
        m.insert("settings.teams.invite.invalid_emails", "Some of the provided email addresses are invalid, already invited, or members of the team.");

        // Teams page - Team members
        m.insert("settings.teams.members.header", "Team Members");
        m.insert("settings.teams.members.cancel_invite", "Cancel invite");
        m.insert("settings.teams.members.transfer_ownership", "Transfer ownership");
        m.insert("settings.teams.members.demote_from_admin", "Demote from admin");
        m.insert("settings.teams.members.promote_to_admin", "Promote to admin");
        m.insert("settings.teams.members.remove_from_team", "Remove from team");
        m.insert("settings.teams.members.remove_domain", "Remove domain");

        // Teams page - Badges
        m.insert("settings.teams.badge.expired", "EXPIRED");
        m.insert("settings.teams.badge.pending", "PENDING");
        m.insert("settings.teams.badge.owner", "OWNER");
        m.insert("settings.teams.badge.admin", "ADMIN");
        m.insert("settings.teams.badge.past_due", "PAST DUE");
        m.insert("settings.teams.badge.unpaid", "UNPAID");

        // Teams page - Plan usage
        m.insert("settings.teams.plan.free_usage_limits", "Free plan usage limits");
        m.insert("settings.teams.plan.usage_limits", "Plan usage limits");
        m.insert("settings.teams.plan.shared_notebooks", "Shared Notebooks");
        m.insert("settings.teams.plan.shared_workflows", "Shared Workflows");

        // Teams page - Limit hit messages
        m.insert("settings.teams.limit.admin", "You've reached the team member limit for your plan. Upgrade to add more teammates.");
        m.insert("settings.teams.limit.admin_not_upgradeable", "You've reached the team member limit for your plan. Contact support@warp.dev to add more teammates.");
        m.insert("settings.teams.limit.non_admin", "You've reached the team member limit for your plan. Contact a team admin to add more teammates.");

        // Teams page - Team limit exceeded messages
        m.insert("settings.teams.limit_exceeded.admin_upgradeable", "You've exceeded the team member limit for your plan. Upgrade to add more teammates.");
        m.insert("settings.teams.limit_exceeded.admin_not_upgradeable", "You've exceeded the team member limit for your plan. Please contact support@warp.dev to upgrade your team.");
        m.insert("settings.teams.limit_exceeded.non_admin", "You've exceeded the team member limit for your plan. Contact a team admin to upgrade your team.");

        // Teams page - Delinquency messages
        m.insert("settings.teams.delinquent.admin_non_self_serve", "Team invites have been restricted due to a payment issue. Please contact support@warp.dev to restore access.");
        m.insert("settings.teams.delinquent.non_admin", "Team invites have been restricted due to a payment issue. Please contact a team admin to restore access.");
        m.insert("settings.teams.delinquent.admin_self_serve_line1", "Team invites have been restricted due to a subscription payment issue.");
        m.insert("settings.teams.delinquent.admin_self_serve_line2_prefix", "Please ");
        m.insert("settings.teams.delinquent.admin_self_serve_line2_link", "update your payment information");
        m.insert("settings.teams.delinquent.admin_self_serve_line2_suffix", " to restore access.");

        // Teams page - Discoverable teams
        m.insert("settings.teams.discoverable.header", "Make team discoverable");
        m.insert("settings.teams.discoverable.allow_domain", "Allow Warp users with an @{domain} email to find and join the team.");
        m.insert("settings.teams.discoverable.allow_same_domain", "Allow Warp users with the same email domain as you to find and join the team.");

        // Teams page - Team discovery
        m.insert("settings.teams.discovery.one_teammate", "1 teammate");
        m.insert("settings.teams.discovery.multiple_teammates", "{count} teammates");
        m.insert("settings.teams.discovery.join_description", "Join this team and start collaborating on workflows, notebooks, and more.");
        m.insert("settings.teams.discovery.join_button", "Join");
        m.insert("settings.teams.discovery.contact_admin", "Contact Admin to request access");

        // Teams page - Pricing
        m.insert("settings.teams.pricing.team_members", "Team members");
        m.insert("settings.teams.pricing.prorated_admin", "You'll be charged for a portion of the team member's usage of Warp.");
        m.insert("settings.teams.pricing.prorated_member", "Your admin will be charged for a portion of the team member's usage of Warp.");
        m.insert("settings.teams.pricing.additional_members_with_cost", "Additional members are billed at your plan's per-user rate: ${monthly_cost}/month or ${yearly_cost}/year, depending on your billing interval. {prorated_message}");
        m.insert("settings.teams.pricing.additional_members_no_cost", "Additional members are billed at your plan's per-user rate. {prorated_message}");

        // Teams page - Upgrade options
        m.insert("settings.teams.upgrade.to_build", "Upgrade to Build");
        m.insert("settings.teams.upgrade.to_turbo", "Upgrade to Turbo plan");
        m.insert("settings.teams.upgrade.to_lightspeed", "Upgrade to Lightspeed plan");
        m.insert("settings.teams.upgrade.compare_plans", "Compare plans");

        // Teams page - Tab labels
        m.insert("settings.teams.tab.link", "Link");
        m.insert("settings.teams.tab.email", "Email");

        // Teams page - Offline
        m.insert("settings.teams.offline", "You are offline.");

        // Teams page - Toast messages
        m.insert("settings.teams.toast.failed_send_invite", "Failed to send invite");
        m.insert("settings.teams.toast.toggled_invite_links", "Toggled invite links");
        m.insert("settings.teams.toast.failed_toggle_invite_links", "Failed to toggle invite links");
        m.insert("settings.teams.toast.reset_invite_links", "Reset invite links");
        m.insert("settings.teams.toast.failed_reset_invite_links", "Failed to reset invite links");
        m.insert("settings.teams.toast.deleted_invite", "Deleted invite");
        m.insert("settings.teams.toast.failed_delete_invite", "Failed to delete invite");
        m.insert("settings.teams.toast.failed_add_domain", "Failed to add domain restriction");
        m.insert("settings.teams.toast.failed_delete_domain", "Failed to delete domain restriction");
        m.insert("settings.teams.toast.failed_upgrade_link", "Failed to generate upgrade link. Please contact us at feedback@warp.dev");
        m.insert("settings.teams.toast.failed_billing_link", "Failed to generate billing link. Please contact us at feedback@warp.dev");
        m.insert("settings.teams.toast.toggled_discoverability", "Toggled team discoverability");
        m.insert("settings.teams.toast.failed_toggle_discoverability", "Failed to toggle team discoverability");
        m.insert("settings.teams.toast.joined_team", "Successfully joined team");
        m.insert("settings.teams.toast.joined_team_named", "Successfully joined {team_name}");
        m.insert("settings.teams.toast.failed_join_team", "Failed to join team");
        m.insert("settings.teams.toast.transferred_ownership", "Successfully transferred team ownership");
        m.insert("settings.teams.toast.failed_transfer_ownership", "Failed to transfer team ownership");
        m.insert("settings.teams.toast.updated_member_role", "Successfully updated team member role");
        m.insert("settings.teams.toast.failed_update_member_role", "Failed to update team member role");
        m.insert("settings.teams.toast.error_leaving_team", "Error leaving team");
        m.insert("settings.teams.toast.left_team", "Successfully left team");
        m.insert("settings.teams.toast.renamed_team", "Successfully renamed team");
        m.insert("settings.teams.toast.failed_rename_team", "Failed to rename team");
        m.insert("settings.teams.toast.link_copied", "Link copied to clipboard!");
        m.insert("settings.teams.toast.invalid_domain_count", "Invalid domains: {count}");
        m.insert("settings.teams.toast.domains_added", "Domain restrictions added: {count}");
        m.insert("settings.teams.toast.invalid_email_count", "Invalid emails: {count}");
        m.insert("settings.teams.toast.invite_sent", "Your invite is on the way!");
        m.insert("settings.teams.toast.invites_sent", "Your {count} invites are on the way!");

        m
    };
}
