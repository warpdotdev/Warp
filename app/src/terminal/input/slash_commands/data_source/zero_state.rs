use itertools::Itertools;
use warp_core::features::FeatureFlag;
use warpui::{Entity, ModelHandle, SingletonEntity};

use crate::ai::skills::SkillManager;
use crate::cloud_object::model::persistence::CloudModel;
use crate::search::data_source::{Query, QueryResult};
use crate::search::mixer::DataSourceRunErrorWrapper;
use crate::search::slash_command_menu::static_commands::commands;
use crate::search::SyncDataSource;
use crate::settings::AISettings;
use crate::terminal::input::slash_commands::{
    AcceptSlashCommandOrSavedPrompt, InlineItem, SlashCommandDataSource,
};

pub struct ZeroStateDataSource {
    slash_command_data_source: ModelHandle<SlashCommandDataSource>,
    /// When true, surface skills (in addition to slash commands) when the query
    /// is empty. Used by the cloud-mode V2 menu, which renders skills in their
    /// own section. The legacy inline menu keeps this disabled.
    include_skills: bool,
    /// When true, surface saved prompts (in addition to slash commands) when
    /// the query is empty. Used by the cloud-mode V2 menu, which renders
    /// prompts in their own section. The legacy inline menu keeps this
    /// disabled.
    include_saved_prompts: bool,
}

impl ZeroStateDataSource {
    pub fn new(slash_command_data_source: &ModelHandle<SlashCommandDataSource>) -> Self {
        Self {
            slash_command_data_source: slash_command_data_source.clone(),
            include_skills: false,
            include_saved_prompts: false,
        }
    }

    /// Constructor for the cloud-mode V2 slash command menu. Surfaces skills
    /// and saved prompts in zero state alongside slash commands so the V2
    /// menu can render all three sections (Commands / Skills / Prompts) before
    /// the user types a query.
    pub fn for_cloud_mode_v2(
        slash_command_data_source: &ModelHandle<SlashCommandDataSource>,
    ) -> Self {
        Self {
            slash_command_data_source: slash_command_data_source.clone(),
            include_skills: true,
            include_saved_prompts: true,
        }
    }
}

impl Entity for ZeroStateDataSource {
    type Event = ();
}

impl SyncDataSource for ZeroStateDataSource {
    type Action = AcceptSlashCommandOrSavedPrompt;

    fn run_query(
        &self,
        query: &Query,
        app: &warpui::AppContext,
    ) -> Result<Vec<QueryResult<Self::Action>>, DataSourceRunErrorWrapper> {
        if !query.text.is_empty() {
            return Ok(vec![]);
        }

        // This is kind of a convoluted way to explicitly order these commands after all others.
        //
        // DataSource implementations must return highest priority items last (results sorted in
        // ascending order of priority).
        //
        // The results construction below basically orders all active commands, sorted
        // alphabetically, except for the commands in this vec, which are explicitly appended
        // to all the other alphabetically sorted commands, in this order.
        let prioritized_commands = vec![
            &*commands::CREATE_ENVIRONMENT,
            &*commands::EDIT,
            &commands::CONVERSATIONS,
            &commands::PROMPTS,
            &*commands::PLAN,
            &commands::AGENT,
        ];

        let mut active_prioritized_commands = vec![];
        let mut results = vec![];

        for (active_command_id, active_command) in self
            .slash_command_data_source
            .as_ref(app)
            .active_commands()
            .sorted_by_key(|(_, command)| std::cmp::Reverse(&command.name))
        {
            if prioritized_commands
                .iter()
                .any(|prioritized_command| prioritized_command.name == active_command.name)
            {
                active_prioritized_commands.push((active_command_id, active_command));
            } else {
                results.push(
                    InlineItem::from_slash_command(active_command_id, active_command, app).into(),
                );
            }
        }

        for prioritized_command in prioritized_commands {
            if let Some((id, command)) = active_prioritized_commands
                .iter()
                .find(|(_, active_command)| active_command.name == prioritized_command.name)
            {
                results.push(InlineItem::from_slash_command(id, command, app).into());
            }
        }

        // Skills are gated by the `ListSkills` feature flag and the global AI
        // setting (matching `SlashCommandDataSource::run_query` for non-empty
        // queries). Items are emitted in name-descending order so the mixer's
        // ascending priority sort lands on alphabetical order.
        if self.include_skills
            && FeatureFlag::ListSkills.is_enabled()
            && AISettings::as_ref(app).is_any_ai_enabled(app)
        {
            let slash_command_data_source = self.slash_command_data_source.as_ref(app);
            let cli_agent_providers = slash_command_data_source.active_cli_agent_providers(app);
            let cwd = slash_command_data_source
                .active_session_for_v2_zero_state()
                .as_ref(app)
                .current_working_directory();
            let cwd_path = cwd.as_ref().map(std::path::Path::new);
            let skill_manager_handle = SkillManager::handle(app);
            let skill_manager = skill_manager_handle.as_ref(app);
            let skills = skill_manager.get_skills_for_working_directory(cwd_path, app);

            for mut skill in skills
                .into_iter()
                .sorted_by(|a, b| b.name.to_lowercase().cmp(&a.name.to_lowercase()))
            {
                // Mirror the CLI-agent provider filtering applied to fuzzy search
                // so zero state and search state stay consistent.
                if let Some(providers) = &cli_agent_providers {
                    if !skill_manager.skill_exists_for_any_provider(&skill, providers) {
                        continue;
                    }
                    skill.provider = skill_manager.best_supported_provider(&skill, providers);
                }
                results.push(InlineItem::from_skill(&skill, app).into());
            }
        }

        // Saved prompts are agent-mode workflows; only surface them when AI is
        // globally enabled. Items are emitted in name-descending order so the
        // mixer's ascending priority sort lands on alphabetical order.
        if self.include_saved_prompts && AISettings::as_ref(app).is_any_ai_enabled(app) {
            let saved_prompts: Vec<_> = CloudModel::as_ref(app)
                .get_all_active_workflows()
                .filter(|cw| cw.model().data.is_agent_mode_workflow())
                .sorted_by(|a, b| {
                    b.model()
                        .data
                        .name()
                        .to_lowercase()
                        .cmp(&a.model().data.name().to_lowercase())
                })
                .collect();
            for saved_prompt in saved_prompts {
                results.push(InlineItem::from_saved_prompt(saved_prompt, app).into());
            }
        }

        Ok(results)
    }
}
