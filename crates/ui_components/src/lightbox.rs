use std::sync::Arc;

use instant::Instant;
use pathfinder_geometry::vector::{Vector2F, vec2f};
use warp_core::ui::{Icon, appearance::Appearance};
use warpui::{
    assets::asset_cache::AssetSource,
    elements::{CacheOption, Dismiss, DispatchEventResult, EventHandler, Image, Shrinkable},
    keymap::Keystroke,
    prelude::{stack::*, *},
};

use crate::{Component, Options as _, button};

/// Padding between the scrim edge and the image.
const SCRIM_PADDING: f32 = 48.;

/// GH9729 §699: how much smaller the metadata strip is than the
/// description. The metadata strip carries secondary information
/// (dimensions, format, size) and should read as supporting detail,
/// not a peer of the description.
const METADATA_TEXT_SIZE_REDUCTION: f32 = 2.;

/// GH9729 §699: alpha for the metadata strip's foreground colour.
/// 0.7 keeps it legible while making clear it's secondary.
const METADATA_TEXT_ALPHA: u8 = 178; // 255 * 0.7

fn metadata_text_size(appearance: &Appearance) -> f32 {
    (lightbox_text_size(appearance) - METADATA_TEXT_SIZE_REDUCTION).max(8.0)
}

fn metadata_text_color() -> ColorU {
    ColorU::new(255, 255, 255, METADATA_TEXT_ALPHA)
}

/// GH9729 §698: minimum zoom factor — below this the image becomes too
/// small to be useful and the controls feel runaway.
pub const MIN_ZOOM_FACTOR: f32 = 0.25;

/// GH9729 §698: maximum zoom factor — above this typical raster decode
/// pixels exceed any reasonable display, and the per-paint resampling
/// cost grows quadratically.
pub const MAX_ZOOM_FACTOR: f32 = 8.0;

/// GH9729 §698: multiplicative step applied per zoom-in / zoom-out
/// keystroke. `1.5` reaches `MAX_ZOOM_FACTOR = 8.0` in five `+` presses
/// and `MIN_ZOOM_FACTOR = 0.25` in four `-` presses from the
/// `1.0` default.
pub const ZOOM_STEP: f32 = 1.5;

/// Spacing between the image/loading area and the description text.
const DESCRIPTION_SPACING: f32 = 12.;
const LIGHTBOX_TEXT_SIZE_DELTA: f32 = 4.;

/// Semi-transparent black background color for the scrim.
fn scrim_color() -> ColorU {
    ColorU::new(0, 0, 0, 230)
}

/// The loading state of a lightbox image.
#[derive(Clone, Debug)]
pub enum LightboxImageSource {
    /// The image metadata is still being fetched.
    Loading,
    /// The image source has been resolved.
    /// Note: the actual image bytes may still be loading via the `AssetCache`.
    Resolved { asset_source: AssetSource },
    /// The image could not be loaded or decoded. The Lightbox renders a
    /// non-blocking error panel inline (see `Lightbox::render`) and dismissal
    /// (Escape, scrim click, ×) continues to work.
    ///
    /// `message` is intended to be one of a small set of sanitized,
    /// human-readable categorical strings. Underlying OS errors and absolute
    /// filesystem paths must NOT be interpolated here; log the original via
    /// `log::warn!` for the operator instead. See `specs/GH9729/tech.md` §182.
    Error { message: String },
}

/// A single image entry in the lightbox.
#[derive(Clone, Debug)]
pub struct LightboxImage {
    /// The loading/loaded state of this image.
    pub source: LightboxImageSource,
    /// Optional description displayed below the image.
    pub description: Option<String>,
}

/// Direction for navigating between images.
#[derive(Clone, Copy, Debug)]
pub enum NavigationDirection {
    Previous,
    Next,
}

/// A handler invoked when the user navigates between images.
pub type NavigateHandler = Arc<dyn Fn(NavigationDirection, &mut EventContext, &AppContext)>;

/// A lightbox component for displaying images in a full-window overlay.
///
/// The lightbox displays one or more images centered on screen with a semi-transparent scrim
/// background. It supports navigating between images via arrow buttons and can be dismissed by
/// clicking outside the image, clicking the close button, or pressing Escape.
#[derive(Default)]
pub struct Lightbox {
    close_button: button::Button,
    prev_button: button::Button,
    next_button: button::Button,
}

