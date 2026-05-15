use crate::adapter::adapters::support::get_api_key;
use crate::adapter::anthropic::AnthropicStreamer;
use crate::adapter::{Adapter, AdapterKind, ServiceType, WebRequestData};
use crate::chat::{
	Binary, BinarySource, CacheControl, CacheCreationDetails, ChatOptionsSet, ChatRequest, ChatResponse,
	ChatResponseFormat, ChatRole, ChatStream, ChatStreamResponse, ContentPart, MessageContent, PromptTokensDetails,
	ReasoningEffort, StopReason, Tool, ToolCall, ToolConfig, ToolName, Usage,
};
use crate::resolver::{AuthData, Endpoint};
use crate::webc::{EventSourceStream, WebResponse};
use crate::{Headers, ModelIden};
use crate::{Result, ServiceTarget};
use reqwest::RequestBuilder;
use serde_json::{Map, Value, json};
use std::sync::OnceLock;
use tracing::info;
use tracing::warn;
use value_ext::JsonValueExt;

pub struct AnthropicAdapter;

const REASONING_LOW: u32 = 1024;
const REASONING_MEDIUM: u32 = 8000;
const REASONING_HIGH: u32 = 24000;

// NOTE: For now, those are opt-ins, but should become opt-out when well supported.
// see: effort doc: https://platform.claude.com/docs/en/build-with-claude/effort
const SUPPORT_EFFORT_MODELS: &[&str] = &["claude-opus-4-6", "claude-sonnet-4-6", "claude-opus-4-5"];
const SUPPORT_REASONING_MAX_MODELS: &[&str] = &["claude-opus-4-6"];
// see:adaptive thinking: https://platform.claude.com/docs/en/build-with-claude/adaptive-thinking
const SUPPORT_ADAPTTIVE_THINK_MODELS: &[&str] = &["claude-opus-4-6", "claude-sonnet-4-6"];

fn has_model(model_prefixes: &[&str], model_name: &str) -> bool {
	model_prefixes.iter().any(|prefix| model_name.contains(prefix))
}

/// Returns true when the given model name looks like a Claude Opus model with
/// version >= `(major, minor)` (e.g. `claude-opus-4-7`, `claude-opus-5-0`).
///
/// The regex is unanchored and tolerates arbitrary prefixes/suffixes around the
/// core `claude-opus-<major>-<minor>` portion. Any parse or regex failure is
/// treated as a conservative `false`.
fn is_opus_at_least(model_name: &str, target_major: u32, target_minor: u32) -> bool {
	static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
	let re = RE.get_or_init(|| regex::Regex::new(r"claude-opus-(\d+)-(\d+)").ok());
	let Some(re) = re.as_ref() else {
		return false;
	};
	let Some(caps) = re.captures(model_name) else {
		return false;
	};
	let major = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
	let minor = caps.get(2).and_then(|m| m.as_str().parse::<u32>().ok());
	match (major, minor) {
		(Some(major), Some(minor)) => (major, minor) >= (target_major, target_minor),
		_ => false,
	}
}

fn is_opus_4_7_or_higher(model_name: &str) -> bool {
	is_opus_at_least(model_name, 4, 7)
}

/// 模型是否支持 1M 上下文 beta(`anthropic-beta: context-1m-2025-08-07`)。
///
/// 判据(对齐 opencode `packages/console/.../anthropic.ts`):
/// - 模型名包含 `sonnet`(Claude Sonnet 4 起全系都支持 1M)
/// - 或为 Opus 4.6 及以上(`claude-opus-4-6`、`claude-opus-4-7`、`claude-opus-5-x` ...)
///
/// 用于 BYOP / 直连 / 中转场景:不带这个 header,某些中转(如 anyrouter)会
/// 直接 400 拒绝带 `claude-opus-4-7` 之类模型名的请求,见 zerx-lab/warp #21。
/// Anthropic 官方接受该 header 后,prompt < 200K 时仍按常规价格,所以默认带上是安全的。
pub(in crate::adapter) fn model_supports_1m_context(model_name: &str) -> bool {
	if model_name.contains("sonnet") {
		return true;
	}
	is_opus_at_least(model_name, 4, 6)
}

const ANTHROPIC_BETA_HEADER: &str = "anthropic-beta";
const ANTHROPIC_BETA_CONTEXT_1M: &str = "context-1m-2025-08-07";

