use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::api::{ApiError, AppState};
use crate::modules::alchemy;
use crate::modules::alchemy as alchemy_api;

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
    Json(payload): Json<alchemy_api::PlanRequest>,
) -> Result<Json<alchemy_api::PlanResponse>, ApiError> {
    let account = state.settings.accounts.iter()
        .find(|a| a.name == payload.account_name)
        .cloned()
        .ok_or_else(|| ApiError::new(
            axum::http::StatusCode::BAD_REQUEST,
            format!("Account '{}' not found in settings", payload.account_name),
        ))?;

    let response = alchemy_api::plan_session(&state.db, &account, payload.items).await.map_err(ApiError::from)?;
    Ok(Json(response))
}

async fn list_sessions(
    State(state): State<AppState>,
    Query(query): Query<SessionListQuery>,
) -> Result<Json<Vec<alchemy_api::SessionSummary>>, ApiError> {
    let limit = query.limit.unwrap_or(20);
    let sessions = alchemy_api::list_sessions(&state.db, limit).await.map_err(ApiError::from)?;
    Ok(Json(sessions))
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<alchemy_api::PlanResponse>, ApiError> {
    let session = alchemy_api::load_session(&state.db, id).await.map_err(ApiError::from)?;
    Ok(Json(session))
}

async fn set_price(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<alchemy_api::SetPriceRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    alchemy_api::set_material_price(&state.db, id, &payload.uniquename, payload.unit_price).await.map_err(ApiError::from)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn send_to_marrow(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<alchemy_api::PlanResponse>, ApiError> {
    alchemy_api::mark_sent_to_marrow(&state.db, id).await.map_err(ApiError::from)?;
    let session = alchemy_api::load_session(&state.db, id).await.map_err(ApiError::from)?;
    Ok(Json(session))
}
