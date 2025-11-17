defmodule SparxTest do
  use ExUnit.Case
  doctest Sparx

  test "greets the world" do
    assert Sparx.hello() == :world
  end
end
