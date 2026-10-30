//! The Worker upload: which files `worker-build` produced and the multipart metadata Cloudflare needs.
//!
//! worker-build 0.8.7 writes `crates/worker/build/index.js` (the esbuild-bundled entry, ES module) next to the
//! wasm-bindgen output it imports (`index_bg.wasm`, plus any `snippets/**.js`). Every `.js`/`.mjs`/`.wasm` file under
//! `build/` is uploaded as a module under its path relative to `build/`, because that is the specifier the bundle
//! imports it by. Hidden directories (`.tmp`, `.old`: worker-build's scratch space) are skipped.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use crate::names::{BUCKET, COMPATIBILITY_DATE, QUEUE};
use crate::paths::tilde;

pub const MAIN_MODULE: &str = "index.js";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// Path relative to the build directory with `/` separators; the multipart part name and module specifier.
    pub name: String,
    pub path: PathBuf,
    pub content_type: &'static str,
}

/// The module content type for a build output file, or `None` for files that are not uploaded (maps, `.d.ts`, …).
pub fn content_type(file_name: &str) -> Option<&'static str> {
    if file_name.ends_with(".d.ts") {
        return None;
    }
    match file_name.rsplit_once('.').map(|(_, ext)| ext) {
        Some("js" | "mjs") => Some("application/javascript+module"),
        Some("wasm") => Some("application/wasm"),
        _ => None,
    }
}

/// Every module under `build_dir`, main module first, the rest sorted by name.
pub fn collect_modules(build_dir: &Path) -> Result<Vec<Module>> {
    if !build_dir.is_dir() {
        bail!("{} does not exist: run `cargo xtask build` first", tilde(build_dir));
    }
    let mut out = Vec::new();
    walk(build_dir, build_dir, &mut out)?;
    out.sort_by(|a, b| (a.name != MAIN_MODULE, &a.name).cmp(&(b.name != MAIN_MODULE, &b.name)));
    if out.first().is_none_or(|m| m.name != MAIN_MODULE) {
        let found: Vec<&str> = out.iter().map(|m| m.name.as_str()).collect();
        bail!("{} has no {MAIN_MODULE} (found: {found:?}); did worker-build's output layout change?", tilde(build_dir));
    }
    if !out.iter().any(|m| m.content_type == "application/wasm") {
        bail!("{} has no .wasm module", tilde(build_dir));
    }
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<Module>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", tilde(dir)))? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if file_name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            walk(root, &path, out)?;
        } else if kind.is_file()
            && let Some(content_type) = content_type(&file_name)
        {
            let rel = path.strip_prefix(root).context("module outside build dir")?;
            let name = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            out.push(Module { name, path, content_type });
        }
    }
    Ok(())
}

/// Plain-text vars bound into the Worker (note 09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vars {
    pub space_url: String,
    pub space_repo: String,
    pub kokoro_rev: String,
    pub ming_rev: String,
    pub app_version: String,
}

/// The `metadata` part of the script upload. Secrets are not here: they are set through the secrets endpoint and
/// survive uploads because of `keep_bindings`.
pub fn metadata(d1_id: &str, vars: &Vars) -> Value {
    let text = |name: &str, value: &str| json!({ "type": "plain_text", "name": name, "text": value });
    json!({
        "main_module": MAIN_MODULE,
        "compatibility_date": COMPATIBILITY_DATE,
        "bindings": [
            { "type": "d1", "name": "DB", "id": d1_id },
            { "type": "r2_bucket", "name": "MEDIA", "bucket_name": BUCKET },
            { "type": "queue", "name": "RENDER_QUEUE", "queue_name": QUEUE },
            text("SPACE_URL", &vars.space_url),
            text("SPACE_REPO", &vars.space_repo),
            text("KOKORO_REV", &vars.kokoro_rev),
            text("MING_REV", &vars.ming_rev),
            text("APP_VERSION", &vars.app_version),
        ],
        "keep_bindings": ["secret_text"],
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn vars() -> Vars {
        Vars {
            space_url: "https://ibrahimar397-nolabs-engine.hf.space".into(),
            space_repo: "ibrahimar397/nolabs-engine".into(),
            kokoro_rev: "hexgrad/Kokoro-82M@abc".into(),
            ming_rev: "inclusionAI/Ming-3.0-tts-0.5B@def".into(),
            app_version: "1a2b3c4".into(),
        }
    }

    #[test]
    fn content_types() {
        assert_eq!(content_type("index.js"), Some("application/javascript+module"));
        assert_eq!(content_type("shim.mjs"), Some("application/javascript+module"));
        assert_eq!(content_type("index_bg.wasm"), Some("application/wasm"));
        for skip in ["index.d.ts", "index.js.map", "package.json", "README", "index_bg.wasm.d.ts"] {
            assert_eq!(content_type(skip), None, "{skip}");
        }
    }

    #[test]
    fn metadata_has_every_binding() {
        let m = metadata("d1-uuid", &vars());
        assert_eq!(m["main_module"], "index.js");
        assert_eq!(m["compatibility_date"], "2026-10-01");
        assert_eq!(m["keep_bindings"], json!(["secret_text"]));
        let bindings = m["bindings"].as_array().unwrap();
        let find = |name: &str| bindings.iter().find(|b| b["name"] == name).unwrap().clone();
        assert_eq!(find("DB"), json!({"type":"d1","name":"DB","id":"d1-uuid"}));
        assert_eq!(find("MEDIA"), json!({"type":"r2_bucket","name":"MEDIA","bucket_name":"nolabs-media"}));
        assert_eq!(find("RENDER_QUEUE"), json!({"type":"queue","name":"RENDER_QUEUE","queue_name":"nolabs-render"}));
        assert_eq!(find("APP_VERSION"), json!({"type":"plain_text","name":"APP_VERSION","text":"1a2b3c4"}));
        assert_eq!(find("KOKORO_REV")["text"], "hexgrad/Kokoro-82M@abc");
        assert_eq!(find("SPACE_URL")["text"], "https://ibrahimar397-nolabs-engine.hf.space");
        assert_eq!(bindings.len(), 8);
        // No secret ever travels in metadata.
        assert!(bindings.iter().all(|b| b["type"] != "secret_text"));
    }

    #[test]
    fn collects_modules_main_first() {
        let dir = tempfile::tempdir().unwrap();
        let b = dir.path();
        fs::create_dir_all(b.join("snippets/worker-abc")).unwrap();
        fs::create_dir_all(b.join(".tmp")).unwrap();
        fs::write(b.join("index.js"), "export default {}").unwrap();
        fs::write(b.join("index_bg.wasm"), [0, 97, 115, 109]).unwrap();
        fs::write(b.join("index.d.ts"), "").unwrap();
        fs::write(b.join("snippets/worker-abc/inline0.js"), "").unwrap();
        fs::write(b.join(".tmp/stale.js"), "").unwrap();
        let mods = collect_modules(b).unwrap();
        let names: Vec<&str> = mods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["index.js", "index_bg.wasm", "snippets/worker-abc/inline0.js"]);
        assert_eq!(mods[1].content_type, "application/wasm");
    }

    #[test]
    fn missing_main_or_wasm_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("index_bg.wasm"), "").unwrap();
        assert!(collect_modules(dir.path()).unwrap_err().to_string().contains("no index.js"));
        fs::write(dir.path().join("index.js"), "").unwrap();
        fs::remove_file(dir.path().join("index_bg.wasm")).unwrap();
        assert!(collect_modules(dir.path()).unwrap_err().to_string().contains("no .wasm"));
        assert!(collect_modules(&dir.path().join("absent")).is_err());
    }
}
