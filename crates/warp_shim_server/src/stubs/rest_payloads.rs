use serde_json::{Value, json};
use uuid::Uuid;

pub(crate) fn firebase_token() -> Value {
    json!({
        "idToken": "local-shim-id-token",
        "refreshToken": "local-shim-refresh-token",
        "expiresIn": "3600",
    })
}

pub(crate) fn unsupported_endpoint() -> Value {
    error("warp-shim: unsupported endpoint")
}

pub(crate) fn error(message: &'static str) -> Value {
    json!({ "error": message })
}

pub(crate) fn list_runs() -> Value {
    json!({ "runs": [] })
}

pub(crate) fn list_agent_events() -> Value {
    json!([])
}

pub(crate) fn report_agent_event() -> Value {
    json!({ "sequence": 0 })
}

pub(crate) fn external_conversation() -> Value {
    json!({ "conversation_id": Uuid::new_v4().to_string() })
}

pub(crate) fn upload_target(public_base_url: &str) -> Value {
    let upload_id = Uuid::new_v4();
    let public_base_url = public_base_url.trim_end_matches('/');
    json!({
        "url": format!("{public_base_url}/api/v1/harness-support/upload/{upload_id}"),
        "method": "PUT",
        "headers": {},
    })
}

pub(crate) fn snapshot_uploads(public_base_url: &str, count: usize) -> Value {
    let uploads = (0..count)
        .map(|_| upload_target(public_base_url))
        .collect::<Vec<_>>();
    json!({ "uploads": uploads })
}

pub(crate) fn resolved_prompt() -> Value {
    json!({
        "prompt": "Local shim mode: no remote task prompt is available.",
        "system_prompt": null,
        "resumption_prompt": null,
    })
}

pub(crate) fn report_artifact() -> Value {
    json!({ "artifact_uid": Uuid::new_v4().to_string() })
}

pub(crate) fn local_shim_auth_page() -> String {
    r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Warp Shim Local Mode</title>
  <style>
    body { font-family: system-ui, sans-serif; max-width: 720px; margin: 4rem auto; line-height: 1.5; }
    code { background: #f4f4f4; padding: 0.15rem 0.3rem; border-radius: 4px; }
  </style>
</head>
<body>
  <h1>Warp Shim Local Mode</h1>
  <p>This browser auth page is a local shim stub. The shim accepts local Warp traffic and does not contact Warp cloud authentication.</p>
  <p>Launch OSS Warp with <code>WARP_API_KEY=local-shim</code> while using this server.</p>
</body>
</html>"#
        .to_string()
}
