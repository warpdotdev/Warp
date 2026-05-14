//! Supporting types for persisting cloud objects to SQLite.

use anyhow::anyhow;
use diesel::{result::Error, SqliteConnection};
use serde::{Deserialize, Serialize};
use warp_core::features::FeatureFlag;

use crate::{
    auth::UserUid,
    cloud_object::{
        LinkSharing, ObjectIdType, ObjectType, Owner, ServerObjectContainer, StoredObjectGuest,
        StoredObjectMetadata, StoredObjectPermissions,
    },
    drive::sharing::{SharingAccessLevel, Subject, TeamKind, UserKind},
    persistence::{model::ObjectMetadata, schema},
    server::ids::ServerId,
};
use persistence::model::{NewObjectMetadata, NewObjectPermissions};

pub type StoredObjectId = i32;
pub type CreateStoredObjectFn =
    Box<dyn FnOnce(&mut SqliteConnection) -> Result<StoredObjectId, Error>>;
pub type UpdateStoredObjectFn =
    Box<dyn FnOnce(&mut SqliteConnection, StoredObjectId) -> Result<(), Error>>;

pub fn upsert_stored_object(
    conn: &mut SqliteConnection,
    cloud_object_type: ObjectType,
    sync_id: crate::server::ids::SyncId,
    cloud_object_metadata: StoredObjectMetadata,
    cloud_object_permissions: StoredObjectPermissions,
    create_object_fn: CreateStoredObjectFn,
    update_object_fn: UpdateStoredObjectFn,
) -> Result<(), Error> {
    use schema::object_metadata::dsl::{
        client_id, current_editor, folder_id, is_pending, last_editor_uid,
        metadata_last_updated_ts, object_metadata, revision_ts, server_id, trashed_ts,
    };
    use schema::object_permissions::dsl::{
        anyone_with_link_access_level, anyone_with_link_source, object_guests, object_metadata_id,
        object_permissions, permissions_last_updated_at, subject_id, subject_type, subject_uid,
    };

    use diesel::prelude::*;

    let (subject_type_value, subject_id_value, subject_uid_value) =
        match cloud_object_permissions.owner {
            Owner::User { user_uid } => ("USER", Some(user_uid.to_string()), user_uid.to_string()),
            Owner::Team { team_uid } => ("TEAM", None, team_uid.to_string()),
        };
    let permissions_ts = cloud_object_permissions
        .permissions_last_updated_ts
        .map(|ts| ts.timestamp_micros());
    let guests = if FeatureFlag::SharedWithMe.is_enabled() {
        match encode_guests(&cloud_object_permissions.guests) {
            Ok(guests) => Some(guests),
            Err(err) => {
                log::warn!("Unable to encode guests: {err:#}");
                None
            }
        }
    } else {
        None
    };
    let (anyone_with_link_access_level_value, anyone_with_link_source_value) =
        if FeatureFlag::SharedWithMe.is_enabled() {
            match cloud_object_permissions
                .anyone_with_link
                .as_ref()
                .map(encode_link_sharing)
            {
                Some(Ok((access_level, source))) => (Some(access_level), source),
                Some(Err(err)) => {
                    log::warn!("Unable to encode link-sharing setting: {err:#}");
                    (None, None)
                }
                None => (None, None),
            }
        } else {
            (None, None)
        };

    let revision = cloud_object_metadata
        .revision
        .as_ref()
        .map(|r| r.timestamp_micros());
    let has_pending_content_changes = cloud_object_metadata.has_pending_content_changes();
    let hashed_sync_id = sync_id.sqlite_uid_hash(cloud_object_type.into());
    let metadata_filter = object_metadata
        .filter(client_id.eq(Some(hashed_sync_id.as_str())))
        .or_filter(server_id.eq(Some(hashed_sync_id.as_str())));
    let metadata: Option<ObjectMetadata> = metadata_filter.first(conn).ok();

    match metadata {
        Some(metadata) => {
            update_object_fn(conn, metadata.shareable_object_id)?;

            let metadata_last_updated_at = cloud_object_metadata
                .metadata_last_updated_ts
                .map(|ts| ts.timestamp_micros());
            let trashed_timestamp = cloud_object_metadata
                .trashed_ts
                .map(|ts| ts.timestamp_micros());
            let folder_id_str = cloud_object_metadata
                .folder_id
                .map(|folder_sync_id| folder_sync_id.sqlite_uid_hash(ObjectIdType::Folder));

            diesel::update(metadata_filter)
                .set((
                    revision_ts.eq(revision),
                    is_pending.eq(has_pending_content_changes),
                    last_editor_uid.eq(cloud_object_metadata.last_editor_uid),
                ))
                .execute(conn)?;

            if !cloud_object_metadata
                .pending_changes_statuses
                .has_pending_metadata_change
            {
                diesel::update(metadata_filter)
                    .set((
                        metadata_last_updated_ts.eq(metadata_last_updated_at),
                        trashed_ts.eq(trashed_timestamp),
                        folder_id.eq(folder_id_str),
                        current_editor.eq(cloud_object_metadata.current_editor_uid),
                    ))
                    .execute(conn)?;
            }

            if !cloud_object_metadata
                .pending_changes_statuses
                .has_pending_permissions_change
            {
                let permissions_filter =
                    object_permissions.filter(object_metadata_id.eq(metadata.id));
                diesel::update(permissions_filter)
                    .set((
                        subject_type.eq(subject_type_value),
                        subject_id.eq(subject_id_value),
                        subject_uid.eq(subject_uid_value),
                        permissions_last_updated_at.eq(permissions_ts),
                        object_guests.eq(guests),
                        anyone_with_link_access_level.eq(anyone_with_link_access_level_value),
                        anyone_with_link_source.eq(anyone_with_link_source_value),
                    ))
                    .execute(conn)?;
            }
        }
        None => {
            let object_id = create_object_fn(conn)?;
            let mut new_object_metadata = NewObjectMetadata {
                object_type: cloud_object_type.sqlite_object_type_as_str().to_string(),
                revision_ts: revision,
                shareable_object_id: object_id,
                is_pending: has_pending_content_changes,
                retry_count: 0,
                author_id: None,
                client_id: None,
                server_id: None,
                metadata_last_updated_ts: cloud_object_metadata
                    .metadata_last_updated_ts
                    .map(|ts| ts.timestamp_micros()),
                trashed_ts: cloud_object_metadata
                    .trashed_ts
                    .map(|ts| ts.timestamp_micros()),
                folder_id: cloud_object_metadata
                    .folder_id
                    .map(|sync_id| sync_id.sqlite_uid_hash(ObjectIdType::Folder)),
                is_welcome_object: cloud_object_metadata.is_welcome_object,
                creator_uid: cloud_object_metadata.creator_uid,
                last_editor_uid: cloud_object_metadata.last_editor_uid,
                current_editor: cloud_object_metadata.current_editor_uid,
            };

            match sync_id {
                crate::server::ids::SyncId::ClientId(_) => {
                    new_object_metadata.client_id = Some(hashed_sync_id);
                }
                crate::server::ids::SyncId::ServerId(_) => {
                    new_object_metadata.server_id = Some(hashed_sync_id);
                }
            }
            diesel::insert_into(schema::object_metadata::dsl::object_metadata)
                .values(new_object_metadata)
                .execute(conn)?;

            let metadata_id: i32 = schema::object_metadata::dsl::object_metadata
                .select(schema::object_metadata::dsl::id)
                .order(schema::object_metadata::dsl::id.desc())
                .first(conn)?;

            let new_object_permissions = NewObjectPermissions {
                object_metadata_id: metadata_id,
                subject_type: subject_type_value.to_owned(),
                subject_id: subject_id_value,
                subject_uid: subject_uid_value,
                permissions_last_updated_at: permissions_ts,
                object_guests: guests,
                anyone_with_link_access_level: anyone_with_link_access_level_value,
                anyone_with_link_source: anyone_with_link_source_value,
            };
            diesel::insert_into(schema::object_permissions::dsl::object_permissions)
                .values(new_object_permissions)
                .execute(conn)?;
        }
    }

    Ok(())
}

