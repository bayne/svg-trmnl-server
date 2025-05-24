use crate::api::AppState;
use crate::context::weather::{AppWeatherConfig, create_weather_context};
use serde_json::{Map, Value};

mod weather;

pub trait ContextConfig {
    fn name() -> &'static str;
}

pub async fn load_contexts(
    app_state: AppState,
    friendly_id: &str,
    contexts: Vec<String>,
) -> anyhow::Result<Map<String, Value>> {
    let mut result = Map::new();
    for context_name in contexts {
        let context = match context_name.as_str() {
            "weather" => {
                let context_config: AppWeatherConfig = app_state.get_context_config(friendly_id)?;
                create_weather_context(&context_config).await?
            }
            _ => return Err(anyhow::anyhow!("Unknown context: {}", context_name)),
        };
        result.insert(context_name, serde_json::to_value(context)?);
    }
    Ok(result)
}
