use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsPage {
    Profile,
    Discord,
}

fn surface_card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    let width = ui.available_width();
    let compact = ui.ctx().content_rect().height() <= 600.0;
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(14)
        .inner_margin(egui::Margin::symmetric(20, if compact { 10 } else { 20 }))
        .show(ui, |ui| {
            ui.set_width((width - 42.0).max(0.0));
            ui.spacing_mut().item_spacing.y = if compact { 6.0 } else { 10.0 };
            content(ui);
        });
}

fn labeled_divider(ui: &mut egui::Ui, text: &str) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 20.0), egui::Sense::hover());
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        egui::FontId::proportional(11.5),
        skin::TERTIARY,
    );
    let text_w = galley.size().x;
    let gap = 12.0;
    let mid_y = rect.center().y;
    let left_end = (rect.center().x - text_w / 2.0 - gap).max(rect.left());
    let right_start = (rect.center().x + text_w / 2.0 + gap).min(rect.right());
    if left_end > rect.left() {
        ui.painter().line_segment(
            [egui::pos2(rect.left(), mid_y), egui::pos2(left_end, mid_y)],
            egui::Stroke::new(1.0, skin::DIVIDER),
        );
    }
    if right_start < rect.right() {
        ui.painter().line_segment(
            [
                egui::pos2(right_start, mid_y),
                egui::pos2(rect.right(), mid_y),
            ],
            egui::Stroke::new(1.0, skin::DIVIDER),
        );
    }
    ui.painter().galley(
        egui::pos2(
            rect.center().x - text_w / 2.0,
            mid_y - galley.size().y / 2.0,
        ),
        galley,
        skin::TERTIARY,
    );
}

fn joining_status(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.colored_label(color, "●");
        ui.label(skin::strong(tr(text), 13.0).color(color));
    });
}

fn detail_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(12.0).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(egui::Label::new(egui::RichText::new(value).size(12.0).color(TEXT)).truncate());
        });
    });
}

pub(super) fn avatar(ui: &mut egui::Ui, name: &str, size: f32, texture: Option<egui::TextureId>) {
    if let Some(texture) = texture {
        ui.add(
            egui::Image::new((texture, egui::vec2(size, size)))
                .corner_radius(egui::CornerRadius::same((size / 2.0) as u8)),
        );
    } else {
        super::avatar(ui, name, size);
    }
}

impl PublisherUi {
    pub(super) fn poll_following(&mut self, ctx: &egui::Context) {
        if self.closing_since.is_some() {
            return;
        }
        let key = self
            .account_state
            .account
            .as_ref()
            .map(|a| format!("{}:{}", self.account_state.server, a.session_id));
        if self.following_key != key {
            self.following_client
                .set_credential(self.account_state.credential.clone());
            self.following_key = key;
        }
        let state = self.controller.snapshot();
        if self.following.current.is_some()
            && !self.browser_invitation_open
            && state.connection == "connected"
            && let Some(access) = &state.profile_access
        {
            self.invitation = access.invitation_url.clone().unwrap_or_default();
        }
        let voice = self.following_client.state();
        let busy = state.busy || Instant::now() < self.next_follow_attempt;
        if let Some(action) = self.following.tick(
            self.account_state.account.is_some(),
            &voice,
            matches!(state.connection, "connected" | "reconnecting"),
            busy,
            self.browser_invitation_open,
        ) {
            self.set_fullscreen(ctx, None);
            self.focused = None;
            self.textures.clear();
            match action {
                crate::following::Action::Leave => self.controller.send(Command::Leave),
                crate::following::Action::Join(location) => {
                    self.next_follow_attempt = Instant::now() + Duration::from_secs(10);
                    self.controller.send(Command::Join {
                        invitation: String::new(),
                        name: self.name.clone(),
                        route: Route::Auto,
                        account: self.account_state.credential.clone(),
                        voice: Some(crate::following::Target {
                            server: self.account_state.server.clone(),
                            location,
                        }),
                    });
                }
            }
        }
    }

