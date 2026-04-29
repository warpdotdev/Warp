use serde_json::json;
use warp_multi_agent_api as api;

use super::*;
use crate::{
    config::FeatureConfig,
    upstream::openai::{OpenAiToolCall, OpenAiToolCallFunction},
};

#[test]
fn run_shell_command_adapter_roundtrips_representative_payload() {
    let registry = registry_for_tools(&[api::ToolType::RunShellCommand]);
    let converted = convert(
        &registry,
        "run_shell_command",
        json!({
            "command": "pwd",
            "risk_category": "read_only",
            "wait_until_complete": true
        }),
    );

    let Some(api::message::tool_call::Tool::RunShellCommand(call)) = converted.tool_call.tool
    else {
        panic!("expected RunShellCommand tool call");
    };
    assert_eq!(call.command, "pwd");
    assert_eq!(call.risk_category, api::RiskCategory::ReadOnly as i32);

    let content = registry.result_to_openai_content(&api::request::input::ToolCallResult {
        tool_call_id: "warp-call".to_string(),
        result: Some(
            api::request::input::tool_call_result::Result::RunShellCommand(
                api::RunShellCommandResult {
                    command: "pwd".to_string(),
                    result: Some(api::run_shell_command_result::Result::CommandFinished(
                        api::ShellCommandFinished {
                            command_id: "cmd-1".to_string(),
                            output: "/tmp\n".to_string(),
                            exit_code: 0,
                        },
                    )),
                    ..Default::default()
                },
            ),
        ),
    });
    assert!(content.contains("/tmp"));
}

#[test]
fn write_to_long_running_shell_command_adapter_roundtrips_representative_payload() {
    let registry = registry_for_tools(&[api::ToolType::WriteToLongRunningShellCommand]);
    let converted = convert(
        &registry,
        "write_to_long_running_shell_command",
        json!({ "command_id": "cmd-1", "input": "q", "mode": "line" }),
    );

    let Some(api::message::tool_call::Tool::WriteToLongRunningShellCommand(call)) =
        converted.tool_call.tool
    else {
        panic!("expected WriteToLongRunningShellCommand tool call");
    };
    assert_eq!(call.command_id, "cmd-1");
    assert_eq!(call.input, b"q".to_vec());

    let content = registry.result_to_openai_content(&api::request::input::ToolCallResult {
        tool_call_id: "warp-call".to_string(),
        result: Some(
            api::request::input::tool_call_result::Result::WriteToLongRunningShellCommand(
                api::WriteToLongRunningShellCommandResult {
                    result: Some(
                        api::write_to_long_running_shell_command_result::Result::CommandFinished(
                            api::ShellCommandFinished {
                                command_id: "cmd-1".to_string(),
                                output: "done".to_string(),
                                exit_code: 0,
                            },
                        ),
                    ),
                },
            ),
        ),
    });
    assert!(content.contains("done"));
}

#[test]
fn read_shell_command_output_adapter_roundtrips_representative_payload() {
    let registry = registry_for_tools(&[api::ToolType::ReadShellCommandOutput]);
    let converted = convert(
        &registry,
        "read_shell_command_output",
        json!({ "command_id": "cmd-1", "delay_ms": 250 }),
    );

    let Some(api::message::tool_call::Tool::ReadShellCommandOutput(call)) =
        converted.tool_call.tool
    else {
        panic!("expected ReadShellCommandOutput tool call");
    };
    assert_eq!(call.command_id, "cmd-1");
    assert!(matches!(
        call.delay,
        Some(api::message::tool_call::read_shell_command_output::Delay::Duration(_))
    ));

    let content = registry.result_to_openai_content(&api::request::input::ToolCallResult {
        tool_call_id: "warp-call".to_string(),
        result: Some(
            api::request::input::tool_call_result::Result::ReadShellCommandOutput(
                api::ReadShellCommandOutputResult {
                    command: "npm test".to_string(),
                    result: Some(
                        api::read_shell_command_output_result::Result::LongRunningCommandSnapshot(
                            api::LongRunningShellCommandSnapshot {
                                command_id: "cmd-1".to_string(),
                                output: "still running".to_string(),
                                is_preempted: true,
                                ..Default::default()
                            },
                        ),
                    ),
                },
            ),
        ),
    });
    assert!(content.contains("still running"));
}

