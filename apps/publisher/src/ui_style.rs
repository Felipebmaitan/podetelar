//! Shared native visual language ("Private Screening Room"). Icons are drawn locally; no font glyph dependency.
use crate::theme::palette;
use eframe::egui::{self, Color32, FontFamily, FontId, RichText};

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("Inter SemiBold".into()))
}
pub fn strong(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(semibold(size))
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Inter".into(),
        egui::FontData::from_static(include_bytes!("../../../assets/fonts/Inter-Regular.ttf"))
            .into(),
    );
    fonts.font_data.insert(
        "Inter SemiBold".into(),
        egui::FontData::from_static(include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf"))
            .into(),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Inter".into());
    let mut bold = vec!["Inter SemiBold".into()];
    bold.extend(fonts.families[&FontFamily::Proportional].clone());
    fonts
        .families
        .insert(FontFamily::Name("Inter SemiBold".into()), bold);
    ctx.set_fonts(fonts);
}

#[derive(Clone, Copy)]
pub enum Icon {
    Volume,
    Muted,
    Close,
    Info,
    Window,
    Focus,
    Spotlight,
    Fullscreen,
    Stop,
    Play,
    Settings,
    Invite,
    More,
    Grid,
    Monitor,
    Camera,
    People,
    ChevronUp,
}

pub fn icon(p: &egui::Painter, rect: egui::Rect, icon: Icon, color: Color32) {
    let c = rect.center();
    let point = |x: f32, y: f32| c + egui::vec2(x, y) * (rect.width() / 20.0);
    let stroke = egui::Stroke::new(1.5, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([point(a.0, a.1), point(b.0, b.1)], stroke);
    };
    let box_at = |x: f32, y: f32, w: f32, h: f32| {
        p.rect_stroke(
            egui::Rect::from_min_max(point(x, y), point(x + w, y + h)),
            2.0,
            stroke,
            egui::StrokeKind::Inside,
        );
    };
    match icon {
        Icon::More => {
            for x in [-5.0, 0.0, 5.0] {
                p.circle_filled(point(x, 0.0), 1.5, color);
            }
        }
        Icon::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![point(-4.0, -7.0), point(7.0, 0.0), point(-4.0, 7.0)],
                color,
                egui::Stroke::NONE,
            ));
        }
        Icon::Camera => {
            box_at(-9.0, -6.0, 12.0, 12.0);
            p.add(egui::Shape::line(
                vec![
                    point(3.0, -2.5),
                    point(8.0, -5.0),
                    point(8.0, 5.0),
                    point(3.0, 2.5),
                ],
                stroke,
            ));
        }
        Icon::Stop => {
            p.rect_filled(
                egui::Rect::from_min_max(point(-5.5, -5.5), point(5.5, 5.5)),
                2.0,
                color,
            );
        }
        Icon::Grid => {
            for (x, y) in [(-7.0, -7.0), (1.0, -7.0), (-7.0, 1.0), (1.0, 1.0)] {
                box_at(x, y, 6.0, 6.0);
            }
        }
        Icon::Spotlight => {
            box_at(-8.0, -6.0, 10.0, 12.0);
            box_at(5.0, -6.0, 3.0, 4.0);
            box_at(5.0, 2.0, 3.0, 4.0);
        }
        Icon::Fullscreen | Icon::Focus => {
            for (x, y, dx, dy) in [
                (-7.0, -7.0, 1.0, 1.0),
                (7.0, -7.0, -1.0, 1.0),
                (-7.0, 7.0, 1.0, -1.0),
                (7.0, 7.0, -1.0, -1.0),
            ] {
                line((x, y + dy * 4.0), (x, y));
                line((x, y), (x + dx * 4.0, y));
            }
            if matches!(icon, Icon::Focus) {
                p.circle_filled(c, 2.0, color);
            }
        }
        Icon::Volume | Icon::Muted => {
            p.add(egui::Shape::line(
                vec![
                    point(-8.0, -3.0),
                    point(-4.0, -3.0),
                    point(1.0, -7.0),
                    point(1.0, 7.0),
                    point(-4.0, 3.0),
                    point(-8.0, 3.0),
                    point(-8.0, -3.0),
                ],
                stroke,
            ));
            if matches!(icon, Icon::Muted) {
                line((4.0, -3.0), (10.0, 3.0));
                line((4.0, 3.0), (10.0, -3.0));
                return;
            }
            p.add(egui::Shape::line(
                vec![point(4.0, -4.0), point(6.0, 0.0), point(4.0, 4.0)],
                stroke,
            ));
            p.add(egui::Shape::line(
                vec![point(7.0, -7.0), point(10.0, 0.0), point(7.0, 7.0)],
                stroke,
            ));
        }
        Icon::Close => {
            line((-5.0, -5.0), (5.0, 5.0));
            line((-5.0, 5.0), (5.0, -5.0));
        }
        Icon::Info => {
            p.circle_stroke(c, 8.0, stroke);
            p.circle_filled(point(0.0, -4.0), 1.1, color);
            line((0.0, -1.0), (0.0, 4.0));
        }
        Icon::Window => {
            box_at(-8.0, -7.0, 16.0, 14.0);
            line((-7.0, -3.0), (7.0, -3.0));
        }
        Icon::Monitor => {
            box_at(-8.0, -7.0, 16.0, 12.0);
            line((0.0, 5.0), (0.0, 9.0));
            line((-4.0, 9.0), (4.0, 9.0));
        }
        Icon::Invite => {
            p.circle_stroke(point(-3.0, -4.0), 3.0, stroke);
            p.add(egui::Shape::line(
                vec![
                    point(-8.0, 7.0),
                    point(-8.0, 4.0),
                    point(-3.0, 1.0),
                    point(2.0, 4.0),
                    point(2.0, 7.0),
                ],
                stroke,
            ));
            line((6.0, -2.0), (6.0, 4.0));
            line((3.0, 1.0), (9.0, 1.0));
        }
        Icon::People => {
            p.circle_stroke(point(-3.0, -4.0), 2.8, stroke);
            p.add(egui::Shape::line(
                vec![
                    point(-8.5, 7.0),
                    point(-8.5, 4.2),
                    point(-3.0, 1.5),
                    point(2.5, 4.2),
                    point(2.5, 7.0),
                ],
                stroke,
            ));
            p.circle_stroke(point(4.5, -3.2), 2.2, stroke);
            p.add(egui::Shape::line(
                vec![point(4.5, 1.5), point(8.5, 3.8), point(8.5, 6.5)],
                stroke,
            ));
        }
        Icon::ChevronUp => {
            p.add(egui::Shape::line(
                vec![point(-5.0, 2.0), point(0.0, -3.0), point(5.0, 2.0)],
                stroke,
            ));
        }
        Icon::Settings => {
            let points = (0..48)
                .map(|i| {
                    let a = i as f32 * std::f32::consts::TAU / 48.0;
                    let radius = if matches!(i % 6, 1..=3) { 8.0 } else { 6.2 };
                    point(a.cos() * radius, a.sin() * radius)
                })
                .collect();
            p.add(egui::Shape::closed_line(points, stroke));
            p.circle_stroke(c, rect.width() * 0.15, stroke);
        }
    }
}

