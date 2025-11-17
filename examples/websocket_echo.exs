# WebSocket Echo Server Example
#
# This example demonstrates WebSocket support in Sparx.
# It upgrades HTTP requests to WebSocket connections and echoes back
# any messages received.
#
# Usage:
#   mix run examples/websocket_echo.exs
#
# Test with wscat:
#   npm install -g wscat
#   wscat -c ws://localhost:7779
#
# Or with websocat:
#   websocat ws://localhost:7779

defmodule WebSocketEcho do
  alias Sparx.Native

  def handle_request(request) do
    IO.puts("Received request: #{request.method} #{request.path}")

    # Check if this is a WebSocket upgrade request
    if is_websocket_upgrade?(request) do
      IO.puts("Upgrading to WebSocket...")

      case Native.upgrade_websocket(request.handle) do
        {:ok, ws_handle} ->
          IO.puts("WebSocket connection established!")
          handle_websocket(ws_handle)

        {:error, reason} ->
          IO.puts("WebSocket upgrade failed: #{reason}")
          Sparx.Response.send_text(request, 400, "Bad Request: #{reason}")
      end
    else
      # Regular HTTP response
      Sparx.Response.send_text(request, 200, """
      WebSocket Echo Server

      Connect using a WebSocket client:
        wscat -c ws://localhost:7779
        websocat ws://localhost:7779
      """)
    end
  end

  defp is_websocket_upgrade?(request) do
    upgrade = get_header(request, "upgrade")
    connection = get_header(request, "connection")

    upgrade && String.downcase(upgrade) == "websocket" &&
      connection && String.contains?(String.downcase(connection), "upgrade")
  end

  defp get_header(request, name) do
    Enum.find_value(request.headers, fn {k, v} ->
      if String.downcase(k) == name, do: v
    end)
  end

  defp handle_websocket(ws_handle) do
    # WebSocket echo loop
    case Native.ws_recv(ws_handle) do
      {:ok, {:text, data}} ->
        message = IO.iodata_to_binary(data)
        IO.puts("Received text: #{message}")
        Native.ws_send_text(ws_handle, "Echo: #{message}")
        handle_websocket(ws_handle)

      {:ok, {:binary, data}} ->
        IO.puts("Received binary: #{byte_size(data)} bytes")
        Native.ws_send_binary(ws_handle, data)
        handle_websocket(ws_handle)

      {:ok, {:ping, data}} ->
        IO.puts("Received ping, sending pong")
        # tungstenite handles ping/pong automatically, but we log it
        handle_websocket(ws_handle)

      {:ok, {:pong, _data}} ->
        IO.puts("Received pong")
        handle_websocket(ws_handle)

      {:error, :close} ->
        IO.puts("WebSocket connection closed by client")
        Native.ws_close(ws_handle)

      {:error, :closed} ->
        IO.puts("WebSocket connection closed")

      {:error, reason} ->
        IO.puts("WebSocket error: #{inspect(reason)}")
        Native.ws_close(ws_handle)
    end
  end
end

# Start the server
{:ok, _server} =
  Sparx.start_link(
    handler: &WebSocketEcho.handle_request/1,
    port: 7779
  )

IO.puts("""

WebSocket Echo Server is running!

  Connect with wscat:    wscat -c ws://localhost:7779
  Connect with websocat: websocat ws://localhost:7779
  Visit in browser:      http://localhost:7779

Press Ctrl+C to stop.
""")

# Keep the process alive
Process.sleep(:infinity)
