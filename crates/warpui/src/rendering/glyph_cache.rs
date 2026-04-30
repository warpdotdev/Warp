use crate::fonts::{canvas, RasterizedGlyph};
use crate::rendering::atlas::{self, AllocatedRegion, AtlasTextureKind, TextureId};
use crate::{fonts::SubpixelAlignment, rendering, scene::GlyphKey};
use anyhow::Result;
use ordered_float::OrderedFloat;
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::{
    rect::RectF,
    vector::{Vector2F, Vector2I},
};
use std::collections::HashMap;

const ATLAS_SIZE: usize = 1024;

/// Callback to create a texture at a given size with a given kind.
///
/// The kind tells the backend which texture format to allocate so that the
/// returned texture is sampleable by the consuming render pipeline
/// (`Generic` is `R8Unorm`, `Subpixel` and `Polychrome` are `Bgra8Unorm`).
type CreateTextureCallback<'a, T> = dyn Fn(usize, AtlasTextureKind) -> T + 'a;

/// Callback to insert [`RasterizedGlyph`] at a region identified by [`AllocatedRegion`] into a
/// texture, `T`.
type InsertIntoTextureCallback<'a, T> = dyn Fn(AllocatedRegion, &RasterizedGlyph, &mut T) + 'a;

/// Callback to compute the bounds of a glyph when rasterized.
///
/// The `bool` argument is `lcd_subpixel`: true requests LCD subpixel
/// rasterization, false grayscale. The flag affects the bitmap's pixel
/// dimensions on backends where subpixel produces wider output, so it
/// must participate in cache-key uniqueness.
pub(crate) type GlyphRasterBoundsFn<'a> =
    dyn Fn(GlyphKey, Vector2F, bool, &rendering::GlyphConfig) -> Result<RectI> + 'a;

/// Callback to rasterize a glyph.
///
/// The `bool` argument is `lcd_subpixel`; see [`GlyphRasterBoundsFn`].
pub(crate) type RasterizeGlyphFn<'a> = dyn Fn(
        GlyphKey,
        Vector2F,
        SubpixelAlignment,
        bool,
        &rendering::GlyphConfig,
        canvas::RasterFormat,
    ) -> Result<RasterizedGlyph>
    + 'a;

/// A cache that caches glyphs in texture atlases of various kinds.
///
/// Each [`AtlasTextureKind`] has its own [`atlas::Manager`] and texture
/// list, so allocations of one kind never share a texture with another.
/// Routing at insertion time: emoji go to `Polychrome`, non-emoji subpixel
/// glyphs to `Subpixel`, everything else to `Generic`.
pub struct GlyphCache<Texture> {
    generic_textures: Vec<Texture>,
    subpixel_textures: Vec<Texture>,
    polychrome_textures: Vec<Texture>,
    cache: HashMap<GlyphCacheKey, GlyphTextureOffset>,
    glyph_config: rendering::GlyphConfig,
    generic_manager: atlas::Manager,
    subpixel_manager: atlas::Manager,
    polychrome_manager: atlas::Manager,
}

#[derive(Hash, PartialEq, Eq)]
struct GlyphCacheKey {
    glyph_key: GlyphKey,
    scale_factor: OrderedFloat<f32>,
    subpixel_alignment: SubpixelAlignment,
    lcd_subpixel: bool,
}

impl GlyphCacheKey {
    fn new(
        glyph_key: GlyphKey,
        scale_factor: f32,
        subpixel_alignment: SubpixelAlignment,
        lcd_subpixel: bool,
    ) -> Self {
        GlyphCacheKey {
            glyph_key,
            scale_factor: scale_factor.into(),
            subpixel_alignment,
            lcd_subpixel,
        }
    }
}

/// A glyph within a texture atlas.
///
/// `kind` and `texture_id` together address a specific atlas texture: the
/// `kind` selects which per-kind list of textures the [`GlyphCache`] holds,
/// and `texture_id` indexes into that list.
#[derive(Copy, Debug, Clone)]
pub(crate) struct GlyphTextureOffset {
    pub kind: AtlasTextureKind,
    pub texture_id: TextureId,
    pub allocated_region: AllocatedRegion,
    pub raster_bounds: RectF,
    pub is_emoji: bool,
}

impl<Texture> GlyphCache<Texture> {
    pub(crate) fn new(glyph_config: rendering::GlyphConfig) -> Self {
        GlyphCache {
            generic_textures: Vec::new(),
            subpixel_textures: Vec::new(),
            polychrome_textures: Vec::new(),
            cache: HashMap::new(),
            glyph_config,
            generic_manager: atlas::Manager::new(ATLAS_SIZE),
            subpixel_manager: atlas::Manager::new(ATLAS_SIZE),
            polychrome_manager: atlas::Manager::new(ATLAS_SIZE),
        }
    }

