#[cfg(test)]
mod tests {
    use crate::ai::mcp::templatable_manager::utils::{should_query_resources, should_query_tools};
    use rmcp::model::ServerCapabilities;

    /// Regression test for warpdotdev/warp#6798: each capability is queried
    /// independently. Previously, asymmetric handling could cause `tools/list`
    /// to be skipped when a server advertised both `tools` and `resources`,
    /// resulting in "No tools available" even though the server had tools.
    #[test]
    fn each_capability_is_queried_independently() {
        let both = ServerCapabilities {
            tools: Some(Default::default()),
            resources: Some(Default::default()),
            ..Default::default()
        };
        assert!(should_query_tools(Some(&both)));
        assert!(should_query_resources(Some(&both)));

        let tools_only = ServerCapabilities {
            tools: Some(Default::default()),
            resources: None,
            ..Default::default()
        };
        assert!(should_query_tools(Some(&tools_only)));
        assert!(!should_query_resources(Some(&tools_only)));

        let resources_only = ServerCapabilities {
            tools: None,
            resources: Some(Default::default()),
            ..Default::default()
        };
        assert!(!should_query_tools(Some(&resources_only)));
        assert!(should_query_resources(Some(&resources_only)));

        let neither = ServerCapabilities {
            tools: None,
            resources: None,
            ..Default::default()
        };
        assert!(!should_query_tools(Some(&neither)));
        assert!(!should_query_resources(Some(&neither)));

        assert!(!should_query_tools(None));
        assert!(!should_query_resources(None));
    }
}
