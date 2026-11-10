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
    controller: Controller,
    invitation: String,
    name: String,
    source_id: String,
    encoder: String,
    quality: Settings,
    download_budget: Option<u64>,
    textures: HashMap<String, egui::TextureHandle>,
    focused: Option<String>,
    stream_menu: Option<String>,
    fullscreen: Option<String>,
    fullscreen_controls_until: Instant,
    store: Option<crate::settings::Store>,
    attempted_profile: Option<crate::settings::Profile>,
    settings_error: String,
    forgetting: bool,
    copied_until: Option<Instant>,
    remembered_volume: HashMap<String, u32>,
    advanced_open: bool,
    diagnostics_open: bool,
    show_invitation: bool,
    self_preview_revision: Option<(crate::preview::Latest, u64)>,
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
            download_budget: None,
            textures: HashMap::new(),
            focused: None,
            stream_menu: None,
            fullscreen: None,
            fullscreen_controls_until: Instant::now(),
            store,
            attempted_profile: profile.clone(),
            settings_error,
            forgetting: false,
            copied_until: None,
            remembered_volume: HashMap::new(),
            advanced_open: false,
            diagnostics_open: false,
            show_invitation: false,
            self_preview_revision: None,
            picker: crate::source_picker::Preview::new(),
            picker_hover: None,
            picker_texture: None,
            picker_failed: None,
            picker_refreshed: None,
            picker_keyboard: false,
            closing_since: None,
            force_close: false,
        };
        if let Some(profile) = profile {
            app.controller.send(Command::Join {
                invitation: profile.invitation,
                name: profile.name,
                route: Route::Auto,
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
            route: Route::Auto,
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
            brand_mark(ui);
            ui.label(skin::strong("FreedomStream", 20.0));
            ui.separator();
            let room_label = format!(
                "{}  ·  {} {}",
                if state.room_name.is_empty() {
                    tr("Your private room")
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
                    egui::Button::new(skin::strong(&room_label, 14.0))
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
                let room_options = skin::icon_button(ui, Icon::More, tr("Room options"));
                egui::Popup::menu(&room_options)
                    .frame(skin::menu_frame())
                    .show(|ui| {
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
        ui.separator();
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 18.0), egui::Sense::hover());
            skin::icon(ui.painter(), rect, Icon::Volume, TEXT);
            ui.label(egui::RichText::new(tr("Source audio")).size(12.0));
        })
        .response
        .on_hover_text(tr("Discord and FreedomStream excluded."));
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
        if let Ok(mut picture) = tile.picture.lock()
            && let Some(image) = picture.take()
        {
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
            ui.close();
        }
        if skin::menu_row(ui, tr("Full screen"), Some(Icon::Fullscreen), "F11", false).clicked() {
            self.set_fullscreen(ui.ctx(), Some(tile.id.clone()));
            ui.close();
        }
        skin::menu_separator(ui);
        if skin::menu_row(ui, tr("Stop watching"), Some(Icon::Stop), "", true).clicked() {
            self.controller.send(Command::Watch {
                id: tile.id.clone(),
                enabled: false,
            });
            self.textures.remove(&tile.id);
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
    }

    fn stream_tile(&mut self, ui: &mut egui::Ui, tile: &Tile, width: f32) {
        ui.push_id(&tile.id, |ui| {
            self.update_picture(ui.ctx(), tile);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, width * 9.0 / 16.0), egui::Sense::click());
            ui.painter().rect_filled(rect, 8.0, egui::Color32::BLACK);
            if let Some(texture) = self.textures.get(&tile.id) {
                egui::Image::new((texture.id(), texture.size_vec2()))
                    .corner_radius(8)
                    .paint_at(ui, fit_picture(rect, texture.aspect_ratio()));
            } else {
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
    fn handle_close(&mut self, ctx: &egui::Context) {
        if self.force_close {
            return;
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.closing_since.is_none() {
            self.closing_since = Some(Instant::now());
            self.clear_picker();
            self.controller.begin_shutdown();
        }
        if self.closing_since.is_some() {
            if self.controller.shutdown_finished() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }
    }
}

impl eframe::App for PublisherUi {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_close(ctx);
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
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
        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
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
            && self.attempted_profile.as_ref().is_none_or(|profile| {
                profile.invitation != self.invitation.trim() || profile.name != self.name.trim()
            })
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
                        ui.horizontal(|ui| {
                            avatar(ui, &self.name, 30.0);
                            ui.vertical(|ui| {
                                ui.label(skin::strong(&self.name, 14.0));
                                ui.colored_label(
                                    if state.connection == "connected" {
                                        ACCENT
                                    } else {
                                        WARNING
                                    },
                                    tr(if state.connection == "connected" {
                                        "Connected"
                                    } else {
                                        "Reconnecting"
                                    }),
                                );
                            });
                            if skin::icon_button(ui, Icon::Settings, tr("Advanced settings"))
                                .clicked()
                            {
                                self.advanced_open = true;
                            }
                        });
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
                .show(&ctx, |ui| {
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
                        .show(ui, |ui| self.advanced(ui, &state));
                    settings_divider(ui);
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [width - 142.0, 44.0],
                            egui::Label::new(
                                egui::RichText::new(tr(
                                    "Apply changes when starting or updating your stream.",
                                ))
                                .size(12.0)
                                .color(MUTED),
                            )
                            .wrap(),
                        );
                        if ui.add(primary(tr("Done"), 126.0)).clicked() {
                            done = true;
                        }
                    });
                });
            self.advanced_open = !done && !modal.should_close();
        }
    }
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
    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 24.0), egui::Sense::hover());
    let screen = egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 13.0));
    ui.painter().rect_stroke(
        screen,
        2.0,
        egui::Stroke::new(1.5, ACCENT),
        egui::StrokeKind::Inside,
    );
    let c = screen.center();
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            c + egui::vec2(-2.0, -3.0),
            c + egui::vec2(3.0, 0.0),
            c + egui::vec2(-2.0, 3.0),
        ],
        ACCENT,
        egui::Stroke::NONE,
    ));
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

    fn app() -> PublisherUi {
        gstreamer::init().expect("Test media runtime");
        PublisherUi {
            controller: Controller::new(),
            invitation: String::new(),
            name: String::new(),
            source_id: String::new(),
            encoder: String::new(),
            quality: Settings::default(),
            download_budget: None,
            textures: HashMap::new(),
            focused: None,
            stream_menu: None,
            fullscreen: None,
            fullscreen_controls_until: Instant::now(),
            store: None,
            attempted_profile: None,
            settings_error: String::new(),
            forgetting: false,
            copied_until: None,
            remembered_volume: HashMap::new(),
            advanced_open: false,
            diagnostics_open: false,
            show_invitation: false,
            self_preview_revision: None,
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
}
