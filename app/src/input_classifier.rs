use std::sync::Arc;

use input_classifier::{HeuristicClassifier, InputClassifier};
#[cfg(any(feature = "nld_classifier_v1", feature = "nld_classifier_v2"))]
use input_classifier::{OnnxClassifier, OnnxModel};
use warpui::{Entity, ModelContext, SingletonEntity};

pub struct InputClassifierModel {
    pub classifier: Arc<dyn InputClassifier>,
}

impl InputClassifierModel {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        #[cfg(feature = "nld_classifier_v1")]
        {
            match OnnxClassifier::new(OnnxModel::BertTinyV1) {
                Ok(classifier) => {
                    log::info!("Loaded onnx classifier");
                    return Self {
                        classifier: Arc::new(classifier),
                    };
                }
                Err(e) => log::warn!("Failed to load onnx classifier: {e:#}"),
            }
        }

        #[cfg(feature = "nld_classifier_v2")]
        {
            match OnnxClassifier::new(OnnxModel::BertTinyV1) {
                Ok(classifier) => {
                    log::info!("Loaded onnx classifier");
                    return Self {
                        classifier: Arc::new(classifier),
                    };
                }
                Err(e) => log::warn!("Failed to load onnx classifier: {e:#}"),
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

impl Entity for InputClassifierModel {
    type Event = ();
}

impl SingletonEntity for InputClassifierModel {}
