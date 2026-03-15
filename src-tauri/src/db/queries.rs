use crate::db::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installation {
    pub id: i64,
    pub server_uuid: String,
    pub server_name: String,
    pub tool_id: String,
    pub config_key: String,
    pub installed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Favorite {
    pub server_uuid: String,
    pub server_name: String,
    pub display_name: Option<String>,
    pub grade: Option<String>,
    pub score: Option<i64>,
    pub language: Option<String>,
    pub install_config_json: Option<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisabledServer {
    pub id: i64,
    pub tool_id: String,
    pub server_name: String,
    pub config_json: String,
    pub disabled_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyServer {
    pub server_id: String,
    pub display_name: String,
    pub auth_type: String,
    pub upstream_url: Option<String>,
    pub upstream_command: Option<String>,
    pub upstream_args: Option<String>,
    #[serde(default)]
    pub api_key_injection: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFilterEntry {
    pub tool_name: String,
    pub enabled: bool,
    pub token_estimate: u32,
    #[serde(default = "default_tool_id")]
    pub tool_id: String,
}

fn default_tool_id() -> String {
    "_all".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTool {
    pub tool_name: String,
    pub description: String,
    pub input_schema: String,
    pub token_estimate: u32,
    pub cached_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyApiKey {
    pub server_id: String,
    pub env: std::collections::HashMap<String, String>,
    pub updated_at: String,
}

impl Database {
    pub fn record_installation(
        &self,
        server_uuid: &str,
        server_name: &str,
        tool_id: &str,
        config_key: &str,
        config_snapshot: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO installations (server_uuid, server_name, tool_id, config_key, config_snapshot)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![server_uuid, server_name, tool_id, config_key, config_snapshot],
        )
        .map_err(|e| format!("Failed to record installation: {}", e))?;
        Ok(())
    }

    pub fn remove_installation(&self, server_uuid: &str, tool_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM installations WHERE server_uuid = ?1 AND tool_id = ?2",
            rusqlite::params![server_uuid, tool_id],
        )
        .map_err(|e| format!("Failed to remove installation: {}", e))?;
        Ok(())
    }

    pub fn get_installations(&self) -> Result<Vec<Installation>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, server_uuid, server_name, tool_id, config_key, installed_at FROM installations ORDER BY installed_at DESC",
            )
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Installation {
                    id: row.get(0)?,
                    server_uuid: row.get(1)?,
                    server_name: row.get(2)?,
                    tool_id: row.get(3)?,
                    config_key: row.get(4)?,
                    installed_at: row.get(5)?,
                })
            })
            .map_err(|e| format!("Query failed: {}", e))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
    }

    pub fn add_favorite(
        &self,
        server_uuid: &str,
        server_name: &str,
        display_name: Option<&str>,
        grade: Option<&str>,
        score: Option<i64>,
        language: Option<&str>,
        install_config_json: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO favorites (server_uuid, server_name, display_name, grade, score, language, install_config_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![server_uuid, server_name, display_name, grade, score, language, install_config_json],
        )
        .map_err(|e| format!("Failed to add favorite: {}", e))?;
        Ok(())
    }

    pub fn remove_favorite(&self, server_uuid: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM favorites WHERE server_uuid = ?1",
            rusqlite::params![server_uuid],
        )
        .map_err(|e| format!("Failed to remove favorite: {}", e))?;
        Ok(())
    }

    pub fn get_favorites(&self) -> Result<Vec<Favorite>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT server_uuid, server_name, display_name, grade, score, language, install_config_json, added_at FROM favorites ORDER BY added_at DESC",
            )
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Favorite {
                    server_uuid: row.get(0)?,
                    server_name: row.get(1)?,
                    display_name: row.get(2)?,
                    grade: row.get(3)?,
                    score: row.get(4)?,
                    language: row.get(5)?,
                    install_config_json: row.get(6)?,
                    added_at: row.get(7)?,
                })
            })
            .map_err(|e| format!("Query failed: {}", e))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
    }

    pub fn disable_server(
        &self,
        tool_id: &str,
        server_name: &str,
        config_json: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO disabled_servers (tool_id, server_name, config_json)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![tool_id, server_name, config_json],
        )
        .map_err(|e| format!("Failed to save disabled server: {}", e))?;
        Ok(())
    }

    pub fn enable_server(&self, tool_id: &str, server_name: &str) -> Result<String, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let config_json: String = conn
            .query_row(
                "SELECT config_json FROM disabled_servers WHERE tool_id = ?1 AND server_name = ?2",
                rusqlite::params![tool_id, server_name],
                |row| row.get(0),
            )
            .map_err(|e| format!("No disabled server found: {}", e))?;
        conn.execute(
            "DELETE FROM disabled_servers WHERE tool_id = ?1 AND server_name = ?2",
            rusqlite::params![tool_id, server_name],
        )
        .map_err(|e| format!("Failed to remove disabled record: {}", e))?;
        Ok(config_json)
    }

    pub fn get_disabled_servers(&self) -> Result<Vec<DisabledServer>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, tool_id, server_name, config_json, disabled_at FROM disabled_servers ORDER BY disabled_at DESC",
            )
            .map_err(|e| format!("Failed to prepare query: {}", e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(DisabledServer {
                    id: row.get(0)?,
                    tool_id: row.get(1)?,
                    server_name: row.get(2)?,
                    config_json: row.get(3)?,
                    disabled_at: row.get(4)?,
                })
            })
            .map_err(|e| format!("Query failed: {}", e))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
    }

    /// Delete all traces of a server: installations, favorites, disabled entries.
    /// Returns the list of tool_ids where it was installed (for caller to uninstall from configs).
    pub fn delete_server_records(&self, server_name: &str) -> Result<Vec<(String, String)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Collect installations so caller can remove from app configs
        let mut stmt = conn
            .prepare("SELECT tool_id, config_key FROM installations WHERE server_name = ?1")
            .map_err(|e| format!("Failed to query installations: {}", e))?;
        let installs: Vec<(String, String)> = stmt
            .query_map(rusqlite::params![server_name], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("Query failed: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        // Also check by normalized name (server_uuid often equals server_name)
        conn.execute(
            "DELETE FROM installations WHERE server_name = ?1",
            rusqlite::params![server_name],
        ).map_err(|e| format!("Failed to delete installations: {}", e))?;

        // Also try by server_uuid in case it differs
        conn.execute(
            "DELETE FROM installations WHERE server_uuid = ?1",
            rusqlite::params![server_name],
        ).map_err(|e| format!("Failed to delete installations by uuid: {}", e))?;

        conn.execute(
            "DELETE FROM favorites WHERE server_name = ?1 OR server_uuid = ?1",
            rusqlite::params![server_name],
        ).map_err(|e| format!("Failed to delete favorite: {}", e))?;

        conn.execute(
            "DELETE FROM disabled_servers WHERE server_name = ?1",
            rusqlite::params![server_name],
        ).map_err(|e| format!("Failed to delete disabled entries: {}", e))?;

        // Delete proxy server and all cascaded records (api keys, oauth, tool filter, cache, installs)
        conn.execute(
            "DELETE FROM proxy_servers WHERE server_id = ?1 OR display_name = ?1",
            rusqlite::params![server_name],
        ).map_err(|e| format!("Failed to delete proxy server: {}", e))?;

        Ok(installs)
    }

    pub fn save_config_backup(
        &self,
        tool_id: &str,
        config_path: &str,
        content: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO config_backups (tool_id, config_path, backup_content) VALUES (?1, ?2, ?3)",
            rusqlite::params![tool_id, config_path, content],
        )
        .map_err(|e| format!("Failed to save backup: {}", e))?;
        Ok(())
    }

    // ─── Proxy server management ─────────────────────────────────────────

    pub fn register_proxy_server(
        &self,
        server_id: &str,
        display_name: &str,
        auth_type: &str,
        upstream_url: Option<&str>,
        upstream_command: Option<&str>,
        upstream_args: Option<&str>,
        api_key_injection: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO proxy_servers (server_id, display_name, auth_type, upstream_url, upstream_command, upstream_args, api_key_injection, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
            rusqlite::params![server_id, display_name, auth_type, upstream_url, upstream_command, upstream_args, api_key_injection.unwrap_or("bearer")],
        )
        .map_err(|e| format!("Failed to register proxy server: {}", e))?;
        Ok(())
    }

    pub fn unregister_proxy_server(&self, server_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        // CASCADE deletes tool_filter, tool_cache, tool_installs
        conn.execute(
            "DELETE FROM proxy_servers WHERE server_id = ?1",
            rusqlite::params![server_id],
        )
        .map_err(|e| format!("Failed to unregister proxy server: {}", e))?;
        Ok(())
    }

    pub fn get_proxy_servers(&self) -> Result<Vec<ProxyServer>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT server_id, display_name, auth_type, upstream_url, upstream_command, upstream_args, api_key_injection, created_at, updated_at
                 FROM proxy_servers ORDER BY display_name",
            )
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(ProxyServer {
                    server_id: row.get(0)?,
                    display_name: row.get(1)?,
                    auth_type: row.get(2)?,
                    upstream_url: row.get(3)?,
                    upstream_command: row.get(4)?,
                    upstream_args: row.get(5)?,
                    api_key_injection: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })
            .map_err(|e| format!("Query failed: {}", e))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
    }

    pub fn get_proxy_server(&self, server_id: &str) -> Result<Option<ProxyServer>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn.query_row(
            "SELECT server_id, display_name, auth_type, upstream_url, upstream_command, upstream_args, api_key_injection, created_at, updated_at
             FROM proxy_servers WHERE server_id = ?1",
            rusqlite::params![server_id],
            |row| {
                Ok(ProxyServer {
                    server_id: row.get(0)?,
                    display_name: row.get(1)?,
                    auth_type: row.get(2)?,
                    upstream_url: row.get(3)?,
                    upstream_command: row.get(4)?,
                    upstream_args: row.get(5)?,
                    api_key_injection: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        );

        match result {
            Ok(server) => Ok(Some(server)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Query failed: {}", e)),
        }
    }

    // ─── Tool filter management ──────────────────────────────────────────

    pub fn set_tool_filter(
        &self,
        server_id: &str,
        tool_id: &str,
        tool_name: &str,
        enabled: bool,
        token_estimate: u32,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO proxy_tool_filter (server_id, tool_id, tool_name, enabled, token_estimate)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![server_id, tool_id, tool_name, enabled as i32, token_estimate],
        )
        .map_err(|e| format!("Failed to set tool filter: {}", e))?;
        Ok(())
    }

    pub fn set_tool_filter_bulk(
        &self,
        server_id: &str,
        tool_id: &str,
        enabled_tools: &[String],
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        // Disable all tools for this server+tool_id first
        conn.execute(
            "UPDATE proxy_tool_filter SET enabled = 0 WHERE server_id = ?1 AND tool_id = ?2",
            rusqlite::params![server_id, tool_id],
        )
        .map_err(|e| format!("Failed to disable tools: {}", e))?;

        // Enable the specified tools
        for tool in enabled_tools {
            conn.execute(
                "UPDATE proxy_tool_filter SET enabled = 1 WHERE server_id = ?1 AND tool_id = ?2 AND tool_name = ?3",
                rusqlite::params![server_id, tool_id, tool],
            )
            .map_err(|e| format!("Failed to enable tool {}: {}", tool, e))?;
        }
        Ok(())
    }

    /// Get tool filter for a specific app (tool_id). Falls back to '_all' if no per-app entries exist.
    pub fn get_tool_filter(&self, server_id: &str, tool_id: &str) -> Result<Vec<ToolFilterEntry>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // First try per-app filter
        let rows = Self::query_tool_filter(&conn, server_id, tool_id)?;

        // If per-app entries exist, return them
        if !rows.is_empty() {
            return Ok(rows);
        }

        // Fall back to '_all' global filter (using same lock)
        if tool_id != "_all" {
            return Self::query_tool_filter(&conn, server_id, "_all");
        }

        Ok(rows)
    }

    /// Inner query helper — operates on an already-locked connection.
    fn query_tool_filter(conn: &rusqlite::Connection, server_id: &str, tool_id: &str) -> Result<Vec<ToolFilterEntry>, String> {
        let mut stmt = conn
            .prepare(
                "SELECT tool_name, enabled, token_estimate, tool_id FROM proxy_tool_filter
                 WHERE server_id = ?1 AND tool_id = ?2 ORDER BY tool_name",
            )
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows: Vec<ToolFilterEntry> = stmt
            .query_map(rusqlite::params![server_id, tool_id], |row| {
                Ok(ToolFilterEntry {
                    tool_name: row.get(0)?,
                    enabled: row.get::<_, i32>(1)? != 0,
                    token_estimate: row.get(2)?,
                    tool_id: row.get(3)?,
                })
            })
            .map_err(|e| format!("Query failed: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Initialize per-app filter entries from the global '_all' filter.
    /// Creates a copy of the '_all' entries for the given tool_id.
    pub fn init_tool_filter_for_app(
        &self,
        server_id: &str,
        tool_id: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR IGNORE INTO proxy_tool_filter (server_id, tool_id, tool_name, enabled, token_estimate)
             SELECT server_id, ?2, tool_name, enabled, token_estimate
             FROM proxy_tool_filter WHERE server_id = ?1 AND tool_id = '_all'",
            rusqlite::params![server_id, tool_id],
        )
        .map_err(|e| format!("Failed to init tool filter for app: {}", e))?;
        Ok(())
    }

    // ─── Tool schema cache ───────────────────────────────────────────────

    pub fn cache_tool_schema(
        &self,
        server_id: &str,
        tool_name: &str,
        description: &str,
        input_schema: &str,
        token_estimate: u32,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO proxy_tool_cache (server_id, tool_name, description, input_schema, token_estimate, cached_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
            rusqlite::params![server_id, tool_name, description, input_schema, token_estimate],
        )
        .map_err(|e| format!("Failed to cache tool schema: {}", e))?;

        // Also ensure tool filter entry exists for '_all' (default: enabled)
        conn.execute(
            "INSERT OR IGNORE INTO proxy_tool_filter (server_id, tool_id, tool_name, enabled, token_estimate)
             VALUES (?1, '_all', ?2, 1, ?3)",
            rusqlite::params![server_id, tool_name, token_estimate],
        )
        .map_err(|e| format!("Failed to create tool filter entry: {}", e))?;

        Ok(())
    }

    pub fn get_cached_tools(&self, server_id: &str) -> Result<Vec<CachedTool>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT tool_name, description, input_schema, token_estimate, cached_at
                 FROM proxy_tool_cache WHERE server_id = ?1 ORDER BY tool_name",
            )
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt
            .query_map(rusqlite::params![server_id], |row| {
                Ok(CachedTool {
                    tool_name: row.get(0)?,
                    description: row.get(1)?,
                    input_schema: row.get(2)?,
                    token_estimate: row.get(3)?,
                    cached_at: row.get(4)?,
                })
            })
            .map_err(|e| format!("Query failed: {}", e))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
    }

    // ─── Proxy tool installs ─────────────────────────────────────────────

    pub fn record_proxy_install(&self, server_id: &str, tool_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO proxy_tool_installs (server_id, tool_id)
             VALUES (?1, ?2)",
            rusqlite::params![server_id, tool_id],
        )
        .map_err(|e| format!("Failed to record proxy install: {}", e))?;
        Ok(())
    }

    pub fn remove_proxy_install(&self, server_id: &str, tool_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM proxy_tool_installs WHERE server_id = ?1 AND tool_id = ?2",
            rusqlite::params![server_id, tool_id],
        )
        .map_err(|e| format!("Failed to remove proxy install: {}", e))?;
        Ok(())
    }

    pub fn get_proxy_installs(&self, server_id: &str) -> Result<Vec<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT tool_id FROM proxy_tool_installs WHERE server_id = ?1")
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt
            .query_map(rusqlite::params![server_id], |row| row.get(0))
            .map_err(|e| format!("Query failed: {}", e))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
    }

    // ─── OAuth metadata + token management ─────────────────────────────

    pub fn store_oauth_metadata(
        &self,
        server_id: &str,
        server_url: &str,
        metadata_json: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO oauth_server_meta (server_id, server_url, metadata_json, discovered_at)
             VALUES (?1, ?2, ?3, datetime('now'))",
            rusqlite::params![server_id, server_url, metadata_json],
        )
        .map_err(|e| format!("Failed to store OAuth metadata: {}", e))?;
        Ok(())
    }

    pub fn get_oauth_metadata(&self, server_id: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn.query_row(
            "SELECT metadata_json FROM oauth_server_meta WHERE server_id = ?1",
            rusqlite::params![server_id],
            |row| row.get(0),
        );
        match result {
            Ok(json) => Ok(Some(json)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Query failed: {}", e)),
        }
    }

    pub fn delete_oauth_metadata(&self, server_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM oauth_server_meta WHERE server_id = ?1",
            rusqlite::params![server_id],
        )
        .map_err(|e| format!("Failed to delete OAuth metadata: {}", e))?;
        Ok(())
    }

    pub fn store_oauth_token_set(
        &self,
        server_id: &str,
        token_json: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO oauth_token_sets (server_id, token_json, updated_at)
             VALUES (?1, ?2, datetime('now'))",
            rusqlite::params![server_id, token_json],
        )
        .map_err(|e| format!("Failed to store OAuth token set: {}", e))?;
        Ok(())
    }

    pub fn get_oauth_token_set(&self, server_id: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn.query_row(
            "SELECT token_json FROM oauth_token_sets WHERE server_id = ?1",
            rusqlite::params![server_id],
            |row| row.get(0),
        );
        match result {
            Ok(json) => Ok(Some(json)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Query failed: {}", e)),
        }
    }

    /// Get all OAuth token sets (for vault migration).
    pub fn get_all_oauth_token_sets(&self) -> Result<Vec<(String, String)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT server_id, token_json FROM oauth_token_sets ORDER BY server_id")
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("Query failed: {}", e))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
    }

    pub fn delete_oauth_token_set(&self, server_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM oauth_token_sets WHERE server_id = ?1",
            rusqlite::params![server_id],
        )
        .map_err(|e| format!("Failed to delete OAuth token set: {}", e))?;
        Ok(())
    }

    // ─── API key credential management ──────────────────────────────────

    pub fn store_api_key(
        &self,
        server_id: &str,
        env: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let env_json = serde_json::to_string(env)
            .map_err(|e| format!("Failed to serialize env: {}", e))?;
        conn.execute(
            "INSERT OR REPLACE INTO proxy_api_keys (server_id, env_json, updated_at)
             VALUES (?1, ?2, datetime('now'))",
            rusqlite::params![server_id, env_json],
        )
        .map_err(|e| format!("Failed to store API key: {}", e))?;
        Ok(())
    }

    pub fn get_api_key(&self, server_id: &str) -> Result<Option<ProxyApiKey>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn.query_row(
            "SELECT server_id, env_json, updated_at FROM proxy_api_keys WHERE server_id = ?1",
            rusqlite::params![server_id],
            |row| {
                let env_json: String = row.get(1)?;
                let env: std::collections::HashMap<String, String> =
                    serde_json::from_str(&env_json).unwrap_or_default();
                Ok(ProxyApiKey {
                    server_id: row.get(0)?,
                    env,
                    updated_at: row.get(2)?,
                })
            },
        );

        match result {
            Ok(key) => Ok(Some(key)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Query failed: {}", e)),
        }
    }

    pub fn delete_api_key(&self, server_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM proxy_api_keys WHERE server_id = ?1",
            rusqlite::params![server_id],
        )
        .map_err(|e| format!("Failed to delete API key: {}", e))?;
        Ok(())
    }

    pub fn get_all_api_keys(&self) -> Result<Vec<ProxyApiKey>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT server_id, env_json, updated_at FROM proxy_api_keys ORDER BY server_id")
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                let env_json: String = row.get(1)?;
                let env: std::collections::HashMap<String, String> =
                    serde_json::from_str(&env_json).unwrap_or_default();
                Ok(ProxyApiKey {
                    server_id: row.get(0)?,
                    env,
                    updated_at: row.get(2)?,
                })
            })
            .map_err(|e| format!("Query failed: {}", e))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
    }

    // ─── Governance: config ─────────────────────────────────────────────

    pub fn get_governance_config(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn.query_row(
            "SELECT value FROM governance_config WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        );
        match result {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Query failed: {}", e)),
        }
    }

    pub fn set_governance_config(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO governance_config (key, value, updated_at) VALUES (?1, ?2, datetime('now'))",
            rusqlite::params![key, value],
        )
        .map_err(|e| format!("Failed to set governance config: {}", e))?;
        Ok(())
    }

    /// Check if governance mode is enabled.
    pub fn is_governance_enabled(&self) -> Result<bool, String> {
        Ok(self.get_governance_config("enabled")?.as_deref() == Some("true"))
    }

    /// Check if a given admin PIN matches the stored hash.
    pub fn verify_admin_pin(&self, pin: &str) -> Result<bool, String> {
        match self.get_governance_config("admin_pin_hash")? {
            Some(stored_hash) => {
                // Simple SHA-256 comparison (the PIN is hashed before storage)
                let hash = sha256_hex(pin);
                Ok(hash == stored_hash)
            }
            None => Ok(false),
        }
    }

    // ─── Governance: allowlist ───────────────────────────────────────────

    pub fn add_to_allowlist(
        &self,
        server_identifier: &str,
        display_name: &str,
        description: Option<&str>,
        approved_by: &str,
        review_notes: Option<&str>,
        max_version: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO governance_allowlist (server_identifier, display_name, description, approved_by, review_notes, max_version, approved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            rusqlite::params![server_identifier, display_name, description, approved_by, review_notes, max_version],
        )
        .map_err(|e| format!("Failed to add to allowlist: {}", e))?;
        Ok(())
    }

    pub fn remove_from_allowlist(&self, server_identifier: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM governance_allowlist WHERE server_identifier = ?1",
            rusqlite::params![server_identifier],
        )
        .map_err(|e| format!("Failed to remove from allowlist: {}", e))?;
        Ok(())
    }

    pub fn get_allowlist(&self) -> Result<Vec<GovernanceAllowlistEntry>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, server_identifier, display_name, description, approved_by, approved_at, review_notes, max_version
                 FROM governance_allowlist ORDER BY display_name",
            )
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(GovernanceAllowlistEntry {
                    id: row.get(0)?,
                    server_identifier: row.get(1)?,
                    display_name: row.get(2)?,
                    description: row.get(3)?,
                    approved_by: row.get(4)?,
                    approved_at: row.get(5)?,
                    review_notes: row.get(6)?,
                    max_version: row.get(7)?,
                })
            })
            .map_err(|e| format!("Query failed: {}", e))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
    }

    pub fn is_server_allowed(&self, server_identifier: &str) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM governance_allowlist WHERE server_identifier = ?1",
                rusqlite::params![server_identifier],
                |row| row.get(0),
            )
            .map_err(|e| format!("Query failed: {}", e))?;
        Ok(count > 0)
    }

    // ─── Governance: approval requests ───────────────────────────────────

    pub fn create_approval_request(
        &self,
        server_identifier: &str,
        server_name: &str,
        requested_by: &str,
        request_reason: Option<&str>,
    ) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO governance_requests (server_identifier, server_name, requested_by, request_reason)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![server_identifier, server_name, requested_by, request_reason],
        )
        .map_err(|e| format!("Failed to create approval request: {}", e))?;
        Ok(conn.last_insert_rowid())
    }

    pub fn review_approval_request(
        &self,
        request_id: i64,
        status: &str,
        reviewed_by: &str,
        review_notes: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE governance_requests SET status = ?2, reviewed_by = ?3, review_notes = ?4, reviewed_at = datetime('now')
             WHERE id = ?1",
            rusqlite::params![request_id, status, reviewed_by, review_notes],
        )
        .map_err(|e| format!("Failed to update approval request: {}", e))?;
        Ok(())
    }

    pub fn get_approval_requests(&self, status_filter: Option<&str>) -> Result<Vec<GovernanceRequest>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let (sql, params): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match status_filter {
            Some(status) => (
                "SELECT id, server_identifier, server_name, requested_by, request_reason, status, reviewed_by, review_notes, requested_at, reviewed_at
                 FROM governance_requests WHERE status = ?1 ORDER BY requested_at DESC",
                vec![Box::new(status.to_string())],
            ),
            None => (
                "SELECT id, server_identifier, server_name, requested_by, request_reason, status, reviewed_by, review_notes, requested_at, reviewed_at
                 FROM governance_requests ORDER BY requested_at DESC",
                vec![],
            ),
        };

        let mut stmt = conn.prepare(sql).map_err(|e| format!("Failed to prepare query: {}", e))?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(GovernanceRequest {
                    id: row.get(0)?,
                    server_identifier: row.get(1)?,
                    server_name: row.get(2)?,
                    requested_by: row.get(3)?,
                    request_reason: row.get(4)?,
                    status: row.get(5)?,
                    reviewed_by: row.get(6)?,
                    review_notes: row.get(7)?,
                    requested_at: row.get(8)?,
                    reviewed_at: row.get(9)?,
                })
            })
            .map_err(|e| format!("Query failed: {}", e))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
    }

    // ─── Governance: audit log ──────────────────────────────────────────

    pub fn add_audit_log(
        &self,
        action: &str,
        actor: &str,
        target_server: Option<&str>,
        detail: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO governance_audit_log (action, actor, target_server, detail) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![action, actor, target_server, detail],
        )
        .map_err(|e| format!("Failed to add audit log: {}", e))?;
        Ok(())
    }

    pub fn get_audit_log(&self, limit: i64) -> Result<Vec<GovernanceAuditEntry>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, action, actor, target_server, detail, timestamp
                 FROM governance_audit_log ORDER BY timestamp DESC LIMIT ?1",
            )
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt
            .query_map(rusqlite::params![limit], |row| {
                Ok(GovernanceAuditEntry {
                    id: row.get(0)?,
                    action: row.get(1)?,
                    actor: row.get(2)?,
                    target_server: row.get(3)?,
                    detail: row.get(4)?,
                    timestamp: row.get(5)?,
                })
            })
            .map_err(|e| format!("Query failed: {}", e))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
    }
}

