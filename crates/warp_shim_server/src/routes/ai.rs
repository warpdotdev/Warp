use std::convert::Infallible;

use anyhow::{Context, Result, anyhow};
use async_stream::stream;
use axum::{
    Json, Router,
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response, sse::Sse},
    routing::post,
};
use futures_util::{Stream, StreamExt};
use serde_json::json;
use uuid::Uuid;
use warp_multi_agent_api as api;

use crate::{
    config::{ShimConfig, UpstreamConfig},
    conversation::transcript::{self, TranscriptMessage, WarpToolKind},
    protocol::{protobuf, response_builder::ResponseBuilder, sse},
    server::ShimState,
    upstream::{openai, prompt, tool_registry::ToolRegistry},
};

pub(crate) fn router() -> Router<ShimState> {
    Router::new()
        .route("/ai/multi-agent", post(post_multi_agent))
        .route("/ai/passive-suggestions", post(post_passive_suggestions))
}

async fn post_multi_agent(State(state): State<ShimState>, body: Bytes) -> Response {
    let request = match protobuf::decode_request(&body) {
        Ok(request) => request,
        Err(error) => {
            tracing::warn!(%error, "failed to decode multi-agent protobuf request");
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("invalid warp_multi_agent_api::Request protobuf: {error}")
                })),
            )
                .into_response();
        }
    };

    Sse::new(multi_agent_stream(state, request)).into_response()
}

async fn post_passive_suggestions(State(state): State<ShimState>, body: Bytes) -> Response {
    let request = match protobuf::decode_request(&body) {
        Ok(request) => request,
        Err(error) => {
            tracing::warn!(%error, "failed to decode passive-suggestions protobuf request");
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("invalid warp_multi_agent_api::Request protobuf: {error}")
                })),
            )
                .into_response();
        }
    };

    Sse::new(passive_suggestions_stream(state, request)).into_response()
}

fn passive_suggestions_stream(
    state: ShimState,
    request: api::Request,
) -> impl Stream<Item = Result<axum::response::sse::Event, Infallible>> {
    stream! {
        let response_context = ResponseContext::from_request(&request);
        let builder = ResponseBuilder::new(
            response_context.conversation_id,
            response_context.request_id,
            response_context.run_id,
        );
        yield Ok(sse::encode_response_event_for_sse(&builder.init()));
        tracing::debug!(
            passive_suggestions_enabled = state.config.features.passive_suggestions_enabled,
            "returning no-op successful passive suggestions stream"
        );
        yield Ok(sse::encode_response_event_for_sse(&builder.finished_success(None)));
    }
}

fn multi_agent_stream(
    state: ShimState,
    request: api::Request,
) -> impl Stream<Item = Result<axum::response::sse::Event, Infallible>> {
    stream! {
        let response_context = ResponseContext::from_request(&request);
        let builder = ResponseBuilder::new(
            response_context.conversation_id.clone(),
            response_context.request_id.clone(),
            response_context.run_id.clone(),
        );
        yield Ok(sse::encode_response_event_for_sse(&builder.init()));

        let _turn_guard = state
            .conversations
            .lock_turn(&response_context.conversation_id)
            .await;

        let seed_messages = transcript::history_from_request(&request);
        let (mut conversation, is_new_conversation) = state
            .conversations
            .load_or_create(
                response_context.conversation_id.clone(),
                response_context.task_id.clone(),
                seed_messages,
            )
            .await;

        if should_create_task(&response_context, is_new_conversation) {
            yield Ok(sse::encode_response_event_for_sse(
                &builder.create_task(&conversation.task_id),
            ));
        }

        let tool_registry = ToolRegistry::for_request(&request, &state.config.features);
        append_tool_call_results(&mut conversation, &request, &tool_registry);

        let input_messages = prompt::input_messages(&request);
        conversation.messages.extend(input_messages);

        let resolved_model = match resolve_model(&state.config, &request) {
            Ok(model) => model,
            Err(error) => {
                tracing::warn!(%error, "failed to resolve upstream model for multi-agent request");
                state.conversations.save(conversation).await;
                yield Ok(sse::encode_response_event_for_sse(
                    &builder.finished_internal_error(error.to_string()),
                ));
                return;
            }
        };

        let chat_request = openai::OpenAiChatRequest::new(
            resolved_model.upstream_model.clone(),
            &conversation.messages,
            prompt::system_message(),
            resolved_model.upstream.streaming,
            tool_registry.openai_tools(),
        );

        let message_id = format!("local-message-{}", Uuid::new_v4());
        let model_message_id = format!("local-model-{}", Uuid::new_v4());

        if resolved_model.upstream.streaming {
            let result = stream_upstream_response(
                state.http_client.clone(),
                builder.clone(),
                conversation.task_id.clone(),
                message_id.clone(),
                resolved_model.upstream.clone(),
                chat_request,
            );
            futures_util::pin_mut!(result);

            let mut final_text = String::new();
            let mut usage = None;
            let mut tool_call_accumulator = openai::OpenAiToolCallAccumulator::default();
            while let Some(item) = result.next().await {
                match item {
                    StreamRouteItem::Event(event) => yield Ok(event),
                    StreamRouteItem::TextDelta(delta) => final_text.push_str(&delta),
                    StreamRouteItem::ToolCallDeltas(deltas) => {
                        tool_call_accumulator.push_deltas(deltas);
                    }
                    StreamRouteItem::Usage(new_usage) => usage = Some(new_usage),
                    StreamRouteItem::Error(error) => {
                        tracing::warn!(%error, "upstream streaming failed");
                        state.conversations.save(conversation).await;
                        yield Ok(sse::encode_response_event_for_sse(
                            &builder.finished_internal_error(error),
                        ));
                        return;
                    }
                }
            }

            let tool_calls = match tool_call_accumulator.finish() {
                Ok(tool_calls) => tool_calls,
                Err(error) => {
                    tracing::warn!(%error, "failed to assemble upstream tool calls");
                    state.conversations.save(conversation).await;
                    yield Ok(sse::encode_response_event_for_sse(
                        &builder.finished_internal_error(error.to_string()),
                    ));
                    return;
                }
            };

            let tool_call_events = match append_upstream_response(
                &mut conversation,
                &tool_registry,
                &builder,
                final_text,
                tool_calls,
            ) {
                Ok(events) => events,
                Err(error) => {
                    tracing::warn!(%error, "failed to convert upstream tool call");
                    state.conversations.save(conversation).await;
                    yield Ok(sse::encode_response_event_for_sse(
                        &builder.finished_internal_error(error.to_string()),
                    ));
                    return;
                }
            };
            for event in tool_call_events {
                yield Ok(sse::encode_response_event_for_sse(&event));
            }
            yield Ok(sse::encode_response_event_for_sse(&builder.model_used(
                &conversation.task_id,
                &model_message_id,
                &resolved_model.warp_model_id,
                &resolved_model.upstream_model,
            )));
            yield Ok(sse::encode_response_event_for_sse(&builder.finished_success(
                usage.map(|usage| (resolved_model.upstream_model.clone(), usage)),
            )));
            state.conversations.save(conversation).await;
        } else {
            match openai::complete_chat_completion(
                &state.http_client,
                &resolved_model.upstream,
                chat_request,
            ).await {
                Ok(output) => {
                    if !output.content.is_empty() {
                        yield Ok(sse::encode_response_event_for_sse(&builder.add_agent_output(
                            &conversation.task_id,
                            &message_id,
                            output.content.clone(),
                        )));
                    }
                    let tool_call_events = match append_upstream_response(
                        &mut conversation,
                        &tool_registry,
                        &builder,
                        output.content,
                        output.tool_calls,
                    ) {
                        Ok(events) => events,
                        Err(error) => {
                            tracing::warn!(%error, "failed to convert upstream tool call");
                            state.conversations.save(conversation).await;
                            yield Ok(sse::encode_response_event_for_sse(
                                &builder.finished_internal_error(error.to_string()),
                            ));
                            return;
                        }
                    };
                    for event in tool_call_events {
                        yield Ok(sse::encode_response_event_for_sse(&event));
                    }
                    yield Ok(sse::encode_response_event_for_sse(&builder.model_used(
                        &conversation.task_id,
                        &model_message_id,
                        &resolved_model.warp_model_id,
                        &resolved_model.upstream_model,
                    )));
                    yield Ok(sse::encode_response_event_for_sse(&builder.finished_success(
                        output.usage.map(|usage| (resolved_model.upstream_model.clone(), usage)),
                    )));
                    state.conversations.save(conversation).await;
                }
                Err(error) => {
                    tracing::warn!(%error, "upstream non-streaming completion failed");
                    state.conversations.save(conversation).await;
                    yield Ok(sse::encode_response_event_for_sse(
                        &builder.finished_internal_error(error.to_string()),
                    ));
                }
            }
        }
    }
}

