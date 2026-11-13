use crate::{
    admission::Admission,
    audio, connection, delivery,
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
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};

const TRACK_NAME: &str = "PodeTelar screen";
const AUDIO_TRACK_NAME: &str = "PodeTelar application audio";
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

// Unlike queued commands, this signal can preempt a network join already being
// awaited. A cancelled stage retains ownership until it can release its result.
struct JoinCancellation(watch::Sender<u64>);
impl JoinCancellation {
    fn new() -> Self {
        Self(watch::channel(0).0)
    }
    fn cancel(&self) {
        self.0
            .send_modify(|generation| *generation = generation.wrapping_add(1));
    }
    fn ticket(&self) -> JoinTicket {
        let receiver = self.0.subscribe();
        let generation = *receiver.borrow();
        JoinTicket {
            receiver,
            generation,
        }
    }
}

struct JoinTicket {
    receiver: watch::Receiver<u64>,
    generation: u64,
}
impl JoinTicket {
    async fn run<T, F, C, CF>(&self, future: F, cleanup: C) -> Result<Option<T>>
    where
        T: Send + 'static,
        F: Future<Output = Result<T>> + Send + 'static,
        C: FnOnce(T) -> CF + Send + 'static,
        CF: Future<Output = ()> + Send + 'static,
    {
        let mut receiver = self.receiver.clone();
        if *receiver.borrow() != self.generation {
            return Ok(None);
        }
        let mut pending = tokio::spawn(future);
        let cancelled = async {
            while *receiver.borrow_and_update() == self.generation {
                if receiver.changed().await.is_err() {
                    break;
                }
            }
        };
        let result = tokio::select! {
            result = &mut pending => Some(result.context("Room connection worker stopped.")?),
            _ = cancelled => None,
        };
        match result {
            Some(Ok(value)) if *self.receiver.borrow() == self.generation => Ok(Some(value)),
            Some(Ok(value)) => {
                tokio::spawn(cleanup(value));
                Ok(None)
            }
            Some(Err(_)) if *self.receiver.borrow() != self.generation => Ok(None),
            Some(Err(error)) => Err(error),
            None => {
                tokio::spawn(async move {
                    if let Ok(Ok(value)) = pending.await {
                        cleanup(value).await;
                    }
                });
                Ok(None)
            }
        }
    }
}

