use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginHostBootstrapRequest {
    pub connection_address: ipc::ConnectionAddress,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginHostBootstrapResponse {
    pub success: bool,
}

/// This is a service implemented by the app process and called by the plugin host process to
/// send a single request at startup containing the connection address for the plugin host server.
///
/// The connection address is used to instantiate an `ipc::Client` that can then be used to call
/// plugin host services.
pub struct PluginHostBootstrapService {}

impl ipc::Service for PluginHostBootstrapService {
    type Request = PluginHostBootstrapRequest;
    type Response = PluginHostBootstrapResponse;
}
