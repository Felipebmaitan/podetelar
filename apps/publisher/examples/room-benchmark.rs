//! Real UI and room workload. Build with --features performance-experiments.
#![cfg_attr(windows, windows_subsystem = "windows")]
#![allow(dead_code)]

#[cfg(feature = "performance-experiments")]
#[path = "../src/admission.rs"]
mod admission;
#[cfg(feature = "performance-experiments")]
#[path = "../src/audio/mod.rs"]
mod audio;
#[cfg(feature = "performance-experiments")]
#[path = "../src/browser_links.rs"]
mod browser_links;
#[cfg(feature = "performance-experiments")]
#[path = "../src/connection.rs"]
mod connection;
#[cfg(feature = "performance-experiments")]
#[path = "../src/delivery.rs"]
mod delivery;
#[cfg(feature = "performance-experiments")]
#[path = "../src/diagnostics.rs"]
mod diagnostics;
#[cfg(feature = "performance-experiments")]
#[path = "../src/encoding.rs"]
mod encoding;
#[cfg(feature = "performance-experiments")]
#[path = "../src/locale.rs"]
mod locale;
#[cfg(feature = "performance-experiments")]
#[path = "../src/network.rs"]
mod network;
#[cfg(feature = "performance-experiments")]
#[path = "../src/planar_experiment.rs"]
mod planar_experiment;
#[cfg(feature = "performance-experiments")]
#[path = "../src/playback.rs"]
mod playback;
#[cfg(feature = "performance-experiments")]
#[path = "../src/preview.rs"]
mod preview;
#[cfg(feature = "performance-experiments")]
#[path = "../src/publisher.rs"]
mod publisher;
#[cfg(feature = "performance-experiments")]
#[path = "../src/quality.rs"]
mod quality;
#[cfg(feature = "performance-experiments")]
#[path = "../src/resource_experiment.rs"]
mod resource_experiment;
#[cfg(feature = "performance-experiments")]
#[path = "../src/settings.rs"]
mod settings;
#[cfg(feature = "performance-experiments")]
#[path = "../src/source_picker.rs"]
mod source_picker;
#[cfg(feature = "performance-experiments")]
#[path = "../src/sources.rs"]
mod sources;
#[cfg(feature = "performance-experiments")]
#[path = "../src/ui.rs"]
mod ui;
#[cfg(feature = "performance-experiments")]
#[path = "../src/ui_style.rs"]
mod ui_style;
#[cfg(feature = "performance-experiments")]
#[path = "../src/video_image.rs"]
mod video_image;
#[cfg(all(windows, feature = "performance-experiments"))]
#[path = "../src/win32.rs"]
mod win32;
#[cfg(feature = "performance-experiments")]
#[path = "../src/worker.rs"]
mod worker;

#[cfg(feature = "performance-experiments")]
fn main() -> anyhow::Result<()> {
    use std::io::Read;
    gstreamer::init().map_err(|_| anyhow::anyhow!("Runtime initialization failed."))?;
    if std::env::args().nth(1).as_deref() == Some("--check-planar") {
        return planar_experiment::check(
            std::env::args()
                .nth(2)
                .ok_or_else(|| anyhow::anyhow!("Choose a report path"))?
                .into(),
        );
    }
    if std::env::args().nth(1).as_deref() == Some("--sdk-soak") {
        return ui::experiment::sdk_soak(&std::path::PathBuf::from(
            std::env::args()
                .nth(2)
                .ok_or_else(|| anyhow::anyhow!("Choose a report path."))?,
        ));
    }
    if std::env::var("FREEDOMSTREAM_COUNT_SDK_LOGS").is_ok_and(|value| value == "1") {
        playback::count_sdk_logs()?;
    }
    if std::env::args().nth(1).as_deref() == Some("--micro") {
        return ui::experiment::micro(&std::path::PathBuf::from(
            std::env::args()
                .nth(2)
                .ok_or_else(|| anyhow::anyhow!("Choose a report path."))?,
        ));
    }
    let mut input = String::new();
    std::io::stdin().take(16385).read_to_string(&mut input)?;
    anyhow::ensure!(input.len() <= 16384, "Benchmark input too large.");
    let request =
        serde_json::from_str(&input).map_err(|_| anyhow::anyhow!("Invalid benchmark input."))?;
    ui::experiment::run(request)
}
#[cfg(not(feature = "performance-experiments"))]
fn main() {}
