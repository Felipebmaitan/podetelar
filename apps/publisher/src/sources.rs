use serde::Serialize;
#[cfg(all(windows, any(debug_assertions, feature = "media-qualification")))]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(all(windows, any(debug_assertions, feature = "media-qualification")))]
static FIXTURE_SOURCES_ONLY: AtomicBool = AtomicBool::new(false);

#[cfg(all(windows, any(debug_assertions, feature = "media-qualification")))]
pub(crate) fn restrict_to_fixture() {
    FIXTURE_SOURCES_ONLY.store(true, Ordering::Release);
}

#[derive(Clone, Serialize)]
pub struct SourceInfo {
    pub id: String,
    pub label: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone)]
pub enum Target {
    Monitor(u64),
    MonitorWgc(u64),
    Window { handle: u64, pid: u32 },
    Pattern,
}

#[derive(Clone)]
pub struct Source {
    pub info: SourceInfo,
    pub target: Target,
}

#[cfg(windows)]
#[path = "sources/windows.rs"]
mod windows;

pub fn enumerate() -> Vec<Source> {
    #[cfg(all(windows, any(debug_assertions, feature = "media-qualification")))]
    if FIXTURE_SOURCES_ONLY.load(Ordering::Acquire) {
        return vec![fixture()];
    }
    #[cfg(windows)]
    {
        let sources = windows::enumerate();
        #[cfg(feature = "performance-experiments")]
        let sources = {
            let mut sources = sources;
            if std::env::var("PODETELAR_PERFORMANCE_FIXTURE").is_ok_and(|value| value == "1") {
                sources.push(fixture());
            }
            sources
        };
        sources
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::enumerate()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Vec::new()
    }
}

pub fn validate(source: &Source) -> bool {
    #[cfg(windows)]
    {
        windows::validate(source)
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::validate(source)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        matches!(source.target, Target::Pattern)
    }
}

pub fn fixture() -> Source {
    Source {
        info: SourceInfo {
            id: "fixture".into(),
            label: "Hardware test pattern".into(),
            width: 1920,
            height: 1080,
        },
        target: Target::Pattern,
    }
}
