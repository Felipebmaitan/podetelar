//! Local screen confirmation and live camera selection preview.
#[cfg(any(not(target_os = "macos"), test))]
use crate::sources::Target;
use crate::sources::{self, Source, SourceInfo};
use anyhow::{Context, Result, ensure};
use eframe::egui::ColorImage;
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSink;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
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
    pending: Option<(u64, Request)>,
    result: Option<Frame>,
    capturing_camera: bool,
    closed: bool,
}

enum Request {
    Screen(SourceInfo),
    Camera {
        key: String,
        device: String,
        quality: crate::quality::Settings,
    },
}

impl Request {
    fn key(&self) -> &str {
        match self {
            Self::Screen(info) => &info.id,
            Self::Camera { key, .. } => key,
        }
    }
}

pub struct Preview {
    shared: Arc<(Mutex<State>, Condvar)>,
    generation: Arc<AtomicU64>,
    worker: Option<std::thread::JoinHandle<()>>,
}

pub struct CameraRelease(Arc<(Mutex<State>, Condvar)>);

impl CameraRelease {
    pub fn wait(self, cancelled: &AtomicBool) -> Result<()> {
        let (lock, ready) = &*self.0;
        let mut state = lock.lock().unwrap_or_else(|p| p.into_inner());
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.capturing_camera && !cancelled.load(Ordering::Acquire) {
            ensure!(
                Instant::now() < deadline,
                "Camera preview is still closing. Try sharing again."
            );
            (state, _) = ready
                .wait_timeout(state, Duration::from_millis(25))
                .unwrap_or_else(|p| p.into_inner());
        }
        ensure!(
            !cancelled.load(Ordering::Acquire),
            "Capture request was cancelled."
        );
        Ok(())
    }
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

    pub(crate) fn with_capture(
        capture: impl Fn(&SourceInfo, &dyn Fn() -> bool) -> Option<ColorImage> + Send + 'static,
    ) -> Self {
        Self::with_captures(capture, capture_camera)
    }

    fn with_captures(
        capture: impl Fn(&SourceInfo, &dyn Fn() -> bool) -> Option<ColorImage> + Send + 'static,
        camera: impl Fn(
            &str,
            crate::quality::Settings,
            &dyn Fn() -> bool,
            &mut dyn FnMut(ColorImage),
        ) -> bool
        + Send
        + 'static,
    ) -> Self {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let generation = Arc::new(AtomicU64::new(0));
        let state = shared.clone();
        let epoch = generation.clone();
        // One worker and a single overwriteable request slot bound work and memory.
        let worker = std::thread::Builder::new()
            .name("podetelar-source-preview".into())
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
                    let (requested, request) = guard.pending.take().expect("Pending request");
                    guard.capturing_camera = matches!(&request, Request::Camera { .. });
                    // Wait until hover/focus settles; rapid list traversal never queues captures.
                    let (mut guard, _) = ready
                        .wait_timeout_while(guard, SETTLE, |state| {
                            !state.closed && epoch.load(Ordering::Acquire) == requested
                        })
                        .unwrap_or_else(|p| p.into_inner());
                    if guard.closed {
                        guard.capturing_camera = false;
                        ready.notify_all();
                        break;
                    }
                    if epoch.load(Ordering::Acquire) != requested {
                        guard.capturing_camera = false;
                        ready.notify_all();
                        continue;
                    }
                    drop(guard);
                    let cancelled = || epoch.load(Ordering::Acquire) != requested;
                    let mut publish = |image| {
                        let mut guard = lock.lock().unwrap_or_else(|p| p.into_inner());
                        if !guard.closed && !cancelled() {
                            guard.result = Some(Frame {
                                source_id: request.key().to_owned(),
                                image: Some(image),
                            });
                        }
                    };
                    let mut completed = false;
                    for attempt in 0..CAPTURE_ATTEMPTS {
                        if cancelled() {
                            break;
                        }
                        completed = match &request {
                            Request::Screen(info) => {
                                capture(info, &cancelled).map(&mut publish).is_some()
                            }
                            Request::Camera {
                                device, quality, ..
                            } => camera(device, *quality, &cancelled, &mut publish),
                        };
                        if completed || cancelled() || attempt + 1 == CAPTURE_ATTEMPTS {
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
                    guard.capturing_camera = false;
                    ready.notify_all();
                    if !completed && !guard.closed && !cancelled() {
                        guard.result = Some(Frame {
                            source_id: request.key().to_owned(),
                            image: None,
                        });
                    }
                }
            });
        if worker.is_err() {
            shared.0.lock().unwrap_or_else(|p| p.into_inner()).closed = true;
        }
        Self {
            shared,
            generation,
            worker: worker.ok(),
        }
    }

    pub fn request(&self, source: &SourceInfo) {
        self.request_frame(Request::Screen(source.clone()));
    }

    pub fn request_camera(&self, key: String, device: String, quality: crate::quality::Settings) {
        self.request_frame(Request::Camera {
            key,
            device,
            quality,
        });
    }

    pub fn camera_release(&self) -> Option<CameraRelease> {
        let state = self.shared.0.lock().unwrap_or_else(|p| p.into_inner());
        state
            .capturing_camera
            .then(|| CameraRelease(self.shared.clone()))
    }

    fn request_frame(&self, request: Request) {
        let mut state = self.shared.0.lock().unwrap_or_else(|p| p.into_inner());
        if state.current.as_deref() == Some(request.key()) {
            return;
        }
        state.current = Some(request.key().to_owned());
        state.result = None;
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        if state.closed {
            state.result = Some(Frame {
                source_id: request.key().to_owned(),
                image: None,
            });
        } else {
            state.pending = Some((generation, request));
        }
        self.shared.1.notify_all();
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
            self.shared.1.notify_all();
        }
    }
}

