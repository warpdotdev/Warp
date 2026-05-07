use std::sync::Arc;

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
        FixedBinding::new("right", LightboxViewAction::NavigateNext, view_id),
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
}

/// A view that renders a full-window lightbox overlay.
pub struct LightboxView {
    params: LightboxParams,
    current_index: usize,
    lightbox: lightbox::Lightbox,
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
        self.start_asset_loads(ctx);
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
    /// The spawn callback also re-queries the asset cache for the post-load
    /// state and rewrites `self.params.images[index].source` to
    /// `LightboxImageSource::Error` on `FailedToLoad` (which after the
    /// §695 refactor also covers the previous `Loaded { Unrecognized }`
    /// path: `try_from_bytes` now returns `Err("could not detect image
    /// format")` for mislabeled bytes, and the asset cache stores that
    /// as `FailedToLoad`). Without this, a load failure or a mislabeled
    /// file would render a permanent spinner. See specs/GH9729/tech.md
    /// §182 and §695.
    fn start_asset_load(
        &mut self,
        index: usize,
        asset_source: AssetSource,
        ctx: &mut ViewContext<Self>,
    ) {
        let asset_cache = AssetCache::as_ref(ctx);
        if let AssetState::Loading { handle } =
            asset_cache.load_asset::<ImageType>(asset_source.clone())
        {
            if let Some(future) = handle.when_loaded(asset_cache) {
                ctx.spawn(future, move |me, (), ctx| {
                    ctx.notify();
                    let asset_cache = AssetCache::as_ref(ctx);
                    let state = asset_cache.load_asset::<ImageType>(asset_source.clone());
                    if let Some(new_source) = rewrite_image_for_load_state(&state) {
                        if let Some(slot) = me.params.images.get_mut(index) {
                            slot.source = new_source;
                        }
                    }
                });
            }
        }
    }
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
                    ctx.notify();
                }
            }
            LightboxViewAction::NavigateNext => {
                if self.current_index + 1 < self.params.images.len() {
                    self.current_index += 1;
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
