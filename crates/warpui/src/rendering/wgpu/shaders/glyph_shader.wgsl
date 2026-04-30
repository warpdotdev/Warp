// Two-stage alpha correction for glyph coverage masks.
//
// Stage 1 — contrast boost (enhance_contrast):
//   Fonts are designed assuming gamma-space blending. At 1.25× or 1.5× DPI
//   the glyph rasterizer produces AA coverage values that map to the right
//   perceived weight ONLY if blended in gamma space. Our pipeline does that
//   (non-sRGB surface, no linearisation). Still, mid-coverage fringe pixels
//   look slightly too thin for bright (white) text because the hardware gamma
//   curve is not perfectly 2.2. The Windows Terminal DWrite formula
//   enhance_contrast(α, k) = α*(k+1)/(αk+1) applies a brightness-scaled
//   boost: brighter text (higher k) gets a stronger push.
//
// Stage 2 — gamma-incorrect-target correction (apply_alpha_correction):
//   This is the ClearType / DirectWrite polynomial correction that accounts
//   for the difference between the true display gamma and the gamma=1.0
//   assumption built into the coverage values. It further boosts mid-range
//   coverage to match what a physically-perfect gamma-2.2 pipeline would
//   produce. Derived from Microsoft's gamma correction lookup table via Zed.
//   GAMMA_RATIOS correspond to gamma=1.8, a good default for Linux/BSD.
//   Reference: https://github.com/zed-industries/zed/blob/main/crates/gpui_wgpu/src/shaders.wgsl
//
// Both stages are skipped for emoji (is_emoji=1): tex_color already contains
// the final RGBA.
fn glyph_color_brightness(color: vec3<f32>) -> f32 {
    // REC. 601 luminance coefficients for perceived brightness.
    return dot(color, vec3<f32>(0.30, 0.59, 0.11));
}

fn enhance_contrast(alpha: f32, k: f32) -> f32 {
    return alpha * (k + 1.0) / (alpha * k + 1.0);
}

// Gamma correction ratios for gamma=1.8 (index 8 in Microsoft's ClearType table).
// Computed as: ratios[i] * NORM, where NORM = 65536/(255²)×4 for indices 0,2
// and 256/255×4 for indices 1,3.
const GAMMA_RATIOS: vec4<f32> = vec4<f32>(0.148, -0.895, 1.476, -0.325);

fn apply_alpha_correction(a: f32, b: f32, g: vec4<f32>) -> f32 {
    let brightness_adjustment = g.x * b + g.y;
    let correction = brightness_adjustment * a + (g.z * b + g.w);
    return a + a * (1.0 - a) * correction;
}

// Per-channel variants used by the subpixel fragment shader. Each LCD
// subpixel has its own coverage, so the contrast and gamma correction are
// applied independently to R, G, and B. The brightness term is also
// vec3 so each component contributes only to its own correction; for a
// monochrome text colour all three components are equal and the result
// matches the scalar formula above.
fn enhance_contrast3(alpha: vec3<f32>, k: f32) -> vec3<f32> {
    return alpha * (k + 1.0) / (alpha * k + 1.0);
}

fn apply_alpha_correction3(a: vec3<f32>, b: vec3<f32>, g: vec4<f32>) -> vec3<f32> {
    let brightness_adjustment = g.x * b + g.y;
    let correction = brightness_adjustment * a + (g.z * b + g.w);
    return a + a * (1.0 - a) * correction;
}

struct Uniforms {
    viewport_size: vec2<f32>,
    // Padding necessary to ensure that the uniforms is 16 bytes. Some wgpu-supported devices (such as webgl) require
    // buffer bindings to be a multiple of 16 bytes.
    padding: vec2<f32>
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

    // Use floor here to vertically align the glyph to the pixel grid.
    // If it's not aligned to the grid, the fragment shader will do its
    // own interpolation, which makes it so we don't use the anti-aliasing
    // from core text, which is what we want.  We don't force the glyph to a
    // horizontal pixel position because we rasterize the glyph at multiple
    // subpixel positions, and so the very slight linear interpolation here
    // won't produce a fuzzy glyph, just a correctly-positioned one.
    pixel_pos = vec2(pixel_pos.x, floor(pixel_pos.y));

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

    // Stage 1: brightness-scaled contrast boost (Windows Terminal formula).
    let k = glyph_color_brightness(color.rgb);
    let contrasted = enhance_contrast(tex_color.r, k);
    // Stage 2: gamma-incorrect-target polynomial correction (ClearType / Zed formula).
    let gamma_corrected = apply_alpha_correction(contrasted, k, GAMMA_RATIOS);
    color.a *= max(gamma_corrected, f32(in.is_emoji));

    // Apply the fade.
    color.a *= saturate(in.fade_alpha);
    return color;
}
