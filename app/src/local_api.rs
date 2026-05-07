//! `LocalApiServer` — exposes a UDS-based control plane to the `wp` CLI.
//!
//! Background `ipc::Service` handlers run on the executor; each request is
//! forwarded over an `async_channel` to the main thread, where a stream
//! handler dispatches it as if a key binding had fired and replies via a
//! oneshot. This mirrors `iTermAPIServer`'s "background socket reads, main
//! thread mutations" pattern and avoids holding `TerminalModel` locks across
//! await points.
//!
//! # Same-UID hardening
//!
//! - The socket binds under `$TMPDIR` (per-user dir on macOS, mode 0700) and
//!   is chmod'd to 0600 immediately after bind.
//! - The address file is written 0600 and contains a per-session random
//!   cookie alongside the socket path. The server rejects requests whose
//!   cookie doesn't match.
//! - The dispatch queue is bounded; oversized `SendText` payloads are
//!   rejected at the boundary so the main thread can't be wedged by a
//!   misbehaving same-UID client.

use std::fs;
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::PathBuf;

use async_trait::async_trait;
use futures::channel::oneshot;
use ipc::{ConnectionAddress, ServerBuilder};
use rand::RngCore as _;
use warp_core::channel::ChannelState;
use warp_local_api::{
    address_publish_path_for, format_address_file, LocalApiEnvelope, LocalApiRequest,
    LocalApiResponse, LocalApiService, SplitDir, MAX_REQUEST_BYTES, MAX_SEND_TEXT_BYTES,
};
use warpui::windowing::WindowManager;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity, ViewHandle};

use crate::pane_group::pane::TerminalPaneId;
use crate::pane_group::{Direction, PaneGroup};
use crate::workspace::Workspace;

/// Bounded pending-command queue. 256 in-flight is generous for an
/// interactive same-UID API; over-burst clients get backpressure rather than
/// a memory blow-up.
const MAX_PENDING_COMMANDS: usize = 256;

type Command = (LocalApiRequest, oneshot::Sender<LocalApiResponse>);

#[derive(Clone)]
struct LocalApiServiceImpl {
    cookie: String,
    cmd_tx: async_channel::Sender<Command>,
}

#[async_trait]
impl ipc::ServiceImpl for LocalApiServiceImpl {
    type Service = LocalApiService;

    async fn handle_request(&self, envelope: LocalApiEnvelope) -> LocalApiResponse {
        if !cookies_eq(&self.cookie, &envelope.cookie) {
            return LocalApiResponse::Err("unauthenticated".into());
        }

        if let LocalApiRequest::SendText { ref text, .. } = envelope.request {
            if text.len() > MAX_SEND_TEXT_BYTES {
                return LocalApiResponse::Err(format!(
                    "send-text payload {}B exceeds {}B limit",
                    text.len(),
                    MAX_SEND_TEXT_BYTES
                ));
            }
        }

        let (reply_tx, reply_rx) = oneshot::channel();
        match self.cmd_tx.try_send((envelope.request, reply_tx)) {
            Ok(()) => {}
            Err(async_channel::TrySendError::Full(_)) => {
                return LocalApiResponse::Err("local-api queue full, retry later".into());
            }
            Err(async_channel::TrySendError::Closed(_)) => {
                return LocalApiResponse::Err("local-api dispatch channel closed".into());
            }
        }
        reply_rx
            .await
            .unwrap_or_else(|_| LocalApiResponse::Err("local-api reply channel dropped".into()))
    }
}

/// Length-checked byte-equality on the cookie. Cookie length is fixed across
/// all sessions, so a length mismatch already implies a bogus client; the
/// loop is defensive against future variable-length cookies.
fn cookies_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub struct LocalApiServer {
    _server: Option<ipc::Server>,
}

impl Entity for LocalApiServer {
    type Event = ();
}

impl SingletonEntity for LocalApiServer {}

impl LocalApiServer {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let (cmd_tx, cmd_rx) = async_channel::bounded::<Command>(MAX_PENDING_COMMANDS);
        let cookie = generate_cookie();
        let socket_path = per_user_socket_path();

        // The bind path may collide with a stale socket from a crashed prior
        // instance; remove it so the bind succeeds.
        let _ = fs::remove_file(&socket_path);

        let service_impl = LocalApiServiceImpl {
            cookie: cookie.clone(),
            cmd_tx,
        };

