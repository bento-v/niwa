//! Windows notifications: when a plant is fully grown, when a blocked app is
//! closed while the card is out of sight, when apps that are already open
//! are about to be closed, and around scheduled focus times.

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

fn card_visible(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|card| card.is_visible().ok())
        .unwrap_or(false)
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    if crate::headless() {
        return;
    }
    let _ = app.notification().builder().title(title).body(body).show();
}

/// A plant finished growing.
pub fn grown(app: &AppHandle, species_name: &str, minutes: u32) {
    notify(
        app,
        &format!("Your {} is fully grown", species_name.to_lowercase()),
        &format!("{minutes} minutes of focus. It's in your garden now."),
    );
}

/// A blocked app was closed. The card shows its own note when it's open, so
/// this only speaks up when the card is hidden.
pub fn slip(app: &AppHandle, label: &str, species_name: &str) {
    if card_visible(app) {
        return;
    }
    notify(
        app,
        &format!("{label} was closed"),
        &format!("Your {} is still growing.", species_name.to_lowercase()),
    );
}

/// Apps that were open when the session started will be closed soon.
pub fn closing_soon(app: &AppHandle, labels: &[String], secs: u32) {
    let (title, verb) = match labels.len() {
        1 => (format!("{} will close in {secs} seconds", labels[0]), "it"),
        _ => (format!("{} apps will close in {secs} seconds", labels.len()), "them"),
    };
    notify(app, &title, &format!("Save your work and quit {verb} before the session closes {verb}."));
}

/// A scheduled focus time starts in a minute.
pub fn focus_time_soon(app: &AppHandle, at: &str) {
    notify(app, &format!("Focus time at {at}"), "Your session starts in a minute.");
}

/// A scheduled focus time couldn't start, and why.
pub fn focus_time_failed(app: &AppHandle, at: &str, reason: &str) {
    notify(app, &format!("Your {at} focus time didn't start"), reason);
}
