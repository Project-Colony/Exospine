use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::db::Database;

/// A task created from an email.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub mail_id: String,
    pub due_date: String,
    pub completed: bool,
    pub created_at: String,
}

impl Database {
    /// Create a task, optionally linked to a mail.
    pub fn create_task(
        &self,
        title: &str,
        description: &str,
        mail_id: &str,
        due_date: &str,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.conn().execute(
            "INSERT INTO tasks (id, title, description, mail_id, due_date, completed, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![id, title, description, mail_id, due_date, now],
        )?;
        Ok(id)
    }

    /// Get all tasks, ordered by completed ASC then due_date ASC.
    pub fn get_tasks(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, title, description, mail_id, due_date, completed, created_at
             FROM tasks
             ORDER BY completed ASC, due_date ASC, created_at DESC",
        )?;
        let results = stmt
            .query_map([], |row| {
                Ok(Task {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    mail_id: row.get(3)?,
                    due_date: row.get(4)?,
                    completed: row.get::<_, i32>(5)? != 0,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(results)
    }

    /// Mark a task as completed.
    pub fn complete_task(&self, id: &str) -> Result<()> {
        self.conn()
            .execute("UPDATE tasks SET completed = 1 WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Delete a task.
    pub fn delete_task(&self, id: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        Ok(())
    }
}