        let builder = ServerBuilder::default()
            .with_service(service_impl)
            .with_fixed_address(socket_path.to_string_lossy().to_string())
            .with_max_request_bytes(MAX_REQUEST_BYTES);

        let server = match builder.build_and_run(ctx.background_executor()) {
            Ok((server, addr)) => {
                if let Err(e) = harden_socket(&addr) {
                    log::warn!("local-api: failed to chmod socket: {e:?}");
                }
                if let Err(e) = publish_address(&addr, &cookie) {
                    log::warn!("local-api: failed to publish socket address: {e:?}");
                }
                log::info!("local-api: listening at {addr}");
                Some(server)
            }
            Err(e) => {
                log::error!("local-api: failed to start server: {e:?}");
                None
            }
        };

        ctx.spawn_stream_local(
            cmd_rx,
            |_me, (req, reply), ctx| {
                let resp = handle(req, ctx);
                let _ = reply.send(resp);
            },
            |_, _| {},
        );

        Self { _server: server }
    }
}

fn generate_cookie() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Per-user, per-instance socket path. On macOS, `$TMPDIR` resolves to a
/// per-user `/var/folders/.../T/` directory with mode 0700, which is the
/// ideal location for a UDS that shouldn't be visible to other accounts.
///
/// The filename embeds:
///   - `ChannelState::data_domain()` so dev / preview / stable / oss
///     installations bind distinct sockets even when launched concurrently;
///   - the current PID so multiple instances of the same channel cohabit
///     without removing each other's bind path on startup.
fn per_user_socket_path() -> PathBuf {
    let dir = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let domain = ChannelState::data_domain();
    let pid = std::process::id();
    dir.join(format!("{domain}-local-api-{pid}.sock"))
}

/// Address-file path for the running app — namespaced by `data_domain` so
/// concurrently running Warp channels don't clobber each other's address
/// file.
fn address_publish_path() -> PathBuf {
    address_publish_path_for(&ChannelState::data_domain())
}

fn harden_socket(addr: &ConnectionAddress) -> std::io::Result<()> {
    fs::set_permissions(addr.to_string(), fs::Permissions::from_mode(0o600))
}

/// Write the address file atomically under 0600 throughout. We never write
/// the cookie into a file whose permissions could be looser than 0600 — if
/// a previous Warp version left the target at 0644, opening it with
/// `OpenOptions::mode(0o600)` would NOT tighten existing perms (mode only
/// applies on creation), and the cookie bytes would be world-readable for
/// the window between `write_all` and `set_permissions`.
///
/// Instead we write to a fresh sibling temp file that the kernel creates
/// via `O_CREAT | O_EXCL` with mode 0600 (`create_new(true) + .mode`),
/// flush, then atomically `rename(2)` it over the destination. The new
/// inode replaces the old one with tight perms from inception.
fn publish_address(addr: &ConnectionAddress, cookie: &str) -> std::io::Result<()> {
    let path = address_publish_path();
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "address path has no parent",
        )
    })?;
    fs::create_dir_all(parent)?;

    let tmp_path = parent.join(format!(".local-api.address.{}.tmp", std::process::id()));
    // Clear any stale temp from a prior crashed run; the open below uses
    // O_EXCL so we must own a fresh inode.
    let _ = fs::remove_file(&tmp_path);

    let body = format_address_file(&addr.to_string(), cookie);
    {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp_path)?;
        f.write_all(body.as_bytes())?;
        f.sync_all()?;
    }

    if let Err(e) = fs::rename(&tmp_path, &path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }
    Ok(())
}

