// SPDX-License-Identifier: AGPL-3.0-only
//
// Doppler section UI for the Settings view (PDX-50 [A4.2]).
//
// Renders a "Sign in with Doppler" button under a "Secrets (Doppler)" header.
// PDX-55 [A4.7] replaces the generic tooltip fallback with per-variant inline
// error banners that carry a specific remediation hint:
//
//   Error state          Banner title                              Subtext
//   ─────────────────────────────────────────────────────────────────────────────
//   NotInstalled         "Doppler CLI not installed"               install command
//   NotAuthenticated     "Not signed in to Doppler"                "doppler login"
//   NoProjectBound       "No Doppler project configured…"          "doppler setup"
//   KeyMissing(name)     "Secret \"<name>\" not found in config"   dashboard hint
//   Unreachable          "Doppler API unreachable"                 network hint
//
// On sign-in click, `doppler login` is spawned fire-and-forget. The CLI
// handles the browser OAuth dance; we do NOT capture stdout/stderr.
// Status checking and project-picker UX are tracked in PDX-51, PDX-52, PDX-54.

use std::path::PathBuf;

use doppler::DopplerError;
use warpui::{
    elements::{
        Align, Container, CrossAxisAlignment, Element, Flex, MouseStateHandle, ParentElement, Text,
    },
    fonts::{Properties, Weight},
    ui_components::{
        button::ButtonVariant,
        components::{Coords, UiComponent, UiComponentStyles},
    },
    AppContext, Entity, TypedActionView, View, ViewContext, ViewHandle,
};

use super::{
    settings_page::{
        render_settings_info_banner, MatchData, PageType, SettingsPageEvent, SettingsPageMeta,
        SettingsPageViewHandle, SettingsWidget, CONTENT_FONT_SIZE, SUBHEADER_FONT_SIZE,
    },
    SettingsSection,
};
use crate::appearance::Appearance;

/// The distinct UI states the Doppler integration can be in.
///
/// Returned by [`doppler_ui_state`] and consumed by the render logic to decide
/// whether the sign-in button is enabled and which inline banner to show.
#[derive(Debug, PartialEq)]
pub(crate) enum DopplerUiState {
    /// CLI detected; no error to surface.
    Ready,
    /// `doppler` binary absent from `PATH`. Button disabled.
    NotInstalled { hint: String },
    /// User is not authenticated.
    NotAuthenticated,
    /// No project/config bound to the current working directory.
    NoProjectBound,
    /// The requested secret key does not exist in the bound config.
    KeyMissing { name: String },
    /// Doppler API is unreachable.
    Unreachable,
    /// Any other error not covered above.
    OtherError { message: String },
}

impl DopplerUiState {
    /// `false` only when the CLI binary is absent — nothing to launch.
    /// All other error states keep the button enabled so the user can retry
    /// after resolving the shown error.
    pub(crate) fn button_enabled(&self) -> bool {
        !matches!(self, DopplerUiState::NotInstalled { .. })
    }

    /// Returns `(title, remediation_hint)` for the inline banner, or `None`
    /// when the state is `Ready` and no banner should appear.
    pub(crate) fn banner_content(&self) -> Option<(String, String)> {
        match self {
            DopplerUiState::Ready => None,
            DopplerUiState::NotInstalled { hint } => Some((
                "Doppler CLI not installed".to_owned(),
                hint.clone(),
            )),
            DopplerUiState::NotAuthenticated => Some((
                "Not signed in to Doppler".to_owned(),
                "Run `doppler login` to authenticate, then click Sign in again".to_owned(),
            )),
            DopplerUiState::NoProjectBound => Some((
                "No Doppler project configured for this directory".to_owned(),
                "Run `doppler setup` in your project root to bind a config".to_owned(),
            )),
            DopplerUiState::KeyMissing { name } => Some((
                format!("Secret \"{name}\" not found in bound config"),
                "Verify the key name exists in your Doppler dashboard".to_owned(),
            )),
            DopplerUiState::Unreachable => Some((
                "Doppler API unreachable".to_owned(),
                "Check your network connection and try again".to_owned(),
            )),
            DopplerUiState::OtherError { message } => Some((
                "Doppler check failed".to_owned(),
                message.clone(),
            )),
        }
    }
}

