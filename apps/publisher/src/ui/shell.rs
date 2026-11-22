//! Window structure: icon rail, room panel, floating control dock and join card.
use super::*;

const RAIL_WIDTH: f32 = 76.0;
const PANEL_WIDTH: f32 = 272.0;

fn ease(ctx: &egui::Context, id: egui::Id, on: bool, seconds: f32) -> f32 {
    ctx.animate_bool_with_time_and_easing(id, on, seconds, egui::emath::easing::cubic_out)
}

/// A rail button that morphs from a circle to a squircle on hover, with the
/// selection pill on the rail's edge.
fn rail_button(
    ui: &mut egui::Ui,
    selected: bool,
    label: &str,
    paint: impl FnOnce(&egui::Painter, egui::Rect, egui::Color32),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::click());
    let hover = ease(
        ui.ctx(),
        response.id.with("hover"),
        response.hovered() || response.has_focus(),
        0.16,
    );
    let picked = ease(ui.ctx(), response.id.with("picked"), selected, 0.24);
    let k = hover.max(picked);
    let p = pal();
    ui.painter().rect_filled(
        rect.translate(egui::vec2(0.0, 1.0 * hover)),
        24.0 - 9.0 * k,
        p.control.lerp_to_gamma(p.accent, k),
    );
    let pill = 8.0 + 24.0 * picked + 6.0 * hover * (1.0 - picked);
    ui.painter().rect_filled(
        egui::Rect::from_center_size(
            egui::pos2(rect.left() - 14.0, rect.center().y),
            egui::vec2(4.0, pill),
        ),
        2.0,
        p.text.gamma_multiply(k.max(0.0)),
    );
    paint(ui.painter(), rect, p.text.lerp_to_gamma(p.on_accent, k));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    response.on_hover_text(label)
}

fn section_label(ui: &mut egui::Ui, text: &str, count: usize) {
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new(format!("{}  {}", text.to_uppercase(), count))
            .font(skin::semibold(11.0))
            .color(pal().muted),
    );
    ui.add_space(2.0);
}

