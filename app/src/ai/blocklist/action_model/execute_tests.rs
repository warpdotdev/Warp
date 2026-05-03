mod binary_detection {
    use std::io::Write as _;

    use async_io::block_on;
    use tempfile::TempDir;

    use super::super::{is_file_content_binary_async, should_read_as_binary};

    fn write_file(dir: &TempDir, name: &str, contents: &[u8]) -> std::path::PathBuf {
        let path = dir.path().join(name);
        let mut file = std::fs::File::create(&path).expect("create temp file");
        file.write_all(contents).expect("write temp file");
        file.flush().expect("flush temp file");
        path
    }

    #[test]
    fn text_file_with_known_extension_is_not_binary() {
        let dir = TempDir::new().expect("create tempdir");
        let path = write_file(&dir, "script.sh", b"#!/usr/bin/env bash\necho hi\n");
        assert!(!block_on(should_read_as_binary(&path)));
    }

    #[test]
    fn binary_file_with_known_extension_is_binary() {
        let dir = TempDir::new().expect("create tempdir");
        // Known binary extension — should be classified as binary without
        // needing content inspection.
        let path = write_file(&dir, "image.png", b"not really a png but extension wins\n");
        assert!(block_on(should_read_as_binary(&path)));
    }

    #[test]
    fn extensionless_shell_script_is_not_binary() {
        // Regression test for QUALITY-507: an extensionless shell script (e.g.
        // `script/linux/bundle`) was being classified as binary solely because
        // its basename isn't in the known extensionless-text allow-list.
        let dir = TempDir::new().expect("create tempdir");
        let path = write_file(
            &dir,
            "bundle",
            b"#!/usr/bin/env bash\n#\n# Builds a Warp binary and bundles it up for distribution.\n\nset -e\n",
        );
        assert!(!block_on(should_read_as_binary(&path)));
    }

    #[test]
    fn extensionless_binary_content_is_binary() {
        // An extensionless file whose contents are actually binary should fall
        // through the content-based check and be classified as binary.
        let dir = TempDir::new().expect("create tempdir");
        let path = write_file(
            &dir,
            "payload",
            // NUL byte is a strong binary signal for content_inspector.
            &[0u8, 1, 2, 3, b'A', 0, 0, 0, 0xFF, 0xFE, 0xFD],
        );
        assert!(block_on(should_read_as_binary(&path)));
    }

    #[test]
    fn extensionless_text_allowlisted_is_not_binary() {
        // Files whose basenames are in the known text allow-list (e.g. README)
        // should take the fast path and skip content inspection.
        let dir = TempDir::new().expect("create tempdir");
        let path = write_file(&dir, "README", b"Hello, world!\n");
        assert!(!block_on(should_read_as_binary(&path)));
    }

    #[test]
    fn empty_extensionless_file_is_not_binary() {
        // `content_inspector` treats an empty buffer as text, which is the
        // desired behavior for `read_files`: an empty file should be
        // surfaced to the agent as an empty string, not as zero binary bytes.
        let dir = TempDir::new().expect("create tempdir");
        let path = write_file(&dir, "empty", b"");
        assert!(!block_on(should_read_as_binary(&path)));
    }

    #[test]
    fn missing_extensionless_file_is_classified_as_binary() {
        // When an extensionless file cannot be opened during content
        // inspection, `should_read_as_binary` must route to the binary path
        // so the binary reader can produce a consistent `Missing` result.
        let dir = TempDir::new().expect("create tempdir");
        let missing = dir.path().join("does-not-exist");
        assert!(block_on(should_read_as_binary(&missing)));
    }

    #[test]
    fn missing_file_helper_is_classified_as_binary() {
        // Direct coverage of the low-level helper: opening a non-existent
        // path must return `true` so the caller doesn't accidentally try the
        // text path on an unreadable file.
        let dir = TempDir::new().expect("create tempdir");
        let missing = dir.path().join("does-not-exist");
        assert!(block_on(is_file_content_binary_async(&missing)));
    }
}

