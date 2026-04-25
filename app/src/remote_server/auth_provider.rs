use std::sync::Arc;

use remote_server::auth::AuthProvider;
use warpui::r#async::BoxFuture;

use crate::auth::auth_state::AuthState;
use crate::server::server_api::auth::AuthClient;

/// App-side implementation of remote-server auth context.
pub struct ServerApiAuthProvider {
    auth_state: Arc<AuthState>,
    auth_client: Arc<dyn AuthClient>,
}

impl ServerApiAuthProvider {
    pub fn new(auth_state: Arc<AuthState>, auth_client: Arc<dyn AuthClient>) -> Self {
        Self {
            auth_state,
            auth_client,
        }
    }

    fn use_authenticated_user_identity(&self) -> bool {
        self.auth_state.is_logged_in() && !self.auth_state.is_user_anonymous().unwrap_or(true)
    }
}

impl AuthProvider for ServerApiAuthProvider {
    fn get_auth_token(&self) -> BoxFuture<'static, Option<String>> {
        if !self.use_authenticated_user_identity() {
            return Box::pin(async { None });
        }

        let auth_client = self.auth_client.clone();
        Box::pin(async move {
            match auth_client.get_or_refresh_access_token().await {
                Ok(token) => token.bearer_token(),
                Err(_) => None,
            }
        })
    }

    fn remote_server_identity_key(&self) -> String {
        if self.use_authenticated_user_identity() {
            self.auth_state
                .user_id()
                .map(|uid| uid.as_string())
                .unwrap_or_else(|| self.auth_state.anonymous_id())
        } else {
            self.auth_state.anonymous_id()
        }
    }
}
