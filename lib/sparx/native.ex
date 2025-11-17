defmodule Sparx.Native do
  @moduledoc false
  # NIF stubs - actual implementations are in Rust

  use Rustler, otp_app: :sparx, crate: "sparx"

  # Server management
  def server_start(_config), do: err()
  def server_stop(_server_ref), do: err()
  def receive_request(_server_ref), do: err()

  # Request streaming
  def read_chunk(_request_handle), do: err()

  # Response streaming
  def send_status(_request_handle, _status), do: err()
  def send_header(_request_handle, _name, _value), do: err()
  def write_chunk(_request_handle, _data), do: err()
  def finish(_request_handle), do: err()

  defp err, do: :erlang.nif_error(:nif_not_loaded)
end
