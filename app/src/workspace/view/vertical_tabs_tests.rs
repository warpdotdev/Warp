use crate::ai::agent::conversation::ConversationStatus;
use crate::context_chips::display_chip::GitLineChanges;
use crate::pane_group::pane::IPaneType;
use crate::pane_group::{PaneId, TerminalPaneId};
use crate::safe_triangle::SafeTriangle;
use crate::terminal::CLIAgent;
use crate::workspace::tab_settings::VerticalTabsDisplayGranularity;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use std::path::PathBuf;
use warpui::elements::PositionedElementOffsetBounds;
use warpui::EntityId;

use super::{
    branch_label_display, coalesce_summary_branch_entries, code_detail_kind_label,
    compact_branch_subtitle_display, detail_sidecar_width_and_bounds,
    detail_target_for_hovered_row, format_summary_primary_labels,
    non_terminal_search_text_fragments, pane_ids_for_display_granularity,
    pane_search_text_fragments, preferred_agent_tab_titles, search_fragments_contain_query,
    secondary_pair_placement, select_summary_pane_kind_icons,
    should_keep_detail_sidecar_visible_for_mouse_position, summary_overflow_count,
    summary_pane_kind_to_status_variant, summary_search_text_fragments, terminal_kind_badge_label,
    terminal_primary_line_data, terminal_pull_request_badge_label, terminal_search_text_fragments,
    terminal_title_fallback_font, uses_outer_group_container, visible_pane_ids_for_detail_target,
    vtab_diff_stats_text, AgentTabTextPreference, SummaryPaneKind, SummaryPaneKindIcons,
    TerminalAgentText, TerminalPrimaryLineData, TerminalPrimaryLineFont, VerticalTabsDetailTarget,
    VerticalTabsDetailTargetKind, VerticalTabsSummaryBranchEntry, VerticalTabsSummaryData,
};
use warpui::elements::{ChildAnchor, ParentAnchor};

fn pane_id() -> PaneId {
    TerminalPaneId::dummy_terminal_pane_id().into()
}
fn code_summary_kind(title: &str) -> SummaryPaneKind {
    SummaryPaneKind::Code {
        title: title.to_string(),
    }
}

#[test]
fn summary_pane_kind_icons_render_single_icon_for_homogeneous_tabs() {
    assert_eq!(
        select_summary_pane_kind_icons([
            (EntityId::from_usize(10), SummaryPaneKind::Terminal),
            (EntityId::from_usize(20), SummaryPaneKind::Terminal),
        ]),
        Some(SummaryPaneKindIcons::Single(SummaryPaneKind::Terminal))
    );
}

#[test]
fn summary_pane_kind_icons_pick_two_oldest_distinct_pane_kinds() {
    assert_eq!(
        select_summary_pane_kind_icons([
            (EntityId::from_usize(30), SummaryPaneKind::Terminal),
            (EntityId::from_usize(20), code_summary_kind("main.rs")),
            (
                EntityId::from_usize(40),
                SummaryPaneKind::Notebook { is_plan: false },
            ),
            (EntityId::from_usize(10), SummaryPaneKind::Terminal),
        ]),
        Some(SummaryPaneKindIcons::Pair {
            primary: SummaryPaneKind::Terminal,
            secondary: code_summary_kind("main.rs"),
        })
    );
}

#[test]
fn summary_pane_kind_icons_recompute_when_oldest_kind_is_removed() {
    assert_eq!(
        select_summary_pane_kind_icons([
            (EntityId::from_usize(20), code_summary_kind("main.rs")),
            (EntityId::from_usize(30), SummaryPaneKind::Terminal),
        ]),
        Some(SummaryPaneKindIcons::Pair {
            primary: code_summary_kind("main.rs"),
            secondary: SummaryPaneKind::Terminal,
        })
    );
}

#[test]
fn summary_pane_kind_icons_distinguish_agent_terminals_from_plain_terminals() {
    assert_eq!(
        select_summary_pane_kind_icons([
            (EntityId::from_usize(10), SummaryPaneKind::Terminal),
            (
                EntityId::from_usize(20),
                SummaryPaneKind::CLIAgent {
                    agent: CLIAgent::Claude,
                    is_ambient: false,
                    status: None,
                },
            ),
            (
                EntityId::from_usize(30),
                SummaryPaneKind::OzAgent {
                    is_ambient: false,
                    status: None,
                },
            ),
        ]),
        Some(SummaryPaneKindIcons::Pair {
            primary: SummaryPaneKind::Terminal,
            secondary: SummaryPaneKind::CLIAgent {
                agent: CLIAgent::Claude,
                is_ambient: false,
                status: None,
            },
        })
    );
}

#[test]
fn summary_pane_kind_icons_distinguish_ambient_claude_from_local_claude() {
    // A local Claude session and a cloud-mode Claude session should count as distinct kinds
    // so they render with different icons (claude.svg vs claude_cloud.svg).
    assert_eq!(
        select_summary_pane_kind_icons([
            (
                EntityId::from_usize(10),
                SummaryPaneKind::CLIAgent {
                    agent: CLIAgent::Claude,
                    is_ambient: false,
                    status: None,
                },
            ),
            (
                EntityId::from_usize(20),
                SummaryPaneKind::CLIAgent {
                    agent: CLIAgent::Claude,
                    is_ambient: true,
                    status: None,
                },
            ),
        ]),
        Some(SummaryPaneKindIcons::Pair {
            primary: SummaryPaneKind::CLIAgent {
                agent: CLIAgent::Claude,
                is_ambient: false,
                status: None,
            },
            secondary: SummaryPaneKind::CLIAgent {
                agent: CLIAgent::Claude,
                is_ambient: true,
                status: None,
            },
        })
    );
}

