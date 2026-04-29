use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use warp_multi_agent_api as api;

use crate::{
    config::FeatureConfig,
    conversation::transcript::{PendingToolCall, TranscriptToolCall, WarpToolKind},
    upstream::openai::{OpenAiFunction, OpenAiToolCall, OpenAiToolDeclaration},
};

#[derive(Clone, Debug, Default)]
pub(crate) struct ToolRegistry {
    adapters: BTreeMap<String, ToolAdapter>,
}

#[derive(Clone, Debug)]
struct ToolAdapter {
    openai_name: String,
    description: String,
    schema: Value,
    kind: AdapterKind,
}

#[derive(Clone, Debug)]
enum AdapterKind {
    Builtin(BuiltinTool),
    Mcp(McpToolRoute),
}

#[derive(Clone, Debug)]
struct McpToolRoute {
    server_id: String,
    server_name: String,
    tool_name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BuiltinTool {
    RunShellCommand,
    WriteToLongRunningShellCommand,
    ReadShellCommandOutput,
    ReadFiles,
    ApplyFileDiffs,
    SearchCodebase,
    Grep,
    FileGlob,
    FileGlobV2,
    ReadMcpResource,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WarpToolCall {
    pub(crate) openai_tool_call: TranscriptToolCall,
    pub(crate) pending: PendingToolCall,
    pub(crate) tool_call: api::message::ToolCall,
}

impl ToolRegistry {
    pub(crate) fn for_request(request: &api::Request, features: &FeatureConfig) -> Self {
        if !features.tools_enabled {
            return Self::default();
        }

        let mut registry = Self::default();
        for builtin in BuiltinTool::all() {
            if request_supports_tool(request, builtin.tool_type()) {
                registry.insert(ToolAdapter::builtin(builtin));
            }
        }

        if features.mcp_tools_enabled && request_supports_tool(request, api::ToolType::CallMcpTool)
        {
            registry.insert_mcp_tools(request);
        }

        registry
    }

    pub(crate) fn openai_tools(&self) -> Vec<OpenAiToolDeclaration> {
        self.adapters
            .values()
            .map(|adapter| OpenAiToolDeclaration {
                kind: "function".to_string(),
                function: OpenAiFunction {
                    name: adapter.openai_name.clone(),
                    description: adapter.description.clone(),
                    parameters: openai_compatible_object_schema(adapter.schema.clone()),
                },
            })
            .collect()
    }

    pub(crate) fn convert_openai_tool_call(
        &self,
        openai_tool_call: &OpenAiToolCall,
        warp_tool_call_id: String,
    ) -> Result<WarpToolCall> {
        let adapter = self
            .adapters
            .get(&openai_tool_call.function.name)
            .ok_or_else(|| {
                anyhow!(
                    "upstream requested unsupported tool `{}`",
                    openai_tool_call.function.name
                )
            })?;
        let args = parse_arguments(&openai_tool_call.function.arguments)?;
        let tool = adapter.openai_args_to_tool(&args)?;
        let kind = adapter.warp_tool_kind();
        let pending = PendingToolCall {
            warp_tool_call_id: warp_tool_call_id.clone(),
            openai_tool_call_id: openai_tool_call.id.clone(),
            openai_tool_name: adapter.openai_name.clone(),
            kind,
        };

        Ok(WarpToolCall {
            openai_tool_call: TranscriptToolCall {
                id: openai_tool_call.id.clone(),
                name: openai_tool_call.function.name.clone(),
                arguments: openai_tool_call.function.arguments.clone(),
            },
            pending,
            tool_call: api::message::ToolCall {
                tool_call_id: warp_tool_call_id,
                tool: Some(tool),
            },
        })
    }

    pub(crate) fn result_to_openai_content(
        &self,
        result: &api::request::input::ToolCallResult,
    ) -> String {
        result_to_openai_content(result)
    }

    fn insert(&mut self, adapter: ToolAdapter) {
        self.adapters.insert(adapter.openai_name.clone(), adapter);
    }

    fn insert_mcp_tools(&mut self, request: &api::Request) {
        let mut used_names = self.adapters.keys().cloned().collect::<BTreeSet<_>>();
        let Some(mcp_context) = request.mcp_context.as_ref() else {
            return;
        };

        for server in &mcp_context.servers {
            let server_key = non_empty(&server.id)
                .or_else(|| non_empty(&server.name))
                .unwrap_or("server");
            let short_server_id = sanitize_openai_name_part(server_key, 24);
            for tool in &server.tools {
                let Some(tool_name) = non_empty(&tool.name) else {
                    continue;
                };
                let sanitized_tool_name = sanitize_openai_name_part(tool_name, 40);
                let base_name = format!("mcp__{short_server_id}__{sanitized_tool_name}");
                let openai_name = unique_tool_name(base_name, &mut used_names);
                let schema = tool
                    .input_schema
                    .as_ref()
                    .map(struct_to_json_value)
                    .unwrap_or_else(empty_object_schema);

                self.insert(ToolAdapter {
                    openai_name,
                    description: if tool.description.trim().is_empty() {
                        format!("Call MCP tool `{tool_name}` on server `{}`.", server.name)
                    } else {
                        tool.description.clone()
                    },
                    schema,
                    kind: AdapterKind::Mcp(McpToolRoute {
                        server_id: server.id.clone(),
                        server_name: server.name.clone(),
                        tool_name: tool.name.clone(),
                    }),
                });
            }
        }
    }
}

impl ToolAdapter {
    fn builtin(tool: BuiltinTool) -> Self {
        Self {
            openai_name: tool.openai_name().to_string(),
            description: tool.description().to_string(),
            schema: tool.schema(),
            kind: AdapterKind::Builtin(tool),
        }
    }

    fn openai_args_to_tool(&self, args: &Value) -> Result<api::message::tool_call::Tool> {
        match &self.kind {
            AdapterKind::Builtin(tool) => tool.openai_args_to_tool(args),
            AdapterKind::Mcp(route) => {
                let args = json_to_struct(args)?;
                Ok(api::message::tool_call::Tool::CallMcpTool(
                    api::message::tool_call::CallMcpTool {
                        name: route.tool_name.clone(),
                        args: Some(args),
                        server_id: route.server_id.clone(),
                    },
                ))
            }
        }
    }

    fn warp_tool_kind(&self) -> WarpToolKind {
        match &self.kind {
            AdapterKind::Builtin(tool) => WarpToolKind::Builtin(tool.tool_type()),
            AdapterKind::Mcp(route) => WarpToolKind::Mcp {
                server_id: route.server_id.clone(),
                server_name: route.server_name.clone(),
                tool_name: route.tool_name.clone(),
            },
        }
    }
}

impl BuiltinTool {
    fn all() -> [Self; 10] {
        [
            Self::RunShellCommand,
            Self::WriteToLongRunningShellCommand,
            Self::ReadShellCommandOutput,
            Self::ReadFiles,
            Self::ApplyFileDiffs,
            Self::SearchCodebase,
            Self::Grep,
            Self::FileGlob,
            Self::FileGlobV2,
            Self::ReadMcpResource,
        ]
    }

    fn tool_type(self) -> api::ToolType {
        match self {
            Self::RunShellCommand => api::ToolType::RunShellCommand,
            Self::WriteToLongRunningShellCommand => api::ToolType::WriteToLongRunningShellCommand,
            Self::ReadShellCommandOutput => api::ToolType::ReadShellCommandOutput,
            Self::ReadFiles => api::ToolType::ReadFiles,
            Self::ApplyFileDiffs => api::ToolType::ApplyFileDiffs,
            Self::SearchCodebase => api::ToolType::SearchCodebase,
            Self::Grep => api::ToolType::Grep,
            Self::FileGlob => api::ToolType::FileGlob,
            Self::FileGlobV2 => api::ToolType::FileGlobV2,
            Self::ReadMcpResource => api::ToolType::ReadMcpResource,
        }
    }

    fn openai_name(self) -> &'static str {
        match self {
            Self::RunShellCommand => "run_shell_command",
            Self::WriteToLongRunningShellCommand => "write_to_long_running_shell_command",
            Self::ReadShellCommandOutput => "read_shell_command_output",
            Self::ReadFiles => "read_files",
            Self::ApplyFileDiffs => "apply_file_diffs",
            Self::SearchCodebase => "search_codebase",
            Self::Grep => "grep",
            Self::FileGlob => "file_glob",
            Self::FileGlobV2 => "file_glob_v2",
            Self::ReadMcpResource => "read_mcp_resource",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::RunShellCommand => "Run a shell command in the user's Warp session.",
            Self::WriteToLongRunningShellCommand => {
                "Write input to a long-running shell command that is still active."
            }
            Self::ReadShellCommandOutput => {
                "Read updated output from a long-running shell command."
            }
            Self::ReadFiles => "Read one or more files from the user's machine.",
            Self::ApplyFileDiffs => {
                "Apply file edits, create files, delete files, or rename files."
            }
            Self::SearchCodebase => "Search the user's codebase semantically for relevant files.",
            Self::Grep => "Search file contents for exact text or patterns.",
            Self::FileGlob => "Find files by glob pattern under a directory.",
            Self::FileGlobV2 => "Find files by filename pattern with depth and result limits.",
            Self::ReadMcpResource => "Read a resource exposed by one of the user's MCP servers.",
        }
    }

    fn schema(self) -> Value {
        match self {
            Self::RunShellCommand => json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Command line to run." },
                    "risk_category": {
                        "type": "string",
                        "enum": ["read_only", "trivial_local_change", "nontrivial_local_change", "external_change", "risky", "unspecified"],
                        "description": "Risk assessment for the command. Use read_only for inspection commands."
                    },
                    "wait_until_complete": { "type": "boolean", "description": "If false, Warp may return an early snapshot for long-running commands." },
                    "uses_pager": { "type": "boolean", "description": "Whether the command is expected to use a pager/TUI." },
                    "is_read_only": { "type": "boolean", "description": "Deprecated compatibility flag; prefer risk_category." },
                    "is_risky": { "type": "boolean", "description": "Deprecated compatibility flag; prefer risk_category." }
                },
                "required": ["command"]
            }),
            Self::WriteToLongRunningShellCommand => json!({
                "type": "object",
                "properties": {
                    "command_id": { "type": "string", "description": "ID from a prior long-running command snapshot." },
                    "input": { "type": "string", "description": "Text or bytes to send to the PTY." },
                    "mode": { "type": "string", "enum": ["raw", "line", "block"], "description": "raw writes bytes as-is; line submits one command/input line; block bracket-pastes multiline text." }
                },
                "required": ["command_id", "input"]
            }),
            Self::ReadShellCommandOutput => json!({
                "type": "object",
                "properties": {
                    "command_id": { "type": "string", "description": "ID from a prior long-running command snapshot." },
                    "delay_ms": { "type": "integer", "minimum": 0, "description": "Optional delay before reading output." },
                    "on_completion": { "type": "boolean", "description": "If true, wait until the command completes or the client cap is reached." }
                },
                "required": ["command_id"]
            }),
            Self::ReadFiles => json!({
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string", "description": "File path to read." },
                                "path": { "type": "string", "description": "Alias for name." },
                                "line_ranges": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "start": { "type": "integer", "minimum": 1 },
                                            "end": { "type": "integer", "minimum": 1 }
                                        },
                                        "required": ["start", "end"]
                                    }
                                }
                            },
                            "required": ["name"]
                        }
                    }
                },
                "required": ["files"]
            }),
            Self::ApplyFileDiffs => json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string", "description": "Brief summary of the requested edits." },
                    "diffs": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" },
                                "search": { "type": "string", "description": "Exact content to replace; empty can mean insertion for compatible clients." },
                                "replace": { "type": "string" }
                            },
                            "required": ["file_path", "search", "replace"]
                        }
                    },
                    "new_files": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" },
                                "content": { "type": "string" }
                            },
                            "required": ["file_path", "content"]
                        }
                    },
                    "deleted_files": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": { "file_path": { "type": "string" } },
                            "required": ["file_path"]
                        }
                    },
                    "v4a_updates": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" },
                                "move_to": { "type": "string" },
                                "hunks": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "change_context": { "type": "array", "items": { "type": "string" } },
                                            "pre_context": { "type": "string" },
                                            "old": { "type": "string" },
                                            "new": { "type": "string" },
                                            "post_context": { "type": "string" }
                                        }
                                    }
                                }
                            },
                            "required": ["file_path", "hunks"]
                        }
                    }
                }
            }),
            Self::SearchCodebase => json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "path_filters": { "type": "array", "items": { "type": "string" } },
                    "codebase_path": { "type": "string" }
                },
                "required": ["query"]
            }),
            Self::Grep => json!({
                "type": "object",
                "properties": {
                    "queries": { "type": "array", "items": { "type": "string" } },
                    "query": { "type": "string", "description": "Alias for a single queries entry." },
                    "path": { "type": "string", "description": "Relative file or directory path to search." }
                },
                "required": ["queries", "path"]
            }),
            Self::FileGlob => json!({
                "type": "object",
                "properties": {
                    "patterns": { "type": "array", "items": { "type": "string" } },
                    "pattern": { "type": "string", "description": "Alias for a single patterns entry." },
                    "path": { "type": "string" }
                },
                "required": ["patterns", "path"]
            }),
            Self::FileGlobV2 => json!({
                "type": "object",
                "properties": {
                    "patterns": { "type": "array", "items": { "type": "string" } },
                    "pattern": { "type": "string", "description": "Alias for a single patterns entry." },
                    "search_dir": { "type": "string" },
                    "max_matches": { "type": "integer", "minimum": 0 },
                    "max_depth": { "type": "integer", "minimum": 0 },
                    "min_depth": { "type": "integer", "minimum": 0 }
                },
                "required": ["patterns", "search_dir"]
            }),
            Self::ReadMcpResource => json!({
                "type": "object",
                "properties": {
                    "uri": { "type": "string" },
                    "server_id": { "type": "string", "description": "ID of the MCP server that owns the resource." }
                },
                "required": ["uri"]
            }),
        }
    }

    fn openai_args_to_tool(self, args: &Value) -> Result<api::message::tool_call::Tool> {
        match self {
            Self::RunShellCommand => run_shell_command_from_args(args),
            Self::WriteToLongRunningShellCommand => {
                write_to_long_running_shell_command_from_args(args)
            }
            Self::ReadShellCommandOutput => read_shell_command_output_from_args(args),
            Self::ReadFiles => read_files_from_args(args),
            Self::ApplyFileDiffs => apply_file_diffs_from_args(args),
            Self::SearchCodebase => search_codebase_from_args(args),
            Self::Grep => grep_from_args(args),
            Self::FileGlob => file_glob_from_args(args),
            Self::FileGlobV2 => file_glob_v2_from_args(args),
            Self::ReadMcpResource => read_mcp_resource_from_args(args),
        }
    }
}

