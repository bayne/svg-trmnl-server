use crate::api::display::preview::{preview_handler, preview_websocket_handler};
use crate::api::display::{display_handler, image_handler};
use crate::api::setup::{setup_handler, setup_image_handler};
use crate::display::DisplayRenderer;
use anyhow::{Context, Error, Result};
use axum::Router;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use config::Config;
use serde::Deserialize;
use std::fmt::Formatter;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::info;

mod display;
pub(crate) mod setup;

#[macro_export]
macro_rules! bad_request { ($($arg:tt)+) => { AppError::ValidationError(format!($($arg)+)) }; }
#[macro_export]
macro_rules! unauthorized { ($($arg:tt)+) => { AppError::AuthorizationError(format!($($arg)+)) }; }
#[macro_export]
macro_rules! forbidden { ($($arg:tt)+) => { AppError::AuthenticationError(format!($($arg)+)) }; }

#[derive(Clone, Deserialize)]
pub struct AppServerConfig {
    pub listen: String,
    pub config_path: PathBuf,
}

#[derive(Clone, Deserialize)]
pub struct AppDeviceConfig {
    pub mac_address: String,
    pub friendly_id: String,
    pub api_key: String,
    pub setup_expiry: String,
}

impl AppDeviceConfig {
    pub fn get_template(&self, timestamp: SystemTime) -> String {
        info!("{:?}", timestamp);
        "test.svg.jinja".to_string()
    }
}

#[derive(Clone, Deserialize)]
pub struct AppConfig {
    pub devices: Option<Vec<AppDeviceConfig>>,
    pub base_url: String,
    pub setup_image_path: String,
    pub display_image_timeout: u64,
    pub templates_path: PathBuf,
    pub fonts_path: PathBuf,
    pub default_context_path: PathBuf,
}

impl AppConfig {
    pub fn get_device_by_mac(&self, mac: &str) -> Option<&AppDeviceConfig> {
        self.devices
            .as_ref()?
            .iter()
            .find(|device| device.mac_address == mac)
    }

    pub fn get_device_by_friendly_id(&self, friendly_id: &str) -> Option<&AppDeviceConfig> {
        self.devices
            .as_ref()?
            .iter()
            .find(|device| device.friendly_id == friendly_id)
    }

    pub fn get_device_by_api_key(&self, api_key: &str) -> Option<&AppDeviceConfig> {
        self.devices
            .as_ref()?
            .iter()
            .find(|device| device.api_key == api_key)
    }
}

#[derive(Clone)]
pub struct AppState {
    pub server_config: AppServerConfig,
    pub clock: Arc<dyn Clock + Sync + Send>,
}

impl AppState {
    pub fn config(&self) -> Result<AppConfig> {
        let AppState {
            server_config: AppServerConfig { config_path, .. },
            ..
        } = self;
        Ok(Config::builder()
            .add_source(config::File::from(config_path.clone()))
            .add_source(config::Environment::with_prefix("TRMNL_SERVER"))
            .build()
            .context("Failed to load config")?
            .try_deserialize()?)
    }

    pub fn display_renderer(&self) -> Result<DisplayRenderer> {
        let config = self.config()?;
        Ok(DisplayRenderer::new(
            config.fonts_path,
            config.templates_path,
        )?)
    }

    pub fn get_device_config_by_friendly_id(&self, friendly_id: &str) -> Result<AppDeviceConfig> {
        let device_config = self
            .config()?
            .get_device_by_friendly_id(friendly_id)
            .context("Failed to get device config")?
            .clone();
        Ok(device_config)
    }

    pub fn get_device_config_by_api_key(&self, api_key: &str) -> Result<AppDeviceConfig> {
        let device_config = self
            .config()?
            .get_device_by_api_key(&api_key)
            .context("Failed to get device config")?
            .clone();
        Ok(device_config)
    }
}

