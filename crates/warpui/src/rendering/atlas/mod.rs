mod allocator;
mod manager;

pub(crate) use manager::{Manager, TextureId};

use pathfinder_geometry::rect::{RectF, RectI};
use thiserror::Error;

/// Distinguishes the kinds of glyph atlases that the renderer maintains.
/// Each kind carries a different texture format and is consumed by
/// different fragment-shader logic.
///
/// - `Generic` (R8Unorm): single-byte coverage from `Format::Alpha`,
///   used by non-emoji glyphs on the grayscale fallback path.
/// - `Subpixel` (Bgra8Unorm): three per-LCD-subpixel coverage values in
///   BGR order from swash's subpixel rasterizer, composited via the
///   dual-source-blend pipeline.
/// - `Polychrome` (Bgra8Unorm): real RGBA colour for emoji glyphs from
///   `Source::ColorOutline` / `Source::ColorBitmap`; sampled as colour.
///
/// Atlases of different kinds never share textures: an allocated rectangle
/// is meaningful only within its kind's manager.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum AtlasTextureKind {
    Generic,
    Subpixel,
    Polychrome,
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
