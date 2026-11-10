use serde::Serialize;

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
    #[cfg(windows)]
    {
        let sources = windows::enumerate();
        #[cfg(feature = "performance-experiments")]
        let sources = {
            let mut sources = sources;
            if std::env::var("FREEDOMSTREAM_PERFORMANCE_FIXTURE").is_ok_and(|value| value == "1") {
                sources.push(fixture());
            }
            sources
        };
        sources
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

pub fn validate(source: &Source) -> bool {
    #[cfg(windows)]
    {
        windows::validate(source)
    }
    #[cfg(not(windows))]
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
