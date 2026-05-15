//! A common websocket API that works for native and `wasm` targets.

#[cfg_attr(not(target_family = "wasm"), path = "native.rs")]
#[cfg_attr(target_family = "wasm", path = "wasm.rs")]
mod imp;
mod sink_map_err;

use anyhow::anyhow;
#[cfg(not(target_family = "wasm"))]
pub use async_tungstenite::tungstenite::client::IntoClientRequest;
#[cfg(not(target_family = "wasm"))]
use async_tungstenite::tungstenite::http::HeaderValue;
use futures_util::{future, SinkExt, TryStreamExt};
#[cfg(not(target_family = "wasm"))]
use itertools::Itertools;
use thiserror::Error;

#[cfg(not(target_family = "wasm"))]
pub use async_tungstenite::tungstenite;

// Issue #72:把 native::proxy 模块的全局代理类型重导出到 crate root,供
// `app::settings::network` / `app::settings::init` 通过 `websocket::ProxyMode`
// 直接引用。
#[cfg(not(target_family = "wasm"))]
pub use imp::proxy::{set_global_proxy_config, ProxyConfig, ProxyMode};

use crate::sink_map_err::map_err;

// Unfortunately, `anyhow::Error` does not implement `std::error::Error`, which is required by the
// `WebsocketMessage`. To workaround this, we implement a wrapper around `anyhow::Error` using
// `thiserror` as suggested in https://github.com/dtolnay/anyhow/issues/63#issuecomment-591011454.
#[derive(Error, Debug)]
#[error(transparent)]
pub struct Error(#[from] anyhow::Error);

/// The message received / sent to the websocket.
#[derive(Debug)]
pub struct Message(imp::Message);

pub trait WebsocketMessage {
    fn new(text: String) -> Self;

    fn text(&self) -> Option<&str>;

    /// Construct a new message using the `Binary` websocket frame.
    fn new_binary(bytes: Vec<u8>) -> Self;

    /// Returns the bytes if this message was from a `Binary` websocket frame or `None` if the
    /// message was from any other frame type.
    fn binary(&self) -> Option<&[u8]>;

    /// Construct a new message using the `Text` websocket frame.
    fn new_text(text: String) -> Self;
}

impl WebsocketMessage for Message {
    fn new(text: String) -> Self {
        Message(imp::Message::new(text))
    }

    fn text(&self) -> Option<&str> {
        self.0.text()
    }

    fn new_binary(bytes: Vec<u8>) -> Self {
        Self(imp::Message::new_binary(bytes))
    }

    fn binary(&self) -> Option<&[u8]> {
        self.0.binary()
    }

    fn new_text(text: String) -> Self {
        Self(imp::Message::new_text(text))
    }
}

/// A [`WebSocket`] that works natively and on the web. To connect to a websocket
/// with just a URL and an optional set of protocols, use [`Websocket::connect`].
///
/// To connect to a websocket with an enriched client request (e.g. with additional
/// request headers), you can also use [`Websocket::connect`] with an [`http::Request`] but
/// this support is only available for non-wasm targets; custom request headers are not supported
/// for websockets on the web.
///
/// In either case, the caller will have a [`Websocket`] returned.
/// To write or read from the resulting socket, use [`WebSocket::split`].  
pub struct WebSocket(imp::WebSocket);

impl WebSocket {
    /// Split the [`WebSocket`] into separate [`Stream`] and [`Sink`] objects.
    pub async fn split(self) -> (impl Sink, impl Stream) {
        let (sink, stream) = self.0.split().await;
        let sink = sink.with(|item: Message| future::ok(item.0));

        let sink = map_err(sink, |e: imp::Error| Error(anyhow!(e)));
        let stream = stream.map_err(|e| Error(anyhow!(e))).map_ok(Message);
        (sink, stream)
    }

    /// Create the [`WebSocket`] by connecting using the provided `request`.
    /// For non-wasm WebSockets, the request can be enriched with custom
    /// request headers.
    #[cfg(not(target_family = "wasm"))]
    pub async fn connect(
        request: impl IntoClientRequest,
        protocols: impl IntoIterator<Item = &str>,
    ) -> anyhow::Result<Self> {
        let mut request = request.into_client_request()?;
        let protocols = protocols.into_iter().join(", ");
        if !protocols.is_empty() {
            request
                .headers_mut()
                .insert("Sec-WebSocket-Protocol", HeaderValue::from_str(&protocols)?);
        }
        let socket = imp::connect(request).await?;
        Ok(Self(socket))
    }

    /// Create the [`WebSocket`] by connecting against the provided `url`.
    #[cfg(target_family = "wasm")]
    pub async fn connect(
        url: impl AsRef<str>,
        protocols: impl IntoIterator<Item = &str>,
    ) -> anyhow::Result<Self> {
        let socket = imp::connect(url, protocols).await?;
        Ok(Self(socket))
    }
}

/// Trait that defines a [`Sink`] returned by the websocket.
pub trait Sink: futures::Sink<Message, Error = Error> + Send + Unpin + 'static {}

/// Trait that defines a [`Stream`] returned by the websocket.
pub trait Stream: futures::Stream<Item = Result<Message, Error>> + Send + Unpin + 'static {}

impl<T> Sink for T where T: futures::Sink<Message, Error = Error> + Send + Unpin + 'static {}
impl<T> Stream for T where T: futures::Stream<Item = Result<Message, Error>> + Send + Unpin + 'static
{}
