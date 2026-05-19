use std::{
    fs,
    io::Read,
    os::unix::{fs::DirBuilderExt, net::UnixListener},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use async_channel::Sender;

use crate::terminal::{cli_agent_sessions::event::CLI_AGENT_NOTIFICATION_SENTINEL, event::Event};

static NEXT_SOCKET_ID: AtomicU64 = AtomicU64::new(1);
const MAX_IPC_MESSAGE_BYTES: u64 = 1024 * 1024;

pub struct CLIAgentIpcListener {
    socket_path: PathBuf,
    shutdown: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

impl CLIAgentIpcListener {
    pub fn start(event_sender: Sender<Event>) -> std::io::Result<Self> {
        let socket_path = socket_path();
        if let Some(parent) = socket_path.parent() {
            fs::DirBuilder::new()
                .mode(0o700)
                .recursive(true)
                .create(parent)?;
        }
        let _ = fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path)?;
        listener.set_nonblocking(true)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = shutdown.clone();
        let thread_socket_path = socket_path.clone();
        let join_handle = thread::Builder::new()
            .name("cli-agent-ipc-listener".to_owned())
            .spawn(move || {
                run_listener(listener, event_sender, thread_shutdown);
                let _ = fs::remove_file(thread_socket_path);
            })?;

        Ok(Self {
            socket_path,
            shutdown,
            join_handle: Some(join_handle),
        })
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }
}

impl Drop for CLIAgentIpcListener {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = fs::remove_file(&self.socket_path);
        if let Some(join_handle) = self.join_handle.take() {
            if let Err(err) = join_handle.join() {
                log::warn!("Failed to join CLI agent IPC listener thread: {err:?}");
            }
        }
    }
}

fn run_listener(listener: UnixListener, event_sender: Sender<Event>, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut bytes = Vec::new();
                match stream
                    .by_ref()
                    .take(MAX_IPC_MESSAGE_BYTES)
                    .read_to_end(&mut bytes)
                {
                    Ok(_) => {
                        let message = String::from_utf8_lossy(&bytes);
                        if let Some((title, body)) = parse_ipc_message(&message) {
                            if let Err(err) =
                                event_sender.try_send(Event::PluggableNotification { title, body })
                            {
                                log::warn!("Failed to forward CLI agent IPC event: {err}");
                            }
                        }
                    }
                    Err(err) => {
                        log::warn!("Failed to read CLI agent IPC event: {err}");
                    }
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(err) => {
                log::warn!("CLI agent IPC listener failed to accept connection: {err}");
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn parse_ipc_message(message: &str) -> Option<(Option<String>, String)> {
    if let Some(rest) = message.strip_prefix("\x1b]777;notify;") {
        let rest = trim_osc_terminator(rest);
        let (title, body) = rest.split_once(';')?;
        let body = body.trim().to_owned();
        if body.is_empty() {
            return None;
        }
        return Some((non_empty_title(title.trim()).map(|title| title.to_owned()), body));
    }

    let body = message.trim().to_owned();
    if body.is_empty() {
        return None;
    }
    Some((Some(CLI_AGENT_NOTIFICATION_SENTINEL.to_owned()), body))
}

fn trim_osc_terminator(value: &str) -> &str {
    value
        .strip_suffix('\x07')
        .or_else(|| value.strip_suffix("\x1b\\"))
        .unwrap_or(value)
}

fn non_empty_title(title: &str) -> Option<&str> {
    (!title.is_empty()).then_some(title)
}

fn socket_path() -> PathBuf {
    let socket_id = NEXT_SOCKET_ID.fetch_add(1, Ordering::Relaxed);
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!(
        "/tmp/warp-cli-agent-{uid}/terminal-{}-{socket_id}.sock",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_osc_777_notification() {
        let (title, body) = parse_ipc_message(
            "\x1b]777;notify;warp://cli-agent;{\"v\":1,\"agent\":\"claude\"}\x07",
        )
        .unwrap();

        assert_eq!(title.as_deref(), Some(CLI_AGENT_NOTIFICATION_SENTINEL));
        assert_eq!(body, "{\"v\":1,\"agent\":\"claude\"}");
    }

    #[test]
    fn treats_plain_json_as_cli_agent_event_body() {
        let (title, body) =
            parse_ipc_message("{\"v\":1,\"agent\":\"claude\",\"event\":\"session_start\"}")
                .unwrap();

        assert_eq!(title.as_deref(), Some(CLI_AGENT_NOTIFICATION_SENTINEL));
        assert_eq!(
            body,
            "{\"v\":1,\"agent\":\"claude\",\"event\":\"session_start\"}"
        );
    }
}