// Regression tests for #9868: agent status badges in vertical tabs summary view.

#[test]
fn summary_view_propagates_oz_agent_status_to_icon_variant() {
    // The summary-icon path must surface the agent's status so that
    // `render_icon_with_status` can paint the badge. Before #9868 the helper
    // hardcoded `status: None`, so the badge never rendered.
    let kind = SummaryPaneKind::OzAgent {
        is_ambient: true,
        status: Some(ConversationStatus::InProgress),
    };
    let variant =
        summary_pane_kind_to_status_variant(&kind).expect("agent kinds produce a variant");
    match variant {
        crate::ui_components::icon_with_status::IconWithStatusVariant::OzAgent {
            status,
            is_ambient,
        } => {
            assert_eq!(status, Some(ConversationStatus::InProgress));
            assert!(is_ambient);
        }
        _ => panic!("expected OzAgent variant"),
    }
}

#[test]
fn summary_view_propagates_cli_agent_status_to_icon_variant() {
    let kind = SummaryPaneKind::CLIAgent {
        agent: CLIAgent::Claude,
        is_ambient: false,
        status: Some(ConversationStatus::Error),
    };
    let variant =
        summary_pane_kind_to_status_variant(&kind).expect("agent kinds produce a variant");
    match variant {
        crate::ui_components::icon_with_status::IconWithStatusVariant::CLIAgent {
            agent,
            status,
            is_ambient,
        } => {
            assert_eq!(agent, CLIAgent::Claude);
            assert_eq!(status, Some(ConversationStatus::Error));
            assert!(!is_ambient);
        }
        _ => panic!("expected CLIAgent variant"),
    }
}

#[test]
fn summary_view_renders_status_for_non_ambient_agents() {
    // Before #9868, only ambient agent kinds went through the status-aware
    // render path; non-ambient agents fell through to a status-less inline
    // render. Both paths must now surface a variant so the status badge can
    // be drawn.
    let local_oz = SummaryPaneKind::OzAgent {
        is_ambient: false,
        status: Some(ConversationStatus::Success),
    };
    let local_cli = SummaryPaneKind::CLIAgent {
        agent: CLIAgent::Claude,
        is_ambient: false,
        status: Some(ConversationStatus::Cancelled),
    };

    assert!(
        summary_pane_kind_to_status_variant(&local_oz).is_some(),
        "non-ambient Oz agents must produce a status-aware variant"
    );
    assert!(
        summary_pane_kind_to_status_variant(&local_cli).is_some(),
        "non-ambient CLI agents must produce a status-aware variant"
    );
}

#[test]
fn summary_view_returns_none_for_non_agent_kinds() {
    // Non-agent kinds keep their existing inline rendering — the status helper
    // returns None so the caller falls back to `render_summary_pane_kind_icon_circle`'s
    // inline branch.
    assert!(summary_pane_kind_to_status_variant(&SummaryPaneKind::Terminal).is_none());
    assert!(summary_pane_kind_to_status_variant(&code_summary_kind("main.rs")).is_none());
    assert!(summary_pane_kind_to_status_variant(&SummaryPaneKind::Settings).is_none());
}

// Regression test for #10179: when the primary kind in a Pair has a status
// badge (i.e., is an agent), the secondary icon must be placed at TR not BR
// — otherwise the BR-anchored status badge gets occluded by the secondary.
//
// The placement decision in render_summary_pane_kind_icons is gated on
// `summary_pane_kind_to_status_variant(primary).is_some()`. This test pins
// the contract that decision-flag depends on, so a future refactor that
// changes which kinds report a badge automatically updates the layout
// behavior in lockstep.
#[test]
fn pair_icon_placement_avoids_badge_for_agent_primary() {
    // Every agent kind (Oz/CLI, ambient/non-ambient, with or without status)
    // routes through the status-aware path now, so the pair-icon layout
    // must move the secondary to TR for all of them.
    for kind in [
        SummaryPaneKind::OzAgent {
            is_ambient: true,
            status: Some(ConversationStatus::InProgress),
        },
        SummaryPaneKind::OzAgent {
            is_ambient: false,
            status: None,
        },
        SummaryPaneKind::CLIAgent {
            agent: CLIAgent::Claude,
            is_ambient: true,
            status: Some(ConversationStatus::Success),
        },
        SummaryPaneKind::CLIAgent {
            agent: CLIAgent::Claude,
            is_ambient: false,
            status: None,
        },
    ] {
        assert!(
            summary_pane_kind_to_status_variant(&kind).is_some(),
            "agent kind must produce a status-aware variant; pair-icon \
             placement relies on this to move the secondary to TR and \
             avoid covering the BR-anchored status badge"
        );
    }

    // Non-agent primaries don't paint a badge, so the secondary takes BR
    // (existing layout, unchanged by #10179).
    for kind in [
        SummaryPaneKind::Terminal,
        code_summary_kind("main.rs"),
        SummaryPaneKind::Notebook { is_plan: false },
        SummaryPaneKind::Settings,
    ] {
        assert!(
            summary_pane_kind_to_status_variant(&kind).is_none(),
            "non-agent kind must NOT produce a status-aware variant; \
             pair-icon placement keeps the secondary at BR for these"
        );
    }
}