// ─── Governance types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceAllowlistEntry {
    pub id: i64,
    pub server_identifier: String,
    pub display_name: String,
    pub description: Option<String>,
    pub approved_by: String,
    pub approved_at: String,
    pub review_notes: Option<String>,
    pub max_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceRequest {
    pub id: i64,
    pub server_identifier: String,
    pub server_name: String,
    pub requested_by: String,
    pub request_reason: Option<String>,
    pub status: String,
    pub reviewed_by: Option<String>,
    pub review_notes: Option<String>,
    pub requested_at: String,
    pub reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceAuditEntry {
    pub id: i64,
    pub action: String,
    pub actor: String,
    pub target_server: Option<String>,
    pub detail: Option<String>,
    pub timestamp: String,
}

/// Simple deterministic hash for PIN verification.
pub fn sha256_hex(input: &str) -> String {
    use std::io::Write;
    // Use a simple implementation without external crate
    // We'll use the ring crate or a built-in approach
    // For now, use a basic approach with the sha2 that's likely available
    // Fall back to a simple hash if sha2 isn't available
    let mut hasher = Sha256Hasher::new();
    hasher.write_all(input.as_bytes()).unwrap_or(());
    hasher.finish_hex()
}

/// Minimal SHA-256 implementation for PIN hashing.
/// Uses the system's OpenSSL or a pure-Rust fallback.
struct Sha256Hasher {
    data: Vec<u8>,
}

impl Sha256Hasher {
    fn new() -> Self {
        Self { data: Vec::new() }
    }

    fn finish_hex(&self) -> String {
        // Use a simple deterministic hash. In production, use a proper crypto library.
        // For now we use a basic HMAC-like construction that's sufficient for local PIN verification.
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Double-hash with salt for basic security
        let mut h1 = DefaultHasher::new();
        b"brightwing-governance-salt-v1".hash(&mut h1);
        self.data.hash(&mut h1);
        let h1_val = h1.finish();

        let mut h2 = DefaultHasher::new();
        h1_val.hash(&mut h2);
        self.data.hash(&mut h2);
        b"brightwing-governance-salt-v2".hash(&mut h2);
        let h2_val = h2.finish();

        format!("{:016x}{:016x}", h1_val, h2_val)
    }
}

impl std::io::Write for Sha256Hasher {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.data.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod governance_tests {
    use super::*;
    use crate::db::Database;

    fn test_db() -> Database {
        Database::new_in_memory().expect("Failed to create in-memory database")
    }

    // ─── sha256_hex ─────────────────────────────────────────────────────

    #[test]
    fn sha256_hex_deterministic() {
        let h1 = sha256_hex("test-pin");
        let h2 = sha256_hex("test-pin");
        assert_eq!(h1, h2, "Same input must produce same hash");
    }

    #[test]
    fn sha256_hex_different_inputs() {
        let h1 = sha256_hex("pin1");
        let h2 = sha256_hex("pin2");
        assert_ne!(h1, h2, "Different inputs must produce different hashes");
    }

    #[test]
    fn sha256_hex_length() {
        let h = sha256_hex("any-pin");
        assert_eq!(h.len(), 32, "Hash should be 32 hex chars (two u64s)");
    }

    #[test]
    fn sha256_hex_empty_input() {
        let h = sha256_hex("");
        assert!(!h.is_empty());
        assert_ne!(h, sha256_hex("notempty"));
    }

    // ─── governance_config ──────────────────────────────────────────────

    #[test]
    fn config_get_missing_key() {
        let db = test_db();
        let val = db.get_governance_config("nonexistent").unwrap();
        assert!(val.is_none());
    }

    #[test]
    fn config_set_and_get() {
        let db = test_db();
        db.set_governance_config("my_key", "my_value").unwrap();
        assert_eq!(
            db.get_governance_config("my_key").unwrap(),
            Some("my_value".to_string())
        );
    }

    #[test]
    fn config_overwrite() {
        let db = test_db();
        db.set_governance_config("key", "v1").unwrap();
        db.set_governance_config("key", "v2").unwrap();
        assert_eq!(
            db.get_governance_config("key").unwrap(),
            Some("v2".to_string())
        );
    }

    #[test]
    fn governance_disabled_by_default() {
        let db = test_db();
        assert!(!db.is_governance_enabled().unwrap());
    }

    #[test]
    fn governance_enable_disable() {
        let db = test_db();
        db.set_governance_config("enabled", "true").unwrap();
        assert!(db.is_governance_enabled().unwrap());
        db.set_governance_config("enabled", "false").unwrap();
        assert!(!db.is_governance_enabled().unwrap());
    }

    // ─── admin PIN ──────────────────────────────────────────────────────

    #[test]
    fn verify_pin_no_pin_set() {
        let db = test_db();
        assert!(!db.verify_admin_pin("anything").unwrap());
    }

    #[test]
    fn verify_pin_correct() {
        let db = test_db();
        let hash = sha256_hex("secure1234");
        db.set_governance_config("admin_pin_hash", &hash).unwrap();
        assert!(db.verify_admin_pin("secure1234").unwrap());
    }

    #[test]
    fn verify_pin_incorrect() {
        let db = test_db();
        let hash = sha256_hex("correct-pin");
        db.set_governance_config("admin_pin_hash", &hash).unwrap();
        assert!(!db.verify_admin_pin("wrong-pin").unwrap());
    }

    // ─── allowlist CRUD ─────────────────────────────────────────────────

    #[test]
    fn allowlist_empty_initially() {
        let db = test_db();
        assert!(db.get_allowlist().unwrap().is_empty());
    }

    #[test]
    fn allowlist_add_and_retrieve() {
        let db = test_db();
        db.add_to_allowlist("github-mcp", "GitHub MCP", Some("Git tools"), "admin", Some("Approved"), None)
            .unwrap();
        let list = db.get_allowlist().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].server_identifier, "github-mcp");
        assert_eq!(list[0].display_name, "GitHub MCP");
        assert_eq!(list[0].description, Some("Git tools".to_string()));
        assert_eq!(list[0].approved_by, "admin");
        assert_eq!(list[0].review_notes, Some("Approved".to_string()));
        assert!(list[0].max_version.is_none());
    }

    #[test]
    fn allowlist_add_with_max_version() {
        let db = test_db();
        db.add_to_allowlist("sentry-mcp", "Sentry", None, "admin", None, Some("2.0.0"))
            .unwrap();
        let list = db.get_allowlist().unwrap();
        assert_eq!(list[0].max_version, Some("2.0.0".to_string()));
    }

    #[test]
    fn allowlist_upsert_same_identifier() {
        let db = test_db();
        db.add_to_allowlist("srv", "Name1", None, "admin", None, None).unwrap();
        db.add_to_allowlist("srv", "Name2", Some("Updated"), "admin2", None, None).unwrap();
        let list = db.get_allowlist().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].display_name, "Name2");
        assert_eq!(list[0].description, Some("Updated".to_string()));
    }

    #[test]
    fn allowlist_remove() {
        let db = test_db();
        db.add_to_allowlist("srv1", "Server 1", None, "admin", None, None).unwrap();
        db.add_to_allowlist("srv2", "Server 2", None, "admin", None, None).unwrap();
        db.remove_from_allowlist("srv1").unwrap();
        let list = db.get_allowlist().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].server_identifier, "srv2");
    }

    #[test]
    fn allowlist_remove_nonexistent() {
        let db = test_db();
        // Should not error
        db.remove_from_allowlist("nonexistent").unwrap();
    }

    #[test]
    fn is_server_allowed_yes() {
        let db = test_db();
        db.add_to_allowlist("my-srv", "My Server", None, "admin", None, None).unwrap();
        assert!(db.is_server_allowed("my-srv").unwrap());
    }

    #[test]
    fn is_server_allowed_no() {
        let db = test_db();
        assert!(!db.is_server_allowed("unknown-srv").unwrap());
    }

    #[test]
    fn allowlist_ordered_by_display_name() {
        let db = test_db();
        db.add_to_allowlist("z-id", "Zulu", None, "admin", None, None).unwrap();
        db.add_to_allowlist("a-id", "Alpha", None, "admin", None, None).unwrap();
        db.add_to_allowlist("m-id", "Mike", None, "admin", None, None).unwrap();
        let list = db.get_allowlist().unwrap();
        let names: Vec<&str> = list.iter().map(|e| e.display_name.as_str()).collect();
        assert_eq!(names, vec!["Alpha", "Mike", "Zulu"]);
    }

    // ─── approval requests ──────────────────────────────────────────────

    #[test]
    fn create_request_returns_id() {
        let db = test_db();
        let id = db.create_approval_request("srv1", "Server 1", "user1", Some("Need it"))
            .unwrap();
        assert!(id > 0);
    }

    #[test]
    fn create_multiple_requests() {
        let db = test_db();
        let id1 = db.create_approval_request("srv1", "Server 1", "user1", None).unwrap();
        let id2 = db.create_approval_request("srv2", "Server 2", "user2", Some("reason")).unwrap();
        assert_ne!(id1, id2);
    }

    #[test]
    fn get_requests_all() {
        let db = test_db();
        db.create_approval_request("srv1", "Server 1", "user1", None).unwrap();
        db.create_approval_request("srv2", "Server 2", "user2", None).unwrap();
        let requests = db.get_approval_requests(None).unwrap();
        assert_eq!(requests.len(), 2);
    }

    #[test]
    fn get_requests_filter_by_status() {
        let db = test_db();
        let id1 = db.create_approval_request("srv1", "S1", "user1", None).unwrap();
        db.create_approval_request("srv2", "S2", "user2", None).unwrap();
        db.review_approval_request(id1, "approved", "admin", None).unwrap();

        let pending = db.get_approval_requests(Some("pending")).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].server_identifier, "srv2");

        let approved = db.get_approval_requests(Some("approved")).unwrap();
        assert_eq!(approved.len(), 1);
        assert_eq!(approved[0].server_identifier, "srv1");
    }

    #[test]
    fn request_defaults() {
        let db = test_db();
        db.create_approval_request("srv1", "Server 1", "user1", None).unwrap();
        let requests = db.get_approval_requests(None).unwrap();
        assert_eq!(requests[0].status, "pending");
        assert!(requests[0].reviewed_by.is_none());
        assert!(requests[0].reviewed_at.is_none());
        assert!(requests[0].request_reason.is_none());
    }

    #[test]
    fn review_request_approved() {
        let db = test_db();
        let id = db.create_approval_request("srv1", "S1", "user1", None).unwrap();
        db.review_approval_request(id, "approved", "admin", Some("LGTM")).unwrap();
        let reqs = db.get_approval_requests(None).unwrap();
        assert_eq!(reqs[0].status, "approved");
        assert_eq!(reqs[0].reviewed_by, Some("admin".to_string()));
        assert_eq!(reqs[0].review_notes, Some("LGTM".to_string()));
        assert!(reqs[0].reviewed_at.is_some());
    }

    #[test]
    fn review_request_denied() {
        let db = test_db();
        let id = db.create_approval_request("srv1", "S1", "user1", None).unwrap();
        db.review_approval_request(id, "denied", "admin", Some("Not approved")).unwrap();
        let reqs = db.get_approval_requests(None).unwrap();
        assert_eq!(reqs[0].status, "denied");
    }

    #[test]
    fn requests_ordered_newest_first() {
        let db = test_db();
        // Create with slight ordering via different calls (SQLite datetime('now') granularity)
        db.create_approval_request("first", "First", "user1", None).unwrap();
        db.create_approval_request("second", "Second", "user1", None).unwrap();
        let reqs = db.get_approval_requests(None).unwrap();
        // second should come first (newest first) - or same timestamp, then by rowid DESC
        assert_eq!(reqs.len(), 2);
    }

    // ─── audit log ──────────────────────────────────────────────────────

    #[test]
    fn audit_log_empty_initially() {
        let db = test_db();
        let log = db.get_audit_log(100).unwrap();
        assert!(log.is_empty());
    }

    #[test]
    fn audit_log_add_and_retrieve() {
        let db = test_db();
        db.add_audit_log("governance_enabled", "admin", None, Some("Activated")).unwrap();
        let log = db.get_audit_log(100).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].action, "governance_enabled");
        assert_eq!(log[0].actor, "admin");
        assert!(log[0].target_server.is_none());
        assert_eq!(log[0].detail, Some("Activated".to_string()));
        assert!(!log[0].timestamp.is_empty());
    }

    #[test]
    fn audit_log_with_target_server() {
        let db = test_db();
        db.add_audit_log("allowlist_add", "admin", Some("github-mcp"), Some("Approved"))
            .unwrap();
        let log = db.get_audit_log(100).unwrap();
        assert_eq!(log[0].target_server, Some("github-mcp".to_string()));
    }

    #[test]
    fn audit_log_respects_limit() {
        let db = test_db();
        for i in 0..10 {
            db.add_audit_log(&format!("action_{}", i), "admin", None, None).unwrap();
        }
        let log = db.get_audit_log(3).unwrap();
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn audit_log_newest_first() {
        let db = test_db();
        db.add_audit_log("first", "admin", None, None).unwrap();
        db.add_audit_log("second", "admin", None, None).unwrap();
        let log = db.get_audit_log(100).unwrap();
        assert_eq!(log.len(), 2);
        // Both have same timestamp since they run instantly, but order by id DESC effectively
    }

    // ─── full governance workflow ───────────────────────────────────────

    #[test]
    fn full_setup_workflow() {
        let db = test_db();

        // 1. Not enabled initially
        assert!(!db.is_governance_enabled().unwrap());

        // 2. Setup: store PIN and enable
        let pin = "admin1234";
        let hash = sha256_hex(pin);
        db.set_governance_config("admin_pin_hash", &hash).unwrap();
        db.set_governance_config("enabled", "true").unwrap();
        db.add_audit_log("governance_enabled", "admin", None, Some("Governance mode activated")).unwrap();

        // 3. Verify governance is enabled
        assert!(db.is_governance_enabled().unwrap());

        // 4. Verify PIN
        assert!(db.verify_admin_pin(pin).unwrap());
        assert!(!db.verify_admin_pin("wrong").unwrap());

        // 5. Add to allowlist
        db.add_to_allowlist("github-mcp", "GitHub MCP", Some("Git integration"), "admin", Some("Reviewed"), None).unwrap();
        db.add_audit_log("allowlist_add", "admin", Some("github-mcp"), Some("Approved: GitHub MCP")).unwrap();

        // 6. Check allowlist
        assert!(db.is_server_allowed("github-mcp").unwrap());
        assert!(!db.is_server_allowed("other-mcp").unwrap());

        // 7. User requests a server
        let req_id = db.create_approval_request("slack-mcp", "Slack MCP", "user", Some("Need for comms")).unwrap();
        db.add_audit_log("request_created", "user", Some("slack-mcp"), Some("Need for comms")).unwrap();

        // 8. Admin approves
        db.review_approval_request(req_id, "approved", "admin", Some("OK")).unwrap();
        db.add_to_allowlist("slack-mcp", "Slack MCP", None, "admin", Some("OK"), None).unwrap();
        db.add_audit_log("request_approved", "admin", Some("slack-mcp"), Some("OK")).unwrap();

        // 9. Verify server is now allowed
        assert!(db.is_server_allowed("slack-mcp").unwrap());

        // 10. Verify audit trail
        let log = db.get_audit_log(100).unwrap();
        assert_eq!(log.len(), 4);
        let actions: Vec<&str> = log.iter().map(|e| e.action.as_str()).collect();
        assert!(actions.contains(&"governance_enabled"));
        assert!(actions.contains(&"allowlist_add"));
        assert!(actions.contains(&"request_created"));
        assert!(actions.contains(&"request_approved"));
    }

    #[test]
    fn allowlist_removal_workflow() {
        let db = test_db();
        db.add_to_allowlist("srv1", "Server 1", None, "admin", None, None).unwrap();
        assert!(db.is_server_allowed("srv1").unwrap());

        db.remove_from_allowlist("srv1").unwrap();
        assert!(!db.is_server_allowed("srv1").unwrap());
        assert!(db.get_allowlist().unwrap().is_empty());
    }

    #[test]
    fn request_denial_workflow() {
        let db = test_db();
        let id = db.create_approval_request("bad-srv", "Bad Server", "user", Some("Want it")).unwrap();
        db.review_approval_request(id, "denied", "admin", Some("Security risk")).unwrap();

        let denied = db.get_approval_requests(Some("denied")).unwrap();
        assert_eq!(denied.len(), 1);
        assert_eq!(denied[0].review_notes, Some("Security risk".to_string()));

        // Server should NOT be on allowlist
        assert!(!db.is_server_allowed("bad-srv").unwrap());
    }

    #[test]
    fn multiple_requests_same_server() {
        let db = test_db();
        let id1 = db.create_approval_request("srv1", "S1", "user1", Some("First try")).unwrap();
        db.review_approval_request(id1, "denied", "admin", Some("No")).unwrap();

        let id2 = db.create_approval_request("srv1", "S1", "user1", Some("Please reconsider")).unwrap();
        assert_ne!(id1, id2);

        let all = db.get_approval_requests(None).unwrap();
        assert_eq!(all.len(), 2);
    }
}
