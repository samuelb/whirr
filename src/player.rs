//! Audio engine: a pure-Rust pipeline that streams the configured MP3 feed,
//! strips ICY metadata, decodes with Symphonia and plays through Rodio/CPAL.
//!
//! Design
//! ------
//! * A single **engine thread** owns the audio output device and processes
//!   commands ([`Command`]) from the UI. The device is opened lazily on the
//!   first play and released (the [`OutputStream`] dropped) whenever playback
//!   stops, so an idle/paused Whirr holds no output stream open — otherwise the
//!   OS audio server keeps mixing silence into the (built-in-speaker) DSP for as
//!   long as the app runs, a continuous CPU cost even while nothing plays.
//! * "Playing" is represented by a monotonically increasing *generation* stored
//!   in a shared [`AtomicUsize`]. Starting playback bumps the generation and
//!   spawns a **worker** tagged with it; pausing/stopping simply bumps the
//!   generation again. A worker keeps running only while its generation is the
//!   active one, so pause is effectively instant and there is never more than
//!   one worker feeding the speakers.
//! * The worker owns the reconnect loop: on any disconnect or error it backs
//!   off and reconnects, so playback self-heals without UI involvement.
//! * Within a session, a small **network thread** reads the HTTP body + ICY
//!   metadata and forwards raw audio bytes over a bounded channel; the worker
//!   decodes from that channel. The channel provides natural back-pressure and,
//!   because a [`crossbeam_channel::Receiver`] is `Send + Sync`, lets us satisfy
//!   Symphonia's `MediaSource` bound without the (non-`Sync`) HTTP response.

use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::config::{Config, USER_AGENT};
use crate::icy::IcyReader;

/// Coarse playback state surfaced to the UI and media controls.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlaybackStatus {
    /// Not playing (user paused/stopped).
    Paused,
    /// Connecting or waiting to reconnect.
    Buffering,
    /// Audio is playing.
    Playing,
    /// A recoverable error occurred; the engine will retry.
    Error,
}

/// Events emitted by the engine for the UI to react to.
#[derive(Clone, Debug)]
pub enum PlayerEvent {
    Status(PlaybackStatus),
    /// Current track title (`None` when unknown/cleared).
    Title(Option<String>),
}

/// Commands accepted by the engine.
enum Command {
    Play,
    Pause,
    Toggle,
    /// Switch to a new stream URL (or none), restarting playback if active.
    SetStreamUrl(Option<String>),
    /// Change the output volume; applied to live playback without a restart.
    SetVolume(f32),
    WorkerFailed(usize),
    Quit,
}

/// Handle used by the UI to control playback. Cloning is cheap.
#[derive(Clone)]
pub struct Player {
    cmd_tx: Sender<Command>,
}

impl Player {
    /// Spawn the engine thread. `emit` is called (from engine/worker threads)
    /// whenever playback state or the current title changes.
    pub fn new<E>(config: Config, emit: E) -> Self
    where
        E: Fn(PlayerEvent) + Send + Clone + 'static,
    {
        let (cmd_tx, cmd_rx) = unbounded();
        let engine_cmd_tx = cmd_tx.clone();
        thread::Builder::new()
            .name("audio-engine".into())
            .spawn(move || engine(config, cmd_rx, engine_cmd_tx, emit))
            .expect("spawn audio engine");
        Self { cmd_tx }
    }

    pub fn play(&self) {
        let _ = self.cmd_tx.send(Command::Play);
    }
    pub fn pause(&self) {
        let _ = self.cmd_tx.send(Command::Pause);
    }
    pub fn toggle(&self) {
        let _ = self.cmd_tx.send(Command::Toggle);
    }
    pub fn set_stream_url(&self, url: Option<String>) {
        let _ = self.cmd_tx.send(Command::SetStreamUrl(url));
    }
    pub fn set_volume(&self, volume: f32) {
        let _ = self.cmd_tx.send(Command::SetVolume(volume));
    }
    pub fn quit(&self) {
        let _ = self.cmd_tx.send(Command::Quit);
    }
}

/// A predicate shared between worker/network threads: "is my generation still
/// the active one?". When it returns false the session must wind down.
type ShouldRun = Arc<dyn Fn() -> bool + Send + Sync>;

