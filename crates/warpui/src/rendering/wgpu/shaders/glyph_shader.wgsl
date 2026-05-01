// Two-stage alpha correction for glyph coverage masks. Skipped for emoji
// (is_emoji=1) where tex_color already contains the final RGBA.
//
// Stage 1, enhance_contrast: applies the Windows Terminal DWrite formula
// alpha*(k+1)/(alpha*k+1) to push mid-coverage fringe pixels darker. The
// boost is brightness-scaled.
//
// Stage 2, apply_alpha_correction: the ClearType / DirectWrite polynomial
// that corrects for the gap between the true display gamma and the gamma=1.0
// assumption baked into the rasterizer's coverage values. GAMMA_RATIOS
// targets gamma=1.8, a Linux/BSD-friendly default.
// Reference: https://github.com/zed-industries/zed/blob/main/crates/gpui_wgpu/src/shaders.wgsl
fn glyph_color_brightness(color: vec3<f32>) -> f32 {
    // REC. 601 luminance coefficients for perceived brightness.
    return dot(color, vec3<f32>(0.30, 0.59, 0.11));
}

fn enhance_contrast(alpha: f32, k: f32) -> f32 {
    return alpha * (k + 1.0) / (alpha * k + 1.0);
}

// gamma_ratios, grayscale_enhanced_contrast, and subpixel_enhanced_contrast
// arrive through the Uniforms buffer below, populated by the host from
// WARP_FONTS_GAMMA / WARP_FONTS_GRAYSCALE_ENHANCED_CONTRAST /
// WARP_FONTS_SUBPIXEL_ENHANCED_CONTRAST at renderer creation.

fn apply_alpha_correction(a: f32, b: f32, g: vec4<f32>) -> f32 {
    let brightness_adjustment = g.x * b + g.y;
    let correction = brightness_adjustment * a + (g.z * b + g.w);
    return a + a * (1.0 - a) * correction;
}

// Per-channel variants for the subpixel fragment shader: each LCD subpixel
// gets its own contrast and gamma correction. Falls back to the scalar
// formula above when all three components are equal.
fn enhance_contrast3(alpha: vec3<f32>, k: f32) -> vec3<f32> {
    return alpha * (k + 1.0) / (alpha * k + 1.0);
}

fn apply_alpha_correction3(a: vec3<f32>, b: vec3<f32>, g: vec4<f32>) -> vec3<f32> {
    let brightness_adjustment = g.x * b + g.y;
    let correction = brightness_adjustment * a + (g.z * b + g.w);
    return a + a * (1.0 - a) * correction;
}

// Despite the name, returns ZERO boost for bright text on dark backgrounds
// (already high contrast; extra boost just thickens). Mid-gray and darker
// text gets the full factor. From Zed's apply_contrast_and_gamma_correction.
fn light_on_dark_contrast(enhanced_contrast: f32, color: vec3<f32>) -> f32 {
    let brightness = glyph_color_brightness(color);
    let multiplier = saturate(4.0 * (0.75 - brightness));
    return enhanced_contrast * multiplier;
}

struct Uniforms {
    viewport_size: vec2<f32>,
    // Eight bytes of padding so gamma_ratios lands at offset 16. Earlier
    // versions used these bytes for a premultiplied_alpha flag, but the
    // ALPHA_BLENDING pipeline blend already produces a pre-multiplied
    // framebuffer from a straight-alpha source (src_factor=SrcAlpha
    // multiplies by alpha at the blend stage), so the shader does not
    // need to pre-multiply on its side.
    _padding_after_viewport: vec2<u32>,
    // ClearType / DirectWrite gamma-correction polynomial coefficients.
    gamma_ratios: vec4<f32>,
    // Stage 1 contrast factor for the grayscale path. Default 1.0.
    grayscale_enhanced_contrast: f32,
    // Stage 1 contrast factor for the subpixel path. Default 0.5.
    subpixel_enhanced_contrast: f32,
    _padding1: vec2<u32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@group(1) @binding(0) var glyphAtlasTexture: texture_2d<f32>;
@group(1) @binding(1) var glyphAtlasSampler: sampler;

struct GlyphVertexShaderInput {
    // The position of the vertex in normalized device coordinates.
    @location(0) vertex_position: vec2<f32>,
    @location(1) bounds: vec4<f32>,
    @location(2) uv_bounds: vec4<f32>,
    @location(3) fade_start: f32,
    @location(4) fade_end: f32,
    @location(5) color: vec4<f32>,
    @location(6) is_emoji: i32,
}

struct GlyphVertexShaderOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) rect_center: vec2<f32>,
    @location(1) rect_corner: vec2<f32>,
    @location(2) texture_coordinate: vec2<f32>,
    @location(3) fade_alpha: f32,
    @location(4) color: vec4<f32>,
    @location(5) is_emoji: i32,
}

