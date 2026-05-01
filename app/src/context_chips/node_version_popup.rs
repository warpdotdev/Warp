use pathfinder_color::ColorU;
use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::Fill;
use warpui::elements::{
    ChildView, ClippedScrollStateHandle, ClippedScrollable, Dismiss, ParentElement, ScrollbarWidth,
};
use warpui::fonts::FamilyId;
use warpui::{
    elements::{
        ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DropShadow, Flex,
        MainAxisAlignment, MainAxisSize, Radius, Text,
    },
    fonts::Properties,
    keymap::FixedBinding,
    AppContext, Element, Entity, ModelHandle, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use crate::menu::{self, Event as MenuEvent, Menu, MenuItemFields};
use crate::terminal::model_events::{ModelEvent, ModelEventDispatcher};
use crate::ui_components::blended_colors;
use crate::ui_components::icons;
use crate::view_components::action_button::{ActionButton, SecondaryTheme};

const MENU_WIDTH: f32 = 300.0;
const MENU_MAX_HEIGHT: f32 = 260.0;

/// The version manager used for Node.js version switching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionManager {
    Nvm,
    Mise,
}

impl VersionManager {
    /// Returns the display name of the version manager.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Nvm => "nvm",
            Self::Mise => "mise",
        }
    }

    /// Returns the command to switch to a specific Node.js version.
    pub fn switch_command(&self, version: &str) -> String {
        match self {
            Self::Nvm => format!("nvm use {version}"),
            // mise expects versions without a 'v' prefix (e.g., node@18.19.1 not node@v18.19.1)
            // --global writes to ~/.config/mise/config.toml; with shell hooks active, this
            // takes effect immediately, matching nvm's `nvm use` behavior.
            Self::Mise => format!("mise use --global node@{}", normalize_version(version)),
        }
    }

    /// Returns the command to install the latest Node.js version.
    pub fn install_latest_command(&self) -> String {
        match self {
            Self::Nvm => "nvm install node".to_string(),
            Self::Mise => "mise install node@latest".to_string(),
        }
    }

    /// Returns the agent query to install this version manager.
    pub fn install_manager_agent_query(&self) -> String {
        match self {
            Self::Nvm => {
                if cfg!(windows) {
                    "Uninstall existing Node.js installation and install nvm for me"
                        .to_string()
                } else {
                    "Install nvm for me".to_string()
                }
            }
            Self::Mise => "Install mise-en-place for me".to_string(),
        }
    }
}

pub struct NodeVersionPopupView {
    install_nvm_button: ViewHandle<ActionButton>,
    install_mise_button: ViewHandle<ActionButton>,
    install_latest_node_button: ViewHandle<ActionButton>,
    has_nvm: bool,
    has_mise: bool,
    active_manager: VersionManager,
    versions: Vec<String>,
    current_version: Option<String>,
    versions_menu: Option<ViewHandle<Menu<NodeVersionPopupAction>>>,
    scroll_state: ClippedScrollStateHandle,
}

#[derive(Debug, Clone)]
pub enum NodeVersionPopupAction {
    ClosePopup,
    InstallNvm,
    InstallMise,
    InstallLatestNodeVersion,
    SelectVersion { version: String },
    SwitchVersionManager { manager: VersionManager },
}

#[derive(Debug, Clone)]
pub enum NodeVersionPopupEvent {
    Close,
    InstallNvm,
    InstallMise,
    InstallLatestNodeVersion { command: String },
    SelectVersion { version: String, switch_command: String },
}

struct Styles {
    ui_font_family: FamilyId,
    background: Fill,
    main_text_color: ColorU,
    secondary_text_color: ColorU,
    tertiary_text_color: ColorU,
    detail_font_size: f32,
}

pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    app.register_fixed_bindings([FixedBinding::new(
        "escape",
        NodeVersionPopupAction::ClosePopup,
        id!(NodeVersionPopupView::ui_name()),
    )]);
}

