//! Device replacement: `POST /v1/devices/replace`.
//!
//! Authenticated by a **rotation key signature** (not a session token).
//! Used during recovery after device loss: the client presents its DID,
//! a signed payload proving possession of the rotation key, and the new
//! device's credentials. The server revokes the old device (invalidating
//! its session tokens and prekey bundles) and registers the new one.
//!
//! The rotation key is a P-256 keypair. The server verifies the ECDSA
//! signature over a canonical payload containing the DID, old device_id,
//! new device_id, and a server-issued nonce (to prevent replay).
//!
//! # Flow
//!
//! 1. Client calls `POST /v1/auth/challenge` with `{ did, device_id }` using
//!    the **old** device_id to get a nonce.
//! 2. Client constructs the replacement payload and signs it with the rotation key.
//! 3. Client calls `POST /v1/devices/replace` with the signed payload + new device info.
//! 4. Server verifies the rotation key signature, deletes the old device, registers
//!    the new one, and returns a session token.

use axum::{extract::State, routing::post, Json, Router};
use base64::prelude::*;
use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::{db, error::ServerError, middleware::client_ip::ClientIp, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/devices/replace", post(replace_device))
        .route("/v1/devices/link", post(link_device))
}

#[derive(Deserialize)]
struct ReplaceDeviceRequest {
    did: String,
    old_device_id: i32,
    new_device_id: i32,
    new_identity_key: String,    // base64
    new_registration_id: i32,
    /// The nonce from `POST /v1/auth/challenge` (issued for the old device).
    nonce: String,
    /// ECDSA P-256 signature over the canonical payload:
    /// `"replace:{did}:{old_device_id}:{new_device_id}:{nonce}"`
    rotation_key_signature: String, // base64
    /// The P-256 public key (SEC1 compressed or uncompressed) that signed the payload.
    /// The server verifies this matches the DID document's rotation key.
    rotation_key: String, // base64
    // Prekeys for the new device:
    signed_prekey: SignedPreKeyUpload,
    one_time_prekeys: Vec<OneTimePreKeyUpload>,
    kyber_prekey: KyberPreKeyUpload,
    /// Updated recovery blob (re-encrypted with the new device's state).
    recovery_blob: Option<String>, // base64
}

#[derive(Deserialize)]
struct SignedPreKeyUpload {
    id: i32,
    public_key: String, // base64
    signature: String,  // base64
}

#[derive(Deserialize)]
struct OneTimePreKeyUpload {
    id: i32,
    public_key: String, // base64
}

#[derive(Deserialize)]
struct KyberPreKeyUpload {
    id: i32,
    public_key: String, // base64
    signature: String,  // base64
}

#[derive(Serialize)]
struct ReplaceDeviceResponse {
    session_token: String,
    expires_at: String,
}

async fn replace_device(
    State(state): State<AppState>,
    Json(req): Json<ReplaceDeviceRequest>,
) -> Result<Json<ReplaceDeviceResponse>, ServerError> {
    // Decode the rotation key.
    let rotation_key_bytes = BASE64_STANDARD
        .decode(&req.rotation_key)
        .map_err(|_| ServerError::BadRequest("invalid base64 rotation_key".into()))?;
    let verifying_key = VerifyingKey::from_sec1_bytes(&rotation_key_bytes)
        .map_err(|_| ServerError::BadRequest("invalid P-256 rotation key".into()))?;

    // Construct the canonical payload that was signed.
    let payload = format!(
        "replace:{}:{}:{}:{}",
        req.did, req.old_device_id, req.new_device_id, req.nonce
    );

    // Verify the rotation key signature.
    let sig_bytes = BASE64_STANDARD
        .decode(&req.rotation_key_signature)
        .map_err(|_| ServerError::BadRequest("invalid base64 rotation_key_signature".into()))?;
    let signature = Signature::from_der(&sig_bytes)
        .or_else(|_| Signature::from_slice(&sig_bytes))
        .map_err(|_| ServerError::BadRequest("invalid ECDSA signature".into()))?;
    verifying_key
        .verify(payload.as_bytes(), &signature)
        .map_err(|_| ServerError::Unauthorized)?;

    // Verify the rotation key is in the PLC directory's authorized
    // rotationKeys list for this DID. Without this check, a valid
    // self-signature proves nothing — anyone can sign with their own
    // freshly generated key.
    //
    // did:local: identifiers (bot accounts) are not in the PLC directory
    // and cannot use this flow.
    if !req.did.starts_with("did:plc:") {
        return Err(ServerError::BadRequest(
            "device replacement requires a did:plc: identifier".into(),
        ));
    }
    let submitted_compressed = verifying_key.to_encoded_point(true).as_bytes().to_vec();
    let authorized = crate::plc::fetch_rotation_keys_p256(&req.did).await?;
    if !authorized.iter().any(|k| k == &submitted_compressed) {
        tracing::warn!(
            did = %req.did,
            "device replace rejected: submitted rotation key not in PLC rotationKeys"
        );
        return Err(ServerError::Unauthorized);
    }

    let mut conn = state.db.acquire().await?;

    // Look up the account and old device.
    let account = db::accounts::find_by_did(&mut conn, &req.did)
        .await?
        .ok_or(ServerError::NotFound)?;

    let old_device = db::devices::find(&mut conn, account.id, req.old_device_id)
        .await?
        .ok_or(ServerError::NotFound)?;

    // Consume the auth challenge nonce (must have been issued for the old device).
    let challenge_device_pk = db::challenges::consume(&mut conn, &req.nonce)
        .await?
        .ok_or(ServerError::Unauthorized)?;

    if challenge_device_pk != old_device.id {
        return Err(ServerError::Unauthorized);
    }

    // Decode the new identity key.
    let new_identity_key = BASE64_STANDARD
        .decode(&req.new_identity_key)
        .map_err(|_| ServerError::BadRequest("invalid base64 new_identity_key".into()))?;

    // Delete the old device (cascades to tokens, prekeys, messages).
    db::devices::delete(&mut conn, old_device.id).await?;

    // Register the new device.
    let new_device_pk = db::devices::create(
        &mut conn,
        account.id,
        req.new_device_id,
        &new_identity_key,
        req.new_registration_id,
    )
    .await?;

    // Store prekeys for the new device.
    store_prekeys(
        &mut conn,
        new_device_pk,
        &req.signed_prekey,
        &req.one_time_prekeys,
        &req.kyber_prekey,
    )
    .await?;

    // Update recovery blob if provided.
    if let Some(blob_b64) = &req.recovery_blob {
        let blob = BASE64_STANDARD
            .decode(blob_b64)
            .map_err(|_| ServerError::BadRequest("invalid base64 recovery_blob".into()))?;
        db::accounts::update_recovery_blob(&mut conn, account.id, Some(&blob)).await?;
    }

    // Issue a session token for the new device.
    let token = generate_token();
    let expires_at =
        db::sessions::create(&mut conn, &token, new_device_pk, state.config.token_lifetime_secs)
            .await?;

    Ok(Json(ReplaceDeviceResponse {
        session_token: token,
        expires_at: expires_at.to_string(),
    }))
}

