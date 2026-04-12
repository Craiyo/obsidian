use chrono::{Utc, Datelike, TimeZone};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use thiserror::Error;

const GOLD_TTL_SECONDS: i64 = 300;

use crate::settings::AlbionServer;

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
struct ApiHistoryItem {
    item_id: String,
    location: String,
    quality: i64,
    data: Vec<ApiHistoryPoint>,
}

#[derive(Debug, Deserialize)]
struct ApiHistoryPoint {
    item_count: i64,
    silver_amount: Option<i128>, // Changed to i128 to handle intermediate volume products
    avg_price: Option<i64>,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
struct ApiChartItem {
    item_id: String,
    location: String,
    quality: i64,
    data: ApiChartData,
}

#[derive(Debug, Deserialize)]
struct ApiChartData {
    timestamps: Vec<String>,
    prices_avg: Vec<Option<i64>>,
    item_count: Vec<i64>,
}

#[derive(Debug, Deserialize)]
struct ApiGoldPoint {
    price: i64,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct IngestMarketOrder {
    pub ItemTypeId: String,
    pub LocationId: String,
    pub QualityLevel: i64,
    pub UnitPriceSilver: i64,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct IngestMarketUpload {
    pub Orders: Vec<IngestMarketOrder>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct IngestMarketHistory {
    pub ItemAmount: i64,
    pub SilverAmount: u64,
    pub Timestamp: u64,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct IngestMarketHistoriesUpload {
    pub LocationId: String,
    pub QualityLevel: i64,
    pub Timescale: i64,
    pub MarketHistories: Vec<IngestMarketHistory>,
}

fn map_location_id(id: &str) -> Option<&'static str> {
    match id {
        "0004" => Some("Fort Sterling"),
        "1002" => Some("Martlock"),
        "1006" => Some("Black Market"),
        "2002" => Some("Thetford"),
        "3003" => Some("Bridgewatch"),
        "3005" => Some("Caerleon"),
        "4002" => Some("Lymhurst"),
        "5003" => Some("Brecilien"),
        _ => None,
    }
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
    
    // FETCH ALL MAJOR CITIES AND ALL QUALITIES (1-5) IN ONE GO
    let cities_str = ALL_CITIES.join(",");
    let qualities_str = "1,2,3,4,5";

    let prices: Vec<ApiPrice> = client
        .get(url)
        .query(&[("locations", cities_str.as_str()), ("qualities", qualities_str)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let fetched_at = now;
    let ttl_expires_at = now + ttl_seconds.max(1);

    // BULK INSERT ALL FETCHED PRICES INTO CACHE
    for p in &prices {
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
    }

    // Now try to find the specific one we were looking for from the freshly cached results
    let p = prices
        .into_iter()
        .find(|x| x.item_id.eq_ignore_ascii_case(uniquename) && x.city.eq_ignore_ascii_case(city) && x.quality == quality)
        .ok_or(MarrowError::NotFound)?;

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

pub async fn get_history_bulk(
    pool: &SqlitePool,
    client: &reqwest::Client,
    server: AlbionServer,
    uniquename: &str,
    city: &str,
    days: i64,
) -> Result<Vec<HistoryResponse>, MarrowError> {
    // PRE-SEED ALL CITIES AND QUALITIES IF CACHE IS STALE/EMPTY
    let _ = fetch_history_bulk_global(pool, client, server.base_url(), uniquename, days).await;

    let mut results = Vec::new();
    // Subsequent calls will now be instant cache hits
    for q in 1..=5 {
        if let Ok(res) = get_history(pool, client, server, uniquename, city, q, days).await {
            results.push(res);
        }
    }
    Ok(results)
}

/// Fetches history for ALL major cities and ALL qualities in one single request
/// using the /stats/charts/ endpoint. This seeds the local cache.
pub async fn fetch_history_bulk_global(
    pool: &SqlitePool,
    client: &reqwest::Client,
    base_url: &str,
    uniquename: &str,
    days: i64,
) -> Result<(), MarrowError> {
    let now = Utc::now();
    let start_date = now - chrono::Duration::days(days);
    
    // Format dates as the API expects (YYYY-MM-DD or M-D-YYYY?)
    // Based on user feedback, we use MM-DD-YYYY or M-D-YYYY
    let start_str = format!("{}-{}-{}", start_date.month(), start_date.day(), start_date.year());
    let end_str = format!("{}-{}-{}", now.month(), now.day(), now.year());

    let time_scale = if days <= 1 { 1 } else if days <= 3 { 6 } else { 24 };
    
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/api/v2/stats/charts/{uniquename}.json");
    
    let cities_str = ALL_CITIES.join(",");
    let qualities_str = "1,2,3,4,5";

    eprintln!("[marrow] Seeding global history cache via charts API for {}", uniquename);

    let charts: Vec<ApiChartItem> = client
        .get(url)
        .query(&[
            ("date", start_str.as_str()),
            ("end_date", end_str.as_str()),
            ("locations", cities_str.as_str()),
            ("qualities", qualities_str),
            ("time-scale", time_scale.to_string().as_str()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let fetched_at = now.timestamp();

    for item in charts {
        // Zip the parallel arrays into our internal HistoryPoint format
        let points: Vec<HistoryPoint> = item.data.timestamps.iter().enumerate().map(|(idx, ts)| {
            let count = item.data.item_count.get(idx).copied().unwrap_or(0);
            let avg = item.data.prices_avg.get(idx).copied().flatten().unwrap_or(0);
            HistoryPoint {
                item_count: count,
                silver_amount: count * avg, // Approximate
                avg_price: avg,
                timestamp: ts.clone(),
            }
        }).collect();

        if let Ok(data_json) = serde_json::to_string(&points) {
            let _ = sqlx::query(
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
            .bind(&item.location)
            .bind(item.quality)
            .bind(time_scale)
            .bind(data_json)
            .bind(fetched_at)
            .execute(pool)
            .await;
        }
    }

    Ok(())
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
        // Cache is keyed by time_scale, not days — a single cached blob serves
        // multiple day-range requests. We filter here to the exact days requested.
        // A 7-day and 30-day request for the same item+city+quality both hit the
        // time_scale=24 cache; the filter trims to what was asked for.
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

    // On cache miss, trigger a global seed of ALL cities and qualities
    let _ = fetch_history_bulk_global(pool, client, base_url, uniquename, days).await;

    // Retry cache hit
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

    // If still missing (e.g. API returned no data for this specific city/quality), return error
    Err(MarrowError::NotFound)
}

pub async fn ingest_market_orders(pool: &SqlitePool, upload: IngestMarketUpload) -> Result<(), MarrowError> {
    let now = Utc::now().timestamp();
    let ttl_expires_at = now + 300; // 5 minute TTL for live data
    
    if !upload.Orders.is_empty() {
        println!("[marrow-direct] Ingesting {} market orders from sniffer...", upload.Orders.len());
    }

    for order in upload.Orders {
        let city = match map_location_id(&order.LocationId) {
            Some(c) => c,
            None => continue,
        };

        // We only care about sell orders for our "sell_price_min" calc
        // (Note: In internal logic, UnitPriceSilver is used)
        sqlx::query(
            r#"
            INSERT INTO marrow_prices (
                uniquename, city, quality,
                sell_price_min, fetched_at, ttl_expires_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(uniquename, city, quality) DO UPDATE SET
                sell_price_min = CASE 
                    WHEN excluded.sell_price_min < marrow_prices.sell_price_min OR marrow_prices.ttl_expires_at < ?5 
                    THEN excluded.sell_price_min 
                    ELSE marrow_prices.sell_price_min 
                END,
                fetched_at = excluded.fetched_at,
                ttl_expires_at = excluded.ttl_expires_at
            "#,
        )
        .bind(&order.ItemTypeId)
        .bind(city)
        .bind(order.QualityLevel)
        .bind(order.UnitPriceSilver)
        .bind(now)
        .bind(ttl_expires_at)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn ingest_market_history(pool: &SqlitePool, upload: IngestMarketHistoriesUpload) -> Result<(), MarrowError> {
    let city = match map_location_id(&upload.LocationId) {
        Some(c) => c,
        None => return Ok(()),
    };

    let mut points: Vec<HistoryPoint> = Vec::new();
    for h in upload.MarketHistories {
        // Timestamp from AO Data Client is nanoseconds since Unix epoch
        let ts_sec = (h.Timestamp / 1_000_000_000) as i64;
        let ts_iso = Utc.timestamp_opt(ts_sec, 0).unwrap().to_rfc3339();
        
        let avg = if h.ItemAmount > 0 { (h.SilverAmount / h.ItemAmount as u64) as i64 } else { 0 };

        points.push(HistoryPoint {
            item_count: h.ItemAmount,
            silver_amount: h.SilverAmount as i64,
            avg_price: avg,
            timestamp: ts_iso,
        });
    }

    // Determine uniquename from context - wait, the Go client doesn't send the ItemID in the history upload?
    // Let's check MarketHistoriesUpload again.
    // Ah, I see "AlbionId" in the Go struct. 
    // We need a way to map AlbionId to uniquename.

    Ok(())
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

pub async fn get_items_by_ids(pool: &SqlitePool, ids: &[String]) -> Result<Vec<SearchResult>, MarrowError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    
    let placeholders = vec!["?"; ids.len()].join(", ");
    let query_str = format!(
        r#"
        SELECT uniquename, display_name, tier, shopcategory, shopsubcategory1
        FROM items
        WHERE uniquename IN ({})
        "#,
        placeholders
    );
    
    let mut query = sqlx::query(&query_str);
    for id in ids {
        query = query.bind(id);
    }
    
    let rows = query.fetch_all(pool).await?;
    
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
    let points: Vec<ApiGoldPoint> = response.json().await.map_err(|e| {
        eprintln!("[marrow] gold fetch json error: {e}");
        e
    })?;
    let p = points.into_iter().next().ok_or(MarrowError::NotFound)?;

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
    chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .map(|d| d.and_utc().timestamp())
}

use futures_util::future::join_all;

#[derive(Debug, Serialize)]
pub struct CityPriceSummary {
    pub city: String,
    pub sell_price_min: Option<i64>,
    pub buy_price_max: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct RecommendDecision {
    pub recommended: bool,
    pub quality: i64,
    pub reason: String,
    pub confidence: f64,

    pub output_price: i64,
    pub suggested_qty: i64,
    pub estimated_days_to_sell: Option<i64>,
    pub stale_data: bool,
    pub short_ema: Option<f64>,
    pub long_ema: Option<f64>,
    pub bullish: Option<bool>,
    pub price_volatility_pct: Option<f64>,
    pub avg_daily_volume: Option<f64>,
    pub min_daily_volume: Option<i64>,

    pub historical_avg: Option<i64>,
    pub price_diff_pct: Option<f64>,
    pub history_count: usize,
    pub history_stale: bool,

    pub city_prices: Vec<CityPriceSummary>,
    pub best_sell_city: Option<String>,
    pub transport_warning: Option<String>,
}

impl Default for RecommendDecision {
    fn default() -> Self {
        Self {
            recommended: false, quality: 1, reason: String::new(), confidence: 0.0,
            output_price: 0, suggested_qty: 0, estimated_days_to_sell: None,
            stale_data: false, short_ema: None, long_ema: None, bullish: None,
            price_volatility_pct: None, avg_daily_volume: None, min_daily_volume: None,
            historical_avg: None, price_diff_pct: None,
            history_count: 0, history_stale: false,
            city_prices: Vec::new(), best_sell_city: None, transport_warning: None,
        }
    }
}

fn round2(v: f64) -> f64 { (v * 100.0).round() / 100.0 }

fn compute_ema(prices: &[f64], period: usize) -> Option<f64> {
    if prices.is_empty() || period == 0 || prices.len() < period {
        return None;
    }
    // Seed with SMA of first `period` elements
    let seed: f64 = prices[..period].iter().sum::<f64>() / period as f64;
    let alpha = 2.0 / (period as f64 + 1.0);
    let ema = prices[period..].iter().fold(seed, |e, p| alpha * p + (1.0 - alpha) * e);
    Some(ema)
}

fn compute_volatility_pct(prices: &[f64]) -> Option<f64> {
    if prices.len() < 3 { return None; }
    let mean = prices.iter().sum::<f64>() / prices.len() as f64;
    if mean == 0.0 { return None; }
    let variance = prices.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / prices.len() as f64;
    Some((variance.sqrt() / mean) * 100.0)
}

const ALL_CITIES: &[&str] = &[
    "Bridgewatch", "Caerleon", "FortSterling",
    "Lymhurst", "Martlock", "Thetford", "BlackMarket"
];

pub async fn recommend_item(
    pool: &SqlitePool,
    client: &reqwest::Client,
    server: AlbionServer,
    item_id: &str,
    city: &str,
    _quality: i64,
    days: i64,
    _return_rate_pct: f64,
    _crafting_fee_pct: f64,
) -> Result<RecommendDecision, MarrowError> {
    eprintln!("[marrow] Analyzing trade recommendation for {}, city={}", item_id, city);
    
    // PRE-SEED HISTORY FOR ALL CITIES AND QUALITIES
    let _ = fetch_history_bulk_global(pool, client, server.base_url(), item_id, days).await;

    let mut best_decision = RecommendDecision::default();
    
    for q in 1..=5 {
        let output = match get_price(pool, client, server, item_id, city, q, 300).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[marrow-trace] Q{} skipped: get_price failed: {}", q, e);
                continue;
            }
        };
        let output_price = match output.sell_price_min { 
            Some(p) if p > 0 => p, 
            _ => {
                eprintln!("[marrow-trace] Q{} skipped: No active sell orders (price={:?})", q, output.sell_price_min);
                continue;
            }
        };

        let stale_data = if let Some(date_str) = &output.sell_price_min_date {
            if let Some(ts) = iso_to_unix(date_str) {
                chrono::Utc::now().timestamp() - ts > 86400
            } else { true }
        } else { true };

        let city_futures: Vec<_> = ALL_CITIES
            .iter()
            .map(|&c| get_price(pool, client, server, item_id, c, q, 300))
            .collect();
        let city_results = futures_util::future::join_all(city_futures).await;

        let mut city_prices: Vec<CityPriceSummary> = ALL_CITIES
            .iter()
            .map(|s| s.to_string())
            .zip(city_results.into_iter())
            .map(|(c, res)| match res {
                Ok(p) => CityPriceSummary { city: c, sell_price_min: p.sell_price_min, buy_price_max: p.buy_price_max },
                Err(_) => CityPriceSummary { city: c, sell_price_min: None, buy_price_max: None },
            })
            .collect();

        city_prices.sort_by(|a, b| b.sell_price_min.unwrap_or(0).cmp(&a.sell_price_min.unwrap_or(0)));
        let best_sell_city: Option<String> = city_prices.iter().find(|c| c.sell_price_min.is_some()).map(|c| c.city.clone());
        let transport_warning = if let Some(ref best) = best_sell_city {
            if best != city { Some(format!("Requires transport to {}", best)) } else { None }
        } else { None };

        let history_points: Vec<HistoryPoint> = match get_history(pool, client, server, item_id, city, q, days).await {
            Ok(h) => h.points,
            Err(e) => {
                eprintln!("[marrow-trace] Q{} skipped: get_history failed: {}", q, e);
                Vec::new()
            }
        };

        if history_points.is_empty() { 
            eprintln!("[marrow-trace] Q{} skipped: No history points found for duration", q);
            continue; 
        }

        eprintln!("[marrow-trace] Q{} processing: price={}, points={}", q, output_price, history_points.len());

        let prices_vec: Vec<f64> = history_points.iter().map(|p| p.avg_price as f64).collect();
        let short_ema = compute_ema(&prices_vec, 3).map(round2);
        let long_ema = compute_ema(&prices_vec, 10).map(round2); 
        let mut bullish = match (short_ema, long_ema) { (Some(s), Some(l)) => Some(s > l), _ => None };
        
        // FALLBACK: If sparse data (e.g. East server), use simple linear trend
        if bullish.is_none() && prices_vec.len() >= 2 {
            let first = prices_vec[0];
            let last = *prices_vec.last().unwrap();
            if last > first * 1.05 { bullish = Some(true); }
            else if last < first * 0.95 { bullish = Some(false); }
        }

        let price_volatility_pct = compute_volatility_pct(&prices_vec).map(round2);

        let historical_avg = if prices_vec.is_empty() { None } else {
            let sum: f64 = prices_vec.iter().sum();
            Some((sum / prices_vec.len() as f64) as i64)
        };
        
        let price_diff_pct = historical_avg.map(|avg| {
            if avg > 0 {
                (output_price as f64 - avg as f64) / avg as f64 * 100.0
            } else {
                0.0
            }
        });

        let history_count = history_points.len();
        let history_stale = history_points.last().map(|p| {
            if let Some(ts) = iso_to_unix(&p.timestamp) {
                Utc::now().timestamp() - ts > 172800 // > 48h since last point
            } else { false }
        }).unwrap_or(true);

        let avg_daily_volume = if history_points.is_empty() { None } else {
            let mean = history_points.iter().map(|p| p.item_count as f64).sum::<f64>() / history_points.len() as f64;
            Some(round2(mean))
        };
        let min_daily_volume = history_points.iter().map(|p| p.item_count).min();

        let suggested_qty = avg_daily_volume.map(|v| ((v * 0.2).round() as i64).max(1)).unwrap_or(1);
        let estimated_days_to_sell = avg_daily_volume.map(|v| if v > 0.0 { (suggested_qty as f64 / v).ceil() as i64 } else { 999 });

        let bullish_comp = match bullish { Some(true) => 1.0, Some(false) => 0.0, None => 0.5 };
        let vol_comp = match price_volatility_pct { Some(v) => ((30.0 - v.min(30.0)) / 30.0).max(0.0), None => 0.5 };
        let volm_comp = match avg_daily_volume { Some(v) => (v.min(100.0) / 100.0).max(0.0), None => 0.5 };

        let w_bull = 0.30; let w_volatility = 0.20; let w_volume = 0.50;
        let weighted_sum = w_bull * bullish_comp + w_volatility * vol_comp + w_volume * volm_comp;
        let mut confidence = round2(weighted_sum.max(0.0).min(1.0));

        let mut reasons: Vec<String> = Vec::new();
        if stale_data {
            confidence = (confidence * 0.5).min(0.3);
        }
        if let Some(days) = estimated_days_to_sell {
            if days > 7 {
                confidence = (confidence * 0.7).min(0.4);
                reasons.push(if days > 900 { "No market liquidity".to_string() } else { format!("Takes ~{} days to sell batch", days) });
            }
        }
        if transport_warning.is_some() { reasons.push("Involves inter-city transport risk".to_string()); }
        if bullish == Some(false) { reasons.push("Price trending down".to_string()); }
        if price_volatility_pct.map(|v| v > 30.0).unwrap_or(false) { reasons.push("High price volatility".to_string()); }
        if avg_daily_volume.map(|v| v < 5.0).unwrap_or(false) { reasons.push("Very low market volume".to_string()); }

        if let Some(diff) = price_diff_pct {
            if diff > 200.0 { // > 3x avg
                confidence = (confidence * 0.4).min(0.2);
                reasons.push(format!("Extreme price outlier (+{:.0}%) vs historical average", diff));
            } else if diff < -50.0 {
                reasons.push(format!("Price is significantly low (-{:.0}%) vs average", diff.abs()));
            }
        }

        let recommended = confidence >= 0.55;
        if recommended { reasons.push("Good trade opportunity".to_string()); } 
        else { reasons.push("Marginal — weak signals".to_string()); }
        
        let reason = if reasons.is_empty() {
            if recommended { "Strong market dynamics".to_string() } else { "Insufficient data".to_string() }
        } else { reasons.join(". ") };

        let decision = RecommendDecision {
            recommended, reason, confidence, quality: q, output_price, suggested_qty, estimated_days_to_sell,
            stale_data, short_ema, long_ema, bullish, price_volatility_pct, avg_daily_volume, min_daily_volume,
            historical_avg, price_diff_pct,
            history_count, history_stale,
            city_prices, best_sell_city, transport_warning,
        };

        if decision.confidence > best_decision.confidence || (decision.confidence == best_decision.confidence && decision.output_price > best_decision.output_price) {
            best_decision = decision;
        }
    }

    if best_decision.output_price == 0 {
        best_decision.reason = "No active sell orders or data for any quality".to_string();
    }

    Ok(best_decision)
}
