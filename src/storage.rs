use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub text: String,
    pub completed: bool,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    #[serde(default)]
    pub completed_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct TodoFile {
    todos: Vec<TodoItem>,
}

#[derive(Debug, Clone)]
pub struct TodoStore {
    path: PathBuf,
}

impl TodoStore {
    pub fn new(app_dir: &Path) -> Result<Self> {
        fs::create_dir_all(app_dir)
            .with_context(|| format!("failed to create app directory: {}", app_dir.display()))?;
        Ok(Self {
            path: app_dir.join("todos.json"),
        })
    }

    pub fn load(&self) -> Result<Vec<TodoItem>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.path)
            .with_context(|| format!("failed to read {}", self.path.display()))?;
        let file: TodoFile = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", self.path.display()))?;
        Ok(file.todos)
    }

    pub fn save(&self, todos: &[TodoItem]) -> Result<()> {
        let file = TodoFile {
            todos: todos.to_vec(),
        };
        atomic_write_json(&self.path, &file)
    }
}

pub fn now_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as i64,
        Err(_) => 0,
    }
}

pub fn make_todo(text: String) -> TodoItem {
    let now = now_ms();
    TodoItem {
        id: Uuid::new_v4().to_string(),
        text,
        completed: false,
        created_at_ms: now,
        updated_at_ms: now,
        completed_at_ms: None,
    }
}

pub fn normalize_text(raw: &str) -> String {
    raw.replace(['\n', '\r', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn mark_completed(item: &mut TodoItem) {
    let now = now_ms();
    item.completed = true;
    item.completed_at_ms = Some(now);
    item.updated_at_ms = now;
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("missing parent directory for {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;

    let payload = serde_json::to_vec_pretty(value).context("failed to serialize todos")?;
    let tmp_path = path.with_extension("tmp");

    fs::write(&tmp_path, payload)
        .with_context(|| format!("failed to write temp file {}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).with_context(|| format!("failed to replace {}", path.display()))?;

    Ok(())
}
