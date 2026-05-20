//! HTTP-level integration tests for the DID resolution endpoint.
//!
//! Uses tower's `oneshot` to drive the full Axum handler stack in-process.
//! Requires `TEST_DATABASE_URL` to be set and the schema to be applied.
//!
//! Unlike `db_tests.rs`, these tests cannot use the transaction-rollback
//! pattern because handlers manage their own connections. Each registration
//! call generates a unique DID (nanosecond timestamp entropy), so leftover
//! rows are benign.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::prelude::*;
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

use server::{config::Config, routes, state::AppState};

async fn test_state() -> AppState {
    let url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must be set to run server tests");
    let pool = PgPool::connect(&url).await.expect("failed to connect to test database");
    let config = Config {
        database_url: url,
        bind_addr: "0.0.0.0:0".into(),
        server_url: "http://localhost:3000".into(),
        token_lifetime_secs: 86400,
        message_expiry_secs: 30 * 86400,
        prekey_low_threshold: 10,
        project_token_lifetime_secs: 3600,
        projects_json: "[]".into(),
        relay_url: None,
    };
    AppState::new(pool, config)
}

/// Register a dummy account and return the parsed response body.
async fn register_dummy(app: &axum::Router) -> Value {
    let body = serde_json::json!({
        "identity_key":     BASE64_STANDARD.encode([1u8; 32]),
        "registration_id":  1,
        "device_id":        1,
        "signed_prekey": {
            "id":         1,
            "public_key": BASE64_STANDARD.encode([2u8; 32]),
            "signature":  BASE64_STANDARD.encode([3u8; 64]),
        },
        "one_time_prekeys": [{ "id": 1, "public_key": BASE64_STANDARD.encode([4u8; 32]) }],
        "kyber_prekey": {
            "id":         1,
            "public_key": BASE64_STANDARD.encode([5u8; 32]),
            "signature":  BASE64_STANDARD.encode([6u8; 64]),
        }
    });

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/accounts")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED, "registration must succeed");
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ── DID resolution endpoint tests ────────────────────────────────────────────

#[tokio::test]
async fn resolve_did_returns_document() {
    let app = routes::router().with_state(test_state().await);

    let reg = register_dummy(&app).await;
    let did = reg["did"].as_str().unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/.well-known/did/{did}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let doc: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(doc["id"], did);
    assert_eq!(doc["verificationMethod"][0]["controller"], did);
    assert_eq!(doc["service"][0]["type"], "ActnetHomeserver");
    assert_eq!(doc["service"][0]["serviceEndpoint"], "http://localhost:3000");
}

#[tokio::test]
async fn resolve_unknown_did_returns_404() {
    let app = routes::router().with_state(test_state().await);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/.well-known/did/did:plc:doesnotexist0000000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
