//! Streaming response handling for the local OpenAI-compatible Responses backend.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{anyhow, Context as _};
use serde_json::{json, Value};
use uuid::Uuid;
use warp_multi_agent_api as api;

use crate::ai::agent::task::TaskId;

use super::request::{
    assistant_output_item, assistant_output_item_with_annotations, function_call_history_item,
    reasoning_history_item, web_search_call_history_item,
};
use super::tool_calls::parse_tool_call;
use super::types::{
    ParsedFunctionCall, ResponsesApiResponse, ResponsesCompletedEvent,
    ResponsesFunctionCallArgumentsDeltaEvent, ResponsesFunctionCallArgumentsDoneEvent,
    ResponsesOutputItem, ResponsesOutputItemDoneEvent, ResponsesOutputTextAnnotation,
    ResponsesReasoningPartAddedEvent, ResponsesReasoningTextDeltaEvent,
    ResponsesReasoningTextDoneEvent, ResponsesTextDeltaEvent, ResponsesWebSearchAction,
    ResponsesWebSearchCallEvent, StreamMessageResult, StreamingFunctionCallState,
    StreamingReasoningMessageState, StreamingResponsesAccumulator, StreamingTextMessageState,
    StreamingWebSearchState,
};
use super::{
    add_messages_event, conversation_state_store, finished_reason_for_error, stream_finished_event,
    user_visible_error_event, Event, RequestParams,
};

/// Translates a streamed Responses SSE message into Warp client events and updates stream state.
pub(super) fn handle_responses_stream_message(
    params: &RequestParams,
    task_id: &TaskId,
    request_id: &str,
    event_name: &str,
    data: &str,
    accumulator: &mut StreamingResponsesAccumulator,
) -> anyhow::Result<StreamMessageResult> {
    let payload = serde_json::from_str::<Value>(data)
        .with_context(|| format!("Failed to decode streamed Responses event payload: {data}"))?;
    let event_type = streamed_event_type(event_name, &payload);

    match event_type.as_str() {
        "response.output_text.delta" | "response.text.delta" => {
            let delta_event: ResponsesTextDeltaEvent = serde_json::from_value(payload)?;
            let Some(event) =
                handle_streamed_text_delta(task_id, request_id, accumulator, delta_event)?
            else {
                return Ok(StreamMessageResult::default());
            };

            Ok(StreamMessageResult {
                events: vec![Ok(event)],
                is_terminal: false,
            })
        }
        "response.reasoning_summary_part.added" | "response.reasoning_text_part.added" => {
            let added_event: ResponsesReasoningPartAddedEvent = serde_json::from_value(payload)?;
            handle_streamed_reasoning_part_added(event_type.as_str(), accumulator, added_event)?;
            Ok(StreamMessageResult::default())
        }
        "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
            let delta_event: ResponsesReasoningTextDeltaEvent = serde_json::from_value(payload)?;
            let Some(event) = handle_streamed_reasoning_delta(
                task_id,
                request_id,
                accumulator,
                event_type.as_str(),
                delta_event,
            )?
            else {
                return Ok(StreamMessageResult::default());
            };

            Ok(StreamMessageResult {
                events: vec![Ok(event)],
                is_terminal: false,
            })
        }
        "response.reasoning_summary_text.done" | "response.reasoning_text.done" => {
            let done_event: ResponsesReasoningTextDoneEvent = serde_json::from_value(payload)?;
            let Some(event) = handle_streamed_reasoning_done(
                task_id,
                request_id,
                accumulator,
                event_type.as_str(),
                done_event,
            )?
            else {
                return Ok(StreamMessageResult::default());
            };

            Ok(StreamMessageResult {
                events: vec![Ok(event)],
                is_terminal: false,
            })
        }
        "response.function_call_arguments.delta" => {
            let delta_event: ResponsesFunctionCallArgumentsDeltaEvent =
                serde_json::from_value(payload)?;
            let function_call_id = streamed_function_call_id(
                delta_event.call_id.as_deref(),
                delta_event.item_id.as_deref(),
            )?;
            accumulator
                .function_calls_by_call_id
                .entry(function_call_id)
                .or_default()
                .arguments
                .push_str(&delta_event.delta);
            Ok(StreamMessageResult::default())
        }
        "response.function_call_arguments.done" => {
            let done_event: ResponsesFunctionCallArgumentsDoneEvent =
                serde_json::from_value(payload)?;
            let Some(function_call) = finalize_streamed_function_call(accumulator, done_event)?
            else {
                return Ok(StreamMessageResult::default());
            };
            Ok(StreamMessageResult {
                events: vec![Ok(add_messages_event(
                    task_id,
                    vec![tool_call_message(task_id, request_id, function_call)?],
                ))],
                is_terminal: false,
            })
        }
        "response.web_search_call.searching" => {
            let search_event: ResponsesWebSearchCallEvent = serde_json::from_value(payload)?;
            let Some(event) = handle_streamed_web_search_searching(
                task_id,
                request_id,
                accumulator,
                search_event,
            ) else {
                return Ok(StreamMessageResult::default());
            };

            Ok(StreamMessageResult {
                events: vec![Ok(event)],
                is_terminal: false,
            })
        }
        "response.output_item.done" => {
            let done_event: ResponsesOutputItemDoneEvent = serde_json::from_value(payload)?;
            if done_event.item.item_type == "reasoning" {
                record_reasoning_history_item(accumulator, &done_event.item);
                let reasoning_messages = reasoning_messages_from_output_item(
                    task_id,
                    request_id,
                    accumulator,
                    &done_event.item,
                );
                if reasoning_messages.is_empty() {
                    return Ok(StreamMessageResult::default());
                }
                for reasoning_text in reasoning_output_texts(&done_event.item) {
                    if let Some(reasoning_key) = backfill_reasoning_key(
                        done_event.item.id.as_deref(),
                        reasoning_text.key_kind,
                        reasoning_text.index,
                    ) {
                        if !reasoning_key_already_emitted(accumulator, reasoning_key.as_str()) {
                            accumulator.emitted_reasoning_keys.push(reasoning_key);
                        }
                    }
                }

                return Ok(StreamMessageResult {
                    events: vec![Ok(add_messages_event(task_id, reasoning_messages))],
                    is_terminal: false,
                });
            }
            if let Some(event) = handle_streamed_assistant_output_item_done(
                task_id,
                request_id,
                accumulator,
                &done_event.item,
            )? {
                return Ok(StreamMessageResult {
                    events: vec![Ok(event)],
                    is_terminal: false,
                });
            }
            if let Some(event) = handle_streamed_web_search_output_item_done(
                task_id,
                request_id,
                accumulator,
                &done_event.item,
            ) {
                return Ok(StreamMessageResult {
                    events: vec![Ok(event)],
                    is_terminal: false,
                });
            }
            let Some(function_call) =
                handle_streamed_output_item_done(accumulator, done_event.item)?
            else {
                return Ok(StreamMessageResult::default());
            };

            Ok(StreamMessageResult {
                events: vec![Ok(add_messages_event(
                    task_id,
                    vec![tool_call_message(task_id, request_id, function_call)?],
                ))],
                is_terminal: false,
            })
        }
        "response.completed" => {
            let completed_event: ResponsesCompletedEvent = serde_json::from_value(payload)?;
            Ok(StreamMessageResult {
                events: finalize_stream_state(
                    params,
                    std::mem::take(accumulator),
                    request_id,
                    Some(completed_event.response),
                )?,
                is_terminal: true,
            })
        }
        "response.failed" | "error" => {
            let message = streamed_error_message(&payload);
            let error = anyhow!("Local OpenAI Responses stream failed: {message}");
            Ok(StreamMessageResult {
                events: vec![
                    Ok(user_visible_error_event(
                        task_id,
                        request_id,
                        &error.to_string(),
                    )),
                    Ok(stream_finished_event(finished_reason_for_error(
                        &error,
                        params.model.to_string(),
                    ))),
                ],
                is_terminal: true,
            })
        }
        _ => Ok(StreamMessageResult::default()),
    }
}

