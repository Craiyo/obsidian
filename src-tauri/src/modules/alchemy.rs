use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use sqlx::Row;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AlchemyError {
    #[error("missing material list")]
    MissingMaterials,
    #[error("missing market price")]
    MissingPrice,
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
}

#[derive(Debug, Deserialize)]
pub struct PlanItem {
    pub uniquename: String,
    pub quantity_out: i64,
}

#[derive(Debug, Deserialize)]
pub struct PlanRequest {
    pub account_name: String,
    pub items: Vec<PlanItem>,
}

#[derive(Debug, Serialize)]
pub struct SessionItem {
    pub uniquename: String,
    pub display_name: String,
    pub quantity_out: i64,
    pub craft_amount: i64,
    pub runs_needed: i64,
    pub rrr: f64,
    pub best_city: Option<String>,
}


#[derive(Debug, Serialize)]
pub struct MaterialRow {
    pub uniquename: String,
    pub display_name: String,
    pub quantity_needed: i64,
    pub unit_price: Option<i64>,
    pub total_cost: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct PlanResponse {
    pub session_id: i64,
    pub items: Vec<SessionItem>,
    pub materials: Vec<MaterialRow>,
    pub account_name: String,
    pub city: String,
    pub rrr_pct: f64,
    pub use_focus: bool,
}

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub session_id: i64,
    pub account_name: String,
    pub created_at: i64,
    pub city: String,
    pub item_count: i64,
    pub rrr: f64,
    pub total_cost: Option<f64>,
    pub sent_to_marrow: bool,
}

#[derive(Debug, Deserialize)]
pub struct SetPriceRequest {
    pub uniquename: String,
    pub unit_price: i64,
}

// Lightweight DB-backed implementations for Alchemy sessions.
// These are still minimal: materials are not auto-derived from item recipes yet,
// but sessions/items/materials are persisted to the alchemy_sessions* tables.


