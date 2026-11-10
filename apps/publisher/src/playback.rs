//! Native room subscriptions. No capture, browser, or application authority in UI state.
use anyhow::{Context, Result};
use eframe::egui;
use futures_util::StreamExt;
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSrc;
use livekit::{
    prelude::*,
    webrtc::{
        MediaType,
        audio_stream::native::{NativeAudioStream, NativeAudioStreamOptions},
        peer_connection_factory::PeerConnectionFactory,
        video_stream::native::{NativeVideoStream, NativeVideoStreamOptions},
    },
};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::task::JoinHandle;

pub const LIMIT_PREFIX: &str = "freedomstream.limit.";
pub const WATCH_PREFIX: &str = "freedomstream.watch.";
pub type Picture = Arc<Mutex<Option<Arc<egui::ColorImage>>>>;

#[derive(Default)]
pub struct Presentation {
    requested: Mutex<Option<Instant>>,
    pub converted: AtomicU64,
    pub uploaded: AtomicU64,
    #[cfg(feature = "performance-experiments")]
    repaint: Mutex<Option<egui::Context>>,
}
impl Presentation {
    pub fn request(&self) {
        if let Ok(mut requested) = self.requested.lock() {
            *requested = Some(Instant::now());
        }
    }
    pub fn request_context(&self, ctx: &egui::Context) {
        self.request();
        #[cfg(feature = "performance-experiments")]
        if frame_driven_repaint()
            && let Ok(mut repaint) = self.repaint.lock()
        {
            *repaint = Some(ctx.clone());
        }
        #[cfg(not(feature = "performance-experiments"))]
        let _ = ctx;
    }
    fn active(&self) -> bool {
        self.requested.lock().is_ok_and(|requested| {
            requested.is_some_and(|time| time.elapsed() < Duration::from_millis(250))
        })
    }
    fn publish(&self, picture: &Picture, image: Arc<egui::ColorImage>) {
        self.converted.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut latest) = picture.lock() {
            *latest = Some(image);
        }
        #[cfg(feature = "performance-experiments")]
        if frame_driven_repaint()
            && let Ok(repaint) = self.repaint.lock()
            && let Some(ctx) = repaint.as_ref()
        {
            let elapsed = self
                .requested
                .lock()
                .ok()
                .and_then(|request| request.map(|time| time.elapsed()))
                .unwrap_or_default();
            ctx.request_repaint_after(Duration::from_millis(16).saturating_sub(elapsed));
        }
    }
}

#[cfg(feature = "performance-experiments")]
pub fn count_sdk_logs() -> Result<()> {
    struct Counter;
    impl log::Log for Counter {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool {
            true
        }
        fn log(&self, record: &log::Record<'_>) {
            if record.target() == "libwebrtc" {
                SDK_MESSAGES.fetch_add(1, Ordering::Relaxed);
            }
        }
        fn flush(&self) {}
    }
    static COUNTER: Counter = Counter;
    log::set_logger(&COUNTER)
        .map_err(|_| anyhow::anyhow!("Cannot enable local message counter."))?;
    log::set_max_level(log::LevelFilter::Debug);
    Ok(())
}
#[cfg(feature = "performance-experiments")]
pub static SDK_MESSAGES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "performance-experiments")]
pub static SUBSCRIPTION_SETTERS: AtomicU64 = AtomicU64::new(0);

fn apply_subscription(publication: &RemoteTrackPublication, enabled: bool) {
    if publication.is_desired() != enabled {
        #[cfg(feature = "performance-experiments")]
        SUBSCRIPTION_SETTERS.fetch_add(1, Ordering::Relaxed);
        publication.set_subscribed(enabled);
    }
}

#[cfg(feature = "performance-experiments")]
pub fn frame_driven_repaint() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("FREEDOMSTREAM_FRAME_REPAINT").is_ok_and(|value| value == "1")
    })
}
#[cfg(feature = "performance-experiments")]
fn conversion_jobs() -> Option<&'static Arc<tokio::sync::Semaphore>> {
    static JOBS: std::sync::OnceLock<Option<Arc<tokio::sync::Semaphore>>> =
        std::sync::OnceLock::new();
    JOBS.get_or_init(|| {
        std::env::var("FREEDOMSTREAM_CONVERSION_WORKERS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|workers| (1..=4).contains(workers))
            .map(|workers| Arc::new(tokio::sync::Semaphore::new(workers)))
    })
    .as_ref()
}

