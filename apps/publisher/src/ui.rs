use crate::{
    connection::Route,
    playback::Tile,
    publisher::{Command, Controller, Snapshot},
    quality::{self, Bitrate, Profile, Settings},
};
use eframe::egui;
use std::{collections::HashMap, sync::atomic::Ordering, time::Duration};

const ACCENT: egui::Color32 = egui::Color32::from_rgb(67, 205, 177);

pub struct PublisherUi {
    controller: Controller,
    invitation: String,
    name: String,
    source_id: String,
    encoder: String,
    quality: Settings,
    route: Route,
    download_budget: Option<u64>,
    textures: HashMap<String, egui::TextureHandle>,
    focused: Option<String>,
    store: Option<crate::settings::Store>,
    attempted_profile: Option<crate::settings::Profile>,
    settings_error: String,
    forgetting: bool,
}
impl PublisherUi {
    pub fn new() -> Self {
        let store = crate::settings::Store::open();
        let mut settings_error = String::new();
        let store = store
            .map_err(|_| settings_error = "Saved room storage is unavailable.".into())
            .ok();
        let profile = store.as_ref().and_then(|store| match store.load() {
            Ok(profile) => profile,
            Err(_) => {
                settings_error = "Could not unlock the saved room. Paste your invitation to join again, or forget it below.".into();
                None
            }
        });
        let app = Self {
            controller: Controller::new(),
            invitation: profile
                .as_ref()
                .map(|p| p.invitation.clone())
                .unwrap_or_default(),
            name: profile.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
            source_id: String::new(),
            encoder: String::new(),
            quality: Settings::default(),
            route: Route::Auto,
            download_budget: None,
            textures: HashMap::new(),
            focused: None,
            store,
            attempted_profile: profile.clone(),
            settings_error,
            forgetting: false,
        };
        if let Some(profile) = profile {
            app.controller.send(Command::Join {
                invitation: profile.invitation,
                name: profile.name,
                route: app.route,
            });
        }
        app
    }

