mod weather;

use tower::ServiceBuilder;
use tower_http::{trace::TraceLayer};
use crate::usvg::Transform;
use anyhow::Result;
use minijinja::{context, Environment};
use resvg::usvg;
use std::fs::read_to_string;
use std::io::{BufReader, Read};
use std::path::Path;

fn convert<'a>(pixmap: &Pixmap) -> Bmp<'a, RawU1> {
    // Create a Pixmap using tinyskia
    let width = 100;
    let height = 100;
    // let mut pixmap = Pixmap::new(width, height).unwrap();
    //
    // // Manipulate the pixmap, drawing a simple colored rectangle
    // for y in 0..height {
    //     for x in 0..width {
    //         let pixel = if (x + y) % 2 == 0 {
    //             tiny_skia::Color::from_rgba8(0, 255, 0, 255) // Green
    //         } else {
    //             tiny_skia::Color::from_rgba8(0, 0, 255, 255) // Blue
    //         };
    //         pixmap.pixel(x as u32, y as u32, pixel);
    //     }
    // }

    // Convert the pixmap to BMP
    // let header = Header {
    //     width: width as u32,
    //     height: height as u32,
    //     color_depth: ColorDepth::Bits24,
    // };

    // Prepare the BMP pixel data buffer
    // let mut bmp_pixels: Vec<u8> = vec![0u8; width as usize * height as usize * 3];

    // for y in 0..height {
    //     for x in 0..width {
    //         let pixel = pixmap.pixel(x as u32, y as u32).unwrap();
    //         let idx = (y * width + x) * 3;
    //         bmp_pixels[idx] = pixel.blue;
    //         bmp_pixels[idx + 1] = pixel.green;
    //         bmp_pixels[idx + 2] = pixel.red;
    //     }
    // }

    // Create a BMP file
    // let bmp = Bmp::from_pixels(header, &bmp_pixels).unwrap();
    Bmp::<RawU1>::from_slice(pixmap.data()).unwrap()

    // Save the BMP file
    // std::fs::write("output.bmp", bmp.as_slice()).expect("Failed to write BMP file");
}

fn generate() -> Result<Pixmap> {
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_fonts_dir("fonts");
    let mut templates = Vec::new();
    let mut env = Environment::new();

    for entry in std::fs::read_dir("./templates")? {
        let path = entry?.path();
        if path.is_file() {
            if let Some(file_name) = path.file_name() {
                let file_name = file_name.to_string_lossy();
                templates.push((file_name.to_string(), read_to_string(&path)?));
            }
        }
    }
    for (file_name, content) in &templates {
        env.add_template(file_name, &content)?;
    }

    let template = env.get_template("test.svg.jinja")?;
    let output = template.render(context! {})?;

    let tree = usvg::Tree::from_data(output.as_bytes(), &opt)?;

    let pixmap_size = tree.size().to_int_size();
    let mut pixmap = Pixmap::new(pixmap_size.width(), pixmap_size.height())
        .unwrap();

    resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());

    Ok(pixmap)
}

use notify::event::{CreateKind, ModifyKind, RemoveKind};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::sync::mpsc::channel;
use tiny_skia::Pixmap;

// fn main() -> Result<()> {
//     // Create a channel to receive the events.
//     let (tx, rx) = channel();
//
//     // Create a watcher object, delivering debounced events.
//     // The notification back-end is selected based on the platform.
//     let mut watcher = notify::recommended_watcher(tx).unwrap();
//     // Add a path to be watched. All files and directories at that path and
//     // below will be monitored for changes.
//     watcher.watch(Path::new("templates"), RecursiveMode::Recursive).unwrap();
//
//     println!("Watching file for changes...");
//
//     loop {
//         let Event { kind, paths, attrs } = rx.recv()??;
//         match kind {
//             EventKind::Create(CreateKind::File) | EventKind::Modify(ModifyKind::Data(_)) | EventKind::Remove(RemoveKind::File) => {
//                 if let Err(e) = generate() {
//                     eprintln!("{}", e);
//                     if let Err(e) = std::fs::remove_file("output.png") {
//                         eprintln!("{}", e);
//                     }
//                 } else {
//                     println!("Regenerated! {}", paths.last().unwrap().to_str().unwrap());
//                 }
//             }
//             _ => {
//             }
//         }
//     }
// }

use axum::{response::Html, routing::get, Extension, Router};
use axum::body::{Body, Bytes};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
// use tower_http::services::ServeFile;


// #[tokio::main]
// async fn main() {
//     // build our application with a route
//     let app = Router::new().route("/", get(handler));
//
//     // run it
//     let listener = tokio::net::TcpListener::bind("0.0.0.0:9080")
//         .await
//         .unwrap();
//     println!("listening on {}", listener.local_addr().unwrap());
//     // ServeFile::new("./static");
//     axum::serve(listener, app).await.unwrap();
// }
//
async fn handler() -> impl IntoResponse {
    let image = generate().unwrap();
    let image = convert(&image);
    // let image = image.encode_png().unwrap();
    let mut res = Body::from(image.into()).into_response();
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("image/bmp")
    );
    res

    // let mut file = std::fs::File::open("./static/index.html").unwrap();
    // let mut buffer = Vec::new();
    // file.read_to_end(&mut buffer).unwrap();
    // buffer.into()
}

