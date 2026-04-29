// strip(tier-3): the firebase crate is gone, but its two API-response
// types are still referenced by auth.rs and a test. They're plain
// schema structs - inlined here so the auth code keeps compiling. With
// skip_login enabled and the http_client warp.dev block, no Firebase
// request ever fires at runtime, so these types are effectively dead.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirebaseError {
    pub code: i32,
    pub message: String,
}

impl std::error::Error for FirebaseError {}

impl std::fmt::Display for FirebaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Firebase request failed with status {} and message: {}",
            self.code, self.message
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FetchAccessTokenResponse {
    Success {
        #[serde(alias = "expiresIn")]
        expires_in: String,

        #[serde(alias = "idToken")]
        id_token: String,

        #[serde(alias = "refreshToken")]
        refresh_token: String,
    },
    Error {
        error: FirebaseError,
    },
}
