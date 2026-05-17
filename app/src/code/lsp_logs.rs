use std::path::{Path, PathBuf};

use lsp::supported_servers::LSPServerType;
use simple_logger::manager::resolve_log_path;
use warp_util::path::workspace_hash;

/// Returns the relative log path (within the LSP log directory) for an LSP server.
/// For example, `rust-analyzer/12345678.log`.
pub fn relative_log_path(server_type: LSPServerType, workspace_path: &Path) -> PathBuf {
    let server_type_name = server_type.binary_name();
    let hash = workspace_hash(workspace_path);

    PathBuf::from(server_type_name).join(format!("{hash}.log"))
}

/// Returns the path to the log file for an LSP server.
///
/// Format: `{secure_state_dir}/lsp/{server_type}/{workspace_hash}.log`
///
/// The workspace path is hashed to avoid filesystem issues with long or special character paths.
pub fn log_file_path(server_type: LSPServerType, workspace_path: &Path) -> PathBuf {
    resolve_log_path("lsp", relative_log_path(server_type, workspace_path))
}
