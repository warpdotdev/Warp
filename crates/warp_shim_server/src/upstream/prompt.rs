use warp_multi_agent_api as api;

use crate::conversation::transcript::TranscriptMessage;

pub(crate) fn system_message() -> String {
    [
        "You are a local Warp Agent Mode shim assistant.",
        "You answer through a local OpenAI-compatible model endpoint configured by the user.",
        "When tools are declared, use them for file reads, shell commands, code search, file edits, glob/grep, and MCP operations instead of pretending you already performed those actions.",
        "After a tool result is returned, continue from that result and either call another declared tool or provide the final answer.",
        "Do not claim Warp cloud capabilities; this shim only routes local Agent Mode requests and client-executed tools.",
    ]
    .join("\n")
}

#[allow(deprecated)]
pub(crate) fn input_messages(request: &api::Request) -> Vec<TranscriptMessage> {
    let Some(input) = request.input.as_ref() else {
        return vec![TranscriptMessage::user("Continue the conversation.")];
    };

    let mut messages = match input.r#type.as_ref() {
        Some(api::request::input::Type::UserInputs(user_inputs)) => user_inputs
            .inputs
            .iter()
            .filter_map(|input| user_input_to_message(input, input_context(input, request)))
            .collect(),
        Some(api::request::input::Type::UserQuery(query)) => {
            vec![user_query_to_message(query, input.context.as_ref(), None)]
        }
        Some(api::request::input::Type::QueryWithCannedResponse(query)) => vec![text_to_message(
            &query.query,
            input.context.as_ref(),
            Some("Warp canned-response query"),
        )],
        Some(api::request::input::Type::AutoCodeDiffQuery(query)) => vec![text_to_message(
            &query.query,
            input.context.as_ref(),
            Some("Automatic code-diff query"),
        )],
        Some(api::request::input::Type::ResumeConversation(_)) => {
            vec![text_to_message(
                "Continue the conversation.",
                input.context.as_ref(),
                None,
            )]
        }
        Some(api::request::input::Type::InitProjectRules(_)) => vec![text_to_message(
            "Create or update project rules for this workspace.",
            input.context.as_ref(),
            None,
        )],
        Some(api::request::input::Type::CreateNewProject(query)) => vec![text_to_message(
            &format!("Create a new project from this request: {}", query.query),
            input.context.as_ref(),
            None,
        )],
        Some(api::request::input::Type::CloneRepository(query)) => vec![text_to_message(
            &format!("Help clone and set up this repository: {}", query.url),
            input.context.as_ref(),
            None,
        )],
        Some(api::request::input::Type::CodeReview(_)) => vec![text_to_message(
            "Review the provided code changes.",
            input.context.as_ref(),
            None,
        )],
        Some(api::request::input::Type::SummarizeConversation(query)) => vec![text_to_message(
            &format!("Summarize the active conversation. {}", query.prompt),
            input.context.as_ref(),
            None,
        )],
        Some(api::request::input::Type::CreateEnvironment(query)) => vec![text_to_message(
            &format!(
                "Create a development environment for repositories: {}",
                query.repo_paths.join(", ")
            ),
            input.context.as_ref(),
            None,
        )],
        Some(api::request::input::Type::FetchReviewComments(query)) => vec![text_to_message(
            &format!(
                "Fetch review comments for repository path: {}",
                query.repo_path
            ),
            input.context.as_ref(),
            None,
        )],
        Some(api::request::input::Type::StartFromAmbientRunPrompt(query)) => vec![text_to_message(
            non_empty(&query.runtime_base_prompt)
                .unwrap_or("Start from the latest ambient run prompt."),
            input.context.as_ref(),
            None,
        )],
        Some(api::request::input::Type::InvokeSkill(query)) => {
            let text = query
                .user_query
                .as_ref()
                .and_then(|query| non_empty(&query.query))
                .unwrap_or("Invoke the requested Warp skill.");
            vec![text_to_message(
                text,
                input.context.as_ref(),
                Some("Skill invocation"),
            )]
        }
        Some(api::request::input::Type::GeneratePassiveSuggestions(_))
        | Some(api::request::input::Type::ToolCallResult(_))
        | None => Vec::new(),
    };

    if messages.is_empty() && should_default_to_continue(input) {
        messages.push(TranscriptMessage::user("Continue the conversation."));
    }
    messages
}

