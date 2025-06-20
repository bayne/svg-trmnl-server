mod api;
mod context;
mod display;
mod dto;

use crate::api::{AppServerConfig, Clock, app};
use anyhow::Result;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::signal;
use tracing::info;
use tracing::level_filters::LevelFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::DEBUG)
        .init();

    let server_config = AppServerConfig {
        listen: env::args()
            .find_map(|arg| arg.strip_prefix("--listen=").map(String::from))
            .unwrap_or("0.0.0.0:9080".to_string()),
        config_path: env::args()
            .find_map(|arg| arg.strip_prefix("--config-path=").map(PathBuf::from))
            .unwrap_or("config/config.toml".into()),
    };
    struct SystemClock;
    impl Clock for SystemClock {
        fn now(&self) -> SystemTime {
            SystemTime::now()
        }
    }

    let signal = shutdown_signal();

    let listen = server_config.listen.clone();
    let clock = Arc::new(SystemClock);
    let app = app(server_config, clock)?;

    let listener = tokio::net::TcpListener::bind(listen).await?;
    info!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(signal)
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
