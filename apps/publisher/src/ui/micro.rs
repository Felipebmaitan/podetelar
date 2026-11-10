//! Bounded local probes. No room, process names, credentials or media payloads in reports.
use super::*;
use gstreamer::{self as gst, prelude::*};
use std::{hint::black_box, path::Path, sync::Arc};

fn timed(name: &str, iterations: usize, mut action: impl FnMut()) -> serde_json::Value {
    for _ in 0..100.min(iterations) {
        action();
    }
    let start = Instant::now();
    for _ in 0..iterations {
        action();
    }
    json!({"name":name,"iterations":iterations,"wallMs":start.elapsed().as_secs_f64()*1000.0})
}

pub fn run(report: &Path) -> Result<()> {
    ensure!(
        report.is_absolute() && !report.exists(),
        "Choose a new absolute report path."
    );
    let mut results = Vec::new();
    let invitation = format!(
        "https://example.test/r/{}#invite={}",
        "a".repeat(22),
        "b".repeat(43)
    );
    let profile = crate::settings::Profile::new(&invitation, "Synthetic benchmark")?;
    results.push(timed("profile-parse", 50_000, || {
        black_box(
            crate::settings::Profile::new(black_box(&invitation), "Synthetic benchmark").unwrap(),
        );
    }));
    results.push(timed("profile-unchanged-check", 50_000, || {
        black_box(
            profile.invitation == *black_box(&invitation) && profile.name == "Synthetic benchmark",
        );
    }));
    let jpeg = crate::preview::Frame {
        sequence: 1,
        bytes: vec![0x5a; 96 * 1024],
    };
    results.push(timed("unchanged-jpeg-clone", 10_000, || {
        black_box(black_box(&jpeg).clone());
    }));
    results.push(timed("unchanged-jpeg-revision-check", 10_000, || {
        black_box(black_box(&jpeg).sequence == 1);
    }));
    let snapshot = crate::publisher::Snapshot {
        sources: (0..32)
            .map(|i| crate::sources::SourceInfo {
                id: format!("fixture-{i}"),
                label: format!("Synthetic source {i}"),
                width: 1920,
                height: 1080,
            })
            .collect(),
        participants: (0..32)
            .map(|i| format!("Synthetic participant {i}"))
            .collect(),
        ..Default::default()
    };
    results.push(timed("snapshot-deep-clone-32-sources", 50_000, || {
        black_box(black_box(&snapshot).clone());
    }));
    let shared = Arc::new(snapshot);
    results.push(timed("snapshot-arc-clone-32-sources", 50_000, || {
        black_box(black_box(&shared).clone());
    }));
    let level = gst::ElementFactory::make("volume").build()?;
    results.push(timed("audio-unchanged-gain-setter", 100_000, || {
        level.set_property("volume", 1.0_f64);
    }));
    let volume = std::sync::atomic::AtomicU32::new(100);
    results.push(timed("audio-unchanged-gain-check", 100_000, || {
        if black_box(volume.load(Ordering::Relaxed)) != 100 {
            level.set_property("volume", 0.5_f64);
        }
    }));
    let pipeline = gst::Pipeline::new();
    results.push(timed("audio-bus-lookup", 100_000, || {
        black_box(pipeline.bus());
    }));
    let bus = pipeline.bus();
    results.push(timed("audio-cached-bus", 100_000, || {
        black_box(&bus);
    }));
    let samples: Vec<i16> = (0..960).map(|i| (i * 97 - 32000) as i16).collect();
    let expected: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    let actual = crate::playback::audio_buffer(samples.clone());
    ensure!(
        actual.map_readable()?.as_slice() == expected,
        "PCM byte equivalence failed."
    );
    results.push(timed("audio-pcm-owned-plus-copy", 100_000, || {
        let owned = black_box(&samples).clone();
        let bytes: Vec<u8> = owned.iter().flat_map(|s| s.to_le_bytes()).collect();
        black_box(gst::Buffer::from_mut_slice(bytes));
    }));
    results.push(timed("audio-pcm-owned-transfer", 100_000, || {
        black_box(crate::playback::audio_buffer(black_box(&samples).clone()));
    }));
    // Preserve every sample peak. Lower-rate publication may reduce locking,
    // whereas skipping sample scans could miss transients and is not equivalent.
    let mut direct_peak = 0.0_f64;
    let scalar = std::sync::Mutex::new((0_u64, 0.0_f64));
    results.push(timed("audio-scalars-every-frame", 20_000, || {
        let mut current = scalar.lock().unwrap();
        for pair in black_box(&samples).chunks_exact(2) {
            current.1 = current.1.max(pair[0].unsigned_abs() as f64 / 32768.0);
        }
        current.0 += 480;
        direct_peak = current.1;
        black_box(&current);
    }));
    let mut local_peak = 0.0_f64;
    let mut local_frames = 0_u64;
    results.push(timed("audio-scalars-publish-ten-frames", 20_000, || {
        for pair in black_box(&samples).chunks_exact(2) {
            local_peak = local_peak.max(pair[0].unsigned_abs() as f64 / 32768.0);
        }
        local_frames += 480;
        if local_frames.is_multiple_of(4800) {
            *scalar.lock().unwrap() = (local_frames, local_peak);
        }
        black_box(local_peak);
    }));
    ensure!(local_peak == direct_peak, "Scalar peak changed.");
    results.push(timed("upload-fresh-64k-chunk", 20_000, || {
        black_box(vec![0x5a_u8; black_box(65_536)]);
    }));
    let chunk = bytes::Bytes::from(vec![0x5a; 65_536]);
    results.push(timed("upload-shared-64k-chunk", 20_000, || {
        black_box(chunk.slice(..black_box(65_536)));
    }));
    results.push(timed("audio-full-inventory", 20, || {
        black_box(crate::audio::applications().is_ok());
    }));
    let factory = livekit::webrtc::peer_connection_factory::PeerConnectionFactory::default();
    results.push(timed("sdk-temporary-capability-factory", 20, || {
        black_box(crate::playback::decoder_codecs());
    }));
    results.push(timed("sdk-reused-capability-factory", 20, || {
        black_box(factory.get_rtp_receiver_capabilities(livekit::webrtc::MediaType::Video));
    }));
    drop(factory);
    #[cfg(windows)]
    let inventory = crate::audio::inventory_benchmark()?;
    #[cfg(not(windows))]
    let inventory = serde_json::Value::Null;
    std::fs::write(
        report,
        serde_json::to_vec_pretty(
            &json!({"schema":1,"kind":"local-operation-microbenchmarks","pcmFullBufferEquivalent":true,"inventory":inventory,"results":results,"limits":"Operation wall times, not whole-application CPU savings. Snapshot case intentionally uses maximum 32 synthetic sources; lower live counts cost less. Full inventory emits no process identities. Reused factory lifetime must be considered separately."}),
        )?,
    )?;
    Ok(())
}

