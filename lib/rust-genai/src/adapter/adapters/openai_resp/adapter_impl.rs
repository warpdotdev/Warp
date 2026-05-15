use crate::adapter::adapters::support::get_api_key;
use crate::adapter::openai::OpenAIAdapter;
use crate::adapter::openai_resp::OpenAIRespStreamer;
use crate::adapter::openai_resp::resp_types::RespResponse;
use crate::adapter::{Adapter, AdapterDispatcher, AdapterKind, ServiceType, WebRequestData};
use crate::chat::{
	CacheControl, ChatOptionsSet, ChatRequest, ChatResponse, ChatResponseFormat, ChatRole, ChatStream,
	ChatStreamResponse, ContentPart, MessageContent, ReasoningEffort, StopReason, Tool, ToolConfig, ToolName, Usage,
};
use crate::resolver::{AuthData, Endpoint};
use crate::webc::{EventSourceStream, WebResponse};
use crate::{Error, Headers, Result};
use crate::{ModelIden, ServiceTarget};
use reqwest::RequestBuilder;
use serde_json::{Map, Value, json};
use value_ext::JsonValueExt;

pub struct OpenAIRespAdapter;

impl OpenAIRespAdapter {
	pub const API_KEY_DEFAULT_ENV_NAME: &str = "OPENAI_API_KEY";
}

impl Adapter for OpenAIRespAdapter {
	const DEFAULT_API_KEY_ENV_NAME: Option<&'static str> = Some(Self::API_KEY_DEFAULT_ENV_NAME);

	fn default_auth() -> AuthData {
		match Self::DEFAULT_API_KEY_ENV_NAME {
			Some(env_name) => AuthData::from_env(env_name),
			None => AuthData::None,
		}
	}

	fn default_endpoint() -> Endpoint {
		const BASE_URL: &str = "https://api.openai.com/v1/";
		Endpoint::from_static(BASE_URL)
	}

	/// Note: Currently returns the common models (see above)
	async fn all_model_names(kind: AdapterKind, endpoint: Endpoint, auth: AuthData) -> Result<Vec<String>> {
		//
		OpenAIAdapter::list_model_names_for_end_target(kind, endpoint, auth).await
	}

	fn get_service_url(model: &ModelIden, service_type: ServiceType, endpoint: Endpoint) -> Result<String> {
		Self::util_get_service_url(model, service_type, endpoint)
	}

