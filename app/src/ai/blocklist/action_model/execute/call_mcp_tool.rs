use super::{ActionExecution, AnyActionExecution, ExecuteActionInput, PreprocessActionInput};
use crate::terminal::model::session::active_session::ActiveSession;
use futures::{future::BoxFuture, FutureExt};
use warpui::{Entity, EntityId, ModelContext, ModelHandle};

#[cfg(not(target_family = "wasm"))]
use super::get_server_output_id;
#[cfg(not(target_family = "wasm"))]
use crate::{
    ai::{
        agent::{AIAgentAction, AIAgentActionResultType, CallMCPToolResult},
        blocklist::{action_model::AIAgentActionType, BlocklistAIPermissions},
        mcp::TemplatableMCPServerManager,
    },
    send_telemetry_from_app_ctx, TelemetryEvent,
};
#[cfg(not(target_family = "wasm"))]
use itertools::Itertools;
#[cfg(not(target_family = "wasm"))]
use warpui::SingletonEntity;

pub struct CallMCPToolExecutor {
    _active_session: ModelHandle<ActiveSession>,
    #[allow(dead_code)]
    terminal_view_id: EntityId,
}

impl CallMCPToolExecutor {
    pub fn new(_active_session: ModelHandle<ActiveSession>, terminal_view_id: EntityId) -> Self {
        Self {
            _active_session,
            terminal_view_id,
        }
    }

    #[cfg_attr(target_family = "wasm", allow(unused_variables), allow(dead_code))]
    pub(super) fn should_autoexecute(
        &self,
        input: ExecuteActionInput,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        #[cfg(target_family = "wasm")]
        {
            false
        }

        #[cfg(not(target_family = "wasm"))]
        {
            let ExecuteActionInput {
                action:
                    AIAgentAction {
                        action:
                            AIAgentActionType::CallMCPTool {
                                server_id, name, ..
                            },
                        ..
                    },
                conversation_id,
            } = input
            else {
                return false;
            };

            BlocklistAIPermissions::as_ref(ctx).can_call_mcp_tool(
                server_id.as_ref(),
                name.as_str(),
                &conversation_id,
                Some(self.terminal_view_id),
                ctx,
            )
        }
    }

    #[cfg_attr(target_family = "wasm", allow(unused_variables), allow(dead_code))]
    pub(super) fn execute(
        &mut self,
        input: ExecuteActionInput,
        ctx: &mut ModelContext<Self>,
    ) -> impl Into<AnyActionExecution> {
        #[cfg(target_family = "wasm")]
        {
            ActionExecution::<()>::InvalidAction
        }

        #[cfg(not(target_family = "wasm"))]
        {
            let server_output_id = get_server_output_id(input.conversation_id, ctx);
            let AIAgentAction {
                action:
                    AIAgentActionType::CallMCPTool {
                        server_id,
                        name,
                        input,
                    },
                ..
            } = input.action
            else {
                return ActionExecution::InvalidAction;
            };

            let name_owned = name.to_owned();
            let name_clone = name_owned.clone();

            let serde_json::Value::Object(mut arguments) = input.clone() else {
                return ActionExecution::Sync(AIAgentActionResultType::CallMCPTool(
                    CallMCPToolResult::Error("MCP server tool input not an object".to_owned()),
                ));
            };

            // Prefer the templatable server over the legacy server if both exist.
            // It is possible for both to exist in some tricky race conditions, but in those cases
            // we shouldn't care about the legacy servers.
            let templatable_mcp_manager = TemplatableMCPServerManager::as_ref(ctx);

            // Coerce whole-number f64 args to i64 for fields declared as `"type": "integer"`
            // in the tool's input schema. MCP tool args round-trip through
            // `google.protobuf.Struct` on the wire, which erases the integer/float distinction
            // by storing everything as f64. Without coercion, the ryu formatter serializes
            // whole-number f64 as "5.0", which strict MCP servers (e.g. GoLand) reject for
            // integer-typed fields.
            if let Some(schema) =
                templatable_mcp_manager.tool_input_schema(*server_id, name.as_str())
            {
                coerce_integer_args(&mut arguments, &schema);
            }

            let templatable_peer = if let Some(installation_id) = server_id {
                templatable_mcp_manager
                    .server_with_installation_id_and_tool_name(*installation_id, name.to_owned())
            } else {
                templatable_mcp_manager.server_with_tool_name(name.to_owned())
            };

            let Some(reconnecting_peer) = templatable_peer else {
                return ActionExecution::Sync(AIAgentActionResultType::CallMCPTool(
                    CallMCPToolResult::Error("MCP server for tool not found".to_owned()),
                ));
            };

            let name_owned_inner = name_owned.clone();
            ActionExecution::new_async(
                async move {
                    reconnecting_peer
                        .call_tool(rmcp::model::CallToolRequestParam {
                            name: name_owned_inner.into(),
                            arguments: Some(arguments),
                        })
                        .await
                },
                move |res, ctx| handle_call_tool_result(res, server_output_id, name_clone, ctx),
            )
        }
    }

    pub(super) fn preprocess_action(
        &mut self,
        _action: PreprocessActionInput,
        _ctx: &mut ModelContext<Self>,
    ) -> BoxFuture<'static, ()> {
        futures::future::ready(()).boxed()
    }
}

impl Entity for CallMCPToolExecutor {
    type Event = ();
}

