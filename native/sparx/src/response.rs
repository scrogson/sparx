use bytes::Bytes;
use http_body_util::{BodyExt, StreamBody};
use hyper::body::Frame;
use hyper::{Response, StatusCode};
use std::convert::Infallible;
use tokio::sync::mpsc;
use futures::stream;
use rustler::{Encoder, Env, Term};

type BoxBody = http_body_util::combinators::BoxBody<Bytes, Infallible>;

/// Custom result type for NIF functions that properly encodes to Elixir
pub enum NifResult {
    Ok,
    Error(String),
}

impl Encoder for NifResult {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match self {
            NifResult::Ok => crate::atoms::ok().encode(env),
            NifResult::Error(msg) => (crate::atoms::error(), msg.as_str()).encode(env),
        }
    }
}

/// Helper to convert status code to hyper::StatusCode
pub fn u16_to_status(code: u16) -> StatusCode {
    StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

/// Build a hyper Response from a stream of response messages
pub struct ResponseBuilder {
    pub status: Option<StatusCode>,
    pub headers: Vec<(String, String)>,
    pub body_chunks: Vec<Bytes>,
}

impl ResponseBuilder {
    pub fn new() -> Self {
        Self {
            status: None,
            headers: Vec::new(),
            body_chunks: Vec::new(),
        }
    }

    pub fn set_status(&mut self, status: u16) {
        self.status = Some(u16_to_status(status));
    }

    pub fn add_header(&mut self, name: String, value: String) {
        self.headers.push((name, value));
    }

    pub fn add_body_chunk(&mut self, chunk: Bytes) {
        self.body_chunks.push(chunk);
    }

    pub fn build(self) -> Result<Response<BoxBody>, String> {
        let status = self.status.unwrap_or(StatusCode::OK);

        let mut response_builder = Response::builder().status(status);

        // Add headers
        for (name, value) in self.headers {
            response_builder = response_builder.header(name, value);
        }

        // Create body from chunks
        let body = if self.body_chunks.is_empty() {
            http_body_util::Empty::<Bytes>::new()
                .map_err(|never| match never {})
                .boxed()
        } else {
            let stream = stream::iter(self.body_chunks.into_iter().map(|chunk| Ok::<_, Infallible>(Frame::data(chunk))));
            StreamBody::new(stream).boxed()
        };

        response_builder
            .body(body)
            .map_err(|e| format!("Failed to build response: {}", e))
    }
}

impl Default for ResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Spawn a task that receives response messages and builds a Response
pub async fn build_response_from_channel(
    mut rx: mpsc::Receiver<crate::request::ResponseMessage>,
) -> Result<Response<BoxBody>, String> {
    use crate::request::ResponseMessage;

    let mut builder = ResponseBuilder::new();

    while let Some(msg) = rx.recv().await {
        match msg {
            ResponseMessage::Status(status) => {
                builder.set_status(status);
            }
            ResponseMessage::Header(name, value) => {
                builder.add_header(name, value);
            }
            ResponseMessage::BodyChunk(chunk) => {
                builder.add_body_chunk(chunk);
            }
            ResponseMessage::Finish => {
                break;
            }
        }
    }

    builder.build()
}