    pub(crate) fn update_config(&mut self, glyph_config: &rendering::GlyphConfig) {
        // If the glyph rendering configuration has changed, blow away the cache
        // and replace ourself with a new one.
        if *glyph_config != self.glyph_config {
            *self = GlyphCache::new(*glyph_config);
        }
    }

    /// Returns the texture for the atlas of the given `kind` at `texture_id`.
    pub(crate) fn texture(
        &self,
        kind: AtlasTextureKind,
        texture_id: &TextureId,
    ) -> Option<&Texture> {
        self.textures_for(kind).get(texture_id.as_usize())
    }

    fn textures_for(&self, kind: AtlasTextureKind) -> &Vec<Texture> {
        match kind {
            AtlasTextureKind::Generic => &self.generic_textures,
            AtlasTextureKind::Subpixel => &self.subpixel_textures,
            AtlasTextureKind::Polychrome => &self.polychrome_textures,
        }
    }

    fn textures_for_mut(&mut self, kind: AtlasTextureKind) -> &mut Vec<Texture> {
        match kind {
            AtlasTextureKind::Generic => &mut self.generic_textures,
            AtlasTextureKind::Subpixel => &mut self.subpixel_textures,
            AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
        }
    }

    fn manager_for(&mut self, kind: AtlasTextureKind) -> &mut atlas::Manager {
        match kind {
            AtlasTextureKind::Generic => &mut self.generic_manager,
            AtlasTextureKind::Subpixel => &mut self.subpixel_manager,
            AtlasTextureKind::Polychrome => &mut self.polychrome_manager,
        }
    }

    /// Returns a [`GlyphTextureOffset`] identified by [`GlyphKey`]. If the [`GlyphKey`] has not
    /// been previously cached, the glyph is rasterized and inserted into the texture via the
    /// `insert_into_texture` callback. If a new texture needs to be created (since a previous
    /// texture is now full), the `create_texture` callback is called to construct a new texture
    /// atlas of the appropriate kind.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn get(
        &mut self,
        glyph_key: GlyphKey,
        scale_factor: f32,
        subpixel_alignment: SubpixelAlignment,
        lcd_subpixel: bool,
        create_texture: &CreateTextureCallback<'_, Texture>,
        insert_into_texture: &InsertIntoTextureCallback<'_, Texture>,
        raster_bounds_fn: &GlyphRasterBoundsFn<'_>,
        rasterize_glyph_fn: &RasterizeGlyphFn<'_>,
    ) -> Result<Option<GlyphTextureOffset>> {
        let cache_key =
            GlyphCacheKey::new(glyph_key, scale_factor, subpixel_alignment, lcd_subpixel);

        if let Some(gto) = self.cache.get(&cache_key) {
            return Ok(Some(*gto));
        }

        let bounds = raster_bounds_fn(
            glyph_key,
            Vector2F::splat(scale_factor),
            lcd_subpixel,
            &self.glyph_config,
        )?;

        if bounds.size() == Vector2I::zero() {
            return Ok(None);
        }

        let rasterized_glyph = rasterize_glyph_fn(
            glyph_key,
            Vector2F::splat(scale_factor),
            subpixel_alignment,
            lcd_subpixel,
            &self.glyph_config,
            crate::fonts::canvas::RasterFormat::Rgba32,
        )?;

        // Route to the atlas whose format matches the pixel data:
        //   - Emoji -> Polychrome (Bgra8Unorm), real RGBA colour.
        //   - lcd_subpixel non-emoji -> Subpixel (Bgra8Unorm), three
        //     independent coverage values per texel.
        //   - Everything else -> Generic (R8Unorm), one coverage byte.
        let kind = if rasterized_glyph.is_emoji {
            AtlasTextureKind::Polychrome
        } else if lcd_subpixel {
            AtlasTextureKind::Subpixel
        } else {
            AtlasTextureKind::Generic
        };

        let texture_offset = self.manager_for(kind).insert(rasterized_glyph.canvas.size)?;
        let idx = texture_offset.texture_id.as_usize();
        let textures = self.textures_for_mut(kind);
        if idx >= textures.len() {
            textures.resize_with(idx + 1, || create_texture(ATLAS_SIZE, kind));
        }
        let texture = &mut textures[idx];
        insert_into_texture(texture_offset.allocated_region, &rasterized_glyph, texture);

        let glyph_texture_offset = GlyphTextureOffset {
            kind,
            texture_id: texture_offset.texture_id,
            raster_bounds: bounds.to_f32(),
            is_emoji: rasterized_glyph.is_emoji,
            allocated_region: texture_offset.allocated_region,
        };

        self.cache.insert(cache_key, glyph_texture_offset);
        Ok(Some(glyph_texture_offset))
    }
}
