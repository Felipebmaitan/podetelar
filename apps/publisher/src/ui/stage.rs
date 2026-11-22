//! The stage: every way of seeing a room. Empty, only you live, one stream, several
//! (grid, spotlight or focus), plus your own picture-in-picture while others stream.
use super::shell::glow;
use super::*;

const GAP: f32 = 14.0;
/// Bottom space kept free for the floating dock.
const DOCK_SPACE: f32 = 112.0;
const FILM_W: f32 = 236.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ViewMode {
    Grid,
    Spotlight,
}

fn ease(ctx: &egui::Context, id: egui::Id, on: bool, seconds: f32) -> f32 {
    ctx.animate_bool_with_time_and_easing(id, on, seconds, egui::emath::easing::cubic_out)
}

fn perimeter_len(rect: egui::Rect, radius: f32) -> f32 {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    2.0 * (rect.width() - 2.0 * r) + 2.0 * (rect.height() - 2.0 * r) + std::f32::consts::TAU * r
}

/// The point `distance` along a rounded rectangle's outline, so motion is continuous.
fn perimeter(rect: egui::Rect, radius: f32, distance: f32) -> egui::Pos2 {
    use std::f32::consts::{FRAC_PI_2, PI};
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let sx = rect.width() - 2.0 * r;
    let sy = rect.height() - 2.0 * r;
    let arc = FRAC_PI_2 * r;
    let mut d = distance.rem_euclid(perimeter_len(rect, radius));
    if d < sx {
        return egui::pos2(rect.left() + r + d, rect.top());
    }
    d -= sx;
    if d < arc {
        let a = -FRAC_PI_2 + d / r;
        return egui::pos2(rect.right() - r + r * a.cos(), rect.top() + r + r * a.sin());
    }
    d -= arc;
    if d < sy {
        return egui::pos2(rect.right(), rect.top() + r + d);
    }
    d -= sy;
    if d < arc {
        let a = d / r;
        return egui::pos2(
            rect.right() - r + r * a.cos(),
            rect.bottom() - r + r * a.sin(),
        );
    }
    d -= arc;
    if d < sx {
        return egui::pos2(rect.right() - r - d, rect.bottom());
    }
    d -= sx;
    if d < arc {
        let a = FRAC_PI_2 + d / r;
        return egui::pos2(
            rect.left() + r + r * a.cos(),
            rect.bottom() - r + r * a.sin(),
        );
    }
    d -= arc;
    if d < sy {
        return egui::pos2(rect.left(), rect.bottom() - r - d);
    }
    d -= sy;
    let a = PI + d / r;
    egui::pos2(rect.left() + r + r * a.cos(), rect.top() + r + r * a.sin())
}

/// A monitor outline traced by a spark of light, on a slow breathing glow.
/// Runs every frame, so the motion stays smooth.
pub(super) fn monitor_art(painter: &egui::Painter, center: egui::Pos2, scale: f32, time: f32) {
    let p = pal();
    let breath = (time * 0.7).sin() * 0.5 + 0.5;
    glow(
        painter,
        center + egui::vec2(-10.0, 0.0) * scale,
        (330.0 + 16.0 * breath) * scale,
        p.accent,
        0.0075,
    );
    glow(
        painter,
        center + egui::vec2(70.0, 50.0) * scale,
        210.0 * scale,
        p.spark,
        0.0035 + 0.002 * breath,
    );
    let screen = egui::Rect::from_center_size(center, egui::vec2(236.0, 148.0) * scale);
    let radius = 26.0 * scale;
    // Faint outline, drawn as a dense loop.
    let total = perimeter_len(screen, radius);
    let points: Vec<egui::Pos2> = (0..=240)
        .map(|k| perimeter(screen, radius, total * k as f32 / 240.0))
        .collect();
    painter.add(egui::Shape::line(points, egui::Stroke::new(1.5, p.border)));
    // The spark: a head and a tail of short segments, positions interpolated per frame.
    let head = total * (time * 0.11).fract();
    let tail = 72;
    let step = total * 0.0032;
    for k in 0..tail {
        let a = perimeter(screen, radius, head - k as f32 * step);
        let b = perimeter(screen, radius, head - (k + 1) as f32 * step);
        let fade = 1.0 - k as f32 / tail as f32;
        painter.line_segment(
            [a, b],
            egui::Stroke::new(
                (1.2 + 2.6 * fade) * scale.max(0.7),
                p.accent.gamma_multiply(fade * fade),
            ),
        );
    }
    let tip = perimeter(screen, radius, head);
    painter.circle_filled(tip, 9.0 * scale, p.accent.gamma_multiply(0.18));
    painter.circle_filled(tip, 4.0 * scale, p.accent);
    // Stand.
    let foot = egui::pos2(center.x, screen.bottom());
    let stroke = egui::Stroke::new(1.5, p.border);
    painter.line_segment([foot, foot + egui::vec2(0.0, 18.0 * scale)], stroke);
    painter.line_segment(
        [
            foot + egui::vec2(-34.0, 18.0) * scale,
            foot + egui::vec2(34.0, 18.0) * scale,
        ],
        stroke,
    );
}

