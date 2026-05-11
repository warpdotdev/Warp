use std::sync::{Arc, Mutex};

use instant::Instant;
use pathfinder_geometry::vector::{Vector2F, vec2f};
use warp_core::ui::{Icon, appearance::Appearance};
use warpui::{
    AfterLayoutContext, AppContext, ClipBounds, Element, EventContext, LayoutContext,
    PaintContext, SizeConstraint,
    assets::asset_cache::AssetSource,
    elements::{CacheOption, Dismiss, Image, Point, Shrinkable},
    event::{DispatchedEvent, Event, ModifiersState},
    keymap::Keystroke,
    prelude::{stack::*, *},
};

use crate::{Component, Options as _, button};

/// Padding between the scrim edge and the image.
const SCRIM_PADDING: f32 = 48.;

/// GH9729 §698 / t2-15: horizontal slot width for each Size::Small
/// zoom icon button (Minus / Plus). Sized to give the two icons a
/// tight visual cluster (button is ~24 px square + a small gap).
///
/// t2-13 used a single 56-px slot for all three buttons including
/// the "100%" label; this failed for the rightmost slot because the
/// label button rendered wider than the slot and overlapped the `+`
/// button's hit area. t2-15 splits the slot constants: icon buttons
/// get their own narrow slot, the label button is positioned
/// separately with a gap so nothing can overlap it (or vice-versa).
const ZOOM_ICON_BUTTON_SLOT: f32 = 32.;

/// GH9729 §698 / t2-17: small visible gap between [−] and [+]
/// inside the icon cluster (user explicit feedback after t2-16's
/// zero-spacing layout was too tight).
const ZOOM_ICON_GAP: f32 = 6.;

/// GH9729 §698 / t2-17: horizontal gap between the icon cluster
/// ([−][+]) and the optional "100%" reset label. The gap visually
/// separates the persistent controls from the conditional reset
/// (reduced from t2-15's 16. after user feedback that the gap was
/// too wide).
const ZOOM_RESET_GAP_FROM_ICONS: f32 = 8.;

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

/// GH9729 §698 / t2-19: a viewport that lets its child render at any
/// size (potentially larger than the viewport itself), centers it, and
/// clips paint to viewport bounds. Also tracks drag-to-pan and routes
/// the user's drag into a caller-supplied `on_pan` callback.
///
/// This is the answer to the long-standing t2-7-r1 gotcha: the
/// framework's `ConstrainedBox::layout` tightens its child's max by
/// parent's max, so images can't grow past viewport via the normal
/// layout path. `PanClippedImage` short-circuits that by forcing its
/// child's layout constraint to `SizeConstraint::strict(desired_size)`
/// — the child renders at exactly the size we ask for, regardless of
/// our own bounds. Then we paint it inside a clipped layer at the
/// viewport rect, optionally offset by `pan_offset`.
///
/// Mouse-down + drag + mouse-up are tracked here so the lightbox view
/// owns the canonical `pan_offset` state and we get a single point of
/// truth for clamping. cmd+scroll-wheel is also captured here (it used
/// to live on the per-image `EventHandler`) so all viewport-scoped
/// gestures share the same hit-test bounds.
struct PanClippedImage {
    child: Box<dyn Element>,
    desired_size: Vector2F,
    pan_offset: Vector2F,
    on_pan: Option<Arc<dyn Fn(Vector2F, &mut EventContext, &AppContext)>>,
    on_zoom: Option<ZoomHandler>,
    on_double_tap_zoom:
        Option<Arc<dyn Fn(Vector2F, &mut EventContext, &AppContext)>>,

    origin: Option<Point>,
    viewport_size: Option<Vector2F>,
    /// GH9729 t2-20: drag state held in the persistent `Lightbox`
    /// struct via `Arc<Mutex<…>>` so it survives the `Pan` action's
    /// `ctx.notify()` re-render. A plain struct field on
    /// `PanClippedImage` would be reset every render and the user's
    /// drag would freeze after the first delta.
    drag_state: Arc<Mutex<Option<Vector2F>>>,
}

impl PanClippedImage {
    fn new(
        child: Box<dyn Element>,
        desired_size: Vector2F,
        pan_offset: Vector2F,
        on_pan: Option<Arc<dyn Fn(Vector2F, &mut EventContext, &AppContext)>>,
        on_zoom: Option<ZoomHandler>,
        on_double_tap_zoom: Option<
            Arc<dyn Fn(Vector2F, &mut EventContext, &AppContext)>,
        >,
        drag_state: Arc<Mutex<Option<Vector2F>>>,
    ) -> Self {
        Self {
            child,
            desired_size,
            pan_offset,
            on_pan,
            on_zoom,
            on_double_tap_zoom,
            origin: None,
            viewport_size: None,
            drag_state,
        }
    }

