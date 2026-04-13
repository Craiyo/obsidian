use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use thiserror::Error;

use crate::settings::AccountProfile;

#[derive(Debug, Error)]
pub enum AlchemyError {
    #[error("account not found")]
    AccountNotFound,
    #[error("item not found in database: {0}")]
    ItemNotFound(String),
    #[error("item is not craftable: {0}")]
    NotCraftable(String),
    #[error("no craft materials for item: {0}")]
    NoMaterials(String),
    #[error("session not found")]
    SessionNotFound,
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

// ─── Request/Response types ──────────────────────────────────────────────────

/// One item the user wants to craft.
#[derive(Debug, Deserialize, Clone)]
pub struct QueueItem {
    pub uniquename: String,
    pub quantity_out: i64,
}

/// Request to plan a full craft session.
#[derive(Debug, Deserialize)]
pub struct PlanRequest {
    /// Account name (must match a profile in settings)
    pub account_name: String,
    /// Items to craft
    pub items: Vec<QueueItem>,
}

/// One material in the aggregated shopping list.
#[derive(Debug, Serialize, Clone)]
pub struct ShoppingMaterial {
    pub uniquename: String,
    pub display_name: String,
    /// Total quantity to buy (after RRR applied)
    pub quantity_needed: i64,
    /// Manually entered price (None until user sets it)
    pub unit_price: Option<i64>,
    /// quantity_needed * unit_price (None until price is set)
    pub total_cost: Option<f64>,
}

/// One item in the planned session.
#[derive(Debug, Serialize, Clone)]
pub struct PlannedItem {
    pub uniquename: String,
    pub display_name: String,
    pub quantity_out: i64,
    pub craft_amount: i64,
    pub runs_needed: i64,
}

/// Full planned session returned to the UI.
#[derive(Debug, Serialize)]
pub struct PlanResponse {
    pub session_id: i64,
    pub account_name: String,
    pub city: String,
    pub use_focus: bool,
    pub rrr: f64,
    pub rrr_pct: f64,
    pub items: Vec<PlannedItem>,
    /// Aggregated shopping list — materials deduplicated and summed across all items
    pub materials: Vec<ShoppingMaterial>,
    pub created_at: i64,
}

/// Request to set a price for one material in a session.
#[derive(Debug, Deserialize)]
pub struct SetPriceRequest {
    pub uniquename: String,
    pub unit_price: i64,
}

/// Summary row for listing past sessions.
#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub id: i64,
    pub account_name: String,
    pub city: String,
    pub use_focus: bool,
    pub rrr: f64,
    pub item_count: i64,
    pub total_cost: Option<f64>,
    pub sent_to_marrow: bool,
    pub created_at: i64,
}

// ─── Internal helpers ────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct CraftResource {
    uniquename: String,
    count: i64,
}

struct ItemMeta {
    display_name: String,
    shopcategory: String,
    craft_amount: i64,
    craft_resources: Vec<CraftResource>,
}

async fn load_item_meta(pool: &SqlitePool, uniquename: &str) -> Result<ItemMeta, AlchemyError> {
    let row = sqlx::query(
        "SELECT display_name, shopcategory, craftable, craft_amount, craft_resources FROM items WHERE uniquename = ?1"
    )
    .bind(uniquename)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AlchemyError::ItemNotFound(uniquename.to_string()))?;

    let craftable: i64 = row.get::<Option<i64>, _>("craftable").unwrap_or(0);
    if craftable == 0 {
        return Err(AlchemyError::NotCraftable(uniquename.to_string()));
    }

    let craft_amount: i64 = row.get::<Option<i64>, _>("craft_amount").unwrap_or(1).max(1);
    let craft_resources_json: Option<String> = row.get("craft_resources");
    let craft_resources: Vec<CraftResource> = match craft_resources_json {
        Some(ref j) => serde_json::from_str(j).unwrap_or_default(),
        None => vec![],
    };

    if craft_resources.is_empty() {
        return Err(AlchemyError::NoMaterials(uniquename.to_string()));
    }

    let display_name: String = row
        .get::<Option<String>, _>("display_name")
        .unwrap_or_else(|| uniquename.to_string());

    let shopcategory: String = row
        .get::<Option<String>, _>("shopcategory")
        .unwrap_or_default();

    Ok(ItemMeta { display_name, shopcategory, craft_amount, craft_resources })
}

