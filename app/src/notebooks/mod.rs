pub mod active_notebook_data;
mod context_menu;
pub mod editor;
pub mod file;
pub mod link;
pub mod manager;
pub mod notebook;
mod styles;
pub mod telemetry;

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use warpui::AppContext;

use crate::{
    ai::document::ai_document_model::AIDocumentId,
    appearance::Appearance,
    cloud_object::{CloudModelType, GenericCloudObject, ObjectType, Owner},
    drive::{
        items::{notebook::WarpDriveNotebook, WarpDriveItem},
        ObjectTypeAndId,
    },
    persistence::ModelEvent,
    server::ids::{ServerId, SyncId},
};

use crate::cloud_object::SerializedModel;

/// Serialized representation of a notebook stored in local object persistence.
/// The AIDocumentID and ConversationID stay grouped with notebook content.
#[derive(Serialize, Deserialize)]
pub(crate) struct SerializedNotebook {
    pub(crate) data: String,
    pub(crate) ai_document_id: Option<String>,
    pub(crate) conversation_id: Option<String>,
}

/// `NotebookObject` is an object-store backed notebook.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NotebookObjectModel {
    pub title: String,
    pub data: String,
    pub ai_document_id: Option<AIDocumentId>,
    /// This is the server-generated conversation token, not the client-side AIConversationId.
    pub conversation_id: Option<String>,
}

pub type NotebookObject = GenericCloudObject<NotebookId, NotebookObjectModel>;

impl CloudModelType for NotebookObjectModel {
    type CloudObjectType = NotebookObject;
    type IdType = NotebookId;

    fn model_type_name(&self) -> &'static str {
        if self.ai_document_id.is_some() {
            "Plan"
        } else {
            "Notebook"
        }
    }

    fn object_type(&self) -> ObjectType {
        ObjectType::Notebook
    }

    fn object_type_and_id(&self, id: SyncId) -> ObjectTypeAndId {
        ObjectTypeAndId::Notebook(id)
    }

    fn display_name(&self) -> String {
        self.title.clone()
    }

    fn set_display_name(&mut self, name: &str) {
        name.clone_into(&mut self.title);
    }

    fn upsert_event(&self, notebook: &NotebookObject) -> ModelEvent {
        ModelEvent::UpsertNotebook {
            notebook: notebook.clone(),
        }
    }

    fn bulk_upsert_event(objects: &[NotebookObject]) -> ModelEvent {
        ModelEvent::UpsertNotebooks(objects.to_vec())
    }

    fn should_update_after_server_conflict(&self) -> bool {
        true
    }

    fn serialized(&self) -> SerializedModel {
        let serialized = SerializedNotebook {
            data: self.data.clone(),
            ai_document_id: self.ai_document_id.as_ref().map(|id| id.to_string()),
            conversation_id: self.conversation_id.clone(),
        };
        let json = serde_json::to_string(&serialized).expect("Failed to serialize notebook");
        SerializedModel::new(json)
    }

    fn renders_in_warp_drive(&self) -> bool {
        true
    }

    fn can_export(&self) -> bool {
        true
    }

    fn to_warp_drive_item(
        &self,
        id: SyncId,
        _appearance: &Appearance,
        notebook: &NotebookObject,
    ) -> Option<Box<dyn WarpDriveItem>> {
        Some(Box::new(WarpDriveNotebook::new(
            self.object_type_and_id(id),
            notebook.clone(),
            notebook.model().ai_document_id.is_some(),
        )))
    }
}

/// This is the notebook_id in the database associated with this notebook.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct NotebookId(ServerId);
crate::server_id_traits! { NotebookId, "Notebook" }

impl From<NotebookId> for SyncId {
    fn from(id: NotebookId) -> Self {
        Self::ServerId(id.into())
    }
}

/// A notebook location. Mainly, this lets us distinguish between object-backed and file-based notebooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum NotebookLocation {
    /// An object-backed notebook in the user's personal space.
    PersonalCloud,
    /// An object-backed notebook in a team space.
    Team,
    /// A notebook backed by a local file.
    LocalFile,
    /// A notebook backed by a remote file.
    RemoteFile,
}

impl From<Owner> for NotebookLocation {
    fn from(owner: Owner) -> Self {
        // TODO(ben): Account for shared objects in notebook telemetry.
        match owner {
            Owner::User { .. } => NotebookLocation::PersonalCloud,
            Owner::Team { .. } => NotebookLocation::Team,
        }
    }
}

/// Initialize notebooks-related keybindings.
pub fn init(app: &mut AppContext) {
    self::notebook::init(app);
    self::file::init(app);
    self::editor::view::init(app);
}

/// Post process a notebook's content read from an external system. This cleans up extra
/// whitespace, and, in the future, may filter out unsupported syntax extensions.
///
/// See CLD-944.
pub fn post_process_notebook(data: &str) -> String {
    // TODO(kevin): We should not strip out newlines in the code block.
    data.lines().filter(|line| !line.is_empty()).join("\n")
}

/// Translate a notebook's Markdown content into an external Markdown format.
///
/// This:
/// * Normalizes code block languages
/// * Includes extra context for embedded objects.
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
pub fn export_notebook(data: &str, ctx: &AppContext) -> anyhow::Result<String> {
    use warp_editor::content::{buffer::Buffer, markdown::MarkdownStyle};

    // Parse the Markdown directly rather than using [`Buffer::from_markdown`] so that we can
    // report errors to the exporter.
    let parsed = markdown_parser::parse_markdown(data)?;
    Ok(Buffer::export_to_markdown(
        parsed,
        Some(editor::notebook_embedded_item_conversion),
        MarkdownStyle::Export {
            app_context: Some(ctx),
            should_not_escape_markdown_punctuation: false,
        },
    ))
}