	/// OpenAI Doc: https://platform.openai.com/docs/api-reference/responses/create
	///
	/// ## Note related to OpenAI Responses API
	/// - `.store = false` - To maintain consistent behavior with other chat completions, store is set to false
	/// - `.instructions` For now we do not use the top ".instructions" (genai::ChatRequest.system),
	///   but just add this top system as a regular system message.
	/// - `.summary` reasoning summary is opt-in via `ChatOptions.capture_reasoning_content(true)` → `"detailed"`
	///
	fn to_web_request_data(
		target: ServiceTarget,
		service_type: ServiceType,
		chat_req: ChatRequest,
		chat_options: ChatOptionsSet<'_, '_>,
	) -> Result<WebRequestData> {
		let ServiceTarget { model, auth, endpoint } = target;
		let (_, model_name) = model.model_name.namespace_and_name();
		let adapter_kind = model.adapter_kind;

		// -- api_key
		let api_key = get_api_key(auth, &model)?;

		// -- url
		let url = AdapterDispatcher::get_service_url(&model, service_type, endpoint)?;

		// -- headers
		let headers = Headers::from(("Authorization".to_string(), format!("Bearer {api_key}")));

		let stream = matches!(service_type, ServiceType::ChatStream);

		// -- compute reasoning_effort and eventual trimmed model_name
		// For now, just for openai AdapterKind
		let (reasoning_effort, model_name): (Option<ReasoningEffort>, &str) =
			if matches!(adapter_kind, AdapterKind::OpenAIResp) {
				let (reasoning_effort, model_name) = chat_options
					.reasoning_effort()
					.cloned()
					.map(|v| (Some(v), model_name))
					.unwrap_or_else(|| ReasoningEffort::from_model_name(model_name));

				(reasoning_effort, model_name)
			} else {
				(None, model_name)
			};

		// -- Extract system prompt before consuming chat_req.
		// Use the Responses API `instructions` field instead of an input system message.
		// `instructions` is the canonical way to set system prompt in the Responses API:
		// - It overrides on each call (important for stateful sessions with previous_response_id)
		// - It separates instructions from conversation items
		// - Inline system messages (ChatRole::System in messages) still go to input as-is
		let instructions = chat_req.system.clone();
		let mut chat_req = chat_req;
		chat_req.system = None;

		// -- Extract stateful session fields before consuming chat_req
		let previous_response_id = chat_req.previous_response_id.clone();
		let explicit_store = chat_req.store;

		// -- Build the basic payload
		let OpenAIRespRequestParts {
			input_items: messages,
			tools,
		} = Self::into_openai_request_parts(&model, chat_req)?;

		// Store: always opt-in. If not explicitly set, default is false.
		// Privacy first: we never implicitly set store=true, even when previous_response_id is set.
		// If previous_response_id is set without store=true, log a warning — the caller must be explicit.
		let store = explicit_store.unwrap_or(false);
		if previous_response_id.is_some() && explicit_store != Some(true) {
			tracing::warn!(
				"previous_response_id is set but store is not explicitly true — \
				 stateful session requires store=true to work. Set `store: Some(true)` explicitly."
			);
		}

		let mut payload = json!({
			"store": store,
			"model": model_name,
			"input": messages,
			"stream": stream,
		});

		// -- System prompt as instructions
		if let Some(instructions) = &instructions {
			payload.x_insert("instructions", instructions.as_str())?;
		}

		// -- Stateful session: add previous_response_id
		if let Some(prev_id) = &previous_response_id {
			payload.x_insert("previous_response_id", prev_id.as_str())?;
		}

		// -- Set reasoning options
		//
		// The `reasoning` object on the request controls two things:
		//   * `.effort` — how much reasoning the model should do
		//   * `.summary` — whether a text summary of the reasoning is
		//     returned in the response (required to populate
		//     `ChatResponse.reasoning_content` for the Responses API)
		//
		// Either half is sufficient to warrant inserting the object;
		// previously the object was only emitted when `reasoning_effort`
		// was set, which silently defeated `capture_reasoning_content(true)`
		// on its own — callers asking for reasoning capture got no
		// `summary=detailed` opt-in, and every response came back with
		// empty `reasoning_content`.
		let capture_reasoning = chat_options.capture_reasoning_content() == Some(true);
		let effort_keyword = reasoning_effort.and_then(|e| e.as_keyword());

		if effort_keyword.is_some() || capture_reasoning {
			let mut reasoning_obj = json!({});
			if let Some(keyword) = effort_keyword {
				reasoning_obj
					.x_insert("effort", keyword)
					.map_err(|e| Error::Internal(format!("reasoning effort insert: {e}")))?;
			}
			if capture_reasoning {
				reasoning_obj
					.x_insert("summary", "detailed")
					.map_err(|e| Error::Internal(format!("reasoning summary insert: {e}")))?;
			}
			payload.x_insert("reasoning", reasoning_obj)?;
		}

		// -- Opt-in: request encrypted reasoning content (thought signatures)
		// when the caller explicitly asks for reasoning content capture.
		if chat_options.capture_reasoning_content() == Some(true) {
			payload.x_insert("include", json!(["reasoning.encrypted_content"]))?;
		}

		// -- Tools
		if let Some(tools) = tools {
			payload.x_insert("/tools", tools)?;
		}

		// -- Compute response format
		let response_format = if let Some(response_format) = chat_options.response_format() {
			match response_format {
				ChatResponseFormat::JsonMode => Some(json!({"type": "json_object"})),
				ChatResponseFormat::JsonSpec(st_json) => {
					// Flatten for OpenAI Responses
					Some(json!({
						"type": "json_schema",
						"name": st_json.name.clone(),
						"strict": true,
						// TODO: add description
						"schema": st_json.schema_with_additional_properties_false(),
					}))
				}
			}
		} else {
			None
		};

		// -- Get verbosity
		let verbosity = chat_options.verbosity().and_then(|v| v.as_keyword());

		if response_format.is_some() || verbosity.is_some() {
			let mut value_map = Map::new();
			if let Some(verbosity) = verbosity {
				value_map.insert("verbosity".into(), verbosity.into());
			}
			if let Some(response_format) = response_format {
				value_map.insert("format".into(), response_format);
			}

			payload.x_insert("text", value_map)?;
		}

		// -- Add supported ChatOptions
		if let Some(temperature) = chat_options.temperature() {
			payload.x_insert("temperature", temperature)?;
		}

		if !chat_options.stop_sequences().is_empty() {
			payload.x_insert("stop", chat_options.stop_sequences())?;
		}

		if let Some(max_tokens) = chat_options.max_tokens() {
			payload.x_insert("max_output_tokens", max_tokens)?;
		}
		if let Some(top_p) = chat_options.top_p() {
			payload.x_insert("top_p", top_p)?;
		}
		if let Some(seed) = chat_options.seed() {
			payload.x_insert("seed", seed)?;
		}

		// -- OpenAI prompt cache options
		if let Some(prompt_cache_key) = chat_options.prompt_cache_key() {
			payload.x_insert("prompt_cache_key", prompt_cache_key)?;
		}
		if let Some(cache_control) = chat_options.cache_control() {
			let prompt_cache_retention = match cache_control {
				CacheControl::Memory | CacheControl::Ephemeral => Some("in_memory"),
				CacheControl::Ephemeral24h => Some("24h"),
				CacheControl::Ephemeral5m | CacheControl::Ephemeral1h => None,
			};
			if let Some(prompt_cache_retention) = prompt_cache_retention {
				payload.x_insert("prompt_cache_retention", prompt_cache_retention)?;
			}
		}

		Ok(WebRequestData { url, headers, payload })
	}

