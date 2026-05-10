//! Accept-loop for the local socket / named pipe.
//!
//! Each accepted connection is read newline-by-newline; every line is parsed
//! as an [`InputMsg`] and forwarded to the consumer-supplied
//! [`tokio::sync::mpsc::Sender`]. Malformed lines are logged and skipped — a
//! single bad line does not close the connection.

use crate::error::{Error, Result};
use crate::protocol::InputMsg;

use async_compat::{Compat, CompatExt};
use interprocess::local_socket::tokio::{LocalSocketListener, LocalSocketStream};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

pub struct Server {
    listener: LocalSocketListener,
    tx: mpsc::Sender<InputMsg>,
}

impl Server {
    /// Bind a listener at `addr`. On Unix this is a filesystem path to a
    /// socket; on Windows it is a named-pipe address (e.g.
    /// `\\.\pipe\WarpStable_RC_INPUT`).
    ///
    /// `bind` is synchronous in `interprocess` 1.2.1 — only the per-connection
    /// I/O is async.
    pub fn bind(addr: &str, tx: mpsc::Sender<InputMsg>) -> Result<Self> {
        let listener = LocalSocketListener::bind(addr)?;
        Ok(Self { listener, tx })
    }

    /// Drive the accept loop. Runs until the consumer drops `tx` or the
    /// listener errors fatally. Per-connection handlers are spawned on the
    /// caller's tokio runtime.
    pub async fn run(self) -> Result<()> {
        loop {
            match self.listener.accept().compat().await {
                Ok(stream) => {
                    let tx = self.tx.clone();
                    tokio::spawn(handle_connection(stream, tx));
                }
                Err(e) => {
                    log::warn!("rc_input: accept failed: {e}");
                    return Err(Error::Io(e));
                }
            }
        }
    }
}

async fn handle_connection(stream: LocalSocketStream, tx: mpsc::Sender<InputMsg>) {
    let (read, _write) = stream.into_split();
    let reader = BufReader::new(Compat::new(read));
    let mut lines = reader.lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match InputMsg::from_line(trimmed) {
                    Ok(msg) => {
                        log::debug!(
                            "rc_input: dispatching {:?} client_id={} bytes={}",
                            msg.kind,
                            msg.client_id,
                            msg.value.len()
                        );
                        if tx.send(msg).await.is_err() {
                            log::debug!("rc_input: consumer dropped, closing connection");
                            return;
                        }
                    }
                    Err(e) => {
                        log::warn!("rc_input: invalid json (skipping line): {e}");
                    }
                }
            }
            Ok(None) => return,
            Err(e) => {
                log::warn!("rc_input: read error: {e}");
                return;
            }
        }
    }
}