#[cfg(not(target_family = "wasm"))]
mod policy_hooks {
    use std::{
        collections::{HashMap, HashSet},
        path::PathBuf,
    };

    use crate::{
        ai::{
            agent::task::TaskId,
            agent::{
                conversation::AIConversationId, AIAgentAction, AIAgentActionId,
                AIAgentActionResultType, AIAgentActionType, AIAgentPtyWriteMode, FileEdit,
                RequestCommandOutputResult, RequestFileEditsResult,
                WriteToLongRunningShellCommandResult,
            },
            policy_hooks::{
                decision::{
                    compose_policy_decisions, AgentPolicyHookEvaluation,
                    WarpPermissionDecisionKind, WarpPermissionSnapshot,
                },
                AgentPolicyAction, AgentPolicyDecisionKind, AgentPolicyEffectiveDecision,
                AgentPolicyEvent, AgentPolicyHookConfig,
            },
        },
        terminal::shell::ShellType,
    };

    use super::super::{
        agent_policy_action, complete_policy_preflight_if_pending,
        confirmed_file_edit_policy_preprocess_state_from_cached_decision,
        normalize_command_for_policy, policy_denied_action_result,
        policy_preflight_state_from_decision, recompose_completed_policy_decision,
        should_consume_completed_policy_preflight,
        should_preprocess_file_edits_after_policy_decision,
        should_preserve_completed_policy_preflight_for_file_edit_preprocess,
        warp_permission_snapshot_for_policy, PolicyPreflightKey, PolicyPreflightState,
    };

    fn command_action(command: &str) -> AIAgentAction {
        AIAgentAction {
            id: AIAgentActionId::from("action_1".to_string()),
            task_id: TaskId::new("task_1".to_string()),
            action: AIAgentActionType::RequestCommandOutput {
                command: command.to_string(),
                is_read_only: Some(false),
                is_risky: Some(true),
                wait_until_completion: false,
                uses_pager: None,
                rationale: None,
                citations: Vec::new(),
            },
            requires_result: true,
        }
    }

    fn policy_command_action(command: &str) -> AgentPolicyAction {
        agent_policy_action(&command_action(command), None, &None, &None)
            .expect("command action should build a policy action")
    }

    fn policy_preflight_key(
        conversation_id: AIConversationId,
        action_id: AIAgentActionId,
        action: AgentPolicyAction,
    ) -> PolicyPreflightKey {
        let event = AgentPolicyEvent::new(
            conversation_id.to_string(),
            action_id.to_string(),
            None,
            false,
            Some("profile_default".to_string()),
            WarpPermissionSnapshot::allow(None),
            action,
        );
        PolicyPreflightKey::new(
            conversation_id,
            action_id,
            &event,
            &AgentPolicyHookConfig::default(),
        )
    }

    fn write_to_shell_action(input: &str) -> AIAgentAction {
        AIAgentAction {
            id: AIAgentActionId::from("action_1".to_string()),
            task_id: TaskId::new("task_1".to_string()),
            action: AIAgentActionType::WriteToLongRunningShellCommand {
                block_id: "block_1".to_string().into(),
                input: bytes::Bytes::from(input.to_string()),
                mode: AIAgentPtyWriteMode::Line,
            },
            requires_result: true,
        }
    }

    fn file_edit_action() -> AIAgentAction {
        AIAgentAction {
            id: AIAgentActionId::from("action_1".to_string()),
            task_id: TaskId::new("task_1".to_string()),
            action: AIAgentActionType::RequestFileEdits {
                file_edits: vec![FileEdit::Create {
                    file: Some("src/lib.rs".to_string()),
                    content: Some("fn main() {}\n".to_string()),
                }],
                title: None,
            },
            requires_result: true,
        }
    }