impl NodeVersionPopupView {
    pub fn new(
        current_version: Option<String>,
        model_events: &ModelHandle<ModelEventDispatcher>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let has_nvm = detect_nvm_installed();
        let has_mise = detect_mise_installed();

        // Default to nvm if both are available, mise if only mise is available
        let active_manager = if has_nvm {
            VersionManager::Nvm
        } else if has_mise {
            VersionManager::Mise
        } else {
            VersionManager::Nvm // fallback, doesn't matter since no manager is installed
        };

        let install_nvm_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Install nvm", SecondaryTheme)
                .with_icon(icons::Icon::Terminal)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(NodeVersionPopupAction::InstallNvm);
                })
        });

        let install_mise_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Install mise", SecondaryTheme)
                .with_icon(icons::Icon::Terminal)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(NodeVersionPopupAction::InstallMise);
                })
        });

        let versions = if has_nvm || has_mise {
            list_versions(active_manager)
        } else {
            Vec::new()
        };

        let install_latest_node_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new(
                active_manager.install_latest_command(),
                SecondaryTheme,
            )
                .with_icon(icons::Icon::Terminal)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(NodeVersionPopupAction::InstallLatestNodeVersion);
                })
        });

        let versions_menu = if has_nvm || has_mise {
            let menu_handle = ctx.add_typed_action_view(|ctx| {
                let mut menu = Menu::new().with_width(MENU_WIDTH);
                menu.set_items(Self::menu_items(&versions, current_version.as_deref(), has_nvm && has_mise, active_manager), ctx);
                let selected_index =
                    get_selected_version_index(&versions, current_version.as_deref());
                menu.set_selected_by_index(selected_index, ctx);
                menu
            });
            ctx.subscribe_to_view(&menu_handle, |_, _, event, ctx| {
                if let MenuEvent::Close { .. } = event {
                    ctx.emit(NodeVersionPopupEvent::Close);
                }
            });
            Some(menu_handle)
        } else {
            None
        };

        // Subscribe to command execution events to refresh
        // when a version manager is installed or a node version is installed
        ctx.subscribe_to_model(model_events, |me, _model, event, ctx| match event {
            ModelEvent::ExecutedInBandCommand(_) | ModelEvent::AfterBlockCompleted(_) => {
                me.refresh(ctx);
            }
            _ => {}
        });

        Self {
            install_nvm_button,
            install_mise_button,
            install_latest_node_button,
            has_nvm,
            has_mise,
            active_manager,
            versions,
            current_version,
            versions_menu,
            scroll_state: Default::default(),
        }
    }

    fn styles(&self, appearance: &Appearance) -> Styles {
        let theme = appearance.theme();
        let background = theme.surface_2();
        let main_text_color = blended_colors::text_main(theme, background);
        let secondary_text_color = blended_colors::text_sub(theme, background);
        let tertiary_text_color = theme.hint_text_color(background).into_solid();
        let detail_font_size = appearance.ui_font_size();
        let ui_font_family = appearance.ui_font_family();

        Styles {
            ui_font_family,
            background,
            main_text_color,
            secondary_text_color,
            tertiary_text_color,
            detail_font_size,
        }
    }

    fn render_install_manager_empty_state(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let styles = self.styles(appearance);

        let mut col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);

        col.add_child(
            Container::new(
                ConstrainedBox::new(
                    icons::Icon::NodeJS
                        .to_warpui_icon(styles.tertiary_text_color.into())
                        .finish(),
                )
                .with_width(24.)
                .with_height(24.)
                .finish(),
            )
            .with_margin_bottom(12.)
            .finish(),
        );

        col.add_child(
            Text::new(
                "Install a version manager to enable version switching",
                styles.ui_font_family,
                styles.detail_font_size + 2.,
            )
            .with_style(Properties::default())
            .with_color(styles.secondary_text_color)
            .finish(),
        );

        col.add_child(
            Container::new(
                Text::new(
                    "This menu helps you switch between Node.js versions — but it requires nvm or mise to be installed.",
                    styles.ui_font_family,
                    styles.detail_font_size,
                )
                .with_color(styles.tertiary_text_color)
                .soft_wrap(true)
                .finish(),
            )
            .with_margin_top(6.)
            .with_margin_bottom(12.)
            .with_horizontal_padding(24.)
            .finish(),
        );

        // Show both install buttons
        let mut buttons_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);

        buttons_row.add_child(ChildView::new(&self.install_nvm_button).finish());
        buttons_row.add_child(
            Container::new(ChildView::new(&self.install_mise_button).finish())
                .with_margin_left(8.)
                .finish(),
        );

        col.add_child(buttons_row.finish());

        ConstrainedBox::new(col.finish())
            .with_max_width(MENU_WIDTH)
            .with_max_height(MENU_MAX_HEIGHT)
            .finish()
    }

    fn render_install_latest_node_version_empty_state(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let styles = self.styles(appearance);

        let mut col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);

        col.add_child(
            Container::new(
                ConstrainedBox::new(
                    icons::Icon::NodeJS
                        .to_warpui_icon(styles.tertiary_text_color.into())
                        .finish(),
                )
                .with_width(24.)
                .with_height(24.)
                .finish(),
            )
            .with_margin_bottom(12.)
            .finish(),
        );

        // Heading
        col.add_child(
            Text::new(
                "No node versions installed",
                styles.ui_font_family,
                styles.detail_font_size + 2.,
            )
            .with_style(Properties::default())
            .with_color(styles.secondary_text_color)
            .finish(),
        );

        // Subheading — mention the active version manager
        col.add_child(
            Container::new(
                Text::new(
                    format!("Try installing versions with {}", self.active_manager.display_name()),
                    styles.ui_font_family,
                    styles.detail_font_size,
                )
                .with_color(styles.tertiary_text_color)
                .soft_wrap(true)
                .finish(),
            )
            .with_margin_top(6.)
            .with_margin_bottom(12.)
            .with_horizontal_padding(24.)
            .finish(),
        );

        // Button
        col.add_child(ChildView::new(&self.install_latest_node_button).finish());

        ConstrainedBox::new(col.finish())
            .with_max_width(MENU_WIDTH)
            .with_max_height(MENU_MAX_HEIGHT)
            .finish()
    }

    fn render_version_manager_toggle(&self, app: &AppContext) -> Option<Box<dyn Element>> {
        // Only show the toggle when both version managers are available
        if !self.has_nvm || !self.has_mise {
            return None;
        }

        let appearance = Appearance::as_ref(app);
        let styles = self.styles(appearance);

        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);

        // nvm tab
        let nvm_active = self.active_manager == VersionManager::Nvm;
        let nvm_bg = if nvm_active {
            Some(appearance.theme().surface_1())
        } else {
            None
        };
        let nvm_text_color = if nvm_active {
            styles.main_text_color
        } else {
            styles.tertiary_text_color
        };

        let mut nvm_container = Container::new(
            Text::new("nvm", styles.ui_font_family, styles.detail_font_size)
                .with_color(nvm_text_color)
                .with_style(Properties::default().weight(warpui::fonts::Weight::Semibold))
                .finish(),
        )
        .with_vertical_padding(4.)
        .with_horizontal_padding(12.);
        if let Some(bg) = nvm_bg {
            nvm_container = nvm_container.with_background(bg);
        }
        row.add_child(nvm_container.with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.))).finish());

        // mise tab
        let mise_active = !nvm_active;
        let mise_bg = if mise_active {
            Some(appearance.theme().surface_1())
        } else {
            None
        };
        let mise_text_color = if mise_active {
            styles.main_text_color
        } else {
            styles.tertiary_text_color
        };

        let mut mise_container = Container::new(
            Text::new("mise", styles.ui_font_family, styles.detail_font_size)
            .with_color(mise_text_color)
            .with_style(Properties::default().weight(warpui::fonts::Weight::Semibold))
            .finish(),
        )
        .with_vertical_padding(4.)
        .with_horizontal_padding(12.);
        if let Some(bg) = mise_bg {
            mise_container = mise_container.with_background(bg);
        }
        row.add_child(mise_container.with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.))).finish());

        Some(
            Container::new(row.finish())
                .with_padding_bottom(8.)
                .finish(),
        )
    }

    fn render_node_version_selector(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let styles = self.styles(appearance);

        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        // Version manager toggle (only when both are available)
        if let Some(toggle) = self.render_version_manager_toggle(app) {
            col.add_child(toggle);
        }

        col.add_child(
            Container::new(
                Text::new("Installed", styles.ui_font_family, styles.detail_font_size)
                    .with_style(Properties::default())
                    .with_color(styles.secondary_text_color)
                    .finish(),
            )
            .with_horizontal_padding(12.)
            .finish(),
        );

        if let Some(menu) = &self.versions_menu {
            col.add_child(ChildView::new(menu).finish());
        }

        Container::new(col.finish()).with_padding_top(8.).finish()
    }

    fn menu_items(
        versions: &[String],
        current_version: Option<&str>,
        has_both_managers: bool,
        active_manager: VersionManager,
    ) -> Vec<menu::MenuItem<NodeVersionPopupAction>> {
        let mut items: Vec<menu::MenuItem<NodeVersionPopupAction>> = Vec::new();

        // If both managers are available, add a switch option at the top
        if has_both_managers {
            let inactive = match active_manager {
                VersionManager::Nvm => VersionManager::Mise,
                VersionManager::Mise => VersionManager::Nvm,
            };
            let label = format!("Switch to {}", inactive.display_name());
            let fields = MenuItemFields::new(&label)
                .with_icon(icons::Icon::Refresh)
                .with_on_select_action(NodeVersionPopupAction::SwitchVersionManager {
                    manager: inactive,
                });
            items.push(menu::MenuItem::Item(fields));
        }

        for ver in versions {
            let mut fields = MenuItemFields::new(ver).with_on_select_action(
                NodeVersionPopupAction::SelectVersion {
                    version: ver.clone(),
                },
            );
            if is_current_version(ver, current_version) {
                fields = fields.with_icon(icons::Icon::Check);
            } else {
                fields = fields.with_indent();
            }
            items.push(menu::MenuItem::Item(fields));
        }

        items
    }

    pub fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.has_nvm = detect_nvm_installed();
        self.has_mise = detect_mise_installed();

        // Update active manager if the current one is no longer available
        if self.active_manager == VersionManager::Nvm && !self.has_nvm && self.has_mise {
            self.active_manager = VersionManager::Mise;
        } else if self.active_manager == VersionManager::Mise && !self.has_mise && self.has_nvm {
            self.active_manager = VersionManager::Nvm;
        }

        self.versions = if self.has_nvm || self.has_mise {
            list_versions(self.active_manager)
        } else {
            Vec::new()
        };

        if let Some(menu) = &self.versions_menu {
            menu.update(ctx, |menu, ctx| {
                menu.set_items(
                    Self::menu_items(&self.versions, self.current_version.as_deref(), self.has_nvm && self.has_mise, self.active_manager),
                    ctx,
                );
                let selected_index =
                    get_selected_version_index(&self.versions, self.current_version.as_deref());
                menu.set_selected_by_index(selected_index, ctx);
            });
        }

        ctx.notify();
    }
}

