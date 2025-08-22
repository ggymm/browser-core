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
    pub id: Option<i64>, // None表示创建，Some表示更新或查询结果
    pub sort: i64,
    pub folder: i64,
    pub parent: i64,
    pub url: String,
    pub name: String,
    pub icon: String,
    pub date: i64,
}

/// 书签查询请求结构
#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkQuery {
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
                    id: Some(row.get(0)?), // 查询结果总是有 id
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
            Some(bookmark_row) => {
                let bookmark = bookmark_row?;
                Ok(Some(bookmark))
            }
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
pub fn save_bookmark(bookmark: Bookmark) -> Result<i64, Error> {
    execute_transaction(connection(), |conn| {
        if let Some(id) = bookmark.id {
            // 更新操作
            let sql = Query::update()
                .table(BookmarkTable::Table)
                .values([
                    (BookmarkTable::Sort, bookmark.sort.into()),
                    (BookmarkTable::Folder, bookmark.folder.into()),
                    (BookmarkTable::Parent, bookmark.parent.into()),
                    (BookmarkTable::Url, bookmark.url.clone().into()),
                    (BookmarkTable::Name, bookmark.name.clone().into()),
                    (BookmarkTable::Icon, bookmark.icon.clone().into()),
                    (BookmarkTable::Date, bookmark.date.into()),
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
                    bookmark.sort.into(),
                    bookmark.folder.into(),
                    bookmark.parent.into(),
                    bookmark.url.into(),
                    bookmark.name.into(),
                    bookmark.icon.into(),
                    bookmark.date.into(),
                ])
                .to_string(SqliteQueryBuilder);

            // 执行
            conn.execute(&sql, []).expect("Failed to execute create");
            Ok(conn.last_insert_rowid())
        }
    })
}

/// 查询书签列表
pub fn query_bookmark(req: BookmarkQuery) -> Result<Vec<Bookmark>, Error> {
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
                    id: Some(row.get(0)?), // 查询结果总是有 id
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
        let bookmark = Bookmark {
            id: None, // 创建时 id 为 None
            sort: 1,
            folder: 0,
            parent: 0,
            url: "Main Bookmark".to_string(),
            name: "Main Bookmark".to_string(),
            icon: "".to_string(),
            date: 1234567890,
        };
        let bookmark_id = save_bookmark(bookmark.clone()).unwrap();

        // 验证创建数据
        assert!(bookmark_id > 0, "Create bookmark Failed");

        // 获取数据
        let retrieved_bookmark = get_bookmark(GetReq { id: bookmark_id })
            .unwrap()
            .expect("Bookmark must exist after create");

        // 验证获取数据
        assert_eq!(retrieved_bookmark.id, Some(bookmark_id));
        assert_eq!(retrieved_bookmark.url, bookmark.url);
        assert_eq!(retrieved_bookmark.name, bookmark.name);
        assert_eq!(retrieved_bookmark.sort, bookmark.sort);

        // 更新数据
        let updated_data = Bookmark {
            id: Some(bookmark_id), // 更新时 id 为 Some
            sort: bookmark.sort,
            folder: bookmark.folder,
            parent: bookmark.parent,
            url: "Updated Bookmark".to_string(),
            name: "Updated Bookmark".to_string(),
            icon: bookmark.icon,
            date: bookmark.date,
        };
        save_bookmark(updated_data).unwrap();
        let updated_bookmark = get_bookmark(GetReq { id: bookmark_id })
            .unwrap()
            .expect("Bookmark must exist after update");

        // 验证更新数据
        assert_eq!(updated_bookmark.url, "Updated Bookmark");
        assert_eq!(updated_bookmark.name, "Updated Bookmark");
    }
}