#[derive(Clone)]
pub struct Tile {
    pub id: String,
    pub name: String,
    pub codec: String,
    pub watching: bool,
    pub picture: Picture,
    pub presentation: Arc<Presentation>,
    pub frames: Arc<AtomicU64>,
    pub received_bps: Arc<AtomicU64>,
    pub volume: Arc<AtomicU32>,
    pub audio: Arc<Mutex<crate::audio::Metrics>>,
    pub status: String,
}

struct Watch {
    picture: Picture,
    presentation: Arc<Presentation>,
    frames: Arc<AtomicU64>,
    received_bps: Arc<AtomicU64>,
    volume: Arc<AtomicU32>,
    since: Instant,
    last_frame: Arc<Mutex<Instant>>,
    audio: Arc<Mutex<crate::audio::Metrics>>,
    error: Arc<Mutex<String>>,
    h264: bool,
    video_sid: String,
}

impl Watch {
    fn begin_video(&mut self) -> (Picture, Arc<Presentation>) {
        // Cancellation cannot interrupt a synchronous conversion already in
        // progress. A replacement pump owns a new mailbox so a late old result
        // cannot overwrite the current source/reconnect generation.
        self.picture = Arc::default();
        self.presentation = Arc::default();
        (self.picture.clone(), self.presentation.clone())
    }
}

struct Pump(JoinHandle<()>);
impl Drop for Pump {
    fn drop(&mut self) {
        self.0.abort();
    }
}

struct PreviewImage {
    revision: String,
    picture: Picture,
    job: Pump,
    loaded: Arc<AtomicBool>,
    attempted: Instant,
}

#[derive(Clone)]
struct ReceivedCounters {
    bytes: u64,
    sampled: Instant,
    packets: u64,
    lost: i64,
    delay: f64,
    emitted: u64,
}

fn received_rate(previous: &ReceivedCounters, current: &ReceivedCounters) -> Option<u64> {
    let bytes = current.bytes.checked_sub(previous.bytes)?;
    let elapsed = current
        .sampled
        .checked_duration_since(previous.sampled)?
        .as_secs_f64();
    (elapsed > 0.0).then(|| (bytes as f64 * 8.0 / elapsed).round() as u64)
}

pub struct Playback {
    watches: HashMap<String, Watch>,
    pumps: HashMap<String, Pump>,
    announced: HashMap<String, String>,
    announce_dirty: bool,
    previews: HashMap<String, PreviewImage>,
    hevc: bool,
    av1: bool,
    h264: bool,
    measure_audio: bool,
    budget: crate::network::ReceiveBudget,
    budget_setting: Option<u64>,
    sampled: Instant,
    received: HashMap<String, ReceivedCounters>,
    recovery: Arc<AtomicU64>,
    observed_recovery: u64,
}

pub fn decoder_codecs() -> Vec<String> {
    PeerConnectionFactory::default()
        .get_rtp_receiver_capabilities(MediaType::Video)
        .codecs
        .into_iter()
        .map(|codec| codec.mime_type.to_lowercase())
        .collect()
}

impl Playback {
    pub fn new() -> Self {
        let codecs = decoder_codecs();
        Self {
            watches: HashMap::new(),
            pumps: HashMap::new(),
            announced: HashMap::new(),
            announce_dirty: false,
            previews: HashMap::new(),
            measure_audio: false,
            budget: crate::network::ReceiveBudget::default(),
            budget_setting: None,
            sampled: Instant::now(),
            received: HashMap::new(),
            recovery: Arc::default(),
            observed_recovery: 0,
            av1: codecs.iter().any(|c| c == "video/av1"),
            hevc: codecs.iter().any(|c| c == "video/h265"),
            h264: codecs.iter().any(|c| c == "video/h264"),
        }
    }

