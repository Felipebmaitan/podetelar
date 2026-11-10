use crate::audio;
use crate::connection::Route;
use crate::publisher::{Command, Controller};
use crate::quality::{self, Bitrate, Profile, Settings};
use eframe::egui;
use std::time::Duration;

pub struct PublisherUi {
    controller: Controller,
    invitation: String,
    name: String,
    source_id: String,
    encoder: String,
    audio: audio::Choice,
    exclusions: Vec<String>,
    quality: Settings,
    route: Route,
    notices: Option<std::path::PathBuf>,
    notice_error: bool,
}

impl PublisherUi {
    pub fn new() -> Self {
        Self {
            controller: Controller::new(),
            invitation: String::new(),
            name: String::new(),
            source_id: String::new(),
            encoder: String::new(),
            audio: audio::Choice::Off,
            exclusions: Vec::new(),
            quality: Settings::default(),
            route: Route::Auto,
            notices: std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent()?.parent().map(|path| path.join("notices")))
                .filter(|path| path.join("README.md").is_file()),
            notice_error: false,
        }
    }
}

impl eframe::App for PublisherUi {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        ctx.request_repaint_after(Duration::from_millis(500));
        let state = self.controller.snapshot();
        if self.source_id.is_empty() {
            self.source_id = state
                .sources
                .first()
                .map(|source| source.id.clone())
                .unwrap_or_default();
        }
        if self.encoder.is_empty() {
            self.encoder = state
                .encoders
                .first()
                .map(|encoder| encoder.factory.clone())
                .unwrap_or_default();
        }
        egui::CentralPanel::default().show(root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("FreedomStream");
                ui.label("Share a screen or game. Watch friends in your private room.");
                #[cfg(windows)]
                if let Some(notices) = &self.notices
                    && ui.small_button("Licenses and library sources").clicked()
                {
                    self.notice_error = std::env::var_os("WINDIR")
                        .map(std::path::PathBuf::from)
                        .is_none_or(|windows| std::process::Command::new(windows.join("explorer.exe"))
                            .arg(notices).spawn().is_err());
                }
                if self.notice_error {
                    ui.label("Could not open the notices folder. It is beside the bundled runtime.");
                }
                ui.add_space(16.0);
                if state.connection != "connected" && state.connection != "reconnecting" {
                    ui.label("Room invitation");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.invitation)
                            .interactive(!state.busy)
                            .char_limit(2048)
                            .desired_width(f32::INFINITY)
                            .hint_text("Paste your private room link"),
                    );
                    ui.label("Your name");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.name)
                            .interactive(!state.busy)
                            .desired_width(260.0)
                            .char_limit(60),
                    );
                    ui.add_enabled_ui(!state.busy, |ui| {
                        egui::ComboBox::from_label("Connection route").selected_text(if self.route == Route::Auto {"Automatic"} else {"Relay only"}).show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.route, Route::Auto, "Automatic");
                            ui.selectable_value(&mut self.route, Route::Relay, "Relay only (restrictive networks)");
                        });
                    });
                    if ui
                        .add_enabled(
                            !state.busy
                                && !self.invitation.trim().is_empty()
                                && !self.name.trim().is_empty(),
                            egui::Button::new("Join room"),
                        )
                        .clicked()
                    {
                        self.invitation = self.invitation.trim().to_owned();
                        self.controller.send(Command::Join {
                            invitation: self.invitation.clone(),
                            name: self.name.trim().to_owned(),
                            route:self.route,
                        });
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.strong(&state.room_name);
                        ui.label(state.connection);
                        if ui
                            .add_enabled(!state.busy, egui::Button::new("Leave room"))
                            .clicked()
                        {
                            self.controller.send(Command::Leave);
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Copy room invitation").clicked() {
                            ctx.copy_text(self.invitation.clone());
                        }
                        if ui
                            .add_enabled(!state.busy, egui::Button::new("Watch friends"))
                            .clicked()
                        {
                            self.controller.send(Command::OpenViewer {
                                invitation: self.invitation.clone(),
                            });
                        }
                    });
                }
                ui.separator();
                ui.add_space(10.0);
                ui.add_enabled_ui(!state.busy, |ui| {
                    ui.label("Capture source");
                    egui::ComboBox::from_id_salt("capture-source")
                        .width(600.0)
                        .selected_text(
                            state
                                .sources
                                .iter()
                                .find(|source| source.id == self.source_id)
                                .map(|source| source.label.as_str())
                                .unwrap_or("Select an available source"),
                        )
                        .show_ui(ui, |ui| {
                            for source in &state.sources {
                                ui.selectable_value(
                                    &mut self.source_id,
                                    source.id.clone(),
                                    &source.label,
                                );
                            }
                        });
                    ui.add_space(10.0);
                    ui.label("Hardware encoder");
                    egui::ComboBox::from_id_salt("hardware-encoder")
                        .width(600.0)
                        .selected_text(
                            state
                                .encoders
                                .iter()
                                .find(|encoder| encoder.factory == self.encoder)
                                .map(|encoder| encoder.label.as_str())
                                .unwrap_or("No supported hardware encoder"),
                        )
                        .show_ui(ui, |ui| {
                            for encoder in &state.encoders {
                                ui.selectable_value(
                                    &mut self.encoder,
                                    encoder.factory.clone(),
                                    &encoder.label,
                                );
                            }
                        });
                    if let Some(encoder) = state
                        .encoders
                        .iter()
                        .find(|encoder| encoder.factory == self.encoder)
                    {
                        ui.label(encoder.codec.label());
                        quality_controls(ui,&mut self.quality,state.sources.iter().find(|source|source.id==self.source_id),encoder);
                    }
                    if ui.button("Refresh sources and encoders").clicked() {
                        self.controller.send(Command::Refresh);
                    }
                    ui.add_space(10.0);
                    ui.label("Stream audio");
                    let audio_label = match &self.audio {
                        audio::Choice::Off => "Off",
                        audio::Choice::Desktop => "Desktop application mix",
                        audio::Choice::Application(id) => state
                            .applications
                            .iter()
                            .find(|app| &app.id == id)
                            .map(|app| app.label.as_str())
                            .unwrap_or("Selected application (waiting for audio)"),
                    };
                    egui::ComboBox::from_id_salt("stream-audio")
                        .width(600.0)
                        .selected_text(audio_label)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.audio, audio::Choice::Off, "Off");
                            ui.selectable_value(
                                &mut self.audio,
                                audio::Choice::Desktop,
                                "Desktop application mix",
                            );
                            for app in &state.applications {
                                ui.selectable_value(
                                    &mut self.audio,
                                    audio::Choice::Application(app.id.clone()),
                                    &app.label,
                                );
                            }
                        });
                    ui.label(
                        "Discord and this app's viewing window are excluded. Microphone stays off.",
                    );
                    ui.collapsing("Exclude more applications", |ui| {
                        ui.label("Watching in your regular browser? Exclude that browser here.");
                        for app in &state.applications {
                            let mut excluded = self.exclusions.contains(&app.id);
                            if ui.checkbox(&mut excluded, &app.label).changed() {
                                self.exclusions.retain(|id| id != &app.id);
                                if excluded {
                                    self.exclusions.push(app.id.clone());
                                }
                            }
                        }
                    });
                });
                ui.add_space(12.0);
                ui.label("Choose hardware H.264 if a friend's browser cannot receive HEVC.");
                ui.add_space(12.0);
                if state.sharing || state.capture_pending {
                    // Stop remains available during acquisition, publication and reconnect.
                    if ui.button("Stop sharing").clicked() {
                        self.controller.send(Command::Stop);
                    }
                    if ui.add_enabled(state.sharing && !state.busy,egui::Button::new("Apply settings")).clicked() {
                        self.controller.send(Command::Apply { source_id:self.source_id.clone(),encoder:self.encoder.clone(),quality:self.quality,audio:self.audio.clone(),exclusions:self.exclusions.clone() });
                    }
                    ui.small("Bitrate applies live. Other changes briefly restart sharing; friends choose Watch again.");
                } else if ui
                    .add_enabled(
                        !state.busy
                            && state.connection == "connected"
                            && !self.source_id.is_empty()
                            && !self.encoder.is_empty(),
                        egui::Button::new("Start sharing"),
                    )
                    .clicked()
                {
                    self.controller.send(Command::Start {
                        source_id: self.source_id.clone(),
                        encoder: self.encoder.clone(),
                        audio: self.audio.clone(),
                        exclusions: self.exclusions.clone(),
                        quality:self.quality,
                    });
                }
                if state.busy {
                    ui.spinner();
                }
                if state.sharing {
                    if let Some(mode)=state.mode { ui.label(format!("Requested · {}×{} · {} fps · {:.0} Mbps",mode.width,mode.height,mode.fps,mode.bitrate as f64/1_000_000.0)); }
                    ui.label(format!(
                        "Encoder · {:.1} fps · {:.1} Mbps output · {:.1} Mbps effective target · {} frames submitted",
                        state.metrics.fps,
                        state.metrics.encoded_bps as f64/1_000_000.0,
                        state.metrics.encoder_bitrate as f64 / 1_000_000.0,
                        state.metrics.accepted
                    ));
                    if state.delivery.available {
                        let rtt = state.delivery.rtt_ms.map_or_else(|| "RTT unavailable".to_owned(), |ms| format!("{ms:.0} ms RTT"));
                        ui.label(format!("Sent to server · {:.1} Mbps · {:.2} Mbps retransmissions · {} · {}",state.delivery.bitrate_bps as f64/1_000_000.0,state.delivery.retransmit_bps as f64/1_000_000.0,state.delivery.route.unwrap_or("unknown"),rtt));
                    }
                    ui.small("Congestion can lower the effective bitrate. Resolution and FPS stay as selected; receiver quality depends on its connection and decoder.");
                    if self.audio != audio::Choice::Off {
                        ui.label(format!(
                            "Audio · {} applications · peak {:.0}%",
                            state.audio.applications,
                            state.audio.peak * 100.0
                        ));
                    }
                }
                if !state.error.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(240, 130, 130), &state.error);
                }
            });
        });
    }
}

