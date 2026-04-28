pub mod cloud_action_confirmation_dialog;
mod cloud_object_naming_dialog;
pub mod cloud_object_styling;
pub mod drive_helpers;
pub mod empty_trash_confirmation_dialog;
pub mod export;
pub mod folders;
pub mod import;
pub(crate) mod index;
pub mod items;
pub mod panel;
pub mod settings;
pub mod sharing;
pub mod workflows;

use std::{cmp::Ordering, fmt};

pub use index::DriveIndexVariant;
pub use panel::{DrivePanel, DrivePanelEvent};
use serde::{Deserialize, Serialize};
use warp_core::user_preferences::GetUserPreferences as _;
use warpui::AppContext;

use crate::{
    cloud_object::{
        model::view::{CloudViewModel, UpdateTimestamp},
        CloudObject, GenericStringObjectFormat, ObjectIdType, ObjectType,
    },
    server::ids::{HashedSqliteId, ObjectUid, ServerId, SyncId},
    ui_components::icons::Icon,
    workflows::CloudWorkflow,
};

type SortByComparator<'a> = dyn FnMut(&&dyn CloudObject, &&dyn CloudObject) -> Ordering + 'a;

#[derive(Copy, Clone, Debug)]
pub enum DriveObjectType {
    Workflow,
    AgentModeWorkflow,
    AIFact,
    AIFactCollection,
    Notebook {
        /// Whether the notebook was created as an AI Document (plan)
        is_ai_document: bool,
    },
    Folder,
    EnvVarCollection,
    MCPServer,
    MCPServerCollection,
}

impl From<DriveObjectType> for Icon {
    fn from(cloud_object_type: DriveObjectType) -> Self {
        match cloud_object_type {
            DriveObjectType::Workflow => Self::Workflow,
            DriveObjectType::AgentModeWorkflow => Self::Prompt,
            DriveObjectType::AIFact => Self::BookOpen,
            DriveObjectType::AIFactCollection => Self::BookOpen,
            DriveObjectType::Notebook { is_ai_document } => {
                if is_ai_document {
                    Self::Compass
                } else {
                    Self::Notebook
                }
            }
            DriveObjectType::Folder => Self::Folder,
            DriveObjectType::EnvVarCollection => Self::EnvVarCollection,
            DriveObjectType::MCPServer => Self::Dataflow,
            DriveObjectType::MCPServerCollection => Self::Dataflow,
        }
    }
}

impl fmt::Display for DriveObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Notebook { .. } => write!(f, "notebook"),
            Self::Workflow => write!(f, "workflow"),
            Self::Folder => write!(f, "folder"),
            Self::EnvVarCollection => write!(f, "env var collection"),
            Self::AgentModeWorkflow => write!(f, "prompt"),
            Self::AIFact => write!(f, "ai fact"),
            Self::AIFactCollection => write!(f, "ai fact collection"),
            Self::MCPServer => write!(f, "mcp server"),
            Self::MCPServerCollection => write!(f, "mcp server collection"),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct OpenWarpDriveObjectSettings {
    /// The folder that should be focused in the Warp Drive when the object is opened.
    pub focused_folder_id: Option<ServerId>,
    /// The email of the user to invite to the object, if the object is being opened via the request access flow.
    pub invitee_email: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OpenWarpDriveObjectArgs {
    pub object_type: ObjectType,
    pub server_id: ServerId,
    pub settings: OpenWarpDriveObjectSettings,
}

/// Enum to use to pass down type and id between actions to avoid multiplying actions whenever we
/// need to pass the object id etc.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum CloudObjectTypeAndId {
    Notebook(SyncId),
    Workflow(SyncId),
    Folder(SyncId),
    GenericStringObject {
        object_type: GenericStringObjectFormat,
        id: SyncId,
    },
}

