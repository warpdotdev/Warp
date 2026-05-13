use std::{borrow::Cow, fmt, str::FromStr};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use derivative::Derivative;
use pathfinder_geometry::vector::vec2f;
use serde::{Deserialize, Serialize};
use warp_core::ui::{appearance::Appearance, theme::Fill, Icon};
use warpui::{
    elements::{
        Align, ChildAnchor, ConstrainedBox, Hoverable, MouseStateHandle, OffsetPositioning,
        ParentAnchor, ParentElement, ParentOffsetBounds, Stack,
    },
    ui_components::components::UiComponent,
    Element,
};

use crate::{
    auth::UserUid,
    drive::sharing::{SharingAccessLevel, Subject},
    server::ids::{ServerId, SyncId},
    server_time::ServerTimestamp,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ObjectIdType {
    Notebook,
    Workflow,
    Folder,
    GenericStringObject,
}

impl ObjectIdType {
    pub fn sqlite_prefix(&self) -> &'static str {
        match self {
            ObjectIdType::Notebook => "Notebook",
            ObjectIdType::Workflow => "Workflow",
            ObjectIdType::Folder => "Folder",
            ObjectIdType::GenericStringObject => "GenericStringObject",
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ObjectType {
    Notebook,
    Workflow,
    Folder,
    GenericStringObject(GenericStringObjectFormat),
}

impl ObjectType {
    pub fn sqlite_object_type_as_str(&self) -> Cow<'_, str> {
        match self {
            ObjectType::Notebook => "NOTEBOOK".into(),
            ObjectType::Workflow => "WORKFLOW".into(),
            ObjectType::Folder => "FOLDER".into(),
            ObjectType::GenericStringObject(format) => format.to_string().into(),
        }
    }
}

const NOTEBOOK_OBJECT_STRING: &str = "notebook";
const WORKFLOW_OBJECT_STRING: &str = "workflow";
const PROMPT_OBJECT_STRING: &str = "prompt";
const FOLDER_OBJECT_STRING: &str = "folder";
const ENV_VAR_COLLECTION_STRING: &str = "env-vars";

impl FromStr for ObjectType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            NOTEBOOK_OBJECT_STRING => Ok(Self::Notebook),
            WORKFLOW_OBJECT_STRING | PROMPT_OBJECT_STRING => Ok(Self::Workflow),
            FOLDER_OBJECT_STRING => Ok(Self::Folder),
            ENV_VAR_COLLECTION_STRING => Ok(Self::GenericStringObject(
                GenericStringObjectFormat::Json(JsonObjectType::EnvVarCollection),
            )),
            _ => Err(anyhow!("Unexpected object type")),
        }
    }
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjectType::Notebook => write!(f, "{NOTEBOOK_OBJECT_STRING}"),
            ObjectType::Workflow => write!(f, "{WORKFLOW_OBJECT_STRING}"),
            ObjectType::Folder => write!(f, "{FOLDER_OBJECT_STRING}"),
            ObjectType::GenericStringObject(GenericStringObjectFormat::Json(
                JsonObjectType::EnvVarCollection,
            )) => write!(f, "{ENV_VAR_COLLECTION_STRING}"),
            ObjectType::GenericStringObject(GenericStringObjectFormat::Json(
                JsonObjectType::AIFact,
            )) => write!(f, "rule"),
            ObjectType::GenericStringObject(_) => write!(f, "string_object_placeholder"),
        }
    }
}

impl From<ObjectType> for ObjectIdType {
    fn from(value: ObjectType) -> Self {
        match value {
            ObjectType::Notebook => ObjectIdType::Notebook,
            ObjectType::Workflow => ObjectIdType::Workflow,
            ObjectType::Folder => ObjectIdType::Folder,
            ObjectType::GenericStringObject(_) => ObjectIdType::GenericStringObject,
        }
    }
}

pub const GENERIC_STRING_OBJECT_PREFIX: &str = "GENERIC_STRING_";
pub const JSON_OBJECT_PREFIX: &str = "JSON_";

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum GenericStringObjectFormat {
    Json(JsonObjectType),
}

