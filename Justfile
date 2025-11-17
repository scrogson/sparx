# Justfile for Sparx development

# Default recipe - show available commands
default:
    @just --list

# Format all code (Elixir + Rust)
format: format-elixir format-rust

# Format Elixir code
format-elixir:
    mix format

# Format Rust code
format-rust:
    cd native/sparx && cargo fmt

# Check formatting for all code
check-format: check-format-elixir check-format-rust

# Check Elixir formatting
check-format-elixir:
    mix format --check-formatted

# Check Rust formatting
check-format-rust:
    cd native/sparx && cargo fmt --check

# Lint all code
lint: lint-elixir lint-rust

# Lint all code (strict mode - treat warnings as errors)
lint-strict: lint-elixir
    cd native/sparx && cargo clippy -- -D warnings

# Lint Elixir code with Credo
lint-elixir:
    mix credo

# Lint Rust code with Clippy
lint-rust:
    cd native/sparx && cargo clippy

# Run all tests
test:
    mix test

# Run tests with coverage
test-coverage:
    mix coveralls

# Run tests with coverage and open HTML report
test-coverage-html:
    mix coveralls.html && open cover/excoveralls.html

# Full CI check (format, lint, test)
ci: check-format lint test

# Full CI check with strict linting
ci-strict: check-format lint-strict test

# Clean build artifacts
clean:
    mix clean
    cd native/sparx && cargo clean

# Build the project
build:
    mix compile

# Build release
build-release:
    MIX_ENV=prod mix compile
