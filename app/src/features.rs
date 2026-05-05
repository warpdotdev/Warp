pub use warp_core::features::*;

pub(crate) fn is_local_to_cloud_handoff_available() -> bool {
    FeatureFlag::OzHandoff.is_enabled()
        && FeatureFlag::HandoffLocalCloud.is_enabled()
        && cfg!(all(feature = "local_fs", not(target_family = "wasm")))
}