#[test]
fn summary_pane_kind_icons_dedup_ignores_status() {
    // Two Oz agents with different statuses still collapse to a single icon
    // in the summary view: status is metadata, not identity, for dedup. The
    // surviving entry preserves the first pane's status (creation-order
    // sorted).
    let icons = select_summary_pane_kind_icons([
        (
            EntityId::from_usize(10),
            SummaryPaneKind::OzAgent {
                is_ambient: false,
                status: Some(ConversationStatus::InProgress),
            },
        ),
        (
            EntityId::from_usize(20),
            SummaryPaneKind::OzAgent {
                is_ambient: false,
                status: Some(ConversationStatus::Success),
            },
        ),
    ])
    .expect("at least one kind");

    match icons {
        SummaryPaneKindIcons::Single(SummaryPaneKind::OzAgent {
            is_ambient,
            status,
        }) => {
            assert!(!is_ambient);
            assert_eq!(status, Some(ConversationStatus::InProgress));
        }
        other => panic!("expected a single OzAgent kind, got {other:?}"),
    }
}

// Regression tests for the #10179 round-2 fix: when the primary kind has a
// status badge (agent), the secondary moves to TR. The Y component of the
// offset must FLIP to negative so the secondary goes UP into the TR
// quadrant, not DOWN into the primary. These tests pin the geometric
// contract that the bot caught a regression in last round.

#[test]
fn pair_secondary_placement_for_agent_primary_anchors_top_right_with_negative_y() {
    let p = secondary_pair_placement(true, 7.5);
    assert_eq!(
        p.parent_anchor,
        ParentAnchor::TopRight,
        "agent primary must anchor secondary at TR to leave BR clear for the status badge"
    );
    assert_eq!(p.child_anchor, ChildAnchor::TopRight);
    assert_eq!(
        p.offset_x, 7.5,
        "X always positive: secondary sits to the right of the primary"
    );
    assert_eq!(
        p.offset_y, -7.5,
        "Y must FLIP to negative for TR: pathfinder is +Y-down, so a +Y \
         offset would push the secondary DOWN into the primary instead of \
         UP into the TR quadrant"
    );
}

#[test]
fn pair_secondary_placement_for_non_agent_primary_anchors_bottom_right_with_positive_y() {
    let p = secondary_pair_placement(false, 7.5);
    assert_eq!(
        p.parent_anchor,
        ParentAnchor::BottomRight,
        "non-agent primary keeps the existing BR placement (no status badge to occlude)"
    );
    assert_eq!(p.child_anchor, ChildAnchor::BottomRight);
    assert_eq!(p.offset_x, 7.5);
    assert_eq!(
        p.offset_y, 7.5,
        "Y stays positive for BR: pushes the secondary further into the BR quadrant"
    );
}

#[test]
fn pair_secondary_placement_offset_magnitude_matches_across_corners() {
    // Both placements use the same magnitude — only the corner / Y-sign changes.
    let agent = secondary_pair_placement(true, 12.0);
    let non_agent = secondary_pair_placement(false, 12.0);
    assert_eq!(agent.offset_x.abs(), non_agent.offset_x.abs());
    assert_eq!(agent.offset_y.abs(), non_agent.offset_y.abs());
    assert_ne!(
        agent.offset_y, non_agent.offset_y,
        "Y signs must differ between TR (agent) and BR (non-agent) placements"
    );
}

#[test]
fn preferred_agent_tab_titles_default_to_title_like_text() {
    let agent_text = TerminalAgentText {
        conversation_display_title: Some("Generated Oz title".to_string()),
        conversation_latest_user_prompt: Some("Latest Oz prompt".to_string()),
        cli_agent_title: Some("CLI summary".to_string()),
        cli_agent_latest_user_prompt: Some("Latest CLI prompt".to_string()),
        is_oz_agent: true,
        cli_agent: Some(CLIAgent::Claude),
    };

    assert_eq!(
        preferred_agent_tab_titles(&agent_text, AgentTabTextPreference::ConversationTitle),
        (
            Some("Generated Oz title".to_string()),
            Some("CLI summary".to_string())
        )
    );
}

#[test]
fn preferred_agent_tab_titles_do_not_use_cli_prompt_when_disabled() {
    let agent_text = TerminalAgentText {
        conversation_display_title: None,
        conversation_latest_user_prompt: None,
        cli_agent_title: None,
        cli_agent_latest_user_prompt: Some("Latest CLI prompt".to_string()),
        is_oz_agent: false,
        cli_agent: Some(CLIAgent::Claude),
    };

    assert_eq!(
        preferred_agent_tab_titles(&agent_text, AgentTabTextPreference::ConversationTitle),
        (None, None)
    );
}

