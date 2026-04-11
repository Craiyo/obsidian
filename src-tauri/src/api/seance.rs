use axum::{extract::{Path, State}, routing::get, Json, Router};

use crate::api::{ApiError, AppState};
use crate::modules::seance;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/session", axum::routing::post(create_session))
        .route("/session/:id/split", axum::routing::post(split_session))
        .route("/wallet/:player", get(wallet))
        .route("/withdrawal", axum::routing::post(withdrawal))
        .route("/regear", get(regear).post(regear_post))
}

async fn create_session(
    State(state): State<AppState>,
    Json(payload): Json<seance::CreateSessionRequest>,
) -> Result<Json<seance::CreateSessionResponse>, ApiError> {
    let response = seance::create_session(&state.db, payload).await?;
    Ok(Json(response))
}

async fn split_session(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<seance::SplitRequest>,
) -> Result<Json<seance::SplitResponse>, ApiError> {
    let response = seance::apply_split(&state.db, id, payload).await?;
    Ok(Json(response))
}

async fn wallet(
    State(state): State<AppState>,
    Path(player): Path<String>,
) -> Result<Json<seance::WalletResponse>, ApiError> {
    let response = seance::wallet(&state.db, &player).await?;
    Ok(Json(response))
}

async fn withdrawal(
    State(state): State<AppState>,
    Json(payload): Json<seance::WithdrawalRequest>,
) -> Result<Json<seance::WalletResponse>, ApiError> {
    let response = seance::record_withdrawal(&state.db, payload).await?;
    Ok(Json(response))
}

async fn regear(
    State(state): State<AppState>,
) -> Result<Json<seance::RegearSummary>, ApiError> {
    let response = seance::regear_summary(&state.db).await?;
    Ok(Json(response))
}

async fn regear_post(
    State(state): State<AppState>,
    Json(payload): Json<seance::RegearRequest>,
) -> Result<Json<seance::RegearEntry>, ApiError> {
    let response = seance::record_regear(&state.db, payload).await?;
    Ok(Json(response))
}
