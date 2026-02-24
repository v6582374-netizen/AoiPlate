use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicU8;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use auto_launch::{AutoLaunch, AutoLaunchBuilder, MacOSLaunchMode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use uuid::Uuid;

use crate::config::{AppConfig, ConfigStore, sanitize_trigger_key};
use crate::hotkey;
use crate::logging::ErrorLogger;
use crate::panel::Panel;
use crate::permissions::{self, PermissionState};
use crate::storage::{TodoItem, TodoStore, make_todo, mark_completed, normalize_text, now_ms};
use crate::tray::{self, TrayCommand, TrayController};

const APP_NAME: &str = "AoiPlate";
const LEGACY_APP_NAME: &str = "TodoLite";
const BUNDLE_ID: &str = "com.shiwen.aoiplate";
const SAVE_DEBOUNCE_MS: u64 = 100;
const HIDE_ANIMATION_MS: u64 = 210;
const MAX_IPC_PAYLOAD_BYTES: usize = 16 * 1024;

#[derive(Debug)]
enum UserEvent {
    HotkeyTrigger,
    HideOverlay,
    FinalizeHide(u64),
    FlushSave(u64),
    IpcRaw(String),
    TrayCommand(TrayCommand),
    HotkeyError(String),
    RefreshPermissions,
    Quit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ViewMode {
    Explosion,
    List,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::Explosion
    }
}

#[derive(Debug, Deserialize)]
struct ClientMessage {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    payload: Value,
}

#[derive(Debug, Deserialize)]
struct IdPayload {
    id: String,
}

#[derive(Debug, Deserialize)]
struct AddTodoPayload {
    text: String,
}

#[derive(Debug, Deserialize)]
struct EditTodoPayload {
    id: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct TriggerPayload {
    key: String,
}

#[derive(Debug, Deserialize)]
struct LaunchAtLoginPayload {
    launch_at_login: bool,
}

#[derive(Debug, Deserialize)]
struct CompleteTodoPayload {
    id: String,
    #[allow(dead_code)]
    via: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ViewModePayload {
    mode: String,
}

struct RuntimeState {
    todos: Vec<TodoItem>,
    config: AppConfig,
    permissions: PermissionState,
    hotkey_started: bool,
    overlay_visible: bool,
    view_mode: ViewMode,
    hide_token: u64,
    save_token: u64,
}

struct AppDirResolution {
    current: PathBuf,
    migrated: bool,
    warnings: Vec<String>,
}

struct LaunchAtLogin {
    inner: Option<AutoLaunch>,
}

impl LaunchAtLogin {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        {
            let exe = std::env::current_exe().ok();
            let auto = exe.and_then(|path| {
                let path_str = path.to_str()?.to_string();
                let mut builder = AutoLaunchBuilder::new();
                builder
                    .set_app_name(APP_NAME)
                    .set_app_path(&path_str)
                    .set_macos_launch_mode(MacOSLaunchMode::LaunchAgent)
                    .set_bundle_identifiers(&[BUNDLE_ID]);
                builder.build().ok()
            });
            return Self { inner: auto };
        }

        #[allow(unreachable_code)]
        Self { inner: None }
    }

    fn set_enabled(&self, enabled: bool) -> Result<()> {
        if let Some(auto) = &self.inner {
            if enabled {
                auto.enable().context("failed to enable launch-at-login")?;
            } else {
                auto.disable()
                    .context("failed to disable launch-at-login")?;
            }
        }
        Ok(())
    }
}

pub fn run() -> Result<()> {
    let app_dir_resolution =
        resolve_app_data_dir().context("failed to resolve app data directory")?;
    let app_dir = app_dir_resolution.current.clone();
    let logger = ErrorLogger::new(&app_dir)?;
    for warning in &app_dir_resolution.warnings {
        eprintln!("{warning}");
        logger.log_error(warning);
    }

    let config_store = ConfigStore::new(&app_dir)?;
    let mut config = config_store.load().unwrap_or_default();
    config.sanitize();

    let todo_store = TodoStore::new(&app_dir)?;
    let mut todos = todo_store.load().unwrap_or_default();
    if todos.is_empty() {
        todos = seeded_todos();
        if let Err(err) = todo_store.save(&todos) {
            logger.log_error(&format!("seed todos failed: {err:#}"));
        }
    }

    let trigger_key = hotkey::trigger_key_atomic(config.trigger_key);

    permissions::prompt_accessibility_if_needed();
    let permissions = permissions::detect_permissions();

    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }

    let proxy = event_loop.create_proxy();

    tray::install_event_handlers({
        let proxy = proxy.clone();
        move |cmd| {
            let _ = proxy.send_event(UserEvent::TrayCommand(cmd));
        }
    });

    let ui_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui");
    let panel = Panel::new(&event_loop, &ui_root, {
        let proxy = proxy.clone();
        move |raw| {
            let _ = proxy.send_event(UserEvent::IpcRaw(raw));
        }
    })?;
    let panel_window_id = panel.window.id();

    let tray = TrayController::new(config.launch_at_login)?;
    let launcher = LaunchAtLogin::new();
    if app_dir_resolution.migrated && config.launch_at_login {
        if let Err(err) = launcher.set_enabled(false) {
            logger.log_error(&format!(
                "launch-at-login refresh (disable old registration) failed: {err:#}"
            ));
        }
    }
    if let Err(err) = launcher.set_enabled(config.launch_at_login) {
        logger.log_error(&format!("launch-at-login sync failed: {err:#}"));
    }

    let mut runtime = RuntimeState {
        todos,
        config,
        permissions,
        hotkey_started: false,
        overlay_visible: false,
        view_mode: ViewMode::Explosion,
        hide_token: 0,
        save_token: 0,
    };

    start_permission_polling(proxy.clone());
    start_hotkey_if_possible(&mut runtime, &trigger_key, proxy.clone(), logger.clone());

    push_state_to_ui(&panel, &runtime);
    push_permissions_to_ui(&panel, &runtime.permissions);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(user_event) => match user_event {
                UserEvent::HotkeyTrigger => toggle_overlay(&panel, &mut runtime, &proxy),
                UserEvent::HideOverlay => hide_overlay(&panel, &mut runtime, &proxy),
                UserEvent::FinalizeHide(token) => {
                    if !runtime.overlay_visible && token == runtime.hide_token {
                        panel.hide_native();
                    }
                }
                UserEvent::FlushSave(token) => {
                    if token == runtime.save_token {
                        if let Err(err) = todo_store.save(&runtime.todos) {
                            logger.log_error(&format!("save todos failed: {err:#}"));
                            emit_error(&panel, "无法保存代办，请检查磁盘权限或空间");
                        }
                    }
                }
                UserEvent::IpcRaw(raw) => {
                    if let Err(err) = handle_ipc(
                        &raw,
                        &panel,
                        &proxy,
                        &mut runtime,
                        &trigger_key,
                        &config_store,
                        &launcher,
                        &tray,
                        &logger,
                    ) {
                        logger.log_error(&format!("ipc handling failed: {err:#}"));
                        emit_error(&panel, "处理请求失败，请稍后重试");
                    }
                }
                UserEvent::TrayCommand(cmd) => match cmd {
                    TrayCommand::TogglePanel => toggle_overlay(&panel, &mut runtime, &proxy),
                    TrayCommand::OpenPermissions => {
                        if let Err(err) = permissions::open_permissions_settings() {
                            logger.log_error(&format!("open settings failed: {err:#}"));
                            emit_error(&panel, "无法打开系统权限设置");
                        }
                    }
                    TrayCommand::ToggleLaunchAtLogin => {
                        runtime.config.launch_at_login = !runtime.config.launch_at_login;
                        tray.set_launch_at_login_checked(runtime.config.launch_at_login);
                        if let Err(err) = launcher.set_enabled(runtime.config.launch_at_login) {
                            logger.log_error(&format!("toggle launch-at-login failed: {err:#}"));
                            emit_error(&panel, "切换开机启动失败");
                        }
                        if let Err(err) = config_store.save(&runtime.config) {
                            logger.log_error(&format!("save config failed: {err:#}"));
                        }
                        push_state_to_ui(&panel, &runtime);
                    }
                    TrayCommand::Quit => {
                        *control_flow = ControlFlow::Exit;
                    }
                },
                UserEvent::HotkeyError(message) => {
                    logger.log_error(&message);
                    emit_error(&panel, "全局快捷键监听失败，已降级为菜单栏点击呼出");
                }
                UserEvent::RefreshPermissions => {
                    let latest = permissions::detect_permissions();
                    if latest != runtime.permissions {
                        runtime.permissions = latest.clone();
                        push_permissions_to_ui(&panel, &latest);
                        if latest.input_monitoring {
                            start_hotkey_if_possible(
                                &mut runtime,
                                &trigger_key,
                                proxy.clone(),
                                logger.clone(),
                            );
                        }
                    }
                }
                UserEvent::Quit => {
                    *control_flow = ControlFlow::Exit;
                }
            },
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } if window_id == panel_window_id => {
                hide_overlay(&panel, &mut runtime, &proxy);
            }
            _ => {}
        }
    });
}