    pub fn suspend(&mut self) {
        self.pumps.clear();
        self.previews.clear();
        self.received.clear();
        self.announce_dirty = true;
        for watch in self.watches.values_mut() {
            watch.since = Instant::now();
            if let Ok(mut last) = watch.last_frame.lock() {
                *last = Instant::now();
            }
            watch.received_bps.store(0, Ordering::Relaxed);
        }
    }

    pub fn recovery_signal(&self) -> Arc<AtomicU64> {
        self.recovery.clone()
    }

    pub fn set_budget(&mut self, bps: Option<u64>) {
        self.budget_setting = bps;
        self.budget.set(bps);
    }

    pub fn measure_audio(&mut self) {
        self.measure_audio = true;
    }

    pub fn compatibility_only(&mut self, enabled: bool) {
        if enabled {
            self.hevc = false;
            self.av1 = false;
        }
    }

    pub fn watch(&mut self, id: String, enabled: bool) -> Result<()> {
        if enabled {
            if self.watches.is_empty() {
                self.budget.set(self.budget_setting);
            }
            anyhow::ensure!(
                self.h264 || self.hevc || self.av1,
                "No compatible native video decoder is available on this computer."
            );
            self.watches.entry(id).or_insert_with(|| Watch {
                picture: Arc::default(),
                presentation: Arc::default(),
                frames: Arc::default(),
                received_bps: Arc::default(),
                volume: Arc::new(AtomicU32::new(100)),
                since: Instant::now(),
                last_frame: Arc::new(Mutex::new(Instant::now())),
                audio: Arc::default(),
                error: Arc::default(),
                h264: !self.hevc && !self.av1,
                video_sid: String::new(),
            });
        } else {
            if self.watches.remove(&id).is_some() {
                // Restore the existing still without requesting another capture.
                self.previews.remove(&id);
            }
        }
        Ok(())
    }

    async fn sample_budget(&mut self, room: &Room) {
        if self.sampled.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.sampled = Instant::now();
        let mut jobs = Vec::new();
        for participant in room.remote_participants().values() {
            if !self.watches.contains_key(participant.identity().as_str()) {
                continue;
            }
            for publication in participant.track_publications().values() {
                if let Some(RemoteTrack::Video(track)) = publication.track() {
                    let sid = publication.sid().to_string();
                    let owner = participant.identity().to_string();
                    jobs.push(async move {
                        (
                            owner,
                            sid,
                            tokio::time::timeout(Duration::from_millis(250), track.get_stats())
                                .await,
                        )
                    });
                }
            }
        }
        let mut packets = 0_u64;
        let mut lost = 0_u64;
        let mut delay = 0.0_f64;
        let mut emitted = 0_u64;
        let mut current = HashSet::new();
        let mut rates = HashMap::<String, u64>::new();
        for (owner, sid, result) in futures_util::future::join_all(jobs).await {
            let Ok(Ok(stats)) = result else {
                continue;
            };
            for stat in stats {
                if let livekit::webrtc::stats::RtcStats::InboundRtp(stat) = stat {
                    let key = format!("{sid}/{}", stat.rtc.id);
                    current.insert(key.clone());
                    let sample = ReceivedCounters {
                        bytes: stat.inbound.bytes_received,
                        sampled: Instant::now(),
                        packets: stat.received.packets_received,
                        lost: stat.received.packets_lost,
                        delay: stat.inbound.jitter_buffer_delay,
                        emitted: stat.inbound.jitter_buffer_emitted_count,
                    };
                    if let Some(previous) = self.received.insert(key, sample.clone())
                        && sample.packets >= previous.packets
                        && sample.lost >= previous.lost
                        && sample.delay >= previous.delay
                        && sample.emitted >= previous.emitted
                    {
                        if let Some(rate) = received_rate(&previous, &sample) {
                            let total = rates.entry(owner.clone()).or_default();
                            *total = total.saturating_add(rate);
                        }
                        packets += sample.packets - previous.packets;
                        lost += (sample.lost - previous.lost) as u64;
                        delay += sample.delay - previous.delay;
                        emitted += sample.emitted - previous.emitted;
                    }
                }
            }
        }
        self.received.retain(|sid, _| current.contains(sid));
        for (owner, watch) in &self.watches {
            watch
                .received_bps
                .store(rates.get(owner).copied().unwrap_or(0), Ordering::Relaxed);
        }
        self.budget.observe(
            (packets + lost > 0).then(|| lost as f64 / (packets + lost) as f64),
            (emitted > 0).then(|| delay / emitted as f64),
        );
    }

