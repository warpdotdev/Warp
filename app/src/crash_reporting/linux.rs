use crate::crash_reporting::VirtualEnvironment;

/// Returns what virtualized environment Warp is running in, if any.
pub fn get_virtualized_environment() -> Option<VirtualEnvironment> {
    if let Ok(output) = command::blocking::Command::new("systemd-detect-virt").output() {
        if !output.status.success() {
            return None;
        } else {
            let value = std::str::from_utf8(&output.stdout).ok()?;
            if value == "none" {
                return None;
            }
            return Some(VirtualEnvironment {
                name: value.to_owned(),
            });
        }
    };

    // Test specifically for WSL based on existence of a particular file under
    // /proc.
    //
    // See: https://superuser.com/questions/1749781/how-can-i-check-if-the-environment-is-wsl-from-a-shell-script
    if std::path::Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists() {
        return Some(VirtualEnvironment {
            name: "wsl".to_owned(),
        });
    }

    None
}
