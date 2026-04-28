mod legacy;
mod maa;
mod static_prompt_suggestions;

use warpui::ModelHandle;

pub use legacy::{
    PassiveSuggestionsEvent as LegacyPassiveSuggestionsEvent,
    PassiveSuggestionsModel as LegacyPassiveSuggestionsModel,
};
pub use maa::{
    PassiveSuggestionsEvent as MaaPassiveSuggestionsEvent,
    PassiveSuggestionsModel as MaaPassiveSuggestionsModel,
};

#[derive(Clone)]
pub struct PassiveSuggestionsModels {
    pub legacy: ModelHandle<LegacyPassiveSuggestionsModel>,
    pub maa: ModelHandle<MaaPassiveSuggestionsModel>,
}
