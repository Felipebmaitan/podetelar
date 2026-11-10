use crate::locale::tr;
use crate::{
    connection::Route,
    playback::Tile,
    publisher::{Command, Controller, SharingSelection, Snapshot},
    quality::{self, Bitrate, Profile, Settings},
};
use eframe::egui;
use std::{
    collections::HashMap,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

const ACCENT: egui::Color32 = egui::Color32::from_rgb(68, 216, 184);
const TEXT: egui::Color32 = egui::Color32::from_rgb(233, 239, 247);
const MUTED: egui::Color32 = egui::Color32::from_rgb(166, 179, 198);
const PANEL: egui::Color32 = egui::Color32::from_rgb(19, 25, 35);
const CANVAS: egui::Color32 = egui::Color32::from_rgb(13, 18, 27);
const CARD: egui::Color32 = egui::Color32::from_rgb(26, 34, 47);
const BORDER: egui::Color32 = egui::Color32::from_rgb(53, 66, 85);
const WARNING: egui::Color32 = egui::Color32::from_rgb(255, 205, 130);
const ERROR: egui::Color32 = egui::Color32::from_rgb(255, 165, 170);

pub fn configure_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.weak_text_color = Some(MUTED);
    style.visuals.panel_fill = PANEL;
    style.visuals.window_fill = CARD;
    style.visuals.extreme_bg_color = CANVAS;
    style.visuals.faint_bg_color = CARD;
    style.visuals.window_corner_radius = egui::CornerRadius::same(10);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(29, 103, 94);
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = egui::CornerRadius::same(6);
        widget.fg_stroke.color = TEXT;
        widget.bg_stroke = egui::Stroke::new(1.0, BORDER);
    }
    style.visuals.widgets.noninteractive.bg_fill = CARD;
    style.visuals.widgets.noninteractive.weak_bg_fill = CARD;
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(39, 50, 67);
    style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(39, 50, 67);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(51, 68, 89);
    style.visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(51, 68, 89);
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, ACCENT);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(33, 91, 85);
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.interact_size.y = 32.0;
    style.spacing.scroll = egui::style::ScrollStyle::solid();
    style.spacing.scroll.bar_width = 8.0;
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(21.0));
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    ctx.set_style_of(egui::Theme::Dark, style);
}

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
    fullscreen: Option<String>,
    fullscreen_controls_until: Instant,
    store: Option<crate::settings::Store>,
    attempted_profile: Option<crate::settings::Profile>,
    settings_error: String,
    forgetting: bool,
    copied_until: Option<Instant>,
    remembered_volume: HashMap<String, u32>,
    advanced_open: bool,
    show_invitation: bool,
    self_preview_revision: Option<(crate::preview::Latest, u64)>,
}
impl PublisherUi {
    pub fn new() -> Self {
        crate::locale::load();
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
            fullscreen: None,
            fullscreen_controls_until: Instant::now(),
            store,
            attempted_profile: profile.clone(),
            settings_error,
            forgetting: false,
            copied_until: None,
            remembered_volume: HashMap::new(),
            advanced_open: false,
            show_invitation: false,
            self_preview_revision: None,
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

    fn join(&mut self) {
        self.show_invitation = false;
        self.forgetting = false;
        self.controller.send(Command::Join {
            invitation: self.invitation.trim().to_owned(),
            name: self.name.trim().to_owned(),
            route: self.route,
        });
    }

    fn forget(&mut self) {
        let Some(store) = &self.store else {
            return;
        };
        match store.forget() {
            Ok(()) => {
                self.forgetting = true;
                self.controller.send(Command::Leave);
                self.invitation.clear();
                self.show_invitation = false;
                self.name.clear();
                self.attempted_profile = None;
                self.settings_error.clear();
                self.textures.clear();
            }
            Err(_) => self.settings_error = "Could not forget the saved room. Try again.".into(),
        }
    }

    fn room_header(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let connected = matches!(state.connection, "connected" | "reconnecting");
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("FreedomStream").size(20.0).strong());
            ui.separator();
            if connected {
                ui.label(egui::RichText::new(tr(if state.protected { "Invitation-encrypted" } else { "Transport encryption only" })).small().color(if state.protected { ACCENT } else { WARNING }));
            }
            ui.menu_button(
                format!(
                    "{} {}",
                    if connected {
                        state.participants.len() + 1
                    } else {
                        0
                    },
                    tr("in room")
                ),
                |ui| {
                    if connected {
                        ui.label(egui::RichText::new(tr("People in this room")).strong());
                        ui.label(format!("{} ({})", self.name, tr("you")));
                        for name in &state.participants {
                            ui.label(name);
                        }
                    } else {
                        ui.label(tr("Join a room to see its members."));
                    }
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut pt = crate::locale::portuguese();
                egui::ComboBox::from_id_salt("ui-language")
                    .selected_text(if pt { "PT-BR" } else { "EN" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut pt, true, "Português (Brasil)");
                        ui.selectable_value(&mut pt, false, "English");
                    });
                if pt != crate::locale::portuguese() && crate::locale::select(pt).is_err() {
                    self.settings_error = "Language preference could not be saved.".into();
                }
                if state.connection == "connecting" {
                    if ui.button(tr("Cancel join")).clicked() {
                        self.controller.send(Command::Leave);
                    }
                } else if connected && ui.button(tr("Leave room")).clicked() {
                    self.controller.send(Command::Leave);
                    self.textures.clear();
                }
                ui.menu_button(tr("Room options"), |ui| {
                    if connected && !state.protected {
                        ui.label(tr("Create a new private invitation in this app. Share it with your friends; old invitations remain unprotected."));
                        if ui.add_enabled(!state.busy, egui::Button::new(tr("Protect room / Create private invitation"))).clicked() {
                            match crate::protection::protect_invitation(&self.invitation) {
                                Ok(invitation) => {
                                    self.invitation = invitation;
                                    self.join();
                                    self.textures.clear();
                                    self.settings_error = "Private invitation created. Rejoining with encryption; copy and share the new invitation with your friends.".into();
                                }
                                Err(_) => self.settings_error = "Could not create a private invitation. Rejoin the room and try again.".into(),
                            }
                            ui.close();
                        }
                        ui.separator();
                    }
                    if connected && state.protected {
                        ui.label(tr("Only private invitation holders can decrypt streams. Names are not verified. Replacing the invitation requires every friend to use the new one; old holders still control the old encrypted group."));
                        if ui.add_enabled(!state.busy, egui::Button::new(tr("Replace private invitation"))).clicked() {
                            match crate::protection::replace_invitation(&self.invitation) {
                                Ok(invitation) => {
                                    self.invitation = invitation;
                                    self.join();
                                    self.textures.clear();
                                    self.settings_error = "Private invitation replaced. Copy and share the new invitation with remaining trusted friends.".into();
                                }
                                Err(_) => self.settings_error = "Could not replace the private invitation. Try again.".into(),
                            }
                            ui.close();
                        }
                        ui.separator();
                    }
                    if ui.button(tr("Advanced settings")).clicked() {
                        self.advanced_open = true;
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.store.is_some(),
                            egui::Button::new(tr("Forget saved room")),
                        )
                        .clicked()
                    {
                        self.forget();
                        ui.close();
                    }
                    ui.separator();
                    open_licenses(ui);
                });
                if connected {
                    let copied = self
                        .copied_until
                        .is_some_and(|until| Instant::now() < until);
                    if ui
                        .button(if copied {
                            tr("Copied!")
                        } else {
                            tr("Copy invitation")
                        })
                        .clicked()
                    {
                        ui.ctx().copy_text(self.invitation.clone());
                        self.copied_until = Some(Instant::now() + Duration::from_secs(2));
                    }
                }
            });
        });
    }

    fn join_form(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        ui.heading(tr("Join your room"));
        ui.label(
            egui::RichText::new(tr(
                "Paste an invitation from a friend, then choose your name.",
            ))
            .color(MUTED),
        );
        ui.add_space(8.0);
        let invitation_label = ui.horizontal(|ui| {
            let label = ui.label(tr("Room invitation"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(!state.busy, egui::Button::new(if self.show_invitation { tr("Hide") } else { tr("Show") })).on_hover_text(tr("Reveal this private invitation only when it is safe to show your screen")).clicked() {
                    self.show_invitation = !self.show_invitation;
                }
            });
            label
        }).inner;
        let invitation = ui
            .add(
                egui::TextEdit::singleline(&mut self.invitation)
                    .id_salt("room-invitation")
                    .password(!self.show_invitation)
                    .interactive(!state.busy)
                    .char_limit(2048)
                    .desired_width(f32::INFINITY)
                    .hint_text(tr("Paste your private invitation link")),
            )
            .labelled_by(invitation_label.id);
        let name_label = ui.label(tr("Your name"));
        let name = ui
            .add(
                egui::TextEdit::singleline(&mut self.name)
                    .id_salt("display-name")
                    .interactive(!state.busy)
                    .char_limit(60)
                    .desired_width(f32::INFINITY)
                    .hint_text(tr("How friends will see you")),
            )
            .labelled_by(name_label.id);
        let valid =
            !state.busy && !self.invitation.trim().is_empty() && !self.name.trim().is_empty();
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter))
            && (invitation.has_focus()
                || invitation.lost_focus()
                || name.has_focus()
                || name.lost_focus());
        if ui
            .add_enabled(
                valid,
                if valid {
                    primary(tr("Join room"), ui.available_width())
                } else {
                    egui::Button::new(egui::RichText::new(tr("Join room")).color(MUTED))
                        .fill(CARD)
                        .min_size(egui::vec2(ui.available_width(), 38.0))
                },
            )
            .clicked()
            || (valid && enter)
        {
            self.join();
        }
        ui.small(tr("Your last joined room and name are encrypted on this PC and remembered for next launch."));
        ui.add_space(8.0);
        ui.small(tr("Saved access can be removed in Room options above."));
    }

    fn selection(&self) -> SharingSelection {
        SharingSelection {
            source_id: self.source_id.clone(),
            encoder: self.encoder.clone(),
            quality: self.quality,
        }
    }

    fn sharing_menu(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        if !matches!(state.connection, "connected" | "reconnecting") {
            self.join_form(ui, state);
            return;
        }
        ui.horizontal(|ui| {
            ui.heading(tr("Share your screen"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(tr("Advanced")).clicked() {
                    self.advanced_open = true;
                }
            });
        });
        ui.spacing_mut().item_spacing.y = 6.0;
        let encoder = state.encoders.iter().find(|e| e.factory == self.encoder);
        let source = state.sources.iter().find(|s| s.id == self.source_id);
        ui.add_enabled_ui(!state.busy, |ui| {
            let label = ui.horizontal(|ui| {
                let label=ui.label(tr("Screen or window"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                    if ui.button(tr("Refresh")).on_hover_text(tr("Look for newly opened screens and windows")).clicked(){self.controller.send(Command::Refresh);}
                });
                label
            }).inner;
            egui::ComboBox::from_id_salt("capture-source").width(ui.available_width()).truncate()
                .selected_text(source.map(|s| s.label.as_str()).unwrap_or(tr("Choose a source")))
                .show_ui(ui, |ui| { for source in &state.sources { ui.selectable_value(&mut self.source_id, source.id.clone(), &source.label); } })
                .response.labelled_by(label.id);
            if let Some(encoder) = encoder { quality_controls(ui, &mut self.quality, source, encoder); }
            else { ui.colored_label(WARNING, tr("No supported hardware encoder was found. You can still watch compatible streams.")); }
        });
    }

    fn sharing_actions(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let valid = state
            .encoders
            .iter()
            .find(|e| e.factory == self.encoder)
            .zip(state.sources.iter().find(|s| s.id == self.source_id))
            .is_some_and(|(e, s)| {
                self.quality
                    .resolve(s, e.codec)
                    .is_ok_and(|mode| e.supports(mode))
            });
        ui.add_space(6.0);
        let note_width = (ui.available_width() - 16.0).max(200.0);
        egui::Frame::new()
            .fill(CANVAS)
            .inner_margin(8)
            .corner_radius(6)
            .show(ui, |ui| {
                ui.set_width(note_width);
                ui.label(
                    egui::RichText::new(tr("Source audio · microphone off"))
                        .strong()
                        .size(13.0),
                );
                ui.small(tr("Discord and FreedomStream excluded."));
            });
        if state.sharing || state.capture_pending {
            if let Some(mode) = state.mode {
                ui.colored_label(
                    ACCENT,
                    format!(
                        "{} {}×{} · {} FPS",
                        tr("Sharing"),
                        mode.width,
                        mode.height,
                        mode.fps
                    ),
                );
                ui.small(format!(
                    "{}: {:.0} FPS · {:.1} Mbps {}",
                    tr("Live"),
                    state.metrics.fps,
                    state.delivery.bitrate_bps as f64 / 1_000_000.0,
                    tr("sent")
                ));
            } else {
                ui.colored_label(WARNING, tr("Starting your stream…"));
            }
            let selection = self.selection();
            let dirty = state
                .active_selection
                .as_ref()
                .is_some_and(|active| active != &selection);
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 8.0) / 2.0;
                if ui
                    .add_sized(
                        [width, 38.0],
                        egui::Button::new(egui::RichText::new(tr("Stop sharing")).color(ERROR))
                            .fill(egui::Color32::from_rgb(65, 37, 46)),
                    )
                    .clicked()
                {
                    self.controller.send(Command::Stop);
                }
                let apply =
                    egui::Button::new(egui::RichText::new(tr("Apply changes")).color(if dirty {
                        CANVAS
                    } else {
                        MUTED
                    }))
                    .fill(if dirty { ACCENT } else { CARD })
                    .min_size(egui::vec2(width, 38.0));
                if ui
                    .add_enabled(state.sharing && !state.busy && valid && dirty, apply)
                    .clicked()
                {
                    self.send_settings(true);
                }
            });
            if dirty {
                let live = state.active_selection.as_ref().is_some_and(|a| {
                    a.source_id == selection.source_id
                        && a.encoder == selection.encoder
                        && a.quality.resolution == selection.quality.resolution
                        && a.quality.fps == selection.quality.fps
                });
                ui.colored_label(
                    WARNING,
                    if live {
                        tr("Changes pending · applies live")
                    } else {
                        tr("Changes pending · restarts stream")
                    },
                );
            }
        } else if ui
            .add_enabled(
                state.connection == "connected" && !state.busy && valid,
                primary(tr("Start sharing"), ui.available_width()),
            )
            .clicked()
        {
            self.send_settings(false);
        }
    }

    fn advanced(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        ui.label(tr("Connection measurement"));
        ui.small(tr("Capacity to this VPS · payload capped at 200 Mbps"));
        if state.testing_connection {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(tr("Measuring connection…"));
            });
        }
        if let Some(rate) = state.capacity.download_bps {
            ui.label(format!(
                "{}: {:.1} Mbps",
                tr("Download"),
                rate as f64 / 1_000_000.0
            ));
        }
        if let Some(rate) = state.capacity.upload_bps {
            ui.label(format!(
                "{}: {:.1} Mbps",
                tr("Upload"),
                rate as f64 / 1_000_000.0
            ));
        }
        if let Some(rate) = state.capacity.upload_video_cap() {
            ui.small(format!(
                "{}: {:.1} Mbps",
                tr("Recommended video ceiling"),
                rate as f64 / 1_000_000.0
            ));
        }
        if !state.capacity.error.is_empty() {
            ui.colored_label(WARNING, tr(&state.capacity.error));
            if state.capacity.download_bps.is_some() || state.capacity.upload_bps.is_some() {
                ui.small(tr("Incomplete test; showing last available measurements."));
            }
        }
        if ui
            .add_enabled(
                state.connection == "connected" && !state.testing_connection,
                egui::Button::new(tr("Test connection again")),
            )
            .clicked()
        {
            self.controller.send(Command::MeasureConnection);
        }
        ui.separator();
        ui.add_enabled_ui(!state.busy, |ui| {
            let label = ui.label(tr("Codec / hardware encoder"));
            egui::ComboBox::from_id_salt("hardware-encoder").width(ui.available_width()).truncate()
                .selected_text(state.encoders.iter().find(|e| e.factory == self.encoder).map(|e| e.label.as_str()).unwrap_or(tr("No supported encoder")))
                .show_ui(ui, |ui| { for encoder in &state.encoders { ui.selectable_value(&mut self.encoder, encoder.factory.clone(), &encoder.label); } }).response.labelled_by(label.id);
            if let Some(encoder) = state.encoders.iter().find(|e| e.factory == self.encoder) {
                ui.small(encoder.codec.label());
            }
            ui.small(tr("AV1 / HEVC switch to H.264 when a watcher needs compatibility."));
            bitrate_controls(ui, &mut self.quality);
            ui.separator();
            let before = self.download_budget;
            let label = ui.label(tr("Watching allowance"));
            egui::ComboBox::from_id_salt("watching-allowance").width(ui.available_width())
                .selected_text(self.download_budget.map(|bps| format!("{} Mbps",bps/1_000_000)).unwrap_or_else(|| format!("{} · {:.1} Mbps",tr("Automatic"),state.capacity.watching_allowance().unwrap_or(crate::network::DEFAULT_DOWNLOAD_BPS) as f64/1_000_000.0)))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.download_budget,None,tr("Automatic"));
                    for mbps in [1,3,5,10,25,50,100,200] { ui.selectable_value(&mut self.download_budget,Some(mbps*1_000_000),format!("{mbps} Mbps")); }
                }).response.labelled_by(label.id);
            if before != self.download_budget { self.controller.send(Command::DownloadBudget { bps: self.download_budget }); }
            ui.small(tr("Automatic uses measured download capacity with headroom. Shared across everything you watch."));
            ui.small(tr("A slower watcher's limit can reduce the one shared stream for everyone watching it."));
            if ui.add_enabled(matches!(state.connection,"connected"|"reconnecting"),egui::Button::new(tr("Retry watching connection"))).clicked() { self.controller.send(Command::DownloadBudget { bps: self.download_budget }); }
            let label = ui.label(tr("Connection route"));
            egui::ComboBox::from_id_salt("connection-route").width(ui.available_width()).selected_text(if self.route == Route::Auto { tr("Automatic") } else { tr("Relay only") })
                .show_ui(ui, |ui| { ui.selectable_value(&mut self.route,Route::Auto,tr("Automatic")); ui.selectable_value(&mut self.route,Route::Relay,tr("Relay only")); }).response.labelled_by(label.id);
            ui.small(tr("Route applies the next time you join."));
        });
        if state.sharing {
            ui.separator();
            ui.label(tr("Stream diagnostics"));
            if let Some(mode) = state.mode {
                ui.small(format!(
                    "{}: {:.1} Mbps",
                    tr("Requested video target"),
                    mode.bitrate as f64 / 1_000_000.0
                ));
            }
            ui.small(format!(
                "{}: {:.1} Mbps",
                tr("Effective encoder target"),
                state.metrics.encoder_bitrate as f64 / 1_000_000.0
            ));
            ui.small(format!(
                "{}: {}",
                tr("Encoder bitrate changes"),
                state.metrics.encoder_bitrate_changes
            ));
            ui.small(format!(
                "{}: {:.1} FPS · {:.1} Mbps",
                tr("Encoded"),
                state.metrics.fps,
                state.metrics.encoded_bps as f64 / 1_000_000.0
            ));
            ui.small(format!(
                "{:.1} Mbps {}",
                state.delivery.bitrate_bps as f64 / 1_000_000.0,
                tr("sent")
            ));
            ui.small(format!(
                "{}: {:.0} ms · {}: {:.0} ms",
                tr("Audio capture age"),
                state.audio.capture_age_ms,
                tr("Audio submit wait"),
                state.audio.transport_wait_ms
            ));
        }
    }

    fn send_settings(&self, apply: bool) {
        self.controller.send(if apply {
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
        });
    }

    fn set_fullscreen(&mut self, ctx: &egui::Context, id: Option<String>) {
        if id.is_some() {
            self.advanced_open = false;
        }
        self.fullscreen = id;
        self.fullscreen_controls_until = Instant::now() + Duration::from_secs(3);
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen.is_some()));
    }

    fn update_picture(&mut self, ctx: &egui::Context, tile: &Tile) {
        if let Ok(mut picture) = tile.picture.lock()
            && let Some(image) = picture.take()
        {
            if let Some(texture) = self.textures.get_mut(&tile.id) {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.textures.insert(
                    tile.id.clone(),
                    ctx.load_texture(&tile.id, image, egui::TextureOptions::LINEAR),
                );
            }
        }
    }

    fn bitrate(ui: &mut egui::Ui, rect: egui::Rect, tile: &Tile) {
        let text = format!(
            "{:.1} Mbps",
            tile.received_bps.load(Ordering::Relaxed) as f64 / 1_000_000.0
        );
        ui.painter().text(
            rect.right_bottom() - egui::vec2(6.0, 2.0),
            egui::Align2::RIGHT_BOTTOM,
            text,
            egui::FontId::proportional(12.0),
            MUTED,
        );
        ui.interact(
            egui::Rect::from_min_max(
                rect.right_bottom() - egui::vec2(94.0, 20.0),
                rect.right_bottom(),
            ),
            ui.scope_id().with("received-bitrate"),
            egui::Sense::hover(),
        )
        .on_hover_text(tr("Received video · recent payload bitrate"));
    }

    fn fullscreen_view(&mut self, root: &mut egui::Ui, tile: &Tile) {
        let ctx = root.ctx().clone();
        self.update_picture(&ctx, tile);
        if ctx.input(|i| {
            i.pointer.delta() != egui::Vec2::ZERO
                || i.pointer.any_pressed()
                || i.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Key {
                            pressed: true,
                            repeat: false,
                            ..
                        }
                    )
                })
        }) {
            self.fullscreen_controls_until = Instant::now() + Duration::from_secs(3);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::BLACK))
            .show(root, |ui| {
                let rect = ui.max_rect();
                if let Some(texture) = self.textures.get(&tile.id) {
                    let aspect = texture.aspect_ratio();
                    let size = egui::vec2(
                        rect.width().min(rect.height() * aspect),
                        rect.height().min(rect.width() / aspect),
                    );
                    let target = egui::Rect::from_center_size(rect.center(), size);
                    ui.painter().image(
                        texture.id(),
                        target,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
                if ui
                    .interact(
                        rect,
                        ui.scope_id().with("fullscreen-video"),
                        egui::Sense::click(),
                    )
                    .double_clicked()
                {
                    self.set_fullscreen(&ctx, None);
                }
                if Instant::now() < self.fullscreen_controls_until {
                    let hud = egui::Rect::from_min_size(
                        rect.right_bottom() - egui::vec2(108.0, 30.0),
                        egui::vec2(102.0, 24.0),
                    );
                    ui.painter()
                        .rect_filled(hud, 4.0, egui::Color32::from_black_alpha(190));
                    Self::bitrate(ui, hud, tile);
                    egui::Area::new(egui::Id::unique("fullscreen-controls"))
                        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(16.0, -16.0))
                        .order(egui::Order::Foreground)
                        .show(&ctx, |ui| {
                            egui::Frame::new()
                                .fill(CARD)
                                .inner_margin(8)
                                .corner_radius(6)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [180.0, 32.0],
                                            egui::Label::new(&tile.name).truncate(),
                                        )
                                        .on_hover_text(&tile.name);
                                        if ui.button(tr("Exit full screen")).clicked() {
                                            self.set_fullscreen(&ctx, None);
                                        }
                                        if ui.button(tr("Stop")).clicked() {
                                            self.controller.send(Command::Watch {
                                                id: tile.id.clone(),
                                                enabled: false,
                                            });
                                            self.textures.remove(&tile.id);
                                            self.set_fullscreen(&ctx, None);
                                        }
                                        let mut volume = tile.volume.load(Ordering::Relaxed);
                                        ui.spacing_mut().slider_width = 120.0;
                                        if ui
                                            .add(
                                                egui::Slider::new(&mut volume, 0..=100).suffix("%"),
                                            )
                                            .on_hover_text(tr("This stream's volume"))
                                            .changed()
                                        {
                                            tile.volume.store(volume, Ordering::Relaxed);
                                        }
                                    });
                                });
                        });
                }
                if !tile.status.is_empty() {
                    ui.painter().text(
                        rect.center_top() + egui::vec2(0.0, 24.0),
                        egui::Align2::CENTER_TOP,
                        tr(&tile.status),
                        egui::FontId::proportional(16.0),
                        WARNING,
                    );
                }
            });
    }

    fn stream_tile(&mut self, ui: &mut egui::Ui, tile: &Tile, width: f32) {
        egui::Frame::new().fill(CARD).stroke(egui::Stroke::new(1.0,BORDER)).corner_radius(10).inner_margin(10)
            .show(ui, |ui| { ui.push_id(&tile.id, |ui| { ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y=6.0;
                let inner = (width-22.0).max(180.0);
                ui.set_width(inner);
                ui.horizontal(|ui| {
                    ui.add_sized([inner-if tile.watching {198.0}else{100.0},26.0],egui::Label::new(egui::RichText::new(&tile.name).strong()).truncate()).on_hover_text(&tile.name);
                    if tile.watching && ui.button(tr("Full screen")).on_hover_text(tr("Double-click video or press F11. Escape exits full screen.")).clicked(){self.set_fullscreen(ui.ctx(),Some(tile.id.clone()));}
                    let active = tile.watching && tile.frames.load(Ordering::Relaxed) > 0;
                    let text = if !tile.watching { tr("Preview") } else if active { tr("Live") } else { tr("Connecting") };
                    badge(ui,if tile.id=="__self_preview" {tr("Your stream")}else{text},if active { ACCENT } else { MUTED });
                }).response.on_hover_text(if tile.id=="__self_preview" { tr("Source confirmation · updates once a minute") } else if tile.watching { tr("Live video and audio subscription") } else { tr("Still preview, updated at most once per minute. Watch starts live video and audio.") });
                self.update_picture(ui.ctx(),tile);
                let (rect,response) = ui.allocate_exact_size(egui::vec2(inner,inner*9.0/16.0),egui::Sense::click());
                if tile.watching && response.double_clicked(){self.set_fullscreen(ui.ctx(),Some(tile.id.clone()));}
                ui.painter().rect_filled(rect,6.0,CANVAS);
                if let Some(texture) = self.textures.get(&tile.id) {
                    let aspect = texture.aspect_ratio();
                    let size = egui::vec2(rect.width().min(rect.height()*aspect),rect.height().min(rect.width()/aspect));
                    ui.painter().image(texture.id(),egui::Rect::from_center_size(rect.center(),size),egui::Rect::from_min_max(egui::pos2(0.0,0.0),egui::pos2(1.0,1.0)),egui::Color32::WHITE);
                } else {
                    ui.painter().text(rect.center(),egui::Align2::CENTER_CENTER,if tile.watching { tr("Starting live playback…") } else { tr("Loading preview…") },egui::FontId::proportional(14.0),MUTED);
                }
                if !tile.watching && tile.id!="__self_preview" {
                    let target=egui::Rect::from_center_size(rect.center(),egui::vec2(124.0,44.0));
                    if ui.put(target,primary(tr("Watch"),124.0)).on_hover_text(tr("Start receiving this stream's live video and audio")).clicked() { self.controller.send(Command::Watch { id:tile.id.clone(),enabled:true }); }
                    ui.advance_cursor_after_rect(rect);
                } else if tile.watching {
                    let (controls,_) = ui.allocate_exact_size(egui::vec2(inner,56.0),egui::Sense::hover());
                    Self::bitrate(ui,controls,tile);
                    let row_rect=egui::Rect::from_min_size(controls.min+egui::vec2(4.0,4.0),egui::vec2(controls.width()-8.0,32.0));
                    let mut row=ui.new_child(egui::UiBuilder::new().id_salt("playback-controls").max_rect(row_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
                    row.set_height(32.0);
                    row.style_mut().text_styles.insert(egui::TextStyle::Button,egui::FontId::proportional(12.0));
                    row.spacing_mut().button_padding.x=4.0;
                    row.spacing_mut().item_spacing.x=5.0;
                    if row.button(tr("Stop")).on_hover_text(tr("Stop receiving this stream's video and audio")).clicked() {
                        self.controller.send(Command::Watch { id:tile.id.clone(),enabled:false }); self.textures.remove(&tile.id);
                    }
                    if self.focused.is_none() && row.button(tr("Focus")).on_hover_text(tr("Enlarge this stream; Escape returns to the grid")).clicked() { self.focused=Some(tile.id.clone()); }
                    let mut volume=tile.volume.load(Ordering::Relaxed);
                    if row.button(if volume==0 { tr("Unmute") } else { tr("Mute") }).clicked() {
                        if volume==0 { volume=self.remembered_volume.get(&tile.id).copied().unwrap_or(100); }
                        else { self.remembered_volume.insert(tile.id.clone(),volume); volume=0; }
                        tile.volume.store(volume,Ordering::Relaxed);
                    }
                    let label=row.add_sized([24.0,32.0],egui::Label::new(tr("Vol")));
                    row.spacing_mut().slider_width=(row.available_width()-78.0).clamp(35.0,120.0);
                    if row.add(egui::Slider::new(&mut volume,0..=100).suffix("%")).labelled_by(label.id).on_hover_text(tr("This stream's volume")).changed() { tile.volume.store(volume,Ordering::Relaxed); }
                }
                if !tile.watching {
                    ui.allocate_ui_with_layout(egui::vec2(inner,40.0),egui::Layout::left_to_right(egui::Align::Center),|ui| {
                        ui.label(egui::RichText::new(if tile.id=="__self_preview" {tr("Source confirmation · updates once a minute")}else{tr("Still preview · updates up to once a minute")}).size(12.0).color(MUTED));
                    });
                }
                if !tile.status.is_empty() {
                    let notice=egui::Rect::from_min_max(rect.left_top()+egui::vec2(6.0,6.0),egui::pos2(rect.right()-6.0,rect.top()+58.0));
                    ui.painter().rect_filled(notice,5.0,egui::Color32::from_black_alpha(235));
                    let mut notice_ui=ui.new_child(egui::UiBuilder::new().id_salt("playback-status").max_rect(notice.shrink(6.0)));
                    notice_ui.label(egui::RichText::new(tr(&tile.status)).size(12.0).color(WARNING));
                }
            }); }); });
    }
}

