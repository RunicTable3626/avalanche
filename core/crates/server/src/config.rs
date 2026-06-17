//! Server configuration loaded from environment variables.
//!
//! All values have sensible defaults for local development. In production,
//! operators set environment variables to override them. The `DATABASE_URL`
//! default points at the Docker Compose Postgres instance.
//!
//! # Safety check: dev credentials + public bind = refuse to start
//!
//! The default `DATABASE_URL` embeds the dev password `actnet-dev`. If a
//! server reaches a non-loopback bind address while still using that
//! default, it almost certainly means an operator forgot to set
//! `DATABASE_URL` in a production environment. [`Config::from_env`] panics
//! with a clear message in that case so the process exits at startup
//! instead of running with dev secrets on a public interface.

/// Sentinel value identifying the local-dev DATABASE_URL. Used by the
/// safety check in [`Config::from_env`].
const DEFAULT_DEV_DATABASE_URL: &str = "postgres://actnet:actnet-dev@localhost/actnet";

/// The hard-coded slug of the privileged adminbot Project. Adminbot authority
/// is membership in the Project with this slug (resolved via `project_bots`),
/// not a pinned DID — so adminbot may use any DID(s) and rotate freely. The
/// row is seeded at startup (`db::projects::ensure_adminbot_project`) and its
/// bot membership is set only from operator config (`ADMINBOT_DIDS`), never via
/// the admin API, which preserves the "superuser only via config" invariant
/// (docs/22 §Project-capabilities).
pub const ADMINBOT_PROJECT_SLUG: &str = "adminbot";

/// Whether new accounts may register freely or only with a valid gatekeeper
/// invite token (docs/24 closed registration). Defaults to [`Open`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationMode {
    /// Anyone may register (rate-limited by IP). The token, if present, is
    /// still validated when supplied, but is not required.
    Open,
    /// Registration is refused unless it satisfies one of the admission arms
    /// (valid gatekeeper token, bootstrap-suffix bot, or a configured adminbot
    /// DID). Fails closed.
    Closed,
}

impl RegistrationMode {
    fn from_env_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "closed" => RegistrationMode::Closed,
            _ => RegistrationMode::Open,
        }
    }
}

/// Server configuration, loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// PostgreSQL connection string.
    pub database_url: String,
    /// Address to bind the HTTP server to.
    pub bind_addr: String,
    /// The public URL of this homeserver (used in DID documents).
    pub server_url: String,
    /// Session token lifetime in seconds (default: 24 hours).
    pub token_lifetime_secs: i64,
    /// Message expiry in seconds (default: 30 days).
    pub message_expiry_secs: i64,
    /// Minimum allowed per-message expiry in seconds (default: 5 minutes).
    pub message_expiry_min_secs: i64,
    /// Maximum allowed per-message expiry in seconds (default: 30 days).
    pub message_expiry_max_secs: i64,
    /// Prekey pool low-water mark (default: 10).
    pub prekey_low_threshold: i64,
    /// Project token lifetime in seconds (default: 1 hour).
    pub project_token_lifetime_secs: i64,
    /// Installed Projects as JSON array: [{"name":"...","url":"...","description":"..."}].
    pub projects_json: String,
    /// Push relay URL (e.g. "http://localhost:3002"). If unset, push is disabled.
    pub relay_url: Option<String>,
    /// Human-readable server name (shown to users during invite/onboarding).
    pub server_name: String,
    /// Domain used for deep link URLs in invite redirects (default: go.theavalanche.net).
    pub invite_domain: String,
    /// DIDs seeded as members of the adminbot Project at startup (and admitted
    /// under closed registration). These are the server's superusers: a caller
    /// is adminbot iff its account is linked to the [`ADMINBOT_PROJECT_SLUG`]
    /// Project, and the only way into that membership is this config list
    /// (never the admin API). Set via `ADMINBOT_DIDS` (comma-separated);
    /// defaults to the reserved well-known DID `did:local:adminbot`.
    pub adminbot_dids: Vec<String>,
    /// Whether registration is open to anyone or gated on a gatekeeper invite
    /// token (docs/24). Set via `REGISTRATION_MODE=open|closed`; default open.
    pub registration_mode: RegistrationMode,
    /// Reserved `did:local:` bot suffixes admitted under closed registration
    /// without a token (bootstrap allowlist — e.g. first-party bots like the
    /// adminbot). Set via `REGISTRATION_BOOTSTRAP_SUFFIXES` (comma-separated);
    /// defaults to `["adminbot"]`.
    pub registration_bootstrap_suffixes: Vec<String>,
}

impl Config {
    pub fn from_env() -> Self {
        let config = Self::from_env_unchecked();
        config.assert_safe_to_start();
        config
    }

