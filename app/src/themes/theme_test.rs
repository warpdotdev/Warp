use super::*;
use crate::util::color::OPAQUE;
use settings_value::SettingsValue as _;

#[test]
#[cfg(not(target_family = "wasm"))]
fn custom_theme_tilde_path_expansion_test() {
    use dirs::home_dir;

    let home = home_dir().expect("home dir must exist for this test");

    // A Custom ThemeKind stored in JSON (as used in settings) with a tilde path.
    let json = serde_json::json!({
        "Custom": {
            "name": "My Theme",
            "path": "~/.warp/themes/my_theme.yaml"
        }
    });

    let theme_kind: ThemeKind = serde_json::from_value(json).expect("should deserialize");

    let expected_path = home.join(".warp/themes/my_theme.yaml");
    match theme_kind {
        ThemeKind::Custom(custom) => {
            assert_eq!(
                custom.path(),
                expected_path,
                "tilde should be expanded to home dir"
            );
        }
        other => panic!("expected ThemeKind::Custom, got {other:?}"),
    }
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn custom_theme_tilde_path_expansion_via_settings_value_test() {
    use dirs::home_dir;

    let home = home_dir().expect("home dir must exist for this test");

    // Simulate loading from the TOML settings file via the SettingsValue path
    // (which bypasses serde and previously skipped tilde expansion).
    let file_value = serde_json::json!({
        "name": "My Theme",
        "path": "~/.warp/themes/my_theme.yaml"
    });

    let custom = CustomTheme::from_file_value(&file_value)
        .expect("SettingsValue::from_file_value should succeed");

    let expected_path = home.join(".warp/themes/my_theme.yaml");
    assert_eq!(
        custom.path(),
        expected_path,
        "tilde should be expanded via SettingsValue::from_file_value"
    );
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn custom_theme_absolute_path_unchanged_test() {
    let json = serde_json::json!({
        "Custom": {
            "name": "My Theme",
            "path": "/absolute/path/to/theme.yaml"
        }
    });

    let theme_kind: ThemeKind = serde_json::from_value(json).expect("should deserialize");

    match theme_kind {
        ThemeKind::Custom(custom) => {
            assert_eq!(
                custom.path(),
                std::path::PathBuf::from("/absolute/path/to/theme.yaml"),
                "absolute path should be unchanged"
            );
        }
        other => panic!("expected ThemeKind::Custom, got {other:?}"),
    }
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn custom_theme_to_file_value_uses_tilde_test() {
    use dirs::home_dir;

    let home = home_dir().expect("home dir must exist for this test");

    // Build a CustomTheme with an absolute path under the home dir.
    let absolute_path = home.join(".warp/themes/my_theme.yaml");
    let custom = CustomTheme::new("My Theme".to_string(), absolute_path);

    // to_file_value should store the path with ~ so settings.toml stays portable.
    let file_value = settings_value::SettingsValue::to_file_value(&custom);
    let path_in_file = file_value["path"]
        .as_str()
        .expect("path should be a string");

    // Normalize path separators to forward slashes for cross-platform comparison
    let normalized_path_in_file = path_in_file.replace('\\', "/");

    assert!(
        normalized_path_in_file.starts_with("~/"),
        "path in settings file should start with '~/', got: {normalized_path_in_file}"
    );
    assert_eq!(normalized_path_in_file, "~/.warp/themes/my_theme.yaml");
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn custom_theme_settings_value_round_trip_test() {
    use dirs::home_dir;

    let home = home_dir().expect("home dir must exist for this test");

    // Start with an absolute path.
    let absolute_path = home.join(".warp/themes/my_theme.yaml");
    let original = CustomTheme::new("My Theme".to_string(), absolute_path.clone());

    // Serialize → deserialize via SettingsValue (the settings file code path).
    let file_value = settings_value::SettingsValue::to_file_value(&original);
    let restored = CustomTheme::from_file_value(&file_value)
        .expect("round-trip via SettingsValue should succeed");

    // The restored path should be the expanded absolute path, not a tilde path.
    assert_eq!(restored.path(), absolute_path);
    assert_eq!(restored.name(), original.name());
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
