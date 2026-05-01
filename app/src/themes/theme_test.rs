use super::*;
use crate::util::color::OPAQUE;

#[test]
fn custom_theme_path_under_theme_root_serializes_relative() {
    let root = PathBuf::from("/home/user/.local/share/warp-terminal/themes");
    let path = root.join("catppuccin/catppuccin_mocha.yml");

    assert_eq!(
        custom_theme_path_for_storage(&path, &root),
        PathBuf::from("catppuccin/catppuccin_mocha.yml")
    );
}

#[test]
fn custom_theme_relative_path_resolves_under_local_theme_root() {
    let root = PathBuf::from("/Users/ivan/.warp/themes");
    let stored = PathBuf::from("catppuccin/catppuccin_latte.yml");

    assert_eq!(
        custom_theme_path_from_storage(&stored, &root),
        root.join("catppuccin/catppuccin_latte.yml")
    );
}

#[test]
fn custom_theme_relative_parent_dir_path_is_preserved() {
    let root = PathBuf::from("/Users/ivan/.warp/themes");
    let stored = PathBuf::from("../outside.yml");

    assert_eq!(custom_theme_path_from_storage(&stored, &root), stored);
}

#[test]
fn custom_theme_relative_parent_dir_path_is_not_portable() {
    let root = PathBuf::from("/Users/ivan/.warp/themes");

    assert!(!custom_theme_path_is_portable(
        &PathBuf::from("../outside.yml"),
        &root
    ));
}

#[test]
fn custom_theme_legacy_macos_path_resolves_by_theme_root_suffix_when_local_file_exists() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("warp-terminal/themes");
    let local = root.join("catppuccin/catppuccin_mocha.yml");
    std::fs::create_dir_all(local.parent().unwrap()).unwrap();
    std::fs::write(&local, "").unwrap();

    let stored = PathBuf::from("/Users/ivan/.warp/themes/catppuccin/catppuccin_mocha.yml");

    assert_eq!(custom_theme_path_from_storage(&stored, &root), local);
}

#[test]
fn custom_theme_legacy_parent_dir_path_is_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("warp-terminal/themes");
    let local = root.join("outside.yml");
    std::fs::create_dir_all(local.parent().unwrap()).unwrap();
    std::fs::write(&local, "").unwrap();

    let stored = PathBuf::from("/Users/ivan/.warp/themes/../outside.yml");

    assert_eq!(custom_theme_path_from_storage(&stored, &root), stored);
}

#[test]
fn custom_theme_legacy_linux_path_resolves_by_theme_root_suffix_when_local_file_exists() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join(".warp/themes");
    let local = root.join("catppuccin/catppuccin_latte.yml");
    std::fs::create_dir_all(local.parent().unwrap()).unwrap();
    std::fs::write(&local, "").unwrap();

    let stored = PathBuf::from(
        "/home/user/.local/share/warp-terminal/themes/catppuccin/catppuccin_latte.yml",
    );

    assert_eq!(custom_theme_path_from_storage(&stored, &root), local);
}

#[test]
fn custom_theme_unmatched_legacy_absolute_path_is_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join(".warp/themes");
    let stored = PathBuf::from("/Users/ivan/.warp/themes/missing.yml");

    assert_eq!(custom_theme_path_from_storage(&stored, &root), stored);
}

#[test]
fn custom_theme_settings_value_writes_portable_path_for_theme_root_file() {
    use settings_value::SettingsValue as _;

    let root_path = crate::user_config::themes_dir().join("my_theme.yml");
    let custom = CustomTheme::new("My Theme".to_string(), root_path);

    let value = custom.to_file_value();

    assert_eq!(value["name"], "My Theme");
    assert_eq!(value["path"], "my_theme.yml");
}

#[test]
fn custom_theme_serde_writes_portable_path_for_theme_root_file() {
    let root_path = crate::user_config::themes_dir().join("my_theme.yml");
    let custom = CustomTheme::new("My Theme".to_string(), root_path);

    let value = serde_json::to_value(custom).unwrap();

    assert_eq!(value["name"], "My Theme");
    assert_eq!(value["path"], "my_theme.yml");
}

#[test]
fn custom_base16_theme_kind_uses_custom_theme_settings_value_path_rules() {
    use settings_value::SettingsValue as _;

    let root_path = crate::user_config::themes_dir().join("base16/ocean.yml");
    let kind = ThemeKind::CustomBase16(CustomTheme::new("Base16 Ocean".to_string(), root_path));

    assert_eq!(
        kind.to_file_value(),
        serde_json::json!({
            "custom_base_16": {
                "name": "Base16 Ocean",
                "path": "base16/ocean.yml"
            }
        })
    );
}

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
