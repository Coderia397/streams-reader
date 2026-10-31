//! Values from the central brand tokens (`brand/dist/tokens/tokens.json`) that Rust needs at run time: the browser
//! chrome colour per theme. The file is compiled in, so changing `brand/tokens.toml` and
//! rebuilding restyles these too; nothing here is a copy of a token.

use std::sync::OnceLock;

use serde_json::Value;

const TOKENS: &str = include_str!("../../../brand/dist/tokens/tokens.json");

fn tokens() -> &'static Value {
    static PARSED: OnceLock<Value> = OnceLock::new();
    PARSED.get_or_init(|| serde_json::from_str(TOKENS).unwrap_or(Value::Null))
}

/// The opaque colour a theme's token resolves to on its background (`bg` for the status bar).
pub fn theme_color(theme: &str, token: &str) -> String {
    tokens()["themes"][theme]["color"][token]["on_bg"].as_str().unwrap_or_default().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_generated_tokens() {
        for theme in ["ink", "paper", "night"] {
            let bg = theme_color(theme, "bg");
            assert!(bg.starts_with('#') && bg.len() == 7, "{theme}: {bg}");
        }
    }
}