#[test]
fn terminal_primary_line_uses_terminal_title_when_disabled_cli_has_only_prompt() {
    let agent_text = TerminalAgentText {
        conversation_display_title: None,
        conversation_latest_user_prompt: None,
        cli_agent_title: None,
        cli_agent_latest_user_prompt: Some("Latest CLI prompt".to_string()),
        is_oz_agent: false,
        cli_agent: Some(CLIAgent::Claude),
    };
    let (conversation_title, cli_title) =
        preferred_agent_tab_titles(&agent_text, AgentTabTextPreference::ConversationTitle);

    let line = terminal_primary_line_data(
        false,
        conversation_title,
        cli_title,
        "Generated Claude Code title",
        "~/warp",
        terminal_title_fallback_font(&agent_text),
        Some("claude".to_string()),
    );

    assert_eq!(line.text(), "Generated Claude Code title");
    assert!(matches!(
        line,
        TerminalPrimaryLineData::Text {
            font: TerminalPrimaryLineFont::Ui,
            ..
        }
    ));
}

#[test]
fn preferred_agent_tab_titles_use_latest_prompt_when_enabled() {
    let agent_text = TerminalAgentText {
        conversation_display_title: Some("Generated Oz title".to_string()),
        conversation_latest_user_prompt: Some("Latest Oz prompt".to_string()),
        cli_agent_title: Some("CLI summary".to_string()),
        cli_agent_latest_user_prompt: Some("Latest CLI prompt".to_string()),
        is_oz_agent: true,
        cli_agent: Some(CLIAgent::Claude),
    };

    assert_eq!(
        preferred_agent_tab_titles(&agent_text, AgentTabTextPreference::LatestUserPrompt),
        (
            Some("Latest Oz prompt".to_string()),
            Some("Latest CLI prompt".to_string())
        )
    );
}

#[test]
fn terminal_primary_line_uses_cli_prompt_when_enabled_cli_has_prompt() {
    let agent_text = TerminalAgentText {
        conversation_display_title: None,
        conversation_latest_user_prompt: None,
        cli_agent_title: None,
        cli_agent_latest_user_prompt: Some("Latest CLI prompt".to_string()),
        is_oz_agent: false,
        cli_agent: Some(CLIAgent::Claude),
    };
    let (conversation_title, cli_title) =
        preferred_agent_tab_titles(&agent_text, AgentTabTextPreference::LatestUserPrompt);

    let line = terminal_primary_line_data(
        false,
        conversation_title,
        cli_title,
        "Generated Claude Code title",
        "~/warp",
        terminal_title_fallback_font(&agent_text),
        Some("claude".to_string()),
    );

    assert_eq!(line.text(), "Latest CLI prompt");
}

#[test]
fn terminal_primary_line_uses_cli_prompt_when_enabled_cli_is_long_running() {
    let agent_text = TerminalAgentText {
        conversation_display_title: None,
        conversation_latest_user_prompt: None,
        cli_agent_title: None,
        cli_agent_latest_user_prompt: Some("Latest CLI prompt".to_string()),
        is_oz_agent: false,
        cli_agent: Some(CLIAgent::Claude),
    };
    let (conversation_title, cli_title) =
        preferred_agent_tab_titles(&agent_text, AgentTabTextPreference::LatestUserPrompt);

    let line = terminal_primary_line_data(
        true,
        conversation_title,
        cli_title,
        "Generated Claude Code title",
        "~/warp",
        terminal_title_fallback_font(&agent_text),
        Some("claude".to_string()),
    );

    assert_eq!(line.text(), "Latest CLI prompt");
}

#[test]
fn preferred_agent_tab_titles_fall_back_when_preferred_text_is_missing() {
    let agent_text = TerminalAgentText {
        conversation_display_title: Some("Generated Oz title".to_string()),
        conversation_latest_user_prompt: None,
        cli_agent_title: None,
        cli_agent_latest_user_prompt: Some("Latest CLI prompt".to_string()),
        is_oz_agent: true,
        cli_agent: Some(CLIAgent::Claude),
    };

    assert_eq!(
        preferred_agent_tab_titles(&agent_text, AgentTabTextPreference::LatestUserPrompt),
        (
            Some("Generated Oz title".to_string()),
            Some("Latest CLI prompt".to_string())
        )
    );
}

fn pane_type_supports_vertical_tabs_detail_sidecar(pane_type: IPaneType) -> bool {
    matches!(
        pane_type,
        IPaneType::Terminal
            | IPaneType::Code
            | IPaneType::Notebook
            | IPaneType::Workflow
            | IPaneType::EnvVarCollection
            | IPaneType::AIFact
            | IPaneType::AIDocument
    )
}

fn collect_normalized_unique_summary_texts(
    texts: impl IntoIterator<Item = impl AsRef<str>>,
) -> Vec<String> {
    texts
        .into_iter()
        .filter_map(|text| {
            let normalized = text
                .as_ref()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            (!normalized.is_empty()).then_some(normalized)
        })
        .fold(Vec::new(), |mut values, normalized| {
            if !values.contains(&normalized) {
                values.push(normalized);
            }
            values
        })
}

