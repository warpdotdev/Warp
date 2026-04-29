use std::{collections::BTreeMap, time::Duration};

use anyhow::{Context, Result, anyhow};
use reqwest_eventsource::{EventSource, RequestBuilderExt};
use serde::{Deserialize, Deserializer, Serialize};
use url::Url;

use crate::{
    config::UpstreamConfig,
    conversation::transcript::{TranscriptMessage, TranscriptRole, TranscriptToolCall},
    protocol::response_builder::UsageTotals,
};

#[derive(Clone, Debug, Serialize)]
pub(crate) struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAiToolDeclaration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<OpenAiToolCall>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct OpenAiToolCall {
    pub(crate) id: String,
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) function: OpenAiToolCallFunction,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct OpenAiToolCallFunction {
    pub(crate) name: String,
    pub(crate) arguments: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct OpenAiToolDeclaration {
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) function: OpenAiFunction,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct OpenAiFunction {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StreamItem {
    Chunk(ParsedStreamChunk),
    Usage(UsageTotals),
    Done,
    Empty,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParsedStreamChunk {
    pub(crate) content: String,
    pub(crate) tool_call_deltas: Vec<OpenAiToolCallDelta>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OpenAiToolCallDelta {
    pub(crate) index: usize,
    pub(crate) id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) arguments: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompletionOutput {
    pub(crate) content: String,
    pub(crate) tool_calls: Vec<OpenAiToolCall>,
    pub(crate) usage: Option<UsageTotals>,
}

#[derive(Debug, Default)]
pub(crate) struct OpenAiToolCallAccumulator {
    calls: BTreeMap<usize, PartialToolCall>,
}

#[derive(Debug, Default)]
struct PartialToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

impl OpenAiChatRequest {
    pub(crate) fn new(
        model: String,
        messages: &[TranscriptMessage],
        system_message: String,
        stream: bool,
        tools: Vec<OpenAiToolDeclaration>,
    ) -> Self {
        let mut openai_messages = vec![OpenAiMessage {
            role: "system".to_string(),
            content: Some(system_message),
            tool_call_id: None,
            tool_calls: vec![],
        }];
        openai_messages.extend(messages.iter().map(OpenAiMessage::from_transcript));
        let tool_choice = (!tools.is_empty()).then_some("auto");

        Self {
            model,
            messages: openai_messages,
            stream,
            tools,
            tool_choice,
        }
    }
}

impl OpenAiMessage {
    fn from_transcript(message: &TranscriptMessage) -> Self {
        let tool_calls = message
            .tool_calls
            .iter()
            .map(OpenAiToolCall::from_transcript)
            .collect::<Vec<_>>();
        let content = match message.role {
            TranscriptRole::Assistant if !tool_calls.is_empty() && message.content.is_empty() => {
                None
            }
            _ => Some(message.content.clone()),
        };

        Self {
            role: message.role.as_openai_role().to_string(),
            content,
            tool_call_id: message.tool_call_id.clone(),
            tool_calls,
        }
    }
}

impl OpenAiToolCall {
    fn from_transcript(tool_call: &TranscriptToolCall) -> Self {
        Self {
            id: tool_call.id.clone(),
            kind: "function".to_string(),
            function: OpenAiToolCallFunction {
                name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
            },
        }
    }
}

impl OpenAiToolCallAccumulator {
    pub(crate) fn push_deltas(&mut self, deltas: Vec<OpenAiToolCallDelta>) {
        for delta in deltas {
            let entry = self.calls.entry(delta.index).or_default();
            if let Some(id) = delta.id {
                entry.id = Some(id);
            }
            if let Some(name) = delta.name {
                entry.name = Some(name);
            }
            entry.arguments.push_str(&delta.arguments);
        }
    }

    pub(crate) fn finish(self) -> Result<Vec<OpenAiToolCall>> {
        self.calls
            .into_iter()
            .map(|(index, call)| {
                let name = call
                    .name
                    .ok_or_else(|| anyhow!("streamed tool call at index {index} had no name"))?;
                Ok(OpenAiToolCall {
                    id: call.id.unwrap_or_else(|| format!("call_{index}")),
                    kind: "function".to_string(),
                    function: OpenAiToolCallFunction {
                        name,
                        arguments: call.arguments,
                    },
                })
            })
            .collect()
    }
}

pub(crate) fn stream_chat_completion(
    client: &reqwest::Client,
    upstream: &UpstreamConfig,
    request: OpenAiChatRequest,
) -> Result<EventSource> {
    let request = request_builder(client, upstream, request)?;
    request
        .eventsource()
        .map_err(|err| anyhow!("failed to open upstream eventsource: {err}"))
}

pub(crate) async fn complete_chat_completion(
    client: &reqwest::Client,
    upstream: &UpstreamConfig,
    request: OpenAiChatRequest,
) -> Result<CompletionOutput> {
    let response = request_builder(client, upstream, request)?
        .send()
        .await
        .context("failed to send upstream chat completion request")?;

    let status = response.status();
    let body = response
        .text()
        .await
        .context("failed to read upstream chat completion response")?;
    if !status.is_success() {
        return Err(anyhow!(
            "upstream returned {status}: {}",
            sanitized_upstream_body_for_error(&body)
        ));
    }

    let response: ChatCompletionResponse = serde_json::from_str(&body).with_context(|| {
        format!(
            "failed to decode upstream chat completion response: {}",
            sanitized_upstream_body_for_error(&body)
        )
    })?;
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    for choice in response.choices {
        if let Some(choice_content) = choice.message.content {
            content.push_str(&choice_content);
        }
        tool_calls.extend(choice.message.tool_calls);
    }

    Ok(CompletionOutput {
        content,
        tool_calls,
        usage: response.usage.map(Into::into),
    })
}

pub(crate) fn parse_stream_item(data: &str) -> Result<StreamItem> {
    let data = data.trim();
    if data.is_empty() {
        return Ok(StreamItem::Empty);
    }
    if data == "[DONE]" {
        return Ok(StreamItem::Done);
    }

    let chunk: ChatCompletionStreamChunk = serde_json::from_str(data).with_context(|| {
        format!(
            "failed to decode upstream streaming chunk: {}",
            truncate_for_error(data)
        )
    })?;
    if let Some(usage) = chunk.usage {
        return Ok(StreamItem::Usage(usage.into()));
    }

    let mut parsed = ParsedStreamChunk::default();
    for choice in chunk.choices {
        let Some(delta) = choice.delta else {
            continue;
        };
        if let Some(content) = delta.content {
            parsed.content.push_str(&content);
        }
        for (position, tool_call) in delta.tool_calls.into_iter().enumerate() {
            parsed.tool_call_deltas.push(OpenAiToolCallDelta {
                index: tool_call.index.unwrap_or(position),
                id: tool_call.id,
                name: tool_call
                    .function
                    .as_ref()
                    .and_then(|function| function.name.clone()),
                arguments: tool_call
                    .function
                    .and_then(|function| function.arguments)
                    .unwrap_or_default(),
            });
        }
    }

    if parsed.content.is_empty() && parsed.tool_call_deltas.is_empty() {
        Ok(StreamItem::Empty)
    } else {
        Ok(StreamItem::Chunk(parsed))
    }
}

const MAX_UPSTREAM_ERROR_BODY_CHARS: usize = 512;

fn sanitized_upstream_body_for_error(body: &str) -> String {
    let redacted = match serde_json::from_str::<serde_json::Value>(body) {
        Ok(mut value) => {
            redact_json_secrets(&mut value);
            value.to_string()
        }
        Err(_) => body.to_string(),
    };
    truncate_for_error(&redacted)
}

fn truncate_for_error(value: &str) -> String {
    let mut chars = value.chars();
    let truncated = chars
        .by_ref()
        .take(MAX_UPSTREAM_ERROR_BODY_CHARS)
        .collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}…<truncated>")
    } else {
        truncated
    }
}

fn redact_json_secrets(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                if is_secret_json_key(key) {
                    *child = serde_json::Value::String("[redacted]".to_string());
                } else {
                    redact_json_secrets(child);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                redact_json_secrets(child);
            }
        }
        _ => {}
    }
}