fn insert_anthropic_reasoning(
	payload: &mut Value,
	output_config: &mut Map<String, Value>,
	model_name: &str,
	effort: &ReasoningEffort,
) -> Result<()> {
	let mut budget: Option<u32> = None;
	let support_effort = has_model(SUPPORT_EFFORT_MODELS, model_name);
	let support_reasoning_max = has_model(SUPPORT_REASONING_MAX_MODELS, model_name);
	let support_adaptive = has_model(SUPPORT_ADAPTTIVE_THINK_MODELS, model_name);
	let support_xhigh = is_opus_4_7_or_higher(model_name);

	// if support effort, we default with effor
	if support_effort {
		let effort = match effort {
			ReasoningEffort::Minimal => "low",
			ReasoningEffort::Low => "low",
			ReasoningEffort::Medium => "medium",
			ReasoningEffort::High => "high",
			ReasoningEffort::XHigh if support_xhigh => "xhigh",
			ReasoningEffort::Max | ReasoningEffort::XHigh if support_reasoning_max => "max",
			ReasoningEffort::Max if support_xhigh => "max",
			ReasoningEffort::XHigh => "high",
			ReasoningEffort::Max => "high",
			// we capture for later
			ReasoningEffort::Budget(val) => {
				budget = Some(*val); // not very elegant
				""
			}
			ReasoningEffort::None => "",
		};

		// if we have an effort, write into the shared output_config map
		if !effort.is_empty() {
			output_config.insert("effort".to_string(), json!(effort));
		}
	}

	// -- if support adaptive, we add it (with the eventual budget tokens)
	// if not (but support effort), it should be fined without the thinking object.
	if support_adaptive {
		let thinking = match budget {
			Some(budget) => json!({
						"type": "adaptive",
						"budget_tokens": budget // if None, should be ok.
			}),
			None => json!({
				"type": "adaptive"}),
		};

		// if support adaptive, we set the thinking type to "adaptive" and let the model decide how to use the budget (if any)
		payload.x_insert("thinking", thinking)?;
	}

	// -- If it does not support effort, fall back on the legacy with with budget
	if !support_effort {
		let thinking_budget = match effort {
			ReasoningEffort::None => None,
			ReasoningEffort::Budget(budget) => Some(*budget),
			ReasoningEffort::Low | ReasoningEffort::Minimal => Some(REASONING_LOW),
			ReasoningEffort::Medium => Some(REASONING_MEDIUM),
			ReasoningEffort::High | ReasoningEffort::Max | ReasoningEffort::XHigh => Some(REASONING_HIGH),
		};

		if let Some(thinking_budget) = thinking_budget {
			payload.x_insert(
				"thinking",
				json!({
					"type": "enabled",
					"budget_tokens": thinking_budget
				}),
			)?;
		}
	}

	Ok(())
}

// NOTE: For Anthropic, the max_tokens must be specified.
//       To avoid surprises, the default value for genai is the maximum for a given model.
// Current logic:
// - if model contains `3-opus` or `3-haiku` 4x max token limit,
// - otherwise assume 8k model
//
// NOTE: Will need to add the thinking option: https://docs.anthropic.com/en/docs/build-with-claude/extended-thinking
// For max model tokens see: https://docs.anthropic.com/en/docs/about-claude/models/overview
//
// fall back
pub(in crate::adapter) const MAX_TOKENS_64K: u32 = 64000; // claude-opus-4-5 claude-sonnet... (4 and above), claude-haiku..., claude-3-7-sonnet,
// custom
pub(in crate::adapter) const MAX_TOKENS_32K: u32 = 32000; // claude-opus-4
pub(in crate::adapter) const MAX_TOKENS_8K: u32 = 8192; // claude-3-5-sonnet, claude-3-5-haiku
pub(in crate::adapter) const MAX_TOKENS_4K: u32 = 4096; // claude-3-opus, claude-3-haiku

const ANTHROPIC_VERSION: &str = "2023-06-01";

impl AnthropicAdapter {
	pub const API_KEY_DEFAULT_ENV_NAME: &str = "ANTHROPIC_API_KEY";

	pub(in crate::adapter::adapters) async fn list_model_names_for_end_target(
		kind: AdapterKind,
		endpoint: Endpoint,
		auth: AuthData,
	) -> Result<Vec<String>> {
		// -- url
		let base_url = endpoint.base_url();
		let url = format!("{base_url}models");

		// -- auth / headers
		let api_key = auth.single_key_value().ok();
		let headers = api_key
			.map(|api_key| {
				Headers::from(vec![
					("x-api-key".to_string(), api_key),
					("anthropic-version".to_string(), ANTHROPIC_VERSION.to_string()),
				])
			})
			.unwrap_or_default();

		// -- Exec request
		let web_c = crate::webc::WebClient::default();
		let mut res = web_c
			.do_get(&url, &headers)
			.await
			.map_err(|webc_error| crate::Error::WebAdapterCall {
				adapter_kind: kind,
				webc_error,
			})?;

		// -- Format result
		let mut models: Vec<String> = Vec::new();

		if let Value::Array(models_value) = res.body.x_take("data")? {
			for mut model in models_value {
				let model_name: String = model.x_take("id")?;
				models.push(model_name);
			}
		}

		Ok(models)
	}
}

impl Adapter for AnthropicAdapter {
	const DEFAULT_API_KEY_ENV_NAME: Option<&'static str> = Some(Self::API_KEY_DEFAULT_ENV_NAME);

	fn default_endpoint() -> Endpoint {
		const BASE_URL: &str = "https://api.anthropic.com/v1/";
		Endpoint::from_static(BASE_URL)
	}

	fn default_auth() -> AuthData {
		match Self::DEFAULT_API_KEY_ENV_NAME {
			Some(env_name) => AuthData::from_env(env_name),
			None => AuthData::None,
		}
	}

	async fn all_model_names(kind: AdapterKind, endpoint: Endpoint, auth: AuthData) -> Result<Vec<String>> {
		Self::list_model_names_for_end_target(kind, endpoint, auth).await
	}

	fn get_service_url(_model: &ModelIden, service_type: ServiceType, endpoint: Endpoint) -> Result<String> {
		let base_url = endpoint.base_url();
		let url = match service_type {
			ServiceType::Chat | ServiceType::ChatStream => format!("{base_url}messages"),
			ServiceType::Embed => format!("{base_url}embeddings"), // Anthropic doesn't support embeddings yet
		};

		Ok(url)
	}

