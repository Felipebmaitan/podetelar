#[path = "ui/markdown.rs"]
mod markdown;
#[path = "ui/profile.rs"]
mod profile;
#[path = "ui/stage.rs"]
mod stage;
use stage::SELF_PREVIEW;
#[cfg(feature = "ui-review")]
#[path = "ui/review.rs"]
pub mod review;
use crate::locale::tr;
#[cfg(feature = "performance-experiments")]
#[allow(dead_code)]
#[path = "ui/experiment.rs"]
pub mod experiment;
use crate::theme::{self, Appearance, Theme, palette};
use crate::ui_style::{self as skin, Icon};
use crate::{
    connection::Route,
    playback::Tile,
    publisher::{Command, Controller, SharingSelection, Snapshot},
    quality::{self, Bitrate, Profile, Settings},
    sources::camera,
};
use eframe::egui;
use std::{
    collections::HashMap,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

pub fn configure_theme(ctx: &egui::Context) {
    skin::install_fonts(ctx);
    apply_palette(ctx, Theme::Amber);
}

fn apply_palette(ctx: &egui::Context, effective: Theme) {
    theme::activate(effective);
    let p = effective.palette();
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = if p.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.visuals.weak_text_color = Some(p.muted);
    style.visuals.disabled_alpha = 0.84;
    style.visuals.panel_fill = p.panel;
    style.visuals.window_fill = p.card;
    style.visuals.extreme_bg_color = p.canvas;
    style.visuals.faint_bg_color = p.card;
    style.visuals.window_corner_radius = egui::CornerRadius::same(12);
    style.visuals.selection.bg_fill = p.accent;
    style.visuals.selection.stroke.color = p.on_accent;
    style.visuals.hyperlink_color = p.accent;
    style.visuals.error_fg_color = p.error;
    style.visuals.warn_fg_color = p.warning;
    style.visuals.window_stroke = egui::Stroke::new(1.0, p.border);
    style.visuals.handle_shape = egui::style::HandleShape::Circle;
    style.visuals.slider_trailing_fill = true;
    style.visuals.popup_shadow = skin::menu_frame().shadow;
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.slider_rail_height = 4.0;
    // Keep normal text in the widget palette so selected controls use on-accent ink.
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(8);
        widget.fg_stroke.color = p.text;
        widget.bg_stroke = egui::Stroke::new(1.0, p.border);
    }
    style.visuals.widgets.noninteractive.bg_fill = p.panel;
    style.visuals.widgets.noninteractive.weak_bg_fill = p.panel;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.divider);
    style.visuals.widgets.open.bg_fill = p.control_active;
    style.visuals.widgets.open.weak_bg_fill = p.control_active;
    style.visuals.widgets.inactive.bg_fill = p.control;
    style.visuals.widgets.inactive.weak_bg_fill = p.control;
    style.visuals.widgets.hovered.bg_fill = p.control_hover;
    style.visuals.widgets.hovered.weak_bg_fill = p.control_hover;
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, p.hover_stroke);
    style.visuals.widgets.active.bg_fill = p.control_active;
    style.visuals.widgets.active.weak_bg_fill = p.control_active;
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, p.accent);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 32.0;
    style.spacing.scroll = egui::style::ScrollStyle {
        bar_width: 8.0,
        floating_width: 3.0,
        floating_allocated_width: 11.0,
        handle_min_length: 24.0,
        dormant_handle_opacity: if p.dark { 0.4 } else { 0.5 },
        active_handle_opacity: 0.55,
        interact_handle_opacity: 0.85,
        active_background_opacity: 0.0,
        interact_background_opacity: 0.25,
        ..egui::style::ScrollStyle::floating()
    };
    style
        .text_styles
        .insert(egui::TextStyle::Heading, skin::semibold(22.0));
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(13.5));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(13.5));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    // Leave egui in System mode so the integration keeps receiving OS changes.
    // Both style slots use the app's effective palette, including manual overrides.
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
    ctx.data_mut(|data| data.insert_temp(egui::Id::unique("studio-palette"), effective));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SharingSetup {
    Screen,
    Camera,
    CameraAdvanced,
}

const SHARING_BUTTON_HEIGHT: f32 = 38.0;
const PLAYBACK_CONTROLS_WIDTH: f32 = 114.0;

struct CameraSettings {
    device_id: String,
    resolution: quality::Resolution,
    fps: u32,
    bitrate_mbps: Option<f32>,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            resolution: quality::Resolution::P720,
            fps: 30,
            bitrate_mbps: None,
        }
    }
}

impl CameraSettings {
    fn quality(&self) -> Settings {
        Settings {
            resolution: self.resolution,
            fps: self.fps,
            bitrate: self.bitrate_mbps.map_or(Bitrate::Auto, |mbps| {
                Bitrate::Manual((mbps * 1_000_000.0).round() as u64)
            }),
            profile: Profile::Motion,
        }
    }
}

struct SharingDock {
    rect: egui::Rect,
    screen: egui::Rect,
    camera: egui::Rect,
}

