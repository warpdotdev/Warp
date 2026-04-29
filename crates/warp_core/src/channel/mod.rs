mod config;
mod state;

use std::fmt;

pub use config::*;
pub use state::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// The official/first-party stable release.
    Stable,
    /// The official/first-party feature preview release.
    Preview,

    /// The internal-only nightly build.
    Dev,
    /// The internal-only HEAD build.
    Local,

    /// The open-source build of Warp.
    Oss,

    /// The integration test build.
    Integration,
}

impl Channel {
    /// Whether or not this channel is for internal use only
    pub fn is_dogfood(&self) -> bool {
        match self {
            Channel::Dev | Channel::Local => true,
            Channel::Stable | Channel::Preview | Channel::Integration | Channel::Oss => false,
        }
    }

    /// Whether this channel honors the `--server-root-url` / `--ws-server-url` /
    /// `--session-sharing-server-url` flags (and their `WARP_*` env-var equivalents).
    ///
    /// First-party release channels (`Stable`, `Preview`) ignore these overrides so shipped
    /// builds can't be redirected away from their baked-in server URLs. OSS and internal-only
    /// channels (`Oss`, `Dev`, `Local`, `Integration`) honor them for local routing and testing.
    pub fn allows_server_url_overrides(&self) -> bool {
        match self {
            Channel::Dev | Channel::Local | Channel::Oss | Channel::Integration => true,
            Channel::Stable | Channel::Preview => false,
        }
    }

    /// Returns the CLI command name corresponding to this channel.
    pub fn cli_command_name(&self) -> &'static str {
        match self {
            Channel::Stable => "oz",
            Channel::Dev => "oz-dev",
            Channel::Preview => "oz-preview",
            Channel::Local => "oz-local",
            Channel::Integration => "oz-integration",
            Channel::Oss => "warp-oss",
        }
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Channel::Stable => "stable",
            Channel::Preview => "preview",
            Channel::Dev => "dev",
            Channel::Integration => "integration",
            Channel::Local => "local",
            Channel::Oss => "warp-oss",
        })
    }
}
