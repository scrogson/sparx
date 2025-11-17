# HTTP/2 Demo for Sparx
#
# Run with: mix run examples/http2_demo.exs
#
# Then test with:
#   # HTTP/1.1 request
#   curl -v http://localhost:7779
#
#   # HTTP/2 request (h2c - HTTP/2 without TLS)
#   curl --http2-prior-knowledge -v http://localhost:7779
#
# The server automatically supports both HTTP/1.1 and HTTP/2!

require Logger

defmodule Http2Demo do
  @moduledoc """
  Demonstrates HTTP/2 support in Sparx.
  The same handler works for both HTTP/1.1 and HTTP/2 requests.
  """

  require Logger

  def handle_request(request) do
    Logger.info("Received request")

    # Simple response that works with both HTTP/1.1 and HTTP/2
    Sparx.Response.send_json(request, 200, %{
      message: "Hello from Sparx!",
      features: ["HTTP/1.1", "HTTP/2", "Streaming", "Low-latency"],
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    })
  end
end

# Start the server
Logger.info("Starting Sparx HTTP/2 demo server on http://localhost:7779")
Logger.info("This server supports both HTTP/1.1 and HTTP/2 automatically!")

{:ok, _server} =
  Sparx.start_link(
    handler: &Http2Demo.handle_request/1,
    port: 7779
  )

Logger.info("Server started! Try these commands:")
Logger.info("  HTTP/1.1: curl -v http://localhost:7779")
Logger.info("  HTTP/2:   curl --http2-prior-knowledge -v http://localhost:7779")

# Keep the process alive
Process.sleep(:infinity)
