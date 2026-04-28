use anyhow::Result;
use futures::channel::oneshot;

use super::plugin::{PluginRequest, PluginResponse};

/// An interface to make asynchronous [`PluginRequest`]s.
///
/// This is intended to be used to implement IPC `Service`s that rely on plugin execution.
#[derive(Debug, Clone)]
pub(super) struct PluginCaller {
    request_tx: async_channel::Sender<(PluginRequest, oneshot::Sender<PluginResponse>)>,
}

impl PluginCaller {
    /// Constructs a new [`PluginCaller`] from the sending end of the channel to the
    /// `PluginRunners` task that broadcasts [`PluginRequest`]s to individual `PluginRunner`s.
    pub(super) fn new(
        request_tx: async_channel::Sender<(PluginRequest, oneshot::Sender<PluginResponse>)>,
    ) -> Self {
        Self { request_tx }
    }

    pub(super) async fn send_message(&self, request: PluginRequest) -> Result<PluginResponse> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx.send((request, response_tx)).await?;
        Ok(response_rx.await?)
    }
}
