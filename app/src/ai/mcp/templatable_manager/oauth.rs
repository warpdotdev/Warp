use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail};
use oauth2::{RefreshToken, TokenResponse as _};
use rmcp::transport::{
    auth::{
        AuthClient, AuthorizationManager, CredentialStore, InMemoryCredentialStore,
        OAuthClientConfig, OAuthState, OAuthTokenResponse, StoredCredentials,
    },
    AuthError, AuthorizationSession,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;
use warp_core::channel::ChannelState;
use warpui::ModelSpawner;
use warpui_extras::secure_storage::AppContextExt as _;

use super::{MCPServerState, TemplatableMCPServerManager};
use {crate::ai::mcp::FileBasedMCPManager, warpui::SingletonEntity};

pub(crate) const TEMPLATABLE_MCP_CREDENTIALS_KEY: &str = "TemplatableMcpCredentials";
pub(crate) const FILE_BASED_MCP_CREDENTIALS_KEY: &str = "FileBasedMcpCredentials";

/// The issuer URL for GitHub's OAuth provider.
const GITHUB_ISSUER: &str = "https://github.com/login/oauth";

static GITHUB_OAUTH_SCOPES: [&str; 7] = [
    "repo",
    "read:org",
    "gist",
    "notifications",
    "user",
    "project",
    "workflow",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedCredentials {
    client_id: String,
    client_secret: Option<String>,
    token_response: OAuthTokenResponse,
    /// Unix timestamp (seconds) when this token was received from the OAuth server.
    ///
    /// Required for rmcp's pre-emptive refresh: it computes remaining lifetime as
    /// `expires_in - (now - token_received_at)` and refreshes when below the buffer.
    /// Without this value the check is skipped, the cached token stays in use past
    /// its TTL, and the next request fails with 401 (see #8863).
    ///
    /// `None` for credentials persisted by older Warp versions; the next refresh
    /// will populate it via the credential-store save path.
    #[serde(default)]
    token_received_at: Option<u64>,
}

/// Returns the current Unix timestamp in seconds.
///
/// Mirrors rmcp's internal `now_epoch_secs` (which is private) so we can stamp
/// `token_received_at` consistently with the timestamps rmcp itself emits during
/// refresh.
fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Maps cloud MCP installation UUID to its OAuth credentials in secure storage.
pub type PersistedCredentialsMap = HashMap<Uuid, PersistedCredentials>;

// Maps a consistent hash of the installation to its persisted credentials
pub type FileBasedPersistedCredentialsMap = HashMap<u64, PersistedCredentials>;

/// A credential store that wraps [`InMemoryCredentialStore`] and persists token
/// updates to Warp's secure storage via a channel.
///
/// When rmcp auto-refreshes an expired access token at runtime, the rotated
/// tokens are only saved to the in-memory store by default. This wrapper
/// ensures they also get written back to secure storage so they survive app
/// restarts.
struct PersistingCredentialStore {
    inner: InMemoryCredentialStore,
    client_secret: Option<String>,
    persist_tx: async_channel::Sender<PersistedCredentials>,
}

impl PersistingCredentialStore {
    /// Per RFC 6749 §6, the authorization server MAY issue a new refresh token on
    /// refresh, but is not required to. Many OAuth providers (e.g. Figma) only
    /// issue a refresh token on the initial authorization grant and omit it from
    /// subsequent refresh responses. If we blindly persist the new token response,
    /// the refresh token is lost and the next session (or next in-process refresh)
    /// requires a full re-auth.
    ///
    /// When the new response omits a refresh token, carry forward the one already
    /// in the store. See: <https://datatracker.ietf.org/doc/html/rfc6749#section-6>
    async fn apply_refresh_token_carry_forward(&self, credentials: &mut StoredCredentials) {
        if credentials
            .token_response
            .as_ref()
            .is_none_or(|tr| tr.refresh_token().is_some())
        {
            return;
        }

        if let Some(prev_rt) = self
            .inner
            .load()
            .await
            .ok()
            .and_then(|opt| opt)
            .and_then(|prev| prev.token_response)
            .and_then(|prev_tr| prev_tr.refresh_token().cloned())
        {
            if let Some(tr) = credentials.token_response.as_mut() {
                // Carry forward the existing/previous refresh token, constructing new if needed
                tr.set_refresh_token(Some(RefreshToken::new(prev_rt.secret().to_string())));
            }
        }
    }
}

#[async_trait::async_trait]
impl CredentialStore for PersistingCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        self.inner.load().await
    }

    async fn save(&self, mut credentials: StoredCredentials) -> Result<(), AuthError> {
        self.apply_refresh_token_carry_forward(&mut credentials)
            .await;

        // Capture before `credentials` is consumed below.
        let token_received_at = credentials.token_received_at;

        self.inner.save(credentials.clone()).await?;

        if let Some(token_response) = credentials.token_response {
            let _ = self.persist_tx.try_send(PersistedCredentials {
                client_id: credentials.client_id,
                client_secret: self.client_secret.clone(),
                token_response,
                token_received_at,
            });
        }
        Ok(())
    }

    async fn clear(&self) -> Result<(), AuthError> {
        self.inner.clear().await
    }
}

