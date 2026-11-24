use super::*;

pub(super) const SELF_PREVIEW: &str = "__self_preview";
const GAP: f32 = 14.0;
const PRESENCE_SLOT: f32 = 48.0;

fn monitor_glow(painter: &egui::Painter, screen: egui::Rect) {
    const SEGMENTS: u32 = 48;
    let accent = palette().accent;
    let center = screen.center();
    let radius = screen.size();
    let mut glow = egui::Mesh::default();
    for (distance, opacity) in [(0.0, 0.11), (0.42, 0.07), (0.72, 0.022), (1.0, 0.0)] {
        for index in 0..SEGMENTS {
            let angle = index as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let offset = egui::vec2(angle.cos() * radius.x, angle.sin() * radius.y) * distance;
            glow.colored_vertex(center + offset, accent.gamma_multiply(opacity));
        }
    }
    for ring in 0..3 {
        for index in 0..SEGMENTS {
            let inner = ring * SEGMENTS;
            let a = inner + index;
            let b = inner + (index + 1) % SEGMENTS;
            glow.add_triangle(a, b, a + SEGMENTS);
            glow.add_triangle(b, b + SEGMENTS, a + SEGMENTS);
        }
    }
    painter.add(egui::Shape::mesh(glow));
}

fn paint_presence_picture(
    ui: &egui::Ui,
    rect: egui::Rect,
    name: &str,
    texture: Option<egui::TextureId>,
) {
    if let Some(texture) = texture {
        egui::Image::new((texture, rect.size()))
            .corner_radius((rect.width() / 2.0).round() as u8)
            .paint_at(ui, rect);
    } else {
        paint_avatar_circle(ui.painter(), rect, name);
    }
}

fn paint_presence_name(ui: &egui::Ui, avatar: egui::Rect, name: &str) {
    let p = palette();
    let clip = ui.clip_rect().shrink2(egui::vec2(8.0, 0.0));
    let max_width = (clip.width() - 20.0).clamp(12.0, 400.0);
    let mut job =
        egui::text::LayoutJob::simple(name.to_owned(), skin::semibold(12.5), p.text, max_width);
    job.wrap.max_rows = 2;
    job.wrap.break_anywhere = true;
    let text = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let size = text.size() + egui::vec2(20.0, 10.0);
    let center = avatar
        .center()
        .x
        .max(clip.left() + size.x / 2.0)
        .min(clip.right() - size.x / 2.0);
    let label = egui::Rect::from_center_size(
        egui::pos2(center, avatar.bottom() + 5.0 + size.y / 2.0),
        size,
    );
    ui.painter().rect_filled(label, 8.0, p.card);
    ui.painter()
        .rect_stroke(label, 8.0, (1.0, p.border), egui::StrokeKind::Inside);
    ui.painter()
        .galley(label.center() - text.size() / 2.0, text, p.text);
}

fn capsule_person(ui: &mut egui::Ui, name: &str, texture: Option<egui::TextureId>) {
    ui.horizontal(|ui| {
        profile::avatar(ui, name, 20.0, texture);
        ui.add(egui::Label::new(name).truncate())
            .on_hover_text(name);
    });
}

