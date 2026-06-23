//! Server info: `GET /v1/info`.
//!
//! Public, unauthenticated. Returns human-readable server metadata
//! including the operator's privacy policy URL (if configured).

use axum::{
    extract::State,
    http::header,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::state::AppState;

/// The bundled default privacy-policy template, embedded at compile time.
/// Served verbatim (placeholders, operator note, and all) by
/// [`privacy_policy_template`] purely as a dev/testing convenience for the
/// signup privacy-policy link. Real operators host their own filled-in policy
/// and point `PRIVACY_POLICY_URL` at that — not at this route.
const PRIVACY_POLICY_TEMPLATE: &str = include_str!("../../../../../PRIVACY-POLICY-TEMPLATE.md");

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/info", get(info))
        .route("/privacy-policy-template", get(privacy_policy_template))
}

#[derive(Serialize)]
struct InfoResponse {
    server_name: String,
    privacy_policy_url: Option<String>,
}

async fn info(State(state): State<AppState>) -> Json<InfoResponse> {
    Json(InfoResponse {
        server_name: state.config.server_name.clone(),
        privacy_policy_url: state.config.privacy_policy_url.clone(),
    })
}

/// `GET /privacy-policy-template` — serves the bundled default policy template
/// as plain text. Dev/testing convenience only (see [`PRIVACY_POLICY_TEMPLATE`]).
async fn privacy_policy_template() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        PRIVACY_POLICY_TEMPLATE,
    )
}