    /// Compute the maximum half-extent of pan in each axis: half of the
    /// overflow of child past viewport. Zero when child fits in viewport.
    fn max_pan(child_size: Vector2F, viewport: Vector2F) -> Vector2F {
        vec2f(
            ((child_size.x() - viewport.x()) / 2.0).max(0.0),
            ((child_size.y() - viewport.y()) / 2.0).max(0.0),
        )
    }

    /// Clamp `pan_offset` so the user can't drag the image so far that
    /// the visible edge moves past viewport center (which would reveal
    /// the scrim and feel sloppy).
    fn clamp_pan(pan: Vector2F, child_size: Vector2F, viewport: Vector2F) -> Vector2F {
        let max = Self::max_pan(child_size, viewport);
        vec2f(
            pan.x().clamp(-max.x(), max.x()),
            pan.y().clamp(-max.y(), max.y()),
        )
    }

    fn point_in_viewport(&self, position: Vector2F) -> bool {
        let Some(origin) = self.origin else {
            return false;
        };
        let Some(viewport) = self.viewport_size else {
            return false;
        };
        let origin_xy = origin.xy();
        position.x() >= origin_xy.x()
            && position.x() <= origin_xy.x() + viewport.x()
            && position.y() >= origin_xy.y()
            && position.y() <= origin_xy.y() + viewport.y()
    }
}

impl Element for PanClippedImage {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        app: &AppContext,
    ) -> Vector2F {
        // Fill parent's max (the lightbox content area). This becomes
        // the viewport — clip bounds and pan-clamp boundary.
        let viewport = constraint.max;
        self.viewport_size = Some(viewport);

        // Force child to render at exactly `desired_size`, bypassing
        // parent-max binding. This is the load-bearing line — without
        // it, t2-7-r1 returns.
        self.child
            .layout(SizeConstraint::strict(self.desired_size), ctx, app);

        viewport
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        self.child.after_layout(ctx, app);
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        let origin_point = Point::from_vec2f(origin, ctx.scene.z_index());
        self.origin = Some(origin_point);

        let viewport = self
            .viewport_size
            .expect("layout must run before paint");
        let child_size = self.child.size().unwrap_or(self.desired_size);

        // Center child within viewport, apply clamped pan offset.
        let pan = Self::clamp_pan(self.pan_offset, child_size, viewport);
        let child_origin = vec2f(
            origin.x() + (viewport.x() - child_size.x()) / 2.0 + pan.x(),
            origin.y() + (viewport.y() - child_size.y()) / 2.0 + pan.y(),
        );

        // Paint inside a clipped layer at viewport bounds so the
        // oversized child doesn't bleed past the lightbox scrim.
        if let Some(visible) = ctx.scene.visible_rect(origin_point, viewport) {
            ctx.scene.start_layer(ClipBounds::BoundedBy(visible));
            self.child.paint(child_origin, ctx, app);
            ctx.scene.stop_layer();
        }
    }

    fn dispatch_event(
        &mut self,
        event: &DispatchedEvent,
        ctx: &mut EventContext,
        app: &AppContext,
    ) -> bool {
        match event.raw_event() {
            Event::LeftMouseDown {
                click_count,
                position,
                ..
            } if self.point_in_viewport(*position) => {
                // GH9729 t2-21: double-click = zoom-and-center on tap.
                // Skip drag-tracking so the second-click's drag delta
                // doesn't pan the image during the gesture.
                if *click_count >= 2 {
                    if let (Some(origin), Some(viewport)) =
                        (self.origin, self.viewport_size)
                    {
                        let viewport_center = origin.xy() + viewport * 0.5;
                        let tap_offset = *position - viewport_center;
                        if let Some(cb) = self.on_double_tap_zoom.as_ref() {
                            cb(tap_offset, ctx, app);
                        }
                    }
                    if let Ok(mut state) = self.drag_state.lock() {
                        *state = None;
                    }
                    return true;
                }
                // Single click: start tracking drag in shared state;
                // consume so the scrim's Dismiss doesn't fire on the
                // same down event.
                if let Ok(mut state) = self.drag_state.lock() {
                    *state = Some(*position);
                }
                true
            }
            Event::LeftMouseDragged { position, .. } => {
                let last = self
                    .drag_state
                    .lock()
                    .ok()
                    .and_then(|state| *state);
                if let Some(last) = last {
                    let delta = *position - last;
                    if let Some(on_pan) = self.on_pan.as_ref() {
                        on_pan(self.pan_offset + delta, ctx, app);
                    }
                    if let Ok(mut state) = self.drag_state.lock() {
                        *state = Some(*position);
                    }
                    return true;
                }
                false
            }
            Event::LeftMouseUp { .. } => {
                let was_dragging = self
                    .drag_state
                    .lock()
                    .ok()
                    .map(|state| state.is_some())
                    .unwrap_or(false);
                if was_dragging {
                    if let Ok(mut state) = self.drag_state.lock() {
                        *state = None;
                    }
                    return true;
                }
                false
            }
            Event::ScrollWheel {
                position,
                delta,
                modifiers,
                ..
            } if self.point_in_viewport(*position) => {
                // cmd+scroll = zoom (preserved from t2-12). Plain
                // scroll falls through so parent surfaces can use it.
                let cmd_or_ctrl = modifiers_have_cmd_or_ctrl(modifiers);
                if !cmd_or_ctrl {
                    return false;
                }
                let dy = delta.y();
                if dy.abs() < SCROLL_ZOOM_DEAD_ZONE {
                    return true;
                }
                if let Some(on_zoom) = self.on_zoom.as_ref() {
                    let direction = if dy > 0.0 {
                        ZoomDirection::In
                    } else {
                        ZoomDirection::Out
                    };
                    on_zoom(direction, ctx, app);
                }
                true
            }
            _ => false,
        }
    }

    fn size(&self) -> Option<Vector2F> {
        self.viewport_size
    }

    fn origin(&self) -> Option<Point> {
        self.origin
    }
}

