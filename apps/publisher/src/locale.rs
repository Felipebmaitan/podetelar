//! UI language is selected only for this process.
use std::{
    collections::BTreeMap,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};
static PORTUGUESE: AtomicBool = AtomicBool::new(true);
static TEXT: OnceLock<BTreeMap<String, String>> = OnceLock::new();
pub fn portuguese() -> bool {
    PORTUGUESE.load(Ordering::Relaxed)
}
pub fn select(portuguese: bool) {
    PORTUGUESE.store(portuguese, Ordering::Relaxed);
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