#[test]
fn detail_sidecar_supports_terminal_code_and_warp_drive_object_panes() {
    assert!(pane_type_supports_vertical_tabs_detail_sidecar(
        IPaneType::Terminal
    ));
    assert!(pane_type_supports_vertical_tabs_detail_sidecar(
        IPaneType::Code
    ));
    assert!(pane_type_supports_vertical_tabs_detail_sidecar(
        IPaneType::Notebook
    ));
    assert!(pane_type_supports_vertical_tabs_detail_sidecar(
        IPaneType::Workflow
    ));
    assert!(pane_type_supports_vertical_tabs_detail_sidecar(
        IPaneType::EnvVarCollection
    ));
    assert!(pane_type_supports_vertical_tabs_detail_sidecar(
        IPaneType::AIFact
    ));
    assert!(pane_type_supports_vertical_tabs_detail_sidecar(
        IPaneType::AIDocument
    ));
    assert!(!pane_type_supports_vertical_tabs_detail_sidecar(
        IPaneType::Settings
    ));
}

#[test]
fn code_detail_kind_label_uses_programming_language_display_name() {
    assert_eq!(
        code_detail_kind_label("block_id.rs"),
        Some("Rust".to_string())
    );
    assert_eq!(
        code_detail_kind_label("Dockerfile"),
        Some("Dockerfile".to_string())
    );
}

#[test]
fn code_detail_kind_label_returns_none_when_language_is_unknown() {
    assert_eq!(code_detail_kind_label("notes.txt"), None);
}

#[test]
fn detail_target_matches_panes_granularity() {
    let pane_group_id = EntityId::new();
    let hovered_pane_id = pane_id();

    assert_eq!(
        detail_target_for_hovered_row(
            pane_group_id,
            hovered_pane_id,
            VerticalTabsDisplayGranularity::Panes,
        ),
        VerticalTabsDetailTarget::Pane {
            pane_group_id,
            pane_id: hovered_pane_id,
        }
    );
}

#[test]
fn detail_target_matches_tabs_granularity() {
    let pane_group_id = EntityId::new();
    let hovered_pane_id = pane_id();

    assert_eq!(
        detail_target_for_hovered_row(
            pane_group_id,
            hovered_pane_id,
            VerticalTabsDisplayGranularity::Tabs,
        ),
        VerticalTabsDetailTarget::Tab {
            pane_group_id,
            source_pane_id: hovered_pane_id,
        }
    );
}

#[test]
fn pane_detail_target_returns_hovered_pane_when_supported() {
    let hovered_pane_id = pane_id();

    assert_eq!(
        visible_pane_ids_for_detail_target(
            &[hovered_pane_id],
            hovered_pane_id,
            VerticalTabsDetailTargetKind::Pane,
            |pane_id| pane_id == hovered_pane_id,
        ),
        Some(vec![hovered_pane_id])
    );
}

#[test]
fn pane_detail_target_returns_none_when_hovered_pane_is_not_supported() {
    let hovered_pane_id = pane_id();

    assert_eq!(
        visible_pane_ids_for_detail_target(
            &[hovered_pane_id],
            hovered_pane_id,
            VerticalTabsDetailTargetKind::Pane,
            |_| false,
        ),
        None
    );
}

#[test]
fn tab_detail_target_returns_all_visible_panes_when_every_pane_is_supported() {
    let pane_1 = pane_id();
    let pane_2 = pane_id();
    let pane_3 = pane_id();
    let visible_pane_ids = vec![pane_1, pane_2, pane_3];

    assert_eq!(
        visible_pane_ids_for_detail_target(
            &visible_pane_ids,
            pane_2,
            VerticalTabsDetailTargetKind::Tab,
            |_| true,
        ),
        Some(visible_pane_ids)
    );
}

#[test]
fn tab_detail_target_returns_none_for_mixed_support_tabs() {
    let pane_1 = pane_id();
    let pane_2 = pane_id();
    let pane_3 = pane_id();

    assert_eq!(
        visible_pane_ids_for_detail_target(
            &[pane_1, pane_2, pane_3],
            pane_2,
            VerticalTabsDetailTargetKind::Tab,
            |pane_id| pane_id != pane_3,
        ),
        None
    );
}

#[test]
fn panes_granularity_returns_all_visible_panes_in_order() {
    let pane_1 = pane_id();
    let pane_2 = pane_id();
    let pane_3 = pane_id();
    let visible_pane_ids = vec![pane_1, pane_2, pane_3];

    assert_eq!(
        pane_ids_for_display_granularity(
            &visible_pane_ids,
            pane_2,
            VerticalTabsDisplayGranularity::Panes,
        ),
        visible_pane_ids
    );
}

#[test]
fn tabs_granularity_returns_focused_pane_when_present() {
    let pane_1 = pane_id();
    let pane_2 = pane_id();
    let pane_3 = pane_id();

    assert_eq!(
        pane_ids_for_display_granularity(
            &[pane_1, pane_2, pane_3],
            pane_2,
            VerticalTabsDisplayGranularity::Tabs,
        ),
        vec![pane_2]
    );
}

#[test]
fn tabs_granularity_falls_back_to_first_visible_pane_when_focused_pane_is_absent() {
    let pane_1 = pane_id();
    let pane_2 = pane_id();
    let pane_3 = pane_id();
    let focused_pane = pane_id();

    assert_eq!(
        pane_ids_for_display_granularity(
            &[pane_1, pane_2, pane_3],
            focused_pane,
            VerticalTabsDisplayGranularity::Tabs,
        ),
        vec![pane_1]
    );
}

