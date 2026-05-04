//! Shared application state available to all request handlers.
//!
//! [`AppState`] is cloned into every Axum handler via the `State` extractor.
//! It holds the database pool, server config, and the in-memory WebSocket
//! connection map. The connection map tracks which devices currently have a
//! live WebSocket so that incoming messages can be pushed immediately rather
//! than waiting for the client to poll.
//!
//! # Scaling note
//!
//! The WebSocket connection map is in-process (`Arc<RwLock<HashMap>>`). This
//! works for a single server instance. For horizontal scaling, the map would
//! need to be backed by PostgreSQL `LISTEN/NOTIFY` or a shared pub/sub layer
//! so that a message enqueued on instance A can notify a WebSocket on
//! instance B.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::config::Config;

/// A WebSocket message to push to a connected device.
#[derive(Debug, Clone)]
pub struct WsMessage(pub String);

/// Shared application state, available to all request handlers via Axum's
/// State extractor.
#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: Config,
    /// Connected WebSocket devices: internal device PK -> sender channel.
    pub ws_connections: Arc<RwLock<HashMap<i64, mpsc::UnboundedSender<WsMessage>>>>,
}

impl AppState {
    pub fn new(db: sqlx::PgPool, config: Config) -> Self {
        Self {
            db,
            config,
            ws_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
