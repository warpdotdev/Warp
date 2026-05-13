use byte_unit::Byte;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Role {
    Reader,
    Executor,
    Full,
}

impl Role {
    pub fn can_execute(&self) -> bool {
        matches!(self, Role::Executor | Role::Full)
    }

    pub fn downgrade_full(&mut self) {
        if *self == Role::Full {
            *self = Role::Executor;
        }
    }
}

impl Default for Role {
    fn default() -> Self {
        Self::Reader
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RoleRequestId(String);

impl From<String> for RoleRequestId {
    fn from(value: String) -> Self {
        RoleRequestId(value)
    }
}

impl RoleRequestId {
    pub fn new() -> RoleRequestId {
        RoleRequestId(Uuid::new_v4().to_string())
    }
}

impl Default for RoleRequestId {
    fn default() -> Self {
        RoleRequestId::new()
    }
}

impl std::fmt::Display for RoleRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ParticipantId(String);

impl From<String> for ParticipantId {
    fn from(value: String) -> Self {
        ParticipantId(value)
    }
}

impl ParticipantId {
    pub fn new() -> ParticipantId {
        ParticipantId(Uuid::new_v4().to_string())
    }
}

impl Default for ParticipantId {
    fn default() -> Self {
        ParticipantId::new()
    }
}

impl std::fmt::Display for ParticipantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

#[derive(Debug, Hash, Serialize, Deserialize, Eq, PartialEq, Clone, Copy)]
#[serde(transparent)]
pub struct SessionId(Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for SessionId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(s).map(SessionId)
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct InputReplicaId(String);

impl From<String> for InputReplicaId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for InputReplicaId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum GridType {
    Prompt,
    Rprompt,
    Output,
    PromptAndCommand,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Point {
    pub row: usize,
    pub col: usize,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockId(String);

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for BlockId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BlockPoint {
    pub block_id: BlockId,
    pub grid_type: GridType,
    pub point: Point,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub enum Selection {
    #[default]
    None,
    Blocks {
        block_ids: Vec<BlockId>,
    },
    BlockText {
        start: BlockPoint,
        end: BlockPoint,
        is_reversed: bool,
    },
    AltScreenText {
        start: Point,
        end: Point,
        is_reversed: bool,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PresenceUpdate {
    Selection(Selection),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ParticipantPresenceUpdate {
    pub participant_id: ParticipantId,
    pub update: PresenceUpdate,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ProfileData {
    pub user_uid: String,
    pub display_name: String,
    pub photo_url: Option<String>,
    pub email: Option<String>,
    pub input_replica_id: InputReplicaId,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ParticipantInfo {
    pub id: ParticipantId,
    pub profile_data: ProfileData,
    pub selection: Selection,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Sharer {
    pub info: ParticipantInfo,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Viewer {
    pub info: ParticipantInfo,
    pub role: Role,
    pub is_present: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PresentViewer {
    pub info: ParticipantInfo,
    pub max_acl: Role,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AbsentViewer {
    pub info: ParticipantInfo,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Guest {
    pub profile_data: ProfileData,
    pub direct_acl: Role,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PendingGuest {
    pub email: String,
    pub direct_acl: Role,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ParticipantList {
    pub sharer: Sharer,
    pub viewers: Vec<Viewer>,
    pub present_viewers: Vec<PresentViewer>,
    pub absent_viewers: Vec<AbsentViewer>,
    pub guests: Vec<Guest>,
    pub pending_guests: Vec<PendingGuest>,
}

impl ParticipantList {
    pub fn downgrade_full_roles(&mut self) {
        for viewer in &mut self.viewers {
            viewer.role.downgrade_full();
        }
        for viewer in &mut self.present_viewers {
            viewer.max_acl.downgrade_full();
        }
        for guest in &mut self.guests {
            guest.direct_acl.downgrade_full();
        }
        for guest in &mut self.pending_guests {
            guest.direct_acl.downgrade_full();
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Scrollback {
    pub blocks: Vec<ScrollbackBlock>,
    pub is_alt_screen_active: bool,
}

impl Scrollback {
    pub fn num_bytes(&self) -> Byte {
        self.blocks
            .iter()
            .map(|b| b.num_bytes().as_u64())
            .fold(0, u64::saturating_add)
            .into()
    }

    pub fn exceeds_size_bytes(&self, size_bytes: Byte) -> bool {
        self.num_bytes() > size_bytes
    }
}

impl std::fmt::Debug for Scrollback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Scrollback {{ num_blocks: {}, is_alt_screen_active: {} }}",
            self.blocks.len(),
            self.is_alt_screen_active
        )
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ScrollbackBlock {
    pub raw: Vec<u8>,
}

impl ScrollbackBlock {
    pub fn num_bytes(&self) -> Byte {
        self.raw.len().into()
    }
}

impl std::fmt::Debug for ScrollbackBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ScrollbackBlock {{ num_bytes: {} }}", self.num_bytes())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AICommandMetadata {
    pub tool_call_id: String,
    #[serde(default)]
    pub is_agent_monitored: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub struct WindowSize {
    pub num_rows: usize,
    pub num_cols: usize,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum OrderedTerminalEventType {
    PtyBytesRead {
        bytes: Vec<u8>,
    },
    CommandExecutionStarted {
        participant_id: ParticipantId,
        #[serde(default)]
        ai_metadata: Option<AICommandMetadata>,
    },
    CommandExecutionFinished {
        next_block_id: BlockId,
    },
    Resize {
        window_size: WindowSize,
    },
    AgentResponseEvent {
        response_initiator: Option<ParticipantId>,
        response_event: String,
        #[serde(default)]
        forked_from_conversation_token: Option<String>,
    },
    AgentConversationReplayStarted,
    AgentConversationReplayEnded,
}

impl std::fmt::Debug for OrderedTerminalEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PtyBytesRead { .. } => f.write_str("PtyBytesRead"),
            Self::CommandExecutionStarted { .. } => f.write_str("CommandExecutionStarted"),
            Self::CommandExecutionFinished { .. } => f.write_str("CommandExecutionFinished"),
            Self::Resize { .. } => f.write_str("Resize"),
            Self::AgentResponseEvent { .. } => f.write_str("AgentResponseEvent"),
            Self::AgentConversationReplayStarted => f.write_str("AgentConversationReplayStarted"),
            Self::AgentConversationReplayEnded => f.write_str("AgentConversationReplayEnded"),
        }
    }
}

impl OrderedTerminalEventType {
    pub fn num_bytes(&self) -> Byte {
        match &self {
            OrderedTerminalEventType::PtyBytesRead { bytes } => bytes.len().into(),
            OrderedTerminalEventType::AgentResponseEvent { response_event, .. } => {
                response_event.len().into()
            }
            OrderedTerminalEventType::CommandExecutionStarted { .. }
            | OrderedTerminalEventType::CommandExecutionFinished { .. }
            | OrderedTerminalEventType::AgentConversationReplayStarted
            | OrderedTerminalEventType::AgentConversationReplayEnded
            | OrderedTerminalEventType::Resize { .. } => Byte::from_u64(0),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentAttachment {
    BlockReference {
        block_id: BlockId,
    },
    PlainText {
        content: String,
    },
    FileReference {
        attachment_id: String,
        file_name: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Copy)]
pub struct ServerConversationToken(Uuid);

impl ServerConversationToken {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for ServerConversationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ServerConversationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ServerConversationToken {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SelectedAgentModel(String);

impl SelectedAgentModel {
    pub fn new(model_id: impl Into<String>) -> Self {
        Self(model_id.into())
    }

    pub fn model_id(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub enum SelectedConversation {
    ExistingConversation(ServerConversationToken),
    #[default]
    NewConversation,
    NoConversation,
}

impl SelectedConversation {
    pub fn new(server_token: Option<ServerConversationToken>) -> Self {
        match server_token {
            Some(token) => Self::ExistingConversation(token),
            None => Self::NewConversation,
        }
    }
}

#[derive(Clone, Default, Deserialize, Serialize, PartialEq, Eq, Debug, Copy)]
pub enum InputType {
    #[default]
    Shell,
    AI,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct InputMode {
    pub input_type: InputType,
    pub is_locked: bool,
}

impl InputMode {
    pub fn new(input_type: InputType, is_locked: bool) -> Self {
        Self {
            input_type,
            is_locked,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub enum CLIAgentSessionState {
    Active {
        cli_agent: String,
        is_rich_input_open: bool,
    },
    #[default]
    Inactive,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum LongRunningCommandAgentInteractionState {
    NotInteracting,
    TaggedIn,
    InControl,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct UniversalDeveloperInputContext {
    pub selected_model: Option<SelectedAgentModel>,
    pub input_mode: Option<InputMode>,
    pub selected_conversation: Option<SelectedConversation>,
    pub long_running_command_agent_interaction_state:
        Option<LongRunningCommandAgentInteractionState>,
    pub auto_approve_agent_actions: Option<bool>,
    #[serde(default)]
    pub cli_agent_session: CLIAgentSessionState,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct UniversalDeveloperInputContextUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_model: Option<SelectedAgentModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_mode: Option<InputMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_conversation: Option<SelectedConversation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_running_command_agent_interaction_state:
        Option<LongRunningCommandAgentInteractionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_approve_agent_actions: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_agent_session: Option<CLIAgentSessionState>,
}

impl UniversalDeveloperInputContextUpdate {
    pub fn changes_cached_context(&self, cached: &UniversalDeveloperInputContext) -> bool {
        let UniversalDeveloperInputContextUpdate {
            selected_model: updated_selected_model,
            input_mode: updated_input_mode,
            selected_conversation: updated_selected_conversation,
            auto_approve_agent_actions: updated_auto_approve_agent_actions,
            long_running_command_agent_interaction_state:
                updated_long_running_command_agent_interaction_state,
            cli_agent_session: updated_cli_agent_session,
        } = self;
        let UniversalDeveloperInputContext {
            selected_model: cached_selected_model,
            input_mode: cached_input_mode,
            selected_conversation: cached_selected_conversation,
            auto_approve_agent_actions: cached_auto_approve_agent_actions,
            long_running_command_agent_interaction_state:
                cached_long_running_command_agent_interaction_state,
            cli_agent_session: cached_cli_agent_session,
        } = cached;

        (updated_selected_model.is_some()
            && updated_selected_model.as_ref() != cached_selected_model.as_ref())
            || (updated_input_mode.is_some()
                && updated_input_mode.as_ref() != cached_input_mode.as_ref())
            || (updated_selected_conversation.is_some()
                && updated_selected_conversation != cached_selected_conversation)
            || (updated_auto_approve_agent_actions.is_some()
                && updated_auto_approve_agent_actions != cached_auto_approve_agent_actions)
            || (updated_long_running_command_agent_interaction_state.is_some()
                && updated_long_running_command_agent_interaction_state
                    != cached_long_running_command_agent_interaction_state)
            || (updated_cli_agent_session.is_some()
                && updated_cli_agent_session.as_ref() != Some(cached_cli_agent_session))
    }

    pub fn merge_into(
        self,
        current: UniversalDeveloperInputContext,
    ) -> UniversalDeveloperInputContext {
        let UniversalDeveloperInputContextUpdate {
            selected_model: updated_selected_model,
            input_mode: updated_input_mode,
            selected_conversation: updated_selected_conversation,
            auto_approve_agent_actions: updated_auto_approve_agent_actions,
            long_running_command_agent_interaction_state:
                updated_long_running_command_agent_interaction_state,
            cli_agent_session: updated_cli_agent_session,
        } = self;
        let UniversalDeveloperInputContext {
            selected_model: current_selected_model,
            input_mode: current_input_mode,
            selected_conversation: current_selected_conversation,
            auto_approve_agent_actions: current_auto_approve_agent_actions,
            long_running_command_agent_interaction_state:
                current_long_running_command_agent_interaction_state,
            cli_agent_session: current_cli_agent_session,
        } = current;

        UniversalDeveloperInputContext {
            selected_model: updated_selected_model.or(current_selected_model),
            input_mode: updated_input_mode.or(current_input_mode),
            selected_conversation: updated_selected_conversation.or(current_selected_conversation),
            auto_approve_agent_actions: updated_auto_approve_agent_actions
                .or(current_auto_approve_agent_actions),
            long_running_command_agent_interaction_state:
                updated_long_running_command_agent_interaction_state
                    .or(current_long_running_command_agent_interaction_state),
            cli_agent_session: updated_cli_agent_session.unwrap_or(current_cli_agent_session),
        }
    }
}

impl From<UniversalDeveloperInputContext> for UniversalDeveloperInputContextUpdate {
    fn from(context: UniversalDeveloperInputContext) -> Self {
        Self {
            selected_model: context.selected_model,
            input_mode: context.input_mode,
            selected_conversation: context.selected_conversation,
            auto_approve_agent_actions: context.auto_approve_agent_actions,
            long_running_command_agent_interaction_state: context
                .long_running_command_agent_interaction_state,
            cli_agent_session: Some(context.cli_agent_session),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum SessionEndedReason {
    EndedBySharer,
    InactivityLimitReached,
    ExceededSizeLimit,
}

#[derive(Default, Debug, Deserialize, Serialize, Clone, Copy)]
pub enum RoleUpdateReason {
    #[default]
    UpdatedBySharer,
    InactivityLimitReached,
}

#[derive(Default, Debug, Deserialize, Serialize, Clone, Copy)]
pub enum RoleUpdatedReason {
    #[default]
    UpdatedBySharer,
    InactivityLimitReached,
}

impl From<RoleUpdateReason> for RoleUpdatedReason {
    fn from(value: RoleUpdateReason) -> Self {
        match value {
            RoleUpdateReason::UpdatedBySharer => RoleUpdatedReason::UpdatedBySharer,
            RoleUpdateReason::InactivityLimitReached => RoleUpdatedReason::InactivityLimitReached,
        }
    }
}

#[derive(Clone, Debug, Serialize, Default)]
pub enum SessionSourceType {
    #[default]
    User,
    AmbientAgent {
        #[serde(default)]
        task_id: Option<String>,
    },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SessionSourceTypeWire {
    Legacy(LegacySessionSourceType),
    New {
        #[serde(rename = "AmbientAgent")]
        ambient_agent: AmbientAgentFields,
    },
}

#[derive(Deserialize)]
struct AmbientAgentFields {
    #[serde(default)]
    task_id: Option<String>,
}

impl From<SessionSourceTypeWire> for SessionSourceType {
    fn from(value: SessionSourceTypeWire) -> Self {
        match value {
            SessionSourceTypeWire::Legacy(LegacySessionSourceType::User) => SessionSourceType::User,
            SessionSourceTypeWire::Legacy(LegacySessionSourceType::AmbientAgent) => {
                SessionSourceType::AmbientAgent { task_id: None }
            }
            SessionSourceTypeWire::New {
                ambient_agent: AmbientAgentFields { task_id },
            } => SessionSourceType::AmbientAgent { task_id },
        }
    }
}

impl<'de> Deserialize<'de> for SessionSourceType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SessionSourceTypeWire::deserialize(deserializer)?;
        Ok(SessionSourceType::from(wire))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub enum LegacySessionSourceType {
    #[default]
    User,
    AmbientAgent,
}
