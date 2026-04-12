use serde::Serialize;
use sqlx::{Row, SqlitePool};
use crate::settings::AlbionServer;
use crate::modules::marrow::{self, HistoryPoint};
use futures::future::join_all;

#[derive(Debug, Serialize)]
pub struct CityPriceSummary {
    pub city: String,
    pub sell_price_min: Option<i64>,
    pub buy_price_max: Option<i64>,
}

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
    // Decision
    pub recommended: bool,
    pub reason: String,
    pub confidence: f64,

    // Profit (all silver, per-batch)
    pub batch_profit: f64,
    pub profit_per_unit: f64,
    pub profit_margin_pct: f64,
    pub material_cost: f64,
    pub effective_material_cost: f64,
    pub crafting_fee_silver: f64,
    pub sales_tax_silver: f64,
    pub output_price: i64,
    pub output_value: f64,
    pub craft_amount: i64,

    // Suggested quantity
    pub suggested_qty: i64,

    // Trend signals
    pub short_ema: Option<f64>,
    pub long_ema: Option<f64>,
    pub bullish: Option<bool>,
    pub price_volatility_pct: Option<f64>,
    pub avg_daily_volume: Option<f64>,
    pub min_daily_volume: Option<i64>,

    // City comparison
    pub city_prices: Vec<CityPriceSummary>,
    pub best_sell_city: Option<String>,

    // Materials breakdown
    pub materials: Vec<CraftMaterial>,

    // Crafting parameters used
    pub return_rate_pct: f64,
    pub crafting_fee_pct: f64,
}

impl Default for RecommendDecision {
    fn default() -> Self {
        Self {
            recommended: false,
            reason: String::new(),
            confidence: 0.0,
            batch_profit: 0.0,
            profit_per_unit: 0.0,
            profit_margin_pct: 0.0,
            material_cost: 0.0,
            effective_material_cost: 0.0,
            crafting_fee_silver: 0.0,
            sales_tax_silver: 0.0,
            output_price: 0,
            output_value: 0.0,
            craft_amount: 0,
            suggested_qty: 0,
            short_ema: None,
            long_ema: None,
            bullish: None,
            price_volatility_pct: None,
            avg_daily_volume: None,
            min_daily_volume: None,
            city_prices: Vec::new(),
            best_sell_city: None,
            materials: Vec::new(),
            return_rate_pct: 0.0,
            crafting_fee_pct: 0.0,
        }
    }
}

fn round2(v: f64) -> f64 { (v * 100.0).round() / 100.0 }

fn compute_ema(prices: &[f64], period: usize) -> Option<f64> {
    if prices.is_empty() || period == 0 || prices.len() < period {
        return None;
    }
    // Seed with SMA of first `period` elements
    let seed: f64 = prices[..period].iter().sum::<f64>() / period as f64;
    let alpha = 2.0 / (period as f64 + 1.0);
    let ema = prices[period..].iter().fold(seed, |e, p| alpha * p + (1.0 - alpha) * e);
    Some(ema)
}

fn compute_volatility_pct(prices: &[f64]) -> Option<f64> {
    if prices.len() < 3 { return None; }
    let mean = prices.iter().sum::<f64>() / prices.len() as f64;
    if mean == 0.0 { return None; }
    let variance = prices.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / prices.len() as f64;
    Some((variance.sqrt() / mean) * 100.0)
}

const ALL_CITIES: &[&str] = &[
    "Bridgewatch", "Caerleon", "Fort Sterling",
    "Lymhurst", "Martlock", "Thetford",
];

