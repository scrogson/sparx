# Echo server example for Sparx
#
# Run with: mix run examples/echo_server.exs
#
# Then test with:
#   curl -X POST http://localhost:7779 -d "Hello, Sparx!"
#   curl -X POST http://localhost:7779 -H "Content-Type: application/json" -d '{"message":"test"}'

require Logger

defmodule EchoServer do
  @moduledoc """
  HTTP server that echoes back the request body.
  Demonstrates request body streaming.
  """

  require Logger

  def handle_request(request) do
    Logger.info("Received request")

    # Read the entire request body
    case Sparx.Request.read_body(request) do
      {:ok, body} ->
        Logger.info("Received body: #{byte_size(body)} bytes")

        # Echo back the body
        Sparx.Response.send(
          request,
          200,
          [{"content-type", "text/plain"}],
          "You sent: #{body}"
        )

      {:error, reason} ->
        Logger.error("Failed to read body: #{inspect(reason)}")
        Sparx.Response.send_text(request, 500, "Failed to read body")
    end
  end
end

# Start the server
Logger.info("Starting Sparx echo server on http://localhost:7779")

{:ok, _server} =
  Sparx.start_link(
    handler: &EchoServer.handle_request/1,
    port: 7779
  )

Logger.info("Server started! Press Ctrl+C to stop")
Logger.info("Try: curl -X POST http://localhost:7779 -d 'Hello, Sparx!'")

# Keep the process alive
Process.sleep(:infinity)
