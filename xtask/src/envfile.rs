//! `KEY=value` credential files under `~/.config`. Values are loaded straight into [`Secret`]s; parse errors name the
//! file and line number only, never the line's content. Files this tool writes are created with mode 600 inside a
//! mode 700 directory and replaced atomically, so a crash never leaves a half-written credential file behind.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};

use crate::paths::tilde;
use crate::secret::Secret;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("line {0} is not KEY=value")]
    NotAssignment(usize),
    #[error("line {0} has an invalid key")]
    BadKey(usize),
}

/// Parses `KEY=value` lines. Blank lines and `#` comments are skipped, an `export ` prefix is accepted, and one pair
/// of matching surrounding quotes is removed from the value. A later duplicate key wins, as in a shell.
pub fn parse(text: &str) -> Result<BTreeMap<String, Secret>, ParseError> {
    let mut out = BTreeMap::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").map(str::trim_start).unwrap_or(line);
        let (key, value) = line.split_once('=').ok_or(ParseError::NotAssignment(i + 1))?;
        let key = key.trim();
        if !is_valid_key(key) {
            return Err(ParseError::BadKey(i + 1));
        }
        out.insert(key.to_owned(), Secret::new(unquote(value.trim())));
    }
    Ok(out)
}

fn is_valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn unquote(value: &str) -> &str {
    for q in ['"', '\''] {
        if let Some(inner) = value.strip_prefix(q).and_then(|v| v.strip_suffix(q)) {
            return inner;
        }
    }
    value
}

/// Renders the file content written by [`write_private`]. Values must be single-line and unquoted-safe; every value
/// this tool writes is hex, base32 or an API identifier.
pub fn render(header: &str, entries: &[(&str, &str)]) -> Result<String> {
    let mut out = String::new();
    for line in header.lines() {
        out.push_str("# ");
        out.push_str(line);
        out.push('\n');
    }
    for (key, value) in entries {
        if !is_valid_key(key) {
            bail!("invalid env key {key:?}");
        }
        if value.chars().any(|c| c.is_whitespace() || c == '"' || c == '\'' || c == '#') {
            // The message names the key only: the value may be a secret.
            bail!("value for {key} contains characters that need quoting");
        }
        out.push_str(key);
        out.push('=');
        out.push_str(value);
        out.push('\n');
    }
    Ok(out)
}

/// A loaded credential file.
#[derive(Debug)]
pub struct EnvFile {
    path: PathBuf,
    vars: BTreeMap<String, Secret>,
}

impl EnvFile {
    pub fn load(path: &Path) -> Result<Self> {
        warn_if_readable_by_others(path);
        let text = fs::read_to_string(path).with_context(|| format!("cannot read {}", tilde(path)))?;
        let vars = parse(&text).with_context(|| format!("cannot parse {}", tilde(path)))?;
        Ok(Self { path: path.to_owned(), vars })
    }

    /// `Ok(None)` when the file does not exist.
    pub fn load_optional(path: &Path) -> Result<Option<Self>> {
        if path.exists() { Self::load(path).map(Some) } else { Ok(None) }
    }

    pub fn get(&self, key: &str) -> Option<&Secret> {
        self.vars.get(key).filter(|v| !v.is_empty())
    }

    pub fn secret(&self, key: &str) -> Result<&Secret> {
        self.get(key).with_context(|| format!("{key} is missing from {}", tilde(&self.path)))
    }

    /// For values that are identifiers rather than credentials (account id, zone id).
    pub fn plain(&self, key: &str) -> Result<String> {
        self.secret(key).map(|s| s.expose().to_owned())
    }
}

#[cfg(unix)]
fn warn_if_readable_by_others(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    if let Ok(meta) = fs::metadata(path)
        && meta.permissions().mode() & 0o077 != 0
    {
        eprintln!("warning: {} is readable by other users; run chmod 600 on it", tilde(path));
    }
}

#[cfg(not(unix))]
fn warn_if_readable_by_others(_path: &Path) {}