	fn to_web_request_data(
		target: ServiceTarget,
		service_type: ServiceType,
		chat_req: ChatRequest,
		options_set: ChatOptionsSet<'_, '_>,
	) -> Result<WebRequestData> {
		let ServiceTarget { endpoint, auth, model } = target;

		// -- api_key
		let api_key = get_api_key(auth, &model)?;

		// -- url
		let url = Self::get_service_url(&model, service_type, endpoint)?;

		// -- headers
		let mut headers = Headers::from(vec![
			("x-api-key".to_string(), api_key),
			("anthropic-version".to_string(), ANTHROPIC_VERSION.to_string()),
		]);

		// -- 1M context beta header(对支持 1M 的模型默认带上)
		// 不带的话,某些中转网关(anyrouter 等)会直接 400 拒绝。
		// Anthropic 官方对 prompt < 200K 仍按常规价格计费,默认带上是安全的。
		// 详见 zerx-lab/warp issue #21。
		let (_, raw_model_name_for_beta) = model.model_name.namespace_and_name();
		if model_supports_1m_context(raw_model_name_for_beta) {
			headers.merge(Headers::from((
				ANTHROPIC_BETA_HEADER.to_string(),
				ANTHROPIC_BETA_CONTEXT_1M.to_string(),
			)));
		}

		// -- 合并用户在 ChatOptions 里追加的 extra_headers(后写覆盖前写)
		// 用户可显式塞同名 header(比如组合多个 beta:`context-1m-...,files-api-...`)
		// 来覆盖 adapter 默认值。
		if let Some(extra_headers) = options_set.extra_headers() {
			headers.merge_with(extra_headers);
		}

		// -- Parts
		let AnthropicRequestParts {
			system,
			messages,
			tools,
		} = Self::into_anthropic_request_parts(chat_req)?;

		// -- Extract Model Name and Reasoning
		let (_, raw_model_name) = model.model_name.namespace_and_name();

		// -- Reasoning Budget
		let (model_name, computed_reasoning_effort) = match (raw_model_name, options_set.reasoning_effort()) {
			// No explicity reasoning_effor, try to infer from model name suffix (supports -zero)
			(model, None) => {
				// let model_name: &str = &model.model_name;
				if let Some((prefix, last)) = raw_model_name.rsplit_once('-') {
					let reasoning = match last {
						"zero" => None,
						"None" => Some(ReasoningEffort::Low),
						"minimal" => Some(ReasoningEffort::Low),
						"low" => Some(ReasoningEffort::Low),
						"medium" => Some(ReasoningEffort::Medium),
						"high" => Some(ReasoningEffort::High),
						"xhigh" => Some(ReasoningEffort::XHigh),
						"max" => Some(ReasoningEffort::Max),
						_ => None,
					};
					// create the model name if there was a `-..` reasoning suffix
					let model = if reasoning.is_some() { prefix } else { model };

					(model, reasoning)
				} else {
					(model, None)
				}
			}
			// If reasoning effort, turn the low, medium, budget ones into Budget
			(model, Some(effort)) => (model, Some(effort.clone())),
		};

		// -- Build the basic payload
		let stream = matches!(service_type, ServiceType::ChatStream);
		let mut payload = json!({
			"model": model_name.to_string(),
			"messages": messages,
			"stream": stream
		});

		if let Some(system) = system {
			payload.x_insert("system", system)?;
		}

		if let Some(tools) = tools {
			payload.x_insert("/tools", tools)?;
		}

		// -- Set the reasoning effort
		// Both reasoning effort and structured-output format write into `output_config`.
		// Build a shared map so both contributions end up in the same object.
		let mut output_config: Map<String, Value> = Map::new();

		if let Some(computed_reasoning_effort) = computed_reasoning_effort {
			insert_anthropic_reasoning(&mut payload, &mut output_config, model_name, &computed_reasoning_effort)?;
		}

		if let Some(cache_control) = options_set.cache_control() {
			info!(
				"Anthropic request-level cache_control '{cache_control:?}' is currently ignored. Use message-level cache_control instead."
			);
		}

		// -- Add supported ChatOptions
		if let Some(ChatResponseFormat::JsonSpec(st_json)) = options_set.response_format() {
			// https://platform.claude.com/docs/en/build-with-claude/structured-outputs#json-outputs
			// Note: Anthropic's json_schema format does not use a schema name; JsonSpec.name is intentionally omitted.
			output_config.insert(
				"format".to_string(),
				json!({
					"type": "json_schema",
					"schema": st_json.schema_with_additional_properties_false(),
				}),
			);
		}

		// Insert output_config once, merging effort + format into a single object.
		if !output_config.is_empty() {
			payload.x_insert("output_config", Value::Object(output_config))?;
		}

		if let Some(temperature) = options_set.temperature() {
			payload.x_insert("temperature", temperature)?;
		}

		if !options_set.stop_sequences().is_empty() {
			payload.x_insert("stop_sequences", options_set.stop_sequences())?;
		}

		let max_tokens = Self::resolve_max_tokens(model_name, &options_set);
		payload.x_insert("max_tokens", max_tokens)?; // required for Anthropic

		if let Some(top_p) = options_set.top_p() {
			payload.x_insert("top_p", top_p)?;
		}

		Ok(WebRequestData { url, headers, payload })
	}

