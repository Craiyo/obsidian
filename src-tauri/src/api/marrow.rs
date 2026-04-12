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
pub struct BulkHistoryQuery {
    pub city: String,
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

#[derive(Debug, Deserialize)]
pub struct ItemsQuery {
    pub ids: String,
}

async fn recommend_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<RecommendQuery>,
) -> Result<Json<crate::modules::marrow::RecommendDecision>, ApiError> {
    let quality = query.quality.unwrap_or(1);
    let days = query.days.unwrap_or(14);
    let decision = crate::modules::marrow::recommend_item(
        &state.db,
        &state.http,
        state.albion_server,
        &id,
        &query.city,
        quality,
        days,
        state.return_rate_pct,
        state.crafting_fee_pct,
    )
    .await?;
    Ok(Json(decision))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/item/:id", get(price))
        .route("/history/:id", get(history))
        .route("/history_bulk/:id", get(history_bulk))
        .route("/search", get(search))
        .route("/favourites", get(favourites).post(add_favourite))
        .route("/favourites/:id", delete(remove_favourite))
        .route("/recommend/:id", get(recommend_item))
        .route("/gold", get(gold))
        .route("/items", get(items))
        .route("/ingest/marketorders", axum::routing::post(ingest_market_orders))
        .route("/ingest/markethistory", axum::routing::post(ingest_market_history))
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

async fn history_bulk(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<BulkHistoryQuery>,
) -> Result<Json<Vec<marrow::HistoryResponse>>, ApiError> {
    let days = query.days.unwrap_or(7);
    let response = marrow::get_history_bulk(
        &state.db,
        &state.http,
        state.albion_server,
        &id,
        &query.city,
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

async fn items(
    State(state): State<AppState>,
    Query(query): Query<ItemsQuery>,
) -> Result<Json<Vec<marrow::SearchResult>>, ApiError> {
    let ids: Vec<String> = query.ids.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let results = marrow::get_items_by_ids(&state.db, &ids).await?;
    Ok(Json(results))
}
async fn ingest_market_orders(
    State(state): State<AppState>,
    Json(payload): Json<marrow::IngestMarketUpload>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let item_ids: Vec<String> = payload.Orders.iter().map(|o| o.ItemTypeId.clone()).collect();
    
    marrow::ingest_market_orders(&state.db, payload).await?;
    
    // Notify UI that these items have new data
    use tauri::Manager;
    let _ = state.app_handle.emit_all("marrow-ingest", item_ids);
    
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

async fn ingest_market_history(
    State(state): State<AppState>,
    Json(payload): Json<marrow::IngestMarketHistoriesUpload>,
) -> Result<Json<serde_json::Value>, ApiError> {
    marrow::ingest_market_history(&state.db, payload).await?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}
