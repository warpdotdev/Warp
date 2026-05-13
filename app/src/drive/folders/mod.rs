use super::items::folder::WarpDriveFolder;
use super::items::WarpDriveItem;
use super::ObjectTypeAndId;
use crate::{
    appearance::Appearance,
    cloud_object::{GenericStoredObject, ObjectType, SerializedModel, Space, StoredObjectModel},
    persistence::ModelEvent,
    server::ids::SyncId,
};

pub use crate::server::ids::FolderId;

/// The model for a `FolderObject`.
#[derive(Clone, Debug, PartialEq)]
pub struct FolderObjectModel {
    pub name: String,
    // TODO: since this is local only state, we should consider only surfacing it as part of the
    // ObjectStoreViewModel. Right now, every object-backed folder uses FolderObjectModel, which means
    // it hardcodes a value of `false` for this property since it can't know what the local state is.
    pub is_open: bool,
    pub is_warp_pack: bool,
}

impl FolderObjectModel {
    pub fn new(name: &str, is_warp_pack: bool) -> Self {
        Self {
            name: name.to_owned(),
            is_open: false,
            is_warp_pack,
        }
    }
}

/// `FolderObject` is an object-store backed folder.
pub type FolderObject = GenericStoredObject<FolderId, FolderObjectModel>;

impl StoredObjectModel for FolderObjectModel {
    type StoredObjectType = FolderObject;
    type IdType = FolderId;

    fn model_type_name(&self) -> &'static str {
        "Folder"
    }

    fn object_type(&self) -> ObjectType {
        ObjectType::Folder
    }

    fn object_type_and_id(&self, id: SyncId) -> ObjectTypeAndId {
        ObjectTypeAndId::Folder(id)
    }

    fn display_name(&self) -> String {
        self.name.clone()
    }

    fn upsert_event(&self, folder: &FolderObject) -> ModelEvent {
        ModelEvent::UpsertFolder {
            folder: folder.clone(),
        }
    }

    fn bulk_upsert_event(objects: &[FolderObject]) -> ModelEvent {
        ModelEvent::UpsertFolders(objects.to_vec())
    }

    fn should_update_after_server_conflict(&self) -> bool {
        false
    }

    fn serialized(&self) -> SerializedModel {
        SerializedModel::new(self.name.to_owned())
    }

    fn can_move_to_space(&self, current_space: Space, new_space: Space) -> bool {
        // We don't currently support moving folders across spaces.
        current_space == new_space
    }

    fn supports_linking(&self) -> bool {
        true
    }

    fn renders_in_warp_drive(&self) -> bool {
        true
    }

    fn to_warp_drive_item(
        &self,
        id: SyncId,
        _appearance: &Appearance,
        folder: &FolderObject,
    ) -> Option<Box<dyn WarpDriveItem>> {
        Some(Box::new(WarpDriveFolder::new(
            self.object_type_and_id(id),
            folder.clone(),
        )))
    }
}
