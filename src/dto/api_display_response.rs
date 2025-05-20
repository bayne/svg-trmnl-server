use crate::dto;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApiDisplayResponse {
    #[serde(rename = "error_detail", skip_serializing_if = "Option::is_none")]
    pub error_detail: Option<String>,
    #[serde(rename = "status")]
    pub status: i32,
    #[serde(rename = "image_url", skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(rename = "image_url_timeout", skip_serializing_if = "Option::is_none")]
    pub image_url_timeout: Option<i32>,
    #[serde(rename = "filename", skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(rename = "update_firmware", skip_serializing_if = "Option::is_none")]
    pub update_firmware: Option<bool>,
    #[serde(rename = "firmware_url", skip_serializing_if = "Option::is_none")]
    pub firmware_url: Option<String>,
    #[serde(rename = "refresh_rate")]
    pub refresh_rate: i32,
    #[serde(rename = "reset_firmware", skip_serializing_if = "Option::is_none")]
    pub reset_firmware: Option<bool>,
    #[serde(rename = "special_function")]
    pub special_function: dto::SpecialFunction,
    #[serde(rename = "action", skip_serializing_if = "Option::is_none")]
    pub action: Option<dto::SpecialFunction>,
}