fn request_supports_tool(request: &api::Request, tool: api::ToolType) -> bool {
    let Some(settings) = request.settings.as_ref() else {
        return true;
    };
    settings.supported_tools.is_empty()
        || settings
            .supported_tools
            .iter()
            .any(|value| api::ToolType::try_from(*value).ok() == Some(tool))
}

fn run_shell_command_from_args(args: &Value) -> Result<api::message::tool_call::Tool> {
    let command = required_string(args, "command")?;
    let risk_category = optional_string(args, "risk_category")
        .as_deref()
        .map(risk_category_from_str)
        .transpose()?
        .unwrap_or_else(|| {
            if optional_bool(args, "is_risky").unwrap_or(false) {
                api::RiskCategory::Risky
            } else if optional_bool(args, "is_read_only").unwrap_or(false) {
                api::RiskCategory::ReadOnly
            } else {
                api::RiskCategory::Unspecified
            }
        });
    let wait_until_complete_value = optional_bool(args, "wait_until_complete").map(|value| {
        api::message::tool_call::run_shell_command::WaitUntilCompleteValue::WaitUntilComplete(value)
    });

    Ok(api::message::tool_call::Tool::RunShellCommand(
        api::message::tool_call::RunShellCommand {
            command,
            is_read_only: risk_category == api::RiskCategory::ReadOnly,
            uses_pager: optional_bool(args, "uses_pager").unwrap_or(false),
            citations: vec![],
            is_risky: risk_category == api::RiskCategory::Risky,
            risk_category: risk_category as i32,
            wait_until_complete_value,
        },
    ))
}

