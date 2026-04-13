use serde::{Deserialize, Serialize};
use crate::modules::alchemy::AlchemyError;
use sqlx::SqlitePool;

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
    pub craft_amount: i64,
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
}

#[derive(Debug, Deserialize)]
pub struct SetPriceRequest {
    pub uniquename: String,
    pub unit_price: i64,
}

// Lightweight stubs so API layer compiles. These should be replaced by full implementations later.

pub async fn plan_session(_pool: &SqlitePool, account: &crate::settings::AccountProfile, items: Vec<PlanItem>) -> Result<PlanResponse, AlchemyError> {
    // Create a minimal PlanResponse using inputs; do not persist.
    let session_id = 0;
    let session_items = items.into_iter().map(|i| SessionItem { uniquename: i.uniquename, craft_amount: i.quantity_out }).collect();
    Ok(PlanResponse {
        session_id,
        items: session_items,
        materials: vec![],
        account_name: account.name.clone(),
        city: account.city.clone(),
        rrr_pct: 0.0,
        use_focus: account.use_focus,
    })
}

pub async fn list_sessions(_pool: &SqlitePool, _limit: i64) -> Result<Vec<SessionSummary>, AlchemyError> {
    Ok(vec![])
}

pub async fn load_session(_pool: &SqlitePool, id: i64) -> Result<PlanResponse, AlchemyError> {
    Ok(PlanResponse { session_id: id, items: vec![], materials: vec![], account_name: String::new(), city: String::new(), rrr_pct: 0.0, use_focus:false })
}

pub async fn set_material_price(_pool: &SqlitePool, _session_id: i64, _uniquename: &str, _unit_price: i64) -> Result<(), AlchemyError> {
    Ok(())
}

pub async fn mark_sent_to_marrow(_pool: &SqlitePool, _session_id: i64) -> Result<(), AlchemyError> {
    Ok(())
}