	fn to_chat_response(
		model_iden: ModelIden,
		web_response: WebResponse,
		_options_set: ChatOptionsSet<'_, '_>,
	) -> Result<ChatResponse> {
		let WebResponse { mut body, .. } = web_response;

		// -- Capture the provider_model_iden
		// TODO: Need to be implemented (if available), for now, just clone model_iden
		let provider_model_name: Option<String> = body.x_remove("model").ok();
		let provider_model_iden = model_iden.from_optional_name(provider_model_name);

		// -- Capture the usage
		let usage = body.x_take::<Value>("usage");

		let usage = usage.map(Self::into_usage).unwrap_or_default();
		let stop_reason = body
			.x_take::<Option<String>>("stop_reason")
			.ok()
			.flatten()
			.map(StopReason::from);

		// -- Capture the content
		let mut content: MessageContent = MessageContent::default();

		// NOTE: Here we are going to concatenate all of the Anthropic text content items into one
		//       genai MessageContent::Text. This is more in line with the OpenAI API style,
		//       but loses the fact that they were originally separate items.
		let json_content_items: Vec<Value> = body.x_take("content")?;

		let mut reasoning_content: Vec<String> = Vec::new();

		for mut item in json_content_items {
			let typ: String = item.x_take("type")?;
			match typ.as_ref() {
				"text" => {
					let part = ContentPart::from_text(item.x_take::<String>("text")?);
					content.push(part);
				}
				"thinking" => reasoning_content.push(item.x_take("thinking")?),
				"tool_use" => {
					let call_id = item.x_take::<String>("id")?;
					let fn_name = item.x_take::<String>("name")?;
					// if not found, will be Value::Null
					let fn_arguments = item.x_take::<Value>("input").unwrap_or_default();
					let tool_call = ToolCall {
						call_id,
						fn_name,
						fn_arguments,
						thought_signatures: None,
					};

					let part = ContentPart::ToolCall(tool_call);
					content.push(part);
				}
				other_typ => {
					// insert it back
					item.x_insert("type", other_typ)?;
					content.push(ContentPart::from_custom(item, Some(model_iden.clone())))
				}
			}
		}

		let reasoning_content = if !reasoning_content.is_empty() {
			Some(reasoning_content.join("\n"))
		} else {
			None
		};

		Ok(ChatResponse {
			content,
			reasoning_content,
			model_iden,
			provider_model_iden,
			stop_reason,
			usage,
			captured_raw_body: None, // Set by the client exec_chat
			response_id: None,
		})
	}

	fn to_chat_stream(
		model_iden: ModelIden,
		reqwest_builder: RequestBuilder,
		options_set: ChatOptionsSet<'_, '_>,
	) -> Result<ChatStreamResponse> {
		let event_source = EventSourceStream::new(reqwest_builder);
		let anthropic_stream = AnthropicStreamer::new(event_source, model_iden.clone(), options_set);
		let chat_stream = ChatStream::from_inter_stream(anthropic_stream);
		Ok(ChatStreamResponse {
			model_iden,
			stream: chat_stream,
		})
	}

	fn to_embed_request_data(
		_service_target: crate::ServiceTarget,
		_embed_req: crate::embed::EmbedRequest,
		_options_set: crate::embed::EmbedOptionsSet<'_, '_>,
	) -> Result<crate::adapter::WebRequestData> {
		Err(crate::Error::AdapterNotSupported {
			adapter_kind: crate::adapter::AdapterKind::Anthropic,
			feature: "embeddings".to_string(),
		})
	}

	fn to_embed_response(
		_model_iden: crate::ModelIden,
		_web_response: crate::webc::WebResponse,
		_options_set: crate::embed::EmbedOptionsSet<'_, '_>,
	) -> Result<crate::embed::EmbedResponse> {
		Err(crate::Error::AdapterNotSupported {
			adapter_kind: crate::adapter::AdapterKind::Anthropic,
			feature: "embeddings".to_string(),
		})
	}
}

// region:    --- Support

impl AnthropicAdapter {
	/// Resolves the max_tokens value for an Anthropic model, using the user-provided
	/// value if set, or a model-appropriate default.
	pub(in crate::adapter) fn resolve_max_tokens(model_name: &str, options_set: &ChatOptionsSet) -> u32 {
		options_set.max_tokens().unwrap_or_else(|| {
			// most likely models used, so put first. Also a little wider with `claude-sonnet` (since name from version 4)
			if model_name.contains("claude-sonnet")
				|| model_name.contains("claude-haiku")
				|| model_name.contains("claude-3-7-sonnet")
				|| model_name.contains("claude-opus-4-5")
			{
				MAX_TOKENS_64K
			} else if model_name.contains("claude-opus-4") {
				MAX_TOKENS_32K
			} else if model_name.contains("claude-3-5") {
				MAX_TOKENS_8K
			} else if model_name.contains("3-opus") || model_name.contains("3-haiku") {
				MAX_TOKENS_4K
			}
			// for now, fall back on the 64K by default (might want to be more conservative)
			else {
				MAX_TOKENS_64K
			}
		})
	}

	pub(in crate::adapter) fn into_usage(mut usage_value: Value) -> Usage {
		// IMPORTANT: For Anthropic, the `input_tokens` does not include `cache_creation_input_tokens` or `cache_read_input_tokens`.
		// Therefore, it must be normalized in the OpenAI style, where it includes both cached and written tokens (for symmetry).
		let input_tokens: i32 = usage_value.x_take("input_tokens").ok().unwrap_or(0);
		let cache_creation_input_tokens: i32 = usage_value.x_take("cache_creation_input_tokens").unwrap_or(0);
		let cache_read_input_tokens: i32 = usage_value.x_take("cache_read_input_tokens").unwrap_or(0);
		let completion_tokens: i32 = usage_value.x_take("output_tokens").ok().unwrap_or(0);

		// Parse cache_creation breakdown if present (TTL-specific breakdown)
		let cache_creation_details = usage_value.get("cache_creation").and_then(parse_cache_creation_details);

		// compute the prompt_tokens
		let prompt_tokens = input_tokens + cache_creation_input_tokens + cache_read_input_tokens;

		// Compute total_tokens
		let total_tokens = prompt_tokens + completion_tokens;

		// For now the logic is to have a Some of PromptTokensDetails if at least one of those value is not 0
		// TODO: Needs to be normalized across adapters.
		let prompt_tokens_details =
			if cache_creation_input_tokens > 0 || cache_read_input_tokens > 0 || cache_creation_details.is_some() {
				Some(PromptTokensDetails {
					cache_creation_tokens: Some(cache_creation_input_tokens),
					cache_creation_details,
					cached_tokens: Some(cache_read_input_tokens),
					audio_tokens: None,
				})
			} else {
				None
			};

		Usage {
			prompt_tokens: Some(prompt_tokens),
			prompt_tokens_details,

			completion_tokens: Some(completion_tokens),
			// for now, None for Anthropic
			completion_tokens_details: None,

			total_tokens: Some(total_tokens),
		}
	}