	fn to_chat_response(
		model_iden: ModelIden,
		web_response: WebResponse,
		options_set: ChatOptionsSet<'_, '_>,
	) -> Result<ChatResponse> {
		let WebResponse { body, .. } = web_response;

		let captured_raw_body = options_set.capture_raw_body().unwrap_or_default().then(|| body.clone());

		let resp: RespResponse = serde_json::from_value(body)?;

		// -- Capture the provider_model_iden
		let provider_model_iden = model_iden.from_name(&resp.model);

		// -- Capture the usage
		let usage = resp.usage.map(Usage::from).unwrap_or_default();

		// -- Capture the content
		let mut content: MessageContent = MessageContent::default();
		let reasoning_content: Option<String> = None;

		// -- Extract the content message
		for output_item in resp.output {
			let parts = ContentPart::from_resp_output_item(output_item)?;
			content.extend(parts);
		}

		Ok(ChatResponse {
			content,
			reasoning_content,
			model_iden,
			provider_model_iden,
			stop_reason: Some(StopReason::from(resp.status)),
			usage,
			captured_raw_body,
			response_id: Some(resp.id),
		})
	}

	fn to_chat_stream(
		model_iden: ModelIden,
		reqwest_builder: RequestBuilder,
		options_sets: ChatOptionsSet<'_, '_>,
	) -> Result<ChatStreamResponse> {
		let event_source = EventSourceStream::new(reqwest_builder);
		let openai_stream = OpenAIRespStreamer::new(event_source, model_iden.clone(), options_sets);
		let chat_stream = ChatStream::from_inter_stream(openai_stream);

		Ok(ChatStreamResponse {
			model_iden,
			stream: chat_stream,
		})
	}

	fn to_embed_request_data(
		_service_target: ServiceTarget,
		_embed_req: crate::embed::EmbedRequest,
		_options_set: crate::embed::EmbedOptionsSet<'_, '_>,
	) -> Result<WebRequestData> {
		Err(crate::Error::AdapterNotSupported {
			adapter_kind: crate::adapter::AdapterKind::OpenAIResp,
			feature: "embeddings".to_string(),
		})
	}

	fn to_embed_response(
		_model_iden: ModelIden,
		_web_response: WebResponse,
		_options_set: crate::embed::EmbedOptionsSet<'_, '_>,
	) -> Result<crate::embed::EmbedResponse> {
		Err(crate::Error::AdapterNotSupported {
			adapter_kind: crate::adapter::AdapterKind::OpenAIResp,
			feature: "embeddings".to_string(),
		})
	}
}

/// Support functions for other adapters that share OpenAI APIs
impl OpenAIRespAdapter {
	pub(in crate::adapter::adapters) fn util_get_service_url(
		_model: &ModelIden,
		service_type: ServiceType,
		// -- utility arguments
		default_endpoint: Endpoint,
	) -> Result<String> {
		let base_url = default_endpoint.base_url();
		// Parse into URL and query-params
		let base_url = reqwest::Url::parse(base_url)
			.map_err(|err| Error::Internal(format!("Cannot parse url: {base_url}. Cause:\n{err}")))?;
		let original_query_params = base_url.query().to_owned();

		let suffix = match service_type {
			ServiceType::Chat | ServiceType::ChatStream => "responses",
			ServiceType::Embed => "embeddings", // TODO: Probably needs to say not supported
		};
		let mut full_url = base_url.join(suffix).map_err(|err| {
			Error::Internal(format!(
				"Cannot joing url suffix '{suffix}' for base_url '{base_url}'. Cause:\n{err}"
			))
		})?;
		full_url.set_query(original_query_params);
		Ok(full_url.to_string())
	}