#[test]
fn read_files_adapter_roundtrips_representative_payload() {
    let registry = registry_for_tools(&[api::ToolType::ReadFiles]);
    let converted = convert(
        &registry,
        "read_files",
        json!({ "files": [{ "name": "src/main.rs", "line_ranges": [{ "start": 1, "end": 3 }] }] }),
    );

    let Some(api::message::tool_call::Tool::ReadFiles(call)) = converted.tool_call.tool else {
        panic!("expected ReadFiles tool call");
    };
    assert_eq!(call.files[0].name, "src/main.rs");
    assert_eq!(call.files[0].line_ranges[0].start, 1);

    let content = registry.result_to_openai_content(&api::request::input::ToolCallResult {
        tool_call_id: "warp-call".to_string(),
        result: Some(api::request::input::tool_call_result::Result::ReadFiles(
            api::ReadFilesResult {
                result: Some(api::read_files_result::Result::TextFilesSuccess(
                    api::read_files_result::TextFilesSuccess {
                        files: vec![file_content("src/main.rs", "fn main() {}")],
                    },
                )),
            },
        )),
    });
    assert!(content.contains("fn main"));
}

#[test]
fn apply_file_diffs_adapter_roundtrips_representative_payload() {
    let registry = registry_for_tools(&[api::ToolType::ApplyFileDiffs]);
    let converted = convert(
        &registry,
        "apply_file_diffs",
        json!({
            "summary": "edit greeting",
            "diffs": [{ "file_path": "src/lib.rs", "search": "hello", "replace": "hi" }],
            "new_files": [{ "file_path": "README.md", "content": "docs" }],
            "deleted_files": [{ "file_path": "old.txt" }]
        }),
    );

    let Some(api::message::tool_call::Tool::ApplyFileDiffs(call)) = converted.tool_call.tool else {
        panic!("expected ApplyFileDiffs tool call");
    };
    assert_eq!(call.summary, "edit greeting");
    assert_eq!(call.diffs[0].replace, "hi");
    assert_eq!(call.new_files[0].file_path, "README.md");

    let content = registry.result_to_openai_content(&api::request::input::ToolCallResult {
        tool_call_id: "warp-call".to_string(),
        result: Some(
            api::request::input::tool_call_result::Result::ApplyFileDiffs(
                api::ApplyFileDiffsResult {
                    result: Some(api::apply_file_diffs_result::Result::Success(
                        api::apply_file_diffs_result::Success {
                            updated_files_v2: vec![
                                api::apply_file_diffs_result::success::UpdatedFileContent {
                                    file: Some(file_content("src/lib.rs", "hi")),
                                    was_edited_by_user: false,
                                },
                            ],
                            deleted_files: vec![
                                api::apply_file_diffs_result::success::DeletedFile {
                                    file_path: "old.txt".to_string(),
                                },
                            ],
                            ..Default::default()
                        },
                    )),
                },
            ),
        ),
    });
    assert!(content.contains("old.txt"));
}

#[test]
fn search_codebase_adapter_roundtrips_representative_payload() {
    let registry = registry_for_tools(&[api::ToolType::SearchCodebase]);
    let converted = convert(
        &registry,
        "search_codebase",
        json!({ "query": "routing state", "path_filters": ["crates/**"], "codebase_path": "/repo" }),
    );

    let Some(api::message::tool_call::Tool::SearchCodebase(call)) = converted.tool_call.tool else {
        panic!("expected SearchCodebase tool call");
    };
    assert_eq!(call.query, "routing state");
    assert_eq!(call.path_filters, vec!["crates/**".to_string()]);

    let content = registry.result_to_openai_content(&api::request::input::ToolCallResult {
        tool_call_id: "warp-call".to_string(),
        result: Some(
            api::request::input::tool_call_result::Result::SearchCodebase(
                api::SearchCodebaseResult {
                    result: Some(api::search_codebase_result::Result::Success(
                        api::search_codebase_result::Success {
                            files: vec![file_content("crates/x.rs", "match route")],
                        },
                    )),
                },
            ),
        ),
    });
    assert!(content.contains("match route"));
}

