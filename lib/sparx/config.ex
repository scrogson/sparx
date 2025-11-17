defmodule Sparx.Config do
  @moduledoc """
  Server configuration struct.

  ## Fields

    * `:host` - Host to bind to (e.g., "127.0.0.1", "0.0.0.0")
    * `:port` - Port to listen on (default: 7779)
    * `:max_connections` - Maximum number of concurrent connections (default: 100,000)
    * `:request_timeout_ms` - Request timeout in milliseconds (default: 30,000)
    * `:keep_alive_timeout_ms` - Keep-alive timeout in milliseconds (default: 60,000)

  ## Examples

      iex> config = %Sparx.Config{}
      %Sparx.Config{host: "127.0.0.1", port: 7779, ...}

      iex> config = %Sparx.Config{port: 8080}
      %Sparx.Config{host: "127.0.0.1", port: 8080, ...}

  """

  @type t :: %__MODULE__{
          host: String.t(),
          port: :inet.port_number(),
          max_connections: pos_integer(),
          request_timeout_ms: pos_integer(),
          keep_alive_timeout_ms: pos_integer()
        }

  defstruct host: "127.0.0.1",
            port: 7779,
            max_connections: 100_000,
            request_timeout_ms: 30_000,
            keep_alive_timeout_ms: 60_000
end
