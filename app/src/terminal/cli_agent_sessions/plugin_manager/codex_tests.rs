use super::CodexPluginManager;
use crate::terminal::cli_agent_sessions::plugin_manager::CliAgentPluginManager;

#[test]
fn can_auto_install_is_false() {
    assert!(!CodexPluginManager.can_auto_install());
}

#[test]
fn does_not_support_update() {
    assert!(!CodexPluginManager.supports_update());
}

#[test]
fn install_instructions_has_steps() {
    let instructions = CodexPluginManager.install_instructions();
    assert!(!instructions.steps.is_empty());
    assert!(!instructions.title.is_empty());
}
