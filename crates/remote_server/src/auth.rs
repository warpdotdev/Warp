use std::sync::Arc;
use warpui::r#async::BoxFuture;
type GetAuthTokenFn = dyn Fn() -> BoxFuture<'static, Option<String>> + Send + Sync;
type RemoteServerIdentityKeyFn = dyn Fn() -> String + Send + Sync;

/// App-supplied authentication context for transport-agnostic remote-server code.
///
/// Bearer tokens are delivered only through protocol messages. Identity keys
/// are non-secret stable partition keys used to select the remote daemon's
/// socket/PID directory.
///
/// This keeps the `remote_server` crate decoupled from app-side auth/server API
/// types while still allowing initial connect and reconnect handshakes to fetch
/// the current app credential.
#[derive(Clone)]
pub struct RemoteServerAuthContext {
    get_auth_token: Arc<GetAuthTokenFn>,
    remote_server_identity_key: Arc<RemoteServerIdentityKeyFn>,
}

impl RemoteServerAuthContext {
    pub fn new(
        get_auth_token: impl Fn() -> BoxFuture<'static, Option<String>> + Send + Sync + 'static,
        remote_server_identity_key: impl Fn() -> String + Send + Sync + 'static,
    ) -> Self {
        Self {
            get_auth_token: Arc::new(get_auth_token),
            remote_server_identity_key: Arc::new(remote_server_identity_key),
        }
    }

    pub fn get_auth_token(&self) -> BoxFuture<'static, Option<String>> {
        (self.get_auth_token)()
    }

    pub fn remote_server_identity_key(&self) -> String {
        (self.remote_server_identity_key)()
    }
}