#[test]
fn tabs_granularity_returns_empty_for_empty_visible_panes() {
    assert_eq!(
        pane_ids_for_display_granularity(&[], pane_id(), VerticalTabsDisplayGranularity::Tabs,),
        Vec::<PaneId>::new()
    );
}

#[test]
fn detail_sidecar_uses_default_width_when_space_allows() {
    let (width, bounds) = detail_sidecar_width_and_bounds(400.);
    assert_eq!(width, 320.);
    assert!(matches!(
        bounds,
        PositionedElementOffsetBounds::WindowBySize
    ));
}

#[test]
fn detail_sidecar_shrinks_to_fit_before_hitting_min_width() {
    let (width, bounds) = detail_sidecar_width_and_bounds(280.);
    assert_eq!(width, 280.);
    assert!(matches!(
        bounds,
        PositionedElementOffsetBounds::WindowBySize
    ));
}

#[test]
fn detail_sidecar_stops_shrinking_at_min_width_and_allows_clipping() {
    let (width, bounds) = detail_sidecar_width_and_bounds(180.);
    assert_eq!(width, 240.);
    assert!(matches!(bounds, PositionedElementOffsetBounds::Unbounded));
}

#[test]
fn detail_sidecar_visibility_helper_keeps_sidecar_visible_inside_sidecar_bounds() {
    let row_rect = RectF::new(Vector2F::new(0., 100.), Vector2F::new(100., 40.));
    let sidecar_rect = RectF::new(Vector2F::new(120., 50.), Vector2F::new(180., 220.));
    let mut safe_triangle = SafeTriangle::new();

    assert!(should_keep_detail_sidecar_visible_for_mouse_position(
        Vector2F::new(200., 120.),
        Some(row_rect),
        Some(sidecar_rect),
        &mut safe_triangle,
    ));
}

#[test]
fn detail_sidecar_visibility_helper_keeps_sidecar_visible_in_safe_triangle() {
    let row_rect = RectF::new(Vector2F::new(0., 100.), Vector2F::new(100., 40.));
    let sidecar_rect = RectF::new(Vector2F::new(120., 50.), Vector2F::new(180., 220.));
    let mut safe_triangle = SafeTriangle::new();
    safe_triangle.set_target_rect(Some(sidecar_rect));
    safe_triangle.update_position(Vector2F::new(90., 120.));

    assert!(should_keep_detail_sidecar_visible_for_mouse_position(
        Vector2F::new(110., 120.),
        Some(row_rect),
        Some(sidecar_rect),
        &mut safe_triangle,
    ));
}

#[test]
fn detail_sidecar_visibility_helper_clears_sidecar_outside_row_sidecar_and_safe_triangle() {
    let row_rect = RectF::new(Vector2F::new(0., 100.), Vector2F::new(100., 40.));
    let sidecar_rect = RectF::new(Vector2F::new(120., 50.), Vector2F::new(180., 220.));
    let mut safe_triangle = SafeTriangle::new();
    safe_triangle.update_position(Vector2F::new(200., 120.));

    assert!(!should_keep_detail_sidecar_visible_for_mouse_position(
        Vector2F::new(340., 120.),
        Some(row_rect),
        Some(sidecar_rect),
        &mut safe_triangle,
    ));
}

#[test]
fn panes_granularity_uses_outer_group_container() {
    assert!(uses_outer_group_container(
        VerticalTabsDisplayGranularity::Panes
    ));
}

#[test]
fn tabs_granularity_does_not_use_outer_group_container() {
    assert!(!uses_outer_group_container(
        VerticalTabsDisplayGranularity::Tabs
    ));
}

#[test]
fn terminal_primary_line_prefers_cli_agent_display_title() {
    let line = terminal_primary_line_data(
        false,
        None,
        Some("Review the failing tests".to_string()),
        "~/warp",
        "~/warp",
        TerminalPrimaryLineFont::Monospace,
        Some("cargo nextest run".to_string()),
    );

    assert_eq!(line.text(), "Review the failing tests");
}

#[test]
fn terminal_primary_line_prefers_cli_agent_display_title_over_conversation_title() {
    let line = terminal_primary_line_data(
        false,
        Some("Review the failing tests".to_string()),
        Some("Summarize the failures".to_string()),
        "~/warp",
        "~/warp",
        TerminalPrimaryLineFont::Monospace,
        Some("cargo nextest run".to_string()),
    );

    assert_eq!(line.text(), "Summarize the failures");
}

#[test]
fn terminal_primary_line_falls_through_to_terminal_title_when_cli_agent_has_no_plugin_data() {
    let line = terminal_primary_line_data(
        false,
        None,
        None,
        "codex - ~/warp",
        "~/warp",
        TerminalPrimaryLineFont::Monospace,
        Some("cargo nextest run".to_string()),
    );

    assert_eq!(line.text(), "codex - ~/warp");
}

#[test]
fn terminal_primary_line_uses_terminal_title_as_fallback() {
    let line = terminal_primary_line_data(
        false,
        None,
        None,
        "nvim src/workspace/view/vertical_tabs.rs",
        "~/warp",
        TerminalPrimaryLineFont::Monospace,
        Some("cargo nextest run".to_string()),
    );

    assert_eq!(line.text(), "nvim src/workspace/view/vertical_tabs.rs");
}

