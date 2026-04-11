# Obsidian (Albion Online Companion)

Desktop companion app for Albion Online, built with:
- **Backend:** Rust + Tauri + Axum + SQLx (SQLite)
- **Frontend:** HTML/CSS/Vanilla JS (Tauri WebView)

## Project structure

```text
obsidian/
├─ assets/                # Static data (items, mappings, binaries)
├─ src-tauri/             # Rust backend + Tauri app
│  ├─ src/
│  │  ├─ api/             # HTTP route handlers
│  │  ├─ modules/         # Domain logic (marrow, alchemy, etc.)
│  │  ├─ db/              # DB pool + migrations
│  │  └─ main.rs
│  └─ src/db/migrations/  # SQL migrations
└─ ui/                    # Frontend pages and scripts
   ├─ pages/
   ├─ styles/
   ├─ app.js
   └─ marrow.js
```

## Prerequisites

- Rust toolchain (stable)
- Cargo
- Linux desktop dependencies required by Tauri/WebKitGTK

## Run locally

From `src-tauri/`:

```bash
cargo tauri dev
```

The local API server binds to:

```text
http://127.0.0.1:38991
```

## Database and migrations

- SQLite database path is resolved via Tauri local data directory.
- Migrations run automatically at startup (`sqlx::migrate!`).
- **Important:** never edit an already-applied migration file in place.
  - If schema changes are needed, create a new migration (e.g. `004_*.sql`).

## Current Marrow notes

- Marrow endpoints are under `/api/v1/marrow`.
- Upstream Albion data calls are server-backed and cached in SQLite.
- API failure logs are centralized and include request/status details.
- Gold/history upstream parsing includes defensive diagnostics for troubleshooting.