fn engine<E>(mut config: Config, cmd_rx: Receiver<Command>, cmd_tx: Sender<Command>, emit: E)
where
    E: Fn(PlayerEvent) + Send + Clone + 'static,
{
    let generation = Arc::new(AtomicUsize::new(0));
    // Volume as f32 bits, shared with workers so changes apply to live
    // playback without restarting the stream.
    let volume = Arc::new(AtomicU32::new(config.volume.clamp(0.0, 1.0).to_bits()));
    let mut playing = false;

    // The audio device, opened lazily on the first play and dropped whenever
    // playback stops (see the module docs). It is not `Send`, so it stays on
    // this engine thread; workers build their `Sink` from a cloned handle
    // (`OutputStreamHandle` *is* `Send`), and dropping the `OutputStream` while
    // a worker winds down is harmless (its `Sink` just feeds a dead mixer).
    let mut stream: Option<(OutputStream, OutputStreamHandle)> = None;

    // Begin a playback session: open the device if needed, then spawn a worker
    // tagged with a fresh generation. Returns whether playback started — `false`
    // (with an `Error` status emitted) means the device could not be opened.
    // Takes the config as a parameter (rather than capturing it) so the command
    // loop below can mutate it between starts.
    let start = |stream: &mut Option<(OutputStream, OutputStreamHandle)>,
                 config: &Config,
                 generation: &Arc<AtomicUsize>|
     -> bool {
        if stream.is_none() {
            match OutputStream::try_default() {
                Ok(pair) => *stream = Some(pair),
                Err(err) => {
                    log::error!("no audio output device: {err}");
                    emit(PlayerEvent::Status(PlaybackStatus::Error));
                    return false;
                }
            }
        }
        let handle = stream.as_ref().expect("stream opened above").1.clone();
        let my_gen = generation.fetch_add(1, Ordering::SeqCst) + 1;
        let gen = generation.clone();
        let should_run: ShouldRun = Arc::new(move || gen.load(Ordering::SeqCst) == my_gen);
        let cfg = config.clone();
        let volume = volume.clone();
        let emit = emit.clone();
        let cmd_tx = cmd_tx.clone();
        emit(PlayerEvent::Status(PlaybackStatus::Buffering));
        thread::Builder::new()
            .name("audio-worker".into())
            .spawn(move || {
                if !worker(cfg, handle, volume, should_run, emit) {
                    let _ = cmd_tx.send(Command::WorkerFailed(my_gen));
                }
            })
            .expect("spawn audio worker");
        true
    };

    let stop = |generation: &Arc<AtomicUsize>| {
        generation.fetch_add(1, Ordering::SeqCst);
    };

    for cmd in cmd_rx.iter() {
        match cmd {
            Command::Play if !playing => {
                if config.stream_url.is_some() {
                    playing = start(&mut stream, &config, &generation);
                } else {
                    log::warn!("no stream URL configured; cannot play");
                    emit(PlayerEvent::Status(PlaybackStatus::Paused));
                }
            }
            Command::Pause if playing => {
                playing = false;
                stop(&generation);
                stream = None; // release the audio device while idle
                emit(PlayerEvent::Status(PlaybackStatus::Paused));
                emit(PlayerEvent::Title(None));
            }
            Command::Toggle => {
                if playing {
                    playing = false;
                    stop(&generation);
                    stream = None; // release the audio device while idle
                    emit(PlayerEvent::Status(PlaybackStatus::Paused));
                    emit(PlayerEvent::Title(None));
                } else if config.stream_url.is_some() {
                    playing = start(&mut stream, &config, &generation);
                } else {
                    log::warn!("no stream URL configured; cannot play");
                    emit(PlayerEvent::Status(PlaybackStatus::Paused));
                }
            }
            Command::SetVolume(v) => {
                let v = v.clamp(0.0, 1.0);
                config.volume = v;
                volume.store(v.to_bits(), Ordering::Relaxed);
            }
            Command::SetStreamUrl(url) => {
                config.stream_url = url;
                if playing {
                    stop(&generation);
                    emit(PlayerEvent::Title(None));
                    if config.stream_url.is_some() {
                        // Reuse the already-open device for the new stream.
                        playing = start(&mut stream, &config, &generation);
                        if !playing {
                            stream = None;
                        }
                    } else {
                        playing = false;
                        stream = None; // release the audio device while idle
                        emit(PlayerEvent::Status(PlaybackStatus::Paused));
                    }
                }
            }
            Command::Quit => {
                stop(&generation);
                break;
            }
            Command::WorkerFailed(worker_gen) => {
                if playing && generation.load(Ordering::SeqCst) == worker_gen {
                    playing = false;
                    stream = None; // worker gave up; release the audio device
                    emit(PlayerEvent::Title(None));
                    // The worker gave up (no audio sink); without this the UI
                    // would keep showing the last Buffering/Error status as if
                    // a reconnect were still coming.
                    emit(PlayerEvent::Status(PlaybackStatus::Paused));
                }
            }
            Command::Play | Command::Pause => {} // already in the requested state
        }
    }
}

