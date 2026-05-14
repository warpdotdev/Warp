//! 把单条 `AIAgentInput::UserQuery` 自带的附件类 `AIAgentContext` 渲染为
//! 发往上游模型的 user message 内容(text 前缀 + binary 多模态部件)。
//!
//! ## 与 warp 自家路径的对齐
//!
//! warp 自家协议下,这些附件走 `api::InputContext` 的 `executed_shell_commands /
//! selected_text / files / images` 字段(见 `app/src/ai/agent/api/convert_to.rs`
//! `convert_context`)。BYOP 直接对接 OpenAI / Anthropic / Gemini / Ollama 兼容
//! `/chat/completions`,没有 `InputContext` 这层结构,只能把数据嵌进 user message。
//!
//! 字段严格对齐 warp protobuf,不引入协议外字段:
//!
//! | 类型             | warp protobuf 字段                                          |
//! |------------------|-------------------------------------------------------------|
//! | `Block`          | command / output / exit_code / command_id / is_auto_attached / started_ts / finished_ts |
//! | `SelectedText`   | text                                                        |
//! | `File`(text)    | file_name / content / line_range                            |
//! | `File`(binary)  | file_name / data / mime_type(P1 binary 通道)              |
//! | `Image`          | mime_type / file_name / data(P1 binary 通道)              |
//!
//! ## 作用域:per-input,不影响 system prompt
//!
//! - 这些附件只注入 user message,不进 system prompt
//! - 历史轮的 `referenced_attachments` 会从持久化 message 重新渲染,避免 BYOP 多轮丢上下文
//! - env / git / skills / project_rules / codebase / current_time 这些**环境型** context
//!   仍由 `prompt_renderer` 渲染进 system,与本模块互不重叠

use std::collections::HashMap;

use base64::Engine;

use crate::ai::agent::{AIAgentAttachment, AIAgentContext, DriveObjectPayload, ImageContext};
use crate::ai::block_context::BlockContext;
use ai::agent::action_result::{AnyFileContent, FileContext};
use warp_multi_agent_api as api;

/// `collect_user_attachments` 返回的双通道结果。
///
/// - `prefix`: 文本前缀块,prepend 到 user message text。包含 block / selected_text /
///   text-like file 的内联 XML,以及 binary 附件的占位提示(让 LLM 能引用文件名)。
/// - `binaries`: 需要作为 `ContentPart::Binary` 注入到多模态 message 的附件
///   (image / PDF / audio)。caller(chat_stream.rs)会按 model capability 过滤后
///   决定是否切到 `MessageContent::Parts`。
#[derive(Debug, Default, Clone)]
pub struct UserAttachments {
    pub prefix: Option<String>,
    pub binaries: Vec<UserBinary>,
}

/// 一条 binary 附件,等价于 genai `Binary::from_base64` 的输入三元组。
#[derive(Debug, Clone)]
pub struct UserBinary {
    pub name: String,
    pub content_type: String,
    /// base64 编码后的数据(无 `data:` 前缀)。
    pub data: String,
}

impl UserAttachments {
    pub fn is_empty(&self) -> bool {
        self.prefix.is_none() && self.binaries.is_empty()
    }
}

