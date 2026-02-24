use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub trigger_key: char,
    pub double_tap_ms: u16,
    pub launch_at_login: bool,
    pub hide_completed: bool,
    pub max_visible_plates: u16,
    pub min_diameter_px: u16,
    pub max_diameter_px: u16,
    pub complete_fade_ms: u16,
    pub input_hotkey: char,
    pub list_mode_hotkey: char,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            trigger_key: 'J',
            double_tap_ms: 250,
            launch_at_login: true,
            hide_completed: false,
            max_visible_plates: 40,
            min_diameter_px: 168,
            max_diameter_px: 440,
            complete_fade_ms: 700,
            input_hotkey: 'N',
            list_mode_hotkey: 'M',
        }
    }
}

impl AppConfig {
    pub fn sanitize(&mut self) {
        self.trigger_key = sanitize_hotkey(self.trigger_key, 'J');
        self.input_hotkey = sanitize_hotkey(self.input_hotkey, 'N');
        self.list_mode_hotkey = sanitize_hotkey(self.list_mode_hotkey, 'M');

        if self.double_tap_ms < 120 || self.double_tap_ms > 600 {
            self.double_tap_ms = 250;
        }

        if self.max_visible_plates == 0 || self.max_visible_plates > 200 {
            self.max_visible_plates = 40;
        }

        if self.min_diameter_px < 168 || self.min_diameter_px > 520 {
            self.min_diameter_px = 168;
        }
        if self.max_diameter_px < 320 || self.max_diameter_px > 760 {
            self.max_diameter_px = 440;
        }
        if self.min_diameter_px > self.max_diameter_px {
            std::mem::swap(&mut self.min_diameter_px, &mut self.max_diameter_px);
        }

        if self.complete_fade_ms < 200 || self.complete_fade_ms > 3000 {
            self.complete_fade_ms = 700;
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(app_dir: &Path) -> Result<Self> {
        fs::create_dir_all(app_dir)
            .with_context(|| format!("failed to create app directory: {}", app_dir.display()))?;
        Ok(Self {
            path: app_dir.join("config.json"),
        })
    }

    pub fn load(&self) -> Result<AppConfig> {
        if !self.path.exists() {
            return Ok(AppConfig::default());
        }

        let content = fs::read_to_string(&self.path)
            .with_context(|| format!("failed to read {}", self.path.display()))?;
        let mut config: AppConfig = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", self.path.display()))?;
        config.sanitize();
        Ok(config)
    }

    pub fn save(&self, config: &AppConfig) -> Result<()> {
        atomic_write_json(&self.path, config)
    }
}

pub fn sanitize_trigger_key(key: char) -> char {
    sanitize_hotkey(key, 'J')
}

pub fn sanitize_hotkey(key: char, fallback: char) -> char {
    let normalized = key.to_ascii_uppercase();
    if normalized.is_ascii_uppercase() {
        normalized
    } else {
        fallback
    }
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("missing parent directory for {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;

    let payload = serde_json::to_vec_pretty(value).context("failed to serialize config")?;
    let tmp_path = path.with_extension("tmp");

    fs::write(&tmp_path, payload)
        .with_context(|| format!("failed to write temp file {}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).with_context(|| format!("failed to replace {}", path.display()))?;

    Ok(())
}
