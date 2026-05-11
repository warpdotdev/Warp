use std::sync::Arc;

use instant::Instant;
use pathfinder_geometry::vector::Vector2F;
use ui_components::{lightbox, Component as _};
use warpui::assets::asset_cache::{AssetCache, AssetSource, AssetState};
use warpui::image_cache::ImageType;
use warpui::keymap::{FixedBinding, Keystroke};
use warpui::prelude::*;
use warpui::{AppContext, BlurContext, Element, Entity, SingletonEntity, View, ViewContext};

use crate::appearance::Appearance;

pub use lightbox::LightboxImage;

pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;
    let view_id = id!(LightboxView::ui_name());
    app.register_fixed_bindings([
        FixedBinding::new("escape", LightboxViewAction::Dismiss, view_id.clone()),
        FixedBinding::new(
            "left",
            LightboxViewAction::NavigatePrevious,
            view_id.clone(),
        ),
        FixedBinding::new("right", LightboxViewAction::NavigateNext, view_id.clone()),
        // GH9729 §698 / t2-11: zoom keybindings.
        //
        // IMPORTANT: must use modifier-prefixed keys. Unmodified character
        // keys (bare "=" / "-" / "0") route to the terminal stdin layer
        // before reaching the lightbox view's action dispatch, so they
        // never fire. Only special keys (escape/left/right above) and
        // modifier-prefixed keys reach view-scoped FixedBindings.
        //
        // `cmdorctrl-=` shadows the workspace-level font-zoom binding from
        // `app/src/util/bindings.rs:296` while the LightboxView is focused
        // — the more-specific view scope wins. When the lightbox dismisses,
        // the binding lapses and cmd-+/-/0 resumes zooming the terminal
        // font.
        FixedBinding::new("cmdorctrl-=", LightboxViewAction::ZoomIn, view_id.clone()),
        FixedBinding::new("cmdorctrl--", LightboxViewAction::ZoomOut, view_id.clone()),
        FixedBinding::new("cmdorctrl-0", LightboxViewAction::ZoomReset, view_id),
    ]);
}

/// Parameters needed to open a lightbox.
#[derive(Clone, Debug)]
pub struct LightboxParams {
    /// The images to display in the lightbox.
    pub images: Vec<LightboxImage>,
    /// The index of the image to display initially.
    pub initial_index: usize,
}

/// Events emitted by the `LightboxView` to its parent.
pub enum LightboxViewEvent {
    /// The user explicitly dismissed the lightbox (Escape, close button, or scrim click).
    Close,
    /// Focus left the lightbox subtree (e.g. the user switched tabs).
    FocusLost,
}

impl Entity for LightboxView {
    type Event = LightboxViewEvent;
}

/// Actions dispatched within the `LightboxView`.
#[derive(Debug)]
pub enum LightboxViewAction {
    /// Dismiss the lightbox (triggered by clicking outside, close button, or Escape).
    Dismiss,
    /// Navigate to the previous image.
    NavigatePrevious,
    /// Navigate to the next image.
    NavigateNext,
    /// GH9729 §698: zoom the current image in by one step.
    ZoomIn,
    /// GH9729 §698: zoom the current image out by one step.
    ZoomOut,
    /// GH9729 §698: reset the current image to native size (`zoom_factor = 1.0`).
    ZoomReset,
}

/// A view that renders a full-window lightbox overlay.
pub struct LightboxView {
    params: LightboxParams,
    current_index: usize,
    lightbox: lightbox::Lightbox,
    /// GH9729 §697: timeline anchor for the currently-displayed image's
    /// animated GIF/WebP playback. Reset to `Instant::now()` whenever
    /// the displayed image changes (construction, params replacement,
    /// arrow-key navigation) so each entry's loop starts from frame 0.
    /// Static images ignore this; the Image element only consults
    /// `started_at` when there's an animated payload.
    animation_start_time: Instant,
    /// GH9729 §698: current zoom factor applied to the displayed image.
    /// `1.0` is native size; `>1` zooms in, `<1` shrinks. Always within
    /// `[lightbox::MIN_ZOOM_FACTOR, lightbox::MAX_ZOOM_FACTOR]` after
    /// any mutation. Reset to `1.0` whenever the displayed image
    /// changes (construction, params replacement, arrow-key navigation)
    /// so a freshly-shown image is never inherited at an unexpected
    /// zoom level.
    zoom_factor: f32,
}

