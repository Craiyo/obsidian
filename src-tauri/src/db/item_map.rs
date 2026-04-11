use serde::Serialize;
use serde_json::Value;
use std::{fs, path::Path};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize)]
pub struct CraftResource {
    pub uniquename: String,
    pub count: i64,
    pub enchantment_level: i64,
}

#[derive(Debug, Clone)]
pub struct ItemRow {
    pub uniquename: String,
    pub display_name: String,
    pub item_type: String,
    pub tier: i64,
    pub enchantment_level: i64,
    pub shopcategory: Option<String>,
    pub shopsubcategory1: Option<String>,
    pub shopsubcategory2: Option<String>,
    pub resource_type: Option<String>,
    pub show_in_marketplace: bool,
    pub craftable: bool,
    pub craft_silver: Option<f64>,
    pub craft_time: Option<f64>,
    pub craft_focus: Option<i64>,
    pub craft_amount: i64,
    pub craft_resources: Option<String>,
    pub upgrade_resource: Option<String>,
    pub upgrade_count: Option<i64>,
}

#[derive(Debug, Clone, Default)]
struct CraftingData {
    craftable: bool,
    craft_silver: Option<f64>,
    craft_time: Option<f64>,
    craft_focus: Option<i64>,
    craft_amount: i64,
    craft_resources: Option<String>,
}

/// Parse the items.json file and return rows for insertion.
pub fn parse_items_json(path: &Path) -> Result<Vec<ItemRow>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let root: Value = serde_json::from_str(&content)?;

    let items_obj = root
        .get("items")
        .and_then(Value::as_object)
        .ok_or_else(|| "missing top-level items object")?;

    let mut out = Vec::new();

    for (item_type, node) in items_obj {
        if item_type.starts_with('@') {
            continue;
        }

        for item in as_vec(node) {
            let item_obj = match item.as_object() {
                Some(o) => o,
                None => continue,
            };

            let base_uniquename = match get_str(item_obj, "@uniquename") {
                Some(s) => s,
                None => continue,
            };

            let tier = get_i64(item_obj, "@tier").unwrap_or(0);
            let show_in_marketplace = get_bool(item_obj, "@showinmarketplace").unwrap_or(false);
            let craft = parse_crafting(item_obj.get("craftingrequirements"));

            let base = ItemRow {
                uniquename: base_uniquename.to_string(),
                display_name: derive_display_name(base_uniquename),
                item_type: item_type.to_string(),
                tier,
                enchantment_level: 0,
                shopcategory: get_str(item_obj, "@shopcategory").map(ToOwned::to_owned),
                shopsubcategory1: get_str(item_obj, "@shopsubcategory1").map(ToOwned::to_owned),
                shopsubcategory2: get_str(item_obj, "@shopsubcategory2").map(ToOwned::to_owned),
                resource_type: get_str(item_obj, "@resourcetype").map(ToOwned::to_owned),
                show_in_marketplace,
                craftable: craft.craftable,
                craft_silver: craft.craft_silver,
                craft_time: craft.craft_time,
                craft_focus: craft.craft_focus,
                craft_amount: craft.craft_amount,
                craft_resources: craft.craft_resources,
                upgrade_resource: None,
                upgrade_count: None,
            };
            out.push(base.clone());

            // enchanted variants
            if let Some(enchantments) = item_obj.get("enchantments") {
                let ench_rows = parse_enchantments(enchantments);
                for ench in ench_rows {
                    let mut row = base.clone();
                    row.enchantment_level = ench.enchantment_level;
                    row.uniquename = format!("{}@{}", base.uniquename, ench.enchantment_level);
                    row.display_name = derive_display_name(&row.uniquename);

                    if ench.craft.craftable {
                        row.craftable = true;
                        row.craft_silver = ench.craft.craft_silver;
                        row.craft_time = ench.craft.craft_time;
                        row.craft_focus = ench.craft.craft_focus;
                        row.craft_amount = ench.craft.craft_amount;
                        row.craft_resources = ench.craft.craft_resources.clone();
                    }

                    row.upgrade_resource = ench.upgrade_resource;
                    row.upgrade_count = ench.upgrade_count;
                    out.push(row);
                }
            }
        }
    }

    Ok(out)
}