/// Installs a [`PersistingCredentialStore`] on the given auth manager so that
/// runtime token auto-refreshes are written back to Warp's secure storage.
///
/// A background tokio task is spawned to receive credential updates and persist
/// them via the [`ModelSpawner`]. The task terminates when the auth manager (and
/// thus the credential store's sender) is dropped.
///
/// Note: this store is not responsible for the initial population of credentials.
/// Instead, the caller seeds the inner store with any existing credentials prior
/// to installation (see [`install_persisting_credential_store`]). This store's
/// sole role is to write token updates back to secure storage as they occur.
async fn install_persisting_credential_store(
    auth_manager: &mut AuthorizationManager,
    client_secret: Option<String>,
    spawner: ModelSpawner<TemplatableMCPServerManager>,
    installation_uuid: Uuid,
    token_received_at: Option<u64>,
) {
    let (persist_tx, persist_rx) = async_channel::unbounded();
    let store = PersistingCredentialStore {
        inner: InMemoryCredentialStore::new(),
        client_secret,
        persist_tx,
    };

    // Seed the new store with the current credentials so that subsequent
    // get_access_token() calls can find them. `token_received_at` must be
    // preserved across the seed; otherwise rmcp's pre-emptive refresh check
    // (which requires both `expires_in` and `token_received_at`) is skipped
    // for the lifetime of this store, and the cached token will be used past
    // its TTL until a request fails with 401 (#8863).
    if let Ok((client_id, Some(token_response))) = auth_manager.get_credentials().await {
        let _ = store
            .inner
            .save(StoredCredentials {
                client_id,
                token_response: Some(token_response),
                granted_scopes: Vec::new(),
                token_received_at,
            })
            .await;
    }

    auth_manager.set_credential_store(store);

    tokio::spawn(async move {
        while let Ok(credentials) = persist_rx.recv().await {
            if let Err(e) = spawner
                .spawn(move |manager, ctx| {
                    manager.save_credentials_to_secure_storage(ctx, installation_uuid, credentials);
                })
                .await
            {
                log::warn!("Failed to persist auto-refreshed MCP credentials: {e:?}");
            }
        }
    });
}

/// Context for OAuth authentication flows.
pub struct AuthContext {
    pub oauth_result_rx: async_channel::Receiver<CallbackResult>,
    pub spawner: ModelSpawner<TemplatableMCPServerManager>,
    pub uuid: Uuid,
    pub persisted_credentials: Option<PersistedCredentials>,
    /// Whether the client is running in headless/CLI mode.
    pub is_headless: bool,
    /// Whether this server was auto-discovered from a repo MCP configuration file.
    pub is_file_based: bool,
}

/// Result of OAuth callback.
#[derive(Debug, Clone)]
pub enum CallbackResult {
    Success { code: String, csrf_token: String },
    Error { error: Option<String> },
}

