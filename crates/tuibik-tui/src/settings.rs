//! User preferences persisted in the store's config table.

use store::Store;

const KEY_INSPECTION: &str = "inspection";
const KEY_SHOW_RUNNING: &str = "show_running_time";

/// Persisted preferences (the theme is persisted separately under `theme`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// WCA inspection before the solve (default off).
    pub inspection: bool,
    /// Show the running time while solving (default on).
    pub show_running_time: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            inspection: false,
            show_running_time: true,
        }
    }
}

/// A row in the settings overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Inspection,
    ShowRunningTime,
    Theme,
}

pub const ROWS: [Row; 3] = [Row::Inspection, Row::ShowRunningTime, Row::Theme];

fn parse_bool(v: Option<String>, default: bool) -> bool {
    match v.as_deref() {
        Some("true") | Some("on") | Some("1") => true,
        Some("false") | Some("off") | Some("0") => false,
        _ => default,
    }
}

impl Settings {
    /// Load from the store; missing or unrecognised values use the defaults.
    pub fn load(store: &Store) -> Settings {
        let d = Settings::default();
        Settings {
            inspection: parse_bool(
                store.get_config(KEY_INSPECTION).ok().flatten(),
                d.inspection,
            ),
            show_running_time: parse_bool(
                store.get_config(KEY_SHOW_RUNNING).ok().flatten(),
                d.show_running_time,
            ),
        }
    }

    /// Persist all settings (errors are ignored so a DB hiccup can't crash the UI).
    pub fn save(&self, store: &Store) {
        let _ = store.set_config(
            KEY_INSPECTION,
            if self.inspection { "true" } else { "false" },
        );
        let _ = store.set_config(
            KEY_SHOW_RUNNING,
            if self.show_running_time {
                "true"
            } else {
                "false"
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_missing() {
        let store = Store::open_in_memory().unwrap();
        let s = Settings::load(&store);
        assert!(!s.inspection);
        assert!(s.show_running_time);
    }

    #[test]
    fn persists_across_reopen() {
        let dir = std::env::temp_dir().join(format!("tuibik-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("db.sqlite");
        {
            let store = Store::open(&path).unwrap();
            Settings {
                inspection: true,
                show_running_time: false,
            }
            .save(&store);
        }
        let store = Store::open(&path).unwrap();
        let s = Settings::load(&store);
        assert!(s.inspection);
        assert!(!s.show_running_time);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_value_falls_back_to_default() {
        let store = Store::open_in_memory().unwrap();
        store.set_config("inspection", "banana").unwrap();
        store.set_config("show_running_time", "banana").unwrap();
        let s = Settings::load(&store);
        assert!(!s.inspection, "banana -> inspection off");
        assert!(s.show_running_time, "banana -> default on");
    }
}