/// Pure logic: map a detect (or fetch) result onto a [`DopplerUiState`].
///
/// Extracted as a pure function so it can be unit-tested without spinning up
/// the GPUI render machinery.
pub(crate) fn doppler_ui_state(result: &Result<PathBuf, DopplerError>) -> DopplerUiState {
    match result {
        Ok(_) => DopplerUiState::Ready,
        Err(DopplerError::NotInstalled { install_hint }) => DopplerUiState::NotInstalled {
            hint: install_hint.clone(),
        },
        Err(DopplerError::NotAuthenticated) => DopplerUiState::NotAuthenticated,
        Err(DopplerError::NoProjectBound) => DopplerUiState::NoProjectBound,
        Err(DopplerError::KeyMissing(name)) => DopplerUiState::KeyMissing {
            name: name.clone(),
        },
        Err(DopplerError::Unreachable) => DopplerUiState::Unreachable,
        Err(other) => DopplerUiState::OtherError {
            message: other.to_string(),
        },
    }
}

/// Action emitted when the user clicks the sign-in button. Fire-and-forget
/// `doppler login` is dispatched on the view (no event payload).
#[derive(Debug, Clone)]
pub enum DopplerSettingsPageAction {
    SignIn,
}

pub struct DopplerSettingsPageView {
    page: PageType<Self>,
}

impl DopplerSettingsPageView {
    pub fn new(_ctx: &mut ViewContext<Self>) -> Self {
        Self {
            page: PageType::new_uncategorized(
                vec![Box::new(DopplerSignInWidget::default())],
                None,
            ),
        }
    }
}

impl Entity for DopplerSettingsPageView {
    type Event = SettingsPageEvent;
}

impl TypedActionView for DopplerSettingsPageView {
    type Action = DopplerSettingsPageAction;

    fn handle_action(&mut self, action: &Self::Action, _ctx: &mut ViewContext<Self>) {
        match action {
            DopplerSettingsPageAction::SignIn => {
                // Fire-and-forget: do NOT wait, do NOT capture output. The
                // Doppler CLI opens the browser and runs the OAuth dance on
                // its own. --yes accepts the "already logged in -> overwrite"
                // prompt non-interactively; --scope . picks the cwd without
                // asking; nulling stdio prevents the child from blocking on
                // the GUI's missing TTY (it would hang at the interactive
                // prompt and never open the browser).
                if let Err(err) = std::process::Command::new("doppler")
                    .arg("login")
                    .arg("--yes")
                    .arg("--scope")
                    .arg(".")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    log::warn!("failed to spawn `doppler login`: {err}");
                }
            }
        }
    }
}

impl View for DopplerSettingsPageView {
    fn ui_name() -> &'static str {
        "DopplerPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

impl SettingsPageMeta for DopplerSettingsPageView {
    fn section() -> SettingsSection {
        SettingsSection::Doppler
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

impl From<ViewHandle<DopplerSettingsPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<DopplerSettingsPageView>) -> Self {
        SettingsPageViewHandle::Doppler(view_handle)
    }
}

#[derive(Default)]
struct DopplerSignInWidget {
    sign_in_button_mouse_state: MouseStateHandle,
}

impl SettingsWidget for DopplerSignInWidget {
    type View = DopplerSettingsPageView;

    fn search_terms(&self) -> &str {
        "doppler secrets sign in login cli"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        let ui_builder = appearance.ui_builder();
        let theme = appearance.theme();

        let detect_result = doppler::detect();
        let ui_state = doppler_ui_state(&detect_result);

        // Section header: "Secrets (Doppler)".
        let header = Container::new(
            Align::new(
                Text::new_inline(
                    "Secrets (Doppler)",
                    appearance.ui_font_family(),
                    SUBHEADER_FONT_SIZE,
                )
                .with_style(Properties::default().weight(Weight::Bold))
                .with_color(theme.active_ui_text_color().into())
                .finish(),
            )
            .left()
            .finish(),
        )
        .with_padding_bottom(8.)
        .finish();

        // Optional one-line description.
        let description = Container::new(
            Align::new(
                Text::new_inline(
                    "Sign in to your Doppler account to fetch secrets via the local CLI.",
                    appearance.ui_font_family(),
                    CONTENT_FONT_SIZE,
                )
                .with_color(theme.nonactive_ui_text_color().into())
                .finish(),
            )
            .left()
            .finish(),
        )
        .with_padding_bottom(12.)
        .finish();

        // Inline error banner — shown for every error state, each with its own
        // title and remediation subtext. Hidden when state is Ready.
        let maybe_banner: Option<Box<dyn Element>> =
            ui_state.banner_content().map(|(title, subtext)| {
                Container::new(render_settings_info_banner(
                    &title,
                    Some(&subtext),
                    appearance,
                ))
                .with_padding_bottom(12.)
                .finish()
            });

        // Sign-in button. Disabled only when the binary is absent.
        let button_builder = ui_builder
            .button(
                ButtonVariant::Accent,
                self.sign_in_button_mouse_state.clone(),
            )
            .with_text_label("Sign in with Doppler".to_owned())
            .with_style(UiComponentStyles {
                font_size: Some(14.),
                font_weight: Some(Weight::Semibold),
                padding: Some(Coords {
                    top: 8.,
                    bottom: 8.,
                    left: 24.,
                    right: 24.,
                }),
                ..Default::default()
            });

        let button_element: Box<dyn Element> = if ui_state.button_enabled() {
            button_builder
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(DopplerSettingsPageAction::SignIn);
                })
                .finish()
        } else {
            button_builder.disabled().build().finish()
        };

