CREATE TABLE IF NOT EXISTS alchemy_sessions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    account_name    TEXT    NOT NULL,
    city            TEXT    NOT NULL,
    use_focus       INTEGER NOT NULL DEFAULT 0,
    rrr             REAL    NOT NULL,
    created_at      INTEGER NOT NULL,
    sent_to_marrow  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS alchemy_session_items (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id      INTEGER NOT NULL REFERENCES alchemy_sessions(id) ON DELETE CASCADE,
    uniquename      TEXT    NOT NULL,
    display_name    TEXT    NOT NULL,
    quantity_out    INTEGER NOT NULL,
    craft_amount    INTEGER NOT NULL,
    runs_needed     INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS alchemy_session_materials (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id      INTEGER NOT NULL REFERENCES alchemy_sessions(id) ON DELETE CASCADE,
    uniquename      TEXT    NOT NULL,
    display_name    TEXT    NOT NULL,
    quantity_needed INTEGER NOT NULL,
    unit_price      INTEGER,
    total_cost      REAL,
    UNIQUE(session_id, uniquename)
);

CREATE INDEX IF NOT EXISTS idx_alchemy_sessions_created
    ON alchemy_sessions(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_alchemy_session_items_session
    ON alchemy_session_items(session_id);

CREATE INDEX IF NOT EXISTS idx_alchemy_session_materials_session
    ON alchemy_session_materials(session_id);
