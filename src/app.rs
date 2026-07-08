//! Application wiring: the event loop that ties together the tray menu, system
//! media controls, media keys and the audio [`Player`].

use anyhow::{Context, Result};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use tray_icon::menu::{MenuEvent, MenuId};
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

use crate::config::{Config, APP_DISPLAY_NAME, REPO_URL, STATION_NAME, STATION_URL};
use crate::player::{PlaybackStatus, Player, PlayerEvent};
use crate::{autostart, controls, icons, tray, util};

/// Events funnelled into the main event loop from various sources.
enum UserEvent {
    Menu(MenuId),
    Tray(TrayIconEvent),
    Media(MediaControlEvent),
    Player(PlayerEvent),
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

    let mut app = App {
        config,
        window,
        player,
        proxy,
        tray: None,
        controls: None,
        status: PlaybackStatus::Paused,
        title: None,
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
}

impl App {
    /// Build the tray and media controls once the platform GUI context is live.
    fn on_init(&mut self) {
        match tray::build(autostart::is_enabled()) {
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
        if self.config.autoplay {
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
        }
    }

    fn on_menu(&mut self, id: &MenuId, control_flow: &mut ControlFlow) {
        if id == tray::ID_PLAY_PAUSE {
            self.player.toggle();
        } else if id == tray::ID_OPEN_SITE {
            util::open_url(STATION_URL);
        } else if id == tray::ID_ABOUT {
            util::open_url(REPO_URL);
        } else if id == tray::ID_AUTOSTART {
            self.toggle_autostart();
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
        self.title = title;

        if let Some(tray) = &self.tray {
            let text = self
                .title
                .clone()
                .unwrap_or_else(|| "Not playing".to_string());
            tray.now_playing.set_text(text);
            let _ = tray.icon.set_tooltip(Some(self.tooltip()));
        }

        if let Some(controls) = &mut self.controls {
            let (artist, title) = match &self.title {
                Some(s) => {
                    let (a, t) = util::split_artist_title(s);
                    (a.map(str::to_string), t.to_string())
                }
                None => (None, STATION_NAME.to_string()),
            };
            let _ = controls.set_metadata(MediaMetadata {
                title: Some(&title),
                artist: artist.as_deref(),
                album: Some(STATION_NAME),
                cover_url: None,
                duration: None,
            });
        }
    }

    fn tooltip(&self) -> String {
        match self.status {
            PlaybackStatus::Playing => match &self.title {
                Some(t) => format!("{STATION_NAME} — {t}"),
                None => format!("{STATION_NAME} — playing"),
            },
            PlaybackStatus::Buffering => format!("{STATION_NAME} — connecting…"),
            PlaybackStatus::Paused => format!("{STATION_NAME} — paused"),
            PlaybackStatus::Error => format!("{STATION_NAME} — reconnecting…"),
        }
    }
}