#[test]
fn grep_adapter_roundtrips_representative_payload() {
    let registry = registry_for_tools(&[api::ToolType::Grep]);
    let converted = convert(
        &registry,
        "grep",
        json!({ "queries": ["TODO"], "path": "src" }),
    );

    let Some(api::message::tool_call::Tool::Grep(call)) = converted.tool_call.tool else {
        panic!("expected Grep tool call");
    };
    assert_eq!(call.queries, vec!["TODO".to_string()]);
    assert_eq!(call.path, "src");

    let content = registry.result_to_openai_content(&api::request::input::ToolCallResult {
        tool_call_id: "warp-call".to_string(),
        result: Some(api::request::input::tool_call_result::Result::Grep(
            api::GrepResult {
                result: Some(api::grep_result::Result::Success(
                    api::grep_result::Success {
                        matched_files: vec![api::grep_result::success::GrepFileMatch {
                            file_path: "src/lib.rs".to_string(),
                            matched_lines: vec![
                                api::grep_result::success::grep_file_match::GrepLineMatch {
                                    line_number: 7,
                                },
                            ],
                        }],
                    },
                )),
            },
        )),
    });
    assert!(content.contains("src/lib.rs"));
}

#[test]
#[allow(deprecated)]
fn file_glob_adapter_roundtrips_representative_payload() {
    let registry = registry_for_tools(&[api::ToolType::FileGlob]);
    let converted = convert(
        &registry,
        "file_glob",
        json!({ "patterns": ["*.rs"], "path": "src" }),
    );

    let Some(api::message::tool_call::Tool::FileGlob(call)) = converted.tool_call.tool else {
        panic!("expected FileGlob tool call");
    };
    assert_eq!(call.patterns, vec!["*.rs".to_string()]);

    let content = registry.result_to_openai_content(&api::request::input::ToolCallResult {
        tool_call_id: "warp-call".to_string(),
        result: Some(api::request::input::tool_call_result::Result::FileGlob(
            api::FileGlobResult {
                result: Some(api::file_glob_result::Result::Success(
                    api::file_glob_result::Success {
                        matched_files: "src/lib.rs\nsrc/main.rs".to_string(),
                    },
                )),
            },
        )),
    });
    assert!(content.contains("src/main.rs"));
}

#[test]
fn file_glob_v2_adapter_roundtrips_representative_payload() {
    let registry = registry_for_tools(&[api::ToolType::FileGlobV2]);
    let converted = convert(
        &registry,
        "file_glob_v2",
        json!({ "patterns": ["*.rs"], "search_dir": "src", "max_matches": 10, "max_depth": 2 }),
    );

    let Some(api::message::tool_call::Tool::FileGlobV2(call)) = converted.tool_call.tool else {
        panic!("expected FileGlobV2 tool call");
    };
    assert_eq!(call.search_dir, "src");
    assert_eq!(call.max_matches, 10);

    let content = registry.result_to_openai_content(&api::request::input::ToolCallResult {
        tool_call_id: "warp-call".to_string(),
        result: Some(api::request::input::tool_call_result::Result::FileGlobV2(
            api::FileGlobV2Result {
                result: Some(api::file_glob_v2_result::Result::Success(
                    api::file_glob_v2_result::Success {
                        matched_files: vec![api::file_glob_v2_result::success::FileGlobMatch {
                            file_path: "src/lib.rs".to_string(),
                        }],
                        warnings: String::new(),
                    },
                )),
            },
        )),
    });
    assert!(content.contains("src/lib.rs"));
}

#[test]
fn read_shell_command_output_rejects_negative_delay() {
    let registry = registry_for_tools(&[api::ToolType::ReadShellCommandOutput]);
    let error = convert_error(
        &registry,
        "read_shell_command_output",
        json!({ "command_id": "cmd-1", "delay_ms": -1 }),
    );

    assert!(error.contains("delay_ms"));
    assert!(error.contains("non-negative"));
}

#[test]
fn read_files_rejects_invalid_line_ranges() {
    let registry = registry_for_tools(&[api::ToolType::ReadFiles]);
    for args in [
        json!({ "files": [{ "name": "src/main.rs", "line_ranges": [{ "start": 0, "end": 3 }] }] }),
        json!({ "files": [{ "name": "src/main.rs", "line_ranges": [{ "start": 4, "end": 3 }] }] }),
        json!({ "files": [{ "name": "src/main.rs", "line_ranges": [{ "start": 1, "end": u64::from(u32::MAX) + 1 }] }] }),
    ] {
        let error = convert_error(&registry, "read_files", args);
        assert!(
            error.contains("start") || error.contains("end"),
            "unexpected error: {error}"
        );
    }
}