    pub async fn reconcile(
        &mut self,
        room: &Room,
        admission: &crate::admission::Admission,
    ) -> Result<Vec<Tile>> {
        let recovery = self.recovery.load(Ordering::Acquire);
        if recovery != self.observed_recovery {
            self.suspend();
            self.observed_recovery = recovery;
        }
        if room.connection_state() != ConnectionState::Connected {
            self.suspend();
            return Ok(Vec::new());
        }
        self.sample_budget(room).await;
        let participants = room.remote_participants();
        let mut selected: Vec<_> = participants
            .values()
            .filter(|p| {
                self.watches.contains_key(p.identity().as_str())
                    && p.track_publications()
                        .values()
                        .any(|v| v.source() == TrackSource::Screenshare)
            })
            .map(|p| p.identity().to_string())
            .collect();
        selected.sort();
        selected.truncate(self.budget.max_streams());
        let allowed: HashSet<_> = selected.into_iter().collect();
        let cap = self.budget.per_stream(allowed.len());
        let mut live = HashSet::new();
        let mut tiles = Vec::new();
        let mut pumps = HashSet::new();
        let mut announce = HashMap::new();
        for participant in participants.values() {
            let id = participant.identity().to_string();
            let publications = participant.track_publications();
            let Some(video) = publications
                .values()
                .find(|p| p.source() == TrackSource::Screenshare)
            else {
                for publication in publications
                    .values()
                    .filter(|p| p.source() == TrackSource::ScreenshareAudio)
                {
                    apply_subscription(publication, false);
                }
                // Retain watch intent while a codec/source replacement is in flight.
                if self.watches.contains_key(&id) {
                    live.insert(id.clone());
                }
                continue;
            };
            live.insert(id.clone());
            let hevc = video.mime_type().eq_ignore_ascii_case("video/h265");
            let av1 = video.mime_type().eq_ignore_ascii_case("video/av1");
            if let Some(watch) = self.watches.get_mut(&id) {
                if watch.video_sid != video.sid().as_str() {
                    watch.video_sid = video.sid().to_string();
                    watch.since = Instant::now();
                    if let Ok(mut last) = watch.last_frame.lock() {
                        *last = Instant::now();
                    }
                }
                if (hevc || av1)
                    && watch
                        .last_frame
                        .lock()
                        .is_ok_and(|last| last.elapsed() > Duration::from_secs(6))
                {
                    watch.h264 = true;
                }
                if allowed.contains(&id) {
                    announce.insert(
                        format!("{WATCH_PREFIX}{id}"),
                        if watch.h264 || (av1 && !self.av1) || (hevc && !self.hevc) {
                            "h264"
                        } else if av1 {
                            "av1"
                        } else {
                            "hevc"
                        }
                        .to_owned(),
                    );
                    announce.insert(format!("{LIMIT_PREFIX}{id}"), cap.to_string());
                }
            }
            if !self.watches.contains_key(&id) {
                let descriptor = participant
                    .attributes()
                    .get("freedomstream.preview.screen")
                    .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok());
                if let Some(descriptor) = descriptor
                    && descriptor.get("trackSid").and_then(|v| v.as_str())
                        == Some(video.sid().as_str())
                    && let Some(revision) = descriptor.get("revision").and_then(|v| v.as_str())
                    && self.previews.get(&id).is_none_or(|old| {
                        old.revision != revision
                            || (old.job.0.is_finished()
                                && !old.loaded.load(Ordering::Acquire)
                                && old.attempted.elapsed() >= Duration::from_secs(2))
                    })
                {
                    let picture: Picture = Arc::default();
                    let latest = picture.clone();
                    let loaded = Arc::new(AtomicBool::new(false));
                    let complete = loaded.clone();
                    let admission = admission.clone();
                    let owner = id.clone();
                    let revision = revision.to_owned();
                    let requested = revision.clone();
                    let job = tokio::spawn(async move {
                        if let Ok(image) = admission.get_preview(&owner, &requested).await
                            && let Ok(mut slot) = latest.lock()
                        {
                            *slot = Some(Arc::new(image));
                            complete.store(true, Ordering::Release);
                        }
                    });
                    self.previews.insert(
                        id.clone(),
                        PreviewImage {
                            revision,
                            picture,
                            job: Pump(job),
                            loaded,
                            attempted: Instant::now(),
                        },
                    );
                }
            }
            let watching = self.watches.contains_key(&id);
            let compatible = self.watches.get(&id).is_some_and(|w| {
                (!hevc && !av1) || (!w.h264 && (!av1 || self.av1) && (!hevc || self.hevc))
            });
            let admitted = allowed.contains(&id);
            for publication in publications.values().filter(|p| {
                matches!(
                    p.source(),
                    TrackSource::Screenshare | TrackSource::ScreenshareAudio
                )
            }) {
                let enabled = watching && compatible && admitted;
                apply_subscription(publication, enabled);
                let sid = publication.sid().to_string();
                if !enabled {
                    continue;
                }
                pumps.insert(sid.clone());
                if self.pumps.contains_key(&sid) {
                    continue;
                }
                let Some(track) = publication.track() else {
                    continue;
                };
                let watch = self
                    .watches
                    .get_mut(&id)
                    .context("Missing playback request.")?;
                let task = match track {
                    RemoteTrack::Video(track) => {
                        let (picture, presentation) = watch.begin_video();
                        let frames = watch.frames.clone();
                        let last_frame = watch.last_frame.clone();
                        tokio::spawn(async move {
                            let mut image_pool = crate::video_image::ImagePool::default();
                            #[cfg(feature = "performance-experiments")]
                            let pooled = std::env::var("FREEDOMSTREAM_IMAGE_POOL")
                                .is_ok_and(|value| value == "1");
                            #[cfg(not(feature = "performance-experiments"))]
                            let pooled = true;
                            #[cfg(feature = "performance-experiments")]
                            let fallback = std::env::var("FREEDOMSTREAM_IMAGE_FALLBACK")
                                .is_ok_and(|value| value == "1");
                            #[cfg(not(feature = "performance-experiments"))]
                            let fallback = true;
                            let mut stream = NativeVideoStream::with_options(
                                track.rtc_track(),
                                NativeVideoStreamOptions {
                                    queue_size_frames: Some(1),
                                },
                            );
                            while let Some(frame) = stream.next().await {
                                // Bound each poll to one frame even when the
                                // latest-frame queue stays ready under load.
                                // Abort/control/audio work must get a turn.
                                tokio::task::yield_now().await;
                                let width = frame.buffer.width();
                                let height = frame.buffer.height();
                                if width > 3840
                                    || height > 3840
                                    || width as u64 * height as u64 > 3840 * 2160
                                {
                                    break;
                                }
                                frames.fetch_add(1, Ordering::Relaxed);
                                if let Ok(mut last) = last_frame.lock() {
                                    *last = Instant::now();
                                }
                                if !presentation.active() {
                                    if let Ok(mut latest) = picture.lock() {
                                        *latest = None;
                                    }
                                    image_pool.clear();
                                    continue;
                                }
                                if pooled {
                                    let mut image = image_pool.convert(frame.buffer.as_ref());
                                    if image.is_none() {
                                        // Release an overwritten mailbox image before retrying;
                                        // an egui-owned image still prevents exclusive mutation.
                                        if let Ok(mut latest) = picture.lock() {
                                            *latest = None;
                                        }
                                        image = image_pool.convert(frame.buffer.as_ref());
                                    }
                                    let image = image.or_else(|| {
                                        if fallback {
                                            image_pool.allocate_fallback(frame.buffer.as_ref())
                                        } else {
                                            None
                                        }
                                    });
                                    if let Some(image) = image {
                                        presentation.publish(&picture, image);
                                    } else if let Ok(mut latest) = picture.lock() {
                                        *latest = None;
                                    }
                                    continue;
                                }
                                #[cfg(feature = "performance-experiments")]
                                let image = if let Some(jobs) = conversion_jobs() {
                                    let Ok(permit) = jobs.clone().acquire_owned().await else {
                                        break;
                                    };
                                    let Ok(image) = tokio::task::spawn_blocking(move || {
                                        let _permit = permit;
                                        crate::video_image::to_color_image(frame.buffer.as_ref())
                                    })
                                    .await
                                    else {
                                        break;
                                    };
                                    image
                                } else {
                                    crate::video_image::to_color_image(frame.buffer.as_ref())
                                };
                                #[cfg(not(feature = "performance-experiments"))]
                                let image =
                                    crate::video_image::to_color_image(frame.buffer.as_ref());
                                presentation.publish(&picture, Arc::new(image));
                            }
                        })
                    }
                    RemoteTrack::Audio(track) => {
                        let volume = watch.volume.clone();
                        let metrics = watch.audio.clone();
                        let error = watch.error.clone();
                        let measure = self.measure_audio;
                        tokio::spawn(async move {
                            if audio(track, volume, metrics, measure).await.is_err()
                                && let Ok(mut message) = error.lock()
                            {
                                *message = "Native audio playback failed. Check the output device and rejoin.".into();
                            }
                        })
                    }
                };
                self.pumps.insert(sid, Pump(task));
            }
            let watch = self.watches.get(&id);
            tiles.push(Tile {
                id: id.clone(),
                name: participant.name(),
                codec: video.mime_type(),
                watching,
                picture: watch
                    .map(|w| w.picture.clone())
                    .or_else(|| self.previews.get(&id).map(|preview| preview.picture.clone()))
                    .unwrap_or_default(),
                presentation: watch.map(|w| w.presentation.clone()).unwrap_or_default(),
                frames: watch.map(|w| w.frames.clone()).unwrap_or_default(),
                received_bps: watch.map(|w|w.received_bps.clone()).unwrap_or_default(),
                audio: watch.map(|w| w.audio.clone()).unwrap_or_default(),
                volume: watch
                    .map(|w| w.volume.clone())
                    .unwrap_or_else(|| Arc::new(AtomicU32::new(100))),
                status: if watching && !admitted {
                    "Paused by your watching budget. Stop another stream or adjust the connection limit.".into()
                } else if watching && !compatible {
                    "Switching to H.264 compatibility…".into()
                } else if let Some(watch) = watch {
                    let error = watch.error.lock().map(|e| e.clone()).unwrap_or_default();
                    if !error.is_empty() { error } else if watch.last_frame.lock().is_ok_and(|last| last.elapsed() > Duration::from_secs(12)) {
                        "No video frames arrived. Check your connection or choose a lower frame rate.".into()
                    } else { String::new() }
                } else { String::new() },
            });
        }
        self.previews.retain(|id, _| live.contains(id));
        self.watches.retain(|id, _| live.contains(id));
        self.pumps
            .retain(|sid, pump| pumps.contains(sid) && !pump.0.is_finished());
        if announce != self.announced || self.announce_dirty {
            let mut changes = announce.clone();
            for key in self.announced.keys() {
                if !announce.contains_key(key) {
                    changes.insert(key.clone(), String::new());
                }
            }
            tokio::time::timeout(
                Duration::from_secs(3),
                room.local_participant().set_attributes(changes),
            )
            .await
            .context("Watch request timed out.")?
            .map_err(|_| {
                anyhow::anyhow!("Could not update watching preferences. Rejoin the room.")
            })?;
            self.announced = announce;
            self.announce_dirty = false;
        }
        tiles.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(tiles)
    }
}

