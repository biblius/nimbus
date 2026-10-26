#[cfg(desktop)]
mod desktop;
mod session_cookie;
mod upload;

use tauri_plugin_sql::{Migration, MigrationKind};

pub fn run() {
    let migrations = vec![
        Migration {
            version: 1,
            description: "client settings",
            sql: "CREATE TABLE settings (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );
            INSERT INTO settings (key, value) VALUES ('endpoint', 'http://localhost:42069');",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "access token",
            // Single-row table: CHECK (id = 1) guarantees at most one token.
            sql: "CREATE TABLE access_token (
                id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
                token TEXT NOT NULL
            );",
            kind: MigrationKind::Up,
        },
    ];

    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                desktop::show_main_window(app);
            }))
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init())
            .setup(|app| {
                desktop::setup_tray(app)?;
                Ok(())
            })
            .on_window_event(|window, event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    // Keep running in the tray instead of quitting.
                    let _ = window.hide();
                    api.prevent_close();
                }
            });
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:nimbus.db", migrations)
                .build(),
        )
        .manage(upload::UploadRegistry::new())
        .invoke_handler(tauri::generate_handler![
            session_cookie::set_session_cookie,
            session_cookie::clear_session_cookie,
            upload::upload_multipart,
            upload::cancel_upload,
        ])
        .run(tauri::generate_context!())
        .expect("error while running nimbus");
}