    #[test]
    fn policy_denied_result_preserves_command_and_policy_reason() {
        let action = command_action("OPENAI_API_KEY=sk-secretsecretsecret rm -rf target");
        let decision = AgentPolicyEffectiveDecision {
            decision: AgentPolicyDecisionKind::Deny,
            reason: Some("blocked".to_string()),
            warp_permission: WarpPermissionSnapshot::allow(None),
            hook_results: vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Deny,
                reason: Some("dangerous command".to_string()),
                external_audit_id: Some("audit_1".to_string()),
                error: None,
            }],
        };

        let result = policy_denied_action_result(&action, &decision);

        assert_eq!(
            result,
            AIAgentActionResultType::RequestCommandOutput(
                RequestCommandOutputResult::PolicyDenied {
                    command: "OPENAI_API_KEY=<redacted> rm -rf target".to_string(),
                    reason: "guard denied the action: dangerous command".to_string(),
                }
            )
        );
    }

    #[test]
    fn policy_denied_file_edit_result_uses_stable_policy_variant() {
        let action = AIAgentAction {
            id: AIAgentActionId::from("action_1".to_string()),
            task_id: TaskId::new("task_1".to_string()),
            action: AIAgentActionType::RequestFileEdits {
                file_edits: vec![FileEdit::Create {
                    file: Some("src/lib.rs".to_string()),
                    content: Some("fn main() {}\n".to_string()),
                }],
                title: None,
            },
            requires_result: true,
        };
        let decision = AgentPolicyEffectiveDecision {
            decision: AgentPolicyDecisionKind::Deny,
            reason: Some("blocked".to_string()),
            warp_permission: WarpPermissionSnapshot::allow(None),
            hook_results: vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Deny,
                reason: Some("protected path".to_string()),
                external_audit_id: Some("audit_1".to_string()),
                error: None,
            }],
        };

        let result = policy_denied_action_result(&action, &decision);

        assert_eq!(
            result,
            AIAgentActionResultType::RequestFileEdits(RequestFileEditsResult::PolicyDenied {
                reason: "guard denied the action: protected path".to_string(),
            })
        );
    }

    #[test]
    fn policy_denied_write_to_shell_result_uses_stable_policy_variant() {
        let action = write_to_shell_action("q\n");
        let decision = AgentPolicyEffectiveDecision {
            decision: AgentPolicyDecisionKind::Deny,
            reason: Some("blocked".to_string()),
            warp_permission: WarpPermissionSnapshot::allow(None),
            hook_results: vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Deny,
                reason: Some("interactive write blocked".to_string()),
                external_audit_id: Some("audit_1".to_string()),
                error: None,
            }],
        };

        let result = policy_denied_action_result(&action, &decision);

        assert_eq!(
            result,
            AIAgentActionResultType::WriteToLongRunningShellCommand(
                WriteToLongRunningShellCommandResult::PolicyDenied {
                    reason: "guard denied the action: interactive write blocked".to_string(),
                }
            )
        );
    }

    #[test]
    fn warp_denied_command_result_preserves_denylisted_variant() {
        let action = command_action("rm -rf target");
        let decision = compose_policy_decisions(
            WarpPermissionSnapshot::deny(Some(
                "command is explicitly denylisted by Warp permissions".to_string(),
            )),
            vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Allow,
                reason: Some("approved by hook".to_string()),
                external_audit_id: Some("audit_1".to_string()),
                error: None,
            }],
            true,
        );

        let result = policy_denied_action_result(&action, &decision);

        assert_eq!(
            result,
            AIAgentActionResultType::RequestCommandOutput(RequestCommandOutputResult::Denylisted {
                command: "rm -rf target".to_string(),
            })
        );
    }

    #[test]
    fn warp_denied_file_edit_result_does_not_use_host_policy_variant() {
        let action = file_edit_action();
        let decision = compose_policy_decisions(
            WarpPermissionSnapshot::deny(Some(
                "file path is protected by Warp permissions".to_string(),
            )),
            vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Allow,
                reason: Some("approved by hook".to_string()),
                external_audit_id: Some("audit_1".to_string()),
                error: None,
            }],
            true,
        );

        let result = policy_denied_action_result(&action, &decision);

        assert_eq!(
            result,
            AIAgentActionResultType::RequestFileEdits(
                RequestFileEditsResult::DiffApplicationFailed {
                    error:
                        "Blocked by Warp permissions: file path is protected by Warp permissions"
                            .to_string(),
                }
            )
        );
    }

    #[test]
    fn warp_permission_snapshot_marks_autonomous_denials_terminal() {
        let snapshot = warp_permission_snapshot_for_policy(false, false, false, true, None);

        assert_eq!(snapshot.decision, WarpPermissionDecisionKind::Deny);
    }

    #[test]
    fn warp_permission_snapshot_preserves_terminal_denial_before_hook_autoapproval() {
        let snapshot = warp_permission_snapshot_for_policy(
            false,
            false,
            true,
            false,
            Some("file path is protected by Warp permissions".to_string()),
        );

        assert_eq!(snapshot.decision, WarpPermissionDecisionKind::Deny);

        let decision = compose_policy_decisions(
            snapshot,
            vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Allow,
                reason: Some("approved by hook".to_string()),
                external_audit_id: None,
                error: None,
            }],
            true,
        );

        assert_eq!(decision.decision, AgentPolicyDecisionKind::Deny);
        assert_eq!(
            decision.reason.as_deref(),
            Some("file path is protected by Warp permissions")
        );
    }

    #[test]
    fn cached_ask_policy_decision_is_retained_until_user_confirmation() {
        let action = command_action("rm -rf target");
        let decision = AgentPolicyEffectiveDecision {
            decision: AgentPolicyDecisionKind::Ask,
            reason: Some("requires approval".to_string()),
            warp_permission: WarpPermissionSnapshot::allow(None),
            hook_results: vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Ask,
                reason: Some("requires approval".to_string()),
                external_audit_id: Some("audit_1".to_string()),
                error: None,
            }],
        };

        let unconfirmed = policy_preflight_state_from_decision(&action, &decision, false);
        assert!(matches!(
            unconfirmed,
            PolicyPreflightState::NeedsConfirmation(_)
        ));
        assert!(!should_consume_completed_policy_preflight(&unconfirmed));

        let confirmed = policy_preflight_state_from_decision(&action, &decision, true);
        assert_eq!(
            confirmed,
            PolicyPreflightState::Allowed {
                skip_confirmation: false
            }
        );
        assert!(should_consume_completed_policy_preflight(&confirmed));
    }

    #[test]
    fn hook_autoapproval_skips_warp_confirmation() {
        let action = command_action("rm -rf target");
        let decision = AgentPolicyEffectiveDecision {
            decision: AgentPolicyDecisionKind::Allow,
            reason: Some("approved by hook".to_string()),
            warp_permission: WarpPermissionSnapshot::ask(Some("AlwaysAsk".to_string())),
            hook_results: vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Allow,
                reason: Some("approved by hook".to_string()),
                external_audit_id: Some("audit_1".to_string()),
                error: None,
            }],
        };

        let state = policy_preflight_state_from_decision(&action, &decision, false);

        assert_eq!(
            state,
            PolicyPreflightState::Allowed {
                skip_confirmation: true
            }
        );
    }

    #[test]
    fn file_edit_policy_ask_defers_diff_preprocessing_until_confirmation() {
        let action = file_edit_action();
        let decision = AgentPolicyEffectiveDecision {
            decision: AgentPolicyDecisionKind::Ask,
            reason: Some("requires approval".to_string()),
            warp_permission: WarpPermissionSnapshot::allow(None),
            hook_results: vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Ask,
                reason: Some("requires approval".to_string()),
                external_audit_id: Some("audit_1".to_string()),
                error: None,
            }],
        };

        assert!(!should_preprocess_file_edits_after_policy_decision(
            &action, &decision
        ));
    }

    #[test]
    fn confirmed_file_edit_policy_preprocess_retry_skips_confirmation() {
        let action = file_edit_action();
        let cached_decision = AgentPolicyEffectiveDecision {
            decision: AgentPolicyDecisionKind::Ask,
            reason: Some("requires approval".to_string()),
            warp_permission: WarpPermissionSnapshot::allow(None),
            hook_results: vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Ask,
                reason: Some("requires approval".to_string()),
                external_audit_id: Some("audit_1".to_string()),
                error: None,
            }],
        };

        assert_eq!(
            confirmed_file_edit_policy_preprocess_state_from_cached_decision(
                &action,
                &cached_decision,
                WarpPermissionSnapshot::allow(None),
                true
            ),
            PolicyPreflightState::Allowed {
                skip_confirmation: true
            }
        );
    }

    #[test]
    fn confirmed_file_edit_policy_preprocess_retry_recomposes_changed_warp_denial() {
        let action = file_edit_action();
        let cached_decision = compose_policy_decisions(
            WarpPermissionSnapshot::allow(Some("initial allow".to_string())),
            vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Allow,
                reason: Some("approved by hook".to_string()),
                external_audit_id: Some("audit_1".to_string()),
                error: None,
            }],
            true,
        );

        let state = confirmed_file_edit_policy_preprocess_state_from_cached_decision(
            &action,
            &cached_decision,
            WarpPermissionSnapshot::deny(Some("managed policy changed".to_string())),
            true,
        );

        assert_eq!(
            state,
            PolicyPreflightState::Denied(AIAgentActionResultType::RequestFileEdits(
                RequestFileEditsResult::DiffApplicationFailed {
                    error: "Blocked by Warp permissions: managed policy changed".to_string()
                }
            ))
        );
    }

    #[test]
    fn confirmed_file_edit_policy_preprocess_retry_reprompts_changed_warp_ask() {
        let action = file_edit_action();
        let cached_decision = compose_policy_decisions(
            WarpPermissionSnapshot::allow(Some("initial allow".to_string())),
            vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Allow,
                reason: Some("approved by hook".to_string()),
                external_audit_id: Some("audit_1".to_string()),
                error: None,
            }],
            true,
        );

        let state = confirmed_file_edit_policy_preprocess_state_from_cached_decision(
            &action,
            &cached_decision,
            WarpPermissionSnapshot::ask(Some("permission changed".to_string())),
            true,
        );

        assert_eq!(
            state,
            PolicyPreflightState::NeedsConfirmation(Some("permission changed".to_string()))
        );
    }

    #[test]
    fn completed_file_edit_policy_preflight_is_preserved_until_preprocessed() {
        let action = file_edit_action();
        let state = PolicyPreflightState::Allowed {
            skip_confirmation: false,
        };

        assert!(
            should_preserve_completed_policy_preflight_for_file_edit_preprocess(
                &action, &state, false
            )
        );
        assert!(
            !should_preserve_completed_policy_preflight_for_file_edit_preprocess(
                &action, &state, true
            )
        );
        assert!(
            !should_preserve_completed_policy_preflight_for_file_edit_preprocess(
                &action,
                &PolicyPreflightState::NeedsConfirmation(Some("requires approval".to_string())),
                false
            )
        );
    }

    #[test]
    fn cached_policy_decision_recomposes_against_current_warp_denial() {
        let cached_decision = compose_policy_decisions(
            WarpPermissionSnapshot::allow(Some("initial allow".to_string())),
            vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Allow,
                reason: Some("approved by hook".to_string()),
                external_audit_id: Some("audit_1".to_string()),
                error: None,
            }],
            true,
        );

        let recomposed = recompose_completed_policy_decision(
            &cached_decision,
            WarpPermissionSnapshot::deny(Some("managed policy changed".to_string())),
            true,
        );

        assert_eq!(recomposed.decision, AgentPolicyDecisionKind::Deny);
        assert_eq!(recomposed.reason.as_deref(), Some("managed policy changed"));
        assert_eq!(
            recomposed.warp_permission.decision,
            WarpPermissionDecisionKind::Deny
        );
        assert_eq!(recomposed.hook_results, cached_decision.hook_results);
    }

    #[test]
    fn cached_policy_decision_does_not_autoapprove_changed_warp_ask() {
        let cached_decision = compose_policy_decisions(
            WarpPermissionSnapshot::allow(Some("initial allow".to_string())),
            vec![AgentPolicyHookEvaluation {
                hook_name: "guard".to_string(),
                decision: AgentPolicyDecisionKind::Allow,
                reason: Some("approved by hook".to_string()),
                external_audit_id: Some("audit_1".to_string()),
                error: None,
            }],
            true,
        );

        let recomposed = recompose_completed_policy_decision(
            &cached_decision,
            WarpPermissionSnapshot::ask(Some("permission changed".to_string())),
            true,
        );

        assert_eq!(recomposed.decision, AgentPolicyDecisionKind::Ask);
        assert_eq!(recomposed.reason.as_deref(), Some("permission changed"));
        assert_eq!(
            recomposed.warp_permission.decision,
            WarpPermissionDecisionKind::Ask
        );
    }

    #[test]
    fn policy_preflight_key_scopes_same_action_id_by_conversation() {
        let action_id = AIAgentActionId::from("action_1".to_string());
        let policy_action = policy_command_action("ls");
        let conversation_one = AIConversationId::new();
        let conversation_two = AIConversationId::new();
        let key_one =
            policy_preflight_key(conversation_one, action_id.clone(), policy_action.clone());
        let key_two = policy_preflight_key(conversation_two, action_id, policy_action);

        assert_ne!(key_one, key_two);

        let mut pending = HashSet::new();
        pending.insert(key_one);
        assert!(!pending.contains(&key_two));
    }

    #[test]
    fn policy_preflight_key_scopes_same_action_id_by_action_payload() {
        let action_id = AIAgentActionId::from("action_1".to_string());
        let conversation_id = AIConversationId::new();
        let old_action = policy_command_action("echo old");
        let new_action = policy_command_action("echo new");

        let old_key = policy_preflight_key(conversation_id, action_id.clone(), old_action);
        let new_key = policy_preflight_key(conversation_id, action_id.clone(), new_action);

        assert_ne!(old_key, new_key);
        assert!(old_key.matches_action(conversation_id, &action_id));
    }

    #[test]
    fn policy_preflight_key_scopes_policy_event_context() {
        let conversation_id = AIConversationId::new();
        let action_id = AIAgentActionId::from("action_1".to_string());
        let action = policy_command_action("ls");
        let config = AgentPolicyHookConfig::default();
        let base_event = AgentPolicyEvent::new(
            conversation_id.to_string(),
            action_id.to_string(),
            Some(PathBuf::from("/repo")),
            false,
            Some("profile_a".to_string()),
            WarpPermissionSnapshot::allow(None),
            action.clone(),
        );
        let changed_cwd = AgentPolicyEvent::new(
            conversation_id.to_string(),
            action_id.to_string(),
            Some(PathBuf::from("/other")),
            false,
            Some("profile_a".to_string()),
            WarpPermissionSnapshot::allow(None),
            action.clone(),
        );
        let changed_run_mode = AgentPolicyEvent::new(
            conversation_id.to_string(),
            action_id.to_string(),
            Some(PathBuf::from("/repo")),
            true,
            Some("profile_a".to_string()),
            WarpPermissionSnapshot::allow(None),
            action.clone(),
        );
        let changed_profile = AgentPolicyEvent::new(
            conversation_id.to_string(),
            action_id.to_string(),
            Some(PathBuf::from("/repo")),
            false,
            Some("profile_b".to_string()),
            WarpPermissionSnapshot::allow(None),
            action,
        );

        let base_key =
            PolicyPreflightKey::new(conversation_id, action_id.clone(), &base_event, &config);

        assert_ne!(
            base_key,
            PolicyPreflightKey::new(conversation_id, action_id.clone(), &changed_cwd, &config)
        );
        assert_ne!(
            base_key,
            PolicyPreflightKey::new(
                conversation_id,
                action_id.clone(),
                &changed_run_mode,
                &config
            )
        );
        assert_ne!(
            base_key,
            PolicyPreflightKey::new(conversation_id, action_id, &changed_profile, &config)
        );
    }

    #[test]
    fn policy_preflight_key_scopes_hook_config() {
        let conversation_id = AIConversationId::new();
        let action_id = AIAgentActionId::from("action_1".to_string());
        let event = AgentPolicyEvent::new(
            conversation_id.to_string(),
            action_id.to_string(),
            None,
            false,
            Some("profile_default".to_string()),
            WarpPermissionSnapshot::allow(None),
            policy_command_action("ls"),
        );
        let old_config = AgentPolicyHookConfig {
            enabled: true,
            timeout_ms: 5_000,
            ..Default::default()
        };
        let new_config = AgentPolicyHookConfig {
            enabled: true,
            timeout_ms: 10_000,
            ..Default::default()
        };

        assert_ne!(
            PolicyPreflightKey::new(conversation_id, action_id.clone(), &event, &old_config),
            PolicyPreflightKey::new(conversation_id, action_id, &event, &new_config)
        );
    }

    #[test]
    fn cancelled_policy_preflight_completion_is_not_cached() {
        let action_id = AIAgentActionId::from("action_1".to_string());
        let preflight_key = policy_preflight_key(
            AIConversationId::new(),
            action_id,
            policy_command_action("ls"),
        );
        let decision = AgentPolicyEffectiveDecision {
            decision: AgentPolicyDecisionKind::Allow,
            reason: None,
            warp_permission: WarpPermissionSnapshot::allow(None),
            hook_results: Vec::new(),
        };
        let mut pending = HashSet::new();
        let mut completed = HashMap::new();

        assert!(!complete_policy_preflight_if_pending(
            &mut pending,
            &mut completed,
            preflight_key.clone(),
            decision.clone()
        ));
        assert!(!completed.contains_key(&preflight_key));

        pending.insert(preflight_key.clone());
        assert!(complete_policy_preflight_if_pending(
            &mut pending,
            &mut completed,
            preflight_key.clone(),
            decision
        ));
        assert!(pending.is_empty());
        assert!(completed.contains_key(&preflight_key));
    }

    #[test]
    fn write_file_policy_action_omits_unavailable_diff_stats() {
        let action = AIAgentAction {
            id: AIAgentActionId::from("action_1".to_string()),
            task_id: TaskId::new("task_1".to_string()),
            action: AIAgentActionType::RequestFileEdits {
                file_edits: vec![FileEdit::Create {
                    file: Some("src/lib.rs".to_string()),
                    content: Some("fn main() {}\n".to_string()),
                }],
                title: None,
            },
            requires_result: true,
        };

        let Some(AgentPolicyAction::WriteFiles(write_files)) =
            agent_policy_action(&action, None, &None, &None)
        else {
            panic!("expected write-files policy action");
        };

        assert_eq!(write_files.paths.len(), 1);
        assert_eq!(write_files.diff_stats, None);
    }

    #[test]
    fn write_to_shell_policy_action_is_governed_and_redacted() {
        let action = write_to_shell_action("Authorization: Bearer secret-token\n:q\n");

        let Some(AgentPolicyAction::WriteToLongRunningShellCommand(write)) =
            agent_policy_action(&action, None, &None, &None)
        else {
            panic!("expected write-to-long-running-shell-command policy action");
        };

        assert_eq!(write.block_id, "block_1");
        assert_eq!(write.mode, "line");
        assert_eq!(write.input, "Authorization: Bearer <redacted>\n:q\n");
    }

    #[test]
    fn command_normalization_matches_shell_escape_style() {
        assert_eq!(
            normalize_command_for_policy("echo one\\\n+two", Some(ShellType::Bash)),
            "echo one +two"
        );
        assert_eq!(
            normalize_command_for_policy("echo one`\n+two", Some(ShellType::PowerShell)),
            "echo one +two"
        );
    }
}
