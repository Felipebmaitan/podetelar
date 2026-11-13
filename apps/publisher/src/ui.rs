#[cfg(feature = "ui-review")]
#[path = "ui/review.rs"]
pub mod review;
use crate::locale::tr;
#[cfg(feature = "performance-experiments")]
#[allow(dead_code)]
#[path = "ui/experiment.rs"]
pub mod experiment;
use crate::ui_style::{self as skin, Icon};
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
const PANEL: egui::Color32 = egui::Color32::from_rgb(30, 33, 38);
const CANVAS: egui::Color32 = egui::Color32::from_rgb(20, 23, 27);
const CARD: egui::Color32 = egui::Color32::from_rgb(37, 41, 47);
const BORDER: egui::Color32 = egui::Color32::from_rgb(55, 61, 69);
const WARNING: egui::Color32 = egui::Color32::from_rgb(255, 205, 130);
const ERROR: egui::Color32 = egui::Color32::from_rgb(255, 165, 170);

pub fn configure_theme(ctx: &egui::Context) {
    skin::install_fonts(ctx);
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
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.selection.stroke.color = CANVAS;
    style.visuals.handle_shape = egui::style::HandleShape::Circle;
    style.visuals.slider_trailing_fill = true;
    style.visuals.popup_shadow = skin::menu_frame().shadow;
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.slider_rail_height = 4.0;
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = egui::CornerRadius::same(6);
        widget.fg_stroke.color = TEXT;
        widget.bg_stroke = egui::Stroke::new(0.5, BORDER);
    }
    style.visuals.widgets.noninteractive.bg_fill = CARD;
    style.visuals.widgets.noninteractive.weak_bg_fill = CARD;
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(43, 48, 56);
    style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(43, 48, 56);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(54, 61, 70);
    style.visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(54, 61, 70);
    style.visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(91, 111, 115));
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(33, 91, 85);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.interact_size.y = 32.0;
    style.spacing.scroll = egui::style::ScrollStyle::solid();
    style.spacing.scroll.bar_width = 8.0;
    style
        .text_styles
        .insert(egui::TextStyle::Heading, skin::semibold(23.0));
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
    updates: crate::updates::Client,
    update_restart_prompt: bool,
    activation: crate::activation::Inbox,
    prior_room: Option<(String, String)>,
    controller: Controller,
    invitation: String,
    name: String,
    session_name: Option<String>,
    source_id: String,
    encoder: String,
    quality: Settings,
    download_budget: Option<u64>,
    textures: HashMap<String, egui::TextureHandle>,
    focused: Option<String>,
    stream_menu: Option<String>,
    fullscreen: Option<String>,
    fullscreen_controls_until: Instant,
    settings_error: String,
    auto_join_ready: bool,
    copied_until: Option<Instant>,
    remembered_volume: HashMap<String, u32>,
    advanced_open: bool,
    diagnostics_open: bool,
    show_invitation: bool,
    browser_invitation_open: bool,
    pending_link: Option<crate::browser_links::Link>,
    link_result: Option<crate::browser_links::Load>,
    browser_links_enabled: bool,
    self_preview_revision: Option<(crate::preview::Latest, u64)>,
    #[cfg(feature = "performance-experiments")]
    planar: crate::planar_experiment::Videos,
    picker: crate::source_picker::Preview,
    picker_hover: Option<crate::sources::SourceInfo>,
    picker_texture: Option<(String, egui::TextureHandle)>,
    picker_failed: Option<String>,
    picker_refreshed: Option<Instant>,
    picker_keyboard: bool,
    closing_since: Option<Instant>,
    force_close: bool,
}
impl PublisherUi {
    fn begin_close(&mut self) {
        self.closing_since = Some(Instant::now());
        self.clear_picker();
        self.picker.begin_shutdown();
        if let Some(load) = &mut self.link_result {
            load.cancel();
        }
        self.controller.begin_shutdown();
    }
    fn open_activation(&mut self, value: &str, ctx: &egui::Context) {
        if !crate::activation_protocol::valid(value) {
            return;
        }
        let Ok(link) = crate::browser_links::Link::parse(value) else {
            self.settings_error =
                "Invalid browser invitation. Open a fresh invitation from your browser.".into();
            return;
        };
        if !self.browser_invitation_open {
            self.prior_room = Some((self.invitation.clone(), self.name.clone()));
        }
        self.auto_join_ready = false;
        self.invitation.clear();
        self.settings_error.clear();
        self.show_invitation = false;
        self.browser_invitation_open = true;
        self.set_fullscreen(ctx, None);
        if self.controller.snapshot().connection == "connecting" {
            self.controller.send(Command::Leave);
        }
        if let Some(load) = &mut self.link_result {
            load.cancel();
            self.pending_link = Some(link);
        } else {
            self.link_result = Some(crate::browser_links::Load::start(link));
        }
    }
    pub fn with_browser_link(browser_link: Option<String>) -> Self {
        crate::settings::remove_legacy();
        let mut settings_error = String::new();
        let is_browser_launch = browser_link.is_some();
        let link = browser_link.and_then(|value| match crate::browser_links::Link::parse(&value) {
            Ok(link) => Some(link),
            Err(_) => {
                settings_error =
                    "Invalid browser invitation. Open a fresh invitation from your browser.".into();
                None
            }
        });
        let link_result = link.map(crate::browser_links::Load::start);
        Self {
            updates: crate::updates::Client::new(),
            update_restart_prompt: false,
            activation: crate::activation::Inbox::new(),
            prior_room: None,
            controller: Controller::new(),
            invitation: String::new(),
            name: String::new(),
            session_name: None,
            source_id: String::new(),
            encoder: String::new(),
            quality: Settings::default(),
            download_budget: None,
            textures: HashMap::new(),
            focused: None,
            stream_menu: None,
            fullscreen: None,
            fullscreen_controls_until: Instant::now(),
            settings_error,
            auto_join_ready: false,
            copied_until: None,
            remembered_volume: HashMap::new(),
            advanced_open: false,
            diagnostics_open: false,
            show_invitation: false,
            browser_invitation_open: is_browser_launch,
            pending_link: None,
            link_result,
            browser_links_enabled: std::env::var("FREEDOMSTREAM_BROWSER_LINKS").as_deref()
                == Ok("1"),
            self_preview_revision: None,
            #[cfg(feature = "performance-experiments")]
            planar: Default::default(),
            picker: crate::source_picker::Preview::new(),
            picker_hover: None,
            picker_texture: None,
            picker_failed: None,
            picker_refreshed: None,
            picker_keyboard: false,
            closing_since: None,
            force_close: false,
        }
    }

    fn join(&mut self) {
        self.session_name = Some(self.name.trim().to_owned());
        self.prior_room = None;
        self.browser_invitation_open = false;
        self.show_invitation = false;
        self.auto_join_ready = false;
        self.controller.send(Command::Join {
            invitation: self.invitation.trim().to_owned(),
            name: self.name.trim().to_owned(),
            route: Route::Auto,
        });
    }

