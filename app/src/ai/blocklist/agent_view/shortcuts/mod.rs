mod model;

pub use model::*;
use pathfinder_color::ColorU;

use std::borrow::Cow;

use warp_core::{features::FeatureFlag, ui::appearance::Appearance};
use warpui::{
    elements::{Border, Container, CrossAxisAlignment, Expanded, Flex, ParentElement, Text},
    keymap::Keystroke,
    ui_components::components::{Coords, UiComponent, UiComponentStyles},
    AppContext, Element, SingletonEntity,
};

use crate::{
    ai::blocklist::agent_view::ENTER_AGENT_VIEW_NEW_CONVERSATION_KEYSTROKE,
    cmd_or_ctrl_shift,
    terminal::{self, TOGGLE_AUTOEXECUTE_MODE_KEYBINDING},
    ui_components::blended_colors,
    util::bindings::keybinding_name_to_keystroke,
    workspace::view::{
        TOGGLE_CONVERSATION_LIST_VIEW_BINDING_NAME, TOGGLE_RIGHT_PANEL_BINDING_NAME,
    },
};

#[derive(Copy, Clone, Debug, Default)]
pub struct AgentShortcutsViewContext {
    pub is_ambient_agent: bool,
    /// True once the user has submitted the first prompt.
    pub has_submitted_first_prompt: bool,
}

#[derive(Default)]
pub struct ShortcutProps {
    pub keystroke: Keystroke,
    pub text: Cow<'static, str>,
    pub text_color: Option<ColorU>,
}

pub fn render_shortcut(props: ShortcutProps, app: &AppContext) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let font_size = styles::font_size(appearance);
    let font_color = props.text_color.unwrap_or_else(|| {
        theme
            .sub_text_color(blended_colors::neutral_1(theme).into())
            .into()
    });
    Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(
            Container::new(render_keystroke(&props.keystroke, app))
                .with_margin_right(4.)
                .finish(),
        )
        .with_child(
            Expanded::new(
                1.,
                Text::new(props.text, appearance.ui_font_family(), font_size)
                    .with_color(font_color)
                    .finish(),
            )
            .finish(),
        )
        .finish()
}

pub fn render_keystroke(keystroke: &Keystroke, app: &AppContext) -> Box<dyn Element> {
    render_keystroke_with_color_overrides(keystroke, None, None, app)
}

pub fn render_keystroke_with_color_overrides(
    keystroke: &Keystroke,
    color: Option<ColorU>,
    background_color: Option<ColorU>,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let font_size = styles::font_size(appearance);
    appearance
        .ui_builder()
        .keyboard_shortcut(keystroke)
        .lowercase_modifier()
        .with_space_between_keys(2.)
        .with_style(UiComponentStyles {
            margin: Some(Coords::default()),
            padding: Some(Coords::default()),
            border_width: Some(1.),
            background: Some(
                background_color
                    .unwrap_or_else(|| blended_colors::neutral_3(theme))
                    .into(),
            ),
            font_color: Some(color.unwrap_or_else(|| theme.foreground().into_solid())),
            font_family_id: Some(appearance.ui_font_family()),
            font_size: Some(font_size),
            width: Some(styles::keystroke_size(appearance)),
            height: Some(styles::keystroke_size(appearance)),
            ..Default::default()
        })
        .with_line_height_ratio(1.0)
        .build()
        .finish()
}