fn modifiers_have_cmd_or_ctrl(modifiers: &ModifiersState) -> bool {
    modifiers.cmd || modifiers.ctrl
}

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

/// GH9729 §698 / t2-21: multiplicative step applied per zoom-in /
/// zoom-out button click. `1.25` matches the macOS Preview /
/// Safari / Chrome cmd-+ convention. At this step,
/// `MAX_ZOOM_FACTOR = 8.0` is reached in ~10 `+` clicks and
/// `MIN_ZOOM_FACTOR = 0.25` in ~7 `-` clicks from the `1.0` default.
pub const ZOOM_STEP: f32 = 1.25;

/// GH9729 §698 / t2-21: target zoom for the double-tap-to-zoom
/// gesture. macOS Preview and iOS Photos use the same convention —
/// double-tap zooms to a fixed multiple of native size, second
/// double-tap returns to native. `2.0` is the established target.
pub const DOUBLE_TAP_TARGET_ZOOM: f32 = 2.0;

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
    /// GH9729 §698 / t2-20: shared drag-tracking state for
    /// `PanClippedImage`. Lives on the persistent `Lightbox` struct so
    /// it survives re-renders (the rendered tree is rebuilt every
    /// `render()`, including the `PanClippedImage`; transient state on
    /// the element itself would be lost on every `ctx.notify()`).
    /// Holds the cursor position from the most recent unhandled
    /// `LeftMouseDown` / `LeftMouseDragged`; `None` when not currently
    /// dragging.
    drag_state: Arc<Mutex<Option<Vector2F>>>,
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

    /// GH9729 §698 / t2-19: drag-to-pan offset (pixels) for the
    /// currently-displayed image. `Vector2F::zero()` for centered.
    /// Caller is responsible for resetting on zoom-to-native and on
    /// navigation. The lightbox component clamps the offset internally
    /// so the user can't drag the image past viewport center.
    pub pan_offset: Vector2F,

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

    /// GH9729 §698 / t2-19: handler invoked when the user drags the
    /// image to pan. Called with the new desired pan offset (before
    /// clamping — caller can clamp or ignore as needed). If `None`,
    /// drag-to-pan is disabled and mouse-down on the image just stops
    /// scrim-dismiss as before.
    pub on_pan: Option<Arc<dyn Fn(Vector2F, &mut EventContext, &AppContext)>>,

    /// GH9729 §698 / t2-21: handler invoked when the user double-taps
    /// inside the image. Called with the tap position **relative to
    /// viewport center** (so the caller can compute a re-centering
    /// pan_offset against any zoom factor). If `None`, double-tap
    /// is ignored and the second click just resumes drag tracking.
    pub on_double_tap_zoom:
        Option<Arc<dyn Fn(Vector2F, &mut EventContext, &AppContext)>>,
}