pub(crate) struct AudioPipeline(gst::Pipeline);
impl Drop for AudioPipeline {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}

async fn audio(
    track: RemoteAudioTrack,
    volume: Arc<AtomicU32>,
    metrics: Arc<Mutex<crate::audio::Metrics>>,
    measure: bool,
) -> Result<()> {
    let pipeline = gst::parse::launch("appsrc name=pcm is-live=true format=time do-timestamp=true block=false max-bytes=38400 ! audio/x-raw,format=S16LE,rate=48000,channels=2,layout=interleaved ! queue name=backlog max-size-time=100000000 max-size-bytes=38400 max-size-buffers=10 leaky=downstream ! audioconvert ! audioresample ! volume name=level ! wasapi2sink low-latency=true sync=false")?
        .downcast::<gst::Pipeline>().map_err(|_| anyhow::anyhow!("Invalid native playback pipeline."))?;
    let pipeline = AudioPipeline(pipeline);
    let input = pipeline
        .0
        .by_name("pcm")
        .context("Missing audio playback input.")?
        .downcast::<AppSrc>()
        .map_err(|_| anyhow::anyhow!("Invalid playback input."))?;
    let level = pipeline
        .0
        .by_name("level")
        .context("Missing playback volume.")?;
    let backlog = pipeline
        .0
        .by_name("backlog")
        .context("Missing playback queue.")?;
    pipeline.0.set_state(gst::State::Playing)?;
    let bus = pipeline.0.bus();
    let mut applied_volume = None;
    let mut stream = NativeAudioStream::with_options(
        track.rtc_track(),
        48000,
        2,
        NativeAudioStreamOptions {
            queue_size_frames: Some(5),
        },
    );
    let mut meter = crate::audio::Meter::default();
    while let Some(frame) = stream.next().await {
        if let Some(message) = bus
            .as_ref()
            .and_then(|bus| bus.pop_filtered(&[gst::MessageType::Error]))
        {
            let _ = message;
            anyhow::bail!("Audio output failed.");
        }
        if let Ok(mut metrics) = metrics.lock() {
            let queue_ns: u64 = backlog.property("current-level-time");
            metrics.playback_queue_ms =
                input.current_level_bytes() as f64 / 192.0 + queue_ns as f64 / 1_000_000.0;
            metrics.playback_queue_max_ms =
                metrics.playback_queue_max_ms.max(metrics.playback_queue_ms);
            if input.current_level_bytes() > 38400 {
                metrics.playback_dropped_frames += 1;
            }
            metrics.frames += frame.samples_per_channel as u64;
            for channels in frame.data.chunks_exact(2) {
                metrics.peak = metrics
                    .peak
                    .max(channels[0].unsigned_abs() as f64 / 32768.0);
                metrics.channel_difference_peak = metrics
                    .channel_difference_peak
                    .max((channels[0] as f64 - channels[1] as f64).abs() / 32768.0);
            }
            if measure {
                meter.push(&frame.data, &mut metrics.tones);
            }
        }
        let requested_volume = volume.load(Ordering::Relaxed);
        if applied_volume != Some(requested_volume) {
            level.set_property("volume", requested_volume as f64 / 100.0);
            applied_volume = Some(requested_volume);
        }
        if input.current_level_bytes() > 38400 {
            continue;
        }
        input.push_buffer(audio_buffer(frame.data.into_owned()))?;
    }
    Ok(())
}

