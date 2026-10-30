//! `migrate`: applies `migrations/NNNN_*.sql` to D1 in order. Each migration and its ledger row go in one query
//! request; an applied file whose checksum changed stops the run before anything is applied.

use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::cf::Cf;
use crate::cmd::resources::{d1_query, require_d1};
use crate::migrations::{self, Applied, LEDGER_DDL, LEDGER_SELECT};
use crate::paths;

pub fn run(dry_run: bool) -> Result<()> {
    let local = migrations::load_dir(&paths::migrations_dir())?;
    let cf = Cf::from_deploy_env()?;
    let db = require_d1(&cf)?;

    let applied = if dry_run {
        // Reading the ledger must not create it during a dry run; a missing table means nothing is applied.
        let exists =
            d1_query(&cf, &db, "SELECT name FROM sqlite_master WHERE type = 'table' AND name = '_migrations';")?;
        if exists.first().is_some_and(|rows| !rows.is_empty()) { read_ledger(&cf, &db)? } else { vec![] }
    } else {
        d1_query(&cf, &db, LEDGER_DDL)?;
        read_ledger(&cf, &db)?
    };

    let pending = migrations::plan(&local, &applied)?;
    println!("{} applied, {} pending, {} local", applied.len(), pending.len(), local.len());
    for m in pending {
        if dry_run {
            println!("would apply {} (sha256 {})", m.name, m.sha256.get(..12).unwrap_or(&m.sha256));
            continue;
        }
        d1_query(&cf, &db, &migrations::apply_sql(m)).with_context(|| format!("applying {}", m.name))?;
        println!("applied {}", m.name);
    }
    Ok(())
}

fn read_ledger(cf: &Cf, db: &crate::cmd::resources::D1) -> Result<Vec<Applied>> {
    let rows = d1_query(cf, db, LEDGER_SELECT)?;
    let rows: Vec<Value> = rows.into_iter().next().unwrap_or_default();
    rows.into_iter().map(|r| serde_json::from_value(r).context("unexpected _migrations row")).collect()
}
