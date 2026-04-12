use axum::{extract::{Query, Path, State}, routing::{delete, get}, Json, Router};
use serde::Deserialize;

use crate::api::{ApiError, AppState};
use crate::modules::marrow;

#[derive(Debug, Deserialize)]
pub struct PriceQuery {
    pub city: String,
    pub quality: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub city: String,
    pub quality: Option<i64>,
    pub days: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Debug, Deserialize)]
pub struct FavouriteRequest {
    pub uniquename: String,
}

#[derive(Debug, Deserialize)]
pub struct RecommendQuery {
    pub city: String,
    pub quality: Option<i64>,
    pub days: Option<i64>,
}

async fn recommend_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<RecommendQuery>,
) -> Result<Json<crate::modules::marrow_recommend::RecommendDecision>, ApiError> {
    let quality = query.quality.unwrap_or(1);
    let days = query.days.unwrap_or(14);
    let decision = crate::modules::marrow_recommend::recommend_item(
        &state.db,
        &state.http,
        state.albion_server,
        &id,
        &query.city,
        quality,
        days,
    )
    .await?;
    Ok(Json(decision))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/item/:id", get(price))
        .route("/history/:id", get(history))
        .route("/search", get(search))
        .route("/favourites", get(favourites).post(add_favourite))
        .route("/favourites/:id", delete(remove_favourite))
        .route("/recommend/:id", get(recommend_item))
        .route("/gold", get(gold))
}

async fn price(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<PriceQuery>,
) -> Result<Json<marrow::PriceResponse>, ApiError> {
    let quality = query.quality.unwrap_or(1);
    let response = marrow::get_price(
        &state.db,
        &state.http,
        state.albion_server,
        &id,
        &query.city,
        quality,
        300,
    )
    .await?;
    Ok(Json(response))
}

async fn history(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<marrow::HistoryResponse>, ApiError> {
    let quality = query.quality.unwrap_or(1);
    let days = query.days.unwrap_or(7);
    let response = marrow::get_history(
        &state.db,
        &state.http,
        state.albion_server,
        &id,
        &query.city,
        quality,
        days,
    )
    .await?;
    Ok(Json(response))
}

async fn search(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<marrow::SearchResult>>, ApiError> {
    let results = marrow::search(&state.db, &query.q).await?;
    Ok(Json(results))
}

async fn favourites(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, ApiError> {
    let results = marrow::get_favourites(&state.db).await?;
    Ok(Json(results))
}

async fn add_favourite(
    State(state): State<AppState>,
    Json(payload): Json<FavouriteRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    marrow::add_favourite(&state.db, &payload.uniquename).await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn remove_favourite(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    marrow::remove_favourite(&state.db, &id).await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn gold(
    State(state): State<AppState>,
) -> Result<Json<marrow::GoldResponse>, ApiError> {
    let response = marrow::get_gold(&state.db, &state.http).await?;
    Ok(Json(response))
}
