//! Audio engine: a pure-Rust pipeline that streams the Example Radio MP3 feed,
//! strips ICY metadata, decodes with Symphonia and plays through Rodio/CPAL.
//!
//! Design
//! ------
//! * A single **engine thread** owns the audio output device and processes
//!   commands ([`Command`]) from the UI.
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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, Sink};
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
        thread::Builder::new()
            .name("audio-engine".into())
            .spawn(move || engine(config, cmd_rx, emit))
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
    pub fn quit(&self) {
        let _ = self.cmd_tx.send(Command::Quit);
    }
}

/// A predicate shared between worker/network threads: "is my generation still
/// the active one?". When it returns false the session must wind down.
type ShouldRun = Arc<dyn Fn() -> bool + Send + Sync>;

fn engine<E>(config: Config, cmd_rx: Receiver<Command>, emit: E)
where
    E: Fn(PlayerEvent) + Send + Clone + 'static,
{
    // Keep the output stream alive for the whole engine lifetime. It is not
    // `Send`, so it must stay on this thread; workers create their own `Sink`
    // from a cloned handle.
    let (_stream, handle) = match OutputStream::try_default() {
        Ok(pair) => pair,
        Err(err) => {
            log::error!("no audio output device: {err}");
            emit(PlayerEvent::Status(PlaybackStatus::Error));
            return;
        }
    };

    let generation = Arc::new(AtomicUsize::new(0));
    let mut playing = false;

    let start = |generation: &Arc<AtomicUsize>| {
        let my_gen = generation.fetch_add(1, Ordering::SeqCst) + 1;
        let gen = generation.clone();
        let should_run: ShouldRun = Arc::new(move || gen.load(Ordering::SeqCst) == my_gen);
        let cfg = config.clone();
        let handle = handle.clone();
        let emit = emit.clone();
        emit(PlayerEvent::Status(PlaybackStatus::Buffering));
        thread::Builder::new()
            .name("audio-worker".into())
            .spawn(move || worker(cfg, handle, should_run, emit))
            .expect("spawn audio worker");
    };

    let stop = |generation: &Arc<AtomicUsize>| {
        generation.fetch_add(1, Ordering::SeqCst);
    };

    for cmd in cmd_rx.iter() {
        match cmd {
            Command::Play if !playing => {
                playing = true;
                start(&generation);
            }
            Command::Pause if playing => {
                playing = false;
                stop(&generation);
                emit(PlayerEvent::Status(PlaybackStatus::Paused));
                emit(PlayerEvent::Title(None));
            }
            Command::Toggle => {
                if playing {
                    playing = false;
                    stop(&generation);
                    emit(PlayerEvent::Status(PlaybackStatus::Paused));
                    emit(PlayerEvent::Title(None));
                } else {
                    playing = true;
                    start(&generation);
                }
            }
            Command::Quit => {
                stop(&generation);
                break;
            }
            Command::Play | Command::Pause => {} // already in the requested state
        }
    }
}

/// Per-session worker: owns a [`Sink`] and reconnects with exponential backoff
/// until its generation is retired.
fn worker<E>(config: Config, handle: rodio::OutputStreamHandle, should_run: ShouldRun, emit: E)
where
    E: Fn(PlayerEvent) + Send + Clone + 'static,
{
    let sink = match Sink::try_new(&handle) {
        Ok(s) => s,
        Err(err) => {
            log::error!("cannot create audio sink: {err}");
            emit(PlayerEvent::Status(PlaybackStatus::Error));
            return;
        }
    };
    sink.set_volume(config.volume.clamp(0.0, 1.0));

    let mut backoff = Duration::from_secs(1);
    while should_run() {
        emit(PlayerEvent::Status(PlaybackStatus::Buffering));
        match stream_session(&config, &sink, &should_run, &emit) {
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
                log::warn!("stream error, reconnecting in {backoff:?}: {err:#}");
                emit(PlayerEvent::Status(PlaybackStatus::Error));
                sleep_interruptible(backoff, &should_run);
                backoff = (backoff * 2).min(Duration::from_secs(30));
            }
        }
    }
    // Dropping `sink` here stops any queued audio for this session.
}

/// Connect once and decode until the stream ends, errors, or the session is
/// retired. Returns `Ok(())` on a clean end (so the caller reconnects promptly).
fn stream_session<E>(config: &Config, sink: &Sink, should_run: &ShouldRun, emit: &E) -> Result<()>
where
    E: Fn(PlayerEvent) + Send + Clone + 'static,
{
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .build()
        .context("building HTTP client")?;

    let response = client
        .get(&config.stream_url)
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

    let result = decode_loop(rx, sink, should_run, emit);
    if should_run() {
        let _ = net.join();
    }
    result
}

/// Decode packets from the channel and feed samples to the sink.
fn decode_loop<E>(
    rx: Receiver<Vec<u8>>,
    sink: &Sink,
    should_run: &ShouldRun,
    emit: &E,
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

    let mut announced = false;
    while should_run() {
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

                if !announced {
                    announced = true;
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
