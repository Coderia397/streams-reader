//! The few brand token values HTML needs outside CSS (`theme-color`, the OG image colours), read from
//! `brand/dist/tokens/tokens.json` so they follow `brand/tokens.toml` like everything else.

use std::path::Path;

use serde_json::Value;

use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeColors {
    /// Background of the default (dark) theme.
    pub dark_bg: String,
    /// Background of the light theme.
    pub light_bg: String,
    /// Brand fill, its label colour and the spoken-word colour of the default theme (favicon, social card).
    pub brand: String,
    pub on_brand: String,
    pub ember: String,
}

impl ThemeColors {
    pub fn load(tokens_json: &Path) -> Result<Self, Error> {
        let raw = std::fs::read_to_string(tokens_json).map_err(|e| Error::io(tokens_json, e))?;
        Self::parse(&raw)
    }

    pub fn parse(raw: &str) -> Result<Self, Error> {
        let doc: Value = serde_json::from_str(raw).map_err(|e| Error::Tokens(e.to_string()))?;
        let theme_name = |key: &str| -> Result<String, Error> {
            doc["meta"][key].as_str().map(str::to_string).ok_or_else(|| Error::Tokens(format!("meta.{key} missing")))
        };
        let color = |theme: &str, name: &str| -> Result<String, Error> {
            doc["themes"][theme]["color"][name]["hex"]
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| Error::Tokens(format!("themes.{theme}.color.{name} missing")))
        };
        let dark = theme_name("default_theme")?;
        Ok(Self {
            dark_bg: color(&dark, "bg")?,
            light_bg: color(&theme_name("light_theme")?, "bg")?,
            brand: color(&dark, "brand")?,
            on_brand: color(&dark, "on-brand")?,
            ember: color(&dark, "ember")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_default_and_light_backgrounds() {
        let raw = r##"{"meta":{"default_theme":"ink","light_theme":"paper"},
            "themes":{"ink":{"color":{"bg":{"hex":"#000001"},"brand":{"hex":"#0000FF"},"on-brand":{"hex":"#FFFFFF"},
            "ember":{"hex":"#FFAA00"}}},"paper":{"color":{"bg":{"hex":"#FFFFF0"}}}}}"##;
        let t = ThemeColors::parse(raw).unwrap();
        assert_eq!((t.dark_bg.as_str(), t.light_bg.as_str()), ("#000001", "#FFFFF0"));
        assert_eq!((t.brand.as_str(), t.on_brand.as_str(), t.ember.as_str()), ("#0000FF", "#FFFFFF", "#FFAA00"));
    }

    #[test]
    fn missing_theme_is_an_error() {
        assert!(ThemeColors::parse(r#"{"meta":{}}"#).is_err());
    }
}
