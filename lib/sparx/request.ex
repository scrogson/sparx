defmodule Sparx.Request do
  @moduledoc """
  HTTP request handling and body streaming.
  """

  alias Sparx.Native

  defmodule Metadata do
    @moduledoc """
    Request metadata returned from Rust.

    ## Fields

      * `:method` - HTTP method as a string (e.g., "GET", "POST")
      * `:path` - Request path
      * `:query` - Query string (optional)
      * `:version` - HTTP version string (e.g., "HTTP/1.1")
      * `:headers` - List of {name, value} tuples

    """
    @type t :: %__MODULE__{
            method: String.t(),
            path: String.t(),
            query: String.t() | nil,
            version: String.t(),
            headers: [{String.t(), String.t()}]
          }

    defstruct [:method, :path, :query, :version, :headers]
  end

  @type request_handle :: reference()

  @doc """
  Read a chunk from the request body.

  Returns `{:ok, binary()}` if a chunk is available, `:eof` when the body is fully consumed,
  or `{:error, reason}` if an error occurs.

  This is a low-level async function. For most use cases, consider using `body_stream/2` instead.

  ## Examples

      {:ok, chunk} = Sparx.Request.read_chunk(request)
      :eof = Sparx.Request.read_chunk(request)

  """
  @spec read_chunk(request_handle()) :: {:ok, binary()} | :eof | {:error, term()}
  def read_chunk(request_handle) do
    case Native.read_chunk(request_handle) do
      {:ok, <<>>} -> :eof
      {:ok, chunk} -> {:ok, chunk}
      {:error, _} = error -> error
    end
  end

  @doc """
  Create a stream for reading the request body.

  Returns a `Stream` that yields chunks of the request body on demand.

  ## Options

    * `:timeout` - Timeout for reading each chunk in milliseconds (default: 30,000)

  ## Examples

      Sparx.Request.body_stream(request)
      |> Stream.each(&process_chunk/1)
      |> Stream.run()

      # Or collect all chunks
      chunks =
        Sparx.Request.body_stream(request)
        |> Enum.to_list()

      body = IO.iodata_to_binary(chunks)

  """
  @spec body_stream(request_handle(), keyword()) :: Enumerable.t()
  def body_stream(request_handle, opts \\ []) do
    _timeout = Keyword.get(opts, :timeout, 30_000)

    Stream.resource(
      fn -> request_handle end,
      fn handle ->
        case read_chunk(handle) do
          {:ok, chunk} ->
            {[chunk], handle}

          :eof ->
            {:halt, handle}

          {:error, reason} ->
            raise "Request body read error: #{inspect(reason)}"
        end
      end,
      fn _handle -> :ok end
    )
  end

  @doc """
  Read the entire request body into a binary.

  This function reads all chunks from the request body and concatenates them.
  Use this for small requests. For large requests, use `body_stream/2` instead.

  ## Options

    * `:timeout` - Timeout for reading the entire body in milliseconds (default: 30,000)
    * `:max_size` - Maximum body size in bytes (default: 10MB)

  ## Examples

      {:ok, body} = Sparx.Request.read_body(request)
      {:error, :too_large} = Sparx.Request.read_body(request, max_size: 1024)

  """
  @spec read_body(request_handle(), keyword()) :: {:ok, binary()} | {:error, term()}
  def read_body(request_handle, opts \\ []) do
    max_size = Keyword.get(opts, :max_size, 10 * 1024 * 1024)
    timeout = Keyword.get(opts, :timeout, 30_000)

    chunks =
      request_handle
      |> body_stream(timeout: timeout)
      |> Enum.reduce_while({[], 0}, fn chunk, {acc, size} ->
        new_size = size + byte_size(chunk)

        if new_size > max_size do
          {:halt, {:error, :too_large}}
        else
          {:cont, {[chunk | acc], new_size}}
        end
      end)

    case chunks do
      {:error, reason} ->
        {:error, reason}

      {chunk_list, _size} ->
        body =
          chunk_list
          |> Enum.reverse()
          |> IO.iodata_to_binary()

        {:ok, body}
    end
  end
end
