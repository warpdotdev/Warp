// TODO(edward): follow-up — gate callers of this module on both
//   1. an `AISettings` opt-out (mirror `is_shared_block_title_generation_enabled`), and
//   2. a customer-type guard (exclude Enterprise unless Warp plan / dogfood),
// matching the pattern in `terminal/share_block_modal.rs::should_send_title_gen_request`.
// `FeatureFlag::GitOperationsInCodeReview` already gates the surrounding UI,
// but does not address AI-specific privacy / opt-out concerns for sending
// diffs to an LLM.
pub(crate) mod api;
