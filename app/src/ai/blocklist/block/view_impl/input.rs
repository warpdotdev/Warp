use crate::ai::blocklist::block::CommentElementState;
use crate::code_review::comments::CommentId;
use std::collections::{HashMap, HashSet};

#[derive(Copy, Clone)]
pub(super) struct Props<'a> {
    pub(super) comments: &'a HashMap<CommentId, CommentElementState>,
    pub(super) addressed_comment_ids: &'a HashSet<CommentId>,
}