fn is_secret_json_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("api_key")
        || key.contains("apikey")
        || key.contains("token")
        || key.contains("secret")
        || key.contains("password")
        || key.contains("authorization")
}

fn request_builder(
    client: &reqwest::Client,
    upstream: &UpstreamConfig,
    request: OpenAiChatRequest,
) -> Result<reqwest::RequestBuilder> {
    let mut builder = client
        .post(chat_completions_url(&upstream.base_url))
        .timeout(Duration::from_secs(upstream.timeout_secs))
        .json(&request);

    if let Some(api_key) = &upstream.api_key {
        builder = builder.bearer_auth(api_key);
    }

    Ok(builder)
}

fn chat_completions_url(base_url: &Url) -> Url {
    let mut url = base_url.clone();
    let path = format!("{}/chat/completions", url.path().trim_end_matches('/'));
    url.set_path(&path);
    url.set_query(None);
    url
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionMessage {
    content: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    tool_calls: Vec<OpenAiToolCall>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionStreamChunk {
    #[serde(default, deserialize_with = "deserialize_null_default")]
    choices: Vec<ChatCompletionStreamChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionStreamChoice {
    delta: Option<ChatCompletionDelta>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionDelta {
    #[serde(default, deserialize_with = "deserialize_null_default")]
    content: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    tool_calls: Vec<RawToolCallDelta>,
}

/// Deserializer that treats explicit JSON `null` as the type's default.
///
/// Some OpenAI-compatible servers (notably sglang/SGL) emit `null` for fields
/// that the OpenAI spec describes as optional or missing. The default serde
/// behavior with `#[serde(default)]` only fills in the default when the field
/// is absent — explicit `null` still triggers a type-mismatch error. This
/// helper bridges that gap so we accept `null`, missing, and present values
/// uniformly.
fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let opt = Option::<T>::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
struct RawToolCallDelta {
    index: Option<usize>,
    id: Option<String>,
    function: Option<RawToolCallDeltaFunction>,
}

#[derive(Debug, Deserialize)]
struct RawToolCallDeltaFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
struct OpenAiUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
}

impl From<OpenAiUsage> for UsageTotals {
    fn from(value: OpenAiUsage) -> Self {
        Self {
            total_input: value.prompt_tokens,
            output: value.completion_tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_streaming_delta() {
        let data = r#"{"choices":[{"delta":{"content":"hello"}}]}"#;
        assert_eq!(
            parse_stream_item(data).unwrap(),
            StreamItem::Chunk(ParsedStreamChunk {
                content: "hello".to_string(),
                tool_call_deltas: vec![],
            })
        );
    }

    #[test]
    fn tolerates_null_tool_calls_in_streaming_delta() {
        // sglang/SGL servers emit explicit `null` for `tool_calls` in
        // streaming chunks. Plain `#[serde(default)]` rejects this; the
        // `deserialize_null_default` shim accepts it. Regression for a real
        // chunk observed against an sglang upstream.
        let data = r#"{"id":"adce1542e8364a12a4686b61e1b00ca4","object":"chat.completion.chunk","created":1777464380,"model":"zai-org/GLM-5.1-FP8","choices":[{"index":0,"delta":{"role":"assistant","content":"","reasoning_content":null,"tool_calls":null},"logprobs":null,"finish_reason":null,"matched_stop":null}],"usage":null}"#;

        // Empty content + empty tool_calls collapses to `Empty` per the
        // existing `parse_stream_item` semantics, which is the right
        // behavior — this chunk simply opens the stream.
        assert_eq!(parse_stream_item(data).unwrap(), StreamItem::Empty);
    }

    #[test]
    fn tolerates_null_content_with_real_text_in_later_chunk() {
        // Same upstream typically follows the role-only opener with chunks
        // that carry actual content. Make sure we still surface the text.
        let data = r#"{"choices":[{"delta":{"role":"assistant","content":"hi","reasoning_content":null,"tool_calls":null}}]}"#;
        assert_eq!(
            parse_stream_item(data).unwrap(),
            StreamItem::Chunk(ParsedStreamChunk {
                content: "hi".to_string(),
                tool_call_deltas: vec![],
            })
        );
    }

    #[test]
    fn parses_streaming_tool_call_delta() {
        let data = serde_json::json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "read_files",
                            "arguments": "{\"files\":[]}",
                        }
                    }]
                }
            }]
        })
        .to_string();
        assert_eq!(
            parse_stream_item(&data).unwrap(),
            StreamItem::Chunk(ParsedStreamChunk {
                content: String::new(),
                tool_call_deltas: vec![OpenAiToolCallDelta {
                    index: 0,
                    id: Some("call_1".to_string()),
                    name: Some("read_files".to_string()),
                    arguments: "{\"files\":[]}".to_string(),
                }],
            })
        );
    }

    #[test]
    fn accumulates_streaming_tool_call_arguments() {
        let mut accumulator = OpenAiToolCallAccumulator::default();
        accumulator.push_deltas(vec![OpenAiToolCallDelta {
            index: 0,
            id: Some("call_1".to_string()),
            name: Some("read_files".to_string()),
            arguments: "{\"files\":".to_string(),
        }]);
        accumulator.push_deltas(vec![OpenAiToolCallDelta {
            index: 0,
            id: None,
            name: None,
            arguments: "[]}".to_string(),
        }]);

        assert_eq!(
            accumulator.finish().unwrap(),
            vec![OpenAiToolCall {
                id: "call_1".to_string(),
                kind: "function".to_string(),
                function: OpenAiToolCallFunction {
                    name: "read_files".to_string(),
                    arguments: "{\"files\":[]}".to_string(),
                },
            }]
        );
    }

    #[test]
    fn appends_chat_completions_to_v1_base_url() {
        let url = Url::parse("http://127.0.0.1:11434/v1").unwrap();
        assert_eq!(
            chat_completions_url(&url).as_str(),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
    }

    #[test]
    fn redacts_and_truncates_upstream_error_bodies() {
        let body = serde_json::json!({
            "error": {
                "message": "bad request",
                "api_key": "sk-secret",
                "nested": { "password": "pw-secret", "token": "tok-secret" }
            },
            "long": "x".repeat(700)
        })
        .to_string();

        let sanitized = sanitized_upstream_body_for_error(&body);

        assert!(sanitized.contains("[redacted]"));
        assert!(sanitized.contains("<truncated>"));
        assert!(!sanitized.contains("sk-secret"));
        assert!(!sanitized.contains("pw-secret"));
        assert!(!sanitized.contains("tok-secret"));
    }
}