pub(super) fn paint_keyboard_focus(ctx: &egui::Context) {
    let Some(id) = ctx.memory(|memory| memory.focused()) else {
        return;
    };
    let Some(response) = ctx.read_response(id) else {
        return;
    };
    if !response.enabled() || !response.interact_rect.is_positive() {
        return;
    }
    let painter = ctx
        .layer_painter(response.layer_id)
        .with_clip_rect(response.interact_rect);
    // The dark and light borders keep the indicator visible on both accent and neutral fills.
    for (inset, color) in [(0.0, palette().accent), (2.0, palette().on_accent)] {
        painter.rect_stroke(
            response.rect.shrink(inset),
            6.0 - inset,
            egui::Stroke::new(2.0, color),
            egui::StrokeKind::Inside,
        );
    }
}

pub fn icon_button(ui: &mut egui::Ui, symbol: Icon, label: &str) -> egui::Response {
    let response = ui.add_sized([32.0, 32.0], egui::Button::new("").frame(false));
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(response.rect, 8.0, palette().control_hover);
    }
    icon(
        ui.painter(),
        egui::Rect::from_center_size(response.rect.center(), egui::vec2(17.0, 17.0)),
        symbol,
        if ui.is_enabled() {
            palette().text
        } else {
            palette().muted
        },
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    response.on_hover_text(label)
}

