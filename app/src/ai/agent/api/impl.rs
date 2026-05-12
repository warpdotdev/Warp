use std::sync::Arc;

use anyhow::anyhow;
use futures_util::StreamExt;

use crate::server::server_api::AIApiError;

use super::{ConvertToAPITypeError, RequestParams, ResponseStream};

pub async fn generate_multi_agent_output(
    _params: RequestParams,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ResponseStream, ConvertToAPITypeError> {
    log::debug!("generate_multi_agent_output disabled in OpenWarp (BYOP-only)");
    let error_stream = futures::stream::once(async {
        Err(Arc::new(AIApiError::Other(anyhow!(
            "Cloud multi-agent endpoint disabled in OpenWarp; configure a BYOP provider in Settings"
        ))))
    })
    .take_until(cancellation_rx);
    Ok(Box::pin(error_stream))
}
