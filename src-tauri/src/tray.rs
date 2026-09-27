//! The menu bar item: a dot in the loudest level's colour, a count while
//! something needs you, a click for the priority panel, and a small menu.

use crate::jev::Level;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

pub const TRAY_ID: &str = "watchglass";
pub const PANEL: &str = "panel";
pub const MAIN: &str = "main";
pub const SHORTCUT: &str = "ctrl+alt+w";
/// Rendered at 2x; tray-icon scales any image to 18 pt tall, so this is
/// crisp on Retina menu bars.
const SIZE: u32 = 36;
/// A 12 pt dot inside the 18 pt row.
const DOT_RADIUS: f32 = 12.0;
/// A tray click can blur the panel before the click itself arrives; the
/// blur hides it and the click must not show it again. Likewise a blur
/// right after showing is the window system settling, not the user leaving.
const GRACE: Duration = Duration::from_millis(300);

static LAST_HIDE: Mutex<Option<Instant>> = Mutex::new(None);
static LAST_SHOW: Mutex<Option<Instant>> = Mutex::new(None);

fn within_grace(slot: &Mutex<Option<Instant>>) -> bool {
    slot.lock().expect("grace").is_some_and(|t| t.elapsed() < GRACE)
}

fn color(level: Level) -> [u8; 3] {
    match level {
        Level::Attention => [255, 96, 96],
        Level::Failing => [255, 150, 70],
        Level::Warning => [242, 204, 96],
        Level::Working => [110, 226, 140],
        Level::Idle => [150, 156, 166],
    }
}

/// A filled, antialiased dot.
pub fn dot(level: Level) -> Image<'static> {
    let [r, g, b] = color(level);
    let n = SIZE as usize;
    let mut rgba = vec![0u8; n * n * 4];
    let c = (n as f32 - 1.0) / 2.0;
    let radius = DOT_RADIUS;
    for y in 0..n {
        for x in 0..n {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
            let a = (radius + 0.5 - d).clamp(0.0, 1.0);
            let i = (y * n + x) * 4;
            rgba[i] = r;
            rgba[i + 1] = g;
            rgba[i + 2] = b;
            rgba[i + 3] = (a * 255.0) as u8;
        }
    }
    Image::new_owned(rgba, SIZE, SIZE)
}

pub fn build(app: &AppHandle<Wry>, autostart_on: bool) -> tauri::Result<TrayIcon<Wry>> {
    let show = MenuItem::with_id(app, "show", "Show list", true, Some("Ctrl+Alt+W"))?;
    let open = MenuItem::with_id(app, "open", "Open the board", true, None::<&str>)?;
    let login = CheckMenuItem::with_id(app, "login", "Start at login", true, autostart_on, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit watchglass", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&show, &open, &PredefinedMenuItem::separator(app)?, &login, &PredefinedMenuItem::separator(app)?, &quit],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(dot(Level::Idle))
        .icon_as_template(false)
        .tooltip("watchglass")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => toggle_panel(app),
            "open" => open_main(app),
            "login" => toggle_autostart(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                toggle_panel(tray.app_handle());
            }
        })
        .build(app)
}

/// Icon colour and the count next to it. Safe from any thread: the work is
/// handed to the main thread and not waited for.
pub fn update(app: &AppHandle<Wry>, top: Level, needs_you: usize) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_icon(Some(dot(top)));
            // tray-icon leaves the old title in place for `None`; an empty string clears it.
            let _ = tray.set_title(Some(if needs_you > 0 { needs_you.to_string() } else { String::new() }));
            let _ = tray.set_tooltip(Some(format!("watchglass · {}", format!("{top:?}").to_lowercase())));
        }
    });
}

pub fn toggle_panel(app: &AppHandle<Wry>) {
    let Some(panel) = app.get_webview_window(PANEL) else {
        return;
    };
    if panel.is_visible().unwrap_or(false) {
        hide_panel(app);
    } else if !within_grace(&LAST_HIDE) {
        show_panel(app);
    }
}

pub fn show_panel(app: &AppHandle<Wry>) {
    use tauri_plugin_positioner::{Position, WindowExt};
    let Some(panel) = app.get_webview_window(PANEL) else {
        return;
    };
    // Under the tray icon once a tray event has said where it is; a keyboard
    // toggle before any click lands top right.
    if panel.move_window(Position::TrayBottomCenter).is_err() {
        let _ = panel.move_window(Position::TopRight);
    }
    *LAST_SHOW.lock().expect("grace") = Some(Instant::now());
    let _ = panel.show();
    let _ = panel.set_focus();
}

pub fn hide_panel(app: &AppHandle<Wry>) {
    if let Some(panel) = app.get_webview_window(PANEL) {
        if panel.is_visible().unwrap_or(false) {
            let _ = panel.hide();
            *LAST_HIDE.lock().expect("grace") = Some(Instant::now());
        }
    }
}

/// The panel lost focus: the user moved on, unless it only just appeared.
pub fn panel_blurred(app: &AppHandle<Wry>) {
    if !within_grace(&LAST_SHOW) {
        hide_panel(app);
    }
}

/// The full board, built on demand from its config entry and destroyed on
/// close so a hidden web view is not rendering eight terminals for nobody.
pub fn open_main(app: &AppHandle<Wry>) {
    hide_panel(app);
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(main) = app.get_webview_window(MAIN) {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
        return;
    }
    let Some(config) = app.config().app.windows.iter().find(|w| w.label == MAIN).cloned() else {
        return;
    };
    match tauri::WebviewWindowBuilder::from_config(app, &config).and_then(|b| b.build()) {
        Ok(window) => {
            let h = app.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::Destroyed = event {
                    main_closed(&h);
                }
            });
            let _ = window.show();
            let _ = window.set_focus();
        }
        Err(e) => crate::tmux::log(&format!("open main: {e}")),
    }
}

fn main_closed(app: &AppHandle<Wry>) {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

fn toggle_autostart(app: &AppHandle<Wry>) {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    let on = launcher.is_enabled().unwrap_or(false);
    let result = if on { launcher.disable() } else { launcher.enable() };
    if let Err(e) = result {
        crate::tmux::log(&format!("autostart: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_is_opaque_in_the_middle_and_clear_in_the_corner() {
        let img = dot(Level::Attention);
        let rgba = img.rgba();
        let n = SIZE as usize;
        let center = ((n / 2) * n + n / 2) * 4;
        assert_eq!(&rgba[center..center + 4], &[255, 96, 96, 255]);
        assert_eq!(rgba[3], 0, "corner is transparent");
        assert_eq!(img.width(), SIZE);
    }

    #[test]
    fn grace_window_expires() {
        let slot: Mutex<Option<Instant>> = Mutex::new(None);
        assert!(!within_grace(&slot));
        *slot.lock().unwrap() = Some(Instant::now());
        assert!(within_grace(&slot));
        *slot.lock().unwrap() = Some(Instant::now() - GRACE * 2);
        assert!(!within_grace(&slot));
    }
}