pub fn layout_button(
    ui: &mut egui::Ui,
    symbol: Icon,
    caption: Option<&str>,
    label: &str,
    selected: bool,
) -> egui::Response {
    let ink = if selected {
        palette().accent
    } else {
        palette().muted
    };
    let text = caption.map(|caption| {
        ui.painter()
            .layout_no_wrap(caption.to_owned(), semibold(12.5), ink)
    });
    let response = ui.add_sized(
        [
            text.as_ref().map_or(32.0, |text| text.size().x + 38.0),
            32.0,
        ],
        egui::Button::new("").frame(false),
    );
    if selected {
        ui.painter()
            .rect_filled(response.rect, 8.0, palette().accent.gamma_multiply(0.14));
        ui.painter().rect_stroke(
            response.rect,
            8.0,
            egui::Stroke::new(1.0, palette().accent.gamma_multiply(0.65)),
            egui::StrokeKind::Inside,
        );
    } else if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(response.rect, 8.0, palette().control_hover);
    }
    let center = response.rect.left_center() + egui::vec2(15.0, 0.0);
    icon(
        ui.painter(),
        egui::Rect::from_center_size(center, egui::vec2(15.0, 15.0)),
        symbol,
        ink,
    );
    if let Some(text) = text {
        ui.painter()
            .galley(center + egui::vec2(13.0, -text.size().y / 2.0), text, ink);
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label)
    });
    response.on_hover_text(label)
}

pub fn menu_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(palette().card)
        .stroke(egui::Stroke::new(1.0, palette().border))
        .corner_radius(12)
        .inner_margin(egui::Margin::same(8))
        .shadow(egui::epaint::Shadow {
            offset: [0, 10],
            blur: 28,
            spread: 2,
            color: Color32::from_black_alpha(145),
        })
}

pub fn dock_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(palette().panel)
        .stroke(egui::Stroke::new(1.0, palette().border))
        .corner_radius(22)
        .inner_margin(egui::Margin::symmetric(10, 7))
        .shadow(egui::epaint::Shadow {
            offset: [0, 5],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(if palette().dark { 120 } else { 62 }),
        })
}

pub fn outline_button(ui: &mut egui::Ui, label: &str, symbol: Icon) -> egui::Response {
    let response = ui.add(
        egui::Button::new(strong(format!("    {label}"), 13.0).color(palette().accent))
            .min_size(egui::vec2(102.0, 32.0))
            .corner_radius(8)
            .stroke(egui::Stroke::new(
                1.0,
                palette().accent.gamma_multiply(0.55),
            ))
            .fill(palette().accent.gamma_multiply(0.08)),
    );
    icon(
        ui.painter(),
        egui::Rect::from_center_size(
            response.rect.left_center() + egui::vec2(16.0, 0.0),
            egui::vec2(16.0, 16.0),
        ),
        symbol,
        palette().accent,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    response
}

pub fn help_label(ui: &mut egui::Ui, label: &str, help: &str) -> egui::Response {
    ui.horizontal(|ui| {
        let response = ui.label(strong(label, 13.0).color(palette().text));
        let (rect, info) = ui.allocate_exact_size(egui::vec2(18.0, 20.0), egui::Sense::click());
        icon(
            ui.painter(),
            rect,
            Icon::Info,
            if info.has_focus() {
                palette().accent
            } else {
                palette().muted
            },
        );
        info.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Button,
                ui.is_enabled(),
                format!("{label} · {help}"),
            )
        });
        if info.has_focus() || info.clicked() {
            egui::Tooltip::for_widget(&info).show(|ui| ui.label(help));
        } else {
            info.on_hover_text(help);
        }
        response
    })
    .inner
}