/// 渲染单条 UserQuery 的附件 context 为「文本前缀 + binary 部件」。
///
/// 调用方应:
/// 1. 把 `prefix` prepend 到 user query 文本前(中间留空行)
/// 2. 按 model capability 过滤 `binaries`,有保留时切 `MessageContent::Parts`
pub fn collect_user_attachments(ctx: &[AIAgentContext]) -> UserAttachments {
    let mut blocks: Vec<&BlockContext> = Vec::new();
    let mut selected_texts: Vec<&str> = Vec::new();
    let mut text_files: Vec<&FileContext> = Vec::new();
    let mut binary_files: Vec<&FileContext> = Vec::new();
    let mut images: Vec<&ImageContext> = Vec::new();

    for c in ctx {
        match c {
            AIAgentContext::Block(b) => blocks.push(b),
            AIAgentContext::SelectedText(t) => selected_texts.push(t),
            AIAgentContext::File(f) => match &f.content {
                AnyFileContent::StringContent(_) => text_files.push(f),
                AnyFileContent::BinaryContent(_) => binary_files.push(f),
            },
            AIAgentContext::Image(img) => images.push(img),
            // 环境型 context 由 prompt_renderer 处理,不进 user message。
            AIAgentContext::Directory { .. }
            | AIAgentContext::ExecutionEnvironment(_)
            | AIAgentContext::CurrentTime { .. }
            | AIAgentContext::Codebase { .. }
            | AIAgentContext::ProjectRules { .. }
            | AIAgentContext::Git { .. }
            | AIAgentContext::Skills { .. } => {}
        }
    }

    let mut out = UserAttachments::default();

    // ----- prefix -----
    let has_any_prefix_content = !blocks.is_empty()
        || !selected_texts.is_empty()
        || !text_files.is_empty()
        || !binary_files.is_empty()
        || !images.is_empty();
    if has_any_prefix_content {
        let mut prefix = String::with_capacity(256);
        prefix.push_str("<attached_context>\n");
        for b in &blocks {
            render_block(&mut prefix, b);
        }
        for t in &selected_texts {
            render_selected_text(&mut prefix, t);
        }
        for f in &text_files {
            render_file_text(&mut prefix, f);
        }
        for f in &binary_files {
            render_file_binary_placeholder(&mut prefix, f);
        }
        for img in &images {
            render_image_placeholder(&mut prefix, img);
        }
        prefix.push_str("</attached_context>");
        out.prefix = Some(prefix);
    }

    // ----- binaries(供 caller 按 capability 过滤后注入 ContentPart::Binary) -----
    for img in &images {
        out.binaries.push(UserBinary {
            name: img.file_name.clone(),
            content_type: img.mime_type.clone(),
            // ImageContext.data 已经是 base64 字符串(`process_non_image_files` 兄弟路径
            // `read_and_process_images_async` 在 PendingAttachment::Image 入队时就完成了 encoding)
            data: img.data.to_string(),
        });
    }
    for f in &binary_files {
        if let AnyFileContent::BinaryContent(bytes) = &f.content {
            // 空 BinaryContent 表示"上层故意没读 bytes"(.exe / 超大文件 / 非多模态
            // mime),只走 placeholder XML 路径,不送 ContentPart::Binary。
            if bytes.is_empty() {
                continue;
            }
            let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
            // 用 file_name 上的扩展名猜 mime;`mime_guess` 与 process_non_image_files
            // 走的是同一套规则,这里再算一遍是因为 FileContext 不保存 mime。
            let mime = mime_guess::from_path(&f.file_name)
                .first_or_octet_stream()
                .to_string();
            out.binaries.push(UserBinary {
                name: f.file_name.clone(),
                content_type: mime,
                data: b64,
            });
        }
    }

    out
}

pub fn render_referenced_attachments(
    referenced_attachments: &HashMap<String, AIAgentAttachment>,
) -> Option<String> {
    if referenced_attachments.is_empty() {
        return None;
    }

    let mut refs = referenced_attachments.iter().collect::<Vec<_>>();
    refs.sort_by(|(left, _), (right, _)| left.cmp(right));

    let mut out = String::with_capacity(256);
    out.push_str("<attached_context>\n");
    for (reference, attachment) in refs {
        render_attachment(&mut out, reference, attachment);
    }
    out.push_str("</attached_context>");
    Some(out)
}

pub fn render_api_referenced_attachments(
    referenced_attachments: &HashMap<String, api::Attachment>,
) -> Option<String> {
    if referenced_attachments.is_empty() {
        return None;
    }

    let mut refs = referenced_attachments.iter().collect::<Vec<_>>();
    refs.sort_by(|(left, _), (right, _)| left.cmp(right));

    let mut out = String::with_capacity(256);
    out.push_str("<attached_context>\n");
    for (reference, attachment) in refs {
        render_api_attachment(&mut out, reference, attachment);
    }
    out.push_str("</attached_context>");
    Some(out)
}

/// 兼容旧调用方:仅取 prefix 文本。新代码请用 `collect_user_attachments`。
#[cfg(test)]
pub fn render_user_attachments(ctx: &[AIAgentContext]) -> Option<String> {
    collect_user_attachments(ctx).prefix
}

// ---------------------------------------------------------------------------
// 子渲染器
// ---------------------------------------------------------------------------

