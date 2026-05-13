use std::fmt;

use itertools::Itertools;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::cloud_object::ObjectIdType;

/// Convert ID enums into and from a hashed UUID.
pub trait HashableId: Sized + Send + Sync {
    fn to_hash(&self) -> String;
    fn from_hash(hash: &str) -> Option<Self>;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(description = "A client-generated unique identifier.")]
pub struct ClientId(Uuid);

impl HashableId for ClientId {
    fn to_hash(&self) -> String {
        self.to_string()
    }

    fn from_hash(hash: &str) -> Option<ClientId> {
        hash.strip_prefix("Client-")
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(ClientId)
    }
}

impl ClientId {
    pub fn new() -> ClientId {
        Self(Uuid::new_v4())
    }

    pub fn sqlite_hash(&self) -> String {
        self.to_string()
    }
}

impl Default for ClientId {
    fn default() -> Self {
        ClientId::new()
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Client-{}", self.0)
    }
}

impl From<String> for ClientId {
    fn from(s: String) -> Self {
        ClientId::from_hash(&s).unwrap_or_default()
    }
}

/// Identifier for a local object that may still carry a legacy server-style ID.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, schemars::JsonSchema)]
#[schemars(description = "Identifier for a local object.")]
pub enum SyncId {
    /// Locally-generated identifier.
    #[schemars(description = "A locally-generated identifier for an object.")]
    ClientId(ClientId),
    /// Legacy server-style identifier retained for existing persisted objects and links.
    #[schemars(description = "A legacy server-style identifier.")]
    ServerId(ServerId),
}

impl SyncId {
    pub fn from_object_id<K>(id: K) -> Self
    where
        K: ToServerId,
    {
        Self::ServerId(id.to_server_id())
    }

    pub fn uid(&self) -> ObjectUid {
        match self {
            Self::ClientId(id) => id.to_string(),
            Self::ServerId(id) => id.uid(),
        }
    }

    pub fn sqlite_uid_hash(&self, object_id_type: ObjectIdType) -> String {
        match self {
            SyncId::ClientId(id) => id.sqlite_hash(),
            SyncId::ServerId(id) => id.sqlite_type_and_uid_hash(object_id_type),
        }
    }

    /// Extract the legacy server-style ID, if present.
    pub fn into_server(self) -> Option<ServerId> {
        match self {
            Self::ServerId(id) => Some(id),
            Self::ClientId(_) => None,
        }
    }

    pub fn into_client(self) -> Option<ClientId> {
        match self {
            Self::ServerId(_) => None,
            Self::ClientId(id) => Some(id),
        }
    }
}

impl settings_value::SettingsValue for SyncId {}

impl fmt::Display for SyncId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ServerId(id) => id.fmt(f),
            Self::ClientId(id) => id.fmt(f),
        }
    }
}

impl From<ServerId> for SyncId {
    fn from(id: ServerId) -> SyncId {
        SyncId::ServerId(id)
    }
}

impl Serialize for SyncId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            SyncId::ServerId(server_id) => server_id.serialize(serializer),
            SyncId::ClientId(client_id) => client_id.to_hash().serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for SyncId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;

        if let Some(hashed) = ClientId::from_hash(s.as_str()) {
            Ok(SyncId::ClientId(hashed))
        } else {
            Ok(SyncId::ServerId(ServerId::from_string_lossy(s)))
        }
    }
}

const SERVER_ID_LENGTH: usize = 22;

/// String-based legacy object ID of length SERVER_ID_LENGTH.
#[derive(Clone, Copy, Default, Hash, PartialEq, Eq, schemars::JsonSchema)]
#[schemars(description = "A legacy string-based unique identifier.")]
pub struct ServerId([char; SERVER_ID_LENGTH]);

/// Used to index into the local object model and persisted metadata rows.
pub type ObjectUid = String;

/// Corresponds to what is stored for a given object id within the local sqlite database.
pub type HashedSqliteId = String;

#[derive(Debug, thiserror::Error)]
pub enum ParseServerIdError {
    #[error("ServerId must be exactly {SERVER_ID_LENGTH} characters, got {len}")]
    InvalidLength { len: usize },
}

