use pathfinder_color::ColorU;
use warp_cli::agent::Harness;
use warpui::elements::{CornerRadius, Element, Radius};
use warpui::ui_components::components::{UiComponent, UiComponentStyles};
use warpui::{AppContext, SingletonEntity};

use crate::ai::agent::conversation::ConversationStatus;
use crate::appearance::Appearance;
use crate::ui_components::agent_icon::agent_icon_variant_for_run;
use crate::ui_components::avatar::{Avatar, AvatarContent};
use crate::ui_components::blended_colors;
use crate::ui_components::icon_with_status::{render_icon_with_status, IconWithStatusVariant};
use crate::ui_components::icons::Icon;
const AGENT_ICON_CIRCLE_RATIO: f32 = 0.76;
const TRANSCRIPT_AVATAR_SCALE: f32 = 1.25;
const CHILD_AVATAR_ICONS: [Icon; 4] = [Icon::Orbit, Icon::Rocket, Icon::Lightning, Icon::Dataflow];
const CHILD_AVATAR_BACKGROUNDS: [ColorU; 4] = [
    ColorU {
        r: 93,
        g: 95,
        b: 239,
        a: 255,
    },
    ColorU {
        r: 28,
        g: 160,
        b: 90,
        a: 255,
    },
    ColorU {
        r: 229,
        g: 160,
        b: 26,
        a: 255,
    },
    ColorU {
        r: 203,
        g: 176,
        b: 247,
        a: 255,
    },
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OrchestrationAvatar {
    Orchestrator,
    Agent {
        display_name: String,
        harness: Harness,
        status: ConversationStatus,
        is_ambient: bool,
    },
}

impl OrchestrationAvatar {
    pub(crate) fn agent(
        display_name: String,
        harness: Harness,
        status: ConversationStatus,
        is_ambient: bool,
    ) -> Self {
        Self::Agent {
            display_name,
            harness,
            status,
            is_ambient,
        }
    }

    pub(crate) fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let size = app.font_cache().line_height(
            appearance.monospace_font_size(),
            appearance.line_height_ratio(),
        ) * TRANSCRIPT_AVATAR_SCALE;

        match self {
            Self::Orchestrator => {
                let background = blended_colors::accent(theme);
                Avatar::new(
                    AvatarContent::Icon(Icon::AgentMode),
                    UiComponentStyles {
                        width: Some(size),
                        height: Some(size),
                        font_family_id: Some(appearance.ui_font_family()),
                        font_size: Some(appearance.monospace_font_size() - 2.),
                        background: Some(background.into()),
                        font_color: Some(blended_colors::text_main(theme, background)),
                        border_radius: Some(CornerRadius::with_all(Radius::Percentage(50.))),
                        ..Default::default()
                    },
                )
                .build()
                .finish()
            }
            Self::Agent {
                display_name,
                harness,
                status,
                is_ambient,
            } => {
                if let Some(variant) =
                    child_agent_icon_variant(*harness, status.clone(), *is_ambient)
                {
                    // `render_icon_with_status` reserves extra room for status/cloud overlays, so
                    // the brand circle is smaller than the total footprint. Scale the footprint up
                    // here so the visible child-agent circle matches the orchestrator avatar's
                    // diameter.
                    let icon_size = size / AGENT_ICON_CIRCLE_RATIO;
                    return render_icon_with_status(
                        variant,
                        icon_size,
                        0.0,
                        theme,
                        theme.background(),
                    );
                }

                render_child_avatar(display_name, size, appearance)
            }
        }
    }
}

pub(crate) fn child_agent_icon_variant(
    harness: Harness,
    status: ConversationStatus,
    is_ambient: bool,
) -> Option<IconWithStatusVariant> {
    match harness {
        Harness::Claude | Harness::OpenCode | Harness::Gemini | Harness::Codex => {
            Some(agent_icon_variant_for_run(harness, status, is_ambient))
        }
        Harness::Oz | Harness::Unknown => None,
    }
}

fn render_child_avatar(display_name: &str, size: f32, appearance: &Appearance) -> Box<dyn Element> {
    let index = child_avatar_index(display_name);
    let background = CHILD_AVATAR_BACKGROUNDS[index];
    Avatar::new(
        AvatarContent::Icon(CHILD_AVATAR_ICONS[index]),
        UiComponentStyles {
            width: Some(size),
            height: Some(size),
            font_family_id: Some(appearance.ui_font_family()),
            font_size: Some(appearance.monospace_font_size() - 2.),
            background: Some(background.into()),
            font_color: Some(blended_colors::text_main(appearance.theme(), background)),
            border_radius: Some(CornerRadius::with_all(Radius::Percentage(50.))),
            ..Default::default()
        },
    )
    .build()
    .finish()
}

fn child_avatar_index(display_name: &str) -> usize {
    display_name.bytes().fold(0usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    }) % CHILD_AVATAR_ICONS.len()
}

#[cfg(test)]
#[path = "orchestration_avatar_tests.rs"]
mod tests;
