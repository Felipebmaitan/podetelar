use windows::Win32::Foundation::{CloseHandle, HANDLE};

// Owns only handles returned by successful Win32 acquisition calls. Sharing a
// handle between tasks is allowed; closing remains unique to this owner.
pub struct Handle(pub HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
unsafe impl Send for Handle {}
