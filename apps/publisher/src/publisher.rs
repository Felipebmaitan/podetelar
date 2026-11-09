use crate::{
    admission::Admission,
    audio, delivery,
    encoding::{Codec, Encoder, Encoding, Metrics, Mode},
    preview::{self, Latest},
    quality::{MAX_BITRATE, RateControl, Settings},
    sources::{self, Source, SourceInfo},
};
use anyhow::{Context, Result, bail};
use livekit::{
    options::{AudioEncoding, TrackPublishOptions, VideoCodec, VideoEncoderBackend, VideoEncoding},
    prelude::*,
    webrtc::{
        audio_source::{AudioSourceOptions, RtcAudioSource, native::NativeAudioSource},
        video_source::{RtcVideoSource, VideoResolution, native::NativeVideoSource},
    },
};
use std::{
    future::Future,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{sync::mpsc, task::JoinHandle};

const TRACK_NAME: &str = "FreedomStream screen";
const AUDIO_TRACK_NAME: &str = "FreedomStream application audio";
#[derive(Default)]
pub(crate) struct CaptureCancellation(Mutex<Vec<Weak<AtomicBool>>>);

impl CaptureCancellation {
    pub(crate) fn register(&self) -> Arc<AtomicBool> {
        let stop = Arc::new(AtomicBool::new(false));
        let mut captures = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        captures.retain(|capture| capture.strong_count() > 0);
        captures.push(Arc::downgrade(&stop));
        stop
    }

    pub(crate) fn cancel(&self) {
        let captures = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for stop in captures.iter().filter_map(Weak::upgrade) {
            stop.store(true, Ordering::Release);
        }
    }
}

type Cancellation = Arc<CaptureCancellation>;

#[derive(Clone)]
pub struct Snapshot {
    pub connection: &'static str,
    pub room_name: String,
    pub error: String,
    pub busy: bool,
    pub sharing: bool,
    pub capture_pending: bool,
    pub sources: Vec<SourceInfo>,
    pub encoders: Vec<Encoder>,
    pub metrics: Metrics,
    pub audio: audio::Metrics,
    pub delivery: delivery::Metrics,
    pub mode: Option<Mode>,
    pub applications: Vec<audio::Application>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            connection: "disconnected",
            room_name: String::new(),
            error: String::new(),
            busy: false,
            sharing: false,
            capture_pending: false,
            sources: Vec::new(),
            encoders: Vec::new(),
            metrics: Metrics::default(),
            audio: audio::Metrics::default(),
            delivery: delivery::Metrics::default(),
            mode: None,
            applications: Vec::new(),
        }
    }
}

pub enum Command {
    Join {
        invitation: String,
        name: String,
    },
    Start {
        source_id: String,
        encoder: String,
        audio: audio::Choice,
        exclusions: Vec<String>,
        quality: Settings,
    },
    Apply {
        source_id: String,
        encoder: String,
        audio: audio::Choice,
        exclusions: Vec<String>,
        quality: Settings,
    },
    OpenViewer {
        invitation: String,
    },
    Stop,
    Leave,
    Refresh,
    Shutdown,
}

pub struct Controller {
    commands: mpsc::Sender<QueuedCommand>,
    snapshot: Arc<Mutex<Snapshot>>,
    cancellation: Cancellation,
}

struct QueuedCommand {
    command: Command,
    capture: Option<Arc<AtomicBool>>,
}

fn update(snapshot: &Mutex<Snapshot>, apply: impl FnOnce(&mut Snapshot)) {
    if let Ok(mut state) = snapshot.lock() {
        apply(&mut state);
    }
}

impl Controller {
    pub fn new() -> Self {
        let (commands, receiver) = mpsc::channel(8);
        let snapshot = Arc::new(Mutex::new(Snapshot::default()));
        let cancellation = Arc::new(CaptureCancellation::default());
        let state = snapshot.clone();
        let cancel = cancellation.clone();
        std::thread::Builder::new()
            .name("freedomstream-room".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build();
                match runtime {
                    Ok(runtime) => runtime.block_on(worker(receiver, state, cancel)),
                    Err(_) => update(&state, |state| {
                        state.error = "Could not start the publisher worker.".into()
                    }),
                }
            })
            .expect("publisher worker thread");
        Self {
            commands,
            snapshot,
            cancellation,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default()
    }

