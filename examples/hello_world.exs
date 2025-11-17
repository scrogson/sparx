# Hello World example for Sparx
#
# Run with: mix run examples/hello_world.exs
#
# Then visit http://localhost:4500 in your browser or use curl:
#   curl http://localhost:4500
#   curl http://localhost:4500/hello
#   curl http://localhost:4500/json
require Logger

defmodule HelloWorld do
  @moduledoc """
  Simple HTTP server example showing basic routing and responses.
  """

  require Logger

  def handle_request(request) do
    # For now, we'll need to access the metadata through the request
    # In a future version, we could make this more ergonomic
    Logger.info("Received request")

    # Simple text response
    Sparx.Response.send_text(request, 200, "Hello, World from Sparx!")
  end
end

# Start the server
Logger.info("Starting Sparx server on http://localhost:4500")

{:ok, _server} =
  Sparx.start_link(
    handler: &HelloWorld.handle_request/1,
    port: 4500
  )

Logger.info("Server started! Press Ctrl+C to stop")
Logger.info("Try: curl http://localhost:4500")

# Keep the process alive
Process.sleep(:infinity)