/// Creates or appends to a streamed assistant text message based on a Responses text delta.
fn handle_streamed_text_delta(
    task_id: &TaskId,
    request_id: &str,
    accumulator: &mut StreamingResponsesAccumulator,
    delta_event: ResponsesTextDeltaEvent,
) -> anyhow::Result<Option<api::ResponseEvent>> {
    if delta_event.delta.is_empty() {
        return Ok(None);
    }

    if let Some(existing_message) = accumulator
        .text_messages_by_item_id
        .get_mut(&delta_event.item_id)
    {
        existing_message.text.push_str(&delta_event.delta);
        return Ok(Some(update_agent_output_text_event(
            task_id,
            request_id,
            &existing_message.message_id,
            existing_message.text.clone(),
        )));
    }

    let message_id = Uuid::new_v4().to_string();
    accumulator
        .emitted_text_item_ids
        .push(delta_event.item_id.clone());
    accumulator.text_messages_by_item_id.insert(
        delta_event.item_id,
        StreamingTextMessageState {
            message_id: message_id.clone(),
            text: delta_event.delta.clone(),
        },
    );

    Ok(Some(add_messages_event(
        task_id,
        vec![agent_output_message_with_id(
            message_id,
            task_id,
            request_id,
            delta_event.delta,
            vec![],
        )],
    )))
}

/// Emits or updates the UI state for a streamed web search entering the searching phase.
fn handle_streamed_web_search_searching(
    task_id: &TaskId,
    request_id: &str,
    accumulator: &mut StreamingResponsesAccumulator,
    search_event: ResponsesWebSearchCallEvent,
) -> Option<api::ResponseEvent> {
    if search_event.item_id.is_empty() {
        return None;
    }

    if let Some(existing_state) = accumulator
        .web_search_states_by_item_id
        .get(&search_event.item_id)
    {
        return Some(update_web_search_status_event(
            task_id,
            request_id,
            &existing_state.message_id,
            web_search_searching_status(None),
        ));
    }

    let message_id = Uuid::new_v4().to_string();
    accumulator.web_search_states_by_item_id.insert(
        search_event.item_id.clone(),
        StreamingWebSearchState {
            message_id: message_id.clone(),
        },
    );
    if !accumulator
        .emitted_web_search_item_ids
        .iter()
        .any(|existing_id| existing_id == &search_event.item_id)
    {
        accumulator
            .emitted_web_search_item_ids
            .push(search_event.item_id);
    }

    Some(add_messages_event(
        task_id,
        vec![web_search_message_with_id(
            message_id,
            task_id,
            request_id,
            web_search_searching_status(None),
        )],
    ))
}

/// Seeds reasoning stream state when the provider announces a new reasoning part.
fn handle_streamed_reasoning_part_added(
    event_type: &str,
    accumulator: &mut StreamingResponsesAccumulator,
    added_event: ResponsesReasoningPartAddedEvent,
) -> anyhow::Result<()> {
    if !reasoning_part_type_matches_event(event_type, added_event.part.item_type.as_str()) {
        return Ok(());
    }

    let reasoning_key = streamed_reasoning_key(
        event_type,
        added_event.item_id.as_str(),
        added_event.summary_index,
        added_event.content_index,
    )?;
    accumulator
        .reasoning_messages_by_key
        .entry(reasoning_key)
        .or_insert_with(new_streaming_reasoning_message_state);
    Ok(())
}

/// Creates or appends to a streamed reasoning message based on a Responses reasoning delta.
fn handle_streamed_reasoning_delta(
    task_id: &TaskId,
    request_id: &str,
    accumulator: &mut StreamingResponsesAccumulator,
    event_type: &str,
    delta_event: ResponsesReasoningTextDeltaEvent,
) -> anyhow::Result<Option<api::ResponseEvent>> {
    if delta_event.delta.is_empty() {
        return Ok(None);
    }

    let reasoning_key = streamed_reasoning_key(
        event_type,
        delta_event.item_id.as_str(),
        delta_event.summary_index,
        delta_event.content_index,
    )?;
    let (message_id, full_text) = {
        let state = accumulator
            .reasoning_messages_by_key
            .entry(reasoning_key.clone())
            .or_insert_with(new_streaming_reasoning_message_state);
        state.text.push_str(&delta_event.delta);
        (state.message_id.clone(), state.text.clone())
    };

    if reasoning_key_already_emitted(accumulator, reasoning_key.as_str()) {
        return Ok(Some(update_agent_reasoning_event(
            task_id,
            request_id,
            &message_id,
            full_text,
            None,
        )));
    }

    accumulator.emitted_reasoning_keys.push(reasoning_key);
    Ok(Some(add_messages_event(
        task_id,
        vec![reasoning_message_with_id(
            message_id, task_id, request_id, full_text, None,
        )],
    )))
}

/// Finalizes a streamed reasoning message once the provider marks the text complete.
fn handle_streamed_reasoning_done(
    task_id: &TaskId,
    request_id: &str,
    accumulator: &mut StreamingResponsesAccumulator,
    event_type: &str,
    done_event: ResponsesReasoningTextDoneEvent,
) -> anyhow::Result<Option<api::ResponseEvent>> {
    let reasoning_key = streamed_reasoning_key(
        event_type,
        done_event.item_id.as_str(),
        done_event.summary_index,
        done_event.content_index,
    )?;
    let resolved_text = completed_reasoning_text(&done_event);
    let (message_id, final_text, finished_duration) = {
        let state = accumulator
            .reasoning_messages_by_key
            .entry(reasoning_key.clone())
            .or_insert_with(new_streaming_reasoning_message_state);
        let final_text = resolved_text.unwrap_or_else(|| state.text.clone());
        if final_text.is_empty() {
            return Ok(None);
        }
        state.text = final_text.clone();
        (
            state.message_id.clone(),
            final_text,
            Some(state.started_at.elapsed()),
        )
    };

    if reasoning_key_already_emitted(accumulator, reasoning_key.as_str()) {
        return Ok(Some(update_agent_reasoning_event(
            task_id,
            request_id,
            &message_id,
            final_text,
            finished_duration,
        )));
    }

    accumulator.emitted_reasoning_keys.push(reasoning_key);
    Ok(Some(add_messages_event(
        task_id,
        vec![reasoning_message_with_id(
            message_id,
            task_id,
            request_id,
            final_text,
            finished_duration,
        )],
    )))
}