impl LightboxView {
    pub fn new(params: LightboxParams, ctx: &mut ViewContext<Self>) -> Self {
        let initial_index = params
            .initial_index
            .min(params.images.len().saturating_sub(1));
        let mut view = Self {
            params,
            current_index: initial_index,
            lightbox: lightbox::Lightbox::default(),
            animation_start_time: Instant::now(),
            zoom_factor: 1.0,
        };
        view.start_asset_loads(ctx);
        view
    }

    /// Replace the images and navigate to the given initial index.
    pub fn update_params(&mut self, params: LightboxParams, ctx: &mut ViewContext<Self>) {
        let initial_index = params
            .initial_index
            .min(params.images.len().saturating_sub(1));
        self.params = params;
        self.current_index = initial_index;
        self.reset_per_image_state();
        self.start_asset_loads(ctx);
    }

    /// GH9729 §697 + §698: reset the per-image transient state (animation
    /// timeline anchor and zoom factor) so the next render starts the
    /// image from frame 0 at native size. Called from every site that
    /// changes which image is currently displayed.
    fn reset_per_image_state(&mut self) {
        self.animation_start_time = Instant::now();
        self.zoom_factor = 1.0;
    }

    /// Update a single image at the given index without replacing the full list.
    pub fn update_image_at(
        &mut self,
        index: usize,
        image: LightboxImage,
        ctx: &mut ViewContext<Self>,
    ) {
        if index >= self.params.images.len() {
            return;
        }
        let asset_source = match &image.source {
            lightbox::LightboxImageSource::Resolved { asset_source } => Some(asset_source.clone()),
            _ => None,
        };
        self.params.images[index] = image;
        if let Some(asset_source) = asset_source {
            self.start_asset_load(index, asset_source, ctx);
        }
    }

    /// Kick off asset loads for all `Resolved` images and schedule re-renders.
    fn start_asset_loads(&mut self, ctx: &mut ViewContext<Self>) {
        // Collect first (immutable borrow) so the per-entry call below can
        // take `&mut self`.
        let to_load: Vec<(usize, AssetSource)> = self
            .params
            .images
            .iter()
            .enumerate()
            .filter_map(|(i, img)| match &img.source {
                lightbox::LightboxImageSource::Resolved { asset_source } => {
                    Some((i, asset_source.clone()))
                }
                _ => None,
            })
            .collect();
        for (index, asset_source) in to_load {
            self.start_asset_load(index, asset_source, ctx);
        }
    }

    /// Eagerly load a single asset and schedule a `ctx.notify()` when the
    /// fetch completes so the lightbox re-renders with the loaded image.
    ///
    /// `rewrite_image_for_load_state` is applied against the asset cache's
    /// returned state on both the synchronous and asynchronous code paths:
    ///
    /// * **Synchronous path** (GH9729 §695 + t2-10): a tiny mislabeled file
    ///   like a `.png` containing tarball bytes is so small the asset
    ///   cache can deliver `FailedToLoad` inline on the first
    ///   `load_asset` call — the §695 `Err("could not detect image
    ///   format")` from `ImageType::try_from_bytes` is reached before
    ///   any executor turn. The `when_loaded` future never fires for an
    ///   already-failed entry, so without the inline rewrite the
    ///   lightbox would render a permanent spinner. Apply the rewrite
    ///   immediately and skip the spawn.
    ///
    /// * **Asynchronous path**: the load is still pending; spawn a
    ///   callback that re-queries the cache once the future completes
    ///   and applies the same rewrite. The callback also calls
    ///   `ctx.notify()` so the lightbox repaints with the loaded image
    ///   (or the rewritten Error variant).
    ///
    /// See specs/GH9729/tech.md §182 and §695.
    fn start_asset_load(
        &mut self,
        index: usize,
        asset_source: AssetSource,
        ctx: &mut ViewContext<Self>,
    ) {
        let asset_cache = AssetCache::as_ref(ctx);
        let state = asset_cache.load_asset::<ImageType>(asset_source.clone());

        // GH9729 §695 / t2-10: apply the rewrite inline for any state
        // the asset cache resolves synchronously. `rewrite_image_for_load_state`
        // returns `None` for `Loading` and (post-§695) `Loaded`, so this
        // is a no-op for the common path — it only fires for
        // synchronously-`FailedToLoad`, which is the case the
        // `Loading`-only spawn below cannot rescue.
        if apply_rewrite_to_slot(&mut self.params.images, index, &state) {
            return;
        }

        if let AssetState::Loading { handle } = state {
            if let Some(future) = handle.when_loaded(asset_cache) {
                ctx.spawn(future, move |me, (), ctx| {
                    ctx.notify();
                    let asset_cache = AssetCache::as_ref(ctx);
                    let state = asset_cache.load_asset::<ImageType>(asset_source.clone());
                    apply_rewrite_to_slot(&mut me.params.images, index, &state);
                });
            }
        }
    }
}

