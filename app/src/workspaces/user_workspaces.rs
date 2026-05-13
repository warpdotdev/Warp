use super::{
    team::{MembershipRole, Team},
    workspace::{
        AdminEnablementSetting, CustomerType, EnterpriseSecretRegex, HostEnablementSetting,
        UgcCollectionEnablementSetting, Workspace, WorkspaceUid,
    },
};
use crate::{
    ai::llms::LLMModelHost,
    auth::{UserUid, TEST_USER_UID},
    channel::ChannelState,
    cloud_object::{
        model::persistence::ObjectStoreModel, ObjectType, Owner, Space, StoredObjectEventEntrypoint,
    },
    pricing::{PricingInfo, PricingInfoModel},
    report_error,
    server::{experiments::ServerExperiment, ids::ServerId},
    settings::{AISettings, PrivacySettings},
    workspaces::workspace::{
        AiAutonomySettings, AiOverages, SandboxedAgentSettings, UsageBasedPricingSettings,
    },
};
use anyhow::Result;
use regex::Regex;
use warp_core::{
    features::FeatureFlag,
    settings::{ChangeEventReason, Setting},
};
use warpui::{AppContext, Entity, ModelContext, SingletonEntity, Tracked};

#[cfg(test)]
use crate::workspaces::workspace::{
    AIAutonomyPolicy, BillingMetadata, WorkspaceMember, WorkspaceSettings,
};

#[cfg(test)]
use super::workspace::WorkspaceMemberUsageInfo;

#[derive(Debug)]
pub enum UserWorkspacesEvent {
    AddDomainRestrictionsSuccess,
    AddDomainRestrictionsRejected(anyhow::Error),
    DeleteDomainRestrictionSuccess,
    DeleteDomainRestrictionRejected(anyhow::Error),
    EmailInviteSent,
    EmailInviteRejected(anyhow::Error),
    ToggleInviteLinksSuccess,
    ToggleInviteLinksRejected(anyhow::Error),
    ResetInviteLinks,
    ResetInviteLinksRejected(anyhow::Error),
    DeleteTeamInvite,
    DeleteTeamInviteRejected(anyhow::Error),
    SetTeamMemberRoleSuccess,
    SetTeamMemberRoleRejected(anyhow::Error),
    UpdateWorkspaceSettingsSuccess,
    UpdateWorkspaceSettingsRejected(anyhow::Error),
    AiOveragesUpdated,
    PurchaseAddonCreditsSuccess,
    PurchaseAddonCreditsRejected(anyhow::Error),
    /// Fired whenever the set of teams the user is on changes.
    TeamsChanged,
    /// Fired when a service agreement's sunsetted_to_build_ts field is updated.
    SunsettedToBuildDataUpdated,
}

/// UserWorkspaces is a singleton model that holds workspace metadata (name, members, etc).
/// It should be used for getting information about the workspaces, teams, current teams,
/// and all other things related to operating on workspace and team data.
/// TODO: consolidate local SQLite refresh/update paths.
pub struct UserWorkspaces {
    current_workspace_uid: Tracked<Option<WorkspaceUid>>,
    workspaces: Tracked<Vec<Workspace>>,
}

/// Represents the workspaces a user potentially has access to.
#[derive(Clone)]
pub struct WorkspacesMetadataResponse {
    /// The list of workspaces the user is currently on.
    pub workspaces: Vec<Workspace>,
    /// The list of experiments applicable to the user.
    pub experiments: Option<Vec<ServerExperiment>>,
    /// TODO(Tyler): Post-workspaces, move this into the workspace object.
    /// Feature model choices may change from user to user and while the app is open, so we need to periodically update this list.
    /// It makes most sense to fetch this in workspaces which is queried every 10 minutes.
    /// This is list of available LLM models for the user.
    pub feature_model_choices: Option<()>,
}

// A representation of all workspace data refreshed together.
// Prefer adding to this struct if you need relatively fresh data vs making
// independent queries.
pub struct WorkspacesMetadataWithPricing {
    pub metadata: WorkspacesMetadataResponse,
    pub pricing_info: Option<PricingInfo>,
}

pub struct CreateTeamResponse {
    pub workspace: Workspace,
    pub team: Team,
}

impl UserWorkspaces {
    #[cfg(test)]
    pub fn mock(cached_workspaces: Vec<Workspace>, _ctx: &mut ModelContext<Self>) -> Self {
        Self {
            current_workspace_uid: cached_workspaces.first().map(|w| w.uid).into(),
            workspaces: cached_workspaces.into(),
        }
    }

    #[cfg(test)]
    pub fn default_mock(ctx: &mut ModelContext<Self>) -> Self {
        Self::mock(vec![], ctx)
    }

    pub fn new(
        cached_workspaces: Vec<Workspace>,
        current_workspace_uid: Option<WorkspaceUid>,
    ) -> Self {
        Self {
            current_workspace_uid: current_workspace_uid.into(),
            workspaces: cached_workspaces.into(),
        }
    }

    pub fn team_from_uid(&self, team_uid: ServerId) -> Option<&Team> {
        let _ = team_uid;
        None
    }

    pub fn team_from_uid_across_all_workspaces(&self, team_uid: ServerId) -> Option<&Team> {
        let _ = team_uid;
        None
    }

