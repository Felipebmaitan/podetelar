use super::*;

pub(super) const SELF_PREVIEW: &str = "__self_preview";
const GAP: f32 = 14.0;

impl PublisherUi {
    pub(super) fn room_capsule(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        let name = if state.room_name.is_empty() {
            tr("Your private room")
        } else if state.room_name == "Friends" {
            tr("Friends")
        } else {
            &state.room_name
        };
        let count = format!("{} {}", state.participants.len() + 1, tr("in room"));
        let label = format!("{name} · {count}");
        let width = (ui.available_width() - 375.0).clamp(80.0, 240.0);
        let avatars = std::iter::once(self.name.as_str())
            .chain(state.participants.iter().map(String::as_str))
            .take(3)
            .collect::<Vec<_>>();
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
            .corner_radius(16)
            .fill(palette().card);
        if width >= 156.0 {
            button = button.right_text(egui::Atom::paint(
                egui::vec2(stack_width, 20.0),
                |ui, args| {
                    for (index, name) in avatars.iter().enumerate() {
                        let rect = egui::Rect::from_min_size(
                            args.rect.min + egui::vec2(index as f32 * 13.0, 0.0),
                            egui::vec2(20.0, 20.0),
                        );
                        paint_avatar_circle(ui.painter(), rect, name);
                    }
                },
            ));
        }
        let response = ui
            .add_sized([width, 32.0], button)
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
                        if !state.tiles.is_empty() {
                            ui.label(skin::strong(tr("Live now"), 12.0).color(palette().muted));
                            for tile in &state.tiles {
                                let action = tr(if tile.watching {
                                    "Focus stream"
                                } else {
                                    "Watch"
                                });
                                let label = format!("{action} · {}", tile.name);
                                if ui
                                    .add(egui::Button::new(&label).truncate())
                                    .on_hover_text(&label)
                                    .clicked()
                                {
                                    if tile.watching {
                                        self.focused = Some(tile.id.clone());
                                        self.spotlight = None;
                                    } else {
                                        self.controller.send(Command::Watch {
                                            id: tile.id.clone(),
                                            enabled: true,
                                        });
                                        if self.focused.is_some() {
                                            self.focused = Some(tile.id.clone());
                                        } else if self.spotlight.is_some() {
                                            self.spotlight = Some(tile.id.clone());
                                        }
                                    }
                                    ui.close();
                                }
                            }
                            ui.separator();
                        }
                        ui.label(
                            skin::strong(tr("People in this room"), 12.0).color(palette().muted),
                        );
                        ui.add(
                            egui::Label::new(format!("{} ({})", self.name, tr("you"))).truncate(),
                        );
                        for name in &state.participants {
                            ui.add(egui::Label::new(name).truncate())
                                .on_hover_text(name);
                        }
                    });
                if ui.button(tr("User settings")).clicked() {
                    self.open_profile();
                    ui.close();
                }
            });
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
        if !state.sharing {
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
        self.refresh_self_preview(ui.ctx(), state);
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(width, width * 9.0 / 16.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 10.0, egui::Color32::BLACK);
        if let Some(texture) = self.textures.get(SELF_PREVIEW) {
            egui::Image::new((texture.id(), texture.size_vec2()))
                .corner_radius(10)
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

    pub(super) fn only_stream_stage(&mut self, ui: &mut egui::Ui, state: &Snapshot) {
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                ui.label(skin::strong(tr("You're live"), 16.0).color(palette().error));
                if let Some(mode) = state.mode {
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
            let width = ui
                .available_width()
                .min((ui.available_height() - 34.0).max(40.0) * 16.0 / 9.0);
            self.paint_self_preview(ui, state, width);
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(tr("Your preview updates at most once a minute."))
                    .color(palette().muted),
            );
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
                    ui.label(skin::strong(tr("My preview"), 12.5).color(palette().text))
                        .on_hover_text(tr("Your preview updates at most once a minute."));
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
        let show_self = state.sharing
            && !self.self_preview_hidden
            && state.tiles.iter().any(|tile| tile.watching);
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
            let scroll_space = rail.spacing().scroll.bar_width
                + rail.spacing().scroll.bar_inner_margin
                + rail.spacing().scroll.bar_outer_margin;
            egui::ScrollArea::vertical()
                .id_salt("spotlight-film")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .max_height(rail.available_height())
                .show(&mut rail, |ui| {
                    for other in state.tiles.iter().filter(|other| other.id != tile.id) {
                        self.stream_tile_with_layout(ui, other, rail_width - scroll_space, true);
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
                    .color(palette().muted),
            );
        }
        let max_width =
            (ui.available_width() - scroll_space - gap * (columns - 1) as f32) / columns as f32;
        let row_height = (ui.available_height() - gap * (rows - 1) as f32) / rows as f32;
        let width = if columns == 1 && tiles.len() > 1 {
            max_width
        } else {
            max_width.min(row_height.max(90.0) * 16.0 / 9.0)
        };
        let inset = ((ui.available_width() - width * columns as f32 - gap * (columns - 1) as f32)
            / 2.0)
            .max(0.0);
        let scroll = egui::ScrollArea::vertical()
            .id_salt("streams-scroll")
            .scroll_bar_visibility(if columns == 1 && tiles.len() > 1 {
                egui::scroll_area::ScrollBarVisibility::AlwaysVisible
            } else {
                egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded
            })
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