pub struct PublisherUi {
    following_client: crate::following::Client,
    following: crate::following::Follow,
    following_key: Option<String>,
    next_follow_attempt: Instant,
    account_client: crate::account::Client,
    account_state: crate::account::State,
    account_server: String,
    profile_connection_key: Option<(String, Option<String>)>,
    profile_client: Option<crate::profile::Client>,
    profile_state: crate::profile::State,
    profile_textures: HashMap<String, (usize, egui::TextureHandle)>,
    profile_started: Instant,
    profile_open: bool,
    profile_name: String,
    saved_guest_name: String,
    updates: crate::updates::Client,
    update_open: bool,
    update_announced_version: Option<String>,
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
    camera_settings: CameraSettings,
    custom_bitrate: bool,
    download_budget: Option<u64>,
    textures: HashMap<String, egui::TextureHandle>,
    focused: Option<String>,
    spotlight: Option<String>,
    self_preview_hidden: bool,
    fullscreen: Option<String>,
    fullscreen_controls_until: Instant,
    settings_error: String,
    auto_join_ready: bool,
    copied_until: Option<Instant>,
    appearance: Appearance,
    advanced_open: bool,
    licenses_open: bool,
    diagnostics_open: bool,
    sharing_setup: Option<SharingSetup>,
    roster_open: bool,
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
        let mut app = Self::with_account_client(browser_link, crate::account::Client::new());
        app.appearance = Appearance::load();
        if let Some(portuguese) = app.appearance.portuguese {
            crate::locale::select(portuguese);
        }
        app
    }

    fn with_account_client(
        browser_link: Option<String>,
        account_client: crate::account::Client,
    ) -> Self {
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
            following_client: crate::following::Client::new(),
            following: Default::default(),
            following_key: None,
            next_follow_attempt: Instant::now(),
            account_client,
            profile_connection_key: None,
            account_state: Default::default(),
            account_server: crate::settings::login_server()
                .unwrap_or_else(|| crate::account::DEFAULT_SERVER.into()),
            profile_client: None,
            profile_state: Default::default(),
            profile_textures: HashMap::new(),
            profile_started: Instant::now(),
            profile_open: false,
            profile_name: String::new(),
            saved_guest_name: crate::settings::guest_name(),
            updates: crate::updates::Client::new(),
            update_open: false,
            update_announced_version: None,
            update_restart_prompt: false,
            activation: crate::activation::Inbox::new(),
            prior_room: None,
            controller: Controller::new(),
            invitation: String::new(),
            name: crate::settings::guest_name(),
            session_name: None,
            source_id: String::new(),
            encoder: String::new(),
            quality: Settings::default(),
            camera_settings: CameraSettings {
                device_id: crate::settings::camera_device().unwrap_or_default(),
                ..Default::default()
            },
            custom_bitrate: false,
            download_budget: None,
            textures: HashMap::new(),
            focused: None,
            spotlight: None,
            self_preview_hidden: false,
            fullscreen: None,
            fullscreen_controls_until: Instant::now(),
            settings_error,
            auto_join_ready: false,
            copied_until: None,
            appearance: Appearance {
                theme: Theme::Amber,
                ..Appearance::default()
            },
            advanced_open: false,
            licenses_open: false,
            diagnostics_open: false,
            sharing_setup: None,
            roster_open: false,
            show_invitation: false,
            browser_invitation_open: is_browser_launch,
            pending_link: None,
            link_result,
            browser_links_enabled: std::env::var("PODETELAR_BROWSER_LINKS").as_deref() == Ok("1"),
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
        self.following.invitation();
        self.session_name = Some(self.name.trim().to_owned());
        self.prior_room = None;
        self.browser_invitation_open = false;
        self.show_invitation = false;
        self.auto_join_ready = false;
        self.controller.send(Command::Join {
            invitation: self.invitation.trim().to_owned(),
            name: self.name.trim().to_owned(),
            route: Route::Auto,
            account: self.account_state.credential.clone(),
            voice: None,
        });
    }

    fn room_header(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let connected = matches!(state.connection, "connected" | "reconnecting");
        let (header, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), egui::Sense::hover());
        let mut brand = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("header-brand")
                .max_rect(header)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        brand_mark(&mut brand);
        brand.label(skin::strong("PodeTelar", 18.0).color(palette().text));

        let mut actions = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("header-actions")
                .max_rect(header)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        {
            let ui = &mut actions;
            self.update_badge(ui);
            let mut pt = crate::locale::portuguese();
            egui::ComboBox::from_id_salt("ui-language")
                .width(68.0)
                .selected_text(if pt { "PT-BR" } else { "EN" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut pt, true, "Português (Brasil)");
                    ui.selectable_value(&mut pt, false, "English");
                })
                .response
                .accessible_name(format!(
                    "{} · {}",
                    tr("Language"),
                    if pt { "Português (Brasil)" } else { "English" }
                ))
                .on_hover_text(tr("Language"));
            if pt != crate::locale::portuguese() {
                crate::locale::select(pt);
                self.appearance.portuguese = Some(pt);
                self.save_appearance();
            }
            if state.connection == "connecting" {
                if ui.button(tr("Cancel join")).clicked() {
                    self.following.manual_leave(&self.following_client.state());
                    self.controller.send(Command::Leave);
                }
            } else if connected && ui.button(tr("Leave room")).clicked() {
                self.following.manual_leave(&self.following_client.state());
                self.controller.send(Command::Leave);
                self.textures.clear();
                self.sharing_setup = None;
                self.roster_open = false;
                self.focused = None;
                self.spotlight = None;
                self.self_preview_hidden = false;
            }
            let room_options = if connected {
                skin::icon_button(ui, Icon::More, tr("Room options"))
            } else {
                skin::icon_button(ui, Icon::Settings, tr("Settings"))
            };
            egui::Popup::menu(&room_options)
                .frame(skin::menu_frame())
                .show(|ui| {
                    if ui.button(tr("User settings")).clicked() {
                        self.open_profile();
                        ui.close();
                    }
                    ui.menu_button(tr("Theme"), |ui| self.theme_options(ui));
                    ui.separator();
                    if std::env::var_os(crate::activation_protocol::INSTALLED).is_some() {
                        ui.label(tr(if self.browser_links_enabled {
                            "Room links open this installed app automatically."
                        } else {
                            "Room links are handled by another application or need repair."
                        }));
                    }
                    if ui.button(tr("Licenses")).clicked() {
                        self.licenses_open = true;
                        ui.close();
                    }
                });
            if connected && !self.invitation.is_empty() {
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
        }
        if connected {
            let left = brand.min_rect().right() + 12.0;
            let right = actions.min_rect().left() - 12.0;
            let width = (right - left).clamp(0.0, 248.0);
            if width >= 80.0 {
                let center = header
                    .center()
                    .x
                    .clamp(left + width / 2.0, right - width / 2.0);
                let mut capsule = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("header-capsule")
                        .max_rect(egui::Rect::from_center_size(
                            egui::pos2(center, header.center().y),
                            egui::vec2(width, header.height()),
                        ))
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                self.room_capsule(&mut capsule, state, width);
            }
        }
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
        let width = (ctx.content_rect().width() - 80.0).clamp(240.0, 420.0);
        let modal = egui::Modal::new(egui::Id::unique("browser-invitation"))
            .backdrop_color(egui::Color32::from_black_alpha(175))
            .frame(
                egui::Frame::new()
                    .fill(palette().panel)
                    .stroke(egui::Stroke::new(1.0, palette().border))
                    .corner_radius(16)
                    .inner_margin(24)
                    .shadow(skin::menu_frame().shadow),
            )
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 12.0;
                ui.horizontal(|ui| {
                    ui.label(
                        skin::strong(tr("Invitation from your browser"), 19.0)
                            .color(palette().text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        dismiss |= skin::icon_button(ui, Icon::Close, tr("Close")).clicked();
                    });
                });
                if self.link_result.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(tr("Loading browser invitation…"));
                    });
                    ctx.request_repaint_after(Duration::from_millis(50));
                } else if !self.invitation.is_empty() {
                    if matches!(state.connection, "connected" | "reconnecting") {
                        ui.colored_label(
                            palette().warning,
                            tr("Joining this room will end your current sharing and watching."),
                        );
                    }
                    self.join_form(ui, state);
                }
                if !self.settings_error.is_empty() {
                    ui.colored_label(palette().error, tr(&self.settings_error));
                }
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        dismiss |= ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(tr("Dismiss browser invitation"))
                                        .size(12.5)
                                        .color(palette().muted),
                                )
                                .frame(false),
                            )
                            .clicked();
                    });
                });
            });
        if dismiss || modal.should_close() {
            self.dismiss_browser_invitation();
        }
    }

    fn join_form(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        if self.browser_invitation_open {
            if let Ok(invitation) = url::Url::parse(&self.invitation) {
                badge(
                    ui,
                    &invitation.origin().ascii_serialization(),
                    palette().muted,
                );
            }
            let label = ui.label(
                egui::RichText::new(tr("Your name"))
                    .size(12.0)
                    .color(palette().muted),
            );
            let name = ui
                .add(
                    egui::TextEdit::singleline(&mut self.name)
                        .id_salt("browser-display-name")
                        .interactive(!state.busy)
                        .char_limit(60)
                        .desired_width(f32::INFINITY)
                        .margin(egui::Margin::symmetric(10, 7))
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
            return;
        }
        ui.spacing_mut().item_spacing.y = 6.0;
        let invitation_label = ui
            .horizontal(|ui| {
                let label = ui.label(
                    egui::RichText::new(tr("Invitation link"))
                        .size(12.0)
                        .color(palette().muted),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            !state.busy,
                            egui::Button::new(
                                egui::RichText::new(if self.show_invitation {
                                    tr("Hide")
                                } else {
                                    tr("Show")
                                })
                                .size(11.5)
                                .color(palette().accent),
                            )
                            .frame(false),
                        )
                        .on_hover_text(tr(
                            "Reveal this private invitation only when it is safe to show your screen",
                        ))
                        .clicked()
                    {
                        self.show_invitation = !self.show_invitation;
                    }
                });
                label
            })
            .inner;
        let invitation = ui
            .add(
                egui::TextEdit::singleline(&mut self.invitation)
                    .id_salt("room-invitation")
                    .password(!self.show_invitation)
                    .interactive(!state.busy)
                    .char_limit(2048)
                    .desired_width(f32::INFINITY)
                    .margin(egui::Margin::symmetric(10, 7))
                    .hint_text(tr("Paste your private invitation link")),
            )
            .labelled_by(invitation_label.id);
        ui.add_space(4.0);
        let name_label = ui.label(
            egui::RichText::new(tr("Your name"))
                .size(12.0)
                .color(palette().muted),
        );
        let name = ui
            .add(
                egui::TextEdit::singleline(&mut self.name)
                    .id_salt("display-name")
                    .interactive(!state.busy)
                    .char_limit(60)
                    .desired_width(f32::INFINITY)
                    .margin(egui::Margin::symmetric(10, 7))
                    .hint_text(tr("How friends will see you")),
            )
            .labelled_by(name_label.id);
        ui.add_space(6.0);
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
                egui::Button::new((
                    egui::Atom::grow(),
                    skin::strong(tr("Join room"), 13.5).color(palette().text),
                    egui::Atom::grow(),
                ))
                .fill(if valid {
                    palette().control_hover
                } else {
                    palette().card
                })
                .stroke(egui::Stroke::new(
                    1.0,
                    if valid {
                        palette().accent.gamma_multiply(0.55)
                    } else {
                        palette().border
                    },
                ))
                .corner_radius(8)
                .min_size(egui::vec2(ui.available_width(), 38.0)),
            )
            .clicked()
            || (valid && enter)
        {
            self.join();
        }
    }

    fn selection(&self) -> SharingSelection {
        SharingSelection {
            source_id: self.source_id.clone(),
            encoder: self.encoder.clone(),
            quality: self.quality,
        }
    }

    fn source_preview(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        self.refresh_self_preview(ui.ctx(), state);
        let width = ui.available_width();
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(width, width * 9.0 / 16.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 6.0, palette().canvas);
        if state.sharing
            && let Some(texture) = self.textures.get(SELF_PREVIEW)
        {
            let target = fit_picture(rect, texture.aspect_ratio());
            ui.painter()
                .image(texture.id(), target, uv(), egui::Color32::WHITE);
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                tr(if state.camera_sharing && !state.sharing {
                    "Your camera"
                } else {
                    "Your screen"
                }),
                egui::FontId::proportional(14.0),
                palette().muted,
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
        let response = list
            .response
            .accessible_name(format!(
                "{} · {}",
                tr("Screen or window"),
                selected
                    .as_ref()
                    .map(source_name)
                    .unwrap_or(tr("Choose a source"))
            ))
            .on_hover_text(tr("Screen or window"));
        if list.inner.is_none() {
            self.clear_picker();
            self.picker_refreshed = None;
        }
        response
    }

    fn update_picker_frame(&mut self, ctx: &egui::Context, key: &str) {
        if self
            .picker_texture
            .as_ref()
            .is_some_and(|(id, _)| id != key)
        {
            self.picker_texture = None;
        }
        if let Some(frame) = self.picker.take()
            && frame.source_id == key
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
                self.picker_failed = Some(frame.source_id);
            }
        }
    }

    fn source_popover(
        &mut self,
        ctx: &egui::Context,
        anchor: egui::Rect,
        source: &crate::sources::SourceInfo,
        sharing: bool,
    ) {
        self.update_picker_frame(ctx, &source.id);
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
                        let symbol = if source.id.starts_with("window:") {
                            Icon::Window
                        } else {
                            Icon::Monitor
                        };
                        skin::icon(ui.painter(), icon, symbol, palette().text);
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
                                .color(palette().muted),
                            );
                        });
                    });
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(width, width * 9.0 / 16.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(rect, 6.0, palette().canvas);
                    if let Some((_, texture)) = &self.picker_texture {
                        egui::Image::new((texture.id(), texture.size_vec2()))
                            .corner_radius(6)
                            .paint_at(ui, fit_picture(rect, texture.aspect_ratio()));
                    } else {
                        let message = if self.picker_failed.as_deref() == Some(&source.id) {
                            tr("Preview unavailable. Refresh sources to try again.")
                        } else {
                            tr("Loading preview…")
                        };
                        let text = ui.painter().layout(
                            message.to_owned(),
                            egui::FontId::proportional(12.0),
                            palette().muted,
                            rect.width() - 32.0,
                        );
                        ui.painter().galley(
                            rect.center() - text.size() / 2.0,
                            text,
                            palette().muted,
                        );
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
                        .color(palette().muted),
                    );
                });
            });
    }

    fn sharing_menu(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        ui.spacing_mut().item_spacing.y = 7.0;
        let encoder = state.encoders.iter().find(|e| e.factory == self.encoder);
        let source = state.sources.iter().find(|s| s.id == self.source_id);
        ui.add_enabled_ui(!state.busy, |ui| {
            self.source_selector(ui, state);
            if let Some(encoder) = encoder {
                quality_controls(ui, &mut self.quality, source, encoder, &state.stream_policy);
            } else {
                ui.colored_label(
                    palette().warning,
                    tr("No supported hardware encoder was found. You can still watch compatible streams."),
                );
            }
        });
        if state.sharing {
            egui::CollapsingHeader::new(tr("Source preview"))
                .default_open(ui.available_height() > 460.0)
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
                state
                    .stream_policy
                    .resolve(self.quality, s, e.codec)
                    .is_ok_and(|mode| e.supports(mode))
            })
    }

    fn sharing_actions(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let valid = self.valid_selection(state);
        ui.add_space(3.0);
        ui.separator();
        if state.sharing || state.capture_pending {
            let selection = self.selection();
            let dirty = state
                .active_selection
                .as_ref()
                .is_some_and(|active| active != &selection);
            let width = ui.available_width();
            let apply_label = if dirty {
                tr("Apply")
            } else {
                tr("No changes to apply")
            };
            let label = skin::strong(apply_label, 13.5).color(if dirty {
                palette().on_accent
            } else {
                palette().muted
            });
            let apply = egui::Button::new((egui::Atom::grow(), label, egui::Atom::grow()))
                .corner_radius(8)
                .fill(if dirty {
                    palette().accent
                } else {
                    palette().card
                })
                .min_size(egui::vec2(width, 36.0));
            if ui
                .add_enabled(state.sharing && !state.busy && valid && dirty, apply)
                .clicked()
            {
                self.send_settings(true);
            }
            if dirty {
                let live = state.active_selection.as_ref().is_some_and(|a| {
                    a.source_id == selection.source_id
                        && a.encoder == selection.encoder
                        && a.quality.resolution == selection.quality.resolution
                        && a.quality.fps == selection.quality.fps
                });
                ui.colored_label(
                    palette().warning,
                    if live {
                        tr("Changes pending · applies live")
                    } else {
                        tr("Changes pending · restarts stream")
                    },
                );
            }
        } else if ui
            .add_enabled(
                state.connection == "connected"
                    && !state.busy
                    && !state.sharing
                    && !state.capture_pending
                    && valid,
                primary(tr("Start sharing"), ui.available_width()),
            )
            .clicked()
        {
            self.send_settings(false);
            self.sharing_setup = None;
        }
    }

    fn advanced(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        ui.spacing_mut().item_spacing.y = if ui.ctx().content_rect().height() <= 600.0 {
            6.0
        } else {
            10.0
        };
        ui.label(skin::strong(tr("Video"), 17.5).color(palette().accent));
        ui.add_space(2.0);
        ui.add_enabled_ui(!state.busy, |ui| {
            ui.columns(2, |columns| {
                let ui = &mut columns[0];
                let label = skin::help_label(
                    ui,
                    tr("Encoder"),
                    tr("AV1 → HEVC → H.264, using hardware support from everyone in the room. Interruptions do not change codecs."),
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
                bitrate_controls(&mut columns[1], &mut self.quality, &mut self.custom_bitrate);
            });
        });
        settings_divider(ui);
        ui.label(skin::strong(tr("Reception"), 17.5).color(palette().accent));
        ui.add_space(2.0);
        ui.add_enabled_ui(!state.busy, |ui| {
            let label = skin::help_label(
                ui,
                tr("Download bandwidth limit"),
                tr("Automatic uses measured download capacity with headroom, or 30 Mbps if unavailable. Shared across everything you watch."),
            );
            let before = self.download_budget;
            egui::ComboBox::from_id_salt("watching-allowance")
                .width(ui.available_width())
                .selected_text(
                    self.download_budget
                        .map(|bps| format!("{} Mbps", bps / 1_000_000))
                        .unwrap_or_else(|| {
                            format!(
                                "{} · {:.0} Mbps",
                                tr("Automatic"),
                                state
                                    .capacity
                                    .watching_allowance()
                                    .unwrap_or(crate::network::DEFAULT_DOWNLOAD_BPS)
                                    as f64
                                    / 1_000_000.0
                            )
                        }),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.download_budget, None, tr("Automatic"));
                    for mbps in [1, 3, 5, 10, 25, 30, 50, 100, 200] {
                        ui.selectable_value(
                            &mut self.download_budget,
                            Some(mbps * 1_000_000),
                            format!("{mbps} Mbps"),
                        );
                    }
                })
                .response
                .labelled_by(label.id);
            if before != self.download_budget {
                self.controller.send(Command::DownloadBudget {
                    bps: self.download_budget,
                });
            }
        });
        settings_divider(ui);
        ui.horizontal(|ui| {
            let failed = !state.capacity.error.is_empty();
            ui.vertical(|ui| {
                ui.label(skin::strong(tr("Connection"), 17.5).color(palette().accent));
                ui.horizontal(|ui| {
                    if state.testing_connection {
                        ui.spinner();
                    }
                    let status = if state.testing_connection {
                        tr("Measuring connection…")
                    } else if failed {
                        tr("Test unavailable")
                    } else if state.capacity.download_bps.is_some() {
                        tr("Connection measured")
                    } else {
                        tr("Not measured")
                    };
                    ui.colored_label(
                        if failed {
                            palette().warning
                        } else {
                            palette().muted
                        },
                        status,
                    );
                });
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        state.connection == "connected" && !state.testing_connection && !state.busy,
                        egui::Button::new(tr(if state.capacity.download_bps.is_some() || failed {
                            "Test again"
                        } else {
                            "Test connection"
                        }))
                        .min_size(egui::vec2(146.0, 34.0)),
                    )
                    .clicked()
                {
                    self.controller.send(Command::MeasureConnection);
                }
            });
        });
        ui.add_space(2.0);
        skin::disclosure(ui, tr("Diagnostics"), &mut self.diagnostics_open);
        if self.diagnostics_open {
            ui.add_space(8.0);
            self.diagnostics(ui, state);
        }
    }

    fn save_appearance(&mut self) {
        if self.appearance.save().is_err() {
            self.settings_error = "Could not save appearance preferences.".into();
        } else if self.settings_error == "Could not save appearance preferences." {
            self.settings_error.clear();
        }
    }

    fn appearance_picker(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_label(tr("Theme"))
            .selected_text(tr(self.appearance.theme.label()))
            .width(156.0)
            .show_ui(ui, |ui| self.theme_options(ui));
    }

    fn theme_options(&mut self, ui: &mut egui::Ui) {
        let previous = self.appearance.theme;
        let system = ui.ctx().system_theme();
        for theme in Theme::ALL {
            let p = theme.resolve(system).palette();
            let swatch = egui::Atom::paint(egui::vec2(26.0, 16.0), |ui, args| {
                let swatch = args.rect;
                ui.painter().rect_filled(swatch, 4.0, p.canvas);
                ui.painter().rect_stroke(
                    swatch,
                    4.0,
                    (1.0, palette().hover_stroke),
                    egui::StrokeKind::Inside,
                );
                ui.painter().circle_filled(
                    swatch.right_center() - egui::vec2(7.0, 0.0),
                    3.0,
                    p.accent,
                );
            });
            let selected = self.appearance.theme == theme;
            let row = egui::Button::selectable(selected, (swatch, tr(theme.label())))
                .gap(8.0)
                .min_size(egui::vec2(0.0, 32.0));
            if ui.add(row).clicked() {
                self.appearance.theme = theme;
                ui.close();
            }
        }
        if self.appearance.theme != previous {
            self.save_appearance();
            ui.ctx().request_repaint();
        }
    }

    fn licenses_dialog(&mut self, ctx: &egui::Context) {
        if !self.licenses_open || self.browser_invitation_open {
            return;
        }
        const FOLDER_ERROR: &str = "Could not open the bundled notices folder.";
        let modal = egui::Modal::new(egui::Id::unique("licenses"))
            .frame(skin::menu_frame().inner_margin(16))
            .show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 48.0).clamp(200.0, 480.0));
                let mut close = false;
                ui.horizontal(|ui| {
                    ui.heading(tr("Licenses"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        close = skin::icon_button(ui, Icon::Close, tr("Close")).clicked();
                    });
                });
                if let Some(notices) = bundled_notices()
                    && ui.button(tr("Open bundled notices and sources")).clicked()
                {
                    if open_notices_folder(&notices).is_err() {
                        self.settings_error = FOLDER_ERROR.into();
                    } else {
                        close = true;
                    }
                }
                if self.settings_error == FOLDER_ERROR {
                    ui.colored_label(palette().error, tr(FOLDER_ERROR));
                }
                ui.separator();
                ui.label(skin::strong(tr("Inter font license"), 14.0));
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 208.0).clamp(96.0, 380.0))
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(include_str!("../../../assets/fonts/OFL-Inter.txt"))
                                .selectable(true)
                                .wrap(),
                        );
                    });
                close
            });
        if modal.inner || modal.should_close() {
            self.licenses_open = false;
            if self.settings_error == FOLDER_ERROR {
                self.settings_error.clear();
            }
        }
    }

    fn advanced_modal(&mut self, ctx: &egui::Context, state: &Snapshot) {
        if self.advanced_open && !self.browser_invitation_open {
            self.clear_picker();
            let mut done = false;
            let compact = ctx.content_rect().height() <= 600.0;
            let width = (ctx.viewport_rect().width() - 72.0).clamp(420.0, 620.0);
            let modal = egui::Modal::new(egui::Id::unique("advanced-settings"))
                .backdrop_color(egui::Color32::from_black_alpha(170))
                .frame(
                    egui::Frame::new()
                        .fill(palette().panel)
                        .stroke(egui::Stroke::new(1.0, palette().border))
                        .corner_radius(16)
                        .inner_margin(if compact { 16 } else { 24 })
                        .shadow(skin::menu_frame().shadow),
                )
                .show(ctx, |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = if compact { 6.0 } else { 12.0 };
                    ui.horizontal(|ui| {
                        ui.label(skin::strong(tr("Advanced settings"), 22.0).color(palette().text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if skin::icon_button(ui, Icon::Close, tr("Close")).clicked() {
                                done = true;
                            }
                        });
                    });
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical()
                        .id_salt("advanced-scroll")
                        .max_height(
                            (ctx.viewport_rect().height() - if compact { 170.0 } else { 230.0 })
                                .clamp(180.0, 560.0),
                        )
                        .show(ui, |ui| {
                            self.advanced(ui, state);
                            settings_divider(ui);
                            self.appearance_picker(ui);
                        });
                    settings_divider(ui);
                    ui.horizontal(|ui| {
                        let dirty = state.sharing
                            && state
                                .active_selection
                                .as_ref()
                                .is_some_and(|active| active != &self.selection());
                        ui.allocate_ui_with_layout(
                            egui::vec2(width - 186.0, 44.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_min_width(width - 186.0);
                                ui.label(
                                    egui::RichText::new(tr(if dirty {
                                        "Updates your current stream."
                                    } else {
                                        "Ready for your next stream."
                                    }))
                                    .size(12.0)
                                    .color(palette().muted),
                                );
                            },
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
            egui::RichText::new(tr("Capacity to this server · payload capped at 200 Mbps"))
                .size(12.0)
                .color(palette().muted),
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
            ui.colored_label(palette().warning, tr(&state.capacity.error));
            if state.capacity.download_bps.is_some() || state.capacity.upload_bps.is_some() {
                ui.small(tr("Incomplete test; showing last available measurements."));
            }
        }
        ui.label(egui::RichText::new(tr("A slower watcher's limit can reduce the one shared stream for everyone watching it.")).size(12.0).color(palette().muted));
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
        if state.any_sharing() {
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

    fn stream_metrics(ui: &egui::Ui, rect: egui::Rect, tile: &Tile, visible: bool) -> egui::Rect {
        let text = stream_metrics_text(&tile.codec, tile.received_bps.load(Ordering::Relaxed));
        let galley = ui.painter().layout_no_wrap(
            text,
            egui::FontId::proportional(12.0),
            egui::Color32::WHITE,
        );
        let size = galley.size() + egui::vec2(12.0, 8.0);
        let background =
            egui::Rect::from_min_size(rect.right_bottom() - egui::vec2(10.0, 8.0) - size, size);
        if visible {
            ui.painter()
                .rect_filled(background, 4.0, egui::Color32::from_black_alpha(195));
            ui.painter().galley(
                background.min + egui::vec2(6.0, 4.0),
                galley,
                egui::Color32::WHITE,
            );
        }
        background
    }

    fn watch_menu(&mut self, ui: &mut egui::Ui, tile: &Tile) {
        ui.set_width(238.0);
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);
        ui.spacing_mut().interact_size.y = 24.0;
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(skin::strong(stream_name(tile).as_ref(), 12.0).color(palette().muted));
        });
        ui.add_space(5.0);
        if tile.kind == crate::preview::Kind::Screen && !tile.watching {
            if skin::menu_row(ui, tr("Watch"), Some(Icon::Play), "", false)
                .accessible_name(format!("{} · {}", tr("Watch"), stream_name(tile)))
                .clicked()
            {
                self.controller.send(Command::Watch {
                    id: tile.id.clone(),
                    kind: tile.kind,
                    enabled: true,
                });
                ui.close();
            }
            return;
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
            self.spotlight = None;
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
            || ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F11))
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
        if tile.kind == crate::preview::Kind::Screen
            && skin::menu_row(ui, tr("Stop watching"), Some(Icon::Stop), "", true).clicked()
        {
            self.stop_watching(ui.ctx(), tile);
            ui.close();
        }
    }

    fn stop_watching(&mut self, ctx: &egui::Context, tile: &Tile) {
        self.controller.send(Command::Watch {
            id: tile.id.clone(),
            kind: tile.kind,
            enabled: false,
        });
        self.textures.remove(&tile.id);
        if self.fullscreen.as_ref() == Some(&tile.id) {
            self.set_fullscreen(ctx, None);
        }
    }

    fn playback_controls(&mut self, ui: &mut egui::Ui, tile: &Tile, fullscreen: bool) {
        let overlay = palette().canvas;
        egui::Frame::new()
            .fill(overlay)
            .stroke(egui::Stroke::new(1.0, palette().border))
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let label = tr(if fullscreen {
                        "Exit full screen"
                    } else {
                        "Full screen"
                    });
                    if skin::icon_button(ui, Icon::Fullscreen, &format!("{label} · {}", tile.name))
                        .clicked()
                    {
                        self.set_fullscreen(ui.ctx(), (!fullscreen).then(|| tile.id.clone()));
                    }
                    if tile.kind == crate::preview::Kind::Screen
                        && skin::icon_button_with_color(
                            ui,
                            Icon::Stop,
                            &format!("{} · {}", tr("Stop watching"), tile.name),
                            palette().error,
                        )
                        .clicked()
                    {
                        self.stop_watching(ui.ctx(), tile);
                    }
                    if tile.kind == crate::preview::Kind::Screen && Self::volume_button(ui, tile) {
                        self.set_fullscreen(ui.ctx(), (!fullscreen).then(|| tile.id.clone()));
                    }
                });
            });
    }

    fn volume_button(ui: &mut egui::Ui, tile: &Tile) -> bool {
        let mut volume = tile.volume.load(Ordering::Relaxed);
        let label = format!("{} · {}", tr("Volume"), tile.name);
        let trigger = skin::icon_button(
            ui,
            if volume == 0 {
                Icon::Muted
            } else {
                Icon::Volume
            },
            &label,
        );
        let mut fullscreen_shortcut = false;
        egui::Popup::from_toggle_button_response(&trigger)
            .align(egui::RectAlign::TOP)
            .gap(6.0)
            .width(46.0)
            .layout(egui::Layout::top_down(egui::Align::Center))
            .frame(
                skin::menu_frame()
                    .corner_radius(15)
                    .inner_margin(egui::Margin::symmetric(6, 8)),
            )
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(32.0);
                ui.label(skin::strong(format!("{volume}%"), 11.5));
                let slider_size = egui::vec2(22.0, 70.0);
                let changed = ui
                    .allocate_ui_with_layout(
                        slider_size,
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            let p = palette();
                            ui.spacing_mut().interact_size.y = slider_size.x;
                            ui.spacing_mut().slider_width = slider_size.y;
                            ui.spacing_mut().slider_rail_height = 3.0;
                            let widgets = &mut ui.visuals_mut().widgets;
                            widgets.inactive.bg_fill = p.control_hover;
                            widgets.inactive.fg_stroke = egui::Stroke::new(3.0, p.accent);
                            for widget in [&mut widgets.hovered, &mut widgets.active] {
                                widget.bg_fill = p.accent;
                                widget.fg_stroke = egui::Stroke::NONE;
                            }
                            ui.add(
                                egui::Slider::new(&mut volume, 0..=100)
                                    .vertical()
                                    .show_value(false)
                                    .handle_shape(egui::style::HandleShape::Rect {
                                        aspect_ratio: 0.3,
                                    }),
                            )
                            .accessible_name(tr("Volume"))
                            .changed()
                        },
                    )
                    .inner;
                if changed {
                    tile.volume.store(volume, Ordering::Relaxed);
                }
                if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F11)) {
                    fullscreen_shortcut = true;
                    ui.close();
                }
            });
        fullscreen_shortcut
    }

    fn stream_context_menu(
        &mut self,
        response: &egui::Response,
        tile: &Tile,
        primary: bool,
    ) -> bool {
        if primary {
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::Role::Button,
                    response.enabled(),
                    format!("{} · {}", tr("Stream options"), tile.name),
                )
            });
        }
        let position_id = response.id.with("stream-menu-position");
        let open = if primary && response.double_clicked() {
            Some(egui::SetOpenCommand::Bool(false))
        } else if response.secondary_clicked() || (primary && response.clicked()) {
            let position = response
                .interact_pointer_pos()
                .unwrap_or(response.rect.center());
            response
                .ctx
                .data_mut(|data| data.insert_temp(position_id, position));
            Some(egui::SetOpenCommand::Bool(true))
        } else {
            None
        };
        let position = response
            .ctx
            .data(|data| data.get_temp(position_id))
            .unwrap_or(response.rect.center());
        egui::Popup::menu(response)
            .open_memory(open)
            .anchor(egui::PopupAnchor::Position(position))
            .gap(8.0)
            .frame(skin::menu_frame())
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| self.watch_menu(ui, tile))
            .is_some()
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
        let hud = egui::Id::unique("fullscreen-controls");
        let hud_focused = ctx
            .memory(|memory| memory.focused())
            .and_then(|id| ctx.read_response(id))
            .is_some_and(|control| control.layer_id.id == hud && control.has_focus());
        let controls_visible = Instant::now() < self.fullscreen_controls_until
            || hud_focused
            || egui::Popup::is_any_open(&ctx);
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
                menu_open = self.stream_context_menu(&response, tile, true);
                if controls_visible {
                    if ui.input(|i| i.pointer.hover_pos().is_some_and(|pos| rect.contains(pos))) {
                        Self::stream_metrics(ui, rect, tile, true);
                    }
                    egui::Area::new(hud)
                        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(12.0, -12.0))
                        .order(egui::Order::Foreground)
                        .show(&ctx, |ui| self.playback_controls(ui, tile, true));
                }
                if !tile.status.is_empty() {
                    ui.painter().text(
                        rect.center_top() + egui::vec2(0.0, 24.0),
                        egui::Align2::CENTER_TOP,
                        tr(&tile.status),
                        egui::FontId::proportional(16.0),
                        palette().warning,
                    );
                }
            });
        if !controls_visible && !menu_open && self.fullscreen.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::None);
        }
    }

    fn stream_tile(&mut self, ui: &mut egui::Ui, tile: &Tile, width: f32) {
        self.stream_tile_with_layout(ui, tile, width, false);
    }

    fn stream_tile_with_layout(
        &mut self,
        ui: &mut egui::Ui,
        tile: &Tile,
        width: f32,
        compact: bool,
    ) {
        ui.push_id(&tile.id, |ui| {
            let can_open_menu = tile.watching && !compact;
            let sense = if can_open_menu {
                egui::Sense::click()
            } else {
                egui::Sense::CLICK
            };
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, width * 9.0 / 16.0), sense);
            #[cfg(feature = "performance-experiments")]
            let present = {
                use std::sync::OnceLock;
                static CLIP: OnceLock<bool> = OnceLock::new();
                let visible = ui.is_rect_visible(rect);
                tile.presentation.visible.store(visible, Ordering::Relaxed);
                !*CLIP.get_or_init(|| {
                    std::env::var("PODETELAR_DEEP_CLIP")
                        .ok()
                        .is_none_or(|v| v != "0")
                }) || visible
            };
            #[cfg(not(feature = "performance-experiments"))]
            let present = ui.is_rect_visible(rect);
            if present {
                self.update_picture(ui.ctx(), tile);
            }
            // The entire video is the hover target, including its overlay buttons.
            let hovered =
                ui.input(|i| i.pointer.hover_pos().is_some()) && ui.rect_contains_pointer(rect);
            let overlay_visible = !tile.watching || stream_overlay_visible(ui, rect, hovered);
            ui.painter().rect_filled(rect, 12.0, egui::Color32::BLACK);
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
                    .corner_radius(12)
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
                    egui::FontId::proportional(13.5),
                    egui::Color32::LIGHT_GRAY,
                );
            }
            ui.painter().rect_stroke(
                rect,
                12.0,
                egui::Stroke::new(
                    1.0,
                    if tile.watching && hovered && !compact {
                        palette().accent.gamma_multiply(0.55)
                    } else {
                        palette().border
                    },
                ),
                egui::StrokeKind::Inside,
            );
            if tile.watching && response.double_clicked() {
                self.set_fullscreen(ui.ctx(), Some(tile.id.clone()));
            }
            if !tile.watching {
                ui.painter()
                    .rect_filled(rect, 12.0, egui::Color32::from_black_alpha(68));
            }
            let overlay = palette().canvas;
            if compact {
                let title_bounds = egui::Rect::from_min_size(
                    rect.min + egui::vec2(6.0, 6.0),
                    egui::vec2(width - 12.0, 25.0),
                );
                let opacity = ui.painter().opacity();
                if !overlay_visible {
                    ui.set_opacity(0.0);
                }
                let title = ui.put(
                    title_bounds,
                    egui::Button::new(skin::strong(stream_name(tile).as_ref(), 12.5))
                        .truncate()
                        .fill(overlay),
                );
                ui.set_opacity(opacity);
                if title
                    .accessible_name(format!(
                        "{} · {}",
                        tr("Spotlight stream"),
                        stream_name(tile)
                    ))
                    .on_hover_text(stream_name(tile).as_ref())
                    .clicked()
                    || response.clicked()
                {
                    self.spotlight = Some(tile.id.clone());
                }
            } else {
                let p = palette();
                let texture = if overlay_visible {
                    self.profile_texture(ui.ctx(), tile.owner())
                } else {
                    None
                };
                let (status, status_color) = if !tile.watching {
                    (tr("Preview"), p.muted)
                } else if tile.frames.load(Ordering::Relaxed) > 0 {
                    (tr("Watching"), p.success)
                } else {
                    (tr("Connecting"), p.warning)
                };
                let status_font = egui::FontId::proportional(11.5);
                let max_width = width - 24.0;
                let show_status = max_width >= 164.0;
                let status_width = if show_status {
                    ui.painter()
                        .layout_no_wrap(status.to_owned(), status_font.clone(), p.muted)
                        .size()
                        .x
                        + 7.0
                } else {
                    0.0
                };
                // Frame padding, avatar, status dot, and the two shared gaps.
                let fixed_width = 16.0 + 20.0 + 6.0 + 14.0;
                let max_name_width = (max_width - fixed_width - status_width).clamp(24.0, 160.0);
                let name = stream_name(tile);
                let name_width = ui
                    .painter()
                    .layout_no_wrap(name.to_string(), skin::semibold(13.0), p.text)
                    .size()
                    .x
                    .clamp(24.0, max_name_width);
                let identity = egui::Rect::from_min_size(
                    rect.min + egui::vec2(12.0, 12.0),
                    egui::vec2(name_width + fixed_width + status_width, 32.0),
                );
                let mut header = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("identity")
                        .max_rect(identity),
                );
                if !overlay_visible {
                    header.set_opacity(0.0);
                }
                egui::Frame::new()
                    .fill(p.card.gamma_multiply(0.96))
                    .stroke(egui::Stroke::new(1.0, p.border))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::symmetric(7, 4))
                    .show(&mut header, |ui| {
                        ui.spacing_mut().item_spacing.x = 7.0;
                        ui.spacing_mut().interact_size.y = 22.0;
                        ui.horizontal(|ui| {
                            profile::avatar(ui, &tile.name, 20.0, texture);
                            ui.add_sized(
                                [name_width, 22.0],
                                egui::Label::new(skin::strong(name.as_ref(), 13.0).color(p.text))
                                    .truncate()
                                    .halign(egui::Align::Min),
                            )
                            .on_hover_text(name.as_ref());
                            let (dot, response) =
                                ui.allocate_exact_size(egui::vec2(6.0, 12.0), egui::Sense::hover());
                            ui.painter().circle_filled(dot.center(), 2.5, status_color);
                            if show_status {
                                ui.label(
                                    egui::RichText::new(status).font(status_font).color(p.muted),
                                );
                            } else {
                                response.accessible_name(status).on_hover_text(status);
                            }
                        });
                    });
            }
            if !tile.watching {
                let watch_width = if compact {
                    width.min(120.0) - 12.0
                } else {
                    132.0
                };
                let target = egui::Rect::from_center_size(
                    rect.center() + egui::vec2(0.0, if compact { 10.0 } else { 0.0 }),
                    egui::vec2(watch_width, 38.0),
                );
                if ui
                    .put(target, primary(tr("Watch"), watch_width))
                    .accessible_name(format!("{} · {}", tr("Watch"), stream_name(tile)))
                    .clicked()
                {
                    self.controller.send(Command::Watch {
                        id: tile.id.clone(),
                        kind: tile.kind,
                        enabled: true,
                    });
                }
            }
            if tile.watching {
                let metrics = (!compact).then(|| Self::stream_metrics(ui, rect, tile, hovered));
                let left = rect.left() + 12.0;
                let bottom = metrics
                    .filter(|badge| badge.left() - left < PLAYBACK_CONTROLS_WIDTH + 8.0)
                    .map_or(rect.bottom() - 8.0, |badge| badge.top() - 4.0);
                let controls = egui::Rect::from_min_size(
                    egui::pos2(left, bottom - 38.0),
                    egui::vec2(PLAYBACK_CONTROLS_WIDTH, 38.0),
                );
                let mut hud = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("stream-controls")
                        .max_rect(controls),
                );
                hud.set_clip_rect(rect.intersect(ui.clip_rect()));
                if !overlay_visible {
                    // Keep the controls in the tab order so focus can reveal them.
                    hud.set_opacity(0.0);
                }
                self.playback_controls(&mut hud, tile, false);
            }
            self.stream_context_menu(&response, tile, can_open_menu);
            if !compact && !tile.status.is_empty() {
                let notice = egui::Rect::from_min_max(
                    rect.min + egui::vec2(12.0, 54.0),
                    egui::pos2(rect.right() - 12.0, rect.top() + 98.0),
                );
                ui.painter().rect_filled(notice, 8.0, overlay);
                let mut notice_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("playback-status")
                        .max_rect(notice.shrink(6.0)),
                );
                notice_ui.label(
                    egui::RichText::new(tr(&tile.status))
                        .size(12.0)
                        .color(palette().warning),
                );
            }
            ui.advance_cursor_after_rect(rect);
        });
    }
}