impl CloudObjectTypeAndId {
    pub fn from_id_and_type(id: SyncId, object_type: ObjectType) -> Self {
        match object_type {
            ObjectType::Notebook => Self::Notebook(id),
            ObjectType::Workflow => Self::Workflow(id),
            ObjectType::Folder => Self::Folder(id),
            ObjectType::GenericStringObject(format) => Self::GenericStringObject {
                object_type: format,
                id,
            },
        }
    }

    pub fn uid(self) -> ObjectUid {
        match self {
            Self::Notebook(id) => id.uid(),
            Self::Workflow(id) => id.uid(),
            Self::Folder(id) => id.uid(),
            Self::GenericStringObject { id, .. } => id.uid(),
        }
    }

    pub fn sync_id(self) -> SyncId {
        match self {
            Self::Notebook(id)
            | Self::Workflow(id)
            | Self::Folder(id)
            | Self::GenericStringObject { id, .. } => id,
        }
    }

    pub fn sqlite_uid_hash(self) -> HashedSqliteId {
        match self {
            Self::Notebook(id) => id.sqlite_uid_hash(ObjectIdType::Notebook),
            Self::Workflow(id) => id.sqlite_uid_hash(ObjectIdType::Workflow),
            Self::Folder(id) => id.sqlite_uid_hash(ObjectIdType::Folder),
            Self::GenericStringObject { object_type: _, id } => {
                id.sqlite_uid_hash(ObjectIdType::GenericStringObject)
            }
        }
    }

    pub fn object_id_type(&self) -> ObjectIdType {
        match self {
            Self::Notebook(_) => ObjectIdType::Notebook,
            Self::Workflow(_) => ObjectIdType::Workflow,
            Self::GenericStringObject { .. } => ObjectIdType::GenericStringObject,
            Self::Folder(_) => ObjectIdType::Folder,
        }
    }

    pub fn object_type(&self) -> ObjectType {
        match self {
            Self::Notebook(_) => ObjectType::Notebook,
            Self::Workflow(_) => ObjectType::Workflow,
            Self::Folder(_) => ObjectType::Folder,
            Self::GenericStringObject { object_type, .. } => {
                ObjectType::GenericStringObject(*object_type)
            }
        }
    }

    pub fn as_folder_id(self) -> Option<SyncId> {
        match self {
            Self::Notebook(_) => None,
            Self::Workflow(_) => None,
            Self::GenericStringObject { .. } => None,
            Self::Folder(f) => Some(f),
        }
    }

