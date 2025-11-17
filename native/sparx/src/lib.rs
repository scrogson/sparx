#![deny(warnings)]

use rustler::{Env, Term};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod atoms;
mod config;
mod request;
mod response;
mod server;

fn load(_env: Env, load_info: Term) -> bool {
    // Configure tracing with SPARX_LOG env variable
    let env_filter = std::env::var("SPARX_LOG")
        .map(|s| EnvFilter::new(&s))
        .unwrap_or_else(|_| EnvFilter::new("warn"));

    tracing_subscriber::registry()
        .with(fmt::layer().with_filter(env_filter))
        .init();

    // Configure Tokio runtime for async tasks
    if let Ok(config) = load_info.decode::<rustler::runtime::RuntimeConfig>() {
        rustler::runtime::configure(config).ok();
    }

    true
}

rustler::init!("Elixir.Sparx.Native", load = load);
