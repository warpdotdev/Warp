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
    /// Executing `warp login`
    Login,
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
            CliTelemetryEvent::AgentRun {
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
            CliTelemetryEvent::AgentProfileList => None,
            CliTelemetryEvent::AgentList => None,
            CliTelemetryEvent::EnvironmentList => None,
            CliTelemetryEvent::EnvironmentCreate => None,
            CliTelemetryEvent::EnvironmentDelete => None,
            CliTelemetryEvent::EnvironmentUpdate => None,
            CliTelemetryEvent::EnvironmentGet => None,
            CliTelemetryEvent::EnvironmentImageList => None,
            CliTelemetryEvent::MCPList => None,
            CliTelemetryEvent::ModelList => None,
            CliTelemetryEvent::Login => None,
            CliTelemetryEvent::Whoami => None,
            CliTelemetryEvent::ProviderSetup => None,
            CliTelemetryEvent::ProviderList => None,
            CliTelemetryEvent::IntegrationCreate => None,
            CliTelemetryEvent::IntegrationUpdate => None,
            CliTelemetryEvent::IntegrationList => None,
            CliTelemetryEvent::ArtifactUpload => None,
            CliTelemetryEvent::ArtifactGet => None,
            CliTelemetryEvent::ArtifactDownload => None,
            CliTelemetryEvent::HarnessSupportPing => None,
            CliTelemetryEvent::HarnessSupportReportArtifact { artifact_type } => {
                Some(json!({ "artifact_type": artifact_type }))
            }
            CliTelemetryEvent::HarnessSupportNotifyUser => None,
            CliTelemetryEvent::HarnessSupportFinishTask { success } => {
                Some(json!({ "success": success }))
            }
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
            CliTelemetryEventDiscriminants::AgentRun => "CLI.Execute.Agent.Run",
            CliTelemetryEventDiscriminants::AgentProfileList => "CLI.Execute.Agent.Profile.List",
            CliTelemetryEventDiscriminants::AgentList => "CLI.Execute.Agent.List",
            CliTelemetryEventDiscriminants::EnvironmentList => "CLI.Execute.Environment.List",
            CliTelemetryEventDiscriminants::EnvironmentCreate => "CLI.Execute.Environment.Create",
            CliTelemetryEventDiscriminants::EnvironmentDelete => "CLI.Execute.Environment.Delete",
            CliTelemetryEventDiscriminants::EnvironmentUpdate => "CLI.Execute.Environment.Update",
            CliTelemetryEventDiscriminants::EnvironmentGet => "CLI.Execute.Environment.Get",
            CliTelemetryEventDiscriminants::EnvironmentImageList => {
                "CLI.Execute.Environment.Image.List"
            }
            CliTelemetryEventDiscriminants::MCPList => "CLI.Execute.MCP.List",
            CliTelemetryEventDiscriminants::ModelList => "CLI.Execute.Model.List",
            CliTelemetryEventDiscriminants::Login => "CLI.Execute.Login",
            CliTelemetryEventDiscriminants::Whoami => "CLI.Execute.Whoami",
            CliTelemetryEventDiscriminants::ProviderSetup => "CLI.Execute.Provider.Setup",
            CliTelemetryEventDiscriminants::ProviderList => "CLI.Execute.Provider.List",
            CliTelemetryEventDiscriminants::IntegrationCreate => "CLI.Execute.Integration.Create",
            CliTelemetryEventDiscriminants::IntegrationUpdate => "CLI.Execute.Integration.Update",
            CliTelemetryEventDiscriminants::IntegrationList => "CLI.Execute.Integration.List",
            CliTelemetryEventDiscriminants::ArtifactUpload => "CLI.Execute.Artifact.Upload",
            CliTelemetryEventDiscriminants::ArtifactGet => "CLI.Execute.Artifact.Get",
            CliTelemetryEventDiscriminants::ArtifactDownload => "CLI.Execute.Artifact.Download",
            CliTelemetryEventDiscriminants::HarnessSupportPing => "CLI.Execute.HarnessSupport.Ping",
            CliTelemetryEventDiscriminants::HarnessSupportReportArtifact => {
                "CLI.Execute.HarnessSupport.ReportArtifact"
            }
            CliTelemetryEventDiscriminants::HarnessSupportNotifyUser => {
                "CLI.Execute.HarnessSupport.NotifyUser"
            }
            CliTelemetryEventDiscriminants::HarnessSupportFinishTask => {
                "CLI.Execute.HarnessSupport.FinishTask"
            }
        }
    }

    fn description(&self) -> &'static str {
        match self {
            CliTelemetryEventDiscriminants::AgentRun => "Ran an agent from the Warp CLI",
            CliTelemetryEventDiscriminants::AgentProfileList => {
                "Listed agent profiles from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::AgentList => "Listed agents from the Warp CLI",
            CliTelemetryEventDiscriminants::EnvironmentList => {
                "Listed cloud environments from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::EnvironmentCreate => {
                "Created a cloud environment from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::EnvironmentDelete => {
                "Deleted a cloud environment from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::EnvironmentUpdate => {
                "Updated a cloud environment from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::EnvironmentGet => {
                "Got cloud environment details from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::EnvironmentImageList => {
                "Listed available base images from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::MCPList => "Listed MCP servers from the Warp CLI",
            CliTelemetryEventDiscriminants::ModelList => "Listed models from the Warp CLI",
            CliTelemetryEventDiscriminants::Login => "Logged in via the Warp CLI",
            CliTelemetryEventDiscriminants::Whoami => "Printed current user info from the Warp CLI",
            CliTelemetryEventDiscriminants::ProviderSetup => "Set up a provider via the Warp CLI",
            CliTelemetryEventDiscriminants::ProviderList => "Listed providers from the Warp CLI",
            CliTelemetryEventDiscriminants::IntegrationCreate => {
                "Created an integration from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::IntegrationUpdate => {
                "Updated an integration from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::IntegrationList => {
                "Listed integrations from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::ArtifactUpload => {
                "Uploaded an artifact from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::ArtifactGet => {
                "Got artifact metadata from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::ArtifactDownload => {
                "Downloaded an artifact from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::HarnessSupportPing => {
                "Pinged harness-support from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::HarnessSupportReportArtifact => {
                "Reported an artifact via harness-support from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::HarnessSupportNotifyUser => {
                "Sent a user notification via harness-support from the Warp CLI"
            }
            CliTelemetryEventDiscriminants::HarnessSupportFinishTask => {
                "Reported task completion via harness-support from the Warp CLI"
            }
        }
    }

    fn enablement_state(&self) -> EnablementState {
        match self {
            Self::HarnessSupportPing
            | Self::HarnessSupportReportArtifact
            | Self::HarnessSupportNotifyUser
            | Self::HarnessSupportFinishTask => EnablementState::Flag(FeatureFlag::AgentHarness),
            Self::ArtifactUpload | Self::ArtifactGet | Self::ArtifactDownload => {
                EnablementState::Flag(FeatureFlag::ArtifactCommand)
            }
            _ => EnablementState::Always,
        }
    }
}

warp_core::register_telemetry_event!(CliTelemetryEvent);
