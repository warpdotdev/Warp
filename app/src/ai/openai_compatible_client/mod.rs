mod convert;

use std::sync::Arc;

use anyhow::anyhow;
use futures::channel::oneshot;
use futures::StreamExt;
use http_client::Client;
use std::sync::Mutex;

use ai::openai_compatible::OpenAiCompatibleEndpoint;

use crate::ai::agent::api::{Event, ResponseStream};
use crate::server::server_api::AIApiError;

use convert::{OpenAiChatRequest, OpenAiChatStreamDelta, StreamingState};

pub use convert::{OpenAiCompatibleRequest, from_request_params};

#[derive(Debug, thiserror::Error)]
pub enum OpenAiCompatibleError {
    #[error("Failed to parse response from endpoint: {0}")]
    ParseError(String),
}

impl From<OpenAiCompatibleError> for crate::ai::agent::api::ConvertToAPITypeError {
    fn from(e: OpenAiCompatibleError) -> Self {
        crate::ai::agent::api::ConvertToAPITypeError::Other(anyhow!("{}", e))
    }
}

pub async fn generate_openai_compatible_output(
    client: &Client,
    endpoint: &OpenAiCompatibleEndpoint,
    request: OpenAiCompatibleRequest,
    cancellation_rx: oneshot::Receiver<()>,
) -> Result<ResponseStream, OpenAiCompatibleError> {
    let url = endpoint.chat_completions_url();
    let request_conversation_id = request.conversation_id.clone();
    let chat_request = OpenAiChatRequest::from_request(request.clone(), &endpoint.models[0].model_id);

    log::info!(
        "Custom endpoint request: url={}, model={}, messages={}, stream={}, tools={}",
        url,
        chat_request.model,
        chat_request.messages.len(),
        chat_request.stream,
        chat_request.tools.len(),
    );

    let mut request_builder = client
        .post(&url)
        .json(&chat_request)
        .prevent_sleep("OpenAI-compatible request in-progress");

    if let Some(ref api_key) = endpoint.api_key {
        if !api_key.is_empty() {
            request_builder = request_builder.bearer_auth(api_key);
        }
    }

    let task_id = request.task_id.clone();
    let conversation_id = request_conversation_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let request_id = uuid::Uuid::new_v4().to_string();
    let run_id = String::new();

    let init_event: Event = Ok(warp_multi_agent_api::ResponseEvent {
        r#type: Some(warp_multi_agent_api::response_event::Type::Init(
            warp_multi_agent_api::response_event::StreamInit {
                conversation_id: conversation_id.clone(),
                request_id: request_id.clone(),
                run_id,
            },
        )),
    });

    let create_task_event: Event = Ok(convert::make_create_task_event(&task_id));
    let user_query_event: Option<Event> = request.user_query.as_ref().map(|query| {
        Ok(warp_multi_agent_api::ResponseEvent {
            r#type: Some(warp_multi_agent_api::response_event::Type::ClientActions(
                convert::make_user_query_client_action(query.clone(), &task_id),
            )),
        })
    });
    let streaming_state = Arc::new(Mutex::new(StreamingState::new()));

    let event_source = request_builder
        .eventsource();

    let cid_for_log = conversation_id.clone();
    let output_stream = event_source
        .take_until(cancellation_rx)
        .flat_map(move |event| {
            let task_id = task_id.clone();
            let state = streaming_state.clone();
            let events: Vec<Event> = match event {
                Ok(reqwest_eventsource::Event::Message(message_event)) => {
                    let data = message_event.data;
                    log::debug!("Custom endpoint SSE chunk: {} bytes", data.len());

                    if data.trim() == "[DONE]" {
                        log::debug!("Custom endpoint: received [DONE] (conversation={})", cid_for_log);

                        let mut tool_call_events = Vec::new();
                        {
                            let mut s = state.lock().unwrap();
                            let tool_calls = s.take_accumulated_tool_calls();

                            if !tool_calls.is_empty() {
                                log::info!("Custom endpoint: finalizing {} tool calls", tool_calls.len());
                                tool_call_events = convert::finalize_tool_call_events(tool_calls, &task_id);
                            }
                        }

                        let mut finish_events = tool_call_events;
                        finish_events.push(Ok(warp_multi_agent_api::ResponseEvent {
                            r#type: Some(
                                warp_multi_agent_api::response_event::Type::Finished(
                                    warp_multi_agent_api::response_event::StreamFinished {
                                        token_usage: vec![],
                                        should_refresh_model_config: false,
                                        request_cost: None,
                                        conversation_usage_metadata: None,
                                        reason: Some(
                                            warp_multi_agent_api::response_event::stream_finished::Reason::Done(
                                                warp_multi_agent_api::response_event::stream_finished::Done {},
                                            ),
                                        ),
                                    },
                                ),
                            ),
                        }));
                        finish_events
                    } else {
                        match serde_json::from_str::<OpenAiChatStreamDelta>(&data) {
                            Ok(delta) => {
                                let mut s = state.lock().unwrap();
                                convert::delta_to_response_events(delta, &task_id, &mut s)
                            }
                            Err(e) => {
                                log::warn!(
                                    "Failed to parse SSE chunk from OpenAI-compatible endpoint: {e}. Raw data: {}",
                                    if data.len() > 300 { &data[..300] } else { &data }
                                );
                                vec![Err(Arc::new(AIApiError::Other(anyhow!(
                                    "Failed to parse response from endpoint: {e}"
                                ))))]
                            }
                        }
                    }
                }
                Ok(reqwest_eventsource::Event::Open) => {
                    log::debug!("Custom endpoint: SSE connection opened");
                    vec![]
                }
                Err(err) => {
                    let ai_error = match &err {
                        reqwest_eventsource::Error::InvalidStatusCode(status, _response) => {
                            let msg = format!(
                                "Endpoint returned HTTP {} {}. Check your endpoint configuration and API key.",
                                status.as_u16(),
                                status.canonical_reason().unwrap_or("")
                            );
                            log::warn!("{}", msg);
                            AIApiError::ErrorStatus(*status, msg)
                        }
                        reqwest_eventsource::Error::Transport(ref e) => {
                            if let Some(status) = e.status() {
                                AIApiError::ErrorStatus(status, format!("Connection error: {e}"))
                            } else {
                                AIApiError::Other(anyhow!("Transport error: {e}"))
                            }
                        }
                        _ => AIApiError::Other(anyhow!(
                            "OpenAI-compatible endpoint stream error: {err}"
                        )),
                    };
                    vec![Err(Arc::new(ai_error))]
                }
            };
            futures::stream::iter(events)
        });

    let mut init_events = vec![init_event, create_task_event];
    if let Some(uq_event) = user_query_event {
        init_events.push(uq_event);
    }
    let init_stream = futures::stream::iter(init_events);
    Ok(Box::pin(init_stream.chain(output_stream)))
}