fn stream_overlay_visible(ui: &egui::Ui, rect: egui::Rect, hovered: bool) -> bool {
    if hovered {
        return true;
    }
    let Some(focus) = ui
        .memory(|memory| memory.focused())
        .and_then(|id| ui.ctx().read_response(id))
        .filter(|focus| focus.layer_id == ui.layer_id() && rect.contains_rect(focus.rect))
    else {
        return false;
    };
    let pointer_moved = ui.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::PointerMoved(_) | egui::Event::PointerGone
            )
        })
    });
    if pointer_moved {
        focus.surrender_focus();
    }
    !pointer_moved
}

impl PublisherUi {
    fn update_badge(&mut self, ui: &mut egui::Ui) {
        use crate::update_protocol::Phase;
        let status = self.updates.snapshot();
        if status.phase == Phase::Disabled {
            return;
        }
        let label = match status.phase {
            Phase::Checking => "Checking for updates…",
            Phase::Current => "Up to date",
            Phase::Available => "Update available",
            Phase::Downloading => "Downloading update…",
            Phase::Ready => "Restart to update",
            Phase::Restarting => "Restarting…",
            Phase::Error => "Retry update check",
            Phase::Disabled => unreachable!(),
        };
        let attention = matches!(status.phase, Phase::Available | Phase::Ready);
        let response = ui.add_sized(
            [32.0, 32.0],
            egui::Button::new(egui::RichText::new("↻").color(if attention {
                palette().accent
            } else {
                palette().text
            }))
            .corner_radius(8)
            .fill(if attention {
                palette().accent.gamma_multiply(0.15)
            } else {
                palette().card
            })
            .stroke(egui::Stroke::new(
                1.0,
                if attention {
                    palette().accent.gamma_multiply(0.60)
                } else {
                    palette().border
                },
            )),
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tr(label)));
        response.clone().on_hover_text(tr(label));
        if status.phase == Phase::Downloading {
            ui.label(format!("{}%", status.progress));
        }
        if response.clicked() {
            self.update_open = true;
        }
    }

    fn update_dialog(&mut self, ctx: &egui::Context) {
        use crate::update_protocol::{Action, Phase};
        let status = self.updates.snapshot();
        if status.phase == Phase::Disabled {
            self.update_open = false;
            self.update_restart_prompt = false;
            return;
        }
        if self.closing_since.is_some()
            || self.browser_invitation_open
            || self.advanced_open
            || self.licenses_open
            || self.profile_open
            || self.fullscreen.is_some()
        {
            return;
        }
        if matches!(status.phase, Phase::Available | Phase::Ready)
            && self.update_announced_version.as_ref() != Some(&status.version)
        {
            self.update_announced_version = Some(status.version.clone());
            self.update_open = true;
        }
        if !self.update_open {
            return;
        }
        self.clear_picker();
        if self.update_restart_prompt {
            self.update_restart_dialog(ctx);
            return;
        }
        let width = (ctx.content_rect().width() - 96.0).clamp(240.0, 520.0);
        let mut dismiss = false;
        let modal = egui::Modal::new(egui::Id::unique("app-update"))
            .backdrop_color(egui::Color32::from_black_alpha(170))
            .frame(
                egui::Frame::new()
                    .fill(palette().panel)
                    .stroke(egui::Stroke::new(1.0, palette().border))
                    .corner_radius(16)
                    .inner_margin(24)
                    .shadow(skin::menu_frame().shadow),
            )
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 12.0;
                ui.horizontal(|ui| {
                    ui.label(skin::strong(tr("App updates"), 22.0).color(palette().text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        dismiss |= skin::icon_button(ui, Icon::Close, tr("Close")).clicked();
                    });
                });
                let label = match status.phase {
                    Phase::Available => "Update available",
                    Phase::Ready => "Restart to update",
                    Phase::Checking => "Checking for updates…",
                    Phase::Current => "Up to date",
                    Phase::Downloading => "Downloading update…",
                    Phase::Restarting => "Restarting…",
                    Phase::Error => "Retry update check",
                    Phase::Disabled => unreachable!(),
                };
                ui.horizontal(|ui| {
                    badge(ui, tr(label), palette().accent);
                    if !status.version.is_empty() {
                        ui.label(
                            skin::strong(format!("{} {}", tr("New version"), status.version), 13.0)
                                .color(palette().muted),
                        );
                    }
                });
                egui::Frame::new()
                    .fill(palette().card)
                    .stroke(egui::Stroke::new(1.0, palette().border))
                    .corner_radius(10)
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("update-notes")
                            .max_height((ctx.content_rect().height() - 272.0).clamp(80.0, 380.0))
                            .show(ui, |ui| {
                                ui.set_width((width - 46.0).max(120.0));
                                if status.notes.trim().is_empty() {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(tr("No release notes provided."))
                                                .color(palette().muted),
                                        )
                                        .wrap(),
                                    );
                                } else {
                                    markdown::render(ui, &status.notes);
                                }
                            });
                    });
                if !status.message.is_empty() {
                    ui.add(egui::Label::new(tr(&status.message)).wrap());
                }
                if status.phase == Phase::Downloading {
                    ui.add(
                        egui::ProgressBar::new(f32::from(status.progress) / 100.0)
                            .show_percentage(),
                    );
                }
                if status.phase == Phase::Ready {
                    ui.label(
                        egui::RichText::new(tr("Restarting ends sharing and watching."))
                            .color(palette().warning),
                    );
                }
                ui.separator();
                let action_width = match status.phase {
                    Phase::Available | Phase::Ready => 180.0,
                    Phase::Current => button_width(ui, tr("Check for updates")),
                    Phase::Error => button_width(ui, tr("Retry update check")),
                    _ => ui.spacing().interact_size.y,
                };
                ui.horizontal(|ui| {
                    let row_width =
                        button_width(ui, tr("Later")) + ui.spacing().item_spacing.x + action_width;
                    ui.add_space(((ui.available_width() - row_width) / 2.0).max(0.0));
                    dismiss |= ui.button(tr("Later")).clicked();
                    match status.phase {
                        Phase::Available => {
                            if ui.add(primary(tr("Download update"), 180.0)).clicked() {
                                self.updates.send(Action::Download);
                            }
                        }
                        Phase::Ready => {
                            if ui.add(primary(tr("Restart to update"), 180.0)).clicked() {
                                self.update_restart_prompt = true;
                            }
                        }
                        Phase::Current | Phase::Error => {
                            if ui
                                .button(tr(if status.phase == Phase::Current {
                                    "Check for updates"
                                } else {
                                    "Retry update check"
                                }))
                                .clicked()
                            {
                                self.updates.send(Action::Check);
                            }
                        }
                        _ => {
                            ui.spinner();
                        }
                    }
                });
            });
        if dismiss || modal.should_close() {
            self.update_open = false;
        }
    }

    fn update_restart_dialog(&mut self, ctx: &egui::Context) {
        if !self.update_restart_prompt {
            return;
        }
        let mut restart = false;
        let mut later = false;
        let modal = egui::Modal::new(egui::Id::unique("restart-update"))
            .backdrop_color(egui::Color32::from_black_alpha(175))
            .frame(
                egui::Frame::new()
                    .fill(palette().panel)
                    .stroke(egui::Stroke::new(1.0, palette().border))
                    .corner_radius(16)
                    .inner_margin(24)
                    .shadow(skin::menu_frame().shadow),
            )
            .show(ctx, |ui| {
                ui.set_max_width(380.0);
                ui.spacing_mut().item_spacing.y = 10.0;
                ui.label(skin::strong(tr("Restart to update"), 20.0).color(palette().text));
                ui.label(tr("Restarting ends sharing and watching."));
                ui.label(
                    egui::RichText::new(tr(
                        "After restart, open a fresh invitation. Your guest nickname is remembered.",
                    ))
                    .color(palette().muted),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let row_width = button_width(ui, tr("Later"))
                        + ui.spacing().item_spacing.x
                        + button_width(ui, tr("Restart to update"));
                    ui.add_space(((ui.available_width() - row_width) / 2.0).max(0.0));
                    later = ui.button(tr("Later")).clicked();
                    restart = ui
                        .add(primary(
                            tr("Restart to update"),
                            button_width(ui, tr("Restart to update")).max(150.0),
                        ))
                        .clicked();
                });
            });
        if later || modal.should_close() {
            self.update_restart_prompt = false;
            self.update_open = false;
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

    fn render(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = root.ctx();
        let effective = self.appearance.theme.resolve(ctx.system_theme());
        let applied = ctx.data(|data| data.get_temp::<Theme>(egui::Id::unique("studio-palette")));
        if applied != Some(effective) {
            apply_palette(ctx, effective);
            root.set_style(ctx.style_of(egui::Theme::Dark));
        } else {
            theme::activate(effective);
        }
        self.render_contents(root, frame);
        skin::paint_keyboard_focus(root.ctx());
    }

    fn render_contents(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.account_state = self.account_client.state();
        if let Some(url) = self.account_client.take_browser_url() {
            ctx.open_url(egui::OpenUrl::new_tab(url));
        }
        self.poll_following(&ctx);
        let profile_snapshot = self.controller.snapshot();
        self.poll_profile(&profile_snapshot);
        self.profile_dialog(&ctx, profile_snapshot.connection == "connected");
        ctx.request_repaint_after(Duration::from_secs(1));
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
        if !self.browser_invitation_open
            && !self.update_open
            && !self.licenses_open
            && ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            if self.profile_open {
                self.profile_open = false;
            } else if self.advanced_open {
                self.advanced_open = false;
            } else if let Some(tile) = profile_snapshot.tiles.iter().rev().find(|tile| {
                tile.watching && tile.presentation.diagnostics.open.load(Ordering::Relaxed)
            }) {
                tile.presentation
                    .diagnostics
                    .open
                    .store(false, Ordering::Relaxed);
            } else if self.fullscreen.is_some() {
                if !egui::Popup::is_any_open(&ctx) {
                    self.set_fullscreen(&ctx, None);
                }
            } else if !egui::Popup::is_any_open(&ctx) {
                if self.sharing_setup.take().is_some() {
                    self.clear_picker();
                } else if self.roster_open {
                    self.roster_open = false;
                } else {
                    self.focused = None;
                    self.spotlight = None;
                }
            }
        }
        let mut state = self.controller.snapshot();
        self.apply_profile_names(&mut state);
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
            (state.any_sharing() && id == SELF_PREVIEW) || state.tiles.iter().any(|t| &t.id == id)
        });
        if !state.any_sharing() {
            self.self_preview_revision = None;
            self.self_preview_hidden = false;
        }
        self.reconcile_spotlight(&state);
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
        if !self.browser_invitation_open
            && !self.update_open
            && !self.licenses_open
            && !self.profile_open
            && !self.advanced_open
            && !egui::Popup::is_any_open(&ctx)
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F11))
        {
            if self.fullscreen.is_some() {
                self.set_fullscreen(&ctx, None);
            } else if let Some(tile) = state
                .tiles
                .iter()
                .find(|t| {
                    t.watching && self.focused.as_ref().or(self.spotlight.as_ref()) == Some(&t.id)
                })
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
        if !state
            .tiles
            .iter()
            .any(|t| Some(&t.id) == self.focused.as_ref())
        {
            self.focused = None;
        }
        let room_blocked = self.browser_invitation_open
            || self.profile_open
            || self.advanced_open
            || self.licenses_open
            || self.update_open;
        #[cfg(windows)]
        if room_blocked {
            ctx.accesskit_node_builder(root.unique_id(), |node| node.set_hidden());
        }
        if room_blocked {
            root.disable();
        }
        let connected = matches!(state.connection, "connected" | "reconnecting");
        egui::Panel::top("room-header")
            .exact_size(58.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(palette().canvas)
                    .inner_margin(egui::Margin::symmetric(16, 8)),
            )
            .show(root, |ui| self.room_header(ui, &state));
        if !state.error.is_empty()
            || !self.settings_error.is_empty()
            || !state.policy_notice.is_empty()
            || state.busy
            || state.connection == "reconnecting"
        {
            egui::Panel::top("action-feedback")
                .frame(
                    egui::Frame::new()
                        .fill(palette().card)
                        .stroke(egui::Stroke::new(1.0, palette().divider))
                        .inner_margin(egui::Margin::symmetric(16, 8)),
                )
                .show(root, |ui| {
                    if !state.error.is_empty() {
                        ui.colored_label(palette().error, tr(&state.error));
                    }
                    if !self.settings_error.is_empty() {
                        ui.colored_label(palette().error, tr(&self.settings_error));
                    }
                    if !state.policy_notice.is_empty() {
                        ui.colored_label(palette().muted, tr(state.policy_notice));
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
        if connected && self.roster_open {
            self.roster_drawer(root, &state);
        }
        let dock = connected.then(|| self.studio_dock(root, &state));
        let stage_bounds = root.available_rect_before_wrap();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(palette().canvas)
                    .inner_margin(egui::Margin::symmetric(18, 14)),
            )
            .show(root, |ui| {
                if !connected {
                    self.clear_picker();
                    self.sharing_setup = None;
                    self.roster_open = false;
                    self.join_landing(ui, &state);
                    return;
                }
                if state.tiles.is_empty() && state.any_sharing() {
                    self.only_stream_stage(ui, &state);
                    return;
                }
                if state.tiles.is_empty() {
                    self.empty_room_stage(ui, &state);
                    return;
                }
                let show_preview_restore = state.any_sharing()
                    && self.self_preview_hidden
                    && state.tiles.iter().any(|tile| tile.watching);
                let show_view_controls = state.tiles.len() > 1 || self.focused.is_some();
                if show_preview_restore || show_view_controls {
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if show_preview_restore {
                                let show = if ui.available_width() >= 475.0 {
                                    skin::outline_button(ui, tr("Show my preview"), Icon::Monitor)
                                } else {
                                    skin::icon_button(ui, Icon::Monitor, tr("Show my preview"))
                                };
                                if show.clicked() {
                                    self.self_preview_hidden = false;
                                }
                            }
                            if show_view_controls {
                                let compact = ui.available_width() < 340.0;
                                self.stage_view_controls(ui, &state, compact);
                            }
                        });
                    });
                    ui.add_space(10.0);
                }
                self.room_stage(ui, &state);
            });
        if let (Some(dock), Some(setup)) = (dock, self.sharing_setup)
            && !self.advanced_open
            && !self.profile_open
            && !self.browser_invitation_open
            && !self.update_open
            && !self.licenses_open
        {
            self.sharing_setup_popover(&ctx, &state, stage_bounds, dock, setup);
        } else {
            self.clear_picker();
        }
        self.advanced_modal(&ctx, &state);
        self.licenses_dialog(&ctx);
        if self.browser_invitation_open {
            defocus_invitation_background(root);
            self.browser_invitation_modal(&ctx, &state);
        }
        self.update_dialog(&ctx);
    }

    fn roster_drawer(&mut self, root: &mut egui::Ui, state: &Snapshot) {
        egui::Panel::right("room-roster")
            .default_size(264.0)
            .min_size(240.0)
            .max_size(320.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(palette().panel)
                    .stroke(egui::Stroke::new(1.0, palette().divider))
                    .inner_margin(egui::Margin::symmetric(16, 12)),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        skin::strong(
                            format!("{} · {}", tr("In room"), state.participants.len() + 1),
                            13.0,
                        )
                        .color(palette().text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if skin::icon_button(ui, Icon::Close, tr("Close")).clicked() {
                            self.roster_open = false;
                        }
                    });
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .id_salt("roster-scroll")
                    .max_height((ui.available_height() - 62.0).max(80.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        let own_id = self
                            .profile_client
                            .as_ref()
                            .map(|c| c.participant_id.clone())
                            .unwrap_or_default();
                        let texture = self.profile_texture(ui.ctx(), &own_id);
                        let discord = self
                            .profile_state
                            .profiles
                            .iter()
                            .any(|p| p.participant_id == own_id && p.discord);
                        if participant(ui, &self.name, true, state.any_sharing(), texture, discord)
                            .clicked()
                        {
                            self.open_profile();
                        }
                        for (index, name) in state.participants.iter().enumerate() {
                            let id = state
                                .participant_ids
                                .get(index)
                                .map(|(id, _)| id.as_str())
                                .unwrap_or("");
                            let texture = self.profile_texture(ui.ctx(), id);
                            participant(
                                ui,
                                name,
                                false,
                                state.tiles.iter().any(|tile| tile.owner() == id),
                                texture,
                                self.profile_state
                                    .profiles
                                    .iter()
                                    .any(|p| p.participant_id == id && p.discord),
                            );
                        }
                    });
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 44.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            let own_id = self
                                .profile_client
                                .as_ref()
                                .map(|c| c.participant_id.clone())
                                .unwrap_or_default();
                            let texture = self.profile_texture(ui.ctx(), &own_id);
                            profile::avatar(ui, &self.name, 30.0, texture);
                            let info_width = (ui.available_width() - 40.0).max(20.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(info_width, 38.0),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui.set_width(info_width);
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    if ui
                                        .add(
                                            egui::Label::new(
                                                skin::strong(&self.name, 13.5)
                                                    .color(palette().text),
                                            )
                                            .truncate()
                                            .sense(egui::Sense::click()),
                                        )
                                        .clicked()
                                    {
                                        self.open_profile();
                                    }
                                    ui.label(
                                        egui::RichText::new(tr(
                                            if state.connection == "connected" {
                                                "Connected to room"
                                            } else {
                                                "Reconnecting"
                                            },
                                        ))
                                        .size(11.5)
                                        .color(
                                            if state.connection == "connected" {
                                                palette().success
                                            } else {
                                                palette().warning
                                            },
                                        ),
                                    );
                                },
                            );
                            if skin::icon_button(ui, Icon::Settings, tr("User settings")).clicked()
                            {
                                self.open_profile();
                            }
                        },
                    );
                    ui.separator();
                });
            });
    }

    fn sharing_panel_title(state: &Snapshot) -> &'static str {
        tr(if state.sharing || state.capture_pending {
            "Edit stream settings"
        } else {
            "Share your screen"
        })
    }

    fn camera_selected(&self, state: &Snapshot) -> bool {
        state
            .cameras
            .iter()
            .any(|device| device.id == self.camera_settings.device_id)
    }

    fn can_start_camera(&self, state: &Snapshot) -> bool {
        cfg!(windows)
            && state.connection == "connected"
            && !state.busy
            && !state.refreshing_cameras
            && !state.encoders.is_empty()
            && !self.picker.camera_capturing()
            && self.camera_selected(state)
    }

    fn start_camera(&mut self) {
        self.clear_picker();
        // Cancellation prevents pending previews from acquiring the device.
        // A capture already in progress must release it before publishing.
        if self.picker.camera_capturing() {
            return;
        }
        self.controller.send(Command::StartCamera {
            device_id: self.camera_settings.device_id.clone(),
            quality: self.camera_settings.quality(),
            preferred_codec: None,
        });
        self.sharing_setup = None;
    }

    fn studio_dock(&mut self, root: &mut egui::Ui, state: &Snapshot) -> SharingDock {
        let camera_live = state.camera_sharing || state.camera_pending;
        if self.picker.camera_capturing() {
            root.ctx().request_repaint_after(Duration::from_millis(50));
        }
        let screen_live = state.sharing || state.capture_pending;
        let can_start = state.connection == "connected" && !state.busy;
        let dirty = state.sharing
            && state
                .active_selection
                .as_ref()
                .is_some_and(|active| active != &self.selection());
        egui::Panel::bottom("studio-dock")
            .exact_size(88.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(palette().canvas))
            .show(root, |ui| {
                // Paint through the reserved panel edge so the dock shadow stays visible.
                let controls = egui::Area::new(egui::Id::unique("studio-controls"))
                    .accessibility_parent(ui.unique_id())
                    .enabled(ui.is_enabled())
                    .pivot(egui::Align2::CENTER_BOTTOM)
                    .fixed_pos(ui.max_rect().center_bottom() - egui::vec2(0.0, 20.0))
                    .fade_in(false)
                    .order(egui::Order::Middle)
                    .show(ui.ctx(), |ui| {
                        skin::dock_frame()
                            .show(ui, |ui| {
                                ui.spacing_mut().interact_size.y = SHARING_BUTTON_HEIGHT;
                                ui.horizontal(|ui| {
                                    let p = palette();
                                    let screen_action = if screen_live {
                                        sharing_action(tr("Stop screen"), Icon::Stop, p.error)
                                            .fill(p.danger.gamma_multiply(0.14))
                                            .stroke((1.0, p.danger.gamma_multiply(0.45)))
                                    } else {
                                        sharing_action(
                                            tr("Share screen"),
                                            Icon::Monitor,
                                            p.on_accent,
                                        )
                                        .fill(p.accent)
                                    };
                                    let can_share = screen_live || (can_start && self.valid_selection(state));
                                    let (screen_action, screen_settings) = sharing_split_button(
                                        ui,
                                        screen_action,
                                        can_share,
                                        tr("Screen settings"),
                                        self.sharing_setup == Some(SharingSetup::Screen),
                                        dirty,
                                    );
                                    if screen_action.clicked() {
                                        if screen_live {
                                            self.controller.send(Command::Stop);
                                        } else {
                                            self.send_settings(false);
                                            self.sharing_setup = None;
                                        }
                                    }

                                    let can_share_camera = self.can_start_camera(state);
                                    let camera_action = if camera_live {
                                        sharing_action(tr("Stop camera"), Icon::Stop, p.error)
                                            .fill(p.danger.gamma_multiply(0.14))
                                            .stroke((1.0, p.danger.gamma_multiply(0.45)))
                                    } else {
                                        sharing_action(tr("Share camera"), Icon::Camera, if can_share_camera { p.text } else { p.muted })
                                            .fill(p.card)
                                            .stroke((1.0, p.border))
                                    };
                                    let (camera_action, camera_settings) = sharing_split_button(
                                        ui,
                                        camera_action,
                                        camera_live || can_share_camera,
                                        tr("Camera settings"),
                                        matches!(self.sharing_setup, Some(SharingSetup::Camera | SharingSetup::CameraAdvanced)),
                                        false,
                                    );
                                    let screen = screen_action.rect.union(screen_settings.rect);
                                    let camera = camera_action.rect.union(camera_settings.rect);
                                    if camera_action.clicked() {
                                        if camera_live {
                                            self.controller.send(Command::StopCamera);
                                        } else {
                                            self.start_camera();
                                        }
                                    }
                                    camera_action.on_disabled_hover_text(tr(if !cfg!(windows) {
                                        "Camera sharing is available on Windows."
                                    } else if self.picker.camera_capturing() {
                                        "Preparing camera preview…"
                                    } else if state.encoders.is_empty() {
                                        "No supported hardware encoder was found. You can still watch compatible streams."
                                    } else {
                                        "Choose a device in Camera settings."
                                    }));

                                    let requested = if screen_settings.clicked() {
                                        Some(SharingSetup::Screen)
                                    } else if camera_settings.clicked() {
                                        Some(SharingSetup::Camera)
                                    } else {
                                        None
                                    };
                                    if let Some(setup) = requested {
                                        let was_open = self.sharing_setup == Some(setup)
                                            || (setup == SharingSetup::Camera && self.sharing_setup == Some(SharingSetup::CameraAdvanced));
                                        self.clear_picker();
                                        self.sharing_setup = (!was_open).then_some(setup);
                                        if self.sharing_setup == Some(SharingSetup::Camera) && cfg!(windows) && !camera_live {
                                            self.controller.send(Command::RefreshCameras);
                                        }
                                    }
                                    (screen, camera)
                                })
                                .inner
                            })
                            .inner
                    });
                SharingDock {
                    rect: controls.response.rect,
                    screen: controls.inner.0,
                    camera: controls.inner.1,
                }
            })
            .inner
    }

    fn sharing_setup_popover(
        &mut self,
        ctx: &egui::Context,
        state: &Snapshot,
        stage: egui::Rect,
        dock: SharingDock,
        setup: SharingSetup,
    ) {
        let screen = setup == SharingSetup::Screen;
        let camera_advanced = setup == SharingSetup::CameraAdvanced;
        let pending = screen
            && state
                .active_selection
                .as_ref()
                .is_some_and(|s| s != &self.selection());
        let reserved_height = if screen {
            112.0 + if pending { 80.0 } else { 50.0 }
        } else {
            74.0
        };
        let max_height = (stage.height() - reserved_height).clamp(48.0, 480.0);
        let trigger = if screen { dock.screen } else { dock.camera };
        let position = egui::pos2(trigger.left(), dock.rect.top() - 10.0);
        let panel = egui::Area::new(egui::Id::unique(if screen {
            "screen-setup"
        } else {
            "camera-setup"
        }))
        .constrain_to(egui::Rect::from_min_max(
            stage.min + egui::vec2(6.0, 6.0),
            egui::pos2(stage.right() - 6.0, position.y),
        ))
        .pivot(egui::Align2::LEFT_BOTTOM)
        .fixed_pos(position)
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            ui.set_clip_rect(egui::Rect::from_min_max(
                stage.min,
                egui::pos2(stage.right(), dock.rect.top()),
            ));
            let mut frame = skin::menu_frame()
                .fill(palette().panel)
                .corner_radius(14)
                .inner_margin(egui::Margin::same(16));
            frame.shadow.offset = [0, 4];
            frame.shadow.blur = 16;
            frame.show(ui, |ui| {
                ui.set_width(332.0);
                ui.horizontal(|ui| {
                    let title = if screen {
                        Self::sharing_panel_title(state)
                    } else if camera_advanced {
                        tr("Advanced camera settings")
                    } else {
                        tr("Camera settings")
                    };
                    ui.label(skin::strong(title, 15.0).color(palette().text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if skin::icon_button(ui, Icon::Close, tr("Close")).clicked() {
                            self.sharing_setup = None;
                            self.clear_picker();
                        }
                        if (screen || cfg!(windows))
                            && ui
                                .add(
                                    egui::Button::new(skin::strong(
                                        tr(if camera_advanced { "Back" } else { "Advanced" }),
                                        12.5,
                                    ))
                                    .frame(false),
                                )
                                .clicked()
                        {
                            self.clear_picker();
                            if screen {
                                self.sharing_setup = None;
                                self.advanced_open = true;
                            } else {
                                self.sharing_setup = Some(if camera_advanced {
                                    SharingSetup::Camera
                                } else {
                                    SharingSetup::CameraAdvanced
                                });
                            }
                        }
                    });
                });
                if self.sharing_setup != Some(setup) {
                    return;
                }
                ui.add_space(6.0);
                let body = ui.available_rect_before_wrap();
                ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("broadcast-setup-scroll")
                        .max_height(max_height)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            if screen {
                                self.sharing_menu(ui, state);
                            } else if camera_advanced {
                                ui.add_enabled_ui(
                                    !state.camera_sharing && !state.camera_pending,
                                    |ui| self.camera_bitrate_controls(ui),
                                );
                            } else {
                                self.camera_setup_fields(ui, state);
                            }
                        });
                });
                if screen {
                    self.sharing_actions(ui, state);
                }
            });
        });
        let preview = if self.sharing_setup == Some(SharingSetup::Camera)
            && setup == SharingSetup::Camera
            && cfg!(windows)
            && !state.camera_sharing
            && !state.camera_pending
            && !state.refreshing_cameras
            && self.camera_selected(state)
        {
            self.camera_preview_panel(ctx, stage.shrink(6.0), panel.response.rect)
        } else {
            None
        };
        if setup == SharingSetup::Camera && preview.is_none() {
            self.clear_picker();
        }
        if !egui::Popup::is_any_open(ctx)
            && ctx.input(|input| {
                input.pointer.any_pressed()
                    && input.pointer.interact_pos().is_some_and(|pos| {
                        !panel.response.rect.contains(pos)
                            && !dock.rect.contains(pos)
                            && preview.is_none_or(|rect| !rect.contains(pos))
                    })
            })
        {
            self.sharing_setup = None;
            self.clear_picker();
        }
    }

    fn camera_setup_fields(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        if !cfg!(windows) {
            ui.label(tr("Camera sharing is available on Windows."));
            return;
        }
        let camera_live = state.camera_sharing || state.camera_pending;
        if camera_live {
            ui.label(tr("Stop camera to change its settings."));
        } else if state.refreshing_cameras {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(tr("Refreshing cameras…"));
            });
        }
        if state.cameras_ready && state.cameras.is_empty() && !state.refreshing_cameras {
            ui.label(tr(
                "No camera found. Check Windows camera permissions and refresh.",
            ));
        }
        ui.add_enabled_ui(!camera_live && !state.refreshing_cameras, |ui| {
            self.camera_controls(ui, state)
        });
    }

    fn camera_preview_panel(
        &mut self,
        ctx: &egui::Context,
        bounds: egui::Rect,
        anchor: egui::Rect,
    ) -> Option<egui::Rect> {
        if ctx.input(|input| {
            input.viewport().visible() == Some(false) || input.viewport().minimized == Some(true)
        }) {
            return None;
        }
        let (position, width) = camera_preview_placement(bounds, anchor)?;
        let settings = &self.camera_settings;
        let key = format!(
            "camera:{}:{}:{}",
            settings.device_id,
            settings.resolution.label(),
            settings.fps
        );
        self.picker
            .request_camera(key.clone(), settings.device_id.clone(), settings.quality());
        self.update_picker_frame(ctx, &key);
        let preview = egui::Area::new(egui::Id::unique("camera-preview-panel"))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .constrain_to(bounds)
            .show(ctx, |ui| {
                skin::menu_frame().inner_margin(10).show(ui, |ui| {
                    ui.set_width(width);
                    ui.label(skin::strong(tr("Camera preview"), 13.5).color(palette().text));
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(width, width * 9.0 / 16.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(rect, 8.0, palette().card);
                    if let Some((_, texture)) = &self.picker_texture {
                        egui::Image::new((texture.id(), texture.size_vec2()))
                            .corner_radius(8)
                            .paint_at(ui, rect);
                    } else {
                        let message = if self.picker_failed.as_deref() == Some(key.as_str()) {
                            tr("Camera preview unavailable. Check the device or choose another mode.")
                        } else {
                            ctx.request_repaint_after(Duration::from_millis(50));
                            tr("Preparing camera preview…")
                        };
                        let text = ui.painter().layout(
                            message.to_owned(),
                            egui::FontId::proportional(12.0),
                            palette().muted,
                            width - 20.0,
                        );
                        ui.painter().galley(
                            rect.center() - text.size() / 2.0,
                            text,
                            palette().muted,
                        );
                    }
                });
            });
        Some(preview.response.rect)
    }

    fn camera_controls(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let settings = &mut self.camera_settings;
        let device = ui
            .horizontal(|ui| {
                let label = ui.label(tr("Camera device"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if skin::icon_button(ui, Icon::Refresh, tr("Refresh cameras")).clicked() {
                        self.controller.send(Command::RefreshCameras);
                    }
                });
                label
            })
            .inner;
        let selected = state
            .cameras
            .iter()
            .find(|device| device.id == settings.device_id);
        let selected_text = selected.map_or_else(
            || {
                tr(if settings.device_id.is_empty() {
                    "Choose a camera"
                } else {
                    "Selected camera is unavailable"
                })
            },
            |device| device.name.as_str(),
        );
        egui::ComboBox::from_id_salt("camera-device")
            .width(ui.available_width())
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                for (index, device) in state.cameras.iter().enumerate() {
                    let name = if state
                        .cameras
                        .iter()
                        .any(|other| other.id != device.id && other.name == device.name)
                    {
                        format!("{} ({})", device.name, index + 1)
                    } else {
                        device.name.clone()
                    };
                    if ui
                        .selectable_label(settings.device_id == device.id, name)
                        .clicked()
                    {
                        settings.device_id.clone_from(&device.id);
                        if crate::settings::save_camera_device(&device.id).is_err() {
                            self.settings_error = "Could not save the selected camera.".into();
                        }
                    }
                }
            })
            .response
            .labelled_by(device.id);
        ui.add_space(6.0);
        ui.columns(2, |columns| {
            let ui = &mut columns[0];
            let resolution = ui.label(tr("Resolution"));
            egui::ComboBox::from_id_salt("camera-resolution")
                .width(ui.available_width())
                .selected_text(tr(settings.resolution.label()))
                .show_ui(ui, |ui| {
                    for resolution in camera::RESOLUTIONS {
                        ui.selectable_value(
                            &mut settings.resolution,
                            resolution,
                            tr(resolution.label()),
                        );
                    }
                })
                .response
                .labelled_by(resolution.id);

            let ui = &mut columns[1];
            let fps = ui.label(tr("Frame rate"));
            egui::ComboBox::from_id_salt("camera-fps")
                .width(ui.available_width())
                .selected_text(format!("{} FPS", settings.fps))
                .show_ui(ui, |ui| {
                    for fps in camera::FRAME_RATES {
                        ui.selectable_value(&mut settings.fps, fps, format!("{fps} FPS"));
                    }
                })
                .response
                .labelled_by(fps.id);
        });
    }

    fn camera_bitrate_controls(&mut self, ui: &mut egui::Ui) {
        let settings = &mut self.camera_settings;
        let mut manual = settings.bitrate_mbps.is_some();
        ui.label(tr("Bitrate"));
        ui.horizontal(|ui| {
            ui.selectable_value(&mut manual, false, tr("Automatic"));
            ui.selectable_value(&mut manual, true, tr("Manual"));
        });
        if !manual {
            settings.bitrate_mbps = None;
            return;
        }
        let bitrate_mbps = settings.bitrate_mbps.get_or_insert(2.0);
        let bitrate = ui
            .horizontal(|ui| {
                let label = ui.label(tr("Target bitrate"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(skin::strong(format!("{bitrate_mbps:.1} Mbps"), 12.0));
                });
                label
            })
            .inner;
        ui.spacing_mut().slider_width = ui.available_width();
        ui.add(
            egui::Slider::new(
                bitrate_mbps,
                camera::MIN_BITRATE as f32 / 1_000_000.0..=camera::MAX_BITRATE as f32 / 1_000_000.0,
            )
            .step_by(0.5)
            .show_value(false),
        )
        .labelled_by(bitrate.id);
    }
}

