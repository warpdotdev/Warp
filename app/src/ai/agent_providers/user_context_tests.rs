use super::*;
use crate::ai::agent::ImageContext;
use crate::ai::block_context::BlockContext;
use ai::agent::action_result::{AnyFileContent, FileContext};
use std::collections::HashMap;

fn block(id: &str, command: &str, output: &str, exit_code: i32, auto: bool) -> BlockContext {
    BlockContext {
        id: id.to_string().into(),
        index: 0.into(),
        command: command.to_string(),
        output: output.to_string(),
        exit_code: exit_code.into(),
        is_auto_attached: auto,
        started_ts: None,
        finished_ts: None,
        pwd: None,
        shell: None,
        username: None,
        hostname: None,
        git_branch: None,
        os: None,
        session_id: None,
    }
}

#[test]
fn empty_context_returns_none() {
    assert!(render_user_attachments(&[]).is_none());
}

#[test]
fn only_environment_context_returns_none() {
    let ctx = vec![
        AIAgentContext::Directory {
            pwd: Some("/x".into()),
            home_dir: None,
            are_file_symbols_indexed: false,
        },
        AIAgentContext::Git {
            head: "abc".into(),
            branch: Some("main".into()),
        },
    ];
    assert!(
        render_user_attachments(&ctx).is_none(),
        "环境型 context 不应进 user message"
    );
}

#[test]
fn single_block_renders_required_fields() {
    let ctx = vec![AIAgentContext::Block(Box::new(block(
        "b-1",
        "ll",
        "ll: not found",
        1,
        false,
    )))];
    let out = render_user_attachments(&ctx).expect("应当渲染");
    assert!(out.starts_with("<attached_context>"));
    assert!(out.ends_with("</attached_context>"));
    assert!(out.contains("command_id=\"b-1\""));
    assert!(out.contains("exit_code=\"1\""));
    assert!(out.contains("auto_attached=\"false\""));
    assert!(out.contains("<command>ll</command>"));
    assert!(out.contains("<output>ll: not found</output>"));
}

#[test]
fn auto_attached_block_marks_attribute() {
    let ctx = vec![AIAgentContext::Block(Box::new(block(
        "b-2", "echo hi", "hi", 0, true,
    )))];
    let out = render_user_attachments(&ctx).unwrap();
    assert!(out.contains("auto_attached=\"true\""));
}

#[test]
fn multiple_blocks_in_attach_order() {
    let ctx = vec![
        AIAgentContext::Block(Box::new(block("first", "a", "1", 0, false))),
        AIAgentContext::Block(Box::new(block("second", "b", "2", 0, true))),
    ];
    let out = render_user_attachments(&ctx).unwrap();
    let p1 = out.find("command_id=\"first\"").unwrap();
    let p2 = out.find("command_id=\"second\"").unwrap();
    assert!(p1 < p2, "should preserve attach order");
}

#[test]
fn selected_text_and_block_mixed() {
    let ctx = vec![
        AIAgentContext::Block(Box::new(block("b", "x", "y", 0, false))),
        AIAgentContext::SelectedText("hello world".into()),
    ];
    let out = render_user_attachments(&ctx).unwrap();
    assert!(out.contains("<executed_shell_command"));
    assert!(out.contains("<selected_text>hello world</selected_text>"));
}

#[test]
fn xml_special_chars_escaped() {
    let ctx = vec![AIAgentContext::Block(Box::new(block(
        "b",
        "echo <hi>",
        "a & b",
        0,
        false,
    )))];
    let out = render_user_attachments(&ctx).unwrap();
    assert!(out.contains("<command>echo &lt;hi&gt;</command>"));
    assert!(out.contains("<output>a &amp; b</output>"));
}

#[test]
fn file_string_content_renders_path_and_body() {
    let f = FileContext::new(
        "src/foo.rs".into(),
        AnyFileContent::StringContent("fn main() {}\n".into()),
        None,
        None,
    );
    let ctx = vec![AIAgentContext::File(f)];
    let out = render_user_attachments(&ctx).unwrap();
    assert!(out.contains("<file path=\"src/foo.rs\""));
    assert!(out.contains("fn main() {}"));
    assert!(out.contains("</file>"));
}