// ── POST /v1/devices/link ───────────────────────────────────────────────────
//
// Additive sibling of /replace (docs/04 §4): links a *new* device to an
// existing identity without deleting any device, so the existing device set
// stays intact and fan-out reaches the new device too. Rotation-key authorized
// like /replace — the new device transports the rotation key over the
// provisioning channel to sign this. The new device builds its own per-device
// state (registration id, prekeys) and pulls durable state via the storage
// service afterward.

#[derive(Deserialize)]
struct LinkDeviceRequest {
    did: String,
    new_device_id: i32,
    /// The shared identity public key — the same key the identity's other
    /// devices publish (docs/04 §1), transported to the new device by linking.
    new_identity_key: String, // base64
    new_registration_id: i32,
    /// Nonce from `POST /v1/auth/challenge`, issued for any existing device of
    /// the account. The rotation signature is the real auth; the nonce is only
    /// anti-replay, so binding it to the account (not a specific device) is fine.
    nonce: String,
    /// ECDSA P-256 signature over `"linkdevice:{did}:{new_device_id}:{nonce}"`.
    rotation_key_signature: String, // base64
    /// The P-256 rotation public key (SEC1) that signed the payload; verified
    /// against the DID document's rotationKeys.
    rotation_key: String, // base64
    signed_prekey: SignedPreKeyUpload,
    one_time_prekeys: Vec<OneTimePreKeyUpload>,
    kyber_prekey: KyberPreKeyUpload,
}

#[derive(Serialize)]
struct LinkDeviceResponse {
    session_token: String,
    expires_at: String,
    /// The device_id the new device was registered under (echoed back).
    device_id: i32,
}

