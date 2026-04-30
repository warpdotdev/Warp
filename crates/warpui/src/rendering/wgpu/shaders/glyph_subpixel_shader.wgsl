// Subpixel glyph fragment shader.
//
// This file is concatenated with glyph_shader.wgsl by the renderer when
// the device exposes wgpu::Features::DUAL_SOURCE_BLENDING; the directive
// `enable dual_source_blending;` is prepended at compile time. It must
// not be compiled standalone because it relies on declarations from
// glyph_shader.wgsl: the vertex output struct GlyphVertexShaderOutput,
// the texture/sampler bindings glyphAtlasTexture and glyphAtlasSampler,
// and the gamma-correction helpers glyph_color_brightness, GAMMA_RATIOS,
// enhance_contrast3, and apply_alpha_correction3.
//
// The mono and subpixel pipelines share the same vertex stage (vs_main
// from glyph_shader.wgsl); only the fragment stage and blend equation
// differ.

// Dual-source blending output: location 0 has two sources at indices 0 and 1.
// The pipeline blend factors Src1 and OneMinusSrc1 reference the index-1
// output to weight each LCD subpixel of the destination independently. The
// foreground RGB at index 0 is the unmodulated text color; the index-1
// channel carries per-subpixel coverage as the effective alpha source.
struct SubpixelFragmentOutput {
    @location(0) @blend_src(0) foreground: vec4<f32>,
    @location(0) @blend_src(1) alpha: vec4<f32>,
}

@fragment
fn fs_subpixel_main(in: GlyphVertexShaderOutput) -> SubpixelFragmentOutput {
    // Sample three independent coverage values from the BGRA8 subpixel
    // atlas. The .rgb swizzle reorders BGR storage to logical RGB, which
    // is what the per-channel contrast helpers expect. The non-emoji
    // branch of the upload path (texture_with_bind_group.rs) deliberately
    // does NOT R<->B swap subpixel data on the CPU, because swash's
    // subpixel byte ordering combined with the Bgra8Unorm storage already
    // produces channels in the order this swizzle expects.
    let coverage_bgr = textureSample(glyphAtlasTexture, glyphAtlasSampler, in.texture_coordinate).rgb;
    let coverage = coverage_bgr.bgr;

    // Stage 1: brightness-modulated contrast boost. The subpixel path uses
    // a smaller base factor than the grayscale path because per-channel
    // coverage already provides perceptual sharpness; an additional full
    // contrast boost on top of that pushes mid-coverage values toward 1.0
    // and reverses the subpixel fringe gradient, producing heavier and
    // softer text. light_on_dark_contrast further drops the factor to zero
    // for bright-on-dark, so white terminal text gets no Stage 1 boost.
    let enhanced_contrast = light_on_dark_contrast(uniforms.subpixel_enhanced_contrast, in.color.rgb);
    let contrasted = enhance_contrast3(coverage, enhanced_contrast);

    // Stage 2: gamma-incorrect-target polynomial correction. The brightness
    // argument is the text colour itself (vec3) so each channel corrects
    // against the matching component of the destination, which is what
    // dual-source blending will combine with.
    let gamma_corrected = apply_alpha_correction3(contrasted, in.color.rgb, uniforms.gamma_ratios);

    // Apply the fade from the vertex stage. Saturate prevents overshoot
    // outside the fade region from boosting alpha past 1.
    let fade = saturate(in.fade_alpha);

    var out: SubpixelFragmentOutput;
    // Index-0 output: unmodulated text colour. Alpha is 1.0 because the
    // dual-source blend uses index-1 as the alpha factor; the alpha
    // channel of index-0 is ignored by the BlendFactor::Src1 equation.
    out.foreground = vec4<f32>(in.color.rgb, 1.0);
    // Index-1 output: per-subpixel coverage scaled by text alpha and fade.
    // BlendFactor::Src1 uses this as the multiplier for the destination
    // colour's RGB channels.
    out.alpha = vec4<f32>(gamma_corrected * in.color.a * fade, in.color.a * fade);
    return out;
}