fn camera_preview_placement(bounds: egui::Rect, anchor: egui::Rect) -> Option<(egui::Pos2, f32)> {
    const GAP: f32 = 10.0;
    const FRAME: f32 = 22.0;
    const PREFERRED_WIDTH: f32 = 288.0;
    let left = anchor.left() - bounds.left() - GAP - FRAME;
    let right = bounds.right() - anchor.right() - GAP - FRAME;
    let on_left = left >= PREFERRED_WIDTH || left >= right;
    let width = (if on_left { left } else { right })
        .min(PREFERRED_WIDTH)
        .min((bounds.height() - 48.0) * 16.0 / 9.0);
    if width < 128.0 {
        let above = anchor.top() - bounds.top() - GAP - 48.0;
        let width = PREFERRED_WIDTH
            .min(bounds.width() - FRAME)
            .min(above * 16.0 / 9.0);
        return (width >= 128.0).then(|| {
            let x = anchor
                .left()
                .min(bounds.right() - FRAME - width)
                .max(bounds.left());
            let y = anchor.top() - GAP - 48.0 - width * 9.0 / 16.0;
            (egui::pos2(x, y), width)
        });
    }
    let x = if on_left {
        anchor.left() - GAP - FRAME - width
    } else {
        anchor.right() + GAP
    };
    let y = anchor
        .top()
        .min(bounds.bottom() - width * 9.0 / 16.0 - 48.0)
        .max(bounds.top());
    Some((egui::pos2(x, y), width))
}