#[test]
fn terminal_primary_line_uses_last_completed_command_when_shell_title_matches_working_directory() {
    let line = terminal_primary_line_data(
        false,
        None,
        None,
        "~/warp",
        "~/warp",
        TerminalPrimaryLineFont::Monospace,
        Some("cargo nextest run".to_string()),
    );

    assert_eq!(line.text(), "cargo nextest run");
}

#[test]
fn terminal_primary_line_falls_back_to_new_session() {
    let line = terminal_primary_line_data(
        false,
        None,
        None,
        "~/warp",
        "~/warp",
        TerminalPrimaryLineFont::Monospace,
        None,
    );

    assert_eq!(line.text(), "New session");
    assert!(matches!(
        line,
        TerminalPrimaryLineData::Text {
            font: TerminalPrimaryLineFont::Ui,
            ..
        }
    ));
}

#[test]
fn terminal_primary_line_uses_monospace_for_last_completed_command() {
    let line = terminal_primary_line_data(
        false,
        None,
        None,
        "~/warp",
        "~/warp",
        TerminalPrimaryLineFont::Monospace,
        Some("cargo nextest run".to_string()),
    );

    assert!(matches!(
        line,
        TerminalPrimaryLineData::Text {
            font: TerminalPrimaryLineFont::Monospace,
            ..
        }
    ));
}

#[test]
fn terminal_search_fragments_include_rendered_terminal_badges() {
    let fragments = terminal_search_text_fragments(
        "Review the failing tests".to_string(),
        "~/warp".to_string(),
        Some("main".to_string()),
        terminal_kind_badge_label(false, Some(CLIAgent::Claude)),
        Some(terminal_pull_request_badge_label(
            "https://github.com/warpdotdev/warp-internal/pull/12345",
        )),
        Some(GitLineChanges {
            files_changed: 1,
            lines_added: 2,
            lines_removed: 3,
        }),
    );

    assert!(search_fragments_contain_query(&fragments, "claude"));
    assert!(search_fragments_contain_query(
        &fragments,
        "review the failing tests"
    ));
    assert!(search_fragments_contain_query(&fragments, "#12345"));
    assert!(search_fragments_contain_query(&fragments, "+2"));
    assert!(search_fragments_contain_query(&fragments, "-3"));
}

#[test]
fn pane_search_fragments_prepend_custom_title_and_keep_generated_metadata() {
    let fragments = pane_search_text_fragments(
        Some("Production API"),
        vec![
            "cargo nextest run".to_string(),
            "~/warp".to_string(),
            "Claude".to_string(),
        ],
    );

    assert_eq!(fragments[0], "Production API");
    assert!(search_fragments_contain_query(&fragments, "production api"));
    assert!(search_fragments_contain_query(&fragments, "cargo nextest"));
    assert!(search_fragments_contain_query(&fragments, "~/warp"));
    assert!(search_fragments_contain_query(&fragments, "claude"));
}

#[test]
fn pane_search_fragments_dedupe_custom_title_against_generated_text() {
    assert_eq!(
        pane_search_text_fragments(
            Some("  Production   API  "),
            vec![
                "Production API".to_string(),
                "~/warp".to_string(),
                "~/warp".to_string(),
            ],
        ),
        vec!["Production API".to_string(), "~/warp".to_string()]
    );
}

#[test]
fn non_terminal_search_fragments_only_include_rendered_text() {
    let fragments = non_terminal_search_text_fragments("Pane title", "and 2 more");

    assert!(search_fragments_contain_query(&fragments, "pane title"));
    assert!(search_fragments_contain_query(&fragments, "and 2 more"));
    assert!(!search_fragments_contain_query(&fragments, "notebook"));
    assert!(!search_fragments_contain_query(&fragments, "unsaved"));
}

#[test]
fn diff_stats_text_matches_rendered_badge_text() {
    assert_eq!(
        vtab_diff_stats_text(&GitLineChanges {
            files_changed: 1,
            lines_added: 2,
            lines_removed: 3,
        }),
        "+2 -3"
    );
    assert_eq!(
        vtab_diff_stats_text(&GitLineChanges {
            files_changed: 1,
            lines_added: 0,
            lines_removed: 0,
        }),
        "0"
    );
}

#[test]
fn branch_label_display_falls_back_without_branch_icon() {
    assert_eq!(
        branch_label_display(None, "~/warp"),
        ("~/warp".to_string(), false)
    );
    assert_eq!(
        branch_label_display(Some(""), "~/warp"),
        ("~/warp".to_string(), false)
    );
    assert_eq!(
        branch_label_display(Some("main"), "~/warp"),
        ("main".to_string(), true)
    );
}

#[test]
fn compact_branch_subtitle_falls_back_to_working_directory_without_branch_icon() {
    assert_eq!(
        compact_branch_subtitle_display(None, Some("~/warp")),
        Some(("~/warp".to_string(), false))
    );
    assert_eq!(
        compact_branch_subtitle_display(Some(""), Some("~/warp")),
        Some(("~/warp".to_string(), false))
    );
    assert_eq!(
        compact_branch_subtitle_display(Some("main"), Some("~/warp")),
        Some(("main".to_string(), true))
    );
}