	/// Takes the GenAI ChatMessages and constructs the System string and JSON Messages for Anthropic.
	/// - Will push the `ChatRequest.system` and system message to `AnthropicRequestParts.system`
	pub(in crate::adapter) fn into_anthropic_request_parts(chat_req: ChatRequest) -> Result<AnthropicRequestParts> {
		let mut messages: Vec<Value> = Vec::new();
		// (content, cache_control)
		let mut systems: Vec<(String, Option<CacheControl>)> = Vec::new();

		// Track TTL ordering for validation (1h must come before 5m)
		let mut seen_5m_cache = false;

		// NOTE: For now, this means the first System cannot have a cache control
		//       so that we do not change too much.
		if let Some(system) = chat_req.system {
			systems.push((system, None));
		}

		// -- Process the messages
		for msg in chat_req.messages {
			let cache_control = msg.options.and_then(|o| o.cache_control);

			// Check TTL ordering constraint
			if let Some(ref cc) = cache_control {
				match cc {
					CacheControl::Memory | CacheControl::Ephemeral | CacheControl::Ephemeral5m => {
						seen_5m_cache = true;
					}
					CacheControl::Ephemeral1h | CacheControl::Ephemeral24h => {
						if seen_5m_cache {
							warn!(
								"Anthropic cache TTL ordering violation: Ephemeral1h appears after Ephemeral/Ephemeral5m. \
								1-hour cache entries must appear before 5-minute cache entries. \
								See: https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching#mixing-different-ttls"
							);
						}
					}
				}
			}

			match msg.role {
				// Collect only text for system; other content parts are ignored by Anthropic here.
				ChatRole::System => {
					if let Some(system_text) = msg.content.joined_texts() {
						systems.push((system_text, cache_control));
					}
				}

				// User message: text, binary (image/document), and tool_result supported.
				ChatRole::User => {
					if msg.content.is_text_only() {
						let text = msg.content.joined_texts().unwrap_or_else(String::new);
						let content = apply_cache_control_to_text(cache_control.as_ref(), text);
						messages.push(json!({"role": "user", "content": content}));
					} else {
						let mut values: Vec<Value> = Vec::new();
						for part in msg.content {
							match part {
								ContentPart::Text(text) => {
									values.push(json!({"type": "text", "text": text}));
								}
								ContentPart::Binary(binary) => {
									let is_image = binary.is_image();
									let Binary {
										content_type, source, ..
									} = binary;

									if is_image {
										match &source {
											BinarySource::Url(_) => {
												// As of this API version, Anthropic doesn't support images by URL directly in messages.
												warn!(
													"Anthropic doesn't support images from URL, need to handle it gracefully"
												);
											}
											BinarySource::Base64(content) => {
												values.push(json!({
													"type": "image",
													"source": {
														"type": "base64",
														"media_type": content_type,
														"data": content,
													}
												}));
											}
										}
									} else {
										match &source {
											BinarySource::Url(url) => {
												values.push(json!({
													"type": "document",
													"source": {
														"type": "url",
														"url": url,
													}
												}));
											}
											BinarySource::Base64(b64) => {
												values.push(json!({
													"type": "document",
													"source": {
														"type": "base64",
														"media_type": content_type,
														"data": b64,
													}
												}));
											}
										}
									}
								}
								// ToolCall is not valid in user content for Anthropic; skip gracefully.
								ContentPart::ToolCall(_tc) => {}
								ContentPart::ToolResponse(tool_response) => {
									values.push(json!({
										"type": "tool_result",
										"content": tool_response.content,
										"tool_use_id": tool_response.call_id,
									}));
								}
								ContentPart::ThoughtSignature(_) => {}
								ContentPart::ReasoningContent(_) => {}
								// Custom are ignored for this logic
								ContentPart::Custom(_) => {}
							}
						}
						let values = apply_cache_control_to_parts(cache_control.as_ref(), values);
						messages.push(json!({"role": "user", "content": values}));
					}
				}

				// Assistant can mix text and tool_use entries.
				ChatRole::Assistant => {
					let mut values: Vec<Value> = Vec::new();
					let mut has_tool_use = false;
					let mut has_text = false;

					for part in msg.content {
						match part {
							ContentPart::Text(text) => {
								has_text = true;
								values.push(json!({"type": "text", "text": text}));
							}
							ContentPart::ToolCall(tool_call) => {
								has_tool_use = true;
								// Anthropic API requires `input` to be an object, never null.
								// Streaming parsers may produce null arguments when deltas are
								// missing or empty; fall back to an empty object in that case.
								let input = if tool_call.fn_arguments.is_null() {
									Value::Object(Map::new())
								} else {
									tool_call.fn_arguments
								};
								// see: https://docs.anthropic.com/en/docs/build-with-claude/tool-use#example-of-successful-tool-result
								values.push(json!({
									"type": "tool_use",
									"id": tool_call.call_id,
									"name": tool_call.fn_name,
									"input": input,
								}));
							}
							// Unsupported for assistant role in Anthropic message content
							ContentPart::Binary(_) => {}
							ContentPart::ToolResponse(_) => {}
							ContentPart::ThoughtSignature(_) => {}
							ContentPart::ReasoningContent(_) => {}
							// Custom are ignored for this logic
							ContentPart::Custom(_) => {}
						}
					}

					if !has_tool_use && has_text && cache_control.is_none() && values.len() == 1 {
						// Optimize to simple string when it's only one text part and no cache control.
						let text = values
							.first()
							.and_then(|v| v.get("text"))
							.and_then(|v| v.as_str())
							.unwrap_or_default()
							.to_string();
						let content = apply_cache_control_to_text(None, text);
						messages.push(json!({"role": "assistant", "content": content}));
					} else {
						let values = apply_cache_control_to_parts(cache_control.as_ref(), values);
						messages.push(json!({"role": "assistant", "content": values}));
					}
				}

				// Tool responses are represented as user tool_result items in Anthropic.
				ChatRole::Tool => {
					let mut values: Vec<Value> = Vec::new();
					for part in msg.content {
						if let ContentPart::ToolResponse(tool_response) = part {
							values.push(json!({
								"type": "tool_result",
								"content": tool_response.content,
								"tool_use_id": tool_response.call_id,
							}));
						}
					}
					if !values.is_empty() {
						let values = apply_cache_control_to_parts(cache_control.as_ref(), values);
						messages.push(json!({"role": "user", "content": values}));
					}
				}
			}
		}

		// -- Create the Anthropic system
		// NOTE: Anthropic does not have a "role": "system", just a single optional system property
		let system = if !systems.is_empty() {
			let has_any_cache = systems.iter().any(|(_, cc)| cc.is_some());
			let system: Value = if has_any_cache {
				// Build multi-part system with per-part cache_control
				let parts: Vec<Value> = systems
					.iter()
					.map(|(content, cc)| {
						if let Some(cc) = cc {
							json!({"type": "text", "text": content, "cache_control": cache_control_to_json(cc)})
						} else {
							json!({"type": "text", "text": content})
						}
					})
					.collect();
				json!(parts)
			} else {
				let content_buff = systems.iter().map(|(content, _)| content.as_str()).collect::<Vec<&str>>();
				// we add empty line in between each system
				let content = content_buff.join("\n\n");
				json!(content)
			};
			Some(system)
		} else {
			None
		};

		// -- Process the tools

		let tools: Option<Vec<Value>> = chat_req
			.tools
			.map(|tools| {
				tools
					.into_iter()
					.map(Self::tool_to_anthropic_tool)
					.collect::<Result<Vec<Value>>>()
			})
			.transpose()?;

		Ok(AnthropicRequestParts {
			system,
			messages,
			tools,
		})
	}

