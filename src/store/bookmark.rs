use anyhow::Error;
use napi_derive::napi;
use rusqlite::Connection;
use sea_query::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use crate::store::{base_path, execute_simple, execute_transaction, open_conn, DeleteReq, GetReq};

// 模块级别的数据库连接
static BOOKMARK_CONNECTION: OnceLock<Arc<Mutex<Connection>>> = OnceLock::new();

/// 获取书签数据库连接
fn connection() -> &'static Arc<Mutex<Connection>> {
    BOOKMARK_CONNECTION.get_or_init(|| {
        let base_path = base_path().unwrap_or("");
        let database_path = PathBuf::from(base_path).join("bookmark.db");
        open_conn(database_path.to_str().unwrap()).expect("Failed to create bookmark database connection")
    })
}

#[derive(Iden)]
enum BookmarkTable {
    Table,
    Id,
    Sort,
    Folder,
    Parent,
    Url,
    Name,
    Icon,
    Date,
}

#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: i64,
    pub sort: i64,
    pub folder: i64,
    pub parent: i64,
    pub url: String,
    pub name: String,
    pub icon: String,
    pub date: i64,
}

/// 书签数据结构（不包含id，用于创建和更新）
#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkData {
    pub sort: i64,
    pub folder: i64,
    pub parent: i64,
    pub url: String,
    pub name: String,
    pub icon: String,
    pub date: i64,
}

/// 书签数据操作请求结构（统一Create和Update）
#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkDataReq {
    pub id: Option<i64>, // None表示创建，Some表示更新
    pub data: BookmarkData,
}

/// 书签查询请求结构
#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkQueryReq {
    // 查询过滤条件
    pub url: Option<String>,
    pub name: Option<String>,
    pub folder: Option<i64>,
    pub parent: Option<i64>,
}

/// 初始化表
pub fn init_bookmark_database() -> Result<(), Error> {
    execute_simple(connection(), |conn| {
        let sql = Table::create()
            .table(BookmarkTable::Table)
            .if_not_exists()
            .col(
                ColumnDef::new(BookmarkTable::Id)
                    .integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            )
            .col(ColumnDef::new(BookmarkTable::Sort).integer().not_null().default(0))
            .col(ColumnDef::new(BookmarkTable::Folder).integer().not_null().default(0))
            .col(ColumnDef::new(BookmarkTable::Parent).integer().not_null().default(0))
            .col(ColumnDef::new(BookmarkTable::Url).text().not_null())
            .col(ColumnDef::new(BookmarkTable::Name).text().not_null())
            .col(ColumnDef::new(BookmarkTable::Icon).text().not_null())
            .col(ColumnDef::new(BookmarkTable::Date).integer().not_null())
            .to_string(SqliteQueryBuilder);

        // 执行
        conn.execute(&sql, [])?;
        Ok(())
    })
}

/// 获取书签
pub fn get_bookmark(req: GetReq) -> Result<Option<Bookmark>, Error> {
    execute_simple(connection(), |conn| {
        let sql = Query::select()
            .columns([
                BookmarkTable::Id,
                BookmarkTable::Sort,
                BookmarkTable::Folder,
                BookmarkTable::Parent,
                BookmarkTable::Url,
                BookmarkTable::Name,
                BookmarkTable::Icon,
                BookmarkTable::Date,
            ])
            .from(BookmarkTable::Table)
            .and_where(Expr::col(BookmarkTable::Id).eq(req.id))
            .to_string(SqliteQueryBuilder);

        // 执行
        let mut stmt = conn.prepare(&sql).expect("Failed to prepare query");
        let mut rows = stmt
            .query_map([], |row| {
                Ok(Bookmark {
                    id: row.get(0)?,
                    sort: row.get(1)?,
                    folder: row.get(2)?,
                    parent: row.get(3)?,
                    url: row.get(4)?,
                    name: row.get(5)?,
                    icon: row.get(6)?,
                    date: row.get(7)?,
                })
            })
            .expect("Failed to execute query");

        match rows.next() {
            Some(bookmark) => Ok(Some(bookmark?)),
            None => Ok(None),
        }
    })
}

/// 删除书签
pub fn delete_bookmark(req: DeleteReq) -> Result<(), Error> {
    execute_transaction(connection(), |conn| {
        let sql = Query::delete()
            .from_table(BookmarkTable::Table)
            .and_where(Expr::col(BookmarkTable::Id).eq(req.id))
            .to_string(SqliteQueryBuilder);

        // 执行
        conn.execute(&sql, []).expect("Failed to execute delete");
        Ok(())
    })
}