#[allow(clippy::result_unit_err)]
pub fn parse_sqlite_id_to_uid(hashed_sqlite_id: HashedSqliteId) -> Result<ObjectUid, ()> {
    let Some(uid) = hashed_sqlite_id.split('-').next_back() else {
        return Err(());
    };

    Ok(uid.to_owned())
}

impl ServerId {
    pub fn from_string_lossy(id: impl AsRef<str>) -> Self {
        let id = id.as_ref();
        Self::try_from(id).unwrap_or_else(|err| {
            if cfg!(debug_assertions) {
                panic!("{err}");
            }
            let normalized = Self::normalize_id_str(id, 0);
            Self::try_from(normalized).expect("id should convert")
        })
    }

    fn normalize_id_str(input: &str, prefix_length: usize) -> String {
        let available_len = SERVER_ID_LENGTH - prefix_length;
        let truncated = if input.len() > available_len {
            &input[input.len() - available_len..]
        } else {
            input
        };
        format!("{truncated:0>available_len$}")
    }

    pub fn uid(&self) -> ObjectUid {
        (*self).into()
    }

    pub fn sqlite_type_and_uid_hash(&self, object_id_type: ObjectIdType) -> HashedSqliteId {
        format!("{}-{}", object_id_type.sqlite_prefix(), self)
    }
}

impl TryFrom<&str> for ServerId {
    type Error = ParseServerIdError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.chars().collect_array() {
            Some(chars) => Ok(Self(chars)),
            None => Err(ParseServerIdError::InvalidLength {
                len: s.chars().count(),
            }),
        }
    }
}

impl TryFrom<String> for ServerId {
    type Error = ParseServerIdError;

    fn try_from(id: String) -> Result<Self, Self::Error> {
        Self::try_from(id.as_str())
    }
}

#[cfg(test)]
impl From<i64> for ServerId {
    fn from(id: i64) -> Self {
        let prefix = "test_uid";
        let id_str = id.abs().to_string();
        let normalized = format!(
            "{}{}",
            prefix,
            Self::normalize_id_str(&id_str, prefix.len())
        );
        Self::try_from(normalized).expect("normalized string should always be valid")
    }
}

impl From<ServerId> for String {
    fn from(id: ServerId) -> String {
        String::from_iter(id.0)
    }
}

impl Serialize for ServerId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s: String = (*self).into();
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for ServerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        ServerId::try_from(s.as_str()).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for ServerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        use std::fmt::Write;
        for ch in self.0.iter() {
            f.write_char(*ch)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for ServerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "ServerId({self})")
    }
}

pub trait ToServerId {
    fn to_server_id(&self) -> ServerId;
}

#[macro_export]
macro_rules! server_id_traits {
    ($t:ty, $prefix:literal) => {
        #[cfg(test)]
        impl From<i64> for $t {
            fn from(id: i64) -> Self {
                Self(id.into())
            }
        }

        impl From<String> for $t {
            fn from(id: String) -> Self {
                Self($crate::server::ids::ServerId::from_string_lossy(id))
            }
        }

        impl From<$t> for String {
            fn from(id: $t) -> String {
                id.0.into()
            }
        }

        impl std::fmt::Display for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
                write!(f, "{}", self.0)
            }
        }

        impl From<$t> for $crate::server::ids::ServerId {
            fn from(id: $t) -> Self {
                id.0
            }
        }

        impl $crate::server::ids::HashableId for $t {
            fn to_hash(&self) -> String {
                format!("{}-{}", $prefix, self)
            }

            fn from_hash(hash: &str) -> Option<$t> {
                hash.strip_prefix(&format!("{}-", $prefix))
                    .map(|s| s.to_string().into())
            }
        }

        impl From<$crate::server::ids::ServerId> for $t {
            fn from(id: $crate::server::ids::ServerId) -> Self {
                Self(id)
            }
        }

        impl $crate::server::ids::ToServerId for $t {
            fn to_server_id(&self) -> $crate::server::ids::ServerId {
                self.0
            }
        }
    };
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash, Default)]
pub struct FolderId(ServerId);
server_id_traits! { FolderId, "Folder" }

impl From<FolderId> for SyncId {
    fn from(id: FolderId) -> Self {
        Self::ServerId(id.into())
    }
}

#[cfg(test)]
#[path = "ids_test.rs"]
mod tests;