	/// Takes the genai ChatMessages and builds the OpenAIChatRequestParts
	/// - `genai::ChatRequest.system`, if present, is added as the first message with role 'system'.
	/// - All messages get added with the corresponding roles (tools are not supported for now)
	///
	fn into_openai_request_parts(_model_iden: &ModelIden, chat_req: ChatRequest) -> Result<OpenAIRespRequestParts> {
		let mut input_items: Vec<Value> = Vec::new();

		// -- Process the system
		if let Some(system_msg) = chat_req.system {
			input_items.push(json!({"role": "system", "content": system_msg}));
		}

		let mut unamed_file_count = 0;

		// -- Process the messages
		for msg in chat_req.messages {
			// Note: Will handle more types later
			match msg.role {
				// For now, system and tool messages go to the system
				ChatRole::System => {
					if let Some(content) = msg.content.into_joined_texts() {
						input_items.push(json!({"role": "system", "content": content}))
					}
					// TODO: Probably need to warn if it is a ToolCalls type of content
				}

				// User - For now support Text and Binary
				ChatRole::User => {
					// -- If we have only text, then, we jjust returned the joined_texts
					if msg.content.is_text_only() {
						// NOTE: for now, if no content, just return empty string (respect current logic)
						let content = json!(msg.content.joined_texts().unwrap_or_else(String::new));
						input_items.push(json! ({"role": "user", "content": content}));
					} else {
						let mut values: Vec<Value> = Vec::new();

						for part in msg.content {
							match part {
								// -- Simple Text
								ContentPart::Text(content) => {
									values.push(json!({"type": "input_text", "text": content}))
								}
								// -- Binary
								ContentPart::Binary(mut binary) => {
									let is_image = binary.is_image();

									// Process the image
									if is_image {
										let image_url = binary.into_url();
										let input_image = json!({
											"type": "input_image",
											"detail": "auto",
											"image_url": image_url
										});
										values.push(input_image);
									}
									// Process file
									// TODO - Needs to support audio
									else {
										let mut input_file = Map::new();
										input_file.insert("type".into(), "input_file".into());

										// Set the file name if not defined (otherwise error)
										if let Some(file_name) = binary.name.take() {
											input_file.insert("filename".into(), file_name.into());
										} else {
											unamed_file_count += 1;
											input_file
												.insert("filename".into(), format!("file-{unamed_file_count}").into());
										}

										let file_url = binary.into_url();
										if file_url.starts_with("data") {
											input_file.insert("file_data".into(), file_url.into());
										} else {
											input_file.insert("file_url".into(), file_url.into());
										}
										let input_file: Value = input_file.into();

										values.push(input_file);
									}
								}

								// Use `match` instead of `if let`. This will allow to future-proof this
								// implementation in case some new message content types would appear,
								// this way library would not compile if not all methods are implemented
								// continue would allow to gracefully skip pushing unserializable message
								// TODO: Probably need to warn if it is a ToolCalls type of content
								ContentPart::ToolCall(_) => (),
								ContentPart::ToolResponse(_) => (),
								ContentPart::ThoughtSignature(_) => (),
								ContentPart::ReasoningContent(_) => (),
								// Custom are ignored for this logic
								ContentPart::Custom(_) => {}
							}
						}
						input_items.push(json! ({"role": "user", "content": values}));
					}
				}

				// Assistant - For now support Text and ToolCalls
				ChatRole::Assistant => {
					// Here we make sure if multiple text content part, we keep them in the same assistant message
					// In the new OpenAI Responses API, the tool call are just items out of assistant message
					let mut item_message_content: Vec<Value> = Vec::new();

					for part in msg.content {
						match part {
							ContentPart::Text(text) => {
								item_message_content.push(json!({
										"type": "output_text",
										"text": text
								}));
							}
							ContentPart::ToolCall(tool_call) => {
								// Make sure to create the assistant message
								if !item_message_content.is_empty() {
									input_items.push(json!({
										"type": "message",
										"role": "assistant",
										"content": item_message_content
									}));
									item_message_content = Vec::new();
								}
								// NOTE: Flatten for OpenAI Responsess API
								input_items.push(json!({
									"type": "function_call",
									"call_id": tool_call.call_id,
									"name": tool_call.fn_name,
									"arguments": tool_call.fn_arguments.to_string(),
								}))
							}

							// TODO: Probably need towarn on this one (probably need to add binary here)
							ContentPart::Binary(_) => {}
							ContentPart::ToolResponse(_) => {}
							ContentPart::ThoughtSignature(_) => {}
							ContentPart::ReasoningContent(_) => {}
							// Custom are ignored for this logic
							ContentPart::Custom(_) => {}
						}
					}

					// Make sure we handle the rest of the assistant message
					if !item_message_content.is_empty() {
						input_items.push(json!({
							"type": "message",
							"role": "assistant",
							"content": item_message_content
						}));
					}
				}

				// Tool Response (Function tool call output)
				ChatRole::Tool => {
					for part in msg.content {
						if let ContentPart::ToolResponse(tool_response) = part {
							input_items.push(json!({
								"type": "function_call_output",
								"call_id": tool_response.call_id,
								"output": tool_response.content,
							}))
						}
					}

					// TODO: Probably need to trace/warn that this will be ignored
				}
			}
		}

		// -- Process the tools
		let tools = chat_req
			.tools
			.map(|tools| tools.into_iter().map(Self::tool_to_openai_tool).collect::<Result<Vec<Value>>>())
			.transpose()?;

		Ok(OpenAIRespRequestParts { input_items, tools })
	}