    fn room_header(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let connected = matches!(state.connection, "connected" | "reconnecting");
        ui.horizontal(|ui| {
            brand_mark(ui);
            ui.label(skin::strong("PodeTelar", 20.0));
            ui.separator();
            let room_label = format!(
                "{}  ·  {} {}",
                if state.room_name.is_empty() {
                    tr("Your private room")
                } else if state.room_name == "Friends" {
                    tr("Friends")
                } else {
                    &state.room_name
                },
                if connected {
                    state.participants.len() + 1
                } else {
                    0
                },
                tr("in room")
            );
            let room_button = ui
                .add_sized(
                    [260.0, 32.0],
                    egui::Button::new("")
                        .left_text(skin::strong(&room_label, 14.0))
                        .truncate()
                        .frame(false),
                )
                .on_hover_text(&room_label);
            egui::Popup::menu(&room_button)
                .frame(skin::menu_frame())
                .show(|ui| {
                    if connected {
                        ui.label(egui::RichText::new(tr("People in this room")).strong());
                        ui.label(format!("{} ({})", self.name, tr("you")));
                        for name in &state.participants {
                            ui.label(name);
                        }
                    } else {
                        ui.label(tr("Join a room to see its members."));
                    }
                });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.update_badge(ui);
                let mut pt = crate::locale::portuguese();
                egui::ComboBox::from_id_salt("ui-language")
                    .selected_text(if pt { "PT-BR" } else { "EN" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut pt, true, "Português (Brasil)");
                        ui.selectable_value(&mut pt, false, "English");
                    });
                if pt != crate::locale::portuguese() {
                    crate::locale::select(pt);
                }
                if state.connection == "connecting" {
                    if ui.button(tr("Cancel join")).clicked() {
                        self.controller.send(Command::Leave);
                    }
                } else if connected && ui.button(tr("Leave room")).clicked() {
                    self.controller.send(Command::Leave);
                    self.textures.clear();
                }
                let room_options = skin::icon_button(ui, Icon::More, tr("Room options"));
                egui::Popup::menu(&room_options)
                    .frame(skin::menu_frame())
                    .show(|ui| {
                        if ui.button(tr("Advanced settings")).clicked() {
                            self.advanced_open = true;
                            ui.close();
                        }
                        ui.separator();
                    if std::env::var_os(crate::activation_protocol::INSTALLED).is_some() {
                        ui.label(tr(if self.browser_links_enabled { "Room links open this installed app automatically." } else { "Room links are handled by another application or need repair." }));
                    } else {
                    let label = if self.browser_links_enabled { "Disable browser links" } else { "Enable browser links" };
                    if ui.button(tr(label)).on_hover_text(tr("Allow invitation pages to open this app for your Windows user. A launcher copy is kept in Local AppData.")).clicked() {
                        match crate::browser_links::set_enabled(!self.browser_links_enabled) {
                            Ok(()) => { self.browser_links_enabled = !self.browser_links_enabled; self.settings_error.clear(); }
                            Err(error) => self.settings_error = error.to_string(),
                        }
                        ui.close();
                    }
                    if self.browser_links_enabled && ui.button(tr("Update browser links to this version")).clicked() {
                        match crate::browser_links::set_enabled(true) {
                            Ok(()) => self.settings_error.clear(),
                            Err(error) => self.settings_error = error.to_string(),
                        }
                        ui.close();
                    }
                    }
                        open_licenses(ui);
                    });
                if connected {
                    let copied = self
                        .copied_until
                        .is_some_and(|until| Instant::now() < until);
                    if skin::outline_button(
                        ui,
                        if copied { tr("Copied!") } else { tr("Invite") },
                        Icon::Invite,
                    )
                    .clicked()
                    {
                        ui.ctx().copy_text(self.invitation.clone());
                        self.copied_until = Some(Instant::now() + Duration::from_secs(2));
                    }
                }
            });
        });
    }

    fn dismiss_browser_invitation(&mut self) {
        self.auto_join_ready = false;
        self.browser_invitation_open = false;
        self.pending_link = None;
        if let Some(load) = &mut self.link_result {
            load.cancel();
        }
        if let Some((invitation, name)) = self.prior_room.take() {
            self.invitation = invitation;
            self.name = name;
        } else {
            self.invitation.clear();
        }
        self.settings_error.clear();
    }

    fn browser_invitation_modal(&mut self, ctx: &egui::Context, state: &Snapshot) {
        let mut dismiss = false;
        let width = (ctx.content_rect().width() - 80.0).clamp(240.0, 440.0);
        let modal = egui::Modal::new(egui::Id::unique("browser-invitation"))
            .backdrop_color(egui::Color32::from_black_alpha(155))
            .frame(
                egui::Frame::new()
                    .fill(CARD)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .corner_radius(16)
                    .inner_margin(28)
                    .shadow(skin::menu_frame().shadow),
            )
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 12.0;
                ui.horizontal(|ui| {
                    ui.label(skin::strong(tr("Invitation from your browser"), 22.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        dismiss |= skin::icon_button(ui, Icon::Close, tr("Close")).clicked();
                    });
                });
                ui.add_space(4.0);
                if self.link_result.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(tr("Loading browser invitation…"));
                    });
                    ctx.request_repaint_after(Duration::from_millis(50));
                } else if !self.invitation.is_empty() {
                    if matches!(state.connection, "connected" | "reconnecting") {
                        ui.label(tr(
                            "Joining this room will end your current sharing and watching.",
                        ));
                    }
                    self.join_form(ui, state);
                }
                if !self.settings_error.is_empty() {
                    ui.colored_label(ERROR, tr(&self.settings_error));
                }
                dismiss |= ui.button(tr("Dismiss browser invitation")).clicked();
            });
        if dismiss || modal.should_close() {
            self.dismiss_browser_invitation();
        }
    }

    fn join_form(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        if self.browser_invitation_open {
            ui.heading(tr("Join your room"));
            if let Ok(invitation) = url::Url::parse(&self.invitation) {
                ui.label(invitation.origin().ascii_serialization());
            }
            let label = ui.label(tr("Your name"));
            let name = ui
                .add(
                    egui::TextEdit::singleline(&mut self.name)
                        .id_salt("browser-display-name")
                        .interactive(!state.busy)
                        .char_limit(60)
                        .desired_width(f32::INFINITY)
                        .hint_text(tr("How friends will see you")),
                )
                .labelled_by(label.id);
            let valid = ui.is_enabled() && !state.busy && !self.name.trim().is_empty();
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter))
                && (name.has_focus() || name.lost_focus());
            if ui
                .add_enabled(valid, primary(tr("Join room"), ui.available_width()))
                .clicked()
                || (valid && enter)
            {
                self.join();
            }
            ui.small(tr(
                "This invitation selects the server. Your name is kept only while the app is open.",
            ));
            return;
        }
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
        let valid = ui.is_enabled()
            && !state.busy
            && !self.invitation.trim().is_empty()
            && !self.name.trim().is_empty();
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
        ui.small(tr(
            "Open a room link from /stream or /telar in Discord, or paste an invitation. Nothing is restored from a previous launch.",
        ));
    }

    fn selection(&self) -> SharingSelection {
        SharingSelection {
            source_id: self.source_id.clone(),
            encoder: self.encoder.clone(),
            quality: self.quality,
        }
    }

    fn source_preview(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let own_tile = if state.sharing {
            // Retain the Arc, not just an address: a later source can reuse
            // a freed allocation and restart its sequence at one.
            let same_source = self
                .self_preview_revision
                .as_ref()
                .is_some_and(|(cache, _)| std::sync::Arc::ptr_eq(cache, &state.self_preview));
            if !same_source {
                self.textures.remove("__self_preview");
            }
            let frame = state.self_preview.lock().ok().and_then(|frame| {
                frame
                    .as_ref()
                    .filter(|frame| {
                        self.self_preview_revision
                            .as_ref()
                            .is_none_or(|(cache, sequence)| {
                                !std::sync::Arc::ptr_eq(cache, &state.self_preview)
                                    || *sequence != frame.sequence
                            })
                            || !self.textures.contains_key("__self_preview")
                    })
                    .cloned()
            });
            let picture: crate::playback::Picture = std::sync::Arc::default();
            if let Some(frame) = frame
                && (self
                    .self_preview_revision
                    .as_ref()
                    .is_none_or(|(cache, sequence)| {
                        !std::sync::Arc::ptr_eq(cache, &state.self_preview)
                            || *sequence != frame.sequence
                    })
                    || !self.textures.contains_key("__self_preview"))
                && let Ok(image) =
                    image::load_from_memory_with_format(&frame.bytes, image::ImageFormat::Jpeg)
            {
                let rgba = image.into_rgba8();
                if let Ok(mut slot) = picture.lock() {
                    *slot = Some(std::sync::Arc::new(
                        egui::ColorImage::from_rgba_unmultiplied(
                            [rgba.width() as usize, rgba.height() as usize],
                            rgba.as_raw(),
                        ),
                    ));
                }
                self.self_preview_revision = Some((state.self_preview.clone(), frame.sequence));
            }
            Some(Tile {
                id: "__self_preview".into(),
                name: state
                    .active_selection
                    .as_ref()
                    .and_then(|a| state.sources.iter().find(|s| s.id == a.source_id))
                    .map(|s| s.label.clone())
                    .unwrap_or_else(|| tr("Your screen").into()),
                codec: String::new(),
                watching: false,
                picture,
                presentation: std::sync::Arc::default(),
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
        if let Some(tile) = own_tile {
            self.update_picture(ui.ctx(), &tile);
        }
        let width = ui.available_width();
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(width, width * 9.0 / 16.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 6.0, CANVAS);
        if state.sharing
            && let Some(texture) = self.textures.get("__self_preview")
        {
            let target = fit_picture(rect, texture.aspect_ratio());
            ui.painter()
                .image(texture.id(), target, uv(), egui::Color32::WHITE);
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                tr("Your screen"),
                egui::FontId::proportional(14.0),
                MUTED,
            );
        }
        ui.interact(
            rect,
            ui.scope_id().with("source-preview"),
            egui::Sense::hover(),
        )
        .on_hover_text(tr("Source confirmation · updates once a minute"));
    }

    fn clear_picker(&mut self) {
        self.picker.clear();
        self.picker_hover = None;
        self.picker_texture = None;
        self.picker_failed = None;
        self.picker_keyboard = false;
    }

    fn source_selector(&mut self, ui: &mut egui::Ui, state: &Snapshot) -> egui::Response {
        let selected = state
            .sources
            .iter()
            .find(|source| source.id == self.source_id)
            .cloned();
        let list = egui::ComboBox::from_id_salt("capture-source")
            .width(ui.available_width())
            .truncate()
            .selected_text(
                selected
                    .as_ref()
                    .map(source_name)
                    .unwrap_or(tr("Choose a source")),
            )
            .show_ui(ui, |ui| {
                if ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO || i.pointer.any_pressed()) {
                    self.picker_keyboard = false;
                }
                if ui.input(|i| {
                    [
                        egui::Key::Tab,
                        egui::Key::ArrowUp,
                        egui::Key::ArrowDown,
                        egui::Key::Home,
                        egui::Key::End,
                    ]
                    .iter()
                    .any(|key| i.keys_down.contains(key))
                }) {
                    self.picker_keyboard = true;
                }
                if ui.is_enabled()
                    && !ui.input(|i| i.viewport().visible() == Some(false))
                    && self
                        .picker_refreshed
                        .is_none_or(|last| last.elapsed() >= Duration::from_secs(1))
                {
                    self.controller.send(Command::RefreshSources);
                    self.picker_refreshed = Some(Instant::now());
                }
                ui.set_min_width(220.0);
                ui.spacing_mut().item_spacing.y = 4.0;
                let mut hovered = None;
                let mut focused = None;
                egui::ScrollArea::vertical()
                    .id_salt("capture-source-list")
                    .max_height((ui.ctx().viewport_rect().height() - 140.0).clamp(160.0, 320.0))
                    .show(ui, |ui| {
                        for source in &state.sources {
                            ui.push_id(&source.id, |ui| {
                                let row = skin::source_row(
                                    ui,
                                    source_name(source),
                                    source.id == self.source_id,
                                    source.id.starts_with("window:"),
                                );
                                if row.hovered() {
                                    hovered = Some(source.clone());
                                }
                                if row.has_focus() {
                                    focused = Some(source.clone());
                                }
                                if row.clicked() {
                                    self.source_id = source.id.clone();
                                    ui.close();
                                }
                            });
                        }
                        if state.sources.is_empty() {
                            ui.label(tr("No capture sources. Refresh the list."));
                        }
                    });
                let source = (if self.picker_keyboard {
                    focused.or(hovered)
                } else {
                    hovered
                })
                .or_else(|| {
                    self.picker_hover
                        .clone()
                        .filter(|old| state.sources.iter().any(|source| source.id == old.id))
                })
                .or_else(|| selected.clone())
                .or_else(|| state.sources.first().cloned());
                if let Some(source) = source
                    && !ui.input(|i| i.viewport().visible() == Some(false))
                {
                    if self
                        .picker_hover
                        .as_ref()
                        .is_none_or(|old| old.id != source.id)
                    {
                        self.picker_failed = None;
                    }
                    self.picker_hover = Some(source.clone());
                    self.picker.request(&source);
                    self.source_popover(ui.ctx(), ui.min_rect(), &source, state.sharing);
                } else {
                    self.clear_picker();
                }
            });
        let response = list.response.on_hover_text(tr("Screen or window"));
        if list.inner.is_none() {
            self.clear_picker();
            self.picker_refreshed = None;
        }
        response
    }

    fn source_popover(
        &mut self,
        ctx: &egui::Context,
        anchor: egui::Rect,
        source: &crate::sources::SourceInfo,
        sharing: bool,
    ) {
        if self
            .picker_texture
            .as_ref()
            .is_some_and(|(id, _)| id != &source.id)
        {
            self.picker_texture = None;
        }
        if let Some(frame) = self.picker.take()
            && frame.source_id == source.id
        {
            self.picker_failed = None;
            if let Some(image) = frame.image {
                self.picker_texture = Some((
                    frame.source_id,
                    ctx.load_texture(
                        "local-source-confirmation",
                        image,
                        egui::TextureOptions::LINEAR,
                    ),
                ));
            } else {
                self.picker_failed = Some(source.id.clone());
            }
        }
        let width = 360.0_f32.min(ctx.viewport_rect().width() - 32.0);
        let height = width * 9.0 / 16.0 + 94.0;
        let bounds = ctx.viewport_rect().shrink(8.0);
        let pos = egui::pos2(
            (anchor.right() + 20.0)
                .min(bounds.right() - width - 20.0)
                .max(bounds.left()),
            anchor.top().min(bounds.bottom() - height).max(bounds.top()),
        );
        egui::Area::new(egui::Id::unique("local-source-confirmation"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .interactable(true)
            .show(ctx, |ui| {
                skin::menu_frame().inner_margin(10).show(ui, |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = 8.0;
                    ui.horizontal(|ui| {
                        let (icon, _) =
                            ui.allocate_exact_size(egui::vec2(22.0, 34.0), egui::Sense::hover());
                        skin::icon(ui.painter(), icon, Icon::Monitor, TEXT);
                        ui.vertical(|ui| {
                            ui.add(
                                egui::Label::new(skin::strong(source_name(source), 14.0))
                                    .truncate(),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} × {}",
                                    source.width, source.height
                                ))
                                .size(12.0)
                                .color(MUTED),
                            );
                        });
                    });
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(width, width * 9.0 / 16.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(rect, 6.0, CANVAS);
                    if let Some((_, texture)) = &self.picker_texture {
                        egui::Image::new((texture.id(), texture.size_vec2()))
                            .corner_radius(6)
                            .paint_at(ui, rect);
                    } else {
                        let message = if self.picker_failed.as_deref() == Some(&source.id) {
                            tr("Preview unavailable. Refresh sources to try again.")
                        } else {
                            tr("Loading preview…")
                        };
                        let text = ui.painter().layout(
                            message.to_owned(),
                            egui::FontId::proportional(12.0),
                            MUTED,
                            rect.width() - 32.0,
                        );
                        ui.painter()
                            .galley(rect.center() - text.size() / 2.0, text, MUTED);
                        if self.picker_failed.as_deref() != Some(&source.id) {
                            ctx.request_repaint_after(Duration::from_millis(40));
                        }
                    }
                    ui.label(
                        egui::RichText::new(tr(if sharing {
                            "Local preview · only visible to you"
                        } else {
                            "Local preview · not broadcasting"
                        }))
                        .size(12.0)
                        .color(MUTED),
                    );
                });
            });
    }

    fn sharing_menu(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        if !matches!(state.connection, "connected" | "reconnecting") {
            self.clear_picker();
            self.join_form(ui, state);
            return;
        }
        ui.horizontal(|ui| {
            ui.label(skin::strong(tr("Share your screen").to_uppercase(), 12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if skin::icon_button(ui, Icon::Refresh, tr("Refresh")).clicked() {
                    self.controller.send(Command::Refresh);
                }
            });
        });
        ui.spacing_mut().item_spacing.y = 6.0;
        let encoder = state.encoders.iter().find(|e| e.factory == self.encoder);
        let source = state.sources.iter().find(|s| s.id == self.source_id);
        ui.add_enabled_ui(!state.busy, |ui| {
            self.source_selector(ui,state);
            if let Some(encoder) = encoder { quality_controls(ui, &mut self.quality, source, encoder); }
            else { ui.colored_label(WARNING, tr("No supported hardware encoder was found. You can still watch compatible streams.")); }
        });
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 22.0), egui::Sense::hover());
            skin::icon(ui.painter(), rect, Icon::Settings, MUTED);
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(tr("Advanced")).size(12.0).color(MUTED))
                        .frame(false),
                )
                .clicked()
            {
                self.advanced_open = true;
            }
        });
        if state.sharing {
            egui::CollapsingHeader::new(tr("Source preview"))
                .default_open(ui.available_height() > 500.0)
                .show(ui, |ui| self.source_preview(ui, state));
        }
    }

    fn valid_selection(&self, state: &Snapshot) -> bool {
        state
            .encoders
            .iter()
            .find(|e| e.factory == self.encoder)
            .zip(state.sources.iter().find(|s| s.id == self.source_id))
            .is_some_and(|(e, s)| {
                self.quality
                    .resolve(s, e.codec)
                    .is_ok_and(|mode| e.supports(mode))
            })
    }

    fn sharing_actions(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let valid = self.valid_selection(state);
        ui.add_space(6.0);
        ui.separator();
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 18.0), egui::Sense::hover());
            skin::icon(ui.painter(), rect, Icon::Volume, TEXT);
            ui.label(egui::RichText::new(tr("Source audio")).size(12.0));
        })
        .response
        .on_hover_text(tr("Discord and PodeTelar excluded."));
        if state.sharing || state.capture_pending {
            if let Some(mode) = state.mode {
                ui.colored_label(
                    ACCENT,
                    skin::strong(
                        format!("{}×{} · {} FPS", mode.width, mode.height, mode.fps),
                        12.0,
                    ),
                )
                .on_hover_text(tr("Sharing"));
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
                        egui::Button::new(egui::RichText::new(tr("Stop")).color(ERROR))
                            .fill(egui::Color32::from_rgb(65, 37, 46)),
                    )
                    .on_hover_text(tr("Stop sharing"))
                    .clicked()
                {
                    self.controller.send(Command::Stop);
                }
                let apply = egui::Button::new(egui::RichText::new(tr("Apply")).color(if dirty {
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
        ui.spacing_mut().item_spacing.y = 10.0;
        ui.label(skin::strong(tr("Video"), 20.0));
        ui.add_space(4.0);
        ui.add_enabled_ui(!state.busy, |ui| {
            ui.columns(2, |columns| {
                let ui = &mut columns[0];
                let label = skin::help_label(
                    ui,
                    tr("Encoder"),
                    tr("AV1 / HEVC switch to H.264 when a watcher needs compatibility."),
                );
                egui::ComboBox::from_id_salt("hardware-encoder")
                    .width(ui.available_width())
                    .truncate()
                    .selected_text(
                        state
                            .encoders
                            .iter()
                            .find(|encoder| encoder.factory == self.encoder)
                            .map(|encoder| encoder.label.as_str())
                            .unwrap_or(tr("No supported encoder")),
                    )
                    .show_ui(ui, |ui| {
                        for encoder in &state.encoders {
                            ui.selectable_value(
                                &mut self.encoder,
                                encoder.factory.clone(),
                                &encoder.label,
                            );
                        }
                    })
                    .response
                    .labelled_by(label.id);
                bitrate_controls(&mut columns[1], &mut self.quality);
            });
        });
        settings_divider(ui);
        ui.label(skin::strong(tr("Reception"), 20.0));
        ui.add_space(4.0);
        ui.add_enabled_ui(!state.busy,|ui| {
            let label=skin::help_label(ui,tr("Watching allowance"),tr("Automatic uses measured download capacity with headroom, or 30 Mbps if unavailable. Shared across everything you watch."));
            let before=self.download_budget;
            egui::ComboBox::from_id_salt("watching-allowance").width(ui.available_width())
                .selected_text(self.download_budget.map(|bps|format!("{} Mbps",bps/1_000_000)).unwrap_or_else(||format!("{} · {:.0} Mbps",tr("Automatic"),state.capacity.watching_allowance().unwrap_or(crate::network::DEFAULT_DOWNLOAD_BPS) as f64/1_000_000.0)))
                .show_ui(ui,|ui| {
                    ui.selectable_value(&mut self.download_budget,None,tr("Automatic"));
                    for mbps in [1,3,5,10,25,30,50,100,200]{ui.selectable_value(&mut self.download_budget,Some(mbps*1_000_000),format!("{mbps} Mbps"));}
                }).response.labelled_by(label.id);
            if before!=self.download_budget{self.controller.send(Command::DownloadBudget{bps:self.download_budget});}
        });
        ui.label(
            egui::RichText::new(tr("Shared across the streams you watch."))
                .size(12.0)
                .color(MUTED),
        );
        settings_divider(ui);
        ui.label(skin::strong(tr("Connection"), 20.0));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let failed = !state.capacity.error.is_empty();
            if state.testing_connection {
                ui.spinner();
            } else {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(24.0, 42.0), egui::Sense::hover());
                skin::icon(
                    ui.painter(),
                    rect,
                    Icon::Info,
                    if failed { WARNING } else { MUTED },
                );
            }
            ui.vertical(|ui| {
                ui.label(skin::strong(
                    tr(if state.testing_connection {
                        "Measuring connection…"
                    } else if failed {
                        "Test unavailable"
                    } else if state.capacity.download_bps.is_some() {
                        "Connection measured"
                    } else {
                        "Not measured"
                    }),
                    14.0,
                ));
                ui.label(
                    egui::RichText::new(tr("To this VPS"))
                        .size(12.0)
                        .color(MUTED),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        state.connection == "connected" && !state.testing_connection && !state.busy,
                        egui::Button::new(tr("Test again")).min_size(egui::vec2(146.0, 34.0)),
                    )
                    .clicked()
                {
                    self.controller.send(Command::MeasureConnection);
                }
            });
        });
        ui.add_space(8.0);
        skin::disclosure(ui, tr("Diagnostics"), &mut self.diagnostics_open);
        if self.diagnostics_open {
            ui.add_space(8.0);
            self.diagnostics(ui, state);
        }
    }

    fn advanced_modal(&mut self, ctx: &egui::Context, state: &Snapshot) {
        if self.advanced_open && !self.browser_invitation_open {
            self.clear_picker();
            let mut done = false;
            let width = (ctx.viewport_rect().width() - 72.0).clamp(420.0, 620.0);
            let modal = egui::Modal::new(egui::Id::unique("advanced-settings"))
                .backdrop_color(egui::Color32::from_black_alpha(150))
                .frame(
                    egui::Frame::new()
                        .fill(CARD)
                        .stroke(egui::Stroke::new(1.0, BORDER))
                        .corner_radius(12)
                        .inner_margin(24),
                )
                .show(ctx, |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = 12.0;
                    ui.horizontal(|ui| {
                        ui.label(skin::strong(tr("Advanced settings"), 24.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if skin::icon_button(ui, Icon::Close, tr("Close")).clicked() {
                                done = true;
                            }
                        });
                    });
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical()
                        .id_salt("advanced-scroll")
                        .max_height((ctx.viewport_rect().height() - 230.0).clamp(180.0, 560.0))
                        .show(ui, |ui| self.advanced(ui, state));
                    settings_divider(ui);
                    ui.horizontal(|ui| {
                        let dirty = state.sharing
                            && state
                                .active_selection
                                .as_ref()
                                .is_some_and(|active| active != &self.selection());
                        ui.add_sized(
                            [width - 186.0, 44.0],
                            egui::Label::new(
                                egui::RichText::new(tr(if dirty {
                                    "Updates your current stream."
                                } else {
                                    "Ready for your next stream."
                                }))
                                .size(12.0)
                                .color(MUTED),
                            )
                            .wrap(),
                        );
                        if ui
                            .add_enabled(
                                !state.busy && (!dirty || self.valid_selection(state)),
                                primary(tr(if dirty { "Apply and close" } else { "Done" }), 170.0),
                            )
                            .clicked()
                        {
                            if dirty {
                                self.send_settings(true);
                            }
                            done = true;
                        }
                    });
                });
            self.advanced_open = !done && !modal.should_close();
        }
    }

    fn diagnostics(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        ui.spacing_mut().item_spacing.y = 10.0;
        if let Some(encoder) = state
            .encoders
            .iter()
            .find(|encoder| encoder.factory == self.encoder)
        {
            ui.label(format!("{}: {}", tr("Encoder"), encoder.codec.label()));
        }
        ui.label(
            egui::RichText::new(tr("Capacity to this VPS · payload capped at 200 Mbps"))
                .size(12.0)
                .color(MUTED),
        );
        for (name, value) in [
            ("Download", state.capacity.download_bps),
            ("Upload", state.capacity.upload_bps),
            (
                "Recommended video ceiling",
                state.capacity.upload_video_cap(),
            ),
        ] {
            if let Some(rate) = value {
                ui.label(format!(
                    "{}: {:.1} Mbps",
                    tr(name),
                    rate as f64 / 1_000_000.0
                ));
            }
        }
        if !state.capacity.error.is_empty() {
            ui.colored_label(WARNING, tr(&state.capacity.error));
            if state.capacity.download_bps.is_some() || state.capacity.upload_bps.is_some() {
                ui.small(tr("Incomplete test; showing last available measurements."));
            }
        }
        ui.label(egui::RichText::new(tr("A slower watcher's limit can reduce the one shared stream for everyone watching it.")).size(12.0).color(MUTED));
        if ui
            .add_enabled(
                matches!(state.connection, "connected" | "reconnecting") && !state.busy,
                egui::Button::new(tr("Retry watching connection")),
            )
            .clicked()
        {
            self.controller.send(Command::DownloadBudget {
                bps: self.download_budget,
            });
        }
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
        self.clear_picker();
        self.fullscreen = id;
        self.fullscreen_controls_until = Instant::now() + Duration::from_secs(3);
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen.is_some()));
    }

    fn update_picture(&mut self, ctx: &egui::Context, tile: &Tile) {
        // Native platforms can still schedule UI work while minimizing. Keep
        // subscription/audio state independent, but leave video owners in their
        // bounded mailbox until the window is presented again.
        if ctx.input(|input| {
            input.viewport().visible() == Some(false) || input.viewport().minimized == Some(true)
        }) {
            return;
        }
        tile.presentation.request_context(ctx);
        #[cfg(feature = "performance-experiments")]
        if !tile.watching
            && let Some(texture) = self.textures.get(&tile.id)
        {
            crate::resource_experiment::untag_rgba_texture(texture.id());
        }
        #[cfg(feature = "performance-experiments")]
        if let Ok(mut latest) = tile.presentation.planar.lock()
            && let Some(picture) = latest.take()
        {
            self.planar.update(&tile.id, picture);
            self.textures.remove(&tile.id);
            tile.presentation.uploaded.fetch_add(1, Ordering::Relaxed);
        }
        if let Ok(mut picture) = tile.picture.lock()
            && let Some(image) = picture.take()
        {
            #[cfg(feature = "performance-experiments")]
            self.planar.rgba(&tile.id);
            tile.presentation.uploaded.fetch_add(1, Ordering::Relaxed);
            if let Some(texture) = self.textures.get_mut(&tile.id) {
                texture.set(egui::ImageData::Color(image), egui::TextureOptions::LINEAR);
            } else {
                self.textures.insert(
                    tile.id.clone(),
                    ctx.load_texture(
                        &tile.id,
                        egui::ImageData::Color(image),
                        egui::TextureOptions::LINEAR,
                    ),
                );
            }
            #[cfg(feature = "performance-experiments")]
            if tile.watching
                && let Some(texture) = self.textures.get(&tile.id)
            {
                crate::resource_experiment::tag_rgba_texture(texture.id());
            }
        }
    }

    fn stream_metrics(ui: &mut egui::Ui, rect: egui::Rect, tile: &Tile) {
        let text = stream_metrics_text(&tile.codec, tile.received_bps.load(Ordering::Relaxed));
        let galley = ui
            .painter()
            .layout_no_wrap(text, egui::FontId::proportional(12.0), TEXT);
        let background = egui::Rect::from_min_size(
            rect.right_bottom() - galley.size() - egui::vec2(18.0, 16.0),
            galley.size() + egui::vec2(12.0, 8.0),
        );
        ui.painter()
            .rect_filled(background, 4.0, egui::Color32::from_black_alpha(195));
        ui.painter()
            .galley(background.min + egui::vec2(6.0, 4.0), galley, TEXT);
    }

    fn watch_menu(&mut self, ui: &mut egui::Ui, tile: &Tile) {
        ui.set_width(238.0);
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);
        ui.spacing_mut().interact_size.y = 24.0;
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(skin::strong(&tile.name, 12.0).color(MUTED));
        });
        ui.add_space(5.0);
        if !tile.watching {
            if skin::menu_row(ui, tr("Watch"), Some(Icon::Play), "", false).clicked() {
                self.controller.send(Command::Watch {
                    id: tile.id.clone(),
                    enabled: true,
                });
                ui.close();
            }
            return;
        }
        let mut volume = tile.volume.load(Ordering::Relaxed);
        let label = ui
            .horizontal(|ui| {
                ui.add_space(10.0);
                let label = ui.label(egui::RichText::new(tr("Volume")).size(13.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    ui.label(skin::strong(format!("{volume}%"), 12.0));
                });
                label
            })
            .inner;
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(17.0, 24.0), egui::Sense::hover());
            skin::icon(ui.painter(), rect, Icon::Volume, TEXT);
            ui.spacing_mut().slider_width = ui.available_width() - 10.0;
            // Keep egui's slider focus, keyboard, drag and range semantics.
            if skin::mint_slider(ui, &mut volume, 0..=100)
                .labelled_by(label.id)
                .changed()
            {
                tile.volume.store(volume, Ordering::Relaxed);
            }
        });
        let mut muted = volume == 0;
        if skin::mute_switch(ui, tr("Mute"), &mut muted).changed() {
            if muted {
                self.remembered_volume.insert(tile.id.clone(), volume);
                volume = 0;
            } else {
                volume = self.remembered_volume.get(&tile.id).copied().unwrap_or(100);
            }
            tile.volume.store(volume, Ordering::Relaxed);
        }
        skin::menu_separator(ui);
        if skin::menu_row(
            ui,
            if self.focused.as_ref() == Some(&tile.id) {
                tr("Back to grid")
            } else {
                tr("Focus stream")
            },
            Some(Icon::Focus),
            "",
            false,
        )
        .clicked()
        {
            self.focused = if self.focused.as_ref() == Some(&tile.id) {
                None
            } else {
                Some(tile.id.clone())
            };
            if self.fullscreen.is_some() {
                self.set_fullscreen(ui.ctx(), None);
            }
            ui.close();
        }
        let fullscreen = self.fullscreen.as_ref() == Some(&tile.id);
        if skin::menu_row(
            ui,
            tr(if fullscreen {
                "Exit full screen"
            } else {
                "Full screen"
            }),
            Some(Icon::Fullscreen),
            "F11",
            false,
        )
        .clicked()
        {
            self.set_fullscreen(
                ui.ctx(),
                if fullscreen {
                    None
                } else {
                    Some(tile.id.clone())
                },
            );
            ui.close();
        }
        if skin::menu_row(ui, tr("Advanced information"), None, "", false).clicked() {
            tile.presentation
                .diagnostics
                .open
                .store(true, Ordering::Relaxed);
            ui.close();
        }
        skin::menu_separator(ui);
        if skin::menu_row(ui, tr("Stop watching"), Some(Icon::Stop), "", true).clicked() {
            self.controller.send(Command::Watch {
                id: tile.id.clone(),
                enabled: false,
            });
            self.textures.remove(&tile.id);
            if fullscreen {
                self.set_fullscreen(ui.ctx(), None);
            }
            ui.close();
        }
    }

    fn fullscreen_view(&mut self, root: &mut egui::Ui, tile: &Tile) {
        let ctx = root.ctx().clone();
        self.clear_picker();
        self.update_picture(&ctx, tile);
        if ctx.input(|i| {
            i.pointer.delta() != egui::Vec2::ZERO
                || i.pointer.any_pressed()
                || !i.keys_down.is_empty()
        }) {
            self.fullscreen_controls_until = Instant::now() + Duration::from_secs(3);
        }
        let controls_visible = Instant::now() < self.fullscreen_controls_until;
        let mut menu_open = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::BLACK))
            .show(root, |ui| {
                let rect = ui.max_rect();
                #[cfg(feature = "performance-experiments")]
                let planar = self.planar.paint(ui, &tile.id, rect);
                #[cfg(not(feature = "performance-experiments"))]
                let planar = false;
                if !planar && let Some(texture) = self.textures.get(&tile.id) {
                    let aspect = texture.aspect_ratio();
                    let size = egui::vec2(
                        rect.width().min(rect.height() * aspect),
                        rect.height().min(rect.width() / aspect),
                    );
                    let target = egui::Rect::from_center_size(rect.center(), size);
                    if let Ok(mut viewport) = tile.presentation.viewport.lock() {
                        let ppp = ui.ctx().pixels_per_point();
                        *viewport = Some((size.x * ppp, size.y * ppp));
                    }
                    ui.painter().image(
                        texture.id(),
                        target,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
                let response = ui.interact(
                    rect,
                    ui.scope_id().with("fullscreen-video"),
                    egui::Sense::click(),
                );
                if response.double_clicked() {
                    self.set_fullscreen(&ctx, None);
                }
                menu_open = egui::Popup::context_menu(&response)
                    .frame(skin::menu_frame())
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| self.watch_menu(ui, tile))
                    .is_some();
                if controls_visible {
                    if ui.input(|i| i.pointer.hover_pos().is_some_and(|pos| rect.contains(pos))) {
                        Self::stream_metrics(ui, rect, tile);
                    }
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
                                        ui.label(&tile.name);
                                        if ui.button(tr("Exit full screen")).clicked() {
                                            self.set_fullscreen(&ctx, None);
                                        }
                                        if ui.button(tr("Stop")).clicked() {
                                            self.controller.send(Command::Watch {
                                                id: tile.id.clone(),
                                                enabled: false,
                                            });
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
        if !controls_visible && !menu_open && self.fullscreen.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::None);
        }
    }

    fn stream_tile(&mut self, ui: &mut egui::Ui, tile: &Tile, width: f32) {
        ui.push_id(&tile.id, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, width * 9.0 / 16.0), egui::Sense::click());
            #[cfg(feature = "performance-experiments")]
            let present = {
                use std::sync::OnceLock;
                static CLIP: OnceLock<bool> = OnceLock::new();
                let visible = ui.is_rect_visible(rect);
                tile.presentation.visible.store(visible, Ordering::Relaxed);
                !*CLIP.get_or_init(|| {
                    std::env::var("FREEDOMSTREAM_DEEP_CLIP")
                        .ok()
                        .is_none_or(|v| v != "0")
                }) || visible
            };
            #[cfg(not(feature = "performance-experiments"))]
            let present = ui.is_rect_visible(rect);
            if present {
                self.update_picture(ui.ctx(), tile);
            }
            ui.painter().rect_filled(rect, 8.0, egui::Color32::BLACK);
            #[cfg(feature = "performance-experiments")]
            let planar = present && self.planar.paint(ui, &tile.id, rect);
            #[cfg(not(feature = "performance-experiments"))]
            let planar = false;
            if !planar && let Some(texture) = self.textures.get(&tile.id) {
                if let Ok(mut viewport) = tile.presentation.viewport.lock() {
                    let target = fit_picture(rect, texture.aspect_ratio());
                    let ppp = ui.ctx().pixels_per_point();
                    *viewport = Some((target.width() * ppp, target.height() * ppp));
                }
                egui::Image::new((texture.id(), texture.size_vec2()))
                    .corner_radius(8)
                    .paint_at(ui, fit_picture(rect, texture.aspect_ratio()));
            } else if !planar {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    if tile.watching {
                        tr("Starting live playback…")
                    } else {
                        tr("Loading preview…")
                    },
                    egui::FontId::proportional(14.0),
                    MUTED,
                );
            }
            if tile.watching && response.double_clicked() {
                self.set_fullscreen(ui.ctx(), Some(tile.id.clone()));
            }
            let overlay = egui::Color32::from_rgba_unmultiplied(23, 31, 37, 230);
            let name_width = ui
                .painter()
                .layout_no_wrap(tile.name.clone(), skin::semibold(14.0), TEXT)
                .size()
                .x
                .min(160.0);
            let identity = egui::Rect::from_min_size(
                rect.min + egui::vec2(10.0, 10.0),
                egui::vec2((width - 20.0).min(name_width + 140.0), 30.0),
            );
            let mut header = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("identity")
                    .max_rect(identity),
            );
            egui::Frame::new()
                .fill(overlay)
                .corner_radius(8)
                .inner_margin(6)
                .show(&mut header, |ui| {
                    ui.horizontal(|ui| {
                        avatar(ui, &tile.name, 28.0);
                        ui.add_sized(
                            [name_width.max(24.0), 28.0],
                            egui::Label::new(skin::strong(&tile.name, 14.0))
                                .truncate()
                                .halign(egui::Align::Min),
                        )
                        .on_hover_text(&tile.name);
                        badge(
                            ui,
                            if !tile.watching {
                                tr("Preview")
                            } else if tile.frames.load(Ordering::Relaxed) > 0 {
                                tr("Watching")
                            } else {
                                tr("Connecting")
                            },
                            if tile.watching { ACCENT } else { MUTED },
                        );
                    });
                });
            if !tile.watching {
                ui.painter()
                    .rect_filled(rect, 6.0, egui::Color32::from_black_alpha(65));
                let target = egui::Rect::from_center_size(rect.center(), egui::vec2(124.0, 40.0));
                if ui.put(target, primary(tr("Watch"), 124.0)).clicked() {
                    self.controller.send(Command::Watch {
                        id: tile.id.clone(),
                        enabled: true,
                    });
                }
                let caption = egui::Rect::from_min_max(
                    egui::pos2(rect.left(), rect.bottom() - 28.0),
                    rect.max,
                );
                ui.painter().rect_filled(caption, 0.0, overlay);
                ui.painter().text(
                    caption.left_center() + egui::vec2(10.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    tr("Still preview · updates up to once a minute"),
                    egui::FontId::proportional(11.0),
                    TEXT,
                );
            }
            // The entire video is the hover target, including its overlay buttons.
            let hovered =
                ui.input(|i| i.pointer.hover_pos().is_some()) && ui.rect_contains_pointer(rect);
            if tile.watching && hovered {
                Self::stream_metrics(ui, rect, tile);
            }
            if tile.watching && (hovered || self.stream_menu.as_ref() == Some(&tile.id)) {
                let controls = egui::Rect::from_min_size(
                    rect.left_bottom() + egui::vec2(8.0, -38.0),
                    egui::vec2(92.0, 30.0),
                );
                let mut hud = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("hover-controls")
                        .max_rect(controls),
                );
                hud.horizontal(|ui| {
                    if fullscreen_button(ui)
                        .on_hover_text(tr("Full screen"))
                        .clicked()
                    {
                        self.set_fullscreen(ui.ctx(), Some(tile.id.clone()));
                    }
                    let trigger = skin::icon_button(ui, Icon::More, tr("Stream options"));
                    let menu = egui::Popup::menu(&trigger)
                        .frame(skin::menu_frame())
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                        .show(|ui| self.watch_menu(ui, tile));
                    if menu.is_some() {
                        self.stream_menu = Some(tile.id.clone());
                    } else if self.stream_menu.as_ref() == Some(&tile.id) {
                        self.stream_menu = None;
                    }
                });
            }
            egui::Popup::context_menu(&response)
                .frame(skin::menu_frame())
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| self.watch_menu(ui, tile));
            if !tile.status.is_empty() {
                let notice = egui::Rect::from_min_max(
                    rect.min + egui::vec2(8.0, 52.0),
                    egui::pos2(rect.right() - 8.0, rect.top() + 96.0),
                );
                ui.painter().rect_filled(notice, 5.0, overlay);
                let mut notice_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("playback-status")
                        .max_rect(notice.shrink(5.0)),
                );
                notice_ui.label(
                    egui::RichText::new(tr(&tile.status))
                        .size(12.0)
                        .color(WARNING),
                );
            }
            ui.advance_cursor_after_rect(rect);
        });
    }
}