#[test]
fn file_with_line_range_emits_attributes() {
    let f = FileContext::new(
        "a.rs".into(),
        AnyFileContent::StringContent("line1\nline2\n".into()),
        Some(10..20),
        None,
    );
    let out = render_user_attachments(&[AIAgentContext::File(f)]).unwrap();
    assert!(out.contains("line_start=\"10\""));
    assert!(out.contains("line_end=\"20\""));
}

#[test]
fn file_binary_content_self_closing_with_size() {
    let f = FileContext::new(
        "logo.png".into(),
        AnyFileContent::BinaryContent(vec![0u8; 42]),
        None,
        None,
    );
    let out = render_user_attachments(&[AIAgentContext::File(f)]).unwrap();
    // 新版 placeholder:path + mime_type + binary + size,缺一不可。
    assert!(out.contains("path=\"logo.png\""));
    assert!(out.contains("mime_type=\"image/png\""));
    assert!(out.contains("binary=\"true\""));
    assert!(out.contains("size=\"42\""));
}

#[test]
fn file_binary_empty_omits_size_attr() {
    // 上层故意没读 bytes(.exe / 超大文件)→ size 属性应被省略,但 path / mime / binary 必存
    let f = FileContext::new(
        "C:\\Users\\me\\WarpSetup.exe".into(),
        AnyFileContent::BinaryContent(Vec::new()),
        None,
        None,
    );
    let out = render_user_attachments(&[AIAgentContext::File(f)]).unwrap();
    assert!(out.contains("path=\"C:\\Users\\me\\WarpSetup.exe\""));
    assert!(out.contains("binary=\"true\""));
    assert!(
        !out.contains("size="),
        "空 BinaryContent 不应输出 size 属性"
    );
    // .exe 默认 mime 是 application/vnd.microsoft.portable-executable 或 octet-stream,
    // 不强 assert 具体值,只验证 mime_type 属性存在
    assert!(out.contains("mime_type=\""));
}

#[test]
fn image_renders_placeholder_only() {
    let img = ImageContext {
        data: "BASE64DATA".into(),
        mime_type: "image/png".into(),
        file_name: "shot.png".into(),
        is_figma: false,
    };
    let out = render_user_attachments(&[AIAgentContext::Image(img)]).unwrap();
    assert!(out.contains("<image file_name=\"shot.png\" mime_type=\"image/png\" />"));
    assert!(
        !out.contains("BASE64DATA"),
        "首版不应内联 base64,避免上下文被填爆"
    );
}

#[test]
fn referenced_notebook_renders_full_payload() {
    let mut attachments = HashMap::new();
    attachments.insert(
        "@base".to_string(),
        AIAgentAttachment::DriveObject {
            uid: "Client-1".to_string(),
            payload: Some(DriveObjectPayload::Notebook {
                title: "base".to_string(),
                content: "base prompt 内容".to_string(),
            }),
        },
    );

    let out = render_referenced_attachments(&attachments).expect("应当渲染");
    assert!(out.contains("<attached_context>"));
    assert!(out.contains("reference=\"@base\""));
    assert!(out.contains("uid=\"Client-1\""));
    assert!(out.contains("type=\"notebook\""));
    assert!(out.contains("<title>\nbase\n    </title>"));
    assert!(out.contains("base prompt 内容"));
}

#[test]
fn referenced_document_content_renders_full_payload() {
    let mut attachments = HashMap::new();
    attachments.insert(
        "@plan".to_string(),
        AIAgentAttachment::DocumentContent {
            document_id: "doc-1".to_string(),
            content: "plan body".to_string(),
            source: crate::ai::agent::DocumentContentAttachmentSource::UserAttached,
            line_range: None,
        },
    );

    let out = render_referenced_attachments(&attachments).expect("应当渲染");
    assert!(out.contains("<document_content"));
    assert!(out.contains("reference=\"@plan\""));
    assert!(out.contains("document_id=\"doc-1\""));
    assert!(out.contains("plan body"));
}
