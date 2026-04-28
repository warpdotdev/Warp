use super::*;
use crate::ai::agent::task::TaskId;
use crate::ai::agent::AIAgentActionResultType;
use crate::ai::agent::ReadSkillRequest;
use crate::ai::agent::ReadSkillResult;
use crate::ai::agent::{AIAgentAction, AIAgentActionId, AIAgentActionType};
use crate::ai::blocklist::action_model::AIConversationId;
use crate::ai::skills::SkillManager;
use crate::warp_managed_paths_watcher::WarpManagedPathsWatcher;
use ai::skills::{parse_skill, SkillReference};
use repo_metadata::{
    repositories::DetectedRepositories, watcher::DirectoryWatcher, RepoMetadataModel,
};
use std::fs;
use std::io::Write;
use tempfile::TempDir;
use warpui::App;
use watcher::HomeDirectoryWatcher;

fn initialize_app(app: &mut App) {
    app.add_singleton_model(DirectoryWatcher::new);
    app.add_singleton_model(|_| DetectedRepositories::default());
    app.add_singleton_model(RepoMetadataModel::new);
    app.add_singleton_model(HomeDirectoryWatcher::new_for_test);
    app.add_singleton_model(WarpManagedPathsWatcher::new_for_testing);
    app.add_singleton_model(SkillManager::new);
}

fn create_test_skill_file(dir: &TempDir, name: &str, description: &str) -> std::path::PathBuf {
    let skill_content = format!(
        r#"---
name: {}
description: {}
---

# {}

## Instructions
Test instructions for this skill.

## Examples
Example usage of the skill.
"#,
        name, description, name
    );

    let skill_dir = dir.path().join(format!(".claude/skills/{}", name));
    fs::create_dir_all(&skill_dir).unwrap();
    let skill_path = skill_dir.join("SKILL.md");
    let mut file = fs::File::create(&skill_path).unwrap();
    file.write_all(skill_content.as_bytes()).unwrap();
    file.flush().unwrap();

    skill_path
}

#[test]
fn test_read_skill_executor_success() {
    let temp_dir = TempDir::new().unwrap();
    let skill_path = create_test_skill_file(&temp_dir, "test-skill", "A test skill");

    App::test((), |mut app| async move {
        initialize_app(&mut app);

        // Populate SkillManager cache with the test skill
        let parsed_skill = parse_skill(&skill_path).expect("Failed to parse test skill");
        SkillManager::handle(&app).update(&mut app, |manager, _ctx| {
            manager.add_skill_for_testing(parsed_skill);
        });

        let executor_handle = app.add_model(|_| ReadSkillExecutor::new());

        let action = AIAgentAction {
            id: AIAgentActionId::from("test-action-id".to_string()),
            action: AIAgentActionType::ReadSkill(ReadSkillRequest {
                skill: SkillReference::Path(skill_path.clone()),
            }),
            task_id: TaskId::new("test-task-id".to_string()),
            requires_result: false,
        };

        let input = ExecuteActionInput {
            action: &action,
            conversation_id: AIConversationId::new(),
        };

        executor_handle.update(&mut app, |executor, ctx| {
            let result: AnyActionExecution = executor.execute(input, ctx).into();

            match result {
                AnyActionExecution::Sync(AIAgentActionResultType::ReadSkill(
                    ReadSkillResult::Success { content },
                )) => {
                    assert_eq!(content.file_name, skill_path.to_string_lossy().to_string());
                }
                _ => panic!("Successfully read skill file; should return ReadSkillResult::Success"),
            }
        });
    });
}

#[test]
fn test_read_skill_executor_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    // Don't create the SKILL.md file
    let skill_path = temp_dir.path().join("SKILL.md");

    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let executor_handle = app.add_model(|_| ReadSkillExecutor::new());

        let action = AIAgentAction {
            id: AIAgentActionId::from("test-action-id".to_string()),
            action: AIAgentActionType::ReadSkill(ReadSkillRequest {
                skill: SkillReference::Path(skill_path),
            }),
            task_id: TaskId::new("test-task-id".to_string()),
            requires_result: false,
        };

        let input = ExecuteActionInput {
            action: &action,
            conversation_id: AIConversationId::new(),
        };

        executor_handle.update(&mut app, |executor, ctx| {
            let result: AnyActionExecution = executor.execute(input, ctx).into();

            match result {
                AnyActionExecution::Sync(AIAgentActionResultType::ReadSkill(
                    ReadSkillResult::Error(error_msg),
                )) => {
                    // Should contain an error about file not found or I/O error
                    assert!(!error_msg.is_empty());
                }
                _ => panic!(
                    "Nonexistent SKILL.md file at given path; should return ReadSkillResult::Error"
                ),
            }
        });
    });
}