fn handle_ipc(
    raw: &str,
    panel: &Panel,
    proxy: &EventLoopProxy<UserEvent>,
    runtime: &mut RuntimeState,
    trigger_key: &Arc<AtomicU8>,
    config_store: &ConfigStore,
    launcher: &LaunchAtLogin,
    tray: &TrayController,
    logger: &ErrorLogger,
) -> Result<()> {
    if raw.len() > MAX_IPC_PAYLOAD_BYTES {
        return Err(anyhow!("ipc payload too large"));
    }

    let msg: ClientMessage = serde_json::from_str(raw).context("invalid ipc payload")?;

    match msg.kind.as_str() {
        "request_state" => {
            push_state_to_ui(panel, runtime);
            push_permissions_to_ui(panel, &runtime.permissions);
        }
        "add_todo" => {
            let payload: AddTodoPayload = serde_json::from_value(msg.payload)?;
            let text = normalize_text(&payload.text);
            if text.is_empty() {
                return Ok(());
            }
            runtime.todos.push(make_todo(text));
            schedule_save(runtime, proxy);
            push_state_to_ui(panel, runtime);
        }
        "edit_todo" => {
            let payload: EditTodoPayload = serde_json::from_value(msg.payload)?;
            if !is_valid_todo_id(&payload.id) {
                return Ok(());
            }
            let text = normalize_text(&payload.text);
            if text.is_empty() {
                return Ok(());
            }

            if let Some(item) = runtime.todos.iter_mut().find(|todo| todo.id == payload.id) {
                item.text = text;
                item.updated_at_ms = now_ms();
                schedule_save(runtime, proxy);
                push_state_to_ui(panel, runtime);
            }
        }
        "delete_todo" => {
            let payload: IdPayload = serde_json::from_value(msg.payload)?;
            if !is_valid_todo_id(&payload.id) {
                return Ok(());
            }
            let before = runtime.todos.len();
            runtime.todos.retain(|todo| todo.id != payload.id);
            if runtime.todos.len() != before {
                schedule_save(runtime, proxy);
                push_state_to_ui(panel, runtime);
            }
        }
        "complete_todo" => {
            let payload: CompleteTodoPayload = serde_json::from_value(msg.payload)?;
            if !is_valid_todo_id(&payload.id) {
                return Ok(());
            }
            if mark_todo_completed(&mut runtime.todos, &payload.id) {
                schedule_save(runtime, proxy);
                push_state_to_ui(panel, runtime);
            }
        }
        "toggle_todo" => {
            let payload: IdPayload = serde_json::from_value(msg.payload)?;
            if !is_valid_todo_id(&payload.id) {
                return Ok(());
            }
            if mark_todo_completed(&mut runtime.todos, &payload.id) {
                schedule_save(runtime, proxy);
                push_state_to_ui(panel, runtime);
            }
        }
        "set_trigger_key" => {
            let payload: TriggerPayload = serde_json::from_value(msg.payload)?;
            if let Some(first) = payload.key.chars().next() {
                let normalized = sanitize_trigger_key(first);
                runtime.config.trigger_key = normalized;
                hotkey::set_trigger_key(trigger_key, normalized);
                if let Err(err) = config_store.save(&runtime.config) {
                    logger.log_error(&format!("save config failed: {err:#}"));
                }
                push_state_to_ui(panel, runtime);
            }
        }
        "set_view_mode" => {
            let payload: ViewModePayload = serde_json::from_value(msg.payload)?;
            runtime.view_mode = parse_view_mode(&payload.mode);
            push_state_to_ui(panel, runtime);
        }
        "open_input_modal" => {
            let payload = json!({
                "type": "open_input_modal",
                "payload": {}
            });
            panel.send_json(&payload);
        }
        "hide_overlay" | "hide_panel" => {
            let _ = proxy.send_event(UserEvent::HideOverlay);
        }
        "open_permissions" => {
            permissions::open_permissions_settings().context("failed to open system settings")?;
        }
        "set_launch_at_login" => {
            let payload: LaunchAtLoginPayload = serde_json::from_value(msg.payload)?;
            runtime.config.launch_at_login = payload.launch_at_login;
            tray.set_launch_at_login_checked(runtime.config.launch_at_login);
            launcher.set_enabled(runtime.config.launch_at_login)?;
            if let Err(err) = config_store.save(&runtime.config) {
                logger.log_error(&format!("save config failed: {err:#}"));
            }
            push_state_to_ui(panel, runtime);
        }
        "quit_app" => {
            let _ = proxy.send_event(UserEvent::Quit);
        }
        _ => {}
    }

    Ok(())
}

