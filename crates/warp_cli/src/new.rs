use std::path::PathBuf;

use clap::{Args, Subcommand};

/// `warp new` has an unusual shape because `warp new <template>` acts as a
/// shorthand for `warp new scaffold <template>`.
#[derive(Debug, Clone, Args)]
#[clap(args_conflicts_with_subcommands = true)]
pub struct NewCommand {
    #[clap(subcommand)]
    pub subcommand: Option<NewSubcommand>,

    #[clap(flatten)]
    pub scaffold: Option<ScaffoldArgs>,
}

impl NewCommand {
    /// Resolve the effective subcommand.
    ///
    /// Returns `None` when the user typed `warp new` with no arguments; the
    /// caller should display help in that case.
    pub fn into_subcommand(self) -> Option<NewSubcommand> {
        if let Some(args) = self.scaffold {
            Some(NewSubcommand::Scaffold(args))
        } else {
            self.subcommand
        }
    }

    /// Borrow the effective subcommand without consuming `self`.
    pub fn subcommand(&self) -> Option<&NewSubcommand> {
        self.subcommand.as_ref()
    }
}

/// Subcommands exposed under `warp new`.
#[derive(Debug, Clone, Subcommand)]
pub enum NewSubcommand {
    /// Scaffold a new project from a registered template.
    ///
    /// Copies template files into a new directory. Built-in templates are
    /// bundled with the binary; user-defined templates live in
    /// `~/.warp/templates/<name>/`.
    ///
    /// Example:
    ///
    ///   warp new scaffold helm-mcp-project
    ///   warp new helm-mcp-project          # shorthand
    Scaffold(ScaffoldArgs),

    /// List all available templates and their descriptions.
    List,
}

/// Arguments for `warp new <template>` (or `warp new scaffold <template>`).
#[derive(Debug, Clone, Args)]
pub struct ScaffoldArgs {
    /// Name of the template to scaffold from.
    ///
    /// Run `warp new list` to see available templates.
    pub template: String,

    /// Project directory name (defaults to the template name).
    ///
    /// The directory is created inside `--dir` (or the current directory).
    #[arg(long = "name", short = 'n', value_name = "NAME")]
    pub name: Option<String>,

    /// Parent directory in which to create the project (defaults to `.`).
    #[arg(long = "dir", short = 'd', value_name = "PATH")]
    pub dir: Option<PathBuf>,

    /// Overwrite existing files without prompting.
    #[arg(long = "force", short = 'f')]
    pub force: bool,
}
