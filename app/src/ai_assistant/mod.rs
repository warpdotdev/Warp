//! AI Assistant has since been renamed to "Warp AI" in the product.
use std::{collections::HashSet, sync::Arc};

use crate::{server::telemetry::OpenedWarpAISource, terminal::model::terminal_model::BlockIndex};
use lazy_static::lazy_static;
use pathfinder_color::ColorU;
use warp_core::command::ExitCode;

pub mod execution_context;
pub mod panel;
pub mod requests;
pub mod transcript;
pub mod utils;

#[cfg(test)]
mod test_util;

/// We want to make sure the user doesn't send a prompt too large.s
/// Since a token is ~ 4 chars, the limit we impose here is 250 tokens.
/// This is also roughly the limit at which the editor starts degrading.
pub const PROMPT_CHARACTER_LIMIT: usize = 1000;

pub const AI_ASSISTANT_FEATURE_NAME: &str = "Warp AI";
pub const ASK_AI_ASSISTANT_TEXT: &str = "Ask Warp AI";

pub const AI_ASSISTANT_SVG_PATH: &str = "bundled/svg/ai-assistant.svg";

lazy_static! {
    pub static ref AI_ASSISTANT_LOGO_COLOR: ColorU = ColorU::new(243, 185, 17, 255);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AskAIType {
    /// Covers all possible origins of text selection, including the block list terminal,
    /// the alt-screen terminal, and the input area. Not all instances will require
    /// `populate_input_box`, which determines whether we should automatically render
    /// something like "Explain the following" within the user's input box.
    FromTextSelection {
        text: Arc<String>,
        populate_input_box: bool,
    },
    /// Data about a block to inform Agent Mode.
    FromBlock {
        input: Arc<String>,
        output: Arc<String>,
        exit_code: ExitCode,
        block_index: BlockIndex,
    },
    /// Which blocks to attach to a block list AI query.
    FromBlocks {
        block_indices: HashSet<BlockIndex>,
    },
    FromAICommandSearch {
        query: Arc<String>,
    },
}

impl From<&AskAIType> for OpenedWarpAISource {
    fn from(value: &AskAIType) -> Self {
        match value {
            AskAIType::FromAICommandSearch { .. } => OpenedWarpAISource::FromAICommandSearch,
            AskAIType::FromBlock { .. } | AskAIType::FromBlocks { .. } => {
                OpenedWarpAISource::HelpWithBlock
            }
            AskAIType::FromTextSelection { .. } => OpenedWarpAISource::HelpWithTextSelection,
        }
    }
}
