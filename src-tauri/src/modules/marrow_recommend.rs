use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use sqlx::{Row, SqlitePool};

use crate::settings::AlbionServer;
use crate::modules::marrow::{self, HistoryPoint};

#[derive(Debug, Serialize)]
pub struct RecommendItem {
    pub uniquename: String,
    pub display_name: String,
    pub tier: i64,
    pub short_ema: f64,
    pub long_ema: f64,
    pub min_daily_volume: i64,
    pub craft_qty: i64,
    pub unit_material_cost: f64,
    pub output_price: i64,
    pub unit_profit: f64,
    pub expected_sales_profit: f64,
    pub bullish: bool,
}

fn compute_ema(prices: &[f64], period: usize) -> Option<f64> {
    if prices.is_empty() || period == 0 {
        return None;
    }
    let alpha = 2.0 / (period as f64 + 1.0);
    let mut ema = prices[0];
    for p in prices.iter().skip(1) {
        ema = alpha * (*p) + (1.0 - alpha) * ema;
    }
    Some(ema)
}

#[derive(Debug, Serialize)]
pub struct RecommendDecision {
    pub recommended: bool,
    pub reason: Option<String>,
    pub item: Option<RecommendItem>,
}

pub async fn recommend(
    pool: &SqlitePool,
    client: &reqwest::Client,
    server: AlbionServer,
    city: &str,
    quality: i64,
    days: i64,
    limit: usize,
) -> Result<Vec<RecommendItem>, marrow::MarrowError> {
    // Load recipes
    let rows = sqlx::query("SELECT item_id, recipe_json FROM alchemy_recipes")
        .fetch_all(pool)
        .await?;

    let mut candidates: Vec<RecommendItem> = Vec::new();

    for row in rows {
        let item_id: String = row.get("item_id");
        let recipe_json: String = row.get("recipe_json");

        // Parse materials from recipe_json. Support two shapes: array of {item_id, quantity}
        // or object mapping item_id -> quantity
        let mut materials: Vec<(String, f64)> = Vec::new();
        if let Ok(v) = serde_json::from_str::<Value>(&recipe_json) {
            match &v {
                Value::Array(arr) => {
                    for entry in arr {
                        if let Some(obj) = entry.as_object() {
                            if let Some(id) = obj.get("item_id").and_then(|x| x.as_str()) {
                                let qty = obj.get("quantity").and_then(|x| x.as_f64()).unwrap_or(1.0);
                                materials.push((id.to_string(), qty));
                            }
                        }
                    }
                }
                Value::Object(map) => {
                    for (k, v) in map.iter() {
                        if let Some(q) = v.as_f64() {
                            materials.push((k.clone(), q));
                        }
                    }
                }
                _ => {}
            }
        }

        if materials.is_empty() {
            continue;
        }

        // Resolve material costs
        let mut unit_material_cost = 0.0f64;
        let mut skip = false;
        for (mat_id, qty) in &materials {
            match marrow::get_price(pool, client, server, mat_id, city, 1, 300).await {
                Ok(p) => {
                    if let Some(u) = p.sell_price_min {
                        unit_material_cost += (*qty) * (u as f64);
                    } else {
                        skip = true;
                        break;
                    }
                }
                Err(_) => {
                    skip = true;
                    break;
                }
            }
        }
        if skip {
            continue;
        }

        // Output price
        let output_price = match marrow::get_price(pool, client, server, &item_id, city, quality, 300).await {
            Ok(p) => match p.sell_price_min { Some(v) => v, None => continue },
            Err(_) => continue,
        };

        // History (days)
        let history = marrow::get_history(pool, client, server, &item_id, city, quality, days).await;

        let mut points: Vec<HistoryPoint> = Vec::new();
        let mut have_history = false;
        if let Ok(h) = history {
            points = h.points;
            if points.len() >= 2 {
                have_history = true;
            }
        }

        if !have_history {
            // skip only if we require history for trend — for now allow fallback later
        }

        let prices: Vec<f64> = points.iter().map(|p| p.avg_price as f64).collect();
        let short_ema = compute_ema(&prices, 3).unwrap_or(0.0);
        let long_ema = compute_ema(&prices, 14).unwrap_or(0.0);

        let min_daily_volume = if have_history { points.iter().map(|p| p.item_count).min().unwrap_or(0) } else { 1 };
        let craft_qty = ((min_daily_volume as f64) * 0.2).round() as i64;
        let craft_qty = if craft_qty < 1 { 1 } else { craft_qty };

        let unit_profit = (output_price as f64) - unit_material_cost;
        let expected_sales_profit = unit_profit * (craft_qty as f64);
        let bullish = short_ema > long_ema;

        // Default decision: recommend if unit_profit > 0 and (bullish or no history)
        if !(unit_profit > 0.0 && (bullish || !have_history)) {
            continue;
        }

        // Load display_name and tier from items table
        let (display_name, tier) = if let Some(r) = sqlx::query("SELECT display_name, tier FROM items WHERE uniquename = ?1")
            .bind(&item_id)
            .fetch_optional(pool)
            .await? {
                (
                    r.get::<Option<String>, _>("display_name").unwrap_or_else(|| item_id.clone()),
                    r.get::<Option<i64>, _>("tier").unwrap_or(0),
                )
        } else {
            (item_id.clone(), 0)
        };

        candidates.push(RecommendItem {
            uniquename: item_id.clone(),
            display_name,
            tier,
            short_ema,
            long_ema,
            min_daily_volume,
            craft_qty,
            unit_material_cost,
            output_price,
            unit_profit,
            expected_sales_profit,
            bullish,
        });
    }

    // Rank by expected_sales_profit
    candidates.sort_by(|a, b| b.expected_sales_profit.partial_cmp(&a.expected_sales_profit).unwrap_or(std::cmp::Ordering::Equal));
    candidates.truncate(limit);
    Ok(candidates)
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
    // Try to load recipe for this item
    let row = sqlx::query("SELECT recipe_json FROM alchemy_recipes WHERE item_id = ?1")
        .bind(item_id)
        .fetch_optional(pool)
        .await?;

    if row.is_none() {
        return Ok(RecommendDecision {
            recommended: false,
            reason: Some("no recipe found".to_string()),
            item: None,
        });
    }

    let recipe_json: String = row.unwrap().get("recipe_json");

    let mut materials: Vec<(String, f64)> = Vec::new();
    if let Ok(v) = serde_json::from_str::<Value>(&recipe_json) {
        match &v {
            Value::Array(arr) => {
                for entry in arr {
                    if let Some(obj) = entry.as_object() {
                        if let Some(id) = obj.get("item_id").and_then(|x| x.as_str()) {
                            let qty = obj.get("quantity").and_then(|x| x.as_f64()).unwrap_or(1.0);
                            materials.push((id.to_string(), qty));
                        }
                    }
                }
            }
            Value::Object(map) => {
                for (k, v) in map.iter() {
                    if let Some(q) = v.as_f64() {
                        materials.push((k.clone(), q));
                    }
                }
            }
            _ => {}
        }
    }

    if materials.is_empty() {
        return Ok(RecommendDecision {
            recommended: false,
            reason: Some("no materials in recipe".to_string()),
            item: None,
        });
    }

    // Resolve material costs
    let mut unit_material_cost = 0.0f64;
    for (mat_id, qty) in &materials {
        match marrow::get_price(pool, client, server, mat_id, city, 1, 300).await {
            Ok(p) => {
                if let Some(u) = p.sell_price_min {
                    unit_material_cost += (*qty) * (u as f64);
                } else {
                    return Ok(RecommendDecision {
                        recommended: false,
                        reason: Some(format!("missing price for material {}", mat_id)),
                        item: None,
                    });
                }
            }
            Err(_) => {
                return Ok(RecommendDecision {
                    recommended: false,
                    reason: Some(format!("failed to fetch price for material {}", mat_id)),
                    item: None,
                });
            }
        }
    }

    // Output price
    let output_price = match marrow::get_price(pool, client, server, item_id, city, quality, 300).await {
        Ok(p) => match p.sell_price_min { Some(v) => v, None => {
            return Ok(RecommendDecision { recommended: false, reason: Some("missing output price".to_string()), item: None });
        } },
        Err(_) => return Ok(RecommendDecision { recommended: false, reason: Some("failed to fetch output price".to_string()), item: None }),
    };

    // History
    let history = marrow::get_history(pool, client, server, item_id, city, quality, days).await;
    let mut have_history = false;
    let mut points: Vec<HistoryPoint> = Vec::new();
    if let Ok(h) = history {
        points = h.points;
        if points.len() >= 2 { have_history = true; }
    }

    let prices: Vec<f64> = points.iter().map(|p| p.avg_price as f64).collect();
    let short_ema = compute_ema(&prices, 3).unwrap_or(0.0);
    let long_ema = compute_ema(&prices, 14).unwrap_or(0.0);
    let min_daily_volume = if have_history { points.iter().map(|p| p.item_count).min().unwrap_or(0) } else { 1 };
    let craft_qty = ((min_daily_volume as f64) * 0.2).round() as i64; let craft_qty = if craft_qty < 1 { 1 } else { craft_qty };
    let unit_profit = (output_price as f64) - unit_material_cost;
    let bullish = short_ema > long_ema;

    let recommended = unit_profit > 0.0 && (bullish || !have_history);
    let reason = if !have_history { Some("history missing; using price-only heuristic".to_string()) } else if !bullish { Some("not trending up".to_string()) } else { None };

    // Load display_name and tier
    let (display_name, tier) = if let Some(r) = sqlx::query("SELECT display_name, tier FROM items WHERE uniquename = ?1").bind(item_id).fetch_optional(pool).await? {
        (
            r.get::<Option<String>, _>("display_name").unwrap_or_else(|| item_id.to_string()),
            r.get::<Option<i64>, _>("tier").unwrap_or(0),
        )
    } else { (item_id.to_string(), 0) };

    let rec_item = RecommendItem {
        uniquename: item_id.to_string(),
        display_name,
        tier,
        short_ema,
        long_ema,
        min_daily_volume,
        craft_qty,
        unit_material_cost,
        output_price,
        unit_profit,
        expected_sales_profit: unit_profit * (craft_qty as f64),
        bullish,
    };

    Ok(RecommendDecision { recommended, reason, item: Some(rec_item) })
}