impl eframe::App for PublisherUi {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.advanced_open {
                self.advanced_open = false;
            } else if self.fullscreen.is_some() {
                self.set_fullscreen(&ctx, None);
            } else {
                self.focused = None;
            }
        }
        let state = self.controller.snapshot();
        self.textures.retain(|id, _| {
            (state.sharing && id == "__self_preview") || state.tiles.iter().any(|t| &t.id == id)
        });
        if !state.sharing {
            self.self_preview_revision = None;
        }
        let hidden = ctx.input(|i| i.viewport().visible() == Some(false));
        ctx.request_repaint_after(Duration::from_millis(if hidden {
            250
        } else if state.tiles.iter().any(|t| t.watching) {
            16
        } else if state.busy || state.connection == "connecting" {
            50
        } else {
            250
        }));
        if ctx.input(|i| {
            i.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::F11,
                        pressed: true,
                        repeat: false,
                        ..
                    }
                )
            })
        }) {
            if self.fullscreen.is_some() {
                self.set_fullscreen(&ctx, None);
            } else if let Some(tile) = state
                .tiles
                .iter()
                .find(|t| t.watching && self.focused.as_ref() == Some(&t.id))
                .or_else(|| state.tiles.iter().find(|t| t.watching))
            {
                self.set_fullscreen(&ctx, Some(tile.id.clone()));
            }
        }
        if let Some(id) = self.fullscreen.clone() {
            if let Some(tile) = state
                .tiles
                .iter()
                .find(|tile| tile.id == id && tile.watching)
            {
                self.fullscreen_view(root, tile);
                return;
            }
            self.set_fullscreen(&ctx, None);
        }
        if state.connection == "connected"
            && !self.forgetting
            && let Ok(profile) = crate::settings::Profile::new(&self.invitation, &self.name)
            && self.attempted_profile.as_ref() != Some(&profile)
        {
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
        self.remembered_volume
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
            .show(root, |ui| self.room_header(ui, &state));
        if !state.error.is_empty()
            || !self.settings_error.is_empty()
            || state.busy
            || state.connection == "reconnecting"
        {
            egui::Panel::top("action-feedback")
                .frame(
                    egui::Frame::new()
                        .fill(PANEL)
                        .inner_margin(egui::Margin::symmetric(16, 8)),
                )
                .show(root, |ui| {
                    if !state.error.is_empty() {
                        ui.colored_label(ERROR, tr(&state.error));
                    }
                    if !self.settings_error.is_empty() {
                        ui.colored_label(ERROR, tr(&self.settings_error));
                    }
                    if state.busy {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(tr(state.operation));
                        });
                    } else if state.connection == "reconnecting" {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                            tr("Reconnecting to your room. Your watching choices will be restored."),
                        );
                        });
                    }
                });
        }
        egui::Panel::left("sharing-menu")
            .default_size(310.0)
            .min_size(286.0)
            .max_size(370.0)
            .resizable(true)
            .show(root, |ui| {
                if matches!(state.connection, "connected" | "reconnecting") {
                    let rect = ui.available_rect_before_wrap();
                    let footer_height = if state.sharing || state.capture_pending {
                        196.0
                    } else {
                        128.0
                    };
                    let split = (rect.bottom() - footer_height).max(rect.top() + 120.0);
                    let body_rect =
                        egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), split - 8.0));
                    let mut body = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("sharing-body")
                            .max_rect(body_rect),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("sharing-scroll")
                        .max_height(body_rect.height())
                        .auto_shrink([false, false])
                        .show(&mut body, |ui| self.sharing_menu(ui, &state));
                    let footer_rect =
                        egui::Rect::from_min_max(egui::pos2(rect.left(), split), rect.max);
                    let mut footer = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("sharing-actions")
                            .max_rect(footer_rect),
                    );
                    footer.separator();
                    self.sharing_actions(&mut footer, &state);
                    ui.advance_cursor_after_rect(rect);
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("joining-scroll")
                        .show(ui, |ui| self.sharing_menu(ui, &state));
                }
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(CANVAS).inner_margin(14))
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(if self.focused.is_some() {
                        tr("Focused stream")
                    } else {
                        tr("Friends' streams")
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.focused.is_some() && ui.button(tr("Back to grid")).clicked() {
                            self.focused = None;
                        }
                        ui.small(format!(
                            "{} {} · {} {}",
                            state.tiles.len(),
                            if state.tiles.len() == 1 {
                                tr("stream")
                            } else {
                                tr("streams")
                            },
                            state.tiles.iter().filter(|t| t.watching).count(),tr("watching")
                        ));
                    });
                });
                ui.add_space(6.0);
                if state.tiles.is_empty() && !state.sharing {
                    ui.vertical_centered(|ui| {
                        ui.add_space((ui.available_height() * 0.25).max(20.0));
                        ui.heading(if state.connection == "connected" {
                            tr("You're in. Make yourself at home.")
                        } else {
                            tr("A room for your friends' screens")
                        });
                        if state.connection == "connected" {
                            ui.label(if state.participants.is_empty() {
                                tr("Invite friends or start sharing. Streams will appear here.")
                            } else {
                                tr("Friends are here. Their streams appear when they start sharing.")
                            });
                        } else {
                            ui.label(
                                tr("Join with a private invitation to share and choose what to watch."),
                            );
                        }
                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new(
                                tr("Previews first. Live video and audio only when you choose Watch."),
                            )
                            .color(MUTED),
                        );
                    });
                    return;
                }
                let own_tile = if state.sharing && self.focused.is_none() {
                    // Retain the Arc, not just an address: a later source can reuse
                    // a freed allocation and restart its sequence at one.
                    let same_source=self.self_preview_revision.as_ref().is_some_and(|(cache,_)|std::sync::Arc::ptr_eq(cache,&state.self_preview));
                    if !same_source {
                        self.textures.remove("__self_preview");
                    }
                    let frame = state
                        .self_preview
                        .lock()
                        .ok()
                        .and_then(|frame| frame.clone());
                    let picture: crate::playback::Picture = std::sync::Arc::default();
                    if let Some(frame) = frame
                        && (self.self_preview_revision.as_ref().is_none_or(|(cache,sequence)|!std::sync::Arc::ptr_eq(cache,&state.self_preview)||*sequence!=frame.sequence)||!self.textures.contains_key("__self_preview"))
                        && let Ok(image) = image::load_from_memory_with_format(
                            &frame.bytes,
                            image::ImageFormat::Jpeg,
                        ) {
                            let rgba = image.into_rgba8();
                            if let Ok(mut slot) = picture.lock() {
                                *slot = Some(egui::ColorImage::from_rgba_unmultiplied(
                                    [rgba.width() as usize, rgba.height() as usize],
                                    rgba.as_raw(),
                                ));
                            }
                            self.self_preview_revision = Some((state.self_preview.clone(), frame.sequence));
                    }
                    Some(Tile {
                        id: "__self_preview".into(),
                        name: state.active_selection.as_ref().and_then(|a|state.sources.iter().find(|s|s.id==a.source_id)).map(|s|s.label.clone()).unwrap_or_else(||tr("Your screen").into()),
                        codec: String::new(),
                        watching: false,
                        picture,
                        frames: std::sync::Arc::default(),
                        received_bps: std::sync::Arc::default(),
                        volume: std::sync::Arc::default(),
                        audio: std::sync::Arc::default(),
                        status: String::new(),
                    })
                } else {
                    self.self_preview_revision = None;
                    None
                };
                let tiles: Vec<_> = own_tile
                    .iter()
                    .chain(state.tiles.iter())
                    .filter(|t| self.focused.as_ref().is_none_or(|id| id == &t.id))
                    .collect();
                let columns = if tiles.len() > 1 && ui.available_width() >= 680.0 {
                    2
                } else {
                    1
                };
                let gap = 14.0;
                let rows = tiles.len().div_ceil(columns).min(2);
                let scroll_space = if columns == 1 && tiles.len() > 1 {
                    ui.spacing().scroll.bar_width
                        + ui.spacing().scroll.bar_inner_margin
                        + ui.spacing().scroll.bar_outer_margin
                } else {
                    0.0
                };
                if scroll_space > 0.0 {
                    ui.label(
                        egui::RichText::new(tr("Scroll to see all streams"))
                            .size(12.0)
                            .color(MUTED),
                    );
                }
                let max_width = (ui.available_width() - scroll_space - gap * (columns - 1) as f32)
                    / columns as f32;
                let row_height = (ui.available_height() - gap * (rows - 1) as f32) / rows as f32;
                let width = if columns == 1 && tiles.len() > 1 {
                    max_width
                } else {
                    max_width
                        .min((row_height - if tiles.iter().any(|t|t.watching){136.0}else{114.0}).max(90.0) * 16.0 / 9.0 + 22.0)
                        // Keep Stop/Focus/Unmute and the volume editor inside the card.
                        // Short windows scroll instead of squeezing playback controls.
                        .max(302.0)
                };
                let inset =
                    ((ui.available_width() - width * columns as f32 - gap * (columns - 1) as f32)
                        / 2.0)
                        .max(0.0);
                egui::ScrollArea::vertical()
                    .id_salt("streams-scroll")
                    .scroll_bar_visibility(if columns == 1 && tiles.len() > 1 {
                        egui::scroll_area::ScrollBarVisibility::AlwaysVisible
                    } else {
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded
                    })
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.add_space(inset);
                            egui::Grid::new("stream-grid")
                                .num_columns(columns)
                                .spacing(egui::vec2(gap, gap))
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
            });
        if self.advanced_open {
            let mut done = false;
            let modal = egui::Modal::new(egui::Id::unique("advanced-settings"))
                .backdrop_color(egui::Color32::from_black_alpha(140))
                .show(&ctx, |ui| {
                    ui.set_width(440.0);
                    ui.horizontal(|ui| {
                        ui.heading(tr("Advanced settings"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(tr("Close")).clicked() {
                                done = true;
                            }
                        });
                    });
                    ui.label(tr(
                        "Publishing settings apply with Start sharing or Apply changes.",
                    ));
                    egui::ScrollArea::vertical()
                        .id_salt("advanced-scroll")
                        .max_height((ctx.viewport_rect().height() - 220.0).clamp(220.0, 500.0))
                        .show(ui, |ui| self.advanced(ui, &state));
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.add(primary(tr("Done"), 96.0)).clicked() {
                            done = true;
                        }
                        let dirty = state
                            .active_selection
                            .as_ref()
                            .is_some_and(|active| active != &self.selection());
                        if state.sharing
                            && ui
                                .add_enabled(
                                    !state.busy && dirty,
                                    egui::Button::new(tr("Apply changes")),
                                )
                                .clicked()
                        {
                            self.send_settings(true);
                        }
                    });
                });
            self.advanced_open = !done && !modal.should_close();
        }
    }
}

