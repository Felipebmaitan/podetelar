use crate::{
    admission::Admission,
    audio,
    encoding::{Codec, Encoder, Encoding, Metrics, Mode},
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
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{sync::mpsc, task::JoinHandle};

const TRACK_NAME: &str = "FreedomStream screen";
const AUDIO_TRACK_NAME: &str = "FreedomStream application audio";
#[derive(Default)]
struct CaptureCancellation(Mutex<Vec<Weak<AtomicBool>>>);

impl CaptureCancellation {
    fn register(&self) -> Arc<AtomicBool> {
        let stop = Arc::new(AtomicBool::new(false));
        let mut captures = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        captures.retain(|capture| capture.strong_count() > 0);
        captures.push(Arc::downgrade(&stop));
        stop
    }

    fn cancel(&self) {
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
        let capture_pending = matches!(command, Command::Start { .. });
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

pub struct Stream {
    stop: StopOnDrop,
    task: JoinHandle<Result<Metrics>>,
    video: LocalVideoTrack,
    pub metrics: Arc<Mutex<Metrics>>,
    pub audio_metrics: Arc<Mutex<audio::Metrics>>,
    audio: Option<(JoinHandle<Result<audio::Metrics>>, LocalAudioTrack)>,
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
    pub async fn start(
        room: &Room,
        source: Source,
        encoder: Encoder,
        stop: Arc<AtomicBool>,
        audio_choice: audio::Choice,
        exclusions: Vec<String>,
    ) -> Result<Self> {
        let mut lease = StopOnDrop::new(stop.clone());
        let mode = Mode::baseline(&source)?;
        let cancelled = stop.clone();
        let encoding = tokio::task::spawn_blocking(move || {
            Encoding::start(&source, &encoder, mode, &cancelled)
        })
        .await
        .context("Capture worker stopped.")??;
        let codec = encoder_codec(&encoding);
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
                max_bitrate: mode.bitrate,
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
        let mut active = Self {
            stop: lease,
            task,
            video,
            metrics,
            audio_metrics: Arc::new(Mutex::new(audio::Metrics::default())),
            audio: None,
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

    pub fn finished(&self) -> bool {
        self.task.is_finished()
            || self
                .audio
                .as_ref()
                .is_some_and(|(task, _)| task.is_finished())
    }

    pub async fn stop(self, room: &Room) -> Result<Metrics> {
        self.stop.cancel();
        let captured = self
            .task
            .await
            .map_err(|_| anyhow::anyhow!("Capture worker stopped unexpectedly."))
            .and_then(|result| result);
        let mut audio_result = Ok(());
        if let Some((task, track)) = self.audio {
            audio_result = match task.await {
                Ok(result) => result.map(|_| ()),
                Err(_) => Err(anyhow::anyhow!("Audio worker stopped unexpectedly.")),
            };
            let _ = tokio::time::timeout(
                Duration::from_secs(5),
                room.local_participant().unpublish_track(&track.sid()),
            )
            .await;
        }
        // Release native capture before awaiting publication cleanup.
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            room.local_participant().unpublish_track(&self.video.sid()),
        )
        .await;
        let owned_ids: Vec<_> = self
            .stop
            .tracks
            .iter()
            .map(|track| track.rtc_track().id())
            .collect();
        let orphaned = room
            .local_participant()
            .track_publications()
            .values()
            .filter_map(|publication| publication.track())
            .any(|track| owned_ids.contains(&track.rtc_track().id()));
        if orphaned || room.connection_state() != ConnectionState::Connected {
            // A full reconnect can retain an SDK republish task. Closing the
            // media identity prevents it from publishing a stopped source later.
            let _ = room.close().await;
        }
        audio_result?;
        captured
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
                } => {
                    if stream.is_some() {
                        bail!("Stop the current stream first.");
                    }
                    let joined = session.as_ref().context("Join a room first.")?;
                    let source = sources
                        .iter()
                        .find(|source| source.info.id == source_id)
                        .context("Refresh and choose a capture source.")?
                        .clone();
                    let encoder = crate::encoding::encoders()
                        .into_iter()
                        .find(|candidate| candidate.factory == encoder)
                        .context("Choose an available hardware encoder.")?;
                    let stop = queued.capture.context("Capture request was cancelled.")?;
                    joined
                        .expected_audio
                        .store(audio != audio::Choice::Off, Ordering::Release);
                    let active = Stream::start(
                        &joined.room,
                        source,
                        encoder,
                        stop.clone(),
                        audio,
                        exclusions,
                    )
                    .await?;
                    if stop.load(Ordering::Acquire) {
                        let _ = active.stop(&joined.room).await;
                    } else {
                        stream = Some(active);
                        update(&snapshot, |state| state.sharing = true);
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
}
