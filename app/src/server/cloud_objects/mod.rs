// PDX-33: only consumed by `cloud_preferences_syncer_tests`, which is itself
// gated on `warp_hosted`. Without the feature the helper has no callers.
#[cfg(all(test, feature = "warp_hosted"))]
pub mod fake_object_client;
pub mod listener;
#[cfg(test)]
pub mod test_utils;
pub mod update_manager;
