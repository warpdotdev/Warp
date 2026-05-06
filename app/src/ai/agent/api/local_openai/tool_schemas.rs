//! OpenAI function schema registry for Warp's built-in local backend tools.

use serde_json::{Map, Value, json};
use warp_multi_agent_api as api;

/// Returns the OpenAI function schema for a built-in Warp tool, if one is exposed locally.
pub(super) fn built_in_tool_schema(tool_type: api::ToolType) -> Option<Value> {
    match tool_type {
        api::ToolType::RunShellCommand => Some(function_schema(
            "run_shell_command",
            "Execute a shell command on user's machine.\n\nTwo modes of execution are available:\n- 'wait': The command runs to completion. Use this for short-lived, non-interactive commands where you only need the final output.\n- 'interact': The command is executed and control is handed off to a subagent that can monitor or interact with it in real time. Use this for long-running or interactive processes such as REPLs, dev servers, or TUIs.",
            json!({
                "type": "object",
                "properties": {
                    "command": string_schema(
                        "A shell command to execute.\n\nAny parameters should be enclosed in double braces, e.g. {{param_name}}. Take into account the user's shell type - for example, do not request unix commands if the user is running Powershell (pwsh) on Windows."
                    ),
                    "mode": enum_schema(
                        "Determines the execution \"mode\" for the command. Must be oneof [\"wait\", \"interact\"].",
                        ["wait", "interact"],
                    ),
                    "is_read_only": boolean_schema(
                        "Whether the shell command is fully read-only and does not produce side-effects. If you are unsure, return false."
                    ),
                    "uses_pager": boolean_schema(
                        "Whether the shell command might use a pager. You MUST set this to true for commands like git log, git diff, less, man.\nMAKE SURE you set uses_pager to true if a command might cause pagination. Otherwise, the command will break. If you are unsure, return true."
                    ),
                    "is_risky": boolean_schema(
                        "Whether the shell command could produce dangerous or unwanted side effects. If you are unsure, return true."
                    ),
                    "interact_task": string_schema(
                        "Specify only when mode is \"interact\". The task instructions to be given to the subagent."
                    ),
                    "wait_params": object_schema(
                        vec![
                            (
                                "reason",
                                string_schema(
                                    "Explain WHY you're running the command and WHAT specific information you need from the output. Use no more than one sentence."
                                ),
                            ),
                            (
                                "do_not_summarize_output",
                                boolean_schema(
                                    "Whether to disable condensation of large command output. If unsure, use false."
                                ),
                            ),
                            (
                                "expected_duration_seconds",
                                number_schema(
                                    "How long you expect the command to take to complete in seconds. Optional but recommended."
                                ),
                            ),
                        ],
                        ["reason", "do_not_summarize_output"],
                    ),
                    "citations": json!({
                        "type": "object",
                        "description": "A list of citations for the command. These MUST be populated if the command was derived from external context OR any of the user's rules. These MUST be formatted in JSON.",
                        "properties": {
                            "documents": {
                                "type": "array",
                                "items": object_schema(
                                    vec![
                                        ("document_type", string_schema("The citation document type.")),
                                        ("document_id", string_schema("The citation document identifier.")),
                                    ],
                                    ["document_type", "document_id"],
                                ),
                            }
                        },
                        "additionalProperties": false
                    }),
                },
                "required": ["command", "is_read_only", "uses_pager", "is_risky", "mode"],
                "additionalProperties": false,
            }),
            false,
        )),
        api::ToolType::ReadFiles => Some(function_schema(
            "read_files",
            "Reads the contents of specified files from the local filesystem. You can access any file directly by using this tool.",
            json!({
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "description": "A list of files to read.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": string_schema("Absolute path to the file."),
                                "ranges": {
                                    "type": "array",
                                    "description": "Optional list of specific, non-overlapping line ranges to be retrieved. Each range should be formatted as a string \"start-end\". Omit to retrieve the entire file.",
                                    "items": { "type": "string" }
                                }
                            },
                            "required": ["path"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["files"],
                "additionalProperties": false
            }),
            false,
        )),
        api::ToolType::SearchCodebase => Some(function_schema(
            "search_codebase",
            "Search the indexed codebase for relevant files.",
            object_schema(
                vec![
                    (
                        "query",
                        string_schema(
                            "The semantic search query to run against the indexed codebase.",
                        ),
                    ),
                    (
                        "path_filters",
                        array_schema(
                            "Optional path filters used to narrow the search to specific areas.",
                            string_schema("A path prefix or partial path filter."),
                        ),
                    ),
                    (
                        "codebase_path",
                        string_schema(
                            "Optional workspace path identifying which indexed codebase to search.",
                        ),
                    ),
                ],
                ["query"],
            ),
            false,
        )),
        api::ToolType::Grep => Some(function_schema(
            "grep",
            "A powerful and fast search tool that operates like grep.",
            object_schema(
                vec![
                    (
                        "queries",
                        array_schema(
                            "A list of search terms or patterns to look for within files. The search will match any of the provided queries. Each query in the list is interpreted as an Extended Regular Expression (ERE).",
                            string_schema(
                                "A search query or ERE pattern to match in file contents.",
                            ),
                        ),
                    ),
                    (
                        "path",
                        string_schema(
                            "The absolute path to the directory to search in.\n\nThe path must be a directory, not a file. Directories are searched recursively.",
                        ),
                    ),
                ],
                ["queries", "path"],
            ),
            true,
        )),
        api::ToolType::FileGlob => None,
        api::ToolType::FileGlob | api::ToolType::FileGlobV2 => Some(function_schema(
            "file_glob",
            "Usage:\n- Use this tool when you need to find files by name patterns rather than content.\n- Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\".\n- Does not match directories (like `find -type f`).",
            json!({
                "type": "object",
                "properties": {
                    "patterns": {
                        "type": "array",
                        "description": "The regex patterns to match files against. Multiple patterns can be provided to match different file types. Only basic *, ?, [ ] patterns are supported.",
                        "items": { "type": "string" }
                    },
                    "search_dir": string_schema("The absolute path to the directory to search in.\n\nThe path must be a directory, not a file. Directories are searched recursively. If not provided, the current working directory will be used."),
                    "max_matches": integer_schema("The maximum number of matches to list. If 0, the default is unlimited."),
                    "max_depth": integer_schema("The maximum depth to search. If 0, the default is unlimited."),
                    "min_depth": integer_schema("The minimum depth to search. If 0, the default is no minimum."),
                },
                "required": ["patterns", "search_dir", "max_matches", "max_depth", "min_depth"],
                "additionalProperties": false
            }),
            false,
        )),
        api::ToolType::ApplyFileDiffs => Some(function_schema(
            "apply_file_diffs",
            "Create, edit, move, or delete files in the workspace.",
            object_schema(
                vec![
                    (
                        "summary",
                        string_schema("A short summary of the intended file changes."),
                    ),
                    (
                        "diffs",
                        array_schema(
                            "Search-and-replace edits to apply to existing files.",
                            object_schema(
                                vec![
                                    ("file_path", string_schema("The file to update.")),
                                    (
                                        "search",
                                        string_schema("The exact text to find in the file."),
                                    ),
                                    ("replace", string_schema("The replacement text to write.")),
                                ],
                                ["file_path", "search", "replace"],
                            ),
                        ),
                    ),
                    (
                        "new_files",
                        array_schema(
                            "New files to create.",
                            object_schema(
                                vec![
                                    (
                                        "file_path",
                                        string_schema("The path of the new file to create."),
                                    ),
                                    ("content", string_schema("The full file contents to write.")),
                                ],
                                ["file_path", "content"],
                            ),
                        ),
                    ),
                    (
                        "deleted_files",
                        array_schema(
                            "Existing files to delete.",
                            object_schema(
                                vec![(
                                    "file_path",
                                    string_schema("The path of the file to delete."),
                                )],
                                ["file_path"],
                            ),
                        ),
                    ),
                    (
                        "v4a_updates",
                        array_schema(
                            "Structured V4A patch updates for advanced file edits and moves.",
                            object_schema(
                                vec![
                                    ("file_path", string_schema("The file to update.")),
                                    (
                                        "move_to",
                                        string_schema(
                                            "Optional new destination path if the file should be moved.",
                                        ),
                                    ),
                                    (
                                        "hunks",
                                        array_schema(
                                            "Patch hunks to apply to the target file.",
                                            object_schema(
                                                vec![
                                                    (
                                                        "change_context",
                                                        array_schema(
                                                            "Optional contextual lines associated with the change.",
                                                            string_schema("A context line."),
                                                        ),
                                                    ),
                                                    (
                                                        "pre_context",
                                                        string_schema(
                                                            "Context immediately before the changed block.",
                                                        ),
                                                    ),
                                                    (
                                                        "old",
                                                        string_schema(
                                                            "The original text being replaced.",
                                                        ),
                                                    ),
                                                    (
                                                        "new",
                                                        string_schema(
                                                            "The new text that should replace the original.",
                                                        ),
                                                    ),
                                                    (
                                                        "post_context",
                                                        string_schema(
                                                            "Context immediately after the changed block.",
                                                        ),
                                                    ),
                                                ],
                                                [],
                                            ),
                                        ),
                                    ),
                                ],
                                ["file_path", "hunks"],
                            ),
                        ),
                    ),
                ],
                [],
            ),
            false,
        )),
        api::ToolType::ReadMcpResource => Some(function_schema(
            "read_mcp_resource",
            "Read a resource exposed by an MCP server.",
            object_schema(
                vec![
                    ("uri", string_schema("The MCP resource URI to read.")),
                    (
                        "server_id",
                        string_schema(
                            "Optional MCP server identifier when multiple servers are active.",
                        ),
                    ),
                ],
                ["uri"],
            ),
            false,
        )),
        api::ToolType::WriteToLongRunningShellCommand => Some(function_schema(
            "write_to_long_running_shell_command",
            "Send input to a running shell command.",
            object_schema(
                vec![
                    (
                        "command_id",
                        string_schema(
                            "The long-running command identifier previously returned by Warp.",
                        ),
                    ),
                    (
                        "input",
                        string_schema("The raw text or bytes to send to the running command."),
                    ),
                    (
                        "mode",
                        enum_schema(
                            "How Warp should write the provided input to the running command.",
                            ["raw", "line", "block"],
                        ),
                    ),
                ],
                ["command_id", "input"],
            ),
            false,
        )),
        api::ToolType::ReadShellCommandOutput => Some(function_schema(
            "read_shell_command_output",
            "Read the output of a running or completed shell command.",
            object_schema(
                vec![
                    (
                        "command_id",
                        string_schema("The command identifier previously returned by Warp."),
                    ),
                    (
                        "delay_seconds",
                        integer_schema(
                            "Optional delay in whole seconds before Warp returns command output.",
                        ),
                    ),
                    (
                        "on_completion",
                        boolean_schema(
                            "If true, wait until the command completes before returning output.",
                        ),
                    ),
                ],
                ["command_id"],
            ),
            false,
        )),
        api::ToolType::SuggestNewConversation => Some(function_schema(
            "suggest_new_conversation",
            "Suggest starting a new conversation from a specific message.",
            object_schema(
                vec![(
                    "message_id",
                    string_schema(
                        "The message that should become the split point for a new conversation.",
                    ),
                )],
                ["message_id"],
            ),
            true,
        )),
        api::ToolType::ReadDocuments => Some(function_schema(
            "read_documents",
            "Read one or more Warp AI documents.",
            object_schema(
                vec![(
                    "documents",
                    array_schema(
                        "The documents and optional line ranges to read.",
                        object_schema(
                            vec![
                                (
                                    "document_id",
                                    string_schema("The document identifier to read."),
                                ),
                                (
                                    "line_ranges",
                                    array_schema(
                                        "Optional 1-based line ranges to read. If omitted, Warp reads the entire document.",
                                        object_schema(
                                            vec![
                                                (
                                                    "start",
                                                    integer_schema(
                                                        "The inclusive 1-based starting line number.",
                                                    ),
                                                ),
                                                (
                                                    "end",
                                                    integer_schema(
                                                        "The inclusive 1-based ending line number.",
                                                    ),
                                                ),
                                            ],
                                            ["start", "end"],
                                        ),
                                    ),
                                ),
                            ],
                            ["document_id"],
                        ),
                    ),
                )],
                ["documents"],
            ),
            false,
        )),
        api::ToolType::EditDocuments => Some(function_schema(
            "edit_documents",
            "Edit one or more existing Warp AI documents.",
            object_schema(
                vec![(
                    "diffs",
                    array_schema(
                        "Search-and-replace edits to apply to existing documents.",
                        object_schema(
                            vec![
                                (
                                    "document_id",
                                    string_schema("The document identifier to update."),
                                ),
                                (
                                    "search",
                                    string_schema("The exact text to find in the document."),
                                ),
                                ("replace", string_schema("The replacement text to write.")),
                            ],
                            ["document_id", "search", "replace"],
                        ),
                    ),
                )],
                ["diffs"],
            ),
            true,
        )),
        api::ToolType::CreateDocuments => Some(function_schema(
            "create_documents",
            "Create one or more new Warp AI documents.",
            object_schema(
                vec![(
                    "new_documents",
                    array_schema(
                        "The documents to create.",
                        object_schema(
                            vec![
                                (
                                    "content",
                                    string_schema("The full contents of the new document."),
                                ),
                                (
                                    "title",
                                    string_schema(
                                        "An optional human-readable title for the new document.",
                                    ),
                                ),
                            ],
                            ["content"],
                        ),
                    ),
                )],
                ["new_documents"],
            ),
            false,
        )),
        api::ToolType::SuggestPrompt => Some(function_schema(
            "suggest_prompt",
            "Suggest a prompt for the user to run.",
            object_schema(
                vec![
                    (
                        "display_mode",
                        enum_schema(
                            "The UI presentation mode for the suggestion.",
                            ["inline_query_banner", "prompt_chip"],
                        ),
                    ),
                    (
                        "title",
                        string_schema("The title shown for an inline query banner suggestion."),
                    ),
                    (
                        "description",
                        string_schema(
                            "The descriptive text shown for an inline query banner suggestion.",
                        ),
                    ),
                    (
                        "query",
                        string_schema("The query used when display_mode is inline_query_banner."),
                    ),
                    (
                        "prompt",
                        string_schema("The prompt used when display_mode is prompt_chip."),
                    ),
                    (
                        "label",
                        string_schema("An optional shorter UI label used for a prompt chip."),
                    ),
                    (
                        "is_trigger_irrelevant",
                        boolean_schema(
                            "Whether the original trigger is unrelated to the suggestion itself.",
                        ),
                    ),
                ],
                ["display_mode"],
            ),
            false,
        )),
        api::ToolType::OpenCodeReview => Some(function_schema(
            "open_code_review",
            "Trigger the client to open the code review pane.",
            object_schema(vec![], []),
            true,
        )),
        api::ToolType::InsertReviewComments => Some(function_schema(
            "insert_review_comments",
            "Send code review comments to the user so that they can be displayed in the appropriate UI.",
            json!({
                "type": "object",
                "properties": {
                    "local_repository_path": string_schema("The absolute path of the repository on the user's machine."),
                    "base_branch": string_schema("The name of the base branch the PR is targeting (e.g. \"main\", \"develop\")."),
                    "comments": {
                        "type": "array",
                        "description": "A list of code review comments to display to the user.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "comment_id": string_schema("Unique identifier for the review comment."),
                                "author": string_schema("The author of the comment."),
                                "last_modified_timestamp": string_schema("Timestamp when the comment was last modified."),
                                "comment_body": string_schema("The content of the review comment."),
                                "html_url": string_schema("The URL to view this comment in GitHub's web UI."),
                            },
                            "required": ["comment_id", "author", "last_modified_timestamp", "comment_body", "html_url"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["local_repository_path", "base_branch", "comments"],
                "additionalProperties": false
            }),
            false,
        )),
        api::ToolType::InitProject => Some(function_schema(
            "init_project",
            "Initialize the project setup flow on the client.",
            object_schema(vec![], []),
            true,
        )),
        api::ToolType::FetchConversation => Some(function_schema(
            "fetch_conversation",
            "Fetch tasks from another or the current conversation.",
            object_schema(
                vec![(
                    "conversation_id",
                    string_schema(
                        "Optional conversation identifier to fetch. Leave empty to target the current conversation.",
                    ),
                )],
                [],
            ),
            false,
        )),
        api::ToolType::ReadSkill => Some(function_schema(
            "read_skill",
            "Read a skill by identifier to get its content and instructions.\n\nYou only need to provide one of bundled_skill_id and skill_path.",
            object_schema(
                vec![
                    (
                        "skill_path",
                        string_schema("The path to a SKILL.md file to load."),
                    ),
                    (
                        "bundled_skill_id",
                        string_schema("The identifier of a skill bundled with the client."),
                    ),
                ],
                [],
            ),
            false,
        )),
        api::ToolType::AskUserQuestion => Some(function_schema(
            "ask_user_question",
            "Ask the user one or more clarifying questions.",
            json!({
                "type": "object",
                "properties": {
                    "questions": {
                        "type": "array",
                        "description": "A single clarifying question to present to the user.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "question": string_schema("The question text to present to the user."),
                                "options": {
                                    "type": "array",
                                    "description": "The list of selectable options. Must contain at least 2 options. Do NOT include an \"Other\" option.",
                                    "items": { "type": "string" }
                                },
                                "recommended_option_index": integer_schema("Zero-based index into options identifying the recommended choice. Only valid for single_select questions; do not set for multi_select. Omit if no recommendation."),
                                "type": enum_schema(
                                    "Whether the user picks exactly one option (\"single_select\") or may pick several (\"multi_select\").",
                                    ["single_select", "multi_select"],
                                ),
                            },
                            "required": ["question", "options", "type"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["questions"],
                "additionalProperties": false
            }),
            false,
        )),
        // MCP tools are exposed through their own rich per-server schemas instead of this generic shell.
        api::ToolType::CallMcpTool => None,
        _ => None,
    }
}

/// Builds an OpenAI function tool definition for the Responses API.
fn function_schema(name: &str, description: &str, parameters: Value, strict: bool) -> Value {
    json!({
        "type": "function",
        "name": name,
        "description": description,
        "parameters": parameters,
        "strict": strict,
    })
}

/// Builds a JSON Schema object type with the provided properties and required keys.
fn object_schema<const N: usize>(properties: Vec<(&str, Value)>, required: [&str; N]) -> Value {
    let properties = properties
        .into_iter()
        .map(|(name, value)| (name.to_string(), value))
        .collect::<Map<String, Value>>();
    let required = required.into_iter().map(str::to_string).collect::<Vec<_>>();

    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

/// Builds a JSON Schema string field with a description.
fn string_schema(description: &str) -> Value {
    json!({
        "type": "string",
        "description": description,
    })
}

/// Builds a JSON Schema boolean field with a description.
fn boolean_schema(description: &str) -> Value {
    json!({
        "type": "boolean",
        "description": description,
    })
}

/// Builds a JSON Schema integer field with a description.
fn integer_schema(description: &str) -> Value {
    json!({
        "type": "integer",
        "description": description,
    })
}

/// Builds a JSON Schema number field with a description.
fn number_schema(description: &str) -> Value {
    json!({
        "type": "number",
        "description": description,
    })
}

/// Builds a JSON Schema enum field with a description.
fn enum_schema<const N: usize>(description: &str, values: [&str; N]) -> Value {
    let values = values.into_iter().map(str::to_string).collect::<Vec<_>>();
    json!({
        "type": "string",
        "description": description,
        "enum": values,
    })
}

/// Builds a JSON Schema array field with a description.
fn array_schema(description: &str, items: Value) -> Value {
    json!({
        "type": "array",
        "description": description,
        "items": items,
    })
}
