use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use thiserror::Error;

const GOLD_TTL_SECONDS: i64 = 60;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AlbionServer {
    Americas,
    Asia,
    Europe,
}

impl AlbionServer {
    pub fn base_url(&self) -> &'static str {
        match self {
            AlbionServer::Americas => "https://west.albion-online-data.com",
            AlbionServer::Asia => "https://east.albion-online-data.com",
            AlbionServer::Europe => "https://europe.albion-online-data.com",
        }
    }
}

#[derive(Debug, Error)]
pub enum MarrowError {
    #[error("not found")]
    NotFound,
    #[error("api error: {0}")]
    Api(#[from] reqwest::Error),
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid date range")]
    InvalidDateRange,
}

#[derive(Debug, Serialize)]
pub struct PriceResponse {
    pub uniquename: String,
    pub display_name: String,
    pub tier: i64,
    pub city: String,
    pub quality: i64,
    // Backward compatibility for existing call sites (e.g. Alchemy)
    pub sell_price: Option<i64>,
    pub buy_price: Option<i64>,
    pub sell_price_min: Option<i64>,
    pub sell_price_max: Option<i64>,
    pub buy_price_min: Option<i64>,
    pub buy_price_max: Option<i64>,
    pub sell_price_min_date: Option<String>,
    pub buy_price_max_date: Option<String>,
    pub fetched_at: i64,
    pub source: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HistoryPoint {
    pub item_count: i64,
    pub silver_amount: i64,
    pub avg_price: i64,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    pub uniquename: String,
    pub city: String,
    pub quality: i64,
    pub time_scale: i64,
    pub fetched_at: i64,
    pub source: String,
    pub points: Vec<HistoryPoint>,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub uniquename: String,
    pub display_name: String,
    pub tier: i64,
    pub shopcategory: String,
    pub shopsubcategory1: String,
}

#[derive(Debug, Serialize)]
pub struct GoldResponse {
    pub price: i64,
    pub timestamp: String,
}

#[derive(Debug, Deserialize)]
struct ApiPrice {
    item_id: String,
    city: String,
    quality: i64,
    sell_price_min: Option<i64>,
    sell_price_max: Option<i64>,
    buy_price_min: Option<i64>,
    buy_price_max: Option<i64>,
    sell_price_min_date: Option<String>,
    buy_price_max_date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ApiGoldPayload {
    List(Vec<ApiGoldPoint>),
    Object(ApiGoldPoint),
}

#[derive(Debug, Deserialize)]
struct ApiHistoryItem {
    item_id: String,
    location: String,
    quality: i64,
    data: Vec<ApiHistoryPoint>,
}

#[derive(Debug, Deserialize)]
struct ApiHistoryPoint {
    item_count: i64,
    avg_price: i64,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
struct ApiGoldPoint {
    price: i64,
    timestamp: String,
}

pub async fn get_price(
    pool: &SqlitePool,
    client: &reqwest::Client,
    server: AlbionServer,
    uniquename: &str,
    city: &str,
    quality: i64,
    ttl_seconds: i64,
) -> Result<PriceResponse, MarrowError> {
    get_price_with_base_url(
        pool,
        client,
        server.base_url(),
        uniquename,
        city,
        quality,
        ttl_seconds,
    )
    .await
}

pub async fn get_price_with_base_url(
    pool: &SqlitePool,
    client: &reqwest::Client,
    base_url: &str,
    uniquename: &str,
    city: &str,
    quality: i64,
    ttl_seconds: i64,
) -> Result<PriceResponse, MarrowError> {
    let now = Utc::now().timestamp();

    if let Some(row) = sqlx::query(
        r#"
        SELECT
            p.uniquename,
            COALESCE(i.display_name, p.uniquename) AS display_name,
            COALESCE(i.tier, 0) AS tier,
            p.city,
            p.quality,
            p.sell_price_min,
            p.sell_price_max,
            p.buy_price_min,
            p.buy_price_max,
            p.sell_price_min_date,
            p.buy_price_max_date,
            p.fetched_at
        FROM marrow_prices p
        LEFT JOIN items i ON i.uniquename = p.uniquename
        WHERE p.uniquename = ?1
          AND p.city = ?2
          AND p.quality = ?3
          AND p.ttl_expires_at > ?4
        "#,
    )
    .bind(uniquename)
    .bind(city)
    .bind(quality)
    .bind(now)
    .fetch_optional(pool)
    .await?
    {
        return Ok(PriceResponse {
            uniquename: row.get("uniquename"),
            display_name: row.get("display_name"),
            tier: row.get("tier"),
            city: row.get("city"),
            quality: row.get("quality"),
            sell_price: row.get("sell_price_min"),
            buy_price: row.get("buy_price_max"),
            sell_price_min: row.get("sell_price_min"),
            sell_price_max: row.get("sell_price_max"),
            buy_price_min: row.get("buy_price_min"),
            buy_price_max: row.get("buy_price_max"),
            sell_price_min_date: row.get("sell_price_min_date"),
            buy_price_max_date: row.get("buy_price_max_date"),
            fetched_at: row.get("fetched_at"),
            source: "cache".to_string(),
        });
    }

    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/api/v2/stats/prices/{uniquename}.json");
    let quality_str = quality.to_string();

    let prices: Vec<ApiPrice> = client
        .get(url)
        .query(&[("locations", city), ("qualities", quality_str.as_str())])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let p = prices
        .into_iter()
        .find(|x| x.item_id == uniquename && x.city.eq_ignore_ascii_case(city) && x.quality == quality)
        .ok_or(MarrowError::NotFound)?;

    let fetched_at = now;
    let ttl_expires_at = now + ttl_seconds.max(1);

    sqlx::query(
        r#"
        INSERT INTO marrow_prices (
            uniquename, city, quality,
            sell_price_min, sell_price_max,
            buy_price_min, buy_price_max,
            sell_price_min_date, buy_price_max_date,
            fetched_at, ttl_expires_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(uniquename, city, quality) DO UPDATE SET
            sell_price_min      = excluded.sell_price_min,
            sell_price_max      = excluded.sell_price_max,
            buy_price_min       = excluded.buy_price_min,
            buy_price_max       = excluded.buy_price_max,
            sell_price_min_date = excluded.sell_price_min_date,
            buy_price_max_date  = excluded.buy_price_max_date,
            fetched_at          = excluded.fetched_at,
            ttl_expires_at      = excluded.ttl_expires_at
        "#,
    )
    .bind(&p.item_id)
    .bind(&p.city)
    .bind(p.quality)
    .bind(p.sell_price_min)
    .bind(p.sell_price_max)
    .bind(p.buy_price_min)
    .bind(p.buy_price_max)
    .bind(p.sell_price_min_date.clone())
    .bind(p.buy_price_max_date.clone())
    .bind(fetched_at)
    .bind(ttl_expires_at)
    .execute(pool)
    .await?;

    let (display_name, tier) = load_item_meta(pool, &p.item_id).await?;

    Ok(PriceResponse {
        uniquename: p.item_id,
        display_name,
        tier,
        city: p.city,
        quality: p.quality,
        sell_price: p.sell_price_min,
        buy_price: p.buy_price_max,
        sell_price_min: p.sell_price_min,
        sell_price_max: p.sell_price_max,
        buy_price_min: p.buy_price_min,
        buy_price_max: p.buy_price_max,
        sell_price_min_date: p.sell_price_min_date,
        buy_price_max_date: p.buy_price_max_date,
        fetched_at,
        source: "api".to_string(),
    })
}

pub async fn get_history(
    pool: &SqlitePool,
    client: &reqwest::Client,
    server: AlbionServer,
    uniquename: &str,
    city: &str,
    quality: i64,
    days: i64,
) -> Result<HistoryResponse, MarrowError> {
    get_history_with_base_url(
        pool,
        client,
        server.base_url(),
        uniquename,
        city,
        quality,
        days,
    )
    .await
}

pub async fn get_history_with_base_url(
    pool: &SqlitePool,
    client: &reqwest::Client,
    base_url: &str,
    uniquename: &str,
    city: &str,
    quality: i64,
    days: i64,
) -> Result<HistoryResponse, MarrowError> {
    if days <= 0 {
        return Err(MarrowError::InvalidDateRange);
    }

    let now = Utc::now().timestamp();
    let time_scale = if days <= 1 {
        1
    } else if days <= 3 {
        6
    } else {
        24
    };

    if let Some(row) = sqlx::query(
        r#"
        SELECT data_json, fetched_at
        FROM marrow_history
        WHERE uniquename = ?1
          AND city = ?2
          AND quality = ?3
          AND time_scale = ?4
          AND fetched_at > ?5
        "#,
    )
    .bind(uniquename)
    .bind(city)
    .bind(quality)
    .bind(time_scale)
    .bind(now - 3600)
    .fetch_optional(pool)
    .await?
    {
        let data_json: String = row.get("data_json");
        let cutoff = now - (days * 86400);
        let points: Vec<HistoryPoint> = serde_json::from_str::<Vec<HistoryPoint>>(&data_json)?
            .into_iter()
            .filter(|p| iso_to_unix(&p.timestamp).is_some_and(|ts| ts >= cutoff))
            .collect();
        return Ok(HistoryResponse {
            uniquename: uniquename.to_string(),
            city: city.to_string(),
            quality,
            time_scale,
            fetched_at: row.get("fetched_at"),
            source: "cache".to_string(),
            points,
        });
    }

    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/api/v2/stats/history/{uniquename}.json");
    let quality_str = quality.to_string();
    let scale_str = time_scale.to_string();

    let response = client
        .get(url)
        .query(&[
            ("locations", city),
            ("qualities", quality_str.as_str()),
            ("time-scale", scale_str.as_str()),
        ])
        .send()
        .await
        .map_err(|e| {
            eprintln!("[marrow] history send error: {e}");
            e
        })?;
    let response = response.error_for_status().map_err(|e| {
        eprintln!("[marrow] history status error: {e}");
        e
    })?;
    let histories: Vec<ApiHistoryItem> = response
        .json()
        .await
        .map_err(|e| {
            eprintln!("[marrow] history json error: {e}");
            e
        })?;

    let h = histories
        .into_iter()
        .find(|x| x.item_id == uniquename && x.location.eq_ignore_ascii_case(city) && x.quality == quality)
        .ok_or(MarrowError::NotFound)?;

    let points: Vec<HistoryPoint> = h
        .data
        .into_iter()
        .map(|p| {
            let silver_amount = p.avg_price.saturating_mul(p.item_count);
            HistoryPoint {
                item_count: p.item_count,
                silver_amount,
                avg_price: p.avg_price,
                timestamp: p.timestamp,
            }
        })
        .collect();

    // Store complete points vector as one row (data_json cache pattern).
    let data_json = serde_json::to_string(&points)?;
    let fetched_at = now;

    sqlx::query(
        r#"
        INSERT INTO marrow_history (
            uniquename, city, quality, time_scale, data_json, fetched_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(uniquename, city, quality, time_scale) DO UPDATE SET
            data_json  = excluded.data_json,
            fetched_at = excluded.fetched_at
        "#,
    )
    .bind(uniquename)
    .bind(city)
    .bind(quality)
    .bind(time_scale)
    .bind(data_json)
    .bind(fetched_at)
    .execute(pool)
    .await?;

    Ok(HistoryResponse {
        uniquename: uniquename.to_string(),
        city: city.to_string(),
        quality,
        time_scale,
        fetched_at,
        source: "api".to_string(),
        points,
    })
}

pub async fn search(pool: &SqlitePool, query: &str) -> Result<Vec<SearchResult>, MarrowError> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT uniquename, display_name, tier, shopcategory, shopsubcategory1
        FROM items
        WHERE display_name LIKE '%' || ?1 || '%'
           OR uniquename   LIKE '%' || ?1 || '%'
        ORDER BY tier, display_name
        LIMIT 50
        "#,
    )
    .bind(q)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SearchResult {
            uniquename: r.get("uniquename"),
            display_name: r.get::<Option<String>, _>("display_name").unwrap_or_default(),
            tier: r.get::<Option<i64>, _>("tier").unwrap_or(0),
            shopcategory: r.get::<Option<String>, _>("shopcategory").unwrap_or_default(),
            shopsubcategory1: r.get::<Option<String>, _>("shopsubcategory1").unwrap_or_default(),
        })
        .collect())
}

pub async fn get_favourites(pool: &SqlitePool) -> Result<Vec<String>, MarrowError> {
    let rows = sqlx::query("SELECT uniquename FROM marrow_favourites ORDER BY added_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.get("uniquename")).collect())
}

pub async fn add_favourite(pool: &SqlitePool, uniquename: &str) -> Result<(), MarrowError> {
    let exists = sqlx::query("SELECT COUNT(*) AS cnt FROM items WHERE uniquename = ?1")
        .bind(uniquename)
        .fetch_one(pool)
        .await?;
    let cnt: i64 = exists.get("cnt");
    if cnt == 0 {
        return Err(MarrowError::NotFound);
    }

    let now = Utc::now().timestamp();
    sqlx::query("INSERT OR IGNORE INTO marrow_favourites (uniquename, added_at) VALUES (?1, ?2)")
        .bind(uniquename)
        .bind(now)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn remove_favourite(pool: &SqlitePool, uniquename: &str) -> Result<(), MarrowError> {
    sqlx::query("DELETE FROM marrow_favourites WHERE uniquename = ?1")
        .bind(uniquename)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_gold(
    pool: &SqlitePool,
    client: &reqwest::Client,
) -> Result<GoldResponse, MarrowError> {
    get_gold_with_base_url(pool, client, AlbionServer::Americas.base_url()).await
}

pub async fn get_gold_with_base_url(
    pool: &SqlitePool,
    client: &reqwest::Client,
    base_url: &str,
) -> Result<GoldResponse, MarrowError> {
    let now = Utc::now().timestamp();

    if let Some(row) = sqlx::query("SELECT price, timestamp, fetched_at FROM marrow_gold WHERE id = 1")
        .fetch_optional(pool)
        .await?
    {
        let fetched_at: i64 = row.get("fetched_at");
        if fetched_at > now - GOLD_TTL_SECONDS {
            return Ok(GoldResponse {
                price: row.get("price"),
                timestamp: row.get("timestamp"),
            });
        }
    }

    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/api/v2/stats/gold.json?count=1");
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| {
            eprintln!("[marrow] gold fetch send error: {e}");
            e
        })?;
    let response = response.error_for_status().map_err(|e| {
        eprintln!("[marrow] gold fetch status error: {e}");
        e
    })?;
    let payload: ApiGoldPayload = response
        .json()
        .await
        .map_err(|e| {
            eprintln!("[marrow] gold fetch json error: {e}");
            e
        })?;
    let p = match payload {
        ApiGoldPayload::List(points) => points.into_iter().next().ok_or(MarrowError::NotFound)?,
        ApiGoldPayload::Object(point) => point,
    };

    sqlx::query(
        r#"
        INSERT INTO marrow_gold (id, price, timestamp, fetched_at)
        VALUES (1, ?1, ?2, ?3)
        ON CONFLICT(id) DO UPDATE SET
            price = excluded.price,
            timestamp = excluded.timestamp,
            fetched_at = excluded.fetched_at
        "#,
    )
    .bind(p.price)
    .bind(&p.timestamp)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(GoldResponse {
        price: p.price,
        timestamp: p.timestamp,
    })
}

async fn load_item_meta(pool: &SqlitePool, uniquename: &str) -> Result<(String, i64), MarrowError> {
    if let Some(row) = sqlx::query("SELECT display_name, tier FROM items WHERE uniquename = ?1")
        .bind(uniquename)
        .fetch_optional(pool)
        .await?
    {
        Ok((
            row.get::<Option<String>, _>("display_name")
                .unwrap_or_else(|| uniquename.to_string()),
            row.get::<Option<i64>, _>("tier").unwrap_or(0),
        ))
    } else {
        Ok((uniquename.to_string(), 0))
    }
}

fn iso_to_unix(ts: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(ts).ok().map(|d| d.timestamp())
}
