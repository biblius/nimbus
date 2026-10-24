#[cfg(desktop)]
mod desktop;

use tauri_plugin_sql::{Migration, MigrationKind};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
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
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:nimbus.db", migrations)
                .build(),
        )
        .run(tauri::generate_context!())
        .expect("error while running nimbus");
}
