// SPDX-License-Identifier: AGPL-3.0-only
//
// Doppler section UI for the Settings view (PDX-50 [A4.2]).
//
// Renders a single "Sign in with Doppler" button under a "Secrets (Doppler)"
// header. The button state is computed at render time from `doppler::detect()`:
//
//   * `Ok(_)`                          -> button enabled, no tooltip
//   * `Err(NotInstalled { hint })`     -> button disabled, tooltip = hint
//   * `Err(_)`                         -> button enabled with a warning hint
//
// On click, this spawns `doppler login` as a fire-and-forget subprocess. The
// Doppler CLI handles the browser handoff itself; we deliberately do NOT
// capture stdout/stderr or wait for the process. Status checking and
// project-picker UX are tracked separately (PDX-51, PDX-52, PDX-54).

use std::path::PathBuf;

use doppler::DopplerError;
use pathfinder_geometry::vector::vec2f;
use warpui::{
    elements::{
        Align, ChildAnchor, Container, CrossAxisAlignment, Element, Flex, Hoverable,
        MouseStateHandle, OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds,
        Stack, Text,
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
        MatchData, PageType, SettingsPageEvent, SettingsPageMeta, SettingsPageViewHandle,
        SettingsWidget, CONTENT_FONT_SIZE, SUBHEADER_FONT_SIZE,
    },
    SettingsSection,
};
use crate::appearance::Appearance;

/// Pure logic: given the current detect result, decide whether the
/// "Sign in with Doppler" button should be enabled and what (if any)
/// tooltip text to show on hover.
///
/// Extracted as a pure function so it can be tested without spinning up
/// the full GPUI render machinery — UI testing in GPUI is non-trivial.
///
/// Returned tuple: `(enabled, tooltip)`.
///   * `enabled` is `false` only when the binary is not installed.
///   * `tooltip` is `Some(_)` when there's a non-empty hint to surface.
///
/// Other detection errors keep the button enabled (the user may still want to
/// retry the login flow) but include a warning tooltip so the failure is
/// visible.
pub(crate) fn doppler_button_state(
    detect_result: &Result<PathBuf, DopplerError>,
) -> (bool, Option<String>) {
    match detect_result {
        Ok(_) => (true, None),
        Err(DopplerError::NotInstalled { install_hint }) => {
            (false, Some(install_hint.clone()))
        }
        Err(other) => (true, Some(format!("Doppler check warning: {other}"))),
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
                // its own. Status reporting will be wired up in PDX-52.
                if let Err(err) = std::process::Command::new("doppler")
                    .arg("login")
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
    tooltip_mouse_state: MouseStateHandle,
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
        let (enabled, tooltip_text) = doppler_button_state(&detect_result);

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

        // Build the button. Disabled when doppler isn't on PATH.
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

        let button_element = if enabled {
            button_builder
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(DopplerSettingsPageAction::SignIn);
                })
                .finish()
        } else {
            button_builder.disabled().build().finish()
        };

        // Wrap the button in a Hoverable so we can show a tooltip on hover
        // (used both for the install hint when disabled and warning text on
        // other detect errors). When there's no tooltip we just render the
        // button as-is.
        let button_with_tooltip: Box<dyn Element> = if let Some(tooltip) = tooltip_text {
            let tooltip_state = self.tooltip_mouse_state.clone();
            let mut stack = Stack::new().with_child(button_element);
            Hoverable::new(tooltip_state, move |mouse_state| {
                if mouse_state.is_hovered() {
                    let tip = appearance.ui_builder().tool_tip(tooltip.clone());
                    stack.add_positioned_overlay_child(
                        tip.build().finish(),
                        OffsetPositioning::offset_from_parent(
                            vec2f(0., 4.),
                            ParentOffsetBounds::Unbounded,
                            ParentAnchor::BottomMiddle,
                            ChildAnchor::TopMiddle,
                        ),
                    );
                }
                stack.finish()
            })
            .finish()
        } else {
            button_element
        };

        Container::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(header)
                .with_child(description)
                .with_child(button_with_tooltip)
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
    fn button_disabled_when_doppler_not_installed() {
        let install_hint = "brew install dopplerhq/cli/doppler".to_string();
        let result: Result<PathBuf, DopplerError> = Err(DopplerError::NotInstalled {
            install_hint: install_hint.clone(),
        });

        let (enabled, tooltip) = doppler_button_state(&result);

        assert!(!enabled, "button should be disabled when doppler not on PATH");
        let tooltip = tooltip.expect("tooltip must be present when disabled");
        assert!(
            tooltip.contains(&install_hint),
            "tooltip should contain the install hint, got: {tooltip}"
        );
    }

    #[test]
    fn button_enabled_when_doppler_present() {
        let result: Result<PathBuf, DopplerError> =
            Ok(PathBuf::from("/usr/local/bin/doppler"));

        let (enabled, tooltip) = doppler_button_state(&result);

        assert!(enabled, "button should be enabled when doppler detected");
        assert!(
            tooltip.is_none(),
            "no tooltip should be shown for the happy path"
        );
    }

    #[test]
    fn button_enabled_with_warning_for_other_errors() {
        // Other DopplerError variants (e.g. spawn failures) keep the button
        // clickable but surface a warning so the failure is visible.
        let result: Result<PathBuf, DopplerError> = Err(DopplerError::NotAuthenticated);

        let (enabled, tooltip) = doppler_button_state(&result);

        assert!(enabled, "non-NotInstalled errors keep the button enabled");
        assert!(tooltip.is_some(), "warning tooltip should be set");
    }
}