pub fn disclosure(ui: &mut egui::Ui, label: &str, expanded: &mut bool) -> egui::Response {
    let mut response = ui.add_sized(
        [ui.available_width(), 42.0],
        egui::Button::new("")
            .fill(palette().card)
            .stroke(egui::Stroke::new(1.0, palette().border))
            .corner_radius(10),
    );
    if response.clicked()
        || (ui.is_enabled()
            && response.has_focus()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Space)))
    {
        *expanded = !*expanded;
        response.request_focus();
        response.mark_changed();
    }
    let rect = response.rect;
    icon(
        ui.painter(),
        egui::Rect::from_center_size(
            rect.left_center() + egui::vec2(22.0, 0.0),
            egui::vec2(18.0, 18.0),
        ),
        Icon::Settings,
        if *expanded {
            palette().accent
        } else {
            palette().muted
        },
    );
    ui.painter().text(
        rect.left_center() + egui::vec2(44.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        semibold(14.5),
        palette().text,
    );
    let c = rect.right_center() - egui::vec2(20.0, 0.0);
    let points = if *expanded {
        vec![
            c + egui::vec2(-4.0, -2.0),
            c + egui::vec2(0.0, 2.0),
            c + egui::vec2(4.0, -2.0),
        ]
    } else {
        vec![
            c + egui::vec2(-2.0, -4.0),
            c + egui::vec2(2.0, 0.0),
            c + egui::vec2(-2.0, 4.0),
        ]
    };
    ui.painter().add(egui::Shape::line(
        points,
        egui::Stroke::new(1.7, palette().muted),
    ));
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), *expanded, label)
    });
    response
}

pub fn source_row(ui: &mut egui::Ui, label: &str, selected: bool, window: bool) -> egui::Response {
    let response = ui.add_sized(
        [ui.available_width(), 40.0],
        egui::Button::new("").frame(false),
    );
    let rect = response.rect;
    if response.hovered() || response.has_focus() || selected {
        ui.painter().rect_filled(
            rect,
            8.0,
            if response.hovered() || response.has_focus() {
                palette().control_hover
            } else {
                palette().accent.gamma_multiply(0.12)
            },
        );
    }
    icon(
        ui.painter(),
        egui::Rect::from_center_size(
            rect.left_center() + egui::vec2(20.0, 0.0),
            egui::vec2(18.0, 18.0),
        ),
        if window { Icon::Window } else { Icon::Monitor },
        if selected {
            palette().accent
        } else {
            palette().text
        },
    );
    let mut job = egui::text::LayoutJob::simple(
        label.to_owned(),
        FontId::proportional(13.0),
        palette().text,
        (rect.width() - 76.0).max(20.0),
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('\u{2026}');
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    ui.painter().galley(
        rect.left_center() + egui::vec2(42.0, -galley.size().y / 2.0),
        galley,
        palette().text,
    );
    if selected {
        let c = rect.right_center() - egui::vec2(17.0, 0.0);
        ui.painter().add(egui::Shape::line(
            vec![
                c + egui::vec2(-4.0, 0.0),
                c + egui::vec2(-1.0, 3.0),
                c + egui::vec2(5.0, -4.0),
            ],
            egui::Stroke::new(1.8, palette().accent),
        ));
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label)
    });
    response
}

pub fn menu_row(
    ui: &mut egui::Ui,
    label: &str,
    symbol: Option<Icon>,
    shortcut: &str,
    danger: bool,
) -> egui::Response {
    let width = ui.available_width();
    let response = ui.add_sized([width, 34.0], egui::Button::new("").frame(false));
    let rect = response.rect;
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(
            rect,
            7.0,
            if danger {
                palette().danger.gamma_multiply(0.16)
            } else {
                palette().control_hover
            },
        );
    }
    let color = if danger {
        palette().error
    } else {
        palette().text
    };
    if let Some(symbol) = symbol {
        icon(
            ui.painter(),
            egui::Rect::from_center_size(
                rect.left_center() + egui::vec2(16.0, 0.0),
                egui::vec2(16.0, 16.0),
            ),
            symbol,
            color,
        );
    }
    ui.painter().text(
        rect.left_center() + egui::vec2(if symbol.is_some() { 34.0 } else { 10.0 }, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.5),
        color,
    );
    ui.painter().text(
        rect.right_center() - egui::vec2(10.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        shortcut,
        FontId::proportional(11.5),
        palette().muted,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    response
}

pub fn mute_switch(ui: &mut egui::Ui, label: &str, muted: &mut bool) -> egui::Response {
    let mut response = menu_row(ui, label, None, "", false);
    if response.clicked()
        || (ui.is_enabled()
            && response.has_focus()
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Space)))
    {
        response.request_focus();
        *muted = !*muted;
        response.mark_changed();
    }
    let track = egui::Rect::from_center_size(
        response.rect.right_center() - egui::vec2(24.0, 0.0),
        egui::vec2(30.0, 17.0),
    );
    ui.painter().rect_filled(
        track,
        9.0,
        if *muted {
            palette().accent
        } else {
            Color32::from_rgb(60, 60, 68)
        },
    );
    ui.painter().circle_filled(
        egui::pos2(
            if *muted {
                track.right() - 8.5
            } else {
                track.left() + 8.5
            },
            track.center().y,
        ),
        6.0,
        if *muted {
            palette().on_accent
        } else {
            palette().text
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Switch, ui.is_enabled(), *muted, label)
    });
    response
}