#[derive(Debug)]
pub enum AppError {
    ValidationError(String),
    AuthenticationError(String),
    AuthorizationError(String),
    UnexpectedError(Error),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            AppError::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
            AppError::AuthorizationError(msg) => write!(f, "Authorization error: {}", msg),
            AppError::UnexpectedError(msg) => write!(f, "Unexpected error: {}", msg),
        }
    }
}

impl From<Error> for AppError {
    fn from(value: Error) -> Self {
        if value.is::<AppError>() {
            value.downcast().unwrap()
        } else {
            AppError::UnexpectedError(value)
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let msg = self.to_string();
        let status_code = match self {
            AppError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AppError::AuthenticationError(_) => StatusCode::FORBIDDEN,
            AppError::AuthorizationError(_) => StatusCode::UNAUTHORIZED,
            AppError::UnexpectedError(error) => {
                tracing::error!("Unexpected error: {:?}", error);
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        tracing::error!("{}", msg);
        (status_code, msg).into_response()
    }
}

pub trait Clock {
    fn now(&self) -> SystemTime;
}

pub fn app(server_config: AppServerConfig, clock: Arc<dyn Clock + Sync + Send>) -> Result<Router> {
    let state = AppState {
        server_config,
        clock,
    };

    let app = Router::new()
        .route("/api/setup/", get(setup_handler))
        .route("/api/display", get(display_handler))
        .route("/api/log", post(logs_handler))
        .route("/display/preview", get(preview_handler))
        .route("/display/preview/ws", get(preview_websocket_handler))
        .route("/setup_image.bmp", get(setup_image_handler))
        .route("/display/{filename}", get(image_handler))
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()))
        .with_state(state);
    Ok(app)
}

async fn logs_handler(input: String) -> impl IntoResponse {
    info!(input);
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::dto::ApiSetupResponse;
    use axum_test::TestServer;
    use serde_json::json;
    use std::fs;
    use std::io::Write;
    use std::sync::Once;
    use std::time::Duration;
    use tempfile::{Builder, NamedTempFile};
    use tracing_subscriber::filter::LevelFilter;

    static INIT: Once = Once::new();

    fn new_test_app() -> (TestServer, NamedTempFile) {
        INIT.call_once(|| {
            let subscriber = tracing_subscriber::fmt()
                .with_max_level(LevelFilter::DEBUG)
                .with_test_writer()
                .finish();
            tracing::subscriber::set_global_default(subscriber).expect("failed setting subscriber");
        });

        let mut config = Builder::new()
            .suffix(".toml")
            .tempfile()
            .expect("Failed to create temp file");
        write!(
            config,
            r#"
            setup_image_path = "src/display/blank.bmp"
            base_url = "http://example.localhost"
            display_image_timeout = 60
            templates_path = "templates"
            default_context_path = "templates/default.json"
            fonts_path = "fonts"

            [[devices]]
            mac_address = "fake_mac_address"
            friendly_id = "fake_friendly_id"
            api_key = "fake_api_key"
            setup_expiry = "9999-01-01T00:00:00Z"
            
            [[devices]]
            mac_address = "fake_mac_address_expired_setup"
            friendly_id = "fake_friendly_id_expired"
            api_key = "fake_api_key_expired"
            setup_expiry = "2000-01-01T00:00:00Z"
        "#
        )
        .expect("Failed to write config");

        let config_path = config.path().to_path_buf();
        let server_config = AppServerConfig {
            listen: "0.0.0.0:9080".to_string(),
            config_path,
        };

        struct FakeClock;
        impl Clock for FakeClock {
            fn now(&self) -> SystemTime {
                SystemTime::UNIX_EPOCH + Duration::from_secs(1234567890)
            }
        }

        let clock = Arc::new(FakeClock);

        let app = app(server_config, clock).unwrap();
        (
            TestServer::builder()
                .expect_success_by_default()
                .mock_transport()
                .build(app)
                .unwrap(),
            config,
        )
    }

    #[tokio::test]
    async fn it_should_succeed_setup_request() {
        let (app, _temp_files) = new_test_app();
        let response = app
            .get("/api/setup/")
            .add_header("ID", "fake_mac_address")
            .add_header("FW-Version", "test")
            .await;

        response.assert_json(&ApiSetupResponse {
            status: 200,
            api_key: Some("fake_api_key".to_string()),
            friendly_id: Some("fake_friendly_id".to_string()),
            image_url: Some("http://example.localhost/setup_image.bmp".to_string()),
            message: "Success".to_string(),
        });
    }

    #[tokio::test]
    async fn it_should_error_setup_request_with_invalid_mac() {
        let (app, _temp_files) = new_test_app();

        let response = app
            .get("/api/setup/")
            .add_header("ID", "invalid_mac_address")
            .add_header("FW-Version", "test")
            .await;
        response.assert_json(&json!({
            "api_key": null,
            "friendly_id": null,
            "image_url": "http://example.localhost/setup_image.bmp",
            "message": "No device config found for MAC=invalid_mac_address",
            "status": 404
        }));
    }

    #[tokio::test]
    async fn it_should_error_forbidden_setup_request_with_expired_setup() {
        let (app, _temp_files) = new_test_app();

        let response = app
            .get("/api/setup/")
            .add_header("ID", "fake_mac_address_expired_setup")
            .add_header("FW-Version", "test")
            .expect_failure()
            .await;
        response.assert_status_forbidden();
        response.assert_text_contains("Authentication error: Attempted setup after expiry: friendly_id=fake_friendly_id_expired setup_expiry=2000-01-01 00:00:00 +00:00");
    }

    #[tokio::test]
    async fn it_should_return_setup_image() {
        let (app, _temp_files) = new_test_app();

        let response = app
            .get("/setup_image.bmp")
            .add_header("ID", "fake_mac_address")
            .add_header("FW-Version", "test")
            .await;
        let blank_bytes = include_bytes!("../display/blank.bmp");
        assert_eq!(blank_bytes, response.as_bytes().iter().as_ref())
    }

    #[tokio::test]
    async fn it_should_return_display_api_response() {
        let (app, _temp_files) = new_test_app();

        let response = app
            .get("/api/display")
            .add_header("Access-Token", "fake_api_key")
            .add_header("FW-Version", "fake_FW-Version")
            .add_header("ID", "fake_ID")
            .add_header("Refresh-Rate", "fake_Refresh-Rate")
            .add_header("Battery-Voltage", "fake_Battery-Voltage")
            .add_header("RSSI", "fake_RSSI")
            .add_header("Display-Width", "fake_Display-Width")
            .add_header("Display-Height", "fake_Display-Height")
            .add_header("Special-Function", "fake_Special-Function")
            .await;

        response.assert_json(&json!({
            "filename": "39bf95b5a576efb89503cf3ed2bafb5a8fb7ac8f12db7bf9164442abb7fbacdd.bmp",
            "image_url": "http://example.localhost/display/39bf95b5a576efb89503cf3ed2bafb5a8fb7ac8f12db7bf9164442abb7fbacdd.bmp?friendly-id=fake_friendly_id&timestamp=1234567890",
            "image_url_timeout": 60,
            "refresh_rate": 3600,
            "special_function": "sleep",
            "status": 0,
        }))
    }

    #[tokio::test]
    async fn it_should_return_display_response() {
        let (app, _temp_files) = new_test_app();
        let timestamp: u64 = 1234567890;
        let expected_filename =
            "39bf95b5a576efb89503cf3ed2bafb5a8fb7ac8f12db7bf9164442abb7fbacdd.bmp";

        let response = app
            .get(&format!("/display/{}", expected_filename))
            .add_query_param("friendly-id", "fake_friendly_id")
            .add_query_param("timestamp", timestamp.to_string())
            .await;
        let expected = fs::read("test.bmp").unwrap();
        assert_eq!(expected, response.as_bytes().iter().as_ref())
    }
}