impl PublisherUi {
    fn update_badge(&mut self, ui: &mut egui::Ui) {
        use crate::update_protocol::{Action, Phase};
        let status = self.updates.snapshot();
        if status.phase == Phase::Disabled {
            return;
        }
        let label = match status.phase {
            Phase::Checking => "Checking for updates…",
            Phase::Current => "Up to date",
            Phase::Available => "Download update",
            Phase::Downloading => "Downloading update…",
            Phase::Ready => "Restart to update",
            Phase::Restarting => "Restarting…",
            Phase::Error => "Retry update check",
            Phase::Disabled => unreachable!(),
        };
        let response = ui.add_enabled(
            !matches!(
                status.phase,
                Phase::Checking | Phase::Downloading | Phase::Restarting
            ),
            egui::Button::new("↻").fill(
                if matches!(status.phase, Phase::Available | Phase::Ready) {
                    egui::Color32::from_rgb(33, 91, 85)
                } else {
                    CARD
                },
            ),
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tr(label)));
        response.clone().on_disabled_hover_text(tr(label));
        response.clone().on_hover_ui(|ui| {
            ui.set_max_width(340.0);
            ui.strong(tr(label));
            if !status.version.is_empty() {
                ui.label(format!("{} {}", tr("New version"), status.version));
            }
            if status.notes.is_empty() {
                ui.label(tr("No release notes provided."));
            } else {
                ui.label(&status.notes);
            }
            if !status.message.is_empty() {
                ui.label(tr(&status.message));
            }
            if status.phase == Phase::Downloading {
                ui.add(
                    egui::ProgressBar::new(f32::from(status.progress) / 100.0).show_percentage(),
                );
            }
            if status.phase == Phase::Ready {
                ui.label(tr("Restarting ends sharing and watching."));
            }
        });
        if status.phase == Phase::Downloading {
            ui.label(format!("{}%", status.progress));
        }
        if response.clicked() {
            match status.phase {
                Phase::Current | Phase::Error => self.updates.send(Action::Check),
                Phase::Available => self.updates.send(Action::Download),
                Phase::Ready => self.update_restart_prompt = true,
                _ => {}
            }
        }
    }

    fn update_restart_dialog(&mut self, ctx: &egui::Context) {
        if !self.update_restart_prompt {
            return;
        }
        let mut restart = false;
        let mut later = false;
        egui::Modal::new(egui::Id::unique("restart-update")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.heading(tr("Restart to update"));
            ui.label(tr("Restarting ends sharing and watching."));
            ui.label(tr(
                "After restart, open a fresh invitation and enter your name again.",
            ));
            ui.horizontal(|ui| {
                later = ui.button(tr("Later")).clicked();
                restart = ui.button(tr("Restart to update")).clicked();
            });
        });
        if later {
            self.update_restart_prompt = false;
        }
        if restart {
            self.updates.send(crate::update_protocol::Action::Restart);
            self.update_restart_prompt = false;
        }
    }

    fn handle_close(&mut self, ctx: &egui::Context) {
        if self.force_close {
            return;
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.closing_since.is_none() {
            self.begin_close();
        }
        if self.closing_since.is_some() {
            if self.controller.shutdown_finished()
                && self.picker.shutdown_finished()
                && self
                    .link_result
                    .as_ref()
                    .is_none_or(crate::browser_links::Load::shutdown_finished)
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }
    }
}