/// Makes an authenticated client for the given authorization server.
///
/// This takes in the URL of the resource to authenticate for, and uses that
/// to determine the authorization server.
///
/// Upon success, returns the client and a boolean indicating whether the user was required to
/// re-authenticate (e.g. re-log in).
pub async fn make_authenticated_client(
    resource_url: &str,
    auth_context: AuthContext,
) -> Result<(AuthClient<reqwest::Client>, bool), AuthError> {
    let AuthContext {
        oauth_result_rx,
        spawner,
        uuid,
        persisted_credentials,
        is_headless,
        is_file_based,
    } = auth_context;

    // Build the redirect URI using the channel's URL scheme.
    // Routing data (the server UUID) is passed via the OAuth `state` parameter instead
    // of the redirect URI so that the URI exactly matches what is registered during
    // Dynamic Client Registration, satisfying RFC 6749 §3.1.2.2 exact-match validation.
    let redirect_uri = format!("{}://mcp/oauth2callback", ChannelState::url_scheme());

    // Create the OAuth state machine.
    let mut oauth_state = OAuthState::new(resource_url, None).await?;

    // If we have cached credentials, use them.
    if let Some(credentials) = persisted_credentials {
        let provider = ChannelState::mcp_oauth_provider_by_client_id(&credentials.client_id);
        let client_secret = credentials
            .client_secret
            .or_else(|| provider.as_ref().map(|p| p.client_secret.to_string()));
        let cached_token_received_at = credentials.token_received_at;
        oauth_state
            .set_credentials(&credentials.client_id, credentials.token_response)
            .await?;
        if let OAuthState::Authorized(mut auth_manager) = oauth_state {
            // If this is a client for which we have a known client secret,
            // update our client config accordingly.
            if let Some(client_secret) = &client_secret {
                auth_manager.configure_client(OAuthClientConfig {
                    client_id: credentials.client_id.clone(),
                    client_secret: Some(client_secret.clone()),
                    scopes: vec![],
                    redirect_uri: redirect_uri.clone(),
                })?;
            }

            // GitHub does not issue refresh tokens for OAuth apps; their access tokens are valid
            // until the user explicitly revokes them.
            //
            // As such, if we have an access token for a GitHub server, we must assume it's valid.
            if provider.as_ref().is_some_and(|p| p.issuer == GITHUB_ISSUER) {
                return Ok((AuthClient::new(reqwest::Client::new(), auth_manager), false));
            }

            // Else, make sure we have an up-to-date access token.
            // We need to do this because our fork of rmcp does not properly detect expired tokens.
            // This is fixed in https://github.com/modelcontextprotocol/rust-sdk/pull/680
            //
            // Install the persisting credential store before refreshing so that
            // the refresh result is automatically written back to secure storage.
            // Pass the cached `token_received_at` so rmcp's pre-emptive refresh
            // check has the data it needs after this connect-time refresh — see
            // #8863 for what happens when this is `None`.
            install_persisting_credential_store(
                &mut auth_manager,
                client_secret,
                spawner.clone(),
                uuid,
                cached_token_received_at,
            )
            .await;
            match auth_manager.refresh_token().await {
                Ok(_) => {
                    return Ok((AuthClient::new(reqwest::Client::new(), auth_manager), false));
                }
                Err(e) => {
                    log::warn!("Failed to refresh token: {e:#}");

                    // We didn't have a valid auth token _and_ we could not refresh it, so
                    // we need to go through the OAuth flow again.
                    oauth_state = OAuthState::new(resource_url, None).await?;
                }
            }
        }
    }

    // If we're in headless mode and we reach here, it means we either have no credentials
    // or the cached credentials failed to refresh. Block interactive OAuth in headless mode.
    if is_headless {
        if is_file_based {
            log::warn!(
                "File-based MCP server {uuid} requires OAuth authentication; \
                 skipping in headless mode. To use this server, authenticate it \
                 in the Warp desktop app first."
            );
        }
        return Err(AuthError::AuthorizationFailed(
            "MCP server requires OAuth authentication. Please authenticate this server in the \
             Warp desktop app first, then try again."
                .to_string(),
        ));
    }

    // Start the authorization process with our custom redirect URI
    oauth_state
        .start_authorization(&[], &redirect_uri, Some("Warp"))
        .await?;

    let OAuthState::Session(AuthorizationSession {
        mut auth_manager, ..
    }) = oauth_state
    else {
        return Err(AuthError::InternalError(
            "OAuth state is not in the expected state".to_string(),
        ));
    };

    // With DCR (Dynamic Client Registration), we don't pass in explicit scopes; they are specified
    // during dynamic registration.
    //
    // For apps for which we have static client IDs (e.g. GitHub), we manually override scopes.
    let mut scopes: &[&str] = &[];

    let config = match auth_manager.register_client("Warp", &redirect_uri).await {
        Ok(config) => config,
        Err(err @ AuthError::RegistrationFailed(_)) => {
            // If we failed dynamic registration, check to see if this is an auth
            // server we have a static client ID for.

            // TODO(vorporeal): adjust APIs in rmcp so that we don't need to make this redundant
            // discover_metadata() call (as it gets made within start_authorization() but we can't
            // look at the results).
            let metadata = auth_manager.discover_metadata().await?;
            let provider = metadata
                .issuer
                .as_deref()
                .and_then(ChannelState::mcp_oauth_provider_by_issuer)
                .ok_or(err)?;

            if provider.issuer == GITHUB_ISSUER {
                scopes = &GITHUB_OAUTH_SCOPES;
            }

            OAuthClientConfig {
                client_id: provider.client_id.into_owned(),
                client_secret: Some(provider.client_secret.into_owned()),
                redirect_uri: redirect_uri.clone(),
                // This `scopes` field appears to be unused by rmcp as of 9/17/25 - we pass scopes
                // in construction of the authorization url below.
                scopes: vec![],
            }
        }
        Err(e) => return Err(e),
    };

    let client_secret = config.client_secret.clone();
    auth_manager.configure_client(config)?;

    let auth_url = auth_manager.get_authorization_url(scopes).await?;
    oauth_state = OAuthState::Session(AuthorizationSession {
        auth_manager,
        auth_url: auth_url.clone(),
        redirect_uri,
    });

    // Extract the CSRF token that rmcp embedded as the `state` query parameter in the
    // authorization URL. We register a csrf→uuid mapping on the manager so that
    // `handle_oauth_callback` can route the callback to the right server without
    // relying on `server_id` being present in the redirect URI.
    let csrf_state = Url::parse(&auth_url)
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "state")
                .map(|(_, v)| v.into_owned())
        })
        .unwrap_or_default();

    if let Err(e) = spawner
        .spawn(move |manager, ctx| {
            if !csrf_state.is_empty() {
                manager.pending_oauth_csrf.insert(csrf_state, uuid);
            }
            ctx.open_url(&auth_url);
            manager.change_server_state(uuid, MCPServerState::Authenticating, ctx);
        })
        .await
    {
        log::warn!("Failed to emit RequiresAuthentication state: {e:?}");
    }

    // Wait for the authorization code from the OAuth callback channel.
    let oauth_result = oauth_result_rx
        .recv()
        .await
        .map_err(|e| AuthError::InternalError(e.to_string()))?;

    let (code, csrf_token) = match &oauth_result {
        CallbackResult::Success { code, csrf_token } => (code, csrf_token),
        CallbackResult::Error { error } => {
            return Err(AuthError::AuthorizationFailed(
                error.as_deref().unwrap_or("unknown error").to_string(),
            ));
        }
    };

    // Handle the callback with the received authorization code and CSRF token.
    oauth_state.handle_callback(code, csrf_token).await?;

    // Save the credentials to secure storage. Stamp `token_received_at` so the
    // pre-emptive refresh check in rmcp has a timestamp to compute remaining
    // lifetime against on subsequent connects (without it, the cached token
    // would be used past its TTL — see #8863).
    let token_received_at = now_epoch_secs();
    let (client_id, token_response) = oauth_state.get_credentials().await?;
    if let Some(token_response) = token_response {
        let credentials = PersistedCredentials {
            client_id,
            client_secret: client_secret.clone(),
            token_response,
            token_received_at: Some(token_received_at),
        };
        spawner
            .spawn(move |manager, ctx| {
                manager.save_credentials_to_secure_storage(ctx, uuid, credentials);
            })
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))?;
    }

    let mut am = oauth_state.into_authorization_manager().ok_or_else(|| {
        AuthError::InternalError("Failed to create authorization manager".to_string())
    })?;

    install_persisting_credential_store(
        &mut am,
        client_secret,
        spawner,
        uuid,
        Some(token_received_at),
    )
    .await;

    Ok((AuthClient::new(reqwest::Client::new(), am), true))
}

