//! Push notification relay service.
//!
//! A standalone Axum service that mediates between homeservers and APNs/FCM.
//! Homeservers never see device push tokens; they send wakeup requests
//! addressed to opaque pseudonyms. The relay maps pseudonyms to device tokens
//! and fires content-free push notifications.
//!
//! # Privacy model
//!
//! - Clients register per-(user, server) pseudonyms, so a relay cannot link
//!   a user's activity across homeservers.
//! - Push payloads are content-free: Apple/Google only see that the app was
//!   pinged, not who sent a message or what it says.
//! - Pseudonyms rotate periodically (default weekly) with a grace period
//!   so old pseudonyms still work briefly.
//!
//! # Current limitations
//!
//! - Storage is in-memory (lost on restart). Production should use SQLite or
//!   Postgres.
//! - APNs/FCM integration is stubbed: wakeup requests are logged but not
//!   actually sent to Apple/Google yet.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

// ── Types ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct Registration {
    device_token: String,
    platform: Platform,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Platform {
    Apns,
    Fcm,
}

struct RelayState {
    /// pseudonym -> registration
    registrations: RwLock<HashMap<String, Registration>>,
}

// ── Client endpoints ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RegisterRequest {
    pseudonym: String,
    device_token: String,
    platform: Platform,
    /// If rotating, the old pseudonym to remove after grace period.
    old_pseudonym: Option<String>,
}

#[derive(Serialize)]
struct RegisterResponse {
    ok: bool,
}

/// Register or update a pseudonym-to-device-token mapping.
async fn register(
    State(state): State<Arc<RelayState>>,
    Json(req): Json<RegisterRequest>,
) -> Json<RegisterResponse> {
    tracing::info!(
        pseudonym = %req.pseudonym,
        platform = ?req.platform,
        "registering pseudonym"
    );

    let mut regs = state.registrations.write().await;
    regs.insert(
        req.pseudonym,
        Registration {
            device_token: req.device_token,
            platform: req.platform,
        },
    );

    // If rotating, remove the old pseudonym immediately.
    // (A production system would keep it for a grace period.)
    if let Some(old) = req.old_pseudonym {
        tracing::info!(old_pseudonym = %old, "removing rotated pseudonym");
        regs.remove(&old);
    }

    Json(RegisterResponse { ok: true })
}

#[derive(Deserialize)]
struct UnregisterRequest {
    pseudonym: String,
}

/// Remove a pseudonym registration (e.g. on logout).
async fn unregister(
    State(state): State<Arc<RelayState>>,
    Json(req): Json<UnregisterRequest>,
) -> StatusCode {
    let mut regs = state.registrations.write().await;
    if regs.remove(&req.pseudonym).is_some() {
        tracing::info!(pseudonym = %req.pseudonym, "unregistered pseudonym");
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

// ── Homeserver endpoints ────────────────────────────────────────────────────

#[derive(Deserialize)]
struct WakeupRequest {
    pseudonyms: Vec<String>,
}

#[derive(Serialize)]
struct WakeupResponse {
    /// Pseudonyms that were successfully woken up.
    woken: Vec<String>,
    /// Pseudonyms with no registration (device may have unregistered).
    unknown: Vec<String>,
}

/// Send content-free push wakeups to one or more pseudonyms.
/// Called by homeservers when a message arrives for an offline device.
async fn wakeup(
    State(state): State<Arc<RelayState>>,
    Json(req): Json<WakeupRequest>,
) -> Json<WakeupResponse> {
    let regs = state.registrations.read().await;

    let mut woken = Vec::new();
    let mut unknown = Vec::new();

    for pseudonym in req.pseudonyms {
        if let Some(reg) = regs.get(&pseudonym) {
            // TODO: Actually send push notification to APNs/FCM.
            // For now, just log it.
            tracing::info!(
                pseudonym = %pseudonym,
                platform = ?reg.platform,
                token_prefix = %&reg.device_token[..8.min(reg.device_token.len())],
                "sending wakeup push (stubbed)"
            );
            woken.push(pseudonym);
        } else {
            tracing::debug!(pseudonym = %pseudonym, "unknown pseudonym, skipping");
            unknown.push(pseudonym);
        }
    }

    Json(WakeupResponse { woken, unknown })
}

// ── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let bind_addr = std::env::var("RELAY_BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:3002".to_string());

    let state = Arc::new(RelayState {
        registrations: RwLock::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/v1/register", post(register))
        .route("/v1/unregister", post(unregister))
        .route("/v1/wakeup", post(wakeup))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!(bind = %bind_addr, "starting push relay");

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app).await.expect("relay error");
}