impl eframe::App for PublisherUi {
    #[cfg(feature = "performance-experiments")]
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.planar.destroy(gl);
    }
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.updates.snapshot().phase == crate::update_protocol::Phase::Restarting
            && self.closing_since.is_none()
        {
            self.begin_close();
        }
        if self.closing_since.is_none() {
            ctx.request_repaint_after(Duration::from_millis(250));
            if let Some(event) = self.activation.poll() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                if let Some(link) = event.link {
                    self.open_activation(&link, ctx);
                }
            }
        }
        self.handle_close(ctx);
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_browser_invitation();
        self.render(root, _frame);
    }
}

impl PublisherUi {
    fn poll_browser_invitation(&mut self) {
        if self.closing_since.is_none()
            && self
                .link_result
                .as_ref()
                .is_some_and(crate::browser_links::Load::shutdown_finished)
        {
            let result = self
                .link_result
                .as_mut()
                .and_then(crate::browser_links::Load::poll);
            self.link_result = None;
            if let Some(link) = self.pending_link.take() {
                self.link_result = Some(crate::browser_links::Load::start(link));
                return;
            }
            if self.browser_invitation_open
                && let Some(result) = result
            {
                match result {
                    Ok(invitation) => {
                        self.invitation = invitation;
                        self.settings_error.clear();
                        let state = self.controller.snapshot();
                        self.auto_join_ready = self.session_name.is_some()
                            && !matches!(state.connection, "connected" | "reconnecting");
                        if self.auto_join_ready {
                            self.name = self.session_name.clone().unwrap();
                        }
                    }
                    Err(error) => self.settings_error = error,
                }
            }
        }
        let state = self.controller.snapshot();
        if self.closing_since.is_none()
            && self.browser_invitation_open
            && self.auto_join_ready
            && !state.busy
            && !matches!(state.connection, "connected" | "reconnecting")
        {
            self.join();
        }
    }

