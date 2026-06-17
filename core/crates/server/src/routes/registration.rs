//! Account registration: `POST /v1/accounts`.
//!
//! Creates a new account with a `did:plc` identifier, registers the first
//! device, stores the device's prekey bundle, and returns a session token.
//! This is the only unauthenticated write endpoint (no token exists yet).
//!
//! # Security notes
//!
//! - **No authentication on registration.** Anyone can create an account.
//!   Rate limiting by IP (not yet implemented) is the primary abuse control.
//! - **DID verification.** When the client provides a DID, the server
//!   verifies it against the PLC directory: the DID must exist and the
//!   `avalanche` verification method must match the client's identity key.
//!   If no DID is provided (tests/bots), the server generates a local stub.
//! - **Prekeys are public material.** The server stores and serves public
//!   key halves; private halves never leave the client.

use axum::{extract::State, routing::post, Json, Router};
use base64::prelude::*;
use libsignal_protocol as signal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgConnection;

use crate::{
    config::{RegistrationMode, ADMINBOT_PROJECT_SLUG},
    db,
    error::ServerError,
    invite_token::{self, TokenError, PURPOSE_INVITE},
    middleware::client_ip::ClientIp,
    state::{AppState, WsPush},
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/v1/accounts", post(register))
}

#[derive(Deserialize)]
struct RegisterRequest {
    /// Client-generated DID (from PLC directory). If absent, server generates a stub.
    did: Option<String>,
    identity_key: String, // base64
    registration_id: i32,
    device_id: i32,
    signed_prekey: SignedPreKeyUpload,
    one_time_prekeys: Vec<OneTimePreKeyUpload>,
    kyber_prekey: KyberPreKeyUpload,
    /// Plaintext display name. **Bot accounts only.** Human accounts should
    /// leave this `None` — human display names are exchanged via encrypted
    /// profile bundles (client-to-client), never stored on the server.
    display_name: Option<String>,
    #[serde(default)]
    is_bot: bool,
    /// Optional reserved suffix for the server-generated `did:local:` DID.
    /// Bot accounts only. When set, the resulting DID is `did:local:{did_suffix}`
    /// instead of a random hash. Used by first-party bots (e.g. the adminbot)
    /// that need a well-known identity. Suffix must be lowercase ASCII
    /// alphanumeric, 3–32 chars.
    did_suffix: Option<String>,
    /// Encrypted recovery blob (opaque ciphertext). Contains rotation key +
    /// identity key + server list, encrypted with the user's passkey-derived
    /// symmetric key. Optional — if absent, no recovery is possible.
    recovery_blob: Option<String>, // base64
    /// Encrypted profile blob (opaque ciphertext, AES-256-GCM under the user's
    /// profile key). Optional — accounts without a profile show DID as the
    /// display name to contacts until set via `PUT /v1/profile`.
    encrypted_profile: Option<String>, // base64
    /// Ed25519 signature proving possession of the identity key.
    /// Signs the canonical payload `"register:{did}"` (base64url, no padding).
    /// Required when `did` is provided.
    identity_key_signature: Option<String>, // base64url
    /// Project-signed invite token (docs/24). Required to register under closed
    /// registration unless the caller qualifies for a bootstrap admission arm.
    /// The raw string is passed through to subscribed bots in the
    /// `AccountJoinedEvent` so they can route by its issuer + routing tags.
    invite_token: Option<String>,
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
struct RegisterResponse {
    did: String,
    session_token: String,
    expires_at: String,
}

async fn register(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(req): Json<RegisterRequest>,
) -> Result<(axum::http::StatusCode, Json<RegisterResponse>), ServerError> {
    {
        let mut conn = state.db.acquire().await?;
        if !db::ip_rate_limits::check_and_increment(
            &mut conn,
            &ip,
            crate::middleware::rate_limit::ACTION_REGISTER,
            crate::middleware::rate_limit::LIMIT_REGISTER,
            crate::middleware::rate_limit::WINDOW_REGISTER,
        )
        .await?
        {
            return Err(ServerError::RateLimited);
        }
    }

    let identity_key = BASE64_STANDARD
        .decode(&req.identity_key)
        .map_err(|_| ServerError::BadRequest("invalid base64 identity_key".into()))?;

    // Human accounts must provide a DID verified against the PLC directory,
    // plus a signature proving possession of the identity key.
    // Bot accounts may omit both.
    let did = if let Some(client_did) = &req.did {
        if !client_did.starts_with("did:plc:") {
            return Err(ServerError::BadRequest("DID must start with did:plc:".into()));
        }
        verify_did_plc(client_did, &identity_key).await?;
        verify_identity_key_signature(client_did, &state.config.server_url, &identity_key, &req.identity_key_signature)?;
        client_did.clone()
    } else if req.is_bot {
        if let Some(suffix) = &req.did_suffix {
            validate_reserved_suffix(suffix)?;
            format!("did:local:{suffix}")
        } else {
            generate_local_did(&identity_key, &state.config.server_url)
        }
    } else {
        return Err(ServerError::BadRequest(
            "did is required for non-bot accounts".into(),
        ));
    };

    if let Some(name) = &req.display_name {
        if name.len() > 100 {
            return Err(ServerError::BadRequest("display_name too long".into()));
        }
    }

    let recovery_blob = req
        .recovery_blob
        .as_deref()
        .map(|b| BASE64_STANDARD.decode(b))
        .transpose()
        .map_err(|_| ServerError::BadRequest("invalid base64 recovery_blob".into()))?;

    let mut conn = state.db.acquire().await?;

    // Closed-registration gating + single-use invite-token validation (docs/24).
    // The server validates the token locally against the issuing Project's
    // pinned key — it never calls the Project. Fails closed.
    gate_registration(&mut conn, &state, &did, &req).await?;

    // Create account.
    let account_id =
        db::accounts::create(&mut conn, &did, req.display_name.as_deref(), req.is_bot).await?;

    // Bootstrap link: if this DID is a configured adminbot identity, link it
    // into the pinned adminbot Project so its session carries superuser
    // authority. Membership is config-driven only — never settable via the
    // admin API — preserving the "superuser only via operator config"
    // invariant (docs/22).
    if state.config.adminbot_dids.iter().any(|d| d == &did) {
        let pid = db::projects::ensure_adminbot_project(&mut conn, ADMINBOT_PROJECT_SLUG).await?;
        db::projects::link_bot(&mut conn, pid, account_id).await?;
    }

    // Store recovery blob if provided.
    if let Some(blob) = &recovery_blob {
        db::accounts::update_recovery_blob(&mut conn, account_id, Some(blob)).await?;
    }

    // Store encrypted profile blob if provided.
    if let Some(profile_b64) = &req.encrypted_profile {
        let profile_blob = BASE64_STANDARD
            .decode(profile_b64)
            .map_err(|_| ServerError::BadRequest("invalid base64 encrypted_profile".into()))?;
        if profile_blob.len() > 16 * 1024 {
            return Err(ServerError::BadRequest("encrypted_profile too large".into()));
        }
        db::profiles::upsert(&mut conn, account_id, &profile_blob).await?;
    }

    // Create device.
    let device_pk = db::devices::create(
        &mut conn,
        account_id,
        req.device_id,
        &identity_key,
        req.registration_id,
    )
    .await?;

    // Store DID document.
    let did_doc = serde_json::json!({
        "@context": ["https://www.w3.org/ns/did/v1"],
        "id": did,
        "verificationMethod": [{
            "id": format!("{did}#key-1"),
            "type": "Ed25519VerificationKey2020",
            "controller": did,
            "publicKeyBase64": req.identity_key,
        }],
        "service": [{
            "id": format!("{did}#avalanche"),
            "type": "AvalancheHomeserver",
            "serviceEndpoint": state.config.server_url,
        }],
    });
    db::did::upsert_document(&mut conn, account_id, &did_doc).await?;

    // Store prekeys.
    store_prekeys(&mut conn, device_pk, &req).await?;

    // Issue session token.
    let token = generate_token();
    let expires_at =
        db::sessions::create(&mut conn, &token, device_pk, state.config.token_lifetime_secs)
            .await?;

    // Announce the new account to bots holding `subscribe.account_joined`.
    // Two paths: (1) a durable append to `server_events` so a disconnected bot
    // can catch up via `GET /v1/admin/events`; (2) a best-effort live fan-out
    // to every currently-subscribed session. The event carries the raw invite
    // token so bots can route by its issuer + routing tags (docs/22, 24).
    let joined_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    if let Err(e) = db::server_events::append_account_joined(
        &mut conn,
        &did,
        req.invite_token.as_deref(),
        joined_at_ms,
    )
    .await
    {
        // Non-fatal: the account exists; only catch-up is degraded.
        tracing::warn!(%did, "failed to append account_joined server event: {e}");
    }
    {
        let subs = state.account_joined_subscribers.read().await;
        tracing::info!(%did, subscribers = subs.len(), "fanning out AccountJoined");
        for tx in subs.values() {
            let _ = tx.send(WsPush::AccountJoined {
                did: did.clone(),
                joined_at_ms,
                invite_token: req.invite_token.clone(),
            });
        }
    }

    Ok((
        axum::http::StatusCode::CREATED,
        Json(RegisterResponse {
            did,
            session_token: token,
            expires_at: expires_at.to_string(),
        }),
    ))
}

/// Closed-registration admission + single-use invite-token validation.
///
/// In `Open` mode any registration is admitted, but a supplied token is still
/// validated and redeemed (so a token can't be silently ignored or reused). In
/// `Closed` mode a registration is admitted only if one of these holds:
///   (a) it carries a valid, unredeemed gatekeeper invite token,
///   (b) it is a bot whose reserved `did_suffix` is in the bootstrap allowlist,
///   (c) its DID is a configured adminbot identity.
/// Otherwise it fails closed (403).
///
/// The server validates the token against the issuing Project's pinned key
/// locally — it never calls the Project.
async fn gate_registration(
    conn: &mut PgConnection,
    state: &AppState,
    did: &str,
    req: &RegisterRequest,
) -> Result<(), ServerError> {
    // Validate + redeem a supplied token (both modes). Returns true if a valid
    // token was redeemed for this registration.
    let has_valid_token = if let Some(raw) = req.invite_token.as_deref() {
        let envelope = invite_token::parse_envelope(raw).map_err(map_token_err)?;

        let project = db::projects::find_by_slug(conn, &envelope.iss)
            .await?
            .ok_or_else(|| ServerError::Forbidden("unknown invite token issuer".into()))?;
        if !db::capabilities::project_has(
            conn,
            project.id,
            db::capabilities::REGISTRATION_GATEKEEPER,
        )
        .await?
        {
            return Err(ServerError::Forbidden(
                "issuer is not a registration gatekeeper".into(),
            ));
        }
        let key = project.signing_public_key.ok_or_else(|| {
            ServerError::Forbidden("gatekeeper has no registered signing key".into())
        })?;

        let claims = invite_token::verify_claims(
            &envelope,
            &key,
            &state.config.server_url,
            PURPOSE_INVITE,
            invite_token::now_unix(),
        )
        .map_err(map_token_err)?;

        // Single-use: INSERT-as-gate before account creation. A replay
        // conflicts and is rejected. A token consumed here is spent even if a
        // later step fails — the fail-closed direction.
        let redeemed =
            db::token_redemptions::try_redeem(conn, &claims.jti, &claims.iss, &claims.purpose, did)
                .await?;
        if !redeemed {
            return Err(ServerError::Forbidden("invite token already redeemed".into()));
        }
        true
    } else {
        false
    };

    match state.config.registration_mode {
        RegistrationMode::Open => Ok(()),
        RegistrationMode::Closed => {
            let bootstrap_bot = req.is_bot
                && req.did_suffix.as_deref().is_some_and(|s| {
                    state
                        .config
                        .registration_bootstrap_suffixes
                        .iter()
                        .any(|allowed| allowed == s)
                });
            let is_adminbot_did = state.config.adminbot_dids.iter().any(|d| d == did);
            if has_valid_token || bootstrap_bot || is_adminbot_did {
                Ok(())
            } else {
                Err(ServerError::Forbidden(
                    "registration is closed: a valid invite token is required".into(),
                ))
            }
        }
    }
}

/// Map an invite-token validation failure to an HTTP status: structural
/// problems are 400; admission failures are 403 (fail-closed).
fn map_token_err(e: TokenError) -> ServerError {
    match e {
        TokenError::Malformed(m) => ServerError::BadRequest(format!("invalid invite token: {m}")),
        other => ServerError::Forbidden(other.to_string()),
    }
}

async fn store_prekeys(
    conn: &mut PgConnection,
    device_pk: i64,
    req: &RegisterRequest,
) -> Result<(), ServerError> {
    let spk = &req.signed_prekey;
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

    let otpks: Vec<(i32, Vec<u8>)> = req
        .one_time_prekeys
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

    let kpk = &req.kyber_prekey;
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

/// Verify that the client actually holds the private key for the identity key.
///
/// The client signs `"register:{did}:{server_url}"` with the Ed25519 identity key.
/// This prevents an attacker from registering with someone else's public key,
/// and the server URL binding prevents cross-server replay.
fn verify_identity_key_signature(
    did: &str,
    server_url: &str,
    identity_key_bytes: &[u8],
    signature: &Option<String>,
) -> Result<(), ServerError> {
    let sig_b64 = signature
        .as_deref()
        .ok_or_else(|| ServerError::BadRequest("identity_key_signature is required".into()))?;

    let sig_bytes = BASE64_URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|_| ServerError::BadRequest("invalid base64 identity_key_signature".into()))?;

    let identity_key = signal::IdentityKey::decode(identity_key_bytes)
        .map_err(|_| ServerError::BadRequest("invalid identity_key".into()))?;

    let payload = format!("register:{did}:{server_url}");
    let valid = identity_key
        .public_key()
        .verify_signature(payload.as_bytes(), &sig_bytes);

    if !valid {
        tracing::warn!(
            "identity_key_signature failed for DID {did} \
             (server_url used in payload: {server_url})"
        );
        return Err(ServerError::BadRequest(
            "identity_key_signature verification failed".into(),
        ));
    }

    Ok(())
}

/// Verify a client-provided DID against the PLC directory.
///
/// Fetches the DID document, finds the `actnet` verification method,
/// decodes the `did:key`, and checks it matches the identity key.
async fn verify_did_plc(did: &str, identity_key: &[u8]) -> Result<(), ServerError> {
    let url = format!("https://plc.directory/{did}");
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| ServerError::Internal(format!("PLC directory request failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(ServerError::BadRequest(format!(
            "DID not found in PLC directory: {did}"
        )));
    }

    let doc: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ServerError::Internal(format!("PLC directory response parse failed: {e}")))?;

    // The resolved DID document has verificationMethod as an array of objects:
    //   { "id": "did:plc:...#avalanche", "type": "Multikey", "publicKeyMultibase": "z6Mk..." }
    // Find the entry whose id ends with "#avalanche".
    let vm_array = doc
        .get("verificationMethod")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            ServerError::BadRequest("DID document missing verificationMethod array".into())
        })?;

    let avalanche_vm = vm_array
        .iter()
        .find(|vm| {
            vm.get("id")
                .and_then(|id| id.as_str())
                .is_some_and(|id| id.ends_with("#avalanche"))
        })
        .ok_or_else(|| {
            ServerError::BadRequest("DID document missing #avalanche verification method".into())
        })?;

    let pub_key_multibase = avalanche_vm
        .get("publicKeyMultibase")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ServerError::BadRequest("avalanche verification method missing publicKeyMultibase".into())
        })?;

    // publicKeyMultibase is "z" + base58btc(multicodec_prefix + raw_key).
    // This is the same encoding as did:key without the "did:key:" prefix.
    let plc_pub_key = crate::plc::decode_did_key_ed25519(&format!("did:key:{pub_key_multibase}"))
        .map_err(|e| {
            ServerError::BadRequest(format!("invalid verification method in DID doc: {e}"))
        })?;

    // The client's identity_key is libsignal format: 0x05 prefix + 32 raw bytes.
    // Strip the prefix for comparison.
    let client_raw = if identity_key.len() == 33 && identity_key[0] == 0x05 {
        &identity_key[1..]
    } else {
        identity_key
    };

    if plc_pub_key != client_raw {
        return Err(ServerError::BadRequest(
            "identity key does not match DID document verification method".into(),
        ));
    }

    Ok(())
}

/// Generate a local-only DID for bot accounts that don't use the PLC directory.
/// Uses `did:local:` prefix to make it clear this is not a real PLC DID.
fn generate_local_did(identity_key: &[u8], server_url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(identity_key);
    hasher.update(server_url.as_bytes());
    hasher.update(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
            .to_le_bytes(),
    );
    let hash = hasher.finalize();
    let encoded = base32::encode(base32::Alphabet::Rfc4648Lower { padding: false }, &hash);
    format!("did:local:{}", &encoded[..24])
}

fn validate_reserved_suffix(suffix: &str) -> Result<(), ServerError> {
    if !(3..=32).contains(&suffix.len())
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    {
        return Err(ServerError::BadRequest(
            "did_suffix must be 3–32 lowercase alphanumeric chars".into(),
        ));
    }
    Ok(())
}

fn generate_token() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::rng().random();
    BASE64_URL_SAFE_NO_PAD.encode(bytes)
}
