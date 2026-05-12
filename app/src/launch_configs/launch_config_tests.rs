use std::path::PathBuf;

use crate::{
    app_state::{
        AppState, BranchSnapshot, LeafContents, LeafSnapshot, NotebookPaneSnapshot, PaneFlex,
        PaneNodeSnapshot, SplitDirection, TabSnapshot, TerminalPaneSnapshot, WindowSnapshot,
    },
    drive::OpenWarpDriveObjectSettings,
    tab::SelectedTabColor,
};

use super::{CommandExecutionMode, LaunchConfig, PaneMode, PaneTemplateType};

fn single_tab_snapshot(root: PaneNodeSnapshot) -> AppState {
    AppState {
        windows: vec![WindowSnapshot {
            tabs: vec![TabSnapshot {
                custom_title: None,
                default_directory_color: None,
                selected_color: SelectedTabColor::default(),
                root,
                left_panel: None,
                right_panel: None,
            }],
            active_tab_index: 0,
            bounds: None,
            quake_mode: false,
            universal_search_width: None,
            warp_ai_width: None,
            voltron_width: None,
            warp_drive_index_width: None,
            left_panel_open: false,
            vertical_tabs_panel_open: false,
            fullscreen_state: Default::default(),
            left_panel_width: None,
            right_panel_width: None,
            agent_management_filters: None,
        }],
        active_window_index: Some(0),
        block_lists: Default::default(),
        running_mcp_servers: Default::default(),
    }
}

fn multi_tab_snapshot(active_tab_index: usize, tabs: Vec<TabSnapshot>) -> AppState {
    AppState {
        windows: vec![WindowSnapshot {
            tabs,
            active_tab_index,
            bounds: None,
            quake_mode: false,
            universal_search_width: None,
            warp_ai_width: None,
            voltron_width: None,
            warp_drive_index_width: None,
            left_panel_open: false,
            vertical_tabs_panel_open: false,
            fullscreen_state: Default::default(),
            left_panel_width: None,
            right_panel_width: None,
            agent_management_filters: None,
        }],
        active_window_index: Some(0),
        block_lists: Default::default(),
        running_mcp_servers: Default::default(),
    }
}

#[test]
fn test_config_from_snapshot_flattens_single_pane() {
    // If only one pane of the branch can be saved into a launch configuration, it should
    // be flattened to a single leaf.

    let state = single_tab_snapshot(PaneNodeSnapshot::Branch(BranchSnapshot {
        direction: SplitDirection::Vertical,
        children: vec![
            (
                PaneFlex(1.),
                PaneNodeSnapshot::Leaf(LeafSnapshot {
                    is_focused: true,
                    custom_vertical_tabs_title: None,
                    contents: LeafContents::Notebook(NotebookPaneSnapshot::CloudNotebook {
                        notebook_id: None,
                        settings: OpenWarpDriveObjectSettings::default(),
                    }),
                }),
            ),
            (
                PaneFlex(1.),
                PaneNodeSnapshot::Leaf(LeafSnapshot {
                    is_focused: true,
                    custom_vertical_tabs_title: None,
                    contents: LeafContents::Terminal(TerminalPaneSnapshot {
                        uuid: vec![],
                        cwd: Some("/some/dir".into()),
                        is_active: true,
                        is_read_only: false,
                        shell_launch_data: None,
                        input_config: None,
                        llm_model_override: None,
                        active_profile_id: None,
                        conversation_ids_to_restore: vec![],
                        active_conversation_id: None,
                    }),
                }),
            ),
        ],
    }));

    let template = LaunchConfig::from_snapshot("Test".into(), &state);
    assert_eq!(
        template.windows[0].tabs[0].layout,
        PaneTemplateType::PaneTemplate {
            is_focused: Some(true),
            cwd: PathBuf::from("/some/dir"),
            commands: vec![],
            command_execution_mode: CommandExecutionMode::ChainedWithAnd,
            pane_mode: PaneMode::Terminal,
            shell: None,
        },
    )
}