pub(crate) fn audio_buffer(samples: Vec<i16>) -> gst::Buffer {
    #[cfg(target_endian = "little")]
    {
        struct Samples(Vec<i16>);
        impl AsMut<[u8]> for Samples {
            fn as_mut(&mut self) -> &mut [u8] {
                bytemuck::cast_slice_mut(&mut self.0)
            }
        }
        gst::Buffer::from_mut_slice(Samples(samples))
    }
    #[cfg(not(target_endian = "little"))]
    gst::Buffer::from_mut_slice(
        samples
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect::<Vec<_>>(),
    )
}

/// Only a participant actively requesting this publication can trigger fallback.
pub fn needs_h264(room: &Room) -> bool {
    let key = format!("{WATCH_PREFIX}{}", room.local_participant().identity());
    room.remote_participants()
        .values()
        .any(|p| p.attributes().get(&key).is_some_and(|v| v == "h264"))
}

/// A bounded, quiet local fixture exercises exclusion of our native playback process.
pub fn qualification_tone() -> Result<AudioPipeline> {
    let pipeline = gst::parse::launch("audiotestsrc is-live=true freq=1320 volume=0.02 samplesperbuffer=480 ! audio/x-raw,rate=48000,channels=2 ! wasapi2sink low-latency=true")?
        .downcast::<gst::Pipeline>().map_err(|_| anyhow::anyhow!("Invalid playback fixture."))?;
    let pipeline = AudioPipeline(pipeline);
    pipeline.0.set_state(gst::State::Playing).map_err(|_| {
        let kind = pipeline.0.bus().and_then(|bus| bus.pop_filtered(&[gst::MessageType::Error])).and_then(|message| {
            if let gst::MessageView::Error(error) = message.view() { error.error().kind::<gst::ResourceError>() } else { None }
        });
        anyhow::anyhow!("Native audio output could not start ({kind:?}). Check the default Windows playback device.")
    })?;
    Ok(pipeline)
}