#[allow(clippy::to_string_trait_impl)]
impl ToString for GenericStringObjectFormat {
    fn to_string(&self) -> String {
        match self {
            GenericStringObjectFormat::Json(json_object_type) => format!(
                "{}{}{}",
                GENERIC_STRING_OBJECT_PREFIX,
                JSON_OBJECT_PREFIX,
                json_object_type.as_str()
            ),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum JsonObjectType {
    Preference,
    EnvVarCollection,
    WorkflowEnum,
    AIFact,
    MCPServer,
    AIExecutionProfile,
    TemplatableMCPServer,
}

impl JsonObjectType {
    pub fn as_str(&self) -> &'static str {
        match self {
            JsonObjectType::Preference => "PREFERENCE",
            JsonObjectType::EnvVarCollection => "ENVVARCOLLECTION",
            JsonObjectType::WorkflowEnum => "WORKFLOWENUM",
            JsonObjectType::AIFact => "AIFACT",
            JsonObjectType::MCPServer => "MCPSERVER",
            JsonObjectType::AIExecutionProfile => "AIEXECUTIONPROFILE",
            JsonObjectType::TemplatableMCPServer => "TEMPLATABLEMCPSERVER",
        }
    }
}

impl TryFrom<&str> for JsonObjectType {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "PREFERENCE" => Ok(JsonObjectType::Preference),
            "ENVVARCOLLECTION" => Ok(JsonObjectType::EnvVarCollection),
            "WORKFLOWENUM" => Ok(JsonObjectType::WorkflowEnum),
            "AIFACT" => Ok(JsonObjectType::AIFact),
            "MCPSERVER" => Ok(JsonObjectType::MCPServer),
            "AIEXECUTIONPROFILE" => Ok(JsonObjectType::AIExecutionProfile),
            "TEMPLATABLEMCPSERVER" => Ok(JsonObjectType::TemplatableMCPServer),
            _ => Err(anyhow!("could not convert unknown json object type")),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq, PartialOrd, Ord)]
pub struct Revision(ServerTimestamp);

impl Revision {
    pub fn from_unix_timestamp_micros(ms_since_epoch: i64) -> Result<Self> {
        let ts = ServerTimestamp::from_unix_timestamp_micros(ms_since_epoch)?;
        Ok(Self(ts))
    }

    pub fn timestamp_micros(&self) -> i64 {
        self.0.timestamp_micros()
    }

    pub fn utc(&self) -> DateTime<Utc> {
        self.0.utc()
    }

    pub fn timestamp(&self) -> ServerTimestamp {
        self.0
    }