#[derive(Debug, Clone, Default)]
struct EnchantmentRow {
    enchantment_level: i64,
    craft: CraftingData,
    upgrade_resource: Option<String>,
    upgrade_count: Option<i64>,
}

fn parse_enchantments(enchantments_node: &Value) -> Vec<EnchantmentRow> {
    let Some(obj) = enchantments_node.as_object() else {
        return Vec::new();
    };

    let Some(enchantment_node) = obj.get("enchantment") else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for ench in as_vec(enchantment_node) {
        let Some(ench_obj) = ench.as_object() else {
            continue;
        };

        let enchantment_level = get_i64(ench_obj, "@enchantmentlevel").unwrap_or(0);
        let craft = parse_crafting(ench_obj.get("craftingrequirements"));
        let (upgrade_resource, upgrade_count) = parse_upgrade_resource(ench_obj.get("upgraderequirements"));

        out.push(EnchantmentRow {
            enchantment_level,
            craft,
            upgrade_resource,
            upgrade_count,
        });
    }

    out
}

fn parse_upgrade_resource(node: Option<&Value>) -> (Option<String>, Option<i64>) {
    let Some(obj) = node.and_then(Value::as_object) else {
        return (None, None);
    };

    let Some(upgrade_node) = obj.get("upgraderesource") else {
        return (None, None);
    };

    let Some(first) = as_vec(upgrade_node).into_iter().next() else {
        return (None, None);
    };

    let Some(first_obj) = first.as_object() else {
        return (None, None);
    };

    (
        get_str(first_obj, "@uniquename").map(ToOwned::to_owned),
        get_i64(first_obj, "@count"),
    )
}

fn parse_crafting(node: Option<&Value>) -> CraftingData {
    let Some(crafting_node) = node else {
        return CraftingData::default();
    };

    // craftingrequirements can be single object or list; we intentionally use index 0.
    let Some(recipe) = as_vec(crafting_node).into_iter().next() else {
        return CraftingData::default();
    };
    let Some(recipe_obj) = recipe.as_object() else {
        return CraftingData::default();
    };

    let resources = parse_craft_resources(recipe_obj.get("craftresource"));
    let craft_resources = if resources.is_empty() {
        None
    } else {
        serde_json::to_string(&resources).ok()
    };

    CraftingData {
        craftable: true,
        craft_silver: get_f64(recipe_obj, "@silver"),
        craft_time: get_f64(recipe_obj, "@time"),
        craft_focus: get_i64(recipe_obj, "@craftingfocus"),
        craft_amount: get_i64(recipe_obj, "@amountcrafted").unwrap_or(1),
        craft_resources,
    }
}

fn parse_craft_resources(node: Option<&Value>) -> Vec<CraftResource> {
    let Some(resource_node) = node else {
        return Vec::new();
    };

    // craftresource can be a single object or list; normalize to Vec.
    as_vec(resource_node)
        .into_iter()
        .filter_map(|entry| {
            let obj = entry.as_object()?;
            let uniquename = get_str(obj, "@uniquename")?.to_string();
            let count = get_i64(obj, "@count").unwrap_or(0);
            let enchantment_level = get_i64(obj, "@enchantmentlevel").unwrap_or(0);
            Some(CraftResource {
                uniquename,
                count,
                enchantment_level,
            })
        })
        .collect()
}

fn as_vec(value: &Value) -> Vec<&Value> {
    match value {
        Value::Array(arr) => arr.iter().collect(),
        _ => vec![value],
    }
}

fn get_str<'a>(obj: &'a serde_json::Map<String, Value>, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(Value::as_str)
}

