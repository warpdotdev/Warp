pub struct SearchConfig {
    pub use_regex: bool,
    pub use_case_sensitivity: bool,
}

#[cfg_attr(not(target_family = "wasm"), path = "model.rs")]
#[cfg_attr(target_family = "wasm", path = "model_wasm.rs")]
pub mod model;
pub mod view;
