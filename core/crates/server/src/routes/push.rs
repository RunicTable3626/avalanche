//! Push pseudonym registration endpoint.
//!
//! Clients call `POST /v1/push/register` to store their push pseudonym on the
//! homeserver. The homeserver uses this pseudonym to send wakeup pings to the
//! push relay when the device is offline.

use axum::{extract::State, routing::post, Json, Router};
use serde::Deserialize;

use crate::{db, error::ServerError, middleware::auth::AuthDevice, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/push/register", post(register))
        .route("/v1/push/unregister", post(unregister))
}

#[derive(Deserialize)]
struct RegisterRequest {
    pseudonym: String,
}

async fn register(
    State(state): State<AppState>,
    auth: AuthDevice,
    Json(req): Json<RegisterRequest>,
) -> Result<(), ServerError> {
    let mut conn = state.db.acquire().await?;
    db::push::register(&mut conn, &req.pseudonym, auth.device_pk).await?;
    tracing::info!(device_pk = auth.device_pk, "push pseudonym registered");
    Ok(())
}

#[derive(Deserialize)]
struct UnregisterRequest {
    pseudonym: String,
}

async fn unregister(
    State(state): State<AppState>,
    auth: AuthDevice,
    Json(req): Json<UnregisterRequest>,
) -> Result<(), ServerError> {
    let mut conn = state.db.acquire().await?;
    db::push::unregister(&mut conn, &req.pseudonym).await?;
    tracing::info!(device_pk = auth.device_pk, "push pseudonym unregistered");
    Ok(())
}
