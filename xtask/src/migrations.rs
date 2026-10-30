//! D1 migrations: `migrations/NNNN_name.sql`, applied in id order and recorded in a `_migrations` ledger. Planning is
//! pure so the rules are unit-tested: an applied file may never change, disappear or be renamed, and a new file may
//! not slot in below one that is already applied.

use std::fs;
use std::path::Path;

use anyhow::{Context as _, Result, bail};
use nolabs_core::cache_key::sha256_hex;
use serde::Deserialize;

use crate::paths::tilde;

pub const LEDGER_DDL: &str = "CREATE TABLE IF NOT EXISTS _migrations (\n  id INTEGER PRIMARY KEY,\n  name TEXT NOT NULL,\n  \
                              sha256 TEXT NOT NULL,\n  applied_at TEXT NOT NULL\n);";
pub const LEDGER_SELECT: &str = "SELECT id, name, sha256 FROM _migrations ORDER BY id;";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub id: u32,
    /// File name, e.g. `0001_init.sql`.
    pub name: String,
    pub sha256: String,
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Applied {
    pub id: u32,
    pub name: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    #[error(
        "{name} was applied with sha256 {applied}, but the file now hashes to {local}; add a new migration instead"
    )]
    ChecksumChanged { name: String, applied: String, local: String },
    #[error("migration {id} was applied as {applied} but the file is now named {local}")]
    Renamed { id: u32, applied: String, local: String },
    #[error("{name} (id {id}) is applied in D1 but missing from migrations/")]
    MissingLocally { id: u32, name: String },
    #[error("{name} is new but its id is below the last applied migration {last}; renumber it")]
    OutOfOrder { name: String, last: u32 },
}

/// The id of a well-formed migration file name (`NNNN_lowercase_words.sql`), else `None`.
pub fn parse_file_name(name: &str) -> Option<u32> {
    let stem = name.strip_suffix(".sql")?;
    let (digits, rest) = stem.split_at_checked(4)?;
    let rest = rest.strip_prefix('_')?;
    let ok = digits.bytes().all(|b| b.is_ascii_digit())
        && !rest.is_empty()
        && rest.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if ok { digits.parse().ok() } else { None }
}

/// Reads every `*.sql` in `dir`, sorted by id. A `.sql` file with a malformed name or a duplicate id is an error so a
/// typo never silently skips a migration.
pub fn load_dir(dir: &Path) -> Result<Vec<Migration>> {
    let mut out = Vec::new();
    let entries = fs::read_dir(dir).with_context(|| format!("cannot read {}", tilde(dir)))?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".sql") || !entry.file_type()?.is_file() {
            continue;
        }
        let Some(id) = parse_file_name(&name) else {
            bail!("{name}: migration files must be named NNNN_lowercase_name.sql");
        };
        let bytes = fs::read(entry.path()).with_context(|| format!("cannot read {name}"))?;
        let sql = String::from_utf8(bytes.clone()).with_context(|| format!("{name} is not UTF-8"))?;
        out.push(Migration { id, name, sha256: sha256_hex(&bytes), sql });
    }
    out.sort_by_key(|m| m.id);
    if let Some(w) = out.windows(2).find(|w| w[0].id == w[1].id) {
        bail!("{} and {} share id {}", w[0].name, w[1].name, w[0].id);
    }
    Ok(out)
}

/// The migrations still to apply, in order. `local` must be sorted by id (as [`load_dir`] returns it).
pub fn plan<'a>(local: &'a [Migration], applied: &[Applied]) -> Result<Vec<&'a Migration>, PlanError> {
    for a in applied {
        let Some(m) = local.iter().find(|m| m.id == a.id) else {
            return Err(PlanError::MissingLocally { id: a.id, name: a.name.clone() });
        };
        if m.name != a.name {
            return Err(PlanError::Renamed { id: a.id, applied: a.name.clone(), local: m.name.clone() });
        }
        if m.sha256 != a.sha256 {
            return Err(PlanError::ChecksumChanged {
                name: m.name.clone(),
                applied: a.sha256.clone(),
                local: m.sha256.clone(),
            });
        }
    }
    let pending: Vec<&Migration> = local.iter().filter(|m| !applied.iter().any(|a| a.id == m.id)).collect();
    if let Some(last) = applied.iter().map(|a| a.id).max()
        && let Some(early) = pending.iter().find(|m| m.id < last)
    {
        return Err(PlanError::OutOfOrder { name: early.name.clone(), last });
    }
    Ok(pending)
}

/// One request body: the migration followed by its ledger row, so both land together or neither does.
pub fn apply_sql(m: &Migration) -> String {
    let mut sql = m.sql.trim_end().to_owned();
    if needs_terminator(&sql) {
        sql.push_str("\n;");
    }
    format!(
        "{sql}\nINSERT INTO _migrations (id, name, sha256, applied_at) VALUES ({}, {}, {}, \
         strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));",
        m.id,
        sql_string(&m.name),
        sql_string(&m.sha256),
    )
}

/// True when the last statement is not closed by `;`. Trailing blank and `--` comment lines are skipped, because a
/// `;` appended to a comment line would be swallowed by the comment.
fn needs_terminator(sql: &str) -> bool {
    match sql.lines().rev().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("--")) {
        Some(last) => !last.ends_with(';'),
        None => false,
    }
}

