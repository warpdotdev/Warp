//! Small helpers shared across the templatable_manager submodules.
//!
//! Currently this is just the capability-gating predicates used during
//! MCP server startup. They live here (rather than inline in `native.rs`)
//! so unit tests can exercise the gating decision in isolation.

/// Whether to query `resources/list` for a server with the given capabilities.
///
/// Per the MCP spec, the client should only invoke a list method when the server
/// has advertised the corresponding capability during initialization.
pub(super) fn should_query_resources(
    capabilities: Option<&rmcp::model::ServerCapabilities>,
) -> bool {
    capabilities.is_some_and(|c| c.resources.is_some())
}

/// Whether to query `tools/list` for a server with the given capabilities.
///
/// Per the MCP spec, the client should only invoke a list method when the server
/// has advertised the corresponding capability during initialization.
pub(super) fn should_query_tools(capabilities: Option<&rmcp::model::ServerCapabilities>) -> bool {
    capabilities.is_some_and(|c| c.tools.is_some())
}