/// A limit applies only while the same participant also requests this publisher.
/// The minimum watcher allowance is shared by the single encoder, not per viewer.
pub fn requested_cap(room: &Room) -> Option<u64> {
    let identity = room.local_participant().identity();
    let watch = format!("{WATCH_PREFIX}{identity}");
    let limit = format!("{LIMIT_PREFIX}{identity}");
    room.remote_participants()
        .values()
        .filter_map(|p| {
            let attributes = p.attributes();
            if !attributes
                .get(&watch)
                .is_some_and(|v| matches!(v.as_str(), "hevc" | "h264" | "av1"))
            {
                return None;
            }
            attributes
                .get(&limit)?
                .parse::<u64>()
                .ok()
                .filter(|bps| (64_000..=100_000_000).contains(bps))
        })
        .min()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_video_keeps_late_old_results_out_of_the_current_mailbox() {
        let mut watch = Watch {
            picture: Arc::default(),
            presentation: Arc::default(),
            frames: Arc::new(AtomicU64::new(7)),
            received_bps: Arc::default(),
            volume: Arc::new(AtomicU32::new(80)),
            since: Instant::now(),
            last_frame: Arc::new(Mutex::new(Instant::now())),
            audio: Arc::default(),
            error: Arc::default(),
            h264: true,
            video_sid: "synthetic-track".into(),
        };
        let (old_picture, old_presentation) = watch.begin_video();
        let (current_picture, current_presentation) = watch.begin_video();
        current_presentation.publish(
            &current_picture,
            Arc::new(egui::ColorImage::filled([2, 2], egui::Color32::RED)),
        );
        old_presentation.publish(
            &old_picture,
            Arc::new(egui::ColorImage::filled([2, 2], egui::Color32::BLACK)),
        );
        assert!(Arc::ptr_eq(&watch.picture, &current_picture));
        assert_eq!(
            watch.picture.lock().unwrap().as_ref().unwrap().pixels[0],
            egui::Color32::RED
        );
        assert_eq!(
            old_picture.lock().unwrap().as_ref().unwrap().pixels[0],
            egui::Color32::BLACK
        );
        assert_eq!(watch.frames.load(Ordering::Relaxed), 7);
        assert_eq!(watch.volume.load(Ordering::Relaxed), 80);
        assert!(watch.h264);
    }
    #[test]
    fn received_bitrate_uses_actual_elapsed_time_and_handles_counter_reset() {
        let now = Instant::now();
        let first = ReceivedCounters {
            bytes: 2_000_000,
            sampled: now,
            packets: 1,
            lost: 0,
            delay: 0.0,
            emitted: 1,
        };
        let mut next = first.clone();
        next.bytes = 4_000_000;
        next.sampled = now + Duration::from_secs(2);
        assert_eq!(received_rate(&first, &next), Some(8_000_000));
        next.bytes = 0;
        assert_eq!(received_rate(&first, &next), None);
        next.bytes = first.bytes;
        assert_eq!(received_rate(&first, &next), Some(0));
    }
}