fn get_i64(obj: &serde_json::Map<String, Value>, key: &str) -> Option<i64> {
    let raw = obj.get(key)?;
    match raw {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                s.parse::<i64>().ok()
            }
        }
        _ => None,
    }
}

fn get_f64(obj: &serde_json::Map<String, Value>, key: &str) -> Option<f64> {
    let raw = obj.get(key)?;
    match raw {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                s.parse::<f64>().ok()
            }
        }
        _ => None,
    }
}

fn get_bool(obj: &serde_json::Map<String, Value>, key: &str) -> Option<bool> {
    let raw = obj.get(key)?;
    match raw {
        Value::Bool(v) => Some(*v),
        Value::Number(n) => n.as_i64().map(|v| v != 0),
        Value::String(s) => {
            let lower = s.trim().to_ascii_lowercase();
            match lower.as_str() {
                "true" | "1" | "yes" => Some(true),
                "false" | "0" | "no" => Some(false),
                _ => None,
            }
        }
        _ => None,
    }
}

fn derive_display_name(uniquename: &str) -> String {
    let base = strip_tier_prefix(uniquename);
    let base = base.replace('@', " ");

    base.split('_')
        .filter(|s| !s.is_empty())
        .map(title_case_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_tier_prefix(input: &str) -> &str {
    if !input.starts_with('T') {
        return input;
    }
    let bytes = input.as_bytes();
    let mut i = 1usize;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 1 && i < bytes.len() && bytes[i] == b'_' {
        &input[i + 1..]
    } else {
        input
    }
}

fn title_case_token(token: &str) -> String {
    if token.is_empty() {
        return String::new();
    }

    let lower = token.to_ascii_lowercase();
    let mut chars = lower.chars();
    let first = chars.next().unwrap().to_ascii_uppercase();
    let mut out = String::with_capacity(token.len());
    out.push(first);
    out.extend(chars);
    out
}

fn bool_to_i64(v: bool) -> i64 {
    if v { 1 } else { 0 }
}

pub async fn insert_items(pool: &SqlitePool, rows: Vec<ItemRow>) -> Result<(), sqlx::Error> {
    if rows.is_empty() {
        return Ok(());
    }

    // Batch size chosen to avoid SQLite variable limits
    let chunk_size = 500usize;
    let cols = [
        "uniquename",
        "display_name",
        "item_type",
        "tier",
        "enchantment_level",
        "shopcategory",
        "shopsubcategory1",
        "shopsubcategory2",
        "resource_type",
        "show_in_marketplace",
        "craftable",
        "craft_silver",
        "craft_time",
        "craft_focus",
        "craft_amount",
        "craft_resources",
        "upgrade_resource",
        "upgrade_count",
    ];

    let col_list = cols.join(", ");

    let mut idx = 0usize;
    while idx < rows.len() {
        let end = std::cmp::min(idx + chunk_size, rows.len());
        let batch = &rows[idx..end];
        let mut placeholders = Vec::with_capacity(batch.len());
        for _ in batch.iter() {
            placeholders.push(format!("({})", vec!["?"; cols.len()].join(", ")));
        }
        let sql = format!(
            "INSERT OR IGNORE INTO items ({}) VALUES {}",
            col_list,
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);

        for r in batch.iter() {
            query = query
                .bind(&r.uniquename)
                .bind(&r.display_name)
                .bind(&r.item_type)
                .bind(r.tier)
                .bind(r.enchantment_level)
                .bind(&r.shopcategory)
                .bind(&r.shopsubcategory1)
                .bind(&r.shopsubcategory2)
                .bind(&r.resource_type)
                .bind(bool_to_i64(r.show_in_marketplace))
                .bind(bool_to_i64(r.craftable))
                .bind(r.craft_silver)
                .bind(r.craft_time)
                .bind(r.craft_focus)
                .bind(r.craft_amount)
                .bind(&r.craft_resources)
                .bind(&r.upgrade_resource)
                .bind(r.upgrade_count);
        }

        query.execute(pool).await?;

        idx = end;
    }

    Ok(())
}