fn toggle_overlay(panel: &Panel, runtime: &mut RuntimeState, proxy: &EventLoopProxy<UserEvent>) {
    if runtime.overlay_visible {
        hide_overlay(panel, runtime, proxy);
    } else {
        show_overlay(panel, runtime);
    }
}

fn show_overlay(panel: &Panel, runtime: &mut RuntimeState) {
    runtime.overlay_visible = true;
    panel.show();
    panel.set_visible_animated(true);
    push_state_to_ui(panel, runtime);
}

fn hide_overlay(panel: &Panel, runtime: &mut RuntimeState, proxy: &EventLoopProxy<UserEvent>) {
    runtime.overlay_visible = false;
    runtime.hide_token = runtime.hide_token.wrapping_add(1);
    let token = runtime.hide_token;

    panel.set_visible_animated(false);

    let proxy = proxy.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(HIDE_ANIMATION_MS));
        let _ = proxy.send_event(UserEvent::FinalizeHide(token));
    });

    push_state_to_ui(panel, runtime);
}

fn push_state_to_ui(panel: &Panel, runtime: &RuntimeState) {
    let active_count = runtime.todos.iter().filter(|todo| !todo.completed).count();
    let visible_limit = runtime.config.max_visible_plates as usize;
    let visible_count = active_count.min(visible_limit);
    let hidden_count = active_count.saturating_sub(visible_limit);
    let completed_count = runtime.todos.len().saturating_sub(active_count);

    let payload = json!({
        "type": "state_sync",
        "payload": {
            "todos": runtime.todos,
            "config": runtime.config,
            "overlay_visible": runtime.overlay_visible,
            "view_mode": runtime.view_mode,
            "visible_count": visible_count,
            "hidden_count": hidden_count,
            "completed_count": completed_count,
        }
    });
    panel.send_json(&payload);
}

