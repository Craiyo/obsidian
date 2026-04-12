use serde::Serialize;
use sqlx::{Row, SqlitePool};
use crate::settings::AlbionServer;
use crate::modules::marrow::{self, HistoryPoint};

#[derive(Debug, Serialize)]
pub struct CraftMaterial {
    pub uniquename: String,
    pub display_name: String,
    pub quantity: f64,
    pub unit_price: i64,
    pub total_cost: f64,
}

#[derive(Debug, Serialize)]
pub struct RecommendDecision {
    pub recommended: bool,
    pub reason: String,
    pub unit_profit: f64,
    pub profit_margin_pct: f64,
    pub suggested_qty: i64,
    pub material_cost: f64,
    pub output_price: i64,
    pub craft_amount: i64,
    pub short_ema: Option<f64>,
    pub long_ema: Option<f64>,
    pub bullish: Option<bool>,
    pub min_daily_volume: Option<i64>,
    pub materials: Vec<CraftMaterial>,
}

impl Default for RecommendDecision {
    fn default() -> Self {
        Self {
            recommended: false,
            reason: String::new(),
            unit_profit: 0.0,
            profit_margin_pct: 0.0,
            suggested_qty: 0,
            material_cost: 0.0,
            output_price: 0,
            craft_amount: 0,
            short_ema: None,
            long_ema: None,
            bullish: None,
            min_daily_volume: None,
            materials: Vec::new(),
        }
    }
}

fn compute_ema(prices: &[f64], period: usize) -> Option<f64> {
    if prices.len() < period || period == 0 {
        return None;
    }
    let alpha = 2.0 / (period as f64 + 1.0);
    let mut ema = prices[0];
    for p in prices.iter().skip(1) {
        ema = alpha * p + (1.0 - alpha) * ema;
    }
    Some(ema)
}

pub async fn recommend_item(
    pool: &SqlitePool,
    client: &reqwest::Client,
    server: AlbionServer,
    item_id: &str,
    city: &str,
    quality: i64,
    days: i64,
) -> Result<RecommendDecision, marrow::MarrowError> {
    // Step 1: load craft data from items table
    let row = sqlx::query(
        "SELECT craftable, craft_resources, craft_amount FROM items WHERE uniquename = ?1"
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await?;

    let item_row = match row {
        Some(r) => r,
        None => return Ok(RecommendDecision { recommended: false, reason: "Item not found in database".to_string(), ..Default::default() }),
    };

    let craftable = item_row.get::<Option<i64>, _>("craftable").unwrap_or(0) != 0;
    if !craftable {
        return Ok(RecommendDecision { recommended: false, reason: "Item is not craftable".to_string(), ..Default::default() });
    }

    let craft_amount = item_row.get::<Option<i64>, _>("craft_amount").unwrap_or(1).max(1);
    let craft_resources_json: Option<String> = item_row.get::<Option<String>, _>("craft_resources");

    // Step 2: parse craft_resources
    #[derive(serde::Deserialize)]
    struct CraftResource { uniquename: String, count: i64 }

    let materials_raw: Vec<CraftResource> = match craft_resources_json {
        Some(ref json) => serde_json::from_str(json).unwrap_or_default(),
        None => vec![],
    };

    if materials_raw.is_empty() {
        return Ok(RecommendDecision { recommended: false, reason: "No craft materials found".to_string(), ..Default::default() });
    }

    // Step 3: fetch material prices
    let mut materials: Vec<CraftMaterial> = Vec::new();
    let mut total_material_cost = 0.0f64;

    for mat in &materials_raw {
        let price = marrow::get_price(pool, client, server, &mat.uniquename, city, 1, 300).await?;
        let unit_price = match price.sell_price_min {
            Some(p) => p,
            None => return Ok(RecommendDecision { recommended: false, reason: format!("No market price for material {}", mat.uniquename), ..Default::default() }),
        };
        let qty = mat.count as f64;
        let total_cost = qty * unit_price as f64;
        total_material_cost += total_cost;
        let display_name = price.display_name.clone();
        materials.push(CraftMaterial { uniquename: mat.uniquename.clone(), display_name, quantity: qty, unit_price, total_cost });
    }

    // Step 4: fetch output price
    let output = marrow::get_price(pool, client, server, item_id, city, quality, 300).await?;
    let output_price = match output.sell_price_min {
        Some(p) => p,
        None => return Ok(RecommendDecision { recommended: false, reason: "No market price for output item".to_string(), ..Default::default() }),
    };

    // Step 5: load history from cache
    let time_scale = if days <= 1 { 1 } else if days <= 3 { 6 } else { 24 };
    let cached = sqlx::query(
        "SELECT data_json FROM marrow_history WHERE uniquename = ?1 AND city = ?2 AND quality = ?3 AND time_scale = ?4"
    )
    .bind(item_id).bind(city).bind(quality).bind(time_scale)
    .fetch_optional(pool)
    .await?;

    let points: Vec<HistoryPoint> = cached
        .and_then(|r| r.get::<Option<String>, _>("data_json"))
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();

    // Step 6: compute EMA and volume
    let prices: Vec<f64> = points.iter().map(|p| p.avg_price as f64).collect();
    let short_ema = compute_ema(&prices, 3);
    let long_ema = compute_ema(&prices, 14);
    let bullish = match (short_ema, long_ema) {
        (Some(s), Some(l)) => Some(s > l),
        _ => None,
    };
    let min_daily_volume = if points.is_empty() { None } else { Some(points.iter().map(|p| p.item_count).min().unwrap_or(0)) };
    let suggested_qty = min_daily_volume.map(|v| ((v as f64 * 0.2).round() as i64).max(1)).unwrap_or(1);

    // Step 7: calculate profit and build decision
    // material_cost is per craft_amount output units
    let unit_profit = (output_price as f64 * craft_amount as f64) - total_material_cost;
    let profit_margin_pct = if total_material_cost > 0.0 { (unit_profit / total_material_cost) * 100.0 } else { 0.0 };
    let recommended = unit_profit > 0.0 && bullish.unwrap_or(true);
    let reason = if unit_profit <= 0.0 {
        format!("Material cost exceeds sell price by {} silver", (-unit_profit).round() as i64)
    } else if bullish == Some(false) {
        "Price is trending down — not a good time to craft".to_string()
    } else if bullish.is_none() {
        "Not enough history for trend analysis — profit looks positive".to_string()
    } else {
        "Price is trending up and profit is positive".to_string()
    };

    Ok(RecommendDecision {
        recommended,
        reason,
        unit_profit,
        profit_margin_pct,
        suggested_qty,
        material_cost: total_material_cost,
        output_price,
        craft_amount,
        short_ema,
        long_ema,
        bullish,
        min_daily_volume,
        materials,
    })
}