fn render_block(out: &mut String, b: &BlockContext) {
    use std::fmt::Write;
    let _ = write!(
        out,
        "  <executed_shell_command command_id=\"{}\" exit_code=\"{}\" auto_attached=\"{}\"",
        xml_attr(&String::from(b.id.clone())),
        b.exit_code.value(),
        b.is_auto_attached
    );
    if let Some(ts) = b.started_ts {
        let _ = write!(out, " started_ts=\"{}\"", ts.to_rfc3339());
    }
    if let Some(ts) = b.finished_ts {
        let _ = write!(out, " finished_ts=\"{}\"", ts.to_rfc3339());
    }
    out.push_str(">\n");
    out.push_str("    <command>");
    out.push_str(&xml_text(&b.command));
    out.push_str("</command>\n");
    out.push_str("    <output>");
    out.push_str(&xml_text(&b.output));
    out.push_str("</output>\n");
    out.push_str("  </executed_shell_command>\n");
}

fn render_attachment(out: &mut String, reference: &str, attachment: &AIAgentAttachment) {
    match attachment {
        AIAgentAttachment::PlainText(text) => {
            render_plain_text_attachment(out, reference, text);
        }
        AIAgentAttachment::DocumentContent {
            document_id,
            content,
            line_range,
            ..
        } => {
            let line_range = line_range
                .as_ref()
                .map(|range| (range.start.as_usize(), range.end.as_usize()));
            render_document_content(out, reference, document_id, content, line_range);
        }
        AIAgentAttachment::DriveObject { uid, payload } => {
            render_drive_object(out, reference, uid, payload.as_ref());
        }
        AIAgentAttachment::DiffHunk {
            file_path,
            line_range,
            diff_content,
            lines_added,
            lines_removed,
            ..
        } => {
            use std::fmt::Write;
            let _ = writeln!(
                out,
                "  <diff_hunk reference=\"{}\" file_path=\"{}\" line_start=\"{}\" line_end=\"{}\" lines_added=\"{}\" lines_removed=\"{}\">",
                xml_attr(reference),
                xml_attr(file_path),
                line_range.start.as_usize(),
                line_range.end.as_usize(),
                lines_added,
                lines_removed,
            );
            out.push_str(&xml_text(diff_content));
            if !diff_content.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("  </diff_hunk>\n");
        }
        AIAgentAttachment::DiffSet { file_diffs, .. } => {
            use std::fmt::Write;
            let _ = writeln!(out, "  <diff_set reference=\"{}\">", xml_attr(reference));
            for (file_path, hunks) in file_diffs {
                let _ = writeln!(out, "    <file path=\"{}\">", xml_attr(file_path));
                for hunk in hunks {
                    let _ = writeln!(
                        out,
                        "      <hunk line_start=\"{}\" line_end=\"{}\" lines_added=\"{}\" lines_removed=\"{}\">",
                        hunk.line_range.start.as_usize(),
                        hunk.line_range.end.as_usize(),
                        hunk.lines_added,
                        hunk.lines_removed,
                    );
                    out.push_str(&xml_text(&hunk.diff_content));
                    if !hunk.diff_content.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str("      </hunk>\n");
                }
                out.push_str("    </file>\n");
            }
            out.push_str("  </diff_set>\n");
        }
        AIAgentAttachment::FilePathReference { file_path, .. } => {
            use std::fmt::Write;
            let _ = writeln!(
                out,
                "  <file_path_reference reference=\"{}\" path=\"{}\" />",
                xml_attr(reference),
                xml_attr(file_path),
            );
        }
        AIAgentAttachment::Block(block) => {
            render_block(out, block);
        }
    }
}

fn render_api_attachment(out: &mut String, reference: &str, attachment: &api::Attachment) {
    match attachment.value.as_ref() {
        Some(api::attachment::Value::PlainText(text)) => {
            render_plain_text_attachment(out, reference, text);
        }
        Some(api::attachment::Value::DocumentContent(document)) => {
            let line_range = document
                .line_range
                .as_ref()
                .map(|range| (range.start as usize, range.end as usize));
            render_document_content(
                out,
                reference,
                &document.document_id,
                &document.content,
                line_range,
            );
        }
        Some(api::attachment::Value::DriveObject(object)) => {
            render_api_drive_object(out, reference, object);
        }
        Some(api::attachment::Value::ExecutedShellCommand(block)) => {
            let block: BlockContext = block.clone().into();
            render_block(out, &block);
        }
        Some(api::attachment::Value::FilePathReference(file)) => {
            use std::fmt::Write;
            let _ = writeln!(
                out,
                "  <file_path_reference reference=\"{}\" path=\"{}\" />",
                xml_attr(reference),
                xml_attr(&file.file_path),
            );
        }
        _ => {}
    }
}

