use std::path::PathBuf;

use ai::skills::{ParsedSkill, SkillProvider, SkillScope};

use crate::ai::{
    agent::{AIAgentInput, InvokeSkillUserQuery},
    blocklist::PersistedAIInputType,
};

fn test_skill(name: &str) -> ParsedSkill {
    ParsedSkill {
        path: PathBuf::from(format!("/tmp/{name}/SKILL.md")),
        name: name.to_owned(),
        description: format!("{name} description"),
        content: format!("# {name}\nSkill instructions."),
        line_range: None,
        provider: SkillProvider::Warp,
        scope: SkillScope::Bundled,
    }
}

#[test]
fn invoke_skill_with_user_query_persists_for_ai_history() {
    let input = AIAgentInput::InvokeSkill {
        context: Default::default(),
        skill: test_skill("update-tab-config"),
        user_query: Some(InvokeSkillUserQuery {
            query: "Update /tmp/tab.toml".to_owned(),
            referenced_attachments: Default::default(),
        }),
    };

    let persisted = PersistedAIInputType::try_from(&input).expect("invoke skill should persist");

    assert_eq!(
        persisted,
        PersistedAIInputType::Query {
            text: "/update-tab-config Update /tmp/tab.toml".to_owned(),
            context: Default::default(),
            referenced_attachments: Default::default(),
        }
    );
}

#[test]
fn invoke_skill_without_user_query_persists_skill_invocation_for_ai_history() {
    let input = AIAgentInput::InvokeSkill {
        context: Default::default(),
        skill: test_skill("tab-configs"),
        user_query: None,
    };

    let persisted = PersistedAIInputType::try_from(&input).expect("invoke skill should persist");

    assert_eq!(
        persisted,
        PersistedAIInputType::Query {
            text: "/tab-configs".to_owned(),
            context: Default::default(),
            referenced_attachments: Default::default(),
        }
    );
}
