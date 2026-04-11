PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS app_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS schema_migrations (
  version TEXT PRIMARY KEY,
  applied_at INTEGER NOT NULL
);

-- Seance
CREATE TABLE IF NOT EXISTS seance_sessions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_at INTEGER NOT NULL,
  party_size INTEGER NOT NULL,
  total_loot_value INTEGER NOT NULL,
  split_type TEXT NOT NULL,
  notes TEXT
);

CREATE TABLE IF NOT EXISTS seance_session_shares (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id INTEGER NOT NULL,
  player_name TEXT NOT NULL,
  weight REAL NOT NULL DEFAULT 1.0,
  share_value INTEGER NOT NULL,
  FOREIGN KEY (session_id) REFERENCES seance_sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS seance_wallets (
  player_name TEXT PRIMARY KEY,
  balance INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS seance_withdrawals (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  player_name TEXT NOT NULL,
  amount INTEGER NOT NULL,
  reason TEXT NOT NULL,
  notes TEXT,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS seance_regear_transactions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  amount INTEGER NOT NULL,
  reason TEXT NOT NULL,
  notes TEXT,
  created_at INTEGER NOT NULL
);

-- Marrow
CREATE TABLE IF NOT EXISTS marrow_prices (
  uniquename TEXT NOT NULL,
  city TEXT NOT NULL,
  quality INTEGER NOT NULL DEFAULT 1,
  sell_price_min INTEGER,
  sell_price_max INTEGER,
  buy_price_min INTEGER,
  buy_price_max INTEGER,
  sell_price_min_date TEXT,
  buy_price_max_date TEXT,
  fetched_at INTEGER NOT NULL,
  ttl_expires_at INTEGER NOT NULL,
  PRIMARY KEY (uniquename, city, quality)
);

CREATE TABLE IF NOT EXISTS marrow_history (
  uniquename TEXT NOT NULL,
  city TEXT NOT NULL,
  quality INTEGER NOT NULL DEFAULT 1,
  time_scale INTEGER NOT NULL,
  data_json TEXT NOT NULL,
  fetched_at INTEGER NOT NULL,
  PRIMARY KEY (uniquename, city, quality, time_scale)
);

CREATE TABLE IF NOT EXISTS marrow_favourites (
  uniquename TEXT PRIMARY KEY,
  added_at INTEGER NOT NULL
);

-- Alchemy
CREATE TABLE IF NOT EXISTS alchemy_recipes (
  item_id TEXT PRIMARY KEY,
  recipe_json TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS alchemy_scenarios (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  item_id TEXT NOT NULL,
  city TEXT NOT NULL,
  return_rate REAL NOT NULL,
  crafting_fee REAL NOT NULL,
  bonus_pct REAL NOT NULL,
  profit INTEGER NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS alchemy_scenario_materials (
  scenario_id INTEGER NOT NULL,
  material_id TEXT NOT NULL,
  quantity REAL NOT NULL,
  unit_cost INTEGER NOT NULL,
  PRIMARY KEY (scenario_id, material_id),
  FOREIGN KEY (scenario_id) REFERENCES alchemy_scenarios(id) ON DELETE CASCADE
);

-- Wraith
CREATE TABLE IF NOT EXISTS wraith_nodes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  zone_id TEXT NOT NULL,
  node_type TEXT NOT NULL,
  tier INTEGER NOT NULL,
  x REAL NOT NULL,
  y REAL NOT NULL,
  spawned_at INTEGER NOT NULL,
  despawned_at INTEGER
);

-- Hemorrhage
CREATE TABLE IF NOT EXISTS hemorrhage_sessions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  started_at INTEGER NOT NULL,
  ended_at INTEGER,
  zone_id TEXT,
  notes TEXT
);

CREATE TABLE IF NOT EXISTS hemorrhage_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id INTEGER NOT NULL,
  timestamp INTEGER NOT NULL,
  source_player TEXT,
  target_player TEXT,
  ability TEXT,
  amount INTEGER NOT NULL,
  event_type TEXT NOT NULL,
  is_crit INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY (session_id) REFERENCES hemorrhage_sessions(id) ON DELETE CASCADE
);

-- Effigy
CREATE TABLE IF NOT EXISTS effigy_zone_state (
  zone_id TEXT PRIMARY KEY,
  guild_name TEXT,
  alliance_name TEXT,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS effigy_zones (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  zone_id TEXT NOT NULL,
  guild_name TEXT,
  alliance_name TEXT,
  changed_at INTEGER NOT NULL
);

-- Hex
CREATE TABLE IF NOT EXISTS hex_gate_state (
  gate_id TEXT PRIMARY KEY,
  zone_id TEXT NOT NULL,
  tier INTEGER NOT NULL,
  dungeon_type TEXT NOT NULL,
  remaining_seconds INTEGER,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS hex_gates (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  gate_id TEXT,
  zone_id TEXT NOT NULL,
  tier INTEGER NOT NULL,
  dungeon_type TEXT NOT NULL,
  remaining_seconds INTEGER,
  detected_at INTEGER NOT NULL,
  expired_at INTEGER
);

-- Specter
CREATE TABLE IF NOT EXISTS specter_sell_list (
  item_id TEXT PRIMARY KEY,
  item_name TEXT,
  target_price INTEGER NOT NULL,
  quantity INTEGER NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS specter_orders (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  item_id TEXT NOT NULL,
  item_name TEXT,
  city TEXT,
  target_price INTEGER NOT NULL,
  quantity INTEGER NOT NULL,
  status TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  completed_at INTEGER,
  notes TEXT
);

-- Wail
CREATE TABLE IF NOT EXISTS wail_catches (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  item_name TEXT,
  zone_id TEXT,
  caught_at INTEGER NOT NULL
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_seance_session_shares_session
  ON seance_session_shares(session_id);

CREATE INDEX IF NOT EXISTS idx_marrow_prices_item
  ON marrow_prices(item_id);

CREATE INDEX IF NOT EXISTS idx_marrow_history_item
  ON marrow_history(item_id);

CREATE INDEX IF NOT EXISTS idx_alchemy_scenarios_item
  ON alchemy_scenarios(item_id);

CREATE INDEX IF NOT EXISTS idx_wraith_nodes_zone
  ON wraith_nodes(zone_id, spawned_at);

CREATE INDEX IF NOT EXISTS idx_hemorrhage_events_session
  ON hemorrhage_events(session_id, timestamp);

CREATE INDEX IF NOT EXISTS idx_effigy_zones_zone
  ON effigy_zones(zone_id, changed_at);

CREATE INDEX IF NOT EXISTS idx_hex_gates_zone
  ON hex_gates(zone_id, detected_at);

CREATE INDEX IF NOT EXISTS idx_specter_orders_status
  ON specter_orders(status, created_at);

CREATE INDEX IF NOT EXISTS idx_wail_catches_time
  ON wail_catches(caught_at);
