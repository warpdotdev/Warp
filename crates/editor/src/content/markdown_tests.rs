use std::sync::Arc;

use markdown_parser::{compute_formatted_text_delta, parse_markdown};
use serde_yaml::Value;
use string_offset::CharOffset;
use vec1::Vec1;
use warpui::{App, ReadModel};

use crate::content::{
    buffer::{
        Buffer, BufferEditAction, EditOrigin, StyledBlockBoundaryBehavior, tests::TestEmbeddedItem,
    },
    text::{IndentBehavior, TABLE_BLOCK_MARKDOWN_LANG},
};

use super::MarkdownStyle;

#[test]
fn test_export_normalizes_code_languages() {
    let formatted = parse_markdown(
        r#"
```JavaScript
console.log("Hello, World");
```
```Rust
println!("Hello, World");
```
```ocaml
print_endline "Hello, World!"
```
"#,
    )
    .unwrap();
    let exported = Buffer::export_to_markdown(
        formatted,
        None,
        MarkdownStyle::Export {
            app_context: None,
            should_not_escape_markdown_punctuation: false,
        },
    );

    // Exporting should use external code languages.
    assert_eq!(
        exported,
        r#"
```js
console.log("Hello, World");
```
```rust
println!("Hello, World");
```
```ocaml
print_endline "Hello, World!"
```
"#
    );
}

#[test]
fn test_mermaid_markdown_round_trip() {
    App::test((), |mut app| async move {
        let _flag = warp_core::features::FeatureFlag::MarkdownMermaid.override_enabled(true);
        let markdown = "```mermaid\ngraph TD\nA --> B\n```\n";
        let (buffer, _selection) = Buffer::mock_from_markdown(
            markdown,
            None,
            Box::new(|_, _| IndentBehavior::Ignore),
            &mut app,
        );

        let internal_markdown = app.read_model(&buffer, |buffer, _| buffer.markdown());
        assert_eq!(internal_markdown, markdown);

        let exported_markdown = app.read_model(&buffer, |buffer, _| buffer.markdown_unescaped());
        assert_eq!(exported_markdown, markdown);
    });
}

#[test]
fn test_export_expands_embeds() {
    // This tests styled block for the edge case of querying just the
    // leading block item (0..1).
    App::test((), |mut app| async move {
        let (buffer, _selection) = Buffer::mock_from_markdown(
            r#"
```warp-embedded-object
id: embed-123
```
```warp-embedded-object
id: embed-456
ignored: value
```"#,
            Some(|mut mapping| match mapping.remove(&"id".into()) {
                Some(Value::String(id)) => Some(Arc::new(TestEmbeddedItem { id })),
                _ => None,
            }),
            Box::new(|_, _| IndentBehavior::Ignore),
            &mut app,
        );

        let exported = app.read_model(&buffer, |buffer, _| {
            buffer.to_markdown(MarkdownStyle::Export {
                app_context: None,
                should_not_escape_markdown_punctuation: false,
            })
        });

        // Exporting should expand the embedded objects.
        assert_eq!(
            exported,
            r#"
```warp-embedded-object
---
id: embed-123
export: true

```
```warp-embedded-object
---
id: embed-456
export: true

```
"#
        );
    });
}

#[test]
fn test_table_html_serialization() {
    App::test((), |mut app| async move {
        let markdown = format!(
            "```{}\nheader 1\theader 2\nvalue 1\tvalue 2\n```\n",
            TABLE_BLOCK_MARKDOWN_LANG
        );
        let (buffer, _selection) = Buffer::mock_from_markdown(
            &markdown,
            None,
            Box::new(|_, _| IndentBehavior::Ignore),
            &mut app,
        );

        let html = app.read_model(&buffer, |buffer, ctx| {
            let range = CharOffset::from(1)..buffer.max_charoffset();
            buffer.ranges_as_html(Vec1::try_from_vec(vec![range]).unwrap(), ctx)
        });

        assert!(html.is_some());
        let html = html.unwrap();
        assert!(html.contains(
            "<table><thead><tr><th align=\"left\">header 1</th><th align=\"left\">header 2</th></tr></thead><tbody><tr><td align=\"left\">value 1</td><td align=\"left\">value 2</td></tr></tbody></table>"
        ));
    });
}