fn quality_controls(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    source: Option<&crate::sources::SourceInfo>,
    encoder: &crate::encoding::Encoder,
) {
    ui.add_space(10.0);
    ui.label("Video quality");
    egui::ComboBox::from_id_salt("resolution")
        .selected_text(settings.resolution.label())
        .show_ui(ui, |ui| {
            for resolution in quality::Resolution::ALL {
                let candidate = Settings {
                    resolution,
                    ..*settings
                };
                let mode = source.and_then(|source| candidate.resolve(source, encoder.codec).ok());
                let label = mode
                    .map(|mode| format!("{} · {}×{}", resolution.label(), mode.width, mode.height))
                    .unwrap_or_else(|| {
                        format!("{} · unavailable for this source", resolution.label())
                    });
                ui.add_enabled_ui(mode.is_some_and(|mode| encoder.supports(mode)), |ui| {
                    ui.selectable_value(&mut settings.resolution, resolution, label);
                });
            }
        });
    egui::ComboBox::from_id_salt("frame-rate")
        .selected_text(format!("{} fps", settings.fps))
        .show_ui(ui, |ui| {
            for fps in quality::FRAME_RATES {
                let available = source
                    .and_then(|source| {
                        Settings { fps, ..*settings }
                            .resolve(source, encoder.codec)
                            .ok()
                    })
                    .is_some_and(|mode| encoder.supports(mode));
                ui.add_enabled_ui(available, |ui| {
                    ui.selectable_value(&mut settings.fps, fps, format!("{fps} fps"));
                });
            }
        });
    egui::ComboBox::from_id_salt("content-profile")
        .selected_text(settings.profile.label())
        .show_ui(ui, |ui| {
            for profile in [Profile::Motion, Profile::Detail] {
                ui.selectable_value(&mut settings.profile, profile, profile.label());
            }
        });
    let recommended = source
        .and_then(|source| {
            Settings {
                bitrate: Bitrate::Auto,
                ..*settings
            }
            .resolve(source, encoder.codec)
            .ok()
        })
        .map(|mode| format!("Auto · {} Mbps", mode.bitrate / 1_000_000))
        .unwrap_or_else(|| "Auto".into());
    let label = match settings.bitrate {
        Bitrate::Auto => recommended.clone(),
        Bitrate::Manual(bps) => format!("Manual · {} Mbps", bps / 1_000_000),
    };
    egui::ComboBox::from_id_salt("bitrate")
        .selected_text(label)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut settings.bitrate, Bitrate::Auto, recommended);
            for mbps in quality::BITRATE_PRESETS {
                ui.selectable_value(
                    &mut settings.bitrate,
                    Bitrate::Manual(mbps * 1_000_000),
                    format!("{mbps} Mbps"),
                );
            }
        });
    if let Bitrate::Manual(bps) = settings.bitrate {
        let mut mbps = bps / 1_000_000;
        ui.horizontal(|ui| {
            ui.label("Manual Mbps");
            if ui
                .add(egui::DragValue::new(&mut mbps).range(3..=100))
                .changed()
            {
                settings.bitrate = Bitrate::Manual(mbps * 1_000_000);
            }
        });
    }
    ui.small("Auto follows resolution, FPS, codec and content, capped at 35 Mbps for group use. Manual supports 3–100 Mbps.");
    if let Some(source) = source
        && let Err(error) = settings.resolve(source, encoder.codec)
    {
        ui.colored_label(egui::Color32::LIGHT_RED, error.to_string());
    }
}
