// MIT. Local-copy instrumentation; never part of the distributed dependency.
use std::{collections::HashSet, sync::{Mutex, OnceLock, atomic::{AtomicU64, Ordering}}};
static TAGS: OnceLock<Mutex<HashSet<egui::TextureId>>> = OnceLock::new();
static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static PARTIAL: AtomicU64 = AtomicU64::new(0);
static OVERFLOW: AtomicU64 = AtomicU64::new(0);
pub fn tag_video(id: egui::TextureId) {
    let mut tags=TAGS.get_or_init(Default::default).lock().unwrap();
    if tags.len()<512||tags.contains(&id) { tags.insert(id); }
    else { OVERFLOW.fetch_add(1,Ordering::Relaxed); }
}
pub(crate) fn forget(id: egui::TextureId) {
    if let Some(tags)=TAGS.get() { tags.lock().unwrap().remove(&id); }
}
pub(crate) fn uploaded(id: egui::TextureId, bytes: usize, partial: bool) {
    if TAGS.get().is_some_and(|tags|tags.lock().unwrap().contains(&id)) {
        CALLS.fetch_add(1,Ordering::Relaxed);
        BYTES.fetch_add(bytes as u64,Ordering::Relaxed);
        if partial { PARTIAL.fetch_add(1,Ordering::Relaxed); }
    }
}
pub fn snapshot() -> [u64;5] {
    [CALLS.load(Ordering::Relaxed),BYTES.load(Ordering::Relaxed),PARTIAL.load(Ordering::Relaxed),
     TAGS.get().map_or(0,|tags|tags.lock().unwrap().len() as u64),OVERFLOW.load(Ordering::Relaxed)]
}
