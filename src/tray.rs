use anyhow::{Context, Result};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const MENU_ID_TOGGLE: &str = "toggle_panel";
const MENU_ID_LAUNCH_AT_LOGIN: &str = "toggle_launch_at_login";
const MENU_ID_PERMISSIONS: &str = "open_permissions";
const MENU_ID_QUIT: &str = "quit_app";

#[derive(Debug, Clone, Copy)]
pub enum TrayCommand {
    TogglePanel,
    ToggleLaunchAtLogin,
    OpenPermissions,
    Quit,
}

pub struct TrayController {
    _tray_icon: TrayIcon,
    launch_at_login_item: CheckMenuItem,
}

impl TrayController {
    pub fn new(launch_at_login: bool) -> Result<Self> {
        let menu = Menu::new();

        let toggle_item = MenuItem::with_id(MENU_ID_TOGGLE, "显示 / 隐藏 AoiPlate", true, None);
        let launch_at_login_item = CheckMenuItem::with_id(
            MENU_ID_LAUNCH_AT_LOGIN,
            "开机启动",
            true,
            launch_at_login,
            None,
        );
        let permissions_item = MenuItem::with_id(MENU_ID_PERMISSIONS, "打开权限设置", true, None);
        let quit_item = MenuItem::with_id(MENU_ID_QUIT, "退出", true, None);

        menu.append_items(&[
            &toggle_item,
            &launch_at_login_item,
            &permissions_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ])
        .context("failed to append tray menu items")?;

        let icon = generate_icon()?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("AoiPlate")
            .with_icon(icon)
            .with_icon_as_template(true)
            .build()
            .context("failed to build tray icon")?;

        Ok(Self {
            _tray_icon: tray_icon,
            launch_at_login_item,
        })
    }

    pub fn set_launch_at_login_checked(&self, checked: bool) {
        self.launch_at_login_item.set_checked(checked);
    }
}

pub fn install_event_handlers(dispatch: impl Fn(TrayCommand) + Send + Sync + 'static) {
    let dispatch = std::sync::Arc::new(dispatch);

    let click_dispatch = std::sync::Arc::clone(&dispatch);
    TrayIconEvent::set_event_handler(Some(move |event| {
        if let TrayIconEvent::Click {
            button,
            button_state,
            ..
        } = event
        {
            if button == MouseButton::Left && button_state == MouseButtonState::Up {
                click_dispatch(TrayCommand::TogglePanel);
            }
        }
    }));

    let menu_dispatch = std::sync::Arc::clone(&dispatch);
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let id = event.id.0;
        match id.as_str() {
            MENU_ID_TOGGLE => menu_dispatch(TrayCommand::TogglePanel),
            MENU_ID_LAUNCH_AT_LOGIN => menu_dispatch(TrayCommand::ToggleLaunchAtLogin),
            MENU_ID_PERMISSIONS => menu_dispatch(TrayCommand::OpenPermissions),
            MENU_ID_QUIT => menu_dispatch(TrayCommand::Quit),
            _ => {}
        }
    }));
}

fn generate_icon() -> Result<Icon> {
    const SIZE: u32 = 18;
    let mut rgba = vec![0_u8; (SIZE * SIZE * 4) as usize];
    let center = (SIZE as f32 - 1.0) / 2.0;
    let radius = 7.2;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let idx = ((y * SIZE + x) * 4) as usize;
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            let plate_body = soft_circle(px, py, center, center, radius, 1.0);
            let top_notch = soft_circle(px, py, center, center - radius + 1.1, 1.45, 0.8);
            let bottom_notch = soft_circle(px, py, center, center + radius - 1.0, 1.8, 0.9);

            let alpha_f = (plate_body * (1.0 - top_notch.max(bottom_notch)))
                .clamp(0.0, 1.0)
                .powf(0.92);
            let alpha = (alpha_f * 255.0).round() as u8;
            rgba[idx] = 255;
            rgba[idx + 1] = 255;
            rgba[idx + 2] = 255;
            rgba[idx + 3] = alpha;
        }
    }

    Icon::from_rgba(rgba, SIZE, SIZE).context("failed to create tray icon pixels")
}

fn soft_circle(px: f32, py: f32, cx: f32, cy: f32, radius: f32, feather: f32) -> f32 {
    let distance = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
    ((radius + feather - distance) / (2.0 * feather)).clamp(0.0, 1.0)
}
