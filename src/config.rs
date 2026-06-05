//! Configuration — a hand-edited JSON file at `%APPDATA%\atref\config.json` (FR3).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// User configuration. Mirrors the on-disk JSON schema (FR3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Absolute directory paths to index. Must be non-empty.
    pub folders: Vec<PathBuf>,
    /// Global chord, in `global-hotkey` `HotKey::from_str` syntax.
    pub chord: String,
    /// Directory names pruned during traversal.
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Config {
    /// The default config written on first launch (FR3).
    pub fn default_with_home(home: PathBuf) -> Self {
        Self {
            folders: vec![home],
            chord: "Control+Space".to_string(),
            exclude: [".git", "node_modules", "target"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    /// Parse and validate config from JSON text.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let cfg: Config = serde_json::from_str(text)
            .map_err(|e| format!("config.json is not valid JSON: {e}"))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Serialize to pretty JSON for the default-write.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("Config always serializes")
    }

    /// Schema validation beyond what serde enforces (FR4).
    pub fn validate(&self) -> Result<(), String> {
        if self.folders.is_empty() {
            return Err("`folders` must list at least one directory".to_string());
        }
        if self.chord.trim().is_empty() {
            return Err("`chord` must not be empty".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_home_chord_and_excludes() {
        // AC5: the first-launch default carries the home folder + Control+Space.
        let cfg = Config::default_with_home(PathBuf::from(r"C:\Users\test"));
        assert_eq!(cfg.folders, vec![PathBuf::from(r"C:\Users\test")]);
        assert_eq!(cfg.chord, "Control+Space");
        assert!(cfg.exclude.contains(&"node_modules".to_string()));
    }

    #[test]
    fn default_roundtrips_through_json() {
        // AC5: what we write on first launch parses back identically.
        let cfg = Config::default_with_home(PathBuf::from(r"C:\Users\test"));
        let back = Config::from_json(&cfg.to_json()).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn malformed_json_is_rejected() {
        // AC6: a broken file is an error, not a silent default.
        assert!(Config::from_json("{ not json").is_err());
    }

    #[test]
    fn empty_folders_is_rejected() {
        // AC6: schema-invalid (no folders) is an error.
        let json = r#"{"folders": [], "chord": "Control+Space"}"#;
        assert!(Config::from_json(json).is_err());
    }

    #[test]
    fn exclude_is_optional() {
        let json = r#"{"folders": ["C:\\x"], "chord": "Control+Space"}"#;
        let cfg = Config::from_json(json).unwrap();
        assert!(cfg.exclude.is_empty());
    }
}