/// Per-session worker: owns a [`Sink`] and reconnects with exponential backoff
/// until its generation is retired.
fn worker<E>(
    config: Config,
    handle: rodio::OutputStreamHandle,
    volume: Arc<AtomicU32>,
    should_run: ShouldRun,
    emit: E,
) -> bool
where
    E: Fn(PlayerEvent) + Send + Clone + 'static,
{
    let sink = match Sink::try_new(&handle) {
        Ok(s) => s,
        Err(err) => {
            log::error!("cannot create audio sink: {err}");
            emit(PlayerEvent::Status(PlaybackStatus::Error));
            return false;
        }
    };
    sink.set_volume(f32::from_bits(volume.load(Ordering::Relaxed)));

    let mut backoff = Duration::from_secs(1);
    while should_run() {
        emit(PlayerEvent::Status(PlaybackStatus::Buffering));
        let mut played = false;
        match stream_session(&config, &sink, &volume, &should_run, &emit, &mut played) {
            Ok(()) => {
                if !should_run() {
                    break;
                }
                // Clean disconnect (server closed / track boundary): reconnect quickly.
                backoff = Duration::from_secs(1);
                sleep_interruptible(Duration::from_millis(500), &should_run);
            }
            Err(err) => {
                if !should_run() {
                    break;
                }
                if played {
                    // The session was healthy before this error; don't punish
                    // it with backoff left over from earlier failures.
                    backoff = Duration::from_secs(1);
                }
                log::warn!("stream error, reconnecting in {backoff:?}: {err:#}");
                emit(PlayerEvent::Status(PlaybackStatus::Error));
                sleep_interruptible(backoff, &should_run);
                backoff = (backoff * 2).min(Duration::from_secs(30));
            }
        }
    }
    // Dropping `sink` here stops any queued audio for this session.
    true
}

/// Connect once and decode until the stream ends, errors, or the session is
/// retired. Returns `Ok(())` on a clean end (so the caller reconnects promptly).
/// Sets `played` once audio actually reached the sink.
fn stream_session<E>(
    config: &Config,
    sink: &Sink,
    volume: &AtomicU32,
    should_run: &ShouldRun,
    emit: &E,
    played: &mut bool,
) -> Result<()>
where
    E: Fn(PlayerEvent) + Send + Clone + 'static,
{
    let url = config
        .stream_url
        .as_deref()
        .context("no stream URL configured")?;

    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        // The blocking client applies this per body-read (a stall guard for
        // the live stream), not as a whole-request deadline. Without it the
        // implicit default is 30s; a shorter guard recovers from dead
        // connections faster and also bounds how long the network thread can
        // linger after its session is retired.
        .timeout(Duration::from_secs(15))
        .build()
        .context("building HTTP client")?;

    let response = client
        .get(url)
        .header("Icy-MetaData", "1")
        .send()
        .context("connecting to stream")?
        .error_for_status()
        .context("stream returned an error status")?;

    let metaint = response
        .headers()
        .get("icy-metaint")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    // Network thread: HTTP body + ICY demux -> bounded channel of audio bytes.
    let (tx, rx) = bounded::<Vec<u8>>(64);
    let net_should = should_run.clone();
    let title_emit = emit.clone();
    let net = thread::Builder::new()
        .name("audio-net".into())
        .spawn(move || {
            let on_title = Box::new(move |t: Option<String>| title_emit(PlayerEvent::Title(t)));
            let mut reader = IcyReader::new(response, metaint, on_title);
            let mut buf = [0u8; 8192];
            while net_should() {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF / disconnect
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break; // decoder gone
                        }
                    }
                    Err(err) => {
                        log::debug!("network read ended: {err}");
                        break;
                    }
                }
            }
            // Dropping `tx` closes the channel, signalling EOF to the decoder.
        })
        .context("spawning network thread")?;

    let result = decode_loop(rx, sink, volume, should_run, emit, played);
    // Always reap the network thread. Dropping `rx` above makes its next
    // `send` fail, and the read timeout on the HTTP client bounds how long a
    // stalled read can keep it alive.
    let _ = net.join();
    result
}