pub fn render_agent_shortcuts_view(
    context: AgentShortcutsViewContext,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);

    let hide_ambient_zero_state_items =
        context.is_ambient_agent && !context.has_submitted_first_prompt;

    let mut shortcuts = vec![];

    if !hide_ambient_zero_state_items {
        shortcuts.push(render_shortcut(
            ShortcutProps {
                keystroke: Keystroke {
                    key: "!".to_owned(),
                    ..Default::default()
                },
                text: crate::t!("agent-shortcuts-input-shell-command").into(),
                ..Default::default()
            },
            app,
        ));
    }

    shortcuts.push(render_shortcut(
        ShortcutProps {
            keystroke: Keystroke {
                key: "/".to_owned(),
                ..Default::default()
            },
            text: crate::t!("agent-shortcuts-slash-commands").into(),
            ..Default::default()
        },
        app,
    ));

    shortcuts.push(render_shortcut(
        ShortcutProps {
            keystroke: Keystroke {
                key: "@".to_owned(),
                ..Default::default()
            },
            text: crate::t!("agent-shortcuts-file-paths-context").into(),
            ..Default::default()
        },
        app,
    ));

    // Code review is not available for ambient agent panes.
    if !context.is_ambient_agent {
        if let Some(keystroke) = keybinding_name_to_keystroke(TOGGLE_RIGHT_PANEL_BINDING_NAME, app)
        {
            shortcuts.push(render_shortcut(
                ShortcutProps {
                    keystroke,
                    text: crate::t!("agent-shortcuts-open-code-review").into(),
                    ..Default::default()
                },
                app,
            ));
        }
    }

    if FeatureFlag::AgentViewConversationListView.is_enabled() {
        if let Some(keystroke) =
            keybinding_name_to_keystroke(TOGGLE_CONVERSATION_LIST_VIEW_BINDING_NAME, app)
        {
            shortcuts.push(render_shortcut(
                ShortcutProps {
                    keystroke,
                    text: crate::t!("agent-shortcuts-toggle-conversation-list").into(),
                    ..Default::default()
                },
                app,
            ));
        }
    }

    shortcuts.push(render_shortcut(
        ShortcutProps {
            keystroke: Keystroke::parse(cmd_or_ctrl_shift("y")).expect("is valid keystroke"),
            text: crate::t!("agent-shortcuts-search-continue-conversations").into(),
            ..Default::default()
        },
        app,
    ));

    let new_conversation_keystroke = ENTER_AGENT_VIEW_NEW_CONVERSATION_KEYSTROKE.clone();

    shortcuts.push(render_shortcut(
        ShortcutProps {
            keystroke: new_conversation_keystroke.clone(),
            text: crate::t!("agent-shortcuts-start-new-conversation").into(),
            ..Default::default()
        },
        app,
    ));

    if !hide_ambient_zero_state_items {
        if let Some(keystroke) =
            keybinding_name_to_keystroke(TOGGLE_AUTOEXECUTE_MODE_KEYBINDING, app)
        {
            shortcuts.push(render_shortcut(
                ShortcutProps {
                    keystroke,
                    text: crate::t!("agent-shortcuts-toggle-auto-accept").into(),
                    ..Default::default()
                },
                app,
            ));
        }
    }

    shortcuts.push(render_shortcut(
        ShortcutProps {
            keystroke: Keystroke {
                key: "c".to_owned(),
                ctrl: true,
                ..Default::default()
            },
            text: crate::t!("agent-shortcuts-pause-agent").into(),
            ..Default::default()
        },
        app,
    ));

    shortcuts.push(render_shortcut(
        ShortcutProps {
            keystroke: Keystroke {
                key: "escape".to_owned(),
                ..Default::default()
            },
            text: crate::t!("agent-zero-state-go-back-to-terminal").into(),
            ..Default::default()
        },
        app,
    ));

    Container::new(
        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(8.)
            .with_children(shortcuts)
            .finish(),
    )
    .with_vertical_padding(16.)
    .with_padding_left(*terminal::view::PADDING_LEFT)
    .with_border(
        Border::new(1.)
            .with_sides(true, false, true, false)
            .with_border_color(blended_colors::neutral_2(appearance.theme())),
    )
    .finish()
}

pub mod styles {
    use warp_core::ui::appearance::Appearance;

    pub fn keystroke_size(appearance: &Appearance) -> f32 {
        font_size(appearance) + 2.
    }

    pub fn font_size(appearance: &Appearance) -> f32 {
        appearance.monospace_font_size() - 2.
    }
}
