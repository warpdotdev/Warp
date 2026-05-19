#[cfg(not(target_family = "wasm"))]
use crate::features::FeatureFlag;

#[cfg(not(target_family = "wasm"))]
pub(crate) fn remote_codebase_indexing_enabled(codebase_context_enabled: bool) -> bool {
    FeatureFlag::RemoteCodebaseIndexing.is_enabled()
        && FeatureFlag::FullSourceCodeEmbedding.is_enabled()
        && codebase_context_enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn remote_indexing_requires_remote_and_fse_flags_and_codebase_context() {
        {
            let _remote_flag = FeatureFlag::RemoteCodebaseIndexing.override_enabled(true);
            let _flag = FeatureFlag::FullSourceCodeEmbedding.override_enabled(false);
            assert!(!remote_codebase_indexing_enabled(true));
        }
        {
            let _remote_flag = FeatureFlag::RemoteCodebaseIndexing.override_enabled(true);
            let _flag = FeatureFlag::FullSourceCodeEmbedding.override_enabled(true);
            assert!(remote_codebase_indexing_enabled(true));
            assert!(!remote_codebase_indexing_enabled(false));
        }
        {
            let _remote_flag = FeatureFlag::RemoteCodebaseIndexing.override_enabled(false);
            let _flag = FeatureFlag::FullSourceCodeEmbedding.override_enabled(true);
            assert!(!remote_codebase_indexing_enabled(true));
        }
    }
}
