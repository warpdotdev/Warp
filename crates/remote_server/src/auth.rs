use std::sync::Arc;
use warpui::r#async::BoxFuture;
type GetAuthTokenFn = dyn Fn() -> BoxFuture<'static, Option<String>> + Send + Sync;
type RemoteServerIdentityKeyFn = dyn Fn() -> String + Send + Sync;
type GetStringFn = dyn Fn() -> String + Send + Sync;
type GetBoolFn = dyn Fn() -> bool + Send + Sync;

/// App-supplied authentication and preference context for transport-agnostic
/// remote-server code.
///
/// Bearer tokens are delivered only through protocol messages. Identity keys
/// are non-secret stable partition keys used to select the remote daemon's
/// socket/PID directory.
///
/// User identity (user_id, user_email) and privacy preferences
/// (crash_reporting_enabled) are forwarded to the daemon via the `Initialize`
/// handshake so it can configure Sentry crash reporting.
///
/// This keeps the `remote_server` crate decoupled from app-side auth/server API
/// types while still allowing initial connect and reconnect handshakes to fetch
/// the current app credential and preferences.
#[derive(Clone)]
pub struct RemoteServerAuthContext {
    get_auth_token: Arc<GetAuthTokenFn>,
    remote_server_identity_key: Arc<RemoteServerIdentityKeyFn>,
    get_user_id: Arc<GetStringFn>,
    get_user_email: Arc<GetStringFn>,
    get_crash_reporting_enabled: Arc<GetBoolFn>,
}

impl RemoteServerAuthContext {
    pub fn new(
        get_auth_token: impl Fn() -> BoxFuture<'static, Option<String>> + Send + Sync + 'static,
        remote_server_identity_key: impl Fn() -> String + Send + Sync + 'static,
        get_user_id: impl Fn() -> String + Send + Sync + 'static,
        get_user_email: impl Fn() -> String + Send + Sync + 'static,
        get_crash_reporting_enabled: impl Fn() -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            get_auth_token: Arc::new(get_auth_token),
            remote_server_identity_key: Arc::new(remote_server_identity_key),
            get_user_id: Arc::new(get_user_id),
            get_user_email: Arc::new(get_user_email),
            get_crash_reporting_enabled: Arc::new(get_crash_reporting_enabled),
        }
    }

    pub fn get_auth_token(&self) -> BoxFuture<'static, Option<String>> {
        (self.get_auth_token)()
    }

    pub fn remote_server_identity_key(&self) -> String {
        (self.remote_server_identity_key)()
    }

    pub fn get_user_id(&self) -> String {
        (self.get_user_id)()
    }

    pub fn get_user_email(&self) -> String {
        (self.get_user_email)()
    }

    pub fn get_crash_reporting_enabled(&self) -> bool {
        (self.get_crash_reporting_enabled)()
    }
}
