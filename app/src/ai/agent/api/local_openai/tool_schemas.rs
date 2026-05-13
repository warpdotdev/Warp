//! OpenAI function schema registry for Warp's built-in local backend tools.

use serde_json::{Map, Value, json};
use warp_multi_agent_api as api;

/// Returns the OpenAI function schema for a built-in Warp tool, if one is exposed locally.
pub(super) fn built_in_tool_schema(tool_type: api::ToolType) -> Option<Value> {
    match tool_type {
        api::ToolType::RunShellCommand => Some(function_schema(
            "run_shell_command",
            "Execute a shell command on user's machine.\n\nTwo modes of execution are available:\n- 'wait': The command runs to completion. Use this for short-lived, non-interactive commands where you only need the final output.\n- 'interact': The command is executed and control is handed off to a subagent that can monitor or interact with it in real time. Use this for long-running or interactive processes such as REPLs, dev servers, or TUIs.\n\nUsage:\n- Use versions of commands that guarantee non-paginated output where possible. For example, when using git commands that might have paginated output, always use the `--no-pager` option.\n- NEVER run a command that will end the active shell process. Do NOT run a command that puts the shell into strict mode, (e.g. `set -e`, `set -u`), as errors in subsequent commands will exit the shell. You may use these commands in non-execution contexts (e.g. writing `set -e` to a file) or in scripts (e.g. running a script that contains `set -e`).\n- Try to maintain your current working directory throughout the session by using absolute paths and avoiding usage of `cd`. You may use `cd` if the User explicitly requests it or it makes sense to do so. <good_example>pytest /foo/bar/tests</good_example> <bad_example>cd /foo/bar && pytest tests</bad_example>\n- If available, use the `grep` and `file_glob` tools directly, rather than using this tool with commands like `find` and `grep`.\n- If available, always prefer reading files using the `read_files` tool over reading via CLI commands (e.g. `cat`, `head`, `tail`).\n- If available, prefer editing files with the `edit_files` tool. Only use CLI commands to edit if it is more efficient (e.g. a sed-based find-and-replace across a large codebase).\n- Do not use the `echo` terminal command to output text for the user to read. You should fully output your response to the user separately from any tool calls.\n- If you need to fetch the contents of a URL, you can use a command to do so (e.g. `curl`), only if the URL seems safe.\n- DO NOT pipe command output into tools like `head` or `tail`. If the command output is too large, the results will be truncated or summarized for you.\n- IMPORTANT: NEVER suggest malicious or harmful commands, full stop.\n- IMPORTANT: Bias strongly against unsafe commands, unless the user has explicitly asked you to execute a process that necessitates running an unsafe command. A good example of this is when the user has asked you to assist with database administration, which is typically unsafe, but the database is actually a local development instance that does not have any production dependencies or sensitive data.\n- IMPORTANT: the user may edit the command before running it. If you receive a result that indicates the user has manually edited the command, DO NOT IGNORE these modifications. Even if the user's changes seem inconsistent with your original intent, you must respect and preserve them. Treat the modified command as the source of truth and adjust your reasoning accordingly.",
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
                        "Whether the shell command might use a pager. You MUST set this to true for commands like git log, git diff, less, man.\nMAKE SURE you set uses_pager to true if a command might cause pagination. Otherwise, the command will break. You must use it for VCS commands.\nIf you are unsure, return true."
                    ),
                    "is_risky": boolean_schema(
                        "Whether the shell command could produce dangerous or unwanted side effects. A shell command is risky if it:\n\t- deletes or overwrites user's files or data\n\t- has side effects that are difficult to undo\n\t- handles sensitive information unsafely\n\t- executes unknown and potentially dangerous code\n\t- requires executing as the root user\n\t- causes any other possibly harmful outcome\nIf you are unsure, return true."
                    ),
                    "interact_task": string_schema(
                        "Specify only when mode is \"interact\". The task instructions to be given to the subagent.\n\nIMPORTANT: The subagent receives the task once the command is already running. DO NOT include instructions to run the command.\nIMPORTANT: For persistent interactive sessions (e.g. SSH, tmux, shells/REPLs) that the user wants to remain open,\ndo NOT instruct the subagent to exit or report back once the session is established. The subagent will keep the\nsession alive and wait for further user instructions. Only instruct the subagent to exit if the user's intent\nclearly requires terminating the session."
                    ),
                    "wait_params": json!({
                        "type": "object",
                        "description": "Additional required parameters when running in \"wait\" mode.",
                        "properties": {
                            "reason": string_schema(
                                "Explain WHY you're running the command and WHAT specific information you need from the output.\nUse no more than one sentence. This will be used to TARGET and CONDENSE the raw output so you only see\nthe relevant parts. Leave empty only for commands with small or highly predictable output.\n\n<examples>\n<good_example> \"Checking test failures - need test names, line numbers, and assertion errors\" </good_example>\n<good_example> \"Investigating build errors - need exact error messages and file paths\" </good_example>\n<good_example> \"Reviewing git history - need commit hashes, authors, and changed files\" </good_example>\n\n<bad_example> \"Running tests\" </bad_example>, not specific enough\n<bad_example> \"\" </bad_example>, empty for a command that will produce lots of output\n</examples>"
                            ),
                            "do_not_summarize_output": boolean_schema(
                                "Whether you want to DISABLE condensation of large command output (>4KB).\n\nSet to true ONLY in VERY SPECIAL cases when you MUST see and process ALL of the raw output exactly as produced,\nsuch as when exact byte-for-byte text or full structured data is required for subsequent programmatic parsing.\nExamples: parsing complete JSON/CSV to transform it, verifying exact formatting, cryptographic material, etc.\n\nIf unsure, use false (i.e., allow summarization so only the relevant parts are shown based on `reason`)."
                            ),
                            "expected_duration_seconds": number_schema(
                                "How long you expect the command to take to complete in seconds. Optional but recommended to help the subagent\nunderstand how long it should reasonably wait for the command to complete."
                            )
                        },
                        "required": ["reason", "do_not_summarize_output"],
                        "additionalProperties": false
                    }),
                    "citations": json!({
                        "type": "object",
                        "description": "A list of citations for the command. These MUST be populated if the command command was derived\nfrom external context OR any of the user's rules. These MUST be formatted in JSON.",
                        "properties": {
                            "documents": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "document_type": { "type": "string" },
                                        "document_id": { "type": "string" }
                                    },
                                    "required": ["document_type", "document_id"],
                                    "additionalProperties": false
                                }
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
            "Reads the contents of specified files from the local filesystem. You can access any file directly by using this tool.\n\nUsage:\n- Filepaths must be absolute paths, not relative paths.\n- You can optionally specify line ranges to read (especially efficient for long files), but it's recommended to read the whole file by not providing these parameters. Only use ranges when you have reason to believe the content you're searching for is in those lines.\n- If you are reading multiple, neighbouring chunks of a file , combine them into a single larger chunk. For example, instead of requesting lines 50-55 and 60-65, request lines 50-65.\n- Results are returned with each line prefixed with \"{line_num}|\".\n- Results might be truncated. If the response indicates that the file was truncated, you can make subsequent requests to read the rest of the file using the line range parameters.\n- This tool is able to read all text files on the machine. If the user provides a path to a file, assume that path is valid. It is okay to read a file that does not exist; an error will be returned.\n- This tool can also be used to read images (eg PNG, JPG, etc). When reading an image file the contents are presented visually in a multimodal LLM.\n- This tool can also be used to read PDF files (.pdf). PDFs are processed page by page, extracting both text and visual content for analysis.\n- If the user provides a path to a screenshot, ALWAYS use this tool to view the file at the path. This tool will work with all temporary file paths like /var/folders/123/abc/T/TemporaryItems/NSIRD_screencaptureui_ZfB1tD/Screenshot.png\n- This tool can only read files, not directories. To read a directory, use `ls` command via the `run_shell_command` tool.\n- You have the capability to read multiple files in a single request. It is always better to speculatively read multiple files as a batch that are potentially useful.",
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
                                    "description": "Optional list of specific, non-overlapping line ranges to be retrieved.\nEach range should be formatted as a string \"start-end\".\nOmit to retrieve the entire file.",
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
            "A powerful and fast search tool that operates like grep.\n\nUsage:\n- Use this tool when you know the exact symbol/function name/etc. to search for.\n- Prefer to use this tool rather than invoking `grep` or `rg` via the `run_shell_command` tool.\n- Supports Extended Regular Expressions (EREs), e.g., \"log.*Error\", \"function\\s+\\w+\"\n- IMPORTANT: the following characters are special symbols and MUST be escaped with a backslash in order to be treated as literal characters: ( ) [ ] . * ? + | ^ $\n- <good_example>func foobar\\(</good_example> <bad_example>func foobar(</bad_example>",
            json!({
                "type": "object",
                "properties": {
                    "queries": {
                        "type": "array",
                        "description": "A list of search terms or patterns to look for within files. The search will match any of the provided queries.\nEach query in the list is interpreted as an Extended Regular Expression (ERE).\n\nThis is a JSON array (not to be specified as a JSON string). For example, `[\"foo.txt\"]`, NOT `\"[\\\"foo.txt\\\"]\"`.\nEvery string within the array needs to be escaped properly; for example, to search for '\"\"', the query in\nthe JSON array would be `\"\\\"\\\"\"`.",
                        "items": {
                            "type": "string"
                        }
                    },
                    "path": string_schema(
                        "The absolute path to the directory to search in.\n\nThe path must be a directory, not a file. Directories are searched recursively."
                    )
                },
                "required": ["queries", "path"],
                "additionalProperties": false
            }),
            true,
        )),
        api::ToolType::FileGlob => None,
        api::ToolType::FileGlobV2 => Some(function_schema(
            "file_glob",
            "Usage:\n- Use this tool when you need to find files by name patterns rather than content.\n- Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\".\n- Does not match directories (like `find -type f`).",
            json!({
                "type": "object",
                "properties": {
                    "patterns": {
                        "type": "array",
                        "description": "The regex patterns to match files against. Multiple patterns can be provided to match\ndifferent file types. Only basic *, ?, [ ] patterns are supported.\n\nThis is a JSON array (not to be specified as a JSON string).\n<good_example>[\"*.go\", \"*.ts\"]</good_example>\n<good_example>[\"*.go\"]</good_example>\n<bad_example>\"*.go\"</bad_example>",
                        "items": { "type": "string" }
                    },
                    "search_dir": string_schema("The absolute path to the directory to search in.\n\nThe path must be a directory, not a file. Directories are searched recursively. If not provided, the current working directory will be used."),
                    "max_matches": integer_schema("The maximum number of matches to list. If 0, the default is unlimited."),
                    "max_depth": integer_schema("The maximum depth to search. E.g. a depth of 1 will only match files in the\nsearch_dir directory. If 0, the default is unlimited."),
                    "min_depth": integer_schema("The minimum depth to search. E.g. a depth of 2 will not match files in the\nsearch_dir directory, but will match files in the search_dir directory's children. If\n0, the default is no minimum."),
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
            "Send code review comments to the user so that they can be displayed in the appropriate UI.\n\nUsage:\n- Use this tool to send code review comments retrieved from the version control system or code review platform to the user.\n- reply_metadata and location_metadata are mutually exclusive. A comment cannot have both.\n- Use reply_metadata for comments that are replies to other comments. Reply comments inherit their location from the parent.\n- Use location_metadata for top-level comments attached to specific code (file-level with filepath only, or line-level with full location details).\n- For PR-level comments that apply to the entire PR (not a specific file), omit both location_metadata and reply_metadata.\n- Ensure all timestamps are properly formatted strings.",
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
                                "reply_metadata": {
                                    "type": "object",
                                    "description": "Reply metadata for comments that are replies to other comments.\nMutually exclusive with LocationMetadata.",
                                    "properties": {
                                        "parent_comment_id": string_schema("The ID of the parent comment this is replying to.")
                                    },
                                    "required": ["parent_comment_id"],
                                    "additionalProperties": false
                                },
                                "location_metadata": {
                                    "type": "object",
                                    "description": "Location metadata for comments attached to specific code locations.\nMutually exclusive with ReplyMetadata.",
                                    "properties": {
                                        "filepath": string_schema("The file path associated with this comment."),
                                        "diff_hunk": string_schema("The text content of the diff hunk this comment is attached to, without line or character numbers.\nIf the diff hunk is too long, replace it with a smaller diff hunk that contains only the specific line comment is attached to.\nThis is the ending line number of the comment range.\nThe diff hunk should always be in unified diff format, with a hunk header:\n```\n@@ -old_hunk_start,old_hunk_count +new_hunk_start,new_hunk_count @@ <optional context>\n<hunk content goes here>\n```\nDO NOT remove sections from the middle of a diff hunk; trim unneeded lines from the start and end instead and adjust the header.\nDon't forget to put the shortened hunk's line range in the hunk header."),
                                        "start_line": integer_schema("The starting line number for line-level comments. If present, it should correspond to a line in the diff hunk."),
                                        "end_line": integer_schema("The ending line number for line-level comments. If present, it should correspond to a line in the diff hunk."),
                                        "side": string_schema("The side of the diff for single-line comments (\"LEFT\" for the old file or \"RIGHT\" for the new file).")
                                    },
                                    "required": ["filepath"],
                                    "additionalProperties": false
                                }
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
            "Read a skill by identifier to get its content and instructions.\n\nUse this tool when you need to invoke a skill. The skill content will be returned\nand you should follow its instructions.\n\nYou only need to provide one of bundled_skill_id and skill_path.",
            object_schema(
                vec![
                    (
                        "bundled_skill_id",
                        string_schema("The unique identifier for the bundled skill to read."),
                    ),
                    (
                        "skill_path",
                        string_schema("The path of the skill to read."),
                    ),
                ],
                [],
            ),
            false,
        )),
        api::ToolType::AskUserQuestion => Some(function_schema(
            "ask_user_question",
            "Ask the user one or more clarifying questions.\n\nUsage:\n- Use this tool to ask the user clarifying questions when you need more information to proceed.\n- Only ask when truly necessary — if the task is clear, proceed without asking.\n- If the user skips answering a question, do NOT re-ask it. Proceed with your best judgment.\n- NEVER include more than 4 questions in one ask_user_question call.\n- The user can skip individual questions or skip all remaining questions.",
            json!({
                "type": "object",
                "properties": {
                    "questions": {
                        "type": "array",
                        "description": "A single clarifying question to present to the user.\n\nUsage:\n- Provide options for the user to choose from. Use single_select when exactly one choice applies; use multi_select when the user may pick several.\n- Do not include labels like \"Select One\" or \"Select All that Apply\" in the question text.",
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
                                "type": string_schema(
                                    "Whether the user picks exactly one option (\"single_select\") or may pick several (\"multi_select\")."
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