    fn sharing_menu(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        ui.heading("Share your screen");
        ui.add_space(12.0);
        let connected = matches!(state.connection, "connected" | "reconnecting");
        if !connected {
            ui.label("Room invitation");
            ui.add(
                egui::TextEdit::singleline(&mut self.invitation)
                    .interactive(!state.busy && state.connection != "connecting")
                    .char_limit(2048)
                    .desired_width(f32::INFINITY)
                    .hint_text("Paste your private room link"),
            );
            ui.label("Your name");
            ui.add(
                egui::TextEdit::singleline(&mut self.name)
                    .interactive(!state.busy && state.connection != "connecting")
                    .char_limit(60)
                    .desired_width(f32::INFINITY),
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
                self.forgetting = false;
                self.controller.send(Command::Join {
                    invitation: self.invitation.trim().to_owned(),
                    name: self.name.trim().to_owned(),
                    route: self.route,
                });
            }
            ui.add_space(12.0);
            ui.small("Your last joined room and name are saved securely for the next launch.");
            ui.separator();
        }
        ui.add_enabled_ui(!state.busy, |ui| {
            ui.label("Source");
            egui::ComboBox::from_id_salt("capture-source").width(ui.available_width()).truncate()
                .selected_text(state.sources.iter().find(|s| s.id == self.source_id).map(|s| s.label.as_str()).unwrap_or("Choose a screen or window"))
                .show_ui(ui, |ui| { for source in &state.sources { ui.selectable_value(&mut self.source_id, source.id.clone(), &source.label); } });
            ui.add_space(12.0);
            if let Some(encoder) = state.encoders.iter().find(|e| e.factory == self.encoder) {
                quality_controls(ui, &mut self.quality, state.sources.iter().find(|s| s.id == self.source_id), encoder);
            }
            ui.add_space(10.0);
            ui.small("Audio follows your source. Discord and FreedomStream playback stay out of the mix. Microphone stays off.");
        });
        ui.add_space(12.0);
        if state.sharing || state.capture_pending {
            if ui
                .add_sized(
                    [ui.available_width(), 36.0],
                    egui::Button::new("Stop sharing"),
                )
                .clicked()
            {
                self.controller.send(Command::Stop);
            }
            if ui
                .add_enabled(
                    state.sharing && !state.busy,
                    egui::Button::new("Apply settings"),
                )
                .clicked()
            {
                self.send_settings(true);
            }
            if let Some(mode) = state.mode {
                ui.small(format!(
                    "Sharing {}×{} · {} FPS",
                    mode.width, mode.height, mode.fps
                ));
            }
        } else {
            let valid = state
                .encoders
                .iter()
                .find(|e| e.factory == self.encoder)
                .and_then(|e| {
                    state
                        .sources
                        .iter()
                        .find(|s| s.id == self.source_id)
                        .and_then(|s| {
                            self.quality
                                .resolve(s, e.codec)
                                .ok()
                                .map(|mode| e.supports(mode))
                        })
                })
                .unwrap_or(false);
            if ui
                .add_enabled(
                    connected && !state.busy && valid,
                    egui::Button::new(
                        egui::RichText::new("Start sharing").color(egui::Color32::BLACK),
                    )
                    .fill(ACCENT)
                    .min_size(egui::vec2(ui.available_width(), 36.0)),
                )
                .clicked()
            {
                self.send_settings(false);
            }
        }
        if state.busy {
            ui.spinner();
        }
        ui.add_space(14.0);
        ui.separator();
        ui.collapsing("Advanced", |ui| {
            ui.add_enabled_ui(!state.busy, |ui| {
                ui.label("Codec / hardware encoder");
                egui::ComboBox::from_id_salt("hardware-encoder").width(ui.available_width()).truncate()
                    .selected_text(state.encoders.iter().find(|e| e.factory == self.encoder).map(|e| e.label.as_str()).unwrap_or("No supported hardware encoder"))
                    .show_ui(ui, |ui| { for encoder in &state.encoders { ui.selectable_value(&mut self.encoder, encoder.factory.clone(), &encoder.label); } });
                if let Some(encoder) = state.encoders.iter().find(|e| e.factory == self.encoder) { ui.small(encoder.codec.label()); }
                ui.small("HEVC switches to H.264 when a watcher needs compatibility.");
                bitrate_controls(ui, &mut self.quality);
                ui.add_space(12.0);
                let before = self.download_budget;
                egui::ComboBox::from_label("Watching limit").selected_text(self.download_budget.map(|bps| format!("{} Mbps",bps/1_000_000)).unwrap_or_else(|| "Automatic".into())).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.download_budget,None,"Automatic");
                    for mbps in [1, 3, 5, 10, 25, 50, 100] { ui.selectable_value(&mut self.download_budget,Some(mbps * 1_000_000),format!("{mbps} Mbps")); }
                });
                if before != self.download_budget { self.controller.send(Command::DownloadBudget { bps: self.download_budget }); }
                ui.small("Shared across everything you watch, with headroom. Automatic starts with a 12 Mbps allowance and adapts to receiver feedback; this is not a speed test.");
                ui.small("One encoding is shared: a slower watcher's limit can reduce quality for everyone watching that stream.");
                if ui.button("Retry watching connection").clicked() { self.controller.send(Command::DownloadBudget { bps: self.download_budget }); }
                egui::ComboBox::from_label("Route").selected_text(if self.route == Route::Auto { "Automatic" } else { "Relay only" }).show_ui(ui, |ui| { ui.selectable_value(&mut self.route, Route::Auto, "Automatic"); ui.selectable_value(&mut self.route, Route::Relay, "Relay only"); });
                ui.small("Route applies next time you join.");
                if ui.button("Refresh sources").clicked() { self.controller.send(Command::Refresh); }
            });
            if state.sharing { ui.small(format!("Encoded {:.1} FPS · {:.1} Mbps · sent {:.1} Mbps", state.metrics.fps, state.metrics.encoded_bps as f64/1_000_000.0,state.delivery.bitrate_bps as f64/1_000_000.0)); }
            ui.add_space(12.0);
            if ui.add_enabled(!state.busy && self.store.is_some(), egui::Button::new("Forget saved room")).clicked() {
                match self.store.as_ref().unwrap().forget() {
                    Ok(()) => {
                        self.forgetting = true;
                        self.controller.send(Command::Leave);
                        self.invitation.clear();
                        self.name.clear();
                        self.attempted_profile = None;
                        self.settings_error.clear();
                        self.textures.clear();
                    }
                    Err(_) => self.settings_error = "Could not forget the saved room. Try again.".into(),
                }
            }
            ui.small("Forgetting also leaves this room.");
            if let Some(notices) = std::env::current_exe().ok()
                .and_then(|p| p.parent()?.parent().map(|p| p.join("notices")))
                .filter(|p| p.is_dir())
                && ui.button("Licenses and library sources").clicked()
                && let Some(windows) = std::env::var_os("WINDIR")
            {
                let _ = std::process::Command::new(std::path::PathBuf::from(windows).join("explorer.exe"))
                    .arg(notices).spawn();
            }
        });
        if !self.settings_error.is_empty() {
            ui.colored_label(egui::Color32::LIGHT_RED, &self.settings_error);
        }
        if !state.error.is_empty() {
            ui.add_space(10.0);
            ui.colored_label(egui::Color32::LIGHT_RED, &state.error);
        }
    }

    fn send_settings(&self, apply: bool) {
        let command = if apply {
            Command::Apply {
                source_id: self.source_id.clone(),
                encoder: self.encoder.clone(),
                quality: self.quality,
                audio: crate::audio::Choice::Off,
                exclusions: Vec::new(),
            }
        } else {
            Command::Start {
                source_id: self.source_id.clone(),
                encoder: self.encoder.clone(),
                quality: self.quality,
                audio: crate::audio::Choice::Off,
                exclusions: Vec::new(),
            }
        };
        self.controller.send(command);
    }

    fn stream_tile(&mut self, ui: &mut egui::Ui, tile: &Tile, width: f32) {
        egui::Frame::group(ui.style())
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_width((width - 20.0).max(180.0));
                ui.add(egui::Label::new(egui::RichText::new(&tile.name).strong()).truncate());
                ui.add_space(6.0);
                if let Ok(mut picture) = tile.picture.lock()
                    && let Some(image) = picture.take()
                {
                    if let Some(texture) = self.textures.get_mut(&tile.id) {
                        texture.set(image, egui::TextureOptions::LINEAR);
                    } else {
                        self.textures.insert(
                            tile.id.clone(),
                            ui.ctx()
                                .load_texture(&tile.id, image, egui::TextureOptions::LINEAR),
                        );
                    }
                }
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), ui.available_width() * 9.0 / 16.0),
                    egui::Sense::hover(),
                );
                ui.painter()
                    .rect_filled(rect, 6.0, egui::Color32::from_rgb(13, 16, 23));
                if let Some(texture) = self.textures.get(&tile.id) {
                    let aspect = texture.aspect_ratio();
                    let size = egui::vec2(
                        rect.width().min(rect.height() * aspect),
                        rect.height().min(rect.width() / aspect),
                    );
                    let image_rect = egui::Rect::from_center_size(rect.center(), size);
                    ui.painter().image(
                        texture.id(),
                        image_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        if tile.watching {
                            "Connecting…"
                        } else {
                            "Preview loading…"
                        },
                        egui::FontId::proportional(14.0),
                        egui::Color32::GRAY,
                    );
                }
                if !tile.watching {
                    let button =
                        egui::Rect::from_center_size(rect.center(), egui::vec2(126.0, 44.0));
                    if ui
                        .put(
                            button,
                            egui::Button::new(
                                egui::RichText::new("▶ Watch")
                                    .size(18.0)
                                    .strong()
                                    .color(egui::Color32::BLACK),
                            )
                            .fill(ACCENT)
                            .corner_radius(22.0),
                        )
                        .clicked()
                    {
                        self.controller.send(Command::Watch {
                            id: tile.id.clone(),
                            enabled: true,
                        });
                    }
                } else {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Stop watching").clicked() {
                            self.controller.send(Command::Watch {
                                id: tile.id.clone(),
                                enabled: false,
                            });
                            self.textures.remove(&tile.id);
                        }
                        if ui
                            .button(if self.focused.is_some() {
                                "Back to grid"
                            } else {
                                "Focus"
                            })
                            .clicked()
                        {
                            self.focused = if self.focused.is_some() {
                                None
                            } else {
                                Some(tile.id.clone())
                            };
                        }
                    });
                    let mut volume = tile.volume.load(Ordering::Relaxed);
                    if ui
                        .add(egui::Slider::new(&mut volume, 0..=100).text("Volume"))
                        .changed()
                    {
                        tile.volume.store(volume, Ordering::Relaxed);
                    }
                    ui.collapsing("Playback details", |ui| {
                        ui.small(format!(
                            "{} · {} decoded frames",
                            tile.codec,
                            tile.frames.load(Ordering::Relaxed)
                        ));
                    });
                }
                if !tile.status.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(235, 189, 105), &tile.status);
                }
            });
    }
}

