//! Agent-facing CLI (spec 007). Pure logic: given the argv tail and the resolved
//! config path, it reads / mutates `config.json` — validating with the same rules
//! the app applies on load — and returns stdout / stderr / an exit code for `main`
//! to emit. It never opens the persistent store or launches the GUI, so
//! `config.json` stays the single coordination point and the resident app applies
//! changes via config hot-reload (spec 006). Output is machine-readable JSON.

use std::path::{Path, PathBuf};

use global_hotkey::hotkey::HotKey;

use crate::config::Config;

/// The result of a CLI invocation: text for stdout, text for stderr, exit code.
pub struct CliOutcome {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

impl CliOutcome {
    fn ok(stdout: String) -> Self {
        Self {
            stdout,
            stderr: String::new(),
            code: 0,
        }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self {
            stdout: String::new(),
            stderr: msg.into(),
            code: 2,
        }
    }
}

const LIST_KEYS: [&str; 2] = ["folders", "exclude"];
const SCALAR_KEYS: [&str; 2] = ["chord", "git_aware"];

/// Run a CLI subcommand. `args` is the argv tail (everything after the program
/// name) and is non-empty — no-args launches the tray, handled by the caller.
pub fn run(args: &[String], config_path: &Path, home: &Path) -> CliOutcome {
    if args.len() > 4 {
        return CliOutcome::err(format!("too many arguments: {}", args.join(" ")));
    }
    let a0 = args.first().map(String::as_str);
    let a1 = args.get(1).map(String::as_str);
    let a2 = args.get(2).map(String::as_str);
    let a3 = args.get(3).map(String::as_str);
    match (a0, a1, a2, a3) {
        (Some("--version") | Some("-V"), None, None, None) => {
            CliOutcome::ok(format!("atref {}", env!("CARGO_PKG_VERSION")))
        }
        (Some("--help") | Some("-h"), None, None, None) => CliOutcome::ok(help_text(config_path)),
        (Some("describe"), None, None, None) => CliOutcome::ok(describe_json(config_path)),
        (Some("add"), path, None, None) => add_folder(path, config_path, home),
        (Some("config"), None, None, None) | (Some("config"), Some("get"), None, None) => {
            get(None, config_path, home)
        }
        (Some("config"), Some("get"), Some(key), None) => get(Some(key), config_path, home),
        (Some("config"), Some("set"), Some(key), Some(val)) => set(key, val, config_path, home),
        (Some("config"), Some("add"), Some(key), Some(val)) => {
            add_list(key, val, config_path, home)
        }
        (Some("config"), Some("remove"), Some(key), Some(val)) => {
            remove_list(key, val, config_path, home)
        }
        _ => CliOutcome::err(format!(
            "unknown command: {}\nrun `atref describe` (JSON) or `atref --help`.",
            args.join(" ")
        )),
    }
}

// --- commands ---------------------------------------------------------------

fn get(key: Option<&str>, config_path: &Path, home: &Path) -> CliOutcome {
    let cfg = match load_or_default(config_path, home) {
        Ok(c) => c,
        Err(e) => return CliOutcome::err(e),
    };
    match key {
        None => CliOutcome::ok(cfg.to_json()),
        Some("folders") => CliOutcome::ok(pretty(&cfg.folders)),
        Some("exclude") => CliOutcome::ok(pretty(&cfg.exclude)),
        Some("chord") => CliOutcome::ok(pretty(&cfg.chord)),
        Some("git_aware") => CliOutcome::ok(pretty(&cfg.git_aware)),
        Some(other) => CliOutcome::err(unknown_key(other)),
    }
}

fn set(key: &str, value: &str, config_path: &Path, home: &Path) -> CliOutcome {
    let mut cfg = match load_or_default(config_path, home) {
        Ok(c) => c,
        Err(e) => return CliOutcome::err(e),
    };
    match key {
        "chord" => {
            if let Err(e) = value.parse::<HotKey>() {
                return CliOutcome::err(format!("invalid chord '{value}': {e}"));
            }
            cfg.chord = value.to_string();
        }
        "git_aware" => match value {
            "true" => cfg.git_aware = true,
            "false" => cfg.git_aware = false,
            _ => {
                return CliOutcome::err(format!("`git_aware` must be true or false, got '{value}'"))
            }
        },
        k if LIST_KEYS.contains(&k) => {
            return CliOutcome::err(format!(
                "`{k}` is a list — use `config add {k} <value>` / `config remove {k} <value>`"
            ));
        }
        other => return CliOutcome::err(unknown_key(other)),
    }
    commit(cfg, config_path, "set", key, value, true)
}

fn add_list(key: &str, value: &str, config_path: &Path, home: &Path) -> CliOutcome {
    let mut cfg = match load_or_default(config_path, home) {
        Ok(c) => c,
        Err(e) => return CliOutcome::err(e),
    };
    match key {
        "folders" => {
            let abs = match absolutize(value) {
                Ok(a) => a,
                Err(e) => return CliOutcome::err(e),
            };
            let p = PathBuf::from(&abs);
            if cfg.folders.contains(&p) {
                return commit(cfg, config_path, "add", "folders", &abs, false);
            }
            cfg.folders.push(p);
            commit(cfg, config_path, "add", "folders", &abs, true)
        }
        "exclude" => {
            if cfg.exclude.iter().any(|e| e == value) {
                return commit(cfg, config_path, "add", "exclude", value, false);
            }
            cfg.exclude.push(value.to_string());
            commit(cfg, config_path, "add", "exclude", value, true)
        }
        k if SCALAR_KEYS.contains(&k) => {
            CliOutcome::err(format!("`{k}` is a scalar — use `config set {k} <value>`"))
        }
        other => CliOutcome::err(unknown_key(other)),
    }
}

fn remove_list(key: &str, value: &str, config_path: &Path, home: &Path) -> CliOutcome {
    let mut cfg = match load_or_default(config_path, home) {
        Ok(c) => c,
        Err(e) => return CliOutcome::err(e),
    };
    match key {
        "folders" => {
            let abs = match absolutize(value) {
                Ok(a) => a,
                Err(e) => return CliOutcome::err(e),
            };
            let p = PathBuf::from(&abs);
            if !cfg.folders.contains(&p) {
                return commit(cfg, config_path, "remove", "folders", &abs, false);
            }
            if cfg.folders.len() == 1 {
                return CliOutcome::err(
                    "refusing to remove the last folder — `folders` must list at least one directory"
                        .to_string(),
                );
            }
            cfg.folders.retain(|f| f != &p);
            commit(cfg, config_path, "remove", "folders", &abs, true)
        }
        "exclude" => {
            if !cfg.exclude.iter().any(|e| e == value) {
                return commit(cfg, config_path, "remove", "exclude", value, false);
            }
            cfg.exclude.retain(|e| e != value);
            commit(cfg, config_path, "remove", "exclude", value, true)
        }
        k if SCALAR_KEYS.contains(&k) => {
            CliOutcome::err(format!("`{k}` is a scalar — use `config set {k} <value>`"))
        }
        other => CliOutcome::err(unknown_key(other)),
    }
}

/// `atref add [PATH]` — the headline shortcut for `config add folders <PATH|cwd>`.
fn add_folder(path: Option<&str>, config_path: &Path, home: &Path) -> CliOutcome {
    let owned;
    let p = match path {
        Some(p) => p,
        None => match std::env::current_dir() {
            Ok(d) => {
                owned = d.to_string_lossy().into_owned();
                owned.as_str()
            }
            Err(e) => return CliOutcome::err(format!("cannot resolve current directory: {e}")),
        },
    };
    add_list("folders", p, config_path, home)
}

// --- helpers ----------------------------------------------------------------

/// Read + validate `config.json`, or fall back to the first-run default (without
/// writing — a write only happens after a successful mutation, atomically).
fn load_or_default(config_path: &Path, home: &Path) -> Result<Config, String> {
    if config_path.exists() {
        let text = std::fs::read_to_string(config_path)
            .map_err(|e| format!("cannot read {}: {e}", config_path.display()))?;
        Config::from_json(&text)
    } else {
        Ok(Config::default_with_home(home.to_path_buf()))
    }
}

/// Validate the mutated config and, if anything changed, write it atomically
/// (temp file + rename, so the hot-reload watcher never sees a half file).
fn commit(
    cfg: Config,
    config_path: &Path,
    action: &str,
    key: &str,
    value: &str,
    changed: bool,
) -> CliOutcome {
    if changed {
        if let Err(e) = cfg.validate() {
            return CliOutcome::err(e);
        }
        if let Err(e) = write_atomic(config_path, &cfg) {
            return CliOutcome::err(e);
        }
    }
    CliOutcome::ok(
        serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "action": action,
            "key": key,
            "value": value,
            "changed": changed,
            "config_path": config_path.to_string_lossy(),
        }))
        .unwrap_or_default(),
    )
}