impl PublisherUi {
    /// Narrow strip: brand, current room, quick appearance and settings.
    pub(super) fn rail(&mut self, root: &mut egui::Ui, state: &Snapshot) {
        let connected = matches!(state.connection, "connected" | "reconnecting");
        egui::Panel::left("rail")
            .exact_size(RAIL_WIDTH)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(pal().canvas)
                    .inner_margin(egui::Margin::symmetric(14, 14)),
            )
            .show(root, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                ui.vertical_centered(|ui| {
                    ui.add_space(2.0);
                    brand_mark_sized(ui, 44.0);
                    ui.add_space(4.0);
                    let (line, _) =
                        ui.allocate_exact_size(egui::vec2(28.0, 2.0), egui::Sense::hover());
                    ui.painter().rect_filled(line, 1.0, pal().border);
                    ui.add_space(2.0);
                    let name = if state.room_name.is_empty() {
                        tr("Your private room").to_owned()
                    } else if state.room_name == "Friends" {
                        tr("Friends").to_owned()
                    } else {
                        state.room_name.clone()
                    };
                    if connected {
                        let initial = name
                            .chars()
                            .next()
                            .unwrap_or('P')
                            .to_uppercase()
                            .to_string();
                        rail_button(ui, true, &name, |painter, rect, color| {
                            painter.text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                initial,
                                skin::semibold(20.0),
                                color,
                            );
                        });
                    } else {
                        rail_button(ui, true, tr("Join your room"), |painter, rect, color| {
                            skin::icon(
                                painter,
                                egui::Rect::from_center_size(rect.center(), egui::vec2(24.0, 24.0)),
                                Icon::Invite,
                                color,
                            );
                        });
                    }
                });
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    if rail_button(ui, false, tr("User settings"), |painter, rect, color| {
                        skin::icon(
                            painter,
                            egui::Rect::from_center_size(rect.center(), egui::vec2(24.0, 24.0)),
                            Icon::Settings,
                            color,
                        );
                    })
                    .clicked()
                    {
                        self.open_profile();
                    }
                });
            });
    }

    /// Room name, who is live, who is here and your own card.
    pub(super) fn room_panel(&mut self, root: &mut egui::Ui, state: &Snapshot) {
        let p = pal();
        egui::Panel::left("room-panel")
            .exact_size(PANEL_WIDTH)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(p.panel)
                    .inner_margin(egui::Margin::symmetric(14, 14)),
            )
            .show(root, |ui| {
                let name = if state.room_name.is_empty() {
                    tr("Your private room")
                } else if state.room_name == "Friends" {
                    tr("Friends")
                } else {
                    &state.room_name
                };
                ui.add(egui::Label::new(skin::strong(name, 20.0)).truncate());
                ui.horizontal(|ui| {
                    let connected = state.connection == "connected";
                    let (dot, _) =
                        ui.allocate_exact_size(egui::vec2(10.0, 14.0), egui::Sense::hover());
                    let time = ui.input(|i| i.time) as f32;
                    let pulse = if connected {
                        (time * 2.0).sin() * 0.5 + 0.5
                    } else {
                        0.0
                    };
                    let tint = if connected { p.success } else { p.warning };
                    ui.painter().circle_filled(
                        dot.center(),
                        3.5 + 3.0 * pulse,
                        tint.gamma_multiply(0.25 * (1.0 - pulse) + 0.05),
                    );
                    ui.painter().circle_filled(dot.center(), 3.5, tint);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {} {}",
                            tr(if connected {
                                "Connected"
                            } else {
                                "Reconnecting"
                            }),
                            state.participants.len() + 1,
                            tr("in room")
                        ))
                        .size(12.0)
                        .color(p.muted),
                    );
                });
                ui.ctx().request_repaint_after(Duration::from_millis(60));
                ui.add_space(6.0);
                ui.separator();
                let footer = 70.0;
                egui::ScrollArea::vertical()
                    .id_salt("room-panel-scroll")
                    .max_height((ui.available_height() - footer).max(80.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        section_label(ui, tr("Live now"), state.tiles.len());
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
                        section_label(ui, tr("In room"), state.participants.len() + 1);
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
                        if participant(ui, &self.name, true, state.sharing, texture, discord)
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
                            let discord = self
                                .profile_state
                                .profiles
                                .iter()
                                .any(|p| p.participant_id == id && p.discord);
                            participant(
                                ui,
                                name,
                                false,
                                state.tiles.iter().any(|tile| tile.id == id),
                                texture,
                                discord,
                            );
                        }
                    });
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    egui::Frame::new()
                        .fill(p.control)
                        .corner_radius(16)
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let own_id = self
                                    .profile_client
                                    .as_ref()
                                    .map(|c| c.participant_id.clone())
                                    .unwrap_or_default();
                                let texture = self.profile_texture(ui.ctx(), &own_id);
                                profile::avatar(ui, &self.name, 34.0, texture);
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    if ui
                                        .add(
                                            egui::Label::new(skin::strong(&self.name, 14.0))
                                                .truncate()
                                                .sense(egui::Sense::click()),
                                        )
                                        .on_hover_text(tr("User settings"))
                                        .clicked()
                                    {
                                        self.open_profile();
                                    }
                                    ui.label(
                                        egui::RichText::new(tr(if state.sharing {
                                            "Sharing"
                                        } else {
                                            "Not sharing"
                                        }))
                                        .size(12.0)
                                        .color(if state.sharing { p.accent } else { p.muted }),
                                    );
                                });
                            });
                        });
                });
            });
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
            .rect_filled(rect, 14.0, p.control.gamma_multiply(0.15 + 0.85 * hover));
        if focused {
            ui.painter().rect_stroke(
                rect,
                14.0,
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
            egui::pos2(rect.right() - 22.0, rect.center().y),
            egui::vec2(28.0, 28.0),
        );
        ui.painter().circle_filled(
            badge.center(),
            14.0,
            if tile.watching {
                p.success.gamma_multiply(0.2)
            } else {
                p.accent.gamma_multiply(0.15 + 0.6 * hover)
            },
        );
        skin::icon(
            ui.painter(),
            badge.shrink(5.0),
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
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Button,
                true,
                if tile.watching {
                    tr("Focus stream")
                } else {
                    tr("Watch")
                },
            )
        });
        if response
            .on_hover_text(if tile.watching {
                tr("Focus stream")
            } else {
                tr("Watch")
            })
            .clicked()
        {
            if tile.watching {
                self.focused = Some(tile.id.clone());
            } else {
                self.controller.send(Command::Watch {
                    id: tile.id.clone(),
                    enabled: true,
                });
            }
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
                stage.bottom() - 18.0 + (1.0 - rise) * 24.0,
            ))
            .show(ctx, |ui| {
                ui.set_opacity(rise);
                egui::Frame::new()
                    .fill(p.card)
                    .stroke(egui::Stroke::new(1.0, p.border))
                    .corner_radius(22)
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
                                    egui::vec2(14.0, 14.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().circle_filled(
                                    dot.center(),
                                    4.0 + 3.0 * pulse,
                                    p.danger.gamma_multiply(0.3 * (1.0 - pulse)),
                                );
                                ui.painter().circle_filled(dot.center(), 4.0, p.danger);
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
                                ui.label(
                                    skin::strong(format!("{}  ", tr("Live")), 13.0).color(p.danger),
                                );
                                ui.label(egui::RichText::new(summary).size(12.0).color(p.muted));
                                if ui
                                    .add_sized(
                                        [96.0, 38.0],
                                        egui::Button::new(
                                            skin::strong(tr("Stop"), 13.0).color(p.error),
                                        )
                                        .fill(p.danger_bg)
                                        .corner_radius(14),
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
                                    [188.0, 40.0],
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
                                    .corner_radius(15),
                                );
                                if button.clicked() {
                                    self.go_live = Some(true);
                                }
                            }
                            ui.add_space(2.0);
                            let (line, _) =
                                ui.allocate_exact_size(egui::vec2(1.0, 24.0), egui::Sense::hover());
                            ui.painter().rect_filled(line, 0.0, p.border);
                            let copied = self
                                .copied_until
                                .is_some_and(|until| Instant::now() < until);
                            if skin::icon_button(
                                ui,
                                Icon::Invite,
                                if copied { tr("Copied!") } else { tr("Invite") },
                            )
                            .clicked()
                            {
                                ui.ctx().copy_text(self.invitation.clone());
                                self.copied_until = Some(Instant::now() + Duration::from_secs(2));
                            }
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
                    .corner_radius(22)
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
