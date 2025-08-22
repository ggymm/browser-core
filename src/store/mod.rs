use anyhow::Error;
use napi_derive::napi;
use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, OnceLock};

pub mod bookmark;
pub mod download;
pub mod favicon;
pub mod history;

pub use bookmark::*;
pub use download::*;
pub use history::*;

// 通用的请求参数结构

/// 通用的获取请求结构
#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetReq {
    pub id: i64,
}

/// 通用的删除请求结构（可扩展额外字段）
#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteReq {
    pub id: i64,
    pub force: Option<bool>,   // 强制删除标志
    pub cascade: Option<bool>, // 级联删除标志
}

// 具体业务模块的数据结构

/// 历史记录数据结构（不包含id）
#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryData {
    pub url: String,
    pub title: String,
    pub favicon: Option<String>,
    pub visit_time: i64,
}

/// 历史记录数据操作请求结构
#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryDataReq {
    pub id: Option<i64>,
    pub data: HistoryData,
}

/// 历史记录查询请求结构
#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryQueryReq {
    // 查询过滤条件
    pub url: Option<String>,
    pub title: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    // 分页和排序
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub order_by: Option<String>,
    pub order_desc: Option<bool>,
}

/// 下载记录数据结构（不包含id）
#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadData {
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

/// 下载记录数据操作请求结构
#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadDataReq {
    pub id: Option<i64>,
    pub data: DownloadData,
}

/// 下载记录查询请求结构
#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadQueryReq {
    // 查询过滤条件
    pub url: Option<String>,
    pub file_name: Option<String>,
    pub status: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    // 分页和排序
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub order_by: Option<String>,
    pub order_desc: Option<bool>,
}

// 基础路径的全局存储
static BASE_PATH: OnceLock<String> = OnceLock::new();
pub fn base_path() -> Option<&'static str> {
    BASE_PATH.get().map(|s| s.as_str())
}

pub fn init(db_path: &str) -> Result<(), Error> {
    BASE_PATH
        .set(db_path.to_string())
        .map_err(|_| anyhow::anyhow!("BASE_PATH already initialized"))?;

    init_history_database()?;
    init_bookmark_database()?;
    init_download_database()?;
    Ok(())
}

pub fn open_conn(db_path: &str) -> Result<Arc<Mutex<Connection>>, Error> {
    let conn = Connection::open(db_path)?;
    Ok(Arc::new(Mutex::new(conn)))
}

pub fn query_simple<F, R>(conn: &Arc<Mutex<Connection>>, query: F) -> Result<R, Error>
where
    F: FnOnce(&Connection) -> Result<R, Error>,
{
    let conn = conn.lock().unwrap();
    query(&conn)
}

pub fn execute_simple<F, R>(conn: &Arc<Mutex<Connection>>, operation: F) -> Result<R, Error>
where
    F: FnOnce(&Connection) -> Result<R, Error>,
{
    let conn = conn.lock().unwrap();
    operation(&conn)
}

pub fn execute_transaction<F, R>(conn: &Arc<Mutex<Connection>>, operation: F) -> Result<R, Error>
where
    F: FnOnce(&Connection) -> Result<R, Error>,
{
    let conn = conn.lock().unwrap();
    let tx = conn.unchecked_transaction()?;

    match operation(&conn) {
        Ok(result) => {
            tx.commit()?;
            Ok(result)
        }
        Err(e) => {
            tx.rollback()?;
            Err(e)
        }
    }
}
