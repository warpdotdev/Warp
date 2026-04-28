use crate::ai::agent::DiffSetHunk;
use crate::code_review::diff_state::{DiffLineType, FileDiff};
use std::collections::HashMap;
use warp_editor::render::model::LineCount;

cfg_if::cfg_if! {
    if #[cfg(feature = "local_fs")] {
        use std::path::Path;
        use crate::ai::agent::{AIAgentAttachment, CurrentHead, DiffBase};
        use crate::ai::blocklist::BlocklistAIContextModel;
        use crate::code_review::{diff_state::DiffMode, DiffSetScope};
        use warpui::{AppContext, ModelHandle};
    }
}

/// Converts file diffs into a HashMap of file paths to DiffSetHunks
/// If repo_path is provided, file paths will be relative to the repo root
pub fn convert_file_diffs_to_diffset_hunks<'a, I>(files: I) -> HashMap<String, Vec<DiffSetHunk>>
where
    I: Iterator<Item = &'a FileDiff>,
{
    let mut file_diffs: HashMap<String, Vec<DiffSetHunk>> = HashMap::new();

    for file_diff in files {
        let file_path = file_diff.file_path.display().to_string();

        let mut file_hunks = Vec::new();
        for hunk in file_diff.hunks.iter() {
            // Format the diff content for this hunk
            let mut diff_lines = Vec::new();
            let mut lines_added = 0;
            let mut lines_removed = 0;
            for line in &hunk.lines {
                let prefix = match line.line_type {
                    DiffLineType::Add => {
                        lines_added += 1;
                        "+"
                    }
                    DiffLineType::Delete => {
                        lines_removed += 1;
                        "-"
                    }
                    DiffLineType::Context => "",
                    DiffLineType::HunkHeader => continue,
                };
                diff_lines.push(format!("{}{}", prefix, line.text));
            }
            let diff_content = diff_lines.join("\n");

            // Create line range using LineCount: Note that git lines are 1-based and LineCount is 0-based
            let line_range = LineCount::from(hunk.new_start_line.saturating_sub(1))
                ..LineCount::from(hunk.new_start_line.saturating_sub(1) + hunk.new_line_count);

            file_hunks.push(DiffSetHunk {
                line_range,
                diff_content,
                lines_added,
                lines_removed,
            });
        }

        if !file_hunks.is_empty() {
            file_diffs.insert(file_path, file_hunks);
        }
    }

    file_diffs
}

/// Creates attachment reference and key for a set of changes based on scope and diff mode
#[cfg(feature = "local_fs")]
pub fn create_attachment_reference_and_key(
    scope: &DiffSetScope,
    diff_mode: &DiffMode,
    main_branch_name: Option<&str>,
    repo_path: &Path,
) -> (String, String) {
    match scope {
        DiffSetScope::All => {
            let diff_set_description = match diff_mode {
                DiffMode::Head => "uncommitted changes".to_string(),
                DiffMode::MainBranch => {
                    let main_branch = main_branch_name.unwrap_or("main");
                    format!("diffset against {main_branch}")
                }
                DiffMode::OtherBranch(branch_name) => {
                    format!("diffset against {branch_name}")
                }
            };
            let key = diff_set_description.clone();
            (format!("<change:{key}>"), key)
        }
        DiffSetScope::File(file_path) => {
            let relative_path = if file_path.is_absolute() {
                file_path
                    .strip_prefix(repo_path)
                    .unwrap_or(file_path)
                    .to_path_buf()
            } else {
                file_path.clone()
            };
            let key = relative_path.display().to_string();
            (format!("<change:{key}>"), key)
        }
    }
}

/// Registers a DiffSet attachment with the AI controller
/// This encapsulates the common logic for creating and registering diff attachments
#[cfg(feature = "local_fs")]
pub fn register_diffset_attachment(
    ai_context_model: &ModelHandle<BlocklistAIContextModel>,
    attachment_key: String,
    file_diffs: HashMap<String, Vec<DiffSetHunk>>,
    current: Option<CurrentHead>,
    base: DiffBase,
    ctx: &mut AppContext,
) {
    // Create the DiffSet attachment
    let attachment = AIAgentAttachment::DiffSet {
        file_diffs,
        current,
        base,
    };

    // Register the attachment with the AI controller
    ai_context_model.update(ctx, |context_model, _| {
        context_model.register_diff_hunk_attachment(attachment_key, attachment);
    });
}
