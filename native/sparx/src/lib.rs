#![deny(warnings)]

use base64::Engine;
use bytes::Bytes;
use rustler::{Env, ResourceArc, Term};
use tokio::sync::mpsc;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod atoms;
mod config;
mod request;
mod response;
mod server;
mod websocket;

use config::ServerConfig;
use request::{RequestHandle, ResponseMessage};
use response::NifResult;
use server::{QueuedRequest, ServerHandle};
use websocket::{Frame, WebSocketHandle};

fn load(_env: Env, load_info: Term) -> bool {
    // Configure tracing with SPARX_LOG env variable
    let env_filter = std::env::var("SPARX_LOG")
        .map(|s| EnvFilter::new(&s))
        .unwrap_or_else(|_| EnvFilter::new("warn"));

    tracing_subscriber::registry()
        .with(fmt::layer().with_filter(env_filter))
        .init();

    // Configure Tokio runtime for async tasks
    if let Ok(config) = load_info.decode::<rustler::runtime::RuntimeConfig>() {
        rustler::runtime::configure(config).ok();
    }

    true
}

// ============================================================================
// Server Management NIFs
// ============================================================================

/// Start the HTTP server
/// Returns {:ok, server_ref} or {:error, reason}
#[rustler::nif]
fn server_start(config: ServerConfig) -> Result<ResourceArc<ServerHandle>, String> {
    // Create request queue
    let (request_tx, request_rx) = mpsc::channel::<QueuedRequest>(1024);
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

    let server_handle = ServerHandle::new(request_rx, shutdown_tx);
    let server_arc = ResourceArc::new(server_handle);

    // Spawn server task
    let config_clone = config.clone();
    rustler::spawn(async move {
        tokio::select! {
            result = server::start_server(config_clone, request_tx) => {
                if let Err(e) = result {
                    tracing::error!("Server error: {}", e);
                }
            }
            _ = shutdown_rx.recv() => {
                tracing::info!("Server shutdown requested");
            }
        }
    });

    Ok(server_arc)
}

/// Stop the HTTP server
#[rustler::nif(schedule = "DirtyCpu")]
fn server_stop(server: ResourceArc<ServerHandle>) -> rustler::Atom {
    rustler::spawn(async move {
        server.shutdown().await;
    });
    atoms::ok()
}

/// Receive a request from the server (demand-driven, async)
/// Returns {:ok, request_handle} or {:error, reason}
#[rustler::nif]
async fn receive_request(
    server: ResourceArc<ServerHandle>,
) -> Result<ResourceArc<RequestHandle>, rustler::Atom> {
    match server.receive_request().await {
        Some(handle) => {
            let handle_arc = ResourceArc::new(handle);
            Ok(handle_arc)
        }
        None => {
            // Server shut down or queue closed
            Err(atoms::error())
        }
    }
}

// ============================================================================
// Request Streaming NIFs
// ============================================================================

