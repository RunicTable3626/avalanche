//! Network client errors.

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

    /// Authenticated request attempted but no `Signer` was configured on the
    /// `Client`. Indicates a misconfiguration — callers should always set up
    /// auth via `with_signer` before issuing authenticated requests.
    #[error("no signer configured for authenticated request")]
    NoSigner,

    /// Signer returned an error while signing the challenge nonce.
    #[error("signing failed: {0}")]
    Signing(String),
}
