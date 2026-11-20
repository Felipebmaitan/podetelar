//! Nonblocking completion checks include Windows thread-local/native cleanup.
use std::thread::JoinHandle;

pub fn finished<T>(worker: &JoinHandle<T>) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::{
            Foundation::{HANDLE, WAIT_OBJECT_0},
            System::Threading::WaitForSingleObject,
        };
        // The owned JoinHandle keeps this handle valid. A zero timeout never
        // blocks the event loop, including while a TLS/native destructor stalls.
        unsafe { WaitForSingleObject(HANDLE(worker.as_raw_handle()), 0) == WAIT_OBJECT_0 }
    }
    #[cfg(not(windows))]
    worker.is_finished()
}

#[cfg(test)]
pub fn drain_fixture<T>(worker: Option<JoinHandle<T>>) {
    if let Some(worker) = worker {
        let started = std::time::Instant::now();
        while !finished(&worker) {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(10),
                "Fixture worker did not stop"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(worker.join().is_ok(), "Fixture worker panicked");
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    struct Cleanup(mpsc::Sender<()>, mpsc::Receiver<()>);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            self.0.send(()).unwrap();
            self.1.recv_timeout(Duration::from_secs(5)).unwrap();
        }
    }
    thread_local! { static CLEANUP: RefCell<Option<Cleanup>> = const { RefCell::new(None) }; }

    #[test]
    fn completion_waits_for_thread_local_cleanup_without_blocking_the_caller() {
        let (entered, cleanup) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            CLEANUP.with(|slot| *slot.borrow_mut() = Some(Cleanup(entered, released)));
        });
        cleanup.recv_timeout(Duration::from_secs(5)).unwrap();
        // The function body has returned, but the TLS destructor is held open.
        let started = Instant::now();
        let premature = finished(&worker);
        let elapsed = started.elapsed();
        release.send(()).unwrap();
        worker.join().unwrap();
        assert!(!premature);
        assert!(elapsed < Duration::from_secs(1));
    }
}
