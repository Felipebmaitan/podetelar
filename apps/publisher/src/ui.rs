use crate::publisher::{Command, Controller};
use eframe::egui;
use std::time::Duration;

pub struct PublisherUi {
    controller: Controller,
    invitation: String,
    name: String,
    source_id: String,
    encoder: String,
}

impl PublisherUi {
    pub fn new() -> Self {
        Self {
            controller: Controller::new(),
            invitation: String::new(),
            name: String::new(),
            source_id: String::new(),
            encoder: String::new(),
        }
    }
}

impl eframe::App for PublisherUi {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        ctx.request_repaint_after(Duration::from_millis(500));
        let state = self.controller.snapshot();
        if !state
            .sources
            .iter()
            .any(|source| source.id == self.source_id)
        {
            self.source_id = state
                .sources
                .first()
                .map(|source| source.id.clone())
                .unwrap_or_default();
        }
        if !state
            .encoders
            .iter()
            .any(|encoder| encoder.factory == self.encoder)
        {
            self.encoder = state
                .encoders
                .first()
                .map(|encoder| encoder.factory.clone())
                .unwrap_or_default();
        }
        egui::CentralPanel::default().show(root, |ui| {
            ui.heading("FreedomStream");
            ui.label("Share a screen or game. Watch friends in your private room.");
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
                    if ui.button("Open room to watch").clicked() {
                        ctx.open_url(egui::OpenUrl::new_tab(self.invitation.clone()));
                    }
                });
            }
            ui.separator();
            ui.add_space(10.0);
            ui.add_enabled_ui(!state.sharing && !state.busy, |ui| {
                ui.label("Capture source");
                egui::ComboBox::from_id_salt("capture-source")
                    .width(600.0)
                    .selected_text(
                        state
                            .sources
                            .iter()
                            .find(|source| source.id == self.source_id)
                            .map(|source| source.label.as_str())
                            .unwrap_or("No source available"),
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
                    ui.label(encoder.codec.mime());
                }
                if ui.button("Refresh sources and encoders").clicked() {
                    self.controller.send(Command::Refresh);
                }
            });
            ui.add_space(12.0);
            ui.label("Video · up to 1080p60 · 12 Mbps target · Audio off");
            ui.label("Choose hardware H.264 if a friend's browser cannot receive HEVC.");
            ui.add_space(12.0);
            if state.sharing || state.capture_pending {
                // Stop remains available during acquisition, publication and reconnect.
                if ui.button("Stop sharing").clicked() {
                    self.controller.send(Command::Stop);
                }
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
                });
            }
            if state.busy {
                ui.spinner();
            }
            if state.sharing {
                ui.label(format!(
                    "{:.1} encoded fps · {:.1} Mbps encoder target · {} frames submitted",
                    state.metrics.fps,
                    state.metrics.encoder_bitrate as f64 / 1_000_000.0,
                    state.metrics.accepted
                ));
            }
            if !state.error.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(240, 130, 130), &state.error);
            }
        });
    }
}