fn stream_upstream_response(
    http_client: reqwest::Client,
    builder: ResponseBuilder,
    task_id: String,
    message_id: String,
    upstream: UpstreamConfig,
    chat_request: openai::OpenAiChatRequest,
) -> impl Stream<Item = StreamRouteItem> {
    stream! {
        let mut upstream_stream = match openai::stream_chat_completion(
            &http_client,
            &upstream,
            chat_request,
        ) {
            Ok(stream) => stream,
            Err(error) => {
                yield StreamRouteItem::Error(error.to_string());
                return;
            }
        };

        let mut has_message = false;
        while let Some(event) = upstream_stream.next().await {
            match event {
                Ok(reqwest_eventsource::Event::Open) => {}
                Ok(reqwest_eventsource::Event::Message(message)) => {
                    match openai::parse_stream_item(&message.data) {
                        Ok(openai::StreamItem::Chunk(chunk)) => {
                            let content = chunk.content;
                            let tool_call_deltas = chunk.tool_call_deltas;
                            if !content.is_empty() {
                                let response_event = if has_message {
                                    builder.append_agent_output(&task_id, &message_id, content.clone())
                                } else {
                                    has_message = true;
                                    builder.add_agent_output(&task_id, &message_id, content.clone())
                                };
                                yield StreamRouteItem::TextDelta(content);
                                yield StreamRouteItem::Event(sse::encode_response_event_for_sse(&response_event));
                            }
                            if !tool_call_deltas.is_empty() {
                                yield StreamRouteItem::ToolCallDeltas(tool_call_deltas);
                            }
                        }
                        Ok(openai::StreamItem::Usage(usage)) => {
                            yield StreamRouteItem::Usage(usage);
                        }
                        Ok(openai::StreamItem::Done) => break,
                        Ok(openai::StreamItem::Empty) => {}
                        Err(error) => {
                            upstream_stream.close();
                            yield StreamRouteItem::Error(error.to_string());
                            return;
                        }
                    }
                }
                Err(error) => {
                    upstream_stream.close();
                    yield StreamRouteItem::Error(error.to_string());
                    return;
                }
            }
        }
    }
}

enum StreamRouteItem {
    Event(axum::response::sse::Event),
    TextDelta(String),
    ToolCallDeltas(Vec<openai::OpenAiToolCallDelta>),
    Usage(crate::protocol::response_builder::UsageTotals),
    Error(String),
}

fn append_upstream_response(
    conversation: &mut crate::conversation::store::ConversationState,
    tool_registry: &ToolRegistry,
    builder: &ResponseBuilder,
    final_text: String,
    openai_tool_calls: Vec<openai::OpenAiToolCall>,
) -> Result<Vec<api::ResponseEvent>> {
    if openai_tool_calls.is_empty() {
        if !final_text.is_empty() {
            conversation
                .messages
                .push(TranscriptMessage::assistant(final_text));
        }
        return Ok(vec![]);
    }

    let mut converted_tool_calls = Vec::new();
    for openai_tool_call in &openai_tool_calls {
        converted_tool_calls.push(tool_registry.convert_openai_tool_call(
            openai_tool_call,
            format!("local-tool-call-{}", Uuid::new_v4()),
        )?);
    }

    let transcript_calls = converted_tool_calls
        .iter()
        .map(|conversion| conversion.openai_tool_call.clone())
        .collect();
    conversation
        .messages
        .push(TranscriptMessage::assistant_tool_calls(
            final_text,
            transcript_calls,
        ));

    let mut events = Vec::new();
    for conversion in converted_tool_calls {
        let message_id = format!("local-tool-message-{}", Uuid::new_v4());
        conversation.pending_tool_calls.insert(
            conversion.pending.warp_tool_call_id.clone(),
            conversion.pending,
        );
        events.push(builder.add_tool_call(
            &conversation.task_id,
            &message_id,
            conversion.tool_call,
        ));
    }
    Ok(events)
}

