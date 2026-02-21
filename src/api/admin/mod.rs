use crate::api::{AppError, AppState};
use crate::display::DisplayRenderer;
use anyhow::Context;
use axum::Form;
use axum::extract::State;
use axum::response::{IntoResponse, Redirect, Response};
use chrono::{DateTime, Utc};
use minijinja::context;
use serde::Deserialize;
use std::fs;
use tracing::error;

/// GET /admin — list all configured devices
pub async fn admin_handler(
    State(app_state): State<AppState>,
) -> anyhow::Result<Response, AppError> {
    let config = app_state.config()?;
    let now: DateTime<Utc> = Utc::now();

    let devices: Vec<_> = config
        .devices
        .unwrap_or_default()
        .into_iter()
        .map(|d| {
            let setup_expired = DateTime::parse_from_rfc3339(&d.setup_expiry)
                .map(|expiry| expiry < now)
                .unwrap_or(true);
            let playlist: Vec<_> = d
                .playlist
                .iter()
                .map(|p| {
                    minijinja::Value::from_serialize(serde_json::json!({
                        "filename": p.filename,
                        "contexts": p.contexts,
                    }))
                })
                .collect();
            minijinja::Value::from_serialize(serde_json::json!({
                "friendly_id": d.friendly_id,
                "mac_address": d.mac_address,
                "api_key": d.api_key,
                "setup_expiry": d.setup_expiry,
                "setup_expired": setup_expired,
                "playlist": playlist,
            }))
        })
        .collect();

    let env = minijinja::Environment::new();
    let html = env
        .render_str(
            include_str!("index.html.jinja"),
            context! { devices },
        )
        .context("Failed to render admin template")?;

    Ok(Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(html.into())
        .context("Failed to build response")?)
}

/// GET /admin/devices/new — render the add-device form
pub async fn admin_new_device_handler(
    State(app_state): State<AppState>,
) -> anyhow::Result<Response, AppError> {
    let templates_path = app_state.config()?.templates_path;
    let templates: Vec<String> = DisplayRenderer::templates(templates_path)
        .unwrap_or_default()
        .into_iter()
        .map(|t| t.name)
        .collect();

    let env = minijinja::Environment::new();
    let html = env
        .render_str(
            include_str!("new_device.html.jinja"),
            context! {
                templates,
                friendly_id => "",
                mac_address => "",
                api_key => "",
                setup_expiry => "",
                flash_error => "",
            },
        )
        .context("Failed to render new device template")?;

    Ok(Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(html.into())
        .context("Failed to build response")?)
}

#[derive(Deserialize, Debug)]
pub struct NewDeviceForm {
    pub friendly_id: String,
    pub mac_address: String,
    pub api_key: String,
    pub setup_expiry: String, // "YYYY-MM-DDTHH:MM" from datetime-local input
    #[serde(default)]
    pub playlist_filename: Vec<String>,
    #[serde(default)]
    pub playlist_contexts: Vec<String>,
}

/// POST /admin/devices — create a new device and persist to config file
pub async fn admin_create_device_handler(
    State(app_state): State<AppState>,
    Form(form): Form<NewDeviceForm>,
) -> anyhow::Result<Response, AppError> {
    // Validate required fields
    let friendly_id = form.friendly_id.trim().to_string();
    let mac_address = form.mac_address.trim().to_string();
    let api_key = form.api_key.trim().to_string();
    let setup_expiry_raw = form.setup_expiry.trim().to_string();

    let render_error = |msg: &str, templates: Vec<String>| -> anyhow::Result<Response> {
        let env = minijinja::Environment::new();
        let html = env.render_str(
            include_str!("new_device.html.jinja"),
            context! {
                templates,
                friendly_id => &friendly_id,
                mac_address => &mac_address,
                api_key => &api_key,
                setup_expiry => &setup_expiry_raw,
                flash_error => msg,
            },
        )?;
        Ok(Response::builder()
            .status(400)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(html.into())?)
    };

    let templates_path = app_state.config()?.templates_path;
    let templates: Vec<String> = DisplayRenderer::templates(templates_path)
        .unwrap_or_default()
        .into_iter()
        .map(|t| t.name)
        .collect();

    if friendly_id.is_empty() || mac_address.is_empty() || api_key.is_empty() || setup_expiry_raw.is_empty() {
        return Ok(render_error("All fields are required.", templates)?);
    }

    // Parse datetime-local (YYYY-MM-DDTHH:MM) → RFC3339
    let setup_expiry_rfc3339 = parse_datetime_local_to_rfc3339(&setup_expiry_raw)
        .ok_or_else(|| AppError::ValidationError(format!("Invalid setup expiry date: {}", setup_expiry_raw)))?;

    // Build playlist entries, skipping blank filenames
    let playlist_entries: Vec<(String, Vec<String>)> = form
        .playlist_filename
        .iter()
        .zip(form.playlist_contexts.iter())
        .filter(|(filename, _)| !filename.trim().is_empty())
        .map(|(filename, contexts_str)| {
            let contexts: Vec<String> = contexts_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            (filename.trim().to_string(), contexts)
        })
        .collect();

    // Append new device to config file
    let config_path = &app_state.server_config.config_path;
    if let Err(e) = append_device_to_config(
        config_path,
        &mac_address,
        &friendly_id,
        &api_key,
        &setup_expiry_rfc3339,
        &playlist_entries,
    ) {
        error!("Failed to write device to config: {:?}", e);
        return Ok(render_error(
            &format!("Failed to save device: {}", e),
            templates,
        )?);
    }

    Ok(Redirect::to("/admin").into_response())
}

/// Parse a `datetime-local` string ("YYYY-MM-DDTHH:MM") into an RFC3339 string.
fn parse_datetime_local_to_rfc3339(s: &str) -> Option<String> {
    // datetime-local format: "YYYY-MM-DDTHH:MM" or "YYYY-MM-DDTHH:MM:SS"
    let s = if s.len() == 16 {
        format!("{}:00", s) // add seconds
    } else {
        s.to_string()
    };
    let naive = chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S").ok()?;
    let utc: DateTime<Utc> = DateTime::from_naive_utc_and_offset(naive, Utc);
    Some(utc.to_rfc3339())
}

/// Append a new [[devices]] block to the TOML config file.
fn append_device_to_config(
    config_path: &std::path::Path,
    mac_address: &str,
    friendly_id: &str,
    api_key: &str,
    setup_expiry: &str,
    playlist: &[(String, Vec<String>)],
) -> anyhow::Result<()> {
    let existing = fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read config file {:?}", config_path))?;

    let mut new_section = format!(
        "\n[[devices]]\nmac_address = {mac}\nfriendly_id = {fid}\napi_key = {key}\nsetup_expiry = {expiry}\n",
        mac = toml_string(mac_address),
        fid = toml_string(friendly_id),
        key = toml_string(api_key),
        expiry = toml_string(setup_expiry),
    );

    for (filename, contexts) in playlist {
        let contexts_toml = contexts
            .iter()
            .map(|c| toml_string(c))
            .collect::<Vec<_>>()
            .join(", ");
        new_section.push_str(&format!(
            "\n[[devices.playlist]]\nfilename = {}\ncontexts = [ {} ]\n",
            toml_string(filename),
            contexts_toml,
        ));
    }

    let updated = format!("{}{}", existing.trim_end(), new_section);
    fs::write(config_path, updated)
        .with_context(|| format!("Failed to write config file {:?}", config_path))?;

    Ok(())
}

/// Escape a string value for TOML (wraps in double quotes, escapes special chars).
fn toml_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
