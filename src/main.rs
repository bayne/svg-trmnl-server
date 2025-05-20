mod api;
mod display;
mod dto;

use crate::api::{AppServerConfig, Clock, app};
use anyhow::Result;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("trace").init();

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

    let listen = server_config.listen.clone();
    let clock = Arc::new(SystemClock);
    let app = app(server_config, clock)?;

    // run it
    let listener = tokio::net::TcpListener::bind(listen).await?;
    info!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
