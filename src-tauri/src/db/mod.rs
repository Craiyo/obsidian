use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, SqlitePool};
use std::{path::PathBuf, str::FromStr};
use tauri::AppHandle;
use thiserror::Error;

pub mod item_map;

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

fn candidates_for(filename: &str) -> Option<std::path::PathBuf> {
    let candidates = [
        std::path::PathBuf::from(filename),
        std::path::PathBuf::from(format!("../{}", filename)),
    ];
    candidates.into_iter().find(|p| p.exists())
}

pub async fn seed_items_if_empty(pool: &SqlitePool, _app: &tauri::AppHandle) -> Result<(), DbError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items")
        .fetch_one(pool)
        .await?;

    if count > 0 {
        return Ok(());
    }

    let items_path = candidates_for("assets/items.json").ok_or_else(|| DbError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "assets/items.json not found",
    )))?;

    let display_names_path = candidates_for("assets/item_names.json").ok_or_else(|| DbError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "assets/item_names.json not found",
    )))?;

    let display_names = item_map::load_display_names(&display_names_path)
        .map_err(|e| DbError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

    let rows = item_map::parse_items_json(&items_path, &display_names)
        .map_err(|e| DbError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

    // Insert all parsed items into the DB (do not filter by item_names.json)
    item_map::insert_items(pool, rows).await?;

    eprintln!("[db] items seeded");
    Ok(())
}
