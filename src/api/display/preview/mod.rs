use crate::api::{AppError, AppState};
use crate::bad_request;
use crate::display::DisplayRenderer;
use anyhow::Context;
use axum::extract::ws::Message::Text;
use axum::extract::ws::WebSocket;
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::response::Response;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use minijinja::context;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::PathBuf;
use tracing::error;
use url::Url;

pub async fn preview_handler(
    State(app_state): State<AppState>,
) -> anyhow::Result<Response, AppError> {
    let env = minijinja::Environment::new();
    let base_url = app_state.config()?.base_url;
    let mut websocket_url =
        Url::parse(&base_url).context(format!("invalid base url, {}", base_url))?;
    websocket_url.set_path("/display/preview/ws");
    let websocket_url = websocket_url.as_str();
    let result = env
        .render_str(
            include_str!("index.html"),
            context! {
                websocket_url
            },
        )
        .context("Failed to render preview template")?;
    Ok(Response::new(result.into()))
}
pub async fn preview_websocket_handler(
    web_socket_upgrade: WebSocketUpgrade,
    State(app_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> anyhow::Result<Response, AppError> {
    let template = params
        .get("template")
        .ok_or(bad_request!("missing template parameter"))?
        .clone();

    let context = app_state.config()?.default_context_path;

    let display_renderer = app_state.display_renderer()?;
    Ok(web_socket_upgrade
        .on_upgrade(|socket| handle_preview_websocket(socket, display_renderer, template, context)))
}

async fn handle_preview_websocket(
    mut socket: WebSocket,
    display_renderer: DisplayRenderer,
    template: String,
    context: PathBuf,
) {
    let msg = create_msg(display_renderer, template, context).await;
    let msg = Text(msg.to_string().into());

    if let Err(e) = socket.send(msg).await {
        error!("failed to send message: {}", e);
        return;
    }
}

async fn create_msg(
    display_renderer: DisplayRenderer,
    template: String,
    context: PathBuf,
) -> Value {
    let context: Value = match read_to_string(&context) {
        Ok(context) => context.into(),
        Err(err) => {
            return json!({
                "status": "error",
                "message": format!("{:?}: {}", context, err.to_string()),
                "image_data": "",
            });
        }
    };

    let image_data = match display_renderer.render_jinja(&template, &context) {
        Ok(image_data) => BASE64_STANDARD.encode(image_data),
        Err(err) => {
            return json!({
                "status": "error",
                "message": err.to_string(),
                "image_data": "",
            });
        }
    };

    json!({
        "status": "ok",
        "message": "",
        "image_data": image_data,
    })
}
