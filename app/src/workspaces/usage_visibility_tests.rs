use super::*;
use crate::auth::UserUid;
use crate::server::ids::ServerId;
use crate::workspaces::team::MembershipRole;
use crate::workspaces::workspace::{
    MaxPriorCycles, UsageVisibilityGranularity, UsageVisibilityPolicy, Workspace, WorkspaceMember,
    WorkspaceMemberUsageInfo,
};

const ADMIN_EMAIL: &str = "alice@acme.com";
const USER_EMAIL: &str = "bob@acme.com";
const NONMEMBER_EMAIL: &str = "carol@elsewhere.com";

fn make_member(email: &str, role: MembershipRole) -> WorkspaceMember {
    WorkspaceMember {
        uid: UserUid::new(email),
        email: email.to_string(),
        role,
        usage_info: WorkspaceMemberUsageInfo {
            is_unlimited: false,
            request_limit: 1500,
            requests_used_since_last_refresh: 0,
            is_request_limit_prorated: false,
        },
    }
}

// `ServerId::from_string_lossy` requires exactly 22 characters.
const TEST_WORKSPACE_UID: &str = "workspace_uid123456789";

fn make_workspace(
    policy: Option<UsageVisibilityPolicy>,
    members: Vec<WorkspaceMember>,
) -> Workspace {
    let mut workspace = Workspace::from_local_cache(
        ServerId::from_string_lossy(TEST_WORKSPACE_UID).into(),
        "Test Workspace".to_string(),
        None,
    );
    workspace.billing_metadata.tier.usage_visibility_policy = policy;
    workspace.members = members;
    workspace
}

fn policy(
    granularity: UsageVisibilityGranularity,
    max_prior_cycles: MaxPriorCycles,
) -> UsageVisibilityPolicy {
    UsageVisibilityPolicy {
        admin_granularity: granularity,
        max_prior_cycles,
    }
}

#[test]
fn missing_policy_falls_back_to_own_only() {
    let workspace = make_workspace(None, vec![make_member(ADMIN_EMAIL, MembershipRole::Owner)]);

    let resolved = resolve_usage_visibility(&workspace, Some(ADMIN_EMAIL));

    assert_eq!(resolved.granularity, UsageVisibilityGranularity::OwnOnly);
    assert_eq!(resolved.max_prior_cycles, MaxPriorCycles::None);
}

#[test]
fn missing_viewer_email_collapses_to_own_only_but_keeps_history() {
    let workspace = make_workspace(
        Some(policy(
            UsageVisibilityGranularity::FullBreakdown,
            MaxPriorCycles::Unlimited,
        )),
        vec![make_member(ADMIN_EMAIL, MembershipRole::Owner)],
    );

    let resolved = resolve_usage_visibility(&workspace, None);

    assert_eq!(resolved.granularity, UsageVisibilityGranularity::OwnOnly);
    assert_eq!(resolved.max_prior_cycles, MaxPriorCycles::Unlimited);
}

#[test]
fn non_member_email_collapses_to_own_only() {
    let workspace = make_workspace(
        Some(policy(
            UsageVisibilityGranularity::FullBreakdown,
            MaxPriorCycles::Limited(11),
        )),
        vec![make_member(ADMIN_EMAIL, MembershipRole::Owner)],
    );

    let resolved = resolve_usage_visibility(&workspace, Some(NONMEMBER_EMAIL));

    assert_eq!(resolved.granularity, UsageVisibilityGranularity::OwnOnly);
    assert_eq!(resolved.max_prior_cycles, MaxPriorCycles::Limited(11));
}

#[test]
fn free_user_resolves_to_own_only_no_history() {
    let workspace = make_workspace(
        Some(policy(
            UsageVisibilityGranularity::OwnOnly,
            MaxPriorCycles::None,
        )),
        vec![make_member(USER_EMAIL, MembershipRole::User)],
    );

    let resolved = resolve_usage_visibility(&workspace, Some(USER_EMAIL));

    assert_eq!(resolved.granularity, UsageVisibilityGranularity::OwnOnly);
    assert_eq!(resolved.max_prior_cycles, MaxPriorCycles::None);
}

