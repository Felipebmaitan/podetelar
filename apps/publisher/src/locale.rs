//! UI language only; preferences contain no room or session authority.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};
static PORTUGUESE: AtomicBool = AtomicBool::new(true);
static TEXT: OnceLock<BTreeMap<String, String>> = OnceLock::new();
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Preferences {
    portuguese: bool,
}
fn path() -> Option<std::path::PathBuf> {
    let root = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA")?);
    root.is_absolute()
        .then(|| root.join("FreedomStream/preferences.json"))
}
pub fn load() {
    use std::io::Read;
    let mut bytes = Vec::new();
    if let Some(path) = path()
        && let Ok(file) = std::fs::File::open(path)
        && file.take(1025).read_to_end(&mut bytes).is_ok()
        && bytes.len() <= 1024
        && let Ok(p) = serde_json::from_slice::<Preferences>(&bytes)
    {
        PORTUGUESE.store(p.portuguese, Ordering::Relaxed);
    }
}
pub fn portuguese() -> bool {
    PORTUGUESE.load(Ordering::Relaxed)
}
pub fn select(portuguese: bool) -> anyhow::Result<()> {
    PORTUGUESE.store(portuguese, Ordering::Relaxed);
    let path = path().ok_or_else(|| anyhow::anyhow!("Preferences unavailable."))?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Preferences unavailable."))?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    std::io::Write::write_all(&mut file, &serde_json::to_vec(&Preferences { portuguese })?)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}
pub fn tr(text: &str) -> &str {
    if !portuguese() {
        return text;
    }
    TEXT.get_or_init(|| {
        serde_json::from_str(include_str!("pt_br.json")).expect("Built-in language catalog")
    })
    .get(text)
    .map(String::as_str)
    .unwrap_or(text)
}
