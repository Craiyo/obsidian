use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SeanceError {
    #[error("invalid split type")]
    InvalidSplitType,
    #[error("no players provided")]
    NoPlayers,
    #[error("invalid player weight")]
    InvalidWeight,
    #[error("session not found")]
    SessionNotFound,
    #[error("insufficient wallet balance")]
    InsufficientBalance,
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub party_size: i64,
    pub total_loot_value: i64,
    pub split_type: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    pub id: i64,
    pub created_at: i64,
    pub party_size: i64,
    pub total_loot_value: i64,
    pub split_type: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PlayerWeight {
    pub player_name: String,
    pub weight: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct SplitRequest {
    pub players: Vec<PlayerWeight>,
}

#[derive(Debug, Serialize)]
pub struct SplitShare {
    pub player_name: String,
    pub weight: f64,
    pub share_value: i64,
}

#[derive(Debug, Serialize)]
pub struct SplitResponse {
    pub session_id: i64,
    pub total_loot_value: i64,
    pub shares: Vec<SplitShare>,
}

#[derive(Debug, Deserialize)]
pub struct WithdrawalRequest {
    pub player_name: String,
    pub amount: i64,
    pub reason: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WalletResponse {
    pub player_name: String,
    pub balance: i64,
}

#[derive(Debug, Deserialize)]
pub struct RegearRequest {
    pub amount: i64,
    pub reason: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegearSummary {
    pub balance: i64,
    pub recent: Vec<RegearEntry>,
}

#[derive(Debug, Serialize)]
pub struct RegearEntry {
    pub id: i64,
    pub amount: i64,
    pub reason: String,
    pub notes: Option<String>,
    pub created_at: i64,
}

pub async fn create_session(pool: &SqlitePool, req: CreateSessionRequest) -> Result<CreateSessionResponse, SeanceError> {
    let now = Utc::now().timestamp();
    let split_type = parse_split_type(&req.split_type)?;

    let result = sqlx::query(
        "INSERT INTO seance_sessions (created_at, party_size, total_loot_value, split_type, notes) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(now)
    .bind(req.party_size)
    .bind(req.total_loot_value)
    .bind(split_type)
    .bind(req.notes.clone())
    .execute(pool)
    .await?;

    Ok(CreateSessionResponse {
        id: result.last_insert_rowid(),
        created_at: now,
        party_size: req.party_size,
        total_loot_value: req.total_loot_value,
        split_type: req.split_type,
        notes: req.notes,
    })
}

pub async fn apply_split(pool: &SqlitePool, session_id: i64, req: SplitRequest) -> Result<SplitResponse, SeanceError> {
    if req.players.is_empty() {
        return Err(SeanceError::NoPlayers);
    }

    let session = sqlx::query("SELECT total_loot_value, split_type FROM seance_sessions WHERE id = ?")
        .bind(session_id)
        .fetch_optional(pool)
        .await?;

    let session = session.ok_or(SeanceError::SessionNotFound)?;

    let total_loot_value: i64 = session.get("total_loot_value");
    let split_type: String = session.get("split_type");
    let split_type = parse_split_type(&split_type)?;

    let weights: Vec<f64> = match split_type.as_str() {
        "equal" => vec![1.0; req.players.len()],
        "weighted" => req
            .players
            .iter()
            .map(|p| p.weight.unwrap_or(0.0))
            .collect(),
        _ => return Err(SeanceError::InvalidSplitType),
    };

    if weights.iter().any(|w| *w <= 0.0) {
        return Err(SeanceError::InvalidWeight);
    }

    let total_weight: f64 = weights.iter().sum();
    let mut remaining = total_loot_value;
    let mut shares = Vec::with_capacity(req.players.len());

    let mut tx = pool.begin().await?;

    for (idx, player) in req.players.iter().enumerate() {
        let weight = weights[idx];
        let share_value = if idx == req.players.len() - 1 {
            remaining
        } else {
            let calc = ((total_loot_value as f64) * (weight / total_weight)).floor() as i64;
            remaining -= calc;
            calc
        };

        sqlx::query(
            "INSERT INTO seance_session_shares (session_id, player_name, weight, share_value) VALUES (?, ?, ?, ?)"
        )
        .bind(session_id)
        .bind(&player.player_name)
        .bind(weight)
        .bind(share_value)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO seance_wallets (player_name, balance, updated_at) VALUES (?, ?, ?)             ON CONFLICT(player_name) DO UPDATE SET balance = balance + excluded.balance, updated_at = excluded.updated_at"
        )
        .bind(&player.player_name)
        .bind(share_value)
        .bind(Utc::now().timestamp())
        .execute(&mut *tx)
        .await?;

        shares.push(SplitShare {
            player_name: player.player_name.clone(),
            weight,
            share_value,
        });
    }

    tx.commit().await?;

    Ok(SplitResponse {
        session_id,
        total_loot_value,
        shares,
    })
}

pub async fn wallet(pool: &SqlitePool, player: &str) -> Result<WalletResponse, SeanceError> {
    let row = sqlx::query("SELECT player_name, balance FROM seance_wallets WHERE player_name = ?")
        .bind(player)
        .fetch_optional(pool)
        .await?;

    Ok(match row {
        Some(row) => WalletResponse {
            player_name: row.get("player_name"),
            balance: row.get("balance"),
        },
        None => WalletResponse {
            player_name: player.to_string(),
            balance: 0,
        },
    })
}

pub async fn record_withdrawal(pool: &SqlitePool, req: WithdrawalRequest) -> Result<WalletResponse, SeanceError> {
    let current = wallet(pool, &req.player_name).await?;
    if current.balance < req.amount {
        return Err(SeanceError::InsufficientBalance);
    }

    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO seance_withdrawals (player_name, amount, reason, notes, created_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&req.player_name)
    .bind(req.amount)
    .bind(&req.reason)
    .bind(req.notes)
    .bind(now)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO seance_wallets (player_name, balance, updated_at) VALUES (?, ?, ?)         ON CONFLICT(player_name) DO UPDATE SET balance = balance - excluded.balance, updated_at = excluded.updated_at"
    )
    .bind(&req.player_name)
    .bind(req.amount)
    .bind(now)
    .execute(pool)
    .await?;

    wallet(pool, &req.player_name).await
}

pub async fn record_regear(pool: &SqlitePool, req: RegearRequest) -> Result<RegearEntry, SeanceError> {
    let now = Utc::now().timestamp();
    let result = sqlx::query(
        "INSERT INTO seance_regear_transactions (amount, reason, notes, created_at) VALUES (?, ?, ?, ?)"
    )
    .bind(req.amount)
    .bind(&req.reason)
    .bind(req.notes.clone())
    .bind(now)
    .execute(pool)
    .await?;

    Ok(RegearEntry {
        id: result.last_insert_rowid(),
        amount: req.amount,
        reason: req.reason,
        notes: req.notes,
        created_at: now,
    })
}

pub async fn regear_summary(pool: &SqlitePool) -> Result<RegearSummary, SeanceError> {
    let balance_row = sqlx::query("SELECT COALESCE(SUM(amount), 0) as total FROM seance_regear_transactions")
        .fetch_one(pool)
        .await?;
    let balance: i64 = balance_row.get("total");

    let rows = sqlx::query(
        "SELECT id, amount, reason, notes, created_at FROM seance_regear_transactions ORDER BY created_at DESC LIMIT 10"
    )
    .fetch_all(pool)
    .await?;

    let recent = rows
        .into_iter()
        .map(|row| RegearEntry {
            id: row.get("id"),
            amount: row.get("amount"),
            reason: row.get("reason"),
            notes: row.get("notes"),
            created_at: row.get("created_at"),
        })
        .collect();

    Ok(RegearSummary { balance, recent })
}

fn parse_split_type(value: &str) -> Result<String, SeanceError> {
    let normalized = value.to_lowercase();
    match normalized.as_str() {
        "equal" | "weighted" => Ok(normalized),
        _ => Err(SeanceError::InvalidSplitType),
    }
}
