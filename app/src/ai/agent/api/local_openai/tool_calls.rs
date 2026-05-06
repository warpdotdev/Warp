//! Tool-call argument parsing for the local OpenAI-compatible Responses backend.

use anyhow::anyhow;
use serde_json::Value;
use warp_multi_agent_api as api;

use super::request::parse_mcp_function_name;

#[allow(deprecated)]
/// Converts a function tool name plus JSON arguments into the corresponding Warp tool call variant.
pub(super) fn parse_tool_call(
    name: &str,
    arguments: Value,
) -> anyhow::Result<api::message::tool_call::Tool> {
    if let Some((server_id, tool_name)) = parse_mcp_function_name(name) {
        return Ok(api::message::tool_call::Tool::CallMcpTool(
            api::message::tool_call::CallMcpTool {
                name: tool_name,
                args: optional_object(&arguments, "args")
                    .or_else(|| arguments.as_object().cloned())
                    .map(serde_json_object_to_prost_struct)
                    .transpose()?,
                server_id: server_id.unwrap_or_default(),
            },
        ));
    }

    match name {
        "run_shell_command" | "shell" => Ok(api::message::tool_call::Tool::RunShellCommand(
            api::message::tool_call::RunShellCommand {
                command: required_string(&arguments, "command")?,
                is_read_only: arguments
                    .get("is_read_only")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                uses_pager: arguments
                    .get("uses_pager")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                citations: vec![],
                is_risky: arguments
                    .get("is_risky")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                wait_until_complete_value: arguments
                    .get("wait_until_complete")
                    .and_then(Value::as_bool)
                    .or_else(|| {
                        optional_string(&arguments, "mode")
                            .map(|mode| mode == "wait")
                    })
                    .map(
                        api::message::tool_call::run_shell_command::WaitUntilCompleteValue::WaitUntilComplete,
                    ),
                risk_category: optional_string(&arguments, "risk_category")
                    .and_then(|value| parse_risk_category(&value))
                    .unwrap_or(api::RiskCategory::Unspecified)
                    .into(),
            },
        )),
        "read_files" => Ok(api::message::tool_call::Tool::ReadFiles(
            api::message::tool_call::ReadFiles {
                files: required_array(&arguments, "files")?
                    .iter()
                    .map(parse_read_file)
                    .collect::<anyhow::Result<Vec<_>>>()?,
            },
        )),
        "search_codebase" => Ok(api::message::tool_call::Tool::SearchCodebase(
            api::message::tool_call::SearchCodebase {
                query: required_string(&arguments, "query")?,
                path_filters: optional_string_array(&arguments, "path_filters"),
                codebase_path: optional_string(&arguments, "codebase_path").unwrap_or_default(),
            },
        )),
        "grep" => Ok(api::message::tool_call::Tool::Grep(api::message::tool_call::Grep {
            queries: required_string_array(&arguments, "queries")?,
            path: required_string(&arguments, "path")?,
        })),
        "file_glob" | "file_glob_v2" => Ok(api::message::tool_call::Tool::FileGlobV2(
            api::message::tool_call::FileGlobV2 {
                patterns: required_string_array(&arguments, "patterns")?,
                search_dir: optional_string(&arguments, "search_dir")
                    .or_else(|| optional_string(&arguments, "path"))
                    .unwrap_or_default(),
                max_matches: optional_i32(&arguments, "max_matches").unwrap_or_default(),
                max_depth: optional_i32(&arguments, "max_depth").unwrap_or_default(),
                min_depth: optional_i32(&arguments, "min_depth").unwrap_or_default(),
            },
        )),
        "apply_file_diffs" => Ok(api::message::tool_call::Tool::ApplyFileDiffs(
            parse_apply_file_diffs(arguments)?,
        )),
        "read_mcp_resource" => Ok(api::message::tool_call::Tool::ReadMcpResource(
            api::message::tool_call::ReadMcpResource {
                uri: required_string(&arguments, "uri")?,
                server_id: optional_string(&arguments, "server_id").unwrap_or_default(),
            },
        )),
        "call_mcp_tool" => Ok(api::message::tool_call::Tool::CallMcpTool(
            api::message::tool_call::CallMcpTool {
                name: required_string(&arguments, "name")?,
                args: optional_object(&arguments, "args")
                    .map(serde_json_object_to_prost_struct)
                    .transpose()?,
                server_id: optional_string(&arguments, "server_id").unwrap_or_default(),
            },
        )),
        "write_to_long_running_shell_command" | "write_to_lrc" => Ok(
            api::message::tool_call::Tool::WriteToLongRunningShellCommand(
                api::message::tool_call::WriteToLongRunningShellCommand {
                    input: required_string(&arguments, "input")?.into_bytes(),
                    mode: optional_string(&arguments, "mode")
                        .map(parse_write_mode)
                        .transpose()?,
                    command_id: required_string(&arguments, "command_id")?,
                },
            ),
        ),
        "read_shell_command_output" => Ok(api::message::tool_call::Tool::ReadShellCommandOutput(
            api::message::tool_call::ReadShellCommandOutput {
                command_id: required_string(&arguments, "command_id")?,
                delay: parse_read_shell_command_output_delay(&arguments)?,
            },
        )),
        "suggest_new_conversation" => Ok(api::message::tool_call::Tool::SuggestNewConversation(
            api::message::tool_call::SuggestNewConversation {
                message_id: required_string(&arguments, "message_id")?,
            },
        )),
        "read_documents" => Ok(api::message::tool_call::Tool::ReadDocuments(
            api::message::tool_call::ReadDocuments {
                documents: required_array(&arguments, "documents")?
                    .iter()
                    .map(parse_read_document)
                    .collect::<anyhow::Result<Vec<_>>>()?,
            },
        )),
        "edit_documents" => Ok(api::message::tool_call::Tool::EditDocuments(
            api::message::tool_call::EditDocuments {
                diffs: required_array(&arguments, "diffs")?
                    .iter()
                    .map(parse_document_diff)
                    .collect::<anyhow::Result<Vec<_>>>()?,
            },
        )),
        "create_documents" => Ok(api::message::tool_call::Tool::CreateDocuments(
            api::message::tool_call::CreateDocuments {
                new_documents: required_array(&arguments, "new_documents")?
                    .iter()
                    .map(parse_new_document)
                    .collect::<anyhow::Result<Vec<_>>>()?,
            },
        )),
        "suggest_prompt" => Ok(api::message::tool_call::Tool::SuggestPrompt(
            parse_suggest_prompt(arguments)?,
        )),
        "open_code_review" => Ok(api::message::tool_call::Tool::OpenCodeReview(
            api::message::tool_call::OpenCodeReview {},
        )),
        "insert_review_comments" => Ok(
            api::message::tool_call::Tool::InsertReviewComments(
                parse_insert_review_comments(arguments)?,
            ),
        ),
        "init_project" => Ok(api::message::tool_call::Tool::InitProject(
            api::message::tool_call::InitProject {},
        )),
        "fetch_conversation" => Ok(api::message::tool_call::Tool::FetchConversation(
            api::message::tool_call::FetchConversation {
                conversation_id: optional_string(&arguments, "conversation_id").unwrap_or_default(),
            },
        )),
        "read_skill" => Ok(api::message::tool_call::Tool::ReadSkill(
            parse_read_skill(arguments)?,
        )),
        "ask_user_question" => Ok(api::message::tool_call::Tool::AskUserQuestion(
            parse_ask_user_question(arguments)?,
        )),
        unsupported => Err(anyhow!("Unsupported local OpenAI tool call: {unsupported}")),
    }
}