impl PublisherUi {
    pub(super) fn room_capsule(&mut self, ui: &mut egui::Ui, state: &Snapshot, width: f32) {
        let name = if state.room_name.is_empty() {
            tr("Your private room")
        } else if state.room_name == "Friends" {
            tr("Friends")
        } else {
            &state.room_name
        };
        let count = format!("{} {}", state.participants.len() + 1, tr("in room"));
        let label = format!("{name} · {count}");
        let own_id = self
            .profile_client
            .as_ref()
            .map(|client| client.participant_id.clone());
        let own_texture = own_id.and_then(|id| self.profile_texture(ui.ctx(), &id));
        let mut avatars = vec![(self.name.clone(), own_texture)];
        for (index, name) in state.participants.iter().take(2).enumerate() {
            let texture = state
                .participant_ids
                .get(index)
                .and_then(|(id, _)| self.profile_texture(ui.ctx(), id));
            avatars.push((name.clone(), texture));
        }
        let stack_width = 20.0 + (avatars.len() - 1) as f32 * 13.0;
        let online = state.connection == "connected";
        let contents = egui::Atoms::new((
            egui::Atom::paint(egui::vec2(8.0, 18.0), move |ui, args| {
                ui.painter().circle_filled(
                    args.rect.center(),
                    3.5,
                    if online {
                        palette().success
                    } else {
                        palette().warning
                    },
                );
            }),
            skin::strong(name, 13.0),
        ));
        let mut button = egui::Button::new(contents)
            .truncate()
            .corner_radius(20)
            .stroke((1.0, palette().border))
            .fill(palette().panel);
        if width >= 156.0 {
            button = button.right_text(egui::Atom::paint(
                egui::vec2(stack_width, 20.0),
                |ui, args| {
                    for (index, (name, texture)) in avatars.iter().enumerate() {
                        let rect = egui::Rect::from_min_size(
                            args.rect.min + egui::vec2(index as f32 * 13.0, 0.0),
                            egui::vec2(20.0, 20.0),
                        );
                        paint_presence_picture(ui, rect, name, *texture);
                        ui.painter()
                            .circle_stroke(rect.center(), 10.0, (1.0, palette().panel));
                    }
                },
            ));
        }
        let response = ui
            .add_sized([width, 36.0], button)
            .accessible_name(&label)
            .on_hover_text(&label);
        egui::Popup::menu(&response)
            .frame(skin::menu_frame())
            .show(|ui| {
                ui.set_width(248.0);
                ui.label(skin::strong(&label, 13.5).color(palette().text));
                egui::ScrollArea::vertical()
                    .max_height(235.0)
                    .show(ui, |ui| {
                        ui.label(
                            skin::strong(tr("People in this room"), 12.0).color(palette().muted),
                        );
                        capsule_person(ui, &format!("{} ({})", self.name, tr("you")), own_texture);
                        for (index, name) in state.participants.iter().enumerate() {
                            let texture = state
                                .participant_ids
                                .get(index)
                                .and_then(|(id, _)| self.profile_texture(ui.ctx(), id));
                            capsule_person(ui, name, texture);
                        }
                    });
                ui.separator();
                if skin::menu_row(ui, tr("Show room roster"), Some(Icon::People), "", false)
                    .clicked()
                {
                    self.roster_open = true;
                    ui.close();
                }
            });
    }

    pub(super) fn stage_view_controls(
        &mut self,
        ui: &mut egui::Ui,
        state: &Snapshot,
        compact: bool,
    ) {
        fn caption(compact: bool, label: &str) -> Option<&str> {
            (!compact).then(|| tr(label))
        }
        // The parent aligns this control group to the right. Add the buttons
        // inward from that edge so Grid reads first across the toolbar.
        let mut controls = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("stage-views")
                .max_rect(ui.available_rect_before_wrap())
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        let watched = state.tiles.iter().find(|tile| tile.watching);
        controls.add_enabled_ui(watched.is_some(), |ui| {
            if skin::layout_button(
                ui,
                Icon::Focus,
                caption(compact, "Focus"),
                tr("Focus stream"),
                self.focused.is_some(),
            )
            .clicked()
            {
                self.focused = state
                    .tiles
                    .iter()
                    .find(|tile| tile.watching && self.spotlight.as_ref() == Some(&tile.id))
                    .or(watched)
                    .map(|tile| tile.id.clone());
                self.spotlight = None;
            }
        });
        controls.add_enabled_ui(state.tiles.len() > 1, |ui| {
            if skin::layout_button(
                ui,
                Icon::Spotlight,
                caption(compact, "Spotlight"),
                tr("Spotlight"),
                self.spotlight.is_some(),
            )
            .clicked()
            {
                self.spotlight = self
                    .focused
                    .take()
                    .or_else(|| Self::default_spotlight(state).map(|tile| tile.id.clone()));
            }
        });
        if skin::layout_button(
            &mut controls,
            Icon::Grid,
            caption(compact, "Grid"),
            tr("Back to grid"),
            self.focused.is_none() && self.spotlight.is_none(),
        )
        .clicked()
        {
            self.focused = None;
            self.spotlight = None;
        }
        ui.advance_cursor_after_rect(controls.min_rect());
    }

