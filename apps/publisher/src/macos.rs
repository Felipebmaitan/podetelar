//! ScreenCaptureKit helper ownership and bounded raw-media transport (MIT).
use crate::sources::{Source, SourceInfo, Target};
use anyhow::{Context, Result, ensure};
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSrc;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    thread::JoinHandle,
};

pub fn request_capture_permission() {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    // The GUI, rather than an ephemeral capture process, owns the permission.
    unsafe {
        if !CGPreflightScreenCaptureAccess() {
            let _ = CGRequestScreenCaptureAccess();
        }
    }
}

fn helper() -> Result<std::path::PathBuf> {
    let exe = std::env::current_exe()?;
    if let Some(helper) = exe
        .ancestors()
        .map(|path| path.join("MacOS/PodeTelar"))
        .find(|path| path.is_file())
    {
        return Ok(helper);
    }
    Ok(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.local/macos/PodeTelar.app/Contents/MacOS/PodeTelar"))
}
fn spawn(action: &str, request: &Value) -> Result<Child> {
    let mut child = Command::new(helper()?)
        .arg(action)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("macOS capture helper unavailable.")?;
    let result = (|| -> Result<()> {
        let mut input = child.stdin.take().context("Missing capture input.")?;
        input.write_all(&serde_json::to_vec(request)?)?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    Ok(child)
}
pub fn query<T: serde::de::DeserializeOwned>(action: &str, request: Value) -> Result<T> {
    // ScreenCaptureKit can wait indefinitely for permission/a dead system service.
    // Always retain and reap the helper, including read errors and timeouts.
    let mut child = spawn(action, &request)?;
    let output = child.stdout.take().context("Missing helper output.")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        output
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| anyhow::anyhow!("macOS helper reader stopped."))??;
    ensure!(
        status.is_some_and(|s| s.success()) && bytes.len() <= 1024 * 1024,
        "macOS screen recording permission or capture source unavailable."
    );
    serde_json::from_slice(&bytes).context("Invalid macOS helper response.")
}

#[derive(Deserialize)]
struct Item {
    kind: String,
    id: u32,
    label: String,
    width: u32,
    height: u32,
    pid: u32,
}
pub fn enumerate() -> Vec<Source> {
    static CACHE: std::sync::Mutex<Option<(std::time::Instant, Vec<Source>)>> =
        std::sync::Mutex::new(None);
    let mut cache = CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((at, sources)) = cache.as_ref()
        && at.elapsed() < std::time::Duration::from_millis(250)
    {
        return sources.clone();
    }
    let sources = enumerate_now();
    *cache = Some((std::time::Instant::now(), sources.clone()));
    sources
}
fn enumerate_now() -> Vec<Source> {
    query::<Vec<Item>>("enumerate", json!({"action":"enumerate"}))
        .unwrap_or_default()
        .into_iter()
        .map(|i| Source {
            info: SourceInfo {
                id: format!("{}:{}", i.kind, i.id),
                label: i.label,
                width: i.width,
                height: i.height,
            },
            target: if i.kind == "window" {
                Target::Window {
                    handle: i.id as u64,
                    pid: i.pid,
                }
            } else {
                Target::Monitor(i.id as u64)
            },
        })
        .collect()
}
pub fn validate(source: &Source) -> bool {
    matches!(source.target, Target::Pattern)
        || enumerate().iter().any(|s| {
            s.info.id == source.info.id
                && match (&s.target, &source.target) {
                    (
                        Target::Window { handle: a, pid: x },
                        Target::Window { handle: b, pid: y },
                    ) => a == b && x == y,
                    (Target::Monitor(a), Target::Monitor(b) | Target::MonitorWgc(b)) => a == b,
                    _ => false,
                }
        })
}
/// Drops before the pipeline: kill closes a blocked pipe, then join its reader.
/// appsrc never blocks, and only two frames can remain queued.
pub struct Capture {
    child: Child,
    reader: Option<JoinHandle<()>>,
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
impl Capture {
    pub fn attach(
        pipeline: &gst::Pipeline,
        name: &str,
        action: &str,
        request: Value,
        caps: gst::Caps,
        max_packet: usize,
        fixed_packet: Option<usize>,
    ) -> Result<Self> {
        let source = pipeline
            .by_name(name)
            .context("Missing macOS media input.")?
            .downcast::<AppSrc>()
            .map_err(|_| anyhow::anyhow!("Invalid macOS media input."))?;
        source.set_caps(Some(&caps));
        source.set_format(gst::Format::Time);
        source.set_property("is-live", true);
        source.set_property("do-timestamp", true);
        source.set_property("block", false);
        source.set_property("max-buffers", 2_u64);
        source.set_property_from_str("leaky-type", "downstream");
        let mut child = spawn(action, &request)?;
        let mut output = child
            .stdout
            .take()
            .context("Missing macOS capture output.")?;
        let reader = std::thread::spawn(move || {
            loop {
                let mut header = [0; 4];
                if output.read_exact(&mut header).is_err() {
                    break;
                }
                let size = u32::from_le_bytes(header) as usize;
                if size == 0 || size > max_packet || fixed_packet.is_some_and(|fixed| fixed != size)
                {
                    break;
                }
                let mut data = vec![0; size];
                if output.read_exact(&mut data).is_err()
                    || source
                        .push_buffer(gst::Buffer::from_mut_slice(data))
                        .is_err()
                {
                    break;
                }
            }
            let _ = source.end_of_stream();
        });
        Ok(Self {
            child,
            reader: Some(reader),
        })
    }
}
pub fn video(
    pipeline: &gst::Pipeline,
    source: &Source,
    width: u32,
    height: u32,
    fps: u32,
) -> Result<Option<Capture>> {
    if matches!(source.target, Target::Pattern) {
        return Ok(None);
    }
    let (kind, id) = match source.target {
        Target::Window { handle, .. } => ("window", handle),
        Target::Monitor(id) | Target::MonitorWgc(id) => ("monitor", id),
        Target::Pattern => unreachable!(),
    };
    let size = width as usize * height as usize * 4;
    Ok(Some(Capture::attach(
        pipeline,
        "capture",
        "video",
        json!({"action":"video","kind":kind,"id":id,"width":width,"height":height,"fps":fps}),
        gst::Caps::builder("video/x-raw")
            .field("format", "BGRA")
            .field("width", width as i32)
            .field("height", height as i32)
            .field("framerate", gst::Fraction::new(fps as i32, 1))
            .build(),
        size,
        Some(size),
    )?))
}
pub fn input(source: &Source) -> Result<gst::Element> {
    if matches!(source.target, Target::Pattern) {
        Ok(gst::ElementFactory::make("videotestsrc")
            .property("is-live", true)
            .build()?)
    } else {
        Ok(gst::ElementFactory::make("appsrc")
            .name("capture")
            .build()?)
    }
}