#[test]
fn test_gfm_table_html_serialization() {
    App::test((), |mut app| async move {
        let _flag = warp_core::features::FeatureFlag::MarkdownTables.override_enabled(true);
        let markdown = "\
| header 1 | header 2 |\n\
| --- | --- |\n\
| value 1 | value 2 |\n";
        let (buffer, _selection) = Buffer::mock_from_markdown(
            markdown,
            None,
            Box::new(|_, _| IndentBehavior::Ignore),
            &mut app,
        );

        let html = app.read_model(&buffer, |buffer, ctx| {
            let range = CharOffset::from(1)..buffer.max_charoffset();
            buffer.ranges_as_html(Vec1::try_from_vec(vec![range]).unwrap(), ctx)
        });

        assert!(html.is_some());
        let html = html.unwrap();
        assert!(html.contains(
            "<table><thead><tr><th align=\"left\">header 1</th><th align=\"left\">header 2</th></tr></thead><tbody><tr><td align=\"left\">value 1</td><td align=\"left\">value 2</td></tr></tbody></table>"
        ));
    });
}

#[test]
fn test_apply_formatted_text_delta_append() {
    App::test((), |mut app| async move {
        let old_markdown = "hello world\n";
        let (buffer, selection) = Buffer::mock_from_markdown(
            old_markdown,
            None,
            Box::new(|_, _| IndentBehavior::Ignore),
            &mut app,
        );

        // Buffer::mock_from_markdown removes the trailing newline, so add it back.
        buffer.update(&mut app, |buffer, ctx| {
            let end_offset = buffer.max_charoffset();
            let edits =
                Vec1::try_from_vec(vec![("\n".to_string(), end_offset..end_offset)]).unwrap();
            buffer.update_content(
                BufferEditAction::InsertAtCharOffsetRanges { edits: &edits },
                EditOrigin::SystemEdit,
                selection.clone(),
                ctx,
            );
        });

        let (exported, old_formatted) = app.read_model(&buffer, |buffer, _| {
            let old_formatted = buffer.range_to_formatted_text(
                CharOffset::from(1)..buffer.max_charoffset(),
                StyledBlockBoundaryBehavior::Exclusive,
            );
            (buffer.markdown_unescaped(), old_formatted)
        });

        assert_eq!(exported, "hello world\n");

        let new_markdown = "hello world\n#";
        let new_formatted = parse_markdown(new_markdown).unwrap();
        let delta = compute_formatted_text_delta(old_formatted, new_formatted.clone());
        // Should just be appending a new line
        assert_eq!(delta.common_prefix_lines, 1);
        // There's a trailing linebreak being replaced
        assert_eq!(delta.old_suffix_formatted_text_lines, 1);
        assert_eq!(delta.new_suffix.len(), 1);
        buffer.update(&mut app, |buffer, ctx| {
            buffer.apply_formatted_text_delta(&delta, selection.clone(), ctx);
        });

        let (exported, formatted_in_buffer) = app.read_model(&buffer, |buffer, _| {
            let new_formatted = buffer.range_to_formatted_text(
                CharOffset::from(1)..buffer.max_charoffset(),
                StyledBlockBoundaryBehavior::Exclusive,
            );
            (buffer.markdown_unescaped(), new_formatted)
        });

        assert_eq!(exported, new_markdown);
        assert_eq!(new_formatted, formatted_in_buffer);

        let new_markdown_2 = "hello world\n# This is a heading";
        let new_formatted_2 = parse_markdown(new_markdown_2).unwrap();
        let delta_2 = compute_formatted_text_delta(new_formatted, new_formatted_2.clone());
        // Should be replacing the # line while keeping the hello world line
        assert_eq!(delta_2.common_prefix_lines, 1);
        assert_eq!(delta_2.old_suffix_formatted_text_lines, 1);
        assert_eq!(delta_2.new_suffix.len(), 1);
        buffer.update(&mut app, |buffer, ctx| {
            buffer.apply_formatted_text_delta(&delta_2, selection.clone(), ctx);
        });

        let (exported, formatted_in_buffer) = app.read_model(&buffer, |buffer, _| {
            let new_formatted = buffer.range_to_formatted_text(
                CharOffset::from(1)..buffer.max_charoffset(),
                StyledBlockBoundaryBehavior::Exclusive,
            );
            (buffer.markdown_unescaped(), new_formatted)
        });

        // We add a trailing newline
        assert_eq!(exported.trim_end(), new_markdown_2);
        assert_eq!(new_formatted_2, formatted_in_buffer);
    });
}

