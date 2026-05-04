//! WebSocket endpoint for real-time message delivery.
//!
//! `GET /v1/ws?token=<session_token>` upgrades to a WebSocket connection.
//! Once connected, the server:
//!
//! 1. Drains any queued messages for this device (catch-up on reconnect).
//! 2. Pushes new messages inline as they arrive (full ciphertext payload,
//!    not just a notification).
//! 3. Accepts `ack` frames from the client to delete delivered messages.
//!
//! # Security notes
//!
//! - **Authentication is via query parameter**, not a header, because the
//!   browser WebSocket API does not support custom headers. The token is
//!   validated before the upgrade completes — an invalid token gets a 401
//!   HTTP response, not a WebSocket connection.
//! - **The WebSocket carries ciphertext only.** The server pushes the same
//!   opaque `bytea` blobs it stores in the message queue.
//! - **Connection state is in-memory.** If the server process restarts,
//!   all WebSocket connections drop. Clients reconnect and drain the queue
//!   via the catch-up mechanism.

use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::Response,
    routing::get,
    Router,
};
use axum::extract::ws::{Message, WebSocket};
use base64::prelude::*;
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::{db, error::ServerError, state::{AppState, WsMessage}};

pub fn routes() -> Router<AppState> {
    Router::new().route("/v1/ws", get(ws_upgrade))
}

#[derive(Deserialize)]
struct WsQuery {
    token: String,
}

async fn ws_upgrade(
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, ServerError> {
    let mut conn = state.db.acquire().await?;
    let device_pk = db::sessions::validate(&mut conn, &query.token)
        .await?
        .ok_or(ServerError::Unauthorized)?;

    Ok(ws.on_upgrade(move |socket| handle_ws(socket, state, device_pk)))
}

async fn handle_ws(mut socket: WebSocket, state: AppState, device_pk: i64) {
    let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();

    // Register this connection.
    state.ws_connections.write().await.insert(device_pk, tx);

    // Drain any queued messages on connect.
    if let Ok(mut conn) = state.db.acquire().await {
        if let Ok(queued) = db::messages::fetch_for_device(&mut conn, device_pk).await {
            for msg in &queued {
                let ws_msg = serde_json::json!({
                    "type": "message",
                    "id": msg.id,
                    "ciphertext": BASE64_STANDARD.encode(&msg.ciphertext),
                    "message_kind": msg.message_kind,
                    "sender_did": msg.sender_did,
                    "sender_device_id": msg.sender_device_id,
                });
                if socket.send(Message::Text(ws_msg.to_string().into())).await.is_err() {
                    break;
                }
            }
        }
    }

    loop {
        tokio::select! {
            // Forward server-side messages to the WebSocket.
            Some(ws_msg) = rx.recv() => {
                if socket.send(Message::Text(ws_msg.0.into())).await.is_err() {
                    break;
                }
            }
            // Handle incoming client messages.
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        handle_client_message(&state, device_pk, &text).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }

    // Unregister on disconnect.
    state.ws_connections.write().await.remove(&device_pk);
}

async fn handle_client_message(state: &AppState, device_pk: i64, text: &str) {
    let Ok(msg) = serde_json::from_str::<serde_json::Value>(text) else {
        return;
    };

    match msg.get("type").and_then(|v| v.as_str()) {
        Some("ack") => {
            if let Some(ids) = msg.get("message_ids").and_then(|v| v.as_array()) {
                let ids: Vec<i64> = ids.iter().filter_map(|v| v.as_i64()).collect();
                if let Ok(mut conn) = state.db.acquire().await {
                    let _ = db::messages::acknowledge(&mut conn, device_pk, &ids).await;
                }
            }
        }
        Some("ping") => {
            // Handled by WebSocket-level ping/pong; application-level is a no-op.
        }
        _ => {}
    }
}
