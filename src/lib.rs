#![deny(clippy::all)]

use napi_derive::napi;

mod store;

#[napi]
pub fn store_init(db_path: String) -> Result<String, napi::Error> {
    match store::init(&db_path) {
        Ok(_) => Ok("store initialized successfully".to_string()),
        Err(e) => Err(napi::Error::from_reason(format!("Failed to initialize store: {}", e))),
    }
}

// 书签管理相关函数导出

/// 获取书签
#[napi]
pub fn get_bookmark(req: store::GetReq) -> Result<Option<store::Bookmark>, napi::Error> {
    match store::get_bookmark(req) {
        Ok(result) => Ok(result),
        Err(e) => Err(napi::Error::from_reason(format!("Failed to get bookmark: {}", e))),
    }
}

/// 删除书签
#[napi]
pub fn delete_bookmark(req: store::DeleteReq) -> Result<String, napi::Error> {
    match store::delete_bookmark(req) {
        Ok(_) => Ok("bookmark deleted successfully".to_string()),
        Err(e) => Err(napi::Error::from_reason(format!("Failed to delete bookmark: {}", e))),
    }
}

/// 保存书签（创建或更新）
#[napi]
pub fn save_bookmark(bookmark: store::Bookmark) -> Result<f64, napi::Error> {
    match store::save_bookmark(bookmark) {
        Ok(id) => Ok(id as f64), // JavaScript 使用 number 类型，转换为 f64
        Err(e) => Err(napi::Error::from_reason(format!("Failed to save bookmark: {}", e))),
    }
}

/// 查询书签列表
#[napi]
pub fn query_bookmark(req: store::BookmarkQuery) -> Result<Vec<store::Bookmark>, napi::Error> {
    match store::query_bookmark(req) {
        Ok(result) => Ok(result),
        Err(e) => Err(napi::Error::from_reason(format!("Failed to query bookmarks: {}", e))),
    }
}

// 历史记录管理相关函数导出

/// 保存历史记录（创建或更新）
#[napi]
pub fn save_history(history: store::History) -> Result<f64, napi::Error> {
    match store::save_history(history) {
        Ok(id) => Ok(id as f64), // JavaScript 使用 number 类型，转换为 f64
        Err(e) => Err(napi::Error::from_reason(format!("Failed to save history: {}", e))),
    }
}