pub async fn recommend_item(
    pool: &SqlitePool,
    client: &reqwest::Client,
    server: AlbionServer,
    item_id: &str,
    city: &str,
    quality: i64,
    days: i64,
    return_rate_pct: f64,
    crafting_fee_pct: f64,
) -> Result<RecommendDecision, marrow::MarrowError> {
    // 1) Load craft data from items table
    let item_row = sqlx::query(
        "SELECT craftable, craft_resources, craft_amount FROM items WHERE uniquename = ?1"
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await?;

    let row = match item_row {
        Some(r) => r,
        None => return Ok(RecommendDecision { recommended: false, reason: "Item not found in database".to_string(), ..Default::default() }),
    };

    let craftable: bool = row.get::<Option<i64>, _>("craftable").unwrap_or(0) != 0;
    if !craftable {
        return Ok(RecommendDecision { recommended: false, reason: "Item is not craftable".to_string(), ..Default::default() });
    }

    let craft_amount: i64 = row.get::<Option<i64>, _>("craft_amount").unwrap_or(1).max(1);
    let craft_resources_json: Option<String> = row.get::<Option<String>, _>("craft_resources");

    #[derive(serde::Deserialize)]
    struct CraftResource { uniquename: String, count: i64 }

    let materials_raw: Vec<CraftResource> = match craft_resources_json {
        Some(ref j) => serde_json::from_str(j).unwrap_or_default(),
        None => vec![],
    };

    if materials_raw.is_empty() {
        return Ok(RecommendDecision { recommended: false, reason: "No craft materials found".to_string(), ..Default::default() });
    }

    // 2) Fetch material prices (sequential)
    let mut materials: Vec<CraftMaterial> = Vec::new();
    let mut material_cost: f64 = 0.0;

    for mat in &materials_raw {
        let price = match marrow::get_price(pool, client, server, &mat.uniquename, city, 1, 300).await {
            Ok(p) => p,
            Err(_) => return Ok(RecommendDecision { recommended: false, reason: format!("Failed to fetch price for material {}", mat.uniquename), ..Default::default() }),
        };
        let unit_price = match price.sell_price_min {
            Some(p) => p,
            None => return Ok(RecommendDecision { recommended: false, reason: format!("No market price for material {}", mat.uniquename), ..Default::default() }),
        };
        let qty = mat.count as f64;
        let total_cost = qty * unit_price as f64;
        material_cost += total_cost;
        let display_name = price.display_name.clone();
        materials.push(CraftMaterial { uniquename: mat.uniquename.clone(), display_name, quantity: qty, unit_price, total_cost: round2(total_cost) });
    }

    // 3) Fetch output item price for requested city
    let output = match marrow::get_price(pool, client, server, item_id, city, quality, 300).await {
        Ok(p) => p,
        Err(_) => return Ok(RecommendDecision { recommended: false, reason: "No market price for output item".to_string(), ..Default::default() }),
    };
    let output_price = match output.sell_price_min { Some(p) => p, None => return Ok(RecommendDecision { recommended: false, reason: "No market price for output item".to_string(), ..Default::default() }), };

    // 4) City comparison — fetch all cities concurrently
    // Note: these futures borrow pool & client; join_all is fine here because it polls them within the same task.
    // If refactoring to tokio::spawn, clone pool/client per future.
    let city_futures: Vec<_> = ALL_CITIES
        .iter()
        .map(|&c| marrow::get_price(pool, client, server, item_id, c, quality, 300))
        .collect();

    let city_results: Vec<Result<marrow::PriceResponse, marrow::MarrowError>> = join_all(city_futures).await;

    let mut city_prices: Vec<CityPriceSummary> = ALL_CITIES
        .iter()
        .map(|s| s.to_string())
        .zip(city_results.into_iter())
        .map(|(c, res)| match res {
            Ok(p) => CityPriceSummary { city: c, sell_price_min: p.sell_price_min, buy_price_max: p.buy_price_max },
            Err(_) => CityPriceSummary { city: c, sell_price_min: None, buy_price_max: None },
        })
        .collect();

    city_prices.sort_by(|a, b| b.sell_price_min.unwrap_or(0).cmp(&a.sell_price_min.unwrap_or(0)));
    let best_sell_city: Option<String> = city_prices.iter().find(|c| c.sell_price_min.is_some()).map(|c| c.city.clone());

    // 5) Fetch history via get_history (populate cache if cold). Ignore errors
    let history_points: Vec<HistoryPoint> = match marrow::get_history(pool, client, server, item_id, city, quality, days).await {
        Ok(h) => h.points,
        Err(_) => Vec::new(),
    };

    // 6) Compute signals
    let prices_vec: Vec<f64> = history_points.iter().map(|p| p.avg_price as f64).collect();
    let short_ema = compute_ema(&prices_vec, 3).map(round2);
    let long_ema = compute_ema(&prices_vec, 14).map(round2);
    let bullish = match (short_ema, long_ema) { (Some(s), Some(l)) => Some(s > l), _ => None };
    let price_volatility_pct = compute_volatility_pct(&prices_vec).map(round2);

    let avg_daily_volume = if history_points.is_empty() { None } else {
        let mean = history_points.iter().map(|p| p.item_count as f64).sum::<f64>() / history_points.len() as f64;
        Some(round2(mean))
    };

    // min_daily_volume: keep worst day for reference. Use .min() then .copied() via into_iter()
    let min_daily_volume = history_points.iter().map(|p| p.item_count).min().into_iter().copied().next();

    let suggested_qty = avg_daily_volume.map(|v| ((v * 0.2).round() as i64).max(1)).unwrap_or(1);

    // 7) Profit formula
    let multiplier = (1.0 - return_rate_pct / 100.0).max(0.0);
    let effective_material_cost = material_cost * multiplier;
    let output_value = output_price as f64 * craft_amount as f64;
    let crafting_fee_silver = output_value * (crafting_fee_pct / 100.0);
    let sales_tax_silver = output_value * 0.03;
    let batch_profit_raw = output_value - effective_material_cost - crafting_fee_silver - sales_tax_silver;
    let batch_profit = round2(batch_profit_raw);
    let profit_per_unit = if craft_amount != 0 { round2(batch_profit / craft_amount as f64) } else { 0.0 };
    let profit_margin_pct = if effective_material_cost > 0.0 { round2((batch_profit / effective_material_cost) * 100.0) } else { 0.0 };

    // 8) Confidence score — clamp profit contribution to >= 0
    let profit_comp = (profit_margin_pct.max(0.0).min(40.0)) / 40.0; // clamp negative margins to 0
    let bullish_comp = match bullish { Some(true) => 1.0, Some(false) => 0.0, None => 0.5 };
    let vol_comp = match price_volatility_pct { Some(v) => ((30.0 - v.min(30.0)) / 30.0).max(0.0), None => 0.5 };
    let volm_comp = match avg_daily_volume { Some(v) => (v.min(100.0) / 100.0).max(0.0), None => 0.5 };

    let w_profit = 0.35; let w_bull = 0.25; let w_volatility = 0.20; let w_volume = 0.20;
    let weighted_sum = w_profit * profit_comp + w_bull * bullish_comp + w_volatility * vol_comp + w_volume * volm_comp;
    let confidence = round2(weighted_sum.max(0.0).min(1.0));

    // 9) Build reason
    let mut reasons: Vec<&str> = Vec::new();
    if batch_profit <= 0.0 { reasons.push("Loss after fees"); }
    if bullish == Some(false) { reasons.push("Price trending down"); }
    if price_volatility_pct.map(|v| v > 30.0).unwrap_or(false) { reasons.push("High price volatility"); }
    if avg_daily_volume.map(|v| v < 5.0).unwrap_or(false) { reasons.push("Very low market volume"); }
    if batch_profit > 0.0 {
        if confidence >= 0.45 { reasons.push("Good craft opportunity"); } else { reasons.push("Marginal — weak signals"); }
    }
    let reason = if reasons.is_empty() {
        if batch_profit > 0.0 { "Profit positive but no strong signals".to_string() } else { "Insufficient data".to_string() }
    } else { reasons.join(". ").to_string() };

    let recommended = batch_profit > 0.0 && confidence >= 0.45;

    Ok(RecommendDecision {
        recommended,
        reason,
        confidence,
        batch_profit,
        profit_per_unit,
        profit_margin_pct,
        material_cost: round2(material_cost),
        effective_material_cost: round2(effective_material_cost),
        crafting_fee_silver: round2(crafting_fee_silver),
        sales_tax_silver: round2(sales_tax_silver),
        output_price,
        output_value: round2(output_value),
        craft_amount,
        suggested_qty,
        short_ema,
        long_ema,
        bullish,
        price_volatility_pct,
        avg_daily_volume,
        min_daily_volume,
        city_prices,
        best_sell_city,
        materials,
        return_rate_pct,
        crafting_fee_pct,
    })
}