async fn load_display_name(pool: &SqlitePool, uniquename: &str) -> String {
    sqlx::query("SELECT display_name FROM items WHERE uniquename = ?1")
        .bind(uniquename)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.get::<Option<String>, _>("display_name"))
        .unwrap_or_else(|| uniquename.to_string())
}

// ─── Core functions ───────────────────────────────────────────────────────────

/// Plan a craft session: compute shopping list from desired output quantities.
pub async fn plan_session(
    pool: &SqlitePool,
    account: &AccountProfile,
    items: Vec<QueueItem>,
) -> Result<PlanResponse, AlchemyError> {
    if items.is_empty() {
        return Err(AlchemyError::NoMaterials("no items in queue".to_string()));
    }

    let mut planned_items: Vec<PlannedItem> = Vec::new();
    // We compute per-item RRR because items may have different bonus eligibility.
    let mut material_totals: std::collections::HashMap<String, i64> = std::collections::HashMap::new();


    for queue_item in &items {
        let meta = load_item_meta(pool, &queue_item.uniquename).await?;

        // Per-item city bonus: check if item's shopcategory maps to a category
        // the account crafts in their home city.
        let has_city_bonus = crate::settings::shopcategory_to_item_category(&meta.shopcategory)
            .map(|cat| account.has_city_bonus_for(&cat))
            .unwrap_or(false);

        let rrr = account.rrr(has_city_bonus);
        let runs_needed = (queue_item.quantity_out as f64 / meta.craft_amount as f64).ceil() as i64;

        for mat in &meta.craft_resources {
            let raw_needed = runs_needed * mat.count;
            let to_buy = (raw_needed as f64 * (1.0 - rrr)).ceil() as i64;
            *material_totals.entry(mat.uniquename.clone()).or_insert(0) += to_buy;
        }

        planned_items.push(PlannedItem {
            uniquename: queue_item.uniquename.clone(),
            display_name: meta.display_name,
            quantity_out: queue_item.quantity_out,
            craft_amount: meta.craft_amount,
            runs_needed,
        });
    }

    // Build shopping list with display names
    let mut materials: Vec<ShoppingMaterial> = Vec::new();
    for (uniquename, quantity_needed) in &material_totals {
        let display_name = load_display_name(pool, uniquename).await;
        materials.push(ShoppingMaterial {
            uniquename: uniquename.clone(),
            display_name,
            quantity_needed: *quantity_needed,
            unit_price: None,
            total_cost: None,
        });
    }
    materials.sort_by(|a, b| a.display_name.cmp(&b.display_name));

    let now = Utc::now().timestamp();

    // Representative RRR for the session row: account with-bonus rrr if they
    // have any crafting lines configured, otherwise base rrr (no bonus).
    let session_rrr = account.rrr(!account.crafting_lines.is_empty());

    // Persist session
    let mut tx = pool.begin().await?;

    let session_result = sqlx::query(
        "INSERT INTO alchemy_sessions (account_name, city, use_focus, rrr, created_at, sent_to_marrow)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)"
    )
    .bind(&account.name)
    .bind(&account.city)
    .bind(if account.use_focus { 1i64 } else { 0i64 })
    .bind(session_rrr)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    let session_id = session_result.last_insert_rowid();

    for item in &planned_items {
        sqlx::query(
            "INSERT INTO alchemy_session_items
             (session_id, uniquename, display_name, quantity_out, craft_amount, runs_needed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
        )
        .bind(session_id)
        .bind(&item.uniquename)
        .bind(&item.display_name)
        .bind(item.quantity_out)
        .bind(item.craft_amount)
        .bind(item.runs_needed)
        .execute(&mut *tx)
        .await?;
    }

    for mat in &materials {
        sqlx::query(
            "INSERT INTO alchemy_session_materials
             (session_id, uniquename, display_name, quantity_needed, unit_price, total_cost)
             VALUES (?1, ?2, ?3, ?4, NULL, NULL)"
        )
        .bind(session_id)
        .bind(&mat.uniquename)
        .bind(&mat.display_name)
        .bind(mat.quantity_needed)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(PlanResponse {
        session_id,
        account_name: account.name.clone(),
        city: account.city.clone(),
        use_focus: account.use_focus,
        rrr: session_rrr,
        rrr_pct: (session_rrr * 100.0 * 100.0).round() / 100.0,
        items: planned_items,
        materials,
        created_at: now,
    })
}


/// Update the unit price for one material in a session and recompute total_cost.
pub async fn set_material_price(
    pool: &SqlitePool,
    session_id: i64,
    uniquename: &str,
    unit_price: i64,
) -> Result<(), AlchemyError> {
    let exists: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM alchemy_sessions WHERE id = ?1"
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;

    if exists.is_none() {
        return Err(AlchemyError::SessionNotFound);
    }

    let qty: i64 = sqlx::query_scalar(
        "SELECT quantity_needed FROM alchemy_session_materials WHERE session_id = ?1 AND uniquename = ?2"
    )
    .bind(session_id)
    .bind(uniquename)
    .fetch_optional(pool)
    .await?
    .unwrap_or(0);

    let total_cost = qty as f64 * unit_price as f64;

    sqlx::query(
        "UPDATE alchemy_session_materials
         SET unit_price = ?1, total_cost = ?2
         WHERE session_id = ?3 AND uniquename = ?4"
    )
    .bind(unit_price)
    .bind(total_cost)
    .bind(session_id)
    .bind(uniquename)
    .execute(pool)
    .await?;

    Ok(())
}

/// Load a full session with all items and materials.
pub async fn load_session(pool: &SqlitePool, session_id: i64) -> Result<PlanResponse, AlchemyError> {
    let session = sqlx::query(
        "SELECT account_name, city, use_focus, rrr, created_at FROM alchemy_sessions WHERE id = ?1"
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AlchemyError::SessionNotFound)?;

    let items: Vec<PlannedItem> = sqlx::query(
        "SELECT uniquename, display_name, quantity_out, craft_amount, runs_needed
         FROM alchemy_session_items WHERE session_id = ?1"
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| PlannedItem {
        uniquename: r.get("uniquename"),
        display_name: r.get("display_name"),
        quantity_out: r.get("quantity_out"),
        craft_amount: r.get("craft_amount"),
        runs_needed: r.get("runs_needed"),
    })
    .collect();

    let materials: Vec<ShoppingMaterial> = sqlx::query(
        "SELECT uniquename, display_name, quantity_needed, unit_price, total_cost
         FROM alchemy_session_materials WHERE session_id = ?1 ORDER BY display_name"
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| ShoppingMaterial {
        uniquename: r.get("uniquename"),
        display_name: r.get("display_name"),
        quantity_needed: r.get("quantity_needed"),
        unit_price: r.get("unit_price"),
        total_cost: r.get("total_cost"),
    })
    .collect();

    let rrr: f64 = session.get("rrr");

    Ok(PlanResponse {
        session_id,
        account_name: session.get("account_name"),
        city: session.get("city"),
        use_focus: session.get::<i64, _>("use_focus") != 0,
        rrr,
        rrr_pct: (rrr * 100.0 * 100.0).round() / 100.0,
        items,
        materials,
        created_at: session.get("created_at"),
    })
}

/// List recent sessions (summary only).
pub async fn list_sessions(pool: &SqlitePool, limit: i64) -> Result<Vec<SessionSummary>, AlchemyError> {
    let rows = sqlx::query(
        r#"
        SELECT
            s.id, s.account_name, s.city, s.use_focus, s.rrr, s.created_at, s.sent_to_marrow,
            COUNT(DISTINCT si.id) AS item_count,
            SUM(sm.total_cost) AS total_cost
        FROM alchemy_sessions s
        LEFT JOIN alchemy_session_items si ON si.session_id = s.id
        LEFT JOIN alchemy_session_materials sm ON sm.session_id = s.id
        GROUP BY s.id
        ORDER BY s.created_at DESC
        LIMIT ?1
        "#
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| SessionSummary {
        id: r.get("id"),
        account_name: r.get("account_name"),
        city: r.get("city"),
        use_focus: r.get::<i64, _>("use_focus") != 0,
        rrr: r.get("rrr"),
        item_count: r.get("item_count"),
        total_cost: r.get("total_cost"),
        sent_to_marrow: r.get::<i64, _>("sent_to_marrow") != 0,
        created_at: r.get("created_at"),
    }).collect())
}

/// Mark a session as sent to Marrow.
pub async fn mark_sent_to_marrow(pool: &SqlitePool, session_id: i64) -> Result<(), AlchemyError> {
    sqlx::query("UPDATE alchemy_sessions SET sent_to_marrow = 1 WHERE id = ?1")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}