/// Finalizes a streamed function call and returns the parsed Warp tool call payload.
pub(super) fn finalize_streamed_function_call(
    accumulator: &mut StreamingResponsesAccumulator,
    done_event: ResponsesFunctionCallArgumentsDoneEvent,
) -> anyhow::Result<Option<ParsedFunctionCall>> {
    let function_call_id = reconcile_streamed_function_call_state_key(
        accumulator,
        done_event.call_id.as_deref(),
        done_event.item_id.as_deref(),
    )?;
    let state = accumulator
        .function_calls_by_call_id
        .entry(function_call_id.clone())
        .or_default();
    state.provider_call_id = done_event
        .call_id
        .clone()
        .or(state.provider_call_id.clone());
    state.output_item_id = done_event.item_id.clone().or(state.output_item_id.clone());
    state.name = done_event.name.clone().or(state.name.clone());
    let final_arguments = if done_event.arguments.is_empty() {
        state.arguments.clone()
    } else {
        done_event.arguments.clone()
    };
    state.arguments = final_arguments.clone();

    maybe_emit_streamed_function_call(accumulator, &function_call_id, false)
}

/// Uses `response.output_item.done` to enrich streamed function call metadata with authoritative IDs.
pub(super) fn handle_streamed_output_item_done(
    accumulator: &mut StreamingResponsesAccumulator,
    item: ResponsesOutputItem,
) -> anyhow::Result<Option<ParsedFunctionCall>> {
    if item.item_type != "function_call" {
        return Ok(None);
    }

    let function_call_id = reconcile_streamed_function_call_state_key(
        accumulator,
        item.call_id.as_deref(),
        item.id.as_deref(),
    )?;
    let state = accumulator
        .function_calls_by_call_id
        .entry(function_call_id.clone())
        .or_default();
    state.provider_call_id = item.call_id.clone().or(state.provider_call_id.clone());
    state.output_item_id = item.id.clone().or(state.output_item_id.clone());
    state.name = item.name.clone().or(state.name.clone());
    if state.arguments.is_empty() {
        state.arguments = item.arguments.clone().unwrap_or_default();
    }

    maybe_emit_streamed_function_call(accumulator, &function_call_id, true)
}

/// Emits a completed streamed function call once its metadata is sufficiently populated.
fn maybe_emit_streamed_function_call(
    accumulator: &mut StreamingResponsesAccumulator,
    function_call_id: &str,
    allow_item_id_fallback: bool,
) -> anyhow::Result<Option<ParsedFunctionCall>> {
    let state = accumulator
        .function_calls_by_call_id
        .get_mut(function_call_id)
        .ok_or_else(|| anyhow!("Missing streamed function call state for id {function_call_id}"))?;
    if state.emitted {
        return Ok(None);
    }

    let Some(name) = state.name.clone() else {
        log::debug!(
            "Deferring streamed function call emission until name is available for id {}",
            function_call_id
        );
        return Ok(None);
    };
    let Some(canonical_call_id) = state.provider_call_id.clone().or_else(|| {
        allow_item_id_fallback
            .then(|| state.output_item_id.clone())
            .flatten()
    }) else {
        log::debug!(
            "Deferring streamed function call emission until call_id metadata is available for id {}",
            function_call_id
        );
        return Ok(None);
    };
    let arguments = if state.arguments.is_empty() {
        json!({})
    } else {
        serde_json::from_str(&state.arguments)
            .context("Failed to parse streamed function call arguments")?
    };

    state.emitted = true;
    if !accumulator
        .emitted_function_call_ids
        .iter()
        .any(|existing_id| existing_id == function_call_id)
    {
        accumulator
            .emitted_function_call_ids
            .push(function_call_id.to_string());
    }

    let function_call = ParsedFunctionCall {
        name,
        call_id: canonical_call_id,
        arguments,
    };
    record_replayable_history_item(
        accumulator,
        format!("function_call:{function_call_id}"),
        function_call_history_item(&function_call),
    );

    Ok(Some(function_call))
}

/// Reconciles a streamed function call state key, promoting item_id-backed state to a real call_id when available.
fn reconcile_streamed_function_call_state_key(
    accumulator: &mut StreamingResponsesAccumulator,
    call_id: Option<&str>,
    item_id: Option<&str>,
) -> anyhow::Result<String> {
    let Some(resolved_key) = call_id.or(item_id).filter(|value| !value.is_empty()) else {
        return Err(anyhow!(
            "Streaming function call event did not include either call_id or item_id"
        ));
    };

    if let (Some(call_id), Some(item_id)) = (
        call_id.filter(|value| !value.is_empty()),
        item_id.filter(|value| !value.is_empty()),
    ) {
        if call_id != item_id {
            let existing_call_state = accumulator.function_calls_by_call_id.remove(item_id);
            let merged_state = merge_streaming_function_call_state(
                existing_call_state,
                accumulator.function_calls_by_call_id.remove(call_id),
            );
            if let Some(merged_state) = merged_state {
                accumulator
                    .function_calls_by_call_id
                    .insert(call_id.to_string(), merged_state);
            }
            return Ok(call_id.to_string());
        }
    }

    Ok(resolved_key.to_string())
}

/// Merges two streamed function call states, preferring the more authoritative populated fields.
fn merge_streaming_function_call_state(
    first: Option<StreamingFunctionCallState>,
    second: Option<StreamingFunctionCallState>,
) -> Option<StreamingFunctionCallState> {
    match (first, second) {
        (None, None) => None,
        (Some(state), None) | (None, Some(state)) => Some(state),
        (Some(first), Some(second)) => Some(StreamingFunctionCallState {
            output_item_id: second.output_item_id.or(first.output_item_id),
            provider_call_id: second.provider_call_id.or(first.provider_call_id),
            name: second.name.or(first.name),
            arguments: if second.arguments.is_empty() {
                first.arguments
            } else {
                second.arguments
            },
            emitted: first.emitted || second.emitted,
        }),
    }
}

/// Finalizes stream state, backfills any non-streamed outputs, and records conversation history.
pub(super) fn finalize_stream_state(
    params: &RequestParams,
    accumulator: StreamingResponsesAccumulator,
    request_id: &str,
    completed_response: Option<ResponsesApiResponse>,
) -> anyhow::Result<Vec<Event>> {
    let mut events = Vec::new();

    if let Some(response) = completed_response {
        let output = response.output;
        let history_items = if output.is_empty() {
            history_items_from_accumulator(&accumulator)?
        } else {
            match history_items_from_completed_output(output.clone(), &accumulator) {
                Ok(history_items) => {
                    if history_items.is_empty() && has_streamed_output(&accumulator) {
                        history_items_from_accumulator(&accumulator)?
                    } else {
                        history_items
                    }
                }
                Err(error) if has_streamed_output(&accumulator) => {
                    log::debug!(
                        "Falling back to streamed local Responses history because completed payload could not be parsed: {error:#}"
                    );
                    history_items_from_accumulator(&accumulator)?
                }
                Err(error) => return Err(error),
            }
        };
        if !history_items.is_empty() {
            let mut state_store = conversation_state_store().lock();
            let state = state_store.entry(params.conversation_id).or_default();
            state.items.extend(history_items);
        }

        let backfill_messages = build_backfill_messages(
            &accumulator,
            params.target_task_id.as_ref(),
            request_id,
            &output,
        )?;
        if let Some(task_id) = params.target_task_id.as_ref() {
            if !backfill_messages.is_empty() {
                events.push(Ok(add_messages_event(task_id, backfill_messages)));
            }
            events.extend(
                build_streamed_message_citation_updates(&accumulator, task_id, request_id, &output)
                    .into_iter()
                    .map(Ok),
            );
        }
    }

    events.push(Ok(stream_finished_event(
        api::response_event::stream_finished::Reason::Done(
            api::response_event::stream_finished::Done {},
        ),
    )));
    Ok(events)
}