    pub(super) fn empty_room_stage(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        ui.vertical_centered(|ui| {
            let p = palette();
            let available = ui.available_size();
            let spacing = ui.spacing().item_spacing.y;
            let layout = |text: egui::RichText| {
                egui::WidgetText::from(text).into_galley(
                    ui,
                    Some(egui::TextWrapMode::Wrap),
                    available.x,
                    egui::TextStyle::Body,
                )
            };
            let title = layout(skin::strong(tr("No one is sharing yet"), 24.0).color(p.text));
            let presence = (!state.participants.is_empty() && available.x >= PRESENCE_SLOT)
                .then(|| layout(egui::RichText::new(tr("Here now")).color(p.muted)));
            let text_height = 24.0
                + spacing
                + title.size().y
                + presence.as_ref().map_or(0.0, |label| {
                    4.0 + spacing * 2.0 + label.size().y + PRESENCE_SLOT
                });
            let scale = ((available.y - text_height) / 136.0)
                .min(available.x / 218.0)
                .clamp(0.01, 1.0);
            let art_size = egui::vec2(218.0, 136.0) * scale;
            ui.add_space(((available.y - text_height - art_size.y) * 0.5).max(0.0));
            let (art, _) = ui.allocate_exact_size(art_size, egui::Sense::hover());
            let screen = egui::Rect::from_min_size(art.min, egui::vec2(art.width(), 122.0 * scale));
            monitor_glow(ui.painter(), screen);
            ui.painter().rect_filled(screen, 18.0, p.panel);
            ui.painter().rect_stroke(
                screen,
                18.0,
                (1.0, p.hover_stroke),
                egui::StrokeKind::Inside,
            );
            let foot = art.center_bottom();
            ui.painter()
                .line_segment([screen.center_bottom(), foot], (2.0, p.hover_stroke));
            ui.painter().line_segment(
                [
                    foot - egui::vec2(24.0 * scale, 0.0),
                    foot + egui::vec2(24.0 * scale, 0.0),
                ],
                (2.0, p.hover_stroke),
            );
            ui.add_space(24.0);
            ui.label(title);
            if let Some(presence) = presence {
                ui.add_space(4.0);
                ui.label(presence);
                self.room_presence(ui, state);
            }
        });
    }