    pub fn send(&self, command: Command) {
        let capture_pending = matches!(command, Command::Start { .. } | Command::Apply { .. });
        // Register before enqueueing: Stop must also cancel capture work that
        // has not yet reached the room worker.
        let capture = capture_pending.then(|| self.cancellation.register());
        if matches!(command, Command::Stop | Command::Leave | Command::Shutdown) {
            self.cancel_capture();
        }
        if self
            .commands
            .try_send(QueuedCommand { command, capture })
            .is_err()
        {
            update(&self.snapshot, |state| {
                state.error = "Publisher is busy. Try again shortly.".into()
            });
        } else {
            update(&self.snapshot, |state| {
                state.busy = true;
                state.capture_pending |= capture_pending;
                state.error.clear();
            });
        }
    }

    fn cancel_capture(&self) {
        self.cancellation.cancel();
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.cancel_capture();
        let _ = self.commands.try_send(QueuedCommand {
            command: Command::Shutdown,
            capture: None,
        });
    }
}

#[derive(Clone)]
pub struct StreamConfig {
    pub source: Source,
    pub encoder: Encoder,
    pub quality: Settings,
    pub audio: audio::Choice,
    pub exclusions: Vec<String>,
}
impl StreamConfig {
    fn live_mode(&self, current: &Self, mode: Mode) -> Result<Option<Mode>> {
        if current.source.info.id != self.source.info.id
            || current.encoder.factory != self.encoder.factory
            || current.encoder.codec != self.encoder.codec
            || current.audio != self.audio
            || current.exclusions != self.exclusions
            || current.quality.resolution != self.quality.resolution
            || current.quality.fps != self.quality.fps
        {
            return Ok(None);
        }
        // Geometry remains the capture-start choice even if a window grew or
        // changed aspect ratio between starting capture and this bitrate edit.
        anyhow::ensure!(
            sources::validate(&self.source)
                && self.source.info.width >= mode.width
                && self.source.info.height >= mode.height,
            "Source is smaller than the active resolution. Stop and choose a smaller mode."
        );
        Ok(Some(Mode {
            bitrate: self
                .quality
                .bitrate_for(mode.width, mode.height, self.encoder.codec)?,
            ..mode
        }))
    }
    pub fn mode(&self) -> Result<Mode> {
        let mode = self
            .quality
            .resolve(&self.source.info, self.encoder.codec)?;
        anyhow::ensure!(
            self.encoder.supports(mode),
            "This resolution/frame rate is unavailable on the selected hardware encoder."
        );
        anyhow::ensure!(
            sources::validate(&self.source),
            "Refresh and select an available capture source."
        );
        Ok(mode)
    }
}
pub struct Stream {
    stop: StopOnDrop,
    task: JoinHandle<Result<Metrics>>,
    video: LocalVideoTrack,
    pub metrics: Arc<Mutex<Metrics>>,
    pub audio_metrics: Arc<Mutex<audio::Metrics>>,
    audio: Option<(JoinHandle<Result<audio::Metrics>>, LocalAudioTrack)>,
    previews: Latest,
    preview_task: Option<JoinHandle<()>>,
    pub preview_uploads: Arc<AtomicU64>,
    pub delivery: Arc<Mutex<delivery::Metrics>>,
    delivery_task: JoinHandle<()>,
    config: StreamConfig,
    rate: RateControl,
    pub mode: Mode,
}

