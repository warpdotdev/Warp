// SPDX-License-Identifier: AGPL-3.0-only
//
// MCP server handler exposing Doppler project/config/secret-name metadata.
//
// SAFETY: No tool in this file ever returns a secret value.

use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::metadata::MetadataClient;

// ── Parameter structs ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ProjectParam {
    /// Doppler project slug (e.g. `"my-backend"`).
    pub project: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SecretNamesParam {
    /// Doppler project slug.
    pub project: String,
    /// Config name within the project (e.g. `"dev"`, `"prd"`).
    pub config: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct HasSecretParam {
    /// Doppler project slug.
    pub project: String,
    /// Config name within the project.
    pub config: String,
    /// Secret key name to check.
    pub name: String,
}

// ── Server ────────────────────────────────────────────────────────────────

/// MCP server exposing Doppler *metadata only* (never raw secret values).
#[derive(Clone)]
pub struct DopplerMcpServer {
    metadata: Arc<MetadataClient>,
    tool_router: ToolRouter<Self>,
}

impl DopplerMcpServer {
    pub fn new(metadata: Arc<MetadataClient>) -> Self {
        Self { metadata, tool_router: Self::tool_router() }
    }
}

// ── Tools ─────────────────────────────────────────────────────────────────

#[tool_router]
impl DopplerMcpServer {
    /// List all Doppler projects (name, slug, description). Never returns secret values.
    #[tool(description = "List all Doppler projects (name, slug, description). Never returns secret values.")]
    async fn list_projects(&self) -> Result<CallToolResult, McpError> {
        let projects = self.metadata.list_projects().await.map_err(doppler_to_mcp)?;
        let json = serde_json::to_string_pretty(&projects)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// List configs (environments / branches) for a Doppler project. Never returns secret values.
    #[tool(description = "List all configs (environments) for a Doppler project. Never returns secret values.")]
    async fn list_configs(
        &self,
        Parameters(ProjectParam { project }): Parameters<ProjectParam>,
    ) -> Result<CallToolResult, McpError> {
        let configs = self.metadata.list_configs(&project).await.map_err(doppler_to_mcp)?;
        let json = serde_json::to_string_pretty(&configs)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// List secret key names in a Doppler project+config. Uses `doppler secrets names` — values are never fetched or returned.
    #[tool(description = "List secret key names in a Doppler project+config. Uses `doppler secrets names` — values are never fetched or returned.")]
    async fn list_secret_names(
        &self,
        Parameters(SecretNamesParam { project, config }): Parameters<SecretNamesParam>,
    ) -> Result<CallToolResult, McpError> {
        let names = self
            .metadata
            .list_secret_names(&project, &config)
            .await
            .map_err(doppler_to_mcp)?;
        let json = serde_json::to_string_pretty(&names)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Check whether a secret key exists in a Doppler project+config. Returns {"exists": bool}. Never returns the secret value.
    #[tool(description = "Check whether a secret key exists in a Doppler project+config. Returns {\"exists\": bool}. Never returns the secret value.")]
    async fn has_secret(
        &self,
        Parameters(HasSecretParam { project, config, name }): Parameters<HasSecretParam>,
    ) -> Result<CallToolResult, McpError> {
        let exists = self
            .metadata
            .has_secret(&project, &config, &name)
            .await
            .map_err(doppler_to_mcp)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({ "exists": exists }).to_string(),
        )]))
    }
}

// ── ServerHandler ─────────────────────────────────────────────────────────

#[tool_handler]
impl ServerHandler for DopplerMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "doppler-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some(
                "Doppler metadata MCP server. \
                 Lists projects, configs, and secret key names. \
                 Secret values are never accessible through this server."
                    .to_string(),
            ),
        }
    }
}

// ── Error mapping ─────────────────────────────────────────────────────────

fn doppler_to_mcp(e: doppler::DopplerError) -> McpError {
    use doppler::DopplerError::*;
    let msg = match e {
        NotInstalled { install_hint } => format!("doppler CLI not found — {install_hint}"),
        NotAuthenticated => "doppler is not authenticated; run `doppler login`".into(),
        NoProjectBound => "no doppler project/config bound; run `doppler setup`".into(),
        KeyMissing(name) => format!("doppler secret not found: {name}"),
        Unreachable => "doppler API unreachable — check network".into(),
        Spawn(io) => format!("failed to spawn doppler: {io}"),
        NonZeroExit { code, stderr } => format!("doppler exited with code {code}: {stderr}"),
    };
    McpError::internal_error(msg, None)
}
