# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Sparx is a high-performance HTTP server for Elixir implemented as a Rust NIF using [Rustler](https://github.com/rusterlium/rustler). It wraps [hyper](https://github.com/hyperium/hyper) to provide low-level HTTP/1.1, HTTP/2, and WebSocket support with a demand-driven streaming architecture.

## Architecture

### Design Principles

1. **Demand-Driven**: Elixir processes explicitly request work from the server
2. **Streaming First**: Request and response bodies stream on-demand, not buffered
3. **Zero-Copy**: Minimize data copying between Rust and Elixir
4. **Process Pool**: Worker processes pull requests and handle them independently

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      Elixir Layer                       │
│                                                         │
│  ┌──────────────┐    ┌──────────────┐                   │
│  │   Worker 1   │    │   Worker 2   │   ...Worker N     │
│  │  (GenStage)  │    │  (GenStage)  │                   │
│  └──────┬───────┘    └──────┬───────┘                   │
│         │ demand            │ demand                    │
│         └────────┬──────────┘                           │
│                  ▼                                      │
│         ┌────────────────┐                              │
│         │ Sparx.Server   │                              │
│         │  (Producer)    │                              │
│         └────────┬───────┘                              │
│                  │                                      │
└──────────────────┼──────────────────────────────────────┘
                   │ NIF boundary
┌──────────────────┼──────────────────────────────────────┐
│                  ▼         Rust NIF Layer               │
│         ┌─────────────────┐                             │
│         │ Request Queue   │                             │
│         │  (mpsc channel) │                             │
│         └────────▲────────┘                             │
│                  │                                      │
│         ┌────────┴────────┐                             │
│         │  Hyper Server   │                             │
│         │ (Tokio Runtime) │                             │
│         └─────────────────┘                             │
│                  ▲                                      │
│                  │ TCP/HTTP                             │
└──────────────────┼──────────────────────────────────────┘
                   │
              HTTP Clients
```

### Request Flow

1. **HTTP Request Arrives**: Hyper accepts connection and parses HTTP request
2. **Enqueue**: Request metadata (method, path, headers) placed in mpsc queue
3. **Demand**: Elixir worker calls `receive_request()` NIF
4. **Dispatch**: NIF pops from queue, returns `RequestHandle` resource to worker
5. **Stream Body**: Worker calls `read_body_chunk(handle)` for each chunk
6. **Process**: Worker processes request (business logic in Elixir)
7. **Stream Response**: Worker calls `send_status()`, `send_header()`, `write_body_chunk()`, `finish_response()`
8. **Complete**: Rust sends response to client, closes connection or keeps alive

### WebSocket Flow

1. **Upgrade Request**: HTTP request with `Upgrade: websocket` header arrives
2. **Dispatch to Worker**: Worker receives request, recognizes upgrade
3. **Accept Upgrade**: Worker calls `accept_websocket(handle)`
4. **Bidirectional Stream**:
   - `receive_ws_frame(ws_handle)` - demand-driven receive
   - `send_ws_frame(ws_handle, frame)` - send frames
5. **Close**: Either side calls `close_websocket(ws_handle)`

## Key Components

### Rust NIF Layer (`native/sparx/src/`)

- **`lib.rs`**: NIF registration, Tokio runtime initialization
- **`server.rs`**: Hyper server setup, connection handling, request queueing
- **`request.rs`**: RequestHandle resource, body streaming
- **`response.rs`**: Response streaming, chunked encoding
- **`websocket.rs`**: WebSocket upgrade, frame streaming
- **`config.rs`**: Server configuration (host, port, TLS, etc.)

### Elixir Layer (`lib/sparx/`)

- **`Sparx`**: Top-level API for starting/stopping servers
- **`Sparx.Server`**: GenStage producer that implements demand-driven request dispatch
- **`Sparx.Request`**: Request struct with metadata and streaming functions
- **`Sparx.Response`**: Response builder with streaming support
- **`Sparx.WebSocket`**: WebSocket connection handling

## Design Decisions

### ✓ Decision 1: Response API Style - **BOTH**

Provide both builder pattern and streaming API:

**Builder Pattern (Simple Cases)**
```elixir
def handle_get(request) do
  Sparx.Response.new(200)
  |> Sparx.Response.header("content-type", "application/json")
  |> Sparx.Response.body(Jason.encode!(%{status: "ok"}))
  |> Sparx.Response.send(request)
end
```

**Streaming API (Large Responses)**
```elixir
def handle_file_download(request) do
  :ok = Sparx.Response.send_status(request, 200)
  :ok = Sparx.Response.send_header(request, "content-type", "video/mp4")

  File.stream!("/path/to/video.mp4", [], 64_000)
  |> Enum.each(fn chunk ->
    :ok = Sparx.Response.write_chunk(request, chunk)
  end)

  :ok = Sparx.Response.finish(request)
end
```

### ✓ Decision 2: Request Body Backpressure - **Hybrid Async Pull**

Provide low-level async NIF primitive with high-level Stream wrapper:

**Low-Level Primitive**
```elixir
def handle_upload(request) do
  read_body_loop(request, <<>>)
end

defp read_body_loop(request, acc) do
  ref = Sparx.Request.read_chunk(request)  # Async NIF, returns reference

  receive do
    {^ref, {:ok, chunk}} ->
      new_acc = process_chunk(acc, chunk)
      read_body_loop(request, new_acc)

    {^ref, :eof} ->
      finalize(acc)
  after
    30_000 -> {:error, :timeout}
  end
end
```

**High-Level Stream API**
```elixir
def handle_upload(request) do
  Sparx.Request.body_stream(request)
  |> Stream.each(&process_chunk/1)
  |> Stream.run()
