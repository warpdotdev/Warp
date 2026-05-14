//! Utility functions for working with skills.

use super::{SkillDescriptor, SkillManager};
use crate::ai::blocklist::view_util::render_provider_icon_button;
use ai::skills::{
    home_skills_path, provider_rank, ParsedSkill, SkillProvider, SKILL_PROVIDER_DEFINITIONS,
};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::color::internal_colors;
use warp_core::ui::Icon;
use warpui::prelude::MouseStateHandle;
use warpui::EventContext;
use warpui::{AppContext, Element, SingletonEntity};

use crate::warp_managed_paths_watcher::warp_managed_skill_dirs;

/// Deduplicates skills by **name and owning directory**, keeping a single best representative per
/// skill name within each directory.
///
/// 优先级规则(同名 skill 多份时):
///
/// 1. **provider rank 小者胜**:依 [`SKILL_PROVIDER_DEFINITIONS`] 顺序(index 0 = 最高优先级),
///    例如 `Agents > Warp > Claude > …`。
/// 2. **同 rank 时 reference 路径短者胜**:取为稳定 tiebreak。
///
/// 该实现覆盖了三种场景:
/// - `npx skills` 软链同名 skill 到 `~/.agents/skills/` / `~/.warp/skills/` / `~/.claude/skills/`
///   (同名不同 provider) → 保留高优先级 provider。
/// - 同名 skill 同时存在于多个目录(例如 repo root + subdir) → 各自保留,让调用方按路径上下文处理。
/// - 同名不同内容 (不同 provider) → 保留高优先级 provider。
///
/// Each element of `skill_paths` is a `(dir_path, skill_file_path)` tuple where
/// `dir_path` is the directory that owns the skill and participates in the dedup key.
///
/// **P0-3 prompt cache 补漏**:返回 Vec 按 `(name, reference)` 字典序排序。
/// 原因:`HashMap::into_values()` 迭代顺序不稳定,该返回值会进入 system prompt 的
/// skills section,顺序漂移就会让全部上游供应商(Anthropic / OpenAI / DeepSeek)的
/// prompt cache 全序失效。与 P0-3 MCP tools 排序同性质。
/// 当前按 `(name, owning directory)` 去重,所以不同目录可以同时保留同名 skill。
/// reference 仍作为稳定排序的次级键,保证输出顺序可复现。
#[cfg_attr(not(feature = "local_fs"), allow(dead_code))]
pub(crate) fn unique_skills(
    skill_paths: &[(PathBuf, PathBuf)],
    skills_by_path: &HashMap<PathBuf, ParsedSkill>,
) -> Vec<SkillDescriptor> {
    let mut name_map: HashMap<(String, PathBuf), SkillDescriptor> = HashMap::new();

    for (dir_path, path) in skill_paths {
        let Some(skill) = skills_by_path.get(path) else {
            continue;
        };
        let descriptor = SkillDescriptor::from(skill.clone());
        match name_map.entry((descriptor.name.clone(), dir_path.clone())) {
            Entry::Vacant(e) => {
                e.insert(descriptor);
            }
            Entry::Occupied(mut e) => {
                let new_rank = provider_rank(descriptor.provider);
                let existing_rank = provider_rank(e.get().provider);
                if new_rank < existing_rank
                    || (new_rank == existing_rank
                        && skill_reference_key(&descriptor.reference).len()
                            < skill_reference_key(&e.get().reference).len())
                {
                    e.insert(descriptor);
                }
            }
        }
    }

    let mut out: Vec<SkillDescriptor> = name_map.into_values().collect();
    // P0-3 补漏:按 (name, reference 字面) 字典序排序使 system prompt 稳定。
    out.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| skill_reference_key(&a.reference).cmp(&skill_reference_key(&b.reference)))
    });
    out
}

/// 为 `SkillReference` 生成用于排序的字面化 key。
/// `Path` 用 `to_string_lossy` 以避免跨平台边界问题;`BundledSkillId`
/// 直接用 id 字串;两者同 key 不会冲突(bundled id 不含路径分隔符)。
fn skill_reference_key(reference: &ai::skills::SkillReference) -> String {
    match reference {
        ai::skills::SkillReference::Path(p) => p.to_string_lossy().into_owned(),
        ai::skills::SkillReference::BundledSkillId(id) => id.clone(),
    }
}

/// 列出当前 working directory 适用的全部 skills。
///
/// **设计说明**:旧版 `list_skills_if_changed` 在云端协议下做差量发送(对比上轮已发的
/// `conversation.latest_skills()`,未变化时返回 `None`)以节省上行 token —— warp 后端
/// 维护会话状态,首轮收到后保留即可。项目去云端后,BYOP 走 OpenAI/Anthropic 等无状态
/// `/chat/completions`,system prompt 每轮在客户端完整重渲染,数据必须每轮都送达,
/// 否则第二轮起 system prompt 里 skills section 会消失。
/// 因此简化为每轮全量返回。
pub fn list_skills(working_directory: Option<&Path>, app: &AppContext) -> Vec<SkillDescriptor> {
    SkillManager::as_ref(app).get_skills_for_working_directory(working_directory, app)
}

/// Renders an 'open skill' button for blocklist AI actions and the code diff view.
pub fn render_skill_button<F>(
    button_label: &str,
    button_handle: MouseStateHandle,
    appearance: &Appearance,
    skill_provider: SkillProvider,
    icon_override: Option<Icon>,
    on_click: F,
) -> Box<dyn Element>
where
    F: FnMut(&mut EventContext) + 'static,
{
    let theme = appearance.theme();
    let logo_fill = internal_colors::fg_overlay_6(theme);

    let icon = icon_override.unwrap_or_else(|| skill_provider.icon());

    let color = if icon_override.is_some() {
        logo_fill
    } else {
        skill_provider.icon_fill(logo_fill)
    };

    render_provider_icon_button(
        button_label,
        button_handle,
        appearance,
        icon,
        color,
        on_click,
    )
}

/// Returns a branded icon override for well-known skill names.
pub fn icon_override_for_skill_name(name: &str) -> Option<Icon> {
    match name {
        "stripe-projects-cli" => Some(Icon::StripeLogo),
        _ => None,
    }
}

pub fn skill_path_from_file_path(file_path: &Path) -> Option<PathBuf> {
    for definition in SKILL_PROVIDER_DEFINITIONS.iter() {
        let home_skill_dirs = if definition.provider == SkillProvider::Warp {
            warp_managed_skill_dirs()
        } else {
            home_skills_path(definition.provider).into_iter().collect()
        };
        for home_skills_path in home_skill_dirs {
            if let Ok(relative_path) = file_path.strip_prefix(&home_skills_path) {
                let skill_name = relative_path.components().next()?;
                return Some(home_skills_path.join(skill_name).join("SKILL.md"));
            }
        }
    }
    let path_components: Vec<_> = file_path.components().collect();

    for def in SKILL_PROVIDER_DEFINITIONS.iter() {
        let skill_components: Vec<_> = def.skills_path.components().collect();

        for (idx, window) in path_components.windows(skill_components.len()).enumerate() {
            if window == skill_components.as_slice() {
                let skill_dir = PathBuf::from_iter(
                    file_path
                        .components()
                        .take(idx + skill_components.len() + 1),
                );
                return Some(skill_dir.join("SKILL.md"));
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "skill_utils_tests.rs"]
mod tests;
