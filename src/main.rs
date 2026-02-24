mod app;
mod config;
mod hotkey;
mod logging;
mod panel;
mod permissions;
mod storage;
mod tray;

fn main() {
    if let Err(err) = app::run() {
        eprintln!("AoiPlate failed: {err:#}");
    }
}