    fn from_env_unchecked() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| DEFAULT_DEV_DATABASE_URL.to_string()),
            bind_addr: std::env::var("BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            server_url: std::env::var("SERVER_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            token_lifetime_secs: std::env::var("TOKEN_LIFETIME_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(86400),
            message_expiry_secs: std::env::var("MESSAGE_EXPIRY_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30 * 86400),
            message_expiry_min_secs: std::env::var("MESSAGE_EXPIRY_MIN_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
            message_expiry_max_secs: std::env::var("MESSAGE_EXPIRY_MAX_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30 * 86400),
            prekey_low_threshold: std::env::var("PREKEY_LOW_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            project_token_lifetime_secs: std::env::var("PROJECT_TOKEN_LIFETIME_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600),
            projects_json: std::env::var("PROJECTS")
                .unwrap_or_else(|_| "[]".to_string()),
            relay_url: std::env::var("RELAY_URL").ok(),
            server_name: std::env::var("SERVER_NAME")
                .unwrap_or_else(|_| "Avalanche Server".to_string()),
            invite_domain: std::env::var("INVITE_DOMAIN")
                .unwrap_or_else(|_| "go.theavalanche.net".to_string()),
            adminbot_dids: parse_csv_env("ADMINBOT_DIDS")
                .unwrap_or_else(|| vec!["did:local:adminbot".to_string()]),
            registration_mode: std::env::var("REGISTRATION_MODE")
                .ok()
                .map(|s| RegistrationMode::from_env_str(&s))
                .unwrap_or(RegistrationMode::Open),
            registration_bootstrap_suffixes: parse_csv_env("REGISTRATION_BOOTSTRAP_SUFFIXES")
                .unwrap_or_else(|| vec!["adminbot".to_string()]),
        }
    }

    /// Panic with a clear message if the configuration looks like a
    /// production server still running with the dev DATABASE_URL. Called
    /// at process startup from [`Config::from_env`].
    ///
    /// Local dev needs to bind to `0.0.0.0` so iOS devices on the LAN can
    /// reach the server, *and* uses the dev DATABASE_URL — exactly the
    /// combination this check rejects. The documented dev recipes
    /// (`make dev`, `dev.py`) set `ACTNET_ALLOW_DEV_DB=1` to opt in. The
    /// opt-in is loud and intentional; production deploys never set it.
    fn assert_safe_to_start(&self) {
        let on_loopback = is_loopback_bind(&self.bind_addr);
        let is_dev_db = self.database_url == DEFAULT_DEV_DATABASE_URL;
        if !is_dev_db || on_loopback {
            return;
        }

        let allow_dev_db = std::env::var("ACTNET_ALLOW_DEV_DB").ok().as_deref() == Some("1");
        if allow_dev_db {
            tracing::warn!(
                bind_addr = %self.bind_addr,
                "running with dev DATABASE_URL on non-loopback bind \
                 (ACTNET_ALLOW_DEV_DB=1 set; intended for local dev only)"
            );
            return;
        }

        panic!(
            "\n\nrefusing to start: DATABASE_URL is the dev default \
             but BIND_ADDR='{}' is not loopback.\n\n\
             This combination almost always means a production deploy \
             forgot to set DATABASE_URL. Either:\n  \
             - set DATABASE_URL to your real database URL (production), or\n  \
             - set BIND_ADDR=127.0.0.1:<port> (loopback-only dev), or\n  \
             - set ACTNET_ALLOW_DEV_DB=1 (LAN-accessible dev with dev DB).\n",
            self.bind_addr,
        );
    }
}

/// Parse a comma-separated env var into a list of trimmed, non-empty values.
/// Returns `None` if the var is unset or empty (so callers can apply a
/// default), `Some(vec)` otherwise.
fn parse_csv_env(name: &str) -> Option<Vec<String>> {
    let raw = std::env::var(name).ok()?;
    let items: Vec<String> = raw
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

/// True if the given `host:port` (or `[ipv6]:port`) binds to a loopback
/// address only — so the server can't be reached from the network. Used
/// to decide whether dev credentials are safe to leave in place.
fn is_loopback_bind(addr: &str) -> bool {
    let Some((host, _port)) = addr.rsplit_once(':') else {
        return false;
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host == "localhost" {
        return true;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(ip) => ip.is_loopback(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_detection() {
        assert!(is_loopback_bind("127.0.0.1:3000"));
        assert!(is_loopback_bind("localhost:3000"));
        assert!(is_loopback_bind("[::1]:3000"));
        assert!(!is_loopback_bind("0.0.0.0:3000"));
        assert!(!is_loopback_bind("192.168.1.5:3000"));
        assert!(!is_loopback_bind("[::]:3000"));
        assert!(!is_loopback_bind("garbage:3000"));
        assert!(!is_loopback_bind("no-colon"));
    }

    #[test]
    fn dev_db_with_loopback_is_allowed() {
        let mut c = Config::from_env_unchecked();
        c.database_url = DEFAULT_DEV_DATABASE_URL.to_string();
        c.bind_addr = "127.0.0.1:3000".to_string();
        c.assert_safe_to_start(); // should not panic
    }

    #[test]
    #[should_panic(expected = "refusing to start")]
    fn dev_db_with_public_bind_panics() {
        let mut c = Config::from_env_unchecked();
        c.database_url = DEFAULT_DEV_DATABASE_URL.to_string();
        c.bind_addr = "0.0.0.0:3000".to_string();
        c.assert_safe_to_start();
    }

    #[test]
    fn explicit_db_url_with_public_bind_is_allowed() {
        let mut c = Config::from_env_unchecked();
        c.database_url = "postgres://prod-user:s3cret@db.internal/avalanche".to_string();
        c.bind_addr = "0.0.0.0:3000".to_string();
        c.assert_safe_to_start(); // should not panic
    }

    #[test]
    fn registration_mode_parse() {
        assert_eq!(RegistrationMode::from_env_str("closed"), RegistrationMode::Closed);
        assert_eq!(RegistrationMode::from_env_str("CLOSED"), RegistrationMode::Closed);
        assert_eq!(RegistrationMode::from_env_str(" closed "), RegistrationMode::Closed);
        assert_eq!(RegistrationMode::from_env_str("open"), RegistrationMode::Open);
        // Anything unrecognized defaults to Open (fail-safe for availability,
        // not security — closing registration is the deliberate opt-in).
        assert_eq!(RegistrationMode::from_env_str("garbage"), RegistrationMode::Open);
        assert_eq!(RegistrationMode::from_env_str(""), RegistrationMode::Open);
    }
}