fn write_atomic(config_path: &Path, cfg: &Config) -> Result<(), String> {
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let name = config_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("config.json");
    let tmp = config_path.with_file_name(format!("{name}.tmp.{}", std::process::id()));
    std::fs::write(&tmp, cfg.to_json())
        .map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, config_path)
        .map_err(|e| format!("cannot replace {}: {e}", config_path.display()))?;
    Ok(())
}

fn absolutize(value: &str) -> Result<String, String> {
    std::path::absolute(value)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| format!("cannot resolve path '{value}': {e}"))
}

fn pretty<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

fn unknown_key(k: &str) -> String {
    format!("unknown config key '{k}'; valid keys: folders, exclude, chord, git_aware")
}

fn describe_json(config_path: &Path) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "name": "atref",
        "description": "Global file-reference picker; CLI to configure it for agents.",
        "usage": "atref <command> [args]",
        "config_path": config_path.to_string_lossy(),
        "commands": {
            "describe": "Print this schema as JSON.",
            "config get [KEY]": "Print the whole config, or one KEY, as JSON.",
            "config set <KEY> <VALUE>": "Set a scalar field (chord, git_aware).",
            "config add <KEY> <VALUE>": "Add to a list field (folders, exclude); idempotent.",
            "config remove <KEY> <VALUE>": "Remove from a list field (folders, exclude); idempotent.",
            "add [PATH]": "Shortcut: add PATH (default: current directory) to folders.",
            "(no args)": "Launch the tray app."
        },
        "fields": {
            "folders":   { "kind": "list",   "type": "absolute path", "default": "[home]",                       "validation": "must stay non-empty; values normalized to absolute" },
            "exclude":   { "kind": "list",   "type": "string",        "default": [".git", "node_modules", "target"] },
            "chord":     { "kind": "scalar", "type": "string",        "default": "Control+Space",                 "validation": "must parse as a global-hotkey chord" },
            "git_aware": { "kind": "scalar", "type": "bool",          "default": true }
        }
    }))
    .unwrap_or_default()
}

