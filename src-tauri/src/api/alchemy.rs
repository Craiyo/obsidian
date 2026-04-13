use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::api::{ApiError, AppState};
use crate::modules::alchemy;

#[derive(Debug, Deserialize)]
pub struct SessionListQuery {
    pub limit: Option<i64>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/plan",               axum::routing::post(plan))
        .route("/sessions",           get(list_sessions))
        .route("/sessions/:id",       get(get_session))
        .route("/sessions/:id/price", axum::routing::post(set_price))
        .route("/sessions/:id/send",  axum::routing::post(send_to_marrow))
}

async fn plan(
    State(state): State<AppState>,
    Json(payload): Json<alchemy::PlanRequest>,
) -> Result<Json<alchemy::PlanResponse>, ApiError> {
    let account = state.settings.accounts.iter()
        .find(|a| a.name == payload.account_name)
        .cloned()
        .ok_or_else(|| ApiError::new(
            axum::http::StatusCode::BAD_REQUEST,
            format!("Account '{}' not found in settings", payload.account_name),
        ))?;

    let response = alchemy::plan_session(&state.db, &account, payload.items).await?;
    Ok(Json(response))
}

async fn list_sessions(
    State(state): State<AppState>,
    Query(query): Query<SessionListQuery>,
) -> Result<Json<Vec<alchemy::SessionSummary>>, ApiError> {
    let limit = query.limit.unwrap_or(20);
    let sessions = alchemy::list_sessions(&state.db, limit).await?;
    Ok(Json(sessions))
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<alchemy::PlanResponse>, ApiError> {
    let session = alchemy::load_session(&state.db, id).await?;
    Ok(Json(session))
}

async fn set_price(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<alchemy::SetPriceRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    alchemy::set_material_price(&state.db, id, &payload.uniquename, payload.unit_price).await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn send_to_marrow(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<alchemy::PlanResponse>, ApiError> {
    alchemy::mark_sent_to_marrow(&state.db, id).await?;
    let session = alchemy::load_session(&state.db, id).await?;
    Ok(Json(session))
}