impl eframe::App for PublisherUi {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        ctx.request_repaint_after(Duration::from_millis(16));
        let state = self.controller.snapshot();
        if state.connection == "connected"
            && !self.forgetting
            && let Ok(profile) = crate::settings::Profile::new(&self.invitation, &self.name)
            && self.attempted_profile.as_ref() != Some(&profile)
        {
            // Attempt once per successful choice, rather than writing every frame.
            self.attempted_profile = Some(profile.clone());
            if let Some(store) = &self.store {
                self.settings_error = match store.save(&profile) {
                    Ok(()) => String::new(),
                    Err(_) => "Joined, but could not save this room for the next launch.".into(),
                };
            }
        }
        if self.source_id.is_empty() {
            self.source_id = state
                .sources
                .first()
                .map(|s| s.id.clone())
                .unwrap_or_default();
        }
        if self.encoder.is_empty() {
            self.encoder = state
                .encoders
                .first()
                .map(|e| e.factory.clone())
                .unwrap_or_default();
        }
        self.textures
            .retain(|id, _| state.tiles.iter().any(|t| &t.id == id));
        if !state
            .tiles
            .iter()
            .any(|t| Some(&t.id) == self.focused.as_ref())
        {
            self.focused = None;
        }
        egui::Panel::top("room-header")
            .exact_size(48.0)
            .show(root, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.heading("FreedomStream");
                    ui.separator();
                    ui.label(if state.room_name.is_empty() {
                        "Your screen. Your friends."
                    } else {
                        &state.room_name
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if matches!(state.connection, "connected" | "reconnecting") {
                            if ui
                                .add_enabled(!state.busy, egui::Button::new("Leave room"))
                                .clicked()
                            {
                                self.controller.send(Command::Leave);
                                self.textures.clear();
                            }
                            if ui.button("Copy invitation").clicked() {
                                ctx.copy_text(self.invitation.clone());
                            }
                            ui.small(state.connection);
                        }
                    });
                });
            });
        egui::Panel::left("sharing-menu")
            .default_size(294.0)
            .min_size(260.0)
            .max_size(380.0)
            .resizable(true)
            .show(root, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.sharing_menu(ui, &state));
            });
        egui::CentralPanel::default().show(root, |ui| {
            ui.heading("Live streams");
            ui.add_space(12.0);
            if state.tiles.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label("Friends' streams appear here. Join a room and choose what to watch.");
                });
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                let tiles: Vec<_> = state
                    .tiles
                    .iter()
                    .filter(|t| self.focused.as_ref().is_none_or(|id| id == &t.id))
                    .collect();
                let columns = if tiles.len() > 1 && ui.available_width() >= 680.0 {
                    2
                } else {
                    1
                };
                let width = (ui.available_width() - 16.0 * (columns - 1) as f32) / columns as f32;
                egui::Grid::new("stream-grid")
                    .num_columns(columns)
                    .spacing(egui::vec2(16.0, 16.0))
                    .show(ui, |ui| {
                        for (index, tile) in tiles.iter().enumerate() {
                            self.stream_tile(ui, tile, width);
                            if (index + 1) % columns == 0 {
                                ui.end_row();
                            }
                        }
                    });
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
    ui.label("Resolution");
    egui::ComboBox::from_id_salt("resolution")
        .selected_text(settings.resolution.label())
        .show_ui(ui, |ui| {
            for resolution in quality::Resolution::ALL {
                let candidate = Settings {
                    resolution,
                    ..*settings
                };
                let mode = source.and_then(|s| candidate.resolve(s, encoder.codec).ok());
                let label = mode
                    .map(|m| format!("{} · {}×{}", resolution.label(), m.width, m.height))
                    .unwrap_or_else(|| resolution.label().into());
                ui.add_enabled_ui(mode.is_some_and(|m| encoder.supports(m)), |ui| {
                    ui.selectable_value(&mut settings.resolution, resolution, label);
                });
            }
        });
    let mut index = quality::FRAME_RATES
        .iter()
        .position(|fps| *fps == settings.fps)
        .unwrap_or(3);
    if ui
        .add(
            egui::Slider::new(&mut index, 0..=quality::FRAME_RATES.len() - 1)
                .text("FPS")
                .custom_formatter(|i, _| {
                    format!("{} fps", quality::FRAME_RATES[i.round() as usize])
                }),
        )
        .changed()
    {
        settings.fps = quality::FRAME_RATES[index];
    }
    egui::ComboBox::from_label("Preset")
        .selected_text(settings.profile.label())
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut settings.profile,
                Profile::Motion,
                Profile::Motion.label(),
            );
            ui.selectable_value(
                &mut settings.profile,
                Profile::Detail,
                Profile::Detail.label(),
            );
            ui.selectable_value(
                &mut settings.profile,
                Profile::Saver,
                Profile::Saver.label(),
            );
        });
    if let Some(source) = source {
        match settings.resolve(source, encoder.codec) {
            Ok(mode) if encoder.supports(mode) => {
                ui.small(format!(
                    "{}×{} · {} fps · SDR",
                    mode.width, mode.height, mode.fps
                ));
            }
            _ => {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    "This source / encoder cannot use that resolution and frame rate.",
                );
            }
        }
    }
}
fn bitrate_controls(ui: &mut egui::Ui, settings: &mut Settings) {
    egui::ComboBox::from_label("Bitrate")
        .selected_text(match settings.bitrate {
            Bitrate::Auto => "Automatic".into(),
            Bitrate::Manual(bps) => format!("{} Mbps", bps / 1_000_000),
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut settings.bitrate, Bitrate::Auto, "Automatic");
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
        if ui
            .add(
                egui::DragValue::new(&mut mbps)
                    .range(1..=100)
                    .suffix(" Mbps"),
            )
            .changed()
        {
            settings.bitrate = Bitrate::Manual(mbps * 1_000_000);
        }
    }
    ui.small("Automatic uses resolution, FPS and content recommendations up to 35 Mbps. Manual supports 1–100 Mbps; connection limits still apply.");
}