struct StopOnDrop {
    flag: Arc<AtomicBool>,
    tracks: Vec<LocalTrack>,
}
impl StopOnDrop {
    fn new(flag: Arc<AtomicBool>) -> Self {
        Self {
            flag,
            tracks: Vec::new(),
        }
    }
    fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
        for track in &self.tracks {
            track.rtc_track().set_enabled(false);
        }
    }
}
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl Stream {
    pub fn sid(&self) -> String {
        self.video.sid().to_string()
    }
    pub fn encoder(&self) -> &Encoder {
        &self.config.encoder
    }
    pub async fn start(room: &Room, config: StreamConfig, stop: Arc<AtomicBool>) -> Result<Self> {
        let mut lease = StopOnDrop::new(stop.clone());
        let mode = config.mode()?;
        let source = config.source.clone();
        let encoder = config.encoder.clone();
        let cancelled = stop.clone();
        let encoding = tokio::task::spawn_blocking(move || {
            Encoding::start(&source, &encoder, mode, &cancelled)
        })
        .await
        .context("Capture worker stopped.")??;
        let codec = encoder_codec(&encoding);
        let rate = encoding.rate();
        let previews = encoding.previews();
        let rtc_source = NativeVideoSource::new_encoded(VideoResolution {
            width: mode.width,
            height: mode.height,
        });
        let track = LocalVideoTrack::create_video_track(
            TRACK_NAME,
            RtcVideoSource::Native(rtc_source.clone()),
        );
        lease.tracks.push(LocalTrack::Video(track.clone()));
        let video = track.clone();
        let outgoing = track.clone();
        let metrics = Arc::new(Mutex::new(Metrics::default()));
        let counters = metrics.clone();
        let cancel = stop.clone();
        // The pump owns capture before publication awaits the network. Stop and
        // terminal-room events can release devices while that request is pending.
        let task = tokio::task::spawn_blocking(move || {
            let result = encoding.run(Some(rtc_source), cancel.clone(), counters, None);
            outgoing.disable();
            cancel.store(true, Ordering::Release);
            result
        });
        let options = TrackPublishOptions {
            video_codec: codec,
            video_encoder: VideoEncoderBackend::PreEncoded,
            simulcast: false,
            source: TrackSource::Screenshare,
            video_encoding: Some(VideoEncoding {
                // Keep the RTC transport ceiling stable across live bitrate edits.
                // The GPU encoder always obeys the user's smaller cap and BWE.
                max_bitrate: MAX_BITRATE,
                max_framerate: mode.fps as f64,
            }),
            ..Default::default()
        };
        let publication = tokio::time::timeout(
            Duration::from_secs(12),
            room.local_participant()
                .publish_track(LocalTrack::Video(track), options),
        )
        .await;
        let _publication = match publication {
            Ok(Ok(publication)) => publication,
            other => {
                stop.store(true, Ordering::Release);
                let _ = task.await;
                // A timed-out publication has ambiguous server state; close this
                // media identity before allowing another attempt.
                let _ = room.close().await;
                let _ = other;
                bail!("Media publication failed. Join the room again to retry.");
            }
        };
        let delivery = Arc::new(Mutex::new(delivery::Metrics::default()));
        let delivery_task = tokio::spawn(delivery::monitor(
            video.clone(),
            stop.clone(),
            delivery.clone(),
        ));
        let audio_choice = config.audio.clone();
        let exclusions = config.exclusions.clone();
        let mut active = Self {
            stop: lease,
            task,
            video,
            metrics,
            audio_metrics: Arc::new(Mutex::new(audio::Metrics::default())),
            audio: None,
            previews,
            preview_task: None,
            preview_uploads: Arc::new(AtomicU64::new(0)),
            delivery,
            delivery_task,
            config,
            rate,
            mode,
        };
        if audio_choice != audio::Choice::Off {
            let source = NativeAudioSource::new(AudioSourceOptions::default(), 48000, 2, 50);
            let track = LocalAudioTrack::create_audio_track(
                AUDIO_TRACK_NAME,
                RtcAudioSource::Native(source.clone()),
            );
            active.stop.tracks.push(LocalTrack::Audio(track.clone()));
            let outgoing = track.clone();
            let cancel = active.stop.flag.clone();
            let counters = active.audio_metrics.clone();
            let task = tokio::task::spawn_blocking(move || {
                let result = audio::run(
                    audio_choice,
                    exclusions,
                    cancel.clone(),
                    counters,
                    Some(source),
                    None,
                );
                outgoing.disable();
                cancel.store(true, Ordering::Release);
                result
            });
            let publication = tokio::time::timeout(
                Duration::from_secs(12),
                room.local_participant().publish_track(
                    LocalTrack::Audio(track.clone()),
                    TrackPublishOptions {
                        source: TrackSource::ScreenshareAudio,
                        audio_encoding: Some(AudioEncoding {
                            max_bitrate: 192_000,
                        }),
                        dtx: false,
                        ..Default::default()
                    },
                ),
            )
            .await;
            match publication {
                Ok(Ok(_publication)) => {
                    active.audio = Some((task, track));
                }
                _ => {
                    active.stop.cancel();
                    let _ = task.await;
                    let _ = active.stop(room).await;
                    let _ = room.close().await;
                    bail!("Audio publication failed. Join the room again to retry.");
                }
            }
        }
        Ok(active)
    }

    pub fn start_previews(&mut self, admission: Admission) {
        if self.preview_task.is_none() {
            self.preview_task = Some(tokio::spawn(preview::upload(
                admission,
                self.previews.clone(),
                self.video.clone(),
                self.stop.flag.clone(),
                self.preview_uploads.clone(),
            )));
        }
    }

    /// Returns false when capture/publication must restart. Validation precedes
    /// any mutation, so an invalid edit cannot interrupt a working stream.
    pub fn apply_live(&mut self, config: &StreamConfig) -> Result<bool> {
        let Some(mode) = config.live_mode(&self.config, self.mode)? else {
            return Ok(false);
        };
        self.rate.set(mode.bitrate)?;
        self.mode = mode;
        self.config = config.clone();
        Ok(true)
    }

    pub fn finished(&self) -> bool {
        self.task.is_finished()
            || self
                .audio
                .as_ref()
                .is_some_and(|(task, _)| task.is_finished())
    }

    pub async fn stop(self, room: &Room) -> Result<Metrics> {
        self.stop.cancel();
        self.delivery_task.abort();
        let _ = self.delivery_task.await;
        let captured = self
            .task
            .await
            .map_err(|_| anyhow::anyhow!("Capture worker stopped unexpectedly."))
            .and_then(|result| result);
        let mut audio_result = Ok(());
        let mut publication_ids = Vec::new();
        if let Some((task, track)) = self.audio {
            audio_result = match task.await {
                Ok(result) => result.map(|_| ()),
                Err(_) => Err(anyhow::anyhow!("Audio worker stopped unexpectedly.")),
            };
            publication_ids.push(track.sid());
        }
        publication_ids.push(self.video.sid());
        let owned_ids: Vec<_> = self
            .stop
            .tracks
            .iter()
            .map(|track| track.rtc_track().id())
            .collect();
        // Both native pumps are released before network cleanup. The SDK drops
        // a publication from its local map before removing its RTC sender, so
        // an empty map is not proof that failed/timed-out removal succeeded.
        settle_publications(
            publication_ids.into_iter().map(|sid| async move {
                matches!(
                    tokio::time::timeout(
                        Duration::from_secs(5),
                        room.local_participant().unpublish_track(&sid)
                    )
                    .await,
                    Ok(Ok(_))
                )
            }),
            || {
                room.connection_state() != ConnectionState::Connected
                    || room
                        .local_participant()
                        .track_publications()
                        .values()
                        .filter_map(|publication| publication.track())
                        .any(|track| owned_ids.contains(&track.rtc_track().id()))
            },
            async {
                let _ = room.close().await;
            },
        )
        .await;
        if let Some(task) = self.preview_task {
            let _ = task.await;
        }
        audio_result?;
        captured
    }
}