/// GH9729 §699 + t2-11: format the lightbox status-footer string for an
/// image with the given intrinsic size and current zoom factor.
///
/// At zoom 1.0 returns `"<W> × <H> px"`. At any other (clamped) zoom
/// returns `"<W> × <H> px · <Z>%"` so the user sees zoom-level feedback
/// even when the visible image size is capped by the parent constraint
/// (the t2-7-r1 gotcha for large images).
///
/// Non-finite `zoom_factor` collapses to 100% so a NaN-poisoned input
/// doesn't surface in the user-visible string; the view's `step_zoom`
/// already guards against this at the input edge but the renderer-side
/// public `Params::zoom_factor` is technically NaN-poisonable from
/// external callers, so defend here too.
fn format_metadata_line(size: pathfinder_geometry::vector::Vector2F, zoom_factor: f32) -> String {
    let w = size.x().round() as i32;
    let h = size.y().round() as i32;
    let zoom_pct = if zoom_factor.is_finite() {
        (zoom_factor * 100.0).round() as i32
    } else {
        100
    };
    if zoom_pct == 100 {
        format!("{w} × {h} px")
    } else {
        format!("{w} × {h} px · {zoom_pct}%")
    }
}

/// GH9729 §695 / t2-10: apply `rewrite_image_for_load_state` to the
/// image at `index`, replacing its `source` if a rewrite is warranted.
/// Returns `true` when the slot was mutated, `false` otherwise. Out-of-
/// bounds indices are tolerated (return `false`) so callers don't have
/// to pre-check the slice length.
fn apply_rewrite_to_slot(
    images: &mut [LightboxImage],
    index: usize,
    state: &AssetState<ImageType>,
) -> bool {
    let Some(new_source) = rewrite_image_for_load_state(state) else {
        return false;
    };
    let Some(slot) = images.get_mut(index) else {
        return false;
    };
    slot.source = new_source;
    true
}

/// GH9729 §698: direction for a single zoom-step keystroke.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ZoomDirection {
    In,
    Out,
}

/// GH9729 §698: compute the next zoom factor for a `ZoomIn` or `ZoomOut`
/// keystroke. Multiplies (or divides) `current` by `lightbox::ZOOM_STEP`
/// and clamps the result to
/// `[lightbox::MIN_ZOOM_FACTOR, lightbox::MAX_ZOOM_FACTOR]`. A non-finite
/// input collapses to `1.0` (the safe default) so a poisoned float
/// cannot escape into the `ConstrainedBox` size.
fn step_zoom(current: f32, direction: ZoomDirection) -> f32 {
    if !current.is_finite() {
        return 1.0;
    }
    let raw = match direction {
        ZoomDirection::In => current * lightbox::ZOOM_STEP,
        ZoomDirection::Out => current / lightbox::ZOOM_STEP,
    };
    raw.clamp(lightbox::MIN_ZOOM_FACTOR, lightbox::MAX_ZOOM_FACTOR)
}