impl TemplatableMCPServerManager {
    /// Handles an incoming OAuth callback URL.
    ///
    /// Routes the callback to the correct in-flight OAuth flow using the `state` query
    /// parameter (the CSRF token that rmcp embedded in the authorization URL). This avoids
    /// encoding routing data in the redirect URI, keeping it RFC 6749 §3.1.2.2 compliant.
    pub fn handle_oauth_callback(&mut self, url: &Url) -> anyhow::Result<()> {
        // Ensure the URL has the expected path
        if url.path() != "/oauth2callback" {
            bail!(
                "Invalid OAuth callback path: expected '/oauth2callback', got '{}'",
                url.path()
            );
        }

        let query_params: HashMap<_, _> = url.query_pairs().collect();

        let Some(state) = query_params.get("state") else {
            bail!("Missing 'state' parameter in OAuth callback");
        };

        let code = query_params.get("code");
        let error = query_params.get("error");

        let result = match code {
            Some(code) => CallbackResult::Success {
                code: code.to_string(),
                // Pass the state value through as the CSRF token; rmcp will validate it
                // against the token it stored when generating the authorization URL.
                csrf_token: state.to_string(),
            },
            None => CallbackResult::Error {
                error: error.map(|e| e.to_string()),
            },
        };

        let Some(&server_uuid) = self.pending_oauth_csrf.get(state.as_ref() as &str) else {
            bail!("No active OAuth flow found for state={state}");
        };

        let Some(server_info) = self.spawned_servers.get(&server_uuid) else {
            bail!("No spawned server found for uuid={server_uuid}");
        };

        warpui::r#async::block_on(server_info.oauth_result_tx.send(result)).map_err(|_| {
            anyhow!("Failed to send OAuth result to server {server_uuid} - receiver dropped")
        })?;

        self.pending_oauth_csrf.remove(state.as_ref() as &str);
        Ok(())
    }

    pub fn save_credentials_to_secure_storage(
        &mut self,
        app: &mut warpui::AppContext,
        installation_uuid: Uuid,
        credentials: PersistedCredentials,
    ) {
        if let Some(hash) = FileBasedMCPManager::as_ref(app).get_hash_by_uuid(installation_uuid) {
            self.file_based_server_credentials.insert(hash, credentials);
            write_to_secure_storage(
                app,
                FILE_BASED_MCP_CREDENTIALS_KEY,
                &self.file_based_server_credentials,
            );
            return;
        }

        if let Some(template_uuid) = self.get_template_uuid(installation_uuid) {
            self.server_credentials.insert(template_uuid, credentials);
            write_to_secure_storage(
                app,
                TEMPLATABLE_MCP_CREDENTIALS_KEY,
                &self.server_credentials,
            );
        } else {
            log::error!(
                "Corresponding file or cloud-based server not found for installation UUID {installation_uuid}"
            );
        }
    }

    pub fn delete_credentials_from_secure_storage(
        &mut self,
        installation_uuid: Uuid,
        app: &mut warpui::AppContext,
    ) {
        if let Some(template_uuid) = self.get_template_uuid(installation_uuid) {
            self.server_credentials.remove(&template_uuid);
            write_to_secure_storage(
                app,
                TEMPLATABLE_MCP_CREDENTIALS_KEY,
                &self.server_credentials,
            );
        } else {
            log::error!("No template UUID found for installation UUID {installation_uuid}");
        }
    }
}