        let mut column_children: Vec<Box<dyn Element>> = vec![header, description];
        if let Some(banner) = maybe_banner {
            column_children.push(banner);
        }
        column_children.push(button_element);

        Container::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_children(column_children)
                .finish(),
        )
        .with_padding_bottom(15.)
        .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn ready_when_doppler_present() {
        let result: Result<PathBuf, DopplerError> = Ok(PathBuf::from("/usr/local/bin/doppler"));
        let state = doppler_ui_state(&result);
        assert_eq!(state, DopplerUiState::Ready);
        assert!(state.button_enabled());
        assert!(state.banner_content().is_none());
    }

    #[test]
    fn not_installed_disables_button_and_shows_install_hint() {
        let hint = "brew install dopplerhq/cli/doppler".to_string();
        let result: Result<PathBuf, DopplerError> = Err(DopplerError::NotInstalled {
            install_hint: hint.clone(),
        });
        let state = doppler_ui_state(&result);

        assert!(!state.button_enabled(), "button must be disabled when binary absent");
        let (title, subtext) = state.banner_content().expect("banner must appear");
        assert!(
            title.to_lowercase().contains("not installed"),
            "title should mention install: {title}"
        );
        assert_eq!(subtext, hint, "subtext should be the exact install hint");
    }

    #[test]
    fn no_project_bound_enables_button_with_setup_hint() {
        let result: Result<PathBuf, DopplerError> = Err(DopplerError::NoProjectBound);
        let state = doppler_ui_state(&result);

        assert!(state.button_enabled(), "button should remain enabled");
        let (title, subtext) = state.banner_content().expect("banner must appear");
        assert!(
            title.to_lowercase().contains("project"),
            "title should reference project binding: {title}"
        );
        assert!(
            subtext.contains("doppler setup"),
            "subtext must include the setup command: {subtext}"
        );
    }

    #[test]
    fn key_missing_enables_button_and_names_the_key() {
        let result: Result<PathBuf, DopplerError> =
            Err(DopplerError::KeyMissing("DATABASE_URL".to_owned()));
        let state = doppler_ui_state(&result);

        assert!(state.button_enabled(), "button should remain enabled");
        let (title, _subtext) = state.banner_content().expect("banner must appear");
        assert!(
            title.contains("DATABASE_URL"),
            "title must name the missing key: {title}"
        );
    }

    #[test]
    fn unreachable_enables_button_with_network_hint() {
        let result: Result<PathBuf, DopplerError> = Err(DopplerError::Unreachable);
        let state = doppler_ui_state(&result);

        assert!(state.button_enabled(), "button should remain enabled");
        let (title, subtext) = state.banner_content().expect("banner must appear");
        assert!(
            title.to_lowercase().contains("unreachable"),
            "title should indicate unreachability: {title}"
        );
        assert!(
            subtext.to_lowercase().contains("network"),
            "subtext should mention network: {subtext}"
        );
    }

    #[test]
    fn not_authenticated_enables_button_with_login_hint() {
        let result: Result<PathBuf, DopplerError> = Err(DopplerError::NotAuthenticated);
        let state = doppler_ui_state(&result);

        assert!(state.button_enabled(), "button should remain enabled");
        let (title, subtext) = state.banner_content().expect("banner must appear");
        assert!(
            title.to_lowercase().contains("sign") || title.to_lowercase().contains("auth"),
            "title should reference authentication: {title}"
        );
        assert!(
            subtext.contains("doppler login"),
            "subtext must include the login command: {subtext}"
        );
    }
}
