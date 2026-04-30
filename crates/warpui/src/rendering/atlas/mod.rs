mod allocator;
mod manager;

pub(crate) use manager::{Manager, TextureId};

use pathfinder_geometry::rect::{RectF, RectI};
use thiserror::Error;

/// Distinguishes the kinds of glyph atlases that the renderer maintains.
///
/// The kinds carry different texture formats and are sampled by different
/// render pipelines:
///
/// - `Generic` is the default `Rgba8Unorm` atlas used for grayscale glyph
///   coverage and color emoji. The fragment shader interprets the sampled
///   value as either coverage (when [`super::scene::Glyph`] is monochrome)
///   or as RGBA color (when the rasterized glyph is an emoji).
///
/// - `Subpixel` is a `Bgra8Unorm` atlas that stores three independent
///   coverage values per texel, one per LCD subpixel in BGR order, produced
///   by swash's subpixel rasterizer. The subpixel render pipeline composites
///   it through dual-source blending so each subpixel weights the
///   destination color independently.
///
/// Atlases of different kinds never share textures: an allocated rectangle
/// is meaningful only within its kind's manager.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum AtlasTextureKind {
    Generic,
    Subpixel,
}

/// A region of an atlas that has been allocated.
#[derive(Copy, Debug, Clone)]
pub(crate) struct AllocatedRegion {
    /// The region of the atlas that was allocated in UV (texture) coordinates.
    pub uv_region: RectF,
    /// The region of the atlas that was allocated in screen coordinates.
    pub pixel_region: RectI,
}

/// Error that can happen when attempting to allocate an element into the atlas.
#[derive(Error, Debug)]
pub(crate) enum AllocationError {
    /// Texture atlas is full.
    #[error("Unable to insert; atlas is full")]
    Full,

    /// The item cannot fit within a single texture.
    #[error("Unable to insert; item is too large to fit into atlas")]
    ItemTooLarge,
}