fn write_to_long_running_shell_command_from_args(
    args: &Value,
) -> Result<api::message::tool_call::Tool> {
    let mode = match optional_string(args, "mode")
        .unwrap_or_else(|| "raw".to_string())
        .as_str()
    {
        "raw" => api::message::tool_call::write_to_long_running_shell_command::mode::Mode::Raw(()),
        "line" => {
            api::message::tool_call::write_to_long_running_shell_command::mode::Mode::Line(())
        }
        "block" => {
            api::message::tool_call::write_to_long_running_shell_command::mode::Mode::Block(())
        }
        mode => bail!("unknown write mode `{mode}`"),
    };

    Ok(
        api::message::tool_call::Tool::WriteToLongRunningShellCommand(
            api::message::tool_call::WriteToLongRunningShellCommand {
                input: required_string(args, "input")?.into_bytes(),
                mode: Some(
                    api::message::tool_call::write_to_long_running_shell_command::Mode {
                        mode: Some(mode),
                    },
                ),
                command_id: required_string(args, "command_id")?,
            },
        ),
    )
}

fn read_shell_command_output_from_args(args: &Value) -> Result<api::message::tool_call::Tool> {
    let delay = if optional_bool(args, "on_completion").unwrap_or(false) {
        Some(api::message::tool_call::read_shell_command_output::Delay::OnCompletion(()))
    } else {
        optional_i64_checked(args, "delay_ms")?
            .map(|millis| {
                if millis < 0 {
                    bail!("`delay_ms` must be non-negative");
                }
                let nanos = i32::try_from((millis % 1000) * 1_000_000)
                    .map_err(|_| anyhow!("`delay_ms` nanos overflowed i32"))?;
                Ok(
                    api::message::tool_call::read_shell_command_output::Delay::Duration(
                        prost_types::Duration {
                            seconds: millis / 1000,
                            nanos,
                        },
                    ),
                )
            })
            .transpose()?
    };

    Ok(api::message::tool_call::Tool::ReadShellCommandOutput(
        api::message::tool_call::ReadShellCommandOutput {
            command_id: required_string(args, "command_id")?,
            delay,
        },
    ))
}

