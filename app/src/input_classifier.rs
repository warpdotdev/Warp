use std::sync::Arc;

use input_classifier::{HeuristicClassifier, InputClassifier, OnnxClassifier, OnnxModel};
use warpui::{Entity, ModelContext, SingletonEntity};

pub struct InputClassifierModel {
    pub classifier: Arc<dyn InputClassifier>,
}

impl InputClassifierModel {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        match OnnxClassifier::new(OnnxModel::BertTiny) {
            Ok(classifier) => {
                log::info!("Loaded onnx classifier");
                return Self {
                    classifier: Arc::new(classifier),
                };
            }
            Err(e) => log::warn!("Failed to load onnx classifier: {e:#}"),
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