    #[cfg(test)]
    pub fn now() -> Self {
        Self(ServerTimestamp::new(Utc::now()))
    }
}

impl From<Revision> for ServerTimestamp {
    fn from(revision: Revision) -> Self {
        revision.0
    }
}

impl From<ServerTimestamp> for Revision {
    fn from(time: ServerTimestamp) -> Self {
        Revision(time)
    }
}

#[cfg(test)]
impl From<DateTime<Utc>> for Revision {
    fn from(time: DateTime<Utc>) -> Self {
        Self(ServerTimestamp::new(time))
    }
}

#[derive(Copy, Clone, Debug, Eq, Serialize, Deserialize, Derivative)]
#[derivative(PartialEq)]
pub enum Owner {
    User { user_uid: UserUid },
    Team { team_uid: ServerId },
}

impl Owner {
    #[cfg(test)]
    pub fn mock_current_user() -> Owner {
        Owner::User {
            user_uid: UserUid::new(crate::auth::TEST_USER_UID),
        }
    }
}

impl From<Owner> for Option<ServerId> {
    fn from(owner: Owner) -> Option<ServerId> {
        match owner {
            Owner::User { .. } => None,
            Owner::Team { team_uid, .. } => Some(team_uid),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerObjectContainer {
    Folder { folder_uid: ServerId },
    Drive { owner: Owner },
}

#[derive(Clone, Debug)]
pub struct NumInFlightRequests(pub usize);

#[derive(Clone, Debug)]
pub enum StoredObjectSyncStatus {
    NoLocalChanges,
    InFlight(NumInFlightRequests),
    InConflict,
    Errored,
}

const SYNC_ICON_DIMENSIONS: f32 = 16.;
const SYNC_STATUS_TOOLTIP_ERROR: &str = "Failed to save";

#[derive(Debug, Clone, PartialEq)]
pub struct StoredObjectPermissions {
    pub owner: Owner,
    pub permissions_last_updated_ts: Option<ServerTimestamp>,
    pub anyone_with_link: Option<LinkSharing>,
    pub guests: Vec<StoredObjectGuest>,
}

impl StoredObjectPermissions {
    #[cfg(test)]
    pub fn mock_personal() -> Self {
        Self {
            owner: Owner::mock_current_user(),
            permissions_last_updated_ts: Some(Utc::now().into()),
            guests: Vec::new(),
            anyone_with_link: None,
        }
    }

    pub fn has_direct_user_access(&self, user_uid: UserUid) -> bool {
        self.anyone_with_link.is_some() || self.guests.iter().any(|g| g.subject.is_user(user_uid))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinkSharing {
    pub access_level: SharingAccessLevel,
    pub source: Option<ServerObjectContainer>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredObjectGuest {
    pub subject: Subject,
    pub access_level: SharingAccessLevel,
    pub source: Option<ServerObjectContainer>,
}

#[derive(Clone, Debug)]
pub struct StoredObjectMetadata {
    pub revision: Option<Revision>,
    pub metadata_last_updated_ts: Option<ServerTimestamp>,
    pub current_editor_uid: Option<String>,
    pub pending_changes_statuses: StoredObjectStatuses,
    pub trashed_ts: Option<ServerTimestamp>,
    pub folder_id: Option<SyncId>,
    pub is_welcome_object: bool,
    pub last_editor_uid: Option<String>,
    pub creator_uid: Option<String>,
    pub last_task_run_ts: Option<ServerTimestamp>,
}

impl StoredObjectMetadata {
    #[cfg(test)]
    pub fn mock() -> Self {
        Self {
            revision: Some(Revision::now()),
            current_editor_uid: None,
            metadata_last_updated_ts: Some(Utc::now().into()),
            pending_changes_statuses: StoredObjectStatuses::mock(),
            trashed_ts: None,
            folder_id: None,
            is_welcome_object: false,
            last_editor_uid: None,
            creator_uid: None,
            last_task_run_ts: None,
        }
    }

    pub fn has_pending_content_changes(&self) -> bool {
        !matches!(
            self.pending_changes_statuses.content_sync_status,
            StoredObjectSyncStatus::NoLocalChanges | StoredObjectSyncStatus::InConflict
        )
    }

    pub fn is_errored(&self) -> bool {
        matches!(
            self.pending_changes_statuses.content_sync_status,
            StoredObjectSyncStatus::Errored
        )
    }

    pub fn has_pending_online_only_change(&self) -> bool {
        self.pending_changes_statuses.has_pending_permissions_change
            || self.pending_changes_statuses.has_pending_metadata_change
            || self.pending_changes_statuses.pending_untrash
            || self.pending_changes_statuses.pending_delete
    }

    pub fn set_current_editor(&mut self, editor_uid: Option<String>) {
        self.current_editor_uid = editor_uid;
    }
}

#[derive(Clone, Debug)]
pub struct StoredObjectStatuses {
    pub content_sync_status: StoredObjectSyncStatus,
    pub has_pending_permissions_change: bool,
    pub has_pending_metadata_change: bool,
    pub pending_untrash: bool,
    pub pending_delete: bool,
}

impl StoredObjectStatuses {
    #[cfg(test)]
    pub fn mock() -> Self {
        Self {
            content_sync_status: StoredObjectSyncStatus::NoLocalChanges,
            has_pending_permissions_change: false,
            has_pending_metadata_change: false,
            pending_untrash: false,
            pending_delete: false,
        }
    }

    pub fn render_icon(
        &self,
        _sync_queue_is_dequeueing: bool,
        hover_state: MouseStateHandle,
        appearance: &Appearance,
    ) -> Option<Box<dyn Element>> {
        let theme = appearance.theme();
        let should_show_error_indicator = matches!(
            self.content_sync_status,
            StoredObjectSyncStatus::Errored | StoredObjectSyncStatus::InConflict
        );

        let icon_and_tooltip_text = if should_show_error_indicator {
            Some((
                Icon::AlertTriangle.to_warpui_icon(Fill::Solid(theme.ui_error_color())),
                SYNC_STATUS_TOOLTIP_ERROR,
            ))
        } else {
            None
        };

        if let Some((icon, tooltip_text)) = icon_and_tooltip_text {
            return Some(
                Align::new(
                    Hoverable::new(hover_state, move |hover_state| {
                        let mut stack = Stack::new().with_child(
                            ConstrainedBox::new(icon.finish())
                                .with_height(SYNC_ICON_DIMENSIONS)
                                .with_width(SYNC_ICON_DIMENSIONS)
                                .finish(),
                        );

                        if hover_state.is_hovered() {
                            let tooltip = appearance
                                .ui_builder()
                                .tool_tip(tooltip_text.to_string())
                                .build()
                                .finish();

                            stack.add_positioned_overlay_child(
                                tooltip,
                                OffsetPositioning::offset_from_parent(
                                    vec2f(0., -24.),
                                    ParentOffsetBounds::Unbounded,
                                    ParentAnchor::Center,
                                    ChildAnchor::Center,
                                ),
                            );
                        }

                        stack.finish()
                    })
                    .finish(),
                )
                .finish(),
            );
        }

        None
    }
}

#[derive(Copy, Default, Clone, Debug, Eq, PartialEq)]
pub enum StoredObjectEventEntrypoint {
    TeamSettings,
    ResourceCenter,
    UniversalSearch,
    ManagementUI,
    Blocklist,
    ImportModal,
    Onboarding,
    #[default]
    Unknown,
}
