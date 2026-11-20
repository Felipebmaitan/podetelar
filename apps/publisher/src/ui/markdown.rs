//! Minimal zero-dependency Markdown renderer for release notes and changelogs.
use super::{ACCENT, BORDER, CANVAS, MUTED, TEXT, skin};
use eframe::egui::{self, Color32, FontFamily, FontId, TextFormat, text::LayoutJob};

#[derive(Debug, PartialEq, Eq)]
enum Block {
    Heading {
        level: u8,
        text: String,
    },
    Bullet {
        indent: usize,
        text: String,
    },
    Ordered {
        indent: usize,
        number: String,
        text: String,
    },
    Quote(String),
    Code(String),
    Rule,
    Paragraph(String),
}

#[derive(Debug, PartialEq, Eq)]
enum Span {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
    Link { label: String, url: String },
}

pub(super) fn render(ui: &mut egui::Ui, markdown: &str) {
    let blocks = parse_blocks(markdown);
    ui.spacing_mut().item_spacing.y = 3.0;
    let mut prev_was_list = false;
    for (index, block) in blocks.iter().enumerate() {
        let is_list = matches!(block, Block::Bullet { .. } | Block::Ordered { .. });
        if index > 0 {
            let space = match block {
                Block::Heading { .. } => 10.0,
                Block::Rule => 6.0,
                _ if is_list && prev_was_list => 3.0,
                _ => 7.0,
            };
            ui.add_space(space);
        }
        match block {
            Block::Heading { level, text } => {
                let size = match level {
                    1 => 16.5,
                    2 => 14.5,
                    3 => 13.5,
                    _ => 13.0,
                };
                render_inline(ui, text, size, true, TEXT);
            }
            Block::Bullet { indent, text } => {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 7.0;
                    if *indent > 0 {
                        ui.add_space((*indent as f32) * 14.0);
                    }
                    ui.label(egui::RichText::new("•").size(13.0).color(ACCENT));
                    ui.vertical(|ui| {
                        render_inline(ui, text, 13.0, false, MUTED);
                    });
                });
            }
            Block::Ordered {
                indent,
                number,
                text,
            } => {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if *indent > 0 {
                        ui.add_space((*indent as f32) * 14.0);
                    }
                    ui.label(skin::strong(format!("{number}."), 12.5).color(ACCENT));
                    ui.vertical(|ui| {
                        render_inline(ui, text, 13.0, false, MUTED);
                    });
                });
            }
            Block::Quote(text) => {
                egui::Frame::new()
                    .fill(CANVAS.gamma_multiply(0.6))
                    .inner_margin(egui::Margin {
                        left: 10,
                        right: 10,
                        top: 6,
                        bottom: 6,
                    })
                    .corner_radius(6)
                    .show(ui, |ui| {
                        let rect = ui.max_rect();
                        ui.painter().line_segment(
                            [
                                egui::pos2(rect.left() - 10.0, rect.top() - 4.0),
                                egui::pos2(rect.left() - 10.0, rect.bottom() + 4.0),
                            ],
                            egui::Stroke::new(2.5, ACCENT.gamma_multiply(0.7)),
                        );
                        render_inline(ui, text, 12.5, false, MUTED);
                    });
            }
            Block::Code(code) => {
                egui::Frame::new()
                    .fill(CANVAS)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .corner_radius(7)
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.set_width((ui.available_width() - 2.0).max(80.0));
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(code)
                                    .font(FontId::monospace(12.0))
                                    .color(TEXT),
                            )
                            .wrap(),
                        );
                    });
            }
            Block::Rule => {
                ui.separator();
            }
            Block::Paragraph(text) => {
                render_inline(ui, text, 13.0, false, MUTED);
            }
        }
        prev_was_list = is_list;
    }
}

fn render_inline(ui: &mut egui::Ui, text: &str, size: f32, heading: bool, base_color: Color32) {
    let spans = parse_inline(text);
    let has_links = spans.iter().any(|s| matches!(s, Span::Link { .. }));
    if !has_links {
        let mut job = LayoutJob::default();
        job.wrap.max_width = ui.available_width();
        for span in spans {
            append_span(&mut job, &span, size, heading, base_color);
        }
        ui.add(egui::Label::new(job).wrap());
        return;
    }

    // When inline links are present, flow spans horizontally with wrapping so
    // each link remains individually clickable while preserving styling.
    ui.horizontal_wrapped(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        ui.spacing_mut().interact_size.y = size;
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
        let mut buffer = LayoutJob::default();
        let flush = |ui: &mut egui::Ui, job: &mut LayoutJob| {
            if !job.is_empty() {
                let finished = std::mem::take(job);
                ui.add(egui::Label::new(finished).wrap());
            }
        };
        for span in spans {
            match span {
                Span::Link { ref label, ref url } => {
                    flush(ui, &mut buffer);
                    let safe = url.starts_with("https://") || url.starts_with("http://");
                    let mut rich = LayoutJob::default();
                    append_span(&mut rich, &span, size, heading, base_color);
                    if safe {
                        if ui.add(egui::Link::new(rich)).on_hover_text(url).clicked() {
                            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                        }
                    } else {
                        ui.label(label);
                    }
                }
                other => append_span(&mut buffer, &other, size, heading, base_color),
            }
        }
        flush(ui, &mut buffer);
    });
}