/// Returns whether the accumulator already captured streamed text or tool calls.
fn has_streamed_output(accumulator: &StreamingResponsesAccumulator) -> bool {
    !accumulator.text_messages_by_item_id.is_empty()
        || !accumulator.emitted_reasoning_keys.is_empty()
        || !accumulator.emitted_function_call_ids.is_empty()
        || !accumulator.emitted_web_search_item_ids.is_empty()
        || !accumulator.replayable_history_item_keys_in_order.is_empty()
}

/// Reconstructs conversation history items from streamed deltas when the completed payload is empty.
fn history_items_from_accumulator(
    accumulator: &StreamingResponsesAccumulator,
) -> anyhow::Result<Vec<Value>> {
    let mut items = accumulator
        .replayable_history_item_keys_in_order
        .iter()
        .filter_map(|key| {
            accumulator
                .replayable_history_items_by_key
                .get(key)
                .cloned()
        })
        .collect::<Vec<_>>();

    for key in &accumulator.reasoning_history_item_keys_in_order {
        let Some(reasoning_item) = accumulator.reasoning_history_items_by_key.get(key) else {
            continue;
        };
        let replayable_key = key
            .strip_prefix("reasoning_history:")
            .map(|suffix| format!("reasoning:{suffix}"));
        if replayable_key.as_ref().is_some_and(|replayable_key| {
            accumulator
                .replayable_history_items_by_key
                .contains_key(replayable_key)
        }) {
            continue;
        }
        items.push(reasoning_item.clone());
    }

    for item_id in &accumulator.emitted_text_item_ids {
        if accumulator
            .replayable_history_items_by_key
            .contains_key(&format!("message:{item_id}"))
        {
            continue;
        }
        let Some(message_state) = accumulator.text_messages_by_item_id.get(item_id) else {
            continue;
        };
        if !message_state.text.is_empty() {
            items.push(assistant_output_item(&message_state.text));
        }
    }

    for call_id in &accumulator.emitted_function_call_ids {
        if accumulator
            .replayable_history_items_by_key
            .contains_key(&format!("function_call:{call_id}"))
        {
            continue;
        }
        let Some(function_call_state) = accumulator.function_calls_by_call_id.get(call_id) else {
            continue;
        };
        let Some(name) = function_call_state.name.clone() else {
            continue;
        };
        let arguments = if function_call_state.arguments.is_empty() {
            json!({})
        } else {
            serde_json::from_str(&function_call_state.arguments)
                .context("Failed to parse streamed function call arguments from accumulator")?
        };
        items.push(function_call_history_item(&ParsedFunctionCall {
            name,
            call_id: call_id.clone(),
            arguments,
        }));
    }

    Ok(items)
}

/// Converts completed Responses output items into replayable history while preserving reasoning context.
fn history_items_from_completed_output(
    output: Vec<ResponsesOutputItem>,
    accumulator: &StreamingResponsesAccumulator,
) -> anyhow::Result<Vec<Value>> {
    let mut history_items = Vec::new();
    let mut recorded_reasoning_keys = std::collections::HashSet::new();
    let citation_titles_by_url = citation_titles_by_url(&output);

    for item in output {
        match item.item_type.as_str() {
            "message" if item.role.as_deref() == Some("assistant") => {
                if let Some(history_item) = assistant_history_item_from_output_item(&item) {
                    history_items.push(history_item);
                }
            }
            "reasoning" => {
                if let Some(reasoning_item) = reasoning_history_item(&item) {
                    if let Some(key) = reasoning_history_item_key(&item) {
                        recorded_reasoning_keys.insert(key);
                    }
                    history_items.push(reasoning_item);
                }
            }
            "function_call" => {
                let name = item.name.context("Missing function call name")?;
                let call_id = item
                    .call_id
                    .or(item.id)
                    .unwrap_or_else(|| format!("call_{}", Uuid::new_v4().simple()));
                let arguments = item
                    .arguments
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()
                    .context("Failed to parse function call arguments")?
                    .unwrap_or_else(|| json!({}));
                history_items.push(function_call_history_item(&ParsedFunctionCall {
                    name,
                    call_id,
                    arguments,
                }));
            }
            "web_search_call" => {
                if let Some(history_item) =
                    web_search_history_item_from_output_item(&item, &citation_titles_by_url)
                {
                    history_items.push(history_item);
                }
            }
            _ => {}
        }
    }

    for key in &accumulator.reasoning_history_item_keys_in_order {
        if recorded_reasoning_keys.contains(key) {
            continue;
        }
        let Some(reasoning_item) = accumulator.reasoning_history_items_by_key.get(key) else {
            continue;
        };
        history_items.push(reasoning_item.clone());
    }

    Ok(history_items)
}

/// Builds fallback messages for any completed outputs that were not already streamed to the UI.
fn build_backfill_messages(
    accumulator: &StreamingResponsesAccumulator,
    task_id: Option<&TaskId>,
    request_id: &str,
    output: &[ResponsesOutputItem],
) -> anyhow::Result<Vec<api::Message>> {
    let Some(task_id) = task_id else {
        return Ok(Vec::new());
    };

    let mut messages = Vec::new();
    let citation_titles_by_url = citation_titles_by_url(output);
    for item in output {
        match item.item_type.as_str() {
            "message" if item.role.as_deref() == Some("assistant") => {
                let Some(item_id) = item.id.as_ref() else {
                    continue;
                };
                if accumulator
                    .finalized_text_item_ids
                    .iter()
                    .any(|existing_id| existing_id == item_id)
                {
                    continue;
                }
                if accumulator
                    .emitted_text_item_ids
                    .iter()
                    .any(|existing_id| existing_id == item_id)
                {
                    continue;
                }

                let text = assistant_message_text(item);
                let citations = citations_from_output_item(item);
                if !text.is_empty() {
                    messages.push(agent_output_message(task_id, request_id, text, citations));
                }
            }
            "reasoning" => {
                messages.extend(reasoning_messages_from_output_item(
                    task_id,
                    request_id,
                    accumulator,
                    &item,
                ));
            }
            "function_call" => {
                let Some(call_id) = item.call_id.as_ref() else {
                    continue;
                };
                if accumulator
                    .emitted_function_call_ids
                    .iter()
                    .any(|existing_id| existing_id == call_id)
                {
                    continue;
                }

                let Some(name) = item.name.clone() else {
                    continue;
                };
                let arguments = item.arguments.clone().unwrap_or_else(|| "{}".to_string());
                messages.push(tool_call_message(
                    task_id,
                    request_id,
                    ParsedFunctionCall {
                        name,
                        call_id: call_id.clone(),
                        arguments: serde_json::from_str(&arguments)
                            .context("Failed to parse backfilled function call arguments")?,
                    },
                )?);
            }
            "web_search_call" => {
                let Some(item_id) = item.id.as_ref() else {
                    continue;
                };
                if accumulator
                    .emitted_web_search_item_ids
                    .iter()
                    .any(|existing_id| existing_id == item_id)
                {
                    continue;
                }
                if let Some(message) = web_search_message_from_output_item(
                    task_id,
                    request_id,
                    item,
                    &citation_titles_by_url,
                ) {
                    messages.push(message);
                }
            }
            _ => {}
        }
    }

    Ok(messages)
}

