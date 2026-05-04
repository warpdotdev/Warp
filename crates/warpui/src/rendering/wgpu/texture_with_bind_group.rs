use crate::fonts::RasterizedGlyph;

use crate::rendering::atlas::AllocatedRegion;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupLayout, Extent3d, Queue, Sampler,
    TexelCopyBufferLayout, Texture, TextureDescriptor, TextureFormat, TextureUsages,
};

/// Helper struct that includes a [`Texture`] and its corresponding [`BindGroup`] for use in the
/// `GlyphCache`.
///
/// The format is recorded so [`Self::insert_glyph_into_texture`] can
/// convert the rasterizer's RGBA32 output into the texture's layout.
pub(super) struct TextureWithBindGroup {
    texture: Texture,
    /// The [`BindGroup`] associated with the `texture`. We compute this whenever we need to create
    /// a new texture as a performance optimization to ensure we don't create it on every render.
    bind_group: BindGroup,
    format: TextureFormat,
}

impl TextureWithBindGroup {
    /// Creates a new atlas texture of the given pixel `format`.
    ///
    /// Two formats are used: `R8Unorm` for the monochrome coverage atlas
    /// (one byte per texel) and `Bgra8Unorm` for both the subpixel coverage
    /// atlas and the polychrome (emoji) atlas (four bytes per texel). The
    /// format drives the upload-path conversion in
    /// [`Self::insert_glyph_into_texture`] below.
    pub(super) fn new(
        size: usize,
        format: TextureFormat,
        device: &wgpu::Device,
        bind_group_layout: &BindGroupLayout,
        sampler: &Sampler,
    ) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Glyph atlas texture"),
            size: Extent3d {
                width: size as u32,
                height: size as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
            label: None,
        });

        Self {
            texture,
            bind_group,
            format,
        }
    }

    pub(super) fn insert_glyph_into_texture(
        &mut self,
        region: AllocatedRegion,
        glyph: &RasterizedGlyph,
        queue: &Queue,
    ) {
        // Convert the rasterizer's RGBA32 canvas into the destination
        // texture's layout. Two cases:
        //
        //   R8Unorm (Generic): extract the alpha byte. The rasterizer
        //   replicates the A8 mask into RGBA32, so the first byte of each
        //   four-byte group is the original coverage.
        //
        //   Bgra8Unorm (Polychrome emoji and Subpixel non-emoji): swash's
        //   Color and SubpixelMask outputs are both RGBA-ordered in memory.
        //   The Subpixel case looks BGRA from its Format::subpixel_bgra
        //   name but zeno actually writes byte 0 = R-coverage and byte 2 =
        //   B-coverage (zeno mask.rs render() with sample offsets
        //   [+0.3, 0, -0.3]). Swap R and B per pixel so the texture's bytes
        //   match its declared BGRA layout, mirroring zed's gpui_wgpu
        //   cosmic_text_system.rs which calls out the same swash quirk.
        let pixel_count = (region.pixel_region.width() * region.pixel_region.height()) as usize;
        let upload_bytes: std::borrow::Cow<'_, [u8]>;
        let bytes_per_row = match self.format {
            TextureFormat::R8Unorm => {
                let mut compact = Vec::with_capacity(pixel_count);
                for chunk in glyph.canvas.pixels.chunks_exact(4) {
                    compact.push(chunk[0]);
                }
                upload_bytes = std::borrow::Cow::Owned(compact);
                region.pixel_region.width() as u32
            }
            TextureFormat::Bgra8Unorm => {
                let mut swapped = glyph.canvas.pixels.clone();
                for pixel in swapped.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
                upload_bytes = std::borrow::Cow::Owned(swapped);
                4 * region.pixel_region.width() as u32
            }
            other => {
                debug_assert!(
                    matches!(self.format, TextureFormat::R8Unorm | TextureFormat::Bgra8Unorm),
                    "unexpected glyph atlas format {other:?}; upload assumes R8Unorm or Bgra8Unorm",
                );
                upload_bytes = std::borrow::Cow::Borrowed(glyph.canvas.pixels.as_slice());
                4 * region.pixel_region.width() as u32
            }
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: region.pixel_region.origin_x() as u32,
                    y: region.pixel_region.origin_y() as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            upload_bytes.as_ref(),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
            Extent3d {
                width: region.pixel_region.width() as u32,
                height: region.pixel_region.height() as u32,
                depth_or_array_layers: 1,
            },
        );
    }

    pub(super) fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }
}