/// 创建或更新书签（统一接口）
pub fn save_bookmark(req: BookmarkDataReq) -> Result<i64, Error> {
    execute_transaction(connection(), |conn| {
        if let Some(id) = req.id {
            // 更新操作
            let sql = Query::update()
                .table(BookmarkTable::Table)
                .values([
                    (BookmarkTable::Sort, req.data.sort.into()),
                    (BookmarkTable::Folder, req.data.folder.into()),
                    (BookmarkTable::Parent, req.data.parent.into()),
                    (BookmarkTable::Url, req.data.url.clone().into()),
                    (BookmarkTable::Name, req.data.name.clone().into()),
                    (BookmarkTable::Icon, req.data.icon.clone().into()),
                    (BookmarkTable::Date, req.data.date.into()),
                ])
                .and_where(Expr::col(BookmarkTable::Id).eq(id))
                .to_string(SqliteQueryBuilder);

            // 执行
            conn.execute(&sql, []).expect("Failed to execute update");
            Ok(id)
        } else {
            // 创建操作
            let sql = Query::insert()
                .into_table(BookmarkTable::Table)
                .columns([
                    BookmarkTable::Sort,
                    BookmarkTable::Folder,
                    BookmarkTable::Parent,
                    BookmarkTable::Url,
                    BookmarkTable::Name,
                    BookmarkTable::Icon,
                    BookmarkTable::Date,
                ])
                .values_panic([
                    req.data.sort.into(),
                    req.data.folder.into(),
                    req.data.parent.into(),
                    req.data.url.into(),
                    req.data.name.into(),
                    req.data.icon.into(),
                    req.data.date.into(),
                ])
                .to_string(SqliteQueryBuilder);

            // 执行
            conn.execute(&sql, []).expect("Failed to execute create");
            Ok(conn.last_insert_rowid())
        }
    })
}

/// 查询书签列表
pub fn query_bookmark(req: BookmarkQueryReq) -> Result<Vec<Bookmark>, Error> {
    execute_simple(connection(), |conn| {
        let mut query = Query::select();
        query
            .columns([
                BookmarkTable::Id,
                BookmarkTable::Sort,
                BookmarkTable::Folder,
                BookmarkTable::Parent,
                BookmarkTable::Url,
                BookmarkTable::Name,
                BookmarkTable::Icon,
                BookmarkTable::Date,
            ])
            .from(BookmarkTable::Table);

        // 应用过滤条件
        for (field, column) in [
            (req.folder.as_ref(), BookmarkTable::Folder),
            (req.parent.as_ref(), BookmarkTable::Parent),
        ] {
            if let Some(val) = field {
                query.and_where(Expr::col(column).eq(*val));
            }
        }
        for (field, column) in [
            (req.url.as_ref(), BookmarkTable::Url),
            (req.name.as_ref(), BookmarkTable::Name),
        ] {
            if let Some(val) = field {
                if !val.is_empty() {
                    query.and_where(Expr::col(column).like(format!("%{}%", val)));
                }
            }
        }

        // 应用排序
        query.order_by(BookmarkTable::Sort, Order::Asc);
        query.order_by(BookmarkTable::Name, Order::Asc);

        // 执行
        let sql = query.to_string(SqliteQueryBuilder);
        let mut stmt = conn.prepare(&sql).expect("Failed to prepare query");
        let rows = stmt
            .query_map([], |row| {
                Ok(Bookmark {
                    id: row.get(0)?,
                    sort: row.get(1)?,
                    folder: row.get(2)?,
                    parent: row.get(3)?,
                    url: row.get(4)?,
                    name: row.get(5)?,
                    icon: row.get(6)?,
                    date: row.get(7)?,
                })
            })
            .expect("Failed to execute query");
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::BASE_PATH;

    #[test]
    fn test_bookmark() {
        BASE_PATH
            .set("/tmp/browser-core/database".to_string())
            .expect("Failed to set BASE_PATH");
        // 每次都尝试初始化数据库表（如果已存在会被忽略）
        init_bookmark_database().expect("Failed to initialize database");

        // 创建数据
        let main_data = BookmarkData {
            sort: 1,
            folder: 0,
            parent: 0,
            url: "Main Bookmark".to_string(),
            name: "Main Bookmark".to_string(),
            icon: "".to_string(),
            date: 1234567890,
        };
        let main_id = save_bookmark(BookmarkDataReq {
            id: None,
            data: main_data.clone(),
        })
        .unwrap();

        // 验证创建数据
        assert!(main_id > 0, "Create bookmark Failed");

        // 获取数据
        let retrieved_bookmark = get_bookmark(GetReq { id: main_id })
            .unwrap()
            .expect("Bookmark must exist after create");

        // 验证获取数据
        assert_eq!(retrieved_bookmark.id, main_id);
        assert_eq!(retrieved_bookmark.url, main_data.url);
        assert_eq!(retrieved_bookmark.name, main_data.name);
        assert_eq!(retrieved_bookmark.sort, main_data.sort);

        // 更新数据
        let updated_data = BookmarkData {
            sort: main_data.sort,
            folder: main_data.folder,
            parent: main_data.parent,
            url: "Updated Bookmark".to_string(),
            name: "Updated Bookmark".to_string(),
            icon: main_data.icon,
            date: main_data.date,
        };
        save_bookmark(BookmarkDataReq {
            id: Some(main_id),
            data: updated_data,
        })
        .unwrap();
        let updated_bookmark = get_bookmark(GetReq { id: main_id })
            .unwrap()
            .expect("Bookmark must exist after update");

        // 验证更新数据
        assert_eq!(updated_bookmark.url, "Updated Bookmark");
        assert_eq!(updated_bookmark.name, "Updated Bookmark");
    }
}
