use std::{fmt, sync::LazyLock};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 本地用户唯一标识。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserUid(lasso::Spur);

static USER_UID_INTERNER: LazyLock<lasso::ThreadedRodeo<lasso::Spur>> =
    LazyLock::new(lasso::ThreadedRodeo::new);

impl Default for UserUid {
    fn default() -> Self {
        Self::new("")
    }
}

impl UserUid {
    pub fn new(uid: &str) -> Self {
        Self(USER_UID_INTERNER.get_or_intern(uid))
    }

    pub fn as_str(&self) -> &str {
        USER_UID_INTERNER.resolve(&self.0)
    }

    pub fn as_string(&self) -> String {
        self.as_str().to_string()
    }
}

impl fmt::Display for UserUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for UserUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UserUid(")?;
        f.write_str(self.as_str())?;
        f.write_str(")")
    }
}

impl Serialize for UserUid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for UserUid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct UidVisitor;

        impl serde::de::Visitor<'_> for UidVisitor {
            type Value = UserUid;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a user UID")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(UserUid::new(v))
            }
        }

        deserializer.deserialize_str(UidVisitor)
    }
}