fn read_files_from_args(args: &Value) -> Result<api::message::tool_call::Tool> {
    let files = required_array(args, "files")?
        .iter()
        .map(|file| {
            let name = optional_string(file, "name")
                .or_else(|| optional_string(file, "path"))
                .ok_or_else(|| anyhow!("read_files entries require `name`"))?;
            let line_ranges = optional_array(file, "line_ranges")
                .into_iter()
                .flatten()
                .map(line_range_from_value)
                .collect::<Result<Vec<_>>>()?;
            Ok(api::message::tool_call::read_files::File { name, line_ranges })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(api::message::tool_call::Tool::ReadFiles(
        api::message::tool_call::ReadFiles { files },
    ))
}

fn apply_file_diffs_from_args(args: &Value) -> Result<api::message::tool_call::Tool> {
    let diffs = optional_array(args, "diffs")
        .into_iter()
        .flatten()
        .map(|diff| {
            Ok(api::message::tool_call::apply_file_diffs::FileDiff {
                file_path: required_string(diff, "file_path")?,
                search: required_string(diff, "search")?,
                replace: required_string(diff, "replace")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let new_files = optional_array(args, "new_files")
        .into_iter()
        .flatten()
        .map(|file| {
            Ok(api::message::tool_call::apply_file_diffs::NewFile {
                file_path: required_string(file, "file_path")?,
                content: required_string(file, "content")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let deleted_files = optional_array(args, "deleted_files")
        .into_iter()
        .flatten()
        .map(|file| {
            Ok(api::message::tool_call::apply_file_diffs::DeleteFile {
                file_path: required_string(file, "file_path")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let v4a_updates = optional_array(args, "v4a_updates")
        .into_iter()
        .flatten()
        .map(v4a_update_from_value)
        .collect::<Result<Vec<_>>>()?;

    Ok(api::message::tool_call::Tool::ApplyFileDiffs(
        api::message::tool_call::ApplyFileDiffs {
            summary: optional_string(args, "summary").unwrap_or_default(),
            diffs,
            new_files,
            deleted_files,
            v4a_updates,
        },
    ))
}

fn search_codebase_from_args(args: &Value) -> Result<api::message::tool_call::Tool> {
    Ok(api::message::tool_call::Tool::SearchCodebase(
        api::message::tool_call::SearchCodebase {
            query: required_string(args, "query")?,
            path_filters: optional_string_array(args, "path_filters")?,
            codebase_path: optional_string(args, "codebase_path").unwrap_or_default(),
        },
    ))
}

fn grep_from_args(args: &Value) -> Result<api::message::tool_call::Tool> {
    Ok(api::message::tool_call::Tool::Grep(
        api::message::tool_call::Grep {
            queries: string_array_or_single(args, "queries", "query")?,
            path: required_string(args, "path")?,
        },
    ))
}

#[allow(deprecated)]
fn file_glob_from_args(args: &Value) -> Result<api::message::tool_call::Tool> {
    Ok(api::message::tool_call::Tool::FileGlob(
        api::message::tool_call::FileGlob {
            patterns: string_array_or_single(args, "patterns", "pattern")?,
            path: required_string(args, "path")?,
        },
    ))
}

fn file_glob_v2_from_args(args: &Value) -> Result<api::message::tool_call::Tool> {
    Ok(api::message::tool_call::Tool::FileGlobV2(
        api::message::tool_call::FileGlobV2 {
            patterns: string_array_or_single(args, "patterns", "pattern")?,
            search_dir: required_string(args, "search_dir")?,
            max_matches: optional_non_negative_i32(args, "max_matches")?.unwrap_or_default(),
            max_depth: optional_non_negative_i32(args, "max_depth")?.unwrap_or_default(),
            min_depth: optional_non_negative_i32(args, "min_depth")?.unwrap_or_default(),
        },
    ))
}

fn read_mcp_resource_from_args(args: &Value) -> Result<api::message::tool_call::Tool> {
    Ok(api::message::tool_call::Tool::ReadMcpResource(
        api::message::tool_call::ReadMcpResource {
            uri: required_string(args, "uri")?,
            server_id: optional_string(args, "server_id").unwrap_or_default(),
        },
    ))
}

fn v4a_update_from_value(
    value: &Value,
) -> Result<api::message::tool_call::apply_file_diffs::V4aFileUpdate> {
    let hunks = optional_array(value, "hunks")
        .into_iter()
        .flatten()
        .map(|hunk| {
            Ok(
                api::message::tool_call::apply_file_diffs::v4a_file_update::Hunk {
                    change_context: optional_string_array(hunk, "change_context")?,
                    pre_context: optional_string(hunk, "pre_context").unwrap_or_default(),
                    old: optional_string(hunk, "old").unwrap_or_default(),
                    new: optional_string(hunk, "new").unwrap_or_default(),
                    post_context: optional_string(hunk, "post_context").unwrap_or_default(),
                },
            )
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(api::message::tool_call::apply_file_diffs::V4aFileUpdate {
        file_path: required_string(value, "file_path")?,
        move_to: optional_string(value, "move_to").unwrap_or_default(),
        hunks,
    })
}

fn line_range_from_value(value: &Value) -> Result<api::FileContentLineRange> {
    let start = required_positive_u32(value, "start")?;
    let end = required_positive_u32(value, "end")?;
    if end < start {
        bail!("`end` must be greater than or equal to `start`");
    }

    Ok(api::FileContentLineRange { start, end })
}

fn result_to_openai_content(result: &api::request::input::ToolCallResult) -> String {
    use api::request::input::tool_call_result::Result as ResultType;

    let value = match result.result.as_ref() {
        Some(ResultType::RunShellCommand(result)) => run_shell_command_result_to_json(result),
        Some(ResultType::WriteToLongRunningShellCommand(result)) => {
            write_to_long_running_shell_command_result_to_json(result)
        }
        Some(ResultType::ReadShellCommandOutput(result)) => {
            read_shell_command_output_result_to_json(result)
        }
        Some(ResultType::ReadFiles(result)) => read_files_result_to_json(result),
        Some(ResultType::ApplyFileDiffs(result)) => apply_file_diffs_result_to_json(result),
        Some(ResultType::SearchCodebase(result)) => search_codebase_result_to_json(result),
        Some(ResultType::Grep(result)) => grep_result_to_json(result),
        #[allow(deprecated)]
        Some(ResultType::FileGlob(result)) => file_glob_result_to_json(result),
        Some(ResultType::FileGlobV2(result)) => file_glob_v2_result_to_json(result),
        Some(ResultType::ReadMcpResource(result)) => read_mcp_resource_result_to_json(result),
        Some(ResultType::CallMcpTool(result)) => call_mcp_tool_result_to_json(result),
        Some(other) => json!({
            "status": "unsupported_result_type",
            "debug": format!("{other:?}"),
        }),
        None => json!({ "status": "cancelled_or_empty" }),
    };

    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
}

fn run_shell_command_result_to_json(result: &api::RunShellCommandResult) -> Value {
    use api::run_shell_command_result::Result as ShellResult;
    match &result.result {
        Some(ShellResult::CommandFinished(finished)) => json!({
            "status": "finished",
            "command": result.command,
            "command_id": finished.command_id,
            "output": finished.output,
            "exit_code": finished.exit_code,
        }),
        Some(ShellResult::LongRunningCommandSnapshot(snapshot)) => {
            long_running_snapshot_to_json("long_running_snapshot", Some(&result.command), snapshot)
        }
        Some(ShellResult::PermissionDenied(_)) => json!({
            "status": "permission_denied",
            "command": result.command,
        }),
        None => json!({ "status": "cancelled", "command": result.command }),
    }
}

fn write_to_long_running_shell_command_result_to_json(
    result: &api::WriteToLongRunningShellCommandResult,
) -> Value {
    use api::write_to_long_running_shell_command_result::Result as WriteResult;
    match &result.result {
        Some(WriteResult::CommandFinished(finished)) => {
            shell_finished_to_json("finished", finished)
        }
        Some(WriteResult::LongRunningCommandSnapshot(snapshot)) => {
            long_running_snapshot_to_json("long_running_snapshot", None, snapshot)
        }
        Some(WriteResult::Error(_)) => json!({ "status": "error", "message": "command not found" }),
        None => json!({ "status": "cancelled" }),
    }
}

fn read_shell_command_output_result_to_json(result: &api::ReadShellCommandOutputResult) -> Value {
    use api::read_shell_command_output_result::Result as ReadResult;
    match &result.result {
        Some(ReadResult::CommandFinished(finished)) => {
            let mut value = shell_finished_to_json("finished", finished);
            value["command"] = json!(result.command);
            value
        }
        Some(ReadResult::LongRunningCommandSnapshot(snapshot)) => {
            long_running_snapshot_to_json("long_running_snapshot", Some(&result.command), snapshot)
        }
        Some(ReadResult::Error(_)) => json!({
            "status": "error",
            "command": result.command,
            "message": "command not found",
        }),
        None => json!({ "status": "cancelled", "command": result.command }),
    }
}

fn read_files_result_to_json(result: &api::ReadFilesResult) -> Value {
    use api::read_files_result::Result as ReadResult;
    match &result.result {
        Some(ReadResult::TextFilesSuccess(success)) => json!({
            "status": "success",
            "files": success.files.iter().map(file_content_to_json).collect::<Vec<_>>(),
        }),
        Some(ReadResult::AnyFilesSuccess(success)) => json!({
            "status": "success",
            "files": success.files.iter().map(any_file_content_to_json).collect::<Vec<_>>(),
        }),
        Some(ReadResult::Error(error)) => json!({ "status": "error", "message": error.message }),
        None => json!({ "status": "cancelled" }),
    }
}

fn apply_file_diffs_result_to_json(result: &api::ApplyFileDiffsResult) -> Value {
    use api::apply_file_diffs_result::Result as ApplyResult;
    match &result.result {
        Some(ApplyResult::Success(success)) => json!({
            "status": "success",
            "updated_files": success.updated_files_v2.iter().filter_map(|file| file.file.as_ref()).map(file_content_to_json).collect::<Vec<_>>(),
            "deleted_files": success.deleted_files.iter().map(|file| file.file_path.clone()).collect::<Vec<_>>(),
        }),
        Some(ApplyResult::Error(error)) => json!({ "status": "error", "message": error.message }),
        None => json!({ "status": "cancelled" }),
    }
}

fn search_codebase_result_to_json(result: &api::SearchCodebaseResult) -> Value {
    use api::search_codebase_result::Result as SearchResult;
    match &result.result {
        Some(SearchResult::Success(success)) => json!({
            "status": "success",
            "files": success.files.iter().map(file_content_to_json).collect::<Vec<_>>(),
        }),
        Some(SearchResult::Error(error)) => json!({ "status": "error", "message": error.message }),
        None => json!({ "status": "cancelled" }),
    }
}

fn grep_result_to_json(result: &api::GrepResult) -> Value {
    use api::grep_result::Result as GrepResult;
    match &result.result {
        Some(GrepResult::Success(success)) => json!({
            "status": "success",
            "matched_files": success.matched_files.iter().map(|file| json!({
                "file_path": file.file_path,
                "matched_lines": file.matched_lines.iter().map(|line| line.line_number).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }),
        Some(GrepResult::Error(error)) => json!({ "status": "error", "message": error.message }),
        None => json!({ "status": "cancelled" }),
    }
}

#[allow(deprecated)]
fn file_glob_result_to_json(result: &api::FileGlobResult) -> Value {
    use api::file_glob_result::Result as GlobResult;
    match &result.result {
        Some(GlobResult::Success(success)) => json!({
            "status": "success",
            "matched_files": success.matched_files,
        }),
        Some(GlobResult::Error(error)) => json!({ "status": "error", "message": error.message }),
        None => json!({ "status": "cancelled" }),
    }
}

fn file_glob_v2_result_to_json(result: &api::FileGlobV2Result) -> Value {
    use api::file_glob_v2_result::Result as GlobResult;
    match &result.result {
        Some(GlobResult::Success(success)) => json!({
            "status": "success",
            "matched_files": success.matched_files.iter().map(|file| file.file_path.clone()).collect::<Vec<_>>(),
            "warnings": success.warnings,
        }),
        Some(GlobResult::Error(error)) => json!({ "status": "error", "message": error.message }),
        None => json!({ "status": "cancelled" }),
    }
}

fn read_mcp_resource_result_to_json(result: &api::ReadMcpResourceResult) -> Value {
    use api::read_mcp_resource_result::Result as ReadResult;
    match &result.result {
        Some(ReadResult::Success(success)) => json!({
            "status": "success",
            "contents": success.contents.iter().map(mcp_resource_content_to_json).collect::<Vec<_>>(),
        }),
        Some(ReadResult::Error(error)) => json!({ "status": "error", "message": error.message }),
        None => json!({ "status": "cancelled" }),
    }
}

fn call_mcp_tool_result_to_json(result: &api::CallMcpToolResult) -> Value {
    use api::call_mcp_tool_result::Result as CallResult;
    match &result.result {
        Some(CallResult::Success(success)) => json!({
            "status": "success",
            "results": success.results.iter().map(mcp_tool_result_to_json).collect::<Vec<_>>(),
        }),
        Some(CallResult::Error(error)) => json!({ "status": "error", "message": error.message }),
        None => json!({ "status": "cancelled" }),
    }
}

fn shell_finished_to_json(status: &str, finished: &api::ShellCommandFinished) -> Value {
    json!({
        "status": status,
        "command_id": finished.command_id,
        "output": finished.output,
        "exit_code": finished.exit_code,
    })
}

fn long_running_snapshot_to_json(
    status: &str,
    command: Option<&str>,
    snapshot: &api::LongRunningShellCommandSnapshot,
) -> Value {
    json!({
        "status": status,
        "command": command.unwrap_or_default(),
        "command_id": snapshot.command_id,
        "output": snapshot.output,
        "cursor": snapshot.cursor,
        "is_alt_screen_active": snapshot.is_alt_screen_active,
        "is_preempted": snapshot.is_preempted,
    })
}

fn file_content_to_json(file: &api::FileContent) -> Value {
    json!({
        "file_path": file.file_path,
        "content": file.content,
        "line_range": file.line_range.as_ref().map(|range| json!({ "start": range.start, "end": range.end })),
    })
}

fn any_file_content_to_json(file: &api::AnyFileContent) -> Value {
    match &file.content {
        Some(api::any_file_content::Content::TextContent(text)) => file_content_to_json(text),
        Some(api::any_file_content::Content::BinaryContent(binary)) => json!({
            "file_path": binary.file_path,
            "binary_bytes": binary.data.len(),
            "content": "[binary file omitted]",
        }),
        None => json!({ "content": "[empty file result]" }),
    }
}

fn mcp_resource_content_to_json(content: &api::McpResourceContent) -> Value {
    match &content.content_type {
        Some(api::mcp_resource_content::ContentType::Text(text)) => json!({
            "uri": content.uri,
            "mime_type": text.mime_type,
            "text": text.content,
        }),
        Some(api::mcp_resource_content::ContentType::Binary(binary)) => json!({
            "uri": content.uri,
            "mime_type": binary.mime_type,
            "binary_bytes": binary.data.len(),
            "content": "[binary MCP resource omitted]",
        }),
        None => json!({ "uri": content.uri, "text": "" }),
    }
}

fn mcp_tool_result_to_json(result: &api::call_mcp_tool_result::success::Result) -> Value {
    use api::call_mcp_tool_result::success::result::Result as ToolResult;
    match &result.result {
        Some(ToolResult::Text(text)) => json!({ "type": "text", "text": text.text }),
        Some(ToolResult::Image(image)) => json!({
            "type": "image",
            "mime_type": image.mime_type,
            "bytes": image.data.len(),
            "content": "[image omitted]",
        }),
        Some(ToolResult::Resource(resource)) => {
            let mut value = mcp_resource_content_to_json(resource);
            value["type"] = json!("resource");
            value
        }
        None => json!({ "type": "empty", "text": "" }),
    }
}

fn parse_arguments(arguments: &str) -> Result<Value> {
    if arguments.trim().is_empty() {
        return Ok(json!({}));
    }
    let value: Value = serde_json::from_str(arguments)
        .map_err(|error| anyhow!("failed to parse tool arguments as JSON: {error}"))?;
    if !value.is_object() {
        bail!("tool arguments must be a JSON object");
    }
    Ok(value)
}

fn required_string(value: &Value, key: &str) -> Result<String> {
    optional_string(value, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("missing required string `{key}`"))
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|value| match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    })
}

fn required_i64(value: &Value, key: &str) -> Result<i64> {
    optional_i64(value, key).ok_or_else(|| anyhow!("missing required integer `{key}`"))
}

fn optional_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|value| match value {
        Value::Number(value) => value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(value) => value.parse::<i64>().ok(),
        _ => None,
    })
}

fn required_positive_u32(value: &Value, key: &str) -> Result<u32> {
    let raw = required_i64(value, key)?;
    if raw < 1 {
        bail!("`{key}` must be greater than or equal to 1");
    }
    u32::try_from(raw).map_err(|_| anyhow!("`{key}` must be less than or equal to u32::MAX"))
}

fn optional_non_negative_i32(value: &Value, key: &str) -> Result<Option<i32>> {
    let Some(raw) = optional_i64_checked(value, key)? else {
        return Ok(None);
    };
    if raw < 0 {
        bail!("`{key}` must be non-negative");
    }
    Ok(Some(i32::try_from(raw).map_err(|_| {
        anyhow!("`{key}` must be less than or equal to i32::MAX")
    })?))
}

fn optional_i64_checked(value: &Value, key: &str) -> Result<Option<i64>> {
    let Some(value) = value.get(key) else {
        return Ok(None);
    };
    match value {
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                Ok(Some(value))
            } else if let Some(value) = number.as_u64() {
                Ok(Some(i64::try_from(value).map_err(|_| {
                    anyhow!("`{key}` must be less than or equal to i64::MAX")
                })?))
            } else {
                bail!("`{key}` must be an integer");
            }
        }
        Value::String(value) => value
            .parse::<i64>()
            .map(Some)
            .map_err(|_| anyhow!("`{key}` must be an integer")),
        _ => bail!("`{key}` must be an integer"),
    }
}

fn optional_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(|value| match value {
        Value::Bool(value) => Some(*value),
        Value::String(value) => value.parse::<bool>().ok(),
        _ => None,
    })
}

fn required_array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    optional_array(value, key).ok_or_else(|| anyhow!("missing required array `{key}`"))
}

fn optional_array<'a>(value: &'a Value, key: &str) -> Option<&'a Vec<Value>> {
    value.get(key).and_then(Value::as_array)
}

fn optional_string_array(value: &Value, key: &str) -> Result<Vec<String>> {
    optional_array(value, key)
        .into_iter()
        .flatten()
        .map(|value| match value {
            Value::String(value) => Ok(value.clone()),
            _ => bail!("`{key}` entries must be strings"),
        })
        .collect()
}

fn string_array_or_single(value: &Value, array_key: &str, single_key: &str) -> Result<Vec<String>> {
    let mut values = optional_string_array(value, array_key)?;
    if values.is_empty()
        && let Some(single) = optional_string(value, single_key)
    {
        values.push(single);
    }
    if values.is_empty() {
        bail!("missing required string array `{array_key}`");
    }
    Ok(values)
}

fn risk_category_from_str(value: &str) -> Result<api::RiskCategory> {
    match value.trim().to_ascii_lowercase().as_str() {
        "read_only" | "readonly" => Ok(api::RiskCategory::ReadOnly),
        "trivial_local_change" | "trivial" => Ok(api::RiskCategory::TrivialLocalChange),
        "nontrivial_local_change" | "nontrivial" => Ok(api::RiskCategory::NontrivialLocalChange),
        "external_change" | "external" => Ok(api::RiskCategory::ExternalChange),
        "risky" => Ok(api::RiskCategory::Risky),
        "unspecified" | "unknown" => Ok(api::RiskCategory::Unspecified),
        other => bail!("unknown risk_category `{other}`"),
    }
}

fn json_to_struct(value: &Value) -> Result<prost_types::Struct> {
    let Some(map) = value.as_object() else {
        bail!("expected JSON object for struct conversion");
    };
    Ok(prost_types::Struct {
        fields: map
            .iter()
            .map(|(key, value)| (key.clone(), json_to_prost_value(value)))
            .collect(),
    })
}

fn json_to_prost_value(value: &Value) -> prost_types::Value {
    use prost_types::value::Kind;
    prost_types::Value {
        kind: Some(match value {
            Value::Null => Kind::NullValue(0),
            Value::Bool(value) => Kind::BoolValue(*value),
            Value::Number(value) => Kind::NumberValue(value.as_f64().unwrap_or_default()),
            Value::String(value) => Kind::StringValue(value.clone()),
            Value::Array(values) => Kind::ListValue(prost_types::ListValue {
                values: values.iter().map(json_to_prost_value).collect(),
            }),
            Value::Object(map) => Kind::StructValue(prost_types::Struct {
                fields: map
                    .iter()
                    .map(|(key, value)| (key.clone(), json_to_prost_value(value)))
                    .collect(),
            }),
        }),
    }
}

fn struct_to_json_value(value: &prost_types::Struct) -> Value {
    Value::Object(
        value
            .fields
            .iter()
            .map(|(key, value)| (key.clone(), prost_value_to_json(value)))
            .collect(),
    )
}

fn prost_value_to_json(value: &prost_types::Value) -> Value {
    use prost_types::value::Kind;
    match value.kind.as_ref() {
        Some(Kind::NullValue(_)) | None => Value::Null,
        Some(Kind::NumberValue(value)) => serde_json::Number::from_f64(*value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        Some(Kind::StringValue(value)) => Value::String(value.clone()),
        Some(Kind::BoolValue(value)) => Value::Bool(*value),
        Some(Kind::StructValue(value)) => struct_to_json_value(value),
        Some(Kind::ListValue(value)) => {
            Value::Array(value.values.iter().map(prost_value_to_json).collect())
        }
    }
}

fn empty_object_schema() -> Value {
    json!({
        "type": "object",
        "properties": {}
    })
}

fn openai_compatible_object_schema(schema: Value) -> Value {
    let Value::Object(mut object) = schema else {
        tracing::warn!("dropping non-object tool schema before sending it upstream");
        return empty_object_schema();
    };

    match object.get("type").and_then(Value::as_str) {
        Some("object") => {}
        None => {
            object.insert("type".to_string(), json!("object"));
        }
        Some(schema_type) => {
            tracing::warn!(
                schema_type,
                "dropping non-object tool schema before sending it upstream"
            );
            return empty_object_schema();
        }
    }

    if !matches!(object.get("properties"), Some(Value::Object(_))) {
        object.insert("properties".to_string(), json!({}));
    }

    let property_names = object
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| properties.keys().cloned().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    let required_is_valid = object
        .get("required")
        .and_then(Value::as_array)
        .map(|required| {
            required.iter().all(|value| {
                value
                    .as_str()
                    .is_some_and(|name| property_names.contains(name))
            })
        })
        .unwrap_or(true);
    if !required_is_valid {
        tracing::warn!("dropping invalid tool schema `required` list before sending it upstream");
        object.remove("required");
    }

    Value::Object(object)
}

fn sanitize_openai_name_part(value: &str, max_len: usize) -> String {
    let mut sanitized = String::new();
    let mut previous_was_separator = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch);
            previous_was_separator = false;
        } else if !previous_was_separator {
            sanitized.push('_');
            previous_was_separator = true;
        }
    }
    let sanitized = sanitized.trim_matches('_');
    let sanitized = if sanitized.is_empty() {
        "tool".to_string()
    } else {
        sanitized.to_string()
    };
    sanitized.chars().take(max_len).collect()
}

fn unique_tool_name(base_name: String, used_names: &mut BTreeSet<String>) -> String {
    let mut candidate: String = base_name.chars().take(64).collect();
    if used_names.insert(candidate.clone()) {
        return candidate;
    }

    let mut suffix_number = 2usize;
    loop {
        let suffix = format!("_{suffix_number}");
        let prefix_len = 64usize.saturating_sub(suffix.len());
        candidate = format!(
            "{}{}",
            base_name.chars().take(prefix_len).collect::<String>(),
            suffix
        );
        if used_names.insert(candidate.clone()) {
            return candidate;
        }
        suffix_number += 1;
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
#[path = "tool_registry_tests.rs"]
mod tests;