    fn room_presence(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        const SLOT: f32 = PRESENCE_SLOT;
        const SPACING: f32 = 8.0;
        let max_slots = ((ui.available_width() + SPACING) / (SLOT + SPACING)).floor() as usize;
        if max_slots == 0 {
            return;
        }
        let slots = state.participants.len().min(max_slots);
        let overflow = state.participants.len() > max_slots;
        let shown = slots - usize::from(overflow);
        let width = slots as f32 * (SLOT + SPACING) - SPACING;
        let (strip, _) = ui.allocate_exact_size(egui::vec2(width, SLOT), egui::Sense::hover());
        let slot = |index: usize| {
            egui::Rect::from_min_size(
                strip.min + egui::vec2(index as f32 * (SLOT + SPACING), 0.0),
                egui::vec2(SLOT, SLOT),
            )
        };
        let pointer = ui.input(|input| input.pointer.hover_pos());
        let mut hovered = None;
        let mut focused = None;
        for (index, name) in state.participants.iter().take(shown).enumerate() {
            let id = state.participant_ids.get(index).map(|(id, _)| id.as_str());
            let texture = id.and_then(|id| self.profile_texture(ui.ctx(), id));
            let rect = slot(index);
            let response = ui.interact(
                rect,
                ui.make_persistent_id(("room-presence-avatar", id, id.is_none().then_some(index))),
                egui::Sense::focusable_noninteractive(),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Image, ui.is_enabled(), name)
            });
            let is_hovered = response.hovered() && pointer.is_some_and(|pos| rect.contains(pos));
            let highlighted = is_hovered || response.has_focus();
            let picture = rect.shrink(if highlighted { 2.0 } else { 4.0 });
            paint_presence_picture(ui, picture, name, texture);
            let border = if highlighted {
                palette().accent
            } else {
                palette().hover_stroke
            };
            ui.painter()
                .circle_stroke(picture.center(), picture.width() / 2.0, (1.5, border));
            if is_hovered {
                hovered = Some((rect, name.as_str()));
            } else if response.has_focus() {
                focused = Some((rect, name.as_str()));
            }
        }
        if overflow {
            let more = ui
                .put(
                    slot(shown).shrink(4.0),
                    egui::Button::new(format!("+{}", state.participants.len() - shown))
                        .corner_radius(20),
                )
                .accessible_name(tr("People in this room"));
            if more.clicked() {
                self.roster_open = true;
            }
        }
        if let Some((rect, name)) = hovered.or(focused) {
            paint_presence_name(ui, rect, name);
        }
    }

    pub(super) fn default_spotlight(state: &Snapshot) -> Option<&Tile> {
        state
            .tiles
            .iter()
            .find(|tile| tile.watching)
            .or(state.tiles.first())
    }

    pub(super) fn reconcile_spotlight(&mut self, state: &Snapshot) {
        if state.tiles.len() < 2 {
            self.spotlight = None;
        } else if let Some(id) = &self.spotlight
            && !state.tiles.iter().any(|tile| tile.id == *id)
        {
            self.spotlight = Self::default_spotlight(state).map(|tile| tile.id.clone());
        }
    }

    pub(super) fn refresh_self_preview(&mut self, ctx: &egui::Context, state: &Snapshot) {
        if !state.any_sharing() {
            self.self_preview_revision = None;
            self.textures.remove(SELF_PREVIEW);
            return;
        }
        if ctx.input(|input| {
            input.viewport().visible() == Some(false) || input.viewport().minimized == Some(true)
        }) {
            return;
        }
        let previous = self
            .self_preview_revision
            .as_ref()
            .filter(|(cache, _)| std::sync::Arc::ptr_eq(cache, &state.self_preview));
        if previous.is_none() {
            self.textures.remove(SELF_PREVIEW);
        }
        let frame = state.self_preview.lock().ok().and_then(|slot| {
            slot.as_ref()
                .filter(|frame| previous.is_none_or(|(_, sequence)| *sequence != frame.sequence))
                .cloned()
        });
        let Some(frame) = frame else { return };
        // Keep the cache owner alongside its sequence because a new source can
        // restart its sequence at one. Read the existing JPEG without touching capture.
        self.self_preview_revision = Some((state.self_preview.clone(), frame.sequence));
        if let Ok(image) =
            image::load_from_memory_with_format(&frame.bytes, image::ImageFormat::Jpeg)
        {
            let image = image.into_rgba8();
            let picture = egui::ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                image.as_raw(),
            );
            if let Some(texture) = self.textures.get_mut(SELF_PREVIEW) {
                texture.set(picture, egui::TextureOptions::LINEAR);
            } else {
                self.textures.insert(
                    SELF_PREVIEW.to_owned(),
                    ctx.load_texture(SELF_PREVIEW, picture, egui::TextureOptions::LINEAR),
                );
            }
        }
    }

    fn paint_self_preview(&mut self, ui: &mut egui::Ui, state: &Snapshot, width: f32) {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(width, width * 9.0 / 16.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 10.0, egui::Color32::BLACK);
        let texture = if state.camera_sharing {
            self.camera_view_texture(ui, state, rect)
        } else {
            self.refresh_self_preview(ui.ctx(), state);
            self.textures.get(SELF_PREVIEW)
        };
        if let Some(texture) = texture {
            egui::Image::new((texture.id(), texture.size_vec2()))
                .corner_radius(10)
                .uv(if state.camera_sharing {
                    mirrored_uv()
                } else {
                    uv()
                })
                .paint_at(ui, fit_picture(rect, texture.aspect_ratio()));
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                tr("Loading preview…"),
                egui::FontId::proportional(12.0),
                egui::Color32::LIGHT_GRAY,
            );
        }
    }

    fn camera_view_texture(
        &mut self,
        ui: &egui::Ui,
        state: &Snapshot,
        rect: egui::Rect,
    ) -> Option<&egui::TextureHandle> {
        let feed = state.camera_feed.as_ref()?;
        let hidden = ui.input(|input| {
            let window = input.viewport();
            window.visible() == Some(false)
                || window.minimized == Some(true)
                || window.occluded == Some(true)
        });
        if hidden || !ui.is_rect_visible(rect) {
            return None;
        }
        if self
            .camera_view
            .as_ref()
            .is_some_and(|(owner, _)| !std::sync::Arc::ptr_eq(owner, feed))
        {
            self.camera_view = None;
        }
        if let Some(image) = feed
            .next_sample()
            .as_ref()
            .and_then(crate::video_image::camera_image)
        {
            if let Some((_, texture)) = &mut self.camera_view {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                let texture =
                    ui.ctx()
                        .load_texture("local-camera", image, egui::TextureOptions::LINEAR);
                self.camera_view = Some((feed.clone(), texture));
            }
        }
        ui.ctx().request_repaint_after(Duration::from_millis(16));
        self.camera_view.as_ref().map(|(_, texture)| texture)
    }

    pub(super) fn only_stream_stage(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                ui.label(skin::strong(tr("You're live"), 16.0).color(palette().error));
                if let Some(mode) = state.camera_mode {
                    ui.label(
                        egui::RichText::new(format!("{}p · {} FPS", mode.height, mode.fps))
                            .color(palette().muted),
                    );
                } else if let Some(mode) = state.mode {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}p · {} FPS · {:.1} Mbps",
                            mode.height,
                            mode.fps,
                            state.delivery.bitrate_bps as f64 / 1_000_000.0
                        ))
                        .color(palette().muted),
                    );
                }
            });
            ui.add_space(6.0);
            let reserve = if state.camera_sharing { 6.0 } else { 34.0 };
            let width = ui
                .available_width()
                .min((ui.available_height() - reserve).max(40.0) * 16.0 / 9.0);
            self.paint_self_preview(ui, state, width);
            if !state.camera_sharing {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr("Your preview updates at most once a minute."))
                        .color(palette().muted),
                );
            }
        });
    }

    fn self_preview_card(&mut self, ui: &mut egui::Ui, state: &Snapshot, width: f32) {
        egui::Frame::new()
            .fill(palette().panel)
            .stroke(egui::Stroke::new(1.0, palette().border))
            .corner_radius(10)
            .inner_margin(6)
            .show(ui, |ui| {
                ui.set_width(width - 14.0);
                ui.horizontal(|ui| {
                    let label = if state.camera_sharing {
                        "Your camera"
                    } else {
                        "My preview"
                    };
                    let title = ui.label(skin::strong(tr(label), 12.5).color(palette().text));
                    if !state.camera_sharing {
                        title.on_hover_text(tr("Your preview updates at most once a minute."));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if skin::icon_button(ui, Icon::Close, tr("Hide my preview")).clicked() {
                            self.self_preview_hidden = true;
                        }
                    });
                });
                self.paint_self_preview(ui, state, width - 14.0);
            });
    }

    pub(super) fn room_stage(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let main = self
            .spotlight
            .as_ref()
            .and_then(|id| state.tiles.iter().find(|tile| tile.id == *id));
        let show_self = state.any_sharing()
            && !self.self_preview_hidden
            && (state.camera_sharing || state.tiles.iter().any(|tile| tile.watching));
        if main.is_none() && !show_self {
            self.grid_stage(ui, state);
            return;
        }

        // A reserved rail keeps the preview and filmstrip clear of stream controls.
        let bounds = ui.available_rect_before_wrap();
        let rail_width = if main.is_some() { 186.0 } else { 162.0 };
        let main_bounds = egui::Rect::from_min_max(
            bounds.min,
            bounds.right_bottom() - egui::vec2(rail_width + GAP, 0.0),
        );
        let rail_bounds = egui::Rect::from_min_max(
            egui::pos2(bounds.right() - rail_width, bounds.top()),
            bounds.max,
        );
        let mut stage = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("stage-main")
                .max_rect(main_bounds),
        );
        stage.set_clip_rect(ui.clip_rect().intersect(main_bounds));
        if let Some(tile) = main {
            let width = main_bounds.width().min(main_bounds.height() * 16.0 / 9.0);
            stage.vertical_centered(|ui| self.stream_tile(ui, tile, width));
        } else {
            self.grid_stage(&mut stage, state);
        }

        let mut rail = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("stage-rail")
                .max_rect(rail_bounds),
        );
        rail.set_clip_rect(ui.clip_rect().intersect(rail_bounds));
        if show_self {
            self.self_preview_card(&mut rail, state, rail_width);
            rail.add_space(6.0);
        }
        if let Some(tile) = main {
            let preview_width = rail_width - rail.spacing().scroll.allocated_width();
            egui::ScrollArea::vertical()
                .id_salt("spotlight-film")
                .auto_shrink([false, true])
                .max_height(rail.available_height())
                .show(&mut rail, |ui| {
                    for other in state.tiles.iter().filter(|other| other.id != tile.id) {
                        self.stream_tile_with_layout(ui, other, preview_width, true);
                    }
                });
        }
        ui.allocate_rect(bounds, egui::Sense::hover());
    }

    fn grid_stage(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let tiles: Vec<_> = state
            .tiles
            .iter()
            .filter(|t| self.focused.as_ref().is_none_or(|id| id == &t.id))
            .collect();
        let columns = if tiles.len() > 1 && ui.available_width() >= 640.0 {
            2
        } else {
            1
        };
        let gap = 14.0;
        let rows = tiles.len().div_ceil(columns).min(2);
        let scroll_space = if tiles.len() > 1 {
            ui.spacing().scroll.allocated_width()
        } else {
            0.0
        };
        if columns == 1 && tiles.len() > 1 {
            ui.label(
                egui::RichText::new(tr("Scroll to see all streams"))
                    .size(12.0)
                    .color(palette().muted),
            );
        }
        let available_width = ui.available_width() - scroll_space;
        let max_width = (available_width - gap * (columns - 1) as f32) / columns as f32;
        let row_height = (ui.available_height() - gap * (rows - 1) as f32) / rows as f32;
        let width = if columns == 1 && tiles.len() > 1 {
            max_width
        } else {
            max_width.min(row_height.max(90.0) * 16.0 / 9.0)
        };
        let inset = ((available_width - width * columns as f32 - gap * (columns - 1) as f32) / 2.0)
            .max(0.0);
        let scroll = egui::ScrollArea::vertical()
            .id_salt("streams-scroll")
            .auto_shrink([false, false]);
        #[cfg(feature = "performance-experiments")]
        let scroll = if let Some(offset) = ui
            .ctx()
            .data(|data| data.get_temp::<f32>(egui::Id::unique("deep-scroll-offset")))
        {
            scroll.vertical_scroll_offset(offset)
        } else {
            scroll
        };
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
    }
}
