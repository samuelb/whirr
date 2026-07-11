//! Application wiring: the event loop that ties together the tray menu, system
//! media controls, media keys and the audio [`Player`].

use anyhow::{Context, Result};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use tray_icon::menu::{MenuEvent, MenuId};
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

use crate::config::{Config, APP_DISPLAY_NAME, REPO_URL};
use crate::player::{PlaybackStatus, Player, PlayerEvent};
use crate::{autostart, config, controls, dialog, icons, notifications, tray, util};

/// Events funnelled into the main event loop from various sources.
enum UserEvent {
    Menu(MenuId),
    Tray(TrayIconEvent),
    Media(MediaControlEvent),
    Player(PlayerEvent),
    /// Result of the stream-URL dialog: the entered text, or `None` on cancel.
    StreamUrlEntered(Option<String>),
    /// The config file on disk was modified (reported by the file watcher).
    ConfigFileChanged,
}

/// Build the event loop, wire everything up, and run until the user quits.
pub fn run(config: Config) -> Result<()> {
    let mut builder = EventLoopBuilder::<UserEvent>::with_user_event();
    #[allow(unused_mut)]
    let mut event_loop = builder.build();
    #[cfg(target_os = "macos")]
    {
        // Run as a background "accessory" app: tray only, no Dock icon.
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }
    let proxy = event_loop.create_proxy();

    // The global tray/menu event handlers must be `Send + Sync`; the proxy is
    // `Send` but not `Sync`, so guard it with a mutex.
    let shared = std::sync::Arc::new(std::sync::Mutex::new(proxy.clone()));
    let menu_proxy = shared.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Ok(p) = menu_proxy.lock() {
            let _ = p.send_event(UserEvent::Menu(event.id));
        }
    }));
    let tray_proxy = shared.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let Ok(p) = tray_proxy.lock() {
            let _ = p.send_event(UserEvent::Tray(event));
        }
    }));

    // A hidden window is required by the Windows SMTC backend and harmless
    // everywhere else.
    let window = WindowBuilder::new()
        .with_visible(false)
        .with_title(APP_DISPLAY_NAME)
        .build(&event_loop)
        .context("creating hidden window")?;

    // Forward player events into the loop.
    let player_proxy = proxy.clone();
    let player = Player::new(config.clone(), move |ev| {
        let _ = player_proxy.send_event(UserEvent::Player(ev));
    });

    // Apply edits to the config file without a restart.
    let watcher_proxy = proxy.clone();
    Config::spawn_watcher(move || {
        let _ = watcher_proxy.send_event(UserEvent::ConfigFileChanged);
    });

    let mut app = App {
        config,
        window,
        player,
        proxy,
        tray: None,
        controls: None,
        status: PlaybackStatus::Paused,
        title: None,
        last_song: None,
        url_dialog_open: false,
    };

    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => app.on_init(),
            Event::UserEvent(ue) => app.on_user_event(ue, control_flow),
            _ => {}
        }
    })
}

struct App {
    config: Config,
    #[allow(dead_code)] // kept alive for the SMTC window handle
    window: Window,
    player: Player,
    proxy: EventLoopProxy<UserEvent>,
    tray: Option<tray::Tray>,
    controls: Option<MediaControls>,
    status: PlaybackStatus,
    title: Option<String>,
    /// The last non-empty song title we announced (or saw while notifications
    /// were off). Used to fire a notification only on a genuine track change,
    /// not on reconnects or pause/resume of the same song.
    last_song: Option<String>,
    /// Whether the stream-URL dialog is currently showing (prevents stacking
    /// a second one from repeated menu clicks).
    url_dialog_open: bool,
}

impl App {
    /// Build the tray and media controls once the platform GUI context is live.
    fn on_init(&mut self) {
        match tray::build(
            autostart::is_enabled(),
            self.config.autoplay,
            self.config.notifications,
            self.config.stream_url.is_some(),
        ) {
            Ok(t) => self.tray = Some(t),
            Err(err) => log::error!("failed to create tray icon: {err:#}"),
        }

        let proxy = self.proxy.clone();
        match controls::build(&self.window, move |ev| {
            let _ = proxy.send_event(UserEvent::Media(ev));
        }) {
            Ok(c) => self.controls = Some(c),
            Err(err) => log::error!("failed to create media controls: {err:#}"),
        }

        self.apply_status(self.status);
        if self.config.autoplay && self.config.stream_url.is_some() {
            self.player.play();
        }
    }

