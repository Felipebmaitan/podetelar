//! Native renderer layout fixtures, without admission, capture or publication.
#![cfg_attr(windows, windows_subsystem = "windows")]
#![allow(dead_code)]

#[path = "../src/activation.rs"]
mod activation;
#[path = "../../activation_protocol.rs"]
mod activation_protocol;
#[path = "../src/admission.rs"]
mod admission;
#[path = "../src/audio/mod.rs"]
mod audio;
#[path = "../src/browser_links.rs"]
mod browser_links;
#[path = "../src/connection.rs"]
mod connection;
#[path = "../src/delivery.rs"]
mod delivery;
#[path = "../src/diagnostics.rs"]
mod diagnostics;
#[path = "../src/encoding.rs"]
mod encoding;
#[path = "../src/locale.rs"]
mod locale;
#[path = "../src/network.rs"]
mod network;
#[path = "../src/playback.rs"]
mod playback;
#[path = "../src/preview.rs"]
mod preview;
#[path = "../src/publisher.rs"]
mod publisher;
#[path = "../src/quality.rs"]
mod quality;
#[path = "../src/settings.rs"]
mod settings;
#[path = "../src/source_picker.rs"]
mod source_picker;
#[path = "../src/sources.rs"]
mod sources;
#[path = "../src/stream_diagnostics.rs"]
mod stream_diagnostics;
#[path = "../src/ui.rs"]
mod ui;
#[path = "../src/ui_style.rs"]
mod ui_style;
#[path = "../../update_protocol.rs"]
mod update_protocol;
#[path = "../src/updates.rs"]
mod updates;
#[path = "../src/video_image.rs"]
mod video_image;
#[cfg(windows)]
#[path = "../src/win32.rs"]
mod win32;
#[path = "../src/worker.rs"]
mod worker;

#[path = "../src/native_output.rs"]
mod native_output;

#[path = "../src/av_qualification.rs"]
mod av_qualification;

fn main() -> anyhow::Result<()> {
    gstreamer::init()?;
    ui::review::run()
}