fn handle(req: LocalApiRequest, ctx: &mut ModelContext<LocalApiServer>) -> LocalApiResponse {
    match req {
        LocalApiRequest::Ping => LocalApiResponse::Pong,
        LocalApiRequest::Split { dir } => match split_active_pane(map_dir(dir), ctx) {
            Ok(id) => LocalApiResponse::PaneId(id.to_local_api_string()),
            Err(msg) => LocalApiResponse::Err(msg),
        },
        LocalApiRequest::SendText { pane, text } => match send_text(pane, text, ctx) {
            Ok(()) => LocalApiResponse::Ok,
            Err(msg) => LocalApiResponse::Err(msg),
        },
        LocalApiRequest::ListPanes => match list_panes(ctx) {
            Ok(ids) => LocalApiResponse::Panes(
                ids.into_iter().map(|id| id.to_local_api_string()).collect(),
            ),
            Err(msg) => LocalApiResponse::Err(msg),
        },
        LocalApiRequest::ActivePane => match active_pane(ctx) {
            Ok(Some(id)) => LocalApiResponse::PaneId(id.to_local_api_string()),
            Ok(None) => LocalApiResponse::Err("no active terminal pane".into()),
            Err(msg) => LocalApiResponse::Err(msg),
        },
        LocalApiRequest::ClosePane { pane } => match close_pane(pane, ctx) {
            Ok(()) => LocalApiResponse::Ok,
            Err(msg) => LocalApiResponse::Err(msg),
        },
    }
}

fn map_dir(d: SplitDir) -> Direction {
    match d {
        SplitDir::Left => Direction::Left,
        SplitDir::Right => Direction::Right,
        SplitDir::Up => Direction::Up,
        SplitDir::Down => Direction::Down,
    }
}

fn parse_pane_id(s: &str) -> Result<TerminalPaneId, String> {
    TerminalPaneId::parse_local_api(s).ok_or_else(|| format!("invalid pane id '{s}'"))
}

fn active_workspace(ctx: &mut AppContext) -> Result<ViewHandle<Workspace>, String> {
    let window_manager = WindowManager::as_ref(ctx);
    let window_id = window_manager
        .active_window()
        .or_else(|| window_manager.frontmost_window_id())
        .or_else(|| window_manager.ordered_window_ids().into_iter().next())
        .ok_or_else(|| "no Warp window available".to_owned())?;

    let workspaces = ctx
        .views_of_type::<Workspace>(window_id)
        .ok_or_else(|| "no workspace in active window".to_owned())?;
    workspaces
        .into_iter()
        .next()
        .ok_or_else(|| "no workspace view found".to_owned())
}

fn with_active_pane_group<R>(
    ctx: &mut ModelContext<LocalApiServer>,
    f: impl FnOnce(&mut PaneGroup, &mut warpui::ViewContext<PaneGroup>) -> R,
) -> Result<R, String> {
    let workspace = active_workspace(ctx)?;
    let mut result = None;
    workspace.update(ctx, |workspace, ctx| {
        let pane_group = workspace.active_tab_pane_group().clone();
        result = Some(pane_group.update(ctx, f));
    });
    result.ok_or_else(|| "failed to access pane group".to_owned())
}

fn split_active_pane(
    direction: Direction,
    ctx: &mut ModelContext<LocalApiServer>,
) -> Result<TerminalPaneId, String> {
    with_active_pane_group(ctx, |pg, ctx| {
        pg.local_api_split_active_pane(direction, ctx)
    })
}

fn list_panes(ctx: &mut ModelContext<LocalApiServer>) -> Result<Vec<TerminalPaneId>, String> {
    with_active_pane_group(ctx, |pg, _ctx| pg.local_api_terminal_pane_ids())
}

fn active_pane(ctx: &mut ModelContext<LocalApiServer>) -> Result<Option<TerminalPaneId>, String> {
    with_active_pane_group(ctx, |pg, ctx| pg.active_session_id(ctx))
}

fn close_pane(pane: String, ctx: &mut ModelContext<LocalApiServer>) -> Result<(), String> {
    let id = parse_pane_id(&pane)?;
    with_active_pane_group(ctx, |pg, ctx| {
        pg.local_api_close_pane(id, ctx)
            .map_err(|msg| format!("{msg}: '{}'", id.to_local_api_string()))
    })?
}

fn send_text(
    pane: Option<String>,
    text: String,
    ctx: &mut ModelContext<LocalApiServer>,
) -> Result<(), String> {
    let target_id = match pane {
        Some(s) => parse_pane_id(&s)?,
        None => active_pane(ctx)?.ok_or_else(|| "no active terminal pane".to_owned())?,
    };

    with_active_pane_group(ctx, |pg, ctx| {
        let Some(view) = pg.terminal_view_from_pane_id(target_id, ctx) else {
            return Err(format!(
                "pane '{}' not found",
                target_id.to_local_api_string()
            ));
        };
        view.update(ctx, |term, ctx| {
            term.write_to_pty(text.into_bytes(), ctx);
        });
        Ok(())
    })?
}
