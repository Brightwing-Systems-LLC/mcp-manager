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
}