impl Preview {
    pub fn begin_shutdown(&self) {
        let mut state = self.shared.0.lock().unwrap_or_else(|p| p.into_inner());
        state.closed = true;
        state.pending = None;
        state.result = None;
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.shared.1.notify_all();
    }

    pub fn shutdown_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(crate::worker::finished)
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        self.begin_shutdown();
        // Normal window close polls completion; explicit Force close stays nonblocking.
        #[cfg(test)]
        crate::worker::drain_fixture(self.worker.take());
    }
}

struct Pipeline(gst::Pipeline);
impl Drop for Pipeline {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}

fn capture_camera(
    device_id: &str,
    quality: crate::quality::Settings,
    cancelled: &dyn Fn() -> bool,
    publish: &mut dyn FnMut(ColorImage),
) -> bool {
    (|| -> Result<()> {
        gst::init()?;
        let device = sources::camera::enumerate()
            .into_iter()
            .find(|device| device.info.id == device_id)
            .context("Camera unavailable.")?;
        capture_frames(&device.source(quality)?, cancelled, publish)
    })()
    .is_ok()
}

fn capture(source: &Source, cancelled: &dyn Fn() -> bool) -> Result<ColorImage> {
    let mut image = None;
    capture_frames(source, cancelled, &mut |frame| image = Some(frame))?;
    image.context("Missing preview frame.")
}

