use super::*;

use itertools::Itertools;

#[test]
fn test_possible_file_paths_in_word() {
    let word = "/path/to/file:16:hello";
    let possible_paths = possible_file_paths_in_word(word).collect_vec();
    assert_eq!(
        possible_paths,
        vec![
            "/path/to/file:16:hello",
            "/path/to/file:16",
            "/path/to/file",
            "16:hello",
            "hello",
            "16"
        ]
    );

    let word = "/path/to/file:162:47.";
    let possible_paths = possible_file_paths_in_word(word).collect_vec();
    assert_eq!(
        possible_paths,
        vec![
            "/path/to/file:162:47.",
            "/path/to/file:162:47",
            "/path/to/file:162",
            "/path/to/file",
            "162:47.",
            "162:47",
            "162",
            "47.",
            "47"
        ]
    );

    let word = "<Cargo.toml:16:4>";
    let possible_paths = possible_file_paths_in_word(word).collect_vec();
    assert_eq!(
        possible_paths,
        vec![
            "<Cargo.toml:16:4>",
            "<Cargo.toml:16:4",
            "Cargo.toml:16:4>",
            "Cargo.toml:16:4",
            "<Cargo.toml:16",
            "Cargo.toml:16",
            "<Cargo.toml",
            "Cargo.toml",
            "16:4>",
            "16:4",
            "16",
            "4>",
            "4"
        ]
    );
}

#[test]
fn test_possible_file_paths_in_word_multibyte() {
    let word = "/path/音楽/テストファイル.txt:16:ḧeĹḹo";
    let possible_paths = possible_file_paths_in_word(word).collect_vec();
    assert_eq!(
        possible_paths,
        vec![
            "/path/音楽/テストファイル.txt:16:ḧeĹḹo",
            "/path/音楽/テストファイル.txt:16",
            "/path/音楽/テストファイル.txt",
            "16:ḧeĹḹo",
            "ḧeĹḹo",
            "16"
        ]
    );
}
#[test]
fn test_detect_wrapped_urls_across_lines_registers_full_url_per_line_segment() {
    let section_index = 7;
    let lines = vec![
        (
            0,
            "See https://example.com/some/very/long/path?param1=value1&param2=va\n".to_string(),
        ),
        (1, "lue2&param3=value3 for details\n".to_string()),
    ];

    let mut wrapped = detect_wrapped_urls_across_lines(section_index, &lines);
    wrapped.sort_by_key(|(location, _)| match location {
        TextLocation::Output { line_index, .. } => *line_index,
        _ => usize::MAX,
    });

    assert_eq!(wrapped.len(), 2);

    let expected_url =
        "https://example.com/some/very/long/path?param1=value1&param2=value2&param3=value3";

    assert_eq!(
        wrapped[0],
        (
            TextLocation::Output {
                section_index,
                line_index: 0,
            },
            vec![(4..67, expected_url.to_string())]
        )
    );

    assert_eq!(
        wrapped[1],
        (
            TextLocation::Output {
                section_index,
                line_index: 1,
            },
            vec![(0..18, expected_url.to_string())]
        )
    );
}

#[test]
fn test_detect_wrapped_urls_across_lines_ignores_single_line_urls() {
    let section_index = 1;
    let lines = vec![
        (0, "Visit https://example.com/foo\n".to_string()),
        (1, "next line\n".to_string()),
    ];

    let wrapped = detect_wrapped_urls_across_lines(section_index, &lines);
    assert!(wrapped.is_empty());
}

#[test]
fn test_detect_wrapped_urls_across_lines_allows_query_value_continuation() {
    let section_index = 1;
    let lines = vec![
        (0, "Visit https://example.com/foo?token=abc\n".to_string()),
        (1, "def for details\n".to_string()),
    ];

    let wrapped = detect_wrapped_urls_across_lines(section_index, &lines);

    assert_eq!(wrapped.len(), 2);
    for (_, entries) in wrapped {
        assert_eq!(
            entries[0].1,
            "https://example.com/foo?token=abcdef".to_string()
        );
    }
}
