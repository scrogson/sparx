use crate::config::ServerConfig;
use crate::request::{extract_metadata, RequestHandle, ResponseMessage};
use crate::response::build_response_from_channel;
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info};

type BoxBody = http_body_util::combinators::BoxBody<Bytes, Infallible>;

/// A queued request waiting to be picked up by Elixir
pub struct QueuedRequest {
    pub handle: RequestHandle,
}

/// Server handle resource
pub struct ServerHandle {
    /// Queue of pending requests
    pub request_queue: Mutex<mpsc::Receiver<QueuedRequest>>,
    /// Shutdown signal sender
    pub shutdown_tx: Mutex<Option<mpsc::Sender<()>>>,
}

impl ServerHandle {
    pub fn new(
        request_rx: mpsc::Receiver<QueuedRequest>,
        shutdown_tx: mpsc::Sender<()>,
    ) -> Self {
        Self {
            request_queue: Mutex::new(request_rx),
            shutdown_tx: Mutex::new(Some(shutdown_tx)),
        }
    }

    /// Receive a request from the queue (demand-driven)
    pub async fn receive_request(&self) -> Option<RequestHandle> {
        let mut queue = self.request_queue.lock().await;
        queue.recv().await.map(|req| req.handle)
    }

    /// Shutdown the server
    pub async fn shutdown(&self) {
        let mut guard = self.shutdown_tx.lock().await;
        if let Some(tx) = guard.take() {
            let _ = tx.send(()).await;
        }
    }
}

unsafe impl Send for ServerHandle {}
unsafe impl Sync for ServerHandle {}

// Implement RefUnwindSafe since we need this for NIF resources
impl std::panic::RefUnwindSafe for ServerHandle {}

#[rustler::resource_impl]
impl rustler::Resource for ServerHandle {}

/// Start the HTTP server
pub async fn start_server(
    config: ServerConfig,
    request_tx: mpsc::Sender<QueuedRequest>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .map_err(|e| format!("Invalid address: {}", e))?;

    let listener = TcpListener::bind(addr).await?;
    info!("Sparx server listening on http://{}", addr);

    loop {
        let (stream, remote_addr) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                error!("Failed to accept connection: {}", e);
                continue;
            }
        };

        let io = TokioIo::new(stream);
        let request_tx = request_tx.clone();

        // Spawn a task to handle this connection
        tokio::spawn(async move {
            let service = service_fn(move |req: Request<Incoming>| {
                let request_tx = request_tx.clone();
                async move {
                    handle_request(req, request_tx).await
                }
            });

            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, service)
                .await
            {
                error!("Error serving connection from {}: {}", remote_addr, e);
            }
        });
    }
}

/// Handle a single HTTP request
async fn handle_request(
    req: Request<Incoming>,
    request_tx: mpsc::Sender<QueuedRequest>,
) -> Result<Response<BoxBody>, Infallible> {
    // Extract request parts
    let (parts, body) = req.into_parts();

    // Extract metadata
    let metadata = extract_metadata(&parts.method, &parts.uri, parts.version, &parts.headers);

    // Create channels for body streaming and response
    let (body_tx, body_rx) = mpsc::channel::<Result<Bytes, String>>(16);
    let (response_tx, response_rx) = mpsc::channel::<ResponseMessage>(16);

    // Create request handle
    let request_handle = RequestHandle::new(metadata, body_rx, response_tx.clone());

    // Spawn task to stream request body into channel
    tokio::spawn(async move {
        let mut body = body;

        loop {
            match body.frame().await {
                Some(Ok(frame)) => {
                    if let Some(chunk) = frame.data_ref() {
                        let bytes = chunk.to_vec();
                        if body_tx.send(Ok(Bytes::from(bytes))).await.is_err() {
                            // Receiver dropped, stop streaming
                            break;
                        }
                    }
                    // If frame has no data (trailers), continue
                }
                Some(Err(e)) => {
                    let _ = body_tx.send(Err(format!("Body read error: {}", e))).await;
                    break;
                }
                None => {
                    // Send empty chunk to signal EOF
                    let _ = body_tx.send(Ok(Bytes::new())).await;
                    break;
                }
            }
        }
    });

    // Queue the request for Elixir to pick up
    let queued = QueuedRequest {
        handle: request_handle,
    };

    if request_tx.send(queued).await.is_err() {
        error!("Failed to queue request - server may be shutting down");
        return Ok(error_response(500, "Server Error"));
    }

    // Wait for Elixir to build and send the response
    match build_response_from_channel(response_rx).await {
        Ok(response) => Ok(response),
        Err(e) => {
            error!("Failed to build response: {}", e);
            Ok(error_response(500, "Internal Server Error"))
        }
    }
}

/// Create an error response
fn error_response(status: u16, message: &str) -> Response<BoxBody> {
    use http_body_util::BodyExt;

    let status_code = crate::response::u16_to_status(status);
    let body = http_body_util::Full::new(Bytes::from(message.to_string()))
        .map_err(|never| match never {})
        .boxed();

    Response::builder()
        .status(status_code)
        .header("content-type", "text/plain")
        .body(body)
        .unwrap()
}