impl crate::Options for Options {
    fn default(_appearance: &Appearance) -> Self {
        Self {
            dismiss_keystroke: None,
            on_navigate: None,
            on_zoom: None,
            on_pan: None,
            on_double_tap_zoom: None,
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
                    // the caller supplied a timeline anchor.
                    let mut image_builder =
                        Image::new(asset_source.clone(), CacheOption::Original).contain();
                    if let Some(start) = params.animation_start_time {
                        image_builder = image_builder.enable_animation_with_start_time(start);
                    }
                    // GH9729 §698 / t2-19: feed the desired (zoom*native)
                    // size into PanClippedImage, which forces the child
                    // to render at exactly that size regardless of
                    // parent's available area. Clamp first so a poisoned
                    // float can't blow up the layout.
                    let zoom = params.zoom_factor.clamp(MIN_ZOOM_FACTOR, MAX_ZOOM_FACTOR);
                    let desired_size = vec2f(native_size.x() * zoom, native_size.y() * zoom);

                    let image_element = image_builder
                        .before_load(Align::new(loading_element(appearance)).finish())
                        .finish();

                    // PanClippedImage handles its own mouse-down (stops
                    // scrim dismiss), drag (pan), and cmd+scroll (zoom),
                    // so the previous `EventHandler` wrapping is no
                    // longer needed.
                    Box::new(PanClippedImage::new(
                        image_element,
                        desired_size,
                        params.pan_offset,
                        params.options.on_pan.clone(),
                        params.options.on_zoom.clone(),
                        params.options.on_double_tap_zoom.clone(),
                        self.drag_state.clone(),
                    ))
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

        // GH9729 §698 / t2-16: zoom toolbar layout. Wrap zoom-out +
        // zoom-in in `Flex::row` with zero spacing so the row
        // partitions its bounding rect precisely between children —
        // no possibility of hit-area overlap. The whole row is added
        // as ONE positioned child.
        //
        // Theory for the persistent `+` bug across t2-12/13/15:
        // separate `add_positioned_child` siblings each report their
        // own bbox to the Stack including the button's interactive
        // padding (hover hit-area beyond visual edge). Stack
        // dispatches first-added-first, so `−`'s extended hit-area
        // claimed clicks intended for `+`. The user's "gap" complaint
        // was the visible evidence of this padding extending into the
        // gap. Flex::row partitions the row's bbox precisely between
        // its children, eliminating the overlap regardless of button
        // padding.
        //
        // Diagnostic logging is included; if `+` STILL fails after
        // this commit, the log will show whether the click is
        // arriving at the closure at all (and which closure).
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
            let on_zoom_in = on_zoom.clone();
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

            // Small spacing between − and + so they have a visible gap
            // (user explicit feedback after t2-16's zero-spacing
            // layout). Still inside a Flex::row so Flex partitions
            // hit-test space precisely between cells.
            let icon_cluster = Flex::row()
                .with_spacing(ZOOM_ICON_GAP)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(zoom_out_button)
                .with_child(zoom_in_button)
                .finish();
            content.add_positioned_child(
                icon_cluster,
                OffsetPositioning::offset_from_parent(
                    vec2f(SCRIM_BUTTON_INSET, -SCRIM_BUTTON_INSET),
                    ParentOffsetBounds::Unbounded,
                    ParentAnchor::BottomLeft,
                    ChildAnchor::BottomLeft,
                ),
            );

            // Reset only renders when the image is NOT at native zoom.
            // Positioned as its own separate positioned child to the
            // right of the icon cluster, with a visible gap. Since the
            // icon cluster width depends on button rendering and we
            // can't measure it from here, the reset's offset is
            // estimated from `ZOOM_ICON_BUTTON_SLOT * 2 +
            // ZOOM_RESET_GAP_FROM_ICONS`.
            if zoom != 1.0 {
                let on_zoom_reset = on_zoom;
                let zoom_reset_button = self.zoom_reset_button.render(
                    appearance,
                    button::Params {
                        content: button::Content::Label("100%".into()),
                        theme: &button::themes::Secondary,
                        options: button::Options {
                            size: button::Size::Small,
                            on_click: Some(Box::new(move |ctx, app, _| {
                                on_zoom_reset(ZoomDirection::Reset, ctx, app);
                            })),
                            ..button::Options::default(appearance)
                        },
                    },
                );
                content.add_positioned_child(
                    zoom_reset_button,
                    OffsetPositioning::offset_from_parent(
                        vec2f(
                            SCRIM_BUTTON_INSET
                                + 2. * ZOOM_ICON_BUTTON_SLOT
                                + ZOOM_RESET_GAP_FROM_ICONS,
                            -SCRIM_BUTTON_INSET,
                        ),
                        ParentOffsetBounds::Unbounded,
                        ParentAnchor::BottomLeft,
                        ChildAnchor::BottomLeft,
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