fn push_permissions_to_ui(panel: &Panel, permissions: &PermissionState) {
    let payload = json!({
        "type": "permission_state",
        "payload": permissions,
    });
    panel.send_json(&payload);
}

fn emit_error(panel: &Panel, message: &str) {
    let payload = json!({
        "type": "error",
        "payload": { "message": message },
    });
    panel.send_json(&payload);
}

fn schedule_save(runtime: &mut RuntimeState, proxy: &EventLoopProxy<UserEvent>) {
    runtime.save_token = runtime.save_token.wrapping_add(1);
    let token = runtime.save_token;
    let proxy = proxy.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(SAVE_DEBOUNCE_MS));
        let _ = proxy.send_event(UserEvent::FlushSave(token));
    });
}

fn start_permission_polling(proxy: EventLoopProxy<UserEvent>) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(5));
            if proxy.send_event(UserEvent::RefreshPermissions).is_err() {
                break;
            }
        }
    });
}

fn start_hotkey_if_possible(
    runtime: &mut RuntimeState,
    trigger_key: &Arc<AtomicU8>,
    proxy: EventLoopProxy<UserEvent>,
    logger: ErrorLogger,
) {
    if runtime.hotkey_started || !runtime.permissions.input_monitoring {
        return;
    }

    runtime.hotkey_started = true;

    let toggle_proxy = proxy.clone();
    let error_proxy = proxy.clone();

    hotkey::start_listener(
        Arc::clone(trigger_key),
        Duration::from_millis(runtime.config.double_tap_ms as u64),
        move || {
            let _ = toggle_proxy.send_event(UserEvent::HotkeyTrigger);
        },
        move |error| {
            logger.log_error(&error);
            let _ = error_proxy.send_event(UserEvent::HotkeyError(error));
        },
    );
}