pub fn decode_link_sharing(
    encoded_access_level: &str,
    encoded_source: Option<&[u8]>,
) -> anyhow::Result<LinkSharing> {
    let access_level = encoded_access_level.parse()?;
    let source = encoded_source.map(bincode::deserialize).transpose()?;
    Ok(LinkSharing {
        access_level,
        source,
    })
}

pub fn encode_link_sharing(
    link_sharing: &LinkSharing,
) -> anyhow::Result<(&'static str, Option<Vec<u8>>)> {
    let source = link_sharing
        .source
        .as_ref()
        .map(bincode::serialize)
        .transpose()?;
    Ok((link_sharing.access_level.to_serializable_value(), source))
}

pub fn decode_guests(encoded_guests: &[u8]) -> anyhow::Result<Vec<StoredObjectGuest>> {
    let persisted_guests = bincode::deserialize::<Vec<PersistedGuest>>(encoded_guests)?;
    Ok(persisted_guests
        .into_iter()
        .map(PersistedGuest::into_stored_object_guest)
        .collect())
}

pub fn encode_guests(guests: &[StoredObjectGuest]) -> anyhow::Result<Vec<u8>> {
    let persisted_guests = guests
        .iter()
        .map(PersistedGuest::try_from_stored_object_guest)
        .collect::<anyhow::Result<Vec<PersistedGuest>>>()?;
    Ok(bincode::serialize(&persisted_guests)?)
}