fn render_plain_text_attachment(out: &mut String, reference: &str, text: &str) {
    use std::fmt::Write;
    let _ = writeln!(out, "  <plain_text reference=\"{}\">", xml_attr(reference),);
    out.push_str(&xml_text(text));
    if !text.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("  </plain_text>\n");
}

fn render_document_content(
    out: &mut String,
    reference: &str,
    document_id: &str,
    content: &str,
    line_range: Option<(usize, usize)>,
) {
    use std::fmt::Write;
    let _ = write!(
        out,
        "  <document_content reference=\"{}\" document_id=\"{}\"",
        xml_attr(reference),
        xml_attr(document_id),
    );
    if let Some((line_start, line_end)) = line_range {
        let _ = write!(
            out,
            " line_start=\"{}\" line_end=\"{}\"",
            line_start, line_end,
        );
    }
    out.push_str(">\n");
    out.push_str(&xml_text(content));
    if !content.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("  </document_content>\n");
}

fn render_drive_object(
    out: &mut String,
    reference: &str,
    uid: &str,
    payload: Option<&DriveObjectPayload>,
) {
    use std::fmt::Write;
    match payload {
        Some(DriveObjectPayload::Workflow {
            name,
            description,
            command,
        }) => {
            let _ = writeln!(
                out,
                "  <drive_object reference=\"{}\" uid=\"{}\" type=\"workflow\">",
                xml_attr(reference),
                xml_attr(uid),
            );
            render_named_text(out, "name", name);
            render_named_text(out, "description", description);
            render_named_text(out, "command", command);
            out.push_str("  </drive_object>\n");
        }
        Some(DriveObjectPayload::Notebook { title, content }) => {
            let _ = writeln!(
                out,
                "  <drive_object reference=\"{}\" uid=\"{}\" type=\"notebook\">",
                xml_attr(reference),
                xml_attr(uid),
            );
            render_named_text(out, "title", title);
            render_named_text(out, "content", content);
            out.push_str("  </drive_object>\n");
        }
        Some(DriveObjectPayload::GenericStringObject {
            payload,
            object_type,
        }) => {
            let _ = writeln!(
                out,
                "  <drive_object reference=\"{}\" uid=\"{}\" type=\"{}\">",
                xml_attr(reference),
                xml_attr(uid),
                xml_attr(object_type),
            );
            render_named_text(out, "payload", payload);
            out.push_str("  </drive_object>\n");
        }
        None => {
            let _ = writeln!(
                out,
                "  <drive_object reference=\"{}\" uid=\"{}\" />",
                xml_attr(reference),
                xml_attr(uid),
            );
        }
    }
}

fn render_api_drive_object(out: &mut String, reference: &str, object: &api::DriveObject) {
    use std::fmt::Write;
    match object.object_payload.as_ref() {
        Some(api::drive_object::ObjectPayload::Workflow(workflow)) => {
            let _ = writeln!(
                out,
                "  <drive_object reference=\"{}\" uid=\"{}\" type=\"workflow\">",
                xml_attr(reference),
                xml_attr(&object.uid),
            );
            render_named_text(out, "name", &workflow.name);
            render_named_text(out, "description", &workflow.description);
            render_named_text(out, "command", &workflow.command);
            out.push_str("  </drive_object>\n");
        }
        Some(api::drive_object::ObjectPayload::Notebook(notebook)) => {
            let _ = writeln!(
                out,
                "  <drive_object reference=\"{}\" uid=\"{}\" type=\"notebook\">",
                xml_attr(reference),
                xml_attr(&object.uid),
            );
            render_named_text(out, "title", &notebook.title);
            render_named_text(out, "content", &notebook.content);
            out.push_str("  </drive_object>\n");
        }
        Some(api::drive_object::ObjectPayload::GenericStringObject(generic)) => {
            let _ = writeln!(
                out,
                "  <drive_object reference=\"{}\" uid=\"{}\" type=\"{}\">",
                xml_attr(reference),
                xml_attr(&object.uid),
                xml_attr(&generic.object_type),
            );
            render_named_text(out, "payload", &generic.payload);
            out.push_str("  </drive_object>\n");
        }
        None => {
            let _ = writeln!(
                out,
                "  <drive_object reference=\"{}\" uid=\"{}\" />",
                xml_attr(reference),
                xml_attr(&object.uid),
            );
        }
    }
}