fn parse_view_mode(raw: &str) -> ViewMode {
    match raw.to_ascii_lowercase().as_str() {
        "list" => ViewMode::List,
        _ => ViewMode::Explosion,
    }
}

fn mark_todo_completed(todos: &mut [TodoItem], id: &str) -> bool {
    if let Some(item) = todos.iter_mut().find(|todo| todo.id == id) {
        if item.completed {
            return false;
        }
        mark_completed(item);
        return true;
    }
    false
}

fn is_valid_todo_id(id: &str) -> bool {
    if id.len() > 64 {
        return false;
    }
    Uuid::parse_str(id).is_ok()
}

fn seeded_todos() -> Vec<TodoItem> {
    let templates = [
        "整理今天最重要的三件事",
        "给项目写一个最小可运行版本",
        "确认全局双击 J 是否稳定触发",
        "优化盘子爆炸展开的节奏",
        "补充 5 条交互边界测试",
        "检查 JSON 持久化是否可靠",
        "回顾今天完成的工作记录",
        "清理一批不再需要的任务",
        "给明天预留一个创意任务",
        "收尾并同步当前版本说明",
    ];

    let base = now_ms() - (templates.len() as i64 * 60_000);
    templates
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let mut todo = make_todo((*text).to_string());
            let ts = base + index as i64 * 60_000;
            todo.created_at_ms = ts;
            todo.updated_at_ms = ts;
            todo
        })
        .collect()
}

fn resolve_app_data_dir() -> Result<AppDirResolution> {
    let base = dirs::data_dir().context("unable to resolve data_dir")?;
    let current_dir = base.join(APP_NAME);
    let legacy_dir = base.join(LEGACY_APP_NAME);

    let mut warnings = Vec::new();
    let mut migrated = false;

    if !current_dir.exists() && legacy_dir.exists() {
        fs::create_dir_all(&current_dir)
            .with_context(|| format!("failed to create {}", current_dir.display()))?;

        for file_name in ["todos.json", "config.json", "error.log"] {
            let from = legacy_dir.join(file_name);
            let to = current_dir.join(file_name);
            if !from.exists() || to.exists() {
                continue;
            }

            if let Err(err) = migrate_file(&from, &to) {
                let message = format!(
                    "migration warning: failed to move {} -> {}: {err:#}",
                    from.display(),
                    to.display()
                );
                eprintln!("{message}");
                warnings.push(message);
            } else {
                migrated = true;
            }
        }
    }

    fs::create_dir_all(&current_dir)
        .with_context(|| format!("failed to create {}", current_dir.display()))?;

    Ok(AppDirResolution {
        current: current_dir,
        migrated,
        warnings,
    })
}

fn migrate_file(from: &Path, to: &Path) -> Result<()> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }

    let mut src = File::open(from).with_context(|| format!("failed to open {}", from.display()))?;
    let mut dst = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(to)
        .with_context(|| format!("failed to create {}", to.display()))?;

    io::copy(&mut src, &mut dst)
        .with_context(|| format!("failed to copy {} -> {}", from.display(), to.display()))?;
    dst.sync_all()
        .with_context(|| format!("failed to sync {}", to.display()))?;
    if let Ok(meta) = src.metadata() {
        let _ = fs::set_permissions(to, meta.permissions());
    }

    fs::remove_file(from).with_context(|| format!("failed to remove {}", from.display()))?;
    Ok(())
}
