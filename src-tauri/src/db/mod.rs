use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, SqlitePool};
use std::{path::PathBuf, str::FromStr};
use tauri::AppHandle;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("data dir unavailable")]
    DataDirMissing,
    #[error("invalid database path")]
    InvalidPath,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

pub async fn init_pool(app: &AppHandle) -> Result<SqlitePool, DbError> {
    let db_path = db_path(app)?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let options = SqliteConnectOptions::from_str(db_path.to_str().ok_or(DbError::InvalidPath)?)?
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    migrate(&pool).await?;

    Ok(pool)
}

fn db_path(_app: &AppHandle) -> Result<PathBuf, DbError> {
    let base = tauri::api::path::local_data_dir().ok_or(DbError::DataDirMissing)?;
    Ok(base.join("obsidian").join("obsidian.db"))
}

async fn migrate(pool: &SqlitePool) -> Result<(), DbError> {
    sqlx::migrate!("src/db/migrations").run(pool).await?;
    Ok(())
}
