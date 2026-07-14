//! whirr — a tiny system-tray player for an internet radio (MP3/AAC) stream.
//!
//! The stream to play is set via `stream_url` in the config file; there is
//! no built-in default.

// On Windows, don't spawn a console window for this GUI/tray app.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod autostart;
mod config;
mod controls;
mod dialog;
mod icons;
mod icy;
mod notifications;
mod player;
mod playlist;
mod tray;
mod util;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Pick up a config left at the pre-0.4 location before loading.
    config::Config::migrate_legacy_path();
    let config = config::Config::load();

    // Hidden diagnostic: connect, decode silently for a few seconds, and report.
    if std::env::args().any(|a| a == "--selftest") {
        std::process::exit(selftest(config));
    }

    // Make sure a config file (with the stream-URL hint on a fresh install)
    // exists for users who prefer editing settings by hand.
    config.write_if_missing();

    if let Err(err) = app::run(config) {
        log::error!("fatal: {err:#}");
        std::process::exit(1);
    }
}

/// Headless smoke test of the audio pipeline. Returns a process exit code.
fn selftest(mut config: config::Config) -> i32 {
    use player::{PlaybackStatus, Player, PlayerEvent};
    use std::time::{Duration, Instant};

    if config.stream_url.is_none() {
        eprintln!("SELFTEST FAILED: no stream URL configured (set stream_url in the config file)");
        return 1;
    }

    config.volume = 0.0; // stay silent during the test
    let (tx, rx) = std::sync::mpsc::channel();
    let player = Player::new(config, move |ev| {
        let _ = tx.send(ev);
    });
    player.play();

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut reached_playing = false;
    let mut title = None;
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(PlayerEvent::Status(s)) => {
                println!("status: {s:?}");
                if s == PlaybackStatus::Playing {
                    reached_playing = true;
                }
            }
            Ok(PlayerEvent::Title(t)) => {
                println!("title:  {t:?}");
                title = t;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }
    player.quit();

    if reached_playing {
        println!("SELFTEST OK (now playing: {title:?})");
        0
    } else {
        eprintln!("SELFTEST FAILED: never reached Playing state");
        1
    }
}
