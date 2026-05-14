//! 把 `SshServerInfo` 拼成 `ssh ...` 命令。纯函数,易测。
//!
//! 写入 PTY 时调 `build_ssh_command_line`,会用 shell-escape 引用每个 arg,
//! 防止用户名 / host / key_path 里的空格或单引号破坏命令行。

use crate::types::{AuthType, SshServerInfo};
use std::borrow::Cow;

pub fn build_ssh_args(server: &SshServerInfo) -> Vec<String> {
    let mut args: Vec<String> = vec!["ssh".into()];
    if server.port != 22 {
        args.push("-p".into());
        args.push(server.port.to_string());
    }
    if server.auth_type == AuthType::Key
        && let Some(path) = server.key_path.as_deref()
        && !path.is_empty()
    {
        args.push("-i".into());
        args.push(path.to_string());
    }
    let target = if server.username.is_empty() {
        server.host.clone()
    } else {
        format!("{}@{}", server.username, server.host)
    };
    args.push(target);
    args
}

pub fn build_ssh_command_line(server: &SshServerInfo) -> String {
    let args = build_ssh_args(server);
    args.iter()
        .map(|a| shell_escape::unix::escape(Cow::Borrowed(a.as_str())).to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> SshServerInfo {
        SshServerInfo {
            node_id: "n".into(),
            host: "1.2.3.4".into(),
            port: 22,
            username: "alice".into(),
            auth_type: AuthType::Password,
            key_path: None,
            last_connected_at: None,
        }
    }

    #[test]
    fn default_port_omitted() {
        let s = server();
        assert_eq!(build_ssh_args(&s), vec!["ssh", "alice@1.2.3.4"]);
        // shell-escape 出于保守会把 user@host 用单引号引起来,这是合法且
        // shell-equivalent 的形式 — 不强求未引用版本。
        let line = build_ssh_command_line(&s);
        assert!(
            line == "ssh alice@1.2.3.4" || line == "ssh 'alice@1.2.3.4'",
            "unexpected: {line}"
        );
    }

    #[test]
    fn custom_port_uses_dash_p() {
        let mut s = server();
        s.port = 2222;
        assert_eq!(
            build_ssh_args(&s),
            vec!["ssh", "-p", "2222", "alice@1.2.3.4"]
        );
    }

    #[test]
    fn key_auth_emits_dash_i() {
        let mut s = server();
        s.auth_type = AuthType::Key;
        s.key_path = Some("/home/u/.ssh/id_ed25519".into());
        assert_eq!(
            build_ssh_args(&s),
            vec!["ssh", "-i", "/home/u/.ssh/id_ed25519", "alice@1.2.3.4"]
        );
    }

    #[test]
    fn key_auth_without_path_is_skipped() {
        let mut s = server();
        s.auth_type = AuthType::Key;
        s.key_path = None;
        assert_eq!(build_ssh_args(&s), vec!["ssh", "alice@1.2.3.4"]);
    }

    #[test]
    fn empty_username_yields_host_only() {
        let mut s = server();
        s.username = String::new();
        assert_eq!(build_ssh_args(&s), vec!["ssh", "1.2.3.4"]);
    }

    #[test]
    fn shell_escapes_spaces_in_path() {
        let mut s = server();
        s.auth_type = AuthType::Key;
        s.key_path = Some("/path with spaces/id_rsa".into());
        let line = build_ssh_command_line(&s);
        assert!(
            line.contains("'/path with spaces/id_rsa'"),
            "actual: {line}"
        );
    }
}