pub fn menu_separator(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [
            rect.left_center() + egui::vec2(6.0, 0.0),
            rect.right_center() - egui::vec2(6.0, 0.0),
        ],
        egui::Stroke::new(1.0, palette().border),
    );
    ui.add_space(4.0);
}

pub fn mint_slider<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    value: &mut N,
    range: std::ops::RangeInclusive<N>,
) -> egui::Response {
    let start = range.start().to_f64();
    let end = range.end().to_f64();
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().interact_size.y = 18.0;
            ui.add(egui::Slider::new(value, range).show_value(false))
        })
        .inner;
    let radius = response.rect.height() / 2.5;
    let fraction = if end > start {
        ((value.to_f64() - start) / (end - start)).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let x = egui::lerp(
        response.rect.left() + radius..=response.rect.right() - radius,
        fraction,
    );
    ui.painter().circle_filled(
        egui::pos2(x, response.rect.center().y),
        radius,
        if ui.is_enabled() {
            palette().text
        } else {
            palette().muted
        },
    );
    ui.painter().circle_stroke(
        egui::pos2(x, response.rect.center().y),
        radius,
        egui::Stroke::new(1.2, palette().accent),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_mute_switch_responds_to_pointer_and_keyboard() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut muted = false;
        let draw = |muted: &mut bool, events: Vec<egui::Event>| {
            let mut rect = egui::Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 300.0),
                    )),
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        ui.set_width(238.0);
                        rect = mute_switch(ui, "Mute", muted).rect;
                    });
                },
            );
            output.textures_delta.clear();
            rect
        };
        draw(&mut muted, vec![]);
        let rect = draw(&mut muted, vec![]);
        let pos = rect.center();
        for pressed in [true, false] {
            draw(
                &mut muted,
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
        assert!(muted, "Pointer click should mute");
        for pressed in [true, false] {
            draw(
                &mut muted,
                vec![egui::Event::Key {
                    key: egui::Key::Space,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Default::default(),
                }],
            );
        }
        assert!(!muted, "Space on the focused switch should unmute");
    }
}
#[cfg(test)]
mod focus_tests {
    use super::*;

    #[test]
    fn keyboard_outline_follows_focus_across_button_styles() {
        let ctx = egui::Context::default();
        for target in 0..3 {
            let mut rects = Vec::new();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                rects.clear();
                for (index, button) in [
                    egui::Button::new("Sign in").fill(palette().accent),
                    egui::Button::new("Show").frame(false),
                    egui::Button::new("Disconnect")
                        .stroke(egui::Stroke::new(1.0, palette().danger)),
                ]
                .into_iter()
                .enumerate()
                {
                    let response = ui.add(button);
                    if index == target {
                        response.request_focus();
                    }
                    rects.push(response.rect);
                }
                paint_keyboard_focus(ui.ctx());
            });
            output.textures_delta.clear();
            for (index, rect) in rects.iter().enumerate() {
                for (inset, color) in [(0.0, palette().accent), (2.0, palette().on_accent)] {
                    let outlined = output.shapes.iter().any(|shape| {
                        matches!(&shape.shape,
                        egui::Shape::Rect(painted) if painted.rect == rect.shrink(inset)
                            && painted.stroke == egui::Stroke::new(2.0, color))
                    });
                    assert_eq!(outlined, index == target);
                }
            }
        }
    }
}
