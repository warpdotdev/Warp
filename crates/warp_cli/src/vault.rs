use clap::{Args, Subcommand};

#[derive(Debug, Clone, Subcommand)]
pub enum VaultCommand {
    Inject(InjectArgs),
}

#[derive(Debug, Clone, Args)]
pub struct InjectArgs {
    #[arg(requires = "env_var")]
    pub path: Option<String>,

    #[arg(long = "as", requires = "path")]
    pub env_var: Option<String>,
}