/// Sanitize a per-asset-cache load error into a small set of categorical
/// strings that never interpolate raw OS errors or filesystem paths. The
/// underlying error is logged via `log::warn!` for the operator.
/// See specs/GH9729/tech.md §182 and §695.
fn sanitize_load_error(err: &anyhow::Error) -> &'static str {
    let s = format!("{err}").to_lowercase();
    if s.contains("too large") || s.contains("exceeds") {
        "image is too large to preview"
    } else if s.contains("could not detect") {
        // GH9729 §695: `ImageType::try_from_bytes` returns this exact
        // string for unrecognized-format bytes (e.g. a `.png` containing
        // tarball bytes). Preserve the user-visible "detect" wording
        // rather than collapsing it into the generic "decode" bucket.
        "could not detect image format"
    } else if s.contains("decode") || s.contains("format") {
        "could not decode image"
    } else {
        "could not read image"
    }
}

/// Inspect an `AssetState` for an image entry and decide whether the
/// `LightboxImage::source` should be rewritten to the `Error` variant.
/// Returns `Some(new_source)` if a rewrite is warranted, or `None` to
/// leave the entry unchanged.
///
/// Per `tech.md` §182 + §695, only `FailedToLoad` triggers a rewrite:
///   * After the §695 refactor, every `Loaded { data }` carries a
///     successfully decoded image (`Svg` / `StaticBitmap` /
///     `AnimatedBitmap`). Mislabeled-or-unsupported bytes now surface as
///     `try_from_bytes` returning `Err`, which the asset cache stores as
///     `FailedToLoad` — handled here by `sanitize_load_error`.
fn rewrite_image_for_load_state(
    state: &AssetState<ImageType>,
) -> Option<lightbox::LightboxImageSource> {
    match state {
        AssetState::FailedToLoad(err) => {
            log::warn!("GH9729: image preview load failed: {}", err);
            Some(lightbox::LightboxImageSource::Error {
                message: sanitize_load_error(err).to_string(),
            })
        }
        _ => None,
    }
}

impl View for LightboxView {
    fn ui_name() -> &'static str {
        "LightboxView"
    }

    fn on_blur(&mut self, _blur_ctx: &BlurContext, ctx: &mut ViewContext<Self>) {
        // Only dismiss if focus has left the entire lightbox subtree.
        if !ctx.is_self_or_child_focused() {
            ctx.emit(LightboxViewEvent::FocusLost);
        }
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);

        // Determine the native pixel size of the current image by querying the
        // asset cache. This will be `Some` once the image bytes have been fully
        // loaded and decoded.
        let current_image_native_size =
            self.params
                .images
                .get(self.current_index)
                .and_then(|img| match &img.source {
                    lightbox::LightboxImageSource::Resolved { asset_source } => {
                        let asset_cache = AssetCache::as_ref(app);
                        match asset_cache.load_asset::<ImageType>(asset_source.clone()) {
                            AssetState::Loaded { data } => data
                                .image_size()
                                .map(|size| Vector2F::new(size.x() as f32, size.y() as f32)),
                            _ => None,
                        }
                    }
                    lightbox::LightboxImageSource::Loading => None,
                    // Error entries have no native size; the existing render
                    // logic tolerates `None` here. See specs/GH9729/tech.md §182.
                    lightbox::LightboxImageSource::Error { .. } => None,
                });

        self.lightbox.render(
            appearance,
            lightbox::Params {
                images: &self.params.images,
                current_index: self.current_index,
                on_dismiss: Arc::new(|ctx, _| {
                    ctx.dispatch_typed_action(LightboxViewAction::Dismiss);
                }),
                current_image_native_size,
                animation_start_time: Some(self.animation_start_time),
                zoom_factor: self.zoom_factor,
                // GH9729 §699 + t2-11: surface the loaded image's intrinsic
                // dimensions plus the current zoom percentage when zoom is
                // not 1.0 (so the user gets visual feedback even when the
                // image is window-capped and a zoom keystroke produces no
                // visible size change — the t2-7-r1 gotcha). Format string
                // and file size are deferred — see `TIER2_TODO::t2-8-r2`.
                metadata_line: current_image_native_size
                    .map(|size| format_metadata_line(size, self.zoom_factor)),
                options: lightbox::Options {
                    dismiss_keystroke: Keystroke::parse("escape").ok(),
                    on_navigate: Some(Arc::new(|direction, ctx, _| match direction {
                        lightbox::NavigationDirection::Previous => {
                            ctx.dispatch_typed_action(LightboxViewAction::NavigatePrevious);
                        }
                        lightbox::NavigationDirection::Next => {
                            ctx.dispatch_typed_action(LightboxViewAction::NavigateNext);
                        }
                    })),
                },
            },
        )
    }
}