fn append_span(job: &mut LayoutJob, span: &Span, size: f32, heading: bool, base_color: Color32) {
    let base_font = if heading {
        skin::semibold(size)
    } else {
        FontId::proportional(size)
    };
    match span {
        Span::Text(s) => job.append(
            s,
            0.0,
            TextFormat {
                font_id: base_font,
                color: base_color,
                ..Default::default()
            },
        ),
        Span::Bold(s) => job.append(
            s,
            0.0,
            TextFormat {
                font_id: skin::semibold(size),
                color: TEXT,
                ..Default::default()
            },
        ),
        Span::Italic(s) => job.append(
            s,
            0.0,
            TextFormat {
                font_id: base_font,
                color: TEXT.gamma_multiply(0.92),
                italics: true,
                ..Default::default()
            },
        ),
        Span::Code(s) => job.append(
            &format!(" {s} "),
            0.0,
            TextFormat {
                font_id: FontId::new((size - 1.0).max(11.0), FontFamily::Monospace),
                color: TEXT,
                background: skin::SURFACE_3,
                ..Default::default()
            },
        ),
        Span::Link { label, .. } => job.append(
            label,
            0.0,
            TextFormat {
                font_id: base_font,
                color: ACCENT,
                underline: egui::Stroke::new(1.0, ACCENT),
                ..Default::default()
            },
        ),
    }
}

fn parse_blocks(input: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut paragraph = Vec::new();
    let mut in_code = false;
    let mut code_lines = Vec::new();

    let flush_paragraph = |paragraph: &mut Vec<String>, blocks: &mut Vec<Block>| {
        if !paragraph.is_empty() {
            blocks.push(Block::Paragraph(paragraph.join(" ")));
            paragraph.clear();
        }
    };

    for raw_line in input.lines() {
        let trimmed_end = raw_line.trim_end();
        let trimmed = trimmed_end.trim();

        if trimmed.starts_with("```") {
            flush_paragraph(&mut paragraph, &mut blocks);
            if in_code {
                blocks.push(Block::Code(code_lines.join("\n")));
                code_lines.clear();
                in_code = false;
            } else {
                in_code = true;
            }
            continue;
        }
        if in_code {
            code_lines.push(trimmed_end.to_owned());
            continue;
        }
        if trimmed.is_empty() {
            flush_paragraph(&mut paragraph, &mut blocks);
            continue;
        }
        if is_horizontal_rule(trimmed) {
            flush_paragraph(&mut paragraph, &mut blocks);
            blocks.push(Block::Rule);
            continue;
        }
        if let Some((level, heading)) = parse_heading(trimmed) {
            flush_paragraph(&mut paragraph, &mut blocks);
            blocks.push(Block::Heading {
                level,
                text: heading.to_owned(),
            });
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("> ") {
            flush_paragraph(&mut paragraph, &mut blocks);
            blocks.push(Block::Quote(rest.trim().to_owned()));
            continue;
        }
        let leading_spaces = raw_line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();
        let indent = leading_spaces / 2;
        if let Some(item) = parse_bullet(trimmed) {
            flush_paragraph(&mut paragraph, &mut blocks);
            blocks.push(Block::Bullet {
                indent,
                text: item.to_owned(),
            });
            continue;
        }
        if let Some((number, item)) = parse_ordered(trimmed) {
            flush_paragraph(&mut paragraph, &mut blocks);
            blocks.push(Block::Ordered {
                indent,
                number: number.to_owned(),
                text: item.to_owned(),
            });
            continue;
        }
        paragraph.push(trimmed.to_owned());
    }

    if in_code && !code_lines.is_empty() {
        blocks.push(Block::Code(code_lines.join("\n")));
    }
    flush_paragraph(&mut paragraph, &mut blocks);
    blocks
}

fn is_horizontal_rule(line: &str) -> bool {
    let chars: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    chars.len() >= 3 && matches!(chars[0], '-' | '*' | '_') && chars.iter().all(|c| *c == chars[0])
}

fn parse_heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    if (1..=6).contains(&hashes) {
        let rest = &line[hashes..];
        if let Some(text) = rest.strip_prefix(' ') {
            return Some((hashes as u8, text.trim().trim_end_matches('#').trim()));
        }
    }
    None
}

fn parse_bullet(line: &str) -> Option<&str> {
    for prefix in ["- ", "* ", "+ ", "• "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Some(rest.trim());
        }
    }
    None
}