	fn tool_to_anthropic_tool(tool: Tool) -> Result<Value> {
		let Tool {
			name,
			description,
			schema,
			config,
			cache_control,
			..
		} = tool;

		let name = match name {
			ToolName::WebSearch => "web_search".to_string(),
			ToolName::Custom(name) => name,
		};

		let mut tool_value = json!({"name": name});

		// -- Add type for builtin tool
		#[allow(clippy::single_match)] // will have more
		match name.as_str() {
			"web_search" => {
				tool_value.x_insert("type", "web_search_20250305")?;
			}
			_ => (),
		}

		// NOTE: Fo now, if tool_value.type then, assume bultin and set config as propertie
		if tool_value.get("type").is_some() {
			if let Some(config) = config {
				match config {
					ToolConfig::WebSearch(config) => {
						if let Some(max_uses) = config.max_uses {
							let _ = tool_value.x_insert("max_uses", max_uses);
						}
						if let Some(allowed_domains) = config.allowed_domains {
							let _ = tool_value.x_insert("allowed_domains", allowed_domains);
						}
						if let Some(blocked_domains) = config.blocked_domains {
							let _ = tool_value.x_insert("blocked_domains", blocked_domains);
						}
					}
					// if custom, we assume we flatten the config properties since we are in a builtin
					ToolConfig::Custom(config) => {
						// NOTE: For now, ignore if not object
						tool_value.x_merge(config)?;
					}
				}
			}
		} else {
			tool_value.x_insert("input_schema", schema)?;
			if let Some(description) = description {
				// TODO: need to handle error
				let _ = tool_value.x_insert("description", description);
			}
		}

		// -- Per-tool cache_control breakpoint
		// Anthropic accepts `cache_control` on any tool in the `tools` array; the
		// canonical pattern is to mark only the **last** tool so the entire tools
		// segment becomes a single cache prefix. We just forward whatever the
		// caller set on the `Tool` and let the caller decide which tool gets it.
		if let Some(cc) = cache_control {
			tool_value.x_insert("cache_control", cache_control_to_json(&cc))?;
		}

		Ok(tool_value)
	}
}

/// Convert CacheControl to Anthropic JSON format.
///
/// See: https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching#1-hour-cache-duration
fn cache_control_to_json(cache_control: &CacheControl) -> Value {
	match cache_control {
		CacheControl::Ephemeral => {
			json!({"type": "ephemeral"})
		}
		CacheControl::Memory => {
			json!({"type": "ephemeral"})
		}
		CacheControl::Ephemeral5m => {
			json!({"type": "ephemeral", "ttl": "5m"})
		}
		CacheControl::Ephemeral1h => {
			json!({"type": "ephemeral", "ttl": "1h"})
		}
		CacheControl::Ephemeral24h => {
			json!({"type": "ephemeral", "ttl": "1h"})
		}
	}
}

