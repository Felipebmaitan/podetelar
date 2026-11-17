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
            }
        } else {
            self.profile_client = None;
            self.profile_state = Default::default();
        }
        if let Some(client) = &self.profile_client {
            self.profile_state = client.state();
            if let Some(own) = self
                .profile_state
                .profiles
                .iter()
                .find(|p| p.participant_id == client.participant_id)
            {
                self.name = own.display_name.clone();
                self.session_name = Some(self.name.clone());
                if self.saved_guest_name != self.name {
                    if crate::settings::save_guest_name(&self.name).is_err() {
                        self.settings_error =
                            "Could not remember your guest name on this PC.".into();
                    }
                    self.saved_guest_name = self.name.clone();
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
            ui.heading(tr("User settings"));
            ui.add_space(12.0);
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
            ui.label(tr("Guest nickname"));
            let field = ui.add(egui::TextEdit::singleline(&mut self.profile_name).char_limit(60));
            let busy = self.profile_state.busy;
            if ui
                .add_enabled(
                    !busy && crate::settings::valid_name(&self.profile_name),
                    egui::Button::new(tr("Save nickname")),
                )
                .clicked()
                || (!busy
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
                    self.settings_error = "Could not remember your guest name on this PC.".into();
                }
            }
            ui.add_space(12.0);
            ui.label(tr(
                "Profile pictures: PNG, JPEG, WebP or animated GIF, up to 3 MB.",
            ));
            if own.as_ref().is_some_and(|p| p.uploads_blocked) {
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
                        !busy && own.as_ref().is_some_and(|p| p.avatar_revision.is_some()),
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
        if close {
            self.profile_open = false;
        }
    }
}
