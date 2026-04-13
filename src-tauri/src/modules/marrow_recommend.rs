use serde::Serialize;
use sqlx::SqlitePool;
use crate::settings::{AccountProfile, AlbionServer, ItemCategory, bonus_city_for};

#[derive(Debug, Serialize)]
pub struct RecommendDecision {
    pub item_id: String,
    pub city: String,
    pub quality: i64,
    pub recommended: bool,
    pub reason: String,

    // Pricing
    pub sell_price: i64,
    pub avg_sell_price_7d: f64,

    // Costs
    pub effective_material_cost: f64,
    pub crafting_fee_silver: f64,
    pub sales_tax_silver: f64,
    pub listing_fee_silver: f64,

    // Results
    pub batch_profit_raw: f64,
    pub profit_margin_pct: f64,

    // Profile context
    pub account_name: String,
    pub rrr_used: f64,
    pub has_city_bonus: bool,
}

impl Default for RecommendDecision {
    fn default() -> Self {
        Self {
            item_id: String::new(),
            city: String::new(),
            quality: 1,
            recommended: false,
            reason: "Insufficient data".to_string(),
            sell_price: 0,
            avg_sell_price_7d: 0.0,
            effective_material_cost: 0.0,
            crafting_fee_silver: 0.0,
            sales_tax_silver: 0.0,
            listing_fee_silver: 0.0,
            batch_profit_raw: 0.0,
            profit_margin_pct: 0.0,
            account_name: String::new(),
            rrr_used: 0.0,
            has_city_bonus: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MarrowRecommendError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("item not found: {0}")]
    ItemNotFound(String),
    #[error("no price data available")]
    NoPriceData,
}

/// Determine if the given item's shopcategory maps to a known ItemCategory
/// that the account has a city bonus for.
fn resolve_has_city_bonus(shopcategory: &str, account: &AccountProfile) -> bool {
    // Map shopcategory string from DB to our ItemCategory enum
    let category = match shopcategory.to_lowercase().as_str() {
        "sword"             => Some(ItemCategory::Sword),
        "bow"               => Some(ItemCategory::Bow),
        "arcanestaff"       => Some(ItemCategory::ArcaneStaff),
        "leatherheadgear"   => Some(ItemCategory::LeatherHeadgear),
        "leathershoes"      => Some(ItemCategory::LeatherShoes),
        "hammer"            => Some(ItemCategory::Hammer),
        "spear"             => Some(ItemCategory::Spear),
        "holystaff"         => Some(ItemCategory::HolyStaff),
        "clotharmor"        => Some(ItemCategory::ClothArmor),
        "plateheadgear"     => Some(ItemCategory::PlateHeadgear),
        "mace"              => Some(ItemCategory::Mace),
        "naturestaff"       => Some(ItemCategory::NatureStaff),
        "firestaff"         => Some(ItemCategory::FireStaff),
        "leatherarmor"      => Some(ItemCategory::LeatherArmor),
        "clothheadgear"     => Some(ItemCategory::ClothHeadgear),
        "axe"               => Some(ItemCategory::Axe),
        "quarterstaff"      => Some(ItemCategory::Quarterstaff),
        "froststaff"        => Some(ItemCategory::FrostStaff),
        "plateshoes"        => Some(ItemCategory::PlateShoes),
        "offhand"           => Some(ItemCategory::Offhand),
        "crossbow"          => Some(ItemCategory::Crossbow),
        "dagger"            => Some(ItemCategory::Dagger),
        "cursedstaff"       => Some(ItemCategory::CursedStaff),
        "platearmor"        => Some(ItemCategory::PlateArmor),
        "clothshoes"        => Some(ItemCategory::ClothShoes),
        _ => None,
    };

    match &category {
        Some(cat) => {
            // City bonus requires: the item's bonus city == account's city AND account crafts that category
            bonus_city_for(cat).eq_ignore_ascii_case(&account.city)
                && account.crafting_lines.contains(cat)
        }
        // Unknown category — fall back to simple city match heuristic
        None => account.city.eq_ignore_ascii_case(
            // Map the request city heuristically
            &account.city
        ) && !account.crafting_lines.is_empty(),
    }
}

pub async fn recommend_item(
    pool: &SqlitePool,
    client: &reqwest::Client,
    server: AlbionServer,
    item_id: &str,
    city: &str,
    quality: i64,
    days: i64,
    account: &AccountProfile,
) -> Result<RecommendDecision, MarrowRecommendError> {
    // 1. Look up item in DB to get shopcategory
    let row = sqlx::query("SELECT shopcategory, craft_resources FROM items WHERE uniquename = ?")
        .bind(item_id)
        .fetch_optional(pool)
        .await?;

    let (shopcategory, craft_resources_json) = match row {
        Some(r) => {
            use sqlx::Row;
            (
                r.get::<Option<String>, _>("shopcategory").unwrap_or_default(),
                r.get::<Option<String>, _>("craft_resources").unwrap_or_default(),
            )
        }
        None => return Err(MarrowRecommendError::ItemNotFound(item_id.to_string())),
    };

    // 2. Determine city bonus using shopcategory
    let has_city_bonus = resolve_has_city_bonus(&shopcategory, account);
    let rrr = account.rrr(has_city_bonus);

    // 3. Fetch current sell price + history from Albion Online Data API
    let base = server.base_url();
    let locations = format!("{city},Black Market");
    let price_url = format!(
        "{base}/api/v2/stats/prices/{item_id}?locations={locations}&qualities={quality}"
    );
    let history_url = format!(
        "{base}/api/v2/stats/history/{item_id}?locations={city}&qualities={quality}&time-scale={days}"
    );

    let price_resp: Vec<serde_json::Value> = client
        .get(&price_url)
        .send()
        .await?
        .json()
        .await
        .unwrap_or_default();

    let history_resp: Vec<serde_json::Value> = client
        .get(&history_url)
        .send()
        .await?
        .json()
        .await
        .unwrap_or_default();

    // Extract best sell price for the requested city
    let sell_price = price_resp.iter()
        .filter(|p| p["city"].as_str().map(|c| c.eq_ignore_ascii_case(city)).unwrap_or(false))
        .filter_map(|p| p["sell_price_min"].as_i64())
        .filter(|&p| p > 0)
        .min()
        .unwrap_or(0);

    if sell_price == 0 {
        return Ok(RecommendDecision {
            item_id: item_id.to_string(),
            city: city.to_string(),
            quality,
            recommended: false,
            reason: "No sell orders found for this item in this city.".to_string(),
            account_name: account.name.clone(),
            rrr_used: rrr,
            has_city_bonus,
            ..Default::default()
        });
    }

    // Extract average sell price from history
    let avg_sell_price_7d = {
        let prices: Vec<f64> = history_resp.iter()
            .filter(|h| h["location"].as_str().map(|c| c.eq_ignore_ascii_case(city)).unwrap_or(false))
            .flat_map(|h| h["data"].as_array().cloned().unwrap_or_default())
            .filter_map(|d| d["avg_price"].as_f64())
            .filter(|&p| p > 0.0)
            .collect();
        if prices.is_empty() {
            sell_price as f64
        } else {
            prices.iter().sum::<f64>() / prices.len() as f64
        }
    };

    // 4. Estimate material cost from craft_resources JSON
    // craft_resources is a JSON array like [{"@uniquename": "...", "@count": "8"}, ...]
    let effective_material_cost = if craft_resources_json.is_empty() {
        // No recipe data — estimate from sell price
        sell_price as f64 * 0.4 * (1.0 - rrr)
    } else {
        let resources: Vec<serde_json::Value> = serde_json::from_str(&craft_resources_json)
            .unwrap_or_default();
        // Without individual material prices, use a rough estimate:
        // craft cost ≈ 30% of sell price per material unit, adjusted by RRR
        let total_count: f64 = resources.iter()
            .filter_map(|r| r["@count"].as_str().or(r["@count"].as_str()))
            .filter_map(|c| c.parse::<f64>().ok())
            .sum();
        let raw_mat_count = if total_count > 0.0 { total_count } else { 8.0 };
        // Effective materials consumed = raw_needed * (1 - RRR)
        let effective_materials = raw_mat_count * (1.0 - rrr);
        // Use average market estimate for a single mat: sell_price * 0.35 / raw_mat_count
        let unit_mat_price = (sell_price as f64 * 0.35) / raw_mat_count;
        effective_materials * unit_mat_price
    };

    // 5. Calculate all fees
    let output_value = sell_price as f64;
    let crafting_fee_silver = output_value * (account.crafting_fee_pct / 100.0);
    let sales_tax_silver = output_value * 0.03;   // 3% Albion sales tax
    let listing_fee_silver = output_value * 0.025; // 2.5% non-refundable listing fee

    let batch_profit_raw = output_value
        - effective_material_cost
        - crafting_fee_silver
        - sales_tax_silver
        - listing_fee_silver;

    let profit_margin_pct = if output_value > 0.0 {
        (batch_profit_raw / output_value) * 100.0
    } else {
        0.0
    };

    let recommended = profit_margin_pct > 5.0 && batch_profit_raw > 1000.0;
    let reason = if recommended {
        format!(
            "Profitable: {:.1}% margin after {:.1}% RRR, {:.1}% fee, 3% tax, 2.5% listing.",
            profit_margin_pct,
            rrr * 100.0,
            account.crafting_fee_pct
        )
    } else {
        format!(
            "Not recommended: {:.1}% margin is too thin (need >5%). Check mat costs or switch city.",
            profit_margin_pct
        )
    };

    Ok(RecommendDecision {
        item_id: item_id.to_string(),
        city: city.to_string(),
        quality,
        recommended,
        reason,
        sell_price,
        avg_sell_price_7d,
        effective_material_cost,
        crafting_fee_silver,
        sales_tax_silver,
        listing_fee_silver,
        batch_profit_raw,
        profit_margin_pct,
        account_name: account.name.clone(),
        rrr_used: rrr,
        has_city_bonus,
    })
}
