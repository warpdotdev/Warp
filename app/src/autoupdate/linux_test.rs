use super::*;

#[test]
fn test_repo_name() {
    assert_eq!(repo_name(Channel::Dev), "warpdotdev-dev");
    assert_eq!(repo_name(Channel::Stable), "warpdotdev");
}

#[test]
fn test_nu_update_command_gates_finish_update_on_success() {
    let command = PackageManager::Apt {
        distribution_update_disabled_repository: false,
    }
    .update_command(ShellType::Nu, "update-123");

    assert!(command.starts_with("try { "));
    assert!(command.contains("sudo apt update; sudo apt install "));
    assert!(command.contains("; warp_finish_update update-123 }"));
    assert!(!command.contains(" && "));
}

#[test]
fn test_nu_update_command_uses_nu_dist_upgrade_handler() {
    let command = PackageManager::Apt {
        distribution_update_disabled_repository: true,
    }
    .update_command(ShellType::Nu, "update-123");

    assert!(command.contains("try { warp_handle_dist_upgrade "));
    assert!(command.contains(" } catch { null }; sudo apt update"));
    assert!(command.contains("; warp_finish_update update-123 }"));
}
