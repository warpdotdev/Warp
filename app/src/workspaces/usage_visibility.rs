use super::workspace::{UsageVisibility, UsageVisibilityGranularity, Workspace};

// `dead_code` is suppressed until the UI scaffold PR consumes this function;
// the unit tests below exercise it but the lint only counts non-test usage.
#[allow(dead_code)]
pub fn resolve_usage_visibility(
    workspace: &Workspace,
    viewer_email: Option<&str>,
) -> UsageVisibility {
    let Some(policy) = workspace.billing_metadata.tier.usage_visibility_policy else {
        return UsageVisibility::default();
    };

    let is_admin = viewer_email.is_some_and(|email| workspace.is_workspace_admin(email));

    UsageVisibility {
        granularity: if is_admin {
            policy.admin_granularity
        } else {
            UsageVisibilityGranularity::OwnOnly
        },
        max_prior_cycles: policy.max_prior_cycles,
    }
}

#[cfg(test)]
#[path = "usage_visibility_tests.rs"]
mod tests;