    pub(super) fn join_landing(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let compact = ui.ctx().content_rect().height() <= 600.0;
        egui::ScrollArea::vertical()
            .id_salt("room-entry")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let width = ui.available_width().min(436.0);
                let offset = ((ui.available_width() - width) / 2.0).max(0.0);
                let top_pad = ((ui.available_height() - 470.0) * 0.36).clamp(16.0, 72.0);
                ui.add_space(if compact { 0.0 } else { top_pad });
                ui.horizontal_top(|ui| {
                    ui.add_space(offset);
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.heading(skin::strong(tr("Join a room"), 26.0).color(TEXT));
                        ui.add_space(if compact { 4.0 } else { 12.0 });

                        // Option 1 (Primary): Discord voice / sign-in
                        surface_card(ui, |ui| {
                            if let Some(display_name) = self
                                .account_state
                                .account
                                .as_ref()
                                .map(|account| account.display_name.clone())
                            {
                                ui.horizontal(|ui| {
                                    avatar(ui, &display_name, 34.0, None);
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = 1.0;
                                        ui.label(skin::strong(&display_name, 14.5).color(TEXT));
                                        ui.label(
                                            egui::RichText::new("Discord").size(11.5).color(MUTED),
                                        );
                                    });
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .add(
                                                    egui::Button::new(tr("Manage"))
                                                        .corner_radius(7),
                                                )
                                                .clicked()
                                            {
                                                self.open_profile();
                                                self.profile_page = SettingsPage::Discord;
                                            }
                                        },
                                    );
                                });
                                ui.separator();
                                self.discord_following(ui);
                            } else {
                                ui.label(skin::strong(tr("Discord voice"), 15.0).color(TEXT));
                                self.discord_sign_in(ui);
                            }
                        });

                        if compact {
                            ui.add_space(6.0);
                        } else {
                            ui.add_space(10.0);
                            labeled_divider(ui, tr("or"));
                            ui.add_space(10.0);
                        }

                        // Option 2 (Secondary): Join via room invitation without signing in
                        surface_card(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(skin::strong(tr("Room invitation"), 15.0).color(TEXT));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(tr("No login required"))
                                                .size(11.5)
                                                .color(skin::TERTIARY),
                                        );
                                    },
                                );
                            });
                            self.join_form(ui, state);
                        });
                        ui.add_space(24.0);
                    });
                });
            });
    }

    fn discord_account(&mut self, ui: &mut egui::Ui) {
        if let Some(account) = &self.account_state.account {
            let display_name = account.display_name.clone();
            let user_id = account.user_id.clone();
            let expires_at = account.expires_at;
            ui.horizontal(|ui| {
                avatar(ui, &display_name, 42.0, None);
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(skin::strong(&display_name, 15.5).color(TEXT));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        ui.colored_label(SAGE, "●");
                        ui.label(
                            egui::RichText::new(tr("Connected"))
                                .size(12.0)
                                .color(MUTED),
                        );
                    });
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let disconnect = ui
                        .add(
                            egui::Button::new(
                                skin::strong(tr("Disconnect Discord"), 12.5).color(skin::DANGER_INK),
                            )
                            .corner_radius(8)
                            .fill(ERROR.gamma_multiply(0.10))
                            .stroke(egui::Stroke::new(1.0, ERROR.gamma_multiply(0.35)))
                            .min_size(egui::vec2(0.0, 32.0)),
                        )
                        .on_hover_text(tr(
                            "Sign out of this server and remove the remembered login from this device.",
                        ));
                    if disconnect.clicked() {
                        self.account_client.sign_out();
                    }
                });
            });
            ui.add_space(2.0);
            egui::CollapsingHeader::new(
                egui::RichText::new(tr("Account details"))
                    .size(12.5)
                    .color(MUTED),
            )
            .id_salt("discord-account-details")
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 5.0;
                detail_row(ui, tr("PodeTelar server"), &self.account_state.server);
                detail_row(ui, "Discord ID", &user_id);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let days = expires_at.saturating_sub(now) / 86_400_000;
                detail_row(ui, tr("Remembered login days remaining"), &days.to_string());
            });
            if !self.account_state.error.is_empty() {
                ui.colored_label(ERROR, tr(&self.account_state.error));
            }
        } else {
            self.discord_sign_in(ui);
        }
    }

    fn discord_sign_in(&mut self, ui: &mut egui::Ui) {
        if self.account_state.busy {
            ui.horizontal_wrapped(|ui| {
                ui.spinner();
                ui.label(tr("Finish signing in in your browser."));
            });
            if ui
                .add(
                    egui::Button::new(tr("Cancel sign-in"))
                        .corner_radius(8)
                        .min_size(egui::vec2(ui.available_width(), 34.0)),
                )
                .clicked()
            {
                self.account_client.cancel();
            }
        } else {
            let valid = crate::admission::valid_server(self.account_server.trim());
            if ui
                .add_enabled(
                    valid,
                    primary(tr("Sign in with Discord"), ui.available_width())
                        .min_size(egui::vec2(ui.available_width(), 40.0)),
                )
                .clicked()
            {
                self.account_client
                    .sign_in(self.account_server.trim().to_owned());
            }
            egui::CollapsingHeader::new(
                egui::RichText::new(tr("Server settings"))
                    .size(12.0)
                    .color(MUTED),
            )
            .id_salt("discord-server-settings")
            .show(ui, |ui| {
                let label = ui.label(
                    egui::RichText::new(tr("PodeTelar server"))
                        .size(12.0)
                        .color(MUTED),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.account_server)
                        .desired_width(f32::INFINITY)
                        .margin(egui::Margin::symmetric(10, 7)),
                )
                .labelled_by(label.id);
                if !valid {
                    ui.colored_label(WARNING, tr("Use an HTTPS server or a local test address."));
                } else if !self.account_state.available
                    && self.account_state.server == self.account_server.trim()
                {
                    ui.label(
                        egui::RichText::new(tr(
                            "Discord sign-in may need to be enabled on this server.",
                        ))
                        .size(12.0)
                        .color(MUTED),
                    );
                }
            });
        }
        if !self.account_state.error.is_empty() {
            ui.colored_label(ERROR, tr(&self.account_state.error));
        }
    }

    fn discord_following(&mut self, ui: &mut egui::Ui) {
        let voice = self.following_client.state();
        if self.following.paused || self.following.independent {
            ui.horizontal(|ui| {
                joining_status(ui, "Automatic joining paused", WARNING);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new(tr("Resume automatic joining")).corner_radius(7))
                        .clicked()
                    {
                        self.following.resume();
                    }
                });
            });
        } else if !voice.known {
            joining_status(ui, "Waiting for Discord voice information…", MUTED);
        } else if !voice.enabled {
            joining_status(ui, "Automatic joining disabled", WARNING);
        } else if voice.locations.is_empty() {
            joining_status(ui, "Waiting for your Discord voice channel…", MUTED);
        } else {
            for channel in &voice.locations {
                let current = self
                    .following
                    .current
                    .as_ref()
                    .is_some_and(|location| location.key() == channel.key());
                egui::Frame::new()
                    .fill(CARD)
                    .stroke(egui::Stroke::new(
                        1.0,
                        if current {
                            ACCENT.gamma_multiply(0.45)
                        } else {
                            BORDER
                        },
                    ))
                    .corner_radius(10)
                    .inner_margin(egui::Margin::symmetric(12, 9))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.label(skin::strong(&channel.channel_name, 13.5).color(TEXT));
                                ui.label(
                                    egui::RichText::new(&channel.guild_name)
                                        .size(11.5)
                                        .color(MUTED),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if !channel.enabled {
                                        badge(ui, tr("Disabled"), WARNING);
                                    } else if current {
                                        badge(ui, tr("Following"), ACCENT);
                                    } else if voice.locations.len() > 1
                                        && ui
                                            .button(format!(
                                                "{} {}",
                                                tr("Follow channel"),
                                                channel.channel_name
                                            ))
                                            .clicked()
                                    {
                                        self.following.selected = Some(channel.key());
                                    }
                                },
                            );
                        });
                    });
            }
        }
    }

    pub(super) fn observe_profile_name(&mut self, name: &str) {
        // An invitation has its own editable name draft in self.name. Live updates
        // to the current room must not replace that draft before it is submitted.
        if !self.browser_invitation_open {
            self.name = name.to_owned();
        }
        self.session_name = Some(name.to_owned());
    }

    pub(super) fn remember_guest_name(&mut self, name: String) {
        self.name = name.clone();
        self.session_name = Some(name.clone());
        self.saved_guest_name = name;
    }

    pub(super) fn open_profile(&mut self) {
        self.profile_name = self.name.clone();
        self.profile_page = SettingsPage::Profile;
        self.profile_open = true;
    }

    pub(super) fn poll_profile(&mut self, state: &Snapshot) {
        if matches!(state.connection, "connected" | "reconnecting") {
            if let Some(access) = &state.profile_access
                && self
                    .profile_client
                    .as_ref()
                    .is_none_or(|client| client.participant_id != access.participant_id)
            {
                self.profile_client = Some(crate::profile::Client::new(access.clone()));
                self.profile_connection_key = None;
            }
        } else {
            self.profile_client = None;
            self.profile_state = Default::default();
        }
        if let Some(client) = &self.profile_client {
            let credential = self
                .account_state
                .credential
                .as_ref()
                .filter(|c| c.for_server(&client.server).is_some())
                .cloned();
            let session = credential
                .as_ref()
                .and(self.account_state.account.as_ref())
                .map(|a| a.session_id.clone());
            let key = (client.participant_id.clone(), session);
            let managed = key.1.is_some();
            if self.profile_connection_key.as_ref() != Some(&key) {
                let was_discord = self
                    .profile_connection_key
                    .as_ref()
                    .is_some_and(|(_, session)| session.is_some());
                let restore = (was_discord && key.1.is_none() && !self.saved_guest_name.is_empty())
                    .then(|| self.saved_guest_name.clone());
                client.connect(credential, restore);
                self.profile_connection_key = Some(key);
            }
            self.profile_state = client.state();
            if let Some(own) = self
                .profile_state
                .profiles
                .iter()
                .find(|p| p.participant_id == client.participant_id)
            {
                let name = own.display_name.clone();
                let discord = own.discord;
                self.observe_profile_name(&name);
                if !discord
                    && !managed
                    && self.profile_state.identity_synced
                    && self.saved_guest_name != name
                {
                    if crate::settings::save_guest_name(&name).is_err() {
                        self.settings_error =
                            "Could not remember your guest name on this PC.".into();
                    }
                    self.saved_guest_name = name;
                }
            }
        }
        self.profile_textures
            .retain(|revision, _| self.profile_state.avatars.contains_key(revision));
    }

    pub(super) fn apply_profile_names(&self, state: &mut Snapshot) {
        for profile in &self.profile_state.profiles {
            if let Some((index, (_, name))) = state
                .participant_ids
                .iter_mut()
                .enumerate()
                .find(|(_, (id, _))| id == &profile.participant_id)
            {
                name.clone_from(&profile.display_name);
                if let Some(label) = state.participants.get_mut(index) {
                    label.clone_from(name);
                }
            }
            if let Some(tile) = state
                .tiles
                .iter_mut()
                .find(|tile| tile.id == profile.participant_id)
            {
                tile.name.clone_from(&profile.display_name);
            }
        }
    }

    pub(super) fn profile_texture(
        &mut self,
        ctx: &egui::Context,
        participant: &str,
    ) -> Option<egui::TextureId> {
        let profile = self
            .profile_state
            .profiles
            .iter()
            .find(|p| p.participant_id == participant)?;
        let revision = profile.avatar_revision.as_ref()?;
        let animation = self.profile_state.avatars.get(revision)?;
        let frame = animation.frame(self.profile_started.elapsed());
        if animation.frames.len() > 1 {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
        if let Some((current, texture)) = self.profile_textures.get_mut(revision) {
            if *current != frame {
                texture.set(
                    animation.frames[frame].0.clone(),
                    egui::TextureOptions::LINEAR,
                );
                *current = frame;
            }
            return Some(texture.id());
        }
        let texture = ctx.load_texture(
            format!("avatar-{revision}"),
            animation.frames[frame].0.clone(),
            egui::TextureOptions::LINEAR,
        );
        let id = texture.id();
        self.profile_textures
            .insert(revision.clone(), (frame, texture));
        Some(id)
    }

    fn edit_nickname(&mut self, ui: &mut egui::Ui) {
        let label = ui.label(
            egui::RichText::new(tr("Guest nickname"))
                .size(12.0)
                .color(MUTED),
        );
        let busy = self.profile_state.busy;
        let valid = !busy && crate::settings::valid_name(&self.profile_name);
        let save_label = tr("Save nickname");
        let save_w = (button_width(ui, save_label) + 8.0).max(96.0);
        let mut submit = false;
        ui.horizontal(|ui| {
            let input_w = (ui.available_width() - save_w - ui.spacing().item_spacing.x).max(120.0);
            let field = ui
                .add_enabled(
                    !busy,
                    egui::TextEdit::singleline(&mut self.profile_name)
                        .char_limit(60)
                        .desired_width(input_w)
                        .margin(egui::Margin::symmetric(10, 7)),
                )
                .labelled_by(label.id);
            if valid && field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submit = true;
            }
            if ui
                .add_enabled(
                    valid,
                    egui::Button::new(skin::strong(save_label, 12.5).color(if valid {
                        AMBER_INK
                    } else {
                        MUTED
                    }))
                    .fill(if valid { ACCENT } else { CARD })
                    .corner_radius(8)
                    .min_size(egui::vec2(save_w, 34.0)),
                )
                .clicked()
            {
                submit = true;
            }
        });
        if submit {
            let name = self.profile_name.trim().to_owned();
            if let Some(client) = &self.profile_client {
                client.name(name);
            } else if crate::settings::save_guest_name(&name).is_ok() {
                self.remember_guest_name(name);
            } else {
                self.settings_error = "Could not remember your guest name on this PC.".into();
            }
        }
    }

    fn profile_cards(&mut self, ui: &mut egui::Ui, connected: bool) {
        let own = self
            .profile_client
            .as_ref()
            .and_then(|client| {
                self.profile_state
                    .profiles
                    .iter()
                    .find(|p| p.participant_id == client.participant_id)
            })
            .cloned();
        let texture = own
            .as_ref()
            .and_then(|p| self.profile_texture(ui.ctx(), &p.participant_id));
        let is_discord = own.as_ref().is_some_and(|p| p.discord);

        surface_card(ui, |ui| {
            ui.horizontal(|ui| {
                avatar(ui, &self.name, 58.0, texture);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 5.0;
                    ui.horizontal(|ui| {
                        ui.label(skin::strong(&self.name, 16.0).color(TEXT));
                        badge(
                            ui,
                            if is_discord { "Discord" } else { tr("Guest") },
                            if is_discord { ACCENT } else { MUTED },
                        );
                    });
                    if is_discord {
                        if ui
                            .add(
                                egui::Button::new(tr("Manage Discord connection")).corner_radius(8),
                            )
                            .clicked()
                        {
                            self.profile_page = SettingsPage::Discord;
                        }
                    } else if let Some(client) = &self.profile_client {
                        let busy = self.profile_state.busy;
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_enabled(
                                    !busy && own.as_ref().is_some_and(|p| !p.uploads_blocked),
                                    egui::Button::new(tr("Upload picture")).corner_radius(8),
                                )
                                .on_hover_text(tr("PNG, JPEG, WebP or animated GIF, up to 3 MB."))
                                .clicked()
                            {
                                client.choose();
                            }
                            if own.as_ref().is_some_and(|p| p.avatar_revision.is_some())
                                && ui
                                    .add_enabled(
                                        !busy,
                                        egui::Button::new(tr("Remove picture")).corner_radius(8),
                                    )
                                    .clicked()
                            {
                                client.remove();
                            }
                            if busy {
                                ui.spinner();
                            }
                        });
                    } else {
                        ui.label(
                            egui::RichText::new(tr(if connected {
                                "Profile pictures are unavailable in this room."
                            } else {
                                "Join a room to manage your profile picture."
                            }))
                            .size(12.0)
                            .color(MUTED),
                        );
                    }
                    if own.as_ref().is_some_and(|p| p.uploads_blocked) {
                        ui.colored_label(
                            WARNING,
                            tr("Profile picture uploads are disabled for this PC."),
                        );
                    }
                });
            });

            if !is_discord {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
                self.edit_nickname(ui);
            }
        });

        if !self.profile_state.error.is_empty() {
            ui.add_space(6.0);
            ui.colored_label(ERROR, tr(&self.profile_state.error));
        }
    }

    pub(super) fn profile_dialog(&mut self, ctx: &egui::Context, connected: bool) {
        if !self.profile_open {
            return;
        }
        let width = (ctx.content_rect().width() - 64.0).clamp(320.0, 472.0);
        let max_body_height = (ctx.content_rect().height() - 255.0).clamp(96.0, 460.0);
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::unique("user-settings"))
            .backdrop_color(egui::Color32::from_black_alpha(175))
            .frame(
                skin::menu_frame()
                    .fill(CANVAS)
                    .corner_radius(16)
                    .inner_margin(egui::Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.horizontal(|ui| {
                    ui.label(skin::strong(tr("User settings"), 20.0).color(TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if skin::icon_button(ui, Icon::Close, tr("Close")).clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(10.0);

                // Crisp underline tabs
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 18.0;
                    for (page, label) in [
                        (SettingsPage::Profile, "Profile"),
                        (SettingsPage::Discord, "Discord"),
                    ] {
                        let selected = self.profile_page == page;
                        let text = skin::strong(tr(label), 13.5).color(if selected {
                            TEXT
                        } else {
                            MUTED
                        });
                        let response = ui.add(
                            egui::Button::new(text)
                                .frame(false)
                                .min_size(egui::vec2(0.0, 30.0)),
                        );
                        if selected {
                            let r = response.rect;
                            ui.painter().line_segment(
                                [
                                    egui::pos2(r.left(), r.bottom() + 2.0),
                                    egui::pos2(r.right(), r.bottom() + 2.0),
                                ],
                                egui::Stroke::new(2.0, ACCENT),
                            );
                        }
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::Tab,
                                ui.is_enabled(),
                                selected,
                                tr(label),
                            )
                        });
                        if response.clicked() {
                            self.profile_page = page;
                        }
                    }
                });
                ui.separator();
                ui.add_space(10.0);

                egui::ScrollArea::vertical()
                    .id_salt(match self.profile_page {
                        SettingsPage::Profile => "profile",
                        SettingsPage::Discord => "discord",
                    })
                    .max_height(max_body_height)
                    .show(ui, |ui| match self.profile_page {
                        SettingsPage::Profile => self.profile_cards(ui, connected),
                        SettingsPage::Discord => {
                            surface_card(ui, |ui| self.discord_account(ui));
                            if self.account_state.account.is_some() {
                                ui.add_space(10.0);
                                surface_card(ui, |ui| {
                                    ui.label(
                                        skin::strong(tr("Automatic joining"), 14.5).color(TEXT),
                                    );
                                    self.discord_following(ui);
                                });
                            }
                        }
                    });
                settings_divider(ui);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(tr("Advanced settings"))
                                .min_size(egui::vec2(0.0, 34.0))
                                .corner_radius(8),
                        )
                        .clicked()
                    {
                        self.advanced_open = true;
                        close = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(primary(tr("Done"), 88.0)).clicked() {
                            close = true;
                        }
                    });
                });
            });
        if close || modal.should_close() {
            self.profile_open = false;
        }
    }
}
