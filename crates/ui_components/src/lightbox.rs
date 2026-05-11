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

/// GH9729 §698 / t2-13: horizontal slot width for each zoom toolbar
/// button, including the gap between buttons. The buttons are
/// positioned individually (one `add_positioned_child` call each)
/// rather than via a shared `Flex::row` wrapper because the t2-12
/// Flex layout caused the rightmost button's click to not fire —
/// suspected hit-test routing bug when multiple children share one
/// positioned parent. Width is an approximation calibrated for
/// `Button::Size::Small` with a `Label("100%")` middle slot; needs
/// re-tuning if a future icon set changes the rendered button width.
const ZOOM_BUTTON_SLOT_WIDTH: f32 = 56.;

/// GH9729 §698 / t2-13: inset for buttons anchored to a scrim corner
/// (close button top-right, zoom toolbar bottom-left). One source of
/// truth so the corners stay symmetric.
const SCRIM_BUTTON_INSET: f32 = 12.;

/// GH9729 §698 / t2-12: scroll-delta magnitude below which a
/// cmd+scroll event is treated as a no-op. Stops trackpad jitter at
/// rest from triggering rapid-fire zoom steps. Empirically: macOS
/// continuous-touch scroll events report values well below 1.0 at
/// rest and above 1.0 during deliberate gestures.
const SCROLL_ZOOM_DEAD_ZONE: f32 = 1.0;

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

/// GH9729 t2-14: nearly-opaque black background colour for the scrim.
///
/// v1 shipped with alpha=230 (90% opacity), which let bright text in
/// the underlying view (new-tab page, terminal output) show through
/// and competed with the lightbox's description / dimensions text for
/// legibility. 250 (~98% opacity) suppresses bleed-through while
/// keeping a faint hint of the underlying surface for spatial
/// context. Set to 255 if a fully opaque modal is preferred.
fn scrim_color() -> ColorU {
    ColorU::new(0, 0, 0, 250)
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
    /// GH9729 §698 / t2-12: zoom-out button rendered in the lightbox
    /// toolbar. Replaces the t2-11 keyboard bindings which didn't
    /// dispatch in a Warp terminal context.
    zoom_out_button: button::Button,
    /// GH9729 §698 / t2-12: zoom-in button.
    zoom_in_button: button::Button,
    /// GH9729 §698 / t2-12: zoom-reset (100%) button.
    zoom_reset_button: button::Button,
}

/// GH9729 §698 / t2-12: direction for a single zoom step in the
/// lightbox. Mirrors `NavigationDirection` in shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomDirection {
    In,
    Out,
    Reset,
}

/// GH9729 §698 / t2-12: handler invoked when the user requests a zoom
/// step via a toolbar button or scroll-wheel gesture.
pub type ZoomHandler = Arc<dyn Fn(ZoomDirection, &mut EventContext, &AppContext)>;

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

    /// GH9729 §698 / t2-12: handler invoked when the user clicks a
    /// zoom button or scrolls (with cmd held) over the image. If
    /// `None`, the zoom toolbar is not rendered and scroll-wheel
    /// events fall through.
    pub on_zoom: Option<ZoomHandler>,
}

