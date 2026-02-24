use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use core_foundation::runloop::CFRunLoop;
#[cfg(target_os = "macos")]
use core_graphics::event::{
    CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CallbackResult, EventField, KeyCode,
};

pub fn trigger_key_atomic(initial: char) -> Arc<AtomicU8> {
    Arc::new(AtomicU8::new(normalize_key(initial) as u8))
}

pub fn set_trigger_key(trigger: &Arc<AtomicU8>, key: char) {
    trigger.store(normalize_key(key) as u8, Ordering::Relaxed);
}

pub fn start_listener(
    trigger_key: Arc<AtomicU8>,
    double_tap_window: Duration,
    on_toggle: impl Fn() + Send + Sync + 'static,
    on_error: impl Fn(String) + Send + Sync + 'static,
) {
    let on_toggle: Arc<dyn Fn() + Send + Sync> = Arc::new(on_toggle);
    let on_error: Arc<dyn Fn(String) + Send + Sync> = Arc::new(on_error);

    #[cfg(target_os = "macos")]
    {
        thread::spawn(move || {
            let state = Arc::new(Mutex::new(TapState {
                last_tap: None,
                awaiting_release: false,
            }));

            let session = run_tap_loop(
                CGEventTapLocation::Session,
                Arc::clone(&state),
                Arc::clone(&trigger_key),
                Arc::clone(&on_toggle),
                double_tap_window,
            );

            if session.is_err() {
                let hid = run_tap_loop(
                    CGEventTapLocation::HID,
                    Arc::clone(&state),
                    Arc::clone(&trigger_key),
                    Arc::clone(&on_toggle),
                    double_tap_window,
                );
                if hid.is_err() {
                    on_error("hotkey listener failed: unable to create CGEventTap".to_string());
                }
            }
        });
        return;
    }

    #[cfg(not(target_os = "macos"))]
    thread::spawn(move || {
        let _ = trigger_key;
        let _ = double_tap_window;
        let _ = on_toggle;
        on_error("hotkey listener is only implemented for macOS".to_string());
    });
}

#[cfg(target_os = "macos")]
fn run_tap_loop(
    location: CGEventTapLocation,
    callback_state: Arc<Mutex<TapState>>,
    callback_trigger: Arc<AtomicU8>,
    callback_toggle: Arc<dyn Fn() + Send + Sync>,
    double_tap_window: Duration,
) -> Result<(), ()> {
    CGEventTap::with_enabled(
        location,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![CGEventType::KeyDown, CGEventType::KeyUp],
        move |_proxy, event_type, event| {
            match event_type {
                CGEventType::KeyDown => {
                    let keycode =
                        event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                    if keycode != read_target_keycode(&callback_trigger) {
                        return CallbackResult::Keep;
                    }
                    if !is_modifier_free(event.get_flags()) {
                        return CallbackResult::Keep;
                    }
                    if event.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) != 0 {
                        return CallbackResult::Keep;
                    }

                    let mut lock = callback_state
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    if lock.awaiting_release {
                        return CallbackResult::Keep;
                    }
                    lock.awaiting_release = true;

                    let now = Instant::now();
                    if let Some(previous) = lock.last_tap {
                        if now.duration_since(previous) <= double_tap_window {
                            lock.last_tap = None;
                            drop(lock);
                            callback_toggle();
                            return CallbackResult::Keep;
                        }
                    }
                    lock.last_tap = Some(now);
                }
                CGEventType::KeyUp => {
                    let keycode =
                        event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                    if keycode == read_target_keycode(&callback_trigger) {
                        let mut lock = callback_state
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        lock.awaiting_release = false;
                    }
                }
                _ => {}
            }

            CallbackResult::Keep
        },
        CFRunLoop::run_current,
    )
}

struct TapState {
    last_tap: Option<Instant>,
    awaiting_release: bool,
}

fn normalize_key(key: char) -> char {
    let upper = key.to_ascii_uppercase();
    if upper.is_ascii_uppercase() {
        upper
    } else {
        'J'
    }
}

#[cfg(target_os = "macos")]
fn is_modifier_free(flags: CGEventFlags) -> bool {
    let disallowed = CGEventFlags::CGEventFlagCommand
        | CGEventFlags::CGEventFlagAlternate
        | CGEventFlags::CGEventFlagControl
        | CGEventFlags::CGEventFlagShift
        | CGEventFlags::CGEventFlagSecondaryFn
        | CGEventFlags::CGEventFlagAlphaShift;
    !flags.intersects(disallowed)
}

#[cfg(target_os = "macos")]
fn read_target_keycode(trigger_key: &Arc<AtomicU8>) -> u16 {
    map_char_to_keycode(trigger_key.load(Ordering::Relaxed) as char)
}

#[cfg(target_os = "macos")]
fn map_char_to_keycode(c: char) -> u16 {
    match c.to_ascii_uppercase() {
        'A' => KeyCode::ANSI_A,
        'B' => KeyCode::ANSI_B,
        'C' => KeyCode::ANSI_C,
        'D' => KeyCode::ANSI_D,
        'E' => KeyCode::ANSI_E,
        'F' => KeyCode::ANSI_F,
        'G' => KeyCode::ANSI_G,
        'H' => KeyCode::ANSI_H,
        'I' => KeyCode::ANSI_I,
        'J' => KeyCode::ANSI_J,
        'K' => KeyCode::ANSI_K,
        'L' => KeyCode::ANSI_L,
        'M' => KeyCode::ANSI_M,
        'N' => KeyCode::ANSI_N,
        'O' => KeyCode::ANSI_O,
        'P' => KeyCode::ANSI_P,
        'Q' => KeyCode::ANSI_Q,
        'R' => KeyCode::ANSI_R,
        'S' => KeyCode::ANSI_S,
        'T' => KeyCode::ANSI_T,
        'U' => KeyCode::ANSI_U,
        'V' => KeyCode::ANSI_V,
        'W' => KeyCode::ANSI_W,
        'X' => KeyCode::ANSI_X,
        'Y' => KeyCode::ANSI_Y,
        'Z' => KeyCode::ANSI_Z,
        _ => KeyCode::ANSI_J,
    }
}