fn append_tool_call_results(
    conversation: &mut crate::conversation::store::ConversationState,
    request: &api::Request,
    tool_registry: &ToolRegistry,
) {
    for result in tool_call_results_from_request(request) {
        if conversation
            .completed_tool_call_ids
            .contains(&result.tool_call_id)
        {
            tracing::warn!(
                tool_call_id = %result.tool_call_id,
                "ignoring duplicate tool-call result"
            );
            continue;
        }

        if let Some(pending) = conversation.pending_tool_calls.remove(&result.tool_call_id) {
            if !tool_result_matches_pending(result, &pending.kind) {
                tracing::warn!(
                    tool_call_id = %result.tool_call_id,
                    openai_tool_name = %pending.openai_tool_name,
                    expected_kind = ?pending.kind,
                    "ignoring mismatched tool-call result"
                );
                conversation
                    .pending_tool_calls
                    .insert(result.tool_call_id.clone(), pending);
                continue;
            }

            let content = tool_registry.result_to_openai_content(result);
            conversation.messages.push(TranscriptMessage::tool_result(
                pending.openai_tool_call_id,
                content,
            ));
            conversation
                .completed_tool_call_ids
                .insert(result.tool_call_id.clone());
        } else {
            let content = tool_registry.result_to_openai_content(result);
            tracing::warn!(
                tool_call_id = %result.tool_call_id,
                "received result for unknown tool call; appending as user-visible text context"
            );
            conversation.messages.push(TranscriptMessage::user(format!(
                "Tool result for unknown tool_call_id `{}`:\n{}",
                result.tool_call_id, content
            )));
            conversation
                .completed_tool_call_ids
                .insert(result.tool_call_id.clone());
        }
    }
}

#[allow(deprecated)]
fn tool_call_results_from_request(
    request: &api::Request,
) -> Vec<&api::request::input::ToolCallResult> {
    let Some(input) = request.input.as_ref() else {
        return vec![];
    };
    match input.r#type.as_ref() {
        Some(api::request::input::Type::ToolCallResult(result)) => vec![result],
        Some(api::request::input::Type::UserInputs(user_inputs)) => user_inputs
            .inputs
            .iter()
            .filter_map(|input| match input.input.as_ref() {
                Some(api::request::input::user_inputs::user_input::Input::ToolCallResult(
                    result,
                )) => Some(result),
                _ => None,
            })
            .collect(),
        _ => vec![],
    }
}

fn tool_result_matches_pending(
    result: &api::request::input::ToolCallResult,
    pending_kind: &WarpToolKind,
) -> bool {
    match pending_kind {
        WarpToolKind::Builtin(expected) => tool_result_type(result) == Some(*expected),
        WarpToolKind::Mcp { .. } => matches!(
            result.result.as_ref(),
            Some(api::request::input::tool_call_result::Result::CallMcpTool(
                _
            ))
        ),
    }
}

#[allow(deprecated)]
fn tool_result_type(result: &api::request::input::ToolCallResult) -> Option<api::ToolType> {
    use api::request::input::tool_call_result::Result as ResultType;

    match result.result.as_ref()? {
        ResultType::RunShellCommand(_) => Some(api::ToolType::RunShellCommand),
        ResultType::WriteToLongRunningShellCommand(_) => {
            Some(api::ToolType::WriteToLongRunningShellCommand)
        }
        ResultType::ReadShellCommandOutput(_) => Some(api::ToolType::ReadShellCommandOutput),
        ResultType::ReadFiles(_) => Some(api::ToolType::ReadFiles),
        ResultType::ApplyFileDiffs(_) => Some(api::ToolType::ApplyFileDiffs),
        ResultType::SearchCodebase(_) => Some(api::ToolType::SearchCodebase),
        ResultType::Grep(_) => Some(api::ToolType::Grep),
        ResultType::FileGlob(_) => Some(api::ToolType::FileGlob),
        ResultType::FileGlobV2(_) => Some(api::ToolType::FileGlobV2),
        ResultType::ReadMcpResource(_) => Some(api::ToolType::ReadMcpResource),
        ResultType::CallMcpTool(_) => Some(api::ToolType::CallMcpTool),
        _ => None,
    }
}

#[derive(Clone, Debug)]
struct ResponseContext {
    conversation_id: String,
    request_id: String,
    run_id: String,
    task_id: String,
    had_request_conversation_id: bool,
    had_request_task_id: bool,
}

impl ResponseContext {
    fn from_request(request: &api::Request) -> Self {
        let request_conversation_id = request
            .metadata
            .as_ref()
            .and_then(|metadata| non_empty(&metadata.conversation_id));
        let task_id_from_request = first_task_id(request);
        let generated_task_id = format!("local-task-{}", Uuid::new_v4());
        let task_id = if request_conversation_id.is_some() {
            task_id_from_request
                .clone()
                .unwrap_or_else(|| generated_task_id.clone())
        } else {
            generated_task_id.clone()
        };
        let run_id = request
            .metadata
            .as_ref()
            .and_then(|metadata| non_empty(&metadata.ambient_agent_task_id))
            .map(str::to_string)
            .unwrap_or_else(|| format!("local-run-{task_id}"));

        Self {
            conversation_id: request_conversation_id
                .map(str::to_string)
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            request_id: Uuid::new_v4().to_string(),
            run_id,
            task_id,
            had_request_conversation_id: request_conversation_id.is_some(),
            had_request_task_id: task_id_from_request.is_some(),
        }
    }
}

#[derive(Clone, Debug)]
struct ResolvedModel {
    warp_model_id: String,
    upstream_model: String,
    upstream: UpstreamConfig,
}

fn resolve_model(config: &ShimConfig, request: &api::Request) -> Result<ResolvedModel> {
    let warp_model_id = request
        .settings
        .as_ref()
        .and_then(|settings| settings.model_config.as_ref())
        .and_then(|model_config| non_empty(&model_config.base))
        .unwrap_or("auto");

    let mapping = config
        .models
        .get(warp_model_id)
        .or_else(|| config.models.get("auto"))
        .or_else(|| config.models.values().next())
        .ok_or_else(|| anyhow!("no model mappings configured in warp shim"))?;
    let upstream = config.upstreams.get(&mapping.upstream).with_context(|| {
        format!(
            "model mapping references unknown upstream `{}`",
            mapping.upstream
        )
    })?;

    Ok(ResolvedModel {
        warp_model_id: warp_model_id.to_string(),
        upstream_model: mapping.model.clone(),
        upstream: upstream.clone(),
    })
}

fn should_create_task(context: &ResponseContext, is_new_conversation: bool) -> bool {
    is_new_conversation && (!context.had_request_conversation_id || !context.had_request_task_id)
}