async fn link_device(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(req): Json<LinkDeviceRequest>,
) -> Result<Json<LinkDeviceResponse>, ServerError> {
    // Decode + verify the rotation key signature over the canonical payload.
    let rotation_key_bytes = BASE64_STANDARD
        .decode(&req.rotation_key)
        .map_err(|_| ServerError::BadRequest("invalid base64 rotation_key".into()))?;
    let verifying_key = VerifyingKey::from_sec1_bytes(&rotation_key_bytes)
        .map_err(|_| ServerError::BadRequest("invalid P-256 rotation key".into()))?;

    let payload = format!("linkdevice:{}:{}:{}", req.did, req.new_device_id, req.nonce);

    let sig_bytes = BASE64_STANDARD
        .decode(&req.rotation_key_signature)
        .map_err(|_| ServerError::BadRequest("invalid base64 rotation_key_signature".into()))?;
    let signature = Signature::from_der(&sig_bytes)
        .or_else(|_| Signature::from_slice(&sig_bytes))
        .map_err(|_| ServerError::BadRequest("invalid ECDSA signature".into()))?;
    verifying_key
        .verify(payload.as_bytes(), &signature)
        .map_err(|_| ServerError::Unauthorized)?;

    // The rotation key must be in the DID's PLC rotationKeys (a valid
    // self-signature alone proves nothing). did:local: bots can't link.
    if !req.did.starts_with("did:plc:") {
        return Err(ServerError::BadRequest(
            "device linking requires a did:plc: identifier".into(),
        ));
    }
    let submitted_compressed = verifying_key.to_encoded_point(true).as_bytes().to_vec();
    let authorized = crate::plc::fetch_rotation_keys_p256(&req.did).await?;
    if !authorized.iter().any(|k| k == &submitted_compressed) {
        tracing::warn!(
            did = %req.did,
            "device link rejected: submitted rotation key not in PLC rotationKeys"
        );
        return Err(ServerError::Unauthorized);
    }

    let mut conn = state.db.acquire().await?;

    if !db::ip_rate_limits::check_and_increment(
        &mut conn,
        &ip,
        crate::middleware::rate_limit::ACTION_DEVICE_LINK,
        crate::middleware::rate_limit::LIMIT_DEVICE_LINK,
        crate::middleware::rate_limit::WINDOW_DEVICE_LINK,
    )
    .await?
    {
        return Err(ServerError::RateLimited);
    }

    let account = db::accounts::find_by_did(&mut conn, &req.did)
        .await?
        .ok_or(ServerError::NotFound)?;

    // Consume the anti-replay nonce. It must belong to a device of *this*
    // account (the new device gets it by challenging an existing device).
    let challenge_device_pk = db::challenges::consume(&mut conn, &req.nonce)
        .await?
        .ok_or(ServerError::Unauthorized)?;
    let challenge_device = db::devices::find_by_pk(&mut conn, challenge_device_pk)
        .await?
        .ok_or(ServerError::Unauthorized)?;
    if challenge_device.account_id != account.id {
        return Err(ServerError::Unauthorized);
    }

    // The new device_id must be free — linking is additive and must never
    // clobber an existing device row.
    if db::devices::find(&mut conn, account.id, req.new_device_id)
        .await?
        .is_some()
    {
        return Err(ServerError::Conflict("device_id already in use".into()));
    }

    let new_identity_key = BASE64_STANDARD
        .decode(&req.new_identity_key)
        .map_err(|_| ServerError::BadRequest("invalid base64 new_identity_key".into()))?;

    let new_device_pk = db::devices::create(
        &mut conn,
        account.id,
        req.new_device_id,
        &new_identity_key,
        req.new_registration_id,
    )
    .await?;

    store_prekeys(
        &mut conn,
        new_device_pk,
        &req.signed_prekey,
        &req.one_time_prekeys,
        &req.kyber_prekey,
    )
    .await?;

    let token = generate_token();
    let expires_at =
        db::sessions::create(&mut conn, &token, new_device_pk, state.config.token_lifetime_secs)
            .await?;

    Ok(Json(LinkDeviceResponse {
        session_token: token,
        expires_at: expires_at.to_string(),
        device_id: req.new_device_id,
    }))
}

async fn store_prekeys(
    conn: &mut sqlx::PgConnection,
    device_pk: i64,
    signed_prekey: &SignedPreKeyUpload,
    one_time_prekeys: &[OneTimePreKeyUpload],
    kyber_prekey: &KyberPreKeyUpload,
) -> Result<(), ServerError> {
    let spk = signed_prekey;
    db::prekeys::upsert_signed(
        conn,
        device_pk,
        spk.id,
        &BASE64_STANDARD
            .decode(&spk.public_key)
            .map_err(|_| ServerError::BadRequest("invalid base64 signed_prekey".into()))?,
        &BASE64_STANDARD
            .decode(&spk.signature)
            .map_err(|_| ServerError::BadRequest("invalid base64 signed_prekey signature".into()))?,
    )
    .await?;

    let otpks: Vec<(i32, Vec<u8>)> = one_time_prekeys
        .iter()
        .map(|k| {
            Ok((
                k.id,
                BASE64_STANDARD
                    .decode(&k.public_key)
                    .map_err(|_| ServerError::BadRequest("invalid base64 one_time_prekey".into()))?,
            ))
        })
        .collect::<Result<_, ServerError>>()?;
    db::prekeys::insert_one_time_batch(conn, device_pk, &otpks).await?;

    let kpk = kyber_prekey;
    db::prekeys::upsert_kyber(
        conn,
        device_pk,
        kpk.id,
        &BASE64_STANDARD
            .decode(&kpk.public_key)
            .map_err(|_| ServerError::BadRequest("invalid base64 kyber_prekey".into()))?,
        &BASE64_STANDARD
            .decode(&kpk.signature)
            .map_err(|_| ServerError::BadRequest("invalid base64 kyber_prekey signature".into()))?,
    )
    .await?;

    Ok(())
}

fn generate_token() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::rng().random();
    BASE64_URL_SAFE_NO_PAD.encode(bytes)
}