fn primary(label: &str, width: f32) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(label).strong().color(CANVAS))
        .fill(ACCENT)
        .min_size(egui::vec2(width, 38.0))
}

fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.13))
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).color(color).size(12.0).strong());
        });
}

fn open_licenses(ui: &mut egui::Ui) {
    if let Some(notices) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent()?.parent().map(|p| p.join("notices")))
        .filter(|p| p.is_dir())
        && ui.button(tr("Licenses and library sources")).clicked()
        && let Some(windows) = std::env::var_os("WINDIR")
    {
        let _ = std::process::Command::new(std::path::PathBuf::from(windows).join("explorer.exe"))
            .arg(notices)
            .spawn();
    }
}

fn quality_controls(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    source: Option<&crate::sources::SourceInfo>,
    encoder: &crate::encoding::Encoder,
) {
    let label = ui.label(tr("Resolution"));
    egui::ComboBox::from_id_salt("resolution")
        .width(ui.available_width())
        .selected_text(tr(settings.resolution.label()))
        .show_ui(ui, |ui| {
            for resolution in quality::Resolution::ALL {
                let candidate = Settings {
                    resolution,
                    ..*settings
                };
                let mode = source.and_then(|s| candidate.resolve(s, encoder.codec).ok());
                let text = mode
                    .map(|m| format!("{} · {}×{}", tr(resolution.label()), m.width, m.height))
                    .unwrap_or_else(|| resolution.label().into());
                ui.add_enabled_ui(mode.is_some_and(|m| encoder.supports(m)), |ui| {
                    ui.selectable_value(&mut settings.resolution, resolution, text);
                });
            }
        })
        .response
        .labelled_by(label.id);
    let label = ui.label(tr("Frame rate · 15–120 FPS"));
    let mut index = quality::FRAME_RATES
        .iter()
        .position(|fps| *fps == settings.fps)
        .unwrap_or(3);
    ui.spacing_mut().slider_width = (ui.available_width() - 75.0).max(100.0);
    if ui
        .add(
            egui::Slider::new(&mut index, 0..=quality::FRAME_RATES.len() - 1)
                .custom_formatter(|i, _| {
                    format!("{} FPS", quality::FRAME_RATES[i.round() as usize])
                })
                .custom_parser(|text| {
                    let fps = text
                        .trim()
                        .trim_end_matches("FPS")
                        .trim()
                        .parse::<u32>()
                        .ok()?;
                    quality::FRAME_RATES
                        .iter()
                        .position(|value| *value == fps)
                        .map(|index| index as f64)
                }),
        )
        .labelled_by(label.id)
        .on_hover_text(tr("Higher frame rates use more upload bandwidth."))
        .changed()
    {
        settings.fps = quality::FRAME_RATES[index];
    }
    let label = ui.label(tr("Quality preset"));
    egui::ComboBox::from_id_salt("quality-preset")
        .width(ui.available_width())
        .selected_text(tr(settings.profile.label()))
        .show_ui(ui, |ui| {
            for profile in [Profile::Motion, Profile::Detail, Profile::Saver] {
                ui.selectable_value(&mut settings.profile, profile, tr(profile.label()));
            }
        })
        .response
        .labelled_by(label.id);
    if let Some(source) = source {
        match settings.resolve(source, encoder.codec) {
            Ok(mode) if encoder.supports(mode) => {
                ui.small(format!(
                    "{}×{} · {} FPS · SDR",
                    mode.width, mode.height, mode.fps
                ));
            }
            _ => {
                ui.colored_label(
                    WARNING,
                    tr("This source / encoder cannot use that resolution and frame rate."),
                );
            }
        }
    }
}