/// Parses the file arguments used by the `read_files` tool.
pub(super) fn parse_read_file(
    value: &Value,
) -> anyhow::Result<api::message::tool_call::read_files::File> {
    let name = optional_string(value, "path")
        .or_else(|| optional_string(value, "name"))
        .ok_or_else(|| anyhow!("Missing required string field: path"))?;
    let line_ranges = parse_line_ranges(value, "line_ranges")?;

    Ok(api::message::tool_call::read_files::File { name, line_ranges })
}

/// Parses the document arguments used by the `read_documents` tool.
fn parse_read_document(
    value: &Value,
) -> anyhow::Result<api::message::tool_call::read_documents::Document> {
    Ok(api::message::tool_call::read_documents::Document {
        document_id: required_string(value, "document_id")?,
        line_ranges: parse_line_ranges(value, "line_ranges")?,
    })
}

/// Parses a single document diff entry for `edit_documents`.
fn parse_document_diff(
    value: &Value,
) -> anyhow::Result<api::message::tool_call::edit_documents::DocumentDiff> {
    Ok(api::message::tool_call::edit_documents::DocumentDiff {
        document_id: required_string(value, "document_id")?,
        search: required_string(value, "search")?,
        replace: required_string(value, "replace")?,
    })
}

/// Parses a single new document entry for `create_documents`.
fn parse_new_document(
    value: &Value,
) -> anyhow::Result<api::message::tool_call::create_documents::NewDocument> {
    Ok(api::message::tool_call::create_documents::NewDocument {
        content: required_string(value, "content")?,
        title: optional_string(value, "title").unwrap_or_default(),
    })
}

