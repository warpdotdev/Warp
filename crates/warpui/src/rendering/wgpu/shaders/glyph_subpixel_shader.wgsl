// Subpixel glyph fragment shader. Concatenated with glyph_shader.wgsl at
// pipeline-build time (with `enable dual_source_blending;` prepended) when
// the device exposes wgpu::Features::DUAL_SOURCE_BLENDING. Must not be
// compiled standalone: it depends on the vertex output struct, texture
// bindings, and gamma helpers from glyph_shader.wgsl, and shares vs_main
// with the mono pipeline.

// Dual-source output: index 0 carries the unmodulated foreground RGB,
// index 1 carries per-subpixel coverage. The pipeline's Src1 /
// OneMinusSrc1 blend factors use index 1 to weight each LCD subpixel of
// the destination independently.
struct SubpixelFragmentOutput {
    @location(0) @blend_src(0) foreground: vec4<f32>,
    @location(0) @blend_src(1) alpha: vec4<f32>,
}

@fragment
fn fs_subpixel_main(in: GlyphVertexShaderOutput) -> SubpixelFragmentOutput {
    // Read the three subpixel coverages as logical RGB. The upload path
    // (texture_with_bind_group.rs) swaps swash's RGBA-ordered bytes into
    // canonical BGRA before write_texture, so a Bgra8Unorm sample yields
    // (R-cov, G-cov, B-cov) directly with no swizzle.
    let coverage = textureSample(glyphAtlasTexture, glyphAtlasSampler, in.texture_coordinate).rgb;

    // Stage 1: brightness-modulated contrast boost. The subpixel base factor
    // is half the grayscale one because per-channel coverage already
    // supplies most of the perceptual sharpness; a full boost on top
    // saturates the fringe gradient and produces heavy, soft text.
    let enhanced_contrast = light_on_dark_contrast(uniforms.subpixel_enhanced_contrast, in.color.rgb);
    let contrasted = enhance_contrast3(coverage, enhanced_contrast);

    // Stage 2: gamma-correction polynomial, with the text colour itself as
    // the brightness vector so each channel corrects against the matching
    // destination component the blend will combine with.
    let gamma_corrected = apply_alpha_correction3(contrasted, in.color.rgb, uniforms.gamma_ratios);

    let fade = saturate(in.fade_alpha);

    var out: SubpixelFragmentOutput;
    // Index 0: the index-0 alpha is ignored by Src1 blending, hence 1.0.
    out.foreground = vec4<f32>(in.color.rgb, 1.0);
    // Index 1: per-subpixel coverage scaled by the text alpha and fade.
    out.alpha = vec4<f32>(gamma_corrected * in.color.a * fade, in.color.a * fade);
    return out;
}