@vertex
fn vs_main(
    glyph: GlyphVertexShaderInput,
) -> GlyphVertexShaderOutput {
    var out: GlyphVertexShaderOutput;
    var origin: vec2<f32> = glyph.bounds.xy;
    var size: vec2<f32> = glyph.bounds.zw;
    var pixel_pos: vec2<f32> = glyph.vertex_position * size + origin;

    // No flooring here: the Rust side already snaps quad origins to integer
    // physical pixels, so pixel_pos is integer at the vertices. Flooring
    // again would shift the quad by < 1 pixel and reintroduce the sub-pixel
    // mismatch the pre-floor was meant to fix.

    // Evaluating the glyphs fade effect. Note that the fade may go in two different directions:
    // - Right to left (default) - where the opaque side is on the right, and transparent on the left
    //   (in this case, the start_fade < end_fade; start is where the fade is transparent)
    // - Left to right - where the opaque side is on the left, and it fades towards the right side.
    //   In this case, start_fade > end_fade, and the opaque side is on the left (end_fade).
    // To clarify: fade_start is ALWAYS where the fade is transparent, and fade_end is ALWAYS where
    // the opaque part is, this is reflected in how we compute width, dist, and alpha.
    var fade_width: f32 = abs(glyph.fade_end - glyph.fade_start);
    var fade_dist: f32 = pixel_pos.x - min(glyph.fade_start, glyph.fade_end);

    var fade_alpha: f32;
    if glyph.fade_end < glyph.fade_start { // left-to-right case
        fade_alpha = fade_dist / fade_width;
    } else { // right-to-left case
        fade_alpha = 1. - fade_dist / fade_width;
    }

     // Convert the position of the item from screen coordinates into normalized device coordinates
    var device_pos: vec2<f32> = pixel_pos / uniforms.viewport_size * vec2(2.0, -2.0) + vec2(-1.0, 1.0);

    var texture_coordinate: vec2<f32> = glyph.uv_bounds.xy + glyph.vertex_position * glyph.uv_bounds.zw;

    out.position = vec4<f32>(device_pos, 0.0, 1.0);
    out.rect_corner = size / 2.0;
    out.rect_center = origin + out.rect_corner;
    out.texture_coordinate = texture_coordinate;
    out.fade_alpha = fade_alpha;
    out.color = glyph.color;
    out.is_emoji = glyph.is_emoji;
    return out;
}

@fragment
fn fs_main(in: GlyphVertexShaderOutput) -> @location(0) vec4<f32> {
    // Sample the texture to obtain a color.
    var tex_color: vec4<f32> = textureSample(glyphAtlasTexture, glyphAtlasSampler, in.texture_coordinate);
    // Use the input color for non-emoji, and the sampled color for emoji.
    var color: vec4<f32> = mix(in.color, tex_color, f32(in.is_emoji));

    // Stage 1: brightness-modulated contrast boost.
    let enhanced_contrast = light_on_dark_contrast(uniforms.grayscale_enhanced_contrast, color.rgb);
    let contrasted = enhance_contrast(tex_color.r, enhanced_contrast);
    // Stage 2: gamma-correction polynomial weighted by the text's luminance.
    let brightness = glyph_color_brightness(color.rgb);
    let gamma_corrected = apply_alpha_correction(contrasted, brightness, uniforms.gamma_ratios);
    color.a *= max(gamma_corrected, f32(in.is_emoji));

    // Apply the fade.
    color.a *= saturate(in.fade_alpha);

    // Emit straight-alpha RGBA. The pipeline's BlendState::ALPHA_BLENDING
    // applies SrcAlpha at the blend stage, which converts this straight
    // source into the pre-multiplied framebuffer the PreMultiplied
    // compositor expects. Pre-multiplying here would double-apply alpha
    // and darken AA edges by a factor of alpha.
    return color;
}
