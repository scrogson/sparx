#![deny(warnings)]

use bytes::Bytes;
use rustler::{Env, ResourceArc, Term};
use tokio::sync::mpsc;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod atoms;
mod config;
mod request;
mod response;
mod server;

use config::ServerConfig;
use request::{RequestHandle, ResponseMessage};
use response::NifResult;
use server::{QueuedRequest, ServerHandle};

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
// NIF Registration
// ============================================================================

rustler::init!("Elixir.Sparx.Native", load = load);