/// Builds update events that attach web citations to assistant messages already streamed via text deltas.
fn build_streamed_message_citation_updates(
    accumulator: &StreamingResponsesAccumulator,
    task_id: &TaskId,
    request_id: &str,
    output: &[ResponsesOutputItem],
) -> Vec<api::ResponseEvent> {
    output
        .iter()
        .filter(|item| item.item_type == "message" && item.role.as_deref() == Some("assistant"))
        .filter(|item| {
            item.id.as_ref().is_some_and(|item_id| {
                !accumulator
                    .finalized_text_item_ids
                    .iter()
                    .any(|existing_id| existing_id == item_id)
            })
        })
        .filter_map(|item| {
            let item_id = item.id.as_ref()?;
            let message_state = accumulator.text_messages_by_item_id.get(item_id)?;
            let citations = citations_from_output_item(item);
            if citations.is_empty() {
                return None;
            }

            Some(update_agent_output_citations_event(
                task_id,
                request_id,
                &message_state.message_id,
                message_state.text.clone(),
                citations,
            ))
        })
        .collect()
}

/// Uses `response.output_item.done` to finalize an assistant message without depending on `response.completed`.
fn handle_streamed_assistant_output_item_done(
    task_id: &TaskId,
    request_id: &str,
    accumulator: &mut StreamingResponsesAccumulator,
    item: &ResponsesOutputItem,
) -> anyhow::Result<Option<api::ResponseEvent>> {
    if item.item_type != "message" || item.role.as_deref() != Some("assistant") {
        return Ok(None);
    }

    let Some(item_id) = item.id.as_ref().filter(|item_id| !item_id.is_empty()) else {
        return Ok(None);
    };

    let full_text = assistant_message_text(item);
    let citations = citations_from_output_item(item);
    let had_existing_state = accumulator.text_messages_by_item_id.contains_key(item_id);

    let message_id =
        if let Some(existing_state) = accumulator.text_messages_by_item_id.get_mut(item_id) {
            if !full_text.is_empty() {
                existing_state.text = full_text.clone();
            }
            existing_state.message_id.clone()
        } else {
            if full_text.is_empty() {
                return Ok(None);
            }
            let message_id = Uuid::new_v4().to_string();
            accumulator.text_messages_by_item_id.insert(
                item_id.clone(),
                StreamingTextMessageState {
                    message_id: message_id.clone(),
                    text: full_text.clone(),
                },
            );
            if !accumulator
                .emitted_text_item_ids
                .iter()
                .any(|existing_id| existing_id == item_id)
            {
                accumulator.emitted_text_item_ids.push(item_id.clone());
            }
            message_id
        };

    if !accumulator
        .finalized_text_item_ids
        .iter()
        .any(|existing_id| existing_id == item_id)
    {
        accumulator.finalized_text_item_ids.push(item_id.clone());
    }
    if let Some(history_item) = assistant_history_item_from_output_item(item) {
        record_replayable_history_item(accumulator, format!("message:{item_id}"), history_item);
    }

    if had_existing_state {
        return Ok(Some(update_agent_output_message_event(
            task_id,
            request_id,
            &message_id,
            accumulator
                .text_messages_by_item_id
                .get(item_id)
                .map(|state| state.text.clone())
                .unwrap_or(full_text),
            citations,
        )));
    }

    let message_state = accumulator
        .text_messages_by_item_id
        .get(item_id)
        .ok_or_else(|| anyhow!("Missing streamed assistant message state for item {item_id}"))?;
    Ok(Some(add_messages_event(
        task_id,
        vec![agent_output_message_with_id(
            message_state.message_id.clone(),
            task_id,
            request_id,
            message_state.text.clone(),
            citations,
        )],
    )))
}

/// Uses `response.output_item.done` to finalize a web-search item emitted by the streamed Responses API.
fn handle_streamed_web_search_output_item_done(
    task_id: &TaskId,
    request_id: &str,
    accumulator: &mut StreamingResponsesAccumulator,
    item: &ResponsesOutputItem,
) -> Option<api::ResponseEvent> {
    if item.item_type != "web_search_call" {
        return None;
    }

    let Some(item_id) = item.id.as_ref().filter(|item_id| !item_id.is_empty()) else {
        return None;
    };
    let status = web_search_status_from_output_item(item, &HashMap::new())?;
    if let Some(history_item) = web_search_history_item_from_output_item(item, &HashMap::new()) {
        record_replayable_history_item(
            accumulator,
            format!("web_search_call:{item_id}"),
            history_item,
        );
    }

    if let Some(existing_state) = accumulator.web_search_states_by_item_id.get(item_id) {
        if !accumulator
            .emitted_web_search_item_ids
            .iter()
            .any(|existing_id| existing_id == item_id)
        {
            accumulator
                .emitted_web_search_item_ids
                .push(item_id.clone());
        }
        return Some(update_web_search_status_event(
            task_id,
            request_id,
            &existing_state.message_id,
            status,
        ));
    }

    let message_id = Uuid::new_v4().to_string();
    accumulator.web_search_states_by_item_id.insert(
        item_id.clone(),
        StreamingWebSearchState {
            message_id: message_id.clone(),
        },
    );
    if !accumulator
        .emitted_web_search_item_ids
        .iter()
        .any(|existing_id| existing_id == item_id)
    {
        accumulator
            .emitted_web_search_item_ids
            .push(item_id.clone());
    }

    Some(add_messages_event(
        task_id,
        vec![web_search_message_with_id(
            message_id, task_id, request_id, status,
        )],
    ))
}

/// Extracts the user-visible assistant text from a Responses assistant message output item.
fn assistant_message_text(item: &ResponsesOutputItem) -> String {
    item.content
        .iter()
        .filter(|content| matches!(content.item_type.as_str(), "output_text" | "text"))
        .filter_map(|content| content.text.clone())
        .collect::<Vec<_>>()
        .join("")
}

/// Converts a completed assistant output item into a replayable Responses history item.
fn assistant_history_item_from_output_item(item: &ResponsesOutputItem) -> Option<Value> {
    let text = assistant_message_text(item);
    if text.is_empty() {
        return None;
    }

    let annotations = item
        .content
        .iter()
        .flat_map(|content| {
            content
                .annotations
                .iter()
                .filter_map(output_text_annotation_history_value)
        })
        .collect::<Vec<_>>();

    Some(assistant_output_item_with_annotations(&text, annotations))
}