fn stream_name(tile: &Tile) -> std::borrow::Cow<'_, str> {
    if tile.kind == crate::preview::Kind::Camera {
        format!("{} · {}", tile.name, tr("Camera")).into()
    } else {
        tile.name.as_str().into()
    }
}

fn sharing_action(label: &str, icon: Icon, ink: egui::Color32) -> egui::Button<'_> {
    egui::Button::new((
        egui::Atom::paint(egui::vec2(16.0, 16.0), move |ui, args| {
            skin::icon(ui.painter(), args.rect, icon, ink);
        }),
        skin::strong(label, 13.5).color(ink),
    ))
    .gap(6.0)
    .min_size(egui::vec2(0.0, SHARING_BUTTON_HEIGHT))
}

fn sharing_split_button(
    ui: &mut egui::Ui,
    action: egui::Button<'_>,
    enabled: bool,
    settings_label: &str,
    open: bool,
    pending: bool,
) -> (egui::Response, egui::Response) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let action = ui.add_enabled(
            enabled,
            action.corner_radius(egui::CornerRadius {
                nw: 9,
                sw: 9,
                ne: 0,
                se: 0,
            }),
        );
        let p = palette();
        let border = if pending {
            p.warning
        } else if open {
            p.hover_stroke
        } else {
            p.border
        };
        let ink = if pending {
            p.warning
        } else if open {
            p.accent
        } else {
            p.text
        };
        let settings = ui.add(
            egui::Button::new(egui::Atom::paint(
                egui::vec2(14.0, 14.0),
                move |ui, args| {
                    skin::icon(ui.painter(), args.rect, Icon::ChevronUp, ink);
                },
            ))
            .min_size(egui::vec2(38.0, SHARING_BUTTON_HEIGHT))
            .corner_radius(egui::CornerRadius {
                nw: 0,
                sw: 0,
                ne: 9,
                se: 9,
            })
            .fill(if open { p.control_hover } else { p.card })
            .stroke((1.0, border)),
        );
        settings.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), open, settings_label)
        });
        (action, settings.on_hover_text(settings_label))
    })
    .inner
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