    fn render(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        ctx.request_repaint_after(Duration::from_secs(1));
        self.update_restart_dialog(&ctx);
        if let Some(started) = self.closing_since {
            egui::CentralPanel::default().show(root, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(48.0);
                    ui.spinner();
                    ui.heading(tr("Closing…"));
                    ui.label(tr("Stopping your stream and leaving the room…"));
                    if started.elapsed() >= Duration::from_secs(10)
                        && ui.button(tr("Force close")).clicked()
                    {
                        self.force_close = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
            return;
        }
        if !self.browser_invitation_open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.advanced_open {
                self.advanced_open = false;
            } else if self.fullscreen.is_some() {
                self.set_fullscreen(&ctx, None);
            } else {
                self.focused = None;
            }
        }
        let state = self.controller.snapshot();
        for tile in &state.tiles {
            crate::stream_diagnostics::panel(&ctx, tile);
        }
        #[cfg(feature = "performance-experiments")]
        self.planar.retain(_frame.gl(), |id| {
            state
                .tiles
                .iter()
                .any(|tile| tile.id == id && tile.watching)
        });
        // Fullscreen returns early below; retired stream/source textures must
        // still be released when another participant stops or leaves.
        self.textures.retain(|id, _| {
            (state.sharing && id == "__self_preview") || state.tiles.iter().any(|t| &t.id == id)
        });
        if !state.sharing {
            self.self_preview_revision = None;
        }
        let hidden = ctx.input(|i| {
            i.viewport().visible() == Some(false) || i.viewport().minimized == Some(true)
        });
        if hidden {
            self.clear_picker();
        }
        #[cfg(feature = "performance-experiments")]
        let frame_driven = crate::playback::frame_driven_repaint();
        #[cfg(not(feature = "performance-experiments"))]
        let frame_driven = false;
        ctx.request_repaint_after(Duration::from_millis(if hidden {
            250
        } else if !frame_driven && state.tiles.iter().any(|t| t.watching) {
            16
        } else if state.busy || state.connection == "connecting" {
            50
        } else {
            250
        }));
        if !self.browser_invitation_open && ctx.input(|i| i.key_pressed(egui::Key::F11)) {
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
        if self.browser_invitation_open {
            root.disable();
        }
        egui::Panel::top("room-header")
            .exact_size(54.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(16, 8)),
            )
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
            .default_size(252.0)
            .min_size(236.0)
            .max_size(300.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(16, 10)),
            )
            .show(root, |ui| {
                if matches!(state.connection, "connected" | "reconnecting") {
                    let pinned_actions = ui.available_height() < 600.0 || state.sharing;
                    let footer = 56.0
                        + if pinned_actions {
                            if state.sharing || state.capture_pending {
                                196.0
                            } else {
                                128.0
                            }
                        } else {
                            0.0
                        };
                    egui::ScrollArea::vertical()
                        .id_salt("sharing-scroll")
                        .max_height((ui.available_height() - footer).max(80.0))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            self.sharing_menu(ui, &state);
                            if !pinned_actions {
                                self.sharing_actions(ui, &state);
                            }
                            ui.add_space(10.0);
                            ui.separator();
                            ui.label(
                                skin::strong(
                                    format!("{} · {}", tr("In room"), state.participants.len() + 1),
                                    11.0,
                                )
                                .color(MUTED),
                            );
                            participant(ui, &self.name, true, state.sharing);
                            for name in &state.participants {
                                participant(
                                    ui,
                                    name,
                                    false,
                                    state.tiles.iter().any(|tile| tile.name == *name),
                                );
                            }
                        });
                    if pinned_actions {
                        self.sharing_actions(ui, &state);
                    }
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 44.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                avatar(ui, &self.name, 30.0);
                                let info_width = (ui.available_width() - 40.0).max(20.0);
                                ui.allocate_ui_with_layout(
                                    egui::vec2(info_width, 38.0),
                                    egui::Layout::top_down(egui::Align::LEFT),
                                    |ui| {
                                        ui.set_width(info_width);
                                        ui.spacing_mut().item_spacing.y = 3.0;
                                        ui.add(
                                            egui::Label::new(skin::strong(&self.name, 14.0))
                                                .truncate(),
                                        );
                                        ui.label(
                                            egui::RichText::new(tr(
                                                if state.connection == "connected" {
                                                    "Connected"
                                                } else {
                                                    "Reconnecting"
                                                },
                                            ))
                                            .size(12.0)
                                            .color(
                                                if state.connection == "connected" {
                                                    ACCENT
                                                } else {
                                                    WARNING
                                                },
                                            ),
                                        );
                                    },
                                );
                                if skin::icon_button(ui, Icon::Settings, tr("Advanced settings"))
                                    .clicked()
                                {
                                    self.advanced_open = true;
                                }
                            },
                        );
                        ui.separator();
                    });
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("joining-scroll")
                        .show(ui, |ui| self.sharing_menu(ui, &state));
                }
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(CANVAS).inner_margin(egui::Margin::symmetric(14,14)))
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(if self.focused.is_some(){tr("Focused stream")}else{tr("Streams")});
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(format!("{} {} · {} {}",state.tiles.len(),tr("live"),state.tiles.iter().filter(|t|t.watching).count(),tr("watching"))).size(13.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                        let watched=state.tiles.iter().find(|tile|tile.watching);
                        ui.add_enabled_ui(watched.is_some(),|ui| {
                            if skin::layout_button(ui,Icon::Focus,tr("Focus stream"),self.focused.is_some()).clicked(){self.focused=watched.map(|tile|tile.id.clone());}
                        });
                        if skin::layout_button(ui,Icon::Grid,tr("Back to grid"),self.focused.is_none()).clicked(){self.focused=None;}
                    });
                });
                ui.add_space(6.0);
                if state.tiles.is_empty() {
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
                let tiles: Vec<_> = state.tiles.iter()
                    .filter(|t| self.focused.as_ref().is_none_or(|id| id == &t.id))
                    .collect();
                let columns = if tiles.len() > 1 && ui.available_width() >= 600.0 {
                    2
                } else {
                    1
                };
                let gap = 10.0;
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
                    max_width.min(row_height.max(90.0) * 16.0 / 9.0)
                };
                let inset =
                    ((ui.available_width() - width * columns as f32 - gap * (columns - 1) as f32)
                        / 2.0)
                        .max(0.0);
                let scroll = egui::ScrollArea::vertical()
                    .id_salt("streams-scroll")
                    .scroll_bar_visibility(if columns == 1 && tiles.len() > 1 {
                        egui::scroll_area::ScrollBarVisibility::AlwaysVisible
                    } else {
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded
                    })
                    .auto_shrink([false, false]);
                #[cfg(feature = "performance-experiments")]
                let scroll = if let Some(offset) = ui.ctx().data(|data| data.get_temp::<f32>(egui::Id::unique("deep-scroll-offset"))) {
                    scroll.vertical_scroll_offset(offset)
                } else { scroll };
                scroll.show(ui, |ui| {
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
        self.advanced_modal(&ctx, &state);
        if self.browser_invitation_open {
            defocus_invitation_background(root);
            self.browser_invitation_modal(&ctx, &state);
        }
    }
}

/// A small fixed kernel softens only this new window's onboarding paint. No
/// framebuffer readback, media capture, persistent texture or renderer changes.
fn defocus_invitation_background(root: &egui::Ui) {
    fn has_callback(shape: &egui::Shape) -> bool {
        match shape {
            egui::Shape::Callback(_) => true,
            egui::Shape::Vec(shapes) => shapes.iter().any(has_callback),
            _ => false,
        }
    }
    root.ctx().graphics_mut(|graphics| {
        let Some(paint) = graphics.get_mut(root.layer_id()) else {
            return;
        };
        let entries: Vec<_> = paint.all_entries().cloned().collect();
        for (index, entry) in entries.into_iter().enumerate() {
            // Never replay a native painting callback (e.g. a video presenter).
            if has_callback(&entry.shape) {
                continue;
            }
            let mut taps = Vec::with_capacity(9);
            for y in [-1.5, 0.0, 1.5] {
                for x in [-1.5, 0.0, 1.5] {
                    let mut shape = entry.shape.clone();
                    shape.translate(egui::vec2(x, y));
                    egui::epaint::shape_transform::adjust_colors(&mut shape, |color| {
                        *color = color.gamma_multiply(1.0 / 9.0);
                    });
                    taps.push(shape);
                }
            }
            paint.set(
                egui::layers::ShapeIdx(index),
                entry.clip_rect,
                egui::Shape::Vec(taps),
            );
        }
    });
}

fn primary(label: &str, width: f32) -> egui::Button<'_> {
    let text = skin::strong(label, 14.0).color(CANVAS);
    let atoms = if label == tr("Watch") || label == tr("Start sharing") {
        egui::Atoms::new((
            egui::Atom::grow(),
            egui::Atom::paint(egui::vec2(16.0, 16.0), |ui, args| {
                skin::icon(ui.painter(), args.rect, Icon::Play, CANVAS)
            }),
            text,
            egui::Atom::grow(),
        ))
    } else {
        egui::Atoms::new((egui::Atom::grow(), text, egui::Atom::grow()))
    };
    egui::Button::new(atoms)
        .fill(ACCENT)
        .min_size(egui::vec2(width, 38.0))
}

fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.13))
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(skin::strong(text, 11.0).color(color));
        });
}

fn open_licenses(ui: &mut egui::Ui) {
    ui.collapsing(tr("Inter font license"), |ui| {
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .max_width(380.0)
            .show(ui, |ui| {
                ui.label(include_str!("../../../assets/fonts/OFL-Inter.txt"));
            });
    });
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
    let label = skin::field_label(ui, tr("Resolution"), Icon::Monitor);
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
    let label = ui
        .horizontal(|ui| {
            let label = ui.label(egui::RichText::new("FPS").size(11.0).color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(skin::strong(format!("{} FPS", settings.fps), 12.0));
            });
            label
        })
        .inner;
    let mut index = quality::FRAME_RATES
        .iter()
        .position(|fps| *fps == settings.fps)
        .unwrap_or(3);
    ui.spacing_mut().slider_width = ui.available_width();
    if skin::mint_slider(ui, &mut index, 0..=quality::FRAME_RATES.len() - 1)
        .labelled_by(label.id)
        .on_hover_text(tr("Higher frame rates use more upload bandwidth."))
        .changed()
    {
        settings.fps = quality::FRAME_RATES[index];
    }
    let label = ui.label(
        egui::RichText::new(tr("Quality preset"))
            .size(11.0)
            .color(MUTED),
    );
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
    let label = skin::help_label(
        ui,
        tr("Bitrate"),
        tr(
            "Automatic follows resolution, FPS and content, up to 35 Mbps. Manual: 1–100 Mbps. Connection limits still apply.",
        ),
    );
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
}

fn uv() -> egui::Rect {
    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0))
}

fn brand_mark(ui: &mut egui::Ui) {
    let id = egui::Id::unique("podetelar-brand-mark");
    let texture = ui
        .ctx()
        .data_mut(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let texture = ui.ctx().load_texture(
                "PodeTelar",
                egui::ColorImage::from_rgba_unmultiplied(
                    [64, 64],
                    include_bytes!("../../../assets/windows/podetelar-64.rgba"),
                ),
                egui::TextureOptions::LINEAR,
            );
            ui.ctx()
                .data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    ui.add(egui::Image::new((texture.id(), egui::vec2(28.0, 28.0))))
        .on_hover_text("PodeTelar");
}

fn fullscreen_button(ui: &mut egui::Ui) -> egui::Response {
    skin::icon_button(ui, Icon::Fullscreen, tr("Full screen"))
}

fn fit_picture(rect: egui::Rect, aspect: f32) -> egui::Rect {
    egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(
            rect.width().min(rect.height() * aspect),
            rect.height().min(rect.width() / aspect),
        ),
    )
}

fn stream_metrics_text(codec: &str, bps: u64) -> String {
    let codec = match codec.to_ascii_lowercase().as_str() {
        "video/h265" | "h265" | "hevc" => "HEVC",
        "video/h264" | "h264" => "H.264",
        "video/av1" | "av1" => "AV1",
        _ => "—",
    };
    format!("{codec}  ·  {:.1} Mbps", bps as f64 / 1_000_000.0)
}

fn avatar(ui: &mut egui::Ui, name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let palette = [
        egui::Color32::from_rgb(55, 81, 85),
        egui::Color32::from_rgb(77, 64, 96),
        egui::Color32::from_rgb(66, 80, 109),
        egui::Color32::from_rgb(91, 72, 64),
        egui::Color32::from_rgb(55, 90, 74),
    ];
    let color = palette[name.bytes().fold(0usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    }) % palette.len()];
    ui.painter().circle_filled(rect.center(), size / 2.0, color);
    ui.painter().circle_stroke(
        rect.center(),
        size / 2.0,
        egui::Stroke::new(1.0, egui::Color32::from_white_alpha(35)),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        name.chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string(),
        skin::semibold(size * 0.43),
        TEXT,
    );
}

fn participant(ui: &mut egui::Ui, name: &str, own: bool, sharing: bool) {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 26.0;
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.horizontal(|ui| {
            avatar(ui, name, 26.0);
            let label = if own {
                format!("{} ({})", name, tr("you"))
            } else {
                name.to_owned()
            };
            let width = (ui.available_width() - if sharing { 66.0 } else { 0.0 }).max(20.0);
            ui.allocate_ui_with_layout(
                egui::vec2(width, 26.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(label).size(12.0)).truncate());
                },
            )
            .response
            .on_hover_text(name);
            if sharing {
                badge(ui, tr("Live"), ACCENT);
            }
        });
    });
}

fn settings_divider(ui: &mut egui::Ui) {
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);
}

