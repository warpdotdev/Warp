//! 长运行 shell 命令的交互工具:
//! - `write_to_long_running_shell_command`: 给一个尚在运行的命令写 stdin/PTY
//! - `read_shell_command_output`: 拿一个尚在运行命令的当前输出快照
//!
//! 这两个工具的 `command_id` 来自 `run_shell_command` 的初始 snapshot
//! (`LongRunningShellCommandSnapshot.command_id`)。模型在调用前需要先看到一个
//! 长运行 shell 的 snapshot 拿到 id。

use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use warp_multi_agent_api as api;

use super::OpenAiTool;

// ---------------------------------------------------------------------------
// write_to_long_running_shell_command
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct WriteArgs {
    command_id: String,
    input: String,
    /// "raw" | "line" | "block",默认 "line"
    #[serde(default = "default_mode")]
    mode: String,
}

fn default_mode() -> String {
    "line".to_owned()
}

fn write_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "command_id": {
                "type": "string",
                "description": "之前 run_shell_command 返回的长运行命令 id。"
            },
            "input": {
                "type": "string",
                "description": "要写到 stdin/PTY 的文本。mode=raw 时可使用 <ESC>/<ENTER>/<CTRL-C> 控制键 token。"
            },
            "mode": {
                "type": "string",
                "enum": ["raw", "line", "block"],
                "description": "raw=原始字节;line=作为一行(自动加换行);block=作为多行块。",
                "default": "line"
            }
        },
        "required": ["command_id", "input"],
        "additionalProperties": false
    })
}

fn write_from_args(args: &str) -> Result<api::message::tool_call::Tool> {
    let parsed: WriteArgs = serde_json::from_str(args)?;
    use api::message::tool_call::write_to_long_running_shell_command::mode::Mode as InnerMode;
    use api::message::tool_call::write_to_long_running_shell_command::Mode;
    let is_raw = parsed.mode == "raw";
    let inner = match parsed.mode.as_str() {
        "raw" => InnerMode::Raw(()),
        "block" => InnerMode::Block(()),
        _ => InnerMode::Line(()),
    };
    let input = if is_raw {
        expand_raw_input_tokens(&parsed.input)
    } else {
        parsed.input.into_bytes()
    };
    Ok(
        api::message::tool_call::Tool::WriteToLongRunningShellCommand(
            api::message::tool_call::WriteToLongRunningShellCommand {
                command_id: parsed.command_id,
                input,
                mode: Some(Mode { mode: Some(inner) }),
            },
        ),
    )
}

fn expand_raw_input_tokens(input: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('<') {
        out.extend_from_slice(&rest.as_bytes()[..start]);
        let token_candidate = &rest[start..];
        if let Some(end) = token_candidate.find('>') {
            let token = &token_candidate[..=end];
            if let Some(byte) = control_token_byte(token) {
                out.push(byte);
                rest = &token_candidate[end + 1..];
                continue;
            }
        }
        out.push(b'<');
        rest = &rest[start + 1..];
    }
    out.extend_from_slice(rest.as_bytes());
    out
}

fn control_token_byte(token: &str) -> Option<u8> {
    match token {
        "<ESC>" | "<Esc>" | "<escape>" | "<Escape>" => Some(0x1b),
        "<ENTER>" | "<Enter>" | "<CR>" | "<LF>" => Some(b'\n'),
        "<TAB>" | "<Tab>" => Some(b'\t'),
        "<BACKSPACE>" | "<Backspace>" => Some(0x7f),
        "<CTRL-C>" | "<Ctrl-C>" | "<C-c>" => Some(0x03),
        "<CTRL-D>" | "<Ctrl-D>" | "<C-d>" => Some(0x04),
        _ => None,
    }
}

fn write_result_to_json(result: &api::message::tool_call_result::Result) -> Option<Value> {
    use api::message::tool_call_result::Result as R;
    use api::write_to_long_running_shell_command_result::Result as WR;
    let r = match result {
        R::WriteToLongRunningShellCommand(r) => r,
        _ => return None,
    };
    let value = match &r.result {
        Some(WR::LongRunningCommandSnapshot(s)) => json!({
            "status": "running",
            "command_id": s.command_id,
            "output": s.output,
            "is_alt_screen_active": s.is_alt_screen_active,
        }),
        Some(WR::CommandFinished(f)) => json!({
            "status": "completed",
            "command_id": f.command_id,
            "exit_code": f.exit_code,
            "output": f.output,
        }),
        // ShellCommandError 现仅有 BlockNotFound 一个 variant
        Some(WR::Error(_)) => json!({
            "status": "error",
            "message": "block_not_found_or_command_id_invalid",
        }),
        None => json!({ "status": "cancelled" }),
    };
    Some(value)
}

pub static WRITE_TO_LONG_RUNNING_SHELL_COMMAND: OpenAiTool = OpenAiTool {
    name: "write_to_long_running_shell_command",
    description: include_str!(
        "../prompts/tool_descriptions/write_to_long_running_shell_command.md"
    ),
    parameters: write_parameters,
    from_args: write_from_args,
    result_to_json: write_result_to_json,
};

// ---------------------------------------------------------------------------
// read_shell_command_output
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ReadArgs {
    command_id: String,
    /// "on_completion"(默认)或 number(秒数 → Duration)
    #[serde(default)]
    delay_seconds: Option<u64>,
}

fn read_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "command_id": {
                "type": "string",
                "description": "运行中命令的 id。"
            },
            "delay_seconds": {
                "type": "integer",
                "description": "可选: 在指定秒数后返回当前 snapshot;不填则等到命令完成才返回。",
                "minimum": 0
            }
        },
        "required": ["command_id"],
        "additionalProperties": false
    })
}

fn read_from_args(args: &str) -> Result<api::message::tool_call::Tool> {
    let parsed: ReadArgs = serde_json::from_str(args)?;
    use api::message::tool_call::read_shell_command_output::Delay;
    let delay = match parsed.delay_seconds {
        Some(secs) => Delay::Duration(prost_types::Duration {
            seconds: secs as i64,
            nanos: 0,
        }),
        None => Delay::OnCompletion(()),
    };
    Ok(api::message::tool_call::Tool::ReadShellCommandOutput(
        api::message::tool_call::ReadShellCommandOutput {
            command_id: parsed.command_id,
            delay: Some(delay),
        },
    ))
}

fn read_result_to_json(result: &api::message::tool_call_result::Result) -> Option<Value> {
    use api::message::tool_call_result::Result as R;
    use api::read_shell_command_output_result::Result as ReadR;
    let r = match result {
        R::ReadShellCommandOutput(r) => r,
        _ => return None,
    };
    let value = match &r.result {
        Some(ReadR::LongRunningCommandSnapshot(s)) => json!({
            "status": "running",
            "command": r.command,
            "command_id": s.command_id,
            "output": s.output,
            "is_alt_screen_active": s.is_alt_screen_active,
        }),
        Some(ReadR::CommandFinished(f)) => json!({
            "status": "completed",
            "command": r.command,
            "command_id": f.command_id,
            "exit_code": f.exit_code,
            "output": f.output,
        }),
        Some(ReadR::Error(_)) => json!({ "status": "error", "command": r.command }),
        None => json!({ "status": "cancelled", "command": r.command }),
    };
    Some(value)
}

pub static READ_SHELL_COMMAND_OUTPUT: OpenAiTool = OpenAiTool {
    name: "read_shell_command_output",
    description: include_str!("../prompts/tool_descriptions/read_shell_command_output.md"),
    parameters: read_parameters,
    from_args: read_from_args,
    result_to_json: read_result_to_json,
};
