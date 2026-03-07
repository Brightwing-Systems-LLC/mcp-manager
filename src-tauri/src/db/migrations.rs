use rusqlite::Connection;

pub fn run_migrations(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS detected_tools (
            id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            config_path TEXT,
            detected_at TEXT,
            last_verified TEXT,
            is_hidden INTEGER DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS installations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            server_uuid TEXT NOT NULL,
            server_name TEXT NOT NULL,
            tool_id TEXT NOT NULL,
            config_key TEXT NOT NULL,
            installed_at TEXT DEFAULT (datetime('now')),
            config_snapshot TEXT,
            UNIQUE(server_uuid, tool_id)
        );

        CREATE TABLE IF NOT EXISTS server_cache (
            uuid TEXT PRIMARY KEY,
            name TEXT,
            display_name TEXT,
            grade TEXT,
            score INTEGER,
            language TEXT,
            install_config_json TEXT,
            compatibility_json TEXT,
            fetched_at TEXT
        );

        CREATE TABLE IF NOT EXISTS config_backups (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tool_id TEXT NOT NULL,
            config_path TEXT NOT NULL,
            backup_content TEXT NOT NULL,
            backed_up_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS favorites (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            server_uuid TEXT NOT NULL UNIQUE,
            server_name TEXT NOT NULL,
            display_name TEXT,
            grade TEXT,
            score INTEGER,
            language TEXT,
            install_config_json TEXT,
            added_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS preferences (
            key TEXT PRIMARY KEY,
            value TEXT
        );
        ",
    )
    .map_err(|e| format!("Migration failed: {}", e))?;

    Ok(())
}
