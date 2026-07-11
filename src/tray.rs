//! The system-tray icon and its context menu.

use anyhow::{Context, Result};
use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::config::APP_DISPLAY_NAME;
use crate::icons;

// Stable menu-item ids, matched against incoming `MenuEvent`s.
pub const ID_PLAY_PAUSE: &str = "play_pause";
pub const ID_NOW_PLAYING: &str = "now_playing";
pub const ID_SET_URL: &str = "set_stream_url";
pub const ID_AUTOSTART: &str = "autostart";
pub const ID_AUTOPLAY: &str = "autoplay";
pub const ID_NOTIFICATIONS: &str = "notifications";
pub const ID_ABOUT: &str = "about";
pub const ID_QUIT: &str = "quit";

/// Owns the tray icon and the menu items we mutate at runtime.
pub struct Tray {
    pub icon: TrayIcon,
    pub play_pause: MenuItem,
    pub now_playing: MenuItem,
    pub autostart: CheckMenuItem,
    pub autoplay: CheckMenuItem,
    pub notifications: CheckMenuItem,
}

/// Build the tray icon and menu. Must be called after the event loop has
/// started (required by tray-icon on macOS and Linux/GTK).
pub fn build(
    autostart_enabled: bool,
    autoplay_enabled: bool,
    notifications_enabled: bool,
    has_stream_url: bool,
) -> Result<Tray> {
    let menu = Menu::new();

    let now_playing = MenuItem::with_id(
        ID_NOW_PLAYING,
        if has_stream_url {
            "Not playing"
        } else {
            "No stream URL configured"
        },
        false,
        None,
    );
    let play_pause = MenuItem::with_id(ID_PLAY_PAUSE, "Play", has_stream_url, None);
    let set_url = MenuItem::with_id(ID_SET_URL, "Set stream URL…", true, None);
    let autostart = CheckMenuItem::with_id(
        ID_AUTOSTART,
        "Start on login",
        true,
        autostart_enabled,
        None,
    );
    let autoplay = CheckMenuItem::with_id(
        ID_AUTOPLAY,
        "Autoplay on startup",
        true,
        autoplay_enabled,
        None,
    );
    let notifications = CheckMenuItem::with_id(
        ID_NOTIFICATIONS,
        "Notify on song change",
        true,
        notifications_enabled,
        None,
    );
    let about = MenuItem::with_id(ID_ABOUT, format!("About {APP_DISPLAY_NAME}"), true, None);
    let quit = MenuItem::with_id(ID_QUIT, "Quit", true, None);

    menu.append(&now_playing).context("append now_playing")?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&play_pause).context("append play_pause")?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&set_url).context("append set_url")?;
    menu.append(&autostart).context("append autostart")?;
    menu.append(&autoplay).context("append autoplay")?;
    menu.append(&notifications)
        .context("append notifications")?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&about).context("append about")?;
    menu.append(&quit).context("append quit")?;

    let icon = TrayIconBuilder::new()
        .with_tooltip(format!("{APP_DISPLAY_NAME} — starting…"))
        .with_icon(icons::tray_icon(false))
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .context("building tray icon")?;

    Ok(Tray {
        icon,
        play_pause,
        now_playing,
        autostart,
        autoplay,
        notifications,
    })
}
