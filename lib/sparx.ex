defmodule Sparx do
  @moduledoc """
  High-performance HTTP server for Elixir powered by Rust NIFs and Hyper.

  Sparx provides a low-level HTTP server with demand-driven streaming architecture.

  ## Quick Start

      # Define a simple handler
      defmodule MyHandler do
        def handle_request(request) do
          Sparx.Response.send_text(request, 200, "Hello, World!")
        end
      end

      # Start the server
      {:ok, server} = Sparx.start_link(
        handler: &MyHandler.handle_request/1,
        port: 4000
      )

      # Server is now listening on http://localhost:4000

  ## Architecture

  Sparx uses a demand-driven architecture where:

  1. HTTP requests arrive via Hyper (Rust)
  2. Requests are queued in Rust
  3. Elixir processes pull requests on-demand
  4. Request/response bodies stream with backpressure

  This architecture provides excellent performance while maintaining BEAM's fault tolerance.
  """

  use GenServer
  alias Sparx.{Config, Native, Response}

  @type server_ref :: GenServer.server()
  @type handler :: (reference() -> :ok)

  ## Client API

  @doc """
  Start a Sparx HTTP server.

  ## Options

    * `:handler` - Function that handles requests (required)
    * `:port` - Port to listen on (default: 4000)
    * `:host` - Host to bind to (default: "127.0.0.1")
    * `:name` - Name to register the server under (optional)
    * `:max_connections` - Maximum concurrent connections (default: 100,000)
    * `:request_timeout_ms` - Request timeout in milliseconds (default: 30,000)
    * `:keep_alive_timeout_ms` - Keep-alive timeout in milliseconds (default: 60,000)

  ## Examples

      {:ok, server} = Sparx.start_link(
        handler: &MyApp.handle_request/1,
        port: 4000
      )

  """
  @spec start_link(keyword()) :: GenServer.on_start()
  def start_link(opts) do
    {gen_opts, sparx_opts} = Keyword.split(opts, [:name])
    GenServer.start_link(__MODULE__, sparx_opts, gen_opts)
  end

  @doc """
  Stop a Sparx HTTP server.

  ## Examples

      :ok = Sparx.stop(server)

  """
  @spec stop(server_ref()) :: :ok
  def stop(server) do
    GenServer.stop(server)
  end

  ## Server Callbacks

  @impl true
  def init(opts) do
    handler = Keyword.fetch!(opts, :handler)

    config = %Config{
      host: Keyword.get(opts, :host, "127.0.0.1"),
      port: Keyword.get(opts, :port, 4000),
      max_connections: Keyword.get(opts, :max_connections, 100_000),
      request_timeout_ms: Keyword.get(opts, :request_timeout_ms, 30_000),
      keep_alive_timeout_ms: Keyword.get(opts, :keep_alive_timeout_ms, 60_000)
    }

    case Native.server_start(config) do
      {:ok, server_ref} ->
        # Spawn worker process to pull and handle requests
        worker_pid = spawn_link(fn ->
          request_loop(server_ref, handler)
        end)

        {:ok, %{server_ref: server_ref, worker: worker_pid, handler: handler}}

      {:error, reason} ->
        {:stop, {:failed_to_start, reason}}
    end
  end

  @impl true
  def terminate(_reason, state) do
    Native.server_stop(state.server_ref)
    :ok
  end

  ## Private Functions

  defp request_loop(server_ref, handler) do
    case Native.receive_request(server_ref) do
      {:ok, request} ->
        # Handle request (catches errors to prevent worker crash)
        try do
          handler.(request)
        rescue
          e ->
            # Log error and send 500 response
            IO.warn("Request handler crashed: #{Exception.format(:error, e, __STACKTRACE__)}")

            Response.send(request, 500, [], "Internal Server Error")
        end

        # Continue loop
        request_loop(server_ref, handler)

      {:error, _reason} ->
        # Server shut down
        :ok
    end
  end
end
