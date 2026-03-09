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
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFilterEntry {
    pub tool_name: String,
    pub enabled: bool,
    pub token_estimate: u32,
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
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO proxy_servers (server_id, display_name, auth_type, upstream_url, upstream_command, upstream_args, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            rusqlite::params![server_id, display_name, auth_type, upstream_url, upstream_command, upstream_args],
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
                "SELECT server_id, display_name, auth_type, upstream_url, upstream_command, upstream_args, created_at, updated_at
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
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
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
            "SELECT server_id, display_name, auth_type, upstream_url, upstream_command, upstream_args, created_at, updated_at
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
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
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
        tool_name: &str,
        enabled: bool,
        token_estimate: u32,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO proxy_tool_filter (server_id, tool_name, enabled, token_estimate)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![server_id, tool_name, enabled as i32, token_estimate],
        )
        .map_err(|e| format!("Failed to set tool filter: {}", e))?;
        Ok(())
    }

    pub fn set_tool_filter_bulk(
        &self,
        server_id: &str,
        enabled_tools: &[String],
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        // Disable all tools for this server first
        conn.execute(
            "UPDATE proxy_tool_filter SET enabled = 0 WHERE server_id = ?1",
            rusqlite::params![server_id],
        )
        .map_err(|e| format!("Failed to disable tools: {}", e))?;

        // Enable the specified tools
        for tool in enabled_tools {
            conn.execute(
                "UPDATE proxy_tool_filter SET enabled = 1 WHERE server_id = ?1 AND tool_name = ?2",
                rusqlite::params![server_id, tool],
            )
            .map_err(|e| format!("Failed to enable tool {}: {}", tool, e))?;
        }
        Ok(())
    }

    pub fn get_tool_filter(&self, server_id: &str) -> Result<Vec<ToolFilterEntry>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT tool_name, enabled, token_estimate FROM proxy_tool_filter
                 WHERE server_id = ?1 ORDER BY tool_name",
            )
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt
            .query_map(rusqlite::params![server_id], |row| {
                Ok(ToolFilterEntry {
                    tool_name: row.get(0)?,
                    enabled: row.get::<_, i32>(1)? != 0,
                    token_estimate: row.get(2)?,
                })
            })
            .map_err(|e| format!("Query failed: {}", e))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(result)
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

        // Also ensure tool filter entry exists (default: enabled)
        conn.execute(
            "INSERT OR IGNORE INTO proxy_tool_filter (server_id, tool_name, enabled, token_estimate)
             VALUES (?1, ?2, 1, ?3)",
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
}
