use futures::{SinkExt, StreamExt};
use hyper_util::rt::TokioIo;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;
use tokio_tungstenite::WebSocketStream;

/// WebSocket frame types that can be sent/received
#[derive(Debug, Clone)]
pub enum Frame {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close,
}

impl Frame {
    /// Convert to tungstenite message
    pub fn to_ws_message(&self) -> WsMessage {
        match self {
            Frame::Text(s) => WsMessage::Text(s.clone()),
            Frame::Binary(b) => WsMessage::Binary(b.clone()),
            Frame::Ping(p) => WsMessage::Ping(p.clone()),
            Frame::Pong(p) => WsMessage::Pong(p.clone()),
            Frame::Close => WsMessage::Close(None),
        }
    }

    /// Convert from tungstenite message
    pub fn from_ws_message(msg: WsMessage) -> Option<Self> {
        match msg {
            WsMessage::Text(s) => Some(Frame::Text(s)),
            WsMessage::Binary(b) => Some(Frame::Binary(b)),
            WsMessage::Ping(p) => Some(Frame::Ping(p)),
            WsMessage::Pong(p) => Some(Frame::Pong(p)),
            WsMessage::Close(_) => Some(Frame::Close),
            WsMessage::Frame(_) => None, // Raw frames not exposed
        }
    }
}

/// WebSocket connection handle
pub struct WebSocketHandle {
    /// The underlying WebSocket stream
    stream: Mutex<Option<WebSocketStream<TokioIo<hyper::upgrade::Upgraded>>>>,
}

impl WebSocketHandle {
    /// Create a new WebSocket handle from an upgraded connection
    #[allow(dead_code)]
    pub fn new(ws_stream: WebSocketStream<TokioIo<hyper::upgrade::Upgraded>>) -> Self {
        Self {
            stream: Mutex::new(Some(ws_stream)),
        }
    }

    /// Send a frame to the WebSocket
    pub async fn send_frame(&self, frame: Frame) -> Result<(), String> {
        let mut stream_opt = self.stream.lock().await;
        if let Some(stream) = stream_opt.as_mut() {
            let ws_msg = frame.to_ws_message();
            stream
                .send(ws_msg)
                .await
                .map_err(|e| format!("Failed to send frame: {}", e))
        } else {
            Err("WebSocket closed".to_string())
        }
    }

    /// Receive a frame from the WebSocket (blocking until frame arrives)
    pub async fn recv_frame(&self) -> Option<Frame> {
        let mut stream_opt = self.stream.lock().await;
        if let Some(stream) = stream_opt.as_mut() {
            match stream.next().await {
                Some(Ok(msg)) => Frame::from_ws_message(msg),
                Some(Err(_)) | None => {
                    // Connection closed or error
                    *stream_opt = None;
                    None
                }
            }
        } else {
            None
        }
    }
}

unsafe impl Send for WebSocketHandle {}
unsafe impl Sync for WebSocketHandle {}

impl std::panic::RefUnwindSafe for WebSocketHandle {}

#[rustler::resource_impl]
impl rustler::Resource for WebSocketHandle {}