impl View for NodeVersionPopupView {
    fn ui_name() -> &'static str {
        "NodeVersionPopup"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let styles = self.styles(appearance);

        let content = if !self.versions.is_empty() {
            self.render_node_version_selector(app)
        } else if self.has_nvm || self.has_mise {
            self.render_install_latest_node_version_empty_state(app)
        } else {
            self.render_install_manager_empty_state(app)
        };

        let scrollable = ClippedScrollable::vertical(
            self.scroll_state.clone(),
            content,
            ScrollbarWidth::Auto,
            appearance.theme().nonactive_ui_detail().into(),
            appearance.theme().active_ui_detail().into(),
            warpui::elements::Fill::None,
        )
        .with_overlayed_scrollbar()
        .finish();

        Dismiss::new(
            ConstrainedBox::new(
                Container::new(scrollable)
                    .with_background(styles.background)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
                    .with_drop_shadow(DropShadow::default())
                    .finish(),
            )
            .with_width(MENU_WIDTH)
            .with_max_height(MENU_MAX_HEIGHT)
            .finish(),
        )
        .prevent_interaction_with_other_elements()
        .on_dismiss(|ctx, _app| ctx.dispatch_typed_action(NodeVersionPopupAction::ClosePopup))
        .finish()
    }
}

impl Entity for NodeVersionPopupView {
    type Event = NodeVersionPopupEvent;
}