#[test]
fn collect_normalized_unique_summary_texts_dedupes_after_whitespace_normalization() {
    assert_eq!(
        collect_normalized_unique_summary_texts([
            "  cargo   test  ",
            "cargo test",
            "",
            " git   status ",
        ]),
        vec!["cargo test".to_string(), "git status".to_string()]
    );
}

#[test]
fn collect_normalized_unique_summary_texts_preserves_first_seen_order() {
    assert_eq!(
        collect_normalized_unique_summary_texts([
            "~/warp-internal",
            "~/warp-server",
            "~/warp-internal",
            "~/warp-terraform",
        ]),
        vec![
            "~/warp-internal".to_string(),
            "~/warp-server".to_string(),
            "~/warp-terraform".to_string(),
        ]
    );
}

#[test]
fn coalesce_summary_branch_entries_groups_by_repo_and_branch() {
    let repo_a = PathBuf::from("/tmp/repo-a");
    let repo_b = PathBuf::from("/tmp/repo-b");
    let entries = vec![
        VerticalTabsSummaryBranchEntry {
            repo_path: repo_a.clone(),
            branch_name: "main".to_string(),
            diff_stats: None,
            pull_request_label: None,
        },
        VerticalTabsSummaryBranchEntry {
            repo_path: repo_a.clone(),
            branch_name: "main".to_string(),
            diff_stats: Some(GitLineChanges {
                files_changed: 1,
                lines_added: 2,
                lines_removed: 3,
            }),
            pull_request_label: Some("#123".to_string()),
        },
        VerticalTabsSummaryBranchEntry {
            repo_path: repo_b.clone(),
            branch_name: "main".to_string(),
            diff_stats: Some(GitLineChanges {
                files_changed: 4,
                lines_added: 5,
                lines_removed: 6,
            }),
            pull_request_label: Some("#456".to_string()),
        },
    ];

    assert_eq!(
        coalesce_summary_branch_entries(entries),
        vec![
            VerticalTabsSummaryBranchEntry {
                repo_path: repo_a,
                branch_name: "main".to_string(),
                diff_stats: Some(GitLineChanges {
                    files_changed: 1,
                    lines_added: 2,
                    lines_removed: 3,
                }),
                pull_request_label: Some("#123".to_string()),
            },
            VerticalTabsSummaryBranchEntry {
                repo_path: repo_b,
                branch_name: "main".to_string(),
                diff_stats: Some(GitLineChanges {
                    files_changed: 4,
                    lines_added: 5,
                    lines_removed: 6,
                }),
                pull_request_label: Some("#456".to_string()),
            },
        ]
    );
}

#[test]
fn format_summary_primary_labels_appends_overflow_count() {
    let labels = vec![
        "Claude".to_string(),
        "Oz".to_string(),
        "cargo".to_string(),
        "code review".to_string(),
        "tests".to_string(),
    ];

    assert_eq!(
        format_summary_primary_labels(&labels, 4),
        Some("Claude • Oz • cargo • code review + 1 more".to_string())
    );
    assert_eq!(summary_overflow_count(labels.len(), 4), 1);
}

#[test]
fn summary_search_fragments_include_hidden_overflow_values() {
    let summary = VerticalTabsSummaryData {
        primary_labels: vec![
            "Claude".to_string(),
            "Oz".to_string(),
            "cargo".to_string(),
            "code review".to_string(),
            "hidden work".to_string(),
        ],
        working_directories: vec!["~/warp-internal".to_string(), "~/warp-server".to_string()],
        branch_entries: vec![
            VerticalTabsSummaryBranchEntry {
                repo_path: PathBuf::from("/tmp/repo-a"),
                branch_name: "main".to_string(),
                diff_stats: Some(GitLineChanges {
                    files_changed: 1,
                    lines_added: 2,
                    lines_removed: 3,
                }),
                pull_request_label: Some("#123".to_string()),
            },
            VerticalTabsSummaryBranchEntry {
                repo_path: PathBuf::from("/tmp/repo-b"),
                branch_name: "feature/hidden".to_string(),
                diff_stats: None,
                pull_request_label: None,
            },
            VerticalTabsSummaryBranchEntry {
                repo_path: PathBuf::from("/tmp/repo-c"),
                branch_name: "cleanup".to_string(),
                diff_stats: None,
                pull_request_label: None,
            },
            VerticalTabsSummaryBranchEntry {
                repo_path: PathBuf::from("/tmp/repo-d"),
                branch_name: "hidden-branch".to_string(),
                diff_stats: None,
                pull_request_label: Some("#789".to_string()),
            },
        ],
    };

    let fragments = summary_search_text_fragments(&summary, Some("Custom tab"));

    assert!(search_fragments_contain_query(&fragments, "custom tab"));
    assert!(search_fragments_contain_query(&fragments, "hidden work"));
    assert!(search_fragments_contain_query(&fragments, "hidden-branch"));
    assert!(search_fragments_contain_query(&fragments, "#789"));
    assert!(search_fragments_contain_query(&fragments, "+2"));
    assert!(search_fragments_contain_query(&fragments, "-3"));
}