struct PendingAdmission(Option<Admission>);
impl Drop for PendingAdmission {
    fn drop(&mut self) {
        if let Some(admission) = self.0.take() {
            tokio::spawn(async move {
                admission.leave().await;
            });
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SharingSelection {
    pub source_id: String,
    pub encoder: String,
    pub quality: Settings,
}

#[derive(Clone)]
pub struct Snapshot {
    pub connection: &'static str,
    pub room_name: String,
    pub error: String,
    pub busy: bool,
    pub operation: &'static str,
    pub sharing: bool,
    pub capture_pending: bool,
    pub sources: Vec<SourceInfo>,
    pub encoders: Vec<Encoder>,
    pub metrics: Metrics,
    pub audio: audio::Metrics,
    pub delivery: delivery::Metrics,
    pub mode: Option<Mode>,
    pub applications: Vec<audio::Application>,
    pub tiles: Vec<crate::playback::Tile>,
    pub participants: Vec<String>,
    pub active_selection: Option<SharingSelection>,
    pub self_preview: Latest,
    pub capacity: crate::network::Capacity,
    pub testing_connection: bool,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            connection: "disconnected",
            room_name: String::new(),
            error: String::new(),
            busy: false,
            operation: "",
            sharing: false,
            capture_pending: false,
            sources: Vec::new(),
            encoders: Vec::new(),
            metrics: Metrics::default(),
            audio: audio::Metrics::default(),
            delivery: delivery::Metrics::default(),
            mode: None,
            applications: Vec::new(),
            tiles: Vec::new(),
            participants: Vec::new(),
            active_selection: None,
            self_preview: Arc::default(),
            capacity: crate::network::Capacity::default(),
            testing_connection: false,
        }
    }
}

pub enum Command {
    Join {
        invitation: String,
        name: String,
        route: connection::Route,
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
    DownloadBudget {
        bps: Option<u64>,
    },
    MeasureConnection,
    Watch {
        id: String,
        enabled: bool,
    },
    Stop,
    Leave,
    Refresh,
    RefreshSources,
    Shutdown,
}

pub struct Controller {
    commands: mpsc::Sender<QueuedCommand>,
    snapshot: Arc<Mutex<Snapshot>>,
    cancellation: Cancellation,
    join_cancellation: JoinCancellation,
    shutdown: Arc<Shutdown>,
    worker: Option<std::thread::JoinHandle<()>>,
}

#[derive(Default)]
struct Shutdown {
    requested: AtomicBool,
    finished: AtomicBool,
    wake: tokio::sync::Notify,
}

struct WorkerCompletion(Arc<Shutdown>);
impl Drop for WorkerCompletion {
    fn drop(&mut self) {
        self.0.finished.store(true, Ordering::Release);
    }
}

struct QueuedCommand {
    command: Command,
    capture: Option<Arc<AtomicBool>>,
    join: Option<JoinTicket>,
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
        let shutdown = Arc::new(Shutdown::default());
        let closing = shutdown.clone();
        let worker = std::thread::Builder::new()
            .name("freedomstream-room".into())
            .spawn(move || {
                let _completion = WorkerCompletion(closing.clone());
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build();
                match runtime {
                    Ok(runtime) => runtime.block_on(worker(receiver, state, cancel, closing)),
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
            join_cancellation: JoinCancellation::new(),
            shutdown,
            worker: Some(worker),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default()
    }

    pub fn send(&self, command: Command) {
        if matches!(command, Command::Shutdown) {
            self.begin_shutdown();
            return;
        }
        if self.shutdown.requested.load(Ordering::Acquire) {
            return;
        }
        if matches!(command, Command::RefreshSources) {
            // Metadata refresh is best-effort and must not disable/close the picker,
            // overwrite operation feedback, or allocate capture authority.
            let _ = self.commands.try_send(QueuedCommand {
                command,
                capture: None,
                join: None,
            });
            return;
        }
        if matches!(
            command,
            Command::Join { .. } | Command::Leave | Command::Shutdown
        ) {
            self.join_cancellation.cancel();
        }
        let join = matches!(command, Command::Join { .. }).then(|| self.join_cancellation.ticket());
        let operation = match &command {
            Command::Join { .. } => "Joining room…",
            Command::Start { .. } => "Starting your stream…",
            Command::Apply { .. } => "Applying stream settings…",
            Command::Stop => "Stopping your stream…",
            Command::Leave => "Leaving the room…",
            Command::Shutdown => "Closing…",
            Command::Refresh | Command::RefreshSources => "Refreshing available sources…",
            Command::DownloadBudget { .. } => "Updating your watching allowance…",
            Command::MeasureConnection => "Measuring your connection…",
            Command::Watch { enabled: true, .. } => "Starting playback…",
            Command::Watch { enabled: false, .. } => "Stopping playback…",
        };
        let capture_pending = matches!(command, Command::Start { .. } | Command::Apply { .. });
        // Register before enqueueing: Stop must also cancel capture work that
        // has not yet reached the room worker.
        let capture = capture_pending.then(|| self.cancellation.register());
        if matches!(command, Command::Stop | Command::Leave | Command::Shutdown) {
            self.cancel_capture();
        }
        if self
            .commands
            .try_send(QueuedCommand {
                command,
                capture,
                join,
            })
            .is_err()
        {
            update(&self.snapshot, |state| {
                state.error = "Publisher is busy. Try again shortly.".into()
            });
        } else {
            update(&self.snapshot, |state| {
                state.busy = true;
                state.operation = operation;
                state.capture_pending |= capture_pending;
                state.error.clear();
            });
        }
    }

    fn cancel_capture(&self) {
        self.cancellation.cancel();
    }

    pub fn begin_shutdown(&self) {
        self.join_cancellation.cancel();
        self.cancel_capture();
        if !self.shutdown.requested.swap(true, Ordering::AcqRel) {
            self.shutdown.wake.notify_one();
        }
    }

    pub fn shutdown_finished(&self) -> bool {
        self.worker.as_ref().map_or_else(
            || self.shutdown.finished.load(Ordering::Acquire),
            crate::worker::finished,
        )
    }

    #[cfg(test)]
    pub(crate) fn recording_for_test(state: Snapshot) -> (Self, impl FnMut() -> Option<Command>) {
        let (commands, mut receiver) = mpsc::channel::<QueuedCommand>(8);
        let shutdown = Arc::new(Shutdown::default());
        shutdown.finished.store(true, Ordering::Release);
        (
            Self {
                commands,
                snapshot: Arc::new(Mutex::new(state)),
                cancellation: Arc::default(),
                join_cancellation: JoinCancellation::new(),
                shutdown,
                worker: None,
            },
            move || receiver.try_recv().ok().map(|queued| queued.command),
        )
    }

    #[cfg(test)]
    pub(crate) fn pending_shutdown_for_test() -> (Self, impl Fn()) {
        let (commands, _) = mpsc::channel(8);
        let shutdown = Arc::new(Shutdown::default());
        let completed = shutdown.clone();
        (
            Self {
                commands,
                snapshot: Arc::default(),
                cancellation: Arc::default(),
                join_cancellation: JoinCancellation::new(),
                shutdown,
                worker: None,
            },
            move || completed.finished.store(true, Ordering::Release),
        )
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.begin_shutdown();
        // Paint fixtures must not let native initialization/cleanup outlive the harness.
        #[cfg(test)]
        crate::worker::drain_fixture(self.worker.take());
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
    fn negotiate(&mut self, room: &Room, preferred: &Encoder, available: &[Encoder]) -> Result<()> {
        let shared = crate::playback::room_codecs(room);
        let codec = shared.best(preferred.codec, |codec| available.iter().any(|encoder| {
            encoder.codec == codec && self.quality.resolve(&self.source.info, codec).is_ok_and(|mode| encoder.supports(mode))
        })).context("No codec can be encoded at this resolution/frame rate and decoded by every room member.")?;
        self.encoder = available
            .iter()
            .filter(|encoder| {
                encoder.codec == codec
                    && self
                        .quality
                        .resolve(&self.source.info, codec)
                        .is_ok_and(|mode| encoder.supports(mode))
            })
            .min_by_key(|encoder| encoder.factory != preferred.factory)
            .context("No compatible hardware encoder is available.")?
            .clone();
        self.mode()?;
        Ok(())
    }
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
    preferred_encoder: Encoder,
    available_encoders: Vec<Encoder>,
    preview_capture: preview::Capture,
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
    diagnostic_task: Option<JoinHandle<()>>,
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
    pub fn cancelled(&self) -> bool {
        self.stop.flag.load(Ordering::Acquire)
    }

    pub fn constrain_watchers(&self, room: &Room) {
        self.rate
            .set_viewer_cap(crate::playback::requested_cap(room));
    }

    pub fn encoder(&self) -> &Encoder {
        &self.config.encoder
    }
    pub fn preview_captures(&self) -> u64 {
        self.preview_capture.gate.captures()
    }
    pub async fn start(room: &Room, config: StreamConfig, stop: Arc<AtomicBool>) -> Result<Self> {
        Self::start_with_upload_cap(room, config, stop, None).await
    }
    pub async fn start_with_upload_cap(
        room: &Room,
        mut config: StreamConfig,
        stop: Arc<AtomicBool>,
        upload_cap: Option<u64>,
    ) -> Result<Self> {
        let preferred = config.encoder.clone();
        config.negotiate(room, &preferred, &crate::encoding::encoders())?;
        let mut active =
            Self::start_with_preview(room, config, stop, upload_cap, preview::Capture::new())
                .await?;
        active.preferred_encoder = preferred;
        Ok(active)
    }
    async fn start_with_preview(
        room: &Room,
        config: StreamConfig,
        stop: Arc<AtomicBool>,
        upload_cap: Option<u64>,
        preview_capture: preview::Capture,
    ) -> Result<Self> {
        let mut lease = StopOnDrop::new(stop.clone());
        let mode = config.mode()?;
        let source = config.source.clone();
        let encoder = config.encoder.clone();
        let cancelled = stop.clone();
        let rate = RateControl::new(mode.bitrate);
        rate.set_upload_cap(upload_cap);
        rate.set_viewer_cap(crate::playback::requested_cap(room));
        let encoding_rate = rate.clone();
        let capture = preview_capture.clone();
        let encoding = tokio::task::spawn_blocking(move || {
            Encoding::start_with_preview(
                &source,
                &encoder,
                mode,
                &cancelled,
                encoding_rate,
                capture,
            )
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
            preferred_encoder: config.encoder.clone(),
            available_encoders: crate::encoding::encoders(),
            preview_capture,
            delivery,
            delivery_task,
            diagnostic_task: None,
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

    pub fn compatible_config(&self, room: &Room) -> Result<Option<StreamConfig>> {
        let mut config = self.config.clone();
        config.negotiate(room, &self.preferred_encoder, &self.available_encoders)?;
        Ok((config.encoder.factory != self.config.encoder.factory).then_some(config))
    }

    pub async fn fallback(
        self,
        room: &Room,
        admission: Admission,
        stop: Arc<AtomicBool>,
        config: StreamConfig,
    ) -> Result<Self> {
        let upload_cap = self.rate.upload_cap();
        let preferred_encoder = self.preferred_encoder.clone();
        let preview_capture = self.preview_capture.clone();
        let cached_preview = self.previews.lock().ok().and_then(|latest| latest.clone());
        let uploads = self.preview_uploads.clone();
        self.stop(room).await?;
        if let Ok(mut latest) = preview_capture.latest.lock() {
            *latest = cached_preview;
        }
        let mut replacement =
            Self::start_with_preview(room, config, stop, Some(upload_cap), preview_capture).await?;
        replacement.preferred_encoder = preferred_encoder;
        replacement.preview_uploads = uploads;
        replacement.start_previews(admission);
        Ok(replacement)
    }

    pub fn start_previews(&mut self, admission: Admission) {
        if self.diagnostic_task.is_none() {
            self.diagnostic_task = Some(tokio::spawn(crate::stream_diagnostics::sender(
                admission.clone(),
                self.video.clone(),
                self.stop.flag.clone(),
                self.metrics.clone(),
                self.delivery.clone(),
                self.rate.clone(),
                self.config.clone(),
                self.mode,
            )));
        }
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
        if let Some(task) = self.diagnostic_task {
            task.abort();
        }
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
        Codec::Av1 => VideoCodec::AV1,
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
        route: connection::Route,
        snapshot: Arc<Mutex<Snapshot>>,
        cancellation: Cancellation,
        recovery: Arc<AtomicU64>,
        ticket: JoinTicket,
    ) -> Result<Option<Self>> {
        let invitation = invitation.to_owned();
        let name = name.to_owned();
        let Some(admission) = ticket
            .run(
                async move { Admission::join(&invitation, &name).await },
                |admission| async move {
                    admission.leave().await;
                },
            )
            .await?
        else {
            return Ok(None);
        };
        let mut pending_admission = PendingAdmission(Some(admission.clone()));
        let client = admission.clone();
        let Some(access) = ticket
            .run(async move { client.media_access().await }, |_| async {})
            .await?
        else {
            return Ok(None);
        };
        let options = connection::options(route);
        let Some((room, mut events)) = ticket
            .run(
                async move {
                    tokio::time::timeout(
                        Duration::from_secs(15),
                        Room::connect(&access.server_url, &access.token, options),
                    )
                    .await
                    .context("Media connection timed out.")?
                    .map_err(|_| anyhow::anyhow!("Could not connect to the media server."))
                },
                |(room, _)| async move {
                    let _ = room.close().await;
                },
            )
            .await?
        else {
            return Ok(None);
        };
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
                        recovery.fetch_add(1, Ordering::AcqRel);
                        update(&snapshot, |state| state.connection = "reconnecting")
                    }
                    RoomEvent::Reconnected => {
                        recovery.fetch_add(1, Ordering::AcqRel);
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
        Ok(Some(Self {
            admission: pending_admission.0.take().expect("Accepted room admission"),
            room,
            events,
            expected_audio,
        }))
    }

    async fn close(self) {
        let _ = self.room.close().await;
        self.events.abort();
        let _ = self.events.await;
        self.admission.leave().await;
    }
}

struct ConnectionProbe(JoinHandle<crate::network::Capacity>);
impl ConnectionProbe {
    fn is_finished(&self) -> bool {
        self.0.is_finished()
    }
    fn abort(&self) {
        self.0.abort();
    }
    async fn finish(mut self) -> Result<crate::network::Capacity, tokio::task::JoinError> {
        (&mut self.0).await
    }
}
impl Drop for ConnectionProbe {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn worker(
    mut commands: mpsc::Receiver<QueuedCommand>,
    snapshot: Arc<Mutex<Snapshot>>,
    cancellation: Cancellation,
    shutdown: Arc<Shutdown>,
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
    let mut playback = crate::playback::Playback::new();
    let mut download_budget = None;
    let mut probe: Option<ConnectionProbe> = None;
    let mut capacity = crate::network::Capacity::default();
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    loop {
        let queued = tokio::select! {
            biased;
            _ = shutdown.wake.notified() => QueuedCommand { command: Command::Shutdown, capture: None, join: None },
            command = commands.recv() => command.unwrap_or(QueuedCommand { command: Command::Shutdown, capture: None, join: None }),
            _ = tick.tick() => {
                if probe.as_ref().is_some_and(|job|job.is_finished()) && let Some(job)=probe.take() {
                        let mut result=job.finish().await.unwrap_or_else(|_|crate::network::Capacity{error:"Connection measurement unavailable. Try again in Advanced.".into(),..Default::default()});
                        result.download_bps=result.download_bps.or(capacity.download_bps);
                        result.upload_bps=result.upload_bps.or(capacity.upload_bps);
                        capacity=result;
                        if download_budget.is_none(){playback.set_budget(capacity.watching_allowance());}
                        update(&snapshot,|state|{state.capacity=capacity.clone();state.testing_connection=false;});
                }
                if let Some(joined) = &session {
                    let mut participants: Vec<_> = joined.room.remote_participants().values().map(|p| {
                        let name = p.name(); if name.is_empty() { "Participant".to_owned() } else { name }
                    }).collect();
                    participants.sort_by_key(|name| name.to_lowercase());
                    update(&snapshot, |state| state.participants = participants);
                    if joined.room.connection_state() == ConnectionState::Connected {
                        match playback.reconcile(&joined.room, &joined.admission).await {
                            Ok(tiles) => update(&snapshot, |state| state.tiles = tiles),
                            Err(_) => update(&snapshot, |state| state.error = "Could not update native playback. Rejoin the room.".into()),
                        }
                    } else {
                        if joined.room.connection_state() == ConnectionState::Reconnecting { playback.suspend(); }
                        else { playback = crate::playback::Playback::new(); }
                        update(&snapshot, |state| state.tiles.clear());
                    }
                    if joined.room.connection_state() == ConnectionState::Connected
                        && stream.as_ref().is_some_and(|s| !s.cancelled()) {
                        match stream.as_ref().unwrap().compatible_config(&joined.room) {
                            Ok(Some(config)) => {
                                let replacement_stop = cancellation.register();
                                let active = stream.take().unwrap();
                                match active.fallback(&joined.room, joined.admission.clone(), replacement_stop, config).await {
                                    Ok(active) => stream = Some(active),
                                    Err(_) => update(&snapshot, |state| { state.sharing = false; state.self_preview = Arc::default(); state.mode = None; state.active_selection = None; state.error = "Could not switch codecs. Choose a supported mode and start again.".into(); }),
                                }
                            }
                            Ok(None) => {},
                            Err(error) => {
                                let _ = stream.take().unwrap().stop(&joined.room).await;
                                update(&snapshot, |state| {state.sharing=false; state.self_preview=Arc::default(); state.mode=None; state.active_selection=None; state.error=error.to_string();});
                            },
                        }
                    }
                }
                if let Some(active) = &stream {
                    active.rate.set_upload_cap(capacity.upload_video_cap());
                    update(&snapshot, |state| state.self_preview=active.previews.clone());
                    active.constrain_watchers(&session.as_ref().unwrap().room);
                    if let Ok(metrics) = active.metrics.lock() { update(&snapshot, |state| state.metrics = metrics.clone()); }
                    if let Ok(metrics) = active.audio_metrics.lock() { update(&snapshot, |state| state.audio = metrics.clone()); }
                    if let Ok(metrics) = active.delivery.lock() { update(&snapshot, |state| { state.delivery = metrics.clone(); state.mode=Some(active.mode); }); }
                    if !active.finished() { continue; }
                    if let (Some(active), Some(joined)) = (stream.take(), session.as_ref()) {
                        let result = active.stop(&joined.room).await;
                        update(&snapshot, |state| { state.sharing = false; state.self_preview = Arc::default(); state.active_selection = None; if let Err(error) = result { state.error = error.to_string(); } });
                    }
                }
                continue;
            }
        };
        let command = queued.command;
        if matches!(command, Command::RefreshSources) {
            sources = sources::enumerate();
            update(&snapshot, |state| {
                state.sources = sources.iter().map(|source| source.info.clone()).collect()
            });
            continue;
        }
        let shutdown = matches!(command, Command::Shutdown);
        let result: Result<()> = async {
            match command {
                Command::Join {
                    invitation,
                    name,
                    route,
                } => {
                    if let Some(job) = probe.take() {
                        job.abort();
                    }
                    capacity = crate::network::Capacity::default();
                    update(&snapshot, |state| {
                        state.capacity = capacity.clone();
                        state.testing_connection = false;
                    });
                    playback = crate::playback::Playback::new();
                    playback.set_budget(download_budget);
                    if let (Some(active), Some(joined)) = (stream.take(), session.as_ref()) {
                        let _ = active.stop(&joined.room).await;
                    }
                    if let Some(joined) = session.take() {
                        joined.close().await;
                    }
                    update(&snapshot, |state| {
                        state.connection = "connecting";
                        state.sharing = false;
                        state.self_preview = Arc::default();
                        state.room_name.clear();
                        state.participants.clear();
                    });
                    let joined = Session::join(
                        &invitation,
                        &name,
                        route,
                        snapshot.clone(),
                        cancellation.clone(),
                        playback.recovery_signal(),
                        queued.join.context("Room join was cancelled.")?,
                    )
                    .await?;
                    if let Some(joined) = joined {
                        let admission = joined.admission.clone();
                        probe = Some(ConnectionProbe(tokio::spawn(async move {
                            admission.measure_connection().await
                        })));
                        update(&snapshot, |state| {
                            state.connection = "connected";
                            state.room_name = joined.admission.room_name.clone();
                            state.testing_connection = true;
                        });
                        session = Some(joined);
                    } else {
                        update(&snapshot, |state| state.connection = "disconnected");
                    }
                }
                Command::Start {
                    source_id,
                    encoder,
                    audio: _requested_audio,
                    exclusions,
                    quality,
                }
                | Command::Apply {
                    source_id,
                    encoder,
                    audio: _requested_audio,
                    exclusions,
                    quality,
                } => {
                    let selection = SharingSelection {
                        source_id: source_id.clone(),
                        encoder: encoder.clone(),
                        quality,
                    };
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
                    let preferred_encoder = encoder.clone();
                    let audio = audio::for_source(&source)?;
                    let stop = queued.capture.context("Capture request was cancelled.")?;
                    anyhow::ensure!(
                        !stop.load(Ordering::Acquire),
                        "Capture request was cancelled."
                    );
                    let mut config = StreamConfig {
                        source,
                        encoder,
                        quality,
                        audio: audio.clone(),
                        exclusions,
                    };
                    config.negotiate(
                        &joined.room,
                        &preferred_encoder,
                        &crate::encoding::encoders(),
                    )?;
                    if let Some(active) = stream.as_mut()
                        && active.apply_live(&config)?
                    {
                        active.preferred_encoder = preferred_encoder;
                        update(&snapshot, |state| {
                            state.mode = Some(active.mode);
                            state.active_selection = Some(selection);
                        });
                        return Ok(());
                    }
                    config.mode()?;
                    if let Some(active) = stream.take() {
                        active.stop(&joined.room).await?;
                    }
                    joined
                        .expected_audio
                        .store(audio != audio::Choice::Off, Ordering::Release);
                    let mut active = Stream::start_with_upload_cap(
                        &joined.room,
                        config,
                        stop.clone(),
                        capacity.upload_video_cap(),
                    )
                    .await?;
                    active.preferred_encoder = preferred_encoder;
                    active.rate.set_upload_cap(capacity.upload_video_cap());
                    active.start_previews(joined.admission.clone());
                    if stop.load(Ordering::Acquire) {
                        let _ = active.stop(&joined.room).await;
                    } else {
                        let mode = active.mode;
                        let self_preview = active.previews.clone();
                        stream = Some(active);
                        update(&snapshot, |state| {
                            state.sharing = true;
                            state.self_preview = self_preview;
                            state.mode = Some(mode);
                            state.metrics = Metrics::default();
                            state.delivery = delivery::Metrics::default();
                            state.active_selection = Some(selection);
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
                    if let Some(job) = probe.take() {
                        job.abort();
                    }
                    capacity = crate::network::Capacity::default();
                    update(&snapshot, |state| {
                        state.capacity = capacity.clone();
                        state.testing_connection = false;
                    });
                    if let (Some(active), Some(joined)) = (stream.take(), session.as_ref()) {
                        let _ = active.stop(&joined.room).await;
                    }
                    if let Some(joined) = session.take() {
                        joined.close().await;
                    }
                    playback = crate::playback::Playback::new();
                    playback.set_budget(download_budget);
                    update(&snapshot, |state| {
                        state.connection = "disconnected";
                        state.sharing = false;
                        state.self_preview = Arc::default();
                        state.room_name.clear();
                        state.tiles.clear();
                        state.participants.clear();
                    });
                }
                Command::Refresh | Command::RefreshSources => {
                    sources = sources::enumerate();
                    update(&snapshot, |state| {
                        state.sources = sources.iter().map(|source| source.info.clone()).collect();
                        state.encoders = crate::encoding::encoders();
                        if let Ok(applications) = audio::applications() {
                            state.applications = applications;
                        }
                    });
                }
                Command::DownloadBudget { bps } => {
                    anyhow::ensure!(
                        bps.is_none_or(
                            |bps| (1_000_000..=crate::network::PROBE_CAP_BPS).contains(&bps)
                        ),
                        "Choose a watching budget from 1 to 200 Mbps."
                    );
                    download_budget = bps;
                    playback.set_budget(bps.or_else(|| capacity.watching_allowance()));
                }
                Command::MeasureConnection => {
                    let joined = session.as_ref().context("Join a room first.")?;
                    if let Some(job) = probe.take() {
                        job.abort();
                    }
                    let admission = joined.admission.clone();
                    probe = Some(ConnectionProbe(tokio::spawn(async move {
                        admission.measure_connection().await
                    })));
                    update(&snapshot, |state| state.testing_connection = true);
                }
                Command::Watch { id, enabled } => {
                    session.as_ref().context("Join a room first.")?;
                    playback.watch(id, enabled)?;
                }
            }
            Ok(())
        }
        .await;
        update(&snapshot, |state| {
            state.busy = false;
            state.operation = "";
            state.capture_pending = false;
            state.sharing = stream.is_some();
            if stream.is_none() {
                state.active_selection = None;
                state.self_preview = Arc::default();
            }
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

    #[tokio::test]
    async fn shutdown_bypasses_a_full_queue_and_rejects_new_work() {
        gstreamer::init().unwrap();
        let (sender, receiver) = mpsc::channel(8);
        let controller = Controller {
            commands: sender,
            snapshot: Arc::default(),
            cancellation: Arc::default(),
            join_cancellation: JoinCancellation::new(),
            shutdown: Arc::default(),
            worker: None,
        };
        for _ in 0..8 {
            controller.send(Command::DownloadBudget { bps: Some(0) });
        }
        assert_eq!(controller.commands.capacity(), 0);
        let capture = controller.cancellation.register();
        controller.begin_shutdown();
        controller.begin_shutdown();
        assert!(capture.load(Ordering::Acquire));
        assert!(!controller.shutdown_finished());
        controller.send(Command::DownloadBudget { bps: Some(0) });
        let completion = WorkerCompletion(controller.shutdown.clone());
        tokio::time::timeout(
            Duration::from_secs(10),
            worker(
                receiver,
                controller.snapshot.clone(),
                controller.cancellation.clone(),
                controller.shutdown.clone(),
            ),
        )
        .await
        .expect("Shutdown must bypass queued actions");
        drop(completion);
        assert!(controller.shutdown_finished());
        assert!(
            !controller
                .snapshot()
                .error
                .contains("Choose a watching budget")
        );
        assert!(!controller.snapshot().sharing);
        assert_eq!(controller.snapshot().connection, "disconnected");
    }

    #[test]
    fn source_catalog_refresh_is_quiet_and_has_no_capture_authority() {
        let (sender, mut receiver) = mpsc::channel(8);
        let snapshot = Arc::new(Mutex::new(Snapshot {
            busy: true,
            operation: "Applying stream settings…",
            error: "Preserve operation feedback".into(),
            capture_pending: true,
            sharing: true,
            ..Default::default()
        }));
        let controller = Controller {
            commands: sender,
            snapshot: snapshot.clone(),
            cancellation: Arc::default(),
            join_cancellation: JoinCancellation::new(),
            shutdown: Arc::default(),
            worker: None,
        };
        controller.send(Command::RefreshSources);
        let command = receiver.try_recv().expect("Quiet source update");
        assert!(matches!(command.command, Command::RefreshSources));
        assert!(command.capture.is_none());
        assert!(command.join.is_none());
        let state = snapshot.lock().unwrap();
        assert!(state.busy && state.sharing && state.capture_pending);
        assert_eq!(state.operation, "Applying stream settings…");
        assert_eq!(state.error, "Preserve operation feedback");
    }

    #[tokio::test]
    async fn cancelling_join_returns_promptly_and_releases_late_results() -> Result<()> {
        let cancellation = JoinCancellation::new();
        let ticket = cancellation.ticket();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, finish) = tokio::sync::oneshot::channel();
        let (cleaned, cleanup) = tokio::sync::oneshot::channel();
        let pending = tokio::spawn(async move {
            ticket
                .run(
                    async move {
                        let _ = started.send(());
                        finish.await.context("Release join fixture")?;
                        Ok(17_u64)
                    },
                    |value| async move {
                        let _ = cleaned.send(value);
                    },
                )
                .await
        });
        ready.await?;
        cancellation.cancel();
        let result = tokio::time::timeout(Duration::from_secs(1), pending).await?;
        assert!(result??.is_none());
        assert_eq!(
            cancellation
                .ticket()
                .run(async { Ok(31_u64) }, |_| async {})
                .await?,
            Some(31)
        );
        let _ = release.send(());
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), cleanup).await??,
            17
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_join_cancelled_before_processing_never_starts_network_work() -> Result<()> {
        let cancellation = JoinCancellation::new();
        let ticket = cancellation.ticket();
        cancellation.cancel();
        let ran = Arc::new(AtomicBool::new(false));
        let observed = ran.clone();
        assert!(
            ticket
                .run(
                    async move {
                        ran.store(true, Ordering::Release);
                        Ok(())
                    },
                    |_| async {}
                )
                .await?
                .is_none()
        );
        assert!(!observed.load(Ordering::Acquire));
        Ok(())
    }

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
        next.quality.bitrate = crate::quality::Bitrate::Manual(0);
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