async fn settle_publications<R, C>(
    removals: impl IntoIterator<Item = R>,
    force_close: impl FnOnce() -> bool,
    close: C,
) where
    R: Future<Output = bool>,
    C: Future<Output = ()>,
{
    // Await every removal, even after one fails. There are at most two tracks,
    // and their individual deadlines bound the complete concurrent cleanup.
    let removed = futures_util::future::join_all(removals).await;
    if removed.into_iter().any(|success| !success) || force_close() {
        close.await;
    }
}

fn encoder_codec(encoding: &Encoding) -> VideoCodec {
    match encoding.codec() {
        Codec::H265 => VideoCodec::H265,
        Codec::H264 => VideoCodec::H264,
    }
}

struct Session {
    admission: Admission,
    room: Arc<Room>,
    events: JoinHandle<()>,
    expected_audio: Arc<AtomicBool>,
}

impl Session {
    async fn join(
        invitation: &str,
        name: &str,
        snapshot: Arc<Mutex<Snapshot>>,
        cancellation: Cancellation,
    ) -> Result<Self> {
        let admission = Admission::join(invitation, name).await?;
        let access = admission.media_access().await?;
        let mut options = RoomOptions::default();
        options.auto_subscribe = false;
        options.join_retries = 0;
        let (room, mut events) = tokio::time::timeout(
            Duration::from_secs(15),
            Room::connect(&access.server_url, &access.token, options),
        )
        .await
        .context("Media connection timed out.")?
        .map_err(|_| anyhow::anyhow!("Could not connect to the media server."))?;
        let room = Arc::new(room);
        let media_room = room.clone();
        let expected_audio = Arc::new(AtomicBool::new(false));
        let audio_expected = expected_audio.clone();
        let events = tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                match event {
                    RoomEvent::LocalTrackPublished {
                        publication, track, ..
                    }
                    | RoomEvent::LocalTrackRepublished {
                        publication, track, ..
                    } if !track.rtc_track().enabled() => {
                        let cleanup = tokio::time::timeout(
                            Duration::from_secs(5),
                            media_room
                                .local_participant()
                                .unpublish_track(&publication.sid()),
                        )
                        .await;
                        if !matches!(cleanup, Ok(Ok(_))) {
                            let _ = media_room.close().await;
                        }
                    }
                    RoomEvent::Disconnected { .. } => {
                        cancellation.cancel();
                        update(&snapshot, |state| {
                            state.connection = "disconnected";
                            state.error = "Media disconnected. Join again to retry.".into();
                        });
                    }
                    RoomEvent::Reconnecting => {
                        update(&snapshot, |state| state.connection = "reconnecting")
                    }
                    RoomEvent::Reconnected => {
                        let publications = media_room.local_participant().track_publications();
                        let published = publications
                            .values()
                            .any(|publication| publication.name() == TRACK_NAME)
                            && (!audio_expected.load(Ordering::Acquire)
                                || publications
                                    .values()
                                    .any(|publication| publication.name() == AUDIO_TRACK_NAME));
                        if !published {
                            cancellation.cancel();
                        }
                        update(&snapshot, |state| state.connection = "connected");
                    }
                    _ => {}
                }
            }
        });
        Ok(Self {
            admission,
            room,
            events,
            expected_audio,
        })
    }

    async fn close(self) {
        let _ = self.room.close().await;
        self.events.abort();
        let _ = self.events.await;
        self.admission.leave().await;
    }
}

