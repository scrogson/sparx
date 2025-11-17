# Sparx ⚡

A high-performance HTTP server for Elixir powered by Rust NIFs and Hyper.

> **⚠️ Status**: Early development - not ready for production use

## Overview

Sparx provides a low-level, high-performance HTTP server for Elixir using Rust NIFs and the Hyper HTTP library.

### Implemented Features ✅

- **HTTP/1.1 and HTTP/2** - Automatic protocol detection
- **Request/Response Streaming** - Demand-driven with backpressure control
- **Async I/O** - Powered by Tokio and async-nifs
- **Zero-copy** - Efficient binary handling between Rust and Elixir

### Planned Features 🚧

- **WebSocket** connections (in progress)
- **TLS/SSL** support via rustls
- **HTTP/3** via quinn + h3

## Quick Start

```elixir
# Add to mix.exs
def deps do
  [
    {:sparx, github: "scrogson/sparx"}
  ]
end
```

## Development Status

This project is in **early development**. The architecture is defined in [CLAUDE.md](CLAUDE.md) and scaffolding is complete. Core functionality is being implemented.

See [CLAUDE.md](CLAUDE.md) for complete architecture documentation and development guidelines.

