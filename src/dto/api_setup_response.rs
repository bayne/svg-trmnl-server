use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApiSetupResponse {
    pub status: i32,
    pub api_key: Option<String>,
    pub friendly_id: Option<String>,
    pub image_url: Option<String>,
    pub message: String,
}
