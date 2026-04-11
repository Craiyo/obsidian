CREATE TABLE IF NOT EXISTS marrow_prices (
    uniquename          TEXT    NOT NULL,
    city                TEXT    NOT NULL,
    quality             INTEGER NOT NULL,
    sell_price_min      INTEGER,
    sell_price_max      INTEGER,
    buy_price_min       INTEGER,
    buy_price_max       INTEGER,
    sell_price_min_date TEXT,
    buy_price_max_date  TEXT,
    fetched_at          INTEGER NOT NULL,
    ttl_expires_at      INTEGER NOT NULL,
    PRIMARY KEY (uniquename, city, quality)
);

CREATE TABLE IF NOT EXISTS marrow_history (
    uniquename   TEXT    NOT NULL,
    city         TEXT    NOT NULL,
    quality      INTEGER NOT NULL,
    time_scale   INTEGER NOT NULL,
    data_json    TEXT    NOT NULL,
    fetched_at   INTEGER NOT NULL,
    PRIMARY KEY  (uniquename, city, quality, time_scale)
);

CREATE TABLE IF NOT EXISTS marrow_favourites (
    uniquename  TEXT    PRIMARY KEY,
    added_at    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS marrow_gold (
    id          INTEGER PRIMARY KEY CHECK(id = 1),
    price       INTEGER NOT NULL,
    timestamp   TEXT    NOT NULL,
    fetched_at  INTEGER NOT NULL
);
