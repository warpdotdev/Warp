//! Gamma correction tables and helpers for the glyph fragment shaders.
//!
//! The numbers in [`get_gamma_correction_ratios`] come from Microsoft's
//! ClearType reference implementation as reproduced by gpui's text
//! rendering pipeline. Each row is a four-coefficient polynomial that
//! corrects coverage values produced by the rasterizer (which assumes
//! gamma = 1.0) for the actual perceived gamma of the destination
//! display, the gamma the user wants the rendered glyph to look correct
//! at.
//!
//! The shader applies the correction as
//!     corrected = a + a * (1 - a) * ((g.x*b + g.y) * a + (g.z*b + g.w))
//! where `a` is the raw coverage, `b` is the text colour brightness, and
//! `g` is the four-element vector returned here.

/// Default gamma to use when the env var is unset. 1.8 is the
/// macOS / Linux compromise gpui uses; it produces text that reads as
/// "neither too thick nor too thin" on the typical desktop displays
/// this renderer targets.
pub const DEFAULT_GAMMA: f32 = 1.8;

/// Default Stage 1 contrast factor for the grayscale glyph path. Matches
/// gpui's `grayscale_enhanced_contrast` default.
pub const DEFAULT_GRAYSCALE_ENHANCED_CONTRAST: f32 = 1.0;

/// Default Stage 1 contrast factor for the LCD subpixel glyph path.
/// Half the grayscale value because per-channel coverage already supplies
/// most of the perceptual sharpness; piling on a full grayscale-strength
/// boost saturates the fringe and reverses the subpixel resolution gain.
pub const DEFAULT_SUBPIXEL_ENHANCED_CONTRAST: f32 = 0.5;

/// Computes the four-element gamma-correction ratio vector that the
/// fragment shader expects in its uniform buffer.
///
/// `gamma` is clamped to the range [1.0, 2.2] in 0.1 increments before
/// the lookup; values outside that range are pinned to the closest
/// supported entry. Calling with the default 1.8 returns the same
/// numbers the shaders previously had hardcoded.
pub fn get_gamma_correction_ratios(gamma: f32) -> [f32; 4] {
    // Coefficients sourced from Microsoft's ClearType reference,
    // reproduced from gpui's gamma table. Each row corresponds to a
    // gamma value of 1.0, 1.1, ..., 2.2 in steps of 0.1; the divisions
    // by 4 are part of the original encoding and are kept literal so
    // the table reads identically to the upstream.
    const RATIOS: [[f32; 4]; 13] = [
        [0.0000 / 4.0, 0.0000 / 4.0, 0.0000 / 4.0, 0.0000 / 4.0],
        [0.0166 / 4.0, -0.0807 / 4.0, 0.2227 / 4.0, -0.0751 / 4.0],
        [0.0350 / 4.0, -0.1760 / 4.0, 0.4325 / 4.0, -0.1370 / 4.0],
        [0.0543 / 4.0, -0.2821 / 4.0, 0.6302 / 4.0, -0.1876 / 4.0],
        [0.0739 / 4.0, -0.3963 / 4.0, 0.8167 / 4.0, -0.2287 / 4.0],
        [0.0933 / 4.0, -0.5161 / 4.0, 0.9926 / 4.0, -0.2616 / 4.0],
        [0.1121 / 4.0, -0.6395 / 4.0, 1.1588 / 4.0, -0.2877 / 4.0],
        [0.1300 / 4.0, -0.7649 / 4.0, 1.3159 / 4.0, -0.3080 / 4.0],
        [0.1469 / 4.0, -0.8911 / 4.0, 1.4644 / 4.0, -0.3234 / 4.0],
        [0.1627 / 4.0, -1.0170 / 4.0, 1.6051 / 4.0, -0.3347 / 4.0],
        [0.1773 / 4.0, -1.1420 / 4.0, 1.7385 / 4.0, -0.3426 / 4.0],
        [0.1908 / 4.0, -1.2652 / 4.0, 1.8650 / 4.0, -0.3476 / 4.0],
        [0.2031 / 4.0, -1.3864 / 4.0, 1.9851 / 4.0, -0.3501 / 4.0],
    ];

    // Normalisation constants from Microsoft's reference. NORM13 applies
    // to the indices that produce a 16-bit-shifted result, NORM24 to the
    // 8-bit-shifted indices.
    const NORM13: f32 = ((0x10000 as f64) / (255.0 * 255.0) * 4.0) as f32;
    const NORM24: f32 = ((0x100 as f64) / 255.0 * 4.0) as f32;

    let index = ((gamma * 10.0).round() as usize).clamp(10, 22) - 10;
    let ratios = RATIOS[index];
    [
        ratios[0] * NORM13,
        ratios[1] * NORM24,
        ratios[2] * NORM13,
        ratios[3] * NORM24,
    ]
}

/// Reads the user-configurable gamma and Stage 1 contrast factors from
/// process env vars and falls back to the defaults above when an env
/// var is unset or fails to parse. Returns
/// `(gamma_ratios, grayscale_enhanced_contrast, subpixel_enhanced_contrast)`.
pub fn read_env_gamma_settings() -> ([f32; 4], f32, f32) {
    let gamma = std::env::var("WARP_FONTS_GAMMA")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_GAMMA)
        .clamp(1.0, 2.2);
    let grayscale = std::env::var("WARP_FONTS_GRAYSCALE_ENHANCED_CONTRAST")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_GRAYSCALE_ENHANCED_CONTRAST)
        .max(0.0);
    let subpixel = std::env::var("WARP_FONTS_SUBPIXEL_ENHANCED_CONTRAST")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_SUBPIXEL_ENHANCED_CONTRAST)
        .max(0.0);
    (get_gamma_correction_ratios(gamma), grayscale, subpixel)
}
