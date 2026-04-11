#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod db;
mod modules;
mod settings;
mod ws;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle();
            let pool = tauri::async_runtime::block_on(db::init_pool(&app_handle))
                .map_err(|e| Box::<dyn std::error::Error>::from(e))?;

            // Seed items table from assets/items.json if it's empty
            tauri::async_runtime::block_on(db::seed_items_if_empty(&pool, &app_handle))
                .map_err(|e| Box::<dyn std::error::Error>::from(e))?;

            let settings_path = settings::settings_path(&app_handle)
                .map_err(|e| Box::<dyn std::error::Error>::from(e))?;

            let state = api::AppState::new(pool, settings_path);
            tauri::async_runtime::spawn(async move {
                if let Err(err) = api::serve(state).await {
                    eprintln!("api server stopped: {err}");
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