#[derive(Serialize, Deserialize)]
struct PersistedGuest {
    subject: PersistedSubject,
    access_level: SharingAccessLevel,
    source: Option<ServerObjectContainer>,
}

#[derive(Serialize, Deserialize)]
enum PersistedSubject {
    User { user_uid: String },
    PendingUser { email: Option<String> },
    Team { team_uid: ServerId },
}

impl PersistedGuest {
    pub fn into_stored_object_guest(self) -> StoredObjectGuest {
        StoredObjectGuest {
            subject: self.subject.into_subject(),
            access_level: self.access_level,
            source: self.source,
        }
    }

    pub fn try_from_stored_object_guest(guest: &StoredObjectGuest) -> anyhow::Result<Self> {
        Ok(PersistedGuest {
            subject: PersistedSubject::try_from_subject(&guest.subject)?,
            access_level: guest.access_level,
            source: guest.source,
        })
    }
}

impl PersistedSubject {
    pub fn into_subject(self) -> Subject {
        match self {
            PersistedSubject::User { user_uid } => {
                Subject::User(UserKind::Account(UserUid::new(&user_uid)))
            }
            PersistedSubject::PendingUser { email } => Subject::PendingUser { email },
            PersistedSubject::Team { team_uid } => Subject::Team(TeamKind::Team { team_uid }),
        }
    }

    pub fn try_from_subject(subject: &Subject) -> anyhow::Result<Self> {
        match subject {
            Subject::User(user_kind) => match user_kind {
                UserKind::Account(user_uid) => Ok(PersistedSubject::User {
                    user_uid: user_uid.to_string(),
                }),
            },
            Subject::PendingUser { email } => Ok(PersistedSubject::PendingUser {
                email: email.clone(),
            }),
            Subject::Team(team_kind) => match team_kind {
                TeamKind::Team { team_uid } => Ok(PersistedSubject::Team {
                    team_uid: *team_uid,
                }),
            },
            Subject::AnyoneWithLink(_) => Err(anyhow!("Anyone with the link not supported")),
        }
    }
}

#[cfg(test)]
#[path = "cloud_object_tests.rs"]
mod tests;