impl TypedActionView for LightboxView {
    type Action = LightboxViewAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            LightboxViewAction::Dismiss => {
                ctx.emit(LightboxViewEvent::Close);
            }
            LightboxViewAction::NavigatePrevious => {
                if self.current_index > 0 {
                    self.current_index -= 1;
                    self.reset_per_image_state();
                    ctx.notify();
                }
            }
            LightboxViewAction::NavigateNext => {
                if self.current_index + 1 < self.params.images.len() {
                    self.current_index += 1;
                    self.reset_per_image_state();
                    ctx.notify();
                }
            }
            LightboxViewAction::ZoomIn => {
                let next = step_zoom(self.zoom_factor, ZoomDirection::In);
                if next != self.zoom_factor {
                    self.zoom_factor = next;
                    ctx.notify();
                }
            }
            LightboxViewAction::ZoomOut => {
                let next = step_zoom(self.zoom_factor, ZoomDirection::Out);
                if next != self.zoom_factor {
                    self.zoom_factor = next;
                    ctx.notify();
                }
            }
            LightboxViewAction::ZoomReset => {
                if self.zoom_factor != 1.0 {
                    self.zoom_factor = 1.0;
                    ctx.notify();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn post_load_callback_rewrites_failed_to_load_to_error() {
        // Simulate the asset cache reporting a load failure for an image
        // entry. The rewrite helper must produce an Error variant whose
        // message is one of the sanitized categorical strings, never the
        // raw error string.
        let err = anyhow::anyhow!(
            "io error: failed to read /home/secret/path/to/image.png: permission denied"
        );
        let state: AssetState<ImageType> = AssetState::FailedToLoad(Rc::new(err));
        let rewritten = rewrite_image_for_load_state(&state).expect("FailedToLoad must rewrite");
        match rewritten {
            lightbox::LightboxImageSource::Error { message } => {
                assert_eq!(
                    message, "could not read image",
                    "expected sanitized 'could not read image' for io/permission errors",
                );
                assert!(
                    !message.contains("/home/"),
                    "sanitized message must not leak the absolute path",
                );
                assert!(
                    !message.contains("permission denied"),
                    "sanitized message must not leak the OS error string",
                );
            }
            other => panic!("expected Error variant, got {other:?}"),
        }
    }

    #[test]
    fn post_load_callback_rewrites_unrecognized_to_error() {
        // GH9729 §695: mislabeled / unsupported bytes (e.g. a `.png`
        // containing tarball bytes) now surface as `try_from_bytes`
        // returning `Err("could not detect image format")`, which the
        // asset cache stores as `FailedToLoad`. The rewrite helper must
        // surface the specific "detect" category, not collapse it into
        // the generic "could not decode image" bucket — `sanitize_load_error`
        // matches the "could not detect" prefix before the generic
        // "decode/format" branch.
        let err = anyhow::anyhow!("could not detect image format");
        let state: AssetState<ImageType> = AssetState::FailedToLoad(Rc::new(err));
        let rewritten = rewrite_image_for_load_state(&state)
            .expect("FailedToLoad on unrecognized format must rewrite");
        match rewritten {
            lightbox::LightboxImageSource::Error { message } => {
                assert_eq!(message, "could not detect image format");
            }
            other => panic!("expected Error variant, got {other:?}"),
        }
    }

    #[test]
    fn sanitize_load_error_picks_too_large_category() {
        let err = anyhow::anyhow!("local asset exceeds size cap");
        assert_eq!(
            super::sanitize_load_error(&err),
            "image is too large to preview"
        );
    }

    #[test]
    fn step_zoom_in_multiplies_by_step() {
        let result = super::step_zoom(1.0, super::ZoomDirection::In);
        assert!((result - lightbox::ZOOM_STEP).abs() < f32::EPSILON);
    }

    #[test]
    fn step_zoom_out_divides_by_step() {
        let result = super::step_zoom(1.0, super::ZoomDirection::Out);
        assert!((result - 1.0 / lightbox::ZOOM_STEP).abs() < f32::EPSILON);
    }

    #[test]
    fn step_zoom_in_clamps_to_max() {
        // GH9729 §698: zooming in repeatedly must saturate, not run away.
        let mut z = 1.0;
        for _ in 0..50 {
            z = super::step_zoom(z, super::ZoomDirection::In);
        }
        assert_eq!(z, lightbox::MAX_ZOOM_FACTOR);
    }

    #[test]
    fn step_zoom_out_clamps_to_min() {
        let mut z = 1.0;
        for _ in 0..50 {
            z = super::step_zoom(z, super::ZoomDirection::Out);
        }
        assert_eq!(z, lightbox::MIN_ZOOM_FACTOR);
    }

    #[test]
    fn format_metadata_line_at_native_zoom_omits_percentage() {
        // GH9729 t2-11: the percentage suffix is a visual signal that
        // zoom has changed. At 1.0 (native) the footer reads exactly
        // like pre-t2-11 so the common case stays uncluttered.
        let size = pathfinder_geometry::vector::Vector2F::new(1024., 768.);
        assert_eq!(super::format_metadata_line(size, 1.0), "1024 × 768 px");
    }

    #[test]
    fn format_metadata_line_appends_zoom_percentage_when_not_native() {
        // GH9729 t2-11 / t2-7-r1: when the image is window-capped the
        // zoom keystroke produces no visible size change, so the
        // footer is the only signal that the action fired. Verify it
        // formats as a percentage with no decimals.
        let size = pathfinder_geometry::vector::Vector2F::new(200., 200.);
        assert_eq!(super::format_metadata_line(size, 1.5), "200 × 200 px · 150%");
        assert_eq!(super::format_metadata_line(size, 2.0), "200 × 200 px · 200%");
        assert_eq!(super::format_metadata_line(size, 0.5), "200 × 200 px · 50%");
    }

    #[test]
    fn format_metadata_line_rounds_zoom_to_integer() {
        // ZOOM_STEP = 1.5 → after one zoom-in from 1.0 the factor is
        // 1.5 exactly, but accumulated multiplications produce
        // irrational-looking values (1.5 * 1.5 = 2.25, so "225%").
        // Confirm rounding behaves at a boundary.
        let size = pathfinder_geometry::vector::Vector2F::new(100., 100.);
        // 0.6667 * 100 = 66.67 → rounds to 67.
        assert_eq!(
            super::format_metadata_line(size, 1.0 / 1.5),
            "100 × 100 px · 67%"
        );
    }

    #[test]
    fn format_metadata_line_sanitises_non_finite_zoom() {
        // A NaN-poisoned `Params::zoom_factor` from an external caller
        // must not produce "NaN%" in the user-visible footer. Match
        // the same posture as the renderer-side clamp guard.
        let size = pathfinder_geometry::vector::Vector2F::new(100., 100.);
        assert_eq!(
            super::format_metadata_line(size, f32::NAN),
            "100 × 100 px",
            "NaN must collapse to native (100%) and drop the suffix"
        );
        assert_eq!(
            super::format_metadata_line(size, f32::INFINITY),
            "100 × 100 px",
        );
    }

    #[test]
    fn format_metadata_line_rounds_fractional_dimensions() {
        // GH9729 t2-8-r2 / t2-11: SVGs can have fractional intrinsic
        // sizes. Use `.round()` rather than truncating-toward-zero
        // `as i32` cast so e.g. 199.7px doesn't render as "199 px".
        let size = pathfinder_geometry::vector::Vector2F::new(199.7, 200.3);
        assert_eq!(super::format_metadata_line(size, 1.0), "200 × 200 px");
    }

    #[test]
    fn apply_rewrite_to_slot_rewrites_synchronous_failed_to_load() {
        // GH9729 t2-10: a synchronously-resolved `FailedToLoad` (reachable
        // for tiny mislabeled files after t2-4 made `try_from_bytes`
        // return `Err` immediately) must be rewritten to the `Error`
        // variant inline — the `when_loaded` future never fires for an
        // already-failed entry, so without this branch the lightbox
        // would render a permanent spinner.
        let mut images = vec![LightboxImage {
            source: lightbox::LightboxImageSource::Resolved {
                asset_source: warpui::assets::asset_cache::AssetSource::Bundled {
                    path: "fake/bundled/path.png",
                },
            },
            description: Some("filename.png".to_string()),
        }];
        let state: AssetState<ImageType> = AssetState::FailedToLoad(Rc::new(
            anyhow::anyhow!("could not detect image format"),
        ));

        let mutated = super::apply_rewrite_to_slot(&mut images, 0, &state);

        assert!(mutated, "synchronous FailedToLoad must mutate the slot");
        match &images[0].source {
            lightbox::LightboxImageSource::Error { message } => {
                assert_eq!(message, "could not detect image format");
            }
            other => panic!("expected Error variant, got {other:?}"),
        }
    }

    #[test]
    fn apply_rewrite_to_slot_leaves_loading_state_alone() {
        // The asynchronous-loading path must NOT trigger an inline
        // rewrite — the spawn callback will run when the load completes
        // and that's what's responsible for the post-load state. This
        // test pins the contract so a future tweak to
        // `rewrite_image_for_load_state` (e.g. adding a `Loading` arm)
        // can't silently break the lightbox by mutating the slot
        // mid-load.
        let mut images = vec![LightboxImage {
            source: lightbox::LightboxImageSource::Resolved {
                asset_source: warpui::assets::asset_cache::AssetSource::Bundled {
                    path: "fake/bundled/path.png",
                },
            },
            description: None,
        }];
        // `AssetState::Evicted` is the cheapest non-Loading,
        // non-FailedToLoad variant to construct in a unit test — it
        // exercises the same "no rewrite" branch as `Loading`, which
        // requires a real `LoadHandle`.
        let state: AssetState<ImageType> = AssetState::Evicted;

        let mutated = super::apply_rewrite_to_slot(&mut images, 0, &state);

        assert!(!mutated, "non-failure states must leave the slot unchanged");
        assert!(matches!(
            images[0].source,
            lightbox::LightboxImageSource::Resolved { .. }
        ));
    }

    #[test]
    fn apply_rewrite_to_slot_tolerates_out_of_bounds_index() {
        // The caller in `start_asset_load` passes a captured index that
        // could in principle be stale by the time the spawn callback
        // fires (e.g., `update_params` shrinks the image list). The
        // helper must not panic.
        let mut images: Vec<LightboxImage> = vec![];
        let state: AssetState<ImageType> = AssetState::FailedToLoad(Rc::new(
            anyhow::anyhow!("could not detect image format"),
        ));

        let mutated = super::apply_rewrite_to_slot(&mut images, 5, &state);

        assert!(!mutated, "out-of-bounds index must be a no-op, not a panic");
    }

    #[test]
    fn step_zoom_recovers_from_non_finite_input() {
        // A NaN-poisoned zoom factor would NaN-poison the ConstrainedBox
        // size and corrupt the layout. The step helper must squelch that
        // back to the safe default of 1.0.
        assert_eq!(
            super::step_zoom(f32::NAN, super::ZoomDirection::In),
            1.0,
            "NaN must be sanitized to 1.0"
        );
        assert_eq!(
            super::step_zoom(f32::INFINITY, super::ZoomDirection::Out),
            1.0,
            "infinity must be sanitized to 1.0"
        );
    }

    #[test]
    fn sanitize_load_error_picks_decode_category() {
        let err = anyhow::anyhow!("png decode error: invalid IHDR chunk");
        assert_eq!(super::sanitize_load_error(&err), "could not decode image");
    }

    #[test]
    fn sanitize_load_error_falls_back_to_read_category() {
        let err = anyhow::anyhow!("io error: connection reset");
        assert_eq!(super::sanitize_load_error(&err), "could not read image");
    }
}