/// Coerces whole-number floats in `args` to integers for fields declared as
/// [`"type": "integer"`](https://json-schema.org/understanding-json-schema/reference/type)
/// in the tool's JSON Schema `input_schema`.
///
/// MCP tool args round-trip through `google.protobuf.Struct` on the wire, whose
/// `NumberValue` stores everything as `f64`. Without this fix, serde_json emits
/// whole-number floats as `"5.0"`, which strict MCP servers reject for integer fields.
pub(crate) fn coerce_integer_args(
    args: &mut serde_json::Map<String, serde_json::Value>,
    input_schema: &serde_json::Map<String, serde_json::Value>,
) {
    let schema = serde_json::Value::Object(input_schema.clone());
    let mut value = serde_json::Value::Object(std::mem::take(args));
    coerce_integer_value(&mut value, &schema, &schema, &mut Vec::new());

    if let serde_json::Value::Object(coerced_args) = value {
        *args = coerced_args;
    }
}

fn coerce_integer_value(
    value: &mut serde_json::Value,
    schema: &serde_json::Value,
    root_schema: &serde_json::Value,
    ref_stack: &mut Vec<String>,
) {
    if let Some(ref_path) = schema.get("$ref").and_then(|ref_path| ref_path.as_str()) {
        if ref_path.starts_with('#') && !ref_stack.iter().any(|seen| seen == ref_path) {
            ref_stack.push(ref_path.to_string());
            if let Some(resolved_schema) =
                root_schema.pointer(ref_path.strip_prefix('#').unwrap_or_default())
            {
                coerce_integer_value(value, resolved_schema, root_schema, ref_stack);
            }
            ref_stack.pop();
        }
    }

    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(schemas) = schema.get(keyword).and_then(|schemas| schemas.as_array()) {
            for nested_schema in schemas {
                coerce_integer_value(value, nested_schema, root_schema, ref_stack);
            }
        }
    }

    if schema_declares_integer(schema) {
        coerce_number_to_integer(value);
    }

    match value {
        serde_json::Value::Object(object) => {
            if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
                for (key, property_schema) in properties {
                    if let Some(property_value) = object.get_mut(key) {
                        coerce_integer_value(
                            property_value,
                            property_schema,
                            root_schema,
                            ref_stack,
                        );
                    }
                }
            }

            if let Some(additional_properties) = schema.get("additionalProperties") {
                if additional_properties.is_object() {
                    for property_value in object.values_mut() {
                        coerce_integer_value(
                            property_value,
                            additional_properties,
                            root_schema,
                            ref_stack,
                        );
                    }
                }
            }
        }
        serde_json::Value::Array(items) => {
            if let Some(items_schema) = schema.get("items") {
                for item in items {
                    coerce_integer_value(item, items_schema, root_schema, ref_stack);
                }
            }
        }
        _ => {}
    }
}

fn schema_declares_integer(schema: &serde_json::Value) -> bool {
    match schema.get("type") {
        Some(serde_json::Value::String(schema_type)) => schema_type == "integer",
        Some(serde_json::Value::Array(schema_types)) => schema_types
            .iter()
            .any(|schema_type| schema_type.as_str() == Some("integer")),
        _ => false,
    }
}

fn coerce_number_to_integer(value: &mut serde_json::Value) {
    let serde_json::Value::Number(number) = value else {
        return;
    };
    let Some(float) = number.as_f64() else {
        return;
    };
    if !float.is_finite() || float.fract() != 0.0 {
        return;
    }
    if float < i64::MIN as f64 || float > i64::MAX as f64 {
        return;
    }

    *number = serde_json::Number::from(float as i64);
}

#[cfg(test)]
#[path = "call_mcp_tool_tests.rs"]
mod tests;

/// Handles the result of a call_tool request, converting it to an AIAgentActionResultType.
#[cfg(not(target_family = "wasm"))]
fn handle_call_tool_result(
    res: Result<rmcp::model::CallToolResult, rmcp::ServiceError>,
    server_output_id: Option<crate::ai::blocklist::action_model::execute::ServerOutputId>,
    tool_name: String,
    ctx: &warpui::AppContext,
) -> AIAgentActionResultType {
    let action_result = match res {
        Ok(result) => {
            // Even if the call was successful, the response could still be an error so we need to check.
            if matches!(result.is_error, Some(true)) {
                let error_message = result
                    .structured_content
                    .map(|content| content.to_string())
                    .unwrap_or_else(|| {
                        let content_str = result
                            .content
                            .into_iter()
                            .filter_map(|content| {
                                use rmcp::model::RawContent::*;
                                if let Text(raw_text_content) = content.raw {
                                    Some(raw_text_content.text)
                                } else {
                                    log::warn!("Error content found unsupported content type");
                                    None
                                }
                            })
                            .collect_vec()
                            .join("\n");
                        if content_str.is_empty() {
                            "MCP tool call returned an error.".to_string()
                        } else {
                            content_str
                        }
                    });
                send_telemetry_from_app_ctx!(
                    TelemetryEvent::MCPToolCallAccepted {
                        server_output_id,
                        tool_call: tool_name,
                        error: Some(
                            crate::server::telemetry::MCPServerTelemetryError::ResponseError(
                                error_message.clone()
                            )
                        ),
                    },
                    ctx
                );
                CallMCPToolResult::Error(error_message)
            } else {
                send_telemetry_from_app_ctx!(
                    TelemetryEvent::MCPToolCallAccepted {
                        server_output_id,
                        tool_call: tool_name,
                        error: None,
                    },
                    ctx
                );
                CallMCPToolResult::Success { result }
            }
        }
        Err(e) => {
            let error_message = e.to_string();
            log::warn!("Executing MCP tool resulted in error: {e:?}");
            send_telemetry_from_app_ctx!(
                TelemetryEvent::MCPToolCallAccepted {
                    server_output_id,
                    tool_call: tool_name,
                    error: Some(rmcp::RmcpError::Service(e).into()),
                },
                ctx
            );
            CallMCPToolResult::Error(error_message)
        }
    };
    AIAgentActionResultType::CallMCPTool(action_result)
}
