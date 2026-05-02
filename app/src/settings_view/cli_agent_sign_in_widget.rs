// SPDX-License-Identifier: AGPL-3.0-only
//
// Coding-agent sign-in widget for the AI settings page (PDX-103 [B1] task 6).
//
// Renders a "Sign in with Claude Code" row directly above the API keys (BYOK)
// section in `ai_page.rs`. PDX-104 (B2) appends Codex + Ollama rows here.
//
// Mirrors the Doppler pattern from PDX-49/PDX-50: the button shells out to
// `claude /login` fire-and-forget; the CLI handles the browser/keychain.
//
// Auth-state probe is stubbed pending PDX-103 task 2 (persistent Router in
// AppContext). Once that lands, swap `CliAgentAuthState::detect_claude` to
// read `Router::health(&AgentId::new("claude-sonnet-46"))` from AppContext
// and gate `ClaudeCodeAgent` registration on the result.

use warpui::elements::{
    Align, Container, CrossAxisAlignment, Element, Flex, MouseStateHandle, ParentElement, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::ui_components::{
    button::ButtonVariant,
    components::{Coords, UiComponent, UiComponentStyles},
};
use warpui::AppContext;

use super::ai_page::{AISettingsPageAction, AISettingsPageView};
use super::settings_page::{
    render_settings_info_banner, SettingsWidget, CONTENT_FONT_SIZE, SUBHEADER_FONT_SIZE,
};
use crate::appearance::Appearance;

/// Three-state auth model surfaced inline in the row.
///
/// Detection is intentionally cheap and synchronous; the auth-probe upgrade
/// (PATH check + `Router::health` lookup) lands in PDX-103 task 6 once the
/// Router is hoisted into AppContext per task 2.
#[derive(Debug, PartialEq)]
#[allow(dead_code)] // `NotInstalled` and `SignedIn` exercised once PDX-103 task 6 lands the real probe.
pub(crate) enum CliAgentAuthState {
    NotInstalled,
    SignedOut,
    SignedIn,
}

impl CliAgentAuthState {
    /// Skeleton stub — always reports `SignedOut`. Replace once `which::which`
    /// is wired in and the Router exposes a health view (PDX-103 task 2 + 6).
    pub(crate) fn detect_claude() -> Self {
        // TODO(PDX-103 task 6): replace stub with
        //   match which::which("claude") {
        //       Err(_) => CliAgentAuthState::NotInstalled,
        //       Ok(_) => router_health(&AgentId::new("claude-sonnet-46"))
        //                   .map(|h| if h.healthy { SignedIn } else { SignedOut })
        //                   .unwrap_or(SignedOut),
        //   }
        Self::SignedOut
    }

    fn banner(&self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::NotInstalled => Some((
                "Claude Code CLI not installed",
                "Install via `brew install anthropic/claude/claude` or follow https://docs.claude.com/claude-code/setup",
            )),
            Self::SignedOut => Some((
                "Not signed in to Claude Code",
                "Click \"Sign in with Claude Code\" — the CLI opens your browser for OAuth.",
            )),
            Self::SignedIn => Some((
                "Signed in to Claude Code",
                "Available in the in-prompt model selector.",
            )),
        }
    }

    fn button_enabled(&self) -> bool {
        !matches!(self, Self::NotInstalled)
    }

    fn button_label(&self) -> &'static str {
        match self {
            Self::NotInstalled => "Install Claude Code",
            Self::SignedOut => "Sign in with Claude Code",
            Self::SignedIn => "Re-sign in with Claude Code",
        }
    }
}

#[derive(Default)]
pub(super) struct CliAgentSignInWidget {
    claude_button: MouseStateHandle,
}

impl SettingsWidget for CliAgentSignInWidget {
    type View = AISettingsPageView;

    fn search_terms(&self) -> &str {
        "claude code codex ollama cli sign in login authenticate third-party coding agent"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        let ui_builder = appearance.ui_builder();
        let theme = appearance.theme();
        let state = CliAgentAuthState::detect_claude();

        let header = Container::new(
            Align::new(
                Text::new_inline(
                    "Coding agent sign-in",
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

        let description = Container::new(
            Align::new(
                Text::new_inline(
                    "Sign in to coding agents Warp dispatches through. Each row shells out to that agent's own login flow.",
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

        let banner: Option<Box<dyn Element>> = state.banner().map(|(title, sub)| {
            Container::new(render_settings_info_banner(title, Some(sub), appearance))
                .with_padding_bottom(12.)
                .finish()
        });

        let button_style = UiComponentStyles {
            font_size: Some(14.),
            font_weight: Some(Weight::Semibold),
            padding: Some(Coords {
                top: 8.,
                bottom: 8.,
                left: 24.,
                right: 24.,
            }),
            ..Default::default()
        };
        let btn_builder = ui_builder
            .button(ButtonVariant::Accent, self.claude_button.clone())
            .with_text_label(state.button_label().to_owned())
            .with_style(button_style);

        let button: Box<dyn Element> = if state.button_enabled() {
            btn_builder
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(AISettingsPageAction::SignInWithClaudeCode);
                })
                .finish()
        } else {
            btn_builder.disabled().build().finish()
        };

        // TODO(PDX-104 task 5): append Codex + Ollama rows below this button
        // sharing the same header/description/auth-state pattern. Codex follows
        // the same sign-in flow (`codex login`); Ollama is local-only with an
        // "Install Ollama" link instead of a sign-in button.

        let mut children: Vec<Box<dyn Element>> = vec![header, description];
        if let Some(b) = banner {
            children.push(b);
        }
        children.push(button);

        Container::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_children(children)
                .finish(),
        )
        .with_padding_bottom(15.)
        .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_installed_disables_button() {
        let s = CliAgentAuthState::NotInstalled;
        assert!(!s.button_enabled());
        assert_eq!(s.button_label(), "Install Claude Code");
        let (title, _) = s.banner().expect("banner");
        assert!(title.to_lowercase().contains("not installed"));
    }

    #[test]
    fn signed_out_enables_button_with_login_label() {
        let s = CliAgentAuthState::SignedOut;
        assert!(s.button_enabled());
        assert_eq!(s.button_label(), "Sign in with Claude Code");
    }

    #[test]
    fn signed_in_offers_resign_path() {
        let s = CliAgentAuthState::SignedIn;
        assert!(s.button_enabled());
        assert!(s.button_label().contains("Re-sign in"));
    }
}