#[test]
fn file_glob_v2_rejects_negative_and_overflowing_limits() {
    let registry = registry_for_tools(&[api::ToolType::FileGlobV2]);
    for (field, value, expected) in [
        ("max_matches", json!(-1), "non-negative"),
        ("max_depth", json!(i64::from(i32::MAX) + 1), "i32::MAX"),
        ("min_depth", json!(u64::MAX), "i64::MAX"),
    ] {
        let mut args = json!({ "patterns": ["*.rs"], "search_dir": "src" });
        args[field] = value;
        let error = convert_error(&registry, "file_glob_v2", args);
        assert!(error.contains(field), "unexpected error: {error}");
        assert!(error.contains(expected), "unexpected error: {error}");
    }
}

#[test]
fn read_mcp_resource_adapter_roundtrips_representative_payload() {
    let registry = registry_for_tools(&[api::ToolType::ReadMcpResource]);
    let converted = convert(
        &registry,
        "read_mcp_resource",
        json!({ "uri": "file://resource", "server_id": "server-1" }),
    );

    let Some(api::message::tool_call::Tool::ReadMcpResource(call)) = converted.tool_call.tool
    else {
        panic!("expected ReadMcpResource tool call");
    };
    assert_eq!(call.uri, "file://resource");
    assert_eq!(call.server_id, "server-1");

    let content = registry.result_to_openai_content(&api::request::input::ToolCallResult {
        tool_call_id: "warp-call".to_string(),
        result: Some(
            api::request::input::tool_call_result::Result::ReadMcpResource(
                api::ReadMcpResourceResult {
                    result: Some(api::read_mcp_resource_result::Result::Success(
                        api::read_mcp_resource_result::Success {
                            contents: vec![api::McpResourceContent {
                                uri: "file://resource".to_string(),
                                content_type: Some(api::mcp_resource_content::ContentType::Text(
                                    api::mcp_resource_content::Text {
                                        content: "resource body".to_string(),
                                        mime_type: "text/plain".to_string(),
                                    },
                                )),
                            }],
                        },
                    )),
                },
            ),
        ),
    });
    assert!(content.contains("resource body"));
}

#[test]
fn mcp_tool_adapter_roundtrips_representative_payload_and_routes_to_original_server_tool() {
    let registry = registry_with_mcp_tool();
    let openai_tool = registry
        .openai_tools()
        .into_iter()
        .find(|tool| {
            tool.function
                .name
                .starts_with("mcp__linear_prod__create_issue")
        })
        .expect("MCP tool declaration exists");
    assert_eq!(
        openai_tool.function.parameters["properties"]["title"]["type"],
        "string"
    );

    let converted = convert(
        &registry,
        &openai_tool.function.name,
        json!({ "title": "Bug", "priority": 1 }),
    );

    let Some(api::message::tool_call::Tool::CallMcpTool(call)) = converted.tool_call.tool else {
        panic!("expected CallMcpTool tool call");
    };
    assert_eq!(call.server_id, "linear-prod");
    assert_eq!(call.name, "create.issue");
    let args = super::struct_to_json_value(call.args.as_ref().unwrap());
    assert_eq!(args["title"], "Bug");

    let content = registry.result_to_openai_content(&api::request::input::ToolCallResult {
        tool_call_id: "warp-call".to_string(),
        result: Some(api::request::input::tool_call_result::Result::CallMcpTool(
            api::CallMcpToolResult {
                result: Some(api::call_mcp_tool_result::Result::Success(
                    api::call_mcp_tool_result::Success {
                        results: vec![api::call_mcp_tool_result::success::Result {
                            result: Some(api::call_mcp_tool_result::success::result::Result::Text(
                                api::call_mcp_tool_result::success::result::Text {
                                    text: "created LIN-1".to_string(),
                                },
                            )),
                        }],
                    },
                )),
            },
        )),
    });
    assert!(content.contains("LIN-1"));
}

#[test]
fn registry_omits_explicitly_unsupported_tools() {
    let registry = registry_for_tools(&[api::ToolType::StartAgent, api::ToolType::UseComputer]);
    assert!(registry.openai_tools().is_empty());
}

