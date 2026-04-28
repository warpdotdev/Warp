/// The command used to proxy ssh requests through GCP's Identity-Aware Proxy.
const PROXY_COMMAND: &str = "gcloud compute start-iap-tunnel ubuntu-14-04 25784 --listen-on-stdin --project=warp-ssh-integration-testing --zone=us-east4-a";

/// Produces a user/host pair for testing a given remote shell.
pub fn user_host(shell: &str) -> String {
    format!("{shell}@ubuntu-14-04")
}

/// Produces the full ssh command to run to ssh into a given remote shell.
pub fn ssh_command(shell: &str, should_use_ssh_wrapper: bool) -> String {
    [
        if should_use_ssh_wrapper {
            "ssh"
        } else {
            "command ssh"
        },
        &user_host(shell),
        "-p 25784",
        &format!("-o ProxyCommand=\"{PROXY_COMMAND}\""),
        "-o StrictHostKeyChecking=no",
        "-o UserKnownHostsFile=/dev/null",
    ]
    .join(" ")
}
