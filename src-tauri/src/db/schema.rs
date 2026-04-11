use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PriceRow {
    pub item_id: String,
    pub city: String,
    pub quality: i64,
    pub buy_price: Option<i64>,
    pub sell_price: Option<i64>,
    pub updated_at: i64,
    pub ttl_expires_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryRow {
    pub item_id: String,
    pub city: String,
    pub quality: i64,
    pub timestamp: i64,
    pub avg_price: i64,
    pub volume: Option<i64>,
}