pub fn sdk_soak(report: &Path) -> Result<()> {
    ensure!(
        report.is_absolute() && !report.exists(),
        "Choose a new absolute report path."
    );
    crate::playback::count_sdk_logs()?;
    use livekit::webrtc::{
        peer_connection_factory::{PeerConnectionFactory, native::PeerConnectionFactoryExt},
        video_source::{VideoResolution, native::NativeVideoSource},
    };
    let start = Instant::now();
    let factory = PeerConnectionFactory::default();
    let source = NativeVideoSource::new_encoded(VideoResolution {
        width: 320,
        height: 180,
    });
    std::thread::sleep(Duration::from_secs(3));
    let mut rounds = Vec::new();
    for round in 1..=20 {
        let before = Instant::now();
        for _ in 0..500 {
            black_box(factory.create_video_track("synthetic-lifecycle-probe", source.clone()));
        }
        rounds.push(json!({"round":round,"tracksCreatedAndDropped":round*500,"wallMs":before.elapsed().as_secs_f64()*1000.0,"second":start.elapsed().as_secs_f64(),"nativeLogMessages":crate::playback::SDK_MESSAGES.load(Ordering::Relaxed)}));
    }
    drop(source);
    drop(factory);
    std::thread::sleep(Duration::from_secs(3));
    std::fs::write(
        report,
        serde_json::to_vec_pretty(
            &json!({"schema":1,"kind":"actual-sdk-track-lifecycle","rounds":rounds,"wallSeconds":start.elapsed().as_secs_f64(),"limits":"10,000 actual tracks created/dropped on one factory; no frames, room, microphone or publication. Message counter discards all text. This stress workload greatly exceeds a normal friend session. It does not itself expose private SDK registry counts or prove a metadata leak."}),
        )?,
    )?;
    Ok(())
}
