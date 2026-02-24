use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use base64::Engine as _;
use serde_json::Value;
use tao::dpi::{PhysicalPosition, PhysicalSize};
use tao::event_loop::EventLoop;
use tao::window::{Window, WindowBuilder};
use wry::http::Request;
use wry::{WebView, WebViewBuilder};

const INIT_SCRIPT_BASE: &str = r#"
window.__TODOLITE_QUEUE = [];
window.__TODOLITE_HANDLE_RUST = null;
window.__TODOLITE_FROM_RUST = function(message) {
  if (window.__TODOLITE_HANDLE_RUST) {
    window.__TODOLITE_HANDLE_RUST(message);
  } else {
    window.__TODOLITE_QUEUE.push(message);
  }
};
window.__TODOLITE_SET_OVERLAY_VISIBLE = function(visible) {
  document.documentElement.classList.toggle('overlay-visible', !!visible);
};
"#;

pub struct Panel {
    pub window: Window,
    webview: WebView,
}

impl Panel {
    pub fn new<T: 'static>(
        event_loop: &EventLoop<T>,
        ui_root: &Path,
        on_ipc: impl Fn(String) + Send + Sync + 'static,
    ) -> Result<Self> {
        let (position, size) = fullscreen_geometry(event_loop);

        let window = WindowBuilder::new()
            .with_title("AoiPlate")
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_always_on_top(true)
            .with_visible(false)
            .with_inner_size(size)
            .with_position(position)
            .build(event_loop)
            .context("failed to build overlay window")?;

        let html = load_html(ui_root)?;
        let plate_image = load_plate_image_data_uri(ui_root.parent().unwrap_or(ui_root));
        let init_script = build_init_script(plate_image.as_deref());
        let on_ipc = Arc::new(on_ipc);

        let webview = WebViewBuilder::new()
            .with_transparent(true)
            .with_initialization_script(&init_script)
            .with_ipc_handler(move |request: Request<String>| {
                on_ipc(request.body().to_string());
            })
            .with_html(&html)
            .build(&window)
            .context("failed to build webview")?;

        Ok(Self { window, webview })
    }

    pub fn show(&self) {
        self.window.set_visible(true);
        self.window.set_focus();
    }

    pub fn hide_native(&self) {
        self.window.set_visible(false);
    }

    pub fn set_visible_animated(&self, visible: bool) {
        let script = format!("window.__TODOLITE_SET_OVERLAY_VISIBLE({visible});");
        let _ = self.webview.evaluate_script(&script);
    }

    pub fn send_json(&self, payload: &Value) {
        let script = format!("window.__TODOLITE_FROM_RUST({payload});");
        let _ = self.webview.evaluate_script(&script);
    }
}

fn load_html(ui_root: &Path) -> Result<String> {
    let index_path = ui_root.join("index.html");
    let css_path = ui_root.join("styles.css");
    let js_path = ui_root.join("app.js");

    let index = fs::read_to_string(&index_path)
        .with_context(|| format!("failed to read {}", index_path.display()))?;
    let css = fs::read_to_string(&css_path)
        .with_context(|| format!("failed to read {}", css_path.display()))?;
    let js = fs::read_to_string(&js_path)
        .with_context(|| format!("failed to read {}", js_path.display()))?;

    Ok(index
        .replace("{{INLINE_STYLES}}", &css)
        .replace("{{INLINE_SCRIPT}}", &js))
}

fn build_init_script(plate_image: Option<&str>) -> String {
    let image_value = match plate_image {
        Some(uri) => serde_json::to_string(uri).unwrap_or_else(|_| "null".to_string()),
        None => "null".to_string(),
    };
    format!("{INIT_SCRIPT_BASE}\nwindow.__TODOLITE_PLATE_IMAGE_DATA = {image_value};\n")
}

fn load_plate_image_data_uri(project_root: &Path) -> Option<String> {
    let prioritized = [
        "plate.png",
        "plate.webp",
        "plate.jpg",
        "plate.jpeg",
        "plate.heic",
    ];

    for file_name in prioritized {
        let path = project_root.join(file_name);
        if path.exists() {
            if let Some(uri) = image_file_to_data_uri(&path) {
                return Some(uri);
            }
        }
    }

    let mut candidates: Vec<PathBuf> = fs::read_dir(project_root)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| {
                    matches!(
                        ext.to_ascii_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "webp" | "heic"
                    )
                })
                .unwrap_or(false)
        })
        .collect();

    candidates.sort();
    for path in candidates {
        if let Some(uri) = image_file_to_data_uri(&path) {
            return Some(uri);
        }
    }
    None
}

fn image_file_to_data_uri(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mime = detect_mime(path, &bytes)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{encoded}"))
}

fn detect_mime(path: &Path, bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }

    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("png") => Some("image/png"),
        Some(ext) if ext.eq_ignore_ascii_case("jpg") => Some("image/jpeg"),
        Some(ext) if ext.eq_ignore_ascii_case("jpeg") => Some("image/jpeg"),
        Some(ext) if ext.eq_ignore_ascii_case("webp") => Some("image/webp"),
        Some(ext) if ext.eq_ignore_ascii_case("heic") => Some("image/heic"),
        _ => None,
    }
}

fn fullscreen_geometry<T>(event_loop: &EventLoop<T>) -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
    if let Some(monitor) = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())
    {
        return (monitor.position(), monitor.size());
    }

    (PhysicalPosition::new(0, 0), PhysicalSize::new(1728, 1117))
}
