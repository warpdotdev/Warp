use super::*;

#[test]
fn posix_agent_command_environment_disables_pagers_for_whole_command() {
    let command = ShellCommandExecutor::command_with_agent_environment_for_shell(
        "cd /workspace/warper && git diff --stat",
        Some(ShellType::Zsh),
    );

    assert!(command.starts_with("(export "));
    assert!(command.contains("PAGER=cat"));
    assert!(command.contains("GIT_PAGER=cat"));
    assert!(command.contains("GH_PAGER=cat"));
    assert!(command.contains("TERM=dumb"));
    assert!(command.ends_with("; cd /workspace/warper && git diff --stat)"));
}

#[test]
fn fish_agent_command_environment_uses_local_exported_scope() {
    let command = ShellCommandExecutor::command_with_agent_environment_for_shell(
        "git diff --stat",
        Some(ShellType::Fish),
    );

    assert!(command.starts_with("begin; "));
    assert!(command.contains("set -lx PAGER cat"));
    assert!(command.contains("set -lx GIT_PAGER cat"));
    assert!(command.contains("set -lx TERM dumb"));
    assert!(command.ends_with("; git diff --stat; end"));
}

#[test]
fn powershell_agent_command_environment_restores_previous_values() {
    let command = ShellCommandExecutor::command_with_agent_environment_for_shell(
        "git diff --stat",
        Some(ShellType::PowerShell),
    );

    assert!(command.starts_with("& { "));
    assert!(command.contains("$__warper_had_PAGER = Test-Path Env:PAGER"));
    assert!(command.contains("$env:PAGER = 'cat'"));
    assert!(command.contains("git diff --stat"));
    assert!(command.contains("finally"));
    assert!(command.contains("Remove-Item Env:PAGER -ErrorAction SilentlyContinue"));
}
