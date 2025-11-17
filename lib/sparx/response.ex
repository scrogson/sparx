defmodule Sparx.Response do
  @moduledoc """
  HTTP response streaming API.

  Provides functions for building and sending HTTP responses.
  """

  alias Sparx.Native

  @type request_handle :: reference()

  @doc """
  Send the HTTP status code for the response.

  Must be called first before sending headers or body.

  ## Examples

      :ok = Sparx.Response.send_status(request, 200)
      :ok = Sparx.Response.send_status(request, 404)

  """
  @spec send_status(request_handle(), 100..599) :: :ok | {:error, term()}
  def send_status(request_handle, status) when is_integer(status) and status >= 100 and status <= 599 do
    Native.send_status(request_handle, status)
  end

  @doc """
  Send a response header.

  Can be called multiple times to send multiple headers.
  Must be called after `send_status/2` and before writing body.

  ## Examples

      :ok = Sparx.Response.send_header(request, "content-type", "application/json")
      :ok = Sparx.Response.send_header(request, "x-custom-header", "value")

  """
  @spec send_header(request_handle(), String.t(), String.t()) :: :ok | {:error, term()}
  def send_header(request_handle, name, value) when is_binary(name) and is_binary(value) do
    Native.send_header(request_handle, name, value)
  end

  @doc """
  Write a chunk of the response body.

  Can be called multiple times to stream the response.

  ## Examples

      :ok = Sparx.Response.write_chunk(request, "Hello ")
      :ok = Sparx.Response.write_chunk(request, "World!")

  """
  @spec write_chunk(request_handle(), iodata()) :: :ok | {:error, term()}
  def write_chunk(request_handle, data) do
    binary_data = IO.iodata_to_binary(data)
    Native.write_chunk(request_handle, binary_data)
  end

  @doc """
  Finish the response.

  Must be called after all headers and body chunks have been sent.
  This signals to the client that the response is complete.

  ## Examples

      :ok = Sparx.Response.finish(request)

  """
  @spec finish(request_handle()) :: :ok | {:error, term()}
  def finish(request_handle) do
    Native.finish(request_handle)
  end

  @doc """
  Send a complete response (status, headers, and body) in one call.

  This is a convenience function for simple responses.

  ## Examples

      Sparx.Response.send(request, 200, [{"content-type", "text/plain"}], "Hello World!")

      Sparx.Response.send(request, 404, [], "Not Found")

  """
  @spec send(request_handle(), 100..599, [{String.t(), String.t()}], iodata()) ::
          :ok | {:error, term()}
  def send(request_handle, status, headers \\ [], body \\ "") do
    require Logger
    Logger.debug("Sending response: status=#{status}")

    with :ok <- send_status(request_handle, status),
         :ok <- send_headers(request_handle, headers),
         :ok <- write_chunk(request_handle, body),
         :ok <- finish(request_handle) do
      Logger.debug("Response sent successfully")
      :ok
    else
      error ->
        Logger.error("Failed to send response: #{inspect(error)}")
        error
    end
  end

  @doc """
  Send a JSON response.

  Automatically sets the content-type header to "application/json" and encodes the data.
  Requires Jason to be available.

  ## Examples

      Sparx.Response.send_json(request, 200, %{status: "ok", data: [1, 2, 3]})

  """
  @spec send_json(request_handle(), 100..599, term()) :: :ok | {:error, term()}
  def send_json(request_handle, status, data) do
    case Jason.encode(data) do
      {:ok, json} ->
        send(request_handle, status, [{"content-type", "application/json"}], json)

      {:error, reason} ->
        {:error, {:json_encode_error, reason}}
    end
  end

  @doc """
  Send a text response.

  Automatically sets the content-type header to "text/plain".

  ## Examples

      Sparx.Response.send_text(request, 200, "Hello, World!")

  """
  @spec send_text(request_handle(), 100..599, String.t()) :: :ok | {:error, term()}
  def send_text(request_handle, status, text) do
    send(request_handle, status, [{"content-type", "text/plain"}], text)
  end

  @doc """
  Send an HTML response.

  Automatically sets the content-type header to "text/html".

  ## Examples

      Sparx.Response.send_html(request, 200, "<h1>Hello, World!</h1>")

  """
  @spec send_html(request_handle(), 100..599, String.t()) :: :ok | {:error, term()}
  def send_html(request_handle, status, html) do
    send(request_handle, status, [{"content-type", "text/html; charset=utf-8"}], html)
  end

  # Private helper to send multiple headers
  defp send_headers(request_handle, headers) do
    Enum.reduce_while(headers, :ok, fn {name, value}, :ok ->
      case send_header(request_handle, name, value) do
        :ok -> {:cont, :ok}
        error -> {:halt, error}
      end
    end)
  end
end
