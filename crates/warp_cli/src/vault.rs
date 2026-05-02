use clap::{Args, Subcommand};

#[derive(Debug, Clone, Subcommand)]
pub enum VaultCommand {
    Inject(InjectArgs),
}

#[derive(Debug, Clone, Args)]
pub struct InjectArgs {
    pub path: Option<String>,

    #[arg(long = "as")]
    pub env_var: Option<String>,
}
