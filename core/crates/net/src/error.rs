//! Network client errors.

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StaleDevice {
    pub did: String,
    pub device_id: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("server returned {0}: {1}")]
    Server(u16, String),

    #[error("invalid base64: {0}")]
    Base64(String),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("stale device: session out of date for {stale_devices:?}")]
    StaleDevice { stale_devices: Vec<StaleDevice> },
}