impl NodeVersionPopupView {
    pub fn focus_content(&self, ctx: &mut ViewContext<Self>) {
        // Focus menu if present to allow keyboard navigation
        if let Some(menu) = &self.versions_menu {
            ctx.focus(menu);
        } else {
            ctx.focus_self();
        }
    }
}

impl TypedActionView for NodeVersionPopupView {
    type Action = NodeVersionPopupAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            NodeVersionPopupAction::ClosePopup => ctx.emit(NodeVersionPopupEvent::Close),
            NodeVersionPopupAction::InstallNvm => ctx.emit(NodeVersionPopupEvent::InstallNvm),
            NodeVersionPopupAction::InstallMise => ctx.emit(NodeVersionPopupEvent::InstallMise),
            NodeVersionPopupAction::InstallLatestNodeVersion => {
                ctx.emit(NodeVersionPopupEvent::InstallLatestNodeVersion {
                    command: self.active_manager.install_latest_command(),
                });
            }
            NodeVersionPopupAction::SelectVersion { version } => {
                ctx.emit(NodeVersionPopupEvent::SelectVersion {
                    version: version.clone(),
                    switch_command: self.active_manager.switch_command(version),
                });
                ctx.emit(NodeVersionPopupEvent::Close);
            }
            NodeVersionPopupAction::SwitchVersionManager { manager } => {
                self.active_manager = *manager;
                // Refresh version list with the new manager
                self.versions = list_versions(self.active_manager);
                if let Some(menu) = &self.versions_menu {
                    menu.update(ctx, |menu, ctx| {
                        menu.set_items(
                            Self::menu_items(&self.versions, self.current_version.as_deref(), self.has_nvm && self.has_mise, self.active_manager),
                            ctx,
                        );
                        let selected_index =
                            get_selected_version_index(&self.versions, self.current_version.as_deref());
                        menu.set_selected_by_index(selected_index, ctx);
                    });
                }
                ctx.notify();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Version manager detection
// ---------------------------------------------------------------------------

/// Helper: check if an executable exists in PATH
fn in_path(candidate: &str) -> bool {
    use std::env;

    if let Ok(path_var) = env::var("PATH") {
        for dir in env::split_paths(&path_var) {
            let mut p = dir.clone();
            p.push(candidate);
            if p.is_file() {
                return true;
            }
        }
    }
    false
}

// Cross-OS detection of nvm availability
fn detect_nvm_installed() -> bool {
    use std::env;

    // 1) Windows nvm-windows
    #[cfg(windows)]
    {
        env::var("PATH").is_ok_and(|path_var| path_var.contains("%NVM_HOME%"))
            || env::var("NVM_HOME").is_ok()
    }

    // 2) POSIX shells: nvm is typically a shell function; detect via standard install locations
    #[cfg(not(windows))]
    {
        use std::path::Path;

        // NVM_DIR env var with nvm.sh present
        if let Ok(nvm_dir) = env::var("NVM_DIR") {
            let nvm_sh = Path::new(&nvm_dir).join("nvm.sh");
            if nvm_sh.is_file() {
                return true;
            }
        }

        // Default NVM_DIR ~/.nvm/nvm.sh
        if let Some(home) = dirs::home_dir() {
            let nvm_sh = home.join(".nvm").join("nvm.sh");
            if nvm_sh.is_file() {
                return true;
            }
        }

        // Homebrew locations
        let brew_paths: &[&str] = &["/opt/homebrew/opt/nvm", "/usr/local/opt/nvm"];
        for base in brew_paths {
            let nvm_sh = Path::new(base).join("nvm.sh");
            if nvm_sh.is_file() {
                return true;
            }
        }

        // Fish plugin-based installs
        if let Some(home) = dirs::home_dir() {
            let fish_nvm = home
                .join(".config")
                .join("fish")
                .join("functions")
                .join("nvm.fish");
            if fish_nvm.is_file() {
                return true;
            }
            // macOS possible alt path for fish conf sometimes under Library
            let fish_alt = home
                .join("Library")
                .join("Application Support")
                .join("fish")
                .join("functions")
                .join("nvm.fish");
            if fish_alt.is_file() {
                return true;
            }
        }

        // If an `nvm` shim exists on PATH (rare on unix because it's a function), still check
        if in_path("nvm") {
            return true;
        }

        false
    }
}

/// Cross-OS detection of mise-en-place availability
fn detect_mise_installed() -> bool {
    use std::env;
    use std::path::Path;

    // 1) Check MISE_DATA_DIR env var
    if let Ok(mise_data_dir) = env::var("MISE_DATA_DIR") {
        let dir = Path::new(&mise_data_dir);
        if dir.is_dir() {
            return true;
        }
    }

    // 2) Check for mise binary in PATH
    if in_path("mise") {
        return true;
    }

    #[cfg(not(windows))]
    {
        if let Some(home) = dirs::home_dir() {
            // Default mise data directory: ~/.local/share/mise
            if home.join(".local/share/mise").is_dir() {
                return true;
            }

            // Alternative: ~/.mise (older installations or mise < v2024.x)
            if home.join(".mise").is_dir() {
                return true;
            }

            // Homebrew installations
            let brew_paths: &[&str] = &["/opt/homebrew/bin/mise", "/usr/local/bin/mise"];
            for path in brew_paths {
                if Path::new(path).is_file() {
                    return true;
                }
            }
        }
    }

    #[cfg(windows)]
    {
        // Windows: check APPDATA and LOCALAPPDATA
        if let Ok(appdata) = env::var("APPDATA") {
            if Path::new(&appdata).join("mise").is_dir() {
                return true;
            }
        }
        if let Ok(local_appdata) = env::var("LOCALAPPDATA") {
            if Path::new(&local_appdata).join("mise").is_dir() {
                return true;
            }
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Version listing
// ---------------------------------------------------------------------------

/// List installed Node.js versions using the given version manager.
fn list_versions(manager: VersionManager) -> Vec<String> {
    match manager {
        VersionManager::Nvm => list_nvm_versions(),
        VersionManager::Mise => list_mise_node_versions(),
    }
}

/// Enumerate installed Node versions managed by nvm (best-effort, cross-OS)
fn list_nvm_versions() -> Vec<String> {
    use std::env;
    use std::path::Path;

    let mut out: Vec<String> = Vec::new();

    #[cfg(windows)]
    {
        if let Ok(nvm_home) = env::var("NVM_HOME") {
            let base = Path::new(&nvm_home);
            if let Ok(read_dir) = std::fs::read_dir(base) {
                for entry in read_dir.flatten() {
                    if let Ok(ft) = entry.file_type() {
                        if ft.is_dir() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            // nvm-windows typically uses folder names like v18.19.1 or 18.19.1
                            if name
                                .chars()
                                .next()
                                .map(|c| c == 'v' || c.is_ascii_digit())
                                .unwrap_or(false)
                            {
                                out.push(name);
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        // Prefer $NVM_DIR/versions/node
        let mut candidates: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(nvm_dir) = env::var("NVM_DIR") {
            candidates.push(Path::new(&nvm_dir).join("versions").join("node"));
        }
        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join(".nvm").join("versions").join("node"));
        }

        for base in candidates {
            if let Ok(read_dir) = std::fs::read_dir(&base) {
                for entry in read_dir.flatten() {
                    if let Ok(ft) = entry.file_type() {
                        if ft.is_dir() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            out.push(name);
                        }
                    }
                }
            }
        }
    }

    // Sort descending so the latest version is first
    out.sort_by(|a, b| b.cmp(a));
    out.dedup();
    out
}

/// Enumerate installed Node versions managed by mise (best-effort, cross-OS)
///
/// Mise stores installs under `<data-dir>/installs/node/<version>/`, but also creates
/// symlinks for major/minor aliases and "latest" (e.g., `25 -> ./25.9.0`, `latest -> ./25.9.0`).
/// We only list real (non-symlink) directories that look like semantic versions
/// (e.g., `25.9.0`), matching how nvm lists only full version directories.
fn list_mise_node_versions() -> Vec<String> {
    use std::env;
    use std::path::Path;

    let mut out: Vec<String> = Vec::new();

    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    // Prefer $MISE_DATA_DIR/installs/node
    if let Ok(mise_data_dir) = env::var("MISE_DATA_DIR") {
        candidates.push(Path::new(&mise_data_dir).join("installs").join("node"));
    }

    #[cfg(not(windows))]
    {
        if let Some(home) = dirs::home_dir() {
            // Default: ~/.local/share/mise/installs/node
            candidates.push(home.join(".local/share/mise/installs/node"));
            // Alternative: ~/.mise/installs/node
            candidates.push(home.join(".mise/installs/node"));
        }
    }

    #[cfg(windows)]
    {
        // Windows: check APPDATA and LOCALAPPDATA
        if let Ok(appdata) = env::var("APPDATA") {
            candidates.push(Path::new(&appdata).join("mise").join("installs").join("node"));
        }
        if let Ok(local_appdata) = env::var("LOCALAPPDATA") {
            candidates.push(Path::new(&local_appdata).join("mise").join("installs").join("node"));
        }
    }

    for base in candidates {
        if let Ok(read_dir) = std::fs::read_dir(&base) {
            for entry in read_dir.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();

                // Skip symlinks — mise creates alias symlinks like `25 -> ./25.9.0`,
                // `latest -> ./25.9.0`, etc. We only want real installed version directories.
                let is_real_dir = entry
                    .file_type()
                    .ok()
                    .is_some_and(|ft| ft.is_dir() && !ft.is_symlink());

                if !is_real_dir {
                    continue;
                }

                // Only include entries that look like semantic versions
                // (e.g., "25.9.0", "18.19.1") — skip non-version names like "latest"
                if is_semantic_version(&name) {
                    out.push(name);
                }
            }
        }
    }

    // Sort descending so the latest version is first
    out.sort_by(|a, b| b.cmp(a));
    out.dedup();
    out
}

// ---------------------------------------------------------------------------
// Version comparison helpers
// ---------------------------------------------------------------------------

fn normalize_version(ver: &str) -> String {
    ver.trim()
        .strip_prefix('v')
        .unwrap_or(ver.trim())
        .to_string()
}

/// Checks whether a string looks like a semantic version (e.g., "18.19.1", "25.9.0").
/// This filters out non-version directory names like "latest" or arbitrary aliases
/// that mise may create as symlinks.
fn is_semantic_version(name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    // A semantic version consists of dot-separated numeric components
    // (e.g., "18", "18.19", "18.19.1"). All parts must be non-empty and numeric.
    name.split('.')
        .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

fn is_current_version(candidate: &str, current: Option<&str>) -> bool {
    current.is_some_and(|cur| normalize_version(candidate) == normalize_version(cur))
}

fn get_selected_version_index(versions: &[String], current_version: Option<&str>) -> usize {
    versions
        .iter()
        .position(|v| is_current_version(v, current_version))
        .unwrap_or(0)
}
