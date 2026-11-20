use crate::activation_protocol as protocol;
pub struct Inbox(
    Option<std::sync::mpsc::Receiver<protocol::Reply>>,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
);
impl Drop for Inbox {
    fn drop(&mut self) {
        self.1.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
impl Inbox {
    pub fn new() -> Self {
        let stopped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (Ok(address), Ok(token)) = (
            std::env::var(protocol::ENDPOINT),
            std::env::var(protocol::TOKEN),
        ) else {
            return Self(None, stopped);
        };
        let stop = stopped.clone();
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            loop {
                let Ok(event) = protocol::exchange(&address, &token, true, None) else {
                    return;
                };
                if event.focus && send.send(event).is_err() {
                    return;
                }
                if stop.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        });
        Self(Some(receive), stopped)
    }
    pub fn poll(&self) -> Option<protocol::Reply> {
        self.0.as_ref()?.try_recv().ok()
    }
}