/// Parses the complex `apply_file_diffs` argument payload.
fn parse_apply_file_diffs(
    arguments: Value,
) -> anyhow::Result<api::message::tool_call::ApplyFileDiffs> {
    let diffs = optional_array(&arguments, "diffs")
        .into_iter()
        .flatten()
        .map(|value| {
            Ok(api::message::tool_call::apply_file_diffs::FileDiff {
                file_path: required_string(value, "file_path")?,
                search: required_string(value, "search")?,
                replace: required_string(value, "replace")?,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let new_files = optional_array(&arguments, "new_files")
        .into_iter()
        .flatten()
        .map(|value| {
            Ok(api::message::tool_call::apply_file_diffs::NewFile {
                file_path: required_string(value, "file_path")?,
                content: required_string(value, "content")?,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let deleted_files = optional_array(&arguments, "deleted_files")
        .into_iter()
        .flatten()
        .map(|value| {
            Ok(api::message::tool_call::apply_file_diffs::DeleteFile {
                file_path: required_string(value, "file_path")?,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let v4a_updates = optional_array(&arguments, "v4a_updates")
        .into_iter()
        .flatten()
        .map(parse_v4a_update)
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(api::message::tool_call::ApplyFileDiffs {
        summary: optional_string(&arguments, "summary").unwrap_or_default(),
        diffs,
        new_files,
        deleted_files,
        v4a_updates,
    })
}

/// Parses the official insert-review-comments payload.
fn parse_insert_review_comments(
    arguments: Value,
) -> anyhow::Result<api::message::tool_call::InsertReviewComments> {
    let comments = required_array(&arguments, "comments")?
        .iter()
        .map(parse_insert_review_comment)
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(api::message::tool_call::InsertReviewComments {
        repo_path: required_string(&arguments, "local_repository_path")?,
        comments,
        base_branch: required_string(&arguments, "base_branch")?,
    })
}

/// Parses a single official review comment payload.
fn parse_insert_review_comment(
    value: &Value,
) -> anyhow::Result<api::message::tool_call::insert_review_comments::Comment> {
    let parent_comment_id = value
        .get("reply_metadata")
        .and_then(|reply| reply.get("parent_comment_id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let location = value
        .get("location_metadata")
        .map(parse_insert_review_comment_location)
        .transpose()?;

    Ok(api::message::tool_call::insert_review_comments::Comment {
        comment_id: required_string(value, "comment_id")?,
        author: required_string(value, "author")?,
        last_modified_timestamp: required_string(value, "last_modified_timestamp")?,
        comment_body: required_string(value, "comment_body")?,
        parent_comment_id,
        location,
        html_url: required_string(value, "html_url")?,
    })
}

/// Parses official location metadata into Warp's review comment location proto.
fn parse_insert_review_comment_location(
    value: &Value,
) -> anyhow::Result<api::message::tool_call::insert_review_comments::CommentLocation> {
    let line = match (
        optional_string(value, "diff_hunk"),
        optional_u32(value, "start_line"),
        optional_u32(value, "end_line"),
    ) {
        (Some(diff_hunk), Some(start), Some(end)) => Some(
            api::message::tool_call::insert_review_comments::CommentLineRange {
                diff_hunk,
                range: Some(api::FileContentLineRange { start, end }),
                side: parse_review_comment_side(optional_string(value, "side").as_deref()).into(),
            },
        ),
        _ => None,
    };

    Ok(
        api::message::tool_call::insert_review_comments::CommentLocation {
            file_path: required_string(value, "filepath")?,
            line,
        },
    )
}

/// Parses the official diff-side string into Warp's enum.
fn parse_review_comment_side(
    value: Option<&str>,
) -> api::message::tool_call::insert_review_comments::CommentSide {
    match value {
        Some("LEFT") => api::message::tool_call::insert_review_comments::CommentSide::Old,
        _ => api::message::tool_call::insert_review_comments::CommentSide::New,
    }
}

/// Parses a single V4A file update definition for `apply_file_diffs`.
fn parse_v4a_update(
    value: &Value,
) -> anyhow::Result<api::message::tool_call::apply_file_diffs::V4aFileUpdate> {
    let hunks = required_array(value, "hunks")?
        .iter()
        .map(|hunk| {
            Ok(
                api::message::tool_call::apply_file_diffs::v4a_file_update::Hunk {
                    change_context: optional_string_array(hunk, "change_context"),
                    pre_context: optional_string(hunk, "pre_context").unwrap_or_default(),
                    old: optional_string(hunk, "old").unwrap_or_default(),
                    new: optional_string(hunk, "new").unwrap_or_default(),
                    post_context: optional_string(hunk, "post_context").unwrap_or_default(),
                },
            )
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(api::message::tool_call::apply_file_diffs::V4aFileUpdate {
        file_path: required_string(value, "file_path")?,
        move_to: optional_string(value, "move_to").unwrap_or_default(),
        hunks,
    })
}

/// Parses the optional line ranges used by read_files and read_documents.
fn parse_line_ranges(value: &Value, key: &str) -> anyhow::Result<Vec<api::FileContentLineRange>> {
    if let Some(line_ranges) = optional_array(value, key) {
        return line_ranges
            .iter()
            .map(|value| {
                Ok(api::FileContentLineRange {
                    start: required_u32(value, "start")?,
                    end: required_u32(value, "end")?,
                })
            })
            .collect();
    }

    if let Some(ranges) = optional_array(value, "ranges") {
        return ranges
            .iter()
            .map(|value| {
                let range = value
                    .as_str()
                    .ok_or_else(|| anyhow!("Range entries must be strings"))?;
                let (start, end) = range
                    .split_once('-')
                    .ok_or_else(|| anyhow!("Range entries must use start-end format"))?;
                Ok(api::FileContentLineRange {
                    start: start
                        .parse()
                        .map_err(|_| anyhow!("Invalid range start: {start}"))?,
                    end: end
                        .parse()
                        .map_err(|_| anyhow!("Invalid range end: {end}"))?,
                })
            })
            .collect();
    }

    // Backwards-compatible fallback for the earlier start_line/end_line shape.
    if let (Some(start), Some(end)) = (
        optional_u32(value, "start_line"),
        optional_u32(value, "end_line"),
    ) {
        return Ok(vec![api::FileContentLineRange { start, end }]);
    }

    Ok(Vec::new())
}

/// Parses the write mode used for long-running shell input.
fn parse_write_mode(
    mode: String,
) -> anyhow::Result<api::message::tool_call::write_to_long_running_shell_command::Mode> {
    let mode = match mode.as_str() {
        "raw" => api::message::tool_call::write_to_long_running_shell_command::mode::Mode::Raw(()),
        "line" => {
            api::message::tool_call::write_to_long_running_shell_command::mode::Mode::Line(())
        }
        "block" => {
            api::message::tool_call::write_to_long_running_shell_command::mode::Mode::Block(())
        }
        _ => {
            return Err(anyhow!(
                "Unsupported write_to_long_running_shell_command mode: {mode}"
            ));
        }
    };

    Ok(api::message::tool_call::write_to_long_running_shell_command::Mode { mode: Some(mode) })
}

/// Parses the optional delay configuration for `read_shell_command_output`.
fn parse_read_shell_command_output_delay(
    arguments: &Value,
) -> anyhow::Result<Option<api::message::tool_call::read_shell_command_output::Delay>> {
    if arguments
        .get("on_completion")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(Some(
            api::message::tool_call::read_shell_command_output::Delay::OnCompletion(()),
        ));
    }

    if let Some(delay_seconds) = optional_i64(arguments, "delay_seconds") {
        return Ok(Some(
            api::message::tool_call::read_shell_command_output::Delay::Duration(
                prost_types::Duration {
                    seconds: delay_seconds,
                    nanos: 0,
                },
            ),
        ));
    }

    Ok(None)
}

/// Parses the prompt suggestion oneof shape into Warp's proto tool call.
fn parse_suggest_prompt(
    arguments: Value,
) -> anyhow::Result<api::message::tool_call::SuggestPrompt> {
    let display_mode = required_string(&arguments, "display_mode")?;
    let display_mode = match display_mode.as_str() {
        "inline_query_banner" => {
            api::message::tool_call::suggest_prompt::DisplayMode::InlineQueryBanner(
                api::message::tool_call::suggest_prompt::InlineQueryBanner {
                    title: required_string(&arguments, "title")?,
                    description: required_string(&arguments, "description")?,
                    query: required_string(&arguments, "query")?,
                },
            )
        }
        "prompt_chip" => api::message::tool_call::suggest_prompt::DisplayMode::PromptChip(
            api::message::tool_call::suggest_prompt::PromptChip {
                prompt: required_string(&arguments, "prompt")?,
                label: optional_string(&arguments, "label").unwrap_or_default(),
            },
        ),
        _ => {
            return Err(anyhow!(
                "Unsupported suggest_prompt display_mode: {display_mode}"
            ));
        }
    };

    Ok(api::message::tool_call::SuggestPrompt {
        display_mode: Some(display_mode),
        is_trigger_irrelevant: arguments
            .get("is_trigger_irrelevant")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Parses the ask-user-question tool call.
fn parse_ask_user_question(arguments: Value) -> anyhow::Result<api::AskUserQuestion> {
    let questions = required_array(&arguments, "questions")?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_ask_user_question_item(value, index))
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(api::AskUserQuestion { questions })
}

/// Parses a single ask-user-question item.
fn parse_ask_user_question_item(
    value: &Value,
    index: usize,
) -> anyhow::Result<api::ask_user_question::Question> {
    let options = required_string_array(value, "options")?
        .into_iter()
        .map(|label| api::ask_user_question::Option { label })
        .collect::<Vec<_>>();
    let question_type = match required_string(value, "type")?.as_str() {
        "single_select" => Some(
            api::ask_user_question::question::QuestionType::MultipleChoice(
                api::ask_user_question::MultipleChoice {
                    options,
                    recommended_option_index: optional_i32(value, "recommended_option_index")
                        .unwrap_or(-1),
                    is_multiselect: false,
                    supports_other: false,
                },
            ),
        ),
        "multi_select" => Some(
            api::ask_user_question::question::QuestionType::MultipleChoice(
                api::ask_user_question::MultipleChoice {
                    options,
                    recommended_option_index: -1,
                    is_multiselect: true,
                    supports_other: false,
                },
            ),
        ),
        other => return Err(anyhow!("Unsupported ask_user_question type: {other}")),
    };

    Ok(api::ask_user_question::Question {
        question_id: format!("q{}", index + 1),
        question: required_string(value, "question")?,
        question_type,
    })
}

/// Parses the read_skill oneof shape into Warp's proto tool call.
fn parse_read_skill(arguments: Value) -> anyhow::Result<api::message::tool_call::ReadSkill> {
    let skill_reference = if let Some(skill_path) = optional_string(&arguments, "skill_path") {
        Some(api::message::tool_call::read_skill::SkillReference::SkillPath(skill_path))
    } else {
        optional_string(&arguments, "bundled_skill_id")
            .map(api::message::tool_call::read_skill::SkillReference::BundledSkillId)
    };

    Ok(api::message::tool_call::ReadSkill {
        skill_reference,
        name: optional_string(&arguments, "name").unwrap_or_default(),
    })
}

/// Parses a string risk category into the protobuf enum.
fn parse_risk_category(value: &str) -> Option<api::RiskCategory> {
    match value {
        "unspecified" => Some(api::RiskCategory::Unspecified),
        "read_only" => Some(api::RiskCategory::ReadOnly),
        "trivial_local_change" => Some(api::RiskCategory::TrivialLocalChange),
        "nontrivial_local_change" => Some(api::RiskCategory::NontrivialLocalChange),
        "external_change" => Some(api::RiskCategory::ExternalChange),
        "risky" => Some(api::RiskCategory::Risky),
        _ => None,
    }
}

/// Converts a serde JSON object into a protobuf Struct for MCP tool arguments.
fn serde_json_object_to_prost_struct(
    object: serde_json::Map<String, Value>,
) -> anyhow::Result<prost_types::Struct> {
    Ok(prost_types::Struct {
        fields: object
            .into_iter()
            .map(|(key, value)| serde_json_to_prost_value(value).map(|value| (key, value)))
            .collect::<anyhow::Result<_>>()?,
    })
}

/// Converts a serde JSON value into a protobuf Value.
fn serde_json_to_prost_value(value: Value) -> anyhow::Result<prost_types::Value> {
    use prost_types::value::Kind;

    let kind = match value {
        Value::Null => Kind::NullValue(0),
        Value::Bool(value) => Kind::BoolValue(value),
        Value::Number(value) => Kind::NumberValue(
            value
                .as_f64()
                .ok_or_else(|| anyhow!("Failed to convert JSON number to f64"))?,
        ),
        Value::String(value) => Kind::StringValue(value),
        Value::Array(values) => Kind::ListValue(prost_types::ListValue {
            values: values
                .into_iter()
                .map(serde_json_to_prost_value)
                .collect::<anyhow::Result<Vec<_>>>()?,
        }),
        Value::Object(object) => Kind::StructValue(serde_json_object_to_prost_struct(object)?),
    };

    Ok(prost_types::Value { kind: Some(kind) })
}

/// Extracts a required string field from a JSON object.
fn required_string(value: &Value, key: &str) -> anyhow::Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("Missing required string field '{key}'"))
}

/// Extracts an optional string field from a JSON object.
fn optional_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

/// Extracts a required array of strings from a JSON object.
fn required_string_array(value: &Value, key: &str) -> anyhow::Result<Vec<String>> {
    required_array(value, key)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| anyhow!("Field '{key}' must contain only strings"))
        })
        .collect()
}

/// Extracts an optional array of strings from a JSON object.
fn optional_string_array(value: &Value, key: &str) -> Vec<String> {
    optional_array(value, key)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

/// Extracts a required array field from a JSON object.
fn required_array<'a>(value: &'a Value, key: &str) -> anyhow::Result<&'a [Value]> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| anyhow!("Missing required array field '{key}'"))
}

/// Extracts an optional array field from a JSON object.
fn optional_array<'a>(value: &'a Value, key: &str) -> Option<&'a [Value]> {
    value.get(key).and_then(Value::as_array).map(Vec::as_slice)
}

/// Extracts an optional object field from a JSON object.
fn optional_object(value: &Value, key: &str) -> Option<serde_json::Map<String, Value>> {
    value.get(key).and_then(Value::as_object).cloned()
}

/// Extracts an optional `u32` field from a JSON object.
fn optional_u32(value: &Value, key: &str) -> Option<u32> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

/// Extracts an optional `i32` field from a JSON object.
fn optional_i32(value: &Value, key: &str) -> Option<i32> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
}

/// Extracts a required `u32` field from a JSON object.
fn required_u32(value: &Value, key: &str) -> anyhow::Result<u32> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| anyhow!("Missing required u32 field '{key}'"))
}

/// Extracts an optional `i64` field from a JSON object.
fn optional_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}
