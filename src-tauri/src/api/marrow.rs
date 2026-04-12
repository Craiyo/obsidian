use axum::{extract::{Query, Path, State}, routing::get, Json, Router};
use serde::Deserialize;
use crate::api::{ApiError, AppState};
use crate::modules::marrow;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Debug, Deserialize)]
pub struct ItemsQuery {
    pub ids: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/search", get(search))
        .route("/items", get(items))
        .route("/gold", get(gold))
        .route("/recommend/:id", get(recommend_stub))
}

async fn recommend_stub() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "recommended": false,
        "reason": "Marrow Engine analysis is currently disabled (Zero Code Slate)."
    }))
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
    // Basic gold price fetch (External API)
    let url = "https://west.albion-onlinedataproject.com/api/v2/stats/gold.json?count=1";
    let client = reqwest::Client::new();
    let points: Vec<serde_json::Value> = client.get(url).send().await
        .map_err(|e| ApiError::new(axum::http::StatusCode::BAD_GATEWAY, e.to_string()))?
        .json().await
        .map_err(|e| ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    let p = points.into_iter().next().ok_or(ApiError::new(axum::http::StatusCode::NOT_FOUND, "Gold data unavailable"))?;
    Ok(Json(p))
}