/// Parse cache_creation breakdown from Anthropic API response.
///
/// The API returns TTL-specific token counts in the `cache_creation` object:
/// ```json
/// "cache_creation": {
///     "ephemeral_5m_input_tokens": 456,
///     "ephemeral_1h_input_tokens": 100
/// }
/// ```
pub(super) fn parse_cache_creation_details(cache_creation: &Value) -> Option<CacheCreationDetails> {
	let ephemeral_5m_tokens = cache_creation
		.get("ephemeral_5m_input_tokens")
		.and_then(|v| v.as_i64())
		.map(|v| v as i32);
	let ephemeral_1h_tokens = cache_creation
		.get("ephemeral_1h_input_tokens")
		.and_then(|v| v.as_i64())
		.map(|v| v as i32);

	// Only return Some if at least one TTL has tokens
	if ephemeral_5m_tokens.is_some() || ephemeral_1h_tokens.is_some() {
		Some(CacheCreationDetails {
			ephemeral_5m_tokens,
			ephemeral_1h_tokens,
		})
	} else {
		None
	}
}

/// Apply the cache control logic to a text content
fn apply_cache_control_to_text(cache_control: Option<&CacheControl>, content: String) -> Value {
	if let Some(cc) = cache_control {
		let value = json!({"type": "text", "text": content, "cache_control": cache_control_to_json(cc)});
		json!(vec![value])
	}
	// simple return
	else {
		json!(content)
	}
}

/// Apply the cache control logic to a text content
fn apply_cache_control_to_parts(cache_control: Option<&CacheControl>, parts: Vec<Value>) -> Vec<Value> {
	let mut parts = parts;
	if let Some(cc) = cache_control
		&& !parts.is_empty()
	{
		let len = parts.len();
		if let Some(last_value) = parts.get_mut(len - 1) {
			// NOTE: For now, if it fails, then, no cache
			let _ = last_value.x_insert("cache_control", cache_control_to_json(cc));
			// TODO: Should warn
		}
	}
	parts
}

pub(in crate::adapter) struct AnthropicRequestParts {
	pub system: Option<Value>,
	pub messages: Vec<Value>,
	pub tools: Option<Vec<Value>>,
}

// endregion: --- Support

