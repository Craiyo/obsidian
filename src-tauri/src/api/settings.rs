use axum::{routing::get, Json, Router};
use axum::extract::State;

use crate::api::{ApiError, AppState};
use crate::settings;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(get_settings).put(update_settings))
}

async fn get_settings(State(state): State<AppState>) -> Result<Json<settings::Settings>, ApiError> {
    let settings = settings::load(&state.settings_path).await?;
    Ok(Json(settings))
}

async fn update_settings(
    State(state): State<AppState>,
    Json(payload): Json<settings::Settings>,
) -> Result<Json<settings::Settings>, ApiError> {
    settings::save(&state.settings_path, &payload).await?;
    Ok(Json(payload))
}