// main.rs
// Dependencies in Cargo.toml:
// axum = "0.6"
// tokio = { version = "1", features = ["full"] }
// serde = { version = "1.0", features = ["derive"] }
// serde_json = "1.0"
// uuid = { version = "1", features = ["v4"] }
// tracing = "0.1"
// tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }

use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    // response::IntoResponse,
    routing::{post},
    // Router,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use embedded_graphics::pixelcolor::raw::RawU1;
use embedded_graphics::pixelcolor::Rgb888;
use tinybmp::Bmp;
use tokio::sync::Mutex;
use uuid::Uuid;
use tracing::{error, info};

#[derive(Clone)]
struct AppState {
    devices: Arc<Mutex<HashMap<String, DeviceInfo>>>, // api_key -> DeviceInfo
}

#[derive(Debug, Clone)]
struct DeviceInfo {
    mac: String,
    friendly_id: String,
}

#[derive(Serialize)]
struct SetupResponse {
    status: u16,
    api_key: String,
    friendly_id: String,
    image_url: String,
    message: String,
}

#[derive(Deserialize)]
struct ApiDisplayInputs {
    api_key: String,
    friendly_id: String,
    mac_address: String,
    firmware_version: String,
    battery_voltage: f32,
    rssi: i32,
    display_width: u32,
    display_height: u32,
    refresh_rate: u64,
    special_function: u8,
}

#[derive(Serialize)]
struct ApiDisplayResponse {
    status: u8,
    image_url: String,
    filename: String,
    refresh_rate: u64,
    update_firmware: bool,
    firmware_url: String,
    reset_firmware: bool,
    special_function: String,
}

#[derive(Deserialize)]
struct LogApiInput {
    api_key: String,
    log: String,
}

#[tokio::main]
async fn main() {
    // Initialize tracing for structured logs
    tracing_subscriber::fmt()
        .with_env_filter("trace")
        .init();

    let state = AppState {
        devices: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()

        .route("/api/setup/", get(handle_setup))
        .route("/api/display", get(handle_display))
        .route("/api/log", post(handle_logs))
        .route("/image", get(handler))
        // .layer(
        //     ServiceBuilder::new()
        //         .layer(TraceLayer::new_for_http())
        //         .layer(Extension(State { 0: () }))
        //
        // )
        .with_state(state);

    // run it
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9080")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    // ServeFile::new("./static");
    axum::serve(listener, app).await.unwrap();
    // axum::serve
    // axum::Server::bind(&addr)
    //     .serve(app.into_make_service())
    //     .await
    //     .unwrap();
}

async fn handle_setup(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mac = headers
        .get("ID")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let fw_version = headers
        .get("FW-Version")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    info!("Setup request from MAC={} FW-Version={}", mac, fw_version);

    // Generate API key and friendly ID
    let api_key = Uuid::new_v4().to_string();
    let friendly_id = format!("TRMNL-{}", &api_key[..8]);

    // Store device info
    state.devices.lock().await.insert(
        api_key.clone(),
        DeviceInfo { mac: mac.clone(), friendly_id: friendly_id.clone() },
    );

    let resp = SetupResponse {
        status: 200,
        api_key,
        friendly_id,
        image_url: "https://example.com/images/setup-logo.bmp".into(),
        message: "Welcome!".into(),
    };

    (StatusCode::OK, axum::Json(resp))
}

async fn handle_display(
    State(state): State<AppState>,
) -> impl IntoResponse {
    // info!("Display ping from api_key={} mac={}  ", input.api_key, input.mac_address);

    // Validate API key
    // let devices = state.devices.lock().await;
    // if !devices.contains_key(&input.api_key) {
    //     error!("Unknown API key: {}", input.api_key);
    //     let resp = ApiDisplayResponse { status: 202, image_url: "".into(), filename: "".into(), refresh_rate: 0, update_firmware: false, firmware_url: "".into(), reset_firmware: false, special_function: 0, action: "".into() };
    //     return (StatusCode::OK, axum::Json(resp));
    // }

    // Build response
    let image_url = "https://trmnl-server.home.southroute.com/image".into();
    let filename = "current-slide.png".into();
    let resp = ApiDisplayResponse {
        status: 0,
        image_url,
        filename,
        refresh_rate: 3600,
        update_firmware: false,
        firmware_url: "".into(),
        reset_firmware: false,
        special_function: "sleep".into(),
    };

    (StatusCode::OK, axum::Json(resp))
}

async fn handle_logs(
    Json(input): Json<LogApiInput>,
) -> impl IntoResponse {
    info!("Log upload for api_key={}", input.api_key);
    println!("Received log: {}", input.log);
    StatusCode::NO_CONTENT
}
