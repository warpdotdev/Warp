//! `wp` — Warp local control CLI.
//!
//! Reads the published address file (socket path + per-session cookie),
//! connects via `ipc::Client`, and dispatches subcommands like
//! `split`/`send-text`/`list-panes`/`close-pane` against the running Warp.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::sync::Arc;

use ipc::{service_caller, ConnectionAddress};
use warp_local_api::{
    parse_address_file, resolve_address_path, AddressResolution, LocalApiEnvelope, LocalApiRequest,
    LocalApiResponse, LocalApiService, SplitDir,
};

#[derive(Debug, Parser)]
#[command(name = "wp", about = "Warp local control CLI")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Round-trip health check against the running Warp.
    Ping,
    /// Split the active pane in the given direction. Prints the new pane id.
    Split { direction: Direction },
    /// Send text to a terminal pane (or the active pane if --pane is omitted).
    SendText {
        #[arg(long)]
        pane: Option<String>,
        text: String,
    },
    /// List all terminal pane ids in the active workspace, one per line.
    ListPanes,
    /// Print the id of the currently focused terminal pane.
    ActivePane,
    /// Close the given terminal pane.
    ClosePane { pane: String },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl From<Direction> for SplitDir {
    fn from(d: Direction) -> Self {
        match d {
            Direction::Left => SplitDir::Left,
            Direction::Right => SplitDir::Right,
            Direction::Up => SplitDir::Up,
            Direction::Down => SplitDir::Down,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let executor = Arc::new(warpui::r#async::executor::Background::new(2, |i| {
        format!("wp-cli-{i}")
    }));
    let executor_for_block = executor.clone();

    let result: Result<LocalApiResponse> = warpui::r#async::block_on(async move {
        let addr_path = match resolve_address_path() {
            AddressResolution::Single(p) => p,
            AddressResolution::Ambiguous(paths) => {
                let listing = paths
                    .iter()
                    .map(|p| format!("  {}", p.display()))
                    .collect::<Vec<_>>()
                    .join("\n");
                return Err(anyhow!(
                    "multiple Warp instances are publishing local-api address files:\n{listing}\n\
                     set WARP_LOCAL_API_DOMAIN=<channel> or WARP_LOCAL_API_ADDRESS=<path> to disambiguate"
                ));
            }
        };
        let body = std::fs::read_to_string(&addr_path).with_context(|| {
            format!(
                "could not read socket address from {} — is Warp running with WARP_ENABLE_LOCAL_API=1?",
                addr_path.display()
            )
        })?;
        let (socket, cookie) = parse_address_file(&body).ok_or_else(|| {
            anyhow!(
                "address file at {} is malformed (expected two lines: socket\\ncookie\\n)",
                addr_path.display()
            )
        })?;
        let address = ConnectionAddress::from(socket);

        let client = ipc::Client::connect(address, executor_for_block)
            .await
            .map_err(|e| anyhow!("failed to connect to Warp: {e:?}"))?;
        let caller = service_caller::<LocalApiService>(Arc::new(client));

        let request = match cli.cmd {
            Cmd::Ping => LocalApiRequest::Ping,
            Cmd::Split { direction } => LocalApiRequest::Split {
                dir: direction.into(),
            },
            Cmd::SendText { pane, text } => LocalApiRequest::SendText { pane, text },
            Cmd::ListPanes => LocalApiRequest::ListPanes,
            Cmd::ActivePane => LocalApiRequest::ActivePane,
            Cmd::ClosePane { pane } => LocalApiRequest::ClosePane { pane },
        };

        caller
            .call(LocalApiEnvelope { cookie, request })
            .await
            .map_err(|e| anyhow!("call failed: {e:?}"))
    });

    drop(executor);

    match result? {
        LocalApiResponse::Pong => println!("pong"),
        LocalApiResponse::Ok => {}
        LocalApiResponse::PaneId(id) => println!("{id}"),
        LocalApiResponse::Panes(ids) => {
            for id in ids {
                println!("{id}");
            }
        }
        LocalApiResponse::Err(msg) => {
            eprintln!("error: {msg}");
            std::process::exit(1);
        }
    }
    Ok(())
}
