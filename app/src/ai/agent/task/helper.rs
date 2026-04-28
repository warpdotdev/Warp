//! This module contains traits and trait implementations for exposing helper methods for accessing
//! proto fields.
use warp_multi_agent_api as api;

pub trait TaskExt {
    fn parent_id(&self) -> Option<&str>;
}

impl TaskExt for api::Task {
    fn parent_id(&self) -> Option<&str> {
        self.dependencies
            .as_ref()
            .map(|deps| deps.parent_task_id.as_str())
            .filter(|id| !id.is_empty())
    }
}

pub trait MessageExt {
    fn todos_op(&self) -> Option<&api::message::update_todos::Operation>;
    fn tool_call(&self) -> Option<&api::message::ToolCall>;
    fn tool_call_mut(&mut self) -> Option<&mut api::message::ToolCall>;
    fn tool_call_result(&self) -> Option<&api::message::ToolCallResult>;
}

pub trait ToolCallExt {
    fn subagent(&self) -> Option<&api::message::tool_call::Subagent>;
    fn subagent_mut(&mut self) -> Option<&mut api::message::tool_call::Subagent>;
}

pub trait ToolExt {
    fn name(&self) -> &'static str;
}

pub trait SubagentExt {
    fn is_cli(&self) -> bool;
    fn is_advice(&self) -> bool;
    fn is_computer_use(&self) -> bool;
    fn is_summarization(&self) -> bool;
    fn is_conversation_search(&self) -> bool;
    fn is_warp_documentation_search(&self) -> bool;
    fn type_name(&self) -> &'static str;
}

impl MessageExt for api::Message {
    fn todos_op(&self) -> Option<&api::message::update_todos::Operation> {
        self.message.as_ref().and_then(|message| {
            if let api::message::Message::UpdateTodos(update) = message {
                update.operation.as_ref()
            } else {
                None
            }
        })
    }

    fn tool_call(&self) -> Option<&api::message::ToolCall> {
        self.message.as_ref().and_then(|message| {
            if let api::message::Message::ToolCall(tool_call) = message {
                Some(tool_call)
            } else {
                None
            }
        })
    }

    fn tool_call_mut(&mut self) -> Option<&mut api::message::ToolCall> {
        self.message.as_mut().and_then(|message| {
            if let api::message::Message::ToolCall(tool_call) = message {
                Some(tool_call)
            } else {
                None
            }
        })
    }

    fn tool_call_result(&self) -> Option<&api::message::ToolCallResult> {
        self.message.as_ref().and_then(|message| {
            if let api::message::Message::ToolCallResult(result) = message {
                Some(result)
            } else {
                None
            }
        })
    }
}

impl ToolCallExt for api::message::ToolCall {
    fn subagent(&self) -> Option<&api::message::tool_call::Subagent> {
        match self.tool.as_ref() {
            Some(api::message::tool_call::Tool::Subagent(subagent)) => Some(subagent),
            _ => None,
        }
    }

    fn subagent_mut(&mut self) -> Option<&mut api::message::tool_call::Subagent> {
        match self.tool.as_mut() {
            Some(api::message::tool_call::Tool::Subagent(subagent)) => Some(subagent),
            _ => None,
        }
    }
}

impl ToolExt for api::message::tool_call::Tool {
    fn name(&self) -> &'static str {
        use api::message::tool_call::Tool;
        match self {
            Self::RunShellCommand(_) => "run_shell_command",
            Self::SearchCodebase(_) => "search_codebase",
            Self::ReadFiles(_) => "read_files",
            Self::UploadFileArtifact(_) => "upload_artifact",
            Self::ApplyFileDiffs(_) => "apply_file_diffs",
            Self::Grep(_) => "grep",
            #[allow(deprecated)]
            Self::FileGlob(_) => "file_glob",
            Self::FileGlobV2(_) => "file_glob_v2",
            Self::ReadMcpResource(_) => "read_mcp_resource",
            Self::CallMcpTool(_) => "call_mcp_tool",
            Self::WriteToLongRunningShellCommand(_) => "write_to_lrc",
            Self::ReadDocuments(_) => "read_documents",
            Self::EditDocuments(_) => "edit_documents",
            Self::CreateDocuments(_) => "create_documents",
            Self::ReadShellCommandOutput(_) => "read_shell_command_output",
            Self::UseComputer(_) => "use_computer",
            Self::RequestComputerUse(_) => "request_computer_use",
            Self::FetchConversation(_) => "fetch_conversation",
            Self::InsertReviewComments(_) => "insert_review_comments",
            Self::ReadSkill(_) => "read_skill",
            Self::SuggestPlan(_) => "suggest_plan",
            Self::SuggestCreatePlan(_) => "suggest_create_plan",
            Self::SuggestNewConversation(_) => "suggest_new_conversation",
            Self::SuggestPrompt(_) => "suggest_prompt",
            Self::OpenCodeReview(_) => "open_code_review",
            Self::InitProject(_) => "init_project",
            Self::StartAgent(_) => "start_agent",
            // Keep the logical tool name stable across the v1/v2 schema split so analytics,
            // history, and UI handling continue to treat both as the same tool.
            Self::StartAgentV2(_) => "start_agent",
            Self::Server(_) => "server",
            Self::Subagent(_) => "subagent",
            Self::AskUserQuestion(_) => "ask_user_question",
            Self::SendMessageToAgent(_) => "send_message_to_agent",
            Self::TransferShellCommandControlToUser(_) => "transfer_shell_command_control",
        }
    }
}

impl SubagentExt for api::message::tool_call::Subagent {
    fn is_cli(&self) -> bool {
        self.metadata.as_ref().is_some_and(|metadata| {
            matches!(
                metadata,
                api::message::tool_call::subagent::Metadata::Cli(_)
            )
        })
    }

    fn is_advice(&self) -> bool {
        self.metadata.as_ref().is_some_and(|metadata| {
            matches!(
                metadata,
                api::message::tool_call::subagent::Metadata::Advice(_)
            )
        })
    }

    fn is_computer_use(&self) -> bool {
        self.metadata.as_ref().is_some_and(|metadata| {
            matches!(
                metadata,
                api::message::tool_call::subagent::Metadata::ComputerUse(_)
            )
        })
    }

    fn is_summarization(&self) -> bool {
        self.metadata.as_ref().is_some_and(|metadata| {
            matches!(
                metadata,
                api::message::tool_call::subagent::Metadata::Summarization(_)
            )
        })
    }

    fn is_conversation_search(&self) -> bool {
        self.metadata.as_ref().is_some_and(|metadata| {
            matches!(
                metadata,
                api::message::tool_call::subagent::Metadata::ConversationSearch(_)
            )
        })
    }

    fn is_warp_documentation_search(&self) -> bool {
        self.metadata.as_ref().is_some_and(|metadata| {
            matches!(
                metadata,
                api::message::tool_call::subagent::Metadata::WarpDocumentationSearch(_)
            )
        })
    }

    fn type_name(&self) -> &'static str {
        use api::message::tool_call::subagent::Metadata;
        match &self.metadata {
            Some(Metadata::Cli(_)) => "cli",
            Some(Metadata::Research(_)) => "research",
            Some(Metadata::Advice(_)) => "advice",
            Some(Metadata::ComputerUse(_)) => "computer_use",
            Some(Metadata::Summarization(_)) => "summarization",
            Some(Metadata::ConversationSearch(_)) => "conversation_search",
            Some(Metadata::WarpDocumentationSearch(_)) => "warp_documentation_search",
            None => "unknown",
        }
    }
}
