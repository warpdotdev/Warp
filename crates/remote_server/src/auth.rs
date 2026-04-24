use warpui::r#async::BoxFuture;

/// Provides the current local authentication context to transport-agnostic
/// remote-server code.
///
/// Bearer tokens are delivered only through protocol messages. Identity keys
/// are non-secret stable partition keys used to select the remote daemon's
/// socket/PID directory.
pub trait AuthProvider: Send + Sync + 'static {
    fn get_auth_token(&self) -> BoxFuture<'static, Option<String>>;

    fn actor_debug_label(&self) -> String;

    fn remote_server_identity_key(&self) -> String;
}