    fn on_user_event(&mut self, event: UserEvent, control_flow: &mut ControlFlow) {
        match event {
            UserEvent::Menu(id) => self.on_menu(&id, control_flow),
            UserEvent::Tray(ev) => self.on_tray(ev),
            UserEvent::Media(ev) => self.on_media(ev),
            UserEvent::Player(PlayerEvent::Status(s)) => self.apply_status(s),
            UserEvent::Player(PlayerEvent::Title(t)) => self.apply_title(t),
            UserEvent::StreamUrlEntered(result) => self.on_stream_url_entered(result),
            UserEvent::ConfigFileChanged => self.on_config_file_changed(),
        }
    }

    fn on_menu(&mut self, id: &MenuId, control_flow: &mut ControlFlow) {
        if id == tray::ID_PLAY_PAUSE {
            self.player.toggle();
        } else if id == tray::ID_SET_URL {
            self.open_url_dialog(false, None);
        } else if id == tray::ID_ABOUT {
            util::open_url(REPO_URL);
        } else if id == tray::ID_AUTOSTART {
            self.toggle_autostart();
        } else if id == tray::ID_AUTOPLAY {
            self.toggle_autoplay();
        } else if id == tray::ID_NOTIFICATIONS {
            self.toggle_notifications();
        } else if id == tray::ID_QUIT {
            self.quit(control_flow);
        }
    }

