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

        CREATE TABLE IF NOT EXISTS disabled_servers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tool_id TEXT NOT NULL,
            server_name TEXT NOT NULL,
            config_json TEXT NOT NULL,
            disabled_at TEXT DEFAULT (datetime('now')),
            UNIQUE(tool_id, server_name)
        );

        -- Proxy servers registered in the auth daemon
        CREATE TABLE IF NOT EXISTS proxy_servers (
            server_id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            auth_type TEXT NOT NULL DEFAULT 'none',
            upstream_url TEXT,
            upstream_command TEXT,
            upstream_args TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );

        -- Per-server per-app tool filter state (which tools are enabled/disabled)
        -- tool_id = app identifier (claude_code, cursor, etc.) or '_all' for global default
        CREATE TABLE IF NOT EXISTS proxy_tool_filter (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            server_id TEXT NOT NULL,
            tool_id TEXT NOT NULL DEFAULT '_all',
            tool_name TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            token_estimate INTEGER NOT NULL DEFAULT 0,
            UNIQUE(server_id, tool_id, tool_name),
            FOREIGN KEY (server_id) REFERENCES proxy_servers(server_id) ON DELETE CASCADE
        );

        -- Tool schema cache (tools/list responses cached from upstream)
        CREATE TABLE IF NOT EXISTS proxy_tool_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            server_id TEXT NOT NULL,
            tool_name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            input_schema TEXT NOT NULL DEFAULT '{}',
            token_estimate INTEGER NOT NULL DEFAULT 0,
            cached_at TEXT DEFAULT (datetime('now')),
            UNIQUE(server_id, tool_name),
            FOREIGN KEY (server_id) REFERENCES proxy_servers(server_id) ON DELETE CASCADE
        );

        -- OAuth server metadata cache (discovery results)
        CREATE TABLE IF NOT EXISTS oauth_server_meta (
            server_id TEXT PRIMARY KEY,
            server_url TEXT NOT NULL,
            metadata_json TEXT NOT NULL,
            discovered_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (server_id) REFERENCES proxy_servers(server_id) ON DELETE CASCADE
        );

        -- OAuth token sets (full token data for refresh)
        CREATE TABLE IF NOT EXISTS oauth_token_sets (
            server_id TEXT PRIMARY KEY,
            token_json TEXT NOT NULL,
            updated_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (server_id) REFERENCES proxy_servers(server_id) ON DELETE CASCADE
        );

        -- API key credentials for proxy servers (env vars as JSON)
        CREATE TABLE IF NOT EXISTS proxy_api_keys (
            server_id TEXT PRIMARY KEY,
            env_json TEXT NOT NULL DEFAULT '{}',
            updated_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (server_id) REFERENCES proxy_servers(server_id) ON DELETE CASCADE
        );

        -- Track which tools have proxy server installs (tool_id = claude_desktop, cursor, etc.)
        CREATE TABLE IF NOT EXISTS proxy_tool_installs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            server_id TEXT NOT NULL,
            tool_id TEXT NOT NULL,
            installed_at TEXT DEFAULT (datetime('now')),
            UNIQUE(server_id, tool_id),
            FOREIGN KEY (server_id) REFERENCES proxy_servers(server_id) ON DELETE CASCADE
        );

        -- Governance: registry policy configuration
        CREATE TABLE IF NOT EXISTS governance_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT DEFAULT (datetime('now'))
        );

        -- Governance: approved servers allowlist
        -- Only servers in this list can be installed when governance is enabled
        CREATE TABLE IF NOT EXISTS governance_allowlist (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            server_identifier TEXT NOT NULL,
            display_name TEXT NOT NULL,
            description TEXT,
            approved_by TEXT NOT NULL,
            approved_at TEXT DEFAULT (datetime('now')),
            review_notes TEXT,
            max_version TEXT,
            UNIQUE(server_identifier)
        );

        -- Governance: approval requests from users wanting to add a server
        CREATE TABLE IF NOT EXISTS governance_requests (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            server_identifier TEXT NOT NULL,
            server_name TEXT NOT NULL,
            requested_by TEXT NOT NULL DEFAULT 'user',
            request_reason TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            reviewed_by TEXT,
            review_notes TEXT,
            requested_at TEXT DEFAULT (datetime('now')),
            reviewed_at TEXT
        );

        -- Governance: audit log for all governance-related actions
        CREATE TABLE IF NOT EXISTS governance_audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            action TEXT NOT NULL,
            actor TEXT NOT NULL,
            target_server TEXT,
            detail TEXT,
            timestamp TEXT DEFAULT (datetime('now'))
        );
        ",
    )
    .map_err(|e| format!("Migration failed: {}", e))?;

    // Migrate proxy_tool_filter to add tool_id column if needed (v2 schema)
    migrate_tool_filter_v2(conn)?;

    // Add api_key_injection column to proxy_servers if missing
    migrate_api_key_injection(conn)?;

    Ok(())
}

/// Add api_key_injection column to proxy_servers for storing how API keys
/// are injected: "bearer" (default) or "query_param:<paramName>".
fn migrate_api_key_injection(conn: &Connection) -> Result<(), String> {
    let has_column = conn
        .prepare("SELECT api_key_injection FROM proxy_servers LIMIT 0")
        .is_ok();

    if has_column {
        return Ok(());
    }

    conn.execute_batch(
        "ALTER TABLE proxy_servers ADD COLUMN api_key_injection TEXT DEFAULT 'bearer';",
    )
    .map_err(|e| format!("api_key_injection migration failed: {}", e))?;

    Ok(())
}

/// Migrate proxy_tool_filter from v1 (server_id, tool_name) to v2 (server_id, tool_id, tool_name).
/// This handles existing databases that already have the old schema.
fn migrate_tool_filter_v2(conn: &Connection) -> Result<(), String> {
    // Check if tool_id column already exists
    let has_tool_id: bool = conn
        .prepare("SELECT tool_id FROM proxy_tool_filter LIMIT 0")
        .is_ok();

    if has_tool_id {
        return Ok(()); // Already migrated
    }

    // Need to recreate the table with the new schema
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS proxy_tool_filter_v2 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            server_id TEXT NOT NULL,
            tool_id TEXT NOT NULL DEFAULT '_all',
            tool_name TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            token_estimate INTEGER NOT NULL DEFAULT 0,
            UNIQUE(server_id, tool_id, tool_name),
            FOREIGN KEY (server_id) REFERENCES proxy_servers(server_id) ON DELETE CASCADE
        );

        INSERT OR IGNORE INTO proxy_tool_filter_v2 (server_id, tool_id, tool_name, enabled, token_estimate)
        SELECT server_id, '_all', tool_name, enabled, token_estimate FROM proxy_tool_filter;

        DROP TABLE proxy_tool_filter;

        ALTER TABLE proxy_tool_filter_v2 RENAME TO proxy_tool_filter;
        ",
    )
    .map_err(|e| format!("Tool filter v2 migration failed: {}", e))?;

    Ok(())
}