fn button_width(ui: &egui::Ui, label: &str) -> f32 {
    let text = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::TextStyle::Button.resolve(ui.style()),
        palette().text,
    );
    (text.size().x + ui.spacing().button_padding.x * 2.0).max(ui.spacing().interact_size.x)
}

fn primary(label: &str, width: f32) -> egui::Button<'_> {
    let text = skin::strong(label, 14.0).color(palette().on_accent);
    let atoms = if label == tr("Watch") || label == tr("Start sharing") {
        egui::Atoms::new((
            egui::Atom::grow(),
            egui::Atom::paint(egui::vec2(16.0, 16.0), |ui, args| {
                skin::icon(ui.painter(), args.rect, Icon::Play, palette().on_accent)
            }),
            text,
            egui::Atom::grow(),
        ))
    } else {
        egui::Atoms::new((egui::Atom::grow(), text, egui::Atom::grow()))
    };
    egui::Button::new(atoms)
        .fill(palette().accent)
        .corner_radius(8)
        .min_size(egui::vec2(width, 38.0))
}

fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.13))
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(skin::strong(text, 12.0).color(color));
        });
}

fn bundled_notices() -> Option<std::path::PathBuf> {
    let executable = std::env::current_exe().ok()?;
    #[cfg(windows)]
    let notices = executable.parent()?.parent()?.join("notices");
    #[cfg(target_os = "macos")]
    let notices = executable
        .ancestors()
        .filter(|dir| dir.file_name().is_some_and(|name| name == "Contents"))
        .map(|dir| dir.join("Resources/notices"))
        .find(|dir| dir.is_dir())?;
    #[cfg(not(any(windows, target_os = "macos")))]
    let notices = executable.parent()?.parent()?.join("notices");
    notices.is_dir().then_some(notices)
}

fn open_notices_folder(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let opener = std::path::PathBuf::from(
        std::env::var_os("WINDIR")
            .ok_or_else(|| std::io::Error::other("Windows directory unavailable"))?,
    )
    .join("explorer.exe");
    #[cfg(target_os = "macos")]
    let opener = "/usr/bin/open";
    #[cfg(not(any(windows, target_os = "macos")))]
    let opener = "xdg-open";
    std::process::Command::new(opener).arg(path).spawn()?;
    Ok(())
}

fn quality_controls(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    source: Option<&crate::sources::SourceInfo>,
    encoder: &crate::encoding::Encoder,
    policy: &quality::StreamPolicy,
) {
    let resolution = |ui: &mut egui::Ui, settings: &mut Settings| {
        let label = ui.label(
            egui::RichText::new(tr("Resolution"))
                .size(12.0)
                .color(palette().muted),
        );
        egui::ComboBox::from_id_salt("resolution")
            .width(ui.available_width())
            .selected_text(tr(settings.resolution.label()))
            .show_ui(ui, |ui| {
                for resolution in quality::Resolution::ALL {
                    let candidate = Settings {
                        resolution,
                        ..*settings
                    };
                    let mode =
                        source.and_then(|s| policy.resolve(candidate, s, encoder.codec).ok());
                    let label = tr(resolution.label());
                    let text = match (resolution, mode) {
                        (quality::Resolution::Source, _) | (_, None) => label.to_owned(),
                        (_, Some(mode)) => format!("{label} · {}×{}", mode.width, mode.height),
                    };
                    ui.add_enabled_ui(mode.is_some_and(|m| encoder.supports(m)), |ui| {
                        ui.selectable_value(&mut settings.resolution, resolution, text);
                    });
                }
            })
            .response
            .labelled_by(label.id);
    };
    let preset = |ui: &mut egui::Ui, settings: &mut Settings| {
        let label = ui.label(
            egui::RichText::new(tr("Quality preset"))
                .size(12.0)
                .color(palette().muted),
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
    };
    let compact = ui.ctx().content_rect().height() <= 600.0;
    if compact {
        ui.columns(2, |columns| {
            resolution(&mut columns[0], settings);
            preset(&mut columns[1], settings);
        });
    } else {
        resolution(ui, settings);
    }
    let label = ui
        .horizontal(|ui| {
            let label = ui.label(egui::RichText::new("FPS").size(12.0).color(palette().muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut value = format!("{} FPS", settings.fps);
                if compact
                    && let Some(mode) =
                        source.and_then(|s| policy.resolve(*settings, s, encoder.codec).ok())
                {
                    value = format!("{}×{} · {} FPS", mode.width, mode.height, mode.fps);
                }
                ui.label(skin::strong(value, 12.0));
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
    if !compact {
        preset(ui, settings);
    }
    if let Some(source) = source {
        match policy.resolve(*settings, source, encoder.codec) {
            Ok(mode) if encoder.supports(mode) => {
                if !compact {
                    ui.small(format!("{}×{} · {} FPS", mode.width, mode.height, mode.fps));
                }
            }
            _ => {
                ui.colored_label(
                    palette().warning,
                    tr("This source / encoder cannot use that resolution and frame rate."),
                );
            }
        }
    }
}

fn bitrate_controls(ui: &mut egui::Ui, settings: &mut Settings, custom: &mut bool) {
    // Keep Custom selected while editing, even if its value matches a preset.
    match settings.bitrate {
        Bitrate::Auto => *custom = false,
        Bitrate::Manual(bps)
            if !quality::BITRATE_PRESETS
                .iter()
                .any(|mbps| bps == mbps * 1_000_000) =>
        {
            *custom = true
        }
        _ => {}
    }
    let label = skin::help_label(
        ui,
        tr("Bitrate"),
        tr(
            "Automatic follows resolution, FPS and content, up to 35 Mbps. Manual: 1–100 Mbps. Connection limits still apply.",
        ),
    );
    egui::ComboBox::from_id_salt("video-bitrate")
        .width(ui.available_width())
        .selected_text(if *custom {
            tr("Custom").into()
        } else {
            match settings.bitrate {
                Bitrate::Auto => tr("Automatic").into(),
                Bitrate::Manual(bps) => format!("{} Mbps", bps / 1_000_000),
            }
        })
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(settings.bitrate == Bitrate::Auto, tr("Automatic"))
                .clicked()
            {
                *custom = false;
                settings.bitrate = Bitrate::Auto;
            }
            for mbps in quality::BITRATE_PRESETS {
                let bitrate = Bitrate::Manual(mbps * 1_000_000);
                if ui
                    .selectable_label(
                        !*custom && settings.bitrate == bitrate,
                        format!("{mbps} Mbps"),
                    )
                    .clicked()
                {
                    *custom = false;
                    settings.bitrate = bitrate;
                }
            }
            if ui.selectable_label(*custom, tr("Custom")).clicked() {
                *custom = true;
                if settings.bitrate == Bitrate::Auto {
                    settings.bitrate = Bitrate::Manual(quality::BITRATE_PRESETS[0] * 1_000_000);
                }
            }
        })
        .response
        .labelled_by(label.id);
    if *custom && let Bitrate::Manual(bps) = settings.bitrate {
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
    let (rect, response) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
    if !palette().dark {
        ui.painter()
            .rect_filled(rect, 6.0, egui::Color32::from_rgb(17, 23, 34));
    }
    egui::Image::new((texture.id(), rect.size())).paint_at(ui, rect);
    response.on_hover_text("PodeTelar");
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

fn paint_avatar_circle(painter: &egui::Painter, rect: egui::Rect, name: &str) {
    let size = rect.width().min(rect.height());
    let palette = [
        egui::Color32::from_rgb(76, 61, 48),
        egui::Color32::from_rgb(56, 69, 78),
        egui::Color32::from_rgb(75, 58, 80),
        egui::Color32::from_rgb(57, 77, 65),
        egui::Color32::from_rgb(86, 66, 52),
    ];
    let color = palette[name.bytes().fold(0usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    }) % palette.len()];
    painter.circle_filled(rect.center(), size / 2.0, color);
    painter.circle_stroke(
        rect.center(),
        size / 2.0,
        egui::Stroke::new(1.0, egui::Color32::from_white_alpha(38)),
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        name.chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string(),
        skin::semibold(size * 0.43),
        egui::Color32::WHITE,
    );
}

fn avatar(ui: &mut egui::Ui, name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    paint_avatar_circle(ui.painter(), rect, name);
}

fn participant(
    ui: &mut egui::Ui,
    name: &str,
    own: bool,
    sharing: bool,
    texture: Option<egui::TextureId>,
    discord: bool,
) -> egui::Response {
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().interact_size.y = 26.0;
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.horizontal(|ui| {
                profile::avatar(ui, name, 26.0, texture);
                let label = if own {
                    format!("{} ({})", name, tr("you"))
                } else {
                    name.to_owned()
                };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if sharing {
                        badge(
                            ui,
                            tr(if own { "Sharing" } else { "Live" }),
                            palette().accent,
                        );
                    }
                    if discord {
                        ui.label(
                            egui::RichText::new("Discord")
                                .size(12.0)
                                .color(palette().muted),
                        );
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(label).size(12.0)).truncate())
                            .on_hover_text(name);
                    });
                });
            });
        })
        .response
        .interact(if own {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        });
    if own {
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), tr("User settings"))
        });
        if response.has_focus() {
            ui.painter().rect_stroke(
                response.rect,
                4.0,
                egui::Stroke::new(1.5, palette().accent),
                egui::StrokeKind::Inside,
            );
        }
    }
    response
}