/// Extracts deduplicated webpage citations from a Responses assistant message output item.
fn citations_from_output_item(item: &ResponsesOutputItem) -> Vec<api::Citation> {
    let mut seen_urls = HashSet::new();
    let mut citations = Vec::new();

    for content in &item.content {
        for annotation in &content.annotations {
            let Some((url, _title)) = output_text_annotation_as_web_page(annotation) else {
                continue;
            };
            if !seen_urls.insert(url.clone()) {
                continue;
            }
            citations.push(api::Citation {
                document_id: url,
                document_type: api::DocumentType::WebPage.into(),
            });
        }
    }

    citations
}

/// Builds a URL-to-title map from all assistant-message citations in the completed Responses output.
fn citation_titles_by_url(output: &[ResponsesOutputItem]) -> HashMap<String, String> {
    let mut titles = HashMap::new();
    for item in output {
        if item.item_type != "message" || item.role.as_deref() != Some("assistant") {
            continue;
        }
        for content in &item.content {
            for annotation in &content.annotations {
                let Some((url, title)) = output_text_annotation_as_web_page(annotation) else {
                    continue;
                };
                if !url.is_empty() {
                    titles.entry(url).or_insert(title);
                }
            }
        }
    }
    titles
}

/// Converts a completed `web_search_call` item into Warp's status message.
fn web_search_message_from_output_item(
    task_id: &TaskId,
    request_id: &str,
    item: &ResponsesOutputItem,
    citation_titles_by_url: &HashMap<String, String>,
) -> Option<api::Message> {
    if item.item_type != "web_search_call" {
        return None;
    }

    let status = web_search_status_from_output_item(item, citation_titles_by_url)?;

    Some(web_search_message_with_id(
        Uuid::new_v4().to_string(),
        task_id,
        request_id,
        status,
    ))
}

/// Converts a completed web-search output item into a replayable Responses history item.
fn web_search_history_item_from_output_item(
    item: &ResponsesOutputItem,
    citation_titles_by_url: &HashMap<String, String>,
) -> Option<Value> {
    if item.item_type != "web_search_call" {
        return None;
    }

    let query = web_search_query_from_action(item.action.as_ref());
    let status = match item.status.as_deref() {
        Some("in_progress") | Some("searching") => "searching",
        Some("failed") => "failed",
        Some("completed") | Some(_) | None => "completed",
    };
    let pages = web_search_pages_from_item(item, citation_titles_by_url);

    Some(web_search_call_history_item(
        query.as_deref(),
        status,
        &pages,
    ))
}

/// Extracts a search query from the minimal web-search action payload.
fn web_search_query_from_action(action: Option<&ResponsesWebSearchAction>) -> Option<String> {
    action
        .filter(|action| action.action_type == "search")
        .and_then(|action| action.query.as_ref())
        .map(|query| query.trim().to_string())
        .filter(|query| !query.is_empty())
}

/// Converts a Responses web-search output item into Warp's web-search status payload.
fn web_search_status_from_output_item(
    item: &ResponsesOutputItem,
    citation_titles_by_url: &HashMap<String, String>,
) -> Option<api::message::web_search::status::Type> {
    let query = web_search_query_from_action(item.action.as_ref()).unwrap_or_default();
    let pages = web_search_pages_from_item(item, citation_titles_by_url);
    match item.status.as_deref() {
        Some("in_progress") | Some("searching") => Some(web_search_searching_status(
            (!query.is_empty()).then_some(query),
        )),
        Some("failed") => Some(api::message::web_search::status::Type::Error(())),
        Some("completed") | Some(_) | None => {
            Some(api::message::web_search::status::Type::Success(
                api::message::web_search::status::Success {
                    query,
                    pages: pages
                        .into_iter()
                        .map(|(url, title)| {
                            api::message::web_search::status::success::SearchedPage { url, title }
                        })
                        .collect(),
                },
            ))
        }
    }
}

/// Builds Warp's searching status payload for a web search.
fn web_search_searching_status(query: Option<String>) -> api::message::web_search::status::Type {
    api::message::web_search::status::Type::Searching(api::message::web_search::status::Searching {
        query: query.unwrap_or_default(),
    })
}

/// Collects the searched pages associated with a completed web-search call.
fn web_search_pages_from_item(
    item: &ResponsesOutputItem,
    citation_titles_by_url: &HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut seen_urls = HashSet::new();
    let mut pages = Vec::new();

    if let Some(action) = item.action.as_ref() {
        for source in &action.sources {
            if source.source_type != "url" {
                continue;
            }
            let Some(url) = source.url.clone().filter(|url| !url.is_empty()) else {
                continue;
            };
            if !seen_urls.insert(url.clone()) {
                continue;
            }
            pages.push((
                url.clone(),
                citation_titles_by_url
                    .get(&url)
                    .cloned()
                    .unwrap_or_default(),
            ));
        }
    }

    pages
}

/// Extracts a URL citation from an `output_text` annotation in either flat or nested form.
fn output_text_annotation_as_web_page(
    annotation: &ResponsesOutputTextAnnotation,
) -> Option<(String, String)> {
    let annotation_type = if annotation.item_type.is_empty() {
        annotation
            .url_citation
            .as_ref()
            .map(|nested| nested.item_type.as_str())
            .unwrap_or_default()
    } else {
        annotation.item_type.as_str()
    };
    if annotation_type != "url_citation" {
        return None;
    }

    let url = annotation.url.clone().or_else(|| {
        annotation
            .url_citation
            .as_ref()
            .and_then(|nested| nested.url.clone())
    })?;
    let title = annotation
        .title
        .clone()
        .or_else(|| {
            annotation
                .url_citation
                .as_ref()
                .and_then(|nested| nested.title.clone())
        })
        .unwrap_or_default();

    Some((url, title))
}

/// Converts a Responses output-text annotation back into a replayable raw annotation payload.
fn output_text_annotation_history_value(
    annotation: &ResponsesOutputTextAnnotation,
) -> Option<Value> {
    let (url, title) = output_text_annotation_as_web_page(annotation)?;
    let mut value = serde_json::Map::new();
    value.insert(
        "type".to_string(),
        Value::String("url_citation".to_string()),
    );
    value.insert("url".to_string(), Value::String(url));
    if !title.is_empty() {
        value.insert("title".to_string(), Value::String(title));
    }
    Some(Value::Object(value))
}

/// Builds reasoning messages from a completed reasoning output item that was not streamed incrementally.
fn reasoning_messages_from_output_item(
    task_id: &TaskId,
    request_id: &str,
    accumulator: &StreamingResponsesAccumulator,
    item: &ResponsesOutputItem,
) -> Vec<api::Message> {
    let fallback_duration = Some(accumulator.stream_started_at.elapsed());
    reasoning_output_texts(item)
        .into_iter()
        .filter_map(|reasoning_text| {
            let reasoning_key = backfill_reasoning_key(
                item.id.as_deref(),
                reasoning_text.key_kind,
                reasoning_text.index,
            );
            if reasoning_key
                .as_deref()
                .is_some_and(|key| reasoning_key_already_emitted(accumulator, key))
            {
                return None;
            }

            Some(reasoning_message_with_id(
                Uuid::new_v4().to_string(),
                task_id,
                request_id,
                reasoning_text.text,
                fallback_duration,
            ))
        })
        .collect()
}

