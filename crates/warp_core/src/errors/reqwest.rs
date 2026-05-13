use http::StatusCode;

use super::{register_error, ErrorExt};

impl ErrorExt for reqwest::Error {
    fn is_actionable(&self) -> bool {
        // Outside of timeouts, there's nothing we can do about errors
        // that occur prior to the successful receipt of an HTTP
        // response.

        // There's no way to check for connection errors via web APIs, so
        // `is_connect` can only be called on native platforms.
        #[cfg(not(target_family = "wasm"))]
        if self.is_connect() {
            return false;
        }

        if self.is_request() || self.is_body() || self.is_decode() {
            return false;
        }

        // 如果服务端返回 capacity error,客户端侧没有可操作的修复动作。
        if self.status() == Some(StatusCode::TOO_MANY_REQUESTS) {
            return false;
        }

        // Internal server errors (5xx) are server-side issues that we can't act upon from the client.
        if self.status().is_some_and(|status| status.is_server_error()) {
            return false;
        }

        // If we're making a request to the staging server and get back
        // a 403 Forbidden, the user is probably not whitelisted to talk
        // to staging from their current IP address, so downgrade to a
        // warning.
        if let (Some(url), Some(status)) = (self.url(), self.status()) {
            if let Some(domain) = url.domain() {
                if domain == "staging.warp.dev" && status == StatusCode::FORBIDDEN {
                    return false;
                }
            }
        }

        true
    }
}
register_error!(reqwest::Error);
