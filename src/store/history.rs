use anyhow::Error;
use napi_derive::napi;
use rusqlite::Connection;
use sea_query::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use crate::store::{base_path, execute_simple, execute_transaction, open_conn};

static HISTORY_CONNECTION: OnceLock<Arc<Mutex<Connection>>> = OnceLock::new();

fn connection() -> &'static Arc<Mutex<Connection>> {
    HISTORY_CONNECTION.get_or_init(|| {
        let base_path = base_path().unwrap_or("");
        let database_path = PathBuf::from(base_path).join("history.db");
        open_conn(database_path.to_str().unwrap()).expect("Failed to create history database connection")
    })
}

#[derive(Iden)]
enum HistoryTable {
    Table,
    Id,
    Url,
    Icon,
    Title,
    Visit,
}

#[napi(object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct History {
    pub id: Option<i64>,
    pub url: Option<String>,
    pub icon: Option<String>,
    pub title: Option<String>,
    pub visit: Option<String>,
}

pub fn init_history_database() -> Result<(), Error> {
    execute_simple(connection(), |conn| {
        conn.execute(
            &Table::create()
                .table(HistoryTable::Table)
                .if_not_exists()
                .col(ColumnDef::new(HistoryTable::Id).integer().primary_key())
                .col(ColumnDef::new(HistoryTable::Url).text())
                .col(ColumnDef::new(HistoryTable::Icon).text())
                .col(ColumnDef::new(HistoryTable::Title).text())
                .col(ColumnDef::new(HistoryTable::Visit).text())
                .to_string(SqliteQueryBuilder),
            [],
        )?;
        Ok(())
    })
}

pub fn save_history(history: History) -> Result<i64, Error> {
    execute_transaction(connection(), |conn| {
        if let Some(id) = history.id {
            conn.execute(
                &Query::update()
                    .table(HistoryTable::Table)
                    .values([
                        (HistoryTable::Url, history.url.unwrap_or_default().into()),
                        (HistoryTable::Icon, history.icon.unwrap_or_default().into()),
                        (HistoryTable::Title, history.title.unwrap_or_default().into()),
                        (HistoryTable::Visit, history.visit.unwrap_or_default().into()),
                    ])
                    .and_where(Expr::col(HistoryTable::Id).eq(id))
                    .to_string(SqliteQueryBuilder),
                [],
            )?;
            Ok(id)
        } else {
            conn.execute(
                &Query::insert()
                    .into_table(HistoryTable::Table)
                    .columns([
                        HistoryTable::Url,
                        HistoryTable::Icon,
                        HistoryTable::Title,
                        HistoryTable::Visit,
                    ])
                    .values_panic([
                        history.url.unwrap_or_default().into(),
                        history.icon.unwrap_or_default().into(),
                        history.title.unwrap_or_default().into(),
                        history.visit.unwrap_or_default().into(),
                    ])
                    .to_string(SqliteQueryBuilder),
                [],
            )?;
            Ok(conn.last_insert_rowid())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::BASE_PATH;

    #[test]
    fn test_history() {
        BASE_PATH
            .set("/tmp/browser-core/database".to_string())
            .expect("Failed to set BASE_PATH");
        init_history_database().expect("Failed to initialize database");

        let history = History {
            id: None,
            url: Some("https://example.com".to_string()),
            icon: Some("icon".to_string()),
            title: Some("Example Site".to_string()),
            visit: Some("2024-01-01".to_string()),
        };
        let history_id = save_history(history.clone()).unwrap();

        assert!(history_id > 0, "Create history Failed");

        let updated_data = History {
            id: Some(history_id),
            url: history.url,
            icon: history.icon,
            title: Some("Updated Title".to_string()),
            visit: Some("2024-01-02".to_string()),
        };
        save_history(updated_data).unwrap();
    }
}