#[test]
fn build_admin_resolves_to_team_aggregate_with_history() {
    let workspace = make_workspace(
        Some(policy(
            UsageVisibilityGranularity::TeamAggregate,
            MaxPriorCycles::Limited(11),
        )),
        vec![
            make_member(ADMIN_EMAIL, MembershipRole::Owner),
            make_member(USER_EMAIL, MembershipRole::User),
        ],
    );

    let resolved = resolve_usage_visibility(&workspace, Some(ADMIN_EMAIL));

    assert_eq!(
        resolved.granularity,
        UsageVisibilityGranularity::TeamAggregate
    );
    assert_eq!(resolved.max_prior_cycles, MaxPriorCycles::Limited(11));
}

#[test]
fn build_non_admin_collapses_to_own_only_but_keeps_history() {
    let workspace = make_workspace(
        Some(policy(
            UsageVisibilityGranularity::TeamAggregate,
            MaxPriorCycles::Limited(11),
        )),
        vec![
            make_member(ADMIN_EMAIL, MembershipRole::Owner),
            make_member(USER_EMAIL, MembershipRole::User),
        ],
    );

    let resolved = resolve_usage_visibility(&workspace, Some(USER_EMAIL));

    assert_eq!(resolved.granularity, UsageVisibilityGranularity::OwnOnly);
    assert_eq!(resolved.max_prior_cycles, MaxPriorCycles::Limited(11));
}

#[test]
fn build_business_admin_resolves_to_per_user_totals_unlimited() {
    let workspace = make_workspace(
        Some(policy(
            UsageVisibilityGranularity::PerUserTotals,
            MaxPriorCycles::Unlimited,
        )),
        vec![make_member(ADMIN_EMAIL, MembershipRole::Owner)],
    );

    let resolved = resolve_usage_visibility(&workspace, Some(ADMIN_EMAIL));

    assert_eq!(
        resolved.granularity,
        UsageVisibilityGranularity::PerUserTotals
    );
    assert_eq!(resolved.max_prior_cycles, MaxPriorCycles::Unlimited);
}

#[test]
fn build_business_non_admin_collapses_to_own_only_unlimited() {
    let workspace = make_workspace(
        Some(policy(
            UsageVisibilityGranularity::PerUserTotals,
            MaxPriorCycles::Unlimited,
        )),
        vec![
            make_member(ADMIN_EMAIL, MembershipRole::Owner),
            make_member(USER_EMAIL, MembershipRole::User),
        ],
    );

    let resolved = resolve_usage_visibility(&workspace, Some(USER_EMAIL));

    assert_eq!(resolved.granularity, UsageVisibilityGranularity::OwnOnly);
    assert_eq!(resolved.max_prior_cycles, MaxPriorCycles::Unlimited);
}

#[test]
fn enterprise_admin_resolves_to_full_breakdown_unlimited() {
    let workspace = make_workspace(
        Some(policy(
            UsageVisibilityGranularity::FullBreakdown,
            MaxPriorCycles::Unlimited,
        )),
        vec![make_member(ADMIN_EMAIL, MembershipRole::Owner)],
    );

    let resolved = resolve_usage_visibility(&workspace, Some(ADMIN_EMAIL));

    assert_eq!(
        resolved.granularity,
        UsageVisibilityGranularity::FullBreakdown
    );
    assert_eq!(resolved.max_prior_cycles, MaxPriorCycles::Unlimited);
}

#[test]
fn admin_role_without_owner_role_still_gets_admin_granularity() {
    // `Workspace::is_workspace_admin` returns true for both `Owner` and
    // `Admin` (it matches the server's `HasAdminLevelPermissions`). Test
    // that an `Admin` viewer gets the admin granularity even when no
    // multi-admin policy is configured.
    let workspace = make_workspace(
        Some(policy(
            UsageVisibilityGranularity::TeamAggregate,
            MaxPriorCycles::Limited(11),
        )),
        vec![make_member(ADMIN_EMAIL, MembershipRole::Admin)],
    );

    let resolved = resolve_usage_visibility(&workspace, Some(ADMIN_EMAIL));

    assert_eq!(
        resolved.granularity,
        UsageVisibilityGranularity::TeamAggregate
    );
    assert_eq!(resolved.max_prior_cycles, MaxPriorCycles::Limited(11));
}
