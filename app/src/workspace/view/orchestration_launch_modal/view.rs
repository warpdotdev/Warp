use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warp_core::ui::theme::{phenomenon::PhenomenonStyle, Fill};
use warpui::assets::asset_cache::AssetSource;
use warpui::elements::{
    Align, CacheOption, ChildAnchor, ChildView, Clipped, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, Expanded, Flex, Image, MainAxisSize, OffsetPositioning, ParentAnchor,
    ParentElement, ParentOffsetBounds, Radius, Stack, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::keymap::FixedBinding;
use warpui::{
    AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use crate::appearance::Appearance;
use crate::ui_components::icons::Icon;
use crate::view_components::action_button::{ActionButton, ActionButtonTheme, ButtonSize};

const MODAL_WIDTH: f32 = 420.;
const HERO_HEIGHT: f32 = 92.;
const HERO_IMAGE_PATH: &str = "async/png/onboarding/orchestration_launch_banner.png";
const LEARN_MORE_URL: &str = "https://warp.dev/placeholder-launch-blog-link";

struct FeatureItem {
    icon: Icon,
    title: &'static str,
    description: &'static str,
    badge: Option<&'static str>,
}

const FEATURE_ITEMS: &[FeatureItem] = &[
    FeatureItem {
        icon: Icon::Cloud,
        title: "Run any agent harness in the cloud",
        description: "Use Oz to spin up Claude Code or Codex agents in the cloud; Oz will help you track and steer the agents.",
        badge: None,
    },
    FeatureItem {
        icon: Icon::Atom02,
        title: "Multi-agent orchestration",
        description: "Warp Agents will now orchestrate subagents automatically, deploying and tracking parallel agents.",
        badge: None,
    },
    FeatureItem {
        icon: Icon::Cognition,
        title: "Agent Memory",
        description: "Agents will now store and access long-term memories, enabling self-improvement over time.",
        badge: Some("Research preview"),
    },
];

pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    app.register_fixed_bindings([FixedBinding::new(
        "escape",
        OrchestrationLaunchModalAction::Close,
        id!(OrchestrationLaunchModal::ui_name()),
    )]);
}

#[derive(Clone, Debug)]
pub enum OrchestrationLaunchModalAction {
    Close,
    LearnMore,
}

#[derive(Clone, Debug)]
pub enum OrchestrationLaunchModalEvent {
    Close,
}

struct CloseButtonTheme;

impl ActionButtonTheme for CloseButtonTheme {
    fn background(&self, hovered: bool, _appearance: &Appearance) -> Option<Fill> {
        if hovered {
            Some(Fill::Solid(PhenomenonStyle::modal_close_button_hover()))
        } else {
            None
        }
    }

    fn text_color(
        &self,
        _hovered: bool,
        _background: Option<Fill>,
        _appearance: &Appearance,
    ) -> ColorU {
        PhenomenonStyle::modal_close_button_text()
    }
}

struct LearnMoreButtonTheme;

impl ActionButtonTheme for LearnMoreButtonTheme {
    fn background(&self, hovered: bool, _appearance: &Appearance) -> Option<Fill> {
        if hovered {
            Some(Fill::Solid(PhenomenonStyle::subtle_border()))
        } else {
            None
        }
    }

    fn text_color(
        &self,
        _hovered: bool,
        _background: Option<Fill>,
        _appearance: &Appearance,
    ) -> ColorU {
        PhenomenonStyle::modal_feature_title_text()
    }

    fn border(&self, _appearance: &Appearance) -> Option<ColorU> {
        Some(PhenomenonStyle::subtle_border())
    }
}

struct CtaButtonTheme;

impl ActionButtonTheme for CtaButtonTheme {
    fn background(&self, hovered: bool, _appearance: &Appearance) -> Option<Fill> {
        Some(PhenomenonStyle::modal_button_background_fill(hovered))
    }

    fn text_color(
        &self,
        _hovered: bool,
        _background: Option<Fill>,
        _appearance: &Appearance,
    ) -> ColorU {
        PhenomenonStyle::modal_button_text()
    }
}

pub struct OrchestrationLaunchModal {
    close_button: ViewHandle<ActionButton>,
    learn_more_button: ViewHandle<ActionButton>,
    go_to_warp_button: ViewHandle<ActionButton>,
}

impl OrchestrationLaunchModal {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let close_button = ctx.add_view(|_ctx| {
            ActionButton::new("", CloseButtonTheme)
                .with_icon(Icon::X)
                .with_size(ButtonSize::Small)
                .on_click(|ctx| ctx.dispatch_typed_action(OrchestrationLaunchModalAction::Close))
        });

        let learn_more_button = ctx.add_view(|_ctx| {
            ActionButton::new("Learn more", LearnMoreButtonTheme)
                .with_icon(Icon::LinkExternal)
                .with_full_width(true)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(OrchestrationLaunchModalAction::LearnMore)
                })
        });

        let go_to_warp_button = ctx.add_view(|_ctx| {
            ActionButton::new("Go to Warp", CtaButtonTheme)
                .with_full_width(true)
                .on_click(|ctx| ctx.dispatch_typed_action(OrchestrationLaunchModalAction::Close))
        });

        Self {
            close_button,
            learn_more_button,
            go_to_warp_button,
        }
    }

    fn render_hero(&self) -> Box<dyn Element> {
        let hero = Clipped::new(
            ConstrainedBox::new(
                Image::new(
                    AssetSource::Bundled {
                        path: HERO_IMAGE_PATH,
                    },
                    CacheOption::Original,
                )
                .with_corner_radius(CornerRadius::with_top(Radius::Pixels(8.)))
                .cover()
                .top_aligned()
                .finish(),
            )
            .with_width(MODAL_WIDTH)
            .with_height(HERO_HEIGHT)
            .finish(),
        )
        .finish();

        let close_el = Container::new(ChildView::new(&self.close_button).finish())
            .with_uniform_padding(4.)
            .with_padding_right(2.)
            .finish();

        let mut hero_stack = Stack::new();
        hero_stack.add_child(hero);
        hero_stack.add_positioned_child(
            close_el,
            OffsetPositioning::offset_from_parent(
                vec2f(-4., 0.),
                ParentOffsetBounds::ParentByPosition,
                ParentAnchor::TopRight,
                ChildAnchor::TopRight,
            ),
        );
        hero_stack.finish()
    }

    fn render_badge(appearance: &Appearance) -> Box<dyn Element> {
        let text = Text::new_inline("New".to_string(), appearance.ui_font_family(), 14.)
            .with_color(PhenomenonStyle::modal_badge_text())
            .finish();
        ConstrainedBox::new(
            Container::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Min)
                    .with_child(text)
                    .finish(),
            )
            .with_horizontal_padding(8.)
            .with_background(Fill::Solid(PhenomenonStyle::modal_badge_background()))
            .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
            .finish(),
        )
        .with_height(24.)
        .finish()
    }

    fn render_title(appearance: &Appearance) -> Box<dyn Element> {
        Text::new(
            "Orchestrate any agent, anywhere",
            appearance.ui_font_family(),
            20.,
        )
        .with_color(PhenomenonStyle::modal_title_text())
        .with_style(Properties::default().weight(Weight::Semibold))
        .finish()
    }

    fn render_description(appearance: &Appearance) -> Box<dyn Element> {
        Text::new(
            "Major improvements to Warp's cloud agent orchestration platform, Oz.",
            appearance.ui_font_family(),
            14.,
        )
        .with_color(PhenomenonStyle::modal_feature_description_text())
        .finish()
    }

    fn render_feature_badge(label: &'static str, appearance: &Appearance) -> Box<dyn Element> {
        let font_family = appearance.ui_font_family();
        let color = PhenomenonStyle::modal_feature_description_text();
        Container::new(
            Text::new_inline(label.to_string(), font_family, 11.)
                .with_color(color)
                .finish(),
        )
        .with_horizontal_padding(6.)
        .with_vertical_padding(2.)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .with_background(Fill::Solid(color).with_opacity(15))
        .finish()
    }

    fn render_feature_row(&self, item: &FeatureItem, appearance: &Appearance) -> Box<dyn Element> {
        let icon_el = ConstrainedBox::new(
            item.icon
                .to_warpui_icon(Fill::Solid(
                    PhenomenonStyle::modal_feature_description_text(),
                ))
                .finish(),
        )
        .with_width(16.)
        .with_height(16.)
        .finish();

        let mut title_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(6.);
        title_row.add_child(
            Text::new_inline(item.title.to_string(), appearance.ui_font_family(), 14.)
                .with_color(PhenomenonStyle::modal_feature_title_text())
                .finish(),
        );
        if let Some(badge_label) = item.badge {
            title_row.add_child(Self::render_feature_badge(badge_label, appearance));
        }

        let text_col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_spacing(2.)
            .with_child(title_row.finish())
            .with_child(
                Text::new(item.description, appearance.ui_font_family(), 14.)
                    .with_color(PhenomenonStyle::modal_feature_description_text())
                    .finish(),
            )
            .finish();

        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_spacing(10.)
            .with_child(icon_el)
            .with_child(Expanded::new(1., text_col).finish())
            .finish()
    }

    fn render_body(&self, appearance: &Appearance) -> Box<dyn Element> {
        let mut features_col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_spacing(12.);
        for item in FEATURE_ITEMS {
            features_col.add_child(self.render_feature_row(item, appearance));
        }

        let footer = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(8.)
            .with_child(
                Expanded::new(1., ChildView::new(&self.learn_more_button).finish()).finish(),
            )
            .with_child(
                Expanded::new(1., ChildView::new(&self.go_to_warp_button).finish()).finish(),
            )
            .finish();

        Container::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(
                    Flex::column()
                        .with_cross_axis_alignment(CrossAxisAlignment::Start)
                        .with_spacing(8.)
                        .with_child(Self::render_badge(appearance))
                        .with_child(Self::render_title(appearance))
                        .with_child(Self::render_description(appearance))
                        .finish(),
                )
                .with_child(
                    Container::new(features_col.finish())
                        .with_margin_top(16.)
                        .finish(),
                )
                .with_child(Container::new(footer).with_margin_top(32.).finish())
                .finish(),
        )
        .with_horizontal_padding(32.)
        .with_vertical_padding(32.)
        .with_background(Fill::Solid(PhenomenonStyle::modal_background()))
        .with_corner_radius(CornerRadius::with_bottom(Radius::Pixels(8.)))
        .finish()
    }
}

