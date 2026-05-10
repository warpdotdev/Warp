//! Wire format for `rc_input` messages.
//!
//! Mirrors §4.1 of the warp_RC.md design doc. One JSON object per newline,
//! with a `kind` discriminator.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    /// Raw text to type at the prompt. Caller is responsible for any trailing
    /// newline; the listener does not append one.
    Text,
    /// A slash command (e.g. `/mcp`). Forwarded to the slash-command dispatcher
    /// the same way a user-typed slash command would be.
    Slash,
    /// "Approve" the pending agent action.
    Approve,
    /// "Deny" the pending agent action.
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputMsg {
    pub kind: MessageKind,
    pub value: String,
    /// Opaque caller-supplied identifier for de-duplication. Optional on the
    /// wire; defaults to empty.
    #[serde(default)]
    pub client_id: String,
    /// RFC-3339 timestamp from the caller. Optional; defaults to empty. The
    /// server does not validate or interpret this — it is logged for audit.
    #[serde(default)]
    pub ts: String,
}

impl InputMsg {
    pub fn from_line(s: &str) -> serde_json::Result<Self> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_message() {
        let line = r#"{"kind":"text","value":"hi","client_id":"a","ts":"t"}"#;
        let msg = InputMsg::from_line(line).unwrap();
        assert_eq!(msg.kind, MessageKind::Text);
        assert_eq!(msg.value, "hi");
        assert_eq!(msg.client_id, "a");
        assert_eq!(msg.ts, "t");
    }

    #[test]
    fn parses_slash_message_without_optional_fields() {
        let line = r#"{"kind":"slash","value":"/mcp"}"#;
        let msg = InputMsg::from_line(line).unwrap();
        assert_eq!(msg.kind, MessageKind::Slash);
        assert_eq!(msg.value, "/mcp");
        assert_eq!(msg.client_id, "");
        assert_eq!(msg.ts, "");
    }

    #[test]
    fn rejects_unknown_kind() {
        let line = r#"{"kind":"explode","value":"x"}"#;
        assert!(InputMsg::from_line(line).is_err());
    }

    #[test]
    fn approve_and_deny_round_trip() {
        for kind in [MessageKind::Approve, MessageKind::Deny] {
            let original = InputMsg {
                kind,
                value: String::new(),
                client_id: "c".into(),
                ts: "t".into(),
            };
            let s = serde_json::to_string(&original).unwrap();
            let parsed = InputMsg::from_line(&s).unwrap();
            assert_eq!(parsed, original);
        }
    }
}
