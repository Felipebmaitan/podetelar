//! Local, one-shot source confirmation. No admission, encoder, audio or upload path.
use crate::sources::{self, Source, SourceInfo, Target};
use anyhow::{Context, Result, ensure};
use eframe::egui::ColorImage;
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSink;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

const SETTLE: Duration = Duration::from_millis(180);
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(2);
const RETRY_DELAY: Duration = Duration::from_millis(300);
const CAPTURE_ATTEMPTS: usize = 2;

pub struct Frame {
    pub source_id: String,
    pub image: Option<ColorImage>,
}

#[derive(Default)]
struct State {
    current: Option<String>,
    pending: Option<(u64, SourceInfo)>,
    result: Option<Frame>,
    closed: bool,
}

pub struct Preview {
    shared: Arc<(Mutex<State>, Condvar)>,
    generation: Arc<AtomicU64>,
}

impl Preview {
    pub fn new() -> Self {
        Self::with_capture(|info, cancelled| {
            let source = sources::enumerate()
                .into_iter()
                .find(|source| source.info.id == info.id)?;
            capture(&source, cancelled).ok()
        })
    }

    fn with_capture(
        capture: impl Fn(&SourceInfo, &dyn Fn() -> bool) -> Option<ColorImage> + Send + 'static,
    ) -> Self {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let generation = Arc::new(AtomicU64::new(0));
        let state = shared.clone();
        let epoch = generation.clone();
        // One worker and a single overwriteable request slot bound work and memory.
        let worker = std::thread::Builder::new()
            .name("freedomstream-source-preview".into())
            .spawn(move || {
                loop {
                    let (lock, ready) = &*state;
                    let mut guard = lock.lock().unwrap_or_else(|p| p.into_inner());
                    while guard.pending.is_none() && !guard.closed {
                        guard = ready.wait(guard).unwrap_or_else(|p| p.into_inner());
                    }
                    if guard.closed {
                        break;
                    }
                    let (requested, info) = guard.pending.take().expect("Pending request");
                    // Wait until hover/focus settles; rapid list traversal never queues captures.
                    let (guard, _) = ready
                        .wait_timeout_while(guard, SETTLE, |state| {
                            !state.closed && epoch.load(Ordering::Acquire) == requested
                        })
                        .unwrap_or_else(|p| p.into_inner());
                    if guard.closed {
                        break;
                    }
                    if epoch.load(Ordering::Acquire) != requested {
                        continue;
                    }
                    drop(guard);
                    let cancelled = || epoch.load(Ordering::Acquire) != requested;
                    let mut image = None;
                    for attempt in 0..CAPTURE_ATTEMPTS {
                        if cancelled() {
                            break;
                        }
                        image = capture(&info, &cancelled);
                        if image.is_some() || cancelled() || attempt + 1 == CAPTURE_ATTEMPTS {
                            break;
                        }
                        // A quick source switch can leave a capture backend briefly busy.
                        // Retry the settled source once; a new hover or dismissal wakes us.
                        let guard = lock.lock().unwrap_or_else(|p| p.into_inner());
                        let (guard, _) = ready
                            .wait_timeout_while(guard, RETRY_DELAY, |state| {
                                !state.closed && !cancelled()
                            })
                            .unwrap_or_else(|p| p.into_inner());
                        if guard.closed || cancelled() {
                            break;
                        }
                    }
                    let mut guard = lock.lock().unwrap_or_else(|p| p.into_inner());
                    if !guard.closed && !cancelled() {
                        guard.result = Some(Frame {
                            source_id: info.id,
                            image,
                        });
                    }
                }
            });
        if worker.is_err() {
            shared.0.lock().unwrap_or_else(|p| p.into_inner()).closed = true;
        }
        // Dropping the handle detaches; Drop cancels without blocking the UI on a driver.
        Self { shared, generation }
    }

    pub fn request(&self, source: &SourceInfo) {
        let mut state = self.shared.0.lock().unwrap_or_else(|p| p.into_inner());
        if state.current.as_deref() == Some(&source.id) {
            return;
        }
        state.current = Some(source.id.clone());
        state.result = None;
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        if state.closed {
            state.result = Some(Frame {
                source_id: source.id.clone(),
                image: None,
            });
        } else {
            state.pending = Some((generation, source.clone()));
        }
        self.shared.1.notify_one();
    }

    pub fn take(&self) -> Option<Frame> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .result
            .take()
    }

    pub fn clear(&self) {
        let mut state = self.shared.0.lock().unwrap_or_else(|p| p.into_inner());
        if state.current.take().is_some() {
            self.generation.fetch_add(1, Ordering::AcqRel);
            state.pending = None;
            state.result = None;
            self.shared.1.notify_one();
        }
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        let mut state = self.shared.0.lock().unwrap_or_else(|p| p.into_inner());
        state.closed = true;
        state.pending = None;
        state.result = None;
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.shared.1.notify_one();
    }
}

struct Pipeline(gst::Pipeline);
impl Drop for Pipeline {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}