impl crate::Options for Options {
    fn default(_appearance: &Appearance) -> Self {
        Self {
            dismiss_keystroke: None,
            on_navigate: None,
            on_zoom: None,
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

                    // GH9729 §698 / t2-12: cmd+scroll-wheel zooms the
                    // image when the caller supplied an `on_zoom`
                    // handler. Plain scroll (no modifier) is left
                    // un-consumed so trackpad-induced flicks don't
                    // surprise the user — matches macOS Preview
                    // convention. Reset isn't reachable via scroll;
                    // that's a button-only action.
                    let scroll_zoom = params.options.on_zoom.clone();
                    let mut handler = EventHandler::new(image)
                        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation);
                    if let Some(on_zoom) = scroll_zoom {
                        handler = handler.on_scroll_wheel(move |ctx, app, delta, modifiers| {
                            if !modifiers.cmd && !modifiers.ctrl {
                                return DispatchEventResult::PropagateToParent;
                            }
                            // Vertical scroll convention: positive y is
                            // "wheel up / push fingers up" → zoom in.
                            // Treat tiny deltas as no-ops so resting
                            // on a near-zero trackpad doesn't drift.
                            let dy = delta.y();
                            if dy.abs() < SCROLL_ZOOM_DEAD_ZONE {
                                return DispatchEventResult::StopPropagation;
                            }
                            let direction = if dy > 0.0 {
                                ZoomDirection::In
                            } else {
                                ZoomDirection::Out
                            };
                            on_zoom(direction, ctx, app);
                            DispatchEventResult::StopPropagation
                        });
                    }
                    handler.finish()
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
                vec2f(-SCRIM_BUTTON_INSET, SCRIM_BUTTON_INSET),
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

        // GH9729 §698 / t2-13: zoom toolbar — three buttons in the
        // bottom-left corner, positioned individually rather than via a
        // shared `Flex::row` wrapper (t2-12 used a Flex wrapper inside
        // `add_positioned_child`; manual test surfaced that the
        // rightmost button's click never fired, suspected hit-test
        // routing bug when multiple children share one positioned
        // parent). Mirrors the existing prev/next button placement —
        // each button is its own positioned child.
        //
        // Reset uses a text "100%" label rather than the silly
        // `Icon::Refresh` glyph from t2-12, and is disabled when the
        // image is already at native size (`zoom_factor == 1.0`) so
        // the user gets a clear visual signal that there's nothing to
        // reset.
        if let Some(on_zoom) = params.options.on_zoom {
            let zoom = params.zoom_factor;
            let on_zoom_out = on_zoom.clone();
            let zoom_out_button = self.zoom_out_button.render(
                appearance,
                button::Params {
                    content: button::Content::Icon(Icon::Minus),
                    theme: &button::themes::Secondary,
                    options: button::Options {
                        size: button::Size::Small,
                        on_click: Some(Box::new(move |ctx, app, _| {
                            on_zoom_out(ZoomDirection::Out, ctx, app);
                        })),
                        ..button::Options::default(appearance)
                    },
                },
            );
            let on_zoom_reset = on_zoom.clone();
            let zoom_reset_button = self.zoom_reset_button.render(
                appearance,
                button::Params {
                    content: button::Content::Label("100%".into()),
                    theme: &button::themes::Secondary,
                    options: button::Options {
                        size: button::Size::Small,
                        disabled: zoom == 1.0,
                        on_click: Some(Box::new(move |ctx, app, _| {
                            on_zoom_reset(ZoomDirection::Reset, ctx, app);
                        })),
                        ..button::Options::default(appearance)
                    },
                },
            );
            let on_zoom_in = on_zoom;
            let zoom_in_button = self.zoom_in_button.render(
                appearance,
                button::Params {
                    content: button::Content::Icon(Icon::Plus),
                    theme: &button::themes::Secondary,
                    options: button::Options {
                        size: button::Size::Small,
                        on_click: Some(Box::new(move |ctx, app, _| {
                            on_zoom_in(ZoomDirection::In, ctx, app);
                        })),
                        ..button::Options::default(appearance)
                    },
                },
            );

            // Each button sits independently at the bottom-left, offset
            // rightward by an accumulating step so they appear in a
            // visible row without depending on Flex layout. The step
            // size (`ZOOM_BUTTON_SLOT_WIDTH`) is an approximation that
            // accommodates Button::Size::Small icon + label widths; on
            // a future tighter layout pass it could be replaced with a
            // hit-test-correct flex wrapper.
            content.add_positioned_child(
                zoom_out_button,
                OffsetPositioning::offset_from_parent(
                    vec2f(SCRIM_BUTTON_INSET, -SCRIM_BUTTON_INSET),
                    ParentOffsetBounds::Unbounded,
                    ParentAnchor::BottomLeft,
                    ChildAnchor::BottomLeft,
                ),
            );
            content.add_positioned_child(
                zoom_reset_button,
                OffsetPositioning::offset_from_parent(
                    vec2f(SCRIM_BUTTON_INSET + ZOOM_BUTTON_SLOT_WIDTH, -SCRIM_BUTTON_INSET),
                    ParentOffsetBounds::Unbounded,
                    ParentAnchor::BottomLeft,
                    ChildAnchor::BottomLeft,
                ),
            );
            content.add_positioned_child(
                zoom_in_button,
                OffsetPositioning::offset_from_parent(
                    vec2f(
                        SCRIM_BUTTON_INSET + 2. * ZOOM_BUTTON_SLOT_WIDTH,
                        -SCRIM_BUTTON_INSET,
                    ),
                    ParentOffsetBounds::Unbounded,
                    ParentAnchor::BottomLeft,
                    ChildAnchor::BottomLeft,
                ),
            );
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
