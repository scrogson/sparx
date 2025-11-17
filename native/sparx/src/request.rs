use bytes::Bytes;
use hyper::http::{HeaderMap, Method, Uri, Version};
use rustler::NifStruct;
use tokio::sync::{mpsc, Mutex};

/// Request metadata sent to Elixir
#[derive(NifStruct, Clone)]
#[module = "Sparx.Request.Metadata"]
pub struct RequestMetadata {
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub version: String,
    pub headers: Vec<(String, String)>,
}

/// Handle to an HTTP request
/// This resource holds the state needed for streaming request body
/// and sending the response
pub struct RequestHandle {
    #[allow(dead_code)]
    pub metadata: RequestMetadata,
    /// Receiver for request body chunks
    pub body_rx: Mutex<Option<mpsc::Receiver<Result<Bytes, String>>>>,
    /// Sender for response parts
    pub response_tx: Mutex<Option<ResponseSender>>,
}

/// Types of response messages
pub enum ResponseMessage {
    Status(u16),
    Header(String, String),
    BodyChunk(Bytes),
    Finish,
}

pub type ResponseSender = mpsc::Sender<ResponseMessage>;

impl RequestHandle {
    pub fn new(
        metadata: RequestMetadata,
        body_rx: mpsc::Receiver<Result<Bytes, String>>,
        response_tx: ResponseSender,
    ) -> Self {
        Self {
            metadata,
            body_rx: Mutex::new(Some(body_rx)),
            response_tx: Mutex::new(Some(response_tx)),
        }
    }

    /// Read a chunk from the request body
    pub async fn read_body_chunk(&self) -> Result<Option<Bytes>, String> {
        let mut body_rx_guard = self.body_rx.lock().await;
        if let Some(ref mut rx) = *body_rx_guard {
            match rx.recv().await {
                Some(Ok(chunk)) => {
                    if chunk.is_empty() {
                        // Empty chunk signals EOF
                        Ok(None)
                    } else {
                        Ok(Some(chunk))
                    }
                }
                Some(Err(e)) => Err(e),
                None => Ok(None), // Channel closed = EOF
            }
        } else {
            Err("Body stream already consumed".to_string())
        }
    }

    /// Get a clone of the response sender (for sending multiple messages)
    pub async fn get_response_sender(&self) -> Option<ResponseSender> {
        let guard = self.response_tx.lock().await;
        guard.as_ref().cloned()
    }
}


/// Helper to convert hyper::Version to string
pub fn version_to_string(version: Version) -> String {
    match version {
        Version::HTTP_09 => "HTTP/0.9".to_string(),
        Version::HTTP_10 => "HTTP/1.0".to_string(),
        Version::HTTP_11 => "HTTP/1.1".to_string(),
        Version::HTTP_2 => "HTTP/2.0".to_string(),
        Version::HTTP_3 => "HTTP/3.0".to_string(),
        _ => "HTTP/1.1".to_string(),
    }
}

/// Helper to extract request metadata from hyper request parts
pub fn extract_metadata(method: &Method, uri: &Uri, version: Version, headers: &HeaderMap) -> RequestMetadata {
    let path = uri.path().to_string();
    let query = uri.query().map(|q| q.to_string());

    let headers_vec: Vec<(String, String)> = headers
        .iter()
        .map(|(name, value)| {
            let name_str = name.as_str().to_string();
            let value_str = value.to_str().unwrap_or("").to_string();
            (name_str, value_str)
        })
        .collect();

    RequestMetadata {
        method: method.as_str().to_string(),
        path,
        query,
        version: version_to_string(version),
        headers: headers_vec,
    }
}

// Implement required traits for Rustler Resource
unsafe impl Send for RequestHandle {}
unsafe impl Sync for RequestHandle {}
impl std::panic::RefUnwindSafe for RequestHandle {}

#[rustler::resource_impl]
impl rustler::Resource for RequestHandle {}
