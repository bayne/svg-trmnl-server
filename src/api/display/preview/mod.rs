use crate::api::{AppError, AppState};
use crate::bad_request;
use crate::display::{DisplayRenderer, Template};
use anyhow::Context;
use async_stream::stream;
use axum::extract::ws::Message::{Ping, Text};
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::response::Response;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use futures_util::SinkExt;
use futures_util::stream::{SplitSink, SplitStream, StreamExt};
use minijinja::context;
use notify::event::{CreateKind, ModifyKind, RemoveKind};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::ops::DerefMut;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;
use tokio::sync::oneshot::{Receiver, Sender};
use tracing::error;
use tracing::info;
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
    let templates_path = app_state.config()?.templates_path;
    let templates = DisplayRenderer::templates(templates_path)?
        .iter()
        .map(|Template { name, .. }| (name.clone(), hex::encode(name)))
        .collect::<Vec<(String, String)>>();
    let result = env
        .render_str(
            include_str!("index.html"),
            context! {
                websocket_url,
                templates
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

    let templates_path = app_state.config()?.templates_path;
    let template = DisplayRenderer::templates(templates_path)?
        .iter()
        .find(|Template { name, .. }| hex::encode(name) == template)
        .ok_or(bad_request!("invalid template"))?
        .name
        .clone();

    Ok(web_socket_upgrade
        .on_upgrade(move |socket| on_websocket_upgrade(socket, app_state, template)))
}

async fn on_websocket_upgrade(socket: WebSocket, app_state: AppState, template: String) {
    let (ws_tx, ws_rx) = socket.split();
    let ws_tx = Arc::new(Mutex::new(ws_tx));
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<bool>();
    generate(ws_tx.clone(), &template, &app_state).await;
    tokio::join!(
        preview_websocket_message_handler(ws_rx, ws_tx.clone(), cancel_tx),
        file_change_event_handler(ws_tx.clone(), cancel_rx, &template, app_state)
    );
    info!("finished");
}

async fn preview_websocket_message_handler(
    mut ws_rx: SplitStream<WebSocket>,
    ws_tx: Arc<Mutex<SplitSink<WebSocket, Message>>>,
    cancel_tx: Sender<bool>,
) {
    let result = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                msg = ws_rx.next() => {
                    match msg {
                        Some(Ok(Message::Close(_))) => {
                            info!("received close message");
                            if let Err(e) = cancel_tx.send(true) {
                                error!("failed to send cancel event {}", e);
                            }
                            break;
                        }
                        Some(Ok(_)) => {}
                        Some(Err(e)) => {
                            error!("failed to receive message: {}", e);
                        }
                        None => {
                            info!("receiver finished on ws");
                            break;
                        }
                    };
                }
                _ = interval.tick() => {
                    if let Err(e) = ws_tx.lock().await.deref_mut().send(Ping("ping".into())).await {
                        error!("failed to send ping: {}", e);
                    }
                }
            }
        }
    })
    .await;
    if let Err(e) = result {
        error!("failed to send message: {}", e);
    }
}

async fn file_change_event_handler(
    ws_tx: Arc<Mutex<SplitSink<WebSocket, Message>>>,
    mut cancel_rx: Receiver<bool>,
    template: &str,
    app_state: AppState,
) {
    let templates_path = match app_state.config() {
        Ok(config) => config.templates_path,
        Err(e) => {
            error!("failed to get templates path from config {}", e);
            return;
        }
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
        Ok(watcher) => watcher,
        Err(e) => {
            error!("failed to create watcher {}", e);
            return;
        }
    };

    if let Err(e) = watcher.watch(&templates_path, RecursiveMode::Recursive) {
        error!("watch error: {}", e);
        return;
    }

    info!("Watching files for changes...");
    let template = template.to_string();
    let result = tokio::spawn(async move {
        let async_rx = stream! {
            while let Ok(value) = rx.recv() {
                yield value;
            }
        };
        tokio::pin!(async_rx);
        let mut last_generate = SystemTime::now();
        loop {
            tokio::select! {
                _ = &mut cancel_rx => {
                    break;
                }
                event = async_rx.next() => {
                    let Event { kind,  .. } = match event {
                        Some(Ok(event)) => event,
                        _ => continue
                    };
                    match kind {
                        EventKind::Create(CreateKind::File) | EventKind::Modify(ModifyKind::Data(_)) | EventKind::Remove(RemoveKind::File) => {
                            let elapsed = last_generate.elapsed().unwrap_or_else(|e| {
                                error!("failed to generate files: {}", e);
                                Duration::from_secs(0)
                            });
                            if elapsed < Duration::from_secs(1) {
                                continue;
                            }
                            last_generate = SystemTime::now();
                            generate(ws_tx.clone(), &template, &app_state).await;
                        }
                        _ => {}
                    }
                }
            }
        }
    }).await;
    if let Err(e) = result {
        error!("failed to stop file watcher: {}", e);
    }
}

async fn generate(
    tx_websocket: Arc<Mutex<SplitSink<WebSocket, Message>>>,
    template: &str,
    app_state: &AppState,
) {
    let display_renderer = match app_state.display_renderer() {
        Ok(display_renderer) => display_renderer,
        Err(e) => {
            error!("Failed to get display renderer {}", e);
            return;
        }
    };
    let context = match app_state.config() {
        Ok(config) => config.default_context_path,
        Err(e) => {
            error!("Failed to get default context path {}", e);
            return;
        }
    };
    let msg = create_msg(&display_renderer, template, &context);
    let msg = Text(msg.to_string().into());
    if let Err(e) = tx_websocket.lock().await.send(msg).await {
        error!("failed to send message, image: {}", e);
    }
}

fn create_msg(display_renderer: &DisplayRenderer, template: &str, context: &Path) -> Value {
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

    let image_data = match display_renderer.render_jinja(template, &context) {
        Ok(image_data) => BASE64_STANDARD.encode(image_data),
        Err(err) => {
            return json!({
                "status": "error",
                "message": err.to_string(),
                "image_data": "",
            });
        }
    };

    info!("updated: {}", template);
    json!({
        "status": "ok",
        "message": "",
        "image_data": image_data,
    })
}