/// Read a chunk from the request body
/// Returns {:ok, binary} | {:error, reason}
#[rustler::nif]
fn read_chunk(
    env: rustler::Env,
    request: ResourceArc<RequestHandle>,
) -> Result<rustler::Binary, rustler::Atom> {
    // Use oneshot channel to wait for async result
    let (result_tx, result_rx) = tokio::sync::oneshot::channel();

    rustler::spawn(async move {
        let result = match request.read_body_chunk().await {
            Ok(Some(chunk)) => Ok(chunk.to_vec()),
            Ok(None) => Ok(Vec::new()), // Empty vec signals EOF
            Err(_e) => Err(atoms::error()),
        };
        let _ = result_tx.send(result);
    });

    match result_rx.blocking_recv() {
        Ok(Ok(vec)) => {
            // Convert Vec<u8> to Binary
            let mut binary = rustler::OwnedBinary::new(vec.len()).unwrap();
            binary.as_mut_slice().copy_from_slice(&vec);
            Ok(binary.release(env))
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err(atoms::error()),
    }
}

// ============================================================================
// Response Streaming NIFs
// ============================================================================

/// Send response status
/// Returns :ok | {:error, reason}
#[rustler::nif]
async fn send_status(request: ResourceArc<RequestHandle>, status: u16) -> NifResult {
    if let Some(tx) = request.get_response_sender().await {
        match tx.send(ResponseMessage::Status(status)).await {
            Ok(_) => NifResult::Ok,
            Err(_) => NifResult::Error("Failed to send status".to_string()),
        }
    } else {
        NifResult::Error("Response already sent".to_string())
    }
}

/// Send response header
/// Returns :ok | {:error, reason}
#[rustler::nif]
async fn send_header(
    request: ResourceArc<RequestHandle>,
    name: String,
    value: String,
) -> NifResult {
    if let Some(tx) = request.get_response_sender().await {
        match tx.send(ResponseMessage::Header(name, value)).await {
            Ok(_) => NifResult::Ok,
            Err(_) => NifResult::Error("Failed to send header".to_string()),
        }
    } else {
        NifResult::Error("Response already sent".to_string())
    }
}

/// Write a chunk to the response body
/// Returns :ok | {:error, reason}
#[rustler::nif]
fn write_chunk(request: ResourceArc<RequestHandle>, data_term: Term) -> NifResult {
    // Decode binary synchronously
    let binary: rustler::Binary = match data_term.decode() {
        Ok(b) => b,
        Err(_) => return NifResult::Error("Invalid binary data".to_string()),
    };
    let bytes = Bytes::copy_from_slice(binary.as_slice());

    // Create oneshot channel for result
    let (result_tx, result_rx) = tokio::sync::oneshot::channel();

    // Spawn async task
    rustler::spawn(async move {
        let result = if let Some(tx) = request.get_response_sender().await {
            match tx.send(ResponseMessage::BodyChunk(bytes)).await {
                Ok(_) => NifResult::Ok,
                Err(_) => NifResult::Error("Failed to write chunk".to_string()),
            }
        } else {
            NifResult::Error("Response already sent".to_string())
        };
        let _ = result_tx.send(result);
    });

    // Wait for result (blocks the NIF, but that's okay for now)
    match result_rx.blocking_recv() {
        Ok(result) => result,
        Err(_) => NifResult::Error("Internal error".to_string()),
    }
}

/// Finish the response
/// Returns :ok | {:error, reason}
#[rustler::nif]
async fn finish(request: ResourceArc<RequestHandle>) -> NifResult {
    if let Some(tx) = request.get_response_sender().await {
        match tx.send(ResponseMessage::Finish).await {
            Ok(_) => NifResult::Ok,
            Err(_) => NifResult::Error("Failed to finish response".to_string()),
        }
    } else {
        NifResult::Error("Response already sent".to_string())
    }
}

// ============================================================================
// WebSocket NIFs
// ============================================================================

/// Upgrade an HTTP request to a WebSocket connection
/// Returns {:ok, websocket_handle} or {:error, reason}
#[rustler::nif]
async fn upgrade_websocket(
    request: ResourceArc<RequestHandle>,
) -> Result<ResourceArc<WebSocketHandle>, String> {
    use sha1::{Digest, Sha1};

    // Take the upgrade future (can only be done once)
    let upgrade_future = request
        .take_upgrade()
        .await
        .ok_or_else(|| "Not an upgradeable request".to_string())?;

    // Get the Sec-WebSocket-Key from request metadata
    let ws_key = request
        .metadata
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("sec-websocket-key"))
        .map(|(_, v)| v.clone())
        .ok_or_else(|| "Missing Sec-WebSocket-Key header".to_string())?;

    // Compute the Sec-WebSocket-Accept value
    const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let mut sha1 = Sha1::new();
    sha1.update(ws_key.as_bytes());
    sha1.update(WS_GUID.as_bytes());
    let accept = base64::engine::general_purpose::STANDARD.encode(sha1.finalize());

    // Send the 101 Switching Protocols response
    if let Some(tx) = request.get_response_sender().await {
        tx.send(ResponseMessage::Status(101))
            .await
            .map_err(|_| "Failed to send status")?;
        tx.send(ResponseMessage::Header(
            "Upgrade".to_string(),
            "websocket".to_string(),
        ))
        .await
        .map_err(|_| "Failed to send Upgrade header")?;
        tx.send(ResponseMessage::Header(
            "Connection".to_string(),
            "Upgrade".to_string(),
        ))
        .await
        .map_err(|_| "Failed to send Connection header")?;
        tx.send(ResponseMessage::Header(
            "Sec-WebSocket-Accept".to_string(),
            accept,
        ))
        .await
        .map_err(|_| "Failed to send Sec-WebSocket-Accept header")?;
        tx.send(ResponseMessage::Finish)
            .await
            .map_err(|_| "Failed to finish response")?;
    } else {
        return Err("Response already sent".to_string());
    }

    // Wait for the upgrade to complete
    let upgraded = upgrade_future
        .await
        .map_err(|e| format!("Upgrade failed: {}", e))?;

    // Wrap in TokioIo
    let io = hyper_util::rt::TokioIo::new(upgraded);

    // Create WebSocket stream
    let ws_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(
        io,
        tokio_tungstenite::tungstenite::protocol::Role::Server,
        None,
    )
    .await;

    // Create and return WebSocketHandle
    let ws_handle = WebSocketHandle::new(ws_stream);
    Ok(ResourceArc::new(ws_handle))
}