async fn worker(
    mut commands: mpsc::Receiver<QueuedCommand>,
    snapshot: Arc<Mutex<Snapshot>>,
    cancellation: Cancellation,
) {
    let mut sources = sources::enumerate();
    update(&snapshot, |state| {
        state.sources = sources.iter().map(|source| source.info.clone()).collect();
        state.encoders = crate::encoding::encoders();
        match audio::applications() {
            Ok(applications) => state.applications = applications,
            Err(_) => {
                state.error =
                    "Audio application discovery is unavailable. Video remains available.".into()
            }
        }
    });
    let mut session: Option<Session> = None;
    let mut stream: Option<Stream> = None;
    let mut viewer: Option<crate::viewer::Viewer> = None;
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    loop {
        let queued = tokio::select! {
            command = commands.recv() => command.unwrap_or(QueuedCommand { command: Command::Shutdown, capture: None }),
            _ = tick.tick() => {
                if let Some(active) = &stream {
                    if let Ok(metrics) = active.metrics.lock() { update(&snapshot, |state| state.metrics = metrics.clone()); }
                    if let Ok(metrics) = active.audio_metrics.lock() { update(&snapshot, |state| state.audio = metrics.clone()); }
                    if let Ok(metrics) = active.delivery.lock() { update(&snapshot, |state| { state.delivery = metrics.clone(); state.mode=Some(active.mode); }); }
                    if !active.finished() { continue; }
                    if let (Some(active), Some(joined)) = (stream.take(), session.as_ref()) {
                        let result = active.stop(&joined.room).await;
                        update(&snapshot, |state| { state.sharing = false; if let Err(error) = result { state.error = error.to_string(); } });
                    }
                }
                continue;
            }
        };
        let command = queued.command;
        let shutdown = matches!(command, Command::Shutdown);
        let result: Result<()> = async {
            match command {
                Command::Join { invitation, name } => {
                    viewer.take();
                    if let (Some(active), Some(joined)) = (stream.take(), session.as_ref()) {
                        let _ = active.stop(&joined.room).await;
                    }
                    if let Some(joined) = session.take() {
                        joined.close().await;
                    }
                    update(&snapshot, |state| {
                        state.connection = "connecting";
                        state.sharing = false;
                    });
                    let joined =
                        Session::join(&invitation, &name, snapshot.clone(), cancellation.clone())
                            .await?;
                    update(&snapshot, |state| {
                        state.connection = "connected";
                        state.room_name = joined.admission.room_name.clone();
                    });
                    session = Some(joined);
                }
                Command::Start {
                    source_id,
                    encoder,
                    audio,
                    exclusions,
                    quality,
                }
                | Command::Apply {
                    source_id,
                    encoder,
                    audio,
                    exclusions,
                    quality,
                } => {
                    let joined = session.as_ref().context("Join a room first.")?;
                    let source = sources::enumerate()
                        .iter()
                        .find(|source| source.info.id == source_id)
                        .context("Refresh and choose a capture source.")?
                        .clone();
                    let encoder = crate::encoding::encoders()
                        .into_iter()
                        .find(|candidate| candidate.factory == encoder)
                        .context("Choose an available hardware encoder.")?;
                    let stop = queued.capture.context("Capture request was cancelled.")?;
                    anyhow::ensure!(
                        !stop.load(Ordering::Acquire),
                        "Capture request was cancelled."
                    );
                    let config = StreamConfig {
                        source,
                        encoder,
                        quality,
                        audio: audio.clone(),
                        exclusions,
                    };
                    if let Some(active) = stream.as_mut()
                        && active.apply_live(&config)?
                    {
                        update(&snapshot, |state| state.mode = Some(active.mode));
                        return Ok(());
                    }
                    config.mode()?;
                    if let Some(active) = stream.take() {
                        active.stop(&joined.room).await?;
                    }
                    joined
                        .expected_audio
                        .store(audio != audio::Choice::Off, Ordering::Release);
                    let mut active = Stream::start(&joined.room, config, stop.clone()).await?;
                    active.start_previews(joined.admission.clone());
                    if stop.load(Ordering::Acquire) {
                        let _ = active.stop(&joined.room).await;
                    } else {
                        let mode = active.mode;
                        stream = Some(active);
                        update(&snapshot, |state| {
                            state.sharing = true;
                            state.mode = Some(mode);
                            state.metrics = Metrics::default();
                            state.delivery = delivery::Metrics::default();
                        });
                    }
                }
                Command::Stop => {
                    if let (Some(active), Some(joined)) = (stream.take(), session.as_ref()) {
                        active.stop(&joined.room).await?;
                    }
                    update(&snapshot, |state| state.sharing = false);
                }
                Command::Leave | Command::Shutdown => {
                    if let (Some(active), Some(joined)) = (stream.take(), session.as_ref()) {
                        let _ = active.stop(&joined.room).await;
                    }
                    if let Some(joined) = session.take() {
                        joined.close().await;
                    }
                    viewer.take();
                    update(&snapshot, |state| {
                        state.connection = "disconnected";
                        state.sharing = false;
                        state.room_name.clear();
                    });
                }
                Command::Refresh => {
                    sources = sources::enumerate();
                    update(&snapshot, |state| {
                        state.sources = sources.iter().map(|source| source.info.clone()).collect();
                        state.encoders = crate::encoding::encoders();
                        if let Ok(applications) = audio::applications() {
                            state.applications = applications;
                        }
                    });
                }
                Command::OpenViewer { invitation } => {
                    session.as_ref().context("Join a room first.")?;
                    viewer.take();
                    viewer = Some(crate::viewer::Viewer::open(&invitation, false).await?);
                }
            }
            Ok(())
        }
        .await;
        update(&snapshot, |state| {
            state.busy = false;
            state.capture_pending = false;
            state.sharing = stream.is_some();
            if let Err(error) = result {
                state.error = error.to_string();
                if state.connection == "connecting" {
                    state.connection = "disconnected";
                }
            }
        });
        if shutdown {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitrate_edit_keeps_active_geometry_after_source_aspect_changes_and_rejects_invalid_edits() {
        let current = StreamConfig {
            source: sources::fixture(),
            encoder: Encoder {
                factory: "fixture".into(),
                label: "Synthetic".into(),
                codec: Codec::H265,
            },
            quality: Settings::default(),
            audio: audio::Choice::Off,
            exclusions: Vec::new(),
        };
        let mode = Mode {
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate: 12_000_000,
        };
        let mut next = current.clone();
        next.source.info.width = 2560;
        next.quality.bitrate = crate::quality::Bitrate::Manual(25_000_000);
        assert_eq!(
            next.live_mode(&current, mode).unwrap(),
            Some(Mode {
                bitrate: 25_000_000,
                ..mode
            })
        );
        next.quality.fps = 120;
        assert_eq!(next.live_mode(&current, mode).unwrap(), None);
        next.quality.fps = 60;
        next.quality.bitrate = crate::quality::Bitrate::Manual(2_000_000);
        assert!(next.live_mode(&current, mode).is_err());
        next.quality.bitrate = crate::quality::Bitrate::Auto;
        next.source.info.width = 1280;
        assert!(next.live_mode(&current, mode).is_err());
    }

    #[test]
    fn stop_reaches_active_and_queued_captures_without_cancelling_future_requests() {
        let cancellation = CaptureCancellation::default();
        let active = cancellation.register();
        let queued = cancellation.register();
        cancellation.cancel();
        assert!(active.load(Ordering::Acquire));
        assert!(queued.load(Ordering::Acquire));
        let later = cancellation.register();
        assert!(!later.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn cancelling_a_pending_operation_drops_its_capture_lease() {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (started, entered) = tokio::sync::oneshot::channel();
        let operation = tokio::spawn(async move {
            let _lease = StopOnDrop::new(flag);
            let _ = started.send(());
            std::future::pending::<()>().await;
        });
        entered.await.unwrap();
        operation.abort();
        let _ = operation.await;
        assert!(stop.load(Ordering::Acquire));
    }

    #[test]
    fn capture_lease_disables_the_real_rtc_tracks_even_when_other_owners_retain_them() {
        let audio = LocalAudioTrack::create_audio_track(
            "lease audio",
            RtcAudioSource::Native(NativeAudioSource::new(
                AudioSourceOptions::default(),
                48000,
                2,
                0,
            )),
        );
        let video = LocalVideoTrack::create_video_track(
            "lease video",
            RtcVideoSource::Native(NativeVideoSource::new_encoded(VideoResolution {
                width: 1280,
                height: 720,
            })),
        );
        assert!(audio.rtc_track().enabled());
        assert!(video.rtc_track().enabled());
        let mut lease = StopOnDrop::new(Arc::new(AtomicBool::new(false)));
        lease.tracks = vec![
            LocalTrack::Audio(audio.clone()),
            LocalTrack::Video(video.clone()),
        ];
        drop(lease);
        assert!(!audio.rtc_track().enabled());
        assert!(!video.rtc_track().enabled());
    }

    #[tokio::test]
    async fn failed_sender_removal_closes_even_after_sdk_empties_its_publication_map() {
        let publications = Arc::new(Mutex::new(std::collections::HashSet::from([
            "audio", "video",
        ])));
        let attempts = Arc::new(Mutex::new(Vec::new()));
        let closed = AtomicBool::new(false);
        let removals = ["audio", "video"].into_iter().map(|id| {
            let publications = publications.clone();
            let attempts = attempts.clone();
            async move {
                attempts.lock().unwrap().push(id);
                publications.lock().unwrap().remove(id);
                // SDK removes its map entry, then audio sender removal fails.
                id != "audio"
            }
        });
        settle_publications(
            removals,
            || !publications.lock().unwrap().is_empty(),
            async {
                closed.store(true, Ordering::Release);
            },
        )
        .await;
        assert!(publications.lock().unwrap().is_empty());
        assert_eq!(attempts.lock().unwrap().len(), 2);
        assert!(closed.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn successful_sender_removal_keeps_a_connected_media_identity_open() {
        let closed = AtomicBool::new(false);
        settle_publications(
            [std::future::ready(true), std::future::ready(true)],
            || false,
            async {
                closed.store(true, Ordering::Release);
            },
        )
        .await;
        assert!(!closed.load(Ordering::Acquire));
    }
}
