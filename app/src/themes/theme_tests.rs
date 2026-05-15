use super::*;
use crate::util::color::OPAQUE;
use warp_core::ui::color::contrast::{high_enough_contrast, MinimumAllowedContrast};

#[test]
#[cfg(not(target_family = "wasm"))]
fn in_memory_theme_generation_test() {
    let mountains_bg_path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "assets",
        "async",
        "jpg",
        "mountains.jpg",
    ]
    .iter()
    .collect();

    let mut in_memory_theme = warpui::r#async::block_on(InMemoryThemeOptions::new(
        "mountains".to_string(),
        mountains_bg_path.clone(),
    ))
    .unwrap();

    let mountains_bg_path_string = mountains_bg_path.to_str().unwrap_or_default().to_owned();
    assert_eq!(
        in_memory_theme.theme(),
        WarpTheme::new(
            // the theme defaults to the 0th bg color
            ColorU::new(35, 31, 44, OPAQUE).into(),
            // this background color makes it a "dark" theme, so the foreground is white
            ColorU::white(),
            // the most distinct accent color is 3rd one
            ColorU::new(238, 203, 111, OPAQUE).into(),
            None,
            Some(Details::Darker),
            dark_mode_colors(),
            Some(Image {
                source: AssetSource::LocalFile {
                    path: mountains_bg_path_string.clone()
                },
                opacity: 30,
            }),
            Some("mountains".to_string()),
        )
    );

    in_memory_theme.chosen_bg_color_index = 2;

    assert_eq!(
        in_memory_theme.theme(),
        WarpTheme::new(
            // now the background is the 2nd one
            ColorU::new(229, 142, 113, OPAQUE).into(),
            // changing the background color made this a light theme
            ColorU::black(),
            // now the 4th color is the most distinct color
            ColorU::new(193, 217, 212, OPAQUE).into(),
            None,
            Some(Details::Lighter),
            light_mode_colors(),
            Some(Image {
                source: AssetSource::LocalFile {
                    path: mountains_bg_path_string
                },
                opacity: 30,
            }),
            Some("mountains".to_string()),
        )
    );
}

#[test]
fn high_contrast_theme_uses_accessible_text_colors() {
    let theme = WarpThemeConfig::new().theme(&ThemeKind::HighContrast);
    let background = theme.background().into_solid();
    let foreground = theme.foreground().into_solid();

    assert_eq!(theme.name().as_deref(), Some("High Contrast"));
    assert!(high_enough_contrast(
        foreground,
        background,
        MinimumAllowedContrast::Text
    ));
    assert!(high_enough_contrast(
        theme.accent().into_solid(),
        background,
        MinimumAllowedContrast::Text
    ));

    let terminal_colors = theme.terminal_colors();
    let ansi_colors = [
        terminal_colors.normal.black,
        terminal_colors.normal.red,
        terminal_colors.normal.green,
        terminal_colors.normal.yellow,
        terminal_colors.normal.blue,
        terminal_colors.normal.magenta,
        terminal_colors.normal.cyan,
        terminal_colors.normal.white,
        terminal_colors.bright.black,
        terminal_colors.bright.red,
        terminal_colors.bright.green,
        terminal_colors.bright.yellow,
        terminal_colors.bright.blue,
        terminal_colors.bright.magenta,
        terminal_colors.bright.cyan,
        terminal_colors.bright.white,
    ];

    for ansi_color in ansi_colors {
        assert!(high_enough_contrast(
            ColorU::from(ansi_color),
            background,
            MinimumAllowedContrast::Text
        ));
    }
}