fn source_name(source: &crate::sources::SourceInfo) -> &str {
    if source.id.starts_with("monitor:") {
        source.label.split(" · ").next().unwrap_or(&source.label)
    } else {
        &source.label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_controls_download_but_require_explicit_restart_confirmation() {
        use crate::update_protocol::{Action, Phase, Status};
        fn draw(
            app: &mut PublisherUi,
            ctx: &egui::Context,
            events: Vec<egui::Event>,
        ) -> egui::FullOutput {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(960.0, 540.0),
                    )),
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        app.update_badge(ui);
                    });
                    app.update_restart_dialog(root.ctx());
                },
            );
            output.textures_delta.clear();
            output
        }
        fn click(app: &mut PublisherUi, ctx: &egui::Context, text: &str) {
            draw(app, ctx, vec![]);
            let output = draw(app, ctx, vec![]);
            let pos = output
                .shapes
                .iter()
                .rev()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text_shape) if text_shape.galley.text() == text => {
                        Some(text_shape.pos + text_shape.galley.size() / 2.0)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing button: {text}"));
            for pressed in [true, false] {
                draw(
                    app,
                    ctx,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
        }
        let mut app = app();
        let ctx = egui::Context::default();
        let (client, actions) = crate::updates::Client::fixture(Status {
            phase: Phase::Available,
            version: "0.1.1".into(),
            ..Default::default()
        });
        app.updates = client;
        click(&mut app, &ctx, "↻");
        assert_eq!(actions.try_recv().unwrap(), Action::Download);
        let (client, actions) = crate::updates::Client::fixture(Status {
            phase: Phase::Ready,
            ..Default::default()
        });
        app.updates = client;
        click(&mut app, &ctx, "↻");
        assert!(app.update_restart_prompt);
        assert!(actions.try_recv().is_err());
        click(&mut app, &ctx, tr("Later"));
        assert!(!app.update_restart_prompt);
        assert!(app.closing_since.is_none());
        assert!(actions.try_recv().is_err());
        app.update_restart_prompt = true;
        click(&mut app, &ctx, tr("Restart to update"));
        assert_eq!(actions.try_recv().unwrap(), Action::Restart);
        assert!(
            app.closing_since.is_none(),
            "Wait for launcher acknowledgement before closing"
        );
    }

    #[test]
    fn forwarded_invitation_loads_any_server_and_dismiss_restores_the_current_room() {
        let mut app = app();
        let previous = format!(
            "https://current.test/r/{}#invite={}",
            "r".repeat(22),
            "s".repeat(43)
        );
        app.invitation = previous.clone();
        app.name = "Current nickname".into();
        let ctx = egui::Context::default();
        let link = format!(
            "freedomstream://join?server=https%3A%2F%2Fother.test#handoff={}",
            "t".repeat(43)
        );
        app.open_activation(&link, &ctx);
        assert!(app.browser_invitation_open);
        assert!(app.pending_link.is_none());
        assert!(
            app.link_result.is_some(),
            "Every invitation loads its own server without a saved profile"
        );
        assert!(
            !app.controller.snapshot().busy,
            "Activation must not join or leave by itself"
        );
        app.name = "Unjoined nickname".into();
        app.dismiss_browser_invitation();
        assert_eq!(app.name, "Current nickname");
        assert_eq!(app.invitation, previous);
        assert!(!app.browser_invitation_open);
        // A new invitation replaces a cancelled worker only once it terminates.
        app.invitation = previous.replace("current.test", "other.test");
        app.open_activation(&link, &ctx);
        assert!(app.pending_link.is_some());
        assert!(app.link_result.is_some());
    }

    fn finish_link(app: &mut PublisherUi, invitation: &str) {
        let invitation = invitation.to_owned();
        app.browser_invitation_open = true;
        app.link_result = Some(crate::browser_links::Load::with_request(async move {
            Ok(invitation)
        }));
        let started = Instant::now();
        while !app.link_result.as_ref().unwrap().shutdown_finished() {
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        app.poll_browser_invitation();
    }

    #[test]
    fn fresh_process_asks_for_name_and_idle_session_reuses_name_on_each_server() {
        let mut app = app();
        let (controller, mut command) = Controller::recording_for_test(Snapshot::default());
        app.controller = controller;
        finish_link(&mut app, "https://first.test/synthetic");
        assert!(app.browser_invitation_open);
        assert!(command().is_none());
        // Typing does not silently submit a partially entered nickname.
        app.name = "F".into();
        app.poll_browser_invitation();
        assert!(command().is_none());
        finish_link(&mut app, "https://second.test/synthetic");
        assert!(app.browser_invitation_open);
        assert!(
            command().is_none(),
            "Another server must not submit a partial nickname"
        );
        app.name = "Friend".into();
        app.join();
        assert!(matches!(command(), Some(Command::Join { name, .. }) if name == "Friend"));
        let (controller, mut command) = Controller::recording_for_test(Snapshot::default());
        app.controller = controller;
        finish_link(&mut app, "https://second.test/synthetic");
        assert!(!app.browser_invitation_open);
        assert!(
            matches!(command(), Some(Command::Join { invitation, name, .. }) if invitation == "https://second.test/synthetic" && name == "Friend")
        );
        assert!(
            command().is_none(),
            "Activation never starts capture or watching"
        );
    }

    #[test]
    fn active_room_requires_confirmation_before_switching_servers() {
        for connection in ["connected", "reconnecting"] {
            let mut app = app();
            app.name = "Friend".into();
            app.session_name = Some("Friend".into());
            let (controller, mut command) = Controller::recording_for_test(Snapshot {
                connection,
                ..Default::default()
            });
            app.controller = controller;
            finish_link(&mut app, "https://other.test/synthetic");
            assert!(app.browser_invitation_open);
            assert!(command().is_none());
            app.join();
            assert!(matches!(command(), Some(Command::Join { .. })));
        }
    }

    #[test]
    fn latest_link_supersedes_pending_load_without_detaching_worker() {
        let mut app = app();
        app.link_result = Some(crate::browser_links::Load::with_request(
            std::future::pending(),
        ));
        app.browser_invitation_open = true;
        app.prior_room = Some(("original".into(), "original nickname".into()));
        let ctx = egui::Context::default();
        for server in ["first.test", "second.test"] {
            app.open_activation(
                &format!(
                    "freedomstream://join?server=https%3A%2F%2F{server}#handoff={}",
                    "t".repeat(43)
                ),
                &ctx,
            );
            assert!(
                app.link_result.is_some(),
                "Retain the cancelled worker until it terminates"
            );
        }
        assert_eq!(
            app.pending_link.as_ref().unwrap().origin.host_str(),
            Some("second.test")
        );
        app.dismiss_browser_invitation();
        assert!(app.pending_link.is_none());
        assert_eq!(app.invitation, "original");
        assert_eq!(app.name, "original nickname");
        assert!(!app.auto_join_ready);
    }

    #[test]
    fn browser_loading_is_centered_and_enter_cannot_join_before_redemption() {
        for size in [egui::vec2(1280.0, 720.0), egui::vec2(960.0, 540.0)] {
            let mut app = app();
            app.browser_invitation_open = true;
            app.link_result = Some(crate::browser_links::Load::with_request(
                std::future::pending(),
            ));
            let ctx = egui::Context::default();
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            for pass in 0..3 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(screen),
                        events: if pass == 2 {
                            vec![egui::Event::Key {
                                key: egui::Key::Enter,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: Default::default(),
                            }]
                        } else {
                            vec![]
                        },
                        ..Default::default()
                    },
                    |root| app.browser_invitation_modal(root.ctx(), &Snapshot::default()),
                );
                output.textures_delta.clear();
                let mut text = String::new();
                for shape in &output.shapes {
                    painted_text(&shape.shape, &mut text);
                }
                if pass == 2 {
                    assert!(text.contains(tr("Loading browser invitation…")));
                }
            }
            let rect = ctx
                .memory(|memory| memory.area_rect(egui::Id::unique("browser-invitation")))
                .unwrap();
            assert!((rect.center() - screen.center()).length() < 2.0);
            assert!(screen.contains_rect(rect));
            assert!(app.pending_link.is_none());
            assert!(app.link_result.is_some());
            assert!(app.invitation.is_empty());
            assert!(!app.controller.snapshot().busy);
        }
    }

    #[test]
    fn dismissing_browser_confirmation_discards_loaded_authority_without_joining() {
        let mut app = app();
        app.browser_invitation_open = true;
        app.invitation = "synthetic pending authority".into();
        app.link_result = Some(crate::browser_links::Load::with_request(
            std::future::pending(),
        ));
        app.dismiss_browser_invitation();
        assert!(!app.browser_invitation_open);
        assert!(app.pending_link.is_none());
        assert!(app.invitation.is_empty());
        assert!(!app.controller.snapshot().busy);
    }

    #[test]
    fn window_close_waits_for_cleanup_and_repeated_close_cannot_skip_it() {
        let mut app = app();
        let (controller, finish) = Controller::pending_shutdown_for_test();
        app.controller = controller;
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let mut input = egui::RawInput::default();
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .events
                .push(egui::ViewportEvent::Close);
            let mut output = ctx.run_ui(input, |root| app.handle_close(root.ctx()));
            output.textures_delta.clear();
            let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
            assert!(commands.contains(&egui::ViewportCommand::CancelClose));
            assert!(!commands.contains(&egui::ViewportCommand::Close));
            assert!(app.closing_since.is_some());
        }
        finish();
        let started = Instant::now();
        while !app.picker.shutdown_finished() {
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut output = ctx.run_ui(egui::RawInput::default(), |root| {
            app.handle_close(root.ctx())
        });
        output.textures_delta.clear();
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&egui::ViewportCommand::Close)
        );
    }

    #[test]
    fn normal_close_waits_for_picker_cleanup_without_blocking_the_ui() {
        use std::sync::mpsc;
        let mut app = app();
        let (controller, finish) = Controller::pending_shutdown_for_test();
        app.controller = controller;
        finish();
        let (started, ready) = mpsc::channel();
        let (release, released) = mpsc::channel();
        // Releasing on panic also keeps a failed assertion from stranding the fixture.
        struct Release(Option<mpsc::Sender<()>>);
        impl Drop for Release {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(());
                }
            }
        }
        let mut release = Release(Some(release));
        app.picker = crate::source_picker::Preview::with_capture(move |_, _| {
            started.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(5)).unwrap();
            None
        });
        app.picker.request(&crate::sources::SourceInfo {
            id: "owned-mock".into(),
            label: "Owned mock source".into(),
            width: 2,
            height: 2,
        });
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let start = Instant::now();
        let mut output = ctx.run_ui(input, |root| app.handle_close(root.ctx()));
        output.textures_delta.clear();
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        let premature = commands.contains(&egui::ViewportCommand::Close);
        let cancelled = commands.contains(&egui::ViewportCommand::CancelClose);
        let elapsed = start.elapsed();
        release.0.take().unwrap().send(()).unwrap();
        while !app.picker.shutdown_finished() {
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!premature && cancelled);
        assert!(elapsed < Duration::from_secs(1));
        let mut output = ctx.run_ui(egui::RawInput::default(), |root| {
            app.handle_close(root.ctx())
        });
        output.textures_delta.clear();
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&egui::ViewportCommand::Close)
        );
    }

    #[test]
    fn normal_close_cancels_browser_loading_and_waits_for_request_cleanup() {
        use std::sync::mpsc;
        let mut app = app();
        let (controller, finish) = Controller::pending_shutdown_for_test();
        app.controller = controller;
        finish();
        let (entered, ready) = mpsc::channel();
        let (cleaned, cleanup) = mpsc::channel();
        let (release, released) = mpsc::channel();
        struct Request(mpsc::Sender<()>, mpsc::Receiver<()>);
        impl Drop for Request {
            fn drop(&mut self) {
                self.0.send(()).unwrap();
                self.1.recv_timeout(Duration::from_secs(5)).unwrap();
            }
        }
        struct Release(Option<mpsc::Sender<()>>);
        impl Drop for Release {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(());
                }
            }
        }
        let mut release = Release(Some(release));
        let request = Request(cleaned, released);
        app.link_result = Some(crate::browser_links::Load::with_request(async move {
            let _request = request;
            entered.send(()).unwrap();
            std::future::pending::<anyhow::Result<String>>().await
        }));
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let start = Instant::now();
        let mut output = ctx.run_ui(input, |root| app.handle_close(root.ctx()));
        output.textures_delta.clear();
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        let premature = commands.contains(&egui::ViewportCommand::Close);
        let cancelled = commands.contains(&egui::ViewportCommand::CancelClose);
        let elapsed = start.elapsed();
        // Cancel reached and dropped the request; hold its cleanup until released.
        cleanup.recv_timeout(Duration::from_secs(3)).unwrap();
        release.0.take().unwrap().send(()).unwrap();
        while !app.picker.shutdown_finished()
            || !app.link_result.as_ref().unwrap().shutdown_finished()
        {
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!premature && cancelled);
        assert!(elapsed < Duration::from_secs(1));
        let mut output = ctx.run_ui(egui::RawInput::default(), |root| {
            app.handle_close(root.ctx())
        });
        output.textures_delta.clear();
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&egui::ViewportCommand::Close)
        );
        assert!(app.invitation.is_empty());
    }

    fn app() -> PublisherUi {
        gstreamer::init().expect("Test media runtime");
        PublisherUi {
            updates: crate::updates::Client::new(),
            update_restart_prompt: false,
            activation: crate::activation::Inbox::new(),
            prior_room: None,
            controller: Controller::new(),
            invitation: String::new(),
            name: String::new(),
            session_name: None,
            source_id: String::new(),
            encoder: String::new(),
            quality: Settings::default(),
            download_budget: None,
            textures: HashMap::new(),
            focused: None,
            stream_menu: None,
            fullscreen: None,
            fullscreen_controls_until: Instant::now(),
            settings_error: String::new(),
            auto_join_ready: false,
            copied_until: None,
            remembered_volume: HashMap::new(),
            advanced_open: false,
            diagnostics_open: false,
            show_invitation: false,
            browser_invitation_open: false,
            pending_link: None,
            link_result: None,
            browser_links_enabled: false,
            self_preview_revision: None,
            #[cfg(feature = "performance-experiments")]
            planar: Default::default(),
            picker: crate::source_picker::Preview::new(),
            picker_hover: None,
            picker_texture: None,
            picker_failed: None,
            picker_refreshed: None,
            picker_keyboard: false,
            closing_since: None,
            force_close: false,
        }
    }

    fn painted_text(shape: &egui::epaint::Shape, text: &mut String) {
        match shape {
            egui::epaint::Shape::Text(shape) => text.push_str(shape.galley.text()),
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    painted_text(shape, text);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn clipped_stream_preserves_picture_and_resumes_without_changing_watch_or_volume() {
        let mut app = app();
        let ctx = egui::Context::default();
        let tile = Tile {
            id: "clipping".into(),
            name: "Remote".into(),
            codec: String::new(),
            watching: true,
            picture: std::sync::Arc::new(std::sync::Mutex::new(Some(std::sync::Arc::new(
                egui::ColorImage::filled([2, 2], egui::Color32::RED),
            )))),
            presentation: Default::default(),
            frames: Default::default(),
            received_bps: Default::default(),
            volume: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(77)),
            audio: Default::default(),
            status: String::new(),
        };
        for clipped in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(960.0, 540.0),
                    )),
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        if clipped {
                            ui.set_clip_rect(egui::Rect::from_min_size(
                                egui::pos2(0.0, 500.0),
                                egui::vec2(960.0, 40.0),
                            ));
                        }
                        app.stream_tile(ui, &tile, 320.0);
                    });
                },
            );
            output.textures_delta.clear();
            assert_eq!(tile.picture.lock().unwrap().is_some(), clipped);
            assert_eq!(
                tile.presentation.uploaded.load(Ordering::Relaxed),
                u64::from(!clipped)
            );
            assert!(tile.watching);
            assert_eq!(tile.volume.load(Ordering::Relaxed), 77);
        }
    }

    #[test]
    fn minimized_video_keeps_the_latest_picture_until_restore() {
        let mut app = app();
        let ctx = egui::Context::default();
        let tile = Tile {
            id: "remote".into(),
            name: "Remote".into(),
            codec: String::new(),
            watching: true,
            picture: std::sync::Arc::new(std::sync::Mutex::new(Some(std::sync::Arc::new(
                egui::ColorImage::filled([2, 2], egui::Color32::RED),
            )))),
            presentation: Default::default(),
            frames: Default::default(),
            received_bps: Default::default(),
            volume: Default::default(),
            audio: Default::default(),
            status: String::new(),
        };
        for minimized in [true, false] {
            let mut input = egui::RawInput::default();
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .minimized = Some(minimized);
            let mut output = ctx.run_ui(input, |root| app.update_picture(root.ctx(), &tile));
            output.textures_delta.clear();
            assert_eq!(tile.picture.lock().unwrap().is_some(), minimized);
            assert_eq!(
                tile.presentation.uploaded.load(Ordering::Relaxed),
                u64::from(!minimized)
            );
            assert!(tile.watching);
        }
    }

    #[test]
    fn source_confirmation_reuses_unchanged_preview_and_replaces_new_owner() {
        let mut app = app();
        let ctx = egui::Context::default();
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut bytes)
            .encode(
                &[255, 0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0],
                2,
                2,
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        let mut state = Snapshot {
            sharing: true,
            ..Default::default()
        };
        *state.self_preview.lock().unwrap() = Some(crate::preview::Frame { sequence: 1, bytes });
        let mut draw = |state: &Snapshot| {
            let mut output = ctx.run_ui(egui::RawInput::default(), |root| {
                egui::CentralPanel::default().show(root, |ui| app.source_preview(ui, state));
            });
            let id = app.textures["__self_preview"].id();
            let changed = output
                .textures_delta
                .set
                .iter()
                .any(|(texture, _)| *texture == id);
            output.textures_delta.clear();
            changed
        };
        assert!(draw(&state));
        assert!(!draw(&state));
        // A replacement source can restart its sequence at one.
        let replacement = state.self_preview.lock().unwrap().clone();
        state.self_preview = std::sync::Arc::new(std::sync::Mutex::new(replacement));
        assert!(draw(&state));
    }

    #[test]
    fn fullscreen_right_click_menu_survives_idle_and_applies_actions() {
        for action in [
            "Mute",
            "Exit full screen",
            "Focus stream",
            "Stop watching",
            "Advanced information",
        ] {
            let mut app = app();
            let ctx = egui::Context::default();
            configure_theme(&ctx);
            let tile = Tile {
                id: "fullscreen-menu-test".into(),
                name: "Friend".into(),
                codec: "video/h264".into(),
                watching: true,
                picture: std::sync::Arc::default(),
                presentation: std::sync::Arc::default(),
                frames: std::sync::Arc::default(),
                received_bps: std::sync::Arc::default(),
                volume: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(75)),
                audio: std::sync::Arc::default(),
                status: String::new(),
            };
            app.set_fullscreen(&ctx, Some(tile.id.clone()));
            let draw = |app: &mut PublisherUi, events: Vec<egui::Event>| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800.0, 600.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |root| app.fullscreen_view(root, &tile),
                );
                output.textures_delta.clear();
                output
            };
            let click = |pos, button, pressed| egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: Default::default(),
            };
            // Open from the video, including when the HUD has already faded.
            draw(&mut app, vec![]);
            app.fullscreen_controls_until = Instant::now() - Duration::from_secs(1);
            let idle = draw(&mut app, vec![]);
            assert_eq!(idle.platform_output.cursor_icon, egui::CursorIcon::None);
            let position = egui::pos2(400.0, 180.0);
            for pressed in [true, false] {
                draw(
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(position),
                        click(position, egui::PointerButton::Secondary, pressed),
                    ],
                );
            }
            // egui measures the popup on its first pass; let it settle.
            draw(&mut app, vec![]);
            app.fullscreen_controls_until = Instant::now() - Duration::from_secs(1);
            let output = draw(&mut app, vec![]);
            assert!(egui::Popup::is_any_open(&ctx));
            assert_ne!(output.platform_output.cursor_icon, egui::CursorIcon::None);
            let point = output
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::epaint::Shape::Text(text) = &shape.shape {
                        (text.galley.text() == tr(action))
                            .then_some(text.pos + egui::vec2(8.0, 6.0))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| panic!("Missing fullscreen menu action: {action}"));
            for pressed in [true, false] {
                draw(
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(point),
                        click(point, egui::PointerButton::Primary, pressed),
                    ],
                );
            }
            match action {
                "Mute" => {
                    assert_eq!(tile.volume.load(Ordering::Relaxed), 0);
                    assert_eq!(app.remembered_volume.get(&tile.id), Some(&75));
                    assert!(egui::Popup::is_any_open(&ctx));
                    assert!(app.fullscreen.is_some());
                }
                "Advanced information" => {
                    assert!(tile.presentation.diagnostics.open.load(Ordering::Relaxed));
                    assert!(app.fullscreen.is_some());
                }
                _ => {
                    assert!(app.fullscreen.is_none(), "{action} must exit fullscreen");
                    if action == "Focus stream" {
                        assert_eq!(app.focused.as_ref(), Some(&tile.id));
                    }
                }
            }
        }
    }

    #[test]
    fn received_codec_and_rate_only_paint_on_hovered_subscriptions() {
        let mut app = app();
        app.fullscreen_controls_until = Instant::now() + Duration::from_secs(3);
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let mut tile = Tile {
            id: "synthetic-stream".into(),
            name: "Friend".into(),
            codec: "video/h265".into(),
            watching: true,
            picture: std::sync::Arc::default(),
            presentation: std::sync::Arc::default(),
            frames: std::sync::Arc::default(),
            received_bps: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(15_500_000)),
            volume: std::sync::Arc::default(),
            audio: std::sync::Arc::default(),
            status: String::new(),
        };
        let mut draw = |tile: &Tile, event: egui::Event, fullscreen: bool| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    events: vec![event],
                    ..Default::default()
                },
                |root| {
                    if fullscreen {
                        app.fullscreen_view(root, tile);
                    } else {
                        egui::CentralPanel::default()
                            .show(root, |ui| app.stream_tile(ui, tile, 600.0));
                    }
                },
            );
            let mut text = String::new();
            output.textures_delta.clear();
            for shape in output.shapes {
                painted_text(&shape.shape, &mut text);
            }
            text
        };
        let hidden = draw(&tile, egui::Event::PointerGone, false);
        assert!(!hidden.contains("Mbps"));
        let visible = draw(
            &tile,
            egui::Event::PointerMoved(egui::pos2(300.0, 180.0)),
            false,
        );
        assert!(visible.contains("HEVC  ·  15.5 Mbps"), "{visible}");
        assert!(!draw(&tile, egui::Event::PointerGone, false).contains("Mbps"));
        tile.watching = false;
        assert!(
            !draw(
                &tile,
                egui::Event::PointerMoved(egui::pos2(300.0, 180.0)),
                false
            )
            .contains("Mbps")
        );
        tile.watching = true;
        assert!(!draw(&tile, egui::Event::PointerGone, true).contains("Mbps"));
        assert!(
            draw(
                &tile,
                egui::Event::PointerMoved(egui::pos2(300.0, 180.0)),
                true
            )
            .contains("HEVC")
        );
    }

    #[test]
    fn long_room_names_preserve_header_actions_at_the_minimum_window_width() {
        let mut app = app();
        let (client, _actions) = crate::updates::Client::fixture(crate::update_protocol::Status {
            phase: crate::update_protocol::Phase::Ready,
            ..Default::default()
        });
        app.updates = client;
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let state = Snapshot {
            connection: "connected",
            room_name: "W".repeat(60),
            ..Default::default()
        };
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(960.0, 540.0),
                    )),
                    ..Default::default()
                },
                |root| {
                    egui::Panel::top("header-test")
                        .exact_size(48.0)
                        .show(root, |ui| app.room_header(ui, &state));
                },
            );
            output.textures_delta.clear();
            let mut labels = Vec::new();
            for shape in &output.shapes {
                if let egui::epaint::Shape::Text(text) = &shape.shape {
                    labels.push((
                        text.galley.text().to_owned(),
                        text.pos.x + text.galley.size().x,
                    ));
                }
            }
            for action in [tr("Invite"), tr("Leave room")] {
                let (_, right) = labels
                    .iter()
                    .find(|(label, _)| label.trim() == action)
                    .expect("Visible header action");
                assert!(*right <= 960.0, "{action} is outside the window: {right}");
            }
        }
    }

    #[test]
    fn codec_labels_follow_the_received_publication_without_inventing_unknown_codecs() {
        assert_eq!(
            stream_metrics_text("video/h264", 2_000_000),
            "H.264  ·  2.0 Mbps"
        );
        assert_eq!(stream_metrics_text("video/AV1", 0), "AV1  ·  0.0 Mbps");
        assert_eq!(stream_metrics_text("", 0), "—  ·  0.0 Mbps");
    }

    #[test]
    fn advanced_groups_hide_verbose_diagnostics_and_microphone_copy_by_default() {
        let mut app = app();
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let state = Snapshot {
            connection: "connected",
            capacity: crate::network::Capacity {
                error: "Connection measurement unavailable.".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960.0, 720.0),
                )),
                ..Default::default()
            },
            |root| {
                egui::CentralPanel::default().show(root, |ui| {
                    ui.set_width(620.0);
                    app.advanced(ui, &state);
                });
            },
        );
        output.textures_delta.clear();
        let mut text = String::new();
        for shape in &output.shapes {
            painted_text(&shape.shape, &mut text);
        }
        for section in [
            "Video",
            "Reception",
            "Connection",
            "Diagnostics",
            "Test unavailable",
        ] {
            assert!(text.contains(tr(section)), "Missing {section}: {text}");
        }
        for hidden in [
            "No microphone",
            "Automatic follows resolution, FPS and content, up to 35 Mbps. Manual: 1–100 Mbps. Connection limits still apply.",
            "Retry watching connection",
        ] {
            assert!(
                !text.contains(tr(hidden)),
                "Unexpected default copy: {hidden}"
            );
        }
        assert!(text.contains("30 Mbps"));
    }

    #[test]
    fn source_hover_previews_without_selecting_and_click_commits_without_publishing() {
        let mut app = app();
        app.source_id = "monitor:demo".into();
        let state = Snapshot {
            connection: "connected",
            sources: vec![
                crate::sources::SourceInfo {
                    id: "monitor:demo".into(),
                    label: "Monitor A".into(),
                    width: 1920,
                    height: 1080,
                },
                crate::sources::SourceInfo {
                    id: "window:demo".into(),
                    label: "Window B".into(),
                    width: 1920,
                    height: 1080,
                },
            ],
            ..Default::default()
        };
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        fn draw(
            app: &mut PublisherUi,
            ctx: &egui::Context,
            state: &Snapshot,
            events: Vec<egui::Event>,
        ) -> (egui::Response, Vec<egui::epaint::ClippedShape>) {
            let mut response = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(960.0, 540.0),
                    )),
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        ui.set_width(236.0);
                        response = Some(app.source_selector(ui, state));
                    });
                },
            );
            output.textures_delta.clear();
            (response.unwrap(), output.shapes)
        }
        draw(&mut app, &ctx, &state, vec![]);
        let (button, _) = draw(&mut app, &ctx, &state, vec![]);
        for pressed in [true, false] {
            draw(
                &mut app,
                &ctx,
                &state,
                vec![
                    egui::Event::PointerMoved(button.rect.center()),
                    egui::Event::PointerButton {
                        pos: button.rect.center(),
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        let (_, shapes) = draw(&mut app, &ctx, &state, vec![]);
        let point = shapes
            .iter()
            .find_map(|shape| {
                if let egui::epaint::Shape::Text(text) = &shape.shape {
                    (text.galley.text() == "Window B").then_some(text.pos + egui::vec2(20.0, 8.0))
                } else {
                    None
                }
            })
            .expect("Open source list");
        draw(
            &mut app,
            &ctx,
            &state,
            vec![egui::Event::PointerMoved(point)],
        );
        assert_eq!(app.source_id, "monitor:demo");
        assert_eq!(
            app.picker_hover.as_ref().map(|source| source.id.as_str()),
            Some("window:demo")
        );
        draw(
            &mut app,
            &ctx,
            &state,
            vec![egui::Event::Key {
                key: egui::Key::ArrowDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(app.picker_keyboard);
        draw(
            &mut app,
            &ctx,
            &state,
            vec![
                egui::Event::Key {
                    key: egui::Key::ArrowDown,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: Default::default(),
                },
                egui::Event::PointerMoved(point + egui::vec2(1.0, 0.0)),
            ],
        );
        assert!(!app.picker_keyboard);
        // Crossing row gaps or entering the preview must not reselect an old
        // keyboard-focused row and cancel the just-requested source.
        draw(
            &mut app,
            &ctx,
            &state,
            vec![egui::Event::PointerMoved(egui::pos2(450.0, 160.0))],
        );
        assert_eq!(
            app.picker_hover.as_ref().map(|source| source.id.as_str()),
            Some("window:demo")
        );
        draw(
            &mut app,
            &ctx,
            &state,
            vec![egui::Event::PointerMoved(point)],
        );
        for pressed in [true, false] {
            draw(
                &mut app,
                &ctx,
                &state,
                vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                }],
            );
        }
        draw(&mut app, &ctx, &state, vec![egui::Event::PointerGone]);
        assert_eq!(app.source_id, "window:demo");
        assert!(app.picker_hover.is_none());
        assert!(app.picker_texture.is_none());
        assert!(!app.controller.snapshot().sharing);
    }

    #[test]
    fn local_preview_blocks_pointer_clicks_to_controls_underneath() {
        let mut app = app();
        app.picker_failed = Some("demo".into());
        let source = crate::sources::SourceInfo {
            id: "demo".into(),
            label: "Source".into(),
            width: 1920,
            height: 1080,
        };
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let mut draw = |events| {
            let mut clicked = false;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(960.0, 540.0),
                    )),
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        clicked = ui
                            .put(
                                egui::Rect::from_min_size(
                                    egui::pos2(350.0, 180.0),
                                    egui::vec2(120.0, 40.0),
                                ),
                                egui::Button::new("Underlying stream control"),
                            )
                            .clicked();
                        app.source_popover(
                            &ctx,
                            egui::Rect::from_min_max(
                                egui::pos2(16.0, 100.0),
                                egui::pos2(250.0, 240.0),
                            ),
                            &source,
                            false,
                        );
                    });
                },
            );
            output.textures_delta.clear();
            clicked
        };
        draw(vec![]);
        draw(vec![]);
        let pos = egui::pos2(400.0, 200.0);
        for pressed in [true, false] {
            assert!(
                !draw(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default()
                    }
                ]),
                "Preview click reached an underlying control"
            );
        }
    }
    #[test]
    fn advanced_confirmation_applies_live_changes_and_never_starts_an_idle_stream() {
        for (sharing, changed, busy, valid) in [
            (true, true, false, true),
            (true, false, false, true),
            (false, true, false, true),
            (true, true, true, true),
            (true, true, false, false),
        ] {
            let mut app = app();
            app.source_id = "fixture".into();
            // Identity accepts arbitrary raw caps; no hardware publication is created.
            app.encoder = "identity".into();
            let state = Snapshot {
                connection: "connected",
                sharing,
                busy,
                sources: if valid {
                    vec![crate::sources::fixture().info]
                } else {
                    vec![]
                },
                encoders: vec![crate::encoding::Encoder {
                    factory: app.encoder.clone(),
                    label: "Test encoder".into(),
                    codec: crate::encoding::Codec::H264,
                }],
                active_selection: sharing.then(|| app.selection()),
                ..Default::default()
            };
            let (controller, mut commands) = Controller::recording_for_test(state.clone());
            app.controller = controller;
            app.advanced_open = true;
            if changed {
                app.quality.bitrate = Bitrate::Manual(12_000_000);
            }
            let ctx = egui::Context::default();
            configure_theme(&ctx);
            let draw = |app: &mut PublisherUi, events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1280.0, 720.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |root| {
                        app.advanced_modal(root.ctx(), &state);
                    },
                );
                output.textures_delta.clear();
                output
            };
            draw(&mut app, vec![]);
            let output = draw(&mut app, vec![]);
            let label = tr(if sharing && changed {
                "Apply and close"
            } else {
                "Done"
            });
            let pos = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos + text.galley.size() / 2.0)
                    }
                    _ => None,
                })
                .expect("Advanced confirmation button");
            for pressed in [true, false] {
                draw(
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
            let should_apply = sharing && changed && !busy && valid;
            let command = commands();
            if should_apply {
                assert!(matches!(
                    command,
                    Some(Command::Apply {
                        quality: Settings {
                            bitrate: Bitrate::Manual(12_000_000),
                            ..
                        },
                        ..
                    })
                ));
            } else {
                assert!(
                    command.is_none(),
                    "Advanced must not publish or apply a disabled/unchanged selection"
                );
            }
            assert_eq!(app.advanced_open, busy || (sharing && changed && !valid));
            assert!(
                commands().is_none(),
                "Only one Apply command per confirmation"
            );
        }
    }
}
