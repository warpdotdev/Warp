// SPDX-License-Identifier: AGPL-3.0-only
//
// doppler_mcp — MCP server exposing Doppler project/config/secret-name
// metadata.  Raw secret values are never accessible through this crate.

pub mod metadata;
pub mod server;

pub use metadata::MetadataClient;
pub use server::DopplerMcpServer;
