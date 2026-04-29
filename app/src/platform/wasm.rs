use js_sys::ReferenceError;
use thiserror::Error;
use wasm_bindgen::{JsCast, JsValue};

// strip(tier-1): warp_web_event_bus existed to push events from a
// wasm-embedded warp into the host JavaScript page. This fork doesn't
// run as a web app, so the bus is gone. Local stubs keep the wasm
// build green if anyone ever does target wasm; on a native build this
// whole module isn't compiled.
#[derive(Debug, Clone)]
pub enum WarpEvent {
    LoggedOut,
    SessionJoined,
    ErrorLogged { error: String },
    OpenOnNative { url: String },
    ThemeBackgroundChanged { color: String },
}

pub fn emit_event(_event: WarpEvent) {}

/// This function should be called early in application initialization to ensure that
/// static variables are initialized.
pub(super) fn init() {
    unsafe {
        extern "C" {
            /// __wasm_call_ctors is a function defined by the `wasm-ld` linker, and is used to
            /// initialize static variables.
            ///
            /// It should be called once at runtime before other code is executed.
            fn __wasm_call_ctors();
        }

        __wasm_call_ctors();
    }
}

mod ffi {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = "warpUserHandoff", catch)]
        pub fn user_handoff() -> Result<Option<String>, JsValue>;
    }
}

#[derive(Debug, Clone, Error)]
pub enum AuthHandoffError {
    #[error("The host page doesn't support user handoff")]
    Unsupported,
    #[error("Unexpected handoff error: {0:?}")]
    Unexpected(JsValue),
}

/// Fetch the user's Firebase refresh token from the host React app.
pub fn user_handoff() -> Result<Option<String>, AuthHandoffError> {
    ffi::user_handoff().map_err(|err| {
        if ReferenceError::instanceof(&err) {
            AuthHandoffError::Unsupported
        } else {
            AuthHandoffError::Unexpected(err)
        }
    })
}