fn sql_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn mig(id: u32, name: &str, sql: &str) -> Migration {
        Migration { id, name: name.into(), sha256: sha256_hex(sql.as_bytes()), sql: sql.into() }
    }

    fn applied(m: &Migration) -> Applied {
        Applied { id: m.id, name: m.name.clone(), sha256: m.sha256.clone() }
    }

    #[test]
    fn file_names() {
        assert_eq!(parse_file_name("0001_init.sql"), Some(1));
        assert_eq!(parse_file_name("0042_add_jobs_2.sql"), Some(42));
        for bad in [
            "001_init.sql",
            "0001init.sql",
            "0001_.sql",
            "0001_Init.sql",
            "0001_init.SQL",
            "abcd_init.sql",
            "0001_in-it.sql",
            "0001_init.sql.bak",
        ] {
            assert_eq!(parse_file_name(bad), None, "{bad}");
        }
    }

    #[test]
    fn fresh_database_applies_everything_in_order() {
        let local = vec![mig(1, "0001_a.sql", "x"), mig(2, "0002_b.sql", "y")];
        let p = plan(&local, &[]).unwrap();
        assert_eq!(p.iter().map(|m| m.id).collect::<Vec<_>>(), [1, 2]);
    }

    #[test]
    fn applied_ones_are_skipped() {
        let local = vec![mig(1, "0001_a.sql", "x"), mig(2, "0002_b.sql", "y"), mig(3, "0003_c.sql", "z")];
        let p = plan(&local, &[applied(&local[0]), applied(&local[1])]).unwrap();
        assert_eq!(p.iter().map(|m| m.id).collect::<Vec<_>>(), [3]);
        assert!(plan(&local, &local.iter().map(applied).collect::<Vec<_>>()).unwrap().is_empty());
    }

    #[test]
    fn changed_checksum_fails() {
        let local = vec![mig(1, "0001_a.sql", "edited")];
        let a = Applied { id: 1, name: "0001_a.sql".into(), sha256: sha256_hex(b"original") };
        assert!(matches!(plan(&local, &[a]), Err(PlanError::ChecksumChanged { .. })));
    }

    #[test]
    fn missing_and_renamed_fail() {
        let local = vec![mig(2, "0002_b.sql", "y")];
        let gone = Applied { id: 1, name: "0001_a.sql".into(), sha256: "h".into() };
        assert_eq!(plan(&local, &[gone]), Err(PlanError::MissingLocally { id: 1, name: "0001_a.sql".into() }));
        let renamed = Applied { id: 2, name: "0002_old.sql".into(), sha256: local[0].sha256.clone() };
        assert!(matches!(plan(&local, &[renamed]), Err(PlanError::Renamed { id: 2, .. })));
    }

    #[test]
    fn new_file_below_applied_fails() {
        let local = vec![mig(1, "0001_a.sql", "x"), mig(2, "0002_b.sql", "y"), mig(3, "0003_c.sql", "z")];
        let err = plan(&local, &[applied(&local[0]), applied(&local[2])]).unwrap_err();
        assert_eq!(err, PlanError::OutOfOrder { name: "0002_b.sql".into(), last: 3 });
    }

    #[test]
    fn apply_sql_appends_ledger_row() {
        let m = mig(7, "0007_x.sql", "CREATE TABLE t (a INTEGER);\n\n");
        let sql = apply_sql(&m);
        assert!(sql.starts_with("CREATE TABLE t (a INTEGER);\nINSERT INTO _migrations"), "{sql}");
        assert!(sql.contains(&format!("VALUES (7, '0007_x.sql', '{}', strftime(", m.sha256)), "{sql}");
        assert!(sql.ends_with(");"));
    }

    #[test]
    fn apply_sql_terminates_an_open_statement_before_a_trailing_comment() {
        let m = mig(1, "0001_a.sql", "CREATE TABLE t (a INTEGER)\n-- done\n");
        let sql = apply_sql(&m);
        assert!(sql.starts_with("CREATE TABLE t (a INTEGER)\n-- done\n;\nINSERT"), "{sql}");
        let closed = apply_sql(&mig(1, "0001_a.sql", "CREATE TABLE t (a INTEGER);\n-- done"));
        assert!(closed.starts_with("CREATE TABLE t (a INTEGER);\n-- done\nINSERT"), "{closed}");
        let only_comment = apply_sql(&mig(1, "0001_a.sql", "-- nothing yet"));
        assert!(only_comment.starts_with("-- nothing yet\nINSERT"), "{only_comment}");
    }

    #[test]
    fn sql_strings_are_escaped() {
        assert_eq!(sql_string("it's"), "'it''s'");
    }

    #[test]
    fn load_dir_sorts_hashes_and_rejects_bad_names() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("0002_b.sql"), "B;").unwrap();
        fs::write(dir.path().join("0001_a.sql"), "A;").unwrap();
        fs::write(dir.path().join("README.md"), "ignored").unwrap();
        let ms = load_dir(dir.path()).unwrap();
        assert_eq!(ms.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["0001_a.sql", "0002_b.sql"]);
        assert_eq!(ms[0].sha256, sha256_hex(b"A;"));
        fs::write(dir.path().join("3_bad.sql"), "C;").unwrap();
        assert!(load_dir(dir.path()).is_err());
    }

    #[test]
    fn load_dir_rejects_duplicate_ids() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("0001_a.sql"), "A;").unwrap();
        fs::write(dir.path().join("0001_b.sql"), "B;").unwrap();
        let err = load_dir(dir.path()).unwrap_err().to_string();
        assert!(err.contains("share id 1"), "{err}");
    }
}