/// Loads credentials from secure storage at the provided key.
pub(crate) fn load_credentials_from_secure_storage<T: DeserializeOwned + Default>(
    app: &mut warpui::AppContext,
    key: &str,
) -> T {
    app.secure_storage()
        .read_value(key)
        .inspect_err(|err| {
            if !matches!(err, warpui_extras::secure_storage::Error::NotFound) {
                log::warn!("Failed to read MCP credentials from secure storage: {err:#}");
            }
        })
        .ok()
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}

/// Writes credentials to secure storage at the provided key.
pub(crate) fn write_to_secure_storage<T: Serialize>(
    app: &mut warpui::AppContext,
    key: &str,
    credentials: &T,
) {
    match serde_json::to_string(credentials) {
        Ok(json) => {
            app.secure_storage()
                .write_value(key, &json)
                .inspect_err(|err| {
                    log::error!("Failed to write MCP credentials to secure storage: {err:#}")
                })
                .ok();
        }
        Err(err) => {
            log::error!("Failed to serialize MCP credentials for secure storage: {err:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::transport::auth::OAuthTokenResponse;

    /// Builds a minimal `OAuthTokenResponse` for tests, optionally with a refresh token.
    fn make_test_token_response(refresh_token: Option<&str>) -> OAuthTokenResponse {
        let mut json = serde_json::json!({
            "access_token": "test_access_token",
            "token_type": "bearer",
            "expires_in": 3600,
        });
        if let Some(rt) = refresh_token {
            json["refresh_token"] = serde_json::Value::String(rt.to_string());
        }
        serde_json::from_value(json).expect("OAuthTokenResponse deserialization")
    }

    /// Constructs a fresh `PersistingCredentialStore` plus the receiver side of its
    /// persist channel so tests can observe what would be written to secure storage.
    fn make_test_store(
        client_secret: Option<String>,
    ) -> (
        PersistingCredentialStore,
        async_channel::Receiver<PersistedCredentials>,
    ) {
        let (tx, rx) = async_channel::unbounded();
        let store = PersistingCredentialStore {
            inner: InMemoryCredentialStore::new(),
            client_secret,
            persist_tx: tx,
        };
        (store, rx)
    }

    #[test]
    fn persisted_credentials_round_trip_through_serde_preserves_received_at() {
        let original = PersistedCredentials {
            client_id: "client-abc".to_string(),
            client_secret: Some("shh".to_string()),
            token_response: make_test_token_response(Some("refresh-1")),
            token_received_at: Some(1_700_000_000),
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: PersistedCredentials = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.client_id, original.client_id);
        assert_eq!(parsed.client_secret, original.client_secret);
        assert_eq!(parsed.token_received_at, Some(1_700_000_000));
    }

    /// Backward compatibility: credentials persisted by older Warp versions do not
    /// have the `token_received_at` field. Deserializing them must succeed and
    /// default to `None` so the next refresh can populate it. Failing this test
    /// would mean every existing user loses their MCP OAuth tokens on upgrade.
    #[test]
    fn persisted_credentials_deserializes_legacy_format_without_received_at() {
        // Legacy format: no `token_received_at` field.
        let legacy_json = r#"{
            "client_id": "client-abc",
            "client_secret": null,
            "token_response": {
                "access_token": "old_access",
                "token_type": "bearer",
                "expires_in": 3600,
                "refresh_token": "old_refresh"
            }
        }"#;

        let parsed: PersistedCredentials =
            serde_json::from_str(legacy_json).expect("legacy format must deserialize");

        assert_eq!(parsed.client_id, "client-abc");
        assert_eq!(parsed.token_received_at, None);
    }

    /// Regression test for #8863. When rmcp persists refreshed credentials via
    /// `CredentialStore::save`, the `token_received_at` must be forwarded into
    /// the channel so the persisted (secure-storage) representation can stamp
    /// it. Without this, a restart would lose the timestamp and rmcp's
    /// pre-emptive refresh check would be permanently disabled for the cached
    /// session.
    #[tokio::test]
    async fn save_forwards_token_received_at_to_persist_channel() {
        let (store, rx) = make_test_store(Some("client_secret_xyz".to_string()));

        let credentials = StoredCredentials {
            client_id: "client-id".to_string(),
            token_response: Some(make_test_token_response(Some("refresh-1"))),
            granted_scopes: Vec::new(),
            token_received_at: Some(1_700_000_500),
        };

        store.save(credentials).await.expect("save succeeds");

        let persisted = rx.try_recv().expect("persist channel received credentials");
        assert_eq!(persisted.token_received_at, Some(1_700_000_500));
        assert_eq!(persisted.client_id, "client-id");
        assert_eq!(
            persisted.client_secret.as_deref(),
            Some("client_secret_xyz")
        );
    }

    /// Defensive: if rmcp ever calls `save` without a `token_received_at`
    /// (e.g., during initial credential set-up before refresh), we must
    /// propagate `None` rather than silently substituting a value.
    #[tokio::test]
    async fn save_forwards_none_when_received_at_is_none() {
        let (store, rx) = make_test_store(None);

        let credentials = StoredCredentials {
            client_id: "c".to_string(),
            token_response: Some(make_test_token_response(None)),
            granted_scopes: Vec::new(),
            token_received_at: None,
        };

        store.save(credentials).await.expect("save succeeds");

        let persisted = rx.try_recv().expect("persist channel received credentials");
        assert_eq!(persisted.token_received_at, None);
    }

    /// `save` only forwards a credentials snapshot to the persist channel when
    /// `token_response` is `Some`. This guards the existing branch from regression.
    #[tokio::test]
    async fn save_skips_persist_when_token_response_absent() {
        let (store, rx) = make_test_store(None);

        let credentials = StoredCredentials {
            client_id: "c".to_string(),
            token_response: None,
            granted_scopes: Vec::new(),
            token_received_at: Some(1_700_000_500),
        };

        store.save(credentials).await.expect("save succeeds");

        assert!(
            rx.try_recv().is_err(),
            "no PersistedCredentials should be sent when token_response is absent"
        );
    }

    /// The carry-forward of refresh tokens (when the OAuth server omits one
    /// from a refresh response) must not interfere with `token_received_at`
    /// propagation. Tests both behaviors in one save: the new credentials get
    /// the prior refresh token AND the new `token_received_at`.
    #[tokio::test]
    async fn save_carries_forward_refresh_token_and_preserves_received_at() {
        let (store, rx) = make_test_store(None);

        // Seed the inner store with prior credentials that have a refresh token.
        store
            .inner
            .save(StoredCredentials {
                client_id: "c".to_string(),
                token_response: Some(make_test_token_response(Some("prior-refresh-token"))),
                granted_scopes: Vec::new(),
                token_received_at: Some(1_699_000_000),
            })
            .await
            .expect("seed succeeds");

        // Now save NEW credentials that omit a refresh token, simulating a
        // refresh response from a server that does not rotate refresh tokens.
        let new_credentials = StoredCredentials {
            client_id: "c".to_string(),
            token_response: Some(make_test_token_response(None)),
            granted_scopes: Vec::new(),
            token_received_at: Some(1_700_000_500),
        };

        store.save(new_credentials).await.expect("save succeeds");

        let persisted = rx.try_recv().expect("persist channel received credentials");
        assert_eq!(
            persisted.token_received_at,
            Some(1_700_000_500),
            "newer received_at preserved"
        );
        assert_eq!(
            persisted
                .token_response
                .refresh_token()
                .map(|rt| rt.secret().to_string()),
            Some("prior-refresh-token".to_string()),
            "prior refresh token carried forward"
        );
    }

    /// `now_epoch_secs` returns a non-zero monotonic-ish value. Sanity check
    /// that the helper does what it claims and matches rmcp's own clock domain.
    #[test]
    fn now_epoch_secs_returns_recent_unix_time() {
        let now = now_epoch_secs();
        // Sanity: any timestamp produced by the running test process must be
        // after the OSS-release date (2026-04-28) and before the year 2200.
        assert!(now > 1_745_000_000, "epoch seconds must be a real time");
        assert!(
            now < 7_258_118_400,
            "epoch seconds must be before year 2200"
        );
    }
}
