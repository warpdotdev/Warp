cfg_if::cfg_if! {
    if #[cfg(feature = "completions_v2")] {
        mod completions;
        pub use completions::*;
    }
}
mod call_js_function;
mod logging;
mod plugin_host_bootstrap;

pub use call_js_function::*;
pub use logging::*;
pub use plugin_host_bootstrap::*;