#[test]
fn test_config_from_snapshot_filters_panes() {
    let state = single_tab_snapshot(PaneNodeSnapshot::Branch(BranchSnapshot {
        direction: SplitDirection::Vertical,
        children: vec![
            (
                PaneFlex(1.),
                PaneNodeSnapshot::Leaf(LeafSnapshot {
                    is_focused: true,
                    custom_vertical_tabs_title: None,
                    contents: LeafContents::Terminal(TerminalPaneSnapshot {
                        uuid: vec![],
                        cwd: Some("/path/to/dir".into()),
                        is_active: true,
                        is_read_only: false,
                        shell_launch_data: None,
                        input_config: None,
                        llm_model_override: None,
                        active_profile_id: None,
                        conversation_ids_to_restore: vec![],
                        active_conversation_id: None,
                    }),
                }),
            ),
            (
                PaneFlex(1.),
                PaneNodeSnapshot::Leaf(LeafSnapshot {
                    is_focused: false,
                    custom_vertical_tabs_title: None,
                    contents: LeafContents::Notebook(NotebookPaneSnapshot::CloudNotebook {
                        notebook_id: None,
                        settings: OpenWarpDriveObjectSettings::default(),
                    }),
                }),
            ),
            (
                PaneFlex(1.),
                PaneNodeSnapshot::Leaf(LeafSnapshot {
                    is_focused: false,
                    custom_vertical_tabs_title: None,
                    contents: LeafContents::Terminal(TerminalPaneSnapshot {
                        uuid: vec![],
                        cwd: Some("/some/dir".into()),
                        is_active: true,
                        is_read_only: false,
                        shell_launch_data: None,
                        input_config: None,
                        llm_model_override: None,
                        active_profile_id: None,
                        conversation_ids_to_restore: vec![],
                        active_conversation_id: None,
                    }),
                }),
            ),
        ],
    }));

    let template = LaunchConfig::from_snapshot("Test".into(), &state);
    assert_eq!(
        template.windows[0].tabs[0].layout,
        PaneTemplateType::PaneBranchTemplate {
            split_direction: SplitDirection::Vertical.into(),
            panes: vec![
                PaneTemplateType::PaneTemplate {
                    is_focused: Some(true),
                    cwd: PathBuf::from("/path/to/dir"),
                    commands: vec![],
                    command_execution_mode: CommandExecutionMode::ChainedWithAnd,
                    pane_mode: PaneMode::Terminal,
                    shell: None,
                },
                PaneTemplateType::PaneTemplate {
                    is_focused: Some(false),
                    cwd: PathBuf::from("/some/dir"),
                    commands: vec![],
                    command_execution_mode: CommandExecutionMode::ChainedWithAnd,
                    pane_mode: PaneMode::Terminal,
                    shell: None,
                },
            ]
        }
    )
}

#[test]
fn test_config_from_snapshot_filters_tabs() {
    // If no panes of a tab are valid, it's filtered out entirely.

    let state = single_tab_snapshot(PaneNodeSnapshot::Branch(BranchSnapshot {
        direction: SplitDirection::Vertical,
        children: vec![(
            PaneFlex(1.),
            PaneNodeSnapshot::Leaf(LeafSnapshot {
                is_focused: true,
                custom_vertical_tabs_title: None,
                contents: LeafContents::Notebook(NotebookPaneSnapshot::CloudNotebook {
                    notebook_id: None,
                    settings: OpenWarpDriveObjectSettings::default(),
                }),
            }),
        )],
    }));

    let template = LaunchConfig::from_snapshot("Test".into(), &state);
    assert!(template.windows[0].tabs.is_empty())
}
#[test]
fn test_pane_template_command_execution_mode_defaults_to_chained_with_and() {
    let config = r#"
name = "Launch Config"
windows = [
  { tabs = [
    { layout = { cwd = "/tmp", commands = [{ exec = "echo one" }, { exec = "echo two" }] } }
  ] }
]
"#;

    let config: LaunchConfig = toml::from_str(config).expect("Should parse launch config");
    let PaneTemplateType::PaneTemplate {
        command_execution_mode,
        ..
    } = config.windows[0].tabs[0].layout
    else {
        panic!("Expected PaneTemplate");
    };

    assert_eq!(command_execution_mode, CommandExecutionMode::ChainedWithAnd);
}

#[test]
fn test_default_command_execution_mode_is_not_serialized() {
    let launch_config = LaunchConfig {
        name: "Launch Config".to_string(),
        active_window_index: None,
        windows: vec![super::WindowTemplate {
            active_tab_index: None,
            tabs: vec![super::TabTemplate {
                title: None,
                layout: PaneTemplateType::PaneTemplate {
                    cwd: PathBuf::from("/tmp"),
                    commands: vec!["echo one".into(), "echo two".into()],
                    command_execution_mode: CommandExecutionMode::ChainedWithAnd,
                    is_focused: None,
                    pane_mode: PaneMode::Terminal,
                    shell: None,
                },
                color: None,
            }],
        }],
    };

    let serialized = toml::to_string(&launch_config).expect("Should serialize launch config");

    assert!(!serialized.contains("command_execution_mode"));
}