/// Writes `content` to `path` with mode 600, creating the parent directory with mode 700. The file is written to a
/// sibling temp file first and renamed over the target.
pub fn write_private(path: &Path, content: &str) -> Result<()> {
    let dir = path.parent().with_context(|| format!("{} has no parent directory", tilde(path)))?;
    create_private_dir(dir)?;
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("env");
    let tmp = dir.join(format!(".{file_name}.tmp"));
    let result = (|| -> Result<()> {
        let mut file = open_private(&tmp)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.with_context(|| format!("cannot write {}", tilde(path)))
}

#[cfg(unix)]
fn create_private_dir(dir: &Path) -> Result<()> {
    use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
    if !dir.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .with_context(|| format!("cannot create {}", tilde(dir)))?;
    }
    let mode = fs::metadata(dir)?.permissions().mode();
    if mode & 0o077 != 0 {
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("cannot chmod 700 {}", tilde(dir)))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("cannot create {}", tilde(dir)))
}

#[cfg(unix)]
fn open_private(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)
}

#[cfg(not(unix))]
fn open_private(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new().write(true).create(true).truncate(true).open(path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parses_comments_export_and_quotes() {
        let text = "# header\n\nCF_EMAIL=a@b.c\nexport CF_KEY=\"k e y\"\nCF_ZONE='z1'\n  SPACED = v \n";
        let vars = parse(text).unwrap();
        assert_eq!(vars["CF_EMAIL"].expose(), "a@b.c");
        assert_eq!(vars["CF_KEY"].expose(), "k e y");
        assert_eq!(vars["CF_ZONE"].expose(), "z1");
        assert_eq!(vars["SPACED"].expose(), "v");
    }

    #[test]
    fn value_may_contain_equals() {
        let vars = parse("TOKEN=abc==\n").unwrap();
        assert_eq!(vars["TOKEN"].expose(), "abc==");
    }

    #[test]
    fn later_duplicate_wins() {
        let vars = parse("A=1\nA=2\n").unwrap();
        assert_eq!(vars["A"].expose(), "2");
    }

    #[test]
    fn errors_name_the_line_not_the_content() {
        let err = parse("A=1\nsecretvaluewithoutequals\n").unwrap_err();
        assert_eq!(err, ParseError::NotAssignment(2));
        assert!(!err.to_string().contains("secretvalue"));
        assert_eq!(parse("1BAD=x").unwrap_err(), ParseError::BadKey(1));
    }

    #[test]
    fn render_round_trips_through_parse() {
        let text =
            render("generated\nby xtask", &[("CF_API_TOKEN", "abc_DEF-123"), ("CF_ACCOUNT_ID", "0f1e")]).unwrap();
        assert!(text.starts_with("# generated\n# by xtask\n"));
        let vars = parse(&text).unwrap();
        assert_eq!(vars["CF_API_TOKEN"].expose(), "abc_DEF-123");
        assert_eq!(vars["CF_ACCOUNT_ID"].expose(), "0f1e");
    }

    #[test]
    fn render_rejects_values_needing_quotes_without_echoing_them() {
        let err = render("", &[("K", "has space")]).unwrap_err().to_string();
        assert!(err.contains('K') && !err.contains("has space"), "{err}");
        assert!(render("", &[("bad key", "v")]).is_err());
    }

    #[test]
    fn empty_value_counts_as_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.env");
        fs::write(&path, "A=\nB=1\n").unwrap();
        let env = EnvFile::load(&path).unwrap();
        assert!(env.get("A").is_none());
        assert!(env.secret("A").is_err());
        assert_eq!(env.plain("B").unwrap(), "1");
    }

    #[cfg(unix)]
    #[test]
    fn write_private_sets_modes_and_replaces_atomically() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("nolabs");
        let path = sub.join("app.env");
        write_private(&path, "A=1\n").unwrap();
        write_private(&path, "A=2\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "A=2\n");
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::metadata(&sub).unwrap().permissions().mode() & 0o777, 0o700);
        let leftovers: Vec<_> = fs::read_dir(&sub).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(leftovers.len(), 1, "{leftovers:?}");
        assert!(EnvFile::load_optional(&sub.join("missing.env")).unwrap().is_none());
    }
}
