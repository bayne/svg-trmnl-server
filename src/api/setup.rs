use crate::api::{AppError, AppState};
use crate::dto::ApiSetupResponse;
use crate::{bad_request, forbidden};
use anyhow::{Context, Result};
use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Local};
use std::fs::File;
use std::io::Read;
use tracing::info;

pub async fn setup_image_handler(State(app_state): State<AppState>) -> Result<Response, AppError> {
    let config = app_state.config()?;
    let setup_image_path = config.setup_image_path.clone();
    let mut file = File::open(setup_image_path)
        .context(config.setup_image_path.clone())
        .expect("Failed to open setup image");
    let mut image = vec![];
    file.read_to_end(&mut image)
        .expect("Failed to read setup image");

    let mut res = Body::from(image).into_response();
    res.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/bmp"));
    Ok(res)
}

pub async fn setup_handler(
    headers: HeaderMap,
    State(app_state): State<AppState>,
) -> Result<Json<ApiSetupResponse>, AppError> {
    let config = app_state.config()?;
    let mac = headers
        .get("ID")
        .and_then(|v| v.to_str().ok())
        .context(bad_request!("missing ID header"))?
        .to_string();
    let fw_version = headers
        .get("FW-Version")
        .and_then(|v| v.to_str().ok())
        .context(bad_request!("missing FW-Version header"))?
        .to_string();

    info!("Setup request from MAC={} FW-Version={}", mac, fw_version);

    let base_url = app_state.config()?.base_url;

    if let Some(device_config) = config.get_device_by_mac(&mac) {
        info!("Found device config for MAC={}", mac);
        let setup_expiry = DateTime::parse_from_rfc3339(&device_config.setup_expiry)
            .context(bad_request!("invalid setup expiry"))?;
        if setup_expiry < Local::now() {
            return Err(forbidden!(
                "Attempted setup after expiry: friendly_id={} setup_expiry={}",
                device_config.friendly_id,
                setup_expiry
            ));
        }

        let resp = ApiSetupResponse {
            status: 200,
            api_key: Some(device_config.api_key.clone()),
            friendly_id: Some(device_config.friendly_id.clone()),
            image_url: Some(base_url + "/setup_image.bmp"),
            message: "Success".to_string(),
        };

        Ok(Json(resp))
    } else {
        let resp = ApiSetupResponse {
            status: 404,
            api_key: None,
            friendly_id: None,
            image_url: Some(base_url + "/setup_image.bmp"),
            message: format!("No device config found for MAC={}", mac),
        };

        Ok(Json(resp))
    }
}
