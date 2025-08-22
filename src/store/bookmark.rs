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
    // 分页和排序
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub order_by: Option<String>,
    pub order_desc: Option<bool>,
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
        if let Some(order_by) = &req.order_by {
            let order = if req.order_desc.unwrap_or(false) {
                Order::Desc
            } else {
                Order::Asc
            };
            match order_by.as_str() {
                "sort" => query.order_by(BookmarkTable::Sort, order),
                "name" => query.order_by(BookmarkTable::Name, order),
                "date" => query.order_by(BookmarkTable::Date, order),
                _ => query.order_by(BookmarkTable::Sort, Order::Asc),
            };
        } else {
            query.order_by(BookmarkTable::Sort, Order::Asc);
        }

        // 应用分页
        if let (Some(page), Some(page_size)) = (req.page, req.page_size) {
            let offset = (page - 1) * page_size;
            query.limit(page_size as u64).offset(offset as u64);
        }

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

        let mut bookmarks = Vec::new();
        for row in rows {
            bookmarks.push(row?);
        }

        Ok(bookmarks)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::BASE_PATH;

    fn init_bookmark() -> Result<(), Error> {
        use std::sync::Once;
        static INIT: Once = Once::new();

        INIT.call_once(|| {
            // mkdir -p /tmp/browser-core/database
            std::fs::create_dir_all("/tmp/browser-core/database").expect("Failed to create test directory");

            // 只在第一次调用时设置 BASE_PATH
            BASE_PATH
                .set("/tmp/browser-core/database".to_string())
                .expect("Failed to set BASE_PATH");
        });

        // 每次都尝试初始化数据库表（如果已存在会被忽略）
        init_bookmark_database()
    }

    fn entity_bookmark_data() -> BookmarkData {
        BookmarkData {
            sort: 1,
            folder: 0,
            parent: 0,
            url: "https://example.com".to_string(),
            name: "Test Bookmark".to_string(),
            icon: "test-icon".to_string(),
            date: 1234567890,
        }
    }

    fn create_test_bookmark_data(name: &str, url: &str, folder: i64, parent: i64) -> BookmarkData {
        BookmarkData {
            sort: 1,
            folder,
            parent,
            url: url.to_string(),
            name: name.to_string(),
            icon: "test-icon".to_string(),
            date: 1234567890,
        }
    }

    fn create_simple_query() -> BookmarkQueryReq {
        BookmarkQueryReq {
            url: None,
            name: None,
            folder: None,
            parent: None,
            page: None,
            page_size: None,
            order_by: None,
            order_desc: None,
        }
    }

    #[test]
    fn test_init() {
        init_bookmark().expect("Initialization failed");
    }

    #[test]
    fn test_get_bookmark() {
        init_bookmark().expect("Initialization failed");

        let bookmark_data = entity_bookmark_data();
        let bookmark_id = save_bookmark(BookmarkDataReq {
            id: None,
            data: bookmark_data.clone(),
        })
        .unwrap();

        let retrieved_bookmark = get_bookmark(GetReq { id: bookmark_id })
            .unwrap()
            .expect("Bookmark should exist");

        assert_eq!(retrieved_bookmark.id, bookmark_id);
        assert_eq!(retrieved_bookmark.url, bookmark_data.url);
        assert_eq!(retrieved_bookmark.name, bookmark_data.name);
        assert_eq!(retrieved_bookmark.sort, bookmark_data.sort);
    }

    #[test]
    fn test_delete_bookmark() {
        init_bookmark().expect("Initialization failed");

        let bookmark_data = entity_bookmark_data();
        let bookmark_id = save_bookmark(BookmarkDataReq {
            id: None,
            data: bookmark_data,
        })
        .unwrap();

        assert!(get_bookmark(GetReq { id: bookmark_id }).unwrap().is_some());

        delete_bookmark(DeleteReq {
            id: bookmark_id,
            force: None,
            cascade: None,
        })
        .unwrap();

        assert!(get_bookmark(GetReq { id: bookmark_id }).unwrap().is_none());
    }

    #[test]
    fn test_query_bookmarks() {
        init_bookmark().expect("Initialization failed");

        // 创建多个书签用于测试
        let bookmarks_data = [
            ("Bookmark 1", "https://example1.com", 1, 10),
            ("Bookmark 2", "https://example2.com", 1, 10),
            ("Test Bookmark", "https://test.com", 2, 20),
        ];

        for (name, url, folder, parent) in bookmarks_data.iter() {
            let data = create_test_bookmark_data(name, url, *folder, *parent);
            save_bookmark(BookmarkDataReq { id: None, data }).unwrap();
        }

        // 测试按 folder 查询
        let mut query = create_simple_query();
        query.folder = Some(1);
        let bookmarks = query_bookmark(query).unwrap();
        assert!(bookmarks.len() >= 2);
        assert!(bookmarks.iter().any(|b| b.name == "Bookmark 1" && b.folder == 1));
        assert!(bookmarks.iter().any(|b| b.name == "Bookmark 2" && b.folder == 1));

        // 测试按 parent 查询
        let mut query = create_simple_query();
        query.parent = Some(20);
        let bookmarks = query_bookmark(query).unwrap();
        assert!(bookmarks.len() >= 1);
        assert!(bookmarks.iter().any(|b| b.name == "Test Bookmark" && b.parent == 20));

        // 测试组合条件查询
        let mut query = create_simple_query();
        query.folder = Some(1);
        query.parent = Some(10);
        let bookmarks = query_bookmark(query).unwrap();
        assert!(bookmarks.len() >= 2);
        assert!(bookmarks
            .iter()
            .any(|b| b.name == "Bookmark 1" && b.folder == 1 && b.parent == 10));
        assert!(bookmarks
            .iter()
            .any(|b| b.name == "Bookmark 2" && b.folder == 1 && b.parent == 10));

        // 测试按 name 模糊查询
        let mut query = create_simple_query();
        query.name = Some("Test".to_string());
        let bookmarks = query_bookmark(query).unwrap();
        assert!(bookmarks.len() >= 1);
        assert!(bookmarks.iter().any(|b| b.name == "Test Bookmark"));

        // 测试按 url 模糊查询
        let mut query = create_simple_query();
        query.url = Some("example".to_string());
        let bookmarks = query_bookmark(query).unwrap();
        assert!(bookmarks.len() >= 2);
        assert!(bookmarks.iter().any(|b| b.url.contains("example1.com")));
        assert!(bookmarks.iter().any(|b| b.url.contains("example2.com")));

        // 测试查询所有书签
        let query = create_simple_query();
        let bookmarks = query_bookmark(query).unwrap();
        assert!(bookmarks.len() >= 3);
    }

    #[test]
    fn test_create_bookmark() {
        init_bookmark().expect("Initialization failed");

        let bookmark_data = entity_bookmark_data();
        let bookmark_id = save_bookmark(BookmarkDataReq {
            id: None,
            data: bookmark_data,
        })
        .unwrap();

        assert!(bookmark_id > 0);
    }

    #[test]
    fn test_update_bookmark() {
        init_bookmark().expect("Initialization failed");

        let bookmark_data = entity_bookmark_data();
        let bookmark_id = save_bookmark(BookmarkDataReq {
            id: None,
            data: bookmark_data.clone(),
        })
        .unwrap();

        let updated_data = BookmarkData {
            sort: bookmark_data.sort,
            folder: bookmark_data.folder,
            parent: bookmark_data.parent,
            url: "https://updated.com".to_string(),
            name: "Updated Bookmark".to_string(),
            icon: bookmark_data.icon,
            date: bookmark_data.date,
        };

        save_bookmark(BookmarkDataReq {
            id: Some(bookmark_id),
            data: updated_data,
        })
        .unwrap();

        let updated_bookmark = get_bookmark(GetReq { id: bookmark_id })
            .unwrap()
            .expect("Updated bookmark should exist");
        assert_eq!(updated_bookmark.name, "Updated Bookmark");
        assert_eq!(updated_bookmark.url, "https://updated.com");
    }
}