pub async fn plan_session(pool: &SqlitePool, account: &crate::settings::AccountProfile, items: Vec<PlanItem>) -> Result<PlanResponse, AlchemyError> {
    // Store session-level RRR without assuming a blanket city bonus — per-item bonuses are resolved below
    let session_rrr = account.rrr(false);
    let now = chrono::Utc::now().timestamp();

    let mut tx = pool.begin().await?;

    // Determine best session city by counting recommended best_city across requested items
    let mut city_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for it in &items {
        if let Some(row) = sqlx::query("SELECT shopcategory FROM items WHERE uniquename = ?")
            .bind(&it.uniquename)
            .fetch_optional(&mut *tx)
            .await?
        {
            if let Some(sc) = row.try_get::<Option<String>, _>("shopcategory")? {
                if let Some(cat) = crate::settings::shopcategory_to_item_category(&sc) {
                    let city = crate::settings::bonus_city_for(&cat).to_string();
                    *city_counts.entry(city).or_insert(0) += 1;
                }
            }
        }
    }
    let session_city = city_counts.into_iter().max_by_key(|(_, c)| *c).map(|(k, _)| k).unwrap_or_else(|| String::new());
    println!("[alchemy] plan_session: computed city_counts={:?}", city_counts);
    println!("[alchemy] plan_session: chosen session_city='{}'", session_city);

    let res = sqlx::query("INSERT INTO alchemy_sessions (account_name, city, use_focus, rrr, created_at, sent_to_marrow) VALUES (?, ?, ?, ?, ?, 0)")
        .bind(&account.name)
        .bind(&session_city)
        .bind(if account.use_focus { 1 } else { 0 })
        .bind(session_rrr)
        .bind(now)
        .execute(&mut *tx)
        .await?;

    let session_id = res.last_insert_rowid();
    println!("[alchemy] plan_session: inserted session_id={} city='{}'", session_id, session_city);

    let mut session_items = Vec::new();
    // For each requested item, persist the item row and attempt to derive its recipe from the items table
    for it in items.into_iter() {
        // Default craft amount; may be overridden by database recipe
        let mut craft_amount = 1i64;
        let runs_needed = it.quantity_out;
        let display_name = it.uniquename.clone();

        // Try to derive materials and craft_amount from items table first (so we can insert accurate item rows)
        let mut derived_materials: Vec<(String, i64)> = Vec::new();
        let mut shopcategory_opt: Option<String> = None;
        let mut display_name_override: Option<String> = None;

        if let Some(row) = sqlx::query("SELECT display_name, craft_amount, craft_resources, shopcategory FROM items WHERE uniquename = ?")
            .bind(&it.uniquename)
            .fetch_optional(&mut *tx)
            .await?
        {
            if let Some(dn) = row.try_get::<Option<String>, _>("display_name")? {
                display_name_override = Some(dn);
            }
            craft_amount = row.get::<i64, _>("craft_amount");
            shopcategory_opt = row.try_get::<Option<String>, _>("shopcategory")?;
            if let Some(cr_json) = row.try_get::<Option<String>, _>("craft_resources")? {
                if let Ok(val) = serde_json::from_str::<Value>(&cr_json) {
                    if let Some(arr) = val.as_array() {
                        for entry in arr {
                            if let Some(un) = entry.get("uniquename").and_then(Value::as_str) {
                                let count = entry.get("count").and_then(Value::as_i64).unwrap_or(1);
                                derived_materials.push((un.to_string(), count));
                            }
                        }
                    }
                }
            }
        }

        let display_name = display_name_override.unwrap_or_else(|| it.uniquename.clone());

        // Insert session item with sourced craft_amount and runs_needed
        craft_amount = craft_amount.max(1);
        let runs_needed = ((it.quantity_out as f64 / craft_amount as f64).ceil() as i64).max(1);
        let best_city = if let Some(ref sc) = shopcategory_opt {
            if let Some(cat) = crate::settings::shopcategory_to_item_category(sc) {
                Some(crate::settings::bonus_city_for(&cat).to_string())
            } else {
                None
            }
        } else {
            None
        };

        sqlx::query("INSERT INTO alchemy_session_items (session_id, uniquename, display_name, quantity_out, craft_amount, runs_needed, best_city) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(session_id)
            .bind(&it.uniquename)
            .bind(&display_name)
            .bind(it.quantity_out)
            .bind(craft_amount)
            .bind(runs_needed)
            .bind(best_city.clone())
            .execute(&mut *tx)
            .await?;

        // Fallback to alchemy_recipes table if no derived materials
        if derived_materials.is_empty() {
            if let Some(row) = sqlx::query("SELECT recipe_json FROM alchemy_recipes WHERE item_id = ?")
                .bind(&it.uniquename)
                .fetch_optional(&mut *tx)
                .await?
            {
                if let Some(recipe_json) = row.try_get::<Option<String>, _>("recipe_json")? {
                    if let Ok(val) = serde_json::from_str::<Value>(&recipe_json) {
                        if let Some(a) = val.get("amount").and_then(Value::as_i64) {
                            craft_amount = a;
                        }
                        if let Some(arr) = val.get("materials").and_then(Value::as_array) {
                            for entry in arr {
                                if let Some(un) = entry.get("uniquename").and_then(Value::as_str) {
                                    let count = entry.get("count").and_then(Value::as_i64).unwrap_or(1);
                                    derived_materials.push((un.to_string(), count));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Determine whether this specific item benefits from the city bonus
        let has_city_bonus_for_item = if let Some(ref sc) = shopcategory_opt {
            if let Some(cat) = crate::settings::shopcategory_to_item_category(sc) {
                account.has_city_bonus_for(&cat)
            } else {
                false
            }
        } else {
            false
        };

        // Persist derived materials into alchemy_session_materials
        for (mat_id, mat_count) in derived_materials.into_iter() {
            // compute quantity_needed using account profile RRR logic for this item
            let qty_needed = account.materials_to_buy(it.quantity_out, craft_amount, mat_count, has_city_bonus_for_item);
            let display_name = mat_id.clone();
            // Upsert material rows so quantities for the same material aggregate across items.
            // Some deployments may not have the UNIQUE(session_id, uniquename) constraint (older DB),
            // which makes ON CONFLICT fail. Use a safe two-step update-then-insert pattern.
            let upd = sqlx::query("UPDATE alchemy_session_materials SET quantity_needed = quantity_needed + ? WHERE session_id = ? AND uniquename = ?")
                .bind(qty_needed)
                .bind(session_id)
                .bind(&mat_id)
                .execute(&mut *tx)
                .await?;
            if upd.rows_affected() == 0 {
                sqlx::query("INSERT INTO alchemy_session_materials (session_id, uniquename, display_name, quantity_needed, unit_price, total_cost) VALUES (?, ?, ?, ?, NULL, NULL)")
                    .bind(session_id)
                    .bind(&mat_id)
                    .bind(&display_name)
                    .bind(qty_needed)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        // Determine per-item RRR (considering whether this item benefits from city bonus)
        let has_city_bonus_for_item = if let Some(ref sc) = shopcategory_opt {
            if let Some(cat) = crate::settings::shopcategory_to_item_category(sc) {
                account.has_city_bonus_for(&cat)
            } else {
                false
            }
        } else {
            false
        };
        let item_rrr = account.rrr(has_city_bonus_for_item);

        session_items.push(SessionItem { 
            uniquename: it.uniquename.clone(), 
            display_name: display_name.clone(),
            quantity_out: it.quantity_out,
            craft_amount,
            runs_needed,
            rrr: item_rrr,
            best_city: best_city.clone(),
        });
    }

    tx.commit().await?;
    println!("[alchemy] plan_session: transaction committed for session_id={}", session_id);

    // Load materials to include in response
    let material_rows = sqlx::query("SELECT uniquename, display_name, quantity_needed, unit_price, total_cost FROM alchemy_session_materials WHERE session_id = ?")
        .bind(session_id)
        .fetch_all(pool)
        .await?;

    // Optional: load session items' best_city from DB when preparing response (not strictly necessary here
    // because we already built session_items while inserting, but include DB read path for consistency).
    let item_rows = sqlx::query("SELECT uniquename, display_name, quantity_out, craft_amount, runs_needed, best_city FROM alchemy_session_items WHERE session_id = ?")
        .bind(session_id)
        .fetch_all(pool)
        .await?;

    // If session_items vector is empty (shouldn't be), populate from DB rows
    if session_items.is_empty() {
        for r in item_rows {
            session_items.push(SessionItem {
                uniquename: r.get("uniquename"),
                display_name: r.get("display_name"),
                quantity_out: r.get("quantity_out"),
                craft_amount: r.get::<i64, _>("craft_amount"),
                runs_needed: r.get::<i64, _>("runs_needed"),
                rrr: account.rrr(false),
                best_city: r.get::<Option<String>, _>("best_city"),
            });
        }
    }

    let mut materials_out = Vec::new();
    for r in material_rows {
        materials_out.push(MaterialRow {
            uniquename: r.get("uniquename"),
            display_name: r.get("display_name"),
            quantity_needed: r.get("quantity_needed"),
            unit_price: r.get::<Option<i64>, _>("unit_price"),
            total_cost: r.get::<Option<f64>, _>("total_cost"),
        });
    }

    Ok(PlanResponse {
        session_id,
        items: session_items,
        materials: materials_out,
        account_name: account.name.clone(),
        city: String::new(),
        rrr_pct: session_rrr,
        use_focus: account.use_focus,
    })
}

pub async fn list_sessions(pool: &SqlitePool, limit: i64) -> Result<Vec<SessionSummary>, AlchemyError> {
    let rows = sqlx::query("SELECT s.id, s.account_name, s.created_at, s.city, s.rrr, s.sent_to_marrow, \n        (SELECT COUNT(*) FROM alchemy_session_items WHERE session_id = s.id) as item_count, \n        (SELECT SUM(total_cost) FROM alchemy_session_materials WHERE session_id = s.id) as total_cost \n        FROM alchemy_sessions s ORDER BY created_at DESC LIMIT ?")
        .bind(limit)
        .fetch_all(pool)
        .await?;

    let summaries = rows.into_iter().map(|r| SessionSummary {
        session_id: r.get("id"),
        account_name: r.get("account_name"),
        created_at: r.get("created_at"),
        city: r.try_get::<Option<String>, _>("city").unwrap_or_default().unwrap_or_default(),
        item_count: r.try_get::<i64, _>("item_count").unwrap_or(0),
        rrr: r.try_get::<f64, _>("rrr").unwrap_or(0.0),
        total_cost: r.try_get::<Option<f64>, _>("total_cost").unwrap_or(None),
        sent_to_marrow: r.try_get::<i64, _>("sent_to_marrow").unwrap_or(0) != 0,
    }).collect();

    Ok(summaries)
}

pub async fn load_session(pool: &SqlitePool, id: i64) -> Result<PlanResponse, AlchemyError> {
    let row = sqlx::query("SELECT account_name, city, use_focus, rrr FROM alchemy_sessions WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;

    let account_name: String = row.get("account_name");
    let city: String = row.get("city");
    let use_focus_i: i64 = row.get("use_focus");
    let use_focus = use_focus_i != 0;
    let rrr: f64 = row.get("rrr");

    let item_rows = sqlx::query("SELECT uniquename, display_name, quantity_out, craft_amount, runs_needed, best_city FROM alchemy_session_items WHERE session_id = ?")
        .bind(id)
        .fetch_all(pool)
        .await?;

    let mut items = Vec::new();
    for r in item_rows {
        let uniquename: String = r.get("uniquename");
        let display_name: String = r.get("display_name");
        let quantity_out: i64 = r.get("quantity_out");
        let craft_amount: i64 = r.get("craft_amount");
        let runs_needed: i64 = r.get("runs_needed");
        let best_city: Option<String> = r.get::<Option<String>, _>("best_city");
        // No per-item city-bonus context here — fall back to session-level RRR
        items.push(SessionItem { uniquename, display_name, quantity_out, craft_amount, runs_needed, rrr: rrr, best_city });
    }

    let material_rows = sqlx::query("SELECT uniquename, display_name, quantity_needed, unit_price, total_cost FROM alchemy_session_materials WHERE session_id = ?")
        .bind(id)
        .fetch_all(pool)
        .await?;

    let mut materials = Vec::new();
    for r in material_rows {
        materials.push(MaterialRow {
            uniquename: r.get("uniquename"),
            display_name: r.get("display_name"),
            quantity_needed: r.get("quantity_needed"),
            unit_price: r.get::<Option<i64>, _>("unit_price"),
            total_cost: r.get::<Option<f64>, _>("total_cost"),
        });
    }

    Ok(PlanResponse {
        session_id: id,
        items,
        materials,
        account_name,
        city,
        rrr_pct: rrr,
        use_focus,
    })
}

pub async fn set_material_price(pool: &SqlitePool, session_id: i64, uniquename: &str, unit_price: i64) -> Result<(), AlchemyError> {
    // Update unit_price and total_cost = unit_price * quantity_needed
    sqlx::query("UPDATE alchemy_session_materials SET unit_price = ?, total_cost = (quantity_needed * ?) WHERE session_id = ? AND uniquename = ?")
        .bind(unit_price)
        .bind(unit_price as f64)
        .bind(session_id)
        .bind(uniquename)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_sent_to_marrow(pool: &SqlitePool, session_id: i64) -> Result<(), AlchemyError> {
    sqlx::query("UPDATE alchemy_sessions SET sent_to_marrow = 1 WHERE id = ?")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_session(pool: &SqlitePool, session_id: i64) -> Result<(), AlchemyError> {
    let mut tx = pool.begin().await?;
    // Delete materials, items, then session
    sqlx::query("DELETE FROM alchemy_session_materials WHERE session_id = ?")
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM alchemy_session_items WHERE session_id = ?")
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM alchemy_sessions WHERE id = ?")
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
