use std::{collections::HashMap, path::Path};

use warpui::{platform::SystemTheme, AppContext};

use crate::themes::theme::{
    resolve_theme_ref, RespectSystemTheme, SelectedSystemThemes, ThemeKind,
};
use settings::{
    macros::define_settings_group, RespectUserSyncSetting, Setting, SupportedPlatforms, SyncToCloud,
};

// Settings group for themes related settings.
// Note that we store just the information needed to derive the current
// theme state, which boils down to:
// ThemeKind: the theme to use when the system theme is off.
// UseSystemTheme: whether to respect the system theme.
// SelectedSystemThemes: the themes to use when the system theme is on.
define_settings_group!(ThemeSettings, settings: [
    theme_kind: Theme {
        type: ThemeKind,
        // Note that for new users, we now override this default value in SettingsInitializer
        // to set the default theme to Phenomenon.
        default: ThemeKind::default(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        toml_path: "appearance.themes.theme",
        max_table_depth: 0,
        description: "The color theme.",
    },
    use_system_theme: UseSystemTheme {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        storage_key: "SystemTheme",
        toml_path: "appearance.themes.system_theme",
        description: "Whether to match the system light/dark theme.",
    },
    selected_system_themes: SystemThemes {
        type: SelectedSystemThemes,
        default: SelectedSystemThemes::default(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        private: false,
        storage_key: "SelectedSystemThemes",
        toml_path: "appearance.themes.selected_system_themes",
        max_table_depth: 0,
        description: "The themes to use for system light and dark modes.",
    },
    directory_overrides: DirectoryOverrides {
        type: DirectoryThemeOverrides,
        default: DirectoryThemeOverrides::default(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        private: true,
        toml_path: "appearance.themes.directory_overrides",
        max_table_depth: 1,
        description: "Local-only map of directory paths to theme names. The focused pane cwd is matched against keys using longest-prefix component-boundary matching.",
    },
]);

impl Theme {
    fn current_value_is_syncable(&self) -> bool {
        let current_value = self.value();
        // Don't sync custom themes because they reference local files that aren't synced to the cloud.
        !matches!(current_value, ThemeKind::Custom(_))
    }
}

/// Returns a derived value for whether to respect the system theme based on
/// the current theme settings.
pub fn respect_system_theme(theme_settings: &ThemeSettings) -> RespectSystemTheme {
    if *theme_settings.use_system_theme.value() {
        RespectSystemTheme::On(theme_settings.selected_system_themes.value().clone())
    } else {
        RespectSystemTheme::Off
    }
}

/// Returns the current theme kind based on the theme settings and the system theme.
pub fn derived_theme_kind(theme_settings: &ThemeSettings, system_theme: SystemTheme) -> ThemeKind {
    let respect_system_theme = respect_system_theme(theme_settings);
    match respect_system_theme {
        RespectSystemTheme::On(selected_system_themes) => match system_theme {
            SystemTheme::Light => selected_system_themes.light.clone(),
            SystemTheme::Dark => selected_system_themes.dark.clone(),
        },
        RespectSystemTheme::Off => theme_settings.theme_kind.value().clone(),
    }
}

/// Return the current theme kind based on the theme settings and active app context.
pub fn active_theme_kind(theme_settings: &ThemeSettings, app: &AppContext) -> ThemeKind {
    derived_theme_kind(theme_settings, app.system_theme())
}

/// Local-only directory path to theme-reference mapping for per-tab theme overrides.
#[derive(
    Default,
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(description = "Local map of directory paths to tab theme overrides.")]
pub struct DirectoryThemeOverrides(pub HashMap<String, String>);

impl DirectoryThemeOverrides {
    /// Returns the resolved theme for `cwd` using longest-prefix matching at path-component boundaries.
    pub fn theme_for_directory(&self, cwd: &Path) -> Option<ThemeKind> {
        let cwd_components = normalized_path_components(&cwd.to_string_lossy());
        self.0
            .iter()
            .filter_map(|(configured_path, theme_ref)| {
                let components = normalized_path_components(configured_path);
                (!components.is_empty()
                    && components.len() <= cwd_components.len()
                    && components
                        .iter()
                        .zip(cwd_components.iter())
                        .all(|(lhs, rhs)| path_component_eq(lhs, rhs)))
                .then(|| (components.len(), resolve_theme_ref(theme_ref)))
            })
            .filter_map(|(len, theme)| theme.map(|theme| (len, theme)))
            .max_by_key(|(len, _)| *len)
            .map(|(_, theme)| theme)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

fn normalized_path_components(raw: &str) -> Vec<String> {
    let expanded = shellexpand::tilde(raw).into_owned();
    expanded
        .replace('\\', "/")
        .split('/')
        .filter_map(|component| {
            let trimmed = component.trim();
            (!trimmed.is_empty() && trimmed != ".").then(|| {
                #[cfg(any(target_os = "macos", windows))]
                {
                    trimmed.to_lowercase()
                }
                #[cfg(not(any(target_os = "macos", windows)))]
                {
                    trimmed.to_string()
                }
            })
        })
        .collect()
}

fn path_component_eq(lhs: &str, rhs: &str) -> bool {
    #[cfg(any(target_os = "macos", windows))]
    {
        lhs.eq_ignore_ascii_case(rhs)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        lhs == rhs
    }
}

#[cfg(test)]
mod directory_theme_override_tests {
    use super::DirectoryThemeOverrides;
    use crate::themes::theme::ThemeKind;
    use std::{collections::HashMap, path::Path};

    #[test]
    fn directory_theme_uses_component_boundary_matching() {
        let overrides = DirectoryThemeOverrides(HashMap::from([(
            "/tmp/work/medone".to_owned(),
            "Dark City".to_owned(),
        )]));

        assert_eq!(
            overrides.theme_for_directory(Path::new("/tmp/work/medone/apps/admin")),
            Some(ThemeKind::DarkCity)
        );
        assert_eq!(
            overrides.theme_for_directory(Path::new("/tmp/work/medone-archive")),
            None
        );
    }

    #[test]
    fn directory_theme_prefers_longest_matching_prefix() {
        let overrides = DirectoryThemeOverrides(HashMap::from([
            ("/tmp/work".to_owned(), "Dracula".to_owned()),
            ("/tmp/work/medone".to_owned(), "Solarized Dark".to_owned()),
        ]));

        assert_eq!(
            overrides.theme_for_directory(Path::new("/tmp/work/medone/api")),
            Some(ThemeKind::SolarizedDark)
        );
        assert_eq!(
            overrides.theme_for_directory(Path::new("/tmp/work/other")),
            Some(ThemeKind::Dracula)
        );
    }

    #[test]
    fn directory_theme_skips_unresolved_theme_values() {
        let overrides = DirectoryThemeOverrides(HashMap::from([(
            "/tmp/work".to_owned(),
            "Definitely Not A Theme".to_owned(),
        )]));

        assert_eq!(
            overrides.theme_for_directory(Path::new("/tmp/work/app")),
            None
        );
    }
}