fn render_named_text(out: &mut String, tag_name: &str, text: &str) {
    use std::fmt::Write;
    let _ = writeln!(out, "    <{tag_name}>");
    out.push_str(&xml_text(text));
    if !text.ends_with('\n') {
        out.push('\n');
    }
    let _ = writeln!(out, "    </{tag_name}>");
}

fn render_selected_text(out: &mut String, t: &str) {
    out.push_str("  <selected_text>");
    out.push_str(&xml_text(t));
    out.push_str("</selected_text>\n");
}

fn render_file_text(out: &mut String, f: &FileContext) {
    use std::fmt::Write;
    let path = xml_attr(&f.file_name);
    let content = match &f.content {
        AnyFileContent::StringContent(content) => content.as_str(),
        AnyFileContent::BinaryContent(_) => return, // shouldn't happen, dispatched away above
    };
    let _ = write!(out, "  <file path=\"{path}\"");
    if let Some(range) = &f.line_range {
        let _ = write!(
            out,
            " line_start=\"{}\" line_end=\"{}\"",
            range.start, range.end
        );
    }
    out.push_str(">\n");
    out.push_str(&xml_text(content));
    if !content.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("  </file>\n");
}

/// Binary 文件 prefix 占位:让 LLM 知道有这个文件、完整路径与 mime,可以决定:
/// - 是直接读 ContentPart::Binary(模型支持 + bytes 已上传)
/// - 还是用 read_files / shell 工具按 path 自行处理(.exe / .zip / 超大文件)
///
/// 实际 bytes 通过 caller 端 `MessageContent::Parts` 走 ContentPart::Binary,
/// 这里**不**重复贴 base64(避免双倍 token + 不少模型对超长 base64 解析慢)。
fn render_file_binary_placeholder(out: &mut String, f: &FileContext) {
    use std::fmt::Write;
    let path = xml_attr(&f.file_name);
    // mime 通过 mime_guess 从文件名/扩展名推,跟 process_non_image_files 一致。
    let mime = mime_guess::from_path(&f.file_name)
        .first_or_octet_stream()
        .to_string();
    let size = match &f.content {
        AnyFileContent::BinaryContent(bytes) if !bytes.is_empty() => Some(bytes.len()),
        // bytes 为空(超大文件 / 非多模态 binary)→ size 未知,不输出 size 属性,
        // 让 AI 用 read_files / Get-Item 之类自己查。
        _ => None,
    };
    if let Some(size) = size {
        let _ = writeln!(
            out,
            "  <file path=\"{path}\" mime_type=\"{}\" binary=\"true\" size=\"{size}\" />",
            xml_attr(&mime)
        );
    } else {
        let _ = writeln!(
            out,
            "  <file path=\"{path}\" mime_type=\"{}\" binary=\"true\" />",
            xml_attr(&mime)
        );
    }
}

/// Image prefix 占位:与 binary file 同语义,实际数据通过 ContentPart::Binary 进多模态。
fn render_image_placeholder(out: &mut String, img: &ImageContext) {
    use std::fmt::Write;
    let _ = writeln!(
        out,
        "  <image file_name=\"{}\" mime_type=\"{}\" />",
        xml_attr(&img.file_name),
        xml_attr(&img.mime_type),
    );
}

// ---------------------------------------------------------------------------
// XML 转义
// ---------------------------------------------------------------------------

fn xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn xml_attr(s: &str) -> String {
    xml_text(s).replace('"', "&quot;")
}

#[cfg(test)]
#[path = "user_context_tests.rs"]
mod tests;