fn capture_frames(
    source: &Source,
    cancelled: &dyn Fn() -> bool,
    publish: &mut dyn FnMut(ColorImage),
) -> Result<()> {
    ensure!(
        !cancelled() && sources::validate(source),
        "Source unavailable."
    );
    #[cfg(any(windows, target_os = "macos"))]
    const CPU_PREVIEW: &str =
        "videoconvert ! videoscale add-borders=true ! video/x-raw,format=RGBA,width=480,height=270";
    #[cfg(not(target_os = "macos"))]
    const GPU_PREVIEW: &str = "d3d11convert add-borders=true ! video/x-raw(memory:D3D11Memory),format=RGBA,width=480,height=270 ! d3d11download ! video/x-raw,format=RGBA";
    #[cfg(target_os = "macos")]
    let processing = CPU_PREVIEW.to_owned();
    #[cfg(not(target_os = "macos"))]
    let processing = match &source.target {
        #[cfg(windows)]
        Target::Camera(camera)
            if camera
                .caps
                .structure(0)
                .is_some_and(|format| format.name() == "image/jpeg") =>
        {
            format!("jpegdec ! {CPU_PREVIEW}")
        }
        #[cfg(windows)]
        Target::Camera(_) => CPU_PREVIEW.to_owned(),
        _ => GPU_PREVIEW.to_owned(),
    };
    let pipeline = Pipeline(
        gst::parse::launch(&format!("queue name=entry max-size-buffers=1 max-size-bytes=0 max-size-time=0 leaky=downstream ! {processing} ! appsink name=local max-buffers=1 drop=true sync=false async=false wait-on-eos=false"))?
            .downcast::<gst::Pipeline>()
            .map_err(|_| anyhow::anyhow!("Invalid local preview pipeline."))?,
    );
    #[cfg(target_os = "macos")]
    let input = crate::macos::input(source)?;
    #[cfg(not(target_os = "macos"))]
    let input = match source.target {
        #[cfg(windows)]
        Target::Camera(ref camera) => camera.device.create_element(None)?,
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
    if !source.is_camera() && input.find_property("num-buffers").is_some() {
        input.set_property("num-buffers", 1_i32);
    }
    pipeline.0.add(&input)?;
    let entry = pipeline
        .0
        .by_name("entry")
        .context("Missing capture queue.")?;
    #[cfg(windows)]
    if let Target::Camera(camera) = &source.target {
        input.link_filtered(&entry, &camera.caps)?;
    } else {
        input.link(&entry)?;
    }
    #[cfg(not(windows))]
    input.link(&entry)?;
    let sink = pipeline
        .0
        .by_name("local")
        .context("Missing local sink.")?
        .downcast::<AppSink>()
        .map_err(|_| anyhow::anyhow!("Invalid local sink."))?;
    ensure!(!cancelled(), "Cancelled.");
    #[cfg(target_os = "macos")]
    let _capture = crate::macos::video(&pipeline.0, source, 480, 270, 30)?;
    pipeline.0.set_state(gst::State::Playing)?;
    let mut last_frame = Instant::now();
    while last_frame.elapsed() < CAPTURE_TIMEOUT && !cancelled() {
        if let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_mseconds(25)) {
            ensure!(!cancelled(), "Cancelled.");
            let image =
                crate::video_image::camera_image(&sample).context("Invalid preview frame.")?;
            ensure!(image.size == [480, 270], "Invalid preview dimensions.");
            publish(image);
            if !source.is_camera() {
                return Ok(());
            }
            last_frame = Instant::now();
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

    #[cfg(windows)]
    #[test]
    #[ignore = "requires a named Windows camera"]
    fn physical_camera_produces_live_local_frames_and_can_be_reopened() {
        let name = std::env::var("PODETELAR_TEST_CAMERA").expect("Name a local camera");
        gst::init().unwrap();
        let device = sources::camera::enumerate()
            .into_iter()
            .find(|device| device.info.name == name)
            .expect("Selected local camera");
        let quality = crate::quality::Settings {
            resolution: crate::quality::Resolution::P720,
            fps: 30,
            bitrate: crate::quality::Bitrate::Auto,
            ..Default::default()
        };
        let preview = Preview::new();
        for _ in 0..2 {
            preview.request_camera("camera".into(), device.info.id.clone(), quality);
            for _ in 0..10 {
                let image = wait(&preview).image.expect("Live camera frame");
                assert_eq!(image.size, [480, 270]);
            }
            let handoff = preview.camera_release().expect("Preview still owns camera");
            preview.clear();
            handoff.wait(&AtomicBool::new(false)).unwrap();
        }
    }

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
    fn camera_selection_reuses_capture_until_it_hands_the_device_off() {
        use eframe::egui::Color32;
        let next = Arc::new(AtomicBool::new(false));
        let open_count = Arc::new(AtomicU64::new(0));
        let preview = Preview::with_captures(|_, _| None, {
            let next = next.clone();
            let open_count = open_count.clone();
            move |_, _, cancelled, publish| {
                open_count.fetch_add(1, Ordering::Relaxed);
                publish(ColorImage::filled([2, 2], Color32::RED));
                while !cancelled() && !next.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                if !cancelled() {
                    publish(ColorImage::filled([2, 2], Color32::BLUE));
                }
                while !cancelled() {
                    std::thread::sleep(Duration::from_millis(5));
                }
                false
            }
        });
        preview.request_camera("choice".into(), "device".into(), Default::default());
        assert_eq!(wait(&preview).image.unwrap().pixels[0], Color32::RED);
        next.store(true, Ordering::Release);
        assert_eq!(wait(&preview).image.unwrap().pixels[0], Color32::BLUE);
        let handoff = preview.camera_release().expect("Preview still owns camera");
        preview.clear();
        handoff.wait(&AtomicBool::new(false)).unwrap();
        assert!(preview.camera_release().is_none());
        assert!(preview.take().is_none());
        assert_eq!(open_count.load(Ordering::Relaxed), 1);
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
                        w!("PodeTelar silent capture fixture"),
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