impl Entity for OrchestrationLaunchModal {
    type Event = OrchestrationLaunchModalEvent;
}

impl View for OrchestrationLaunchModal {
    fn ui_name() -> &'static str {
        "OrchestrationLaunchModal"
    }

    fn on_focus(&mut self, _focus_ctx: &warpui::FocusContext, ctx: &mut ViewContext<Self>) {
        ctx.focus_self();
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);

        let card = ConstrainedBox::new(
            Container::new(
                Flex::column()
                    .with_main_axis_size(MainAxisSize::Min)
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_child(self.render_hero())
                    .with_child(self.render_body(appearance))
                    .finish(),
            )
            .with_background(Fill::Solid(PhenomenonStyle::modal_background()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
            .finish(),
        )
        .with_width(MODAL_WIDTH)
        .finish();

        Container::new(Align::new(card).finish())
            .with_background_color(ColorU::new(18, 18, 18, 128))
            .finish()
    }
}

impl TypedActionView for OrchestrationLaunchModal {
    type Action = OrchestrationLaunchModalAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            OrchestrationLaunchModalAction::Close => {
                ctx.emit(OrchestrationLaunchModalEvent::Close);
            }
            OrchestrationLaunchModalAction::LearnMore => {
                ctx.open_url(LEARN_MORE_URL);
            }
        }
    }
}