fn bitrate_controls(ui: &mut egui::Ui, settings: &mut Settings) {
    let label = ui.label(tr("Video bitrate"));
    egui::ComboBox::from_id_salt("video-bitrate")
        .width(ui.available_width())
        .selected_text(match settings.bitrate {
            Bitrate::Auto => tr("Automatic").into(),
            Bitrate::Manual(bps) => format!("{} Mbps", bps / 1_000_000),
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut settings.bitrate, Bitrate::Auto, tr("Automatic"));
            for mbps in quality::BITRATE_PRESETS {
                ui.selectable_value(
                    &mut settings.bitrate,
                    Bitrate::Manual(mbps * 1_000_000),
                    format!("{mbps} Mbps"),
                );
            }
        })
        .response
        .labelled_by(label.id);
    if let Bitrate::Manual(bps) = settings.bitrate {
        let label = ui.label(tr("Custom bitrate"));
        let mut mbps = bps / 1_000_000;
        if ui
            .add(
                egui::DragValue::new(&mut mbps)
                    .range(1..=100)
                    .suffix(" Mbps"),
            )
            .labelled_by(label.id)
            .changed()
        {
            settings.bitrate = Bitrate::Manual(mbps * 1_000_000);
        }
    }
    ui.small(tr("Automatic follows resolution, FPS and content, up to 35 Mbps. Manual: 1–100 Mbps. Connection limits still apply."));
}
