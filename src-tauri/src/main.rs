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

            let settings_path = settings::settings_path(&app_handle)
                .map_err(|e| Box::<dyn std::error::Error>::from(e))?;

            let state = api::AppState::new(pool.clone(), settings_path);

            // Clone for background seeding
            let seed_pool = pool.clone();
            let seed_handle = app_handle.clone();

            tauri::async_runtime::spawn(async move {
                // Seed items in background so window opens immediately
                if let Err(err) = db::seed_items_if_empty(&seed_pool, &seed_handle).await {
                    eprintln!("[db] item seed failed: {err}");
                }

                if let Err(err) = api::serve(state).await {
                    eprintln!("api server stopped: {err}");
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
