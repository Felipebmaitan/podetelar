//! Window structure, "Palco": the stage is the whole window. The only chrome is a
//! floating capsule at the top (room and people), small corner actions, and a floating dock.
use super::*;

const ISLAND_W: f32 = 392.0;

fn ease(ctx: &egui::Context, id: egui::Id, on: bool, seconds: f32) -> f32 {
    ctx.animate_bool_with_time_and_easing(id, on, seconds, egui::emath::easing::cubic_out)
}

/// A soft, layered glow (egui has no blur): decoration only.
pub(super) fn glow(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    tint: egui::Color32,
    strength: f32,
) {
    // A true radial gradient: rings of vertices whose alpha eases to nothing, so there is
    // no visible edge and no banding.
    const SEGMENTS: usize = 72;
    const RINGS: usize = 7;
    let peak = (strength * 14.0).clamp(0.0, 0.6);
    let mut mesh = egui::Mesh::default();
    for ring in 0..=RINGS {
        let t = ring as f32 / RINGS as f32;
        let alpha = peak * (1.0 - t).powf(2.2);
        let color = egui::Color32::from_rgba_unmultiplied(
            tint.r(),
            tint.g(),
            tint.b(),
            (alpha * 255.0) as u8,
        );
        for k in 0..SEGMENTS {
            let angle = k as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let point = center + egui::vec2(angle.cos(), angle.sin()) * radius * t;
            mesh.colored_vertex(point, color);
        }
    }
    for ring in 0..RINGS {
        for k in 0..SEGMENTS {
            let next = (k + 1) % SEGMENTS;
            let a = (ring * SEGMENTS + k) as u32;
            let b = (ring * SEGMENTS + next) as u32;
            let c = ((ring + 1) * SEGMENTS + k) as u32;
            let d = ((ring + 1) * SEGMENTS + next) as u32;
            mesh.add_triangle(a, b, c);
            mesh.add_triangle(b, d, c);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

impl PublisherUi {
    /// Corner actions and the floating capsule. Nothing here takes layout space.
    pub(super) fn chrome(&mut self, ctx: &egui::Context, state: &Snapshot, stage: egui::Rect) {
        let p = pal();
        let connected = matches!(state.connection, "connected" | "reconnecting");
        // Brand, top left.
        egui::Area::new(egui::Id::NULL.with("chrome-brand"))
            .order(egui::Order::Foreground)
            .fixed_pos(stage.left_top() + egui::vec2(20.0, 16.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    brand_mark_sized(ui, 38.0);
                    ui.add_space(6.0);
                    ui.label(skin::strong("PodeTelar", 17.0));
                });
            });
        // Actions, top right.
        egui::Area::new(egui::Id::NULL.with("chrome-actions"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::RIGHT_TOP)
            .fixed_pos(stage.right_top() + egui::vec2(-20.0, 20.0))
            .show(ctx, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(430.0, 40.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        self.room_actions(ui, state);
                        if connected && !self.invitation.is_empty() {
                            let copied = self
                                .copied_until
                                .is_some_and(|until| Instant::now() < until);
                            if ui
                                .add(
                                    egui::Button::new(
                                        skin::strong(
                                            if copied { tr("Copied!") } else { tr("Invite") },
                                            13.0,
                                        )
                                        .color(p.on_accent),
                                    )
                                    .fill(p.accent)
                                    .corner_radius(16)
                                    .min_size(egui::vec2(88.0, 38.0)),
                                )
                                .clicked()
                            {
                                ui.ctx().copy_text(self.invitation.clone());
                                self.copied_until = Some(Instant::now() + Duration::from_secs(2));
                            }
                        }
                        if connected && !state.tiles.is_empty() {
                            let watched = state.tiles.iter().find(|tile| tile.watching);
                            ui.add_enabled_ui(watched.is_some(), |ui| {
                                if skin::layout_button(
                                    ui,
                                    Icon::Focus,
                                    tr("Focus stream"),
                                    self.focused.is_some(),
                                )
                                .clicked()
                                {
                                    self.focused = watched.map(|tile| tile.id.clone());
                                }
                            });
                            ui.add_enabled_ui(state.tiles.len() > 1, |ui| {
                                if skin::layout_button(
                                    ui,
                                    Icon::Spotlight,
                                    tr("Spotlight"),
                                    self.focused.is_none() && self.view_mode == ViewMode::Spotlight,
                                )
                                .clicked()
                                {
                                    self.view_mode = ViewMode::Spotlight;
                                    self.focused = None;
                                }
                            });
                            if skin::layout_button(
                                ui,
                                Icon::Grid,
                                tr("Back to grid"),
                                self.focused.is_none() && self.view_mode == ViewMode::Grid,
                            )
                            .clicked()
                            {
                                self.view_mode = ViewMode::Grid;
                                self.focused = None;
                            }
                        }
                    },
                );
            });
        if connected {
            self.island(ctx, state, stage);
        }
    }

    /// The capsule: room name and people when closed; streams, people and shortcuts when open.
    fn island(&mut self, ctx: &egui::Context, state: &Snapshot, stage: egui::Rect) {
        let p = pal();
        let open = ease(
            ctx,
            egui::Id::NULL.with("island-open"),
            self.island_open,
            0.32,
        );
        let name = if state.room_name.is_empty() {
            tr("Your private room")
        } else if state.room_name == "Friends" {
            tr("Friends")
        } else {
            &state.room_name
        };
        let response = egui::Area::new(egui::Id::NULL.with("island"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::CENTER_TOP)
            .fixed_pos(egui::pos2(stage.center().x, stage.top() + 18.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(p.card)
                    .stroke(egui::Stroke::new(1.0, p.border))
                    .corner_radius((26.0 - 6.0 * open) as u8)
                    .inner_margin(egui::Margin::symmetric(16, 8))
                    .shadow(skin::menu_frame().shadow)
                    .show(ui, |ui| {
                        ui.set_width(ISLAND_W - 32.0 + (open * 0.0));
                        let head = ui
                            .horizontal(|ui| {
                                let connected = state.connection == "connected";
                                let (dot, _) = ui.allocate_exact_size(
                                    egui::vec2(14.0, 28.0),
                                    egui::Sense::hover(),
                                );
                                let time = ui.input(|i| i.time) as f32;
                                let pulse = (time * 2.2).sin() * 0.5 + 0.5;
                                let tint = if connected { p.success } else { p.warning };
                                ui.painter().circle_filled(
                                    dot.center(),
                                    4.0 + 3.0 * pulse,
                                    tint.gamma_multiply(0.3 * (1.0 - pulse)),
                                );
                                ui.painter().circle_filled(dot.center(), 4.0, tint);
                                ctx.request_repaint_after(Duration::from_millis(60));
                                ui.add(egui::Label::new(skin::strong(name, 14.0)).truncate());
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        // Chevron that turns as the capsule opens.
                                        let (chev, _) = ui.allocate_exact_size(
                                            egui::vec2(18.0, 18.0),
                                            egui::Sense::hover(),
                                        );
                                        let c = chev.center();
                                        let dy = 2.5 * (1.0 - 2.0 * open);
                                        ui.painter().add(egui::Shape::line(
                                            vec![
                                                c + egui::vec2(-5.0, -dy),
                                                c + egui::vec2(0.0, dy),
                                                c + egui::vec2(5.0, -dy),
                                            ],
                                            egui::Stroke::new(1.8, p.muted),
                                        ));
                                        ui.add_space(4.0);
                                        // Overlapping avatars.
                                        let shown = 1 + state.participants.len().min(3);
                                        let width =
                                            shown as f32 * 26.0 - (shown as f32 - 1.0) * 8.0;
                                        let (rect, _) = ui.allocate_exact_size(
                                            egui::vec2(width, 28.0),
                                            egui::Sense::hover(),
                                        );
                                        let mut row = ui.new_child(
                                            egui::UiBuilder::new().max_rect(rect).layout(
                                                egui::Layout::left_to_right(egui::Align::Center),
                                            ),
                                        );
                                        row.spacing_mut().item_spacing.x = -8.0;
                                        let own_id = self
                                            .profile_client
                                            .as_ref()
                                            .map(|c| c.participant_id.clone())
                                            .unwrap_or_default();
                                        let texture = self.profile_texture(ui.ctx(), &own_id);
                                        profile::avatar(&mut row, &self.name, 26.0, texture);
                                        for (index, who) in
                                            state.participants.iter().take(3).enumerate()
                                        {
                                            let id = state
                                                .participant_ids
                                                .get(index)
                                                .map(|(id, _)| id.as_str())
                                                .unwrap_or("");
                                            let texture = self.profile_texture(ui.ctx(), id);
                                            profile::avatar(&mut row, who, 26.0, texture);
                                        }
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{}",
                                                state.participants.len() + 1
                                            ))
                                            .size(12.0)
                                            .color(p.muted),
                                        );
                                    },
                                );
                            })
                            .response;
                        if open > 0.01 {
                            ui.scope(|ui| {
                                ui.set_opacity(open);
                                ui.set_max_height(open * 380.0);
                                ui.shrink_clip_rect(ui.max_rect());
                                ui.add_space(8.0);
                                ui.separator();
                                self.island_body(ui, state);
                            });
                        }
                        head
                    })
                    .inner
            });
        let head = response.inner;
        let toggle = ctx.input(|i| i.pointer.any_click())
            && ctx
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|pos| head.rect.expand2(egui::vec2(16.0, 8.0)).contains(pos));
        if toggle {
            self.island_open = !self.island_open;
        } else if self.island_open
            && ctx.input(|i| i.pointer.any_pressed())
            && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
            && !response.response.rect.contains(pos)
        {
            self.island_open = false;
        }
    }

    fn island_body(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let p = pal();
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.label(
            egui::RichText::new(format!(
                "{}  {}",
                tr("Live now").to_uppercase(),
                state.tiles.len()
            ))
            .font(skin::semibold(11.0))
            .color(p.muted),
        );
        if state.tiles.is_empty() {
            ui.label(
                egui::RichText::new(tr("Nobody is live yet"))
                    .size(12.0)
                    .color(p.muted),
            );
        }
        for tile in &state.tiles {
            self.stream_row(ui, tile);
        }
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!(
                "{}  {}",
                tr("In room").to_uppercase(),
                state.participants.len() + 1
            ))
            .font(skin::semibold(11.0))
            .color(p.muted),
        );
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
            .any(|q| q.participant_id == own_id && q.discord);
        if participant(ui, &self.name, true, state.sharing, texture, discord).clicked() {
            self.open_profile();
        }
        for (index, who) in state.participants.iter().enumerate() {
            let id = state
                .participant_ids
                .get(index)
                .map(|(id, _)| id.as_str())
                .unwrap_or("");
            let texture = self.profile_texture(ui.ctx(), id);
            let discord = self
                .profile_state
                .profiles
                .iter()
                .any(|q| q.participant_id == id && q.discord);
            participant(
                ui,
                who,
                false,
                state.tiles.iter().any(|tile| tile.id == id),
                texture,
                discord,
            );
        }
        ui.add_space(6.0);
        if ui
            .add(
                egui::Button::new(skin::strong(tr("User settings"), 13.0))
                    .fill(p.control)
                    .corner_radius(14)
                    .min_size(egui::vec2(ui.available_width(), 38.0)),
            )
            .clicked()
        {
            self.island_open = false;
            self.open_profile();
        }
    }

    fn stream_row(&mut self, ui: &mut egui::Ui, tile: &Tile) {
        let p = pal();
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 52.0), egui::Sense::click());
        let hover = ease(
            ui.ctx(),
            response.id.with("hover"),
            response.hovered(),
            0.14,
        );
        let focused = self.focused.as_ref() == Some(&tile.id);
        ui.painter()
            .rect_filled(rect, 16.0, p.control.gamma_multiply(0.3 + 0.7 * hover));
        if focused {
            ui.painter().rect_stroke(
                rect,
                16.0,
                egui::Stroke::new(1.5, p.accent),
                egui::StrokeKind::Inside,
            );
        }
        let mut row = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect.shrink2(egui::vec2(10.0, 0.0)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let texture = self.profile_texture(ui.ctx(), &tile.id);
        profile::avatar(&mut row, &tile.name, 34.0, texture);
        row.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.add(egui::Label::new(skin::strong(&tile.name, 13.0)).truncate());
            ui.label(
                egui::RichText::new(format!(
                    "{} · {}",
                    tile.codec,
                    tr(if tile.watching { "Watching" } else { "Preview" })
                ))
                .size(11.0)
                .color(p.muted),
            );
        });
        let badge = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 24.0, rect.center().y),
            egui::vec2(30.0, 30.0),
        );
        ui.painter().circle_filled(
            badge.center(),
            15.0,
            if tile.watching {
                p.success.gamma_multiply(0.2)
            } else {
                p.accent.gamma_multiply(0.18 + 0.6 * hover)
            },
        );
        skin::icon(
            ui.painter(),
            badge.shrink(6.0),
            if tile.watching {
                Icon::Focus
            } else {
                Icon::Play
            },
            if tile.watching {
                p.success
            } else {
                p.accent.lerp_to_gamma(p.on_accent, hover)
            },
        );
        let label = if tile.watching {
            tr("Focus stream")
        } else {
            tr("Watch")
        };
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        if response.on_hover_text(label).clicked() {
            if tile.watching {
                self.focused = Some(tile.id.clone());
                self.island_open = false;
            } else {
                self.controller.send(Command::Watch {
                    id: tile.id.clone(),
                    enabled: true,
                });
            }
        }
    }

    /// Errors and progress, as a floating notice under the capsule instead of a bar.
    pub(super) fn toast(&mut self, ctx: &egui::Context, state: &Snapshot, stage: egui::Rect) {
        let p = pal();
        let reconnecting = state.connection == "reconnecting";
        let any = !state.error.is_empty()
            || !self.settings_error.is_empty()
            || state.busy
            || reconnecting;
        let t = ease(ctx, egui::Id::NULL.with("toast"), any, 0.28);
        if t < 0.01 {
            return;
        }
        egui::Area::new(egui::Id::NULL.with("toast-area"))
            .order(egui::Order::Tooltip)
            .pivot(egui::Align2::CENTER_TOP)
            .fixed_pos(egui::pos2(
                stage.center().x,
                stage.top() + 84.0 - (1.0 - t) * 12.0,
            ))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_opacity(t);
                ui.spacing_mut().item_spacing.y = 8.0;
                let notice = |ui: &mut egui::Ui, tint: egui::Color32, text: String, busy: bool| {
                    egui::Frame::new()
                        .fill(p.card)
                        .stroke(egui::Stroke::new(1.0, tint.gamma_multiply(0.6)))
                        .corner_radius(20)
                        .inner_margin(egui::Margin::symmetric(16, 10))
                        .shadow(skin::menu_frame().shadow)
                        .show(ui, |ui| {
                            ui.set_max_width(520.0);
                            ui.horizontal(|ui| {
                                if busy {
                                    ui.spinner();
                                } else {
                                    let (dot, _) = ui.allocate_exact_size(
                                        egui::vec2(12.0, 16.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().circle_filled(dot.center(), 4.0, tint);
                                }
                                ui.add(
                                    egui::Label::new(egui::RichText::new(text).color(if busy {
                                        p.text
                                    } else {
                                        tint
                                    }))
                                    .wrap(),
                                );
                            });
                        });
                };
                if !state.error.is_empty() {
                    notice(ui, p.error, tr(&state.error).to_owned(), false);
                }
                if !self.settings_error.is_empty() {
                    notice(ui, p.error, tr(&self.settings_error).to_owned(), false);
                }
                if state.busy {
                    notice(ui, p.accent, tr(state.operation).to_owned(), true);
                } else if reconnecting {
                    notice(
                        ui,
                        p.warning,
                        tr("Reconnecting to your room. Your watching choices will be restored.")
                            .to_owned(),
                        true,
                    );
                }
            });
    }

    /// Soft ambient glows drifting behind the first screen.
    pub(super) fn orbs(&self, ui: &mut egui::Ui) {
        let p = pal();
        let area = ui.max_rect();
        let time = ui.input(|i| i.time) as f32;
        ui.ctx().request_repaint_after(Duration::from_millis(16));
        // Three slow, soft lights: they never move far and never reach the content.
        for (k, (fx, fy, radius, tint, strength)) in [
            (0.12, 0.10, 520.0, p.accent, 0.0085),
            (0.92, 0.88, 460.0, p.spark, 0.0050),
            (0.62, 0.30, 360.0, p.accent, 0.0035),
        ]
        .into_iter()
        .enumerate()
        {
            let drift = egui::vec2(
                (time * 0.18 + k as f32 * 2.1).sin() * 34.0,
                (time * 0.15 + k as f32 * 1.3).cos() * 26.0,
            );
            let center = area.min + egui::vec2(area.width() * fx, area.height() * fy) + drift;
            glow(ui.painter(), center, radius, tint, strength);
        }
    }

    /// Controls that matter while a room is open, floating over the stage.
    pub(super) fn dock(&mut self, ctx: &egui::Context, state: &Snapshot, stage: egui::Rect) {
        if !matches!(state.connection, "connected" | "reconnecting") || self.fullscreen.is_some() {
            return;
        }
        let p = pal();
        let live = state.sharing || state.capture_pending;
        let rise = ease(
            ctx,
            egui::Id::NULL.with("dock-rise"),
            ctx.cumulative_frame_nr() > 3,
            0.5,
        );
        egui::Area::new(egui::Id::NULL.with("room-dock"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::CENTER_BOTTOM)
            .fixed_pos(egui::pos2(
                stage.center().x,
                stage.bottom() - 20.0 + (1.0 - rise) * 24.0,
            ))
            .show(ctx, |ui| {
                ui.set_opacity(rise);
                egui::Frame::new()
                    .fill(p.card)
                    .stroke(egui::Stroke::new(1.0, p.border))
                    .corner_radius(28)
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .shadow(skin::menu_frame().shadow)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui.horizontal(|ui| {
                            if live {
                                let time = ui.input(|i| i.time) as f32;
                                let pulse = (time * 2.4).sin() * 0.5 + 0.5;
                                ctx.request_repaint_after(Duration::from_millis(50));
                                let (dot, _) = ui.allocate_exact_size(
                                    egui::vec2(16.0, 16.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().circle_filled(
                                    dot.center(),
                                    4.5 + 3.5 * pulse,
                                    p.danger.gamma_multiply(0.3 * (1.0 - pulse)),
                                );
                                ui.painter().circle_filled(dot.center(), 4.5, p.danger);
                                let summary = state
                                    .mode
                                    .map(|m| {
                                        format!(
                                            "{}p · {} FPS · {:.1} Mbps",
                                            m.height,
                                            m.fps,
                                            state.delivery.bitrate_bps as f64 / 1_000_000.0
                                        )
                                    })
                                    .unwrap_or_else(|| tr("Starting your stream…").to_owned());
                                ui.label(skin::strong(tr("Live"), 13.0).color(p.danger));
                                ui.label(egui::RichText::new(summary).size(12.0).color(p.muted));
                                if ui
                                    .add_sized(
                                        [96.0, 40.0],
                                        egui::Button::new(
                                            skin::strong(tr("Stop"), 13.0).color(p.error),
                                        )
                                        .fill(p.danger_bg)
                                        .corner_radius(18),
                                    )
                                    .on_hover_text(tr("Stop sharing"))
                                    .clicked()
                                {
                                    self.controller.send(Command::Stop);
                                }
                                if skin::icon_button(ui, Icon::Monitor, tr("Stream settings"))
                                    .clicked()
                                {
                                    self.go_live = Some(false);
                                }
                            } else {
                                let button = ui.add_sized(
                                    [212.0, 44.0],
                                    egui::Button::new(egui::Atoms::new((
                                        egui::Atom::grow(),
                                        egui::Atom::paint(egui::vec2(18.0, 18.0), |ui, args| {
                                            skin::icon(
                                                ui.painter(),
                                                args.rect,
                                                Icon::Monitor,
                                                pal().on_accent,
                                            )
                                        }),
                                        skin::strong(tr("Share your screen"), 14.0)
                                            .color(p.on_accent),
                                        egui::Atom::grow(),
                                    )))
                                    .fill(p.accent)
                                    .corner_radius(20),
                                );
                                if button.clicked() {
                                    self.go_live = Some(true);
                                }
                            }
                            ui.add_space(2.0);
                            let (line, _) =
                                ui.allocate_exact_size(egui::vec2(1.0, 24.0), egui::Sense::hover());
                            ui.painter().rect_filled(line, 0.0, p.border);
                            if skin::icon_button(ui, Icon::Settings, tr("Advanced settings"))
                                .clicked()
                            {
                                self.advanced_open = true;
                            }
                        });
                    });
            });
    }

    /// Source, quality and start/stop, opened from the dock instead of a permanent sidebar.
    pub(super) fn go_live_modal(&mut self, ctx: &egui::Context, state: &Snapshot) {
        let Some(from_idle) = self.go_live else {
            return;
        };
        if self.advanced_open || self.browser_invitation_open {
            return;
        }
        if from_idle && (state.sharing || state.capture_pending) {
            self.go_live = None;
            self.clear_picker();
            return;
        }
        let width = (ctx.viewport_rect().width() - 96.0).clamp(300.0, 460.0);
        let mut done = false;
        let modal = egui::Modal::new(egui::Id::unique("go-live"))
            .backdrop_color(egui::Color32::from_black_alpha(150))
            .frame(
                egui::Frame::new()
                    .fill(pal().card)
                    .stroke(egui::Stroke::new(1.0, pal().border))
                    .corner_radius(28)
                    .inner_margin(24),
            )
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 10.0;
                ui.horizontal(|ui| {
                    ui.label(skin::strong(tr("Share your screen"), 22.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if skin::icon_button(ui, Icon::Close, tr("Close")).clicked() {
                            done = true;
                        }
                    });
                });
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .id_salt("go-live-scroll")
                    .max_height((ctx.viewport_rect().height() - 260.0).clamp(160.0, 520.0))
                    .show(ui, |ui| self.sharing_menu(ui, state));
                self.sharing_actions(ui, state);
            });
        if done || modal.should_close() {
            self.go_live = None;
            self.clear_picker();
        }
    }
}
