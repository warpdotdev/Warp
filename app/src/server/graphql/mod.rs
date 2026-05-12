pub mod schema;

use warp_graphql::client::RequestOptions;

/// Returns the default [`RequestOptions`] that should be used for a GraphQL request.
pub fn default_request_options() -> RequestOptions {
    RequestOptions {
        #[cfg(feature = "agent_mode_evals")]
        path_prefix: Some("/agent-mode-evals".to_string()),
        ..Default::default()
    }
}
