//! Symphony daemon entry point.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Output;
use std::sync::Arc;

use agents::{ClaudeCodeAgent, ClaudeModel};
use clap::Parser;
use doppler::{CommandRunner, DopplerClient, DopplerError, SecretValue, DEFAULT_TTL};
use orchestrator::{AgentRegistration, Budget, Cap, Provider, Router};
use symphony::audit::AuditLog;
use symphony::orchestrator::{IssueSource, Orchestrator};
use symphony::tracker::LinearClient;
use symphony::workflow::WorkflowDefinition;
use symphony::workspace::WorkspaceManager;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "symphony", about = "Linear-driven coding-agent orchestrator")]
struct Cli {
    /// Path to the WORKFLOW.md file.
    #[arg(long, default_value = "./WORKFLOW.md")]
    workflow: PathBuf,
    /// Run a single tick and exit (smoke testing).
    #[arg(long, default_value_t = false)]
    once: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // rustls 0.23 requires an explicit crypto provider before any TLS client
    // is constructed (reqwest's rustls-tls in this workspace doesn't install
    // one automatically). Idempotent: ignore the error if a provider is
    // already installed (e.g. by another crate in the same process).
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let workflow = WorkflowDefinition::load(&cli.workflow)?;

    let api_key = resolve_api_key(&workflow.config.tracker.api_key).await?;

    let tracker = match &workflow.config.tracker.team_key {
        Some(team_key) => LinearClient::new_with_team(
            workflow.config.tracker.endpoint.clone(),
            api_key,
            workflow.config.tracker.project_slug.clone(),
            team_key.clone(),
        )?,
        None => LinearClient::new(
            workflow.config.tracker.endpoint.clone(),
            api_key,
            workflow.config.tracker.project_slug.clone(),
        )?,
    };

    let workspaces = Arc::new(WorkspaceManager::new(
        workflow.config.workspace.root.clone(),
        workflow.config.hooks.clone(),
    ));

    let mut caps: HashMap<Provider, Cap> = HashMap::new();
    // Generous defaults for the MVP: $100/mo, $20/session per provider.
    let cap = Cap {
        monthly_micro_dollars: 100_000_000,
        session_micro_dollars: 20_000_000,
    };
    caps.insert(Provider::ClaudeCode, cap);
    let budget = Arc::new(Budget::new(caps));
    let mut router = Router::new(Arc::clone(&budget));

    let claude = ClaudeCodeAgent::new(
        orchestrator::AgentId("claude-sonnet-46".to_string()),
        ClaudeModel::Sonnet46,
    )?;
    router.register(AgentRegistration {
        agent: Arc::new(claude),
        provider: Provider::ClaudeCode,
        estimated_micros_per_task: 50_000,
    });
    let router = Arc::new(router);

    let audit_path = home_dir().join(".warp/symphony/audit.log");
    let audit = Arc::new(AuditLog::open(audit_path));

    let orch = Arc::new(Orchestrator::new(
        workflow,
        Arc::new(tracker) as Arc<dyn IssueSource>,
        workspaces,
        router,
        audit,
    ));

    if cli.once {
        orch.tick().await?;
        // Wait indefinitely for spawned agent tasks to drain. Real coding
        // tasks against the warp source can take 10-30 minutes; capping the
        // drain causes Symphony to exit before agents finish, which means
        // diff-stat / comment / state-transition never run. Use Ctrl-C to
        // abort if needed.
        loop {
            let (running, _completed) = orch.state_snapshot().await;
            if running.is_empty() {
                tracing::info!("once: all dispatched agents finished");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        return Ok(());
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            tracing::info!("ctrl-c received");
            let _ = shutdown_tx.send(());
        }
    });

    Arc::clone(&orch).run(shutdown_rx).await;
    Ok(())
}

/// Resolve the Linear API key into a [`SecretValue`].
///
/// The workflow loader has already substituted `$VAR` indirection. If the
/// resulting string is non-empty, we wrap it through a one-shot stub
/// [`CommandRunner`] (the only way to construct a `SecretValue` outside the
/// `doppler` crate without modifying its public surface). If the string is
/// empty, we fall back to the real Doppler CLI.
async fn resolve_api_key(spec: &str) -> Result<SecretValue, Box<dyn std::error::Error>> {
    if !spec.is_empty() {
        let runner = Arc::new(LiteralRunner {
            value: spec.to_string(),
        });
        let client = DopplerClient::with_runner(DEFAULT_TTL, runner);
        return Ok(client.get("LINEAR_API_KEY").await?);
    }

    match doppler::detect() {
        Ok(_) => {
            let client = DopplerClient::new(DEFAULT_TTL);
            let v = client.get("LINEAR_API_KEY").await?;
            Ok(v)
        }
        Err(DopplerError::NotInstalled { install_hint }) => Err(format!(
            "LINEAR_API_KEY is not set in env and Doppler is not installed ({install_hint})"
        )
        .into()),
        Err(e) => Err(Box::new(e)),
    }
}

/// One-shot [`CommandRunner`] that returns a fixed string as the secret.
struct LiteralRunner {
    value: String,
}

#[async_trait::async_trait]
impl CommandRunner for LiteralRunner {
    async fn run(&self, _args: &[&str]) -> std::io::Result<Output> {
        use std::os::unix::process::ExitStatusExt;
        Ok(Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: self.value.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}
