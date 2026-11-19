use super::*;

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
    pub(super) fn discord_controls(&mut self, ui: &mut egui::Ui) {
        ui.label(skin::strong(tr("Discord connection"), 16.0));
        if let Some(account) = &self.account_state.account {
            ui.label(format!("{}: {}", tr("Connected as"), account.display_name));
            ui.small(format!("Discord ID: {}", account.user_id));
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            ui.small(format!(
                "{}: {}",
                tr("Remembered login days remaining"),
                account.expires_at.saturating_sub(now) / 86_400_000
            ));
            if ui.button(tr("Disconnect Discord")).clicked() {
                self.account_client.sign_out();
            }
            let voice = self.following_client.state();
            if self.following.paused || self.following.independent {
                ui.label(tr("Automatic joining is paused."));
                if ui.button(tr("Resume automatic joining")).clicked() {
                    self.following.resume();
                }
            } else if !voice.known {
                ui.label(tr("Waiting for Discord voice information…"));
            } else if !voice.enabled {
                ui.label(tr("Automatic joining is disabled by the administrator."));
            } else if voice.locations.is_empty() {
                ui.label(tr("Waiting for your Discord voice channel…"));
            } else {
                for channel in &voice.locations {
                    ui.label(format!("{} / {}", channel.guild_name, channel.channel_name));
                    if voice.locations.len() > 1
                        && ui
                            .add_enabled(
                                channel.enabled,
                                egui::Button::new(format!(
                                    "{} {}",
                                    tr("Follow channel"),
                                    channel.channel_name
                                )),
                            )
                            .clicked()
                    {
                        self.following.selected = Some(channel.key());
                    }
                }
            }
        } else {
            ui.label(tr("Sign in with Discord or continue as a guest."));
            ui.label(tr("PodeTelar server"));
            ui.add_enabled(
                !self.account_state.busy,
                egui::TextEdit::singleline(&mut self.account_server),
            );
            if ui
                .add_enabled(
                    !self.account_state.busy
                        && crate::admission::valid_server(&self.account_server),
                    egui::Button::new(tr("Sign in with Discord")),
                )
                .clicked()
            {
                self.account_client
                    .sign_in(self.account_server.trim().to_owned());
            }
            if self.account_state.busy {
                ui.label(tr("Finish signing in in your browser."));
                if ui.button(tr("Cancel sign-in")).clicked() {
                    self.account_client.cancel();
                }
            } else if !self.account_state.available
                && self.account_state.server == self.account_server
            {
                ui.small(tr("Discord sign-in may need to be enabled on this server."));
            }
        }
        if !self.account_state.error.is_empty() {
            ui.colored_label(ERROR, tr(&self.account_state.error));
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
    pub(super) fn profile_dialog(&mut self, ctx: &egui::Context) {
        if !self.profile_open {
            return;
        }
        let mut close = false;
        egui::Modal::new(egui::Id::unique("user-settings")).show(ctx, |ui| {
            ui.set_width(390.0);
            egui::ScrollArea::vertical()
                .max_height(520.0)
                .show(ui, |ui| {
                    ui.heading(tr("User settings"));
                    ui.add_space(12.0);
                    self.discord_controls(ui);
                    settings_divider(ui);
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
                        .and_then(|p| self.profile_texture(ctx, &p.participant_id));
                    avatar(ui, &self.name, 64.0, texture);
                    let discord = own.as_ref().is_some_and(|p| p.discord);
                    if discord {
                        ui.label(tr("Your nickname and picture are managed by Discord."));
                    }
                    ui.label(tr("Guest nickname"));
                    let field = ui.add_enabled(
                        !discord,
                        egui::TextEdit::singleline(&mut self.profile_name).char_limit(60),
                    );
                    let busy = self.profile_state.busy;
                    if ui
                        .add_enabled(
                            !discord && !busy && crate::settings::valid_name(&self.profile_name),
                            egui::Button::new(tr("Save nickname")),
                        )
                        .clicked()
                        || (!discord
                            && !busy
                            && field.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && crate::settings::valid_name(&self.profile_name))
                    {
                        let name = self.profile_name.trim().to_owned();
                        if let Some(client) = &self.profile_client {
                            client.name(name);
                        } else if crate::settings::save_guest_name(&name).is_ok() {
                            self.remember_guest_name(name);
                        } else {
                            self.settings_error =
                                "Could not remember your guest name on this PC.".into();
                        }
                    }
                    ui.add_space(12.0);
                    ui.label(tr(
                        "Profile pictures: PNG, JPEG, WebP or animated GIF, up to 3 MB.",
                    ));
                    if !discord && own.as_ref().is_some_and(|p| p.uploads_blocked) {
                        ui.colored_label(
                            WARNING,
                            tr("Profile picture uploads are disabled for this PC."),
                        );
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !busy && own.as_ref().is_some_and(|p| !p.uploads_blocked),
                                egui::Button::new(tr("Upload picture")),
                            )
                            .clicked()
                            && let Some(client) = &self.profile_client
                        {
                            client.choose();
                        }
                        if ui
                            .add_enabled(
                                !discord
                                    && !busy
                                    && own.as_ref().is_some_and(|p| p.avatar_revision.is_some()),
                                egui::Button::new(tr("Remove picture")),
                            )
                            .clicked()
                            && let Some(client) = &self.profile_client
                        {
                            client.remove();
                        }
                    });
                    if self.profile_client.is_none() {
                        ui.label(tr("Join a room to manage your profile picture."));
                    }
                    if busy {
                        ui.spinner();
                    }
                    if !self.profile_state.error.is_empty() {
                        ui.colored_label(ERROR, tr(&self.profile_state.error));
                    }
                    settings_divider(ui);
                    if ui.button(tr("Advanced settings")).clicked() {
                        self.advanced_open = true;
                        close = true;
                    }
                    if ui.button(tr("Done")).clicked() {
                        close = true;
                    }
                });
        });
        if close {
            self.profile_open = false;
        }
    }
}
