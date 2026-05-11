use std::sync::Arc;

use input_classifier::{HeuristicClassifier, InputClassifier};
use warpui::{Entity, ModelContext, SingletonEntity};

pub struct InputClassifierModel {
    pub classifier: Arc<dyn InputClassifier>,
}

impl InputClassifierModel {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        #[cfg(feature = "nld_onnx_model")]
        {
            let model = select_onnx_model(_ctx);
            match input_classifier::OnnxClassifier::new(model) {
                Ok(classifier) => {
                    log::info!("Loaded onnx classifier ({model:?})");
                    return Self {
                        classifier: Arc::new(classifier),
                    };
                }
                Err(e) => log::warn!("Failed to load onnx classifier ({model:?}): {e:#}"),
            }
        }

        #[cfg(feature = "nld_fasttext_model")]
        if is_nld_classifier_enabled(_ctx) {
            match input_classifier::FasttextClassifier::new() {
                Ok(classifier) => {
                    log::info!("Loaded fasttext classifier");
                    return Self {
                        classifier: Arc::new(classifier),
                    };
                }
                Err(e) => log::warn!("Failed to load fasttext classifier: {e:#}"),
            }
        }

        Self {
            classifier: Arc::new(HeuristicClassifier),
        }
    }

    pub fn classifier(&self) -> Arc<dyn InputClassifier> {
        self.classifier.clone()
    }
}

#[cfg(feature = "nld_onnx_model")]
/// Selects which ONNX classifier model to load. Defaults to the baseline
/// `BertTiny` everywhere; switches to `BertTinyV2` when the
/// `NLDOnnxModelV2Enabled` feature flag is on (auto-enabled for Dev/Local
/// channels via `DOGFOOD_FLAGS`). On dogfood channels, the
/// `EnableNLDOnnxModelV2` private user preference can force-enable or
/// force-disable v2 to override the flag.
fn select_onnx_model(ctx: &warpui::AppContext) -> input_classifier::OnnxModel {
    use warp_core::user_preferences::GetUserPreferences as _;
    use warp_core::{channel::ChannelState, features::FeatureFlag};

    let v2_enabled = if ChannelState::channel().is_dogfood() {
        ctx.private_user_preferences()
            .read_value("EnableNLDOnnxModelV2")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(FeatureFlag::NLDOnnxModelV2Enabled.is_enabled())
    } else {
        FeatureFlag::NLDOnnxModelV2Enabled.is_enabled()
    };

    if v2_enabled {
        input_classifier::OnnxModel::BertTinyV2
    } else {
        input_classifier::OnnxModel::BertTiny
    }
}

impl Entity for InputClassifierModel {
    type Event = ();
}

impl SingletonEntity for InputClassifierModel {}

#[cfg(feature = "nld_fasttext_model")]
/// Returns true iff the NLD classifier model is enabled.
pub fn is_nld_classifier_enabled(ctx: &warpui::AppContext) -> bool {
    use warp_core::user_preferences::GetUserPreferences as _;
    use warp_core::{channel::ChannelState, features::FeatureFlag};

    if ChannelState::channel().is_dogfood() {
        // The `EnableNLDClassifierModel` can be used to force enable / disable
        // use if it is set.
        ctx.private_user_preferences()
            .read_value("EnableNLDClassifierModel")
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(FeatureFlag::NLDClassifierModelEnabled.is_enabled())
    } else {
        FeatureFlag::NLDClassifierModelEnabled.is_enabled()
    }
}