fn parse_ordered(line: &str) -> Option<(&str, &str)> {
    let digits = line.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let rest = &line[digits..];
    let item = rest
        .strip_prefix(". ")
        .or_else(|| rest.strip_prefix(") "))?;
    Some((&line[..digits], item.trim()))
}

fn parse_inline(input: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut text = String::new();
    let bytes = input.as_bytes();
    let mut i = 0;

    let push_text = |text: &mut String, spans: &mut Vec<Span>| {
        if !text.is_empty() {
            spans.push(Span::Text(std::mem::take(text)));
        }
    };

    while i < bytes.len() {
        // Escape sequences like \* or \`
        if bytes[i] == b'\\'
            && i + 1 < bytes.len()
            && let Some(ch) = input[i + 1..].chars().next()
        {
            text.push(ch);
            i += 1 + ch.len_utf8();
            continue;
        }
        // Inline code: `code`
        if bytes[i] == b'`'
            && let Some(end) = input[i + 1..].find('`')
            && end > 0
        {
            push_text(&mut text, &mut spans);
            spans.push(Span::Code(input[i + 1..i + 1 + end].to_owned()));
            i += end + 2;
            continue;
        }
        // Links: [label](url)
        if bytes[i] == b'['
            && let Some(close_bracket) = input[i + 1..].find("](")
        {
            let url_start = i + 1 + close_bracket + 2;
            if let Some(close_paren) = input[url_start..].find(')') {
                let label = &input[i + 1..i + 1 + close_bracket];
                let url = &input[url_start..url_start + close_paren];
                if !label.is_empty() && !url.is_empty() {
                    push_text(&mut text, &mut spans);
                    spans.push(Span::Link {
                        label: label.to_owned(),
                        url: url.to_owned(),
                    });
                    i = url_start + close_paren + 1;
                    continue;
                }
            }
        }
        // Bold: **text** or __text__
        if (input[i..].starts_with("**") || input[i..].starts_with("__"))
            && let Some(delim) = input.get(i..i + 2)
            && let Some(end) = input[i + 2..].find(delim)
            && end > 0
        {
            push_text(&mut text, &mut spans);
            spans.push(Span::Bold(input[i + 2..i + 2 + end].to_owned()));
            i += end + 4;
            continue;
        }
        // Italic: *text* or _text_
        if (bytes[i] == b'*' || bytes[i] == b'_') && i + 1 < bytes.len() {
            let marker = bytes[i] as char;
            // Avoid treating snake_case underscores mid-word as italics.
            let prev_is_word = i > 0
                && input[..i]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_alphanumeric);
            if !(marker == '_' && prev_is_word)
                && let Some(end) = input[i + 1..].find(marker)
                && end > 0
            {
                let inner = &input[i + 1..i + 1 + end];
                if !inner.starts_with(' ') && !inner.ends_with(' ') {
                    push_text(&mut text, &mut spans);
                    spans.push(Span::Italic(inner.to_owned()));
                    i += end + 2;
                    continue;
                }
            }
        }
        if let Some(ch) = input[i..].chars().next() {
            text.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    push_text(&mut text, &mut spans);
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_headings_lists_rules_quotes_and_code_blocks() {
        let md = "\
## What's new

- **Cleaner UI** with `AV1` support
  - Nested *detail*
1. First step
2. Second step

> Note for testers

```
podetelar --version
```

---

See [release](https://example.com/releases/0.1.5).";
        let blocks = parse_blocks(md);
        assert_eq!(
            blocks,
            vec![
                Block::Heading {
                    level: 2,
                    text: "What's new".into(),
                },
                Block::Bullet {
                    indent: 0,
                    text: "**Cleaner UI** with `AV1` support".into(),
                },
                Block::Bullet {
                    indent: 1,
                    text: "Nested *detail*".into(),
                },
                Block::Ordered {
                    indent: 0,
                    number: "1".into(),
                    text: "First step".into(),
                },
                Block::Ordered {
                    indent: 0,
                    number: "2".into(),
                    text: "Second step".into(),
                },
                Block::Quote("Note for testers".into()),
                Block::Code("podetelar --version".into()),
                Block::Rule,
                Block::Paragraph("See [release](https://example.com/releases/0.1.5).".into()),
            ]
        );
    }

    #[test]
    fn parses_inline_formatting_without_breaking_snake_case() {
        let spans = parse_inline(
            "Fixed **capture_rate** in `nvenc_av1` with *smooth* [docs](https://example.com).",
        );
        assert_eq!(
            spans,
            vec![
                Span::Text("Fixed ".into()),
                Span::Bold("capture_rate".into()),
                Span::Text(" in ".into()),
                Span::Code("nvenc_av1".into()),
                Span::Text(" with ".into()),
                Span::Italic("smooth".into()),
                Span::Text(" ".into()),
                Span::Link {
                    label: "docs".into(),
                    url: "https://example.com".into(),
                },
                Span::Text(".".into()),
            ]
        );
    }
}
