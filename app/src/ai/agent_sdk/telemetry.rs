use crate::features::FeatureFlag;
use serde_json::{json, Value};
use strum_macros::{EnumDiscriminants, EnumIter};
use warp_core::telemetry::{EnablementState, TelemetryEvent, TelemetryEventDesc};

#[derive(Debug, EnumDiscriminants)]
#[strum_discriminants(derive(EnumIter))]
pub(super) enum CliTelemetryEvent {
    /// Executing `warp agent run`
    AgentRun {
        gui: bool,
        requested_mcp_servers: usize,
        has_environment: bool,
        /// Optional task ID when running against an ambient agent task.
        task_id: Option<String>,
        /// Which execution harness was selected (e.g. "oz", "claude").
        harness: String,
    },
    /// Executing `warp agent run-ambient`
    AgentRunAmbient,
    /// Executing `warp agent profile list`
    AgentProfileList,
    /// Executing `warp agent list`
    AgentList,
    /// Executing `warp environment list`
    EnvironmentList,
    /// Executing `warp environment create`
    EnvironmentCreate,
    /// Executing `warp environment delete`
    EnvironmentDelete,
    /// Executing `warp environment update`
    EnvironmentUpdate,
    /// Executing `warp environment get`
    EnvironmentGet,
    /// Executing `warp environment image list`
    EnvironmentImageList,
    /// Executing `warp mcp list`
    MCPList,
    /// Executing `warp model list`
    ModelList,
    /// Executing `warp task list`
    TaskList,
    /// Executing `warp task get`
    TaskGet,
    /// Executing `warp run conversation get`
    ConversationGet,
    /// Executing `warp run get <id> --conversation`
    RunConversationGet,
    /// Executing `warp run message watch`
    RunMessageWatch { harness: &'static str },
    /// Executing `warp run message send`
    RunMessageSend { harness: &'static str },
    /// Executing `warp run message list`
    RunMessageList { harness: &'static str },
    /// Executing `warp run message read`
    RunMessageRead { harness: &'static str },
    /// Executing `warp run message mark-delivered`
    RunMessageMarkDelivered { harness: &'static str },
    /// Executing `warp login`
    Login,
    /// Executing `warp logout`
    Logout,
    /// Executing `warp whoami`
    Whoami,
    /// Executing `warp provider setup`
    ProviderSetup,
    /// Executing `warp provider list`
    ProviderList,
    /// Executing `warp integration create`
    IntegrationCreate,
    /// Executing `warp integration update`
    IntegrationUpdate,
    /// Executing `warp integration list`
    IntegrationList,
    /// Executing `warp artifact upload`
    ArtifactUpload,
    /// Executing `warp artifact get`
    ArtifactGet,
    /// Executing `warp artifact download`
    ArtifactDownload,
    /// Executing `warp schedule create`
    ScheduleCreate,
    /// Executing `warp schedule list`
    ScheduleList,
    /// Executing `warp schedule get`
    ScheduleGet,
    /// Executing `warp schedule pause`
    SchedulePause,
    /// Executing `warp schedule unpause`
    ScheduleUnpause,
    /// Executing `warp schedule update`
    ScheduleUpdate,
    /// Executing `warp schedule delete`
    ScheduleDelete,
    /// Executing `warp secret create`
    SecretCreate,
    /// Executing `warp secret delete`
    SecretDelete,
    /// Executing `warp secret update`
    SecretUpdate,
    /// Executing `warp secret list`
    SecretList,
    /// Executing `warp federate issue-token`
    FederateIssueToken,
    /// Executing `warp federate issue-gcp-token`
    FederateIssueGcpToken,
    /// Executing `warp harness-support ping`
    HarnessSupportPing,
    /// Executing `warp harness-support report-artifact`
    HarnessSupportReportArtifact { artifact_type: &'static str },
    /// Executing `warp harness-support notify-user`
    HarnessSupportNotifyUser,
    /// Executing `warp harness-support finish-task`
    HarnessSupportFinishTask { success: bool },
}

impl TelemetryEvent for CliTelemetryEvent {
    fn name(&self) -> &'static str {
        CliTelemetryEventDiscriminants::from(self).name()
    }

    fn payload(&self) -> Option<Value> {
        match self {
            Self::AgentRun {
                gui,
                requested_mcp_servers,
                has_environment,
                task_id,
                harness,
            } => Some(json!({
                "gui": gui,
                "requested_mcp_servers": requested_mcp_servers,
                "has_environment": has_environment,
                "task_id": task_id,
                "harness": harness,
            })),
            Self::AgentRunAmbient => None,
            Self::AgentProfileList => None,
            Self::AgentList => None,
            Self::EnvironmentList => None,
            Self::EnvironmentCreate => None,
            Self::EnvironmentDelete => None,
            Self::EnvironmentUpdate => None,
            Self::EnvironmentGet => None,
            Self::EnvironmentImageList => None,
            Self::MCPList => None,
            Self::ModelList => None,
            Self::TaskList => None,
            Self::TaskGet => None,
            Self::ConversationGet => None,
            Self::RunConversationGet => None,
            Self::RunMessageWatch { harness } => Some(json!({ "harness": harness })),
            Self::RunMessageSend { harness } => Some(json!({ "harness": harness })),
            Self::RunMessageList { harness } => Some(json!({ "harness": harness })),
            Self::RunMessageRead { harness } => Some(json!({ "harness": harness })),
            Self::RunMessageMarkDelivered { harness } => Some(json!({ "harness": harness })),
            Self::Login => None,
            Self::Logout => None,
            Self::Whoami => None,
            Self::ProviderSetup => None,
            Self::ProviderList => None,
            Self::IntegrationCreate => None,
            Self::IntegrationUpdate => None,
            Self::IntegrationList => None,
            Self::ArtifactUpload => None,
            Self::ArtifactGet => None,
            Self::ArtifactDownload => None,
            Self::ScheduleCreate => None,
            Self::ScheduleList => None,
            Self::ScheduleGet => None,
            Self::SchedulePause => None,
            Self::ScheduleUnpause => None,
            Self::ScheduleUpdate => None,
            Self::ScheduleDelete => None,
            Self::SecretCreate => None,
            Self::SecretDelete => None,
            Self::SecretUpdate => None,
            Self::SecretList => None,
            Self::FederateIssueToken => None,
            Self::FederateIssueGcpToken => None,
            Self::HarnessSupportPing => None,
            Self::HarnessSupportReportArtifact { artifact_type } => {
                Some(json!({ "artifact_type": artifact_type }))
            }
            Self::HarnessSupportNotifyUser => None,
            Self::HarnessSupportFinishTask { success } => Some(json!({ "success": success })),
        }
    }

    fn description(&self) -> &'static str {
        CliTelemetryEventDiscriminants::from(self).description()
    }

    fn enablement_state(&self) -> EnablementState {
        CliTelemetryEventDiscriminants::from(self).enablement_state()
    }

    fn contains_ugc(&self) -> bool {
        false
    }

    fn event_descs() -> impl Iterator<Item = Box<dyn TelemetryEventDesc>> {
        warp_core::telemetry::enum_events::<Self>()
    }
}

impl TelemetryEventDesc for CliTelemetryEventDiscriminants {
    fn name(&self) -> &'static str {
        match self {
            Self::AgentRun => "CLI.Execute.Agent.Run",
            Self::AgentRunAmbient => "CLI.Execute.Agent.RunAmbient",
            Self::AgentProfileList => "CLI.Execute.Agent.Profile.List",
            Self::AgentList => "CLI.Execute.Agent.List",
            Self::EnvironmentList => "CLI.Execute.Environment.List",
            Self::EnvironmentCreate => "CLI.Execute.Environment.Create",
            Self::EnvironmentDelete => "CLI.Execute.Environment.Delete",
            Self::EnvironmentUpdate => "CLI.Execute.Environment.Update",
            Self::EnvironmentGet => "CLI.Execute.Environment.Get",
            Self::EnvironmentImageList => "CLI.Execute.Environment.Image.List",
            Self::MCPList => "CLI.Execute.MCP.List",
            Self::ModelList => "CLI.Execute.Model.List",
            Self::TaskList => "CLI.Execute.Task.List",
            Self::TaskGet => "CLI.Execute.Task.Get",
            Self::ConversationGet => "CLI.Execute.Conversation.Get",
            Self::RunConversationGet => "CLI.Execute.Run.Conversation.Get",
            Self::RunMessageWatch => "CLI.Execute.Run.Message.Watch",
            Self::RunMessageSend => "CLI.Execute.Run.Message.Send",
            Self::RunMessageList => "CLI.Execute.Run.Message.List",
            Self::RunMessageRead => "CLI.Execute.Run.Message.Read",
            Self::RunMessageMarkDelivered => "CLI.Execute.Run.Message.MarkDelivered",
            Self::Login => "CLI.Execute.Login",
            Self::Logout => "CLI.Execute.Logout",
            Self::Whoami => "CLI.Execute.Whoami",
            Self::ProviderSetup => "CLI.Execute.Provider.Setup",
            Self::ProviderList => "CLI.Execute.Provider.List",
            Self::IntegrationCreate => "CLI.Execute.Integration.Create",
            Self::IntegrationUpdate => "CLI.Execute.Integration.Update",
            Self::IntegrationList => "CLI.Execute.Integration.List",
            Self::ArtifactUpload => "CLI.Execute.Artifact.Upload",
            Self::ArtifactGet => "CLI.Execute.Artifact.Get",
            Self::ArtifactDownload => "CLI.Execute.Artifact.Download",
            Self::ScheduleCreate => "CLI.Execute.Schedule.Create",
            Self::ScheduleList => "CLI.Execute.Schedule.List",
            Self::ScheduleGet => "CLI.Execute.Schedule.Get",
            Self::SchedulePause => "CLI.Execute.Schedule.Pause",
            Self::ScheduleUnpause => "CLI.Execute.Schedule.Unpause",
            Self::ScheduleUpdate => "CLI.Execute.Schedule.Update",
            Self::ScheduleDelete => "CLI.Execute.Schedule.Delete",
            Self::SecretCreate => "CLI.Execute.Secret.Create",
            Self::SecretDelete => "CLI.Execute.Secret.Delete",
            Self::SecretUpdate => "CLI.Execute.Secret.Update",
            Self::SecretList => "CLI.Execute.Secret.List",
            Self::FederateIssueToken => "CLI.Execute.Federate.IssueToken",
            Self::FederateIssueGcpToken => "CLI.Execute.Federate.IssueGcpToken",
            Self::HarnessSupportPing => "CLI.Execute.HarnessSupport.Ping",
            Self::HarnessSupportReportArtifact => "CLI.Execute.HarnessSupport.ReportArtifact",
            Self::HarnessSupportNotifyUser => "CLI.Execute.HarnessSupport.NotifyUser",
            Self::HarnessSupportFinishTask => "CLI.Execute.HarnessSupport.FinishTask",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::AgentRun => "Ran an agent from the Warp CLI",
            Self::AgentRunAmbient => "Ran an ambient agent from the Warp CLI",
            Self::AgentProfileList => "Listed agent profiles from the Warp CLI",
            Self::AgentList => "Listed agents from the Warp CLI",
            Self::EnvironmentList => "Listed cloud environments from the Warp CLI",
            Self::EnvironmentCreate => "Created a cloud environment from the Warp CLI",
            Self::EnvironmentDelete => "Deleted a cloud environment from the Warp CLI",
            Self::EnvironmentUpdate => "Updated a cloud environment from the Warp CLI",
            Self::EnvironmentGet => "Got cloud environment details from the Warp CLI",
            Self::EnvironmentImageList => "Listed available base images from the Warp CLI",
            Self::MCPList => "Listed MCP servers from the Warp CLI",
            Self::ModelList => "Listed models from the Warp CLI",
            Self::TaskList => "Listed tasks from the Warp CLI",
            Self::TaskGet => "Got status of task from the Warp CLI",
            Self::ConversationGet => "Got conversation by ID from the Warp CLI",
            Self::RunConversationGet => "Got run conversation from the Warp CLI",
            Self::RunMessageWatch => "Watched run messages from the Warp CLI",
            Self::RunMessageSend => "Sent a run message from the Warp CLI",
            Self::RunMessageList => "Listed run messages from the Warp CLI",
            Self::RunMessageRead => "Read a run message from the Warp CLI",
            Self::RunMessageMarkDelivered => "Marked a run message as delivered from the Warp CLI",
            Self::Login => "Logged in via the Warp CLI",
            Self::Logout => "Logged out via the Warp CLI",
            Self::Whoami => "Printed current user info from the Warp CLI",
            Self::ProviderSetup => "Set up a provider via the Warp CLI",
            Self::ProviderList => "Listed providers from the Warp CLI",
            Self::IntegrationCreate => "Created an integration from the Warp CLI",
            Self::IntegrationUpdate => "Updated an integration from the Warp CLI",
            Self::IntegrationList => "Listed integrations from the Warp CLI",
            Self::ArtifactUpload => "Uploaded an artifact from the Warp CLI",
            Self::ArtifactGet => "Got artifact metadata from the Warp CLI",
            Self::ArtifactDownload => "Downloaded an artifact from the Warp CLI",
            Self::ScheduleCreate => "Created a scheduled agent from the Warp CLI",
            Self::ScheduleList => "Listed scheduled agents from the Warp CLI",
            Self::ScheduleGet => "Got scheduled agent configuration from the Warp CLI",
            Self::SchedulePause => "Paused a scheduled agent from the Warp CLI",
            Self::ScheduleUnpause => "Unpaused a scheduled agent from the Warp CLI",
            Self::ScheduleUpdate => "Updated a scheduled agent from the Warp CLI",
            Self::ScheduleDelete => "Deleted a scheduled agent from the Warp CLI",
            Self::SecretCreate => "Created a secret from the Warp CLI",
            Self::SecretDelete => "Deleted a secret from the Warp CLI",
            Self::SecretUpdate => "Updated a secret from the Warp CLI",
            Self::SecretList => "Listed secrets from the Warp CLI",
            Self::FederateIssueToken => "Issued a federated identity token from the Warp CLI",
            Self::FederateIssueGcpToken => {
                "Issued a GCP federated identity token from the Warp CLI"
            }
            Self::HarnessSupportPing => "Pinged harness-support from the Warp CLI",
            Self::HarnessSupportReportArtifact => {
                "Reported an artifact via harness-support from the Warp CLI"
            }
            Self::HarnessSupportNotifyUser => {
                "Sent a user notification via harness-support from the Warp CLI"
            }
            Self::HarnessSupportFinishTask => {
                "Reported task completion via harness-support from the Warp CLI"
            }
        }
    }

    fn enablement_state(&self) -> EnablementState {
        match self {
            Self::FederateIssueToken | Self::FederateIssueGcpToken => {
                EnablementState::Flag(FeatureFlag::OzIdentityFederation)
            }
            Self::HarnessSupportPing
            | Self::HarnessSupportReportArtifact
            | Self::HarnessSupportNotifyUser
            | Self::HarnessSupportFinishTask => EnablementState::Flag(FeatureFlag::AgentHarness),
            Self::ArtifactUpload | Self::ArtifactGet | Self::ArtifactDownload => {
                EnablementState::Flag(FeatureFlag::ArtifactCommand)
            }
            Self::RunMessageWatch
            | Self::RunMessageSend
            | Self::RunMessageList
            | Self::RunMessageRead
            | Self::RunMessageMarkDelivered => EnablementState::Flag(FeatureFlag::OrchestrationV2),
            _ => EnablementState::Always,
        }
    }
}

warp_core::register_telemetry_event!(CliTelemetryEvent);
