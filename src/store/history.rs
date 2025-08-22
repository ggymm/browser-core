use anyhow::Error;
use napi_derive::napi;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use crate::store::{base_path, execute_simple, execute_transaction, open_conn};

// 模块级别的数据库连接
static HISTORY_CONNECTION: OnceLock<Arc<Mutex<Connection>>> = OnceLock::new();

/// 获取历史数据库连接
fn connection() -> &'static Arc<Mutex<Connection>> {
    HISTORY_CONNECTION.get_or_init(|| {
        let base_path = base_path().unwrap_or("");
        let database_path = PathBuf::from(base_path).join("history.db");
        open_conn(database_path.to_str().unwrap()).expect("Failed to create history database connection")
    })
}

#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct History {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub favicon: Option<String>,
    pub visit_time: i64,
}

/// 初始化历史记录数据库
pub fn init_history_database() -> Result<(), Error> {
    execute_simple(connection(), |conn| {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                title TEXT NOT NULL,
                favicon TEXT,
                visit_time INTEGER NOT NULL
            )",
            [],
        )?;

        // 创建索引
        conn.execute("CREATE INDEX IF NOT EXISTS idx_history_url ON history(url)", [])?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_history_visit_time ON history(visit_time)",
            [],
        )?;

        Ok(())
    })
}

/// 添加历史记录
pub fn add_history(history: History) -> Result<i64, Error> {
    execute_transaction(connection(), |conn| {
        let mut stmt = conn.prepare(
            "INSERT INTO history (url, title, favicon, visit_time) 
             VALUES (?1, ?2, ?3, ?4)",
        )?;

        stmt.execute([
            &history.url,
            &history.title,
            &history.favicon.unwrap_or_default(),
            &history.visit_time.to_string(),
        ])?;

        Ok(conn.last_insert_rowid())
    })
}

/// 更新历史记录
pub fn update_history(history: History) -> Result<(), Error> {
    execute_transaction(connection(), |conn| {
        let mut stmt = conn.prepare(
            "UPDATE history SET url = ?1, title = ?2, favicon = ?3, visit_time = ?4 
             WHERE id = ?5",
        )?;

        stmt.execute([
            &history.url,
            &history.title,
            &history.favicon.unwrap_or_default(),
            &history.visit_time.to_string(),
            &history.id.to_string(),
        ])?;

        Ok(())
    })
}

/// 根据ID删除历史记录
pub fn delete_history_by_id(id: i64) -> Result<(), Error> {
    execute_simple(connection(), |conn| {
        let mut stmt = conn.prepare("DELETE FROM history WHERE id = ?1")?;
        stmt.execute([&id.to_string()])?;
        Ok(())
    })
}

/// 根据URL删除历史记录
pub fn delete_history_by_url(url: String) -> Result<(), Error> {
    execute_simple(connection(), |conn| {
        let mut stmt = conn.prepare("DELETE FROM history WHERE url = ?1")?;
        stmt.execute([&url])?;
        Ok(())
    })
}

/// 清空所有历史记录
pub fn clear_all_history() -> Result<(), Error> {
    execute_simple(connection(), |conn| {
        conn.execute("DELETE FROM history", [])?;
        Ok(())
    })
}

/// 根据ID获取历史记录
pub fn get_history_by_id(id: i64) -> Result<Option<History>, Error> {
    execute_simple(connection(), |conn| {
        let mut stmt = conn.prepare("SELECT id, url, title, favicon, visit_time FROM history WHERE id = ?1")?;

        let mut history_iter = stmt.query_map([&id.to_string()], |row| {
            Ok(History {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                favicon: row.get(3)?,
                visit_time: row.get(4)?,
            })
        })?;

        match history_iter.next() {
            Some(history) => Ok(Some(history?)),
            None => Ok(None),
        }
    })
}

/// 根据URL获取历史记录
pub fn get_history_by_url(url: String) -> Result<Vec<History>, Error> {
    execute_simple(connection(), |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, url, title, favicon, visit_time FROM history 
             WHERE url = ?1 ORDER BY visit_time DESC",
        )?;

        let history_iter = stmt.query_map([&url], |row| {
            Ok(History {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                favicon: row.get(3)?,
                visit_time: row.get(4)?,
            })
        })?;

        let mut histories = Vec::new();
        for history in history_iter {
            histories.push(history?);
        }

        Ok(histories)
    })
}

/// 分页获取历史记录
pub fn get_history_paginated(page: i32, page_size: i32) -> Result<Vec<History>, Error> {
    execute_simple(connection(), |conn| {
        let offset = (page - 1) * page_size;
        let mut stmt = conn.prepare(
            "SELECT id, url, title, favicon, visit_time FROM history 
             ORDER BY visit_time DESC LIMIT ?1 OFFSET ?2",
        )?;

        let history_iter = stmt.query_map([&page_size.to_string(), &offset.to_string()], |row| {
            Ok(History {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                favicon: row.get(3)?,
                visit_time: row.get(4)?,
            })
        })?;

        let mut histories = Vec::new();
        for history in history_iter {
            histories.push(history?);
        }

        Ok(histories)
    })
}

/// 搜索历史记录
pub fn search_history(keyword: String, limit: Option<i32>) -> Result<Vec<History>, Error> {
    execute_simple(connection(), |conn| {
        let search_pattern = format!("%{}%", keyword);

        let sql = if let Some(limit) = limit {
            format!(
                "SELECT id, url, title, favicon, visit_time FROM history 
                 WHERE title LIKE ?1 OR url LIKE ?1 
                 ORDER BY visit_time DESC LIMIT {}",
                limit
            )
        } else {
            "SELECT id, url, title, favicon, visit_time FROM history 
             WHERE title LIKE ?1 OR url LIKE ?1 
             ORDER BY visit_time DESC"
                .to_string()
        };

        let mut stmt = conn.prepare(&sql)?;
        let history_iter = stmt.query_map([&search_pattern], |row| {
            Ok(History {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                favicon: row.get(3)?,
                visit_time: row.get(4)?,
            })
        })?;

        let mut histories = Vec::new();
        for history in history_iter {
            histories.push(history?);
        }

        Ok(histories)
    })
}

/// 获取历史记录总数
pub fn get_history_count() -> Result<i64, Error> {
    execute_simple(connection(), |conn| {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM history")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    })
}

/// 获取最近访问的记录
pub fn get_recent_history(limit: i32) -> Result<Vec<History>, Error> {
    execute_simple(connection(), |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, url, title, favicon, visit_time FROM history 
             ORDER BY visit_time DESC LIMIT ?1",
        )?;

        let history_iter = stmt.query_map([&limit.to_string()], |row| {
            Ok(History {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                favicon: row.get(3)?,
                visit_time: row.get(4)?,
            })
        })?;

        let mut histories = Vec::new();
        for history in history_iter {
            histories.push(history?);
        }

        Ok(histories)
    })
}