pub struct Params<'a> {
    /// The list of images to display.
    pub images: &'a [LightboxImage],

    /// The index of the currently displayed image.
    pub current_index: usize,

    /// Handler to invoke when the lightbox is dismissed.
    pub on_dismiss: DismissHandler,

    /// The native pixel dimensions of the currently displayed image, if known.
    /// When `Some`, the image is fully loaded and the lightbox renders it with a
    /// `ConstrainedBox` plus description. When `None`, the lightbox shows a loading
    /// indicator instead.
    pub current_image_native_size: Option<Vector2F>,

    /// GH9729 §697: timeline anchor for animated GIF/WebP frame progression.
    /// When `Some`, the rendered `Image` is wired with
    /// `enable_animation_with_start_time`, driving continuous playback via
    /// the implicit per-frame `ctx.repaint_after` loop in
    /// `Image::paint_animated_image`. When `None`, the image renders the
    /// first frame only (legacy behaviour, kept for callers that don't
    /// want animation, e.g. inert example/test surfaces).
    ///
    /// Static images ignore this field entirely.
    pub animation_start_time: Option<Instant>,

    /// GH9729 §699: optional pre-formatted metadata strip rendered
    /// below the description in a smaller, dimmer font (filename
    /// already lives in `description`, so the strip typically carries
    /// `"<width>×<height>"` plus format / size when the caller knows
    /// them). `None` hides the strip entirely. The string is rendered
    /// verbatim — the caller is responsible for sanitising and
    /// localising it.
    pub metadata_line: Option<String>,

    /// GH9729 §698: zoom factor applied to the image's bounding box.
    /// `1.0` renders at native size (the v1 default). Values `> 1.0`
    /// scale the `ConstrainedBox` linearly so the image renders larger
    /// (the surrounding `Align` keeps it centred; oversized images
    /// overflow the centred stack — the lightbox scrim is full-window
    /// so they stay inside the window). `< 1.0` shrinks below native
    /// size for very-large images.
    ///
    /// The companion drag-to-pan deliverable from §698 is deferred —
    /// see `TIER2_TODO.md::t2-7-pan` — because this GPUI fork has no
    /// `Translate`/`Offset` primitive that lets us shift an element
    /// during paint without an upstream addition.
    pub zoom_factor: f32,

    /// Optional configuration for the lightbox.
    pub options: Options,
}

impl crate::Params for Params<'_> {
    type Options<'a> = Options;
}

/// A function that handles dismiss events.
pub type DismissHandler = Arc<dyn Fn(&mut EventContext, &AppContext)>;

pub struct Options {
    /// Optional keystroke associated with the dismiss action. This will be rendered alongside
    /// the dismiss button in the dialog, but the caller is responsible for adding a keybinding.
    pub dismiss_keystroke: Option<Keystroke>,

    /// Handler to invoke when the user navigates between images.
    /// If `None`, navigation buttons are not shown.
    pub on_navigate: Option<NavigateHandler>,
}

impl crate::Options for Options {
    fn default(_appearance: &Appearance) -> Self {
        Self {
            dismiss_keystroke: None,
            on_navigate: None,
        }
    }
}

impl Component for Lightbox {
    type Params<'a> = Params<'a>;