    fn on_tray(&mut self, event: TrayIconEvent) {
        // Left-click toggles playback (where the platform reports clicks).
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            self.player.toggle();
        }
    }

    fn on_media(&mut self, event: MediaControlEvent) {
        match event {
            MediaControlEvent::Play => self.player.play(),
            MediaControlEvent::Pause | MediaControlEvent::Stop => self.player.pause(),
            MediaControlEvent::Toggle => self.player.toggle(),
            _ => {}
        }
    }

    /// Show the stream-URL dialog, pre-filled with `prefill` (falling back to
    /// the configured URL). The result arrives as [`UserEvent::StreamUrlEntered`].
    fn open_url_dialog(&mut self, invalid: bool, prefill: Option<String>) {
        if self.url_dialog_open {
            return;
        }
        self.url_dialog_open = true;
        let proxy = self.proxy.clone();
        dialog::prompt_stream_url(
            prefill.or_else(|| self.config.stream_url.clone()),
            invalid,
            move |result| {
                let _ = proxy.send_event(UserEvent::StreamUrlEntered(result));
            },
        );
    }

    fn on_stream_url_entered(&mut self, result: Option<String>) {
        self.url_dialog_open = false;
        let Some(input) = result else { return };
        let input = input.trim().to_string();
        if input.is_empty() {
            return;
        }
        if !config::is_valid_stream_url(&input) {
            // Re-open the dialog with the rejected input so it can be fixed.
            self.open_url_dialog(true, Some(input));
            return;
        }

        self.config.stream_url = Some(input);
        if let Err(err) = self.config.save() {
            log::warn!("could not save config: {err:#}");
        }
        self.player.set_stream_url(self.config.stream_url.clone());

        // Refresh the "No stream URL configured" states, then start playing
        // the newly configured stream right away.
        if let Some(tray) = &self.tray {
            tray.play_pause.set_enabled(true);
        }
        self.apply_status(self.status);
        let title = self.title.take();
        self.apply_title(title);
        self.player.play();
    }

    fn toggle_autostart(&mut self) {
        let enable = !autostart::is_enabled();
        match autostart::set(enable) {
            Ok(()) => {
                self.config.autostart = enable;
                if let Err(err) = self.config.save() {
                    log::warn!("could not save config: {err:#}");
                }
            }
            Err(err) => log::error!("could not change autostart: {err:#}"),
        }
        // Reflect the actual OS state, whatever happened.
        if let Some(tray) = &self.tray {
            tray.autostart.set_checked(autostart::is_enabled());
        }
    }

    /// Reload the config file and apply whatever changed. A missing or
    /// unparsable file (e.g. an edit still being written) is ignored; our own
    /// saves arrive here too and no-op because nothing differs.
    fn on_config_file_changed(&mut self) {
        let new = match Config::load_strict() {
            Ok(cfg) => cfg,
            Err(err) => {
                log::warn!("ignoring config file change: {err:#}");
                return;
            }
        };
        if new == self.config {
            return;
        }
        log::info!("config file changed; applying new settings");
        let old = std::mem::replace(&mut self.config, new);

        if self.config.stream_url != old.stream_url {
            // The engine restarts (or stops) playback as needed; playback is
            // not started here if it wasn't already running.
            self.player.set_stream_url(self.config.stream_url.clone());
            if let Some(tray) = &self.tray {
                tray.play_pause
                    .set_enabled(self.config.stream_url.is_some());
            }
            self.apply_status(self.status);
            let title = self.title.take();
            self.apply_title(title);
        }
        if self.config.volume != old.volume {
            self.player.set_volume(self.config.volume);
        }
        if self.config.autostart != old.autostart {
            if let Err(err) = autostart::set(self.config.autostart) {
                log::error!("could not change autostart: {err:#}");
            }
        }
        if let Some(tray) = &self.tray {
            tray.autostart.set_checked(autostart::is_enabled());
            tray.autoplay.set_checked(self.config.autoplay);
            tray.notifications.set_checked(self.config.notifications);
        }
    }

    /// Toggle autoplay-on-startup. Only affects future launches; the current
    /// playback state is left alone.
    fn toggle_autoplay(&mut self) {
        self.config.autoplay = !self.config.autoplay;
        if let Err(err) = self.config.save() {
            log::warn!("could not save config: {err:#}");
        }
        if let Some(tray) = &self.tray {
            tray.autoplay.set_checked(self.config.autoplay);
        }
    }

    fn toggle_notifications(&mut self) {
        self.config.notifications = !self.config.notifications;
        if let Err(err) = self.config.save() {
            log::warn!("could not save config: {err:#}");
        }
        if let Some(tray) = &self.tray {
            tray.notifications.set_checked(self.config.notifications);
        }
    }

    fn quit(&mut self, control_flow: &mut ControlFlow) {
        self.player.quit();
        self.controls = None;
        self.tray = None;
        *control_flow = ControlFlow::Exit;
    }

    fn apply_status(&mut self, status: PlaybackStatus) {
        self.status = status;
        let active = matches!(status, PlaybackStatus::Playing | PlaybackStatus::Buffering);

        if let Some(tray) = &self.tray {
            let _ = tray.icon.set_icon(Some(icons::tray_icon(active)));
            tray.play_pause
                .set_text(if active { "Pause" } else { "Play" });
            let _ = tray.icon.set_tooltip(Some(self.tooltip()));
        }

        if let Some(controls) = &mut self.controls {
            let playback = match status {
                PlaybackStatus::Playing | PlaybackStatus::Buffering => {
                    MediaPlayback::Playing { progress: None }
                }
                PlaybackStatus::Paused | PlaybackStatus::Error => {
                    MediaPlayback::Paused { progress: None }
                }
            };
            let _ = controls.set_playback(playback);
        }
    }

    fn apply_title(&mut self, title: Option<String>) {
        // Announce genuine song changes with a desktop notification (best-effort).
        // A change to an empty/None title (e.g. on pause) is not a new song and
        // must not reset `last_song`, so resuming the same track stays quiet.
        if let Some(song) = &title {
            if self.last_song.as_deref() != Some(song.as_str()) {
                if self.config.notifications {
                    notifications::song_changed(song);
                }
                self.last_song = Some(song.clone());
            }
        }

        self.title = title;

        if let Some(tray) = &self.tray {
            let text = match &self.title {
                Some(t) => t.clone(),
                None if self.config.stream_url.is_none() => "No stream URL configured".to_string(),
                None => "Not playing".to_string(),
            };
            tray.now_playing.set_text(text);
            let _ = tray.icon.set_tooltip(Some(self.tooltip()));
        }

        if let Some(controls) = &mut self.controls {
            let (artist, title) = match &self.title {
                Some(s) => {
                    let (a, t) = util::split_artist_title(s);
                    (a.map(str::to_string), t.to_string())
                }
                None => (None, APP_DISPLAY_NAME.to_string()),
            };
            let _ = controls.set_metadata(MediaMetadata {
                title: Some(&title),
                artist: artist.as_deref(),
                album: None,
                cover_url: None,
                duration: None,
            });
        }
    }

    fn tooltip(&self) -> String {
        if self.config.stream_url.is_none() {
            return "No stream URL configured".to_string();
        }
        match self.status {
            PlaybackStatus::Playing => match &self.title {
                Some(t) => t.clone(),
                None => "Playing".to_string(),
            },
            PlaybackStatus::Buffering => "Connecting…".to_string(),
            PlaybackStatus::Paused => "Paused".to_string(),
            PlaybackStatus::Error => "Reconnecting…".to_string(),
        }
    }
}
