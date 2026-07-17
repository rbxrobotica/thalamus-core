//! Migration runner for the durable audit store (master plan §2).
//!
//! Runs with `thalamus_migrator` credentials — migrations are owned
//! exclusively by Thalamus. Usage:
//!
//! ```text
//! THALAMUS_MIGRATE_DATABASE_URL=postgres://thalamus_migrator:...@jaguar:5432/thalamus \
//!     thalamus-migrate
//! ```

use thalamus_postgres_adapter::PostgresAudit;

fn main() {
    let url = std::env::var("THALAMUS_MIGRATE_DATABASE_URL").unwrap_or_else(|_| {
        eprintln!("THALAMUS_MIGRATE_DATABASE_URL is required (thalamus_migrator DSN)");
        std::process::exit(2);
    });
    match PostgresAudit::run_migrations(&url) {
        Ok(applied) if applied.is_empty() => println!("schema up to date; nothing to apply"),
        Ok(applied) => {
            for version in applied {
                println!("applied {version}");
            }
        }
        Err(err) => {
            eprintln!("migration failed: {err}");
            std::process::exit(1);
        }
    }
}
