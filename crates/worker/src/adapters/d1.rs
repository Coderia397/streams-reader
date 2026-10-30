//! [`Db`] over the D1 binding.

use async_trait::async_trait;
use nolabs_server::ports::{Db, PortError, Row, SqlValue, Stmt, StmtResult};
use worker::wasm_bindgen::JsValue;
use worker::{D1Database, D1PreparedStatement, D1Result};

pub struct D1Db(pub D1Database);

fn port(e: worker::Error) -> PortError {
    PortError::new(format!("D1: {e}"))
}

fn js(value: &SqlValue) -> JsValue {
    match value {
        SqlValue::Null => JsValue::NULL,
        // Every integer we store (times in ms, sizes) is below 2^53, so a JS number holds it exactly.
        SqlValue::Int(i) => JsValue::from_f64(*i as f64),
        SqlValue::Real(f) => JsValue::from_f64(*f),
        SqlValue::Text(s) => JsValue::from_str(s),
    }
}

impl D1Db {
    fn prepare(&self, stmt: &Stmt) -> Result<D1PreparedStatement, PortError> {
        let params: Vec<JsValue> = stmt.params.iter().map(js).collect();
        self.0.prepare(stmt.sql.as_str()).bind(&params).map_err(port)
    }
}

fn rows(result: &D1Result) -> Result<Vec<Row>, PortError> {
    result.results::<Row>().map_err(port)
}

fn changes(result: &D1Result) -> Result<u64, PortError> {
    Ok(result.meta().map_err(port)?.and_then(|m| m.changes).map_or(0, |c| c as u64))
}

#[async_trait(?Send)]
impl Db for D1Db {
    async fn all(&self, stmt: &Stmt) -> Result<Vec<Row>, PortError> {
        let result = self.prepare(stmt)?.all().await.map_err(port)?;
        if let Some(error) = result.error() {
            return Err(PortError::new(format!("D1: {error}")));
        }
        rows(&result)
    }

    async fn run(&self, stmt: &Stmt) -> Result<u64, PortError> {
        let result = self.prepare(stmt)?.run().await.map_err(port)?;
        if let Some(error) = result.error() {
            return Err(PortError::new(format!("D1: {error}")));
        }
        changes(&result)
    }

    /// D1 runs a batch as one implicit transaction: a failing statement rolls back the whole batch.
    async fn batch(&self, stmts: &[Stmt]) -> Result<Vec<StmtResult>, PortError> {
        let prepared = stmts.iter().map(|s| self.prepare(s)).collect::<Result<Vec<_>, _>>()?;
        let results = self.0.batch(prepared).await.map_err(port)?;
        results.iter().map(|r| Ok(StmtResult { rows: rows(r)?, changes: changes(r)? })).collect()
    }
}
