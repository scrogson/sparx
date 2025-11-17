defmodule Sparx.MixProject do
  use Mix.Project

  def project do
    [
      app: :sparx,
      version: "0.1.0",
      elixir: "~> 1.16",
      start_permanent: Mix.env() == :prod,
      deps: deps(),

      # Docs
      name: "Sparx",
      description: "High-performance HTTP server for Elixir powered by Rust NIFs and Hyper",
      source_url: "https://github.com/scrogson/sparx",
      homepage_url: "https://github.com/scrogson/sparx",
      docs: docs(),

      # Package
      package: package(),
      licenses: ["MIT"],

      # Test coverage
      test_coverage: [tool: ExCoveralls],
      preferred_cli_env: [
        coveralls: :test,
        "coveralls.detail": :test,
        "coveralls.post": :test,
        "coveralls.html": :test,
        "coveralls.github": :test
      ]
    ]
  end

  def application do
    [
      extra_applications: [:logger]
    ]
  end

  defp deps do
    [
      {:rustler,
       github: "rusterlium/rustler", branch: "async-nifs", sparse: "rustler_mix", runtime: false},
      {:telemetry, "~> 1.0"},
      {:gen_stage, "~> 1.0"},
      {:ex_doc, "~> 0.34", only: :dev, runtime: false},
      {:excoveralls, "~> 0.18", only: :test},
      {:credo, "~> 1.7", only: [:dev, :test], runtime: false}
    ]
  end

  defp package do
    [
      name: "sparx",
      files: ~w(lib native priv .formatter.exs mix.exs README.md LICENSE CLAUDE.md),
      licenses: ["MIT"],
      links: %{
        "GitHub" => "https://github.com/scrogson/sparx",
        "Docs" => "https://hexdocs.pm/sparx"
      },
      maintainers: ["Sonny Scroggin"]
    ]
  end

  defp docs do
    [
      main: "readme",
      extras: [
        "README.md",
        "CLAUDE.md"
      ],
      groups_for_modules: [
        "Core API": [
          Sparx,
          Sparx.Server,
          Sparx.Request,
          Sparx.Response
        ],
        WebSocket: [
          Sparx.WebSocket
        ],
        Configuration: [
          Sparx.Config
        ]
      ]
    ]
  end
end
