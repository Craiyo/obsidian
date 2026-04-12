#[derive(Debug, serde::Serialize)]
pub struct SearchResult {
    pub uniquename: String,
    pub display_name: String,
    pub tier: i64,
    pub shopcategory: String,
    pub shopsubcategory1: String,
}

pub async fn search(pool: &sqlx::SqlitePool, query: &str) -> Result<Vec<SearchResult>, Box<dyn std::error::Error>> {
    let q = query.trim();
    if q.is_empty() { return Ok(Vec::new()); }
    let rows = sqlx::query(
        "SELECT uniquename, display_name, tier, shopcategory, shopsubcategory1 FROM items WHERE display_name LIKE '%' || ?1 || '%' OR uniquename LIKE '%' || ?1 || '%' ORDER BY tier, display_name LIMIT 50"
    )
    .bind(q).fetch_all(pool).await?;

    Ok(rows.into_iter().map(|r| {
        use sqlx::Row;
        SearchResult {
            uniquename: r.get("uniquename"),
            display_name: r.get::<Option<String>, _>("display_name").unwrap_or_default(),
            tier: r.get::<Option<i64>, _>("tier").unwrap_or(0),
            shopcategory: r.get::<Option<String>, _>("shopcategory").unwrap_or_default(),
            shopsubcategory1: r.get::<Option<String>, _>("shopsubcategory1").unwrap_or_default(),
        }
    }).collect())
}

pub async fn get_items_by_ids(pool: &sqlx::SqlitePool, ids: &[String]) -> Result<Vec<SearchResult>, Box<dyn std::error::Error>> {
    if ids.is_empty() { return Ok(Vec::new()); }
    let placeholders = vec!["?"; ids.len()].join(", ");
    let query_str = format!("SELECT uniquename, display_name, tier, shopcategory, shopsubcategory1 FROM items WHERE uniquename IN ({})", placeholders);
    let mut query = sqlx::query(&query_str);
    for id in ids { query = query.bind(id); }
    let rows = query.fetch_all(pool).await?;

    Ok(rows.into_iter().map(|r| {
        use sqlx::Row;
        SearchResult {
            uniquename: r.get("uniquename"),
            display_name: r.get::<Option<String>, _>("display_name").unwrap_or_default(),
            tier: r.get::<Option<i64>, _>("tier").unwrap_or(0),
            shopcategory: r.get::<Option<String>, _>("shopcategory").unwrap_or_default(),
            shopsubcategory1: r.get::<Option<String>, _>("shopsubcategory1").unwrap_or_default(),
        }
    }).collect())
}