fn first_task_id(request: &api::Request) -> Option<String> {
    request
        .task_context
        .as_ref()?
        .tasks
        .iter()
        .find_map(|task| non_empty(&task.id).map(str::to_string))
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, HashMap, HashSet},
        net::Ipv4Addr,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use axum::{
        Json, Router,
        body::{Body, to_bytes},
        extract::State as AxumState,
        http::{Request, StatusCode},
        response::sse::{Event, Sse},
        routing::post,
    };
    use futures_util::stream;
    use prost::Message as _;
    use serde_json::{Value, json};
    use tokio::{
        net::TcpListener,
        sync::{Notify, oneshot},
        time::{Duration, sleep},
    };
    use tower::ServiceExt;
    use url::Url;

    use super::*;
    use crate::{
        config::{FeatureConfig, ModelMapping, ServerConfig},
        protocol::sse::decode_response_event_data_like_client,
    };

    #[tokio::test]
    async fn multi_agent_streams_fake_openai_reply_as_warp_response_events() {
        let upstream_url = spawn_fake_openai_upstream().await;
        let config = test_config(upstream_url, true);
        let request = test_multi_agent_request();

        let response = crate::server::router(Arc::new(config))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/multi-agent")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(request.encode_to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let events = decode_sse_response_events(&body);

        assert!(matches!(
            events.first().and_then(|event| event.r#type.as_ref()),
            Some(api::response_event::Type::Init(_))
        ));
        assert!(events.iter().any(|event| matches!(
            event.r#type.as_ref(),
            Some(api::response_event::Type::Finished(finished))
                if matches!(
                    finished.reason,
                    Some(api::response_event::stream_finished::Reason::Done(_))
                )
        )));

        let (assistant_text, agent_output_ids) = collect_agent_output_text(&events);
        assert_eq!(assistant_text, "Hello from fake upstream.");
        assert_eq!(
            agent_output_ids.len(),
            1,
            "agent output message id must be stable"
        );
        assert!(events.iter().any(contains_model_used_event));
    }

    #[tokio::test]
    async fn tool_call_result_loop_resumes_upstream_and_emits_final_text() {
        let seen_requests = Arc::new(Mutex::new(Vec::new()));
        let upstream_url = spawn_tool_loop_openai_upstream(seen_requests.clone()).await;
        let config = test_config(upstream_url, true);
        let app = crate::server::router(Arc::new(config));

        let first_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/multi-agent")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(
                        test_multi_agent_request_with_supported_tools(
                            "tool-loop-conversation",
                            &[api::ToolType::RunShellCommand],
                        )
                        .encode_to_vec(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first_response.status(), StatusCode::OK);
        let first_body = to_bytes(first_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let first_events = decode_sse_response_events(&first_body);
        let tool_calls = collect_tool_calls(&first_events);
        assert_eq!(tool_calls.len(), 1);
        let warp_tool_call_id = tool_calls[0].tool_call_id.clone();
        let Some(api::message::tool_call::Tool::RunShellCommand(run)) = &tool_calls[0].tool else {
            panic!("expected run shell command tool call");
        };
        assert_eq!(run.command, "pwd");

        let second_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/multi-agent")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(
                        test_tool_result_request("tool-loop-conversation", &warp_tool_call_id)
                            .encode_to_vec(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second_response.status(), StatusCode::OK);
        let second_body = to_bytes(second_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let second_events = decode_sse_response_events(&second_body);
        let (assistant_text, _) = collect_agent_output_text(&second_events);
        assert_eq!(assistant_text, "Tool result received.");

        let requests = seen_requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(
            requests[0]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["function"]["name"] == "run_shell_command")
        );
        let second_messages = requests[1]["messages"].as_array().unwrap();
        assert!(second_messages.iter().any(|message| {
            message["role"] == "assistant"
                && message["tool_calls"][0]["id"] == "call_run_1"
                && message["tool_calls"][0]["function"]["name"] == "run_shell_command"
        }));
        assert!(second_messages.iter().any(|message| {
            message["role"] == "tool" && message["tool_call_id"] == "call_run_1"
        }));
    }

    #[test]
    #[allow(deprecated)]
    fn mismatched_tool_result_keeps_pending_call_and_does_not_append_context() {
        let registry =
            ToolRegistry::for_request(&api::Request::default(), &FeatureConfig::default());
        let mut conversation = crate::conversation::store::ConversationState {
            conversation_id: "conversation".to_string(),
            task_id: "task".to_string(),
            messages: vec![],
            pending_tool_calls: HashMap::from([(
                "warp-call".to_string(),
                crate::conversation::transcript::PendingToolCall {
                    warp_tool_call_id: "warp-call".to_string(),
                    openai_tool_call_id: "openai-call".to_string(),
                    openai_tool_name: "run_shell_command".to_string(),
                    kind: WarpToolKind::Builtin(api::ToolType::RunShellCommand),
                },
            )]),
            completed_tool_call_ids: HashSet::new(),
        };
        let request = api::Request {
            input: Some(api::request::Input {
                r#type: Some(api::request::input::Type::ToolCallResult(
                    api::request::input::ToolCallResult {
                        tool_call_id: "warp-call".to_string(),
                        result: Some(api::request::input::tool_call_result::Result::ReadFiles(
                            api::ReadFilesResult::default(),
                        )),
                    },
                )),
                ..Default::default()
            }),
            ..Default::default()
        };

        append_tool_call_results(&mut conversation, &request, &registry);

        assert!(conversation.pending_tool_calls.contains_key("warp-call"));
        assert!(conversation.completed_tool_call_ids.is_empty());
        assert!(conversation.messages.is_empty());
    }

    #[tokio::test]
    async fn mcp_tools_are_declared_with_sanitized_names_and_routed_back_to_call_mcp_tool() {
        let seen_requests = Arc::new(Mutex::new(Vec::new()));
        let upstream_url = spawn_mcp_tool_openai_upstream(seen_requests.clone()).await;
        let config = test_config(upstream_url, true);
        let request = test_mcp_request();

        let response = crate::server::router(Arc::new(config))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/multi-agent")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(request.encode_to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let events = decode_sse_response_events(&body);
        let tool_calls = collect_tool_calls(&events);
        assert_eq!(tool_calls.len(), 1);
        let Some(api::message::tool_call::Tool::CallMcpTool(call)) = &tool_calls[0].tool else {
            panic!("expected CallMcpTool tool call");
        };
        assert_eq!(call.server_id, "linear-prod");
        assert_eq!(call.name, "create.issue");
        let args = struct_to_json_for_test(call.args.as_ref().unwrap());
        assert_eq!(args["title"], "Bug");

        let requests = seen_requests.lock().unwrap();
        let tools = requests[0]["tools"].as_array().unwrap();
        let mcp_tool = tools
            .iter()
            .find(|tool| tool["function"]["name"] == "mcp__linear_prod__create_issue")
            .expect("MCP tool declaration");
        assert_eq!(
            mcp_tool["function"]["parameters"]["properties"]["title"]["type"],
            "string"
        );
    }

    #[tokio::test]
    async fn passive_suggestions_returns_successful_noop_stream() {
        let config = test_config(Url::parse("http://127.0.0.1:1/v1").unwrap(), true);
        let mut request = test_multi_agent_request();
        request.input = Some(api::request::Input {
            r#type: Some(api::request::input::Type::GeneratePassiveSuggestions(
                api::request::input::GeneratePassiveSuggestions::default(),
            )),
            ..Default::default()
        });

        let response = crate::server::router(Arc::new(config))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/passive-suggestions")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(request.encode_to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let events = decode_sse_response_events(&body);
        assert!(events.iter().any(|event| matches!(
            event.r#type.as_ref(),
            Some(api::response_event::Type::Finished(finished))
                if matches!(finished.reason, Some(api::response_event::stream_finished::Reason::Done(_)))
        )));
    }

    #[tokio::test]
    async fn invalid_protobuf_returns_json_400_before_sse_starts() {
        let config = test_config(Url::parse("http://127.0.0.1:1/v1").unwrap(), true);
        let response = crate::server::router(Arc::new(config))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/ai/multi-agent")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from("not protobuf"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );
    }

    #[tokio::test]
    async fn http_e2e_streams_fake_openai_reply_over_reqwest_protobuf() {
        let upstream_url = spawn_fake_openai_upstream().await;
        let (shim_url, shutdown, server) = spawn_shim_server(test_config(upstream_url, true)).await;

        let response = reqwest::Client::new()
            .post(shim_url.join("/ai/multi-agent").unwrap())
            .header(reqwest::header::CONTENT_TYPE, "application/x-protobuf")
            .body(test_multi_agent_request().encode_to_vec())
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(
            content_type.starts_with("text/event-stream"),
            "unexpected content-type: {content_type}"
        );

        let body = response.bytes().await.unwrap();
        let events = decode_sse_response_events(&body);
        let (assistant_text, _) = collect_agent_output_text(&events);
        assert_eq!(assistant_text, "Hello from fake upstream.");
        assert!(has_finished_done(&events));

        shutdown_shim_server(shutdown, server).await;
    }

    #[tokio::test]
    async fn tool_loop_http_e2e_sends_tool_result_context_upstream() {
        let seen_requests = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let upstream_url = spawn_tool_loop_openai_upstream_raw(seen_requests.clone()).await;
        let (shim_url, shutdown, server) = spawn_shim_server(test_config(upstream_url, true)).await;
        let client = reqwest::Client::new();

        let first_response = client
            .post(shim_url.join("/ai/multi-agent").unwrap())
            .header(reqwest::header::CONTENT_TYPE, "application/x-protobuf")
            .body(
                test_multi_agent_request_with_supported_tools(
                    "tool-loop-http-conversation",
                    &[api::ToolType::RunShellCommand],
                )
                .encode_to_vec(),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(first_response.status(), StatusCode::OK);
        let first_events = decode_sse_response_events(&first_response.bytes().await.unwrap());
        let tool_calls = collect_tool_calls(&first_events);
        assert_eq!(tool_calls.len(), 1);
        let warp_tool_call_id = tool_calls[0].tool_call_id.clone();
        let Some(api::message::tool_call::Tool::RunShellCommand(run)) = &tool_calls[0].tool else {
            panic!("expected run shell command tool call");
        };
        assert_eq!(run.command, "pwd");
        assert!(has_finished_done(&first_events));

        let second_response = client
            .post(shim_url.join("/ai/multi-agent").unwrap())
            .header(reqwest::header::CONTENT_TYPE, "application/x-protobuf")
            .body(
                test_tool_result_request("tool-loop-http-conversation", &warp_tool_call_id)
                    .encode_to_vec(),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(second_response.status(), StatusCode::OK);
        let second_events = decode_sse_response_events(&second_response.bytes().await.unwrap());
        let (assistant_text, _) = collect_agent_output_text(&second_events);
        assert_eq!(assistant_text, "Tool result received.");
        assert!(has_finished_done(&second_events));

        {
            let requests = seen_requests.lock().unwrap();
            assert_eq!(requests.len(), 2);
            let first_upstream_request: Value = serde_json::from_slice(&requests[0]).unwrap();
            assert!(
                first_upstream_request["tools"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|tool| tool["function"]["name"] == "run_shell_command")
            );

            let second_upstream_request: Value = serde_json::from_slice(&requests[1]).unwrap();
            let second_messages = second_upstream_request["messages"].as_array().unwrap();
            assert!(second_messages.iter().any(|message| {
                message["role"] == "assistant"
                    && message["tool_calls"][0]["id"] == "call_run_1"
                    && message["tool_calls"][0]["function"]["name"] == "run_shell_command"
            }));
            assert!(second_messages.iter().any(|message| {
                message["role"] == "tool" && message["tool_call_id"] == "call_run_1"
            }));
        }

        shutdown_shim_server(shutdown, server).await;
    }

    #[tokio::test]
    async fn same_conversation_requests_are_serialized_before_upstream() {
        let upstream_state = Arc::new(BlockingUpstreamState::new());
        let upstream_url = spawn_blocking_counter_openai_upstream(upstream_state.clone()).await;
        let (shim_url, shutdown, server) = spawn_shim_server(test_config(upstream_url, true)).await;
        let client = reqwest::Client::new();

        let first_request =
            test_multi_agent_request_with_supported_tools("shared-conversation", &[]);
        let first_url = shim_url.join("/ai/multi-agent").unwrap();
        let first_client = client.clone();
        let first = tokio::spawn(async move {
            let response = first_client
                .post(first_url)
                .header(reqwest::header::CONTENT_TYPE, "application/x-protobuf")
                .body(first_request.encode_to_vec())
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            decode_sse_response_events(&response.bytes().await.unwrap())
        });

        upstream_state.first_started.notified().await;

        let second_request =
            test_multi_agent_request_with_supported_tools("shared-conversation", &[]);
        let second_url = shim_url.join("/ai/multi-agent").unwrap();
        let second_client = client.clone();
        let second = tokio::spawn(async move {
            let response = second_client
                .post(second_url)
                .header(reqwest::header::CONTENT_TYPE, "application/x-protobuf")
                .body(second_request.encode_to_vec())
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            decode_sse_response_events(&response.bytes().await.unwrap())
        });

        sleep(Duration::from_millis(50)).await;
        assert_eq!(
            upstream_state.hits.load(Ordering::SeqCst),
            1,
            "second request reached upstream before first turn finished"
        );

        upstream_state.release_first.notify_waiters();
        let first_events = first.await.unwrap();
        let second_events = second.await.unwrap();

        assert!(has_finished_done(&first_events));
        assert!(has_finished_done(&second_events));
        assert_eq!(upstream_state.hits.load(Ordering::SeqCst), 2);
        assert_eq!(upstream_state.max_active.load(Ordering::SeqCst), 1);

        shutdown_shim_server(shutdown, server).await;
    }

    #[tokio::test]
    async fn upstream_http_error_after_sse_starts_finishes_with_internal_error() {
        let upstream_url = spawn_http_error_openai_upstream().await;
        let (shim_url, shutdown, server) = spawn_shim_server(test_config(upstream_url, true)).await;

        let response = reqwest::Client::new()
            .post(shim_url.join("/ai/multi-agent").unwrap())
            .header(reqwest::header::CONTENT_TYPE, "application/x-protobuf")
            .body(test_multi_agent_request().encode_to_vec())
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let events = decode_sse_response_events(&response.bytes().await.unwrap());
        assert!(matches!(
            events.first().and_then(|event| event.r#type.as_ref()),
            Some(api::response_event::Type::Init(_))
        ));
        assert!(has_finished_internal_error(&events));
        assert!(!has_finished_done(&events));

        shutdown_shim_server(shutdown, server).await;
    }

    async fn spawn_tool_loop_openai_upstream(seen_requests: Arc<Mutex<Vec<Value>>>) -> Url {
        async fn chat_completions(
            AxumState(seen_requests): AxumState<Arc<Mutex<Vec<Value>>>>,
            Json(body): Json<Value>,
        ) -> Sse<impl futures_util::Stream<Item = std::result::Result<Event, Infallible>>> {
            seen_requests.lock().unwrap().push(body.clone());
            let has_tool_result = body["messages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|message| message["role"] == "tool");

            let chunks = if has_tool_result {
                vec![
                    Ok(Event::default().data(
                        json!({ "choices": [{ "delta": { "content": "Tool result received." } }] })
                            .to_string(),
                    )),
                    Ok(Event::default().data("[DONE]")),
                ]
            } else {
                assert_eq!(body["tool_choice"], "auto");
                vec![
                    Ok(Event::default().data(
                        json!({
                            "choices": [{
                                "delta": {
                                    "tool_calls": [{
                                        "index": 0,
                                        "id": "call_run_1",
                                        "type": "function",
                                        "function": {
                                            "name": "run_shell_command",
                                            "arguments": "{\"command\":\"pwd\",\"risk_category\":\"read_only\"}"
                                        }
                                    }]
                                }
                            }]
                        })
                        .to_string(),
                    )),
                    Ok(Event::default().data("[DONE]")),
                ]
            };
            Sse::new(stream::iter(chunks))
        }

        let app = Router::new()
            .route("/v1/chat/completions", post(chat_completions))
            .with_state(seen_requests);
        spawn_openai_router(app).await
    }

    async fn spawn_tool_loop_openai_upstream_raw(seen_requests: Arc<Mutex<Vec<Vec<u8>>>>) -> Url {
        async fn chat_completions(
            AxumState(seen_requests): AxumState<Arc<Mutex<Vec<Vec<u8>>>>>,
            body: Bytes,
        ) -> Sse<impl futures_util::Stream<Item = std::result::Result<Event, Infallible>>> {
            seen_requests.lock().unwrap().push(body.to_vec());
            let body: Value = serde_json::from_slice(&body).unwrap();
            let has_tool_result = body["messages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|message| message["role"] == "tool");

            let chunks = if has_tool_result {
                vec![
                    Ok(Event::default().data(
                        json!({ "choices": [{ "delta": { "content": "Tool result received." } }] })
                            .to_string(),
                    )),
                    Ok(Event::default().data("[DONE]")),
                ]
            } else {
                vec![
                    Ok(Event::default().data(
                        json!({
                            "choices": [{
                                "delta": {
                                    "tool_calls": [{
                                        "index": 0,
                                        "id": "call_run_1",
                                        "type": "function",
                                        "function": {
                                            "name": "run_shell_command",
                                            "arguments": "{\"command\":\"pwd\",\"risk_category\":\"read_only\"}"
                                        }
                                    }]
                                }
                            }]
                        })
                        .to_string(),
                    )),
                    Ok(Event::default().data("[DONE]")),
                ]
            };
            Sse::new(stream::iter(chunks))
        }

        let app = Router::new()
            .route("/v1/chat/completions", post(chat_completions))
            .with_state(seen_requests);
        spawn_openai_router(app).await
    }

    struct BlockingUpstreamState {
        active: AtomicUsize,
        hits: AtomicUsize,
        max_active: AtomicUsize,
        first_started: Notify,
        release_first: Notify,
    }

    impl BlockingUpstreamState {
        fn new() -> Self {
            Self {
                active: AtomicUsize::new(0),
                hits: AtomicUsize::new(0),
                max_active: AtomicUsize::new(0),
                first_started: Notify::new(),
                release_first: Notify::new(),
            }
        }

        fn enter(&self) {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            let mut observed = self.max_active.load(Ordering::SeqCst);
            while active > observed {
                match self.max_active.compare_exchange(
                    observed,
                    active,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(next) => observed = next,
                }
            }
        }

        fn exit(&self) {
            self.active.fetch_sub(1, Ordering::SeqCst);
        }
    }

    async fn spawn_blocking_counter_openai_upstream(state: Arc<BlockingUpstreamState>) -> Url {
        async fn chat_completions(
            AxumState(state): AxumState<Arc<BlockingUpstreamState>>,
            _body: Bytes,
        ) -> Sse<impl futures_util::Stream<Item = std::result::Result<Event, Infallible>>> {
            state.enter();
            let hit = state.hits.fetch_add(1, Ordering::SeqCst) + 1;
            if hit == 1 {
                state.first_started.notify_waiters();
                state.release_first.notified().await;
            }
            let chunks = vec![
                Ok(Event::default().data(
                    json!({ "choices": [{ "delta": { "content": format!("reply {hit}") } }] })
                        .to_string(),
                )),
                Ok(Event::default().data("[DONE]")),
            ];
            state.exit();
            Sse::new(stream::iter(chunks))
        }

        let app = Router::new()
            .route("/v1/chat/completions", post(chat_completions))
            .with_state(state);
        spawn_openai_router(app).await
    }

    async fn spawn_http_error_openai_upstream() -> Url {
        async fn chat_completions() -> impl IntoResponse {
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "fake upstream exploded" })),
            )
        }

        let app = Router::new().route("/v1/chat/completions", post(chat_completions));
        spawn_openai_router(app).await
    }

    async fn spawn_shim_server(
        config: ShimConfig,
    ) -> (
        Url,
        oneshot::Sender<()>,
        tokio::task::JoinHandle<anyhow::Result<()>>,
    ) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown, shutdown_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            crate::server::serve_listener(listener, config, async move {
                let _ = shutdown_rx.await;
            })
            .await
        });
        (
            Url::parse(&format!("http://{addr}")).unwrap(),
            shutdown,
            server,
        )
    }

    async fn shutdown_shim_server(
        shutdown: oneshot::Sender<()>,
        server: tokio::task::JoinHandle<anyhow::Result<()>>,
    ) {
        let _ = shutdown.send(());
        let result = server.await.unwrap();
        assert!(result.is_ok(), "shim server failed: {result:?}");
    }

    async fn spawn_mcp_tool_openai_upstream(seen_requests: Arc<Mutex<Vec<Value>>>) -> Url {
        async fn chat_completions(
            AxumState(seen_requests): AxumState<Arc<Mutex<Vec<Value>>>>,
            Json(body): Json<Value>,
        ) -> Sse<impl futures_util::Stream<Item = std::result::Result<Event, Infallible>>> {
            seen_requests.lock().unwrap().push(body.clone());
            let chunks = vec![
                Ok(Event::default().data(
                    json!({
                        "choices": [{
                            "delta": {
                                "tool_calls": [{
                                    "index": 0,
                                    "id": "call_mcp_1",
                                    "type": "function",
                                    "function": {
                                        "name": "mcp__linear_prod__create_issue",
                                        "arguments": "{\"title\":\"Bug\"}"
                                    }
                                }]
                            }
                        }]
                    })
                    .to_string(),
                )),
                Ok(Event::default().data("[DONE]")),
            ];
            Sse::new(stream::iter(chunks))
        }

        let app = Router::new()
            .route("/v1/chat/completions", post(chat_completions))
            .with_state(seen_requests);
        spawn_openai_router(app).await
    }

    async fn spawn_openai_router(app: Router) -> Url {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Url::parse(&format!("http://{addr}/v1")).unwrap()
    }

    async fn spawn_fake_openai_upstream() -> Url {
        async fn chat_completions(
            Json(body): Json<Value>,
        ) -> Sse<impl futures_util::Stream<Item = std::result::Result<Event, Infallible>>> {
            assert_eq!(body["model"], "local-test-model");
            assert_eq!(body["stream"], true);

            let chunks = vec![
                Ok(Event::default().data(
                    json!({
                        "choices": [{ "delta": { "content": "Hello " } }]
                    })
                    .to_string(),
                )),
                Ok(Event::default().data(
                    json!({
                        "choices": [{ "delta": { "content": "from fake upstream." } }]
                    })
                    .to_string(),
                )),
                Ok(Event::default().data("[DONE]")),
            ];
            Sse::new(stream::iter(chunks))
        }

        let app = Router::new().route("/v1/chat/completions", post(chat_completions));
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Url::parse(&format!("http://{addr}/v1")).unwrap()
    }

    fn test_config(upstream_url: Url, streaming: bool) -> ShimConfig {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "default".to_string(),
            UpstreamConfig {
                base_url: upstream_url,
                api_key: None,
                api_key_env: None,
                timeout_secs: 180,
                streaming,
            },
        );

        let mut models = BTreeMap::new();
        models.insert(
            "auto".to_string(),
            ModelMapping {
                upstream: "default".to_string(),
                model: "local-test-model".to_string(),
            },
        );

        ShimConfig {
            config_path: None,
            server: ServerConfig {
                host: Ipv4Addr::LOCALHOST.into(),
                port: 4444,
                public_base_url: "http://127.0.0.1:4444".to_string(),
            },
            upstreams,
            models,
            features: FeatureConfig::default(),
        }
    }

    fn test_multi_agent_request() -> api::Request {
        api::Request {
            task_context: Some(api::request::TaskContext {
                tasks: vec![api::Task {
                    id: "client-task".to_string(),
                    ..Default::default()
                }],
            }),
            input: Some(api::request::Input {
                context: Some(api::InputContext {
                    directory: Some(api::input_context::Directory {
                        pwd: "/tmp/warp-shim-test".to_string(),
                        ..Default::default()
                    }),
                    shell: Some(api::input_context::Shell {
                        name: "zsh".to_string(),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                r#type: Some(api::request::input::Type::UserInputs(
                    api::request::input::UserInputs {
                        inputs: vec![api::request::input::user_inputs::UserInput {
                            input: Some(
                                api::request::input::user_inputs::user_input::Input::UserQuery(
                                    api::request::input::UserQuery {
                                        query: "Say hello".to_string(),
                                        ..Default::default()
                                    },
                                ),
                            ),
                        }],
                    },
                )),
            }),
            settings: Some(api::request::Settings {
                model_config: Some(api::request::settings::ModelConfig {
                    base: "auto".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            metadata: Some(api::request::Metadata::default()),
            ..Default::default()
        }
    }

    fn test_multi_agent_request_with_supported_tools(
        conversation_id: &str,
        supported_tools: &[api::ToolType],
    ) -> api::Request {
        let mut request = test_multi_agent_request();
        request.metadata = Some(api::request::Metadata {
            conversation_id: conversation_id.to_string(),
            ..Default::default()
        });
        request.settings = Some(api::request::Settings {
            model_config: Some(api::request::settings::ModelConfig {
                base: "auto".to_string(),
                ..Default::default()
            }),
            supported_tools: supported_tools.iter().map(|tool| *tool as i32).collect(),
            ..Default::default()
        });
        request
    }

    fn test_tool_result_request(conversation_id: &str, tool_call_id: &str) -> api::Request {
        api::Request {
            task_context: Some(api::request::TaskContext {
                tasks: vec![api::Task {
                    id: "client-task".to_string(),
                    ..Default::default()
                }],
            }),
            input: Some(api::request::Input {
                r#type: Some(api::request::input::Type::UserInputs(
                    api::request::input::UserInputs {
                        inputs: vec![api::request::input::user_inputs::UserInput {
                            input: Some(
                                api::request::input::user_inputs::user_input::Input::ToolCallResult(
                                    api::request::input::ToolCallResult {
                                        tool_call_id: tool_call_id.to_string(),
                                        result: Some(
                                            api::request::input::tool_call_result::Result::RunShellCommand(
                                                api::RunShellCommandResult {
                                                    command: "pwd".to_string(),
                                                    result: Some(
                                                        api::run_shell_command_result::Result::CommandFinished(
                                                            api::ShellCommandFinished {
                                                                command_id: "cmd-1".to_string(),
                                                                output: "/tmp\n".to_string(),
                                                                exit_code: 0,
                                                            },
                                                        ),
                                                    ),
                                                    ..Default::default()
                                                },
                                            ),
                                        ),
                                    },
                                ),
                            ),
                        }],
                    },
                )),
                ..Default::default()
            }),
            settings: Some(api::request::Settings {
                model_config: Some(api::request::settings::ModelConfig {
                    base: "auto".to_string(),
                    ..Default::default()
                }),
                supported_tools: vec![api::ToolType::RunShellCommand as i32],
                ..Default::default()
            }),
            metadata: Some(api::request::Metadata {
                conversation_id: conversation_id.to_string(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn test_mcp_request() -> api::Request {
        let mut request = test_multi_agent_request_with_supported_tools(
            "mcp-conversation",
            &[api::ToolType::CallMcpTool],
        );
        request.mcp_context = Some(api::request::McpContext {
            servers: vec![api::request::mcp_context::McpServer {
                id: "linear-prod".to_string(),
                name: "Linear Prod".to_string(),
                tools: vec![api::request::mcp_context::McpTool {
                    name: "create.issue".to_string(),
                    description: "Create a Linear issue".to_string(),
                    input_schema: Some(json_to_struct_for_test(&json!({
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" }
                        },
                        "required": ["title"]
                    }))),
                }],
                ..Default::default()
            }],
            ..Default::default()
        });
        request
    }

    fn decode_sse_response_events(body: &[u8]) -> Vec<api::ResponseEvent> {
        let body = String::from_utf8(body.to_vec()).unwrap();
        body.split("\n\n")
            .filter_map(|frame| {
                let data = frame
                    .lines()
                    .filter_map(|line| line.strip_prefix("data:"))
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join("\n");
                (!data.is_empty()).then(|| decode_response_event_data_like_client(&data).unwrap())
            })
            .collect()
    }

    fn collect_tool_calls(events: &[api::ResponseEvent]) -> Vec<api::message::ToolCall> {
        let mut tool_calls = Vec::new();
        for event in events {
            let Some(api::response_event::Type::ClientActions(actions)) = event.r#type.as_ref()
            else {
                continue;
            };
            for action in &actions.actions {
                if let Some(api::client_action::Action::AddMessagesToTask(add)) = &action.action {
                    for message in &add.messages {
                        if let Some(api::message::Message::ToolCall(tool_call)) = &message.message {
                            tool_calls.push(tool_call.clone());
                        }
                    }
                }
            }
        }
        tool_calls
    }

    fn json_to_struct_for_test(value: &Value) -> prost_types::Struct {
        let fields = value
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), json_to_prost_value_for_test(value)))
            .collect();
        prost_types::Struct { fields }
    }

    fn json_to_prost_value_for_test(value: &Value) -> prost_types::Value {
        use prost_types::value::Kind;
        prost_types::Value {
            kind: Some(match value {
                Value::Null => Kind::NullValue(0),
                Value::Bool(value) => Kind::BoolValue(*value),
                Value::Number(value) => Kind::NumberValue(value.as_f64().unwrap()),
                Value::String(value) => Kind::StringValue(value.clone()),
                Value::Array(values) => Kind::ListValue(prost_types::ListValue {
                    values: values.iter().map(json_to_prost_value_for_test).collect(),
                }),
                Value::Object(_) => Kind::StructValue(json_to_struct_for_test(value)),
            }),
        }
    }

    fn struct_to_json_for_test(value: &prost_types::Struct) -> Value {
        Value::Object(
            value
                .fields
                .iter()
                .map(|(key, value)| (key.clone(), prost_value_to_json_for_test(value)))
                .collect(),
        )
    }

    fn prost_value_to_json_for_test(value: &prost_types::Value) -> Value {
        use prost_types::value::Kind;
        match value.kind.as_ref() {
            Some(Kind::NullValue(_)) | None => Value::Null,
            Some(Kind::NumberValue(value)) => serde_json::Number::from_f64(*value)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            Some(Kind::StringValue(value)) => Value::String(value.clone()),
            Some(Kind::BoolValue(value)) => Value::Bool(*value),
            Some(Kind::StructValue(value)) => struct_to_json_for_test(value),
            Some(Kind::ListValue(value)) => Value::Array(
                value
                    .values
                    .iter()
                    .map(prost_value_to_json_for_test)
                    .collect(),
            ),
        }
    }

    fn collect_agent_output_text(events: &[api::ResponseEvent]) -> (String, Vec<String>) {
        let mut text = String::new();
        let mut ids = Vec::<String>::new();
        for event in events {
            let Some(api::response_event::Type::ClientActions(actions)) = event.r#type.as_ref()
            else {
                continue;
            };
            for action in &actions.actions {
                match action.action.as_ref() {
                    Some(api::client_action::Action::AddMessagesToTask(add)) => {
                        for message in &add.messages {
                            if let Some(api::message::Message::AgentOutput(output)) =
                                &message.message
                            {
                                if !ids.contains(&message.id) {
                                    ids.push(message.id.clone());
                                }
                                text.push_str(&output.text);
                            }
                        }
                    }
                    Some(api::client_action::Action::AppendToMessageContent(append)) => {
                        if let Some(message) = append.message.as_ref()
                            && let Some(api::message::Message::AgentOutput(output)) =
                                &message.message
                        {
                            if !ids.contains(&message.id) {
                                ids.push(message.id.clone());
                            }
                            text.push_str(&output.text);
                        }
                    }
                    _ => {}
                }
            }
        }
        (text, ids)
    }

    fn contains_model_used_event(event: &api::ResponseEvent) -> bool {
        let Some(api::response_event::Type::ClientActions(actions)) = event.r#type.as_ref() else {
            return false;
        };
        actions
            .actions
            .iter()
            .any(|action| match action.action.as_ref() {
                Some(api::client_action::Action::AddMessagesToTask(add)) => {
                    add.messages.iter().any(|message| {
                        matches!(message.message, Some(api::message::Message::ModelUsed(_)))
                    })
                }
                _ => false,
            })
    }

    fn has_finished_done(events: &[api::ResponseEvent]) -> bool {
        events.iter().any(|event| {
            matches!(
                event.r#type.as_ref(),
                Some(api::response_event::Type::Finished(finished))
                    if matches!(
                        finished.reason,
                        Some(api::response_event::stream_finished::Reason::Done(_))
                    )
            )
        })
    }

    fn has_finished_internal_error(events: &[api::ResponseEvent]) -> bool {
        events.iter().any(|event| {
            matches!(
                event.r#type.as_ref(),
                Some(api::response_event::Type::Finished(finished))
                    if matches!(
                        finished.reason,
                        Some(api::response_event::stream_finished::Reason::InternalError(_))
                    )
            )
        })
    }
}
