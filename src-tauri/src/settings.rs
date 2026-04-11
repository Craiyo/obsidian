use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::AppHandle;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("config dir unavailable")]
    ConfigDirMissing,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("toml encode error: {0}")]
    TomlEncode(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub language: String,
    pub theme: String,
    pub default_city: String,
    pub return_rate_pct: f64,
    pub crafting_fee_pct: f64,
    pub seance_party_size: i64,
    pub seance_split_type: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "English".to_string(),
            theme: "Undead Ritual".to_string(),
            default_city: "Bridgewatch".to_string(),
            return_rate_pct: 15.0,
            crafting_fee_pct: 6.0,
            seance_party_size: 10,
            seance_split_type: "equal".to_string(),
        }
    }
}

pub fn settings_path(_app: &AppHandle) -> Result<PathBuf, SettingsError> {
    let base = tauri::api::path::config_dir().ok_or(SettingsError::ConfigDirMissing)?;
    Ok(base.join("obsidian").join("settings.toml"))
}

pub async fn load(path: &PathBuf) -> Result<Settings, SettingsError> {
    if !path.exists() {
        return Ok(Settings::default());
    }
    let data = tokio::fs::read_to_string(path).await?;
    Ok(toml::from_str(&data)?)
}

pub async fn save(path: &PathBuf, settings: &Settings) -> Result<(), SettingsError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let data = toml::to_string_pretty(settings)?;
    tokio::fs::write(path, data).await?;
    Ok(())
}
