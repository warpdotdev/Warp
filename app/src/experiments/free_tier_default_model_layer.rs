use super::{BucketRange, Experiment, Layer};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::str::FromStr;
use warpui::AppContext;

lazy_static! {
    pub static ref FREE_TIER_DEFAULT_MODEL_LAYER: Layer = Layer {
        name: "FreeTierDefaultModelLayer",
        hasher_seeds: (3141, 5926),
        traffic_allocations: HashMap::from([
            (FreeTierDefaultModel::AutoEfficient.get_group_id(), 50.0),
            (FreeTierDefaultModel::AutoOpen.get_group_id(), 50.0),
        ]),
        bucket_ranges: vec![
            BucketRange::new(FreeTierDefaultModel::AutoEfficient, 0..500),
            BucketRange::new(FreeTierDefaultModel::AutoOpen, 500..1000),
        ]
    };
}

/// 50/50 A/B test of the default model surfaced to free-tier users in the
/// pre-signup onboarding ("configure oz") model picker.
#[derive(Debug)]
pub enum FreeTierDefaultModel {
    /// Control: keep the existing free-tier default (auto (cost-efficient)).
    AutoEfficient,
    /// Experiment: surface auto (open-weights) as the default for free users.
    AutoOpen,
}

const FREE_TIER_DEFAULT_MODEL_AUTO_EFFICIENT: &str = "AutoEfficient";
const FREE_TIER_DEFAULT_MODEL_AUTO_OPEN: &str = "AutoOpen";

impl Experiment<FreeTierDefaultModel> for FreeTierDefaultModel {
    fn name() -> &'static str {
        "FreeTierDefaultModel"
    }

    fn variant(&self) -> &'static str {
        match self {
            Self::AutoEfficient => FREE_TIER_DEFAULT_MODEL_AUTO_EFFICIENT,
            Self::AutoOpen => FREE_TIER_DEFAULT_MODEL_AUTO_OPEN,
        }
    }

    fn allow_user_overrides_in_stable() -> bool {
        false
    }
}

impl FromStr for FreeTierDefaultModel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            FREE_TIER_DEFAULT_MODEL_AUTO_EFFICIENT => Ok(Self::AutoEfficient),
            FREE_TIER_DEFAULT_MODEL_AUTO_OPEN => Ok(Self::AutoOpen),
            _ => Err(anyhow::anyhow!(
                "Variant {} is not a valid group in FreeTierDefaultModel",
                s
            )),
        }
    }
}

impl FreeTierDefaultModel {
    pub fn should_default_to_auto_open(ctx: &mut AppContext) -> bool {
        matches!(Self::get_group(ctx), Some(Self::AutoOpen))
    }
}
