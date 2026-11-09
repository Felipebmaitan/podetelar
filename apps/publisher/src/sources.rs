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
mod windows;

pub fn enumerate() -> Vec<Source> {
    #[cfg(windows)]
    {
        windows::enumerate()
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
