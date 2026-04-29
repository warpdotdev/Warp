use warp_multi_agent_api as api;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct UsageTotals {
    pub(crate) total_input: u32,
    pub(crate) output: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct ResponseBuilder {
    conversation_id: String,
    request_id: String,
    run_id: String,
}

impl ResponseBuilder {
    pub(crate) fn new(conversation_id: String, request_id: String, run_id: String) -> Self {
        Self {
            conversation_id,
            request_id,
            run_id,
        }
    }

    pub(crate) fn init(&self) -> api::ResponseEvent {
        api::ResponseEvent {
            r#type: Some(api::response_event::Type::Init(
                api::response_event::StreamInit {
                    conversation_id: self.conversation_id.clone(),
                    request_id: self.request_id.clone(),
                    run_id: self.run_id.clone(),
                },
            )),
        }
    }

    pub(crate) fn create_task(&self, task_id: &str) -> api::ResponseEvent {
        self.client_actions(vec![api::ClientAction {
            action: Some(api::client_action::Action::CreateTask(
                api::client_action::CreateTask {
                    task: Some(api::Task {
                        id: task_id.to_string(),
                        description: "Local Warp Agent Mode".to_string(),
                        ..Default::default()
                    }),
                },
            )),
        }])
    }

    pub(crate) fn add_agent_output(
        &self,
        task_id: &str,
        message_id: &str,
        text: impl Into<String>,
    ) -> api::ResponseEvent {
        self.client_actions(vec![api::ClientAction {
            action: Some(api::client_action::Action::AddMessagesToTask(
                api::client_action::AddMessagesToTask {
                    task_id: task_id.to_string(),
                    messages: vec![self.agent_output_message(task_id, message_id, text)],
                },
            )),
        }])
    }

    pub(crate) fn append_agent_output(
        &self,
        task_id: &str,
        message_id: &str,
        text_delta: impl Into<String>,
    ) -> api::ResponseEvent {
        self.client_actions(vec![api::ClientAction {
            action: Some(api::client_action::Action::AppendToMessageContent(
                api::client_action::AppendToMessageContent {
                    task_id: task_id.to_string(),
                    message: Some(self.agent_output_message(task_id, message_id, text_delta)),
                    mask: Some(prost_types::FieldMask {
                        paths: vec!["agent_output.text".to_string()],
                    }),
                },
            )),
        }])
    }

    pub(crate) fn add_tool_call(
        &self,
        task_id: &str,
        message_id: &str,
        tool_call: api::message::ToolCall,
    ) -> api::ResponseEvent {
        self.client_actions(vec![api::ClientAction {
            action: Some(api::client_action::Action::AddMessagesToTask(
                api::client_action::AddMessagesToTask {
                    task_id: task_id.to_string(),
                    messages: vec![api::Message {
                        id: message_id.to_string(),
                        task_id: task_id.to_string(),
                        request_id: self.request_id.clone(),
                        message: Some(api::message::Message::ToolCall(tool_call)),
                        ..Default::default()
                    }],
                },
            )),
        }])
    }

    pub(crate) fn model_used(
        &self,
        task_id: &str,
        message_id: &str,
        model_id: &str,
        display_name: &str,
    ) -> api::ResponseEvent {
        self.client_actions(vec![api::ClientAction {
            action: Some(api::client_action::Action::AddMessagesToTask(
                api::client_action::AddMessagesToTask {
                    task_id: task_id.to_string(),
                    messages: vec![api::Message {
                        id: message_id.to_string(),
                        task_id: task_id.to_string(),
                        request_id: self.request_id.clone(),
                        message: Some(api::message::Message::ModelUsed(api::message::ModelUsed {
                            model_id: model_id.to_string(),
                            model_display_name: display_name.to_string(),
                            is_fallback: false,
                        })),
                        ..Default::default()
                    }],
                },
            )),
        }])
    }

    pub(crate) fn finished_success(
        &self,
        usage: Option<(String, UsageTotals)>,
    ) -> api::ResponseEvent {
        api::ResponseEvent {
            r#type: Some(api::response_event::Type::Finished(
                api::response_event::StreamFinished {
                    reason: Some(api::response_event::stream_finished::Reason::Done(
                        api::response_event::stream_finished::Done {},
                    )),
                    token_usage: usage
                        .map(|(model_id, usage)| {
                            vec![api::response_event::stream_finished::TokenUsage {
                                model_id,
                                total_input: usage.total_input,
                                output: usage.output,
                                ..Default::default()
                            }]
                        })
                        .unwrap_or_default(),
                    should_refresh_model_config: false,
                    request_cost: None,
                    conversation_usage_metadata: None,
                },
            )),
        }
    }

    pub(crate) fn finished_internal_error(&self, message: impl Into<String>) -> api::ResponseEvent {
        api::ResponseEvent {
            r#type: Some(api::response_event::Type::Finished(
                api::response_event::StreamFinished {
                    reason: Some(api::response_event::stream_finished::Reason::InternalError(
                        api::response_event::stream_finished::InternalError {
                            message: message.into(),
                        },
                    )),
                    token_usage: vec![],
                    should_refresh_model_config: false,
                    request_cost: None,
                    conversation_usage_metadata: None,
                },
            )),
        }
    }

    fn agent_output_message(
        &self,
        task_id: &str,
        message_id: &str,
        text: impl Into<String>,
    ) -> api::Message {
        api::Message {
            id: message_id.to_string(),
            task_id: task_id.to_string(),
            request_id: self.request_id.clone(),
            message: Some(api::message::Message::AgentOutput(
                api::message::AgentOutput { text: text.into() },
            )),
            ..Default::default()
        }
    }

    fn client_actions(&self, actions: Vec<api::ClientAction>) -> api::ResponseEvent {
        api::ResponseEvent {
            r#type: Some(api::response_event::Type::ClientActions(
                api::response_event::ClientActions { actions },
            )),
        }
    }
}
