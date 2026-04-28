/// Stages during the course of bootstrapping the shell.  
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BootstrapStage {
    /// Warp is re-parsing historical blocks for this session. We haven't yet started
    /// bootstrapping.
    RestoreBlocks,
    /// Warp is writing the bootstrap script into the running shell.
    WarpInput,
    /// Execution of any shell startup scripts such as .rc or .profile files.
    ScriptExecution,
    /// Model is fully bootstrapped (i.e the `Bootstrap` message was successfully received by Warp).   
    Bootstrapped,
    /// Model is fully bootstrapped and we've received the precmd that results from bootstrapping itself
    PostBootstrapPrecmd,
}

impl BootstrapStage {
    pub fn next_stage(&self) -> Self {
        match self {
            Self::RestoreBlocks => Self::WarpInput,
            Self::WarpInput => Self::ScriptExecution,
            Self::ScriptExecution => {
                log::error!("calling next_stage on a block that should be bootstrapped");
                Self::ScriptExecution
            }
            Self::Bootstrapped => Self::PostBootstrapPrecmd,
            Self::PostBootstrapPrecmd => {
                log::error!(
                    "calling next_stage on an already bootstrapped block that has seen precmd"
                );
                Self::PostBootstrapPrecmd
            }
        }
    }

    pub fn is_bootstrapped(&self) -> bool {
        matches!(self, Self::Bootstrapped | Self::PostBootstrapPrecmd)
    }

    pub fn is_done(&self) -> bool {
        matches!(self, Self::PostBootstrapPrecmd)
    }

    /// WarpInput is the one block that is hidden by default (unless debug mode is on).
    pub fn is_hidden(&self) -> bool {
        matches!(self, Self::WarpInput)
    }

    /// We only can have an empty block that's shown if it's a block a user created by hitting enter, or if it's
    /// a restored block that was created by the user hitting enter.
    pub fn is_empty_block_allowed(&self) -> bool {
        matches!(self, Self::RestoreBlocks | Self::PostBootstrapPrecmd)
    }
}
