use serde::{Deserialize, Serialize};

///
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum SpecialFunction {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "identify")]
    Identify,
    #[serde(rename = "sleep")]
    Sleep,
    #[serde(rename = "add_wifi")]
    AddWifi,
    #[serde(rename = "restart_playlist")]
    RestartPlaylist,
    #[serde(rename = "rewind")]
    Rewind,
    #[serde(rename = "send_to_me")]
    SendToMe,
}

impl std::fmt::Display for SpecialFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Identify => write!(f, "identify"),
            Self::Sleep => write!(f, "sleep"),
            Self::AddWifi => write!(f, "add_wifi"),
            Self::RestartPlaylist => write!(f, "restart_playlist"),
            Self::Rewind => write!(f, "rewind"),
            Self::SendToMe => write!(f, "send_to_me"),
        }
    }
}

impl Default for SpecialFunction {
    fn default() -> SpecialFunction {
        Self::None
    }
}
