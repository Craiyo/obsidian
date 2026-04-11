use axum::{extract::{State}, routing::get, Json, Router};

use crate::api::{ApiError, AppState};
use crate::modules::alchemy;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/calculate", axum::routing::post(calculate))
        .route("/scenarios", get(list_scenarios).post(save_scenario))
}

async fn calculate(
    State(state): State<AppState>,
    Json(payload): Json<alchemy::CalculateRequest>,
) -> Result<Json<alchemy::CalculateResponse>, ApiError> {
    let response = alchemy::calculate(&state.db, &state.http, payload).await?;
    Ok(Json(response))
}

async fn save_scenario(
    State(state): State<AppState>,
    Json(payload): Json<alchemy::ScenarioRequest>,
) -> Result<Json<alchemy::ScenarioRow>, ApiError> {
    let response = alchemy::save_scenario(&state.db, &state.http, payload).await?;
    Ok(Json(response))
}

async fn list_scenarios(
    State(state): State<AppState>,
) -> Result<Json<Vec<alchemy::ScenarioRow>>, ApiError> {
    let response = alchemy::scenarios(&state.db).await?;
    Ok(Json(response))
}
