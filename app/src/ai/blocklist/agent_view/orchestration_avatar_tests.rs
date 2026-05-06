use warp_cli::agent::Harness;

use super::child_agent_icon_variant;
use crate::ai::agent::conversation::ConversationStatus;
use crate::terminal::CLIAgent;
use crate::ui_components::icon_with_status::IconWithStatusVariant;

#[test]
fn child_agent_icon_variant_uses_existing_cli_agent_logo_for_known_harness() {
    let variant = child_agent_icon_variant(Harness::Claude, ConversationStatus::InProgress, false);

    match variant.expect("known harness should use an existing CLI agent logo") {
        IconWithStatusVariant::CLIAgent {
            agent,
            status,
            is_ambient,
        } => {
            assert_eq!(agent, CLIAgent::Claude);
            assert_eq!(status, Some(ConversationStatus::InProgress));
            assert!(!is_ambient);
        }
        IconWithStatusVariant::OzAgent { .. }
        | IconWithStatusVariant::Neutral { .. }
        | IconWithStatusVariant::NeutralElement { .. } => {
            panic!("Claude harness should render with the existing Claude CLI agent logo")
        }
    }
}

#[test]
fn child_agent_icon_variant_uses_distinct_child_avatar_for_oz_and_unknown_harnesses() {
    for harness in [Harness::Oz, Harness::Unknown] {
        let variant = child_agent_icon_variant(harness, ConversationStatus::Success, true);
        assert!(
            variant.is_none(),
            "{harness:?} child agents should use a distinct child avatar, not the lead Oz/orchestrator avatar"
        );
    }
}
