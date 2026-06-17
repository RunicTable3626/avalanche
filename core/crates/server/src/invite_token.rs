//! Project-signed invite/registration token format + verification (docs/24).
//!
//! The server admits a closed-registration account only if it presents a token
//! a gatekeeper Project signed. The server **never calls the Project**: it pins
//! the Project's Ed25519 public key (registered when `registration.gatekeeper`
//! is granted) and verifies the signature locally, failing closed.
//!
//! # Wire format
//!
//! A token is `base64url(JSON)` of an *envelope*:
//!
//! ```json
//! { "server_url": "...", "iss": "<project-slug>",
//!   "claims": "<base64url(claims-json)>", "sig": "<base64url(ed25519 sig)>" }
//! ```
//!
//! The top-level `server_url`/`iss` are **untrusted hints** — the substrate
//! reads `server_url` to know which server to call (docs/51), and the server
//! reads `iss` to pick which pinned key to verify against. Authority lives only
//! in the signed `claims`:
//!
//! ```json
//! { "server_url": "...", "iss": "<project-slug>", "exp": <unix-secs>,
//!   "jti": "<unique-id>", "purpose": "invite", "routing": { ... } }
//! ```
//!
//! The signature covers the exact `claims` base64url string, so there is no
//! cross-language JSON-canonicalization hazard. `purpose` lets one signing key
//! serve multiple token kinds safely (the server gates by purpose → capability,
//! not by the key) — `"invite"` for human onboarding today, room for `"bot"`
//! signup later.

use base64::prelude::*;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// `purpose` value for a human-onboarding invite (the only kind accepted on the
/// registration path today).
pub const PURPOSE_INVITE: &str = "invite";

/// The untrusted outer envelope.
#[derive(Debug, Deserialize)]
pub struct InviteEnvelope {
    #[allow(dead_code)]
    pub server_url: String,
    pub iss: String,
    /// base64url(claims JSON) — the exact bytes the signature covers.
    pub claims: String,
    /// base64url(Ed25519 signature over the `claims` string bytes).
    pub sig: String,
}

/// The signed claims — the authoritative content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InviteClaims {
    pub server_url: String,
    pub iss: String,
    /// Expiry, unix epoch seconds.
    pub exp: i64,
    /// Unique token id; single-use redemption key.
    pub jti: String,
    pub purpose: String,
    /// Opaque routing payload the gatekeeper controls; carried through to the
    /// `AccountJoinedEvent` for the post-join router. The server does not
    /// interpret it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing: Option<serde_json::Value>,
}

/// Why a token was rejected. The route maps these to HTTP statuses: structural
/// problems are 400 (BadRequest); admission failures (signature/issuer/expiry/
/// purpose/server) are 403 (Forbidden, fail-closed).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TokenError {
    #[error("malformed token: {0}")]
    Malformed(String),
    #[error("invalid signature")]
    InvalidSignature,
    #[error("issuer mismatch")]
    IssuerMismatch,
    #[error("token expired")]
    Expired,
    #[error("token is for a different server")]
    WrongServer,
    #[error("unexpected token purpose")]
    WrongPurpose,
}

/// Decode the outer envelope from the base64url token string.
pub fn parse_envelope(token: &str) -> Result<InviteEnvelope, TokenError> {
    let bytes = BASE64_URL_SAFE_NO_PAD
        .decode(token.trim())
        .map_err(|_| TokenError::Malformed("invalid base64url".into()))?;
    serde_json::from_slice(&bytes).map_err(|_| TokenError::Malformed("invalid envelope JSON".into()))
}

/// Verify the envelope's signature against `public_key` and decode + check the
/// claims. Returns the validated claims on success.
///
/// Checks, in order: Ed25519 signature over the `claims` bytes; `claims.iss`
/// equals the envelope `iss` (so the untrusted hint can't redirect to a
/// different signed issuer); `server_url`; `purpose`; expiry against `now`.
pub fn verify_claims(
    envelope: &InviteEnvelope,
    public_key: &[u8],
    expected_server_url: &str,
    expected_purpose: &str,
    now_unix: i64,
) -> Result<InviteClaims, TokenError> {
    // Verifying key (32 bytes).
    let key_bytes: [u8; 32] = public_key
        .try_into()
        .map_err(|_| TokenError::Malformed("signing key not 32 bytes".into()))?;
    let verifying_key =
        VerifyingKey::from_bytes(&key_bytes).map_err(|_| TokenError::InvalidSignature)?;

    // Signature (64 bytes).
    let sig_bytes = BASE64_URL_SAFE_NO_PAD
        .decode(&envelope.sig)
        .map_err(|_| TokenError::Malformed("invalid base64url sig".into()))?;
    let signature =
        Signature::from_slice(&sig_bytes).map_err(|_| TokenError::Malformed("bad sig length".into()))?;

    // Signature covers the exact `claims` base64url string bytes.
    verifying_key
        .verify(envelope.claims.as_bytes(), &signature)
        .map_err(|_| TokenError::InvalidSignature)?;

    // Decode the now-authenticated claims.
    let claims_bytes = BASE64_URL_SAFE_NO_PAD
        .decode(&envelope.claims)
        .map_err(|_| TokenError::Malformed("invalid base64url claims".into()))?;
    let claims: InviteClaims = serde_json::from_slice(&claims_bytes)
        .map_err(|_| TokenError::Malformed("invalid claims JSON".into()))?;

    if claims.iss != envelope.iss {
        return Err(TokenError::IssuerMismatch);
    }
    if claims.server_url.trim_end_matches('/') != expected_server_url.trim_end_matches('/') {
        return Err(TokenError::WrongServer);
    }
    if claims.purpose != expected_purpose {
        return Err(TokenError::WrongPurpose);
    }
    if claims.exp <= now_unix {
        return Err(TokenError::Expired);
    }

    Ok(claims)
}