#[test]
fn test_config_with_active_tab_index() {
    let state = multi_tab_snapshot(
        1,
        vec![
            TabSnapshot {
                custom_title: None,
                default_directory_color: None,
                selected_color: SelectedTabColor::default(),
                root: PaneNodeSnapshot::Branch(BranchSnapshot {
                    direction: SplitDirection::Vertical,
                    children: vec![(
                        PaneFlex(1.),
                        PaneNodeSnapshot::Leaf(LeafSnapshot {
                            is_focused: true,
                            custom_vertical_tabs_title: None,
                            contents: LeafContents::Terminal(TerminalPaneSnapshot {
                                uuid: vec![],
                                cwd: Some("/path/to/dir".into()),
                                is_active: true,
                                is_read_only: false,
                                shell_launch_data: None,
                                input_config: None,
                                llm_model_override: None,
                                active_profile_id: None,
                                conversation_ids_to_restore: vec![],
                                active_conversation_id: None,
                            }),
                        }),
                    )],
                }),
                left_panel: None,
                right_panel: None
            };
            3
        ],
    );

    let template = LaunchConfig::from_snapshot("Test".into(), &state);
    assert_eq!(template.windows[0].active_tab_index, Some(1))
}

#[test]
fn test_config_with_active_tab_index_and_filtered_tabs() {
    let state = multi_tab_snapshot(
        1,
        vec![
            TabSnapshot {
                custom_title: None,
                default_directory_color: None,
                selected_color: SelectedTabColor::default(),
                root: PaneNodeSnapshot::Branch(BranchSnapshot {
                    direction: SplitDirection::Vertical,
                    children: vec![(
                        PaneFlex(1.),
                        PaneNodeSnapshot::Leaf(LeafSnapshot {
                            is_focused: true,
                            custom_vertical_tabs_title: None,
                            contents: LeafContents::Notebook(NotebookPaneSnapshot::CloudNotebook {
                                notebook_id: None,
                                settings: OpenWarpDriveObjectSettings::default(),
                            }),
                        }),
                    )],
                }),
                left_panel: None,
                right_panel: None,
            },
            TabSnapshot {
                custom_title: None,
                default_directory_color: None,
                selected_color: SelectedTabColor::default(),
                root: PaneNodeSnapshot::Branch(BranchSnapshot {
                    direction: SplitDirection::Vertical,
                    children: vec![(
                        PaneFlex(1.),
                        PaneNodeSnapshot::Leaf(LeafSnapshot {
                            is_focused: true,
                            custom_vertical_tabs_title: None,
                            contents: LeafContents::Terminal(TerminalPaneSnapshot {
                                uuid: vec![],
                                cwd: Some("/path/to/dir".into()),
                                is_active: true,
                                is_read_only: false,
                                shell_launch_data: None,
                                input_config: None,
                                llm_model_override: None,
                                active_profile_id: None,
                                conversation_ids_to_restore: vec![],
                                active_conversation_id: None,
                            }),
                        }),
                    )],
                }),
                left_panel: None,
                right_panel: None,
            },
        ],
    );

    let template = LaunchConfig::from_snapshot("Test".into(), &state);
    assert_eq!(template.windows[0].active_tab_index, Some(0))
}

#[test]
fn test_config_with_active_tab_being_filtered() {
    let state = multi_tab_snapshot(
        1,
        vec![
            TabSnapshot {
                custom_title: None,
                default_directory_color: None,
                selected_color: SelectedTabColor::default(),
                root: PaneNodeSnapshot::Branch(BranchSnapshot {
                    direction: SplitDirection::Vertical,
                    children: vec![(
                        PaneFlex(1.),
                        PaneNodeSnapshot::Leaf(LeafSnapshot {
                            is_focused: true,
                            custom_vertical_tabs_title: None,
                            contents: LeafContents::Terminal(TerminalPaneSnapshot {
                                uuid: vec![],
                                cwd: Some("/path/to/dir".into()),
                                is_active: true,
                                is_read_only: false,
                                shell_launch_data: None,
                                input_config: None,
                                llm_model_override: None,
                                active_profile_id: None,
                                conversation_ids_to_restore: vec![],
                                active_conversation_id: None,
                            }),
                        }),
                    )],
                }),
                left_panel: None,
                right_panel: None,
            },
            TabSnapshot {
                custom_title: None,
                default_directory_color: None,
                selected_color: SelectedTabColor::default(),
                root: PaneNodeSnapshot::Branch(BranchSnapshot {
                    direction: SplitDirection::Vertical,
                    children: vec![(
                        PaneFlex(1.),
                        PaneNodeSnapshot::Leaf(LeafSnapshot {
                            is_focused: true,
                            custom_vertical_tabs_title: None,
                            contents: LeafContents::Notebook(NotebookPaneSnapshot::CloudNotebook {
                                notebook_id: None,
                                settings: OpenWarpDriveObjectSettings::default(),
                            }),
                        }),
                    )],
                }),
                left_panel: None,
                right_panel: None,
            },
        ],
    );

    let template = LaunchConfig::from_snapshot("Test".into(), &state);
    assert_eq!(template.windows[0].active_tab_index, None)
}
