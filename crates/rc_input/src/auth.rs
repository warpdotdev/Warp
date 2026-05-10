//! Filesystem-permission auth for the local socket.
//!
//! On Unix we chmod the bound socket to `0700` so only the owning user can
//! connect. On Windows, named pipes inherit a default DACL that already
//! restricts access to the creating user + SYSTEM, so v1 takes no extra
//! action. Tightening the Windows DACL (e.g. removing SYSTEM) is tracked as a
//! follow-up in the design doc §6.

#[cfg(unix)]
pub fn restrict_socket_perms(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
pub fn restrict_socket_perms(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}
