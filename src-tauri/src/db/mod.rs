pub mod migrations;
pub mod queries;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn new() -> Result<Self, String> {
        let db_path = Self::db_path()?;

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create DB directory: {}", e))?;
        }

        let conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        // WAL mode allows concurrent reads from GUI and daemon processes
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| format!("Failed to set WAL mode: {}", e))?;

        migrations::run_migrations(&conn)?;

        Ok(Database {
            conn: Mutex::new(conn),
        })
    }

    /// Create an in-memory database for testing.
    pub fn new_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory()
            .map_err(|e| format!("Failed to open in-memory database: {}", e))?;
        migrations::run_migrations(&conn)?;
        Ok(Database {
            conn: Mutex::new(conn),
        })
    }

    fn db_path() -> Result<PathBuf, String> {
        let data_dir = dirs::data_dir()
            .ok_or("Could not determine data directory")?;
        Ok(data_dir.join("com.brightwing.mcp-manager").join("brightwing.db"))
    }
}