/// Records a replayable history item in stream order so empty `response.completed.output` can still be replayed faithfully.
fn record_replayable_history_item(
    accumulator: &mut StreamingResponsesAccumulator,
    key: String,
    item: Value,
) {
    if !accumulator
        .replayable_history_items_by_key
        .contains_key(&key)
    {
        accumulator
            .replayable_history_item_keys_in_order
            .push(key.clone());
    }
    accumulator
        .replayable_history_items_by_key
        .insert(key, item);
}

/// Records a replayable reasoning history item from streaming metadata so stateless follow-ups keep encrypted context.
fn record_reasoning_history_item(
    accumulator: &mut StreamingResponsesAccumulator,
    item: &ResponsesOutputItem,
) {
    let Some(reasoning_item) = reasoning_history_item(item) else {
        return;
    };
    let Some(key) = reasoning_history_item_key(item) else {
        return;
    };

    if !accumulator
        .reasoning_history_items_by_key
        .contains_key(&key)
    {
        accumulator
            .reasoning_history_item_keys_in_order
            .push(key.clone());
    }
    accumulator
        .reasoning_history_items_by_key
        .insert(key, reasoning_item);

    if let Some(item_id) = item.id.as_ref().filter(|item_id| !item_id.is_empty()) {
        if let Some(reasoning_item) = reasoning_history_item(item) {
            record_replayable_history_item(
                accumulator,
                format!("reasoning:{item_id}"),
                reasoning_item,
            );
        }
    }
}

/// Extracts the reasoning texts Warp should render from a Responses reasoning output item.
fn reasoning_output_texts(item: &ResponsesOutputItem) -> Vec<ReasoningOutputText> {
    let summary_texts = item
        .summary
        .iter()
        .enumerate()
        .filter_map(|(index, part)| {
            normalize_reasoning_output_text(part.item_type.as_str(), part.text.as_deref()).map(
                |text| ReasoningOutputText {
                    key_kind: "summary",
                    index,
                    text,
                },
            )
        })
        .collect::<Vec<_>>();
    if !summary_texts.is_empty() {
        return summary_texts;
    }

    item.content
        .iter()
        .enumerate()
        .filter_map(|(index, part)| {
            normalize_reasoning_output_text(part.item_type.as_str(), part.text.as_deref()).map(
                |text| ReasoningOutputText {
                    key_kind: "content",
                    index,
                    text,
                },
            )
        })
        .collect()
}

/// Normalizes a reasoning text fragment if the Responses content part is one Warp can display.
fn normalize_reasoning_output_text(part_type: &str, text: Option<&str>) -> Option<String> {
    matches!(part_type, "summary_text" | "reasoning_text")
        .then(|| text.unwrap_or_default().trim().to_string())
        .filter(|text| !text.is_empty())
}

/// Returns the stable key used to dedupe backfilled reasoning messages against streamed ones.
fn backfill_reasoning_key(item_id: Option<&str>, key_kind: &str, index: usize) -> Option<String> {
    item_id.map(|item_id| format!("reasoning:{key_kind}:{item_id}:{index}"))
}

/// Returns the stable key used to store replayable reasoning history items in the stream accumulator.
fn reasoning_history_item_key(item: &ResponsesOutputItem) -> Option<String> {
    item.id
        .as_ref()
        .filter(|item_id| !item_id.is_empty())
        .map(|item_id| format!("reasoning_history:{item_id}"))
        .or_else(|| {
            item.encrypted_content
                .as_ref()
                .filter(|encrypted_content| !encrypted_content.is_empty())
                .map(|encrypted_content| format!("reasoning_history:{encrypted_content}"))
        })
}

/// Returns whether a reasoning part type matches the corresponding streamed event family.
fn reasoning_part_type_matches_event(event_type: &str, part_type: &str) -> bool {
    match event_type {
        "response.reasoning_summary_part.added" => part_type == "summary_text",
        "response.reasoning_text_part.added" => part_type == "reasoning_text",
        _ => false,
    }
}

/// Returns the completed reasoning text from a `*.done` event if the payload carries one explicitly.
fn completed_reasoning_text(done_event: &ResponsesReasoningTextDoneEvent) -> Option<String> {
    if !done_event.text.trim().is_empty() {
        return Some(done_event.text.trim().to_string());
    }

    done_event.part.as_ref().and_then(|part| {
        normalize_reasoning_output_text(part.item_type.as_str(), part.text.as_deref())
    })
}

/// Builds the stable accumulator key for a streamed reasoning fragment.
fn streamed_reasoning_key(
    event_type: &str,
    item_id: &str,
    summary_index: Option<usize>,
    content_index: Option<usize>,
) -> anyhow::Result<String> {
    if item_id.is_empty() {
        return Err(anyhow!(
            "Streaming reasoning event did not include an item_id"
        ));
    }

    match event_type {
        "response.reasoning_summary_part.added"
        | "response.reasoning_summary_text.delta"
        | "response.reasoning_summary_text.done" => Ok(format!(
            "reasoning:summary:{item_id}:{}",
            summary_index.unwrap_or(0)
        )),
        "response.reasoning_text_part.added"
        | "response.reasoning_text.delta"
        | "response.reasoning_text.done" => Ok(format!(
            "reasoning:content:{item_id}:{}",
            content_index.unwrap_or(0)
        )),
        _ => Err(anyhow!(
            "Unsupported reasoning stream event type: {event_type}"
        )),
    }
}

/// Returns whether the given reasoning key has already produced a client-visible message.
fn reasoning_key_already_emitted(
    accumulator: &StreamingResponsesAccumulator,
    reasoning_key: &str,
) -> bool {
    accumulator
        .emitted_reasoning_keys
        .iter()
        .any(|existing_key| existing_key == reasoning_key)
}

/// Creates a new streamed reasoning state with a stable message ID and start timestamp.
fn new_streaming_reasoning_message_state() -> StreamingReasoningMessageState {
    StreamingReasoningMessageState {
        message_id: Uuid::new_v4().to_string(),
        text: String::new(),
        started_at: std::time::Instant::now(),
    }
}