end

# Implemented as:
def body_stream(request, opts \\ []) do
  timeout = Keyword.get(opts, :timeout, 30_000)

  Stream.resource(
    fn -> request end,
    fn req ->
      ref = read_chunk(req)
      receive do
        {^ref, {:ok, chunk}} -> {[chunk], req}
        {^ref, :eof} -> {:halt, req}
      after
        timeout -> raise "Body read timeout"
      end
    end,
    fn _req -> :ok end
  )
end
```

**Key Points:**
- `read_chunk/1` is async NIF returning reference
- Elixir receives `{ref, result}` when chunk ready
- Elixir controls backpressure by deciding when to call `read_chunk/1` again
- Stream wrapper provides convenience for common cases

### ✓ Decision 3: TLS/SSL Support - **Deferred**

No TLS support in initial version. Use reverse proxy (nginx, HAProxy, etc.) for TLS termination. Can add native TLS with `rustls` in later phase.

### ✓ Decision 4: Error Handling - **Auto-Close**

When Elixir worker crashes while processing request:
- Rust detects dropped `RequestHandle` resource
- Automatically sends 500 Internal Server Error
- Closes connection gracefully
- Logs error for debugging

### ✓ Decision 5: Keep-Alive & Connection Pooling - **Rust Manages**

Hyper handles keep-alive automatically. Rust layer manages connection pooling and reuse. Sensible defaults, no Elixir configuration needed initially.

## Development Workflow

### Initial Setup

```bash
# Create Elixir project
cd /Users/scrogson/github/scrogson/sparx
mix new . --app sparx

# Add Rustler dependency with async-nifs
# Edit mix.exs to add rustler

# Create Rust NIF
mix rustler.new sparx

# Build
mix compile
```

### Development Commands

```bash
# Compile both Elixir and Rust
mix compile

# Run tests
mix test

# Format code
just format  # (once Justfile is created)

# Lint
just lint

# Run example server
mix run examples/hello_world.exs
```

## Implementation Phases

### Phase 1: Core HTTP/1.1 (MVP)
- [ ] Hyper server setup with Tokio runtime
- [ ] Request queue with demand-driven dispatch
- [ ] Request streaming (headers + body chunks)
- [ ] Response streaming (status + headers + body chunks)
- [ ] Basic keep-alive support
- [ ] GenStage producer/consumer pattern
- [ ] Example: Simple HTTP server

### Phase 2: HTTP/2 & WebSockets
- [ ] HTTP/2 support via hyper
- [ ] WebSocket upgrade handling
- [ ] WebSocket frame streaming (bidirectional)
- [ ] Ping/Pong handling
- [ ] Example: WebSocket echo server
- [ ] Example: HTTP/2 server with server push

### Phase 3: Performance & Production
- [ ] Connection pooling optimization
- [ ] Graceful shutdown
- [ ] Telemetry integration
- [ ] Benchmarking suite
- [ ] Memory optimization (zero-copy where possible)
- [ ] Example: High-performance API server

### Phase 4: Advanced Features
- [ ] TLS/SSL support (rustls)
- [ ] HTTP/3 support (quinn + h3)
- [ ] Rate limiting
- [ ] Request/response compression
- [ ] Server-Sent Events (SSE)
- [ ] Multipart form handling

## Dependencies

### Rust (Cargo.toml)
```toml
[dependencies]
rustler = { git = "https://github.com/rusterlium/rustler", branch = "async-nifs" }
hyper = { version = "1.0", features = ["full"] }
hyper-util = { version = "0.1", features = ["full"] }
tokio = { version = "1.0", features = ["full"] }
tokio-tungstenite = "0.23"  # WebSocket support
http-body-util = "0.1"
bytes = "1.0"
futures = "0.3"
tracing = "0.1"
tracing-subscriber = "0.3"
```

### Elixir (mix.exs)
```elixir
{:rustler, github: "rusterlium/rustler", branch: "async-nifs", sparse: "rustler_mix", runtime: false},
{:telemetry, "~> 1.0"},
{:gen_stage, "~> 1.0"}  # For demand-driven dispatch
```

## Testing Strategy

- **Unit Tests (Rust)**: Test individual components (parsing, streaming)
- **Unit Tests (Elixir)**: Test API modules
- **Integration Tests (Elixir)**: Full request/response cycle
- **Benchmark Tests**: Compare against Cowboy, Bandit
- **Load Tests**: Concurrent connections, sustained throughput
- **Streaming Tests**: Large file uploads/downloads
- **WebSocket Tests**: Long-lived connections, frame handling

## Performance Targets

- **Latency**: p50 < 1ms, p99 < 10ms for simple responses
- **Throughput**: > 100k req/s on modern hardware
- **Memory**: < 1KB per idle connection
- **Concurrency**: Support 100k+ concurrent connections
- **Streaming**: Constant memory regardless of body size

## Notes

- Uses async-nifs branch of Rustler for async/await support
- Tokio runtime initialized once at NIF load time
- Request/Response resources must be cleaned up properly
- WebSocket connections are long-lived, need careful resource management
- Consider BEAM scheduler interaction for long-running Rust tasks

## Open Questions

1. Should we expose raw hyper `Service` trait for maximum flexibility?
2. How to handle HTTP/2 server push from Elixir API?
3. Should we support custom protocol upgrades (not just WebSocket)?
4. What's the best way to handle request timeout configuration?
5. How to integrate with Phoenix? Plug adapter?

---

**Status**: Project initialization in progress

kkkkkkkkj Focus**: Gathering architectural decisions before implementation