#[allow(deprecated)]
fn should_default_to_continue(input: &api::request::Input) -> bool {
    match input.r#type.as_ref() {
        Some(api::request::input::Type::ToolCallResult(_))
        | Some(api::request::input::Type::GeneratePassiveSuggestions(_)) => false,
        Some(api::request::input::Type::UserInputs(user_inputs)) => !user_inputs.inputs.iter().any(
            |input| matches!(
                input.input.as_ref(),
                Some(api::request::input::user_inputs::user_input::Input::ToolCallResult(_))
                    | Some(
                        api::request::input::user_inputs::user_input::Input::PassiveSuggestionResult(_),
                    )
            ),
        ),
        _ => true,
    }
}

#[allow(deprecated)]
pub(crate) fn context_prefix(context: &api::InputContext) -> String {
    let mut lines = Vec::new();

    if let Some(directory) = context.directory.as_ref()
        && let Some(pwd) = non_empty(&directory.pwd)
    {
        lines.push(format!("- Current directory: {pwd}"));
    }
    if let Some(os) = context.operating_system.as_ref() {
        let platform = non_empty(&os.platform).unwrap_or("unknown");
        if let Some(distribution) = non_empty(&os.distribution) {
            lines.push(format!("- OS: {platform} ({distribution})"));
        } else if platform != "unknown" {
            lines.push(format!("- OS: {platform}"));
        }
    }
    if let Some(shell) = context.shell.as_ref() {
        match (non_empty(&shell.name), non_empty(&shell.version)) {
            (Some(name), Some(version)) => lines.push(format!("- Shell: {name} {version}")),
            (Some(name), None) => lines.push(format!("- Shell: {name}")),
            _ => {}
        }
    }
    if let Some(git) = context.git.as_ref() {
        if let Some(branch) = non_empty(&git.branch) {
            lines.push(format!("- Git branch: {branch}"));
        } else if let Some(head) = non_empty(&git.head) {
            lines.push(format!("- Git head: {head}"));
        }
    }

    for selected in &context.selected_text {
        if let Some(text) = non_empty(&selected.text) {
            lines.push(format!("- Selected text:\n{text}"));
        }
    }
    for command in &context.executed_shell_commands {
        let command_text = non_empty(&command.command).unwrap_or("<unknown command>");
        let output = non_empty(&command.output).unwrap_or("<no output>");
        lines.push(format!(
            "- Executed command (exit {}): {command_text}\n{output}",
            command.exit_code
        ));
    }
    for image in &context.images {
        lines.push(format!(
            "- [image omitted: mime_type={}, bytes={}]",
            empty_as_unknown(&image.mime_type),
            image.data.len()
        ));
    }
    for file in &context.files {
        if let Some(content) = file.content.as_ref()
            && let Some(text) = non_empty(&content.content)
        {
            lines.push(format!("- File {}:\n{text}", content.file_path));
        }
    }
    for codebase in &context.codebases {
        lines.push(format!(
            "- Codebase: {} at {}",
            empty_as_unknown(&codebase.name),
            empty_as_unknown(&codebase.path)
        ));
    }
    for rules in &context.project_rules {
        if let Some(root) = non_empty(&rules.root_path) {
            lines.push(format!("- Project rules root: {root}"));
        }
        for rule_file in &rules.active_rule_files {
            if let Some(content) = non_empty(&rule_file.content) {
                lines.push(format!("- Active rule {}:\n{content}", rule_file.file_path));
            }
        }
        if !rules.additional_rule_file_paths.is_empty() {
            lines.push(format!(
                "- Additional rule files: {}",
                rules.additional_rule_file_paths.join(", ")
            ));
        }
    }

    if lines.is_empty() {
        String::new()
    } else {
        format!("Context from Warp:\n{}", lines.join("\n"))
    }
}

