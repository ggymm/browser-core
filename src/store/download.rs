use anyhow::Error;
use napi_derive::napi;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use crate::store::{base_path, execute_simple, execute_transaction, open_conn};

// 模块级别的数据库连接
static DOWNLOAD_CONNECTION: OnceLock<Arc<Mutex<Connection>>> = OnceLock::new();

/// 获取下载数据库连接
fn connection() -> &'static Arc<Mutex<Connection>> {
    DOWNLOAD_CONNECTION.get_or_init(|| {
        let base_path = base_path().unwrap_or("");
        let database_path = PathBuf::from(base_path).join("download.db");
        open_conn(database_path.to_str().unwrap()).expect("Failed to create download database connection")
    })
}

#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub id: i64,
    pub url: String,
    pub file_name: String,
    pub file_path: String,
    pub file_size: i64,
    pub downloaded_size: i64,
    pub status: String,
    pub start_time: i64,
    pub end_time: Option<i64>,
    pub mime_type: Option<String>,
}

/// 初始化下载记录数据库
pub fn init_download_database() -> Result<(), Error> {
    execute_simple(connection(), |conn| {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS download (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                file_name TEXT NOT NULL,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL DEFAULT 0,
                downloaded_size INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                start_time INTEGER NOT NULL,
                end_time INTEGER,
                mime_type TEXT
            )",
            [],
        )?;

        // 创建索引
        conn.execute("CREATE INDEX IF NOT EXISTS idx_download_status ON download(status)", [])?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_download_start_time ON download(start_time)",
            [],
        )?;

        Ok(())
    })
}

/// 添加下载记录
pub fn add_download(download: Download) -> Result<i64, Error> {
    execute_transaction(connection(), |conn| {
        let mut stmt = conn.prepare(
            "INSERT INTO download (url, file_name, file_path, file_size, downloaded_size, status, start_time, end_time, mime_type) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;

        stmt.execute([
            &download.url,
            &download.file_name,
            &download.file_path,
            &download.file_size.to_string(),
            &download.downloaded_size.to_string(),
            &download.status,
            &download.start_time.to_string(),
            &download.end_time.map(|t| t.to_string()).unwrap_or_default(),
            &download.mime_type.unwrap_or_default(),
        ])?;

        Ok(conn.last_insert_rowid())
    })
}

/// 更新下载记录
pub fn update_download(download: Download) -> Result<(), Error> {
    execute_transaction(connection(), |conn| {
        let mut stmt = conn.prepare(
            "UPDATE download SET url = ?1, file_name = ?2, file_path = ?3, file_size = ?4, 
             downloaded_size = ?5, status = ?6, start_time = ?7, end_time = ?8, mime_type = ?9 
             WHERE id = ?10",
        )?;

        stmt.execute([
            &download.url,
            &download.file_name,
            &download.file_path,
            &download.file_size.to_string(),
            &download.downloaded_size.to_string(),
            &download.status,
            &download.start_time.to_string(),
            &download.end_time.map(|t| t.to_string()).unwrap_or_default(),
            &download.mime_type.unwrap_or_default(),
            &download.id.to_string(),
        ])?;

        Ok(())
    })
}

/// 更新下载状态
pub fn update_download_status(id: i64, status: String) -> Result<(), Error> {
    execute_simple(connection(), |conn| {
        let mut stmt = conn.prepare("UPDATE download SET status = ?1 WHERE id = ?2")?;
        stmt.execute([&status, &id.to_string()])?;
        Ok(())
    })
}

/// 更新下载大小
pub fn update_download_size(id: i64, downloaded_size: i64) -> Result<(), Error> {
    execute_simple(connection(), |conn| {
        let mut stmt = conn.prepare("UPDATE download SET downloaded_size = ?1 WHERE id = ?2")?;
        stmt.execute([&downloaded_size.to_string(), &id.to_string()])?;
        Ok(())
    })
}

/// 根据ID删除下载记录
pub fn delete_download_by_id(id: i64) -> Result<(), Error> {
    execute_simple(connection(), |conn| {
        let mut stmt = conn.prepare("DELETE FROM download WHERE id = ?1")?;
        stmt.execute([&id.to_string()])?;
        Ok(())
    })
}

/// 清空所有下载记录
pub fn clear_all_downloads() -> Result<(), Error> {
    execute_simple(connection(), |conn| {
        conn.execute("DELETE FROM download", [])?;
        Ok(())
    })
}

/// 根据ID获取下载记录
pub fn get_download_by_id(id: i64) -> Result<Option<Download>, Error> {
    execute_simple(connection(), |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, url, file_name, file_path, file_size, downloaded_size, status, start_time, end_time, mime_type 
             FROM download WHERE id = ?1",
        )?;

        let mut download_iter = stmt.query_map([&id.to_string()], |row| {
            Ok(Download {
                id: row.get(0)?,
                url: row.get(1)?,
                file_name: row.get(2)?,
                file_path: row.get(3)?,
                file_size: row.get(4)?,
                downloaded_size: row.get(5)?,
                status: row.get(6)?,
                start_time: row.get(7)?,
                end_time: {
                    let end_time_str: String = row.get(8)?;
                    if end_time_str.is_empty() {
                        None
                    } else {
                        Some(end_time_str.parse().unwrap_or(0))
                    }
                },
                mime_type: {
                    let mime_str: String = row.get(9)?;
                    if mime_str.is_empty() {
                        None
                    } else {
                        Some(mime_str)
                    }
                },
            })
        })?;

        match download_iter.next() {
            Some(download) => Ok(Some(download?)),
            None => Ok(None),
        }
    })
}

