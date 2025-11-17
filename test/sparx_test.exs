defmodule SparxTest do
  use ExUnit.Case
  doctest Sparx

  test "starts and stops server" do
    handler = fn request ->
      Sparx.Response.send_text(request, 200, "test")
    end

    {:ok, server} = Sparx.start_link(handler: handler, port: 0)
    assert Process.alive?(server)

    :ok = Sparx.stop(server)
    refute Process.alive?(server)
  end
end
