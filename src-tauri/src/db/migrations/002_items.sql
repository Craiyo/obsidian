CREATE TABLE IF NOT EXISTS items (
  uniquename          TEXT PRIMARY KEY,
  display_name        TEXT,
  item_type           TEXT NOT NULL,
  tier                INTEGER NOT NULL DEFAULT 0,
  enchantment_level   INTEGER NOT NULL DEFAULT 0,
  shopcategory        TEXT,
  shopsubcategory1    TEXT,
  shopsubcategory2    TEXT,
  resource_type       TEXT,
  show_in_marketplace INTEGER NOT NULL DEFAULT 0,
  craftable           INTEGER NOT NULL DEFAULT 0,
  craft_silver        REAL,
  craft_time          REAL,
  craft_focus         INTEGER,
  craft_amount        INTEGER NOT NULL DEFAULT 1,
  craft_resources     TEXT,
  upgrade_resource    TEXT,
  upgrade_count       INTEGER
);

CREATE INDEX IF NOT EXISTS idx_items_item_type ON items(item_type);
CREATE INDEX IF NOT EXISTS idx_items_shopcategory ON items(shopcategory);
CREATE INDEX IF NOT EXISTS idx_items_tier ON items(tier);
CREATE INDEX IF NOT EXISTS idx_items_craftable ON items(craftable);
CREATE INDEX IF NOT EXISTS idx_items_resource_type ON items(resource_type);
