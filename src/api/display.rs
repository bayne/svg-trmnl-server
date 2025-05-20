use crate::api::{AppError, AppState};
use crate::display::generate_filename;
use crate::dto::{ApiDisplayResponse, SpecialFunction};
use crate::{bad_request, unauthorized};
use anyhow::Context;
use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use minijinja::context;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::Url;

#[allow(dead_code)]
struct AppDisplayRequestHeaders {
    mac_address: String,
    api_key: String,
    refresh_rate: String,
    battery_voltage: String,
    fw_version: String,
    rssi: String,
    special_function: Option<String>,
}

trait RequiredHeader {
    fn get_required_header(&self, header: &str) -> Result<String, AppError>;
}

impl RequiredHeader for HeaderMap {
    fn get_required_header(&self, header: &str) -> Result<String, AppError> {
        Ok(self
            .get(header)
            .and_then(|v| v.to_str().ok())
            .context(bad_request!("missing {} header", header))?
            .to_string())
    }
}

impl TryInto<AppDisplayRequestHeaders> for HeaderMap {
    type Error = AppError;

    fn try_into(self) -> Result<AppDisplayRequestHeaders, Self::Error> {
        Ok(AppDisplayRequestHeaders {
            api_key: self.get_required_header("Access-Token")?,
            mac_address: self.get_required_header("ID")?,
            refresh_rate: self.get_required_header("Refresh-Rate")?,
            battery_voltage: self.get_required_header("Battery-Voltage")?,
            fw_version: self.get_required_header("FW-Version")?,
            rssi: self.get_required_header("RSSI")?,
            special_function: self
                .get("Special-Function")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.to_string()),
        })
    }
}

pub async fn handle_display(
    headers: HeaderMap,
    State(app_state): State<AppState>,
) -> Result<Json<ApiDisplayResponse>, AppError> {
    let headers: AppDisplayRequestHeaders = headers.try_into()?;
    let device_config = app_state.get_device_config_by_api_key(&headers.api_key)?;
    let base_url = app_state.config()?.base_url;
    let now = app_state.clock.now();

    let filename = generate_filename(headers.api_key, now)?;
    let mut image_url = Url::parse(&base_url).context(format!("invalid base url, {}", base_url))?;

    let timestamp = now
        .duration_since(UNIX_EPOCH)
        .context("failed to get elapsed time")?
        .as_secs();
    image_url
        .query_pairs_mut()
        .append_pair("friendly-id", device_config.friendly_id.as_str())
        .append_pair("timestamp", &timestamp.to_string());
    image_url.set_path(format!("/display/{}", filename).as_str());

    let image_url = image_url.to_string();

    let display_image_timeout = app_state.config()?.display_image_timeout;
    let resp = ApiDisplayResponse {
        error_detail: None,
        status: 0,
        image_url: Some(image_url),
        image_url_timeout: Some(display_image_timeout as i32),
        filename: Some(filename),
        refresh_rate: 3600,
        update_firmware: None,
        firmware_url: None,
        reset_firmware: None,
        special_function: SpecialFunction::Sleep,
        action: None,
    };

    Ok(Json(resp))
}

pub async fn handle_image(
    State(app_state): State<AppState>,
    Path(filename): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> anyhow::Result<Response, AppError> {
    let friendly_id = params
        .get("friendly-id")
        .ok_or(bad_request!("missing friendly-id query param"))?;
    let timestamp: u64 = params
        .get("timestamp")
        .ok_or(bad_request!("missing timestamp query param"))?
        .parse()
        .context(bad_request!("invalid timestamp query param"))?;

    let device_config = app_state.get_device_config_by_friendly_id(friendly_id)?;
    let timestamp = SystemTime::UNIX_EPOCH + Duration::from_secs(timestamp);
    if filename != generate_filename(device_config.api_key, timestamp)? {
        return Err(unauthorized!("invalid filename"));
    }

    let elapsed = app_state
        .clock
        .now()
        .duration_since(timestamp)
        .context("failed to get elapsed time")?;
    let display_image_timeout = app_state.config()?.display_image_timeout;
    if elapsed > Duration::from_secs(display_image_timeout) {
        return Err(unauthorized!("image expired"));
    }

    let device_config = app_state.get_device_config_by_friendly_id(friendly_id)?;
    let template = device_config.get_template(timestamp);
    let display_renderer = app_state.display_renderer()?;
    let image = display_renderer.render_jinja(&template, &context! {})?;
    let image = image.to_vec();
    let mut res = Body::from(image).into_response();
    res.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/bmp"));
    Ok(res)
}