/// Current unix epoch seconds (wall clock). Isolated so the route stays terse.
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Issue a token by signing `claims` with `signing_key`. This is what a
/// gatekeeper Project does; the server only verifies. Exposed for tests (and
/// any in-tree gatekeeper) so the canonical format lives in one place.
pub fn issue(signing_key: &ed25519_dalek::SigningKey, claims: &InviteClaims) -> String {
    use ed25519_dalek::Signer;
    let claims_json = serde_json::to_vec(claims).expect("claims serialize");
    let claims_b64 = BASE64_URL_SAFE_NO_PAD.encode(&claims_json);
    let sig = signing_key.sign(claims_b64.as_bytes());
    let envelope = serde_json::json!({
        "server_url": claims.server_url,
        "iss": claims.iss,
        "claims": claims_b64,
        "sig": BASE64_URL_SAFE_NO_PAD.encode(sig.to_bytes()),
    });
    BASE64_URL_SAFE_NO_PAD.encode(serde_json::to_vec(&envelope).expect("envelope serialize"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    const SERVER: &str = "http://localhost:3000";

    fn signer() -> SigningKey {
        // Deterministic key from fixed seed bytes — no rng dependency.
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn claims(exp: i64, purpose: &str) -> InviteClaims {
        InviteClaims {
            server_url: SERVER.into(),
            iss: "vetting".into(),
            exp,
            jti: "tok-1".into(),
            purpose: purpose.into(),
            routing: Some(serde_json::json!({ "audience": "northeast" })),
        }
    }

    #[test]
    fn round_trip_valid() {
        let sk = signer();
        let pk = sk.verifying_key().to_bytes();
        let token = issue(&sk, &claims(1_000, PURPOSE_INVITE));
        let env = parse_envelope(&token).unwrap();
        let got = verify_claims(&env, &pk, SERVER, PURPOSE_INVITE, 500).unwrap();
        assert_eq!(got.jti, "tok-1");
        assert_eq!(got.iss, "vetting");
        assert_eq!(got.routing.unwrap()["audience"], "northeast");
    }

    #[test]
    fn expired_rejected() {
        let sk = signer();
        let pk = sk.verifying_key().to_bytes();
        let token = issue(&sk, &claims(1_000, PURPOSE_INVITE));
        let env = parse_envelope(&token).unwrap();
        assert_eq!(
            verify_claims(&env, &pk, SERVER, PURPOSE_INVITE, 1_000),
            Err(TokenError::Expired)
        );
    }

    #[test]
    fn wrong_purpose_rejected() {
        let sk = signer();
        let pk = sk.verifying_key().to_bytes();
        let token = issue(&sk, &claims(1_000, "bot"));
        let env = parse_envelope(&token).unwrap();
        assert_eq!(
            verify_claims(&env, &pk, SERVER, PURPOSE_INVITE, 500),
            Err(TokenError::WrongPurpose)
        );
    }

    #[test]
    fn wrong_key_rejected() {
        let sk = signer();
        let other_pk = SigningKey::from_bytes(&[9u8; 32]).verifying_key().to_bytes();
        let token = issue(&sk, &claims(1_000, PURPOSE_INVITE));
        let env = parse_envelope(&token).unwrap();
        assert_eq!(
            verify_claims(&env, &other_pk, SERVER, PURPOSE_INVITE, 500),
            Err(TokenError::InvalidSignature)
        );
    }

    #[test]
    fn tampered_claims_rejected() {
        let sk = signer();
        let pk = sk.verifying_key().to_bytes();
        let token = issue(&sk, &claims(1_000, PURPOSE_INVITE));
        let mut env = parse_envelope(&token).unwrap();
        // Swap in a different claims blob; signature no longer matches.
        let forged = BASE64_URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&claims(9_999, PURPOSE_INVITE)).unwrap(),
        );
        env.claims = forged;
        assert_eq!(
            verify_claims(&env, &pk, SERVER, PURPOSE_INVITE, 500),
            Err(TokenError::InvalidSignature)
        );
    }

    #[test]
    fn issuer_mismatch_rejected() {
        let sk = signer();
        let pk = sk.verifying_key().to_bytes();
        let token = issue(&sk, &claims(1_000, PURPOSE_INVITE));
        let mut env = parse_envelope(&token).unwrap();
        env.iss = "someone-else".into(); // hint disagrees with signed claims.iss
        assert_eq!(
            verify_claims(&env, &pk, SERVER, PURPOSE_INVITE, 500),
            Err(TokenError::IssuerMismatch)
        );
    }

    #[test]
    fn malformed_token_rejected() {
        assert!(matches!(
            parse_envelope("!!!not base64!!!"),
            Err(TokenError::Malformed(_))
        ));
    }
}
