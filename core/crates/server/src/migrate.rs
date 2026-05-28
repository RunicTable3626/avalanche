//! Schema migrations embedded into the server binary.
//!
//! `sqlx::migrate!` walks `infra/migrations/*.sql` at compile time and bakes
//! each file (plus its BLAKE3 checksum) into the binary. At runtime, [`run`]
//! creates the `_sqlx_migrations` tracking table if needed and applies any
//! files whose version has not yet been recorded — so it's idempotent and
//! safe to call against an already-migrated database.
//!
//! Invoked explicitly via the `migrate` subcommand on the server binary —
//! never on startup, so a bad migration cannot crash-loop the service.

use sqlx::PgPool;

pub async fn run(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("../../../infra/migrations").run(pool).await
}