/// Send a text frame over the WebSocket
#[rustler::nif]
async fn ws_send_text(ws: ResourceArc<WebSocketHandle>, text: String) -> NifResult {
    ws.send_frame(Frame::Text(text))
        .await
        .map(|_| NifResult::Ok)
        .unwrap_or_else(NifResult::Error)
}

/// Send a binary frame over the WebSocket
#[rustler::nif]
fn ws_send_binary(ws: ResourceArc<WebSocketHandle>, data: rustler::Binary) -> NifResult {
    let bytes = data.as_slice().to_vec();
    let (tx, rx) = tokio::sync::oneshot::channel();

    rustler::spawn(async move {
        let result = ws
            .send_frame(Frame::Binary(bytes))
            .await
            .map(|_| NifResult::Ok)
            .unwrap_or_else(NifResult::Error);
        let _ = tx.send(result);
    });

    match rx.blocking_recv() {
        Ok(result) => result,
        Err(_) => NifResult::Error("Internal error".to_string()),
    }
}

/// Receive a frame from the WebSocket
/// Returns {:text, data} | {:binary, data} | {:ping, data} | {:pong, data} | :close | :closed
#[rustler::nif]
fn ws_recv(
    env: rustler::Env,
    ws: ResourceArc<WebSocketHandle>,
) -> Result<(rustler::Atom, rustler::Binary), rustler::Atom> {
    // Use oneshot channel to wait for async result
    let (result_tx, result_rx) = tokio::sync::oneshot::channel();

    rustler::spawn(async move {
        let result = match ws.recv_frame().await {
            Some(Frame::Text(text)) => Ok((atoms::text(), text.into_bytes())),
            Some(Frame::Binary(data)) => Ok((atoms::binary(), data)),
            Some(Frame::Ping(data)) => Ok((atoms::ping(), data)),
            Some(Frame::Pong(data)) => Ok((atoms::pong(), data)),
            Some(Frame::Close) => Err(atoms::close()),
            None => Err(atoms::closed()),
        };
        let _ = result_tx.send(result);
    });

    match result_rx.blocking_recv() {
        Ok(Ok((frame_type, data))) => {
            let mut binary = rustler::OwnedBinary::new(data.len()).unwrap();
            binary.as_mut_slice().copy_from_slice(&data);
            Ok((frame_type, binary.release(env)))
        }
        Ok(Err(atom)) => Err(atom),
        Err(_) => Err(atoms::error()),
    }
}

/// Close the WebSocket connection
#[rustler::nif]
async fn ws_close(ws: ResourceArc<WebSocketHandle>) -> NifResult {
    ws.send_frame(Frame::Close)
        .await
        .map(|_| NifResult::Ok)
        .unwrap_or_else(NifResult::Error)
}

// ============================================================================
// NIF Registration
// ============================================================================

rustler::init!("Elixir.Sparx.Native", load = load);
