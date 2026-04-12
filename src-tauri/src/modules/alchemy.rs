use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AlchemyAnalysis {
    pub item_id: String,
    pub display_name: String,
    pub best_city: String,
    pub rrr: f64,
    pub yield_multiplier: f64,
    pub materials: Vec<MaterialYield>,
    pub craft_amount: i64,
    pub batch_size: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MaterialYield {
    pub uniquename: String,
    pub display_name: String,
    pub total_required: i64,
    pub net_consumed: f64,
    pub return_amount: f64,
}

pub fn get_rrr(has_bonus: bool, use_focus: bool, daily_bonus: bool) -> f64 {
    // Standard Albion Royal City RRR values
    (match (has_bonus, use_focus) {
        (true, true) => 0.471,
        (true, false) => 0.152,
        (false, true) => 0.435,
        (false, false) => 0.0,
    }) + if daily_bonus { 0.10 } else { 0.0 }
}

pub fn get_hideout_rrr(power_level: i64, _quality: i64, use_focus: bool) -> f64 {
    // Simplified Hideout RRR logic
    // Base HO RRR is usually around 20-25% without focus
    let base = 0.25 + (power_level as f64 * 0.01);
    if use_focus {
        base + 0.25 // Roughly additive for focus in HO
    } else {
        base
    }
}

pub fn get_best_city(shop_category: &str, sub_category: &str) -> &'static str {
    let cat = shop_category.to_lowercase();
    let sub = sub_category.to_lowercase();
    match cat.as_str() {
        "consumables" => {
            if sub.contains("potion") { "Brecilien" }
            else if sub.contains("food") { "Caerleon" }
            else { "Caerleon" }
        }
        "weapons" => {
            if sub.contains("crossbow") { "Bridgewatch" }
            else if ["sword", "bow", "arcanestaff"].iter().any(|&s| sub.contains(s)) { "Lymhurst" }
            else if sub.contains("axe") { "Martlock" }
            else if ["dagger", "cursestaff"].iter().any(|&s| sub.contains(s)) { "Bridgewatch" }
            else if ["mace", "naturestaff", "firestaff"].iter().any(|&s| sub.contains(s)) { "Thetford" }
            else if ["hammer", "spear", "holystaff", "froststaff"].iter().any(|&s| sub.contains(s)) { "FortSterling" }
            else { "Caerleon" }
        }
        "offhands" => "Martlock",
        "armors" => {
            if sub.contains("plate_armor") { "FortSterling" }
            else if sub.contains("leather_armor") { "Thetford" }
            else if sub.contains("cloth_armor") { "Martlock" }
            else { "Caerleon" }
        }
        "head" => {
            if sub.contains("leather_helmet") { "Lymhurst" }
            else if sub.contains("plate_helmet") { "Bridgewatch" }
            else if sub.contains("cloth_helmet") { "Thetford" }
            else { "Caerleon" }
        }
        "shoes" => {
            if sub.contains("leather_shoes") { "Lymhurst" }
            else if sub.contains("plate_shoes") { "Martlock" }
            else if sub.contains("cloth_shoes") { "FortSterling" }
            else { "Caerleon" }
        }
        _ => "Caerleon",
    }
}

pub async fn analyze_yield(
    pool: &SqlitePool,
    item_id: &str,
    batch_size: i64,
    use_focus: bool,
    daily_bonus: bool,
    is_hideout: bool,
    hideout_power: i64,
) -> Result<AlchemyAnalysis, Box<dyn std::error::Error>> {
    let row = sqlx::query(
        "SELECT uniquename, display_name, shopcategory, shopsubcategory1, craft_amount, craft_resources FROM items WHERE uniquename = ?"
    )
    .bind(item_id).fetch_one(pool).await?;

    let display_name: String = row.get("display_name");
    let category: String = row.get::<Option<String>, _>("shopcategory").unwrap_or_default();
    let subcategory: String = row.get::<Option<String>, _>("shopsubcategory1").unwrap_or_default();
    let craft_amount: i64 = row.get("craft_amount");
    let resources_json: String = row.get::<Option<String>, _>("craft_resources").unwrap_or_else(|| "[]".to_string());
    
    let best_city = get_best_city(&category, &subcategory);
    
    let rrr = if is_hideout {
        get_hideout_rrr(hideout_power, 6, use_focus)
    } else {
        get_rrr(true, use_focus, daily_bonus) // Assume crafting in bonus city
    };

    // Yield Multiplier: How much more do we get? 
    // If RRR is 47.1%, we get 1 / (1 - 0.471) = 1.89x effective resources
    let yield_multiplier = 1.0 / (1.0 - rrr);

    let resources: Vec<crate::db::item_map::CraftResource> = serde_json::from_str(&resources_json)?;
    let mut materials = Vec::new();

    for res in resources {
        let total_req = res.count * batch_size;
        let return_amt = total_req as f64 * rrr;
        let net_consumed = total_req as f64 - return_amt;

        materials.push(MaterialYield {
            uniquename: res.uniquename,
            display_name: String::new(), // UI can resolve this or we can join
            total_required: total_req,
            net_consumed,
            return_amount: return_amt,
        });
    }

    Ok(AlchemyAnalysis {
        item_id: item_id.to_string(),
        display_name,
        best_city: best_city.to_string(),
        rrr,
        yield_multiplier,
        materials,
        craft_amount,
        batch_size,
    })
}