fn settings_divider(ui: &mut egui::Ui) {
    let padding = if ui.ctx().content_rect().height() <= 600.0 {
        4.0
    } else {
        10.0
    };
    ui.add_space(padding);
    ui.separator();
    ui.add_space(padding);
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
                    app.update_dialog(root.ctx());
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
        draw(&mut app, &ctx, vec![]);
        assert!(app.update_open, "Announce a newly available version");
        assert!(actions.try_recv().is_err(), "Opening never downloads");
        click(&mut app, &ctx, tr("Later"));
        draw(&mut app, &ctx, vec![]);
        assert!(
            !app.update_open,
            "Later must stay dismissed for this version"
        );
        click(&mut app, &ctx, "↻");
        assert!(app.update_open);
        assert!(actions.try_recv().is_err());
        click(&mut app, &ctx, tr("Download update"));
        assert_eq!(actions.try_recv().unwrap(), Action::Download);
        let (client, actions) = crate::updates::Client::fixture(Status {
            phase: Phase::Ready,
            ..Default::default()
        });
        app.updates = client;
        click(&mut app, &ctx, tr("Restart to update"));
        assert!(app.update_restart_prompt);
        assert!(actions.try_recv().is_err());
        draw(&mut app, &ctx, vec![]);
        draw(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(!app.update_open && !app.update_restart_prompt);
        assert!(actions.try_recv().is_err());
        click(&mut app, &ctx, "↻");
        click(&mut app, &ctx, tr("Restart to update"));
        click(&mut app, &ctx, tr("Later"));
        assert!(!app.update_restart_prompt);
        assert!(app.closing_since.is_none());
        assert!(actions.try_recv().is_err());
        click(&mut app, &ctx, "↻");
        click(&mut app, &ctx, tr("Restart to update"));
        click(&mut app, &ctx, tr("Restart to update"));
        assert_eq!(actions.try_recv().unwrap(), Action::Restart);
        assert!(
            app.closing_since.is_none(),
            "Wait for launcher acknowledgement before closing"
        );
        app.update_open = false;
        for phase in [
            Phase::Checking,
            Phase::Current,
            Phase::Downloading,
            Phase::Error,
        ] {
            let (client, actions) = crate::updates::Client::fixture(Status {
                phase,
                progress: 42,
                ..Default::default()
            });
            app.updates = client;
            click(&mut app, &ctx, "↻");
            assert!(
                app.update_open,
                "Status must remain accessible during {phase:?}"
            );
            assert!(actions.try_recv().is_err());
            if matches!(phase, Phase::Current | Phase::Error) {
                click(
                    &mut app,
                    &ctx,
                    tr(if phase == Phase::Current {
                        "Check for updates"
                    } else {
                        "Retry update check"
                    }),
                );
                assert_eq!(actions.try_recv().unwrap(), Action::Check);
            }
            click(&mut app, &ctx, tr("Later"));
            assert!(actions.try_recv().is_err());
        }
    }

    #[test]
    fn update_announcement_waits_for_other_dialogs_and_fullscreen() {
        use crate::update_protocol::{Phase, Status};
        let mut app = app();
        let ctx = egui::Context::default();
        let (client, actions) = crate::updates::Client::fixture(Status {
            phase: Phase::Available,
            version: "0.1.3".into(),
            ..Default::default()
        });
        app.updates = client;
        for blocked in 0..5 {
            app.browser_invitation_open = blocked == 0;
            app.advanced_open = blocked == 1;
            app.profile_open = blocked == 3;
            app.fullscreen = (blocked == 2).then(|| "watched-stream".into());
            let mut output = ctx.run_ui(egui::RawInput::default(), |root| {
                app.update_dialog(root.ctx())
            });
            output.textures_delta.clear();
            assert_eq!(app.update_open, blocked == 4);
            assert_eq!(app.update_announced_version.is_some(), blocked == 4);
        }
        assert!(actions.try_recv().is_err());
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
            "podetelar://join?server=https%3A%2F%2Fother.test#handoff={}",
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
    fn offline_nickname_change_is_used_by_the_next_invitation() {
        let mut app = app();
        app.session_name = Some("Previous name".into());
        app.name = "Previous name".into();
        app.remember_guest_name("Updated offline name".into());
        let (controller, mut command) = Controller::recording_for_test(Snapshot::default());
        app.controller = controller;
        finish_link(&mut app, "https://next.test/synthetic");
        assert!(matches!(command(),Some(Command::Join{name,..})if name=="Updated offline name"));
        assert!(
            command().is_none(),
            "A nickname update must not start capture or subscriptions"
        );
    }

    #[test]
    fn current_profile_updates_preserve_an_active_invitation_name_draft() {
        let mut app = app();
        app.browser_invitation_open = true;
        app.invitation = "https://destination.test/synthetic".into();
        app.name = "Destination nickname draft".into();
        app.session_name = Some("Current nickname".into());
        let (controller, mut command) = Controller::recording_for_test(Snapshot {
            connection: "connected",
            ..Default::default()
        });
        app.controller = controller;
        app.observe_profile_name("Updated current nickname");
        assert_eq!(app.name, "Destination nickname draft");
        assert_eq!(
            app.session_name.as_deref(),
            Some("Updated current nickname")
        );
        app.join();
        assert!(
            matches!(command(),Some(Command::Join{name,..})if name=="Destination nickname draft")
        );
        assert!(command().is_none());
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
                    "podetelar://join?server=https%3A%2F%2F{server}#handoff={}",
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
            following_client: crate::following::Client::disabled(),
            following: Default::default(),
            following_key: None,
            next_follow_attempt: Instant::now(),
            account_client: crate::account::Client::disabled(),
            profile_connection_key: None,
            account_state: Default::default(),
            account_server: crate::account::DEFAULT_SERVER.into(),
            profile_client: None,
            profile_state: Default::default(),
            profile_textures: HashMap::new(),
            profile_started: Instant::now(),
            profile_open: false,
            profile_name: String::new(),
            saved_guest_name: String::new(),
            updates: crate::updates::Client::new(),
            update_open: false,
            update_announced_version: None,
            update_restart_prompt: false,
            activation: crate::activation::Inbox::new(),
            prior_room: None,
            // Paint fixtures exercise UI state without requesting OS capture permission.
            controller: Controller::recording_for_test(Snapshot::default()).0,
            invitation: String::new(),
            name: String::new(),
            session_name: None,
            source_id: String::new(),
            encoder: String::new(),
            quality: Settings::default(),
            camera_settings: CameraSettings::default(),
            custom_bitrate: false,
            download_budget: None,
            textures: HashMap::new(),
            focused: None,
            spotlight: None,
            self_preview_hidden: false,
            fullscreen: None,
            fullscreen_controls_until: Instant::now(),
            settings_error: String::new(),
            auto_join_ready: false,
            copied_until: None,
            appearance: Appearance {
                theme: Theme::Amber,
                ..Appearance::default()
            },
            advanced_open: false,
            licenses_open: false,
            sharing_setup: None,
            roster_open: false,
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
    fn user_settings_shows_profile_and_discord_controls_together() {
        for signed_in in [false, true] {
            let mut app = app();
            app.remember_guest_name("Example guest".into());
            app.account_state.account = signed_in.then(|| crate::account::Account {
                session_id: "fixture-session".into(),
                user_id: "123456789012345678".into(),
                display_name: "Example account".into(),
                expires_at: 0,
            });
            app.open_profile();
            let ctx = egui::Context::default();
            configure_theme(&ctx);
            let draw = || egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960.0, 540.0),
                )),
                ..Default::default()
            };
            ctx.run_ui(draw(), |root| app.profile_dialog(root.ctx(), false))
                .textures_delta
                .clear();
            let mut output = ctx.run_ui(draw(), |root| app.profile_dialog(root.ctx(), false));
            output.textures_delta.clear();
            let mut text = String::new();
            for shape in output.shapes {
                painted_text(&shape.shape, &mut text);
            }
            assert!(text.contains(tr("Guest nickname")));
            assert!(text.contains(tr("Discord account")));
            assert_eq!(text.contains(tr("Sign in with Discord")), !signed_in);
            assert_eq!(text.contains(tr("Disconnect Discord")), signed_in);
            assert_eq!(text.contains(tr("Automatic joining")), signed_in);
        }
    }

    #[test]
    fn one_license_menu_action_opens_the_notice_dialog() {
        let mut app = app();
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let mut frame = eframe::Frame::_new_kittest();
        let mut draw = |app: &mut PublisherUi, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(960.0, 540.0),
                    )),
                    events,
                    ..Default::default()
                },
                |root| app.render(root, &mut frame),
            );
            output.textures_delta.clear();
            output
        };
        let clicks = |pos, pressed| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ]
        };
        draw(&mut app, vec![]);
        for pressed in [true, false] {
            draw(&mut app, clicks(egui::pos2(836.0, 26.0), pressed));
        }
        draw(&mut app, vec![]);
        let menu = draw(&mut app, vec![]);
        let licenses: Vec<_> = menu
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == tr("Licenses") => {
                    Some(text.pos + text.galley.size() / 2.0)
                }
                _ => None,
            })
            .collect();
        assert_eq!(licenses.len(), 1);
        for pressed in [true, false] {
            draw(&mut app, clicks(licenses[0], pressed));
        }
        assert!(app.licenses_open);
        assert!(!egui::Popup::is_any_open(&ctx));
        draw(&mut app, vec![]);
        let output = draw(&mut app, vec![]);
        let mut text = String::new();
        for shape in &output.shapes {
            painted_text(&shape.shape, &mut text);
        }
        assert!(text.contains(tr("Inter font license")));
        assert!(text.contains("SIL OPEN FONT LICENSE"));
        draw(
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(!app.licenses_open);
    }

    #[test]
    fn selected_theme_option_renders_on_accent_text_in_every_palette() {
        let mut app = app();
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        for system in [egui::Theme::Dark, egui::Theme::Light] {
            for preference in Theme::ALL {
                app.appearance.theme = preference;
                let effective = preference.resolve(Some(system));
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        system_theme: Some(system),
                        ..Default::default()
                    },
                    |root| {
                        apply_palette(root.ctx(), effective);
                        root.set_style(root.ctx().style_of(system));
                        app.theme_options(root);
                    },
                );
                output.textures_delta.clear();
                let label = tr(preference.label());
                let selected = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == label => Some(text),
                        _ => None,
                    })
                    .expect("Selected theme label");
                let palette = effective.palette();
                assert!(
                    output.shapes.iter().any(|shape| {
                        matches!(&shape.shape, egui::Shape::Rect(rect)
                        if rect.fill == palette.accent && rect.rect.contains(selected.pos))
                    }),
                    "Missing selected fill for {preference:?} in {effective:?}"
                );
                for section in &selected.galley.job.sections {
                    let color = selected.override_text_color.unwrap_or_else(|| {
                        if section.format.color == egui::Color32::PLACEHOLDER {
                            selected.fallback_color
                        } else {
                            section.format.color
                        }
                    });
                    assert_eq!(color, palette.on_accent, "{preference:?} in {effective:?}");
                }
            }
        }
    }

    #[test]
    fn rendering_updates_system_colors_and_respects_a_fixed_theme() {
        let mut app = app();
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let mut frame = eframe::Frame::_new_kittest();
        for (preference, system, expected) in [
            (Theme::System, egui::Theme::Light, Theme::Light),
            (Theme::System, egui::Theme::Dark, Theme::Black),
            (Theme::Amber, egui::Theme::Light, Theme::Amber),
        ] {
            app.appearance.theme = preference;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    system_theme: Some(system),
                    ..Default::default()
                },
                |root| {
                    app.render(root, &mut frame);
                    assert_eq!(root.visuals().panel_fill, expected.palette().panel);
                    assert_eq!(palette().canvas, expected.palette().canvas);
                },
            );
            output.textures_delta.clear();
        }
    }

    #[test]
    fn spotlight_selection_keeps_watch_as_an_explicit_command() {
        let mut app = app();
        let template = Tile {
            id: "main".into(),
            kind: crate::preview::Kind::Screen,
            name: "Main".into(),
            codec: String::new(),
            watching: true,
            picture: Default::default(),
            presentation: Default::default(),
            frames: Default::default(),
            received_bps: Default::default(),
            volume: Default::default(),
            audio: Default::default(),
            status: String::new(),
        };
        let preview = Tile {
            id: "preview".into(),
            name: "Preview participant".into(),
            watching: false,
            ..template.clone()
        };
        let mut state = Snapshot {
            tiles: vec![template, preview],
            ..Default::default()
        };
        let (controller, mut next_command) = Controller::recording_for_test(Snapshot::default());
        app.controller = controller;
        app.spotlight = Some("main".into());
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let draw = |app: &mut PublisherUi, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 350.0),
                    )),
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| app.room_stage(ui, &state));
                },
            );
            output.textures_delta.clear();
            output.platform_output.events
        };
        let key = |key, pressed| egui::Event::Key {
            key,
            pressed,
            physical_key: None,
            repeat: false,
            modifiers: Default::default(),
        };
        let label = "Preview participant";
        draw(&mut app, vec![]);
        for _ in 0..20 {
            let events = draw(&mut app, vec![key(egui::Key::Tab, true)]);
            draw(&mut app, vec![key(egui::Key::Tab, false)]);
            if events.iter().any(|event| matches!(event,
                egui::output::OutputEvent::FocusGained(info) if info.label.as_deref() == Some(label)
            )) {
                draw(&mut app, vec![key(egui::Key::Enter, true)]);
                draw(&mut app, vec![key(egui::Key::Enter, false)]);
                break;
            }
        }
        assert_eq!(app.spotlight.as_deref(), Some("preview"));
        assert!(next_command().is_none());
        state.tiles.remove(1);
        app.reconcile_spotlight(&state);
        assert!(app.spotlight.is_none());
    }

    #[test]
    fn f11_from_the_second_watched_stream_menu_opens_that_stream() {
        let mut app = app();
        let watched = |id: &str| Tile {
            id: id.into(),
            kind: crate::preview::Kind::Screen,
            name: id.into(),
            codec: String::new(),
            watching: true,
            picture: Default::default(),
            presentation: Default::default(),
            frames: Default::default(),
            received_bps: Default::default(),
            volume: Default::default(),
            audio: Default::default(),
            status: String::new(),
        };
        app.controller = Controller::recording_for_test(Snapshot {
            connection: "connected",
            tiles: vec![watched("Felipe"), watched("Lucas")],
            ..Default::default()
        })
        .0;
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let mut frame = eframe::Frame::_new_kittest();
        let mut draw = |app: &mut PublisherUi, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(960.0, 540.0),
                    )),
                    events,
                    ..Default::default()
                },
                |root| app.render(root, &mut frame),
            );
            output.textures_delta.clear();
            output.platform_output.events
        };
        let key = |key, pressed| egui::Event::Key {
            key,
            pressed,
            physical_key: None,
            repeat: false,
            modifiers: Default::default(),
        };
        draw(&mut app, vec![]);
        let label = format!("{} · Lucas", tr("Stream options"));
        for _ in 0..64 {
            let mut events = draw(&mut app, vec![key(egui::Key::Tab, true)]);
            events.extend(draw(&mut app, vec![key(egui::Key::Tab, false)]));
            if events.iter().any(|event| matches!(event,
                egui::output::OutputEvent::FocusGained(info) if info.label.as_ref() == Some(&label)
            )) {
                draw(&mut app, vec![key(egui::Key::Enter, true)]);
                draw(&mut app, vec![key(egui::Key::Enter, false)]);
                break;
            }
        }
        assert!(
            egui::Popup::is_any_open(&ctx),
            "The second stream's menu must be open"
        );
        draw(&mut app, vec![key(egui::Key::F11, true)]);
        assert_eq!(app.fullscreen.as_deref(), Some("Lucas"));
        draw(&mut app, vec![key(egui::Key::F11, false)]);
        draw(&mut app, vec![key(egui::Key::F11, true)]);
        assert!(app.fullscreen.is_none());
        draw(&mut app, vec![key(egui::Key::F11, false)]);
        draw(&mut app, vec![key(egui::Key::F11, true)]);
        assert_eq!(
            app.fullscreen.as_deref(),
            Some("Felipe"),
            "Grid shortcut still opens the first watched stream"
        );
    }

    #[test]
    fn keyboard_can_watch_and_stop_the_second_stream_without_hover() {
        let mut app = app();
        let (controller, mut next_command) = Controller::recording_for_test(Snapshot::default());
        app.controller = controller;
        let first = Tile {
            id: "first".into(),
            kind: crate::preview::Kind::Screen,
            name: "First".into(),
            codec: String::new(),
            watching: true,
            picture: Default::default(),
            presentation: Default::default(),
            frames: Default::default(),
            received_bps: Default::default(),
            volume: Default::default(),
            audio: Default::default(),
            status: String::new(),
        };
        let mut second = Tile {
            id: "second".into(),
            name: "Second".into(),
            watching: false,
            volume: std::sync::Arc::new(63.into()),
            ..first.clone()
        };
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let mut draw = |second: &Tile, events| {
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
                        app.stream_tile(ui, &first, 320.0);
                        app.stream_tile(ui, second, 320.0);
                    });
                },
            );
            output.textures_delta.clear();
            output.platform_output.events
        };
        draw(&second, vec![]);
        let mut activate = |tile: &Tile, label: &str| {
            let key = |key, pressed| egui::Event::Key {
                key,
                pressed,
                physical_key: None,
                repeat: false,
                modifiers: Default::default(),
            };
            for _ in 0..32 {
                let mut events = draw(tile, vec![key(egui::Key::Tab, true)]);
                events.extend(draw(tile, vec![key(egui::Key::Tab, false)]));
                if events.iter().any(|event| matches!(event,
                    egui::output::OutputEvent::FocusGained(info) if info.label.as_deref() == Some(label)
                )) {
                    draw(tile, vec![key(egui::Key::Enter, true)]);
                    draw(tile, vec![key(egui::Key::Enter, false)]);
                    return;
                }
            }
            panic!("Could not reach {label} with Tab");
        };
        activate(&second, tr("Watch"));
        assert!(
            matches!(next_command(), Some(Command::Watch { id, enabled: true, .. }) if id == second.id)
        );
        second.watching = true;
        activate(&second, &format!("{} · Second", tr("Stop watching")));
        assert!(
            matches!(next_command(), Some(Command::Watch { id, enabled: false, .. }) if id == second.id)
        );
        assert!(next_command().is_none());
    }

    #[test]
    fn clipped_stream_preserves_picture_and_resumes_without_changing_watch_or_volume() {
        let mut app = app();
        let ctx = egui::Context::default();
        let tile = Tile {
            id: "clipping".into(),
            kind: crate::preview::Kind::Screen,
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
            kind: crate::preview::Kind::Screen,
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
            let id = app.textures[SELF_PREVIEW].id();
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
    fn theme_menu_accepts_clicks_on_the_swatch_and_trailing_space() {
        for swatch in [true, false] {
            let mut app = app();
            let ctx = egui::Context::default();
            configure_theme(&ctx);
            let mut draw = |events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800.0, 600.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |root| {
                        egui::CentralPanel::default().show(root, |ui| {
                            let button = ui.button(tr("Theme"));
                            egui::Popup::menu(&button).show(|ui| app.theme_options(ui));
                        });
                    },
                );
                output.textures_delta.clear();
                output
            };
            let trigger = draw(vec![])
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) if text.galley.text() == tr("Theme") => {
                        Some(text.pos + text.galley.size() / 2.0)
                    }
                    _ => None,
                })
                .expect("Theme menu button is visible");
            let mut click = |pos| {
                for pressed in [true, false] {
                    draw(vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ]);
                }
                draw(vec![]);
                draw(vec![])
            };
            // Open the theme popup, then use the actual rendered swatch or the space after Ruby.
            let output = click(trigger);
            let theme = Theme::Ruby;
            let point = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) if text.galley.text() == tr(theme.label()) => {
                        let size = text.galley.size();
                        let offset = if swatch { -21.0 } else { size.x + 24.0 };
                        Some(text.pos + egui::vec2(offset, size.y / 2.0))
                    }
                    _ => None,
                })
                .expect("Theme popup is visible");
            click(point);
            assert_eq!(app.appearance.theme, theme);
            assert!(!egui::Popup::is_any_open(&ctx));
        }
    }

    #[test]
    fn fullscreen_click_menu_survives_idle_and_applies_actions() {
        for action in [
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
                kind: crate::preview::Kind::Screen,
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
            let button = if action == "Focus stream" {
                egui::PointerButton::Primary
            } else {
                egui::PointerButton::Secondary
            };
            for pressed in [true, false] {
                draw(
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(position),
                        click(position, button, pressed),
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
    fn speaker_opens_a_vertical_slider_and_changes_volume_at_the_bottom() {
        let mut app = app();
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let tile = Tile {
            id: "volume-test".into(),
            kind: crate::preview::Kind::Screen,
            name: "Friend".into(),
            codec: String::new(),
            watching: true,
            picture: Default::default(),
            presentation: Default::default(),
            frames: Default::default(),
            received_bps: Default::default(),
            volume: std::sync::Arc::new(63.into()),
            audio: Default::default(),
            status: String::new(),
        };
        let mut draw = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 500.0),
                    )),
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        ui.add_space(240.0);
                        app.playback_controls(ui, &tile, false);
                    });
                },
            );
            output.textures_delta.clear();
            output
        };
        let initial = draw(vec![]);
        let stop = initial
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.fill == palette().error && rect.rect.width() < 14.0 =>
                {
                    Some(rect.rect.center())
                }
                _ => None,
            })
            .expect("Red stop button");
        let mut click = |pos| {
            for pressed in [true, false] {
                draw(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ]);
            }
            draw(vec![]);
            draw(vec![])
        };
        assert!(!egui::Popup::is_any_open(&ctx));
        let speaker = stop + egui::vec2(36.0, 0.0);
        let popup = click(speaker);
        assert!(egui::Popup::is_any_open(&ctx));
        assert_eq!(tile.volume.load(Ordering::Relaxed), 63);
        let rail = popup
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.rect.width() < 14.0 && rect.rect.height() > 60.0 =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .expect("Vertical volume rail");
        assert!(rail.bottom() < speaker.y);
        assert!(
            (rail.center().x - speaker.x).abs() <= 2.0,
            "Rail {rail:?} should align with the speaker at {speaker:?}"
        );
        click(rail.center_bottom() - egui::vec2(0.0, 1.0));
        assert_eq!(tile.volume.load(Ordering::Relaxed), 0);
        assert!(egui::Popup::is_any_open(&ctx));
    }

    #[test]
    fn stream_badge_uses_the_publishers_photo_by_participant_id() {
        let mut app = app();
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let name = "Friend";
        let revision = "fixture-picture";
        app.profile_state.profiles = [("other", None), ("publisher", Some(revision))]
            .into_iter()
            .map(|(id, revision)| crate::profile::Participant {
                participant_id: id.into(),
                display_name: name.into(),
                avatar_revision: revision.map(str::to_owned),
                uploads_blocked: false,
                discord: false,
            })
            .collect();
        app.profile_state.avatars.insert(
            revision.into(),
            std::sync::Arc::new(crate::profile::Animation {
                frames: vec![(
                    egui::ColorImage::filled([2, 2], egui::Color32::RED),
                    Duration::from_secs(1),
                )],
                duration: Duration::from_secs(1),
            }),
        );
        for (id, has_photo) in [("publisher", true), ("other", false)] {
            let tile = Tile {
                id: id.into(),
                kind: crate::preview::Kind::Screen,
                name: name.into(),
                codec: String::new(),
                watching: true,
                picture: Default::default(),
                presentation: Default::default(),
                frames: Default::default(),
                received_bps: Default::default(),
                volume: Default::default(),
                audio: Default::default(),
                status: String::new(),
            };
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    events: vec![egui::Event::PointerMoved(egui::pos2(300.0, 180.0))],
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default()
                        .show(root, |ui| app.stream_tile(ui, &tile, 600.0));
                },
            );
            output.textures_delta.clear();
            let avatar_id = app.profile_textures[revision].1.id();
            let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
            let painted = primitives.iter().any(|entry| {
                matches!(&entry.primitive, egui::epaint::Primitive::Mesh(mesh) if mesh.texture_id == avatar_id)
            });
            assert_eq!(painted, has_photo, "{id}");
        }
    }

    #[test]
    fn watched_stream_overlays_follow_hover_and_metrics_hug_the_corner() {
        let mut app = app();
        app.fullscreen_controls_until = Instant::now() + Duration::from_secs(3);
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let mut tile = Tile {
            id: "synthetic-stream".into(),
            kind: crate::preview::Kind::Screen,
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
        let mut draw = |tile: &Tile, events, fullscreen| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    events,
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
            let mut video = None;
            let mut metrics = None;
            let mut stop_icon = false;
            output.textures_delta.clear();
            for shape in output.shapes {
                painted_text(&shape.shape, &mut text);
                if let egui::Shape::Rect(rect) = shape.shape {
                    if rect.fill == egui::Color32::BLACK && rect.rect.width() == 600.0 {
                        video = Some(rect.rect);
                    }
                    if rect.fill == egui::Color32::from_black_alpha(195) {
                        metrics = Some(rect.rect);
                    }
                    stop_icon |= rect.fill == palette().error && rect.rect.width() < 14.0;
                }
            }
            (text, video, metrics, stop_icon)
        };
        let inside = egui::Event::PointerMoved(egui::pos2(300.0, 180.0));
        let outside = egui::Event::PointerMoved(egui::pos2(740.0, 180.0));
        let check_hidden = |text: &str| {
            assert!(!text.contains("Mbps"));
            assert!(!text.contains("Friend"));
            assert!(!text.contains(tr("Stop watching")));
        };
        check_hidden(&draw(&tile, vec![egui::Event::PointerGone], false).0);
        let (visible, video, metrics, stop_icon) = draw(&tile, vec![inside.clone()], false);
        assert!(visible.contains("HEVC  ·  15.5 Mbps"), "{visible}");
        assert!(visible.contains("Friend"));
        assert!(stop_icon);
        let inset = video.unwrap().right_bottom() - metrics.unwrap().right_bottom();
        assert!((inset - egui::vec2(10.0, 8.0)).length() < 0.1);
        check_hidden(&draw(&tile, vec![outside.clone()], false).0);

        for pressed in [true, false] {
            draw(
                &tile,
                vec![egui::Event::Key {
                    key: egui::Key::Tab,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Default::default(),
                }],
                false,
            );
        }
        let keyboard = draw(&tile, vec![], false);
        assert!(keyboard.3);
        check_hidden(&draw(&tile, vec![egui::Event::PointerGone], false).0);

        tile.watching = false;
        let preview = draw(&tile, vec![], false).0;
        assert!(preview.contains(tr("Watch")));
        assert!(preview.contains("Friend"));
        assert!(!preview.contains("Mbps"));
        tile.watching = true;
        assert!(!draw(&tile, vec![], true).0.contains("Mbps"));
        assert!(draw(&tile, vec![inside], true).0.contains("HEVC"));
    }

    #[test]
    fn empty_room_avatar_displays_a_name_on_hover_and_keyboard_focus() {
        let mut app = app();
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let name = "Marina Silva";
        let state = Snapshot {
            participants: vec![name.into()],
            ..Default::default()
        };
        let mut draw = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 420.0),
                    )),
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| app.empty_room_stage(ui, &state));
                },
            );
            output.textures_delta.clear();
            output
        };
        let contains_name = |output: egui::FullOutput| {
            let mut rendered = String::new();
            for shape in &output.shapes {
                painted_text(&shape.shape, &mut rendered);
            }
            rendered.contains(name)
        };
        let idle = draw(vec![]);
        let center = idle
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Circle(circle) if (circle.radius - 20.0).abs() < 0.1 => {
                    Some(circle.center)
                }
                _ => None,
            })
            .expect("The presence avatar");
        assert!(!contains_name(idle));
        assert!(contains_name(draw(vec![egui::Event::PointerMoved(center)])));
        assert!(!contains_name(draw(vec![egui::Event::PointerGone])));
        assert!(contains_name(draw(vec![egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }])));
    }

    #[test]
    fn camera_preview_prefers_the_left_and_stays_inside_the_stage() {
        let rect = |x, y, width, height| {
            egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height))
        };
        let stage = rect(6.0, 60.0, 948.0, 392.0);
        let settings = rect(520.0, 200.0, 364.0, 240.0);
        let (pos, width) = camera_preview_placement(stage, settings).unwrap();
        assert!(pos.x + width + 22.0 < settings.left());
        assert!(pos.y + width * 9.0 / 16.0 + 48.0 <= stage.bottom());

        let settings = settings.translate(egui::vec2(-490.0, 0.0));
        let (pos, width) = camera_preview_placement(stage, settings).unwrap();
        assert!(pos.x > settings.right());
        assert!(pos.x + width + 22.0 <= stage.right());

        let stage = rect(6.0, 52.0, 398.0, 480.0);
        let settings = rect(24.0, 270.0, 364.0, 220.0);
        let (pos, width) = camera_preview_placement(stage, settings).unwrap();
        assert!(stage.contains(pos));
        assert!(pos.y + width * 9.0 / 16.0 + 48.0 < settings.top());
    }

    #[test]
    #[cfg(windows)]
    fn camera_dock_uses_separate_settings_and_requires_explicit_start() {
        let mut app = app();
        let state = Snapshot {
            connection: "connected",
            cameras: vec![camera::Info {
                id: "test-camera".into(),
                name: "Test camera".into(),
            }],
            encoders: vec![crate::encoding::Encoder {
                factory: "test-encoder".into(),
                label: "Test encoder".into(),
                codec: quality::Codec::H264,
            }],
            ..Default::default()
        };
        let (controller, mut commands) = Controller::recording_for_test(state);
        app.controller = controller;
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960.0, 540.0));
        let draw = |app: &mut PublisherUi, events| {
            let state = app.controller.snapshot();
            let mut buttons = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(window),
                    events,
                    ..Default::default()
                },
                |root| {
                    let dock = app.studio_dock(root, &state);
                    buttons = Some([dock.screen, dock.camera]);
                    if let Some(setup) = app.sharing_setup {
                        let stage = root.available_rect_before_wrap();
                        app.sharing_setup_popover(root.ctx(), &state, stage, dock, setup);
                    }
                },
            );
            output.textures_delta.clear();
            (buttons.unwrap(), output)
        };
        let click = |app: &mut PublisherUi, pos| {
            for pressed in [true, false] {
                draw(
                    app,
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
            draw(app, vec![]);
            draw(app, vec![]).1
        };
        let text_center = |output: &egui::FullOutput, label| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos + text.galley.size() / 2.0)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing control: {label}"))
        };
        draw(&mut app, vec![]);
        let ([screen, camera], _) = draw(&mut app, vec![]);
        assert!((screen.center().y - camera.center().y).abs() < 0.5);
        let arrow = |button: egui::Rect| button.right_center() - egui::vec2(16.0, 0.0);
        click(&mut app, arrow(screen));
        assert!(app.sharing_setup == Some(SharingSetup::Screen));
        let screen_quality = app.quality;
        let output = click(&mut app, arrow(camera));
        assert!(app.sharing_setup == Some(SharingSetup::Camera));
        assert!(matches!(commands(), Some(Command::RefreshCameras)));
        let output = click(&mut app, text_center(&output, "30 FPS"));
        click(&mut app, text_center(&output, "24 FPS"));
        assert_eq!(app.camera_settings.fps, 24);
        assert_eq!(app.quality, screen_quality);
        click(&mut app, arrow(camera));
        assert!(app.sharing_setup.is_none());
        click(&mut app, camera.left_center() + egui::vec2(42.0, 0.0));
        assert!(app.sharing_setup.is_none());
        assert!(commands().is_none());
        app.camera_settings.device_id = "test-camera".into();
        draw(&mut app, vec![]);
        let ([_, camera], _) = draw(&mut app, vec![]);
        click(&mut app, camera.left_center() + egui::vec2(42.0, 0.0));
        assert!(matches!(
            commands(),
            Some(Command::StartCamera { device_id, quality, .. })
                if device_id == "test-camera" && quality.fps == 24
                    && quality.resolution == quality::Resolution::P720
                    && quality.bitrate == Bitrate::Auto
        ));
        let ([_, camera], _) = draw(&mut app, vec![]);
        click(&mut app, camera.left_center() + egui::vec2(42.0, 0.0));
        assert!(matches!(commands(), Some(Command::StopCamera)));
    }

    #[test]
    fn floating_dock_keeps_its_bottom_edge_and_shadow_inside_the_window() {
        let mut app = app();
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let mut state = Snapshot {
            connection: "connected",
            ..Default::default()
        };
        let frame = skin::dock_frame();
        let mut dock = egui::Rect::NOTHING;
        for (width, height, sharing, roster) in [
            (960.0, 540.0, false, false),
            (960.0, 540.0, false, true),
            (820.0, 480.0, false, true),
            (820.0, 480.0, true, true),
        ] {
            state.sharing = sharing;
            state.active_selection = sharing.then(|| app.selection());
            app.source_id.push('x');
            let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
            let mut painted_with_room_for_shadow = false;
            let mut roster_reaches_bottom = false;
            let mut stage = window;
            for _ in 0..3 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(window),
                        ..Default::default()
                    },
                    |root| {
                        if roster {
                            app.roster_drawer(root, &state);
                        }
                        stage = root.available_rect_before_wrap();
                        dock = app.studio_dock(root, &state).rect;
                    },
                );
                output.textures_delta.clear();
                painted_with_room_for_shadow = output.shapes.iter().any(|shape| {
                    shape.clip_rect == window && matches!(&shape.shape,
                        egui::Shape::Vec(parts) if parts.iter().any(|part| matches!(part,
                            egui::Shape::Rect(rect) if rect.corner_radius == frame.corner_radius
                                && rect.fill == frame.fill && rect.stroke == frame.stroke))
                            && parts.iter().all(|part| stage.contains_rect(part.visual_bounding_rect()))
                    )
                });
                roster_reaches_bottom = output.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Rect(rect)
                        if rect.fill == palette().panel && rect.stroke.color == palette().divider
                            && rect.rect.left() >= stage.right() - 1.0
                            && (rect.rect.bottom() - window.bottom()).abs() <= 1.0)
                });
            }
            assert!(painted_with_room_for_shadow);
            assert_eq!(roster_reaches_bottom, roster);
            assert!(stage.contains_rect(dock));
            assert!((dock.center().x - stage.center().x).abs() <= 1.0);
            assert!((window.bottom() - dock.bottom() - 20.0).abs() <= 1.0);
        }
    }

    #[test]
    fn long_room_names_preserve_header_actions_at_the_minimum_window_width() {
        let mut app = app();
        app.invitation="https://room.example/r/AAAAAAAAAAAAAAAAAAAAAA#invite=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into();
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