#[test]
fn test_image_html_serialization() {
    App::test((), |mut app| async move {
        let markdown = "![Alt text](image.png)\n";
        let (buffer, _selection) = Buffer::mock_from_markdown(
            markdown,
            None,
            Box::new(|_, _| IndentBehavior::Ignore),
            &mut app,
        );

        let html = app.read_model(&buffer, |buffer, ctx| {
            let range = CharOffset::from(1)..buffer.max_charoffset();
            buffer.ranges_as_html(Vec1::try_from_vec(vec![range]).unwrap(), ctx)
        });

        // Image should be serialized as <img src="image.png" alt="Alt text" />
        assert!(html.is_some());
        let html = html.unwrap();
        assert!(html.contains("<img"));
        assert!(html.contains("src=\"image.png\""));
        assert!(html.contains("alt=\"Alt text\""));
    });
}

#[test]
fn test_multiple_images_html_serialization() {
    App::test((), |mut app| async move {
        let markdown = "![First](./path/img1.jpg)\n![Second](https://example.com/img2.png)\n";
        let (buffer, _selection) = Buffer::mock_from_markdown(
            markdown,
            None,
            Box::new(|_, _| IndentBehavior::Ignore),
            &mut app,
        );

        let html = app.read_model(&buffer, |buffer, ctx| {
            let range = CharOffset::from(1)..buffer.max_charoffset();
            buffer.ranges_as_html(Vec1::try_from_vec(vec![range]).unwrap(), ctx)
        });

        // Check both images are in the HTML
        assert!(html.is_some());
        let html = html.unwrap();
        assert!(html.contains("src=\"./path/img1.jpg\""));
        assert!(html.contains("alt=\"First\""));
        assert!(html.contains("src=\"https://example.com/img2.png\""));
        assert!(html.contains("alt=\"Second\""));
    });
}

#[test]
fn test_table_markdown_round_trip() {
    App::test((), |mut app| async move {
        let markdown = format!(
            "```{}\nheader 1\theader 2\nvalue 1\tvalue 2\n```\n",
            TABLE_BLOCK_MARKDOWN_LANG
        );
        let (buffer, _selection) = Buffer::mock_from_markdown(
            &markdown,
            None,
            Box::new(|_, _| IndentBehavior::Ignore),
            &mut app,
        );
        let internal_markdown = app.read_model(&buffer, |buffer, _| buffer.markdown());
        assert_eq!(internal_markdown, markdown);

        let exported_markdown = app.read_model(&buffer, |buffer, _| buffer.markdown_unescaped());
        assert_eq!(
            exported_markdown,
            "| header 1 | header 2 |\n| --- | --- |\n| value 1 | value 2 |\n"
        );
    });
}

#[test]
fn test_table_markdown_export_escapes_pipe_characters() {
    App::test((), |mut app| async move {
        let markdown = format!(
            "```{}\nhead|er 1\theader 2\nvalue | 1\tvalue 2\n```\n",
            TABLE_BLOCK_MARKDOWN_LANG
        );
        let (buffer, _selection) = Buffer::mock_from_markdown(
            &markdown,
            None,
            Box::new(|_, _| IndentBehavior::Ignore),
            &mut app,
        );

        let exported_markdown = app.read_model(&buffer, |buffer, _| buffer.markdown_unescaped());
        assert_eq!(
            exported_markdown,
            "| head\\|er 1 | header 2 |\n| --- | --- |\n| value \\| 1 | value 2 |\n"
        );
    });
}

#[test]
fn test_image_with_content_html_serialization() {
    App::test((), |mut app| async move {
        let markdown = "# Header\n\n![Image](test.png)\n\nSome text\n";
        let (buffer, _selection) = Buffer::mock_from_markdown(
            markdown,
            None,
            Box::new(|_, _| IndentBehavior::Ignore),
            &mut app,
        );

        let html = app.read_model(&buffer, |buffer, ctx| {
            let range = CharOffset::from(1)..buffer.max_charoffset();
            buffer.ranges_as_html(Vec1::try_from_vec(vec![range]).unwrap(), ctx)
        });

        // Check that header, image, and text are all present
        assert!(html.is_some());
        let html = html.unwrap();
        assert!(html.contains("<h1>"));
        assert!(html.contains("Header"));
        assert!(html.contains("<img"));
        assert!(html.contains("src=\"test.png\""));
        assert!(html.contains("Some text"));
    });
}
