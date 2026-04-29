use std::collections::HashMap;

lazy_static::lazy_static! {
    pub static ref TRANSLATIONS: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();

        // Navigation sidebar
        m.insert("nav.account", "\u{8D26}\u{6237}");
        m.insert("nav.appearance", "\u{5916}\u{89C2}");
        m.insert("nav.features", "\u{529F}\u{80FD}");
        m.insert("nav.keybindings", "\u{952E}\u{76D8}\u{5FEB}\u{6377}\u{952E}");
        m.insert("nav.privacy", "\u{9690}\u{79C1}");
        m.insert("nav.teams", "\u{56E2}\u{961F}");
        m.insert("nav.about", "\u{5173}\u{4E8E}");
        m.insert("nav.ai", "AI");
        m.insert("nav.code", "\u{4EE3}\u{7801}");
        m.insert("nav.cloud_platform", "\u{4E91}\u{5E73}\u{53F0}");
        m.insert("nav.agents", "\u{4EE3}\u{7406}");
        m.insert("nav.billing_and_usage", "\u{8BA1}\u{8D39}\u{4E0E}\u{7528}\u{91CF}");
        m.insert("nav.shared_blocks", "\u{5171}\u{4EAB}\u{4EE3}\u{7801}\u{5757}");
        m.insert("nav.mcp_servers", "MCP \u{670D}\u{52A1}\u{5668}");
        m.insert("nav.warp_drive", "Warp Drive");
        m.insert("nav.warp_agent", "Warp Agent");
        m.insert("nav.agent_profiles", "\u{914D}\u{7F6E}\u{6587}\u{4EF6}");
        m.insert("nav.agent_mcp_servers", "MCP \u{670D}\u{52A1}\u{5668}");
        m.insert("nav.knowledge", "\u{77E5}\u{8BC6}");
        m.insert("nav.third_party_cli_agents", "\u{7B2C}\u{4E09}\u{65B9} CLI \u{4EE3}\u{7406}");
        m.insert("nav.code_indexing", "\u{7D22}\u{5F15}\u{548C}\u{9879}\u{76EE}");
        m.insert("nav.editor_and_code_review", "\u{7F16}\u{8F91}\u{5668}\u{548C}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}");
        m.insert("nav.cloud_environments", "\u{73AF}\u{5883}");
        m.insert("nav.oz_cloud_api_keys", "Oz Cloud API \u{5BC6}\u{94A5}");

        // Settings shell
        m.insert("settings.title", "\u{8BBE}\u{7F6E}");
        m.insert("settings.search", "\u{641C}\u{7D22}");
        m.insert("settings.shell.split_right", "\u{53F3}\u{4FA7}\u{62C6}\u{5206}\u{7A97}\u{683C}");
        m.insert("settings.shell.split_left", "\u{5DE6}\u{4FA7}\u{62C6}\u{5206}\u{7A97}\u{683C}");
        m.insert("settings.shell.split_down", "\u{5411}\u{4E0B}\u{62C6}\u{5206}\u{7A97}\u{683C}");
        m.insert("settings.shell.split_up", "\u{5411}\u{4E0A}\u{62C6}\u{5206}\u{7A97}\u{683C}");
        m.insert("settings.shell.close_pane", "\u{5173}\u{95ED}\u{7A97}\u{683C}");
        m.insert("settings.shell.toggle.code_review_show", "\u{5728}\u{6807}\u{7B7E}\u{680F}\u{4E2D}\u{663E}\u{793A}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}\u{6309}\u{94AE}");
        m.insert("settings.shell.toggle.code_review_hide", "\u{5728}\u{6807}\u{7B7E}\u{680F}\u{4E2D}\u{9690}\u{85CF}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}\u{6309}\u{94AE}");
        m.insert("settings.shell.toggle.init_block_show", "\u{663E}\u{793A}\u{521D}\u{59CB}\u{5316}\u{4EE3}\u{7801}\u{5757}");
        m.insert("settings.shell.toggle.init_block_hide", "\u{9690}\u{85CF}\u{521D}\u{59CB}\u{5316}\u{4EE3}\u{7801}\u{5757}");
        m.insert("settings.shell.toggle.in_band_show", "\u{663E}\u{793A}\u{5E26}\u{5185}\u{547D}\u{4EE4}\u{4EE3}\u{7801}\u{5757}");
        m.insert("settings.shell.toggle.in_band_hide", "\u{9690}\u{85CF}\u{5E26}\u{5185}\u{547D}\u{4EE4}\u{4EE3}\u{7801}\u{5757}");
        m.insert("settings.shell.toggle.tab_bar_show", "\u{59CB}\u{7EC8}\u{663E}\u{793A}\u{6807}\u{7B7E}\u{680F}");
        m.insert("settings.shell.toggle.tab_bar_hide_fullscreen", "\u{5168}\u{5C4F}\u{65F6}\u{9690}\u{85CF}\u{6807}\u{7B7E}\u{680F}");
        m.insert("settings.shell.toggle.tab_bar_hover", "\u{4EC5}\u{5728}\u{60AC}\u{505C}\u{65F6}\u{663E}\u{793A}\u{6807}\u{7B7E}\u{680F}");

        // Appearance page - Language
        m.insert("settings.appearance.language.label", "\u{8BED}\u{8A00}");
        m.insert("settings.appearance.language.subtitle", "\u{8BBE}\u{7F6E} Warp \u{754C}\u{9762}\u{7684}\u{663E}\u{793A}\u{8BED}\u{8A00}\u{3002}");

        // Appearance page - Themes
        m.insert("settings.appearance.themes", "\u{4E3B}\u{9898}");
        m.insert("settings.appearance.themes.create_custom", "\u{521B}\u{5EFA}\u{81EA}\u{5B9A}\u{4E49}\u{4E3B}\u{9898}");
        m.insert("settings.appearance.themes.light", "\u{6D45}\u{8272}");
        m.insert("settings.appearance.themes.dark", "\u{6DF1}\u{8272}");
        m.insert("settings.appearance.themes.current", "\u{5F53}\u{524D}\u{4E3B}\u{9898}");
        m.insert("settings.appearance.themes.sync_with_os", "\u{4E0E}\u{7CFB}\u{7EDF}\u{540C}\u{6B65}");
        m.insert("settings.appearance.themes.sync_with_os.description", "\u{5F53}\u{7CFB}\u{7EDF}\u{5207}\u{6362}\u{6D45}\u{8272}\u{548C}\u{6DF1}\u{8272}\u{4E3B}\u{9898}\u{65F6}\u{81EA}\u{52A8}\u{5207}\u{6362}\u{3002}");

        // Appearance page - Window
        m.insert("settings.appearance.window", "\u{7A97}\u{53E3}");
        m.insert("settings.appearance.icon", "\u{56FE}\u{6807}");
        m.insert("settings.appearance.icon.customize", "\u{81EA}\u{5B9A}\u{4E49}\u{5E94}\u{7528}\u{56FE}\u{6807}");
        m.insert("settings.appearance.icon.bundle_warning", "\u{66F4}\u{6539}\u{5E94}\u{7528}\u{56FE}\u{6807}\u{9700}\u{8981}\u{5E94}\u{7528}\u{5DF2}\u{6253}\u{5305}\u{3002}");
        m.insert("settings.appearance.icon.restart_warning", "\u{60A8}\u{53EF}\u{80FD}\u{9700}\u{8981}\u{91CD}\u{542F} Warp \u{624D}\u{80FD}\u{5E94}\u{7528}\u{9996}\u{9009}\u{7684}\u{56FE}\u{6807}\u{6837}\u{5F0F}\u{3002}");
        m.insert("settings.appearance.window.opacity", "\u{7A97}\u{53E3}\u{4E0D}\u{900F}\u{660E}\u{5EA6}");
        m.insert("settings.appearance.window.blur", "\u{80CC}\u{666F}\u{6A21}\u{7CCA}");
        m.insert("settings.appearance.window.blur_texture", "\u{80CC}\u{666F}\u{6A21}\u{7CCA}\u{7EB9}\u{7406}");
        m.insert("settings.appearance.window.custom_size", "\u{4EE5}\u{81EA}\u{5B9A}\u{4E49}\u{5927}\u{5C0F}\u{6253}\u{5F00}\u{65B0}\u{7A97}\u{53E3}");
        m.insert("settings.appearance.window.columns", "\u{5217}");
        m.insert("settings.appearance.window.rows", "\u{884C}");
        m.insert("settings.appearance.window.opacity.label", "\u{7A97}\u{53E3}\u{4E0D}\u{900F}\u{660E}\u{5EA6}:");
        m.insert("settings.appearance.window.opacity.unsupported", "\u{60A8}\u{7684}\u{56FE}\u{5F62}\u{9A71}\u{52A8}\u{7A0B}\u{5E8F}\u{4E0D}\u{652F}\u{6301}\u{900F}\u{660E}\u{5EA6}\u{3002}");
        m.insert("settings.appearance.window.opacity.value", "\u{7A97}\u{53E3}\u{4E0D}\u{900F}\u{660E}\u{5EA6}: {opacity_value}");
        m.insert("settings.appearance.window.opacity.graphics_warning", "\u{9009}\u{62E9}\u{7684}\u{56FE}\u{5F62}\u{8BBE}\u{7F6E}\u{53EF}\u{80FD}\u{4E0D}\u{652F}\u{6301}\u{6E32}\u{67D3}\u{900F}\u{660E}\u{7A97}\u{53E3}\u{3002}");
        m.insert("settings.appearance.window.opacity.graphics_hint", " \u{8BF7}\u{5C1D}\u{8BD5}\u{66F4}\u{6539}\u{529F}\u{80FD} > \u{7CFB}\u{7EDF}\u{4E2D}\u{7684}\u{56FE}\u{5F62}\u{540E}\u{7AEF}\u{6216}\u{96C6}\u{6210} GPU \u{7684}\u{8BBE}\u{7F6E}\u{3002}");
        m.insert("settings.appearance.window.blur.value", "\u{7A97}\u{53E3}\u{6A21}\u{7CCA}\u{534A}\u{5F84}: {blur_value}");
        m.insert("settings.appearance.window.blur.use_acrylic", "\u{4F7F}\u{7528}\u{7A97}\u{53E3}\u{6A21}\u{7CCA}\u{FF08}\u{4E9A}\u{514B}\u{529B}\u{7EB9}\u{7406}\u{FF09}");
        m.insert("settings.appearance.window.blur.hardware_warning", "\u{9009}\u{62E9}\u{7684}\u{786C}\u{4EF6}\u{53EF}\u{80FD}\u{4E0D}\u{652F}\u{6301}\u{6E32}\u{67D3}\u{900F}\u{660E}\u{7A97}\u{53E3}\u{3002}");

        // Appearance page - Input
        m.insert("settings.appearance.input", "\u{8F93}\u{5165}");
        m.insert("settings.appearance.input.type", "\u{8F93}\u{5165}\u{7C7B}\u{578B}");
        m.insert("settings.appearance.input.mode", "\u{8F93}\u{5165}\u{4F4D}\u{7F6E}");
        m.insert("settings.appearance.input.warp", "Warp");
        m.insert("settings.appearance.input.shell_ps1", "Shell (PS1)");
        m.insert("settings.appearance.input.mode.warp", "\u{56FA}\u{5B9A}\u{5728}\u{5E95}\u{90E8} (Warp \u{6A21}\u{5F0F})");
        m.insert("settings.appearance.input.mode.reverse", "\u{56FA}\u{5B9A}\u{5728}\u{9876}\u{90E8} (\u{53CD}\u{5411}\u{6A21}\u{5F0F})");
        m.insert("settings.appearance.input.mode.classic", "\u{4ECE}\u{9876}\u{90E8}\u{5F00}\u{59CB} (\u{7ECF}\u{5178}\u{6A21}\u{5F0F})");

        // Appearance page - Blocks
        m.insert("settings.appearance.blocks", "\u{4EE3}\u{7801}\u{5757}");
        m.insert("settings.appearance.blocks.jump_to_bottom", "\u{663E}\u{793A}\u{8DF3}\u{8F6C}\u{5230}\u{4EE3}\u{7801}\u{5757}\u{5E95}\u{90E8}\u{6309}\u{94AE}");
        m.insert("settings.appearance.blocks.dividers", "\u{663E}\u{793A}\u{4EE3}\u{7801}\u{5757}\u{5206}\u{9694}\u{7EBF}");
        m.insert("settings.appearance.panes", "\u{7A97}\u{683C}");
        m.insert("settings.appearance.panes.consistent_tools", "\u{5DE5}\u{5177}\u{9762}\u{677F}\u{53EF}\u{89C1}\u{6027}\u{5728}\u{6807}\u{7B7E}\u{9875}\u{4E4B}\u{95F4}\u{4FDD}\u{6301}\u{4E00}\u{81F4}");
        m.insert("settings.appearance.panes.dim_inactive", "\u{4F4E}\u{4EAE}\u{975E}\u{6D3B}\u{52A8}\u{7A97}\u{683C}");
        m.insert("settings.appearance.panes.focus_follows_mouse", "\u{7126}\u{70B9}\u{8DDF}\u{968F}\u{9F20}\u{6807}");
        m.insert("settings.appearance.panes.compact_mode", "\u{7D27}\u{51D1}\u{6A21}\u{5F0F}");
        m.insert("settings.appearance.cursor", "\u{5149}\u{6807}");
        m.insert("settings.appearance.cursor.type", "\u{5149}\u{6807}\u{7C7B}\u{578B}");
        m.insert("settings.appearance.cursor.type.disabled_vim", "\u{5728} Vim \u{6A21}\u{5F0F}\u{4E0B}\u{5149}\u{6807}\u{7C7B}\u{578B}\u{88AB}\u{7981}\u{7528}");
        m.insert("settings.appearance.cursor.blink", "\u{5149}\u{6807}\u{95EA}\u{70C1}");

        // Appearance page - Text
        m.insert("settings.appearance.text", "\u{6587}\u{672C}");
        m.insert("settings.appearance.text.font_size", "\u{5B57}\u{4F53}\u{5927}\u{5C0F}");
        m.insert("settings.appearance.text.font_family", "\u{5B57}\u{4F53}");
        m.insert("settings.appearance.text.agent_font", "\u{4EE3}\u{7406}\u{5B57}\u{4F53}");
        m.insert("settings.appearance.text.match_terminal", "\u{5339}\u{914D}\u{7EC8}\u{7AEF}");
        m.insert("settings.appearance.text.line_height", "\u{884C}\u{9AD8}");
        m.insert("settings.appearance.text.reset_default", "\u{91CD}\u{7F6E}\u{4E3A}\u{9ED8}\u{8BA4}\u{503C}");
        m.insert("settings.appearance.text.terminal_font", "\u{7EC8}\u{7AEF}\u{5B57}\u{4F53}");
        m.insert("settings.appearance.text.view_system_fonts", "\u{67E5}\u{770B}\u{6240}\u{6709}\u{53EF}\u{7528}\u{7684}\u{7CFB}\u{7EDF}\u{5B57}\u{4F53}");
        m.insert("settings.appearance.text.font_weight", "\u{5B57}\u{4F53}\u{7C97}\u{7EC6}");
        m.insert("settings.appearance.text.font_size_px", "\u{5B57}\u{4F53}\u{5927}\u{5C0F} (px)");
        m.insert("settings.appearance.text.notebook_font_size", "\u{7B14}\u{8BB0}\u{672C}\u{5B57}\u{4F53}\u{5927}\u{5C0F}");
        m.insert("settings.appearance.text.thin_strokes", "\u{4F7F}\u{7528}\u{7EC6}\u{7EBF}\u{684F}");
        m.insert("settings.appearance.text.min_contrast", "\u{5F3A}\u{5236}\u{6700}\u{5C0F}\u{5BF9}\u{6BD4}\u{5EA6}");
        m.insert("settings.appearance.text.ligatures", "\u{5728}\u{7EC8}\u{7AEF}\u{4E2D}\u{663E}\u{793A}\u{8FDE}\u{5B57}");
        m.insert("settings.appearance.text.ligatures.warning", "\u{8FDE}\u{5B57}\u{53EF}\u{80FD}\u{964D}\u{4F4E}\u{6027}\u{80FD}");

        // Appearance page - Full-screen Apps
        m.insert("settings.appearance.full_screen_apps", "\u{5168}\u{5C4F}\u{5E94}\u{7528}");

        // Appearance page - Tabs
        m.insert("settings.appearance.tabs", "\u{6807}\u{7B7E}\u{9875}");
        m.insert("settings.appearance.tabs.close_position", "\u{6807}\u{7B7E}\u{5173}\u{95ED}\u{6309}\u{94AE}\u{4F4D}\u{7F6E}");
        m.insert("settings.appearance.tabs.indicators", "\u{663E}\u{793A}\u{6807}\u{7B7E}\u{6307}\u{793A}\u{5668}");
        m.insert("settings.appearance.tabs.code_review_button", "\u{663E}\u{793A}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}\u{6309}\u{94AE}");
        m.insert("settings.appearance.tabs.preserve_color", "\u{4E3A}\u{65B0}\u{6807}\u{7B7E}\u{4FDD}\u{7559}\u{6D3B}\u{52A8}\u{6807}\u{7B7E}\u{989C}\u{8272}");
        m.insert("settings.appearance.tabs.vertical_layout", "\u{4F7F}\u{7528}\u{5782}\u{76F4}\u{6807}\u{7B7E}\u{5E03}\u{5C40}");
        m.insert("settings.appearance.tabs.prompt_as_title", "\u{4F7F}\u{7528}\u{6700}\u{65B0}\u{7528}\u{6237}\u{63D0}\u{793A}\u{4F5C}\u{4E3A}\u{6807}\u{7B7E}\u{540D}\u{4E2D}\u{7684}\u{4F1A}\u{8BDD}\u{6807}\u{9898}");
        m.insert("settings.appearance.tabs.prompt_as_title.description", "\u{5728}\u{5782}\u{76F4}\u{6807}\u{7B7E}\u{4E2D}\u{663E}\u{793A}\u{6700}\u{65B0}\u{7684}\u{7528}\u{6237}\u{63D0}\u{793A}\u{800C}\u{975E} Oz \u{548C}\u{7B2C}\u{4E09}\u{65B9}\u{4EE3}\u{7406}\u{4F1A}\u{8BDD}\u{7684}\u{751F}\u{6210}\u{4F1A}\u{8BDD}\u{6807}\u{9898}\u{3002}");
        m.insert("settings.appearance.tabs.header_layout", "\u{6807}\u{9898}\u{680F}\u{5DE5}\u{5177}\u{680F}\u{5E03}\u{5C40}");
        m.insert("settings.appearance.tabs.directory_colors", "\u{76EE}\u{5F55}\u{6807}\u{7B7E}\u{989C}\u{8272}");
        m.insert("settings.appearance.tabs.directory_colors.description", "\u{6839}\u{636E}\u{60A8}\u{5DE5}\u{4F5C}\u{7684}\u{76EE}\u{5F55}\u{6216}\u{4ED3}\u{5E93}\u{81EA}\u{52A8}\u{4E3A}\u{6807}\u{7B7E}\u{7740}\u{8272}\u{3002}");
        m.insert("settings.appearance.tabs.directory_colors.default", "\u{9ED8}\u{8BA4}\u{FF08}\u{65E0}\u{989C}\u{8272}\u{FF09}");
        m.insert("settings.appearance.tabs.show_tab_bar", "\u{663E}\u{793A}\u{6807}\u{7B7E}\u{680F}");
        m.insert("settings.appearance.tabs.alt_screen_padding", "\u{5728}\u{5907}\u{7528}\u{5C4F}\u{5E55}\u{4E2D}\u{4F7F}\u{7528}\u{81EA}\u{5B9A}\u{4E49}\u{8FB9}\u{8DDD}");
        m.insert("settings.appearance.tabs.uniform_padding", "\u{7EDF}\u{4E00}\u{8FB9}\u{8DDD} (px)");

        // Appearance page - Zoom
        m.insert("settings.appearance.zoom", "\u{7F29}\u{653E}");
        m.insert("settings.appearance.zoom.description", "\u{8C03}\u{6574}\u{6240}\u{6709}\u{7A97}\u{53E3}\u{7684}\u{9ED8}\u{8BA4}\u{7F29}\u{653E}\u{7EA7}\u{522B}");

        // Appearance page - Dropdown options
        m.insert("settings.appearance.option.never", "\u{4ECE}\u{4E0D}");
        m.insert("settings.appearance.option.always", "\u{59CB}\u{7EC8}");
        m.insert("settings.appearance.option.left", "\u{5DE6}\u{4FA7}");
        m.insert("settings.appearance.option.right", "\u{53F3}\u{4FA7}");
        m.insert("settings.appearance.option.on_low_dpi", "\u{4F4E} DPI \u{663E}\u{793A}\u{5668}\u{4E0A}");
        m.insert("settings.appearance.option.on_high_dpi", "\u{9AD8} DPI \u{663E}\u{793A}\u{5668}\u{4E0A}");
        m.insert("settings.appearance.option.only_named_colors", "\u{4EC5}\u{547D}\u{540D}\u{989C}\u{8272}");
        m.insert("settings.appearance.option.when_windowed", "\u{7A97}\u{53E3}\u{6A21}\u{5F0F}\u{65F6}");
        m.insert("settings.appearance.option.only_on_hover", "\u{4EC5}\u{60AC}\u{505C}\u{65F6}");

        // Appearance page - Input binding descriptions
        m.insert("settings.appearance.input.binding.start_top", "\u{4ECE}\u{9876}\u{90E8}\u{5F00}\u{59CB}\u{8F93}\u{5165}");
        m.insert("settings.appearance.input.binding.pin_top", "\u{56FA}\u{5B9A}\u{8F93}\u{5165}\u{5230}\u{9876}\u{90E8}");
        m.insert("settings.appearance.input.binding.pin_bottom", "\u{56FA}\u{5B9A}\u{8F93}\u{5165}\u{5230}\u{5E95}\u{90E8}");
        m.insert("settings.appearance.input.binding.toggle", "\u{5207}\u{6362}\u{8F93}\u{5165}\u{6A21}\u{5F0F}\u{FF08}Warp/\u{7ECF}\u{5178}\u{FF09}");

        // Common buttons
        m.insert("button.reset", "\u{91CD}\u{7F6E}");
        m.insert("button.add", "\u{6DFB}\u{52A0}");
        m.insert("button.remove", "\u{79FB}\u{9664}");
        m.insert("button.save", "\u{4FDD}\u{5B58}");
        m.insert("button.cancel", "\u{53D6}\u{6D88}");
        m.insert("button.close", "\u{5173}\u{95ED}");
        m.insert("button.sign_up", "\u{6CE8}\u{518C}");
        m.insert("button.apply", "\u{5E94}\u{7528}");

        // Features page
        m.insert("settings.features", "\u{529F}\u{80FD}");
        m.insert("settings.features.copy_on_select", "\u{9009}\u{4E2D}\u{65F6}\u{590D}\u{5236}");

        // Features page - Categories
        m.insert("settings.features.category.general", "\u{901A}\u{7528}");
        m.insert("settings.features.category.session", "\u{4F1A}\u{8BDD}");
        m.insert("settings.features.category.keys", "\u{952E}\u{76D8}");
        m.insert("settings.features.category.text_editing", "\u{6587}\u{672C}\u{7F16}\u{8F91}");
        m.insert("settings.features.category.terminal_input", "\u{7EC8}\u{7AEF}\u{8F93}\u{5165}");
        m.insert("settings.features.category.terminal", "\u{7EC8}\u{7AEF}");
        m.insert("settings.features.category.notifications", "\u{901A}\u{77E5}");
        m.insert("settings.features.category.workflows", "\u{5DE5}\u{4F5C}\u{6D41}");
        m.insert("settings.features.category.system", "\u{7CFB}\u{7EDF}");

        // Features page - General
        m.insert("settings.features.open_links_in_desktop_app", "\u{5728}\u{684C}\u{9762}\u{5E94}\u{7528}\u{4E2D}\u{6253}\u{5F00}\u{94FE}\u{63A5}");
        m.insert("settings.features.open_links_in_desktop_app.description", "\u{5C3D}\u{53EF}\u{80FD}\u{81EA}\u{52A8}\u{5728}\u{684C}\u{9762}\u{5E94}\u{7528}\u{4E2D}\u{6253}\u{5F00}\u{94FE}\u{63A5}\u{3002}");
        m.insert("settings.features.restore_on_startup", "\u{542F}\u{52A8}\u{65F6}\u{6062}\u{590D}\u{7A97}\u{53E3}\u{3001}\u{6807}\u{7B7E}\u{548C}\u{7A97}\u{683C}");
        m.insert("settings.features.wayland_positions_warning", "\u{5728} Wayland \u{4E0A}\u{4E0D}\u{4F1A}\u{6062}\u{590D}\u{7A97}\u{53E3}\u{4F4D}\u{7F6E}\u{3002} ");
        m.insert("settings.features.see_docs", "\u{67E5}\u{770B}\u{6587}\u{6863}\u{3002}");
        m.insert("settings.features.sticky_command_header", "\u{663E}\u{793A}\u{56FA}\u{5B9A}\u{547D}\u{4EE4}\u{5934}");
        m.insert("settings.features.link_tooltip", "\u{70B9}\u{51FB}\u{94FE}\u{63A5}\u{65F6}\u{663E}\u{793A}\u{5DE5}\u{5177}\u{63D0}\u{793A}");
        m.insert("settings.features.quit_warning", "\u{9000}\u{51FA}/\u{767B}\u{51FA}\u{524D}\u{663E}\u{793A}\u{8B66}\u{544A}");
        m.insert("settings.features.login_item_macos", "\u{767B}\u{5F55}\u{65F6}\u{542F}\u{52A8} Warp \u{FF08}\u{9700}\u{8981} macOS 13+\u{FF09}");
        m.insert("settings.features.login_item", "\u{767B}\u{5F55}\u{65F6}\u{542F}\u{52A8} Warp");
        m.insert("settings.features.quit_when_all_closed", "\u{5173}\u{95ED}\u{6240}\u{6709}\u{7A97}\u{53E3}\u{65F6}\u{9000}\u{51FA}");
        m.insert("settings.features.changelog_after_updates", "\u{66F4}\u{65B0}\u{540E}\u{663E}\u{793A}\u{66F4}\u{65B0}\u{65E5}\u{5FD7}\u{63D0}\u{793A}");
        m.insert("settings.features.mouse_scroll_interval", "\u{9F20}\u{6807}\u{6EDA}\u{8F6E}\u{6BCF}\u{6B21}\u{6EDA}\u{52A8}\u{7684}\u{884C}\u{6570}");
        m.insert("settings.features.mouse_scroll_interval.description", "\u{652F}\u{6301} 1 \u{5230} 20 \u{4E4B}\u{95F4}\u{7684}\u{6D6E}\u{70B9}\u{6570}\u{503C}\u{3002}");
        m.insert("settings.features.mouse_scroll_interval.allowed_values", "\u{5141}\u{8BB8}\u{7684}\u{503C}\u{FF1A}1-20");
        m.insert("settings.features.auto_open_code_review", "\u{81EA}\u{52A8}\u{6253}\u{5F00}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}\u{9762}\u{677F}");
        m.insert("settings.features.auto_open_code_review.description", "\u{5F00}\u{542F}\u{540E}\u{FF0C}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}\u{9762}\u{677F}\u{4F1A}\u{5728}\u{5BF9}\u{8BDD}\u{4E2D}\u{7B2C}\u{4E00}\u{4E2A}\u{63A5}\u{53D7}\u{7684}\u{5DEE}\u{5F02}\u{5904}\u{6253}\u{5F00}");
        m.insert("settings.features.warp_is_default_terminal", "Warp \u{662F}\u{9ED8}\u{8BA4}\u{7EC8}\u{7AEF}");
        m.insert("settings.features.make_default_terminal", "\u{5C06} Warp \u{8BBE}\u{4E3A}\u{9ED8}\u{8BA4}\u{7EC8}\u{7AEF}");
        m.insert("settings.features.max_rows", "\u{4EE3}\u{7801}\u{5757}\u{7684}\u{6700}\u{5927}\u{884C}\u{6570}");
        m.insert("settings.features.max_rows.description", "\u{5C06}\u{9650}\u{5236}\u{8BBE}\u{4E3A}\u{8D85}\u{8FC7} 10 \u{4E07}\u{884C}\u{53EF}\u{80FD}\u{4F1A}\u{5F71}\u{54CD}\u{6027}\u{80FD}\u{3002}\u{652F}\u{6301}\u{7684}\u{6700}\u{5927}\u{884C}\u{6570}\u{4E3A} {max_rows}\u{3002}");
        m.insert("settings.features.ssh_wrapper", "Warp SSH \u{5305}\u{88C5}\u{5668}");
        m.insert("settings.features.new_sessions_effect", "\u{6B64}\u{66F4}\u{6539}\u{5C06}\u{5728}\u{65B0}\u{4F1A}\u{8BDD}\u{4E2D}\u{751F}\u{6548}");
        m.insert("settings.features.default", "\u{9ED8}\u{8BA4}");

        // Features page - Notifications
        m.insert("settings.features.desktop_notifications", "\u{63A5}\u{6536} Warp \u{7684}\u{684C}\u{9762}\u{901A}\u{77E5}");
        m.insert("settings.features.notify_agent_task_completed", "\u{4EE3}\u{7406}\u{5B8C}\u{6210}\u{4EFB}\u{52A1}\u{65F6}\u{901A}\u{77E5}");
        m.insert("settings.features.notify_needs_attention", "\u{547D}\u{4EE4}\u{6216}\u{4EE3}\u{7406}\u{9700}\u{8981}\u{60A8}\u{7684}\u{6CE8}\u{610F}\u{4EE5}\u{7EE7}\u{7EED}\u{65F6}\u{901A}\u{77E5}");
        m.insert("settings.features.notification_sounds", "\u{64AD}\u{653E}\u{901A}\u{77E5}\u{58F0}\u{97F3}");
        m.insert("settings.features.in_app_agent_notifications", "\u{663E}\u{793A}\u{5E94}\u{7528}\u{5185}\u{4EE3}\u{7406}\u{901A}\u{77E5}");
        m.insert("settings.features.toast_duration", "\u{63D0}\u{793A}\u{901A}\u{77E5}\u{663E}\u{793A}\u{65F6}\u{957F}");
        m.insert("settings.features.seconds", "\u{79D2}");
        m.insert("settings.features.command_longer_than", "\u{5F53}\u{547D}\u{4EE4}\u{6267}\u{884C}\u{8D85}\u{8FC7}");
        m.insert("settings.features.seconds_to_complete", "\u{79D2}\u{65F6}");

        // Features page - Session
        m.insert("settings.features.default_shell", "\u{65B0}\u{4F1A}\u{8BDD}\u{7684}\u{9ED8}\u{8BA4} Shell");
        m.insert("settings.features.working_directory", "\u{65B0}\u{4F1A}\u{8BDD}\u{7684}\u{5DE5}\u{4F5C}\u{76EE}\u{5F55}");
        m.insert("settings.features.confirm_close_shared", "\u{5173}\u{95ED}\u{5171}\u{4EAB}\u{4F1A}\u{8BDD}\u{524D}\u{786E}\u{8BA4}");
        m.insert("settings.features.new_tab_placement", "\u{65B0}\u{6807}\u{7B7E}\u{4F4D}\u{7F6E}");
        m.insert("settings.features.after_all_tabs", "\u{6240}\u{6709}\u{6807}\u{7B7E}\u{4E4B}\u{540E}");
        m.insert("settings.features.after_current_tab", "\u{5F53}\u{524D}\u{6807}\u{7B7E}\u{4E4B}\u{540E}");
        m.insert("settings.features.default_session_mode", "\u{65B0}\u{4F1A}\u{8BDD}\u{7684}\u{9ED8}\u{8BA4}\u{6A21}\u{5F0F}");
        m.insert("settings.features.global_workflows", "\u{5728}\u{547D}\u{4EE4}\u{641C}\u{7D22}\u{4E2D}\u{663E}\u{793A}\u{5168}\u{5C40}\u{5DE5}\u{4F5C}\u{6D41} (ctrl-r)");

        // Features page - Keys
        m.insert("settings.features.global_hotkey", "\u{5168}\u{5C40}\u{5FEB}\u{6377}\u{952E}\u{FF1A}");
        m.insert("settings.features.configure_global_hotkey", "\u{914D}\u{7F6E}\u{5168}\u{5C40}\u{5FEB}\u{6377}\u{952E}");
        m.insert("settings.features.wayland_not_supported", "\u{5728} Wayland \u{4E0A}\u{4E0D}\u{652F}\u{6301}\u{3002} ");
        m.insert("settings.features.keybinding", "\u{952E}\u{76D8}\u{7ED1}\u{5B9A}");
        m.insert("settings.features.click_to_set_hotkey", "\u{70B9}\u{51FB}\u{8BBE}\u{7F6E}\u{5168}\u{5C40}\u{5FEB}\u{6377}\u{952E}");
        m.insert("settings.features.press_new_shortcut", "\u{6309}\u{4E0B}\u{65B0}\u{7684}\u{952E}\u{76D8}\u{5FEB}\u{6377}\u{952E}");
        m.insert("settings.features.change_keybinding", "\u{66F4}\u{6539}\u{952E}\u{76D8}\u{7ED1}\u{5B9A}");
        m.insert("settings.features.pin_to_top", "\u{56FA}\u{5B9A}\u{5230}\u{9876}\u{90E8}");
        m.insert("settings.features.pin_to_bottom", "\u{56FA}\u{5B9A}\u{5230}\u{5E95}\u{90E8}");
        m.insert("settings.features.pin_to_left", "\u{56FA}\u{5B9A}\u{5230}\u{5DE6}\u{4FA7}");
        m.insert("settings.features.pin_to_right", "\u{56FA}\u{5B9A}\u{5230}\u{53F3}\u{4FA7}");
        m.insert("settings.features.active_screen", "\u{6D3B}\u{52A8}\u{5C4F}\u{5E55}");
        m.insert("settings.features.width_percent", "\u{5BBD}\u{5EA6} %");
        m.insert("settings.features.height_percent", "\u{9AD8}\u{5EA6} %");
        m.insert("settings.features.autohide_keyboard_focus", "\u{5931}\u{53BB}\u{952E}\u{76D8}\u{7126}\u{70B9}\u{65F6}\u{81EA}\u{52A8}\u{9690}\u{85CF}");
        m.insert("settings.features.meta_key_left.option", "\u{5DE6} Option \u{952E}\u{4E3A} Meta \u{952E}");
        m.insert("settings.features.meta_key_right.option", "\u{53F3} Option \u{952E}\u{4E3A} Meta \u{952E}");
        m.insert("settings.features.meta_key_left.alt", "\u{5DE6} Alt \u{952E}\u{4E3A} Meta \u{952E}");
        m.insert("settings.features.meta_key_right.alt", "\u{53F3} Alt \u{952E}\u{4E3A} Meta \u{952E}");

        // Features page - Text Editing
        m.insert("settings.features.autocomplete_symbols", "\u{81EA}\u{52A8}\u{8865}\u{5168}\u{5F15}\u{53F7}\u{3001}\u{62EC}\u{53F7}\u{548C}\u{65B9}\u{62EC}\u{53F7}");
        m.insert("settings.features.error_underlining", "\u{547D}\u{4EE4}\u{9519}\u{8BEF}\u{4E0B}\u{5212}\u{7EBF}");
        m.insert("settings.features.syntax_highlighting", "\u{547D}\u{4EE4}\u{8BED}\u{6CD5}\u{9AD8}\u{4EAE}");
        m.insert("settings.features.completions_while_typing", "\u{8F93}\u{5165}\u{65F6}\u{6253}\u{5F00}\u{8865}\u{5168}\u{83DC}\u{5355}");
        m.insert("settings.features.suggest_corrections", "\u{5EFA}\u{8BAE}\u{7EA0}\u{6B63}\u{547D}\u{4EE4}");
        m.insert("settings.features.expand_aliases", "\u{8F93}\u{5165}\u{65F6}\u{5C55}\u{5F00}\u{522B}\u{540D}");
        m.insert("settings.features.middle_click_paste", "\u{4E2D}\u{952E}\u{70B9}\u{51FB}\u{7C98}\u{8D34}");
        m.insert("settings.features.vim_mode", "\u{4F7F}\u{7528} Vim \u{952E}\u{76D8}\u{7ED1}\u{5B9A}\u{7F16}\u{8F91}\u{4EE3}\u{7801}\u{548C}\u{547D}\u{4EE4}");
        m.insert("settings.features.vim_unnamed_clipboard", "\u{5C06}\u{672A}\u{547D}\u{540D}\u{5BC4}\u{5B58}\u{5668}\u{8BBE}\u{4E3A}\u{7CFB}\u{7EDF}\u{526A}\u{8D34}\u{677F}");
        m.insert("settings.features.vim_status_bar", "\u{663E}\u{793A} Vim \u{72B6}\u{6001}\u{680F}");

        // Features page - Terminal Input
        m.insert("settings.features.at_context_menu", "\u{5728}\u{7EC8}\u{7AEF}\u{6A21}\u{5F0F}\u{4E2D}\u{542F}\u{7528} '@' \u{4E0A}\u{4E0B}\u{6587}\u{83DC}\u{5355}");
        m.insert("settings.features.slash_commands", "\u{5728}\u{7EC8}\u{7AEF}\u{6A21}\u{5F0F}\u{4E2D}\u{542F}\u{7528}\u{659C}\u{6760}\u{547D}\u{4EE4}");
        m.insert("settings.features.outline_codebase_symbols", "\u{4E3A} '@' \u{4E0A}\u{4E0B}\u{6587}\u{83DC}\u{5355}\u{5927}\u{7EB2}\u{5316}\u{4EE3}\u{7801}\u{5E93}\u{7B26}\u{53F7}");
        m.insert("settings.features.terminal_input_message", "\u{663E}\u{793A}\u{7EC8}\u{7AEF}\u{8F93}\u{5165}\u{6D88}\u{606F}\u{884C}");
        m.insert("settings.features.autosuggestion_keybinding_hint", "\u{663E}\u{793A}\u{81EA}\u{52A8}\u{5EFA}\u{8BAE}\u{952E}\u{76D8}\u{7ED1}\u{5B9A}\u{63D0}\u{793A}");
        m.insert("settings.features.autosuggestion_ignore_button", "\u{663E}\u{793A}\u{81EA}\u{52A8}\u{5EFA}\u{8BAE}\u{5FFD}\u{7565}\u{6309}\u{94AE}");
        m.insert("settings.features.tab_key_behavior", "Tab \u{952E}\u{884C}\u{4E3A}");
        m.insert("settings.features.ctrl_tab_behavior", "Ctrl+Tab \u{884C}\u{4E3A}\u{FF1A}");
        m.insert("settings.features.arrow_accepts_autosuggestions", "\u{2192} \u{63A5}\u{53D7}\u{81EA}\u{52A8}\u{5EFA}\u{8BAE}\u{3002}");
        m.insert("settings.features.keystroke_accepts_autosuggestions", "{keystroke} \u{63A5}\u{53D7}\u{81EA}\u{52A8}\u{5EFA}\u{8BAE}\u{3002}");
        m.insert("settings.features.completions_open_as_you_type", "\u{8F93}\u{5165}\u{65F6}\u{6253}\u{5F00}\u{8865}\u{5168}\u{3002}");
        m.insert("settings.features.completions_open_as_you_type_or", "\u{8F93}\u{5165}\u{65F6}\u{6253}\u{5F00}\u{8865}\u{5168}\u{FF08}\u{6216} {keystroke}\u{FF09}\u{3002}");
        m.insert("settings.features.completion_menu_unbound", "\u{6253}\u{5F00}\u{8865}\u{5168}\u{83DC}\u{5355}\u{672A}\u{7ED1}\u{5B9A}\u{3002}");
        m.insert("settings.features.keystroke_opens_completion_menu", "{keystroke} \u{6253}\u{5F00}\u{8865}\u{5168}\u{83DC}\u{5355}\u{3002}");
        m.insert("settings.features.accept_autosuggestion", "\u{63A5}\u{53D7}\u{81EA}\u{52A8}\u{5EFA}\u{8BAE}");
        m.insert("settings.features.open_completions_menu", "\u{6253}\u{5F00}\u{8865}\u{5168}\u{83DC}\u{5355}");
        m.insert("settings.features.word_char_config", "\u{88AB}\u{89C6}\u{4E3A}\u{5355}\u{8BCD}\u{4E00}\u{90E8}\u{5206}\u{7684}\u{5B57}\u{7B26}");

        // Features page - Terminal
        m.insert("settings.features.mouse_reporting", "\u{542F}\u{7528}\u{9F20}\u{6807}\u{62A5}\u{544A}");
        m.insert("settings.features.scroll_reporting", "\u{542F}\u{7528}\u{6EDA}\u{52A8}\u{62A5}\u{544A}");
        m.insert("settings.features.focus_reporting", "\u{542F}\u{7528}\u{7126}\u{70B9}\u{62A5}\u{544A}");
        m.insert("settings.features.audible_bell", "\u{4F7F}\u{7528}\u{58F0}\u{97F3}\u{63D0}\u{793A}");
        m.insert("settings.features.smart_selection", "\u{53CC}\u{51FB}\u{667A}\u{80FD}\u{9009}\u{62E9}");
        m.insert("settings.features.show_help_block", "\u{5728}\u{65B0}\u{4F1A}\u{8BDD}\u{4E2D}\u{663E}\u{793A}\u{5E2E}\u{52A9}\u{4EE3}\u{7801}\u{5757}");
        m.insert("settings.features.linux_selection_clipboard", "\u{9075}\u{5B88} Linux \u{9009}\u{62E9}\u{526A}\u{8D34}\u{677F}");
        m.insert("settings.features.linux_selection_clipboard.description", "\u{662F}\u{5426}\u{5E94}\u{652F}\u{6301} Linux \u{4E3B}\u{526A}\u{8D34}\u{677F}\u{3002}");

        // Features page - System
        m.insert("settings.features.prefer_low_power_gpu", "\u{4F18}\u{5148}\u{4F7F}\u{7528}\u{96C6}\u{6210} GPU \u{6E32}\u{67D3}\u{65B0}\u{7A97}\u{53E3}\u{FF08}\u{4F4E}\u{529F}\u{8017}\u{FF09}");
        m.insert("settings.features.changes_new_windows", "\u{66F4}\u{6539}\u{5C06}\u{5E94}\u{7528}\u{4E8E}\u{65B0}\u{7A97}\u{53E3}\u{3002}");
        m.insert("settings.features.wayland_window_management", "\u{4F7F}\u{7528} Wayland \u{8FDB}\u{884C}\u{7A97}\u{53E3}\u{7BA1}\u{7406}");
        m.insert("settings.features.wayland_window_management.description", "\u{542F}\u{7528} Wayland \u{7684}\u{4F7F}\u{7528}");
        m.insert("settings.features.wayland_hotkey_warning", "\u{542F}\u{7528}\u{6B64}\u{8BBE}\u{7F6E}\u{4F1A}\u{7981}\u{7528}\u{5168}\u{5C40}\u{5FEB}\u{6377}\u{952E}\u{652F}\u{6301}\u{3002}\u{7981}\u{7528}\u{65F6}\u{FF0C}\u{5982}\u{679C}\u{60A8}\u{7684} Wayland \u{5408}\u{6210}\u{5668}\u{4F7F}\u{7528}\u{5206}\u{6570}\u{7F29}\u{653E}\u{FF08}\u{5982}\u{FF1A}125%\u{FF09}\u{FF0C}\u{6587}\u{672C}\u{53EF}\u{80FD}\u{4F1A}\u{6A21}\u{7CCA}\u{3002}");
        m.insert("settings.features.restart_warp_effect", "\u{91CD}\u{542F} Warp \u{4EE5}\u{4F7F}\u{66F4}\u{6539}\u{751F}\u{6548}\u{3002}");
        m.insert("settings.features.preferred_graphics_backend", "\u{9996}\u{9009}\u{56FE}\u{5F62}\u{540E}\u{7AEF}");
        m.insert("settings.features.current_backend", "\u{5F53}\u{524D}\u{540E}\u{7AEF}\u{FF1A}{backend}");

        // AI page
        m.insert("settings.ai", "AI");
        m.insert("settings.ai.warp_agent", "Warp \u{4EE3}\u{7406}");
        m.insert("settings.ai.active_ai", "\u{6D3B}\u{8DC3} AI");
        m.insert("settings.ai.usage", "\u{7528}\u{91CF}");
        m.insert("settings.ai.credits", "\u{79EF}\u{5206}");
        m.insert("settings.ai.unlimited", "\u{65E0}\u{9650}");
        m.insert("settings.ai.restricted_billing", "\u{56E0}\u{8BA1}\u{8D39}\u{95EE}\u{9898}\u{88AB}\u{9650}\u{5236}");
        m.insert("settings.ai.resets", "\u{91CD}\u{7F6E}\u{65F6}\u{95F4} {formatted_next_refresh_time}");
        m.insert("settings.ai.credits_limit_description", "\u{8FD9}\u{662F}\u{60A8}\u{8D26}\u{6237}\u{7684} {0} AI \u{79EF}\u{5206}\u{4E0A}\u{9650}\u{3002}");
        m.insert("settings.ai.upgrade", "\u{5347}\u{7EA7}");
        m.insert("settings.ai.get_more_usage", "\u{4EE5}\u{83B7}\u{53D6}\u{66F4}\u{591A} AI \u{7528}\u{91CF}\u{3002}");
        m.insert("settings.ai.compare_plans", "\u{6BD4}\u{8F83}\u{5957}\u{9910}");
        m.insert("settings.ai.more_usage", "\u{4EE5}\u{83B7}\u{53D6}\u{66F4}\u{591A} AI \u{7528}\u{91CF}\u{3002}");
        m.insert("settings.ai.contact_support", "\u{8054}\u{7CFB}\u{652F}\u{6301}");
        m.insert("settings.ai.contact_sales", "\u{8054}\u{7CFB}\u{9500}\u{552E}");
        m.insert("settings.ai.enable_byo_enterprise", "\u{4EE5}\u{5728}\u{60A8}\u{7684}\u{4F01}\u{4E1A}\u{7248}\u{5957}\u{9910}\u{4E0A}\u{542F}\u{7528}\u{81EA}\u{5E26} API \u{5BC6}\u{94A5}\u{3002}");
        m.insert("settings.ai.upgrade_build_plan", "\u{5347}\u{7EA7}\u{5230} Build \u{5957}\u{9910}");
        m.insert("settings.ai.use_own_api_keys", "\u{4EE5}\u{4F7F}\u{7528}\u{60A8}\u{81EA}\u{5DF1}\u{7684} API \u{5BC6}\u{94A5}\u{3002}");
        m.insert("settings.ai.ask_admin_upgrade", "\u{8BF7}\u{8BA9}\u{60A8}\u{7684}\u{56E2}\u{961F}\u{7BA1}\u{7406}\u{5458}\u{5347}\u{7EA7}\u{5230} Build \u{5957}\u{9910}\u{4EE5}\u{4F7F}\u{7528}\u{60A8}\u{81EA}\u{5DF1}\u{7684} API \u{5BC6}\u{94A5}\u{3002}");
        m.insert("settings.ai.org_disallows_remote_ai", "\u{60A8}\u{7684}\u{7EC4}\u{7EC7}\u{7981}\u{6B62}\u{5728}\u{6D3B}\u{52A8}\u{7A97}\u{683C}\u{5305}\u{542B}\u{8FDC}\u{7A0B}\u{4F1A}\u{8BDD}\u{5185}\u{5BB9}\u{65F6}\u{4F7F}\u{7528} AI");
        m.insert("settings.ai.create_account_prompt", "\u{8981}\u{4F7F}\u{7528} AI \u{529F}\u{80FD}\u{FF0C}\u{8BF7}\u{521B}\u{5EFA}\u{8D26}\u{6237}\u{3002}");
        m.insert("settings.ai.org_enforced_setting", "\u{6B64}\u{9009}\u{9879}\u{7531}\u{60A8}\u{7684}\u{7EC4}\u{7EC7}\u{8BBE}\u{7F6E}\u{5F3A}\u{5236}\u{6267}\u{884C}\u{FF0C}\u{65E0}\u{6CD5}\u{81EA}\u{5B9A}\u{4E49}\u{3002}");
        m.insert("settings.ai.learn_more", "\u{4E86}\u{89E3}\u{66F4}\u{591A}");

        // AI page - Agents section
        m.insert("settings.ai.agents", "\u{4EE3}\u{7406}");
        m.insert("settings.ai.agents_description", "\u{8BBE}\u{7F6E}\u{4EE3}\u{7406}\u{7684}\u{8FD0}\u{4F5C}\u{8FB9}\u{754C}\u{3002}\u{9009}\u{62E9}\u{5B83}\u{53EF}\u{4EE5}\u{8BBF}\u{95EE}\u{4EC0}\u{4E48}\u{FF0C}\u{5B83}\u{6709}\u{591A}\u{5C11}\u{81EA}\u{4E3B}\u{6743}\u{FF0C}\u{4EE5}\u{53CA}\u{4F55}\u{65F6}\u{5FC5}\u{987B}\u{5F81}\u{6C42}\u{60A8}\u{7684}\u{6279}\u{51C6}\u{3002}\u{60A8}\u{8FD8}\u{53EF}\u{4EE5}\u{5BF9}\u{81EA}\u{7136}\u{8BED}\u{8A00}\u{8F93}\u{5165}\u{3001}\u{4EE3}\u{7801}\u{5E93}\u{611F}\u{77E5}\u{7B49}\u{884C}\u{4E3A}\u{8FDB}\u{884C}\u{7EC6}\u{5316}\u{8C03}\u{6574}\u{3002}");
        m.insert("settings.ai.profiles", "\u{914D}\u{7F6E}\u{6587}\u{4EF6}");
        m.insert("settings.ai.profiles_description", "\u{914D}\u{7F6E}\u{6587}\u{4EF6}\u{8BA9}\u{60A8}\u{5B9A}\u{4E49}\u{4EE3}\u{7406}\u{7684}\u{8FD0}\u{4F5C}\u{65B9}\u{5F0F} \u{2014} \u{4ECE}\u{5B83}\u{53EF}\u{4EE5}\u{6267}\u{884C}\u{7684}\u{64CD}\u{4F5C}\u{548C}\u{4F55}\u{65F6}\u{9700}\u{8981}\u{6279}\u{51C6}\u{FF0C}\u{5230}\u{5B83}\u{7528}\u{4E8E}\u{7F16}\u{7801}\u{548C}\u{89C4}\u{5212}\u{7B49}\u{4EFB}\u{52A1}\u{7684}\u{6A21}\u{578B}\u{3002}\u{60A8}\u{8FD8}\u{53EF}\u{4EE5}\u{5C06}\u{5B83}\u{4EEC}\u{4F5C}\u{7528}\u{4E8E}\u{5355}\u{4E2A}\u{9879}\u{76EE}\u{3002}");
        m.insert("settings.ai.add_profile", "\u{6DFB}\u{52A0}\u{914D}\u{7F6E}\u{6587}\u{4EF6}");
        m.insert("settings.ai.models", "\u{6A21}\u{578B}");
        m.insert("settings.ai.permissions", "\u{6743}\u{9650}");

        // AI page - Permissions
        m.insert("settings.ai.apply_code_diffs", "\u{5E94}\u{7528}\u{4EE3}\u{7801}\u{5DEE}\u{5F02}");
        m.insert("settings.ai.read_files", "\u{8BFB}\u{53D6}\u{6587}\u{4EF6}");
        m.insert("settings.ai.execute_commands", "\u{6267}\u{884C}\u{547D}\u{4EE4}");
        m.insert("settings.ai.interact_with_running_commands", "\u{4E0E}\u{8FD0}\u{884C}\u{4E2D}\u{7684}\u{547D}\u{4EE4}\u{4EA4}\u{4E92}");
        m.insert("settings.ai.workspace_managed_permissions", "\u{60A8}\u{7684}\u{90E8}\u{5206}\u{6743}\u{9650}\u{7531}\u{5DE5}\u{4F5C}\u{533A}\u{7BA1}\u{7406}\u{3002}");
        m.insert("settings.ai.call_mcp_servers", "\u{8C03}\u{7528} MCP \u{670D}\u{52A1}\u{5668}");
        m.insert("settings.ai.command_denylist", "\u{547D}\u{4EE4}\u{9ED1}\u{540D}\u{5355}");
        m.insert("settings.ai.command_denylist.description", "\u{7528}\u{4E8E}\u{5339}\u{914D}\u{547D}\u{4EE4}\u{7684}\u{6B63}\u{5219}\u{8868}\u{8FBE}\u{5F0F}\u{FF0C} Warp \u{4EE3}\u{7406}\u{59CB}\u{7EC8}\u{5E94}\u{8BE5}\u{8BF7}\u{6C42}\u{6267}\u{884C}\u{8BB8}\u{53EF}\u{3002}");
        m.insert("settings.ai.command_allowlist", "\u{547D}\u{4EE4}\u{767D}\u{540D}\u{5355}");
        m.insert("settings.ai.command_allowlist.description", "\u{7528}\u{4E8E}\u{5339}\u{914D}\u{53EF}\u{4EE5}\u{7531} Warp \u{4EE3}\u{7406}\u{81EA}\u{52A8}\u{6267}\u{884C}\u{7684}\u{547D}\u{4EE4}\u{7684}\u{6B63}\u{5219}\u{8868}\u{8FBE}\u{5F0F}\u{3002}");
        m.insert("settings.ai.directory_allowlist", "\u{76EE}\u{5F55}\u{767D}\u{540D}\u{5355}");
        m.insert("settings.ai.directory_allowlist.description", "\u{5141}\u{8BB8}\u{4EE3}\u{7406}\u{8BBF}\u{95EE}\u{7279}\u{5B9A}\u{76EE}\u{5F55}\u{3002}");
        m.insert("settings.ai.mcp_allowlist", "MCP \u{767D}\u{540D}\u{5355}");
        m.insert("settings.ai.mcp_allowlist.description", "\u{5141}\u{8BB8} Warp \u{4EE3}\u{7406}\u{8C03}\u{7528}\u{8FD9}\u{4E9B} MCP \u{670D}\u{52A1}\u{5668}\u{3002}");
        m.insert("settings.ai.mcp_denylist", "MCP \u{9ED1}\u{540D}\u{5355}");
        m.insert("settings.ai.mcp_denylist.description", "Warp \u{4EE3}\u{7406}\u{5728}\u{8C03}\u{7528}\u{6B64}\u{5217}\u{8868}\u{4E0A}\u{7684}\u{4EFB}\u{4F55} MCP \u{670D}\u{52A1}\u{5668}\u{4E4B}\u{524D}\u{59CB}\u{7EC8}\u{4F1A}\u{8BF7}\u{6C42}\u{8BB8}\u{53EF}\u{3002}");
        m.insert("settings.ai.mcp_zero_state_description", "\u{60A8}\u{8FD8}\u{6CA1}\u{6709}\u{6DFB}\u{52A0}\u{4EFB}\u{4F55} MCP \u{670D}\u{52A1}\u{5668}\u{3002}\u{6DFB}\u{52A0}\u{540E}\u{FF0C}\u{60A8}\u{5C06}\u{80FD}\u{591F}\u{63A7}\u{5236} Warp \u{4EE3}\u{7406}\u{4E0E}\u{5B83}\u{4EEC}\u{4EA4}\u{4E92}\u{65F6}\u{7684}\u{81EA}\u{4E3B}\u{6743}\u{3002} ");
        m.insert("settings.ai.add_a_server", "\u{6DFB}\u{52A0}\u{670D}\u{52A1}\u{5668}");
        m.insert("settings.ai.or", "\u{6216} ");
        m.insert("settings.ai.learn_more_mcps", "\u{4E86}\u{89E3}\u{66F4}\u{591A}\u{5173}\u{4E8E} MCP \u{7684}\u{4FE1}\u{606F}\u{3002}");
        m.insert("settings.ai.show_model_picker_in_prompt", "\u{5728}\u{63D0}\u{793A}\u{7B26}\u{4E2D}\u{663E}\u{793A}\u{6A21}\u{578B}\u{9009}\u{62E9}\u{5668}");
        m.insert("settings.ai.base_model", "\u{57FA}\u{7840}\u{6A21}\u{578B}");
        m.insert("settings.ai.base_model.description", "\u{6B64}\u{6A21}\u{578B}\u{4F5C}\u{4E3A} Warp \u{4EE3}\u{7406}\u{7684}\u{4E3B}\u{8981}\u{5F15}\u{64CE}\u{3002}\u{5B83}\u{4E3B}\u{8981}\u{7528}\u{4E8E}\u{5927}\u{591A}\u{6570}\u{4EA4}\u{4E92}\u{FF0C}\u{5E76}\u{5728}\u{5FC5}\u{8981}\u{65F6}\u{8C03}\u{7528}\u{5176}\u{4ED6}\u{6A21}\u{578B}\u{6765}\u{5904}\u{7406}\u{89C4}\u{5212}\u{6216}\u{4EE3}\u{7801}\u{751F}\u{6210}\u{7B49}\u{4EFB}\u{52A1}\u{3002}Warp \u{53EF}\u{80FD}\u{4F1A}\u{6839}\u{636E}\u{6A21}\u{578B}\u{53EF}\u{7528}\u{6027}\u{6216}\u{8F85}\u{52A9}\u{4EFB}\u{52A1}\u{81EA}\u{52A8}\u{5207}\u{6362}\u{5230}\u{66FF}\u{4EE3}\u{6A21}\u{578B}\u{3002}");
        m.insert("settings.ai.codebase_context", "\u{4EE3}\u{7801}\u{5E93}\u{4E0A}\u{4E0B}\u{6587}");
        m.insert("settings.ai.codebase_context.description", "\u{5141}\u{8BB8} Warp \u{4EE3}\u{7406}\u{751F}\u{6210}\u{4EE3}\u{7801}\u{5E93}\u{7684}\u{5927}\u{7EB2}\u{4EE5}\u{7528}\u{4E8E}\u{4E0A}\u{4E0B}\u{6587}\u{3002}\u{4EE3}\u{7801}\u{6C38}\u{8FDC}\u{4E0D}\u{4F1A}\u{5B58}\u{50A8}\u{5728}\u{6211}\u{4EEC}\u{7684}\u{670D}\u{52A1}\u{5668}\u{4E0A}\u{3002} ");
        m.insert("settings.ai.toolbar_layout", "\u{5DE5}\u{5177}\u{680F}\u{5E03}\u{5C40}");

        // AI page - Permission options
        m.insert("settings.ai.option.agent_decides", "\u{4EE3}\u{7406}\u{51B3}\u{5B9A}");
        m.insert("settings.ai.option.always_allow", "\u{59CB}\u{7EC8}\u{5141}\u{8BB8}");
        m.insert("settings.ai.option.always_ask", "\u{59CB}\u{7EC8}\u{8BE2}\u{95EE}");
        m.insert("settings.ai.option.ask_on_first_write", "\u{9996}\u{6B21}\u{5199}\u{5165}\u{65F6}\u{8BE2}\u{95EE}");
        m.insert("settings.ai.option.read_only", "\u{53EA}\u{8BFB}");
        m.insert("settings.ai.option.supervised", "\u{76D1}\u{7763}");
        m.insert("settings.ai.option.allow_in_specific_directories", "\u{5141}\u{8BB8}\u{5728}\u{7279}\u{5B9A}\u{76EE}\u{5F55}\u{4E2D}");
        m.insert("settings.ai.option.new_tab", "\u{65B0}\u{6807}\u{7B7E}\u{9875}");
        m.insert("settings.ai.option.split_pane", "\u{62C6}\u{5206}\u{7A97}\u{683C}");
        m.insert("settings.ai.select_mcp_servers", "\u{9009}\u{62E9} MCP \u{670D}\u{52A1}\u{5668}");
        m.insert("settings.ai.select_coding_agent", "\u{9009}\u{62E9}\u{7F16}\u{7801}\u{4EE3}\u{7406}");

        // AI page - Active AI section
        m.insert("settings.ai.next_command", "\u{4E0B}\u{4E00}\u{6761}\u{547D}\u{4EE4}");
        m.insert("settings.ai.next_command.description", "\u{8BA9} AI \u{6839}\u{636E}\u{60A8}\u{7684}\u{547D}\u{4EE4}\u{5386}\u{53F2}\u{3001}\u{8F93}\u{51FA}\u{548C}\u{5E38}\u{89C1}\u{5DE5}\u{4F5C}\u{6D41}\u{7A0B}\u{5EFA}\u{8BAE}\u{4E0B}\u{4E00}\u{6761}\u{8981}\u{8FD0}\u{884C}\u{7684}\u{547D}\u{4EE4}\u{3002}");
        m.insert("settings.ai.prompt_suggestions", "\u{63D0}\u{793A}\u{5EFA}\u{8BAE}");
        m.insert("settings.ai.prompt_suggestions.description", "\u{8BA9} AI \u{6839}\u{636E}\u{6700}\u{8FD1}\u{7684}\u{547D}\u{4EE4}\u{53CA}\u{5176}\u{8F93}\u{51FA}\u{FF0C}\u{5728}\u{8F93}\u{5165}\u{4E2D}\u{4EE5}\u{5185}\u{8054}\u{6807}\u{8BED}\u{7684}\u{5F62}\u{5F0F}\u{5EFA}\u{8BAE}\u{81EA}\u{7136}\u{8BED}\u{8A00}\u{63D0}\u{793A}\u{3002}");
        m.insert("settings.ai.suggested_code_banners", "\u{4EE3}\u{7801}\u{5EFA}\u{8BAE}\u{6807}\u{8BED}");
        m.insert("settings.ai.suggested_code_banners.description", "\u{8BA9} AI \u{6839}\u{636E}\u{6700}\u{8FD1}\u{7684}\u{547D}\u{4EE4}\u{53CA}\u{5176}\u{8F93}\u{51FA}\u{FF0C}\u{5728}\u{9ED1}\u{540D}\u{5355}\u{4E2D}\u{4EE5}\u{5185}\u{8054}\u{6807}\u{8BED}\u{7684}\u{5F62}\u{5F0F}\u{5EFA}\u{8BAE}\u{4EE3}\u{7801}\u{5DEE}\u{5F02}\u{548C}\u{67E5}\u{8BE2}\u{3002}");
        m.insert("settings.ai.natural_language_autosuggestions", "\u{81EA}\u{7136}\u{8BED}\u{8A00}\u{81EA}\u{52A8}\u{5EFA}\u{8BAE}");
        m.insert("settings.ai.natural_language_autosuggestions.description", "\u{8BA9} AI \u{6839}\u{636E}\u{6700}\u{8FD1}\u{7684}\u{547D}\u{4EE4}\u{53CA}\u{5176}\u{8F93}\u{51FA}\u{5EFA}\u{8BAE}\u{81EA}\u{7136}\u{8BED}\u{8A00}\u{81EA}\u{52A8}\u{5B8C}\u{6210}\u{3002}");
        m.insert("settings.ai.shared_block_title_generation", "\u{5171}\u{4EAB}\u{5757}\u{6807}\u{9898}\u{751F}\u{6210}");
        m.insert("settings.ai.shared_block_title_generation.description", "\u{8BA9} AI \u{6839}\u{636E}\u{547D}\u{4EE4}\u{548C}\u{8F93}\u{51FA}\u{4E3A}\u{60A8}\u{7684}\u{5171}\u{4EAB}\u{5757}\u{751F}\u{6210}\u{6807}\u{9898}\u{3002}");
        m.insert("settings.ai.commit_pull_request_generation", "\u{63D0}\u{4EA4}\u{548C}\u{62C9}\u{53D6}\u{8BF7}\u{6C42}\u{751F}\u{6210}");
        m.insert("settings.ai.git_operations_autogen.description", "\u{8BA9} AI \u{751F}\u{6210}\u{63D0}\u{4EA4}\u{4FE1}\u{606F}\u{3001}\u{62C9}\u{53D6}\u{8BF7}\u{6C42}\u{6807}\u{9898}\u{548C}\u{63CF}\u{8FF0}\u{3002}");

        // AI page - Input section
        m.insert("settings.ai.input", "\u{8F93}\u{5165}");
        m.insert("settings.ai.show_input_hint_text", "\u{663E}\u{793A}\u{8F93}\u{5165}\u{63D0}\u{793A}\u{6587}\u{672C}");
        m.insert("settings.ai.show_agent_tips", "\u{663E}\u{793A}\u{4EE3}\u{7406}\u{63D0}\u{793A}");
        m.insert("settings.ai.include_agent_commands_in_history", "\u{5728}\u{5386}\u{53F2}\u{8BB0}\u{5F55}\u{4E2D}\u{5305}\u{542B}\u{4EE3}\u{7406}\u{6267}\u{884C}\u{7684}\u{547D}\u{4EE4}");
        m.insert("settings.ai.natural_language_detection", "\u{81EA}\u{7136}\u{8BED}\u{8A00}\u{68C0}\u{6D4B}");
        m.insert("settings.ai.autodetect_agent_prompts", "\u{81EA}\u{52A8}\u{68C0}\u{6D4B}\u{7EC8}\u{7AEF}\u{8F93}\u{5165}\u{4E2D}\u{7684}\u{4EE3}\u{7406}\u{63D0}\u{793A}");
        m.insert("settings.ai.autodetect_terminal_commands", "\u{81EA}\u{52A8}\u{68C0}\u{6D4B}\u{4EE3}\u{7406}\u{8F93}\u{5165}\u{4E2D}\u{7684}\u{7EC8}\u{7AEF}\u{547D}\u{4EE4}");
        m.insert("settings.ai.incorrect_detection", "\u{68C0}\u{6D4B}\u{5230}\u{4E0D}\u{6B63}\u{786E}\u{7684}\u{68C0}\u{6D4B}\u{FF1F} ");
        m.insert("settings.ai.incorrect_input_detection", " \u{68C0}\u{6D4B}\u{5230}\u{4E0D}\u{6B63}\u{786E}\u{7684}\u{8F93}\u{5165}\u{68C0}\u{6D4B}\u{FF1F} ");
        m.insert("settings.ai.let_us_know", "\u{544A}\u{8BC9}\u{6211}\u{4EEC}");
        m.insert("settings.ai.nld_description", "\u{542F}\u{7528}\u{81EA}\u{7136}\u{8BED}\u{8A00}\u{68C0}\u{6D4B}\u{5C06}\u{5728}\u{7EC8}\u{7AEF}\u{8F93}\u{5165}\u{4E2D}\u{68C0}\u{6D4B}\u{81EA}\u{7136}\u{8BED}\u{8A00}\u{FF0C}\u{7136}\u{540E}\u{81EA}\u{52A8}\u{5207}\u{6362}\u{5230}\u{4EE3}\u{7406}\u{6A21}\u{5F0F}\u{8FDB}\u{884C} AI \u{67E5}\u{8BE2}\u{3002}");
        m.insert("settings.ai.natural_language_denylist", "\u{81EA}\u{7136}\u{8BED}\u{8A00}\u{9ED1}\u{540D}\u{5355}");
        m.insert("settings.ai.natural_language_denylist.description", "\u{6B64}\u{5904}\u{5217}\u{51FA}\u{7684}\u{547D}\u{4EE4}\u{6C38}\u{8FDC}\u{4E0D}\u{4F1A}\u{89E6}\u{53D1}\u{81EA}\u{7136}\u{8BED}\u{8A00}\u{68C0}\u{6D4B}\u{3002}");

        // AI page - MCP Servers section
        m.insert("settings.ai.mcp_servers", "MCP \u{670D}\u{52A1}\u{5668}");
        m.insert("settings.ai.mcp_description", "\u{6DFB}\u{52A0} MCP \u{670D}\u{52A1}\u{5668}\u{4EE5}\u{6269}\u{5C55} Warp \u{4EE3}\u{7406}\u{7684}\u{529F}\u{80FD}\u{3002}MCP \u{670D}\u{52A1}\u{5668}\u{901A}\u{8FC7}\u{6807}\u{51C6}\u{5316}\u{63A5}\u{53E3}\u{5411}\u{4EE3}\u{7406}\u{66B4}\u{9732}\u{6570}\u{636E}\u{6E90}\u{6216}\u{5DE5}\u{5177}\u{FF0C}\u{672C}\u{8D28}\u{4E0A}\u{5C31}\u{50CF}\u{63D2}\u{4EF6}\u{3002} ");
        m.insert("settings.ai.auto_spawn_mcp_servers", "\u{81EA}\u{52A8}\u{4ECE}\u{7B2C}\u{4E09}\u{65B9}\u{4EE3}\u{7406}\u{751F}\u{6210}\u{670D}\u{52A1}\u{5668}");
        m.insert("settings.ai.auto_spawn_mcp.description", "\u{81EA}\u{52A8}\u{68C0}\u{6D4B}\u{5E76}\u{4ECE}\u{5168}\u{5C40}\u{7B2C}\u{4E09}\u{65B9} AI \u{4EE3}\u{7406}\u{914D}\u{7F6E}\u{6587}\u{4EF6}\u{FF08}\u{5982}\u{60A8}\u{7684}\u{4E3B}\u{76EE}\u{5F55}\u{4E2D}\u{7684}\u{6587}\u{4EF6}\u{FF09}\u{751F}\u{6210} MCP \u{670D}\u{52A1}\u{5668}\u{3002}\u{5728}\u{4ED3}\u{5E93}\u{5185}\u{68C0}\u{6D4B}\u{5230}\u{7684}\u{670D}\u{52A1}\u{5668}\u{6C38}\u{8FDC}\u{4E0D}\u{4F1A}\u{81EA}\u{52A8}\u{751F}\u{6210}\u{FF0C}\u{5FC5}\u{987B}\u{4ECE} MCP \u{8BBE}\u{7F6E}\u{9875}\u{5355}\u{72EC}\u{7ACB}\u{542F}\u{7528}\u{3002} ");
        m.insert("settings.ai.see_supported_providers", "\u{67E5}\u{770B}\u{652F}\u{6301}\u{7684}\u{63D0}\u{4F9B}\u{5546}\u{3002}");
        m.insert("settings.ai.manage_mcp_servers", "\u{7BA1}\u{7406} MCP \u{670D}\u{52A1}\u{5668}");

        // AI page - Knowledge section
        m.insert("settings.ai.knowledge", "\u{77E5}\u{8BC6}");
        m.insert("settings.ai.rules", "\u{89C4}\u{5219}");
        m.insert("settings.ai.rules.description", "\u{89C4}\u{5219}\u{5E2E}\u{52A9} Warp \u{4EE3}\u{7406}\u{9075}\u{5B88}\u{60A8}\u{7684}\u{7EA6}\u{5B9A}\u{FF0C}\u{65E0}\u{8BBA}\u{662F}\u{4EE3}\u{7801}\u{5E93}\u{8FD8}\u{662F}\u{7279}\u{5B9A}\u{5DE5}\u{4F5C}\u{6D41}\u{3002} ");
        m.insert("settings.ai.suggested_rules", "\u{5EFA}\u{8BAE}\u{89C4}\u{5219}");
        m.insert("settings.ai.suggested_rules.description", "\u{8BA9} AI \u{6839}\u{636E}\u{60A8}\u{7684}\u{4EA4}\u{4E92}\u{5EFA}\u{8BAE}\u{8981}\u{4FDD}\u{5B58}\u{7684}\u{89C4}\u{5219}\u{3002}");
        m.insert("settings.ai.manage_rules", "\u{7BA1}\u{7406}\u{89C4}\u{5219}");
        m.insert("settings.ai.warp_drive_as_agent_context", "Warp Drive \u{4F5C}\u{4E3A}\u{4EE3}\u{7406}\u{4E0A}\u{4E0B}\u{6587}");
        m.insert("settings.ai.warp_drive_context.description", "Warp \u{4EE3}\u{7406}\u{53EF}\u{4EE5}\u{5229}\u{7528}\u{60A8}\u{7684} Warp Drive \u{5185}\u{5BB9}\u{6765}\u{5B9A}\u{5236}\u{54CD}\u{5E94}\u{FF0C}\u{4EE5}\u{9002}\u{5408}\u{60A8}\u{7684}\u{4E2A}\u{4EBA}\u{548C}\u{56E2}\u{961F}\u{5F00}\u{53D1}\u{5DE5}\u{4F5C}\u{6D41}\u{548C}\u{73AF}\u{5883}\u{3002}\u{8FD9}\u{5305}\u{62EC}\u{4EFB}\u{4F55}\u{5DE5}\u{4F5C}\u{6D41}\u{3001}\u{7B14}\u{8BB0}\u{672C}\u{548C}\u{73AF}\u{5883}\u{53D8}\u{91CF}\u{3002}");

        // AI page - Voice section
        m.insert("settings.ai.voice", "\u{8BED}\u{97F3}");
        m.insert("settings.ai.voice_input", "\u{8BED}\u{97F3}\u{8F93}\u{5165}");
        m.insert("settings.ai.voice_input.description", "\u{8BED}\u{97F3}\u{8F93}\u{5165}\u{5141}\u{8BB8}\u{60A8}\u{901A}\u{8FC7}\u{76F4}\u{63A5}\u{5BF9}\u{7EC8}\u{7AEF}\u{8BF4}\u{8BDD}\u{6765}\u{63A7}\u{5236} Warp \u{FF08}\u{7531} ");
        m.insert("settings.ai.voice_input_key", "\u{6FC0}\u{6D3B}\u{8BED}\u{97F3}\u{8F93}\u{5165}\u{7684}\u{6309}\u{952E}");
        m.insert("settings.ai.voice_input.press_hold", "\u{6309}\u{4F4F}\u{4E0D}\u{653E}\u{4EE5}\u{6FC0}\u{6D3B}\u{3002}");

        // AI page - Other section
        m.insert("settings.ai.other", "\u{5176}\u{4ED6}");
        m.insert("settings.ai.show_oz_changelog", "\u{5728}\u{65B0}\u{4F1A}\u{8BDD}\u{89C6}\u{56FE}\u{4E2D}\u{663E}\u{793A} Oz \u{66F4}\u{65B0}\u{65E5}\u{5FD7}");
        m.insert("settings.ai.show_use_agent_footer", "\u{663E}\u{793A}\u{201C}\u{4F7F}\u{7528}\u{4EE3}\u{7406}\u{201D}\u{811A}\u{6CE8}");
        m.insert("settings.ai.use_agent_footer.description", "\u{5728}\u{957F}\u{65F6}\u{95F4}\u{8FD0}\u{884C}\u{7684}\u{547D}\u{4EE4}\u{4E2D}\u{663E}\u{793A}\u{4F7F}\u{7528}\u{201C}\u{5B8C}\u{5168}\u{7EC8}\u{7AEF}\u{4F7F}\u{7528}\u{201D}\u{542F}\u{7528}\u{7684}\u{4EE3}\u{7406}\u{7684}\u{63D0}\u{793A}\u{3002}");
        m.insert("settings.ai.show_conversation_history", "\u{5728}\u{5DE5}\u{5177}\u{9762}\u{677F}\u{4E2D}\u{663E}\u{793A}\u{4F1A}\u{8BDD}\u{5386}\u{53F2}");
        m.insert("settings.ai.agent_thinking_display", "\u{4EE3}\u{7406}\u{601D}\u{7EF4}\u{663E}\u{793A}");
        m.insert("settings.ai.thinking_display.description", "\u{63A7}\u{5236}\u{63A8}\u{7406}/\u{601D}\u{7EF4}\u{8DDF}\u{8FF9}\u{7684}\u{663E}\u{793A}\u{65B9}\u{5F0F}\u{3002}");
        m.insert("settings.ai.preferred_conversation_layout", "\u{6253}\u{5F00}\u{73B0}\u{6709}\u{4EE3}\u{7406}\u{4F1A}\u{8BDD}\u{65F6}\u{7684}\u{9996}\u{9009}\u{5E03}\u{5C40}");

        // AI page - Third party CLI agents section
        m.insert("settings.ai.third_party_cli_agents", "\u{7B2C}\u{4E09}\u{65B9} CLI \u{4EE3}\u{7406}");
        m.insert("settings.ai.show_coding_agent_toolbar", "\u{663E}\u{793A}\u{7F16}\u{7801}\u{4EE3}\u{7406}\u{5DE5}\u{5177}\u{680F}");
        m.insert("settings.ai.coding_agent_toolbar.description", "\u{5728}\u{8FD0}\u{884C}\u{7F16}\u{7801}\u{4EE3}\u{7406}\u{65F6}\u{663E}\u{793A}\u{5E26}\u{6709}\u{5FEB}\u{6377}\u{64CD}\u{4F5C}\u{7684}\u{5DE5}\u{5177}\u{680F}\u{FF0C}\u{5982} ");
        m.insert("settings.ai.auto_toggle_rich_input", "\u{6839}\u{636E}\u{4EE3}\u{7406}\u{72B6}\u{6001}\u{81EA}\u{52A8}\u{663E}\u{793A}/\u{9690}\u{85CF}\u{5BCC}\u{6587}\u{672C}\u{8F93}\u{5165}");
        m.insert("settings.ai.requires_warp_plugin", "\u{9700}\u{8981}\u{60A8}\u{7684}\u{7F16}\u{7801}\u{4EE3}\u{7406}\u{7684} Warp \u{63D2}\u{4EF6}");
        m.insert("settings.ai.auto_open_rich_input", "\u{5728}\u{7F16}\u{7801}\u{4EE3}\u{7406}\u{4F1A}\u{8BDD}\u{5F00}\u{59CB}\u{65F6}\u{81EA}\u{52A8}\u{6253}\u{5F00}\u{5BCC}\u{6587}\u{672C}\u{8F93}\u{5165}");
        m.insert("settings.ai.auto_dismiss_rich_input", "\u{5728}\u{63D0}\u{793A}\u{63D0}\u{4EA4}\u{540E}\u{81EA}\u{52A8}\u{5173}\u{95ED}\u{5BCC}\u{6587}\u{672C}\u{8F93}\u{5165}");
        m.insert("settings.ai.commands_enable_toolbar", "\u{542F}\u{7528}\u{5DE5}\u{5177}\u{680F}\u{7684}\u{547D}\u{4EE4}");
        m.insert("settings.ai.toolbar_command_patterns.description", "\u{6DFB}\u{52A0}\u{6B63}\u{5219}\u{8868}\u{8FBE}\u{5F0F}\u{4EE5}\u{4E3A}\u{5339}\u{914D}\u{7684}\u{547D}\u{4EE4}\u{663E}\u{793A}\u{7F16}\u{7801}\u{4EE3}\u{7406}\u{5DE5}\u{5177}\u{680F}\u{3002}");

        // AI page - Agent Attribution section
        m.insert("settings.ai.agent_attribution", "\u{4EE3}\u{7406}\u{5F52}\u{5C5E}");
        m.insert("settings.ai.enable_agent_attribution", "\u{542F}\u{7528}\u{4EE3}\u{7406}\u{5F52}\u{5C5E}");
        m.insert("settings.ai.agent_attribution.description", "Oz \u{53EF}\u{4EE5}\u{5728}\u{5B83}\u{521B}\u{5EFA}\u{7684}\u{63D0}\u{4EA4}\u{4FE1}\u{606F}\u{548C}\u{62C9}\u{53D6}\u{8BF7}\u{6C42}\u{4E2D}\u{6DFB}\u{52A0}\u{5F52}\u{5C5E}\u{4FE1}\u{606F}");

        // AI page - Experimental section
        m.insert("settings.ai.experimental", "\u{5B9E}\u{9A8C}\u{6027}");
        m.insert("settings.ai.computer_use_cloud_agents", "\u{4E91}\u{4EE3}\u{7406}\u{4E2D}\u{7684}\u{8BA1}\u{7B97}\u{673A}\u{4F7F}\u{7528}");
        m.insert("settings.ai.computer_use.description", "\u{5728}\u{4ECE} Warp \u{5E94}\u{7528}\u{542F}\u{52A8}\u{7684}\u{4E91}\u{4EE3}\u{7406}\u{4F1A}\u{8BDD}\u{4E2D}\u{542F}\u{7528}\u{8BA1}\u{7B97}\u{673A}\u{4F7F}\u{7528}\u{3002}");
        m.insert("settings.ai.orchestration", "\u{7F16}\u{6392}");
        m.insert("settings.ai.orchestration.description", "\u{542F}\u{7528}\u{591A}\u{4EE3}\u{7406}\u{7F16}\u{6392}\u{FF0C}\u{5141}\u{8BB8}\u{4EE3}\u{7406}\u{751F}\u{6210}\u{548C}\u{534F}\u{8C03}\u{5E76}\u{884C}\u{5B50}\u{4EE3}\u{7406}\u{3002}");

        // AI page - API Keys section
        m.insert("settings.ai.api_keys", "API \u{5BC6}\u{94A5}");
        m.insert("settings.ai.api_keys.description", "\u{4F7F}\u{7528}\u{60A8}\u{81EA}\u{5DF1}\u{7684}\u{6A21}\u{578B}\u{63D0}\u{4F9B}\u{5546} API \u{5BC6}\u{94A5}\u{4F9B} Warp \u{4EE3}\u{7406}\u{4F7F}\u{7528}\u{3002}API \u{5BC6}\u{94A5}\u{5B58}\u{50A8}\u{5728}\u{672C}\u{5730}\u{FF0C}\u{6C38}\u{8FDC}\u{4E0D}\u{4F1A}\u{540C}\u{6B65}\u{5230}\u{4E91}\u{7AEF}\u{3002}\u{4F7F}\u{7528}\u{81EA}\u{52A8}\u{6A21}\u{578B}\u{6216}\u{60A8}\u{672A}\u{63D0}\u{4F9B} API \u{5BC6}\u{94A5}\u{7684}\u{63D0}\u{4F9B}\u{5546}\u{7684}\u{6A21}\u{578B}\u{5C06}\u{6D88}\u{8017} Warp \u{79EF}\u{5206}\u{3002}");
        m.insert("settings.ai.warp_credit_fallback", "Warp \u{79EF}\u{5206}\u{56DE}\u{9000}");
        m.insert("settings.ai.warp_credit_fallback.description", "\u{542F}\u{7528}\u{540E}\u{FF0C}\u{5728}\u{53D1}\u{751F}\u{9519}\u{8BEF}\u{65F6}\u{FF0C}\u{4EE3}\u{7406}\u{8BF7}\u{6C42}\u{53EF}\u{80FD}\u{4F1A}\u{88AB}\u{8DEF}\u{7531}\u{5230} Warp \u{63D0}\u{4F9B}\u{7684}\u{6A21}\u{578B}\u{3002}Warp \u{4F1A}\u{4F18}\u{5148}\u{4F7F}\u{7528}\u{60A8}\u{7684} API \u{5BC6}\u{94A5}\u{800C}\u{975E} Warp \u{79EF}\u{5206}\u{3002}");

        // AI page - AWS Bedrock section
        m.insert("settings.ai.aws_bedrock", "AWS Bedrock");
        m.insert("settings.ai.use_aws_bedrock_credentials", "\u{4F7F}\u{7528} AWS Bedrock \u{51ED}\u{8BC1}");
        m.insert("settings.ai.aws_bedrock_credentials.description", "Warp \u{52A0}\u{8F7D}\u{5E76}\u{53D1}\u{9001}\u{672C}\u{5730} AWS CLI \u{51ED}\u{8BC1}\u{7528}\u{4E8E} Bedrock \u{652F}\u{6301}\u{7684}\u{6A21}\u{578B}\u{3002}");
        m.insert("settings.ai.aws_bedrock_credentials.admin_description", "Warp \u{52A0}\u{8F7D}\u{5E76}\u{53D1}\u{9001}\u{672C}\u{5730} AWS CLI \u{51ED}\u{8BC1}\u{7528}\u{4E8E} Bedrock \u{652F}\u{6301}\u{7684}\u{6A21}\u{578B}\u{3002}\u{6B64}\u{8BBE}\u{7F6E}\u{7531}\u{60A8}\u{7684}\u{7EC4}\u{7EC7}\u{7BA1}\u{7406}\u{3002}");
        m.insert("settings.ai.login_command", "\u{767B}\u{5F55}\u{547D}\u{4EE4}");
        m.insert("settings.ai.aws_profile", "AWS \u{914D}\u{7F6E}\u{6587}\u{4EF6}");
        m.insert("settings.ai.auto_run_login_command", "\u{81EA}\u{52A8}\u{8FD0}\u{884C}\u{767B}\u{5F55}\u{547D}\u{4EE4}");
        m.insert("settings.ai.auto_login.description", "\u{542F}\u{7528}\u{540E}\u{FF0C}\u{5F53} AWS Bedrock \u{51ED}\u{8BC1}\u{8FC7}\u{671F}\u{65F6}\u{5C06}\u{81EA}\u{52A8}\u{8FD0}\u{884C}\u{767B}\u{5F55}\u{547D}\u{4EE4}\u{3002}");

        // AI page - Placeholder hints
        m.insert("settings.ai.placeholder.code_repo", "\u{4F8B}\u{5982} ~/code-repos/repo");
        m.insert("settings.ai.placeholder.commands_comma", "\u{547D}\u{4EE4}\u{FF0C}\u{7528}\u{9017}\u{53F7}\u{5206}\u{9694}");
        m.insert("settings.ai.placeholder.regex_ls", "\u{4F8B}\u{5982} ls .*");
        m.insert("settings.ai.placeholder.regex_rm", "\u{4F8B}\u{5982} rm .*");
        m.insert("settings.ai.placeholder.command_regex", "\u{547D}\u{4EE4}\u{FF08}\u{652F}\u{6301}\u{6B63}\u{5219}\u{8868}\u{8FBE}\u{5F0F}\u{FF09}");

        // Code page
        m.insert("settings.code.indexing.title", "\u{4EE3}\u{7801}\u{5E93}\u{7D22}\u{5F15}");
        m.insert("settings.code.indexing.index_new_folder", "\u{7D22}\u{5F15}\u{65B0}\u{6587}\u{4EF6}\u{5939}");
        m.insert("settings.code.indexing.init_settings", "\u{521D}\u{59CB}\u{5316}\u{8BBE}\u{7F6E}");
        m.insert("settings.code.indexing.description", "Warp \u{53EF}\u{4EE5}\u{81EA}\u{52A8}\u{7D22}\u{5F15}\u{4EE3}\u{7801}\u{4ED3}\u{5E93}\u{FF0C}\u{4EE5}\u{652F}\u{6301}\u{4EE3}\u{7801}\u{5E93}\u{641C}\u{7D22}\u{548C}\u{4E0A}\u{4E0B}\u{6587}\u{611F}\u{77E5}\u{5EFA}\u{8BAE}\u{7B49} AI \u{529F}\u{80FD}\u{3002}");
        m.insert("settings.code.indexing.exclude_description", "\u{8981}\u{4ECE}\u{7D22}\u{5F15}\u{4E2D}\u{6392}\u{9664}\u{7279}\u{5B9A}\u{6587}\u{4EF6}\u{6216}\u{76EE}\u{5F55}\u{FF0C}\u{8BF7}\u{5C06}\u{5B83}\u{4EEC}\u{6DFB}\u{52A0}\u{5230} .warpindexignore \u{6587}\u{4EF6}\u{4E2D}\u{3002}");
        m.insert("settings.code.indexing.index_new_folder.description", "\u{8BBE}\u{7F6E}\u{4E3A} true \u{65F6}\u{FF0C}Warp \u{5C06}\u{81EA}\u{52A8}\u{7D22}\u{5F15}\u{65B0}\u{53D1}\u{73B0}\u{7684}\u{6587}\u{4EF6}\u{5939}\u{3002}");
        m.insert("settings.code.indexing.disabled_by_admin", "\u{56E2}\u{961F}\u{7BA1}\u{7406}\u{5458}\u{5DF2}\u{7981}\u{7528}\u{4EE3}\u{7801}\u{5E93}\u{7D22}\u{5F15}\u{3002}");
        m.insert("settings.code.indexing.enabled_by_admin", "\u{56E2}\u{961F}\u{7BA1}\u{7406}\u{5458}\u{5DF2}\u{542F}\u{7528}\u{4EE3}\u{7801}\u{5E93}\u{7D22}\u{5F15}\u{3002}");
        m.insert("settings.code.indexing.ai_required", "\u{5FC5}\u{987B}\u{542F}\u{7528} AI \u{529F}\u{80FD}\u{624D}\u{80FD}\u{4F7F}\u{7528}\u{4EE3}\u{7801}\u{5E93}\u{7D22}\u{5F15}\u{3002}");
        m.insert("settings.code.indexing.max_indices", "\u{60A8}\u{5DF2}\u{8FBE}\u{5230}\u{4EE3}\u{7801}\u{5E93}\u{7D22}\u{5F15}\u{7684}\u{6700}\u{5927}\u{6570}\u{91CF}\u{3002}\u{8BF7}\u{5220}\u{9664}\u{73B0}\u{6709}\u{7D22}\u{5F15}\u{540E}\u{518D}\u{6DFB}\u{52A0}\u{65B0}\u{7684}\u{3002}");
        m.insert("settings.code.indexing.initialized_folders", "\u{5DF2}\u{521D}\u{59CB}\u{5316} / \u{5DF2}\u{7D22}\u{5F15}\u{7684}\u{6587}\u{4EF6}\u{5939}");
        m.insert("settings.code.indexing.no_folders", "\u{5C1A}\u{672A}\u{521D}\u{59CB}\u{5316}\u{4EFB}\u{4F55}\u{6587}\u{4EF6}\u{5939}\u{3002}");
        m.insert("settings.code.indexing.open_project_rules", "\u{6253}\u{5F00}\u{9879}\u{76EE}\u{89C4}\u{5219}");
        m.insert("settings.code.indexing.status_label", "\u{7D22}\u{5F15}\u{4E2D}");
        m.insert("settings.code.indexing.no_index", "\u{672A}\u{521B}\u{5EFA}\u{7D22}\u{5F15}");
        m.insert("settings.code.indexing.discovered_chunks", "\u{5DF2}\u{53D1}\u{73B0} {total_nodes} \u{4E2A}\u{4EE3}\u{7801}\u{5757}");
        m.insert("settings.code.indexing.syncing_progress", "\u{540C}\u{6B65}\u{4E2D} - {completed_nodes} / {total_nodes}");
        m.insert("settings.code.indexing.syncing", "\u{540C}\u{6B65}\u{4E2D}...");
        m.insert("settings.code.indexing.synced", "\u{5DF2}\u{540C}\u{6B65}");
        m.insert("settings.code.indexing.too_large", "\u{4EE3}\u{7801}\u{5E93}\u{8FC7}\u{5927}");
        m.insert("settings.code.indexing.stale", "\u{5DF2}\u{8FC7}\u{671F}");
        m.insert("settings.code.indexing.failed", "\u{5931}\u{8D25}");
        m.insert("settings.code.indexing.no_index_built", "\u{672A}\u{6784}\u{5EFA}\u{7D22}\u{5F15}");

        // Code page - LSP Servers
        m.insert("settings.code.lsp.title", "LSP \u{670D}\u{52A1}\u{5668}");
        m.insert("settings.code.lsp.installed", "\u{5DF2}\u{5B89}\u{88C5}");
        m.insert("settings.code.lsp.installing", "\u{5B89}\u{88C5}\u{4E2D}...");
        m.insert("settings.code.lsp.checking", "\u{68C0}\u{67E5}\u{4E2D}...");
        m.insert("settings.code.lsp.available_download", "\u{53EF}\u{4E0B}\u{8F7D}");
        m.insert("settings.code.lsp.restart", "\u{91CD}\u{542F}\u{670D}\u{52A1}\u{5668}");
        m.insert("settings.code.lsp.view_logs", "\u{67E5}\u{770B}\u{65E5}\u{5FD7}");
        m.insert("settings.code.lsp.status_available", "\u{53EF}\u{7528}");
        m.insert("settings.code.lsp.status_busy", "\u{7E41}\u{5FD9}");
        m.insert("settings.code.lsp.status_failed", "\u{5931}\u{8D25}");
        m.insert("settings.code.lsp.status_stopped", "\u{5DF2}\u{505C}\u{6B62}");
        m.insert("settings.code.lsp.status_not_running", "\u{672A}\u{8FD0}\u{884C}");

        m.insert("settings.code.editor.title", "\u{7F16}\u{8F91}\u{5668}\u{548C}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}");
        m.insert("settings.code.editor.category", "\u{4EE3}\u{7801}\u{7F16}\u{8F91}\u{5668}\u{548C}\u{5BA1}\u{67E5}");
        m.insert("settings.code.editor.default_app", "\u{9ED8}\u{8BA4}\u{5E94}\u{7528}");
        m.insert("settings.code.editor.layout.split_pane", "\u{62C6}\u{5206}\u{7A97}\u{683C}");
        m.insert("settings.code.editor.layout.new_tab", "\u{65B0}\u{6807}\u{7B7E}\u{9875}");
        m.insert("settings.code.editor.open_file_links", "\u{9009}\u{62E9}\u{7528}\u{4E8E}\u{6253}\u{5F00}\u{6587}\u{4EF6}\u{94FE}\u{63A5}\u{7684}\u{7F16}\u{8F91}\u{5668}");
        m.insert("settings.code.editor.open_code_panel_files", "\u{9009}\u{62E9}\u{7528}\u{4E8E}\u{4ECE}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}\u{9762}\u{677F}\u{3001}\u{9879}\u{76EE}\u{6D4F}\u{89C8}\u{5668}\u{548C}\u{5168}\u{5C40}\u{641C}\u{7D22}\u{6253}\u{5F00}\u{6587}\u{4EF6}\u{7684}\u{7F16}\u{8F91}\u{5668}");
        m.insert("settings.code.editor.open_files_layout", "\u{9009}\u{62E9}\u{5728} Warp \u{4E2D}\u{6253}\u{5F00}\u{6587}\u{4EF6}\u{7684}\u{5E03}\u{5C40}");
        m.insert("settings.code.editor.group_files", "\u{5C06}\u{6587}\u{4EF6}\u{5206}\u{7EC4}\u{5230}\u{5355}\u{4E2A}\u{7F16}\u{8F91}\u{5668}\u{7A97}\u{683C}");
        m.insert("settings.code.editor.group_files.description", "\u{5F00}\u{542F}\u{540E}\u{FF0C}\u{5728}\u{540C}\u{4E00}\u{6807}\u{7B7E}\u{9875}\u{4E2D}\u{6253}\u{5F00}\u{7684}\u{6240}\u{6709}\u{6587}\u{4EF6}\u{90FD}\u{4F1A}\u{81EA}\u{52A8}\u{5206}\u{7EC4}\u{5230}\u{5355}\u{4E2A}\u{7F16}\u{8F91}\u{5668}\u{7A97}\u{683C}\u{4E2D}\u{3002}");
        m.insert("settings.code.editor.open_markdown_in_viewer", "\u{9ED8}\u{8BA4}\u{5728} Warp \u{7684} Markdown \u{67E5}\u{770B}\u{5668}\u{4E2D}\u{6253}\u{5F00} Markdown \u{6587}\u{4EF6}");
        m.insert("settings.code.editor.auto_open_code_review_panel", "\u{81EA}\u{52A8}\u{6253}\u{5F00}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}\u{9762}\u{677F}");
        m.insert("settings.code.editor.auto_open_code_review_panel.description", "\u{5F00}\u{542F}\u{540E}\u{FF0C}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}\u{9762}\u{677F}\u{4F1A}\u{5728}\u{5BF9}\u{8BDD}\u{4E2D}\u{7B2C}\u{4E00}\u{4E2A}\u{63A5}\u{53D7}\u{7684}\u{5DEE}\u{5F02}\u{5904}\u{6253}\u{5F00}");
        m.insert("settings.code.editor.show_code_review_button", "\u{663E}\u{793A}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}\u{6309}\u{94AE}");
        m.insert("settings.code.editor.show_code_review_button.description", "\u{5728}\u{7A97}\u{53E3}\u{53F3}\u{4E0A}\u{89D2}\u{663E}\u{793A}\u{7528}\u{4E8E}\u{5207}\u{6362}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}\u{9762}\u{677F}\u{7684}\u{6309}\u{94AE}\u{3002}");
        m.insert("settings.code.editor.show_diff_stats", "\u{5728}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}\u{6309}\u{94AE}\u{4E0A}\u{663E}\u{793A}\u{5DEE}\u{5F02}\u{7EDF}\u{8BA1}");
        m.insert("settings.code.editor.show_diff_stats.description", "\u{5728}\u{4EE3}\u{7801}\u{5BA1}\u{67E5}\u{6309}\u{94AE}\u{4E0A}\u{663E}\u{793A}\u{65B0}\u{589E}\u{548C}\u{5220}\u{9664}\u{7684}\u{884C}\u{6570}\u{3002}");
        m.insert("settings.code.editor.project_explorer", "\u{9879}\u{76EE}\u{6D4F}\u{89C8}\u{5668}");
        m.insert("settings.code.editor.project_explorer.description", "\u{5728}\u{5DE6}\u{4FA7}\u{5DE5}\u{5177}\u{9762}\u{677F}\u{4E2D}\u{6DFB}\u{52A0} IDE \u{98CE}\u{683C}\u{7684}\u{9879}\u{76EE}\u{6D4F}\u{89C8}\u{5668}/\u{6587}\u{4EF6}\u{6811}\u{3002}");
        m.insert("settings.code.editor.global_file_search", "\u{5168}\u{5C40}\u{6587}\u{4EF6}\u{641C}\u{7D22}");
        m.insert("settings.code.editor.global_file_search.description", "\u{5C06}\u{5168}\u{5C40}\u{6587}\u{4EF6}\u{641C}\u{7D22}\u{6DFB}\u{52A0}\u{5230}\u{5DE6}\u{4FA7}\u{5DE5}\u{5177}\u{9762}\u{677F}\u{3002}");

        // Keybindings page
        m.insert("settings.keybindings", "\u{952E}\u{76D8}\u{5FEB}\u{6377}\u{952E}");
        m.insert("settings.keybindings.search_placeholder", "\u{6309}\u{540D}\u{79F0}\u{6216}\u{6309}\u{952E}\u{641C}\u{7D22}\u{FF08}\u{4F8B}\u{5982} \"cmd d\"\u{FF09}");
        m.insert("settings.keybindings.conflict_warning", "\u{6B64}\u{5FEB}\u{6377}\u{952E}\u{4E0E}\u{5176}\u{4ED6}\u{952E}\u{7ED1}\u{5B9A}\u{51B2}\u{7A81}");
        m.insert("settings.keybindings.default", "\u{9ED8}\u{8BA4}");
        m.insert("settings.keybindings.clear", "\u{6E05}\u{9664}");
        m.insert("settings.keybindings.press_new_shortcut", "\u{6309}\u{4E0B}\u{65B0}\u{7684}\u{952E}\u{76D8}\u{5FEB}\u{6377}\u{952E}");
        m.insert("settings.keybindings.add_custom", "\u{5728}\u{4E0B}\u{65B9}\u{6DFB}\u{52A0}\u{81EA}\u{5B9A}\u{4E49}\u{952E}\u{7ED1}\u{5B9A}\u{3002}");
        m.insert("settings.keybindings.use", "\u{4F7F}\u{7528}");
        m.insert("settings.keybindings.not_synced", "\u{952E}\u{76D8}\u{5FEB}\u{6377}\u{952E}\u{4E0D}\u{4F1A}\u{540C}\u{6B65}\u{5230}\u{4E91}\u{7AEF}");
        m.insert("settings.keybindings.configure", "\u{914D}\u{7F6E}\u{952E}\u{76D8}\u{5FEB}\u{6377}\u{952E}");
        m.insert("settings.keybindings.command", "\u{547D}\u{4EE4}");

        // Privacy page
        m.insert("settings.privacy", "\u{9690}\u{79C1}");
        m.insert("settings.privacy.safe_mode", "\u{5BC6}\u{7801}\u{8131}\u{654F}");
        m.insert("settings.privacy.safe_mode.description", "\u{542F}\u{7528}\u{6B64}\u{8BBE}\u{7F6E}\u{540E}\u{FF0C}Warp \u{5C06}\u{626B}\u{63CF}\u{4EE3}\u{7801}\u{5757}\u{3001}Warp Drive \u{5BF9}\u{8C61}\u{7684}\u{5185}\u{5BB9}\u{4EE5}\u{53CA} Oz \u{63D0}\u{793A}\u{4E2D}\u{7684}\u{6F5C}\u{5728}\u{654F}\u{611F}\u{4FE1}\u{606F}\u{FF0C}\u{5E76}\u{963B}\u{6B62}\u{4FDD}\u{5B58}\u{6216}\u{5C06}\u{6B64}\u{6570}\u{636E}\u{53D1}\u{9001}\u{5230}\u{4EFB}\u{4F55}\u{670D}\u{52A1}\u{5668}\u{3002}\u{60A8}\u{53EF}\u{4EE5}\u{901A}\u{8FC7}\u{6B63}\u{5219}\u{8868}\u{8FBE}\u{5F0F}\u{81EA}\u{5B9A}\u{4E49}\u{6B64}\u{5217}\u{8868}\u{3002}");
        m.insert("settings.privacy.user_secret_regex", "\u{81EA}\u{5B9A}\u{4E49}\u{5BC6}\u{7801}\u{8131}\u{654F}");
        m.insert("settings.privacy.user_secret_regex.description", "\u{4F7F}\u{7528}\u{6B63}\u{5219}\u{8868}\u{8FBE}\u{5F0F}\u{5B9A}\u{4E49}\u{989D}\u{5916}\u{7684}\u{5BC6}\u{7801}\u{6216}\u{6570}\u{636E}\u{8FDB}\u{884C}\u{8131}\u{654F}\u{3002}\u{6B64}\u{66F4}\u{6539}\u{5C06}\u{5728}\u{4E0B}\u{4E00}\u{6761}\u{547D}\u{4EE4}\u{8FD0}\u{884C}\u{65F6}\u{751F}\u{6548}\u{3002}\u{60A8}\u{53EF}\u{4EE5}\u{4F7F}\u{7528}\u{5185}\u{8054} (?i) \u{6807}\u{5FD7}\u{4F5C}\u{4E3A}\u{6B63}\u{5219}\u{8868}\u{8FBE}\u{5F0F}\u{7684}\u{524D}\u{7F00}\u{4EE5}\u{5B9E}\u{73B0}\u{4E0D}\u{533A}\u{5206}\u{5927}\u{5C0F}\u{5199}\u{3002}");
        m.insert("settings.privacy.telemetry", "\u{5E2E}\u{52A9}\u{6539}\u{8FDB} Warp");
        m.insert("settings.privacy.telemetry.description", "\u{5E94}\u{7528}\u{5206}\u{6790}\u{5E2E}\u{52A9}\u{6211}\u{4EEC}\u{4E3A}\u{60A8}\u{6253}\u{9020}\u{66F4}\u{597D}\u{7684}\u{4EA7}\u{54C1}\u{3002}\u{6211}\u{4EEC}\u{53EF}\u{80FD}\u{4F1A}\u{6536}\u{96C6}\u{67D0}\u{4E9B}\u{63A7}\u{5236}\u{53F0}\u{4EA4}\u{4E92}\u{4EE5}\u{6539}\u{8FDB} Warp \u{7684} AI \u{529F}\u{80FD}\u{3002}");
        m.insert("settings.privacy.telemetry.description_old", "\u{5E94}\u{7528}\u{5206}\u{6790}\u{5E2E}\u{52A9}\u{6211}\u{4EEC}\u{4E3A}\u{60A8}\u{6253}\u{9020}\u{66F4}\u{597D}\u{7684}\u{4EA7}\u{54C1}\u{3002}\u{6211}\u{4EEC}\u{4EC5}\u{6536}\u{96C6}\u{5E94}\u{7528}\u{4F7F}\u{7528}\u{5143}\u{6570}\u{636E}\u{FF0C}\u{4E0D}\u{4F1A}\u{6536}\u{96C6}\u{63A7}\u{5236}\u{53F0}\u{8F93}\u{5165}\u{6216}\u{8F93}\u{51FA}\u{3002}");
        m.insert("settings.privacy.telemetry.free_tier_note", "\u{5728}\u{514D}\u{8D39}\u{7248}\u{4E2D}\u{FF0C}\u{5FC5}\u{987B}\u{542F}\u{7528}\u{5206}\u{6790}\u{624D}\u{80FD}\u{4F7F}\u{7528} AI \u{529F}\u{80FD}\u{3002}");
        m.insert("settings.privacy.telemetry.read_more", "\u{4E86}\u{89E3} Warp \u{5982}\u{4F55}\u{4F7F}\u{7528}\u{6570}\u{636E}");
        m.insert("settings.privacy.data_management", "\u{7BA1}\u{7406}\u{60A8}\u{7684}\u{6570}\u{636E}");
        m.insert("settings.privacy.data_management.description", "\u{60A8}\u{53EF}\u{4EE5}\u{968F}\u{65F6}\u{9009}\u{62E9}\u{6C38}\u{4E45}\u{5220}\u{9664}\u{60A8}\u{7684} Warp \u{8D26}\u{6237}\u{3002}\u{5C4A}\u{65F6}\u{60A8}\u{5C06}\u{65E0}\u{6CD5}\u{518D}\u{4F7F}\u{7528} Warp\u{3002}");
        m.insert("settings.privacy.data_management.link", "\u{8BBF}\u{95EE}\u{6570}\u{636E}\u{7BA1}\u{7406}\u{9875}\u{9762}");
        m.insert("settings.privacy.privacy_policy", "\u{9690}\u{79C1}\u{653F}\u{7B56}");
        m.insert("settings.privacy.privacy_policy.link", "\u{9605}\u{8BFB} Warp \u{7684}\u{9690}\u{79C1}\u{653F}\u{7B56}");
        m.insert("settings.privacy.personal", "\u{4E2A}\u{4EBA}");
        m.insert("settings.privacy.enterprise", "\u{4F01}\u{4E1A}");
        m.insert("settings.privacy.enterprise.cannot_modify", "\u{4F01}\u{4E1A}\u{5BC6}\u{7801}\u{8131}\u{654F}\u{65E0}\u{6CD5}\u{4FEE}\u{6539}\u{3002}");
        m.insert("settings.privacy.enterprise.no_regexes", "\u{60A8}\u{7684}\u{7EC4}\u{7EC7}\u{5C1A}\u{672A}\u{914D}\u{7F6E}\u{4EFB}\u{4F55}\u{4F01}\u{4E1A}\u{6B63}\u{5219}\u{8868}\u{8FBE}\u{5F0F}\u{3002}");
        m.insert("settings.privacy.managed_by_org", "\u{5DF2}\u{7531}\u{60A8}\u{7684}\u{7EC4}\u{7EC7}\u{542F}\u{7528}\u{3002}");
        m.insert("settings.privacy.managed_by_org.tooltip", "\u{6B64}\u{8BBE}\u{7F6E}\u{7531}\u{60A8}\u{7684}\u{7EC4}\u{7EC7}\u{7BA1}\u{7406}\u{3002}");
        m.insert("settings.privacy.secret_visual_mode", "\u{5BC6}\u{7801}\u{89C6}\u{89C9}\u{8131}\u{654F}\u{6A21}\u{5F0F}");
        m.insert("settings.privacy.secret_visual_mode.description", "\u{9009}\u{62E9}\u{5BC6}\u{7801}\u{5728}\u{4EE3}\u{7801}\u{5757}\u{5217}\u{8868}\u{4E2D}\u{7684}\u{89C6}\u{89C9}\u{663E}\u{793A}\u{65B9}\u{5F0F}\u{FF0C}\u{540C}\u{65F6}\u{4FDD}\u{6301}\u{53EF}\u{641C}\u{7D22}\u{3002}\u{6B64}\u{8BBE}\u{7F6E}\u{4EC5}\u{5F71}\u{54CD}\u{60A8}\u{5728}\u{4EE3}\u{7801}\u{5757}\u{5217}\u{8868}\u{4E2D}\u{770B}\u{5230}\u{7684}\u{5185}\u{5BB9}\u{3002}");
        m.insert("settings.privacy.recommended", "\u{63A8}\u{8350}");
        m.insert("settings.privacy.add_all", "\u{5168}\u{90E8}\u{6DFB}\u{52A0}");
        m.insert("settings.privacy.add_regex", "\u{6DFB}\u{52A0}\u{6B63}\u{5219}\u{8868}\u{8FBE}\u{5F0F}");
        m.insert("settings.privacy.add_regex_pattern", "\u{6DFB}\u{52A0}\u{6B63}\u{5219}\u{8868}\u{8FBE}\u{5F0F}\u{6A21}\u{5F0F}");
        m.insert("settings.privacy.send_crash_reports", "\u{53D1}\u{9001}\u{5D29}\u{6E83}\u{62A5}\u{544A}");
        m.insert("settings.privacy.send_crash_reports.description", "\u{5D29}\u{6E83}\u{62A5}\u{544A}\u{53EF}\u{5E2E}\u{52A9}\u{8C03}\u{8BD5}\u{548C}\u{63D0}\u{9AD8}\u{7A33}\u{5B9A}\u{6027}\u{3002}");
        m.insert("settings.privacy.store_ai_conversations", "\u{5C06} AI \u{5BF9}\u{8BDD}\u{5B58}\u{50A8}\u{5728}\u{4E91}\u{7AEF}");
        m.insert("settings.privacy.store_ai_conversations.enabled_description", "\u{4EE3}\u{7406}\u{5BF9}\u{8BDD}\u{53EF}\u{4EE5}\u{4E0E}\u{4ED6}\u{4EBA}\u{5171}\u{4EAB}\u{FF0C}\u{5E76}\u{5728}\u{60A8}\u{5728}\u{4E0D}\u{540C}\u{8BBE}\u{5907}\u{767B}\u{5F55}\u{65F6}\u{4FDD}\u{7559}\u{3002}\u{6B64}\u{6570}\u{636E}\u{4EC5}\u{7528}\u{4E8E}\u{4EA7}\u{54C1}\u{529F}\u{80FD}\u{FF0C}Warp \u{4E0D}\u{4F1A}\u{5C06}\u{5176}\u{7528}\u{4E8E}\u{5206}\u{6790}\u{3002}");
        m.insert("settings.privacy.store_ai_conversations.disabled_description", "\u{4EE3}\u{7406}\u{5BF9}\u{8BDD}\u{4EC5}\u{5B58}\u{50A8}\u{5728}\u{60A8}\u{7684}\u{8BA1}\u{7B97}\u{673A}\u{672C}\u{5730}\u{FF0C}\u{767B}\u{51FA}\u{540E}\u{5C06}\u{4E22}\u{5931}\u{FF0C}\u{4E14}\u{65E0}\u{6CD5}\u{5171}\u{4EAB}\u{3002}\u{6CE8}\u{610F}\u{FF1A}\u{73AF}\u{5883}\u{4EE3}\u{7406}\u{7684}\u{5BF9}\u{8BDD}\u{6570}\u{636E}\u{4ECD}\u{5B58}\u{50A8}\u{5728}\u{4E91}\u{7AEF}\u{3002}");
        m.insert("settings.privacy.network_log_console", "\u{7F51}\u{7EDC}\u{65E5}\u{5FD7}\u{63A7}\u{5236}\u{53F0}");
        m.insert("settings.privacy.network_log_console.description", "\u{6211}\u{4EEC}\u{6784}\u{5EFA}\u{4E86}\u{4E00}\u{4E2A}\u{539F}\u{751F}\u{63A7}\u{5236}\u{53F0}\u{FF0C}\u{5141}\u{8BB8}\u{60A8}\u{67E5}\u{770B} Warp \u{4E0E}\u{5916}\u{90E8}\u{670D}\u{52A1}\u{5668}\u{7684}\u{6240}\u{6709}\u{901A}\u{4FE1}\u{FF0C}\u{786E}\u{4FDD}\u{60A8}\u{7684}\u{5DE5}\u{4F5C}\u{59CB}\u{7EC8}\u{5B89}\u{5168}\u{3002}");
        m.insert("settings.privacy.network_log_console.link", "\u{67E5}\u{770B}\u{7F51}\u{7EDC}\u{65E5}\u{5FD7}");
        m.insert("settings.privacy.zero_data_retention", "\u{60A8}\u{7684}\u{7BA1}\u{7406}\u{5458}\u{5DF2}\u{4E3A}\u{60A8}\u{7684}\u{56E2}\u{961F}\u{542F}\u{7528}\u{96F6}\u{6570}\u{636E}\u{4FDD}\u{7559}\u{3002}\u{7528}\u{6237}\u{751F}\u{6210}\u{7684}\u{5185}\u{5BB9}\u{5C06}\u{6C38}\u{4E0D}\u{6536}\u{96C6}\u{3002}");

        // About page
        m.insert("settings.about", "\u{5173}\u{4E8E}");

        // Environments page
        m.insert("settings.environments.title", "\u{73AF}\u{5883}");
        m.insert("settings.environments.description", "\u{73AF}\u{5883}\u{5B9A}\u{4E49}\u{4E86}\u{60A8}\u{7684}\u{667A}\u{80FD}\u{4EE3}\u{7406}\u{8FD0}\u{884C}\u{7684}\u{4F4D}\u{7F6E}\u{3002}\u{901A}\u{8FC7} GitHub \u{FF08}\u{63A8}\u{8350}\u{FF09}\u{3001}Warp \u{8F85}\u{52A9}\u{8BBE}\u{7F6E}\u{6216}\u{624B}\u{52A8}\u{914D}\u{7F6E}\u{FF0C}\u{51E0}\u{5206}\u{949F}\u{5373}\u{53EF}\u{5B8C}\u{6210}\u{8BBE}\u{7F6E}\u{3002}");
        m.insert("settings.environments.search_placeholder", "\u{641C}\u{7D22}\u{73AF}\u{5883}...");
        m.insert("settings.environments.no_matches", "\u{6CA1}\u{6709}\u{73AF}\u{5883}\u{5339}\u{914D}\u{60A8}\u{7684}\u{641C}\u{7D22}\u{3002}");
        m.insert("settings.environments.section.personal", "\u{4E2A}\u{4EBA}");
        m.insert("settings.environments.section.shared_by_team", "Warp \u{548C} {team_name} \u{5171}\u{4EAB}");
        m.insert("settings.environments.section.shared_by_default", "Warp \u{548C}\u{60A8}\u{7684}\u{56E2}\u{961F}\u{5171}\u{4EAB}");
        m.insert("settings.environments.empty.loading", "\u{52A0}\u{8F7D}\u{4E2D}...");
        m.insert("settings.environments.empty.retry", "\u{91CD}\u{8BD5}");
        m.insert("settings.environments.empty.authorize", "\u{6388}\u{6743}");
        m.insert("settings.environments.empty.get_started", "\u{5F00}\u{59CB}\u{4F7F}\u{7528}");
        m.insert("settings.environments.empty.launch_agent", "\u{542F}\u{52A8}\u{4EE3}\u{7406}");
        m.insert("settings.environments.empty.quick_setup", "\u{5FEB}\u{901F}\u{8BBE}\u{7F6E}");
        m.insert("settings.environments.empty.suggested", "\u{63A8}\u{8350}");
        m.insert("settings.environments.empty.github_subtitle", "\u{9009}\u{62E9}\u{60A8}\u{60F3}\u{8981}\u{4F7F}\u{7528}\u{7684} GitHub \u{4ED3}\u{5E93}\u{FF0C}\u{6211}\u{4EEC}\u{5C06}\u{4E3A}\u{60A8}\u{63A8}\u{8350}\u{57FA}\u{7840}\u{955C}\u{50CF}\u{548C}\u{914D}\u{7F6E}");
        m.insert("settings.environments.empty.use_agent", "\u{4F7F}\u{7528}\u{4EE3}\u{7406}");
        m.insert("settings.environments.empty.agent_subtitle", "\u{9009}\u{62E9}\u{4E00}\u{4E2A}\u{672C}\u{5730}\u{8BBE}\u{7F6E}\u{7684}\u{9879}\u{76EE}\u{FF0C}\u{6211}\u{4EEC}\u{5C06}\u{5E2E}\u{52A9}\u{60A8}\u{57FA}\u{4E8E}\u{5B83}\u{8BBE}\u{7F6E}\u{73AF}\u{5883}");
        m.insert("settings.environments.empty.no_envs_header", "\u{60A8}\u{8FD8}\u{6CA1}\u{6709}\u{8BBE}\u{7F6E}\u{4EFB}\u{4F55}\u{73AF}\u{5883}\u{3002}");
        m.insert("settings.environments.empty.no_envs_subheader", "\u{9009}\u{62E9}\u{60A8}\u{60F3}\u{8981}\u{5982}\u{4F55}\u{8BBE}\u{7F6E}\u{60A8}\u{7684}\u{73AF}\u{5883}\u{FF1A}");
        m.insert("settings.environments.card.env_id", "\u{73AF}\u{5883} ID\u{FF1A}{env_id}");
        m.insert("settings.environments.card.image", "\u{955C}\u{50CF}\u{FF1A}{image}");
        m.insert("settings.environments.card.repos", "\u{4ED3}\u{5E93}\u{FF1A}{repos}");
        m.insert("settings.environments.card.setup_commands", "\u{8BBE}\u{7F6E}\u{547D}\u{4EE4}\u{FF1A}{commands}");
        m.insert("settings.environments.card.view_runs", "\u{67E5}\u{770B}\u{6211}\u{7684}\u{8FD0}\u{884C}");
        m.insert("settings.environments.card.share", "\u{5171}\u{4EAB}");
        m.insert("settings.environments.card.edit", "\u{7F16}\u{8F91}");
        m.insert("settings.environments.timestamp.last_edited", "\u{6700}\u{540E}\u{7F16}\u{8F91}\u{FF1A}{duration}");
        m.insert("settings.environments.timestamp.last_used", "\u{6700}\u{540E}\u{4F7F}\u{7528}\u{FF1A}{duration}");
        m.insert("settings.environments.timestamp.last_used_never", "\u{6700}\u{540E}\u{4F7F}\u{7528}\u{FF1A}\u{4ECE}\u{672A}");
        m.insert("settings.environments.toast.updated", "\u{73AF}\u{5883}\u{66F4}\u{65B0}\u{6210}\u{529F}");
        m.insert("settings.environments.toast.created", "\u{73AF}\u{5883}\u{521B}\u{5EFA}\u{6210}\u{529F}");
        m.insert("settings.environments.toast.deleted", "\u{73AF}\u{5883}\u{5220}\u{9664}\u{6210}\u{529F}");
        m.insert("settings.environments.toast.shared", "\u{73AF}\u{5883}\u{5171}\u{4EAB}\u{6210}\u{529F}");
        m.insert("settings.environments.toast.share_failed", "\u{4E0E}\u{56E2}\u{961F}\u{5171}\u{4EAB}\u{73AF}\u{5883}\u{5931}\u{8D25}");
        m.insert("settings.environments.toast.create_not_logged_in", "\u{65E0}\u{6CD5}\u{521B}\u{5EFA}\u{73AF}\u{5883}\u{FF1A}\u{672A}\u{767B}\u{5F55}\u{3002}");
        m.insert("settings.environments.toast.save_not_found", "\u{65E0}\u{6CD5}\u{4FDD}\u{5B58}\u{FF1A}\u{73AF}\u{5883}\u{4E0D}\u{5B58}\u{5728}\u{3002}");
        m.insert("settings.environments.toast.share_no_team", "\u{65E0}\u{6CD5}\u{5171}\u{4EAB}\u{73AF}\u{5883}\u{FF1A}\u{60A8}\u{5F53}\u{524D}\u{4E0D}\u{5728}\u{56E2}\u{961F}\u{4E2D}\u{3002}");
        m.insert("settings.environments.toast.share_not_synced", "\u{65E0}\u{6CD5}\u{5171}\u{4EAB}\u{73AF}\u{5883}\u{FF1A}\u{73AF}\u{5883}\u{5C1A}\u{672A}\u{540C}\u{6B65}\u{3002}");

        // Warp Drive page
        m.insert("settings.warp_drive.create_account_prompt", "\u{8981}\u{4F7F}\u{7528} Warp Drive\u{FF0C}\u{8BF7}\u{5148}\u{521B}\u{5EFA}\u{8D26}\u{6237}\u{3002}");
        m.insert("settings.warp_drive.title", "Warp Drive");
        m.insert("settings.warp_drive.description", "Warp Drive \u{662F}\u{60A8}\u{7EC8}\u{7AEF}\u{4E2D}\u{7684}\u{5DE5}\u{4F5C}\u{7A7A}\u{95F4}\u{FF0C}\u{60A8}\u{53EF}\u{4EE5}\u{5728}\u{5176}\u{4E2D}\u{4FDD}\u{5B58}\u{5DE5}\u{4F5C}\u{6D41}\u{3001}\u{7B14}\u{8BB0}\u{672C}\u{3001}\u{63D0}\u{793A}\u{548C}\u{73AF}\u{5883}\u{53D8}\u{91CF}\u{FF0C}\u{7528}\u{4E8E}\u{4E2A}\u{4EBA}\u{4F7F}\u{7528}\u{6216}\u{4E0E}\u{56E2}\u{961F}\u{5171}\u{4EAB}\u{3002}");

        // Settings footer
        m.insert("settings.footer.open_settings_file", "\u{6253}\u{5F00}\u{8BBE}\u{7F6E}\u{6587}\u{4EF6}");
        m.insert("settings.footer.open_file", "\u{6253}\u{5F00}\u{6587}\u{4EF6}");
        m.insert("settings.footer.fix_with_oz", "\u{4F7F}\u{7528} Oz \u{4FEE}\u{590D}");

        // Transfer ownership confirmation
        m.insert("settings.transfer.confirm_message", "\u{60A8}\u{786E}\u{5B9A}\u{8981}\u{5C06}\u{56E2}\u{961F}\u{6240}\u{6709}\u{6743}\u{8F6C}\u{79FB}\u{7ED9} {email} \u{5417}\u{FF1F}\u{6B64}\u{64CD}\u{4F5C}\u{4E0D}\u{53EF}\u{64A4}\u{9500}\u{3002}\u{60A8}\u{5C06}\u{5931}\u{53BB}\u{7BA1}\u{7406}\u{5458}\u{6743}\u{9650}\u{3002}");
        m.insert("settings.transfer.button", "\u{8F6C}\u{79FB}");

        // Main page (Account)
        m.insert("settings.main.referral_cta", "\u{901A}\u{8FC7}\u{5411}\u{670B}\u{53CB}\u{548C}\u{540C}\u{4E8B}\u{5206}\u{4EAB} Warp \u{83B7}\u{53D6}\u{5956}\u{52B1}");
        m.insert("settings.main.log_out", "\u{9000}\u{51FA}\u{767B}\u{5F55}");
        m.insert("settings.main.free_plan", "\u{514D}\u{8D39}");
        m.insert("settings.main.compare_plans", "\u{6BD4}\u{8F83}\u{5957}\u{9910}");
        m.insert("settings.main.contact_support", "\u{8054}\u{7CFB}\u{652F}\u{6301}");
        m.insert("settings.main.manage_billing", "\u{7BA1}\u{7406}\u{8D26}\u{5355}");
        m.insert("settings.main.upgrade_turbo", "\u{5347}\u{7EA7}\u{5230} Turbo \u{5957}\u{9910}");
        m.insert("settings.main.upgrade_lightspeed", "\u{5347}\u{7EA7}\u{5230} Lightspeed \u{5957}\u{9910}");
        m.insert("settings.main.refer_friend", "\u{63A8}\u{8350}\u{7ED9}\u{670B}\u{53CB}");
        m.insert("settings.main.version", "\u{7248}\u{672C}");
        m.insert("settings.main.up_to_date", "\u{5DF2}\u{662F}\u{6700}\u{65B0}\u{7248}\u{672C}");
        m.insert("settings.main.check_updates", "\u{68C0}\u{67E5}\u{66F4}\u{65B0}");
        m.insert("settings.main.checking_update", "\u{6B63}\u{5728}\u{68C0}\u{67E5}\u{66F4}\u{65B0}...");
        m.insert("settings.main.downloading_update", "\u{6B63}\u{5728}\u{4E0B}\u{8F7D}\u{66F4}\u{65B0}...");
        m.insert("settings.main.update_available", "\u{6709}\u{66F4}\u{65B0}\u{53EF}\u{7528}");
        m.insert("settings.main.relaunch_warp", "\u{91CD}\u{65B0}\u{542F}\u{52A8} Warp");
        m.insert("settings.main.updating", "\u{6B63}\u{5728}\u{66F4}\u{65B0}...");
        m.insert("settings.main.installed_update", "\u{5DF2}\u{5B89}\u{88C5}\u{66F4}\u{65B0}");
        m.insert("settings.main.update_unavailable", "\u{6709}\u{65B0}\u{7248}\u{672C}\u{7684} Warp \u{53EF}\u{7528}\u{FF0C}\u{4F46}\u{65E0}\u{6CD5}\u{5B89}\u{88C5}");
        m.insert("settings.main.update_manually", "\u{624B}\u{52A8}\u{66F4}\u{65B0} Warp");
        m.insert("settings.main.update_launch_error", "\u{5DF2}\u{5B89}\u{88C5}\u{65B0}\u{7248}\u{672C}\u{7684} Warp\u{FF0C}\u{4F46}\u{65E0}\u{6CD5}\u{542F}\u{52A8}\u{3002}");
        m.insert("settings.main.settings_sync", "\u{8BBE}\u{7F6E}\u{540C}\u{6B65}");

        // Directory color picker
        m.insert("settings.dir_color.add_directory", "+ \u{6DFB}\u{52A0}\u{76EE}\u{5F55}\u{2026}");
        m.insert("settings.dir_color.add_button", "\u{6DFB}\u{52A0}\u{76EE}\u{5F55}\u{989C}\u{8272}");

        // Show blocks view (Shared blocks)
        m.insert("settings.show_blocks.unshare_confirm", "\u{60A8}\u{786E}\u{5B9A}\u{8981}\u{53D6}\u{6D88}\u{5171}\u{4EAB}\u{6B64}\u{5757}\u{5417}\u{FF1F}\n\nIt \u{5C06}\u{4E0D}\u{518D}\u{901A}\u{8FC7}\u{94FE}\u{63A5}\u{8BBF}\u{95EE}\u{FF0C}\u{5E76}\u{5C06}\u{4ECE} Warp \u{670D}\u{52A1}\u{5668}\u{4E2D}\u{6C38}\u{4E45}\u{5220}\u{9664}\u{3002}");
        m.insert("settings.show_blocks.no_shared_blocks", "\u{60A8}\u{8FD8}\u{6CA1}\u{6709}\u{5171}\u{4EAB}\u{5757}\u{3002}");
        m.insert("settings.show_blocks.getting_blocks", "\u{6B63}\u{5728}\u{83B7}\u{53D6}\u{5757}...");
        m.insert("settings.show_blocks.load_failed", "\u{52A0}\u{8F7D}\u{5757}\u{5931}\u{8D25}\u{3002}\u{8BF7}\u{91CD}\u{8BD5}\u{3002}");
        m.insert("settings.show_blocks.executed_on", "\u{6267}\u{884C}\u{65F6}\u{95F4}\u{FF1A}{timestamp}");
        m.insert("settings.show_blocks.link_copied", "\u{94FE}\u{63A5}\u{5DF2}\u{590D}\u{5236}\u{3002}");
        m.insert("settings.show_blocks.unshare_success", "\u{5757}\u{5DF2}\u{6210}\u{529F}\u{53D6}\u{6D88}\u{5171}\u{4EAB}\u{3002}");
        m.insert("settings.show_blocks.unshare_failed", "\u{53D6}\u{6D88}\u{5171}\u{4EAB}\u{5757}\u{5931}\u{8D25}\u{3002}\u{8BF7}\u{91CD}\u{8BD5}\u{3002}");
        m.insert("settings.show_blocks.unshare_title", "\u{53D6}\u{6D88}\u{5171}\u{4EAB}\u{5757}");
        m.insert("settings.show_blocks.deleting", "\u{6B63}\u{5728}\u{5220}\u{9664}...");
        m.insert("settings.show_blocks.copy_link", "\u{590D}\u{5236}\u{94FE}\u{63A5}");

        // Execution profile view
        m.insert("settings.execution_profile.edit", "\u{7F16}\u{8F91}");
        m.insert("settings.execution_profile.models", "\u{6A21}\u{578B}");
        m.insert("settings.execution_profile.base_model", "\u{57FA}\u{7840}\u{6A21}\u{578B}\u{FF1A}");
        m.insert("settings.execution_profile.full_terminal_use", "\u{5B8C}\u{5168}\u{7EC8}\u{7AEF}\u{4F7F}\u{7528}\u{FF1A}");
        m.insert("settings.execution_profile.computer_use", "\u{8BA1}\u{7B97}\u{673A}\u{4F7F}\u{7528}\u{FF1A}");
        m.insert("settings.execution_profile.permissions", "\u{6743}\u{9650}");
        m.insert("settings.execution_profile.apply_code_diffs", "\u{5E94}\u{7528}\u{4EE3}\u{7801}\u{5DEE}\u{5F02}\u{FF1A}");
        m.insert("settings.execution_profile.read_files", "\u{8BFB}\u{53D6}\u{6587}\u{4EF6}\u{FF1A}");
        m.insert("settings.execution_profile.execute_commands", "\u{6267}\u{884C}\u{547D}\u{4EE4}\u{FF1A}");
        m.insert("settings.execution_profile.interact_running", "\u{4E0E}\u{8FD0}\u{884C}\u{4E2D}\u{7684}\u{547D}\u{4EE4}\u{4EA4}\u{4E92}\u{FF1A}");
        m.insert("settings.execution_profile.ask_questions", "\u{63D0}\u{95EE}\u{FF1A}");
        m.insert("settings.execution_profile.call_mcp_servers", "\u{8C03}\u{7528} MCP \u{670D}\u{52A1}\u{5668}\u{FF1A}");
        m.insert("settings.execution_profile.call_web_tools", "\u{8C03}\u{7528}\u{7F51}\u{7EDC}\u{5DE5}\u{5177}\u{FF1A}");
        m.insert("settings.execution_profile.auto_sync_plans", "\u{81EA}\u{52A8}\u{540C}\u{6B65}\u{8BA1}\u{5212}\u{5230} Warp Drive\u{FF1A}");
        m.insert("settings.execution_profile.directory_allowlist", "\u{76EE}\u{5F55}\u{5141}\u{8BB8}\u{5217}\u{8868}\u{FF1A}");
        m.insert("settings.execution_profile.command_allowlist", "\u{547D}\u{4EE4}\u{5141}\u{8BB8}\u{5217}\u{8868}\u{FF1A}");
        m.insert("settings.execution_profile.command_denylist", "\u{547D}\u{4EE4}\u{62D2}\u{7EDD}\u{5217}\u{8868}\u{FF1A}");
        m.insert("settings.execution_profile.mcp_allowlist", "MCP \u{5141}\u{8BB8}\u{5217}\u{8868}\u{FF1A}");
        m.insert("settings.execution_profile.mcp_denylist", "MCP \u{62D2}\u{7EDD}\u{5217}\u{8868}\u{FF1A}");
        m.insert("settings.execution_profile.none", "\u{65E0}");
        m.insert("settings.execution_profile.agent_decides", "\u{4EE3}\u{7406}\u{51B3}\u{5B9A}");
        m.insert("settings.execution_profile.always_allow", "\u{59CB}\u{7EC8}\u{5141}\u{8BB8}");
        m.insert("settings.execution_profile.always_ask", "\u{59CB}\u{7EC8}\u{8BE2}\u{95EE}");
        m.insert("settings.execution_profile.unknown", "\u{672A}\u{77E5}");
        m.insert("settings.execution_profile.ask_on_first_write", "\u{9996}\u{6B21}\u{5199}\u{5165}\u{65F6}\u{8BE2}\u{95EE}");
        m.insert("settings.execution_profile.never", "\u{4ECE}\u{4E0D}");
        m.insert("settings.execution_profile.never_ask", "\u{4ECE}\u{4E0D}\u{8BE2}\u{95EE}");
        m.insert("settings.execution_profile.ask_unless_auto_approve", "\u{9664}\u{975E}\u{81EA}\u{52A8}\u{6279}\u{51C6}\u{5426}\u{5219}\u{8BE2}\u{95EE}");
        m.insert("settings.execution_profile.on", "\u{5F00}\u{542F}");
        m.insert("settings.execution_profile.off", "\u{5173}\u{95ED}");

        // Delete environment confirmation dialog
        m.insert("settings.delete_env.title", "\u{5220}\u{9664}\u{73AF}\u{5883}\u{FF1F}");
        m.insert("settings.delete_env.description", "\u{60A8}\u{786E}\u{5B9A}\u{8981}\u{5220}\u{9664} {env_name} \u{73AF}\u{5883}\u{5417}\u{FF1F}");
        m.insert("settings.delete_env.confirm", "\u{5220}\u{9664}\u{73AF}\u{5883}");

        // Environment form
        m.insert("settings.env_form.create", "\u{521B}\u{5EFA}");
        m.insert("settings.env_form.save", "\u{4FDD}\u{5B58}");
        m.insert("settings.env_form.delete_env", "\u{5220}\u{9664}\u{73AF}\u{5883}");
        m.insert("settings.env_form.create_env", "\u{521B}\u{5EFA}\u{73AF}\u{5883}");
        m.insert("settings.env_form.edit_env", "\u{7F16}\u{8F91}\u{73AF}\u{5883}");
        m.insert("settings.env_form.save_env", "\u{4FDD}\u{5B58}\u{73AF}\u{5883}");
        m.insert("settings.env_form.share_with_team", "\u{4E0E}\u{56E2}\u{961F}\u{5171}\u{4EAB}");
        m.insert("settings.env_form.name_placeholder", "\u{73AF}\u{5883}\u{540D}\u{79F0}");
        m.insert("settings.env_form.description_label", "\u{63CF}\u{8FF0}");
        m.insert("settings.env_form.char_count", "{count} / {max} \u{4E2A}\u{5B57}\u{7B26}");
        m.insert("settings.env_form.repos_label", "\u{4ED3}\u{5E93}");
        m.insert("settings.env_form.docker_image_label", "Docker \u{955C}\u{50CF}\u{5F15}\u{7528}");
        m.insert("settings.env_form.suggest_image", "\u{63A8}\u{8350}\u{955C}\u{50CF}");
        m.insert("settings.env_form.launch_agent", "\u{542F}\u{52A8}\u{4EE3}\u{7406}");
        m.insert("settings.env_form.authenticate", "\u{8BA4}\u{8BC1}");
        m.insert("settings.env_form.auth_github", "\u{4F7F}\u{7528} GitHub \u{8BA4}\u{8BC1}");
        m.insert("settings.env_form.retry", "\u{91CD}\u{8BD5}");
        m.insert("settings.env_form.no_repos_found", "\u{672A}\u{627E}\u{5230}\u{4ED3}\u{5E93}");
        m.insert("settings.env_form.configure_github", "\u{5728} GitHub \u{4E0A}\u{914D}\u{7F6E}\u{8BBF}\u{95EE}");
        m.insert("settings.env_form.grant_access_hint", "\u{60A8}\u{9700}\u{8981}\u{6388}\u{6743}\u{8BBF}\u{95EE}\u{60A8}\u{7684} GitHub \u{4ED3}\u{5E93}\u{624D}\u{80FD}\u{63A8}\u{8350} Docker \u{955C}\u{50CF}");
        m.insert("settings.env_form.share_warning", "\u{4E2A}\u{4EBA}\u{73AF}\u{5883}\u{65E0}\u{6CD5}\u{4E0E}\u{5916}\u{90E8}\u{96C6}\u{6210}\u{6216}\u{56E2}\u{961F} API \u{5BC6}\u{94A5}\u{914D}\u{5408}\u{4F7F}\u{7528}\u{3002}\u{4E3A}\u{83B7}\u{5F97}\u{6700}\u{4F73}\u{4F53}\u{9A8C}\u{FF0C}\u{8BF7}\u{4F7F}\u{7528}\u{5171}\u{4EAB}\u{73AF}\u{5883}\u{3002}");

        // Environment modal
        m.insert("settings.env_modal.title", "\u{4E3A}\u{60A8}\u{7684}\u{73AF}\u{5883}\u{9009}\u{62E9}\u{4ED3}\u{5E93}");
        m.insert("settings.env_modal.description.indexed", "\u{9009}\u{62E9}\u{672C}\u{5730}\u{5DF2}\u{7D22}\u{5F15}\u{7684}\u{4ED3}\u{5E93}\u{FF0C}\u{4E3A}\u{73AF}\u{5883}\u{521B}\u{5EFA}\u{4EE3}\u{7406}\u{63D0}\u{4F9B}\u{4E0A}\u{4E0B}\u{6587}\u{3002}");
        m.insert("settings.env_modal.description.default", "\u{9009}\u{62E9}\u{4ED3}\u{5E93}\u{FF0C}\u{4E3A}\u{73AF}\u{5883}\u{521B}\u{5EFA}\u{4EE3}\u{7406}\u{63D0}\u{4F9B}\u{4E0A}\u{4E0B}\u{6587}\u{3002}");
        m.insert("settings.env_modal.section.selected_repos", "\u{5DF2}\u{9009}\u{62E9}\u{7684}\u{4ED3}\u{5E93}");
        m.insert("settings.env_modal.section.available_repos", "\u{53EF}\u{7528}\u{7684}\u{5DF2}\u{7D22}\u{5F15}\u{4ED3}\u{5E93}");
        m.insert("settings.env_modal.empty.no_selected", "\u{5C1A}\u{672A}\u{9009}\u{62E9}\u{4EFB}\u{4F55}\u{4ED3}\u{5E93}");
        m.insert("settings.env_modal.empty.all_selected", "\u{6240}\u{6709}\u{672C}\u{5730}\u{5DF2}\u{7D22}\u{5F15}\u{7684}\u{4ED3}\u{5E93}\u{5DF2}\u{5168}\u{90E8}\u{9009}\u{62E9}\u{3002}");
        m.insert("settings.env_modal.empty.no_indexed", "\u{5C1A}\u{672A}\u{627E}\u{5230}\u{672C}\u{5730}\u{5DF2}\u{7D22}\u{5F15}\u{7684}\u{4ED3}\u{5E93}\u{3002}\u{8BF7}\u{5148}\u{7D22}\u{5F15}\u{4E00}\u{4E2A}\u{4ED3}\u{5E93}\u{FF0C}\u{7136}\u{540E}\u{91CD}\u{8BD5}\u{3002}");
        m.insert("settings.env_modal.empty.unavailable", "\u{6B64}\u{6784}\u{5EFA}\u{7248}\u{672C}\u{4E0D}\u{652F}\u{6301}\u{672C}\u{5730}\u{4ED3}\u{5E93}\u{9009}\u{62E9}\u{3002}");
        m.insert("settings.env_modal.loading", "\u{6B63}\u{5728}\u{52A0}\u{8F7D}\u{672C}\u{5730}\u{5DF2}\u{7D22}\u{5F15}\u{7684}\u{4ED3}\u{5E93}\u{2026}");
        m.insert("settings.env_modal.button.add_repo", "\u{6DFB}\u{52A0}\u{4ED3}\u{5E93}");
        m.insert("settings.env_modal.button.create_environment", "\u{521B}\u{5EFA}\u{73AF}\u{5883}");
        m.insert("settings.env_modal.toast.not_git_repo", "\u{9009}\u{62E9}\u{7684}\u{6587}\u{4EF6}\u{5939}\u{4E0D}\u{662F} Git \u{4ED3}\u{5E93}\u{FF1A}{path}");

        // Platform page
        m.insert("settings.platform.new_api_key", "\u{65B0}\u{5EFA} API \u{5BC6}\u{94A5}");
        m.insert("settings.platform.save_your_key", "\u{4FDD}\u{5B58}\u{60A8}\u{7684}\u{5BC6}\u{94A5}");
        m.insert("settings.platform.api_key_deleted", "API \u{5BC6}\u{94A5}\u{5DF2}\u{5220}\u{9664}");
        m.insert("settings.platform.oz_cloud_api_keys", "Oz \u{4E91} API \u{5BC6}\u{94A5}");
        m.insert("settings.platform.create_api_key", "+ \u{521B}\u{5EFA} API \u{5BC6}\u{94A5}");
        m.insert("settings.platform.description", "\u{521B}\u{5EFA}\u{548C}\u{7BA1}\u{7406} API \u{5BC6}\u{94A5}\u{4EE5}\u{5141}\u{8BB8}\u{5176}\u{4ED6} Oz \u{4E91}\u{4EE3}\u{7406}\u{8BBF}\u{95EE}\u{60A8}\u{7684} Warp \u{8D26}\u{6237}\u{3002}\n\u{6B32}\u{4E86}\u{89E3}\u{66F4}\u{591A}\u{4FE1}\u{606F}\u{FF0C}\u{8BF7}\u{8BBF}\u{95EE} ");
        m.insert("settings.platform.documentation", "\u{6587}\u{6863}\u{3002}");
        m.insert("settings.platform.header.name", "\u{540D}\u{79F0}");
        m.insert("settings.platform.header.key", "\u{5BC6}\u{94A5}");
        m.insert("settings.platform.header.scope", "\u{8303}\u{56F4}");
        m.insert("settings.platform.header.created", "\u{521B}\u{5EFA}\u{65F6}\u{95F4}");
        m.insert("settings.platform.header.last_used", "\u{4E0A}\u{6B21}\u{4F7F}\u{7528}");
        m.insert("settings.platform.header.expires_at", "\u{8FC7}\u{671F}\u{65F6}\u{95F4}");
        m.insert("settings.platform.never", "\u{4ECE}\u{672A}");
        m.insert("settings.platform.scope.personal", "\u{4E2A}\u{4EBA}");
        m.insert("settings.platform.scope.team", "\u{56E2}\u{961F}");
        m.insert("settings.platform.no_api_keys", "\u{65E0} API \u{5BC6}\u{94A5}");
        m.insert("settings.platform.no_api_keys_description", "\u{521B}\u{5EFA}\u{5BC6}\u{94A5}\u{4EE5}\u{7BA1}\u{7406}\u{5BF9} Warp \u{7684}\u{5916}\u{90E8}\u{8BBF}\u{95EE}");

        // MCP Servers page
        m.insert("settings.mcp.page_title", "MCP \u{670D}\u{52A1}\u{5668}");
        m.insert("settings.mcp.logout_success_with_name", "\u{5DF2}\u{6210}\u{529F}\u{9000}\u{51FA} {name} MCP \u{670D}\u{52A1}\u{5668}");
        m.insert("settings.mcp.logout_success", "\u{5DF2}\u{6210}\u{529F}\u{9000}\u{51FA} MCP \u{670D}\u{52A1}\u{5668}");
        m.insert("settings.mcp.finish_current_install", "\u{8BF7}\u{5148}\u{5B8C}\u{6210}\u{5F53}\u{524D}\u{7684} MCP \u{5B89}\u{88C5}\u{FF0C}\u{518D}\u{6253}\u{5F00}\u{53E6}\u{4E00}\u{4E2A}\u{5B89}\u{88C5}\u{94FE}\u{63A5}\u{3002}");
        m.insert("settings.mcp.unknown_server", "\u{672A}\u{77E5}\u{7684} MCP \u{670D}\u{52A1}\u{5668} '{autoinstall_param}'");
        m.insert("settings.mcp.cannot_install_from_link", "MCP \u{670D}\u{52A1}\u{5668} '{gallery_title}' \u{65E0}\u{6CD5}\u{4ECE}\u{6B64}\u{94FE}\u{63A5}\u{5B89}\u{88C5}\u{3002}");

        // Warpify page
        m.insert("settings.warpify.title", "Warpify");
        m.insert("settings.warpify.description", "\u{914D}\u{7F6E} Warp \u{662F}\u{5426}\u{5C1D}\u{8BD5} \u{201c}Warpify\u{201D}\u{FF08}\u{6DFB}\u{52A0}\u{5BF9}\u{5757}\u{3001}\u{8F93}\u{5165}\u{6A21}\u{5F0F}\u{7B49}\u{7684}\u{652F}\u{6301}\u{FF09}\u{67D0}\u{4E9B} shell\u{3002} ");
        m.insert("settings.warpify.learn_more", "\u{4E86}\u{89E3}\u{66F4}\u{591A}");
        m.insert("settings.warpify.subshells", "\u{5B50} Shell");
        m.insert("settings.warpify.subshells_description", "\u{652F}\u{6301}\u{7684}\u{5B50} Shell\u{FF1A}bash\u{3001}zsh \u{548C} fish\u{3002}");
        m.insert("settings.warpify.ssh", "SSH");
        m.insert("settings.warpify.ssh_description", "\u{5BF9}\u{60A8}\u{7684}\u{4EA4}\u{4E92}\u{5F0F} SSH \u{4F1A}\u{8BDD}\u{8FDB}\u{884C} Warpify\u{3002}");
        m.insert("settings.warpify.ssh_session_detection", "SSH \u{4F1A}\u{8BDD}\u{68C0}\u{6D4B}\u{FF08}\u{7528}\u{4E8E} Warpification\u{FF09}");
        m.insert("settings.warpify.placeholder_command", "\u{547D}\u{4EE4}\u{FF08}\u{652F}\u{6301}\u{6B63}\u{5219}\u{8868}\u{8FBE}\u{5F0F}\u{FF09}");
        m.insert("settings.warpify.placeholder_host", "\u{4E3B}\u{673A}\u{FF08}\u{652F}\u{6301}\u{6B63}\u{5219}\u{8868}\u{8FBE}\u{5F0F}\u{FF09}");
        m.insert("settings.warpify.added_commands", "\u{5DF2}\u{6DFB}\u{52A0}\u{7684}\u{547D}\u{4EE4}");
        m.insert("settings.warpify.denylisted_commands", "\u{62D2}\u{7EDD}\u{5217}\u{8868}\u{4E2D}\u{7684}\u{547D}\u{4EE4}");
        m.insert("settings.warpify.warpify_ssh_sessions", "Warpify SSH \u{4F1A}\u{8BDD}");
        m.insert("settings.warpify.install_ssh_extension", "\u{5B89}\u{88C5} SSH \u{6269}\u{5C55}");
        m.insert("settings.warpify.ssh_extension_install_mode_description", "\u{63A7}\u{5236}\u{5F53}\u{8FDC}\u{7A0B}\u{4E3B}\u{673A}\u{672A}\u{5B89}\u{88C5} Warp SSH \u{6269}\u{5C55}\u{65F6}\u{7684}\u{5B89}\u{88C5}\u{884C}\u{4E3A}\u{3002}");
        m.insert("settings.warpify.use_tmux_warpification", "\u{4F7F}\u{7528} Tmux Warpification");
        m.insert("settings.warpify.ssh_tmux_warpification_description", "tmux ssh \u{5305}\u{88C5}\u{5668}\u{5728}\u{9ED8}\u{8BA4}\u{5305}\u{88C5}\u{5668}\u{4E0D}\u{8D77}\u{4F5C}\u{7528}\u{7684}\u{5F88}\u{591A}\u{60C5}\u{51B5}\u{4E0B}\u{90FD}\u{53EF}\u{4EE5}\u{5DE5}\u{4F5C}\u{FF0C}\u{4F46}\u{53EF}\u{80FD}\u{9700}\u{8981}\u{60A8}\u{70B9}\u{51FB}\u{6309}\u{94AE}\u{6765}\u{8FDB}\u{884C} warpify\u{3002}\u{5728}\u{65B0}\u{6807}\u{7B7E}\u{9875}\u{4E2D}\u{751F}\u{6548}\u{3002}");
        m.insert("settings.warpify.denylisted_hosts", "\u{62D2}\u{7EDD}\u{5217}\u{8868}\u{4E2D}\u{7684}\u{4E3B}\u{673A}");

        // Referrals page
        m.insert("settings.referrals.header", "\u{9080}\u{8BF7}\u{670B}\u{53CB}\u{52A0}\u{5165} Warp");
        m.insert("settings.referrals.anonymous_header", "\u{6CE8}\u{518C}\u{4EE5}\u{53C2}\u{4E0E} Warp \u{7684}\u{63A8}\u{8350}\u{8BA1}\u{5212}");
        m.insert("settings.referrals.link_error", "\u{52A0}\u{8F7D}\u{63A8}\u{8350}\u{7801}\u{5931}\u{8D25}\u{3002}");
        m.insert("settings.referrals.copy_link", "\u{590D}\u{5236}\u{94FE}\u{63A5}");
        m.insert("settings.referrals.send_email", "\u{53D1}\u{9001}");
        m.insert("settings.referrals.sending", "\u{53D1}\u{9001}\u{4E2D}...");
        m.insert("settings.referrals.loading", "\u{52A0}\u{8F7D}\u{4E2D}...");
        m.insert("settings.referrals.link_copied", "\u{94FE}\u{63A5}\u{5DF2}\u{590D}\u{5236}\u{3002}");
        m.insert("settings.referrals.email_success", "\u{90AE}\u{4EF6}\u{53D1}\u{9001}\u{6210}\u{529F}\u{3002}");
        m.insert("settings.referrals.email_failure", "\u{90AE}\u{4EF6}\u{53D1}\u{9001}\u{5931}\u{8D25}\u{3002}\u{8BF7}\u{91CD}\u{8BD5}\u{3002}");
        m.insert("settings.referrals.reward_intro", "\u{5F53}\u{60A8}\u{63A8}\u{8350}\u{6709}\u{4EBA}\u{65F6}\u{FF0C}\u{53EF}\u{83B7}\u{53D6} Warp \u{72EC}\u{5BB6}\u{5468}\u{8FB9}*");
        m.insert("settings.referrals.terms_link", "\u{67D0}\u{4E9B}\u{9650}\u{5236}\u{9002}\u{7528}\u{3002}");
        m.insert("settings.referrals.terms_contact", " \u{5982}\u{679C}\u{60A8}\u{5BF9}\u{63A8}\u{8350}\u{8BA1}\u{5212}\u{6709}\u{4EFB}\u{4F55}\u{7591}\u{95EE}\u{FF0C}\u{8BF7}\u{8054}\u{7CFB} referrals@warp.dev\u{3002}");
        m.insert("settings.referrals.current_referral_singular", "\u{5F53}\u{524D}\u{63A8}\u{8350}");
        m.insert("settings.referrals.current_referral_plural", "\u{5F53}\u{524D}\u{63A8}\u{8350}");
        m.insert("settings.referrals.link_label", "\u{94FE}\u{63A5}");
        m.insert("settings.referrals.email_label", "\u{90AE}\u{7BB1}");
        m.insert("settings.referrals.sign_up", "\u{6CE8}\u{518C}");
        m.insert("settings.referrals.enter_email_error", "\u{8BF7}\u{8F93}\u{5165}\u{90AE}\u{7BB1}\u{3002}");
        m.insert("settings.referrals.invalid_email_error", "\u{8BF7}\u{786E}\u{4FDD}\u{4EE5}\u{4E0B}\u{90AE}\u{7BB1}\u{6709}\u{6548}\u{FF1A}{invalid_email}");
        m.insert("settings.referrals.reward_exclusive_theme", "\u{72EC}\u{5BB6}\u{4E3B}\u{9898}");
        m.insert("settings.referrals.reward_keycaps_stickers", "\u{952E}\u{5E3D} + \u{8D34}\u{7EB8}");
        m.insert("settings.referrals.reward_tshirt", "T \u{604B}");
        m.insert("settings.referrals.reward_notebook", "\u{7B14}\u{8BB0}\u{672C}");
        m.insert("settings.referrals.reward_baseball_cap", "\u{68D2}\u{7403}\u{5E3D}");
        m.insert("settings.referrals.reward_hoodie", "\u{8FDE}\u{5E3D}\u{8863}");
        m.insert("settings.referrals.reward_hydro_flask", "\u{9AD8}\u{7EA7} Hydro Flask");
        m.insert("settings.referrals.reward_backpack", "\u{80CC}\u{5305}");

        // Teams page
        m.insert("settings.teams.header", "\u{56E2}\u{961F}");
        m.insert("settings.teams.create.title", "\u{521B}\u{5EFA}\u{56E2}\u{961F}");
        m.insert("settings.teams.create.description", "\u{521B}\u{5EFA}\u{56E2}\u{961F}\u{540E}\u{FF0C}\u{60A8}\u{53EF}\u{4EE5}\u{901A}\u{8FC7}\u{5171}\u{4EAB}\u{4E91}\u{7AEF}\u{4EE3}\u{7406}\u{8FD0}\u{884C}\u{3001}\u{73AF}\u{5883}\u{3001}\u{81EA}\u{52A8}\u{5316}\u{548C}\u{5DE5}\u{4EF6}\u{6765}\u{8FDB}\u{884C}\u{4EE3}\u{7406}\u{9A71}\u{52A8}\u{7684}\u{534F}\u{4F5C}\u{5F00}\u{53D1}\u{3002}\u{60A8}\u{8FD8}\u{53EF}\u{4EE5}\u{4E3A}\u{56E2}\u{961F}\u{6210}\u{5458}\u{548C}\u{4EE3}\u{7406}\u{521B}\u{5EFA}\u{5171}\u{4EAB}\u{77E5}\u{8BC6}\u{5E93}\u{3002}");
        m.insert("settings.teams.create.team_name_placeholder", "\u{56E2}\u{961F}\u{540D}\u{79F0}");
        m.insert("settings.teams.create.button", "\u{521B}\u{5EFA}");
        m.insert("settings.teams.create.discoverable_checkbox_domain", "\u{5141}\u{8BB8}\u{4F7F}\u{7528} @{domain} \u{90AE}\u{7BB1}\u{7684} Warp \u{7528}\u{6237}\u{67E5}\u{627E}\u{5E76}\u{52A0}\u{5165}\u{6B64}\u{56E2}\u{961F}\u{3002}");
        m.insert("settings.teams.create.discoverable_checkbox_generic", "\u{5141}\u{8BB8}\u{4F7F}\u{7528}\u{76F8}\u{540C}\u{90AE}\u{7BB1}\u{57DF}\u{540D}\u{7684} Warp \u{7528}\u{6237}\u{67E5}\u{627E}\u{5E76}\u{52A0}\u{5165}\u{6B64}\u{56E2}\u{961F}\u{3002}");
        m.insert("settings.teams.create.join_existing", "\u{6216}\u{8005}\u{FF0C}\u{52A0}\u{5165}\u{516C}\u{53F8}\u{5185}\u{73B0}\u{6709}\u{7684}\u{56E2}\u{961F}");
        m.insert("settings.teams.manage.leave_team", "\u{79BB}\u{5F00}\u{56E2}\u{961F}");
        m.insert("settings.teams.manage.delete_team", "\u{5220}\u{9664}\u{56E2}\u{961F}");
        m.insert("settings.teams.manage.rename_placeholder", "\u{60A8}\u{7684}\u{65B0}\u{56E2}\u{961F}\u{540D}\u{79F0}");
        m.insert("settings.teams.manage.transfer_ownership_title", "\u{8F6C}\u{79FB}\u{56E2}\u{961F}\u{6240}\u{6709}\u{6743}\u{FF1F}");
        m.insert("settings.teams.manage.contact_support", "\u{8054}\u{7CFB}\u{5BA2}\u{670D}");
        m.insert("settings.teams.manage.manage_billing", "\u{7BA1}\u{7406}\u{8D26}\u{5355}");
        m.insert("settings.teams.manage.open_admin_panel", "\u{6253}\u{5F00}\u{7BA1}\u{7406}\u{9762}\u{677F}");
        m.insert("settings.teams.manage.manage_plan", "\u{7BA1}\u{7406}\u{65B9}\u{6848}");
        m.insert("settings.teams.invite.by_link", "\u{901A}\u{8FC7}\u{94FE}\u{63A5}\u{9080}\u{8BF7}");
        m.insert("settings.teams.invite.link_toggle_instructions", "\u{4F5C}\u{4E3A}\u{7BA1}\u{7406}\u{5458}\u{FF0C}\u{60A8}\u{53EF}\u{4EE5}\u{9009}\u{62E9}\u{542F}\u{7528}\u{6216}\u{7981}\u{7528}\u{56E2}\u{961F}\u{6210}\u{5458}\u{901A}\u{8FC7}\u{9080}\u{8BF7}\u{94FE}\u{63A5}\u{9080}\u{8BF7}\u{4ED6}\u{4EBA}\u{7684}\u{529F}\u{80FD}\u{3002}");
        m.insert("settings.teams.invite.reset_links", "\u{91CD}\u{7F6E}\u{94FE}\u{63A5}");
        m.insert("settings.teams.invite.restrict_by_domain", "\u{6309}\u{57DF}\u{540D}\u{9650}\u{5236}");
        m.insert("settings.teams.invite.domain_restrictions_instructions", "\u{4EC5}\u{5141}\u{8BB8}\u{62E5}\u{6709}\u{7279}\u{5B9A}\u{57DF}\u{540D}\u{90AE}\u{7BB1}\u{7684}\u{7528}\u{6237}\u{901A}\u{8FC7}\u{9080}\u{8BF7}\u{94FE}\u{63A5}\u{52A0}\u{5165}\u{60A8}\u{7684}\u{56E2}\u{961F}\u{3002}");
        m.insert("settings.teams.invite.domains_placeholder", "\u{57DF}\u{540D}\u{FF0C}\u{7528}\u{9017}\u{53F7}\u{5206}\u{9694}");
        m.insert("settings.teams.invite.set_button", "\u{8BBE}\u{7F6E}");
        m.insert("settings.teams.invite.invalid_domains", "\u{90E8}\u{5206}\u{63D0}\u{4F9B}\u{7684}\u{57DF}\u{540D}\u{65E0}\u{6548}\u{6216}\u{5DF2}\u{88AB}\u{6DFB}\u{52A0}\u{3002}");
        m.insert("settings.teams.invite.failed_load_link", "\u{52A0}\u{8F7D}\u{9080}\u{8BF7}\u{94FE}\u{63A5}\u{5931}\u{8D25}\u{3002}");
        m.insert("settings.teams.invite.by_email", "\u{901A}\u{8FC7}\u{90AE}\u{7BB1}\u{9080}\u{8BF7}");
        m.insert("settings.teams.invite.email_expiry_instructions", "\u{90AE}\u{7BB1}\u{9080}\u{8BF7}\u{6709}\u{6548}\u{671F}\u{4E3A} 7 \u{5929}\u{3002}");
        m.insert("settings.teams.invite.emails_placeholder", "\u{90AE}\u{7BB1}\u{5730}\u{5740}\u{FF0C}\u{7528}\u{9017}\u{53F7}\u{5206}\u{9694}");
        m.insert("settings.teams.invite.invite_button", "\u{9080}\u{8BF7}");
        m.insert("settings.teams.invite.invalid_emails", "\u{90E8}\u{5206}\u{63D0}\u{4F9B}\u{7684}\u{90AE}\u{7BB1}\u{5730}\u{5740}\u{65E0}\u{6548}\u{3001}\u{5DF2}\u{88AB}\u{9080}\u{8BF7}\u{6216}\u{5DF2}\u{662F}\u{56E2}\u{961F}\u{6210}\u{5458}\u{3002}");
        m.insert("settings.teams.members.header", "\u{56E2}\u{961F}\u{6210}\u{5458}");
        m.insert("settings.teams.members.cancel_invite", "\u{53D6}\u{6D88}\u{9080}\u{8BF7}");
        m.insert("settings.teams.members.transfer_ownership", "\u{8F6C}\u{79FB}\u{6240}\u{6709}\u{6743}");
        m.insert("settings.teams.members.demote_from_admin", "\u{964D}\u{7EA7}\u{4E3A}\u{666E}\u{901A}\u{6210}\u{5458}");
        m.insert("settings.teams.members.promote_to_admin", "\u{63D0}\u{5347}\u{4E3A}\u{7BA1}\u{7406}\u{5458}");
        m.insert("settings.teams.members.remove_from_team", "\u{4ECE}\u{56E2}\u{961F}\u{79FB}\u{9664}");
        m.insert("settings.teams.members.remove_domain", "\u{79FB}\u{9664}\u{57DF}\u{540D}");
        m.insert("settings.teams.badge.expired", "\u{5DF2}\u{8FC7}\u{671F}");
        m.insert("settings.teams.badge.pending", "\u{5F85}\u{5904}\u{7406}");
        m.insert("settings.teams.badge.owner", "\u{6240}\u{6709}\u{8005}");
        m.insert("settings.teams.badge.admin", "\u{7BA1}\u{7406}\u{5458}");
        m.insert("settings.teams.badge.past_due", "\u{903E}\u{671F}");
        m.insert("settings.teams.badge.unpaid", "\u{672A}\u{4ED8}\u{6B3E}");
        m.insert("settings.teams.plan.free_usage_limits", "\u{514D}\u{8D39}\u{65B9}\u{6848}\u{4F7F}\u{7528}\u{9650}\u{5236}");
        m.insert("settings.teams.plan.usage_limits", "\u{65B9}\u{6848}\u{4F7F}\u{7528}\u{9650}\u{5236}");
        m.insert("settings.teams.plan.shared_notebooks", "\u{5171}\u{4EAB}\u{7B14}\u{8BB0}\u{672C}");
        m.insert("settings.teams.plan.shared_workflows", "\u{5171}\u{4EAB}\u{5DE5}\u{4F5C}\u{6D41}");
        m.insert("settings.teams.limit.admin", "\u{60A8}\u{5DF2}\u{8FBE}\u{5230}\u{65B9}\u{6848}\u{7684}\u{56E2}\u{961F}\u{6210}\u{5458}\u{4E0A}\u{9650}\u{3002}\u{5347}\u{7EA7}\u{4EE5}\u{6DFB}\u{52A0}\u{66F4}\u{591A}\u{6210}\u{5458}\u{3002}");
        m.insert("settings.teams.limit.admin_not_upgradeable", "\u{60A8}\u{5DF2}\u{8FBE}\u{5230}\u{65B9}\u{6848}\u{7684}\u{56E2}\u{961F}\u{6210}\u{5458}\u{4E0A}\u{9650}\u{3002}\u{8BF7}\u{8054}\u{7CFB} support@warp.dev \u{4EE5}\u{6DFB}\u{52A0}\u{66F4}\u{591A}\u{6210}\u{5458}\u{3002}");
        m.insert("settings.teams.limit.non_admin", "\u{60A8}\u{5DF2}\u{8FBE}\u{5230}\u{65B9}\u{6848}\u{7684}\u{56E2}\u{961F}\u{6210}\u{5458}\u{4E0A}\u{9650}\u{3002}\u{8BF7}\u{8054}\u{7CFB}\u{56E2}\u{961F}\u{7BA1}\u{7406}\u{5458}\u{4EE5}\u{6DFB}\u{52A0}\u{66F4}\u{591A}\u{6210}\u{5458}\u{3002}");
        m.insert("settings.teams.limit_exceeded.admin_upgradeable", "\u{60A8}\u{5DF2}\u{8D85}\u{51FA}\u{65B9}\u{6848}\u{7684}\u{56E2}\u{961F}\u{6210}\u{5458}\u{4E0A}\u{9650}\u{3002}\u{5347}\u{7EA7}\u{4EE5}\u{6DFB}\u{52A0}\u{66F4}\u{591A}\u{6210}\u{5458}\u{3002}");
        m.insert("settings.teams.limit_exceeded.admin_not_upgradeable", "\u{60A8}\u{5DF2}\u{8D85}\u{51FA}\u{65B9}\u{6848}\u{7684}\u{56E2}\u{961F}\u{6210}\u{5458}\u{4E0A}\u{9650}\u{3002}\u{8BF7}\u{8054}\u{7CFB} support@warp.dev \u{5347}\u{7EA7}\u{60A8}\u{7684}\u{56E2}\u{961F}\u{3002}");
        m.insert("settings.teams.limit_exceeded.non_admin", "\u{60A8}\u{5DF2}\u{8D85}\u{51FA}\u{65B9}\u{6848}\u{7684}\u{56E2}\u{961F}\u{6210}\u{5458}\u{4E0A}\u{9650}\u{3002}\u{8BF7}\u{8054}\u{7CFB}\u{56E2}\u{961F}\u{7BA1}\u{7406}\u{5458}\u{5347}\u{7EA7}\u{60A8}\u{7684}\u{56E2}\u{961F}\u{3002}");
        m.insert("settings.teams.delinquent.admin_non_self_serve", "\u{7531}\u{4E8E}\u{4ED8}\u{6B3E}\u{95EE}\u{9898}\u{FF0C}\u{56E2}\u{961F}\u{9080}\u{8BF7}\u{5DF2}\u{88AB}\u{9650}\u{5236}\u{3002}\u{8BF7}\u{8054}\u{7CFB} support@warp.dev \u{6062}\u{590D}\u{8BBF}\u{95EE}\u{3002}");
        m.insert("settings.teams.delinquent.non_admin", "\u{7531}\u{4E8E}\u{4ED8}\u{6B3E}\u{95EE}\u{9898}\u{FF0C}\u{56E2}\u{961F}\u{9080}\u{8BF7}\u{5DF2}\u{88AB}\u{9650}\u{5236}\u{3002}\u{8BF7}\u{8054}\u{7CFB}\u{56E2}\u{961F}\u{7BA1}\u{7406}\u{5458}\u{6062}\u{590D}\u{8BBF}\u{95EE}\u{3002}");
        m.insert("settings.teams.delinquent.admin_self_serve_line1", "\u{7531}\u{4E8E}\u{8BA2}\u{9605}\u{4ED8}\u{6B3E}\u{95EE}\u{9898}\u{FF0C}\u{56E2}\u{961F}\u{9080}\u{8BF7}\u{5DF2}\u{88AB}\u{9650}\u{5236}\u{3002}");
        m.insert("settings.teams.delinquent.admin_self_serve_line2_prefix", "\u{8BF7}");
        m.insert("settings.teams.delinquent.admin_self_serve_line2_link", "\u{66F4}\u{65B0}\u{60A8}\u{7684}\u{4ED8}\u{6B3E}\u{4FE1}\u{606F}");
        m.insert("settings.teams.delinquent.admin_self_serve_line2_suffix", "\u{4EE5}\u{6062}\u{590D}\u{8BBF}\u{95EE}\u{3002}");
        m.insert("settings.teams.discoverable.header", "\u{4F7F}\u{56E2}\u{961F}\u{53EF}\u{88AB}\u{53D1}\u{73B0}");
        m.insert("settings.teams.discoverable.allow_domain", "\u{5141}\u{8BB8}\u{4F7F}\u{7528} @{domain} \u{90AE}\u{7BB1}\u{7684} Warp \u{7528}\u{6237}\u{67E5}\u{627E}\u{5E76}\u{52A0}\u{5165}\u{6B64}\u{56E2}\u{961F}\u{3002}");
        m.insert("settings.teams.discoverable.allow_same_domain", "\u{5141}\u{8BB8}\u{4F7F}\u{7528}\u{76F8}\u{540C}\u{90AE}\u{7BB1}\u{57DF}\u{540D}\u{7684} Warp \u{7528}\u{6237}\u{67E5}\u{627E}\u{5E76}\u{52A0}\u{5165}\u{6B64}\u{56E2}\u{961F}\u{3002}");
        m.insert("settings.teams.discovery.one_teammate", "1 \u{540D}\u{6210}\u{5458}");
        m.insert("settings.teams.discovery.multiple_teammates", "{count} \u{540D}\u{6210}\u{5458}");
        m.insert("settings.teams.discovery.join_description", "\u{52A0}\u{5165}\u{6B64}\u{56E2}\u{961F}\u{FF0C}\u{5F00}\u{59CB}\u{534F}\u{4F5C}\u{5904}\u{7406}\u{5DE5}\u{4F5C}\u{6D41}\u{3001}\u{7B14}\u{8BB0}\u{672C}\u{7B49}\u{3002}");
        m.insert("settings.teams.discovery.join_button", "\u{52A0}\u{5165}");
        m.insert("settings.teams.discovery.contact_admin", "\u{8054}\u{7CFB}\u{7BA1}\u{7406}\u{5458}\u{7533}\u{8BF7}\u{8BBF}\u{95EE}\u{6743}\u{9650}");
        m.insert("settings.teams.pricing.team_members", "\u{56E2}\u{961F}\u{6210}\u{5458}");
        m.insert("settings.teams.pricing.prorated_admin", "\u{60A8}\u{5C06}\u{6309}\u{6BD4}\u{4F8B}\u{652F}\u{4ED8}\u{56E2}\u{961F}\u{6210}\u{5458}\u{4F7F}\u{7528} Warp \u{7684}\u{8D39}\u{7528}\u{3002}");
        m.insert("settings.teams.pricing.prorated_member", "\u{60A8}\u{7684}\u{7BA1}\u{7406}\u{5458}\u{5C06}\u{6309}\u{6BD4}\u{4F8B}\u{652F}\u{4ED8}\u{56E2}\u{961F}\u{6210}\u{5458}\u{4F7F}\u{7528} Warp \u{7684}\u{8D39}\u{7528}\u{3002}");
        m.insert("settings.teams.pricing.additional_members_with_cost", "\u{989D}\u{5916}\u{6210}\u{5458}\u{6309}\u{60A8}\u{65B9}\u{6848}\u{7684}\u{6BCF}\u{7528}\u{6237}\u{8D39}\u{7387}\u{8BA1}\u{8D39}\u{FF1A}\u{6BCF}\u{6708}${monthly_cost} \u{6216}\u{6BCF}\u{5E74}${yearly_cost}\u{FF0C}\u{53D6}\u{51B3}\u{4E8E}\u{60A8}\u{7684}\u{8BA1}\u{8D39}\u{5468}\u{671F}\u{3002}{prorated_message}");
        m.insert("settings.teams.pricing.additional_members_no_cost", "\u{989D}\u{5916}\u{6210}\u{5458}\u{6309}\u{60A8}\u{65B9}\u{6848}\u{7684}\u{6BCF}\u{7528}\u{6237}\u{8D39}\u{7387}\u{8BA1}\u{8D39}\u{3002}{prorated_message}");
        m.insert("settings.teams.upgrade.to_build", "\u{5347}\u{7EA7}\u{5230} Build");
        m.insert("settings.teams.upgrade.to_turbo", "\u{5347}\u{7EA7}\u{5230} Turbo \u{65B9}\u{6848}");
        m.insert("settings.teams.upgrade.to_lightspeed", "\u{5347}\u{7EA7}\u{5230} Lightspeed \u{65B9}\u{6848}");
        m.insert("settings.teams.upgrade.compare_plans", "\u{6BD4}\u{8F83}\u{65B9}\u{6848}");
        m.insert("settings.teams.tab.link", "\u{94FE}\u{63A5}");
        m.insert("settings.teams.tab.email", "\u{90AE}\u{7BB1}");
        m.insert("settings.teams.offline", "\u{60A8}\u{5904}\u{4E8E}\u{79BB}\u{7EBF}\u{72B6}\u{6001}\u{3002}");
        m.insert("settings.teams.toast.failed_send_invite", "\u{53D1}\u{9001}\u{9080}\u{8BF7}\u{5931}\u{8D25}");
        m.insert("settings.teams.toast.toggled_invite_links", "\u{5DF2}\u{5207}\u{6362}\u{9080}\u{8BF7}\u{94FE}\u{63A5}");
        m.insert("settings.teams.toast.failed_toggle_invite_links", "\u{5207}\u{6362}\u{9080}\u{8BF7}\u{94FE}\u{63A5}\u{5931}\u{8D25}");
        m.insert("settings.teams.toast.reset_invite_links", "\u{5DF2}\u{91CD}\u{7F6E}\u{9080}\u{8BF7}\u{94FE}\u{63A5}");
        m.insert("settings.teams.toast.failed_reset_invite_links", "\u{91CD}\u{7F6E}\u{9080}\u{8BF7}\u{94FE}\u{63A5}\u{5931}\u{8D25}");
        m.insert("settings.teams.toast.deleted_invite", "\u{5DF2}\u{5220}\u{9664}\u{9080}\u{8BF7}");
        m.insert("settings.teams.toast.failed_delete_invite", "\u{5220}\u{9664}\u{9080}\u{8BF7}\u{5931}\u{8D25}");
        m.insert("settings.teams.toast.failed_add_domain", "\u{6DFB}\u{52A0}\u{57DF}\u{540D}\u{9650}\u{5236}\u{5931}\u{8D25}");
        m.insert("settings.teams.toast.failed_delete_domain", "\u{5220}\u{9664}\u{57DF}\u{540D}\u{9650}\u{5236}\u{5931}\u{8D25}");
        m.insert("settings.teams.toast.failed_upgrade_link", "\u{751F}\u{6210}\u{5347}\u{7EA7}\u{94FE}\u{63A5}\u{5931}\u{8D25}\u{3002}\u{8BF7}\u{8054}\u{7CFB}\u{6211}\u{4EEC} feedback@warp.dev");
        m.insert("settings.teams.toast.failed_billing_link", "\u{751F}\u{6210}\u{8D26}\u{5355}\u{94FE}\u{63A5}\u{5931}\u{8D25}\u{3002}\u{8BF7}\u{8054}\u{7CFB}\u{6211}\u{4EEC} feedback@warp.dev");
        m.insert("settings.teams.toast.toggled_discoverability", "\u{5DF2}\u{5207}\u{6362}\u{56E2}\u{961F}\u{53EF}\u{53D1}\u{73B0}\u{6027}");
        m.insert("settings.teams.toast.failed_toggle_discoverability", "\u{5207}\u{6362}\u{56E2}\u{961F}\u{53EF}\u{53D1}\u{73B0}\u{6027}\u{5931}\u{8D25}");
        m.insert("settings.teams.toast.joined_team", "\u{6210}\u{529F}\u{52A0}\u{5165}\u{56E2}\u{961F}");
        m.insert("settings.teams.toast.joined_team_named", "\u{6210}\u{529F}\u{52A0}\u{5165} {team_name}");
        m.insert("settings.teams.toast.failed_join_team", "\u{52A0}\u{5165}\u{56E2}\u{961F}\u{5931}\u{8D25}");
        m.insert("settings.teams.toast.transferred_ownership", "\u{6210}\u{529F}\u{8F6C}\u{79FB}\u{56E2}\u{961F}\u{6240}\u{6709}\u{6743}");
        m.insert("settings.teams.toast.failed_transfer_ownership", "\u{8F6C}\u{79FB}\u{56E2}\u{961F}\u{6240}\u{6709}\u{6743}\u{5931}\u{8D25}");
        m.insert("settings.teams.toast.updated_member_role", "\u{6210}\u{529F}\u{66F4}\u{65B0}\u{56E2}\u{961F}\u{6210}\u{5458}\u{89D2}\u{8272}");
        m.insert("settings.teams.toast.failed_update_member_role", "\u{66F4}\u{65B0}\u{56E2}\u{961F}\u{6210}\u{5458}\u{89D2}\u{8272}\u{5931}\u{8D25}");
        m.insert("settings.teams.toast.error_leaving_team", "\u{79BB}\u{5F00}\u{56E2}\u{961F}\u{65F6}\u{51FA}\u{9519}");
        m.insert("settings.teams.toast.left_team", "\u{5DF2}\u{6210}\u{529F}\u{79BB}\u{5F00}\u{56E2}\u{961F}");
        m.insert("settings.teams.toast.renamed_team", "\u{5DF2}\u{6210}\u{529F}\u{91CD}\u{547D}\u{540D}\u{56E2}\u{961F}");
        m.insert("settings.teams.toast.failed_rename_team", "\u{91CD}\u{547D}\u{540D}\u{56E2}\u{961F}\u{5931}\u{8D25}");
        m.insert("settings.teams.toast.link_copied", "\u{94FE}\u{63A5}\u{5DF2}\u{590D}\u{5236}\u{5230}\u{526A}\u{8D34}\u{677F}\u{FF01}");
        m.insert("settings.teams.toast.invalid_domain_count", "\u{65E0}\u{6548}\u{57DF}\u{540D}\u{FF1A}{count}");
        m.insert("settings.teams.toast.domains_added", "\u{5DF2}\u{6DFB}\u{52A0}\u{57DF}\u{540D}\u{9650}\u{5236}\u{FF1A}{count}");
        m.insert("settings.teams.toast.invalid_email_count", "\u{65E0}\u{6548}\u{90AE}\u{7BB1}\u{FF1A}{count}");
        m.insert("settings.teams.toast.invite_sent", "\u{60A8}\u{7684}\u{9080}\u{8BF7}\u{5DF2}\u{53D1}\u{9001}\u{FF01}");
        m.insert("settings.teams.toast.invites_sent", "\u{60A8}\u{7684} {count} \u{4E2A}\u{9080}\u{8BF7}\u{5DF2}\u{53D1}\u{9001}\u{FF01}");

        // Billing and usage page
        m.insert("settings.billing.overage.admin_header", "\u{542F}\u{7528}\u{9AD8}\u{7EA7}\u{6A21}\u{578B}\u{7528}\u{91CF}\u{8D85}\u{989D}");
        m.insert("settings.billing.overage.user_header_enabled", "\u{9AD8}\u{7EA7}\u{6A21}\u{578B}\u{7528}\u{91CF}\u{8D85}\u{989D}\u{5DF2}\u{542F}\u{7528}");
        m.insert("settings.billing.overage.user_header_disabled", "\u{9AD8}\u{7EA7}\u{6A21}\u{578B}\u{7528}\u{91CF}\u{8D85}\u{989D}\u{672A}\u{542F}\u{7528}");
        m.insert("settings.billing.overage.description", "\u{7EE7}\u{7EED}\u{4F7F}\u{7528}\u{9AD8}\u{7EA7}\u{6A21}\u{578B}\u{8D85}\u{51FA}\u{60A8}\u{7684}\u{8BA1}\u{5212}\u{9650}\u{989D}\u{3002}\u{7528}\u{91CF}\u{6309} $20 \u{589E}\u{91CF}\u{8BA1}\u{8D39}\u{FF0C}\u{76F4}\u{5230}\u{8FBE}\u{5230}\u{60A8}\u{7684}\u{6D88}\u{8D39}\u{9650}\u{989D}\u{FF0C}\u{4EFB}\u{4F55}\u{5269}\u{4F59}\u{4F59}\u{989D}\u{5C06}\u{5728}\u{60A8}\u{7684}\u{8BA1}\u{5212}\u{8D26}\u{5355}\u{65E5}\u{671F}\u{6536}\u{53D6}\u{3002}");
        m.insert("settings.billing.overage.user_description", "\u{8BF7}\u{56E2}\u{961F}\u{7BA1}\u{7406}\u{5458}\u{542F}\u{7528}\u{8D85}\u{989D}\u{4EE5}\u{83B7}\u{5F97}\u{66F4}\u{591A}AI\u{7528}\u{91CF}\u{3002}");
        m.insert("settings.billing.overage.link_text", "\u{67E5}\u{770B}\u{8D85}\u{989D}\u{7528}\u{91CF}\u{8BE6}\u{60C5}");
        m.insert("settings.billing.overage.monthly_spending_limit", "\u{6708}\u{5EA6}\u{8D85}\u{989D}\u{6D88}\u{8D39}\u{9650}\u{989D}");
        m.insert("settings.billing.overage.monthly_spending_limit.tooltip", "\u{8BBE}\u{7F6E}\u{8D85}\u{51FA}\u{8BA1}\u{5212}\u{91D1}\u{989D}\u{7684}\u{6708}\u{5EA6}\u{8D85}\u{989D}\u{6D88}\u{8D39}\u{9650}\u{989D}");
        m.insert("settings.billing.overage.not_set", "\u{672A}\u{8BBE}\u{7F6E}");
        m.insert("settings.billing.overage.total", "\u{603B}\u{8D85}\u{989D}");
        m.insert("settings.billing.overage.resets_on", "\u{7528}\u{91CF}\u{5728}{date}\u{91CD}\u{7F6E}");
        m.insert("settings.billing.overage.one_credit", "1\u{4E2A}\u{79EF}\u{5206}");
        m.insert("settings.billing.overage.credits", "{count}\u{4E2A}\u{79EF}\u{5206}");
        m.insert("settings.billing.sort.a_to_z", "A\u{5230}Z");
        m.insert("settings.billing.sort.z_to_a", "Z\u{5230}A");
        m.insert("settings.billing.sort.ascending", "\u{7528}\u{91CF}\u{5347}\u{5E8F}");
        m.insert("settings.billing.sort.descending", "\u{7528}\u{91CF}\u{964D}\u{5E8F}");
        m.insert("settings.billing.sort.label", "\u{6392}\u{5E8F}\u{65B9}\u{5F0F}");
        m.insert("settings.billing.auto_reload.exceed_limit_warning", "\u{81EA}\u{52A8}\u{5145}\u{503C}\u{5DF2}\u{7981}\u{7528}\u{FF0C}\u{56E0}\u{4E3A}\u{4E0B}\u{6B21}\u{5145}\u{503C}\u{5C06}\u{8D85}\u{51FA}\u{60A8}\u{7684}\u{6708}\u{5EA6}\u{6D88}\u{8D39}\u{9650}\u{989D}\u{3002}\u{589E}\u{52A0}\u{9650}\u{989D}\u{4EE5}\u{4F7F}\u{7528}\u{81EA}\u{52A8}\u{5145}\u{503C}\u{3002}");
        m.insert("settings.billing.auto_reload.delinquent_warning", "\u{7531}\u{4E8E}\u{8D26}\u{5355}\u{95EE}\u{9898}\u{53D7}\u{5230}\u{9650}\u{5236}\u{3002}\u{66F4}\u{65B0}\u{60A8}\u{7684}\u{652F}\u{4ED8}\u{65B9}\u{5F0F}\u{4EE5}\u{8D2D}\u{4E70}\u{9644}\u{52A0}\u{79EF}\u{5206}\u{3002}");
        m.insert("settings.billing.auto_reload.restricted_warning", "\u{7531}\u{4E8E}\u{6700}\u{8FD1}\u{5145}\u{503C}\u{5931}\u{8D25}\u{FF0C}\u{81EA}\u{52A8}\u{5145}\u{503C}\u{5DF2}\u{7981}\u{7528}\u{3002}\u{8BF7}\u{66F4}\u{65B0}\u{60A8}\u{7684}\u{652F}\u{4ED8}\u{65B9}\u{5F0F}\u{5E76}\u{91CD}\u{8BD5}\u{3002}");
        m.insert("settings.billing.auto_reload.exceed_limit_with_link", "\u{5145}\u{503C}\u{5C06}\u{8D85}\u{51FA}\u{60A8}\u{7684}\u{6708}\u{5EA6}\u{9650}\u{989D}\u{3002}");
        m.insert("settings.billing.auto_reload.increase_limit_link", "\u{589E}\u{52A0}\u{9650}\u{989D}");
        m.insert("settings.billing.auto_reload.to_continue", "\u{4EE5}\u{7EE7}\u{7EED}\u{3002}");
        m.insert("settings.billing.restricted.billing_issue", "\u{7531}\u{4E8E}\u{8D26}\u{5355}\u{95EE}\u{9898}\u{53D7}\u{5230}\u{9650}\u{5236}");
        m.insert("settings.billing.tab.overview", "\u{6982}\u{89C8}");
        m.insert("settings.billing.tab.usage_history", "\u{7528}\u{91CF}\u{5386}\u{53F2}");
        m.insert("settings.billing.enterprise.callout_header", "\u{7528}\u{91CF}\u{62A5}\u{544A}\u{76EE}\u{524D}\u{6709}\u{9650}\u{5236}");
        m.insert("settings.billing.enterprise.callout_admin_prefix", "\u{4F01}\u{4E1A}\u{79EF}\u{5206}\u{7528}\u{91CF}\u{5728}\u{6B64}\u{89C6}\u{56FE}\u{4E2D}\u{5C1A}\u{4E0D}\u{5B8C}\u{5168}\u{53EF}\u{7528}\u{3002}\u{4E3A}\u{4E86}\u{6700}\u{51C6}\u{786E}\u{7684}\u{6D88}\u{8D39}\u{8DDF}\u{8E2A}\u{FF0C}");
        m.insert("settings.billing.enterprise.callout_admin_link", "\u{8BBF}\u{95EE}\u{7BA1}\u{7406}\u{5458}\u{9762}\u{677F}");
        m.insert("settings.billing.enterprise.callout_admin_suffix", "\u{3002}");
        m.insert("settings.billing.enterprise.callout_non_admin", "\u{4F01}\u{4E1A}\u{79EF}\u{5206}\u{7528}\u{91CF}\u{5728}\u{6B64}\u{89C6}\u{56FE}\u{4E2D}\u{5C1A}\u{4E0D}\u{5B8C}\u{5168}\u{53EF}\u{7528}\u{3002}\u{8BF7}\u{8054}\u{7CFB}\u{56E2}\u{961F}\u{7BA1}\u{7406}\u{5458}\u{83B7}\u{53D6}\u{8BE6}\u{7EC6}\u{7684}\u{7528}\u{91CF}\u{62A5}\u{544A}\u{3002}");
        m.insert("settings.billing.addon.title", "\u{9644}\u{52A0}\u{79EF}\u{5206}");
        m.insert("settings.billing.addon.description", "\u{9644}\u{52A0}\u{79EF}\u{5206}\u{4EE5}\u{9884}\u{4ED8}\u{5305}\u{5F62}\u{5F0F}\u{8D2D}\u{4E70}\u{FF0C}\u{6BCF}\u{4E2A}\u{8BA1}\u{8D39}\u{5468}\u{671F}\u{7ED3}\u{8F6C}\u{FF0C}\u{4E00}\u{5E74}\u{540E}\u{8FC7}\u{671F}\u{3002}\u{8D2D}\u{4E70}\u{91CF}\u{8D8A}\u{5927}\u{FF0C}\u{5355}\u{4EF7}\u{8D8A}\u{4F4E}\u{3002}\u{57FA}\u{7840}\u{8BA1}\u{5212}\u{79EF}\u{5206}\u{7528}\u{5B8C}\u{540E}\u{FF0C}\u{5C06}\u{6D88}\u{8017}\u{9644}\u{52A0}\u{79EF}\u{5206}\u{3002}");
        m.insert("settings.billing.addon.description_team", "\u{8D2D}\u{4E70}\u{7684}\u{9644}\u{52A0}\u{79EF}\u{5206}\u{5728}\u{60A8}\u{7684}\u{56E2}\u{961F}\u{4E2D}\u{5171}\u{4EAB}\u{3002}");
        m.insert("settings.billing.addon.monthly_spend_limit", "\u{6708}\u{5EA6}\u{6D88}\u{8D39}\u{9650}\u{989D}");
        m.insert("settings.billing.addon.monthly_spend_limit.tooltip", "\u{8BBE}\u{7F6E}\u{9644}\u{52A0}\u{79EF}\u{5206}\u{7684}\u{6708}\u{5EA6}\u{6D88}\u{8D39}\u{9650}\u{989D}");
        m.insert("settings.billing.addon.purchased_this_month", "\u{672C}\u{6708}\u{5DF2}\u{8D2D}\u{4E70}");
        m.insert("settings.billing.addon.auto_reload", "\u{81EA}\u{52A8}\u{5145}\u{503C}");
        m.insert("settings.billing.addon.auto_reload.description", "\u{542F}\u{7528}\u{540E}\u{FF0C}\u{81EA}\u{52A8}\u{5145}\u{503C}\u{5C06}\u{5728}\u{60A8}\u{7684}\u{9644}\u{52A0}\u{79EF}\u{5206}\u{4F59}\u{989D}\u{964D}\u{81F3} 100 \u{79EF}\u{5206}\u{65F6}\u{81EA}\u{52A8}\u{8D2D}\u{4E70} {amount} \u{79EF}\u{5206}\u{3002}");
        m.insert("settings.billing.addon.one_time_purchase", "\u{4E00}\u{6B21}\u{6027}\u{8D2D}\u{4E70}");
        m.insert("settings.billing.addon.buy", "\u{8D2D}\u{4E70}");
        m.insert("settings.billing.addon.buying", "\u{8D2D}\u{4E70}\u{4E2D}\u{2026}");
        m.insert("settings.billing.addon.one_credit", "1\u{4E2A}\u{79EF}\u{5206}");
        m.insert("settings.billing.addon.credits", "{count}\u{4E2A}\u{79EF}\u{5206}");
        m.insert("settings.billing.addon.zero_credits", "0\u{4E2A}\u{79EF}\u{5206}");
        m.insert("settings.billing.addon.contact_account_executive", "\u{8BF7}\u{8054}\u{7CFB}\u{60A8}\u{7684}\u{5BA2}\u{6237}\u{7ECF}\u{7406}\u{83B7}\u{53D6}\u{66F4}\u{591A}\u{9644}\u{52A0}\u{79EF}\u{5206}\u{3002}");
        m.insert("settings.billing.addon.contact_admin", "\u{8BF7}\u{8054}\u{7CFB}\u{56E2}\u{961F}\u{7BA1}\u{7406}\u{5458}\u{8D2D}\u{4E70}\u{9644}\u{52A0}\u{79EF}\u{5206}\u{3002}");
        m.insert("settings.billing.addon.switch_build", "\u{5207}\u{6362}\u{5230} Build \u{8BA1}\u{5212}");
        m.insert("settings.billing.addon.upgrade_build", "\u{5347}\u{7EA7}\u{5230} Build \u{8BA1}\u{5212}");
        m.insert("settings.billing.addon.to_purchase_suffix", "\u{4EE5}\u{8D2D}\u{4E70}\u{9644}\u{52A0}\u{79EF}\u{5206}\u{3002}");
        m.insert("settings.billing.ambient_trial.title", "\u{4E91}\u{4EE3}\u{7406}\u{8BD5}\u{7528}");
        m.insert("settings.billing.ambient_trial.one_credit_remaining", "\u{5269}\u{4F59} 1 \u{4E2A}\u{79EF}\u{5206}");
        m.insert("settings.billing.ambient_trial.credits_remaining", "\u{5269}\u{4F59} {count} \u{4E2A}\u{79EF}\u{5206}");
        m.insert("settings.billing.ambient_trial.new_agent", "\u{65B0}\u{4EE3}\u{7406}");
        m.insert("settings.billing.ambient_trial.buy_more", "\u{8D2D}\u{4E70}\u{66F4}\u{591A}");
        m.insert("settings.billing.usage_history.last_30_days", "\u{6700}\u{8FD1} 30 \u{5929}");
        m.insert("settings.billing.usage_history.load_more", "\u{52A0}\u{8F7D}\u{66F4}\u{591A}");
        m.insert("settings.billing.usage_history.empty_title", "\u{65E0}\u{7528}\u{91CF}\u{5386}\u{53F2}");
        m.insert("settings.billing.usage_history.empty_description", "\u{542F}\u{52A8}\u{4E00}\u{4E2A}\u{4EE3}\u{7406}\u{4EFB}\u{52A1}\u{4EE5}\u{5728}\u{6B64}\u{67E5}\u{770B}\u{7528}\u{91CF}\u{5386}\u{53F2}\u{3002}");
        m.insert("settings.billing.usage.title", "\u{7528}\u{91CF}");
        m.insert("settings.billing.usage.credits", "\u{79EF}\u{5206}");
        m.insert("settings.billing.usage.resets", "\u{5728} {time} \u{91CD}\u{7F6E}");
        m.insert("settings.billing.usage.limit_description", "\u{8FD9}\u{662F}\u{60A8}\u{8D26}\u{6237}\u{7684} {duration} AI \u{79EF}\u{5206}\u{9650}\u{989D}\u{3002}");
        m.insert("settings.billing.usage.team_total", "\u{56E2}\u{961F}\u{603B}\u{8BA1}");
        m.insert("settings.billing.overage.modal_title", "\u{8D85}\u{989D}\u{6D88}\u{8D39}\u{9650}\u{989D}");
        m.insert("settings.billing.addon.modal_title", "\u{6708}\u{5EA6}\u{6D88}\u{8D39}\u{9650}\u{989D}");
        m.insert("settings.billing.prorated.tooltip_current_user", "\u{60A8}\u{7684}\u{79EF}\u{5206}\u{9650}\u{989D}\u{662F}\u{6309}\u{6BD4}\u{4F8B}\u{8BA1}\u{7B97}\u{7684}\u{FF0C}\u{56E0}\u{4E3A}\u{60A8}\u{5728}\u{8BA1}\u{8D39}\u{5468}\u{671F}\u{4E2D}\u{9014}\u{52A0}\u{5165}\u{3002}");
        m.insert("settings.billing.prorated.tooltip_other_user", "\u{6B64}\u{79EF}\u{5206}\u{9650}\u{989D}\u{662F}\u{6309}\u{6BD4}\u{4F8B}\u{8BA1}\u{7B97}\u{7684}\u{FF0C}\u{56E0}\u{4E3A}\u{6B64}\u{7528}\u{6237}\u{5728}\u{8BA1}\u{8D39}\u{5468}\u{671F}\u{4E2D}\u{9014}\u{52A0}\u{5165}\u{3002}");
        m.insert("settings.billing.toast.update_settings_failed", "\u{66F4}\u{65B0}\u{5DE5}\u{4F5C}\u{533A}\u{8BBE}\u{7F6E}\u{5931}\u{8D25}");
        m.insert("settings.billing.toast.purchase_success", "\u{6210}\u{529F}\u{8D2D}\u{4E70}\u{9644}\u{52A0}\u{79EF}\u{5206}");
        m.insert("settings.billing.plan.title", "\u{8BA1}\u{5212}");
        m.insert("settings.billing.plan.free", "\u{514D}\u{8D39}");
        m.insert("settings.billing.plan.sign_up", "\u{6CE8}\u{518C}");
        m.insert("settings.billing.plan.compare_plans", "\u{6BD4}\u{8F83}\u{8BA1}\u{5212}");
        m.insert("settings.billing.plan.manage_billing", "\u{7BA1}\u{7406}\u{8D26}\u{5355}");
        m.insert("settings.billing.plan.open_admin_panel", "\u{6253}\u{5F00}\u{7BA1}\u{7406}\u{5458}\u{9762}\u{677F}");
        m.insert("settings.billing.upgrade.manage_billing_regain", "\u{7BA1}\u{7406}\u{8D26}\u{5355}");
        m.insert("settings.billing.upgrade.to_regain_access", "\u{4EE5}\u{6062}\u{590D}\u{5BF9}AI\u{529F}\u{80FD}\u{7684}\u{8BBF}\u{95EE}\u{3002}");
        m.insert("settings.billing.upgrade.contact_admin_billing", "\u{8BF7}\u{8054}\u{7CFB}\u{60A8}\u{7684}\u{56E2}\u{961F}\u{7BA1}\u{7406}\u{5458}\u{89E3}\u{51B3}\u{8D26}\u{5355}\u{95EE}\u{9898}\u{3002}");
        m.insert("settings.billing.upgrade.switch_build", "\u{5207}\u{6362}\u{5230} Build \u{8BA1}\u{5212}");
        m.insert("settings.billing.upgrade.for_flexible_pricing", "\u{4EE5}\u{83B7}\u{5F97}\u{66F4}\u{7075}\u{6D3B}\u{7684}\u{5B9A}\u{4EF7}\u{6A21}\u{5F0F}\u{3002}");
        m.insert("settings.billing.upgrade.upgrade_build", "\u{5347}\u{7EA7}\u{5230} Build \u{8BA1}\u{5212}");
        m.insert("settings.billing.upgrade.bring_your_own_key", "\u{4F7F}\u{7528}\u{81EA}\u{5DF1}\u{7684}\u{5BC6}\u{94A5}");
        m.insert("settings.billing.upgrade.or", "\u{6216}");
        m.insert("settings.billing.upgrade.for_increased_access", "\u{4EE5}\u{83B7}\u{5F97}\u{66F4}\u{591A}AI\u{529F}\u{80FD}\u{3002}");
        m.insert("settings.billing.upgrade.to_turbo", "\u{5347}\u{7EA7}\u{5230} Turbo \u{8BA1}\u{5212}");
        m.insert("settings.billing.upgrade.to_lightspeed", "\u{5347}\u{7EA7}\u{5230} Lightspeed \u{8BA1}\u{5212}");
        m.insert("settings.billing.upgrade.generic", "\u{5347}\u{7EA7}");
        m.insert("settings.billing.upgrade.to_get_more_usage", "\u{4EE5}\u{83B7}\u{5F97}\u{66F4}\u{591A}AI\u{7528}\u{91CF}\u{3002}");
        m.insert("settings.billing.upgrade.to_max", "\u{5347}\u{7EA7}\u{5230} Max");
        m.insert("settings.billing.upgrade.for_more_ai_credits", "\u{4EE5}\u{83B7}\u{5F97}\u{66F4}\u{591A}AI\u{79EF}\u{5206}\u{3002}");
        m.insert("settings.billing.upgrade.switch_business", "\u{5207}\u{6362}\u{5230} Business");
        m.insert("settings.billing.upgrade.for_security_features", "\u{4EE5}\u{83B7}\u{5F97}SSO\u{7B49}\u{5B89}\u{5168}\u{529F}\u{80FD}\u{548C}\u{81EA}\u{52A8}\u{5E94}\u{7528}\u{7684}\u{96F6}\u{6570}\u{636E}\u{4FDD}\u{7559}\u{3002}");
        m.insert("settings.billing.upgrade.to_enterprise", "\u{5347}\u{7EA7}\u{5230} Enterprise");
        m.insert("settings.billing.upgrade.for_custom_limits", "\u{4EE5}\u{83B7}\u{5F97}\u{81EA}\u{5B9A}\u{4E49}\u{9650}\u{989D}\u{548C}\u{4E13}\u{5C5E}\u{652F}\u{6301}\u{3002}");
        m.insert("settings.billing.upgrade.contact_support", "\u{8054}\u{7CFB}\u{652F}\u{6301}");
        m.insert("settings.billing.upgrade.for_more_ai_usage_generic", "\u{4EE5}\u{83B7}\u{5F97}\u{66F4}\u{591A}AI\u{7528}\u{91CF}\u{3002}");
        m.insert("settings.billing.upgrade.for_more_credits_models", "\u{4EE5}\u{83B7}\u{5F97}\u{66F4}\u{591A}\u{79EF}\u{5206}\u{548C}\u{66F4}\u{591A}\u{6A21}\u{578B}\u{3002}");

        m
    };
}
