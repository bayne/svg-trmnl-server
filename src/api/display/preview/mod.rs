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
use futures_util::future::err;
use tokio::sync::Mutex;
use tokio::sync::oneshot::Receiver;
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
        .on_upgrade(move |socket| handle_preview_websocket(socket, app_state, template)))
}

// async fn read(mut receiver: SplitStream<WebSocket>, tx: Sender<bool>) {
//     tokio::spawn(async move {
//         loop {
//             match receiver.next().await {
//                 Some(Ok(msg)) => {
//                     if let Message::Close(_) = msg {
//                         info!("received close message from client");
//                         tx.send(true).unwrap();
//                         return;
//                     }
//                 }
//                 Some(Err(e)) => {
//                     error!("failed to receive message: {}", e);
//                 }
//                 None => {
//                     error!("receiver finished on ws");
//                     return;
//                 }
//             }
//         }
//     })
//     .await
//     .unwrap();
// }

async fn auto_generate(
    mut tx_websocket: SplitSink<WebSocket, Message>,
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
    let mut watcher =
        RecommendedWatcher::new(tx, Config::default().with_compare_contents(false)).unwrap();

    if let Err(e) = watcher.watch(&templates_path, RecursiveMode::Recursive) {
        error!("watch error: {}", e);
        return;
    }

    info!("Watching files for changes...");
    let template = template.to_string();
    tokio::spawn(async move {
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
                            if last_generate.elapsed().unwrap() < Duration::from_secs(1) {
                                continue;
                            }
                            last_generate = SystemTime::now();
                            generate(&mut tx_websocket, &template, &app_state).await;
                        }
                        _ => {}
                    }
                }
            }
        }
    }).await.unwrap();
}

async fn generate(
    tx_websocket: &mut SplitSink<WebSocket, Message>,
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
    if let Err(e) = tx_websocket.send(msg).await {
        error!("failed to send message, image: {}", e);
    }
}

async fn handle_preview_websocket(socket: WebSocket, app_state: AppState, template: String) {
    let (ws_tx, mut ws_rx) = socket.split();
    let ws_tx = Arc::new(Mutex::new(ws_tx));
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<bool>();
    // let cancel_rx2: Receiver<bool> = cancel_tx.subscribe();

    // tokio::spawn(async move {
    //     loop {
    //         match receiver.next().await {
    //             Some(Ok(msg)) => {
    //                 if let Message::Close(_) = msg {
    //                     info!("received close message from client");
    //                     tx.send(true).unwrap();
    //                     return;
    //                 }
    //             }
    //             Some(Err(e)) => {
    //                 error!("failed to receive message: {}", e);
    //             }
    //             None => {
    //                 error!("receiver finished on ws");
    //                 return;
    //             }
    //         }
    //     }
    // })
    //     .await
    //     .unwrap();
    // let ping_sender = ws_tx.clone();
    let ping_interval = async move {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                tokio::select! {
                    msg = ws_rx.next() => {
                        match msg {
                            Some(Ok(Message::Close(_))) => {
                                info!("received close message");
                                cancel_tx.send(true).unwrap();
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
        });
    };

    // let generate_sender = ws_tx2.clone();
    let mut ws_tx = ws_tx.lock().await;
    generate(
        &mut ws_tx,
        &template,
        &app_state,
    )
    .await;

    tokio::join!(
        ping_interval,
        // read(ws_rx, cancel_tx),
        auto_generate(
            &mut ws_tx,
            cancel_rx,
            &template,
            app_state
        )
    );
    info!("finished");
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