/// 分页获取下载记录
pub fn get_downloads_paginated(page: i32, page_size: i32) -> Result<Vec<Download>, Error> {
    execute_simple(connection(), |conn| {
        let offset = (page - 1) * page_size;
        let mut stmt = conn.prepare(
            "SELECT id, url, file_name, file_path, file_size, downloaded_size, status, start_time, end_time, mime_type 
             FROM download ORDER BY start_time DESC LIMIT ?1 OFFSET ?2",
        )?;

        let download_iter = stmt.query_map([&page_size.to_string(), &offset.to_string()], |row| {
            Ok(Download {
                id: row.get(0)?,
                url: row.get(1)?,
                file_name: row.get(2)?,
                file_path: row.get(3)?,
                file_size: row.get(4)?,
                downloaded_size: row.get(5)?,
                status: row.get(6)?,
                start_time: row.get(7)?,
                end_time: {
                    let end_time_str: String = row.get(8)?;
                    if end_time_str.is_empty() {
                        None
                    } else {
                        Some(end_time_str.parse().unwrap_or(0))
                    }
                },
                mime_type: {
                    let mime_str: String = row.get(9)?;
                    if mime_str.is_empty() {
                        None
                    } else {
                        Some(mime_str)
                    }
                },
            })
        })?;

        let mut downloads = Vec::new();
        for download in download_iter {
            downloads.push(download?);
        }

        Ok(downloads)
    })
}

/// 根据状态获取下载记录
pub fn get_downloads_by_status(status: String) -> Result<Vec<Download>, Error> {
    execute_simple(connection(), |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, url, file_name, file_path, file_size, downloaded_size, status, start_time, end_time, mime_type 
             FROM download WHERE status = ?1 ORDER BY start_time DESC",
        )?;

        let download_iter = stmt.query_map([&status], |row| {
            Ok(Download {
                id: row.get(0)?,
                url: row.get(1)?,
                file_name: row.get(2)?,
                file_path: row.get(3)?,
                file_size: row.get(4)?,
                downloaded_size: row.get(5)?,
                status: row.get(6)?,
                start_time: row.get(7)?,
                end_time: {
                    let end_time_str: String = row.get(8)?;
                    if end_time_str.is_empty() {
                        None
                    } else {
                        Some(end_time_str.parse().unwrap_or(0))
                    }
                },
                mime_type: {
                    let mime_str: String = row.get(9)?;
                    if mime_str.is_empty() {
                        None
                    } else {
                        Some(mime_str)
                    }
                },
            })
        })?;

        let mut downloads = Vec::new();
        for download in download_iter {
            downloads.push(download?);
        }

        Ok(downloads)
    })
}

/// 搜索下载记录
pub fn search_downloads(keyword: String, limit: Option<i32>) -> Result<Vec<Download>, Error> {
    execute_simple(connection(), |conn| {
        let search_pattern = format!("%{}%", keyword);

        let sql = if let Some(limit) = limit {
            format!(
                "SELECT id, url, file_name, file_path, file_size, downloaded_size, status, start_time, end_time, mime_type 
                 FROM download WHERE file_name LIKE ?1 OR url LIKE ?1 
                 ORDER BY start_time DESC LIMIT {}",
                limit
            )
        } else {
            "SELECT id, url, file_name, file_path, file_size, downloaded_size, status, start_time, end_time, mime_type 
             FROM download WHERE file_name LIKE ?1 OR url LIKE ?1 
             ORDER BY start_time DESC"
                .to_string()
        };

        let mut stmt = conn.prepare(&sql)?;
        let download_iter = stmt.query_map([&search_pattern], |row| {
            Ok(Download {
                id: row.get(0)?,
                url: row.get(1)?,
                file_name: row.get(2)?,
                file_path: row.get(3)?,
                file_size: row.get(4)?,
                downloaded_size: row.get(5)?,
                status: row.get(6)?,
                start_time: row.get(7)?,
                end_time: {
                    let end_time_str: String = row.get(8)?;
                    if end_time_str.is_empty() {
                        None
                    } else {
                        Some(end_time_str.parse().unwrap_or(0))
                    }
                },
                mime_type: {
                    let mime_str: String = row.get(9)?;
                    if mime_str.is_empty() {
                        None
                    } else {
                        Some(mime_str)
                    }
                },
            })
        })?;

        let mut downloads = Vec::new();
        for download in download_iter {
            downloads.push(download?);
        }

        Ok(downloads)
    })
}

/// 获取下载记录总数
pub fn get_download_count() -> Result<i64, Error> {
    execute_simple(connection(), |conn| {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM download")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    })
}

/// 获取活跃下载（正在下载的记录）
pub fn get_active_downloads() -> Result<Vec<Download>, Error> {
    execute_simple(connection(), |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, url, file_name, file_path, file_size, downloaded_size, status, start_time, end_time, mime_type 
             FROM download WHERE status IN ('downloading', 'pending') ORDER BY start_time DESC",
        )?;

        let download_iter = stmt.query_map([], |row| {
            Ok(Download {
                id: row.get(0)?,
                url: row.get(1)?,
                file_name: row.get(2)?,
                file_path: row.get(3)?,
                file_size: row.get(4)?,
                downloaded_size: row.get(5)?,
                status: row.get(6)?,
                start_time: row.get(7)?,
                end_time: {
                    let end_time_str: String = row.get(8)?;
                    if end_time_str.is_empty() {
                        None
                    } else {
                        Some(end_time_str.parse().unwrap_or(0))
                    }
                },
                mime_type: {
                    let mime_str: String = row.get(9)?;
                    if mime_str.is_empty() {
                        None
                    } else {
                        Some(mime_str)
                    }
                },
            })
        })?;

        let mut downloads = Vec::new();
        for download in download_iter {
            downloads.push(download?);
        }

        Ok(downloads)
    })
}
