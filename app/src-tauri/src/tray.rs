//! The system tray icon: the 庭 stamp, with how long is left and the plant's
//! stage on hover.

use tauri::image::Image;
use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Wry};

use crate::views::SessionView;
use crate::{lock, AppState};

const TRAY_ID: &str = "main";
const ICON: &[u8] = include_bytes!("../icons/tray/stamp.png");

/// The menu items that change while the app runs.
pub struct TrayItems {
    mode: MenuItem<Wry>,
    quit: MenuItem<Wry>,
    /// The stage last shown, so the menu is only updated when it changes.
    stage: u8,
}

pub fn build(app: &App) -> tauri::Result<TrayItems> {
    let open = MenuItemBuilder::with_id("open", "Open Niwa").build(app)?;
    let mode = MenuItemBuilder::with_id("mode", "No session running")
        .enabled(false)
        .build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit Niwa").build(app)?;
    let menu = MenuBuilder::new(app)
        .items(&[&open, &mode])
        .separator()
        .item(&quit)
        .build()?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(ICON)?)
        .tooltip("Niwa")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => crate::show_window(app),
            "quit" => crate::quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::show_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(TrayItems {
        mode,
        quit,
        stage: 0,
    })
}

/// Updates the tray for a running session.
///
/// Tray changes are applied on the main thread, and the tray items are only
/// ever locked there. The session thread must not hold that lock while it
/// waits on the main thread, or the two could wait on each other.
pub fn show_session(app: &AppHandle, view: &SessionView) {
    let (handle, view) = (app.clone(), view.clone());
    let _ = app.run_on_main_thread(move || apply_session(&handle, &view));
}

pub fn show_idle(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || apply_idle(&handle));
}

fn apply_session(app: &AppHandle, view: &SessionView) {
    let state = app.state::<AppState>();
    let mut items = lock(&state.tray);
    let (Some(items), Some(tray)) = (items.as_mut(), app.tray_by_id(TRAY_ID)) else {
        return;
    };

    let _ = tray.set_tooltip(Some(format!(
        "{} left · {}, stage {} of 5",
        clock(view.remaining_secs),
        view.species_name,
        view.stage
    )));
    if items.stage != view.stage {
        let mode = match view.mode {
            "strict" => "Strict mode",
            _ => "Gentle mode",
        };
        let _ = items.mode.set_text(format!("{mode} · {}", view.list_name));
        // Quitting mid-session would be giving up; that's done from the card.
        let _ = items.quit.set_enabled(false);
        items.stage = view.stage;
    }
}

fn apply_idle(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut items = lock(&state.tray);
    let (Some(items), Some(tray)) = (items.as_mut(), app.tray_by_id(TRAY_ID)) else {
        return;
    };
    let _ = tray.set_tooltip(Some("Niwa"));
    let _ = items.mode.set_text("No session running");
    let _ = items.quit.set_enabled(true);
    items.stage = 0;
}

/// "18:20", or "1:05:00" past an hour.
fn clock(secs: u32) -> String {
    match secs / 3600 {
        0 => format!("{}:{:02}", secs / 60, secs % 60),
        hours => format!("{hours}:{:02}:{:02}", secs % 3600 / 60, secs % 60),
    }
}

#[cfg(test)]
mod tests {
    use super::clock;

    #[test]
    fn formats_time_left() {
        assert_eq!(clock(18 * 60 + 20), "18:20");
        assert_eq!(clock(59), "0:59");
        assert_eq!(clock(3600), "1:00:00");
        assert_eq!(clock(5 * 3600 - 1), "4:59:59");
    }
}