fn capture(source: &Source, cancelled: &dyn Fn() -> bool) -> Result<ColorImage> {
    ensure!(
        !cancelled() && sources::validate(source),
        "Source unavailable."
    );
    let pipeline = Pipeline(gst::parse::launch(
        "queue name=entry max-size-buffers=1 max-size-bytes=0 max-size-time=0 leaky=downstream ! d3d11convert add-borders=true ! video/x-raw(memory:D3D11Memory),format=RGBA,width=480,height=270 ! d3d11download ! video/x-raw,format=RGBA ! appsink name=local max-buffers=1 drop=true sync=false async=false wait-on-eos=false"
    )?.downcast::<gst::Pipeline>().map_err(|_|anyhow::anyhow!("Invalid local preview pipeline."))?);
    let input = match source.target {
        Target::Pattern => gst::ElementFactory::make("d3d11testsrc")
            .property("is-live", true)
            .build()?,
        Target::Monitor(handle) => gst::ElementFactory::make("d3d11screencapturesrc")
            .property("monitor-handle", handle)
            .property("show-cursor", false)
            .build()?,
        Target::MonitorWgc(handle) => gst::ElementFactory::make("d3d11screencapturesrc")
            .property_from_str("capture-api", "wgc")
            .property("monitor-handle", handle)
            .property("show-cursor", false)
            .build()?,
        Target::Window { handle, .. } => gst::ElementFactory::make("d3d11screencapturesrc")
            .property_from_str("capture-api", "wgc")
            .property("window-handle", handle)
            .property("show-cursor", false)
            .build()?,
    };
    input.set_property("num-buffers", 1_i32);
    pipeline.0.add(&input)?;
    input.link(
        &pipeline
            .0
            .by_name("entry")
            .context("Missing capture queue.")?,
    )?;
    let sink = pipeline
        .0
        .by_name("local")
        .context("Missing local sink.")?
        .downcast::<AppSink>()
        .map_err(|_| anyhow::anyhow!("Invalid local sink."))?;
    ensure!(!cancelled(), "Cancelled.");
    pipeline.0.set_state(gst::State::Playing)?;
    let started = Instant::now();
    while started.elapsed() < CAPTURE_TIMEOUT && !cancelled() {
        if let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_mseconds(25)) {
            ensure!(!cancelled(), "Cancelled.");
            let info = gstreamer_video::VideoInfo::from_caps(
                sample.caps().context("Missing preview caps.")?,
            )?;
            ensure!(
                info.width() == 480 && info.height() == 270,
                "Invalid preview dimensions."
            );
            let buffer = sample
                .buffer()
                .context("Missing preview frame.")?
                .map_readable()?;
            let stride = usize::try_from(info.stride()[0])?;
            ensure!(
                stride >= 480 * 4 && buffer.len() >= (269 * stride + 480 * 4),
                "Invalid preview stride."
            );
            let mut rgba = Vec::with_capacity(480 * 270 * 4);
            for row in 0..270 {
                rgba.extend_from_slice(&buffer[row * stride..row * stride + 480 * 4]);
            }
            return Ok(ColorImage::from_rgba_unmultiplied([480, 270], &rgba));
        }
        if sink.is_eos() {
            break;
        }
        if let Some(bus) = pipeline.0.bus()
            && bus.pop_filtered(&[gst::MessageType::Error]).is_some()
        {
            break;
        }
    }
    anyhow::bail!("Local preview unavailable.")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn info(id: &str) -> SourceInfo {
        SourceInfo {
            id: id.into(),
            label: id.into(),
            width: 1920,
            height: 1080,
        }
    }
    fn wait(preview: &Preview) -> Frame {
        let start = Instant::now();
        loop {
            if let Some(frame) = preview.take() {
                return frame;
            }
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn latest_hover_wins_and_one_still_is_captured_per_source() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = calls.clone();
        let preview = Preview::with_capture(move |_, _| {
            seen.fetch_add(1, Ordering::Relaxed);
            Some(ColorImage::filled([2, 2], eframe::egui::Color32::WHITE))
        });
        for i in 0..100 {
            preview.request(&info(&i.to_string()));
        }
        assert_eq!(wait(&preview).source_id, "99");
        for _ in 0..20 {
            preview.request(&info("99"));
        }
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        preview.clear();
        preview.request(&info("99"));
        wait(&preview);
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn clearing_cancels_in_flight_capture_and_discards_its_pixels() {
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed = started.clone();
        let ended = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let finished = ended.clone();
        let preview = Preview::with_capture(move |_, cancelled| {
            observed.store(true, Ordering::Release);
            while !cancelled() {
                std::thread::sleep(Duration::from_millis(2));
            }
            finished.store(true, Ordering::Release);
            Some(ColorImage::filled([2, 2], eframe::egui::Color32::WHITE))
        });
        preview.request(&info("private-source"));
        let start = Instant::now();
        while !started.load(Ordering::Acquire) {
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(5));
        }
        preview.clear();
        while !ended.load(Ordering::Acquire) {
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(preview.take().is_none());
    }

    #[test]
    fn settled_source_recovers_from_a_transient_failure_without_another_hover() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = calls.clone();
        let preview = Preview::with_capture(move |_, _| {
            (observed.fetch_add(1, Ordering::Relaxed) > 0)
                .then(|| ColorImage::filled([2, 2], eframe::egui::Color32::WHITE))
        });
        preview.request(&info("latest"));
        let frame = wait(&preview);
        assert_eq!(frame.source_id, "latest");
        assert!(frame.image.is_some());
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn permanent_failure_has_a_bounded_retry_count() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = calls.clone();
        let preview = Preview::with_capture(move |_, _| {
            observed.fetch_add(1, Ordering::Relaxed);
            None
        });
        preview.request(&info("unavailable"));
        assert!(wait(&preview).image.is_none());
        for _ in 0..100 {
            preview.request(&info("unavailable"));
        }
        assert_eq!(calls.load(Ordering::Relaxed), CAPTURE_ATTEMPTS);
        assert!(preview.shared.0.lock().unwrap().pending.is_none());
    }

    #[test]
    fn rapid_a_b_a_switch_during_capture_returns_the_latest_a_frame() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = calls.clone();
        let preview = Preview::with_capture(move |_, cancelled| {
            if observed.fetch_add(1, Ordering::AcqRel) == 0 {
                let start = Instant::now();
                while !cancelled() {
                    assert!(start.elapsed() < Duration::from_secs(3));
                    std::thread::sleep(Duration::from_millis(2));
                }
                None
            } else {
                Some(ColorImage::filled([2, 2], eframe::egui::Color32::WHITE))
            }
        });
        preview.request(&info("A"));
        let start = Instant::now();
        while calls.load(Ordering::Acquire) == 0 {
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        preview.request(&info("B"));
        preview.request(&info("A"));
        let frame = wait(&preview);
        assert_eq!(frame.source_id, "A");
        assert!(frame.image.is_some());
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn new_hover_cancels_backoff_instead_of_retrying_the_previous_source() {
        let old_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = old_calls.clone();
        let preview = Preview::with_capture(move |source, _| {
            if source.id == "old" {
                observed.fetch_add(1, Ordering::Release);
                None
            } else {
                Some(ColorImage::filled([2, 2], eframe::egui::Color32::WHITE))
            }
        });
        preview.request(&info("old"));
        let start = Instant::now();
        while old_calls.load(Ordering::Acquire) == 0 {
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        preview.request(&info("new"));
        let frame = wait(&preview);
        assert_eq!(frame.source_id, "new");
        assert!(frame.image.is_some());
        assert_eq!(old_calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    #[ignore = "Requires the Windows D3D11 runtime; run explicitly on local hardware"]
    fn real_gpu_still_capture_without_encoder_audio_or_publication() {
        gst::init().unwrap();
        let image =
            capture(&sources::fixture(), &|| false).expect("Real D3D11 test-source capture");
        assert_eq!(image.size, [480, 270]);
        assert!(image.pixels.windows(2).any(|pair| pair[0] != pair[1]));
        // Exercise a real monitor too, retaining all pixels only in this process.
        let monitor = sources::enumerate()
            .into_iter()
            .find(|source| matches!(source.target, Target::Monitor(_)))
            .expect("Local monitor");
        assert_eq!(
            capture(&monitor, &|| false)
                .expect("Real monitor still capture")
                .size,
            [480, 270]
        );
        #[cfg(windows)]
        {
            use windows::{
                Win32::{
                    Foundation::HWND,
                    Graphics::Gdi::UpdateWindow,
                    UI::WindowsAndMessaging::{
                        CreateWindowExW, DestroyWindow, SW_SHOWNOACTIVATE, ShowWindow,
                        WS_EX_APPWINDOW, WS_OVERLAPPEDWINDOW,
                    },
                },
                core::w,
            };
            struct Fixture(HWND);
            impl Drop for Fixture {
                fn drop(&mut self) {
                    unsafe {
                        let _ = DestroyWindow(self.0);
                    }
                }
            }
            // A user's largest window can be protected, disappear or change
            // while the test runs. Capture an owned, non-activating fixture.
            let fixture = Fixture(
                unsafe {
                    CreateWindowExW(
                        WS_EX_APPWINDOW,
                        w!("STATIC"),
                        w!("FreedomStream silent capture fixture"),
                        WS_OVERLAPPEDWINDOW,
                        40,
                        40,
                        320,
                        240,
                        None,
                        None,
                        None,
                        None,
                    )
                }
                .expect("Create capture fixture"),
            );
            unsafe {
                let _ = ShowWindow(fixture.0, SW_SHOWNOACTIVATE);
                let _ = UpdateWindow(fixture.0);
            }
            let window = Source {
                info: crate::sources::SourceInfo {
                    id: "capture-fixture".into(),
                    label: "Capture fixture".into(),
                    width: 320,
                    height: 240,
                },
                target: Target::Window {
                    handle: fixture.0.0 as usize as u64,
                    pid: std::process::id(),
                },
            };
            let image = capture(&window, &|| false).expect("Real WGC window still capture");
            assert_eq!(image.size, [480, 270]);
            assert!(
                image.pixels.windows(2).any(|pair| pair[0] != pair[1]),
                "Window capture must contain rendered pixels"
            );
        }
    }
}
