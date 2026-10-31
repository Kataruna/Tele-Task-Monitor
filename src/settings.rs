//! Persistent user settings (`settings.json`) for port, default theme, and process list limits.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct UserSettings {
    pub port: u16,
    pub default_theme: String,
    pub overview_limit: usize,
    pub page_limit: usize,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            port: 8800,
            default_theme: "nier".to_string(),
            overview_limit: 8,
            page_limit: 24,
        }
    }
}

impl UserSettings {
    pub fn sanitize(mut self) -> Self {
        if self.port < 1024 {
            self.port = 8800;
        }
        let valid_id = !self.default_theme.is_empty()
            && self.default_theme.len() <= 40
            && self
                .default_theme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid_id {
            self.default_theme = "nier".to_string();
        }
        self.overview_limit = self.overview_limit.clamp(1, 25);
        self.page_limit = self.page_limit.clamp(1, 60);
        self
    }
}

#[derive(Clone)]
pub struct SettingsStore {
    inner: Arc<RwLock<UserSettings>>,
    path: Option<PathBuf>,
}

impl SettingsStore {
    pub fn load_or_default(exe_dir: Option<&Path>) -> Self {
        let path = Self::resolve_path(exe_dir);
        let settings = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<UserSettings>(&s).ok())
            .unwrap_or_default()
            .sanitize();
        crate::aggregate::set_top_n(settings.page_limit.max(settings.overview_limit));
        Self {
            inner: Arc::new(RwLock::new(settings)),
            path,
        }
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        let s = UserSettings::default();
        Self {
            inner: Arc::new(RwLock::new(s)),
            path: None,
        }
    }

    fn resolve_path(exe_dir: Option<&Path>) -> Option<PathBuf> {
        let cwd_file = PathBuf::from("settings.json");
        if cwd_file.is_file() {
            return Some(cwd_file);
        }
        if let Some(dir) = exe_dir {
            let exe_file = dir.join("settings.json");
            if exe_file.is_file() || dir.join("web").is_dir() {
                return Some(exe_file);
            }
        }
        Some(cwd_file)
    }

    pub fn get(&self) -> UserSettings {
        self.inner.read().expect("settings lock").clone()
    }

    pub fn update(&self, next: UserSettings) -> Result<UserSettings, String> {
        let clean = next.sanitize();
        crate::aggregate::set_top_n(clean.page_limit.max(clean.overview_limit));
        if let Some(p) = &self.path {
            let json = serde_json::to_string_pretty(&clean).map_err(|e| e.to_string())?;
            if let Err(e) = std::fs::write(p, json) {
                eprintln!("warning: could not write {}: {e}", p.display());
            }
        }
        *self.inner.write().expect("settings lock") = clean.clone();
        Ok(clean)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_clamps_bounds_and_validates_theme() {
        let raw = UserSettings {
            port: 80,
            default_theme: "../bad".into(),
            overview_limit: 0,
            page_limit: 999,
        };
        let clean = raw.sanitize();
        assert_eq!(clean.port, 8800);
        assert_eq!(clean.default_theme, "nier");
        assert_eq!(clean.overview_limit, 1);
        assert_eq!(clean.page_limit, 60);
    }

    #[test]
    fn store_updates_in_memory() {
        let store = SettingsStore::in_memory();
        let updated = store
            .update(UserSettings {
                port: 8899,
                default_theme: "doraemon".into(),
                overview_limit: 6,
                page_limit: 30,
            })
            .unwrap();
        assert_eq!(updated.port, 8899);
        assert_eq!(updated.default_theme, "doraemon");
        assert_eq!(updated.overview_limit, 6);
        assert_eq!(updated.page_limit, 30);
        assert_eq!(store.get(), updated);
    }
}
