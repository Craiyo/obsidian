use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use thiserror::Error;

use crate::modules::marrow;

#[derive(Debug, Error)]
pub enum AlchemyError {
    #[error("missing material list")]
    MissingMaterials,
    #[error("missing market price")]
    MissingPrice,
    #[error("marrow error: {0}")]
    Marrow(#[from] marrow::MarrowError),
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
}

#[derive(Debug, Deserialize, Clone)]
pub struct MaterialInput {
    pub item_id: String,
    pub quantity: f64,
    pub unit_cost: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CalculateRequest {
    pub item_id: String,
    pub city: String,
    pub return_rate_pct: f64,
    pub crafting_fee_pct: f64,
    pub bonus_pct: f64,
    pub materials: Vec<MaterialInput>,
}

#[derive(Debug, Serialize)]
pub struct CalculateResponse {
    pub item_id: String,
    pub city: String,
    pub output_price: i64,
    pub material_cost: f64,
    pub effective_material_cost: f64,
    pub crafting_fee: f64,
    pub profit: f64,
}

#[derive(Debug, Deserialize)]
pub struct ScenarioRequest {
    pub name: String,
    pub calculation: CalculateRequest,
}

#[derive(Debug, Serialize)]
pub struct ScenarioRow {
    pub id: i64,
    pub name: String,
    pub item_id: String,
    pub city: String,
    pub return_rate_pct: f64,
    pub crafting_fee_pct: f64,
    pub bonus_pct: f64,
    pub profit: i64,
    pub created_at: i64,
}

pub async fn calculate(
    pool: &SqlitePool,
    client: &reqwest::Client,
    req: CalculateRequest,
) -> Result<CalculateResponse, AlchemyError> {
    if req.materials.is_empty() {
        return Err(AlchemyError::MissingMaterials);
    }

    let mut material_cost = 0.0;
    for (_, quantity, unit_cost) in resolve_material_costs(pool, client, &req.city, &req.materials).await? {
        material_cost += quantity * unit_cost as f64;
    }

    let output_price = marrow::get_price(
        pool,
        client,
        marrow::AlbionServer::Americas,
        &req.item_id,
        &req.city,
        1,
        300,
    )
        .await?
        .sell_price
        .ok_or(AlchemyError::MissingPrice)?;

    let effective_material_cost = material_cost * (1.0 - (req.return_rate_pct / 100.0)).max(0.0);
    let output_value = (output_price as f64) * (1.0 + req.bonus_pct / 100.0);
    let crafting_fee = output_value * (req.crafting_fee_pct / 100.0);
    let profit = output_value - effective_material_cost - crafting_fee;

    Ok(CalculateResponse {
        item_id: req.item_id,
        city: req.city,
        output_price,
        material_cost,
        effective_material_cost,
        crafting_fee,
        profit,
    })
}

pub async fn save_scenario(
    pool: &SqlitePool,
    client: &reqwest::Client,
    req: ScenarioRequest,
) -> Result<ScenarioRow, AlchemyError> {
    let calc = req.calculation;
    if calc.materials.is_empty() {
        return Err(AlchemyError::MissingMaterials);
    }

    let resolved_materials = resolve_material_costs(pool, client, &calc.city, &calc.materials).await?;
    let mut material_cost = 0.0;
    for (_, quantity, unit_cost) in &resolved_materials {
        material_cost += *quantity * *unit_cost as f64;
    }

    let output_price = marrow::get_price(
        pool,
        client,
        marrow::AlbionServer::Americas,
        &calc.item_id,
        &calc.city,
        1,
        300,
    )
    .await?
    .sell_price
    .ok_or(AlchemyError::MissingPrice)?;

    let effective_material_cost = material_cost * (1.0 - (calc.return_rate_pct / 100.0)).max(0.0);
    let output_value = (output_price as f64) * (1.0 + calc.bonus_pct / 100.0);
    let crafting_fee = output_value * (calc.crafting_fee_pct / 100.0);
    let profit = output_value - effective_material_cost - crafting_fee;

    let calculation = CalculateResponse {
        item_id: calc.item_id.clone(),
        city: calc.city.clone(),
        output_price,
        material_cost,
        effective_material_cost,
        crafting_fee,
        profit,
    };
    let now = chrono::Utc::now().timestamp();

    let result = sqlx::query(
        "INSERT INTO alchemy_scenarios (name, item_id, city, return_rate, crafting_fee, bonus_pct, profit, created_at)         VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&req.name)
    .bind(&calculation.item_id)
    .bind(&calculation.city)
    .bind(calc.return_rate_pct)
    .bind(calc.crafting_fee_pct)
    .bind(calc.bonus_pct)
    .bind(calculation.profit.round() as i64)
    .bind(now)
    .execute(pool)
    .await?;

    let scenario_id = result.last_insert_rowid();

    for (material_id, quantity, unit_cost) in resolved_materials {
        sqlx::query(
            "INSERT INTO alchemy_scenario_materials (scenario_id, material_id, quantity, unit_cost) VALUES (?, ?, ?, ?)"
        )
        .bind(scenario_id)
        .bind(material_id)
        .bind(quantity)
        .bind(unit_cost)
        .execute(pool)
        .await?;
    }

    Ok(ScenarioRow {
        id: scenario_id,
        name: req.name,
        item_id: calculation.item_id,
        city: calculation.city,
        return_rate_pct: calc.return_rate_pct,
        crafting_fee_pct: calc.crafting_fee_pct,
        bonus_pct: calc.bonus_pct,
        profit: calculation.profit.round() as i64,
        created_at: now,
    })
}

async fn resolve_material_costs(
    pool: &SqlitePool,
    client: &reqwest::Client,
    city: &str,
    materials: &[MaterialInput],
) -> Result<Vec<(String, f64, i64)>, AlchemyError> {
    let mut resolved = Vec::with_capacity(materials.len());
    for material in materials {
        let unit_cost = if let Some(cost) = material.unit_cost {
            cost
        } else {
            let price = marrow::get_price(
                pool,
                client,
                marrow::AlbionServer::Americas,
                &material.item_id,
                city,
                1,
                300,
            )
            .await?;
            price.sell_price.ok_or(AlchemyError::MissingPrice)?
        };
        resolved.push((material.item_id.clone(), material.quantity, unit_cost));
    }
    Ok(resolved)
}

pub async fn scenarios(pool: &SqlitePool) -> Result<Vec<ScenarioRow>, AlchemyError> {
    let rows = sqlx::query(
        "SELECT id, name, item_id, city, return_rate as return_rate_pct, crafting_fee as crafting_fee_pct, bonus_pct, profit, created_at \
         FROM alchemy_scenarios ORDER BY created_at DESC"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ScenarioRow {
            id: row.get("id"),
            name: row.get("name"),
            item_id: row.get("item_id"),
            city: row.get("city"),
            return_rate_pct: row.get("return_rate_pct"),
            crafting_fee_pct: row.get("crafting_fee_pct"),
            bonus_pct: row.get("bonus_pct"),
            profit: row.get("profit"),
            created_at: row.get("created_at"),
        })
        .collect())
}
