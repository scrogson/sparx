use rustler::NifStruct;

#[derive(NifStruct, Clone)]
#[module = "Sparx.Config"]
pub struct ServerConfig {
    /// Host to bind to (e.g., "127.0.0.1", "0.0.0.0")
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// Maximum number of concurrent connections
    pub max_connections: usize,

    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,

    /// Keep-alive timeout in milliseconds
    pub keep_alive_timeout_ms: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 4000,
            max_connections: 100_000,
            request_timeout_ms: 30_000,
            keep_alive_timeout_ms: 60_000,
        }
    }
}
