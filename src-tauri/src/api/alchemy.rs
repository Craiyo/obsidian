use axum::{extract::{Query, State}, routing::get, Json, Router};
use serde::Deserialize;
use crate::api::{ApiError, AppState};
use crate::modules::alchemy;

#[derive(Debug, Deserialize)]
pub struct AnalyzeQuery {
    pub item_id: String,
    pub batch_size: Option<i64>,
    pub use_focus: Option<bool>,
    pub daily_bonus: Option<bool>,
    pub is_hideout: Option<bool>,
    pub hideout_power: Option<i64>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/analyze", get(analyze))
}

async fn analyze(
    State(state): State<AppState>,
    Query(query): Query<AnalyzeQuery>,
) -> Result<Json<alchemy::AlchemyAnalysis>, ApiError> {
    let batch_size = query.batch_size.unwrap_or(1);
    let use_focus = query.use_focus.unwrap_or(false);
    let daily_bonus = query.daily_bonus.unwrap_or(false);
    let is_hideout = query.is_hideout.unwrap_or(false);
    let hideout_power = query.hideout_power.unwrap_or(0);

    let analysis = alchemy::analyze_yield(
        &state.db,
        &query.item_id,
        batch_size,
        use_focus,
        daily_bonus,
        is_hideout,
        hideout_power,
    )
    .await
    .map_err(|e| ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(analysis))
}
