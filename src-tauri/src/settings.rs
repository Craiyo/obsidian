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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AlbionServer {
    Americas,
    Asia,
    Europe,
}

impl AlbionServer {
    pub fn base_url(&self) -> &'static str {
        match self {
            AlbionServer::Americas => "https://west.albion-online-data.com",
            AlbionServer::Asia    => "https://east.albion-online-data.com",
            AlbionServer::Europe  => "https://europe.albion-online-data.com",
        }
    }
}

impl Default for AlbionServer {
    fn default() -> Self { AlbionServer::Asia }
}

/// The item categories a city gives a crafting bonus for.
/// Used to determine whether an account gets the +29% production bonus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ItemCategory {
    Sword, Bow, ArcaneStaff, LeatherHeadgear, LeatherShoes,   // Lymhurst
    Hammer, Spear, HolyStaff, ClothArmor, PlateHeadgear,      // Fort Sterling
    Mace, NatureStaff, FireStaff, LeatherArmor, ClothHeadgear, // Thetford
    Axe, Quarterstaff, FrostStaff, PlateShoes, Offhand,        // Martlock
    Crossbow, Dagger, CursedStaff, PlateArmor, ClothShoes,     // Bridgewatch
}

/// Returns the city that gives a bonus for a given item category.
pub fn bonus_city_for(category: &ItemCategory) -> &'static str {
    match category {
        ItemCategory::Sword | ItemCategory::Bow | ItemCategory::ArcaneStaff |
        ItemCategory::LeatherHeadgear | ItemCategory::LeatherShoes => "Lymhurst",

        ItemCategory::Hammer | ItemCategory::Spear | ItemCategory::HolyStaff |
        ItemCategory::ClothArmor | ItemCategory::PlateHeadgear => "Fort Sterling",

        ItemCategory::Mace | ItemCategory::NatureStaff | ItemCategory::FireStaff |
        ItemCategory::LeatherArmor | ItemCategory::ClothHeadgear => "Thetford",

        ItemCategory::Axe | ItemCategory::Quarterstaff | ItemCategory::FrostStaff |
        ItemCategory::PlateShoes | ItemCategory::Offhand => "Martlock",

        ItemCategory::Crossbow | ItemCategory::Dagger | ItemCategory::CursedStaff |
        ItemCategory::PlateArmor | ItemCategory::ClothShoes => "Bridgewatch",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountProfile {
    /// Display name e.g. "Warrior", "Hunter", "Mage"
    pub name: String,
    /// The city this account crafts in
    pub city: String,
    /// Item categories this account crafts
    pub crafting_lines: Vec<ItemCategory>,
    /// Whether this account uses focus when crafting
    pub use_focus: bool,
    /// Station crafting fee percentage (typically 1.5–3.0)
    pub crafting_fee_pct: f64,
}

impl AccountProfile {
    /// Whether this account gets the city bonus for the given item category.
    pub fn has_city_bonus_for(&self, category: &ItemCategory) -> bool {
        bonus_city_for(category) == self.city && self.crafting_lines.contains(category)
    }

    /// Production bonus as a percentage.
    /// Base: 18% always in royal city.
    /// City bonus item: +29% (total 47%).
    /// Focus: adds enough to reach ~77% no-bonus or ~106% with-bonus.
    pub fn production_bonus_pct(&self, has_city_bonus: bool) -> f64 {
        let base = 18.0;
        let city_bonus = if has_city_bonus { 29.0 } else { 0.0 };
        let focus_bonus = if self.use_focus { 59.0 } else { 0.0 };
        base + city_bonus + focus_bonus
    }

    /// RRR as a decimal (0.0 to 1.0).
    /// Formula: 1 - 1 / (1 + production_bonus / 100)
    pub fn rrr(&self, has_city_bonus: bool) -> f64 {
        let pb = self.production_bonus_pct(has_city_bonus);
        1.0 - 1.0 / (1.0 + pb / 100.0)
    }

    /// Materials to buy for a desired output quantity.
    /// to_buy = ceil(quantity_out / craft_amount) * materials_per_run * (1 - RRR)
    pub fn materials_to_buy(
        &self,
        quantity_out: i64,
        craft_amount: i64,
        materials_per_run: i64,
        has_city_bonus: bool,
    ) -> i64 {
        let runs = (quantity_out as f64 / craft_amount as f64).ceil() as i64;
        let raw_needed = runs * materials_per_run;
        let rrr = self.rrr(has_city_bonus);
        (raw_needed as f64 * (1.0 - rrr)).ceil() as i64
    }
}

impl Default for AccountProfile {
    fn default() -> Self {
        Self {
            name: "Account 1".to_string(),
            city: "Lymhurst".to_string(),
            crafting_lines: vec![ItemCategory::Sword],
            use_focus: false,
            crafting_fee_pct: 3.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub language: String,
    pub theme: String,
    pub albion_server: AlbionServer,
    pub seance_party_size: i64,
    pub seance_split_type: String,
    /// Up to 3 account profiles
    pub accounts: Vec<AccountProfile>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "English".to_string(),
            theme: "Undead Ritual".to_string(),
            albion_server: AlbionServer::default(),
            seance_party_size: 10,
            seance_split_type: "equal".to_string(),
            accounts: vec![
                AccountProfile {
                    name: "Warrior".to_string(),
                    city: "Martlock".to_string(),
                    crafting_lines: vec![ItemCategory::Axe],
                    use_focus: false,
                    crafting_fee_pct: 3.0,
                },
                AccountProfile {
                    name: "Hunter".to_string(),
                    city: "Lymhurst".to_string(),
                    crafting_lines: vec![ItemCategory::Bow],
                    use_focus: false,
                    crafting_fee_pct: 3.0,
                },
                AccountProfile {
                    name: "Mage".to_string(),
                    city: "Thetford".to_string(),
                    crafting_lines: vec![ItemCategory::FireStaff],
                    use_focus: false,
                    crafting_fee_pct: 3.0,
                },
            ],
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
