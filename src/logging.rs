use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct ErrorLogger {
    path: PathBuf,
}

impl ErrorLogger {
    pub fn new(app_dir: &Path) -> Result<Self> {
        fs::create_dir_all(app_dir)
            .with_context(|| format!("failed to create app directory: {}", app_dir.display()))?;
        Ok(Self {
            path: app_dir.join("error.log"),
        })
    }

    pub fn log_error(&self, message: &str) {
        let timestamp = unix_timestamp_ms();
        let line = format!("[{timestamp}] {message}\n");

        let mut file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            Ok(file) => file,
            Err(_) => return,
        };

        let _ = file.write_all(line.as_bytes());
    }
}

fn unix_timestamp_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as i64,
        Err(_) => 0,
    }
}
