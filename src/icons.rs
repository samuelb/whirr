//! Tray icon generation.
//!
//! The brand logo is embedded at compile time and decoded to RGBA for the tray.
//! A desaturated, dimmed variant is used to signal the paused/stopped state.

use tray_icon::Icon;

/// The embedded PNG logo (see `assets/icons/logo-64.png`).
static LOGO_PNG: &[u8] = include_bytes!("../assets/icons/logo-64.png");

/// Build a tray [`Icon`]. When `active` is false the icon is grayed out to
/// indicate that playback is paused/stopped.
pub fn tray_icon(active: bool) -> Icon {
    let image = image::load_from_memory(LOGO_PNG)
        .expect("embedded logo is a valid PNG")
        .into_rgba8();
    let (w, h) = image.dimensions();
    let mut rgba = image.into_raw();

    if !active {
        // Convert to a dimmed grayscale so paused state reads at a glance.
        for px in rgba.chunks_exact_mut(4) {
            let luma = (0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32) as u8;
            let dimmed = (luma as u16 * 5 / 8) as u8;
            px[0] = dimmed;
            px[1] = dimmed;
            px[2] = dimmed;
        }
    }

    Icon::from_rgba(rgba, w, h).expect("valid RGBA icon")
}
