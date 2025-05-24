use anyhow::{Context, Result};
use resvg::usvg;
use resvg::usvg::Transform;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::fs::read_to_string;
use std::path::PathBuf;
use std::time::SystemTime;
use tiny_skia::Pixmap;

const WIDTH: usize = 800;
const HEIGHT: usize = 480;
const PIXELS_PER_BYTE: usize = 8;
const PIXEL_BUFFER_SIZE: usize = (WIDTH * HEIGHT / PIXELS_PER_BYTE) as usize;
const HEADER_SIZE: usize = 62;
const IMAGE_SIZE: usize = PIXEL_BUFFER_SIZE + HEADER_SIZE;
const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLANK_BMP: &[u8] = include_bytes!("blank.bmp");
const TEMPLATE_FILE_EXT: &str = "jinja";

pub struct Template {
    pub name: String,
    pub content: String,
}

pub struct DisplayRenderer {
    templates: Vec<Template>,
    fonts_path: PathBuf,
}

impl DisplayRenderer {
    pub fn new(fonts_path: PathBuf, templates_path: PathBuf) -> Result<DisplayRenderer> {
        let templates = DisplayRenderer::templates(templates_path)?;
        Ok(DisplayRenderer {
            templates,
            fonts_path,
        })
    }

    pub fn templates(templates_path: PathBuf) -> Result<Vec<Template>> {
        let mut templates = vec![];
        for entry in std::fs::read_dir(&templates_path)? {
            let path = entry?.path();
            let extension = path.extension().context("missing extension")?;
            if path.is_file() && extension == TEMPLATE_FILE_EXT {
                let content = read_to_string(&path)?;
                let name = path
                    .strip_prefix(&templates_path)
                    .context("failed to strip prefix")?
                    .to_str()
                    .context("failed to convert path to string")?
                    .to_string();
                templates.push(Template {
                    name,
                    content,
                });
            }
        }
        Ok(templates)
    }

    fn minijinja_env(&self) -> Result<minijinja::Environment> {
        let mut env = minijinja::Environment::new();
        for Template { name, content, .. } in &self.templates {
            env.add_template(name, content)?;
        }
        Ok(env)
    }

    fn usvg_opt(&self) -> usvg::Options {
        let mut opt = usvg::Options::default();
        opt.fontdb_mut().load_fonts_dir(&self.fonts_path);
        opt
    }

    pub fn render_jinja(&self, template: &str, ctx: &Map<String, Value>) -> Result<DisplayImage> {
        let env = self.minijinja_env()?;
        let template = env.get_template(template)?;

        let icons_context: Value = serde_json::from_str(include_str!("icons.json"))
            .context("failed to parse icons.json")?;
        let icons_context = icons_context
            .as_object()
            .context("icons.json is not an object")?
            .to_owned();

        let mut ctx = ctx.clone();
        ctx.insert("icons".to_string(), Value::Object(icons_context));

        let svg = template.render(ctx)?;

        self.render(&svg)
    }

    pub fn render(&self, svg: &str) -> Result<DisplayImage> {
        let tree = usvg::Tree::from_data(svg.as_bytes(), &self.usvg_opt())?;

        let pixmap_size = tree.size().to_int_size();
        let mut pixmap = Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();

        resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());

        create_bmp(&pixmap.take())
    }
}

pub type DisplayImage = [u8; IMAGE_SIZE];

fn create_bmp(pixel_data: &[u8]) -> Result<DisplayImage> {
    let mut buffer: [u8; IMAGE_SIZE] = [0; IMAGE_SIZE];
    let pixels = buffer[HEADER_SIZE..].as_mut();
    for (index, pixel) in pixel_data.chunks(4).into_iter().enumerate() {
        let row = HEIGHT - 1 - index / WIDTH;
        let col = index % WIDTH;
        let flipped_index = row * WIDTH + col;
        let byte_index = flipped_index / PIXELS_PER_BYTE;
        let bit_index = flipped_index % PIXELS_PER_BYTE;
        if *pixel == WHITE {
            pixels[byte_index] |= 1 << 7 - bit_index;
        }
    }

    buffer[..HEADER_SIZE].copy_from_slice(&BLANK_BMP[..HEADER_SIZE]);

    Ok(buffer)
}

pub fn generate_filename(api_key: String, timestamp: SystemTime) -> Result<String> {
    let timestamp: u64 = timestamp
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("failed to get elapsed time")?
        .as_secs();
    let hash = Sha256::new()
        .chain_update(api_key.as_bytes())
        .chain_update(timestamp.to_be_bytes())
        .finalize();
    Ok(format!("{}.bmp", hex::encode(hash)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;
    use std::path::Path;
    use std::time::Duration;

    #[test]
    fn it_should_render_image() {
        let display_renderer = DisplayRenderer::new("fonts".into(), "templates".into()).unwrap();
        let ctx = Map::new();
        let image = display_renderer
            .render_jinja("test.svg.jinja", &ctx)
            .unwrap();
        write(Path::new("test.bmp"), image).unwrap();
    }

    #[test]
    fn it_should_generate_filename() {
        let timestamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1234567890);
        let filename = generate_filename("fake_api_key".to_string(), timestamp).unwrap();
        assert_eq!(
            filename,
            "39bf95b5a576efb89503cf3ed2bafb5a8fb7ac8f12db7bf9164442abb7fbacdd.bmp"
        );
    }
}