	fn tool_to_openai_tool(tool: Tool) -> Result<Value> {
		let Tool {
			name,
			description,
			schema,
			strict,
			config,
			cache_control: _,
		} = tool;

		let name = match name {
			ToolName::WebSearch => "web_search".to_string(),
			ToolName::Custom(name) => name,
		};

		let tool_value = match name.as_ref() {
			"web_search" => {
				let mut tool_value = json!({"type": "web_search"});
				match config {
					Some(ToolConfig::WebSearch(_ws_config)) => {
						// FIXME: Implement what is posible in filters
					}
					Some(ToolConfig::Custom(config_value)) => {
						// IMPORTANT: Here like anthropic, we merge it on top of the toll value
						//            (and not as value of "name" as this would not fit that api)
						//            Gemini does a `{name: config}` which fit that API
						tool_value.x_merge(config_value)?;
					}
					None => (),
				};
				tool_value
			}
			name => {
				let strict = strict.unwrap_or(false);
				let mut parameters = schema;

				// When strict mode is enabled, OpenAI requires `additionalProperties: false`
				// on every object node in the schema.
				if strict && let Some(ref mut schema_val) = parameters {
					schema_val.x_walk(|parent_map, prop_name| {
						if prop_name == "type" {
							let typ = parent_map.get("type").and_then(|v| v.as_str()).unwrap_or("");
							if typ == "object" {
								parent_map.insert("additionalProperties".to_string(), false.into());
							}
						}
						true
					});
				}

				json!({
					"type": "function",
					"name": name,
					"description": description,
					"parameters": parameters,
					"strict": strict,
				})
			}
		};

		Ok(tool_value)
	}
}
// region:    --- Support

struct OpenAIRespRequestParts {
	input_items: Vec<Value>,
	tools: Option<Vec<Value>>,
}

// endregion: --- Support

// region:    --- Tests

#[cfg(test)]
mod tests {
	use super::*;
	use crate::adapter::AdapterKind;
	use crate::chat::ChatMessage;

	/// Test that assistant message text content uses "output_text" type (not "input_text").
	///
	/// This is required by OpenAI's Responses API - assistant content is model output,
	/// so it must use "output_text". Using "input_text" causes:
	/// "Invalid value: 'input_text'. Supported values are: 'output_text' and 'refusal'."
	#[test]
	fn test_assistant_message_uses_output_text_content_type() {
		let model_iden = ModelIden::new(AdapterKind::OpenAIResp, "gpt-5-codex");

		// Create a chat request with an assistant message
		let chat_req = ChatRequest::default()
			.with_system("You are a helpful assistant.")
			.append_message(ChatMessage::user("What's the weather?"))
			.append_message(ChatMessage::assistant("The weather is sunny."));

		// Serialize to OpenAI Responses API format
		let parts =
			OpenAIRespAdapter::into_openai_request_parts(&model_iden, chat_req).expect("Should serialize successfully");

		// Find the assistant message in input_items
		let assistant_msg = parts
			.input_items
			.iter()
			.find(|item| {
				item.get("type").and_then(|t| t.as_str()) == Some("message")
					&& item.get("role").and_then(|r| r.as_str()) == Some("assistant")
			})
			.expect("Should have an assistant message");

		// Check the content uses "output_text" type
		let content = assistant_msg
			.get("content")
			.and_then(|c| c.as_array())
			.expect("Assistant message should have content array");

		assert!(!content.is_empty(), "Content should not be empty");

		let first_content = &content[0];
		let content_type = first_content
			.get("type")
			.and_then(|t| t.as_str())
			.expect("Content should have a type");

		assert_eq!(
			content_type, "output_text",
			"Assistant message content should use 'output_text' type, not 'input_text'"
		);
	}
}

// endregion: --- Tests
