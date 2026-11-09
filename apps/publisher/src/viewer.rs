// A separate Chrome profile/process keeps room playback out of captured game
// and browser application audio. No invitation is passed in a command line.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::Viewer;

#[cfg(not(windows))]
pub struct Viewer;
#[cfg(not(windows))]
impl Viewer {
    pub async fn open(_: &str, _: bool) -> anyhow::Result<Self> {
        anyhow::bail!("The isolated viewer is currently available on Windows only.")
    }
    pub async fn qualification_tone(&mut self) -> anyhow::Result<()> {
        anyhow::bail!("The isolated viewer is currently available on Windows only.")
    }
}
