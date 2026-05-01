use std::mem;

use pathfinder_geometry::vector::Vector2F;
use wgpu::{BindGroup, BindGroupLayout, Buffer};

use crate::rendering::wgpu::{shader_types, Resources};

pub(super) struct Uniforms {
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    buffer: Buffer,
}

impl Uniforms {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Quad Uniforms Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                // The glyph fragment shader reads gamma_ratios and the two
                // enhanced_contrast factors from this buffer, on top of the
                // viewport_size the vertex stage needs. Visibility has to
                // cover both stages.
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        mem::size_of::<shader_types::Uniforms>() as wgpu::BufferAddress,
                    ),
                },
                count: None,
            }],
        });

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniforms buffer"),
            size: mem::size_of::<shader_types::Uniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniforms Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            bind_group_layout,
            bind_group,
            buffer,
        }
    }

    pub fn bind_group_layout(&self) -> &BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn configure_render_pass<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        drawable_size: Vector2F,
        resources: &Resources,
    ) {
        // Gamma and Stage 1 contrast factors are cached on Resources but
        // re-uploaded per-frame so the uniform buffer's payload stays
        // self-contained. The CompositeAlphaMode is intentionally not
        // sampled here: the pipeline's BlendState::ALPHA_BLENDING
        // already converts straight-alpha shader output into the
        // pre-multiplied framebuffer that the PreMultiplied compositor
        // expects, so the shader does not need to pre-multiply on its
        // side.
        let uniforms = shader_types::Uniforms::new(
            drawable_size,
            resources.gamma_ratios,
            resources.grayscale_enhanced_contrast,
            resources.subpixel_enhanced_contrast,
        );
        resources
            .queue
            .write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[uniforms]));
        render_pass.set_bind_group(0, &self.bind_group, &[]);
    }
}
