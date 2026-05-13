use warpui::{
    elements::{ChildView, Container, CrossAxisAlignment, Expanded, Flex, ParentElement, Text},
    fonts::{Properties, Style, Weight},
    AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use crate::{
    ai::blocklist::block::view_impl::{
        common::render_user_avatar, CONTENT_HORIZONTAL_PADDING, CONTENT_ITEM_VERTICAL_MARGIN,
    },
    appearance::Appearance,
    ui_components::{blended_colors, icons::Icon},
    view_components::action_button::{ActionButton, ButtonSize, NakedTheme},
};

/// Renders a pending user query block with dimmed text and a "Queued" badge.
/// Displayed when a follow-up prompt is queued via `/fork-and-compact <prompt>`,
/// `/compact-and <prompt>`, `/queue <prompt>`, or for the initial prompt of a
/// Cloud Mode run waiting for its real shared-session transcript query to arrive.
pub struct PendingUserQueryBlock {
    prompt: String,
    user_display_name: String,
    profile_image_path: Option<String>,
    close_button: Option<ViewHandle<ActionButton>>,
    send_now_button: Option<ViewHandle<ActionButton>>,
}

impl PendingUserQueryBlock {
    pub fn new(
        prompt: String,
        user_display_name: String,
        profile_image_path: Option<String>,
        show_close_button: bool,
        show_send_now_button: bool,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let close_button = show_close_button.then(|| {
            ctx.add_typed_action_view(|_| {
                ActionButton::new("Remove queued prompt", NakedTheme)
                    .with_icon(Icon::X)
                    .with_size(ButtonSize::XSmall)
                    .on_click(|ctx| {
                        ctx.dispatch_typed_action(PendingUserQueryBlockAction::Dismiss);
                    })
            })
        });
        let send_now_button = show_send_now_button.then(|| {
            ctx.add_typed_action_view(|_| {
                ActionButton::new("Send now", NakedTheme)
                    .with_icon(Icon::Play)
                    .with_size(ButtonSize::XSmall)
                    .on_click(|ctx| {
                        ctx.dispatch_typed_action(PendingUserQueryBlockAction::SendNow);
                    })
            })
        });
        Self {
            prompt,
            user_display_name,
            profile_image_path,
            close_button,
            send_now_button,
        }
    }
}

#[derive(Clone, Debug)]
pub enum PendingUserQueryBlockAction {
    Dismiss,
    SendNow,
}

pub enum PendingUserQueryBlockEvent {
    Dismissed,
    SendNow,
}

impl Entity for PendingUserQueryBlock {
    type Event = PendingUserQueryBlockEvent;
}

impl TypedActionView for PendingUserQueryBlock {
    type Action = PendingUserQueryBlockAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            PendingUserQueryBlockAction::Dismiss => {
                ctx.emit(PendingUserQueryBlockEvent::Dismissed);
            }
            PendingUserQueryBlockAction::SendNow => {
                ctx.emit(PendingUserQueryBlockEvent::SendNow);
            }
        }
    }
}

impl View for PendingUserQueryBlock {
    fn ui_name() -> &'static str {
        "PendingUserQueryBlock"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let dimmed_color = blended_colors::text_sub(theme, theme.surface_1());

        let avatar = Container::new(render_user_avatar(
            &self.user_display_name,
            self.profile_image_path.as_ref(),
            None,
            app,
        ))
        .with_margin_right(16.)
        .finish();

        let properties = Properties {
            style: Style::Normal,
            weight: Weight::Bold,
        };

        let prompt_text = Text::new(
            self.prompt.clone(),
            appearance.monospace_font_family(),
            appearance.monospace_font_size(),
        )
        .with_style(properties)
        .with_color(dimmed_color)
        .with_selectable(false)
        .finish();

        let queued_badge = Text::new(
            "Queued",
            appearance.ui_font_family(),
            appearance.monospace_font_size().max(4.) - 2.,
        )
        .with_style(Properties {
            style: Style::Italic,
            weight: Weight::Normal,
        })
        .with_color(dimmed_color)
        .with_selectable(false)
        .finish();

        let text_column = Flex::column()
            .with_child(prompt_text)
            .with_child(Container::new(queued_badge).with_margin_top(4.).finish())
            .finish();

        let mut buttons_column = Flex::column().with_spacing(2.);
        if let Some(close_button) = &self.close_button {
            buttons_column.add_child(ChildView::new(close_button).finish());
        }
        if let Some(send_now_button) = &self.send_now_button {
            buttons_column.add_child(ChildView::new(send_now_button).finish());
        }

        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(avatar)
            .with_child(Expanded::new(1., text_column).finish());
        if self.close_button.is_some() || self.send_now_button.is_some() {
            let buttons = Container::new(buttons_column.finish())
                .with_margin_left(8.)
                .finish();
            row.add_child(buttons);
        }
        let row = row.finish();

        Container::new(row)
            .with_horizontal_padding(CONTENT_HORIZONTAL_PADDING)
            .with_padding_top(CONTENT_ITEM_VERTICAL_MARGIN)
            .with_padding_bottom(CONTENT_ITEM_VERTICAL_MARGIN)
            .finish()
    }
}