/// Decode packets from the channel and feed samples to the sink.
fn decode_loop<E>(
    rx: Receiver<Vec<u8>>,
    sink: &Sink,
    volume: &AtomicU32,
    should_run: &ShouldRun,
    emit: &E,
    played: &mut bool,
) -> Result<()>
where
    E: Fn(PlayerEvent) + Send + Clone + 'static,
{
    let source = Box::new(ChannelSource::new(rx, should_run.clone()));
    let mss = MediaSourceStream::new(source, Default::default());

    let mut hint = Hint::new();
    hint.mime_type("audio/mpeg");

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("probing stream format")?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .cloned()
        .context("stream has no audio track")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("creating decoder")?;

    let mut volume_bits = volume.load(Ordering::Relaxed);
    while should_run() {
        // Pick up live volume changes (e.g. from a config-file edit).
        let bits = volume.load(Ordering::Relaxed);
        if bits != volume_bits {
            volume_bits = bits;
            sink.set_volume(f32::from_bits(bits));
        }

        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                return Ok(()); // stream ended
            }
            Err(SymError::ResetRequired) => return Ok(()), // reconnect to re-probe
            Err(e) => return Err(anyhow!("reading packet: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let channels = spec.channels.count() as u16;
                let mut sample_buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                sample_buf.copy_interleaved_ref(decoded);

                // Back-pressure: don't let the queue grow without bound.
                while sink.len() > 32 && should_run() {
                    thread::sleep(Duration::from_millis(20));
                }
                if !should_run() {
                    return Ok(());
                }
                sink.append(SamplesBuffer::new(
                    channels,
                    spec.rate,
                    sample_buf.samples().to_vec(),
                ));

                if !*played {
                    *played = true;
                    sink.play();
                    emit(PlayerEvent::Status(PlaybackStatus::Playing));
                }
            }
            // A corrupt packet mid-stream is expected occasionally; skip it.
            Err(SymError::DecodeError(e)) => log::debug!("decode error (skipping): {e}"),
            Err(SymError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(anyhow!("decoding: {e}")),
        }
    }
    Ok(())
}

/// Sleep for `total`, waking early once the session is retired.
fn sleep_interruptible(total: Duration, should_run: &ShouldRun) {
    let step = Duration::from_millis(100);
    let mut elapsed = Duration::ZERO;
    while elapsed < total && should_run() {
        thread::sleep(step);
        elapsed += step;
    }
}

/// A [`MediaSource`] backed by a channel of audio byte chunks. Reads block until
/// the next chunk arrives (natural pacing); a closed channel signals EOF. The
/// stream is not seekable (it is live radio).
struct ChannelSource {
    rx: Receiver<Vec<u8>>,
    should_run: ShouldRun,
    current: Vec<u8>,
    pos: usize,
}

impl ChannelSource {
    fn new(rx: Receiver<Vec<u8>>, should_run: ShouldRun) -> Self {
        Self {
            rx,
            should_run,
            current: Vec::new(),
            pos: 0,
        }
    }
}

impl Read for ChannelSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.current.len() {
            while (self.should_run)() {
                match self.rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(chunk) => {
                        self.current = chunk;
                        self.pos = 0;
                        if self.current.is_empty() {
                            return Ok(0);
                        }
                        break;
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return Ok(0),
                }
            }
            if !(self.should_run)() {
                return Ok(0);
            }
        }
        let n = (self.current.len() - self.pos).min(buf.len());
        buf[..n].copy_from_slice(&self.current[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl Seek for ChannelSource {
    fn seek(&mut self, _pos: SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "live stream is not seekable",
        ))
    }
}

impl MediaSource for ChannelSource {
    fn is_seekable(&self) -> bool {
        false
    }
    fn byte_len(&self) -> Option<u64> {
        None
    }
}
