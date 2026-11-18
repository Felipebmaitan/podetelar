use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingsPage {
    Profile,
    Discord,
}

fn settings_card(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    let width = ui.available_width();
    egui::Frame::new()
        .fill(CARD)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(10)
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width((width - 30.0).max(0.0));
            ui.label(skin::strong(tr(title), 16.0));
            ui.add_space(3.0);
            content(ui);
        });
}

fn description(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(tr(text)).size(13.0).color(MUTED));
}

fn joining_status(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(color, "●");
        ui.label(skin::strong(tr(text), 13.0).color(color));
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
        egui::ScrollArea::vertical()
            .id_salt("room-entry")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let width = ui.available_width().min(860.0);
                let offset = ((ui.available_width() - width) / 2.0).max(0.0);
                ui.add_space((ui.available_height() * 0.09).clamp(14.0, 52.0));
                ui.horizontal_top(|ui| {
                    ui.add_space(offset);
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.label(skin::strong(tr("Join a room"), 28.0));
                        ui.add_space(12.0);
                        let left = |this: &mut Self, ui: &mut egui::Ui| {
                            settings_card(ui, "Discord voice", |ui| {
                                if let Some(account) = &this.account_state.account {
                                    ui.label(skin::strong(&account.display_name, 14.0));
                                    this.discord_following(ui);
                                    if ui.button(tr("Manage Discord connection")).clicked() {
                                        this.open_profile();
                                        this.profile_page = SettingsPage::Discord;
                                    }
                                } else {
                                    this.discord_sign_in(ui);
                                }
                            });
                        };
                        let right = |this: &mut Self, ui: &mut egui::Ui| {
                            settings_card(ui, "Room invitation", |ui| {
                                description(ui, "Use a link from a friend to join independently.");
                                ui.add_space(5.0);
                                this.join_form(ui, state);
                            });
                        };
                        if width >= 630.0 {
                            ui.spacing_mut().item_spacing.x = 14.0;
                            ui.columns(2, |columns| {
                                left(self, &mut columns[0]);
                                right(self, &mut columns[1]);
                            });
                        } else {
                            left(self, ui);
                            ui.add_space(8.0);
                            right(self, ui);
                        }
                    });
                });
            });
    }

    fn discord_account(&mut self, ui: &mut egui::Ui) {
        if let Some(account) = &self.account_state.account {
            ui.horizontal(|ui| {
                avatar(ui, &account.display_name, 36.0, None);
                ui.vertical(|ui| {
                    ui.label(skin::strong(&account.display_name, 15.0));
                    description(ui, "Signed in with Discord");
                });
            });
            ui.add_space(4.0);
            let disconnect = ui.button(tr("Disconnect Discord")).on_hover_text(tr(
                "Sign out of this server and remove the remembered login from this device.",
            ));
            if disconnect.clicked() {
                self.account_client.sign_out();
            }
            egui::CollapsingHeader::new(tr("Account details"))
                .id_salt("discord-account-details")
                .show(ui, |ui| {
                    ui.label(format!(
                        "{}: {}",
                        tr("PodeTelar server"),
                        self.account_state.server
                    ));
                    ui.label(format!("Discord ID: {}", account.user_id));
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    ui.label(format!(
                        "{}: {}",
                        tr("Remembered login days remaining"),
                        account.expires_at.saturating_sub(now) / 86_400_000
                    ));
                });
            if !self.account_state.error.is_empty() {
                ui.colored_label(ERROR, tr(&self.account_state.error));
            }
        } else {
            self.discord_sign_in(ui);
        }
    }

    fn discord_sign_in(&mut self, ui: &mut egui::Ui) {
        description(ui, "Sign in to follow your Discord voice channel.");
        ui.add_space(5.0);
        if self.account_state.busy {
            ui.horizontal_wrapped(|ui| {
                ui.spinner();
                ui.label(tr("Finish signing in in your browser."));
            });
            if ui.button(tr("Cancel sign-in")).clicked() {
                self.account_client.cancel();
            }
        } else {
            let valid = crate::admission::valid_server(self.account_server.trim());
            if ui
                .add_enabled(
                    valid,
                    primary(tr("Sign in with Discord"), ui.available_width()),
                )
                .clicked()
            {
                self.account_client
                    .sign_in(self.account_server.trim().to_owned());
            }
            egui::CollapsingHeader::new(tr("Server settings"))
                .id_salt("discord-server-settings")
                .show(ui, |ui| {
                    let label = ui.label(tr("PodeTelar server"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.account_server)
                            .desired_width(f32::INFINITY),
                    )
                    .labelled_by(label.id);
                    if !valid {
                        ui.colored_label(
                            WARNING,
                            tr("Use an HTTPS server or a local test address."),
                        );
                    } else if !self.account_state.available
                        && self.account_state.server == self.account_server.trim()
                    {
                        description(ui, "Discord sign-in may need to be enabled on this server.");
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
            joining_status(ui, "Automatic joining paused", WARNING);
            if ui.button(tr("Resume automatic joining")).clicked() {
                self.following.resume();
            }
        } else if !voice.known {
            joining_status(ui, "Waiting for Discord voice information…", MUTED);
        } else if !voice.enabled {
            joining_status(ui, "Automatic joining disabled", WARNING);
            description(ui, "The server administrator has paused this feature.");
        } else if voice.locations.is_empty() {
            joining_status(ui, "Waiting for your Discord voice channel…", MUTED);
            description(
                ui,
                "Join a voice channel in Discord with the PodeTelar bot.",
            );
        } else {
            if voice.locations.len() > 1 {
                description(ui, "Choose which voice channel to follow.");
            }
            for channel in &voice.locations {
                let current = self
                    .following
                    .current
                    .as_ref()
                    .is_some_and(|location| location.key() == channel.key());
                ui.label(skin::strong(&channel.channel_name, 14.0));
                ui.label(
                    egui::RichText::new(&channel.guild_name)
                        .size(13.0)
                        .color(MUTED),
                );
                if !channel.enabled {
                    joining_status(ui, "Automatic joining disabled for this channel", WARNING);
                } else if current {
                    joining_status(ui, "Following", ACCENT);
                } else if voice.locations.len() > 1
                    && ui
                        .button(format!("{} {}", tr("Follow channel"), channel.channel_name))
                        .clicked()
                {
                    self.following.selected = Some(channel.key());
                }
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
                    && self.account_state.credential.is_none()
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
        let label = ui.label(tr("Guest nickname"));
        let busy = self.profile_state.busy;
        let field = ui
            .add_enabled(
                !busy,
                egui::TextEdit::singleline(&mut self.profile_name)
                    .char_limit(60)
                    .desired_width(f32::INFINITY),
            )
            .labelled_by(label.id);
        let valid = !busy && crate::settings::valid_name(&self.profile_name);
        if ui
            .add_enabled(valid, egui::Button::new(tr("Save nickname")))
            .clicked()
            || (valid && field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
        {
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

    fn profile_cards(&mut self, ui: &mut egui::Ui) {
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
        if own.as_ref().is_some_and(|p| p.discord) {
            settings_card(ui, "Room profile", |ui| {
                ui.horizontal(|ui| {
                    avatar(ui, &self.name, 48.0, texture);
                    ui.vertical(|ui| {
                        ui.label(skin::strong(&self.name, 15.0));
                        description(ui, "Your nickname and picture are managed by Discord.");
                    });
                });
                if ui.button(tr("Manage Discord connection")).clicked() {
                    self.profile_page = SettingsPage::Discord;
                }
            });
            return;
        }
        settings_card(ui, "Room profile", |ui| self.edit_nickname(ui));
        ui.add_space(8.0);
        settings_card(ui, "Profile picture", |ui| {
            ui.horizontal_top(|ui| {
                avatar(ui, &self.name, 48.0, texture);
                ui.vertical(|ui| {
                    if self.profile_client.is_none() {
                        description(ui, "Join a room to manage your profile picture.");
                    } else {
                        description(ui, "PNG, JPEG, WebP or animated GIF, up to 3 MB.");
                    }
                    if own.as_ref().is_some_and(|p| p.uploads_blocked) {
                        ui.colored_label(
                            WARNING,
                            tr("Profile picture uploads are disabled for this PC."),
                        );
                    }
                    if let Some(client) = &self.profile_client {
                        ui.horizontal_wrapped(|ui| {
                            let busy = self.profile_state.busy;
                            if ui
                                .add_enabled(
                                    !busy && own.as_ref().is_some_and(|p| !p.uploads_blocked),
                                    egui::Button::new(tr("Upload picture")),
                                )
                                .clicked()
                            {
                                client.choose();
                            }
                            if own.as_ref().is_some_and(|p| p.avatar_revision.is_some())
                                && ui
                                    .add_enabled(!busy, egui::Button::new(tr("Remove picture")))
                                    .clicked()
                            {
                                client.remove();
                            }
                        });
                    }
                });
            });
        });
        if self.profile_state.busy {
            ui.spinner();
        }
        if !self.profile_state.error.is_empty() {
            ui.colored_label(ERROR, tr(&self.profile_state.error));
        }
    }

    pub(super) fn profile_dialog(&mut self, ctx: &egui::Context) {
        if !self.profile_open {
            return;
        }
        let width = (ctx.content_rect().width() - 64.0).clamp(300.0, 490.0);
        let max_body_height = (ctx.content_rect().height() - 145.0).clamp(130.0, 460.0);
        let mut close = false;
        egui::Modal::new(egui::Id::unique("user-settings"))
            .frame(skin::menu_frame().inner_margin(egui::Margin::same(16)))
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.heading(tr("User settings"));
                ui.add_space(8.0);
                ui.columns(2, |columns| {
                    let pages = [
                        (SettingsPage::Profile, "Profile"),
                        (SettingsPage::Discord, "Discord"),
                    ];
                    for (column, (page, label)) in columns.iter_mut().zip(pages) {
                        let selected = self.profile_page == page;
                        let response = column.add_sized(
                            [column.available_width(), 34.0],
                            egui::Button::new(skin::strong(tr(label), 13.0).color(if selected {
                                ACCENT
                            } else {
                                MUTED
                            }))
                            .fill(if selected {
                                egui::Color32::from_rgb(34, 55, 53)
                            } else {
                                PANEL
                            })
                            .stroke(egui::Stroke::new(
                                1.0,
                                if selected { ACCENT } else { BORDER },
                            )),
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::Tab,
                                column.is_enabled(),
                                selected,
                                tr(label),
                            )
                        });
                        if response.clicked() {
                            self.profile_page = page;
                        }
                    }
                });
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .id_salt(match self.profile_page {
                        SettingsPage::Profile => "profile",
                        SettingsPage::Discord => "discord",
                    })
                    .max_height(max_body_height)
                    .show(ui, |ui| match self.profile_page {
                        SettingsPage::Profile => self.profile_cards(ui),
                        SettingsPage::Discord => {
                            settings_card(ui, "Discord account", |ui| self.discord_account(ui));
                            if self.account_state.account.is_some() {
                                ui.add_space(8.0);
                                settings_card(ui, "Automatic joining", |ui| {
                                    self.discord_following(ui)
                                });
                            }
                        }
                    });
                settings_divider(ui);
                ui.horizontal(|ui| {
                    if ui.button(tr("Advanced settings")).clicked() {
                        self.advanced_open = true;
                        close = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(primary(tr("Done"), 80.0)).clicked() {
                            close = true;
                        }
                    });
                });
            });
        if close {
            self.profile_open = false;
        }
    }
}
