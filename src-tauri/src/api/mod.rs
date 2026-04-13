use axum::{
    extract::Request,
    middleware::{self, Next},
    response::Response,
    routing::get,
    Json, Router,
};
use serde_json::json;
use sqlx::SqlitePool;
use std::{net::SocketAddr, path::PathBuf};
use tower_http::cors::CorsLayer;

pub mod alchemy;
pub mod marrow;
pub mod seance;
pub mod settings;
pub mod chronicle;
pub mod effigy;
pub mod hemorrhage;
pub mod hex;
pub mod specter;
pub mod wail;
pub mod wraith;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub settings_path: PathBuf,
    pub http: reqwest::Client,
    pub albion_server: crate::settings::AlbionServer,
    pub settings: crate::settings::Settings,
}

impl AppState {
    pub fn new(
        db: SqlitePool,
        settings_path: PathBuf,
        settings: crate::settings::Settings,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to create reqwest client");
        Self {
            albion_server: settings.albion_server,
            db,
            settings_path,
            http,
            settings,
        }
    }
}

#[derive(Debug)]
pub struct ApiError {
    pub status: axum::http::StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn new(status: axum::http::StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = Json(json!({ "message": self.message }));
        (self.status, body).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
    }
}

impl From<crate::settings::SettingsError> for ApiError {
    fn from(err: crate::settings::SettingsError) -> Self {
        ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
    }
}

impl From<crate::modules::seance::SeanceError> for ApiError {
    fn from(err: crate::modules::seance::SeanceError) -> Self {
        use crate::modules::seance::SeanceError::*;
        match err {
            InvalidSplitType | NoPlayers | InvalidWeight | InsufficientBalance => {
                ApiError::new(axum::http::StatusCode::BAD_REQUEST, err.to_string())
            }
            SessionNotFound => ApiError::new(axum::http::StatusCode::NOT_FOUND, err.to_string()),
            Sqlx(_) => ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        }
    }
}

pub async fn serve(state: AppState) -> Result<(), std::io::Error> {
    let app = Router::new()
        .route("/api/v1/health", get(health))
        .nest("/api/v1/settings", settings::router())
        .nest("/api/v1/marrow", marrow::router())
        .nest("/api/v1/alchemy", alchemy::router())
        .nest("/api/v1/seance", seance::router())
        .nest("/api/v1/chronicle",   chronicle::router())
        .nest("/api/v1/effigy",      effigy::router())
        .nest("/api/v1/hemorrhage",  hemorrhage::router())
        .nest("/api/v1/hex",         hex::router())
        .nest("/api/v1/specter",     specter::router())
        .nest("/api/v1/wail",        wail::router())
        .nest("/api/v1/wraith",      wraith::router())
        .with_state(state)
        .layer(middleware::from_fn(aggregate_failure_log))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([127, 0, 0, 1], 38991));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

async fn aggregate_failure_log(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let res = next.run(req).await;
    let status = res.status();
    if status.is_client_error() || status.is_server_error() {
        eprintln!("api failure: {} {} -> {}", method, path, status);
    }
    res
}
