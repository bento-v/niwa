//! The mini timer: a small always-on-top window in the corner of the screen
//! that the card collapses into when it is minimised.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::{App, AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

use crate::{lock, AppState};

pub const LABEL: &str = "mini";
/// Where the user last dragged the timer to, as "x,y" in screen pixels.
const POSITION_SETTING: &str = "mini_position";

/// Set once the user starts dragging the timer, so only their moves are
/// remembered, not the app placing it in the corner.
static DRAGGED: AtomicBool = AtomicBool::new(false);
/// Where a drag left the timer, waiting to be saved.
static UNSAVED: Mutex<Option<PhysicalPosition<i32>>> = Mutex::new(None);
/// The window is a little larger than the timer so its shadow has room.
const WIDTH: f64 = 268.0;
const HEIGHT: f64 = 84.0;
/// How far the window sits from the edges of the screen's free area. The
/// window's own transparent margin adds to this.
const MARGIN: f64 = 6.0;

/// Creates the mini window, hidden. It loads the same page as the card,
/// which draws the timer when it finds itself in this window.
pub fn create(app: &App) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Niwa timer")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        // Appearing must never take the keyboard from whatever is in use.
        .focused(false)
        .visible(false)
        .additional_browser_args(crate::BROWSER_ARGS)
        .build()?;
    Ok(())
}

/// Shows the mini timer where the user last left it, or in the bottom-right
/// corner of the screen the card was on, above the taskbar.
pub fn show(app: &AppHandle) {
    let Some(mini) = app.get_webview_window(LABEL) else {
        return;
    };
    if let Some(position) = remembered(app).filter(|&position| on_screen(&mini, position)) {
        let _ = mini.set_position(position);
        crate::set_shown(&mini, true);
        return;
    }
    let monitor = app
        .get_webview_window("main")
        .and_then(|card| card.current_monitor().ok().flatten())
        .or_else(|| mini.primary_monitor().ok().flatten());
    if let (Some(monitor), Ok(size)) = (monitor, mini.outer_size()) {
        let area = monitor.work_area();
        let margin = (MARGIN * monitor.scale_factor()).round() as i32;
        let x = area.position.x + area.size.width as i32 - size.width as i32 - margin;
        let y = area.position.y + area.size.height as i32 - size.height as i32 - margin;
        let _ = mini.set_position(PhysicalPosition::new(x, y));
    }
    crate::set_shown(&mini, true);
}

/// The user has started dragging the timer.
pub fn drag_started() {
    DRAGGED.store(true, Ordering::SeqCst);
}

/// The timer's window moved. Windows reports every step of a drag; the last
/// one is kept and written to the database later, not on every step.
pub fn moved(position: PhysicalPosition<i32>) {
    if DRAGGED.load(Ordering::SeqCst) {
        *lock(&UNSAVED) = Some(position);
    }
}

/// Writes where the user dragged the timer to, if they did. Called when the
/// timer hides and when the app quits.
pub fn save_position(app: &AppHandle) {
    let Some(position) = lock(&UNSAVED).take() else {
        return;
    };
    if let Some(state) = app.try_state::<AppState>() {
        let _ = lock(&state.store).set_setting(POSITION_SETTING, &format!("{},{}", position.x, position.y));
    }
}

fn remembered(app: &AppHandle) -> Option<PhysicalPosition<i32>> {
    let state = app.try_state::<AppState>()?;
    let saved = lock(&state.store).setting(POSITION_SETTING).ok()??;
    let (x, y) = saved.split_once(',')?;
    Some(PhysicalPosition::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// Whether the timer would be on a screen that is connected now. A position
/// saved on a monitor that has since been unplugged is ignored.
fn on_screen(mini: &tauri::WebviewWindow, position: PhysicalPosition<i32>) -> bool {
    let Ok(size) = mini.outer_size() else {
        return false;
    };
    let centre = (
        position.x + size.width as i32 / 2,
        position.y + size.height as i32 / 2,
    );
    mini.available_monitors().unwrap_or_default().iter().any(|monitor| {
        let area = monitor.work_area();
        centre.0 >= area.position.x
            && centre.0 < area.position.x + area.size.width as i32
            && centre.1 >= area.position.y
            && centre.1 < area.position.y + area.size.height as i32
    })
}

pub fn hide(app: &AppHandle) {
    save_position(app);
    if let Some(mini) = app.get_webview_window(LABEL) {
        crate::set_shown(&mini, false);
    }
}