fn secondary_button(label: &str, width: f32) -> egui::Button<'_> {
    egui::Button::new(skin::strong(label, 14.0))
        .fill(pal().control)
        .corner_radius(18)
        .min_size(egui::vec2(width, 42.0))
}

impl PublisherUi {
    /// Pull the newest still of your own screen into a texture (once a minute at most).
    pub(super) fn refresh_self_preview(&mut self, ctx: &egui::Context, state: &Snapshot) {
        if !state.sharing {
            self.self_preview_revision = None;
            self.textures.remove("__self_preview");
            return;
        }
        let same_source = self
            .self_preview_revision
            .as_ref()
            .is_some_and(|(cache, _)| std::sync::Arc::ptr_eq(cache, &state.self_preview));
        if !same_source {
            self.textures.remove("__self_preview");
        }
        let stale = |this: &Self, sequence: u64| {
            this.self_preview_revision
                .as_ref()
                .is_none_or(|(cache, seq)| {
                    !std::sync::Arc::ptr_eq(cache, &state.self_preview) || *seq != sequence
                })
                || !this.textures.contains_key("__self_preview")
        };
        let frame = state
            .self_preview
            .lock()
            .ok()
            .and_then(|frame| frame.as_ref().filter(|f| stale(self, f.sequence)).cloned());
        let picture: crate::playback::Picture = std::sync::Arc::default();
        if let Some(frame) = frame
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
            let tile = Tile {
                id: "__self_preview".into(),
                name: String::new(),
                codec: String::new(),
                watching: false,
                picture,
                presentation: std::sync::Arc::default(),
                frames: std::sync::Arc::default(),
                received_bps: std::sync::Arc::default(),
                volume: std::sync::Arc::default(),
                audio: std::sync::Arc::default(),
                status: String::new(),
            };
            self.update_picture(ctx, &tile);
        }
    }

    /// Nobody is sharing, and you are not either.
    pub(super) fn empty_stage(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let p = pal();
        let area = ui.available_rect_before_wrap();
        let time = ui.input(|i| i.time) as f32;
        ui.ctx().request_repaint();
        let art = egui::pos2(
            area.center().x,
            area.top() + (area.height() * 0.30).max(120.0),
        );
        monitor_art(ui.painter(), art, 0.9, time);
        let mut column = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_max(
                    egui::pos2(area.left(), art.y + 120.0),
                    area.max,
                ))
                .layout(egui::Layout::top_down(egui::Align::Center)),
        );
        column.spacing_mut().item_spacing.y = 8.0;
        column.label(skin::strong(tr("Nobody is sharing yet"), 28.0));
        column.label(
            egui::RichText::new(if state.participants.is_empty() {
                tr("Invite friends or start sharing. Streams will appear here.")
            } else {
                tr("Friends are here. Their streams appear when they start sharing.")
            })
            .size(14.0)
            .color(p.muted),
        );
        column.add_space(8.0);
        column.horizontal(|ui| {
            let width = if self.invitation.is_empty() {
                220.0
            } else {
                220.0 + 12.0 + 130.0
            };
            ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
            if ui
                .add(primary(tr("Share your screen"), 220.0).corner_radius(18))
                .clicked()
            {
                self.go_live = Some(true);
            }
            ui.add_space(12.0 - ui.spacing().item_spacing.x);
            if !self.invitation.is_empty() {
                let copied = self
                    .copied_until
                    .is_some_and(|until| Instant::now() < until);
                if ui
                    .add(secondary_button(
                        if copied { tr("Copied!") } else { tr("Invite") },
                        130.0,
                    ))
                    .clicked()
                {
                    ui.ctx().copy_text(self.invitation.clone());
                    self.copied_until = Some(Instant::now() + Duration::from_secs(2));
                }
            }
        });
        if !state.participants.is_empty() {
            column.add_space(10.0);
            column.label(
                egui::RichText::new(tr("Here now").to_uppercase())
                    .font(skin::semibold(11.0))
                    .color(p.muted),
            );
            column.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                let total: f32 = state
                    .participants
                    .iter()
                    .take(6)
                    .map(|n| 60.0 + n.chars().count() as f32 * 7.5)
                    .sum();
                ui.add_space(
                    ((ui.available_width() - total.min(ui.available_width())) / 2.0).max(0.0),
                );
                for (index, name) in state.participants.iter().take(6).enumerate() {
                    let id = state
                        .participant_ids
                        .get(index)
                        .map(|(id, _)| id.as_str())
                        .unwrap_or("");
                    let texture = self.profile_texture(ui.ctx(), id);
                    egui::Frame::new()
                        .fill(p.control)
                        .corner_radius(16)
                        .inner_margin(egui::Margin::symmetric(8, 5))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                profile::avatar(ui, name, 24.0, texture);
                                ui.label(skin::strong(name, 13.0));
                            });
                        });
                }
            });
        }
        ui.allocate_rect(area, egui::Sense::hover());
    }

    /// Only you are on air: your own source, with what is being sent.
    pub(super) fn my_stage(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let p = pal();
        let ctx = ui.ctx().clone();
        self.refresh_self_preview(&ctx, state);
        let area = ui.available_rect_before_wrap();
        let time = ui.input(|i| i.time) as f32;
        ctx.request_repaint_after(Duration::from_millis(16));
        let usable_h = (area.height() - DOCK_SPACE - 46.0).max(160.0);
        let width = (area.width() - 40.0)
            .min(usable_h * 16.0 / 9.0)
            .clamp(280.0, 900.0);
        let card = egui::Rect::from_center_size(
            egui::pos2(area.center().x, area.top() + usable_h / 2.0 + 6.0),
            egui::vec2(width, width * 9.0 / 16.0),
        );
        let pulse = (time * 1.6).sin() * 0.5 + 0.5;
        glow(
            ui.painter(),
            card.center(),
            width * 0.62,
            p.danger,
            0.006 + 0.004 * pulse,
        );
        for layer in 1..=4 {
            ui.painter().rect_filled(
                card.expand(layer as f32 * 5.0),
                28.0 + layer as f32 * 5.0,
                p.danger.gamma_multiply(0.04 / layer as f32),
            );
        }
        ui.painter().rect_filled(card, 28.0, egui::Color32::BLACK);
        if let Some(texture) = self.textures.get("__self_preview") {
            let target = fit_picture(card, texture.aspect_ratio());
            egui::Image::new((texture.id(), texture.size_vec2()))
                .corner_radius(28)
                .paint_at(ui, target);
        }
        ui.painter()
            .rect_filled(card, 28.0, egui::Color32::from_black_alpha(80));
        ui.painter().rect_stroke(
            card,
            28.0,
            egui::Stroke::new(1.5, p.danger.gamma_multiply(0.6 + 0.4 * pulse)),
            egui::StrokeKind::Inside,
        );
        // Live pill.
        let pill =
            egui::Rect::from_min_size(card.min + egui::vec2(18.0, 18.0), egui::vec2(150.0, 34.0));
        ui.painter().rect_filled(pill, 17.0, p.overlay);
        ui.painter().circle_filled(
            pill.left_center() + egui::vec2(18.0, 0.0),
            4.0 + 3.0 * pulse,
            p.danger.gamma_multiply(0.3),
        );
        ui.painter()
            .circle_filled(pill.left_center() + egui::vec2(18.0, 0.0), 4.0, p.danger);
        ui.painter().text(
            pill.left_center() + egui::vec2(34.0, 0.0),
            egui::Align2::LEFT_CENTER,
            tr("You're live"),
            skin::semibold(13.0),
            p.text,
        );
        // What is being sent.
        if let Some(mode) = state.mode {
            let text = format!(
                "{}p · {} FPS · {:.1} Mbps",
                mode.height,
                mode.fps,
                state.delivery.bitrate_bps as f64 / 1_000_000.0
            );
            let chip = egui::Rect::from_min_size(
                card.left_bottom() + egui::vec2(18.0, -52.0),
                egui::vec2(230.0, 34.0),
            );
            ui.painter().rect_filled(chip, 17.0, p.overlay);
            ui.painter().text(
                chip.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(13.0),
                p.text,
            );
        }
        let mut column = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_max(
                    egui::pos2(area.left(), card.bottom() + 14.0),
                    area.max,
                ))
                .layout(egui::Layout::top_down(egui::Align::Center)),
        );
        column.label(
            egui::RichText::new(tr("Friends watch from their own Watch button. This still updates about once a minute."))
                .size(13.0)
                .color(p.muted),
        );
        ui.allocate_rect(area, egui::Sense::hover());
    }

    /// Your own screen, small, while you watch others. Click the cross to hide it.
    pub(super) fn pip(&mut self, ctx: &egui::Context, state: &Snapshot, stage: egui::Rect) {
        if !state.sharing || state.tiles.is_empty() || self.pip_hidden || self.fullscreen.is_some()
        {
            return;
        }
        self.refresh_self_preview(ctx, state);
        let p = pal();
        let rise = ease(ctx, egui::Id::NULL.with("pip-rise"), true, 0.5);
        let size = egui::vec2(168.0, 94.0);
        let texture = self
            .textures
            .get("__self_preview")
            .map(|t| (t.id(), t.aspect_ratio(), t.size_vec2()));
        egui::Area::new(egui::Id::NULL.with("pip"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(egui::pos2(
                stage.left() + 20.0,
                stage.bottom() - 20.0 + (1.0 - rise) * 20.0,
            ))
            .show(ctx, |ui| {
                ui.set_opacity(rise);
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter().rect_filled(
                    rect.translate(egui::vec2(0.0, 5.0)),
                    18.0,
                    egui::Color32::from_black_alpha(80),
                );
                ui.painter().rect_filled(rect, 18.0, egui::Color32::BLACK);
                if let Some((id, aspect, tex_size)) = texture {
                    egui::Image::new((id, tex_size))
                        .corner_radius(18)
                        .paint_at(ui, fit_picture(rect, aspect));
                }
                ui.painter().rect_stroke(
                    rect,
                    18.0,
                    egui::Stroke::new(1.5, p.danger.gamma_multiply(0.8)),
                    egui::StrokeKind::Inside,
                );
                let chip = egui::Rect::from_min_size(
                    rect.min + egui::vec2(8.0, 8.0),
                    egui::vec2(84.0, 24.0),
                );
                ui.painter().rect_filled(chip, 12.0, p.overlay);
                ui.painter().circle_filled(
                    chip.left_center() + egui::vec2(12.0, 0.0),
                    3.5,
                    p.danger,
                );
                ui.painter().text(
                    chip.left_center() + egui::vec2(22.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    tr("You"),
                    skin::semibold(12.0),
                    p.text,
                );
                let close = egui::Rect::from_center_size(
                    rect.right_top() + egui::vec2(-20.0, 20.0),
                    egui::vec2(26.0, 26.0),
                );
                let hover = ui.interact(close, response.id.with("close"), egui::Sense::click());
                ui.painter().circle_filled(close.center(), 13.0, p.overlay);
                skin::icon(
                    ui.painter(),
                    close.shrink(5.0),
                    Icon::Close,
                    if hover.hovered() { p.text } else { p.muted },
                );
                if hover.on_hover_text(tr("Hide preview")).clicked() {
                    self.pip_hidden = true;
                }
            });
    }

    /// One, several or many streams: grid, spotlight or focus, with a soft crossfade between them.
    pub(super) fn stage_tiles(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let ctx = ui.ctx().clone();
        let key = (self.view_mode, self.focused.clone(), state.tiles.len());
        if self.view_key.as_ref() != Some(&key) {
            self.view_key = Some(key);
            self.view_changed = ctx.input(|i| i.time);
        }
        let progress = (((ctx.input(|i| i.time) - self.view_changed) as f32) / 0.4).clamp(0.0, 1.0);
        let t = egui::emath::easing::cubic_out(progress);
        if progress < 1.0 {
            ctx.request_repaint();
        }
        ui.set_opacity(0.15 + 0.85 * t);
        ui.add_space((1.0 - t) * 12.0);
        let tiles: Vec<&Tile> = state.tiles.iter().collect();
        let usable_h = (ui.available_height() - DOCK_SPACE).max(150.0);
        let focus = self
            .focused
            .as_ref()
            .and_then(|id| tiles.iter().find(|tile| &tile.id == id))
            .copied();
        if let Some(tile) = focus {
            let width = ui.available_width().min(usable_h * 16.0 / 9.0).max(260.0);
            let inset = ((ui.available_width() - width) / 2.0).max(0.0);
            ui.horizontal_top(|ui| {
                ui.add_space(inset);
                self.stream_tile(ui, tile, width);
            });
        } else if self.view_mode == ViewMode::Spotlight && tiles.len() > 1 {
            self.spotlight_layout(ui, &tiles, usable_h);
        } else {
            self.grid_layout(ui, &tiles, usable_h);
        }
    }

    fn grid_layout(&mut self, ui: &mut egui::Ui, tiles: &[&Tile], usable_h: f32) {
        let n = tiles.len();
        let avail_w = ui.available_width();
        let (cols, rows) = match n {
            0 | 1 => (1, 1),
            2 => {
                if avail_w >= 820.0 {
                    (2, 1)
                } else {
                    (1, 2)
                }
            }
            3 | 4 => (2, 2),
            5 | 6 => (3, 2),
            _ => (3, n.div_ceil(3)),
        };
        let by_width = (avail_w - GAP * (cols - 1) as f32) / cols as f32;
        let by_height = (usable_h - GAP * (rows - 1) as f32) / rows as f32 * 16.0 / 9.0;
        let scrolls = rows > 3 || (cols == 1 && rows > 1 && by_height < 300.0);
        let width = if scrolls {
            by_width
        } else {
            by_width.min(by_height)
        }
        .max(220.0);
        let height = width * 9.0 / 16.0;
        let inset_x = ((avail_w - (width * cols as f32 + GAP * (cols - 1) as f32)) / 2.0).max(0.0);
        let inset_y = if scrolls {
            0.0
        } else {
            ((usable_h - (height * rows as f32 + GAP * (rows - 1) as f32)) / 2.0).max(0.0)
        };
        egui::ScrollArea::vertical()
            .id_salt("stage-grid-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(inset_y);
                ui.horizontal_top(|ui| {
                    ui.add_space(inset_x);
                    egui::Grid::new("stream-grid")
                        .num_columns(cols)
                        .spacing(egui::vec2(GAP, GAP))
                        .show(ui, |ui| {
                            for (index, tile) in tiles.iter().enumerate() {
                                self.stream_tile(ui, tile, width);
                                if (index + 1) % cols == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                });
                if scrolls {
                    ui.add_space(DOCK_SPACE);
                }
            });
    }

    fn spotlight_layout(&mut self, ui: &mut egui::Ui, tiles: &[&Tile], usable_h: f32) {
        let main_id = self
            .spotlight
            .clone()
            .filter(|id| tiles.iter().any(|tile| &tile.id == id))
            .or_else(|| {
                tiles
                    .iter()
                    .find(|tile| tile.watching)
                    .map(|tile| tile.id.clone())
            })
            .or_else(|| tiles.first().map(|tile| tile.id.clone()))
            .unwrap_or_default();
        let avail_w = ui.available_width();
        let main_w = (avail_w - FILM_W - GAP)
            .min(usable_h * 16.0 / 9.0)
            .max(300.0);
        let main_h = main_w * 9.0 / 16.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let inset = ((avail_w - main_w - FILM_W - GAP) / 2.0).max(0.0);
            ui.add_space(inset);
            ui.vertical(|ui| {
                ui.add_space(((usable_h - main_h) / 2.0).max(0.0));
                if let Some(tile) = tiles.iter().find(|tile| tile.id == main_id) {
                    self.stream_tile(ui, tile, main_w);
                }
            });
            egui::ScrollArea::vertical()
                .id_salt("spotlight-film")
                .auto_shrink([false, false])
                .max_height(usable_h + DOCK_SPACE)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = GAP;
                        ui.set_width(FILM_W);
                        for tile in tiles.iter().filter(|tile| tile.id != main_id) {
                            self.stream_tile(ui, tile, FILM_W);
                        }
                        ui.add_space(DOCK_SPACE);
                    });
                });
        });
    }
}
