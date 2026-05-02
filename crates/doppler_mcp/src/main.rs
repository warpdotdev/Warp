// SPDX-License-Identifier: AGPL-3.0-only
//
// Binary entry point for the Doppler metadata MCP server (stdio transport).
// Log output goes to stderr so it doesn't interfere with MCP JSON-RPC framing.
//
// Wire up in a Claude / Warp agent config:
//   { "mcpServers": { "doppler": { "command": "doppler-mcp" } } }

use std::sync::Arc;

use doppler::TokioCommandRunner;
use doppler_mcp::{DopplerMcpServer, MetadataClient};
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("doppler-mcp starting");

    let metadata = Arc::new(MetadataClient::with_runner(Arc::new(TokioCommandRunner)));
    let service = DopplerMcpServer::new(metadata)
        .serve(stdio())
        .await
        .inspect_err(|e| tracing::error!("server error: {e:?}"))?;

    service.waiting().await?;
    Ok(())
}
