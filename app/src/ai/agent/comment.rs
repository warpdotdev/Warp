use crate::code_review::comments::CommentId;
use std::path::PathBuf;

/// The current state of a code review.
#[derive(Debug, Clone, Default)]
pub struct CodeReview {
    /// Comments that are currently pending (have yet to be addressed).
    pub pending_comments: Vec<ReviewComment>,
    /// Comments that have been addressed.
    pub addressed_comments: Vec<ReviewComment>,
}

impl CodeReview {
    pub fn new_with_pending_comments(pending_comments: Vec<ReviewComment>) -> Self {
        Self {
            pending_comments,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReviewComment {
    pub id: CommentId,
    pub content: String,
    pub diff: ReviewDiff,
    pub head_title: Option<String>,
}

impl ReviewComment {
    pub fn title(&self) -> String {
        match (&self.diff.file_path, self.diff.line_number) {
            (Some(file_path), Some(line_number)) => {
                let file_name = file_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Invalid File Name");
                let display_line = line_number + 1;
                format!("{file_name}:{display_line}")
            }
            (Some(file_path), None) => {
                let file_name = file_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Invalid File Name");
                file_name.to_string()
            }
            (None, _) => self
                .head_title
                .as_ref()
                .cloned()
                .unwrap_or_else(|| "Review Comment".to_string()),
        }
    }
}

impl From<crate::code_review::comments::AttachedReviewComment> for ReviewComment {
    fn from(comment: crate::code_review::comments::AttachedReviewComment) -> Self {
        let head_title = comment.head().map(|head| head.title());

        ReviewComment {
            id: comment.id,
            content: comment.content,
            diff: comment.target.into(),
            head_title,
        }
    }
}

impl From<crate::code_review::comments::AttachedReviewCommentTarget> for ReviewDiff {
    fn from(val: crate::code_review::comments::AttachedReviewCommentTarget) -> Self {
        // Convert from the server format of a line number (which is zero indexed)
        // to one that is one-indexed to display within the blocklist.
        match val {
            crate::code_review::comments::AttachedReviewCommentTarget::Line {
                absolute_file_path,
                line,
                content: _,
            } => {
                let line_number = line
                    .line_number()
                    .map(|line_number| line_number.as_usize() + 1);
                Self {
                    file_path: Some(absolute_file_path),
                    line_number,
                }
            }
            crate::code_review::comments::AttachedReviewCommentTarget::File {
                absolute_file_path,
            } => Self {
                file_path: Some(absolute_file_path),
                line_number: None,
            },
            crate::code_review::comments::AttachedReviewCommentTarget::General => Self {
                file_path: None,
                line_number: None,
            },
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReviewDiff {
    pub file_path: Option<PathBuf>,
    pub line_number: Option<usize>,
}
