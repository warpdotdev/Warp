//! The implementation of `LogService` to be served by the app process to the plugin host process.
use async_trait::async_trait;

use crate::plugin::service::{LogService, LogServiceRequest, LogServiceResponse};

#[derive(Debug, Clone)]
pub struct LogServiceImpl {}

impl LogServiceImpl {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl ipc::ServiceImpl for LogServiceImpl {
    type Service = LogService;

    async fn handle_request(&self, request: LogServiceRequest) -> LogServiceResponse {
        let LogServiceRequest {
            level,
            target,
            message,
        } = request;
        let log_fn = || {
            log::log!(target: target.as_str(), level, "{message}");
        };
        log_fn();
        LogServiceResponse { success: true }
    }
}