fn user_input_to_message(
    input: &api::request::input::user_inputs::UserInput,
    context: Option<&api::InputContext>,
) -> Option<TranscriptMessage> {
    match input.input.as_ref()? {
        api::request::input::user_inputs::user_input::Input::UserQuery(query) => {
            Some(user_query_to_message(query, context, None))
        }
        api::request::input::user_inputs::user_input::Input::CliAgentUserQuery(query) => {
            let user_query = query.user_query.as_ref()?;
            let mut message = user_query_to_message(user_query, context, Some("CLI agent query"));
            if let Some(running_command) = query.running_command.as_ref() {
                let mut extra = format!("\n\nRunning command: {}", running_command.command);
                if let Some(snapshot) = running_command.snapshot.as_ref()
                    && let Some(output) = non_empty(&snapshot.output)
                {
                    extra.push_str("\nCurrent command output:\n");
                    extra.push_str(output);
                }
                message.content.push_str(&extra);
            }
            Some(message)
        }
        api::request::input::user_inputs::user_input::Input::MessagesReceivedFromAgents(messages) => {
            let content = messages
                .messages
                .iter()
                .map(|message| {
                    format!(
                        "Message from {} about {}:\n{}",
                        empty_as_unknown(&message.sender_agent_id),
                        empty_as_unknown(&message.subject),
                        message.message_body
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            non_empty(&content).map(TranscriptMessage::user)
        }
        api::request::input::user_inputs::user_input::Input::EventsFromAgents(events) => {
            (!events.agent_events.is_empty()).then(|| {
                TranscriptMessage::user(format!(
                    "Received {} agent event(s). Tool/event handling is not implemented in this shim item.",
                    events.agent_events.len()
                ))
            })
        }
        api::request::input::user_inputs::user_input::Input::PassiveSuggestionResult(_)
        | api::request::input::user_inputs::user_input::Input::ToolCallResult(_) => None,
    }
}

fn input_context<'a>(
    input: &'a api::request::input::user_inputs::UserInput,
    request: &'a api::Request,
) -> Option<&'a api::InputContext> {
    match input.input.as_ref()? {
        api::request::input::user_inputs::user_input::Input::UserQuery(_)
        | api::request::input::user_inputs::user_input::Input::CliAgentUserQuery(_) => {
            request.input.as_ref()?.context.as_ref()
        }
        _ => None,
    }
}

fn user_query_to_message(
    query: &api::request::input::UserQuery,
    context: Option<&api::InputContext>,
    label: Option<&str>,
) -> TranscriptMessage {
    let mut content = format_user_content(&query.query, context, label);
    if !query.referenced_attachments.is_empty() {
        let attachments = query
            .referenced_attachments
            .keys()
            .map(|key| format!("[attachment omitted: {key}]"))
            .collect::<Vec<_>>()
            .join("\n");
        content.push_str("\n\nReferenced attachments:\n");
        content.push_str(&attachments);
    }
    TranscriptMessage::user(content)
}

fn text_to_message(
    text: &str,
    context: Option<&api::InputContext>,
    label: Option<&str>,
) -> TranscriptMessage {
    TranscriptMessage::user(format_user_content(text, context, label))
}

fn format_user_content(
    text: &str,
    context: Option<&api::InputContext>,
    label: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    if let Some(context) = context
        .map(context_prefix)
        .filter(|context| !context.is_empty())
    {
        parts.push(context);
    }
    if let Some(label) = label {
        parts.push(format!("Request kind: {label}"));
    }
    parts.push(format!("User request:\n{}", text.trim()));
    parts.join("\n\n")
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn empty_as_unknown(value: &str) -> &str {
    non_empty(value).unwrap_or("unknown")
}