    pub fn as_notebook_id(self) -> Option<SyncId> {
        match self {
            Self::Notebook(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_generic_string_object_id(self) -> Option<SyncId> {
        match self {
            Self::GenericStringObject { object_type: _, id } => Some(id),
            _ => None,
        }
    }

    pub fn has_server_id(self) -> bool {
        matches!(
            self,
            Self::Notebook(SyncId::ServerId(_))
                | Self::Workflow(SyncId::ServerId(_))
                | Self::Folder(SyncId::ServerId(_))
                | Self::GenericStringObject {
                    id: SyncId::ServerId(_),
                    ..
                }
        )
    }

    pub fn server_id(self) -> Option<ServerId> {
        match self {
            Self::Notebook(SyncId::ServerId(notebook_id)) => Some(notebook_id),
            Self::Workflow(SyncId::ServerId(workflow_id)) => Some(workflow_id),
            Self::Folder(SyncId::ServerId(folder_id)) => Some(folder_id),
            Self::GenericStringObject {
                id: SyncId::ServerId(json_object_id),
                ..
            } => Some(json_object_id),
            _ => None,
        }
    }

    pub fn drive_row_position_id(self) -> String {
        format!("WarpDriveRow_{}", self.uid())
    }

    pub fn from_generic_string_object(object_type: GenericStringObjectFormat, id: SyncId) -> Self {
        Self::GenericStringObject { object_type, id }
    }
}

pub fn should_auto_open_welcome_folder(app: &mut AppContext) -> bool {
    app.private_user_preferences()
        .read_value(settings::HAS_AUTO_OPENED_WELCOME_FOLDER)
        .unwrap_or_default()
        .and_then(|s| serde_json::from_str(&s).ok())
        .map(|has_opened: bool| !has_opened)
        .unwrap_or(true)
}

pub fn write_has_auto_opened_welcome_folder_to_user_defaults(app: &mut AppContext) {
    let _ = app
        .private_user_preferences()
        .write_value(settings::HAS_AUTO_OPENED_WELCOME_FOLDER, true.to_string());
}

/// Enum used for sorting elements in the Warp Drive Index (and potentially other places).
/// In the future it can be used to add other options (like, by name or by author), and exposed to
/// users in the index.
#[derive(
    Default,
    PartialEq,
    Eq,
    Hash,
    Clone,
    Copy,
    Debug,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Sort order for Warp Drive items.",
    rename_all = "snake_case"
)]
pub enum DriveSortOrder {
    /// Sort by newest revision first in main index, most recently trashed in trash index
    #[default]
    ByTimestamp,
    /// A => Z
    AlphabeticalDescending,
    /// Z => A
    AlphabeticalAscending,
    /// Sort by object type, with folders first
    ByObjectType,
}

impl DriveSortOrder {
    /// Returns the comparator that can be used for sorting items returned by
    /// CloudModel::cloud_objects_in_space, for example (so more specifically, on the iterator of
    /// type Iterator<Item = &'_ dyn CloudObject>)
    pub fn sort_by<'a>(
        &self,
        cloud_model: &'a CloudViewModel,
        update_timestamp: UpdateTimestamp,
        app: &'a AppContext,
    ) -> Box<SortByComparator<'a>> {
        match self {
            // Sorts newly-created objects to be at the top of the list
            Self::ByTimestamp => Box::new(
                move |a: &&dyn CloudObject, b: &&dyn CloudObject| -> Ordering {
                    cloud_model
                        .object_sorting_timestamp(*a, update_timestamp, app)
                        .cmp(&cloud_model.object_sorting_timestamp(*b, update_timestamp, app))
                        .reverse()
                },
            ),
            Self::AlphabeticalDescending => Box::new(
                move |a: &&dyn CloudObject, b: &&dyn CloudObject| -> Ordering {
                    a.display_name()
                        .to_lowercase()
                        .cmp(&b.display_name().to_lowercase())
                },
            ),
            Self::AlphabeticalAscending => Box::new(
                move |a: &&dyn CloudObject, b: &&dyn CloudObject| -> Ordering {
                    b.display_name()
                        .to_lowercase()
                        .cmp(&a.display_name().to_lowercase())
                },
            ),
            Self::ByObjectType => Box::new(
                move |a: &&dyn CloudObject, b: &&dyn CloudObject| -> Ordering {
                    let order = |obj: &&dyn CloudObject| match obj.object_type() {
                        ObjectType::Folder => 0,
                        ObjectType::GenericStringObject(_) => 1,
                        ObjectType::Notebook => 2,
                        ObjectType::Workflow => {
                            let Some(workflow) = obj.as_any().downcast_ref::<CloudWorkflow>()
                            else {
                                return 3;
                            };

                            if workflow.model().data.is_agent_mode_workflow() {
                                4
                            } else {
                                3
                            }
                        }
                    };

                    // First compare by object type ordering, then by display name alphabetically if equal
                    order(a).cmp(&order(b)).then_with(|| {
                        a.display_name()
                            .to_lowercase()
                            .cmp(&b.display_name().to_lowercase())
                    })
                },
            ),
        }
    }

    /// Returns the text that is used to display the sorting option in the KnowledgeIndex's sorting menu
    pub fn menu_text(&self, index_variant: DriveIndexVariant) -> &str {
        match (self, index_variant) {
            (Self::ByTimestamp, DriveIndexVariant::MainIndex) => "Last updated",
            (Self::ByTimestamp, DriveIndexVariant::Trash) => "Last trashed",
            (Self::AlphabeticalDescending, _) => "A to Z",
            (Self::AlphabeticalAscending, _) => "Z to A",
            (Self::ByObjectType, _) => "Type",
        }
    }
}