/// Resolves the canonical event type for a streamed SSE message.
fn streamed_event_type(event_name: &str, payload: &Value) -> String {
    if !event_name.is_empty() {
        return event_name.to_string();
    }

    payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Extracts the most useful error message from a streamed Responses failure payload.
fn streamed_error_message(payload: &Value) -> String {
    if let Some(message) = payload.get("message").and_then(Value::as_str) {
        return message.to_string();
    }

    if let Some(message) = payload
        .get("error")
        .and_then(Value::as_object)
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
    {
        return message.to_string();
    }

    payload.to_string()
}

/// Resolves the function call identifier from a streamed Responses event.
fn streamed_function_call_id(
    call_id: Option<&str>,
    item_id: Option<&str>,
) -> anyhow::Result<String> {
    if let Some(call_id) = call_id.filter(|value| !value.is_empty()) {
        return Ok(call_id.to_string());
    }

    if let Some(item_id) = item_id.filter(|value| !value.is_empty()) {
        log::debug!("Streaming Responses event omitted call_id, falling back to item_id");
        return Ok(item_id.to_string());
    }

    Err(anyhow!(
        "Streaming function call event did not include either call_id or item_id"
    ))
}

/// Converts a parsed Responses function call into a Warp tool call message.
fn tool_call_message(
    task_id: &TaskId,
    request_id: &str,
    function_call: ParsedFunctionCall,
) -> anyhow::Result<api::Message> {
    Ok(api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_string(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: function_call.call_id,
            tool: Some(parse_tool_call(
                function_call.name.as_str(),
                function_call.arguments,
            )?),
        })),
        request_id: request_id.to_string(),
        timestamp: None,
    })
}

/// Converts assistant text into a Warp agent output message.
fn agent_output_message(
    task_id: &TaskId,
    request_id: &str,
    text: String,
    citations: Vec<api::Citation>,
) -> api::Message {
    agent_output_message_with_id(
        Uuid::new_v4().to_string(),
        task_id,
        request_id,
        text,
        citations,
    )
}

/// Converts a web-search status into a Warp task message with a stable message ID.
fn web_search_message_with_id(
    message_id: String,
    task_id: &TaskId,
    request_id: &str,
    status: api::message::web_search::status::Type,
) -> api::Message {
    api::Message {
        id: message_id,
        task_id: task_id.to_string(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::WebSearch(api::message::WebSearch {
            status: Some(api::message::web_search::Status {
                r#type: Some(status),
            }),
        })),
        request_id: request_id.to_string(),
        timestamp: None,
    }
}

/// Converts reasoning text into a Warp agent reasoning message with a stable message ID.
fn reasoning_message_with_id(
    message_id: String,
    task_id: &TaskId,
    request_id: &str,
    text: String,
    finished_duration: Option<Duration>,
) -> api::Message {
    api::Message {
        id: message_id,
        task_id: task_id.to_string(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::AgentReasoning(
            api::message::AgentReasoning {
                reasoning: text,
                finished_duration: finished_duration.map(duration_to_proto),
            },
        )),
        request_id: request_id.to_string(),
        timestamp: None,
    }
}

/// Converts assistant text into a Warp agent output message with a stable message ID.
pub(super) fn agent_output_message_with_id(
    message_id: String,
    task_id: &TaskId,
    request_id: &str,
    text: String,
    citations: Vec<api::Citation>,
) -> api::Message {
    api::Message {
        id: message_id,
        task_id: task_id.to_string(),
        server_message_data: String::new(),
        citations,
        message: Some(api::message::Message::AgentOutput(
            api::message::AgentOutput { text },
        )),
        request_id: request_id.to_string(),
        timestamp: None,
    }
}

/// Builds an update client action that replaces an existing streamed assistant text message.
pub(super) fn update_agent_output_text_event(
    task_id: &TaskId,
    request_id: &str,
    message_id: &str,
    full_text: String,
) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::UpdateTaskMessage(
                        api::client_action::UpdateTaskMessage {
                            task_id: task_id.to_string(),
                            message: Some(agent_output_message_with_id(
                                message_id.to_string(),
                                task_id,
                                request_id,
                                full_text,
                                vec![],
                            )),
                            mask: Some(prost_types::FieldMask {
                                paths: vec!["agent_output.text".to_string()],
                            }),
                        },
                    )),
                }],
            },
        )),
    }
}

/// Builds an update client action that replaces assistant text and citations together.
fn update_agent_output_message_event(
    task_id: &TaskId,
    request_id: &str,
    message_id: &str,
    full_text: String,
    citations: Vec<api::Citation>,
) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::UpdateTaskMessage(
                        api::client_action::UpdateTaskMessage {
                            task_id: task_id.to_string(),
                            message: Some(agent_output_message_with_id(
                                message_id.to_string(),
                                task_id,
                                request_id,
                                full_text,
                                citations,
                            )),
                            mask: Some(prost_types::FieldMask {
                                paths: vec![
                                    "agent_output.text".to_string(),
                                    "citations".to_string(),
                                ],
                            }),
                        },
                    )),
                }],
            },
        )),
    }
}

/// Builds an update client action that attaches citations to an existing assistant message.
fn update_agent_output_citations_event(
    task_id: &TaskId,
    request_id: &str,
    message_id: &str,
    full_text: String,
    citations: Vec<api::Citation>,
) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::UpdateTaskMessage(
                        api::client_action::UpdateTaskMessage {
                            task_id: task_id.to_string(),
                            message: Some(agent_output_message_with_id(
                                message_id.to_string(),
                                task_id,
                                request_id,
                                full_text,
                                citations,
                            )),
                            mask: Some(prost_types::FieldMask {
                                paths: vec!["citations".to_string()],
                            }),
                        },
                    )),
                }],
            },
        )),
    }
}

/// Builds an update client action that replaces the status of an existing streamed web-search message.
fn update_web_search_status_event(
    task_id: &TaskId,
    request_id: &str,
    message_id: &str,
    status: api::message::web_search::status::Type,
) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::UpdateTaskMessage(
                        api::client_action::UpdateTaskMessage {
                            task_id: task_id.to_string(),
                            message: Some(web_search_message_with_id(
                                message_id.to_string(),
                                task_id,
                                request_id,
                                status,
                            )),
                            mask: Some(prost_types::FieldMask {
                                paths: vec!["web_search.status".to_string()],
                            }),
                        },
                    )),
                }],
            },
        )),
    }
}

/// Builds an update client action for a streamed reasoning message, optionally finalizing its duration.
fn update_agent_reasoning_event(
    task_id: &TaskId,
    request_id: &str,
    message_id: &str,
    full_text: String,
    finished_duration: Option<Duration>,
) -> api::ResponseEvent {
    let mut mask_paths = vec!["agent_reasoning.reasoning".to_string()];
    if finished_duration.is_some() {
        mask_paths.push("agent_reasoning.finished_duration".to_string());
    }

    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: vec![api::ClientAction {
                    action: Some(api::client_action::Action::UpdateTaskMessage(
                        api::client_action::UpdateTaskMessage {
                            task_id: task_id.to_string(),
                            message: Some(reasoning_message_with_id(
                                message_id.to_string(),
                                task_id,
                                request_id,
                                full_text,
                                finished_duration,
                            )),
                            mask: Some(prost_types::FieldMask { paths: mask_paths }),
                        },
                    )),
                }],
            },
        )),
    }
}

/// Converts a std Duration into the protobuf Duration used by task message updates.
fn duration_to_proto(duration: Duration) -> prost_types::Duration {
    prost_types::Duration {
        seconds: i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
        nanos: i32::try_from(duration.subsec_nanos()).unwrap_or(i32::MAX),
    }
}

/// Describes one reasoning text fragment extracted from a Responses reasoning output item.
struct ReasoningOutputText {
    key_kind: &'static str,
    index: usize,
    text: String,
}
