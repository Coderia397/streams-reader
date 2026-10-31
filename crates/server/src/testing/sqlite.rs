//! [`Db`] over an in-memory SQLite database with every migration applied, standing in for D1 (which is SQLite).

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use rusqlite::types::{Value as SqlRaw, ValueRef};
use rusqlite::{Connection, params_from_iter};
use serde_json::Value;

use crate::ports::{Db, PortError, Row, SqlValue, Stmt, StmtResult};

#[derive(Clone)]
pub struct SqliteDb {
    conn: Rc<RefCell<Connection>>,
}

fn err(e: rusqlite::Error) -> PortError {
    PortError::new(e.to_string())
}

impl SqliteDb {
    /// A fresh database with `migrations/*.sql` applied in file-name order.
    pub fn migrated() -> SqliteDb {
        let conn = Connection::open_in_memory().unwrap();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "sql"))
            .collect();
        files.sort();
        assert!(!files.is_empty(), "no migrations in {}", dir.display());
        for f in files {
            conn.execute_batch(&std::fs::read_to_string(&f).unwrap()).unwrap_or_else(|e| panic!("{f:?}: {e}"));
        }
        SqliteDb { conn: Rc::new(RefCell::new(conn)) }
    }

    /// Run raw SQL in a test (setup or inspection).
    pub fn exec(&self, sql: &str) {
        self.conn.borrow().execute_batch(sql).unwrap();
    }

    /// One value from a raw query, as JSON.
    pub fn scalar(&self, sql: &str) -> Value {
        let conn = self.conn.borrow();
        conn.query_row(sql, [], |r| Ok(to_json(r.get_ref(0)?))).unwrap()
    }
}

fn bind(v: &SqlValue) -> SqlRaw {
    match v {
        SqlValue::Null => SqlRaw::Null,
        SqlValue::Int(i) => SqlRaw::Integer(*i),
        SqlValue::Real(f) => SqlRaw::Real(*f),
        SqlValue::Text(s) => SqlRaw::Text(s.clone()),
    }
}

fn to_json(v: ValueRef<'_>) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::from(i),
        ValueRef::Real(f) => Value::from(f),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Value::from(b.to_vec()),
    }
}

/// Run one statement, collecting rows (for RETURNING and SELECT) and the change count.
fn execute(conn: &Connection, stmt: &Stmt) -> Result<StmtResult, PortError> {
    let mut prepared = conn.prepare(&stmt.sql).map_err(err)?;
    let names: Vec<String> = prepared.column_names().into_iter().map(str::to_owned).collect();
    let mut rows = prepared.query(params_from_iter(stmt.params.iter().map(bind))).map_err(err)?;
    let mut out = Vec::new();
    while let Some(r) = rows.next().map_err(err)? {
        let mut row = Row::new();
        for (i, name) in names.iter().enumerate() {
            row.insert(name.clone(), to_json(r.get_ref(i).map_err(err)?));
        }
        out.push(row);
    }
    drop(rows);
    let is_write = !stmt.sql.trim_start().to_ascii_uppercase().starts_with("SELECT");
    let changes = if is_write { conn.changes() } else { 0 };
    Ok(StmtResult { rows: out, changes })
}

#[async_trait(?Send)]
impl Db for SqliteDb {
    async fn all(&self, stmt: &Stmt) -> Result<Vec<Row>, PortError> {
        Ok(execute(&self.conn.borrow(), stmt)?.rows)
    }

    async fn run(&self, stmt: &Stmt) -> Result<u64, PortError> {
        Ok(execute(&self.conn.borrow(), stmt)?.changes)
    }

    async fn batch(&self, stmts: &[Stmt]) -> Result<Vec<StmtResult>, PortError> {
        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction().map_err(err)?;
        let mut out = Vec::with_capacity(stmts.len());
        for s in stmts {
            out.push(execute(&tx, s)?);
        }
        tx.commit().map_err(err)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batches_are_atomic() {
        let db = SqliteDb::migrated();
        let ok = |n: i64| {
            Stmt::new(
                "INSERT INTO config (key, value, updated_at_ms) VALUES (?, 'v', 0)",
                vec![SqlValue::Text(format!("k{n}"))],
            )
        };
        let bad = Stmt::new("INSERT INTO nowhere VALUES (1)", vec![]);
        assert!(futures::executor::block_on(db.batch(&[ok(1), bad])).is_err());
        assert_eq!(db.scalar("SELECT count(*) FROM config"), Value::from(0));
        let res = futures::executor::block_on(db.batch(&[ok(1), ok(2)])).unwrap();
        assert_eq!(res.iter().map(|r| r.changes).collect::<Vec<_>>(), vec![1, 1]);
    }

    /// 0002 moved documents and the passphrase login out of D1 and added the per-user columns.
    #[test]
    fn migrations_leave_the_accounts_schema() {
        let db = SqliteDb::migrated();
        let tables = db.scalar(
            "SELECT group_concat(name, ',') FROM (SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name)",
        );
        assert_eq!(tables, Value::from("config,gpu_ledger,jobs,owner_cache,renders,voices"));
        assert_eq!(db.scalar("SELECT count(*) FROM config"), Value::from(0), "no session epoch any more");
        for (table, column) in [
            ("voices", "user_id"),
            ("voices", "synced_state"),
            ("jobs", "user_id"),
            ("jobs", "gpu_share_s"),
            ("gpu_ledger", "user_id"),
        ] {
            let n = db.scalar(&format!("SELECT count(*) FROM pragma_table_info('{table}') WHERE name = '{column}'"));
            assert_eq!(n, Value::from(1), "{table}.{column}");
        }
    }
}
