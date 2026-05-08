use super::{
    settings_page::{
        MatchData, PageType, SettingsPageEvent, SettingsPageMeta, SettingsPageViewHandle,
        SettingsWidget,
    },
    SettingsSection,
};
use crate::{
    appearance::Appearance, channel::ChannelState, themes::theme::ColorScheme,
    workspace::WorkspaceAction,
};
use serde::Deserialize;
use std::{path::Path, sync::OnceLock};
use warpui::{
    assets::asset_cache::AssetSource,
    elements::{
        Align, CacheOption, ConstrainedBox, Container, CrossAxisAlignment, Element, Flex, Image,
        MainAxisAlignment, MouseStateHandle, ParentElement, Wrap,
    },
    ui_components::components::UiComponent,
    AppContext, Entity, View, ViewContext, ViewHandle,
};

const ABOUT_PAGE_VERSION_PLACEHOLDER: &str = "v#.##.###";
const BUNDLED_VERSION_METADATA_PATH: &str = "bundled/metadata/version.json";

pub struct AboutPageView {
    page: PageType<Self>,
}

impl AboutPageView {
    pub fn new(_ctx: &mut ViewContext<AboutPageView>) -> Self {
        AboutPageView {
            page: PageType::new_monolith(AboutPageWidget::default(), None, false),
        }
    }
}

impl Entity for AboutPageView {
    type Event = SettingsPageEvent;
}

impl View for AboutPageView {
    fn ui_name() -> &'static str {
        "AboutPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

#[derive(Default)]
struct AboutPageWidget {
    copy_version_button_mouse_state: MouseStateHandle,
}

/// Stores the bundled Warp version written by the release packaging scripts.
#[derive(Deserialize)]
struct BundledVersionMetadata {
    warp_version: String,
}

impl SettingsWidget for AboutPageWidget {
    type View = AboutPageView;

    fn search_terms(&self) -> &str {
        "about warp version"
    }

    fn render(
        &self,
        _view: &AboutPageView,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let ui_builder = appearance.ui_builder();

        let image_path = if theme.inferred_color_scheme() == ColorScheme::LightOnDark {
            "bundled/svg/warp-logo-with-light-title.svg"
        } else {
            "bundled/svg/warp-logo-with-dark-title.svg"
        };

        let version = about_page_version();

        let version_text = ui_builder
            .span(version.to_string())
            .with_soft_wrap()
            .build()
            .with_margin_top(16.)
            .finish();

        let copy_version_icon = appearance
            .ui_builder()
            .copy_button(16., self.copy_version_button_mouse_state.clone())
            .build()
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::CopyVersion(version));
            })
            .finish();

        let version_row = Wrap::row()
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_children([
                version_text,
                Container::new(copy_version_icon)
                    .with_margin_top(16.)
                    .with_padding_left(6.)
                    .finish(),
            ]);

        Align::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    ConstrainedBox::new(
                        Image::new(
                            AssetSource::Bundled { path: image_path },
                            CacheOption::BySize,
                        )
                        .finish(),
                    )
                    .with_max_height(100.)
                    .with_max_width(350.)
                    .finish(),
                )
                .with_child(version_row.finish())
                .with_child(
                    ui_builder
                        .span("Copyright 2026 Warp")
                        .build()
                        .with_margin_top(16.)
                        .finish(),
                )
                .finish(),
        )
        .finish()
    }
}

/// Returns the version string shown on the About page.
fn about_page_version() -> &'static str {
    ChannelState::app_version()
        .or_else(bundled_app_version)
        .unwrap_or(ABOUT_PAGE_VERSION_PLACEHOLDER)
}

/// Returns the packaged Warp version from bundled metadata when available.
fn bundled_app_version() -> Option<&'static str> {
    static BUNDLED_APP_VERSION: OnceLock<Option<String>> = OnceLock::new();

    BUNDLED_APP_VERSION
        .get_or_init(load_bundled_app_version)
        .as_deref()
}

/// Loads the packaged Warp version from the bundled metadata file on disk.
fn load_bundled_app_version() -> Option<String> {
    let resources_dir = warp_core::paths::bundled_resources_dir()?;
    let version_metadata_path = resources_dir.join(BUNDLED_VERSION_METADATA_PATH);
    load_bundled_app_version_from_path(&version_metadata_path)
}

/// Reads and parses the bundled Warp version metadata file from the given path.
fn load_bundled_app_version_from_path(version_metadata_path: &Path) -> Option<String> {
    let contents = match std::fs::read_to_string(version_metadata_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
        Err(err) => {
            log::warn!(
                "Failed to read bundled version metadata from {}: {err:#}",
                version_metadata_path.display()
            );
            return None;
        }
    };

    match serde_json::from_str::<BundledVersionMetadata>(&contents) {
        Ok(metadata) if metadata.warp_version.is_empty() => {
            log::warn!(
                "Bundled version metadata at {} is missing warp_version",
                version_metadata_path.display()
            );
            None
        }
        Ok(metadata) => Some(metadata.warp_version),
        Err(err) => {
            log::warn!(
                "Failed to parse bundled version metadata from {}: {err:#}",
                version_metadata_path.display()
            );
            None
        }
    }
}

impl SettingsPageMeta for AboutPageView {
    fn section() -> SettingsSection {
        SettingsSection::About
    }

    fn should_render(&self, _ctx: &AppContext) -> bool {
        true
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        self.page.update_filter(query, ctx)
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.page.scroll_to_widget(widget_id)
    }

    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}

impl From<ViewHandle<AboutPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<AboutPageView>) -> Self {
        SettingsPageViewHandle::About(view_handle)
    }
}

#[cfg(test)]
mod tests {
    use super::load_bundled_app_version_from_path;

    /// Confirms that valid bundled metadata is surfaced as the About page version.
    #[test]
    fn loads_bundled_version_metadata_when_present() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let version_metadata_path = temp_dir.path().join("version.json");
        std::fs::write(
            &version_metadata_path,
            r#"{"warp_version":"v0.2026.05.08.06.21.oss"}"#,
        )
        .expect("version metadata should be written");

        let version = load_bundled_app_version_from_path(&version_metadata_path);

        assert_eq!(version.as_deref(), Some("v0.2026.05.08.06.21.oss"));
    }

    /// Confirms that malformed bundled metadata does not produce a misleading version string.
    #[test]
    fn ignores_invalid_bundled_version_metadata() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let version_metadata_path = temp_dir.path().join("version.json");
        std::fs::write(&version_metadata_path, r#"{"warp_version":42}"#)
            .expect("version metadata should be written");

        let version = load_bundled_app_version_from_path(&version_metadata_path);

        assert_eq!(version, None);
    }
}