    pub fn workspace_from_uid(&self, workspace_uid: WorkspaceUid) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.uid == workspace_uid)
    }

    pub fn workspace_from_uid_mut(
        &mut self,
        workspace_uid: WorkspaceUid,
    ) -> Option<&mut Workspace> {
        self.workspaces.iter_mut().find(|w| w.uid == workspace_uid)
    }

    pub fn is_at_tier_limit_for_object_type(
        team_uid: ServerId,
        object_type: ObjectType,
        ctx: &AppContext,
    ) -> bool {
        match object_type {
            ObjectType::Notebook => {
                !UserWorkspaces::has_capacity_for_shared_notebooks(team_uid, ctx, 1)
            }
            ObjectType::Workflow => {
                !UserWorkspaces::has_capacity_for_shared_workflows(team_uid, ctx, 1)
            }
            ObjectType::Folder => false,
            ObjectType::GenericStringObject(_) => false,
        }
    }

    pub fn is_at_tier_limit_for_some_warp_drive_objects(
        team_uid: ServerId,
        ctx: &AppContext,
    ) -> bool {
        UserWorkspaces::is_at_tier_limit_for_object_type(team_uid, ObjectType::Notebook, ctx)
            || UserWorkspaces::is_at_tier_limit_for_object_type(team_uid, ObjectType::Workflow, ctx)
    }

    // Checks if the team has capacity for another shared notebook for their current
    // billing tier, given their current notebook count and delinquency status.
    pub fn has_capacity_for_shared_notebooks(
        team_uid: ServerId,
        ctx: &AppContext,
        new_shared_notebooks: usize,
    ) -> bool {
        let current_shared_notebooks = ObjectStoreModel::as_ref(ctx)
            .active_notebooks_in_space(Space::Team { team_uid }, ctx)
            .count();

        let team = UserWorkspaces::as_ref(ctx).team_from_uid(team_uid);
        if let Some(team) = team {
            // If the team is past due or unpaid, then don't allow new notebooks.
            if team.billing_metadata.is_delinquent_due_to_payment_issue() {
                return false;
            }

            if let Some(policy) = team.billing_metadata.tier.shared_notebooks_policy {
                // Allow new notebooks if policy is unlimited or if the number of notebooks
                // is less than the limit.
                policy.is_unlimited
                    || current_shared_notebooks + new_shared_notebooks
                        <= policy
                            .limit
                            .try_into()
                            .expect("shared notebooks limit should be within max i64 range")
            } else {
                // If no policy is set, then allow it to go through by default (should still be enforced server-side)
                true
            }
        } else {
            // If the team is not found, then allow it to go through by default (should still be enforced server-side)
            true
        }
    }

    // Checks if the team has capacity for another shared workflow for their current
    // billing tier, given their current workflow count and delinquency status.
    pub fn has_capacity_for_shared_workflows(
        team_uid: ServerId,
        ctx: &AppContext,
        new_shared_workflows: usize,
    ) -> bool {
        let current_shared_workflows = ObjectStoreModel::as_ref(ctx)
            .active_workflows_in_space(Space::Team { team_uid }, ctx)
            .count();

        let team = UserWorkspaces::as_ref(ctx).team_from_uid(team_uid);
        if let Some(team) = team {
            // If the team is past due or unpaid, then don't allow new workflows.
            if team.billing_metadata.is_delinquent_due_to_payment_issue() {
                return false;
            }

            if let Some(policy) = team.billing_metadata.tier.shared_workflows_policy {
                // Allow new workflows if policy is unlimited or if the number of workflows
                // is less than the limit.
                policy.is_unlimited
                    || current_shared_workflows + new_shared_workflows
                        <= policy
                            .limit
                            .try_into()
                            .expect("shared workflows limit should be within max i64 range")
            } else {
                // If no policy is set, then allow it to go through by default (should still be enforced server-side)
                true
            }
        } else {
            // If the team is not found, then allow it to go through by default (should still be enforced server-side)
            true
        }
    }

    /// Return the uid of user's current team (if any) without refreshing.
    pub fn current_team_uid(&self) -> Option<ServerId> {
        None
    }

    pub fn current_team_mut(&mut self) -> Option<&mut Team> {
        None
    }

    /// Note that the team is populated with dummy data until
    /// the initial fetch completes (only team name and ID are cached in sqlite locally).
    /// Consider whether you need to wait for the results of the fetch before checking the
    /// values of other fields.
    pub fn current_team(&self) -> Option<&Team> {
        None
    }

    /// Note that the workspace is populated with dummy data until the initial fetch
    /// completes (only workspace name/ID and workspace team's name/ID are cached in
    /// sqlite locally).
    /// Consider whether you need to wait for the results of the fetch before checking the
    /// values of other fields.
    pub fn current_workspace(&self) -> Option<&Workspace> {
        self.current_workspace_uid
            .and_then(|workspace_uid| self.workspace_from_uid(workspace_uid))
    }

    pub fn current_workspace_mut(&mut self) -> Option<&mut Workspace> {
        self.current_workspace_uid
            .and_then(|workspace_uid| self.workspace_from_uid_mut(workspace_uid))
    }

    pub fn workspaces(&self) -> &Vec<Workspace> {
        &self.workspaces
    }

    pub fn set_current_workspace_uid(
        &mut self,
        workspace_uid: WorkspaceUid,
        ctx: &mut ModelContext<Self>,
    ) {
        *self.current_workspace_uid = Some(workspace_uid);
        self.notify_and_emit_teams_changed(ctx);
    }

    /// Returns `true` if active AI is allowed for the current workspace, based on billing config.
    ///
    /// In the future, we should store active AI enablement on the policy directly. For now, we
    /// proxy whether active AI by checking if prompt suggestions, next command, or code suggestions are enabled.
    pub fn is_active_ai_allowed(&self) -> bool {
        self.current_team().is_none_or(|team| {
            team.billing_metadata
                .tier
                .warp_ai_policy
                .is_none_or(|policy| {
                    policy.is_prompt_suggestions_toggleable
                        || policy.is_next_command_enabled
                        || policy.is_code_suggestions_toggleable
                })
        })
    }

    /// Returns `true` if the current team's enterprise status allows AI features that have an
    /// enterprise gate. Non-enterprise teams always pass; enterprise teams pass only if they
    /// are on the Warp Plan or the build is dogfood (both our internal Warp team and dogfood
    /// team are billed as enterprise).
    pub fn ai_allowed_for_current_team(&self) -> bool {
        !self
            .current_team()
            .is_some_and(|team| team.billing_metadata.customer_type == CustomerType::Enterprise)
            || self
                .current_team()
                .is_some_and(|team| team.billing_metadata.is_warp_plan())
            || ChannelState::channel().is_dogfood()
    }

    /// Whether Prompt Suggestions should be toggleable for the current user, based on the active policies.
    /// Note that the value may be incorrect if called before the team's billing metadata has been fetched.
    pub fn is_prompt_suggestions_toggleable(&self) -> bool {
        self.current_team()
            // If the user has no team, they can toggle prompt suggestions (no restrictions).
            .is_none_or(|team| {
                team.billing_metadata
                    .tier
                    .warp_ai_policy
                    .is_some_and(|policy| policy.is_prompt_suggestions_toggleable)
            })
    }

    /// Whether Code Suggestions should be toggleable for the current user, based on the active policies.
    /// Note that the value may be incorrect if called before the team's billing metadata has been fetched.
    pub fn is_code_suggestions_toggleable(&self) -> bool {
        self.current_team()
            // If the user has no team, they can toggle code suggestions (no restrictions).
            .is_none_or(|team| {
                team.billing_metadata
                    .tier
                    .warp_ai_policy
                    .is_some_and(|policy| policy.is_code_suggestions_toggleable)
            })
    }

    /// Whether Next Command should be toggleable for the current user, based on the active policies.
    /// Note that the value may be incorrect if called before the team's billing metadata has been fetched.
    pub fn is_next_command_enabled(&self) -> bool {
        self.current_team()
            // If the user has no team, they can toggle Next Command (no restrictions).
            .is_none_or(|team| {
                team.billing_metadata
                    .tier
                    .warp_ai_policy
                    .is_some_and(|policy| policy.is_next_command_enabled)
            })
    }

    /// Whether voice input should be toggleable for the current user, based on the active policies.
    /// Note that the value may be incorrect if called before the team's billing metadata has been fetched.
    /// If voice input support is not compiled into this build, always returns `false`.
    pub fn is_voice_enabled(&self) -> bool {
        cfg!(feature = "voice_input")
            && self
                .current_team()
                // If the user has no team, they can toggle Voice (no restrictions).
                .is_none_or(|team| {
                    team.billing_metadata
                        .tier
                        .warp_ai_policy
                        .is_some_and(|policy| policy.is_voice_enabled)
                })
    }

    /// Whether BYO API key is enabled for the current user, based on the active policies.
    /// Note that the value may be incorrect if called before the team's billing metadata has been fetched.
    /// For solo users (no workspace), this is controlled by the `SoloUserByok` feature flag.
    pub fn is_byo_api_key_enabled(&self) -> bool {
        true
    }

    pub fn aws_bedrock_host_settings(&self) -> Option<&super::workspace::LlmHostSettings> {
        self.current_workspace().and_then(|workspace| {
            workspace
                .settings
                .llm_settings
                .host_configs
                .get(&LLMModelHost::AwsBedrock)
        })
    }

    /// Did the admin enable AWS Bedrock for the current workspace?
    pub fn is_aws_bedrock_available_from_workspace(&self) -> bool {
        self.current_workspace().is_some_and(|workspace| {
            workspace.settings.llm_settings.enabled
                && self
                    .aws_bedrock_host_settings()
                    .is_some_and(|settings| settings.enabled)
        })
    }
    pub fn aws_bedrock_host_enablement_setting(&self) -> HostEnablementSetting {
        self.aws_bedrock_host_settings()
            .map(|settings| settings.enablement_setting.clone())
            .unwrap_or_default()
    }

    pub fn is_aws_bedrock_credentials_toggleable(&self) -> bool {
        matches!(
            self.aws_bedrock_host_enablement_setting(),
            HostEnablementSetting::RespectUserSetting
        )
    }

    pub fn is_aws_bedrock_credentials_enabled(&self, app: &AppContext) -> bool {
        // i.e. did the admin go and toggle on aws bedrock in the admin panel?
        if !self.is_aws_bedrock_available_from_workspace() {
            return false;
        }

        match self.aws_bedrock_host_enablement_setting() {
            HostEnablementSetting::Enforce => true,
            HostEnablementSetting::RespectUserSetting => *AISettings::as_ref(app)
                .aws_bedrock_credentials_enabled
                .value(),
        }
    }

    /// Returns the AI autonomy settings that are enforced by the workspace for all its members.
    /// If a setting is `None`, the workspace doesn't enforce a particular setting.
    pub fn ai_autonomy_settings(&self) -> AiAutonomySettings {
        self.current_team()
            .map(|team| team.organization_settings.ai_autonomy_settings.clone())
            .unwrap_or_default()
    }

    /// Returns the sandboxed agent settings enforced by the workspace, if any.
    pub fn sandboxed_agent_settings(&self) -> Option<SandboxedAgentSettings> {
        self.current_team()
            .and_then(|team| team.organization_settings.sandboxed_agent_settings.clone())
    }

    /// Returns true iff AI autonomy features are allowed for this client.
    /// TODO: This should be deleted soon. AI autonomy settings have been moved into organization
    /// settings (see `ai_autonomy_settings` above), but there could be an interim time where we
    /// have not set up the org settings yet for an enterprise that previously had the entire
    /// feature set disabled. To capture that case, we'll see if all the settings are `None`;
    /// if so, we'll fall back to their billing metadata's value. Once we've migrated everyone
    /// into org settings, we should remove `is_enabled` from the policy and delete this function.
    pub fn is_ai_autonomy_allowed(&self) -> bool {
        self.current_team().is_none_or(|team| {
            let settings = &team.organization_settings.ai_autonomy_settings;
            let all_settings_none = settings.apply_code_diffs_setting.is_none()
                && settings.read_files_setting.is_none()
                && settings.read_files_allowlist.is_none()
                && settings.execute_commands_setting.is_none()
                && settings.execute_commands_allowlist.is_none()
                && settings.execute_commands_denylist.is_none();

            if all_settings_none {
                team.billing_metadata
                    .tier
                    .ai_autonomy_policy
                    .is_some_and(|policy| policy.is_enabled)
            } else {
                true
            }
        })
    }

    // OpenWarp:团队空间是云端协作入口,本地版不暴露任何 Team space。
    pub fn team_spaces(&self) -> Vec<Space> {
        vec![]
    }

    // OpenWarp:Drive 只保留本地 Personal space。Team / Shared 都是云端协作面,
    // 即使旧缓存里还有 workspace metadata,也不能重新进入 Drive 或 Workflow UI。
    pub fn all_user_spaces(&self, ctx: &AppContext) -> Vec<Space> {
        let _ = ctx;
        vec![Space::Personal]
    }

    // OpenWarp(本地化分支)个人空间 owner 固定绑到本地占位用户。
    // 必须保持稳定,否则重启后旧对象 owner 字段对不上,Personal Space 列表里"看不见"旧数据。
    fn effective_personal_user_uid() -> UserUid {
        UserUid::new(TEST_USER_UID)
    }

    // Returns the [`Owner`] for the user's personal drive.
    // OpenWarp:Drive Personal 空间下的 Workflow / EnvVar / Folder / Notebook / Import
    // 等 Create 动作统一归属本地占位用户(只本地 sqlite 持久化)。
    pub fn personal_drive(&self, ctx: &AppContext) -> Option<Owner> {
        let _ = ctx;
        Some(Owner::User {
            user_uid: Self::effective_personal_user_uid(),
        })
    }

    // Maps a [`Space`] into an [`Owner`], based on the user's team memberships. If the space
    // does not directly identify an owner (it's the space for shared objects), returns `None`.
    pub fn space_to_owner(&self, space: Space, ctx: &AppContext) -> Option<Owner> {
        match space {
            Space::Team { .. } => None,
            Space::Personal => self.personal_drive(ctx),
            Space::Shared => None,
        }
    }

    // Maps an [`Owner`] into a [`Space`], based on the user's team memberships.
    // This is always possible, as unknown owners imply the shared space.
    pub fn owner_to_space(&self, owner: Owner, ctx: &AppContext) -> Space {
        let _ = ctx;
        match owner {
            Owner::User { user_uid } => {
                if !FeatureFlag::SharedWithMe.is_enabled() {
                    return Space::Personal;
                }

                // OpenWarp:用 effective_personal_user_uid 比较,确保无 auth 下
                // 本地 Owner(user_uid="openwarp")也归到 Personal 而非 Shared。
                if user_uid == Self::effective_personal_user_uid() {
                    Space::Personal
                } else {
                    Space::Shared
                }
            }
            Owner::Team { .. } => Space::Shared,
        }
    }

    pub fn has_teams(&self) -> bool {
        false
    }

    pub fn has_workspaces(&self) -> bool {
        !self.workspaces.is_empty()
    }

    pub fn update_workspaces(&mut self, workspaces: Vec<Workspace>, ctx: &mut ModelContext<Self>) {
        // Check if sunsetted_to_build_ts changed for any workspace
        let sunsetted_to_build_changed = self.has_sunsetted_to_build_data_changed(&workspaces);

        *self.workspaces = workspaces;
        self.notify_and_emit_teams_changed(ctx);

        if sunsetted_to_build_changed {
            ctx.emit(UserWorkspacesEvent::SunsettedToBuildDataUpdated);
        }
    }

    /// Checks if any workspace's service agreement sunsetted_to_build_ts field has changed.
    fn has_sunsetted_to_build_data_changed(&self, new_workspaces: &[Workspace]) -> bool {
        for new_workspace in new_workspaces {
            // Find the corresponding old workspace
            let old_workspace = self.workspaces.iter().find(|w| w.uid == new_workspace.uid);

            if let Some(old_workspace) = old_workspace {
                // Check if any team's service agreement sunsetted_to_build_ts changed
                for new_team in &new_workspace.teams {
                    let old_team = old_workspace.teams.iter().find(|t| t.uid == new_team.uid);

                    if let Some(old_team) = old_team {
                        let old_sunsetted = old_team
                            .billing_metadata
                            .service_agreements
                            .first()
                            .and_then(|sa| sa.sunsetted_to_build_ts);

                        let new_sunsetted = new_team
                            .billing_metadata
                            .service_agreements
                            .first()
                            .and_then(|sa| sa.sunsetted_to_build_ts);

                        // Detect if it changed from None to Some or changed value
                        if old_sunsetted != new_sunsetted {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn notify_and_emit_teams_changed(&self, ctx: &mut ModelContext<Self>) {
        // PrivacySettings can't observe UserWorkspaces for updates, as it's initialized too early in
        // the app initialization flow. So, we update it manually whenever teams data changes.
        PrivacySettings::handle(ctx).update(ctx, |settings, ctx| {
            settings.set_is_telemetry_force_enabled(self.is_telemetry_force_enabled());
            settings.set_enterprise_secret_redaction_settings(
                self.is_enterprise_secret_redaction_enabled(),
                self.get_enterprise_secret_redaction_regex_list(),
                ChangeEventReason::CloudSync,
                ctx,
            );
        });

        ctx.emit(UserWorkspacesEvent::TeamsChanged);
        ctx.notify();
    }

    fn on_workspaces_updated(
        &mut self,
        result: Result<WorkspacesMetadataWithPricing>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Ok(response) => {
                if let Some(pricing_info) = response.pricing_info {
                    PricingInfoModel::handle(ctx).update(ctx, |model, ctx| {
                        model.update_pricing_info(pricing_info, ctx);
                    });
                }

                let workspaces = response.metadata.workspaces;

                self.update_workspaces(workspaces.clone(), ctx);

                // Check if the current workspace is still in the list of workspaces.
                // If it's not, then set the current workspace to the first workspace in the list.
                if let Some(current_workspace) = self.current_workspace() {
                    if !self
                        .workspaces
                        .iter()
                        .any(|w| w.uid == current_workspace.uid)
                    {
                        if let Some(workspace_uid) = workspaces.first().map(|w| w.uid) {
                            self.set_current_workspace_uid(workspace_uid, ctx);
                        }
                    }
                } else if let Some(workspace_uid) = workspaces.first().map(|w| w.uid) {
                    self.set_current_workspace_uid(workspace_uid, ctx);
                }
            }
            Err(e) => {
                report_error!(e.context("Failed to load user workspaces"));
            }
        }
    }

    pub fn team_created(
        &mut self,
        create_team_response: &CreateTeamResponse,
        ctx: &mut ModelContext<Self>,
    ) {
        self.workspaces.push(create_team_response.workspace.clone());
        self.set_current_workspace_uid(create_team_response.workspace.uid, ctx);
        self.notify_and_emit_teams_changed(ctx);
    }

    pub fn remove_user_from_team(
        &mut self,
        user_uid: UserUid,
        team_uid: ServerId,
        entrypoint: StoredObjectEventEntrypoint,
        _ctx: &mut ModelContext<Self>,
    ) {
        // OpenWarp(本地化):移除成员路径在本地无远端 team 写入目标 → no-op。
        let _ = (user_uid, team_uid, entrypoint);
    }

    fn on_add_invite_link_domain_restrictions(
        &mut self,
        result: Result<WorkspacesMetadataWithPricing>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Err(err) => ctx.emit(UserWorkspacesEvent::AddDomainRestrictionsRejected(err)),
            Ok(result) => {
                self.on_workspaces_updated(Ok(result), ctx);
                ctx.emit(UserWorkspacesEvent::AddDomainRestrictionsSuccess);
            }
        };
        ctx.notify();
    }

    pub fn add_invite_link_domain_restrictions(
        &mut self,
        team_uid: ServerId,
        domains: Vec<String>,
        ctx: &mut ModelContext<Self>,
    ) {
        // OpenWarp(本地化):域限制路径在本地无远端 team/invite 写入目标 → 发 Success 事件使 UI 不卡住。
        let _ = (team_uid, domains);
        ctx.emit(UserWorkspacesEvent::AddDomainRestrictionsSuccess);
        ctx.notify();
    }

    fn on_delete_invite_link_domain_restriction(
        &mut self,
        result: Result<WorkspacesMetadataWithPricing>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Err(err) => ctx.emit(UserWorkspacesEvent::DeleteDomainRestrictionRejected(err)),
            Ok(result) => {
                self.on_workspaces_updated(Ok(result), ctx);
                ctx.emit(UserWorkspacesEvent::DeleteDomainRestrictionSuccess);
            }
        };
        ctx.notify();
    }

    pub fn delete_invite_link_domain_restriction(
        &mut self,
        team_uid: ServerId,
        domain_uid: ServerId,
        ctx: &mut ModelContext<Self>,
    ) {
        let _ = (team_uid, domain_uid);
        ctx.emit(UserWorkspacesEvent::DeleteDomainRestrictionSuccess);
        ctx.notify();
    }

    fn on_email_invite_sent(
        &mut self,
        result: Result<WorkspacesMetadataWithPricing>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Err(err) => ctx.emit(UserWorkspacesEvent::EmailInviteRejected(err)),
            Ok(result) => {
                self.on_workspaces_updated(Ok(result), ctx);
                ctx.emit(UserWorkspacesEvent::EmailInviteSent);
            }
        };
        ctx.notify();
    }

    pub fn send_email_invites(
        &mut self,
        team_uid: ServerId,
        emails: Vec<String>,
        ctx: &mut ModelContext<Self>,
    ) {
        let _ = (team_uid, emails);
        ctx.emit(UserWorkspacesEvent::EmailInviteSent);
        ctx.notify();
    }

    pub fn on_is_invite_link_enabled_set(
        &mut self,
        result: Result<WorkspacesMetadataWithPricing>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Err(err) => ctx.emit(UserWorkspacesEvent::ToggleInviteLinksRejected(err)),
            Ok(result) => {
                self.on_workspaces_updated(Ok(result), ctx);
                ctx.emit(UserWorkspacesEvent::ToggleInviteLinksSuccess);
            }
        };
        ctx.notify();
    }

    pub fn set_is_invite_link_enabled(
        &mut self,
        team_uid: ServerId,
        new_value: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        let _ = (team_uid, new_value);
        ctx.emit(UserWorkspacesEvent::ToggleInviteLinksSuccess);
        ctx.notify();
    }

    pub fn on_invite_links_reset(
        &mut self,
        result: Result<WorkspacesMetadataWithPricing>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Err(err) => ctx.emit(UserWorkspacesEvent::ResetInviteLinksRejected(err)),
            Ok(result) => {
                self.on_workspaces_updated(Ok(result), ctx);
                ctx.emit(UserWorkspacesEvent::ResetInviteLinks);
            }
        };
        ctx.notify();
    }

    pub fn reset_invite_links(&mut self, team_uid: ServerId, ctx: &mut ModelContext<Self>) {
        let _ = team_uid;
        ctx.emit(UserWorkspacesEvent::ResetInviteLinks);
        ctx.notify();
    }

    fn on_team_member_role_set(
        &mut self,
        result: Result<WorkspacesMetadataWithPricing>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Err(err) => ctx.emit(UserWorkspacesEvent::SetTeamMemberRoleRejected(err)),
            Ok(result) => {
                self.on_workspaces_updated(Ok(result), ctx);
                ctx.emit(UserWorkspacesEvent::SetTeamMemberRoleSuccess);
            }
        };
        ctx.notify();
    }

    pub fn set_team_member_role(
        &mut self,
        user_uid: UserUid,
        team_uid: ServerId,
        role: MembershipRole,
        ctx: &mut ModelContext<Self>,
    ) {
        let _ = (user_uid, team_uid, role);
        ctx.emit(UserWorkspacesEvent::SetTeamMemberRoleSuccess);
        ctx.notify();
    }

    pub fn on_delete_team_invite(
        &mut self,
        result: Result<WorkspacesMetadataWithPricing>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Err(err) => ctx.emit(UserWorkspacesEvent::DeleteTeamInviteRejected(err)),
            Ok(result) => {
                self.on_workspaces_updated(Ok(result), ctx);
                ctx.emit(UserWorkspacesEvent::DeleteTeamInvite);
            }
        };
        ctx.notify();
    }

    pub fn delete_team_invite(
        &mut self,
        team_uid: ServerId,
        invitee_email: String,
        ctx: &mut ModelContext<Self>,
    ) {
        let _ = (team_uid, invitee_email);
        ctx.emit(UserWorkspacesEvent::DeleteTeamInvite);
        ctx.notify();
    }

    pub fn update_usage_based_pricing_settings(
        &mut self,
        team_uid: ServerId,
        usage_based_pricing_enabled: bool,
        max_monthly_spend_cents: Option<u32>,
        ctx: &mut ModelContext<Self>,
    ) {
        let _ = (
            team_uid,
            usage_based_pricing_enabled,
            max_monthly_spend_cents,
        );
        ctx.emit(UserWorkspacesEvent::UpdateWorkspaceSettingsSuccess);
        ctx.notify();
    }

    fn on_update_workspace_metadata(
        &mut self,
        result: Result<WorkspacesMetadataResponse>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Ok(result) => {
                let wrapped = WorkspacesMetadataWithPricing {
                    metadata: result,
                    pricing_info: None,
                };
                self.on_workspaces_updated(Ok(wrapped), ctx);
                ctx.emit(UserWorkspacesEvent::UpdateWorkspaceSettingsSuccess);
            }
            Err(err) => {
                let err_for_event = anyhow::anyhow!("{}", err);
                self.on_workspaces_updated(Err(err), ctx);
                ctx.emit(UserWorkspacesEvent::UpdateWorkspaceSettingsRejected(
                    err_for_event,
                ));
            }
        };
        ctx.notify();
    }

    pub fn purchase_addon_credits(
        &mut self,
        team_uid: ServerId,
        credits: i32,
        ctx: &mut ModelContext<Self>,
    ) {
        let _ = (team_uid, credits);
        ctx.emit(UserWorkspacesEvent::PurchaseAddonCreditsSuccess);
        ctx.notify();
    }

    fn on_purchase_addon_credits(
        &mut self,
        result: Result<WorkspacesMetadataResponse>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Ok(result) => {
                let wrapped = WorkspacesMetadataWithPricing {
                    metadata: result,
                    pricing_info: None,
                };
                self.on_workspaces_updated(Ok(wrapped), ctx);
                ctx.emit(UserWorkspacesEvent::PurchaseAddonCreditsSuccess);
            }
            Err(err) => {
                ctx.emit(UserWorkspacesEvent::PurchaseAddonCreditsRejected(
                    anyhow::anyhow!(err),
                ));
            }
        };
        ctx.notify();
    }

    pub fn refresh_ai_overages(&mut self, _ctx: &mut ModelContext<Self>) {
        // OpenWarp(本地化,Phase 5):本地无云端 AI overages 查询,no-op。
        // 调用点 (`blocklist/controller.rs::maybe_refresh_ai_overages`) UI 不发起有意义的更新。
    }

    pub fn update_addon_credits_settings(
        &mut self,
        team_uid: ServerId,
        auto_reload_enabled: Option<bool>,
        max_monthly_spend_cents: Option<i32>,
        selected_auto_reload_credit_denomination: Option<i32>,
        ctx: &mut ModelContext<Self>,
    ) {
        let _ = (
            team_uid,
            auto_reload_enabled,
            max_monthly_spend_cents,
            selected_auto_reload_credit_denomination,
        );
        ctx.emit(UserWorkspacesEvent::UpdateWorkspaceSettingsSuccess);
        ctx.notify();
    }

    fn on_refresh_ai_overages(&mut self, result: Result<AiOverages>, ctx: &mut ModelContext<Self>) {
        match result {
            Ok(fresh_ai_overages) => {
                // TODO: We really need to stop having duplicate billing metadata...
                if let Some(workspace) = self.current_workspace_mut() {
                    workspace.billing_metadata.ai_overages = Some(fresh_ai_overages.clone());
                }
                if let Some(team) = self.current_team_mut() {
                    team.billing_metadata.ai_overages = Some(fresh_ai_overages);
                }

                ctx.emit(UserWorkspacesEvent::AiOveragesUpdated);
                ctx.notify();
            }
            Err(e) => {
                log::warn!("Failed to refresh AI overages for workspace: {e:?}");
            }
        }
    }

    pub fn usage_based_pricing_settings(&self) -> UsageBasedPricingSettings {
        self.current_workspace()
            .map(|workspace| workspace.settings.usage_based_pricing_settings.clone())
            .unwrap_or_default()
    }

    pub fn is_telemetry_force_enabled(&self) -> bool {
        self.current_team()
            .map(|team| team.organization_settings.telemetry_settings.force_enabled)
            .unwrap_or(false)
    }

    pub fn is_enterprise_secret_redaction_enabled(&self) -> bool {
        self.current_team()
            .map(|team| team.organization_settings.secret_redaction_settings.enabled)
            .unwrap_or(false)
    }

    pub fn get_enterprise_secret_redaction_regex_list(&self) -> Vec<EnterpriseSecretRegex> {
        self.current_team()
            .map(|team| {
                team.organization_settings
                    .secret_redaction_settings
                    .regexes
                    .clone()
            })
            .unwrap_or_default()
    }

    pub fn get_ugc_collection_enablement_setting(&self) -> UgcCollectionEnablementSetting {
        self.current_team()
            .map(|team| {
                team.organization_settings
                    .ugc_collection_settings
                    .setting
                    .clone()
            })
            .unwrap_or_default()
    }

    pub fn is_ai_allowed_in_remote_sessions(&self) -> bool {
        self.current_team()
            .map(|team| {
                team.organization_settings
                    .ai_permissions_settings
                    .allow_ai_in_remote_sessions
            })
            .unwrap_or(true)
    }

    pub fn get_remote_session_regex_list(&self) -> Vec<Regex> {
        self.current_team()
            .map(|team| {
                team.organization_settings
                    .ai_permissions_settings
                    .remote_session_regex_list
                    .clone()
            })
            .unwrap_or_default()
    }

    pub fn is_anyone_with_link_sharing_enabled(&self) -> bool {
        self.current_team()
            .map(|team| {
                team.organization_settings
                    .link_sharing_settings
                    .anyone_with_link_sharing_enabled
            })
            .unwrap_or(true)
    }

    pub fn is_direct_link_sharing_enabled(&self) -> bool {
        self.current_team()
            .map(|team| {
                team.organization_settings
                    .link_sharing_settings
                    .direct_link_sharing_enabled
            })
            .unwrap_or(true)
    }

    /// Returns the team-level agent attribution setting.
    ///
    /// Use this to decide whether the user's attribution toggle should be locked
    /// (`Enable`/`Disable`) or editable (`RespectUserSetting`).
    pub fn get_agent_attribution_setting(&self) -> AdminEnablementSetting {
        self.current_team()
            .map(|team| team.organization_settings.enable_warp_attribution.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
impl UserWorkspaces {
    /// Creates a test workspace with a team and sets it as the current workspace.
    /// Returns the workspace UID and admin UID for use in tests.
    pub fn setup_test_workspace(&mut self, ctx: &mut ModelContext<Self>) {
        let workspace_uid = WorkspaceUid::from(ServerId::from(1));
        let owner_uid = UserUid::new("test_owner");

        let workspace_settings = WorkspaceSettings::default();

        let workspace = Workspace {
            uid: workspace_uid,
            name: "Test Workspace".to_string(),
            stripe_customer_id: None,
            teams: vec![Team {
                uid: ServerId::from(2),
                name: "Test Team".to_string(),
                organization_settings: workspace_settings.clone(),
                billing_metadata: BillingMetadata::default(),
                members: vec![],
                invite_code: None,
                pending_email_invites: vec![],
                invite_link_domain_restrictions: vec![],
                stripe_customer_id: None,
                is_eligible_for_discovery: false,
                has_billing_history: false,
            }],
            members: vec![WorkspaceMember {
                uid: owner_uid,
                email: "test@example.com".to_string(),
                role: MembershipRole::Owner,
                usage_info: WorkspaceMemberUsageInfo {
                    requests_used_since_last_refresh: 0,
                    request_limit: 1000,
                    is_unlimited: false,
                    is_request_limit_prorated: false,
                },
            }],
            billing_metadata: BillingMetadata::default(),
            bonus_grants_purchased_this_month: Default::default(),
            has_billing_history: false,
            settings: workspace_settings,
            invite_code: None,
            invite_link_domain_restrictions: vec![],
            pending_email_invites: vec![],
            is_eligible_for_discovery: false,
            total_requests_used_since_last_refresh: 0,
        };

        self.update_workspaces(vec![workspace], ctx);
        self.set_current_workspace_uid(workspace_uid, ctx);
    }

    /// Updates the current workspace by applying a mutation function.
    pub fn update_current_workspace<F>(&mut self, f: F, ctx: &mut ModelContext<Self>)
    where
        F: FnOnce(&mut Workspace),
    {
        if let Some(workspace) = self.current_workspace() {
            if workspace.teams.is_empty() {
                panic!("No team found in current workspace. Did you call setup_test_workspace()?");
            }

            let mut new_workspace = workspace.clone();
            f(&mut new_workspace);

            self.update_workspaces(vec![new_workspace], ctx);
        } else {
            panic!("No workspace found. Did you call setup_test_workspace()?");
        }
    }

    pub fn update_sandboxed_agent_settings<F>(&mut self, f: F, ctx: &mut ModelContext<Self>)
    where
        F: FnOnce(&mut Option<SandboxedAgentSettings>),
    {
        self.update_current_workspace(
            |workspace| {
                if let Some(team) = workspace.teams.first_mut() {
                    f(&mut team.organization_settings.sandboxed_agent_settings);
                } else {
                    panic!(
                        "No team found in current workspace. Did you call setup_test_workspace()?"
                    );
                }
            },
            ctx,
        );
    }

    pub fn update_ai_autonomy_settings<F>(&mut self, f: F, ctx: &mut ModelContext<Self>)
    where
        F: FnOnce(&mut AiAutonomySettings),
    {
        self.update_current_workspace(
            |workspace| {
                if let Some(team) = workspace.teams.first_mut() {
                    f(&mut team.organization_settings.ai_autonomy_settings);
                } else {
                    panic!(
                        "No team found in current workspace. Did you call setup_test_workspace()?"
                    );
                }
            },
            ctx,
        );
    }

    pub fn update_ai_autonomy_policy_flag(&mut self, enabled: bool, ctx: &mut ModelContext<Self>) {
        self.update_current_workspace(
            |workspace| {
                if let Some(team) = workspace.teams.first_mut() {
                    team.billing_metadata.tier.ai_autonomy_policy = Some(AIAutonomyPolicy {
                        is_enabled: enabled,
                        toggleable: true,
                    });
                } else {
                    panic!(
                        "No team found in current workspace. Did you call setup_test_workspace()?"
                    );
                }
            },
            ctx,
        );
    }
}

impl Entity for UserWorkspaces {
    type Event = UserWorkspacesEvent;
}

/// Mark UserWorkspaces as global application state.
impl SingletonEntity for UserWorkspaces {}

// OpenWarp(本地化,Phase 5):`user_workspaces_tests.rs` 全部针对 team RPC 路径(`MockTeamClient` / `mockall::Sequence`),
// 本地化后这些路径不可达，整文件物理删除。