fn help_text(config_path: &Path) -> String {
    format!(
        "atref — global file-reference picker.

USAGE:
  atref                            launch the tray app
  atref describe                   print the command + config schema as JSON
  atref config get [KEY]           print the whole config, or one KEY, as JSON
  atref config set <KEY> <VALUE>   set a scalar field (chord, git_aware)
  atref config add <KEY> <VALUE>   add to a list field (folders, exclude)
  atref config remove <KEY> <VALUE>  remove from a list field
  atref add [PATH]                 add PATH (default: current dir) to folders
  atref --version | -V             print version
  atref --help | -h                print this help

config: {}",
        config_path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("atref_cli_{tag}_{}", std::process::id()))
    }

    fn setup(tag: &str) -> (PathBuf, PathBuf) {
        let dir = temp_dir(tag);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        (dir.join("config.json"), home)
    }

    fn run_args(parts: &[&str], cfg: &Path, home: &Path) -> CliOutcome {
        let v: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
        run(&v, cfg, home)
    }

    fn load(cfg: &Path) -> Config {
        Config::from_json(&std::fs::read_to_string(cfg).unwrap()).unwrap()
    }

    #[test]
    fn describe_is_valid_json_with_fields_and_path() {
        // AC2: describe emits valid JSON naming the commands, every field, and the path.
        let (cfg, home) = setup("describe");
        let out = run_args(&["describe"], &cfg, &home);
        assert_eq!(out.code, 0);
        let v: serde_json::Value = serde_json::from_str(&out.stdout).expect("valid JSON");
        assert!(v["commands"]["describe"].is_string());
        for key in ["folders", "exclude", "chord", "git_aware"] {
            assert!(v["fields"][key]["kind"].is_string(), "field {key} present");
        }
        assert_eq!(v["config_path"], serde_json::json!(cfg.to_string_lossy()));
    }

    #[test]
    fn add_folder_creates_default_and_normalizes_and_is_idempotent() {
        // AC4 + AC7 + AC10: missing config -> created from default; path stored
        // absolute; a second add is a no-op (changed=false).
        let (cfg, home) = setup("addfolder");
        let target = temp_dir("addfolder").join("proj");
        std::fs::create_dir_all(&target).unwrap();

        let out = run_args(
            &["config", "add", "folders", target.to_str().unwrap()],
            &cfg,
            &home,
        );
        assert_eq!(out.code, 0, "{}", out.stderr);
        assert!(cfg.exists(), "config.json created from default");
        let loaded = load(&cfg);
        let abs = std::path::absolute(&target).unwrap();
        assert!(loaded.folders.contains(&abs), "absolute path stored");

        let again = run_args(
            &["config", "add", "folders", target.to_str().unwrap()],
            &cfg,
            &home,
        );
        let v: serde_json::Value = serde_json::from_str(&again.stdout).unwrap();
        assert_eq!(v["changed"], serde_json::json!(false), "idempotent");
        assert_eq!(load(&cfg).folders.iter().filter(|f| **f == abs).count(), 1);
    }

    #[test]
    fn set_chord_validates() {
        // AC6: a valid chord persists; an unparseable one is rejected, file unchanged.
        let (cfg, home) = setup("chord");
        assert_eq!(
            run_args(&["config", "set", "chord", "Control+Shift+P"], &cfg, &home).code,
            0
        );
        assert_eq!(load(&cfg).chord, "Control+Shift+P");

        let before = std::fs::read(&cfg).unwrap();
        let bad = run_args(&["config", "set", "chord", "@@@"], &cfg, &home);
        assert_eq!(bad.code, 2);
        assert!(bad.stderr.contains("invalid chord"));
        assert_eq!(
            std::fs::read(&cfg).unwrap(),
            before,
            "file unchanged on invalid"
        );
    }

    #[test]
    fn set_git_aware_bool_only() {
        // AC5: bool persists; a non-bool is rejected.
        let (cfg, home) = setup("gitaware");
        assert_eq!(
            run_args(&["config", "set", "git_aware", "false"], &cfg, &home).code,
            0
        );
        assert!(!load(&cfg).git_aware);
        assert_eq!(
            run_args(&["config", "set", "git_aware", "nope"], &cfg, &home).code,
            2
        );
    }

    #[test]
    fn remove_last_folder_is_refused() {
        // AC9: cannot empty `folders`.
        let (cfg, home) = setup("lastfolder");
        // default has exactly one folder (home)
        run_args(&["config", "get"], &cfg, &home); // no-op read
        let only = std::path::absolute(&home).unwrap();
        // ensure config exists with the single home folder
        run_args(&["config", "add", "exclude", "x"], &cfg, &home);
        let out = run_args(
            &["config", "remove", "folders", only.to_str().unwrap()],
            &cfg,
            &home,
        );
        assert_eq!(out.code, 2, "removing the last folder is refused");
        assert!(out.stderr.contains("last folder"));
    }

    #[test]
    fn exclude_add_remove_idempotent() {
        // AC8
        let (cfg, home) = setup("exclude");
        assert_eq!(
            run_args(&["config", "add", "exclude", "dist"], &cfg, &home).code,
            0
        );
        assert!(load(&cfg).exclude.contains(&"dist".to_string()));
        let again = run_args(&["config", "add", "exclude", "dist"], &cfg, &home);
        let v: serde_json::Value = serde_json::from_str(&again.stdout).unwrap();
        assert_eq!(v["changed"], serde_json::json!(false));
        assert_eq!(
            run_args(&["config", "remove", "exclude", "dist"], &cfg, &home).code,
            0
        );
        assert!(!load(&cfg).exclude.contains(&"dist".to_string()));
        // removing an absent value is a no-op success
        let gone = run_args(&["config", "remove", "exclude", "ghost"], &cfg, &home);
        assert_eq!(gone.code, 0);
    }

    #[test]
    fn unknown_command_and_key_error() {
        // AC3
        let (cfg, home) = setup("unknown");
        assert_eq!(run_args(&["frobnicate"], &cfg, &home).code, 2);
        assert_eq!(run_args(&["config", "get", "nope"], &cfg, &home).code, 2);
        assert_eq!(run_args(&["--version"], &cfg, &home).code, 0);
    }

    #[test]
    fn get_one_key() {
        // AC1
        let (cfg, home) = setup("getkey");
        run_args(&["config", "set", "git_aware", "true"], &cfg, &home);
        let out = run_args(&["config", "get", "git_aware"], &cfg, &home);
        assert_eq!(out.code, 0);
        assert_eq!(out.stdout.trim(), "true");
    }

    #[test]
    fn add_shortcut_adds_folder_and_defaults_to_cwd() {
        // AC10: `atref add <path>` == `config add folders <path>`; bare `atref add`
        // uses the current working directory.
        let (cfg, home) = setup("addshortcut");
        let target = temp_dir("addshortcut").join("viaShortcut");
        std::fs::create_dir_all(&target).unwrap();
        assert_eq!(
            run_args(&["add", target.to_str().unwrap()], &cfg, &home).code,
            0
        );
        let abs = std::path::absolute(&target).unwrap();
        assert!(
            load(&cfg).folders.contains(&abs),
            "shortcut adds the folder"
        );

        assert_eq!(run_args(&["add"], &cfg, &home).code, 0);
        let cwd = std::path::absolute(std::env::current_dir().unwrap()).unwrap();
        assert!(load(&cfg).folders.contains(&cwd), "bare add uses the CWD");
    }
}