#[test]
fn declared_tool_schemas_are_openai_compatible_json_schema_objects() {
    #[derive(serde::Deserialize)]
    struct FunctionSchema {
        #[serde(rename = "type")]
        schema_type: String,
        properties: serde_json::Map<String, serde_json::Value>,
        #[serde(default)]
        required: Vec<String>,
    }

    let registry = registry_for_tools(&[
        api::ToolType::RunShellCommand,
        api::ToolType::WriteToLongRunningShellCommand,
        api::ToolType::ReadShellCommandOutput,
        api::ToolType::ReadFiles,
        api::ToolType::ApplyFileDiffs,
        api::ToolType::SearchCodebase,
        api::ToolType::Grep,
        api::ToolType::FileGlob,
        api::ToolType::FileGlobV2,
        api::ToolType::ReadMcpResource,
    ]);

    for tool in registry.openai_tools() {
        let schema: FunctionSchema =
            serde_json::from_value(tool.function.parameters.clone()).unwrap();
        assert_eq!(schema.schema_type, "object", "{}", tool.function.name);
        for required in &schema.required {
            assert!(
                schema.properties.contains_key(required),
                "{} required unknown property `{}`",
                tool.function.name,
                required
            );
        }
    }
}

#[test]
fn mcp_tool_schema_is_normalized_before_declaration() {
    let registry = ToolRegistry::for_request(
        &api::Request {
            settings: Some(api::request::Settings {
                supported_tools: vec![api::ToolType::CallMcpTool as i32],
                ..Default::default()
            }),
            mcp_context: Some(api::request::McpContext {
                servers: vec![api::request::mcp_context::McpServer {
                    id: "server".to_string(),
                    name: "Server".to_string(),
                    tools: vec![api::request::mcp_context::McpTool {
                        name: "broken".to_string(),
                        input_schema: Some(
                            super::json_to_struct(&json!({
                                "properties": { "query": { "type": "string" } },
                                "required": ["query"]
                            }))
                            .unwrap(),
                        ),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        },
        &FeatureConfig::default(),
    );

    let tool = registry.openai_tools().remove(0);
    assert_eq!(tool.function.parameters["type"], "object");
    assert_eq!(
        tool.function.parameters["properties"]["query"]["type"],
        "string"
    );
}

fn registry_for_tools(tools: &[api::ToolType]) -> ToolRegistry {
    ToolRegistry::for_request(
        &api::Request {
            settings: Some(api::request::Settings {
                supported_tools: tools.iter().map(|tool| *tool as i32).collect(),
                ..Default::default()
            }),
            ..Default::default()
        },
        &FeatureConfig::default(),
    )
}

fn registry_with_mcp_tool() -> ToolRegistry {
    ToolRegistry::for_request(
        &api::Request {
            settings: Some(api::request::Settings {
                supported_tools: vec![api::ToolType::CallMcpTool as i32],
                ..Default::default()
            }),
            mcp_context: Some(api::request::McpContext {
                servers: vec![api::request::mcp_context::McpServer {
                    id: "linear-prod".to_string(),
                    name: "Linear Prod".to_string(),
                    tools: vec![api::request::mcp_context::McpTool {
                        name: "create.issue".to_string(),
                        description: "Create a Linear issue".to_string(),
                        input_schema: Some(
                            super::json_to_struct(&json!({
                                "type": "object",
                                "properties": {
                                    "title": { "type": "string" },
                                    "priority": { "type": "integer" }
                                },
                                "required": ["title"]
                            }))
                            .unwrap(),
                        ),
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        },
        &FeatureConfig::default(),
    )
}

fn convert(registry: &ToolRegistry, name: &str, args: serde_json::Value) -> WarpToolCall {
    convert_result(registry, name, args).unwrap()
}

fn convert_error(registry: &ToolRegistry, name: &str, args: serde_json::Value) -> String {
    convert_result(registry, name, args)
        .unwrap_err()
        .to_string()
}

fn convert_result(
    registry: &ToolRegistry,
    name: &str,
    args: serde_json::Value,
) -> anyhow::Result<WarpToolCall> {
    registry.convert_openai_tool_call(
        &OpenAiToolCall {
            id: "openai-call".to_string(),
            kind: "function".to_string(),
            function: OpenAiToolCallFunction {
                name: name.to_string(),
                arguments: args.to_string(),
            },
        },
        "warp-call".to_string(),
    )
}

fn file_content(path: &str, content: &str) -> api::FileContent {
    api::FileContent {
        file_path: path.to_string(),
        content: content.to_string(),
        line_range: None,
    }
}
