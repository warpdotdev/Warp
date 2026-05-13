use ai::skills::{SkillProvider, SkillReference, SkillScope};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SkillOpenOrigin {
    // 'Open skill' button on ReadSkill tool call result
    ReadSkill,
    // 'Open skill' button on ReadFiles tool call result
    ReadFiles,
    // 'Open skill' button on CodeDiffView
    EditFiles,
    // /open-skill command
    OpenSkillCommand,
    // Skill manager panel edit button
    SkillManager,
}

/// Telemetry events for skills
#[derive(Serialize, Debug)]
pub enum SkillTelemetryEvent {
    /// Emitted when a skill is invoked via the ReadSkill tool call.
    Read {
        /// Specifies the unique reference to the skill, which is either a path or a bundled skill ID
        reference: SkillReference,
        /// Specifies the parsed skill name from SKILL.md front matter, if the reference resolved
        name: Option<String>,
        /// Specifies the scope of the skill.
        scope: Option<SkillScope>,
        /// Specifies the provider of the skill (Warp, Claude, Codex, etc.)
        provider: Option<SkillProvider>,
        /// Whether the ReadSkill lookup failed (reference could not be resolved)
        error: bool,
    },
    /// Emitted when a SKILL.md file is opened from an 'open skill' button or /edit-skill command.
    Opened {
        /// Specifies the unique reference to the skill, which is either a path or a bundled skill ID
        reference: SkillReference,
        /// Specifies the parsed skill name from SKILL.md front matter, if the reference resolved
        name: Option<String>,
        /// Specifies the origin of the skill open (button or command)
        origin: SkillOpenOrigin,
    },
}
