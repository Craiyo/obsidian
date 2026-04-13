use axum::{extract::{Query, Path, State}, routing::get, Json, Router};
use serde::Deserialize;
use crate::api::{ApiError, AppState};
use crate::modules::marrow;
use crate::modules::marrow_recommend;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Debug, Deserialize)]
pub struct ItemsQuery {
    pub ids: String,
}

#[derive(Debug, Deserialize)]
pub struct RecommendQuery {
    pub city: String,
    pub quality: Option<i64>,
    pub days: Option<i64>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/search", get(search))
        .route("/items", get(items))
        .route("/gold", get(gold))
        .route("/recommend/:id", get(recommend_item))
}

async fn recommend_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<RecommendQuery>,
) -> Result<Json<marrow_recommend::RecommendDecision>, ApiError> {
    let quality = query.quality.unwrap_or(1);
    let days = query.days.unwrap_or(14);

    // Find account whose city matches the requested city, fall back to first account
    let account = state.settings.accounts.iter()
        .find(|a| a.city.eq_ignore_ascii_case(&query.city))
        .or_else(|| state.settings.accounts.first())
        .cloned()
        .unwrap_or_default();

    let decision = marrow_recommend::recommend_item(
        &state.db,
        &state.http,
        state.albion_server,
        &id,
        &query.city,
        quality,
        days,
        &account,
    )
    .await
    .map_err(|e| ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(decision))
}

async fn search(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<marrow::SearchResult>>, ApiError> {
    let results = marrow::search(&state.db, &query.q).await
        .map_err(|e| ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(results))
}

async fn items(
    State(state): State<AppState>,
    Query(query): Query<ItemsQuery>,
) -> Result<Json<Vec<marrow::SearchResult>>, ApiError> {
    let ids: Vec<String> = query.ids.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let results = marrow::get_items_by_ids(&state.db, &ids).await
        .map_err(|e| ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(results))
}

async fn gold(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let base = state.albion_server.base_url();
    let url = format!("{base}/api/v2/stats/gold.json?count=1");
    let points: Vec<serde_json::Value> = state.http.get(&url).send().await
        .map_err(|e| ApiError::new(axum::http::StatusCode::BAD_GATEWAY, e.to_string()))?
        .json().await
        .map_err(|e| ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let p = points.into_iter().next()
        .ok_or(ApiError::new(axum::http::StatusCode::NOT_FOUND, "Gold data unavailable"))?;
    Ok(Json(p))
}