// region:    --- Tests

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ServiceTarget;
	use crate::adapter::{Adapter, ServiceType};
	use crate::chat::{ChatOptions, ChatRequest, JsonSpec};
	use crate::resolver::AuthData;

	/// Regression guard: when both `reasoning_effort` and `JsonSpec` response format are set
	/// on a model that uses the `output_config` effort API (e.g. `claude-sonnet-4-6`), both
	/// `effort` and `format` must appear inside the same `output_config` JSON object.
	#[test]
	fn test_output_config_merges_effort_and_format() {
		let chat_options = ChatOptions {
			reasoning_effort: Some(ReasoningEffort::High),
			response_format: Some(ChatResponseFormat::JsonSpec(JsonSpec::new(
				"anthropic_ignores_name", // NOTE: Anthropic doesn't recognize a "name" field
				json!({"type": "object", "properties": {}}),
			))),
			..Default::default()
		};

		let model_iden = ModelIden::new(AdapterKind::Anthropic, "claude-sonnet-4-6");
		let target = ServiceTarget {
			endpoint: AnthropicAdapter::default_endpoint(),
			auth: AuthData::from_single("test-key"),
			model: model_iden,
		};
		let options_set = ChatOptionsSet::default().with_chat_options(Some(&chat_options));

		let result = AnthropicAdapter::to_web_request_data(
			target,
			ServiceType::Chat,
			ChatRequest::from_user("hello"),
			options_set,
		);

		let web_req = result.expect("to_web_request_data should succeed");
		let output_config = web_req.payload.get("output_config").expect("output_config must be present");

		assert_eq!(
			output_config.get("effort").and_then(|v| v.as_str()),
			Some("high"),
			"effort must be present in output_config"
		);
		assert_eq!(
			output_config.get("format").and_then(|f| f.get("type")).and_then(|v| v.as_str()),
			Some("json_schema"),
			"format.type must be present in output_config"
		);
	}

	/// 辅助:从 Headers 中按名取值(case-sensitive)。
	fn header_value<'a>(headers: &'a crate::Headers, name: &str) -> Option<&'a str> {
		headers.iter().find_map(|(k, v)| (k == name).then_some(v.as_str()))
	}

	fn build_minimal_request(model_name: &str) -> WebRequestData {
		let chat_options = ChatOptions::default();
		let target = ServiceTarget {
			endpoint: AnthropicAdapter::default_endpoint(),
			auth: AuthData::from_single("test-key"),
			model: ModelIden::new(AdapterKind::Anthropic, model_name),
		};
		let options_set = ChatOptionsSet::default().with_chat_options(Some(&chat_options));
		AnthropicAdapter::to_web_request_data(
			target,
			ServiceType::Chat,
			ChatRequest::from_user("hello"),
			options_set,
		)
		.expect("to_web_request_data should succeed")
	}

	/// 回归保护(zerx-lab/warp #21):支持 1M 上下文的模型必须默认带
	/// `anthropic-beta: context-1m-2025-08-07`,否则 anyrouter 等中转网关 400。
	#[test]
	fn test_anthropic_beta_header_for_opus_4_7() {
		let req = build_minimal_request("claude-opus-4-7");
		assert_eq!(
			header_value(&req.headers, "anthropic-beta"),
			Some("context-1m-2025-08-07"),
			"opus-4-7 must default to 1m context beta header"
		);
	}

	#[test]
	fn test_anthropic_beta_header_for_sonnet_4_6() {
		let req = build_minimal_request("claude-sonnet-4-6");
		assert_eq!(
			header_value(&req.headers, "anthropic-beta"),
			Some("context-1m-2025-08-07"),
			"sonnet 全系都支持 1m context"
		);
	}

	#[test]
	fn test_anthropic_beta_header_for_opus_4_6() {
		let req = build_minimal_request("claude-opus-4-6");
		assert_eq!(
			header_value(&req.headers, "anthropic-beta"),
			Some("context-1m-2025-08-07"),
		);
	}

	#[test]
	fn test_no_beta_header_for_old_models() {
		let req = build_minimal_request("claude-3-5-haiku");
		assert!(
			header_value(&req.headers, "anthropic-beta").is_none(),
			"haiku 3.5 不支持 1m,不应注入 beta header"
		);

		let req = build_minimal_request("claude-opus-4-5");
		assert!(
			header_value(&req.headers, "anthropic-beta").is_none(),
			"opus 4.5 不支持 1m,不应注入 beta header"
		);
	}

	#[test]
	fn test_extra_headers_can_override_beta() {
		// 用户在 ChatOptions 里塞自定义 anthropic-beta(比如组合多个 beta feature)
		// 应该覆盖 adapter 默认的 1m 单值。
		let custom = crate::Headers::from((
			"anthropic-beta".to_string(),
			"context-1m-2025-08-07,files-api-2025-04-14".to_string(),
		));
		let chat_options = ChatOptions::default().with_extra_headers(custom);
		let target = ServiceTarget {
			endpoint: AnthropicAdapter::default_endpoint(),
			auth: AuthData::from_single("test-key"),
			model: ModelIden::new(AdapterKind::Anthropic, "claude-opus-4-7"),
		};
		let options_set = ChatOptionsSet::default().with_chat_options(Some(&chat_options));
		let req = AnthropicAdapter::to_web_request_data(
			target,
			ServiceType::Chat,
			ChatRequest::from_user("hello"),
			options_set,
		)
		.expect("to_web_request_data should succeed");
		assert_eq!(
			header_value(&req.headers, "anthropic-beta"),
			Some("context-1m-2025-08-07,files-api-2025-04-14"),
			"用户 extra_headers 必须覆盖 adapter 默认 beta header"
		);
	}

	#[test]
	fn test_model_supports_1m_context_matrix() {
		assert!(model_supports_1m_context("claude-sonnet-4-6"));
		assert!(model_supports_1m_context("claude-sonnet-4-7"));
		assert!(model_supports_1m_context("anthropic/claude-sonnet-4-6"));
		assert!(model_supports_1m_context("claude-opus-4-6"));
		assert!(model_supports_1m_context("claude-opus-4-7"));
		assert!(model_supports_1m_context("claude-opus-5-0"));
		assert!(!model_supports_1m_context("claude-opus-4-5"));
		assert!(!model_supports_1m_context("claude-3-5-haiku"));
		assert!(!model_supports_1m_context("claude-haiku-4-5"));
		assert!(!model_supports_1m_context("gpt-4o"));
	}

	#[test]
	fn test_cache_control_to_json_ephemeral() {
		let result = cache_control_to_json(&CacheControl::Ephemeral);
		assert_eq!(result, json!({"type": "ephemeral"}));
	}

	#[test]
	fn test_cache_control_to_json_ephemeral_5m() {
		let result = cache_control_to_json(&CacheControl::Ephemeral5m);
		assert_eq!(result, json!({"type": "ephemeral", "ttl": "5m"}));
	}

	#[test]
	fn test_cache_control_to_json_memory() {
		let result = cache_control_to_json(&CacheControl::Memory);
		assert_eq!(result, json!({"type": "ephemeral"}));
	}

	#[test]
	fn test_cache_control_to_json_ephemeral_1h() {
		let result = cache_control_to_json(&CacheControl::Ephemeral1h);
		assert_eq!(result, json!({"type": "ephemeral", "ttl": "1h"}));
	}

	#[test]
	fn test_cache_control_to_json_ephemeral_24h() {
		let result = cache_control_to_json(&CacheControl::Ephemeral24h);
		assert_eq!(result, json!({"type": "ephemeral", "ttl": "1h"}));
	}

	#[test]
	fn test_parse_cache_creation_details_with_both_ttls() {
		let cache_creation = json!({
			"ephemeral_5m_input_tokens": 456,
			"ephemeral_1h_input_tokens": 100
		});
		let result = parse_cache_creation_details(&cache_creation);
		assert!(result.is_some());
		let details = result.unwrap();
		assert_eq!(details.ephemeral_5m_tokens, Some(456));
		assert_eq!(details.ephemeral_1h_tokens, Some(100));
	}

	#[test]
	fn test_parse_cache_creation_details_with_5m_only() {
		let cache_creation = json!({
			"ephemeral_5m_input_tokens": 456
		});
		let result = parse_cache_creation_details(&cache_creation);
		assert!(result.is_some());
		let details = result.unwrap();
		assert_eq!(details.ephemeral_5m_tokens, Some(456));
		assert_eq!(details.ephemeral_1h_tokens, None);
	}

	#[test]
	fn test_parse_cache_creation_details_with_1h_only() {
		let cache_creation = json!({
			"ephemeral_1h_input_tokens": 100
		});
		let result = parse_cache_creation_details(&cache_creation);
		assert!(result.is_some());
		let details = result.unwrap();
		assert_eq!(details.ephemeral_5m_tokens, None);
		assert_eq!(details.ephemeral_1h_tokens, Some(100));
	}

	#[test]
	fn test_parse_cache_creation_details_empty() {
		let cache_creation = json!({});
		let result = parse_cache_creation_details(&cache_creation);
		assert!(result.is_none());
	}
}

// endregion: --- Tests
