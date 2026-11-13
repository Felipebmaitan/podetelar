//! On-demand aggregates. No persistence, media contents or raw SDK stats.
use crate::{
    admission::Admission,
    delivery,
    encoding::Metrics,
    publisher::StreamConfig,
    quality::{Mode, RateControl},
};
use livekit::prelude::LocalVideoTrack;
use reqwest::Method;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Panel {
    pub open: AtomicBool,
    pub attempts: std::sync::atomic::AtomicU64,
    pub successes: std::sync::atomic::AtomicU64,
    pub sender: Mutex<Option<(Instant, Value)>>,
    pub local: Mutex<Option<(Instant, u64, u64, u64)>>,
    pub rates: Mutex<Option<(f64, f64, f64)>>,
    pub rtc: Mutex<Vec<(String, String)>>,
    pub rtc_at: Mutex<Option<Instant>>,
}

#[allow(clippy::too_many_arguments)]
pub async fn sender(
    admission: Admission,
    video: LocalVideoTrack,
    stop: Arc<AtomicBool>,
    counters: Arc<Mutex<Metrics>>,
    delivery: Arc<Mutex<delivery::Metrics>>,
    rate: RateControl,
    config: StreamConfig,
    mode: Mode,
) {
    let mut sid = video.sid().to_string();
    let mut previous: Option<(Instant, Metrics)> = None;
    let mut sequence = 0_u64;
    let mut last_capture = 0;
    let mut last_inputs = 0;
    let mut last_overruns = 0;
    while !stop.load(Ordering::Acquire) {
        let current_sid = video.sid().to_string();
        if sid != current_sid {
            let _ = admission
                .stream_diagnostic(Method::DELETE, "self", &sid, None)
                .await;
            sid = current_sid;
            previous = None;
            sequence = 0;
            rate.diagnostic_enabled.store(false, Ordering::Relaxed);
        }
        if let Ok(demand) = admission
            .stream_diagnostic(Method::GET, "self", &sid, None)
            .await
        {
            if demand["demand"].as_u64().unwrap_or(0) > 0 {
                rate.diagnostic_enabled.store(true, Ordering::Relaxed);
                let captured = rate.capture_frames.load(Ordering::Relaxed);
                let inputs = rate.encoder_inputs.load(Ordering::Relaxed);
                let current = counters.lock().map(|m| m.clone()).unwrap_or_default();
                let now = Instant::now();
                if let Some((time, last)) = previous.take() {
                    let seconds = now.duration_since(time).as_secs_f64();
                    let encoder_seconds = current.elapsed_seconds - last.elapsed_seconds;
                    let outbound = delivery.lock().map(|m| m.clone()).unwrap_or_default();
                    sequence += 1;
                    let rejected = current
                        .frames
                        .saturating_sub(last.frames)
                        .saturating_sub(current.accepted.saturating_sub(last.accepted));
                    let sample = json!({"window_ms": (seconds*1000.0) as u64, "sequence":sequence, "codec": config.encoder.codec, "encoder":config.encoder.factory, "metrics": {
                        "encoded_window_ms":(encoder_seconds>0.0).then_some(encoder_seconds*1000.0), "capture_queue_frames":current.capture_queue_frames,"encoder_queue_frames":current.encoder_queue_frames, "capture_queue_overruns":rate.capture_overruns.load(Ordering::Relaxed).saturating_sub(last_overruns),
                        "requested_bps": rate.requested(), "effective_bps":current.encoder_bitrate,
                        "encoded_bps":(encoder_seconds>0.0).then(||current.bytes.saturating_sub(last.bytes) as f64*8.0/encoder_seconds),
                        "sent_bps":outbound.available.then_some(outbound.bitrate_bps),
                        "upload_cap_bps":rate.upload_cap(), "watcher_cap_bps":rate.viewer_cap(), "congestion_bps":current.congestion_bps,
                        "capture_fps":captured.saturating_sub(last_capture) as f64/seconds, "encoder_input_fps":inputs.saturating_sub(last_inputs) as f64/seconds,
                        "requested_fps":mode.fps, "encoded_fps":(encoder_seconds>0.0).then(||current.frames.saturating_sub(last.frames) as f64/encoder_seconds),
                        "sent_fps":outbound.available.then_some(outbound.sent_fps), "accepted_fps":(encoder_seconds>0.0).then(||current.accepted.saturating_sub(last.accepted) as f64/encoder_seconds),
                        "encoder_reconfigurations":current.encoder_bitrate_changes.saturating_sub(last.encoder_bitrate_changes), "handoff_rejections":rejected,
                        "source_width":config.source.info.width, "source_height":config.source.info.height,
                        "encoded_width":mode.width, "encoded_height":mode.height,
                        "rtt_ms":outbound.rtt_ms, "loss_fraction":outbound.loss_fraction
                    }});
                    let _ = admission
                        .stream_diagnostic(Method::PUT, "self", &sid, Some(&sample))
                        .await;
                }
                previous = Some((now, current));
                last_capture = captured;
                last_inputs = inputs;
                last_overruns = rate.capture_overruns.load(Ordering::Relaxed);
            } else {
                previous = None;
                rate.diagnostic_enabled.store(false, Ordering::Relaxed);
            }
        } else {
            previous = None;
            rate.diagnostic_enabled.store(false, Ordering::Relaxed);
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    rate.diagnostic_enabled.store(false, Ordering::Relaxed);
    let _ = admission
        .stream_diagnostic(Method::DELETE, "self", &sid, None)
        .await;
}

pub async fn viewer(
    admission: Admission,
    owner: String,
    sid: String,
    presentation: Arc<crate::playback::Presentation>,
    frames: Arc<std::sync::atomic::AtomicU64>,
    _bps: Arc<std::sync::atomic::AtomicU64>,
    track: livekit::prelude::RemoteVideoTrack,
) {
    let panel = &presentation.diagnostics;
    let mut demanded = false;
    let mut rtc_previous: Option<(Instant, livekit::webrtc::stats::InboundRtpStats)> = None;
    loop {
        let open = panel.open.load(Ordering::Relaxed);
        if open {
            if let Ok(mut state) = panel.rtc.lock() {
                state.clear();
            }
            if let Ok(Ok(stats)) =
                tokio::time::timeout(Duration::from_millis(500), track.get_stats()).await
            {
                for stat in stats {
                    if let livekit::webrtc::stats::RtcStats::InboundRtp(current) = stat {
                        let now = Instant::now();
                        if let Some((time, last)) = rtc_previous.take() {
                            let seconds = now.duration_since(time).as_secs_f64();
                            if current.rtc.id == last.rtc.id
                                && current.inbound.frames_decoded >= last.inbound.frames_decoded
                            {
                                let frames =
                                    current.inbound.frames_decoded - last.inbound.frames_decoded;
                                let emitted = current
                                    .inbound
                                    .jitter_buffer_emitted_count
                                    .saturating_sub(last.inbound.jitter_buffer_emitted_count);
                                let mut values = Vec::new();
                                let mut add = |label: &str, value: f64, unit: &str| {
                                    if value.is_finite() && value >= 0.0 {
                                        values
                                            .push((label.to_owned(), format!("{value:.2} {unit}")));
                                    }
                                };
                                add(
                                    "Received FPS",
                                    current
                                        .inbound
                                        .frames_received
                                        .saturating_sub(last.inbound.frames_received)
                                        as f64
                                        / seconds,
                                    "FPS",
                                );
                                add("SDK decoded FPS", frames as f64 / seconds, "FPS");
                                if frames > 0 {
                                    add(
                                        "Decode time",
                                        (current.inbound.total_decode_time
                                            - last.inbound.total_decode_time)
                                            * 1000.0
                                            / frames as f64,
                                        "ms/frame",
                                    );
                                }
                                if emitted > 0 {
                                    add(
                                        "Jitter-buffer delay",
                                        (current.inbound.jitter_buffer_delay
                                            - last.inbound.jitter_buffer_delay)
                                            * 1000.0
                                            / emitted as f64,
                                        "ms/frame",
                                    );
                                }
                                for (label, count) in [
                                    (
                                        "SDK dropped frames",
                                        current
                                            .inbound
                                            .frames_dropped
                                            .saturating_sub(last.inbound.frames_dropped)
                                            as u64,
                                    ),
                                    (
                                        "NACK",
                                        current
                                            .inbound
                                            .nack_count
                                            .saturating_sub(last.inbound.nack_count)
                                            as u64,
                                    ),
                                    (
                                        "PLI",
                                        current
                                            .inbound
                                            .pli_count
                                            .saturating_sub(last.inbound.pli_count)
                                            as u64,
                                    ),
                                    (
                                        "Freezes",
                                        current
                                            .inbound
                                            .freeze_count
                                            .saturating_sub(last.inbound.freeze_count)
                                            as u64,
                                    ),
                                    (
                                        "Retransmitted packets",
                                        current
                                            .inbound
                                            .retransmitted_packets_received
                                            .saturating_sub(
                                                last.inbound.retransmitted_packets_received,
                                            ),
                                    ),
                                ] {
                                    if count > 0 {
                                        add(label, count as f64, "/window");
                                    }
                                }
                                if current.received.packets_lost > last.received.packets_lost {
                                    add(
                                        "Lost packets",
                                        (current.received.packets_lost - last.received.packets_lost)
                                            as f64,
                                        "/window",
                                    );
                                }
                                let decoder = match current.inbound.decoder_implementation.as_str()
                                {
                                    "FreedomStream Direct3D11" => Some("Direct3D11 (hardware)"),
                                    "Dav1d" | "dav1d" => Some("dav1d (software)"),
                                    "libvpx" => Some("libvpx (software)"),
                                    _ => None,
                                };
                                if let Some(decoder) = decoder {
                                    values.push(("Decoder".into(), decoder.into()));
                                }
                                if let Ok(mut state) = panel.rtc.lock() {
                                    *state = values;
                                    if let Ok(mut time) = panel.rtc_at.lock() {
                                        *time = Some(Instant::now());
                                    }
                                }
                            }
                        }
                        rtc_previous = Some((now, current));
                    }
                }
            } else {
                rtc_previous = None;
                if let Ok(mut state) = panel.rtc.lock() {
                    state.clear();
                }
            }
            let now = Instant::now();
            let decoded = frames.load(Ordering::Relaxed);
            let uploaded = presentation.uploaded.load(Ordering::Relaxed);
            let overwritten = presentation.overwritten.load(Ordering::Relaxed);
            if let Ok(mut previous) = panel.local.lock() {
                if let Some((time, d, u, o)) = *previous {
                    let seconds = now.duration_since(time).as_secs_f64();
                    if let Ok(mut rates) = panel.rates.lock() {
                        *rates = Some((
                            decoded.saturating_sub(d) as f64 / seconds,
                            uploaded.saturating_sub(u) as f64 / seconds,
                            overwritten.saturating_sub(o) as f64 / seconds,
                        ));
                    }
                }
                *previous = Some((now, decoded, uploaded, overwritten));
            }
            panel.attempts.fetch_add(1, Ordering::Relaxed);
            if let Ok(value) = admission
                .stream_diagnostic(Method::POST, &owner, &sid, None)
                .await
                && let Ok(mut state) = panel.sender.lock()
            {
                panel.successes.fetch_add(1, Ordering::Relaxed);
                *state = Some((Instant::now(), value));
            }
            demanded = true;
        } else {
            if let Ok(mut state) = panel.rtc.lock() {
                state.clear();
            }
            rtc_previous = None;
            if demanded {
                let _ = admission
                    .stream_diagnostic(Method::DELETE, &owner, &sid, None)
                    .await;
                demanded = false;
            }
            if let Ok(mut previous) = panel.local.lock() {
                *previous = None;
            }
            if let Ok(mut rates) = panel.rates.lock() {
                *rates = None;
            }
            if let Ok(mut sender) = panel.sender.lock() {
                *sender = None;
            }
        }
        tokio::time::sleep(Duration::from_secs(if open { 3 } else { 1 })).await;
    }
}

pub fn panel(ctx: &eframe::egui::Context, tile: &crate::playback::Tile) {
    use crate::{locale::tr, ui_style as skin};
    use eframe::egui;
    let state = &tile.presentation.diagnostics;
    let mut open = state.open.load(Ordering::Relaxed) && tile.watching;
    if !open {
        return;
    }
    egui::Window::new(tr("Advanced information"))
        .id(egui::Id::unique(("stats", &tile.id)))
        .open(&mut open).default_width(560.0)
        .default_height((ctx.viewport_rect().height() - 100.0).clamp(260.0, 620.0))
        .max_height((ctx.viewport_rect().height() - 80.0).clamp(260.0, 640.0))
        .default_pos(eframe::egui::pos2(((ctx.viewport_rect().width() - 580.0) / 2.0).max(8.0), 32.0))
        .vscroll(true)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(skin::strong(&tile.name, 18.0));
            ui.label(egui::RichText::new(tr("Live measurements · refresh every 3 seconds")).size(12.0).color(skin::SECONDARY));
            section(ui, "Reception");
            metric(ui, "Codec", &tile.codec);
            metric(ui, "Received video payload", &format!("{:.2} Mbps", tile.received_bps.load(Ordering::Relaxed) as f64 / 1e6));
            metric(ui, "Decoder output (may include padding)", &format!("{} × {}", tile.presentation.width.load(Ordering::Relaxed), tile.presentation.height.load(Ordering::Relaxed)));
            if let Ok(rates) = state.rates.lock() {
                if let Some((decoded, _, _)) = *rates {
                    metric(ui, "Decoded output", &format!("{decoded:.1} FPS"));
                } else { note(ui, "Waiting for a complete sample window…"); }
            }
            egui::CollapsingHeader::new(tr("Playback details")).show(ui, |ui| {
                if let Ok(rates) = state.rates.lock() && let Some((_, submitted, overwritten)) = *rates {
                    metric(ui, "UI texture submissions", &format!("{submitted:.1} FPS"));
                    metric(ui, "Presentation overwrites", &format!("{overwritten:.1}/s"));
                }
                if let Ok(viewport) = tile.presentation.viewport.lock() && let Some((w,h)) = *viewport {
                    metric(ui, "Rendered viewport", &format!("{w:.0} × {h:.0} px"));
                    let width = tile.presentation.width.load(Ordering::Relaxed);
                    let scale = if width > 0 { format!("{:.3}×", w / width as f32) } else { tr("Unavailable").into() };
                    metric(ui, "Scale", &scale);
                }
            });
            if state.rtc_at.lock().is_ok_and(|time| time.is_some_and(|t| t.elapsed() < Duration::from_secs(7))) && let Ok(rtc) = state.rtc.lock() && !rtc.is_empty() {
                section(ui, "Network and decoding");
                for (label, value) in rtc.iter() { metric(ui, label, value); }
            }
            section(ui, "Sender");
            let sender = state.sender.lock().ok();
            if let Some(Some((received, value))) = sender.as_deref() {
                let age = value["age_ms"].as_u64().map(|age| age.saturating_add(received.elapsed().as_millis() as u64));
                metric(ui, "Sender sample age", &age.map(|age| format!("{age} ms")).unwrap_or_else(|| tr("Unavailable").into()));
                if value["stale"].as_bool().unwrap_or(true) || age.is_none_or(|a| a > 7000) {
                    note(ui, "Sender statistics unavailable or stale");
                } else {
                    let sample = &value["sample"];
                    metric(ui, "Encoder", sample["encoder"].as_str().unwrap_or(tr("Unavailable")));
                    for (key, label, divisor, unit) in [
                        ("requested_bps", "Requested bitrate", 1e6, "Mbps"),
                        ("effective_bps", "Effective encoder target", 1e6, "Mbps"),
                        ("sent_bps", "Sent video payload", 1e6, "Mbps"),
                        ("requested_fps", "Requested FPS", 1.0, "FPS"),
                        ("encoded_fps", "Encoded FPS", 1.0, "FPS"),
                        ("sent_fps", "Sent FPS", 1.0, "FPS"),
                    ] { sender_metric(ui, sample, key, label, divisor, unit); }
                    let requested = sample["metrics"]["requested_bps"].as_f64();
                    for (key,label) in [("watcher_cap_bps","Watcher cap"),("upload_cap_bps","Upload cap"),("congestion_bps","Congestion estimate")] {
                        if let (Some(requested),Some(cap)) = (requested,sample["metrics"][key].as_f64()) && cap < requested {
                            metric(ui, "Limiting input", tr(label));
                        }
                    }
                    ui.add_space(6.0);
                    egui::CollapsingHeader::new(tr("Encoder and transport details")).show(ui, |ui| {
                        metric(ui, "Sender window", &format!("{} ms · #{} · {}", sample["window_ms"], sample["sequence"], sample["codec"].as_str().unwrap_or("?")));
                        for (key, label, divisor, unit) in [
                            ("encoded_window_ms", "Encoder counter window", 1.0, "ms"),
                            ("capture_queue_frames", "Capture queue", 1.0, "frames"),
                            ("encoder_queue_frames", "Encoder input queue", 1.0, "frames"),
                            ("capture_queue_overruns", "Capture queue overruns", 1.0, "events/window"),
                            ("encoded_bps", "Encoded payload", 1e6, "Mbps"),
                            ("upload_cap_bps", "Upload cap", 1e6, "Mbps"),
                            ("watcher_cap_bps", "Watcher cap", 1e6, "Mbps"),
                            ("capture_fps", "Capture output FPS", 1.0, "FPS"),
                            ("encoder_input_fps", "Encoder input FPS", 1.0, "FPS"),
                            ("congestion_bps", "Congestion estimate", 1e6, "Mbps"),
                            ("accepted_fps", "SDK accepted FPS", 1.0, "FPS"),
                            ("handoff_rejections", "Handoff skips or rejections", 1.0, "frames"),
                            ("encoder_reconfigurations", "Encoder target changes", 1.0, "events"),
                            ("rtt_ms", "Transport RTT (not end-to-end)", 1.0, "ms"),
                        ] { sender_metric(ui, sample, key, label, divisor, unit); }
                        for (label, width, height) in [("Source", "source_width", "source_height"), ("Encoder input dimensions", "encoded_width", "encoded_height")] {
                            let dimensions = sample["metrics"][width].as_u64().zip(sample["metrics"][height].as_u64()).map(|(w,h)| format!("{w} × {h}")).unwrap_or_else(|| tr("Unavailable").into());
                            metric(ui, label, &dimensions);
                        }
                    });
                }
            } else { note(ui, "Sender statistics unavailable or stale"); }
            ui.add_space(12.0);
            ui.separator();
            note(ui, "UI submissions are not measured screen refresh. Physical presentation FPS unavailable.");
            egui::CollapsingHeader::new(tr("About these measurements")).show(ui, |ui| {
                for text in [
                    "Local window: 3 s; sender window and age below. Clocks are independent.",
                    "SDK counters: omitted zero/default fields are unavailable or no reported events.",
                    "Caps explain configured limits; congestion and encoder ramp may also lower the target.",
                    "Unavailable: capture drops, internal encoder timing/overload, visible crop, unrecognized decoder, render queue and physical end-to-end latency.",
                ] { note(ui, text); }
            });
        });
    state.open.store(open, Ordering::Relaxed);
}