    fn render<'a>(&self, appearance: &Appearance, params: Self::Params<'a>) -> Box<dyn Element> {
        let on_dismiss_for_button = params.on_dismiss.clone();
        let on_dismiss = params.on_dismiss;
        let image_count = params.images.len();
        let current_index = params.current_index;

        // Extract current image data via direct indexing.
        let current_image = params.images.get(current_index);
        let current_source = current_image.map(|img| &img.source);
        let current_description = current_image.and_then(|img| img.description.clone());
        let text_size = lightbox_text_size(appearance);

        // Close button in the top-right corner.
        let close_button = self.close_button.render(
            appearance,
            button::Params {
                content: button::Content::Icon(Icon::X),
                theme: &button::themes::Secondary,
                options: button::Options {
                    size: button::Size::Small,
                    on_click: Some(Box::new(move |ctx, app, _| {
                        on_dismiss_for_button(ctx, app);
                    })),
                    keystroke: params.options.dismiss_keystroke,
                    ..button::Options::default(appearance)
                },
            },
        );

        // Build the central content based on the image source and whether the
        // native size is known (i.e. the image data has been loaded).
        let central_content: Box<dyn Element> =
            match (current_source, params.current_image_native_size) {
                // Image source resolved AND native size known → render the image.
                (Some(LightboxImageSource::Resolved { asset_source }), Some(native_size)) => {
                    // GH9729 §697: opt into continuous animated playback when
                    // the caller supplied a timeline anchor. The Image
                    // element's `paint_animated_image` schedules its own
                    // per-frame redraw via `ctx.repaint_after`, so no
                    // explicit timer or task is needed at this layer.
                    let mut image_builder =
                        Image::new(asset_source.clone(), CacheOption::Original).contain();
                    if let Some(start) = params.animation_start_time {
                        image_builder = image_builder.enable_animation_with_start_time(start);
                    }
                    // GH9729 §698: scale the bounding box by the caller-
                    // supplied zoom factor. Negative or non-finite values
                    // would NaN-poison the layout, so clamp to a sane
                    // range first.
                    let zoom = params.zoom_factor.clamp(MIN_ZOOM_FACTOR, MAX_ZOOM_FACTOR);
                    let image = ConstrainedBox::new(
                        image_builder
                            .before_load(Align::new(loading_element(appearance)).finish())
                            .finish(),
                    )
                    .with_max_width(native_size.x() * zoom)
                    .with_max_height(native_size.y() * zoom)
                    .finish();

                    EventHandler::new(image)
                        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
                        .finish()
                }
                // Per-image load/decode failure: render a non-blocking error
                // panel with the filename (description) on one line and the
                // sanitized message on the next. Dismissal (Escape, scrim, ×)
                // continues to work; this panel never traps focus. See
                // specs/GH9729/tech.md §182.
                (Some(LightboxImageSource::Error { message }), _) => {
                    let mut column = Flex::column()
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_spacing(DESCRIPTION_SPACING);
                    if let Some(description) = current_description.clone() {
                        column = column.with_child(
                            Text::new(description, appearance.ui_font_family(), text_size)
                                .with_color(ColorU::white())
                                .finish(),
                        );
                    }
                    column = column.with_child(
                        Text::new(message.clone(), appearance.ui_font_family(), text_size)
                            .with_color(ColorU::white())
                            .finish(),
                    );
                    column.finish()
                }
                // No images provided at all.
                _ if image_count == 0 => {
                    Text::new("No images", appearance.ui_font_family(), text_size)
                        .with_color(ColorU::white())
                        .finish()
                }
                // Still loading (either metadata or image bytes).
                _ => loading_element(appearance),
            };

        // Show the description only when the image is fully loaded (native size known).
        // GH9729 §699: also append the optional metadata strip (smaller,
        // dimmer text) under the description on the same gating, so the
        // footer never appears next to a half-loaded spinner.
        let content_with_description = if let (Some(description), Some(_)) =
            (current_description, params.current_image_native_size)
        {
            let description_text = Text::new(description, appearance.ui_font_family(), text_size)
                .with_color(ColorU::white())
                .finish();

            let mut column = Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_spacing(DESCRIPTION_SPACING)
                .with_child(Shrinkable::new(1.0, central_content).finish())
                .with_child(description_text);

            if let Some(metadata_line) = params.metadata_line {
                let metadata_text = Text::new(
                    metadata_line,
                    appearance.ui_font_family(),
                    metadata_text_size(appearance),
                )
                .with_color(metadata_text_color())
                .finish();
                column = column.with_child(metadata_text);
            }

            column.finish()
        } else {
            central_content
        };

        let centered_content = Align::new(content_with_description).finish();

        let scrim = Container::new(
            Dismiss::new(centered_content)
                .prevent_interaction_with_other_elements()
                .on_dismiss(move |ctx, app| on_dismiss(ctx, app))
                .finish(),
        )
        .with_background_color(scrim_color())
        .with_uniform_padding(SCRIM_PADDING)
        .finish();

        // Stack the scrim, close button, and optional navigation arrows.
        let mut content = Stack::new().with_child(scrim);
        content.add_positioned_child(
            close_button,
            OffsetPositioning::offset_from_parent(
                vec2f(-12., 12.),
                ParentOffsetBounds::Unbounded,
                ParentAnchor::TopRight,
                ChildAnchor::TopRight,
            ),
        );

        // Navigation arrows (only shown when there are multiple images).
        if image_count > 1
            && let Some(on_navigate) = params.options.on_navigate
        {
            // Previous button (hidden on first image).
            if current_index > 0 {
                let on_nav = on_navigate.clone();
                let prev_button = self.prev_button.render(
                    appearance,
                    button::Params {
                        content: button::Content::Icon(Icon::ChevronLeft),
                        theme: &button::themes::Secondary,
                        options: button::Options {
                            size: button::Size::Small,
                            on_click: Some(Box::new(move |ctx, app, _| {
                                on_nav(NavigationDirection::Previous, ctx, app);
                            })),
                            ..button::Options::default(appearance)
                        },
                    },
                );
                content.add_positioned_child(
                    prev_button,
                    OffsetPositioning::offset_from_parent(
                        vec2f(12., 0.),
                        ParentOffsetBounds::Unbounded,
                        ParentAnchor::MiddleLeft,
                        ChildAnchor::MiddleLeft,
                    ),
                );
            }

            // Next button (hidden on last image).
            if current_index < image_count - 1 {
                let on_nav = on_navigate;
                let next_button = self.next_button.render(
                    appearance,
                    button::Params {
                        content: button::Content::Icon(Icon::ChevronRight),
                        theme: &button::themes::Secondary,
                        options: button::Options {
                            size: button::Size::Small,
                            on_click: Some(Box::new(move |ctx, app, _| {
                                on_nav(NavigationDirection::Next, ctx, app);
                            })),
                            ..button::Options::default(appearance)
                        },
                    },
                );
                content.add_positioned_child(
                    next_button,
                    OffsetPositioning::offset_from_parent(
                        vec2f(-12., 0.),
                        ParentOffsetBounds::Unbounded,
                        ParentAnchor::MiddleRight,
                        ChildAnchor::MiddleRight,
                    ),
                );
            }
        }

        content.finish()
    }
}

/// Builds the shared "Loading..." text element used in both the `Loading` state
/// and as the `before_load` fallback while the `AssetCache` fetches image bytes.
fn loading_element(appearance: &Appearance) -> Box<dyn Element> {
    Text::new(
        "Loading...",
        appearance.ui_font_family(),
        lightbox_text_size(appearance),
    )
    .with_color(ColorU::white())
    .finish()
}

fn lightbox_text_size(appearance: &Appearance) -> f32 {
    appearance.ui_font_size() + LIGHTBOX_TEXT_SIZE_DELTA
}
