use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, rename_all = "snake_case")]
pub enum RootAccess {
    IsRoot,
    CanRunSudo,
    #[default]
    NoRootAccess,
}

impl FromStr for RootAccess {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "is_root" => Ok(RootAccess::IsRoot),
            "can_run_sudo" => Ok(RootAccess::CanRunSudo),
            "no_root_access" => Ok(RootAccess::NoRootAccess),
            _ => Err(anyhow::anyhow!("Invalid RootAccess")),
        }
    }
}