fn section(ui: &mut eframe::egui::Ui, label: &str) {
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(4.0);
    ui.label(crate::ui_style::strong(crate::locale::tr(label), 15.0).color(crate::ui_style::MINT));
}

fn note(ui: &mut eframe::egui::Ui, text: &str) {
    ui.label(
        eframe::egui::RichText::new(crate::locale::tr(text))
            .size(12.0)
            .color(crate::ui_style::SECONDARY),
    );
}

fn metric(ui: &mut eframe::egui::Ui, label: &str, value: &str) {
    use eframe::egui;
    let width = ui.available_width();
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 20.0;
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(width * 0.60, 20.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_width(width * 0.60);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(crate::locale::tr(label))
                                .size(12.0)
                                .color(crate::ui_style::SECONDARY),
                        )
                        .wrap(),
                    );
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::Label::new(crate::ui_style::strong(
                        if value.is_empty() {
                            crate::locale::tr("Unavailable")
                        } else {
                            value
                        },
                        14.0,
                    ))
                    .wrap(),
                );
            });
        });
    });
}

fn sender_metric(
    ui: &mut eframe::egui::Ui,
    sample: &Value,
    key: &str,
    label: &str,
    divisor: f64,
    unit: &str,
) {
    let value = sample["metrics"][key]
        .as_f64()
        .map(|n| format!("{:.2} {unit}", n / divisor))
        .unwrap_or_else(|| crate::locale::tr("Unavailable").into());
    metric(ui, label, &value);
}
