//! The formatted ("what you see is what you get") way of writing in Markdown fields.
//!
//! The text is still Markdown underneath, so nothing changes in the campaign file, the wiki
//! export or the Markdown view. While writing, bold looks bold, headings look like headings,
//! and the `**`, `#`, `[[…]]` marks are hidden — except on the line the cursor is on, so
//! they can still be edited (like Obsidian's live preview). A toolbar and Ctrl+B / Ctrl+I
//! add formatting without knowing any Markdown.

use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use egui::text::{CCursor, CCursorRange, LayoutJob, TextFormat};
use egui::text_edit::TextEditState;
use egui::{Color32, FontFamily, FontId, RichText, Stroke, Ui};

/// Font family with a real bold face (registered in `settings::apply_fonts`).
pub fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
}

/// The bold family if the fonts are installed yet (they arrive a frame after start-up),
/// else the normal one.
pub fn bold(ctx: &egui::Context) -> FontFamily {
    let f = bold_family();
    if ctx.fonts(|fonts| fonts.definitions().families.contains_key(&f)) {
        f
    } else {
        FontFamily::Proportional
    }
}

static SHOW_MARKDOWN: AtomicBool = AtomicBool::new(false);
static PREF_CHANGED: AtomicBool = AtomicBool::new(false);

/// Edit with the Markdown marks showing (plain text box) instead of formatted.
pub fn show_markdown() -> bool {
    SHOW_MARKDOWN.load(Ordering::Relaxed)
}
pub fn set_show_markdown(v: bool) {
    SHOW_MARKDOWN.store(v, Ordering::Relaxed);
}
/// The user flipped the toggle in an editor since the last call (so settings can be saved).
pub fn take_pref_changed() -> bool {
    PREF_CHANGED.swap(false, Ordering::Relaxed)
}
fn user_set_show_markdown(v: bool) {
    set_show_markdown(v);
    PREF_CHANGED.store(true, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// Parsing (a small, forgiving subset of Markdown, one line at a time)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct Fx {
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
    link: bool,
    /// Markdown punctuation (`**`, `# `, `[[`…), hidden unless the cursor is on the line.
    marker: bool,
    /// List bullets / numbers / checkboxes: always shown, dimmed.
    bullet: bool,
    /// The text of a ticked checklist item.
    done: bool,
    /// Drawn over when the line isn't being edited: 1 = bullet "•", 2 = checkbox.
    deco: u8,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Block {
    Normal,
    Heading(u8),
    Quote,
    Rule,
    Code,
}

/// Split one line into styled byte ranges (covering the whole line, in order).
fn parse_line(line: &str, in_fence: &mut bool) -> (Block, Vec<(Range<usize>, Fx)>) {
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") {
        *in_fence = !*in_fence;
        return (
            Block::Code,
            vec![(
                0..line.len(),
                Fx {
                    marker: true,
                    ..Default::default()
                },
            )],
        );
    }
    if *in_fence {
        return (
            Block::Code,
            vec![(
                0..line.len(),
                Fx {
                    code: true,
                    ..Default::default()
                },
            )],
        );
    }
    let mut out: Vec<(Range<usize>, Fx)> = Vec::new();
    let mut block = Block::Normal;
    let mut start = 0;
    let mut base = Fx::default();
    let indent = line.len() - trimmed.len();
    // block prefixes
    let hashes = trimmed.bytes().take_while(|b| *b == b'#').count();
    if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
        block = Block::Heading(hashes as u8);
        start = indent + hashes + 1;
        out.push((
            0..start,
            Fx {
                marker: true,
                ..Default::default()
            },
        ));
    } else if trimmed.starts_with("> ") || trimmed == ">" {
        block = Block::Quote;
        start = indent + 1 + usize::from(trimmed.len() > 1);
        out.push((
            0..start,
            Fx {
                marker: true,
                ..Default::default()
            },
        ));
    } else if trimmed.len() >= 3 && (trimmed.trim_end().chars().all(|c| c == '-') || trimmed.trim_end().chars().all(|c| c == '*')) {
        return (
            Block::Rule,
            vec![(
                0..line.len(),
                Fx {
                    bullet: true,
                    ..Default::default()
                },
            )],
        );
    } else if let Some(rest) = ["- ", "* ", "+ "].iter().find_map(|b| trimmed.strip_prefix(b)) {
        let lower = rest.get(..4).map(|s| s.to_ascii_lowercase());
        let bullet = Fx {
            bullet: true,
            ..Default::default()
        };
        if indent > 0 {
            out.push((0..indent, bullet));
        }
        if lower.as_deref() == Some("[ ] ") || lower.as_deref() == Some("[x] ") {
            // "- " hidden like a mark, "[ ]" drawn as a checkbox
            base.done = lower.as_deref() == Some("[x] ");
            out.push((
                indent..indent + 2,
                Fx {
                    marker: true,
                    ..Default::default()
                },
            ));
            out.push((indent + 2..indent + 5, Fx { deco: 2, ..bullet }));
            out.push((indent + 5..indent + 6, bullet));
            start = indent + 6;
        } else {
            out.push((indent..indent + 1, Fx { deco: 1, ..bullet }));
            out.push((indent + 1..indent + 2, bullet));
            start = indent + 2;
        }
    } else {
        let digits = trimmed.bytes().take_while(|b| b.is_ascii_digit()).count();
        if digits > 0 && (trimmed[digits..].starts_with(". ") || trimmed[digits..].starts_with(") ")) {
            start = indent + digits + 2;
            out.push((
                0..start,
                Fx {
                    bullet: true,
                    ..Default::default()
                },
            ));
        }
    }
    inline(line, start, base, &mut out);
    (block, out)
}

/// Inline marks: **bold** __bold__ *italic* _italic_ ~~strike~~ `code` [[link]] [text](url).
fn inline(line: &str, from: usize, base: Fx, out: &mut Vec<(Range<usize>, Fx)>) {
    let b = line.as_bytes();
    let mut fx = base;
    let mut i = from;
    let mut run = from; // start of the current plain run
    let push = |out: &mut Vec<(Range<usize>, Fx)>, r: Range<usize>, f: Fx| {
        if r.start < r.end {
            out.push((r, f));
        }
    };
    let marker = Fx {
        marker: true,
        ..Default::default()
    };
    while i < b.len() {
        let rest = &line[i..];
        let prev = if i == 0 { b' ' } else { b[i - 1] };
        // `code`
        if rest.starts_with('`') {
            if let Some(end) = rest[1..].find('`') {
                push(out, run..i, fx);
                push(out, i..i + 1, marker);
                push(out, i + 1..i + 1 + end, Fx { code: true, ..fx });
                push(out, i + 1 + end..i + 2 + end, marker);
                i += end + 2;
                run = i;
                continue;
            }
        }
        // [[wiki link]]
        if rest.starts_with("[[") {
            if let Some(end) = rest[2..].find("]]") {
                push(out, run..i, fx);
                push(out, i..i + 2, marker);
                push(out, i + 2..i + 2 + end, Fx { link: true, ..fx });
                push(out, i + 2 + end..i + 4 + end, marker);
                i += end + 4;
                run = i;
                continue;
            }
        }
        // [text](url) and ![alt](path)
        if rest.starts_with('[') || rest.starts_with("![") {
            let open = usize::from(rest.starts_with('!')) + 1;
            if let Some(close) = rest[open..].find("](") {
                let after = open + close + 2;
                if let Some(paren) = rest[after..].find(')') {
                    push(out, run..i, fx);
                    push(out, i..i + open, marker);
                    push(out, i + open..i + open + close, Fx { link: true, ..fx });
                    push(out, i + open + close..i + after + paren + 1, marker);
                    i += after + paren + 1;
                    run = i;
                    continue;
                }
            }
        }
        // ** __ ~~ (opening only if a closing one follows on the line)
        let mut toggled = false;
        for (mark, kind) in [("**", 0), ("__", 0), ("~~", 1)] {
            if rest.starts_with(mark) {
                let on = if kind == 0 { fx.bold } else { fx.strike };
                let next = rest[2..].chars().next();
                let can_open = !on && next.map(|c| !c.is_whitespace()).unwrap_or(false) && rest[2..].contains(mark);
                let can_close = on && !(prev as char).is_whitespace();
                if can_open || can_close {
                    push(out, run..i, fx);
                    push(out, i..i + 2, marker);
                    if kind == 0 {
                        fx.bold = !on;
                    } else {
                        fx.strike = !on;
                    }
                    i += 2;
                    run = i;
                    toggled = true;
                }
                break;
            }
        }
        if toggled {
            continue;
        }
        // * _ italic
        if (rest.starts_with('*') || rest.starts_with('_')) && !rest.starts_with("**") && !rest.starts_with("__") {
            let mark = &rest[..1];
            let next = rest[1..].chars().next();
            let word_inside = mark == "_" && (prev as char).is_alphanumeric(); // snake_case stays as is
            let can_open = !fx.italic && !word_inside && next.map(|c| !c.is_whitespace()).unwrap_or(false) && rest[1..].contains(mark);
            let can_close = fx.italic && !(prev as char).is_whitespace();
            if can_open || can_close {
                push(out, run..i, fx);
                push(out, i..i + 1, marker);
                fx.italic = !fx.italic;
                i += 1;
                run = i;
                continue;
            }
        }
        i += rest.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    push(out, run..b.len(), fx);
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// Extra space after each paragraph (line), as a fraction of the text size.
const PARA_GAP: f32 = 0.35;

/// Lay out `text` with formatting. Lines whose char range touches `reveal` show their marks.
fn layout(ui: &Ui, text: &str, wrap: f32, reveal: Option<Range<usize>>) -> LayoutJob {
    let v = ui.visuals();
    let body = egui::TextStyle::Body.resolve(ui.style());
    let mono = egui::TextStyle::Monospace.resolve(ui.style());
    let bold_fam = bold(ui.ctx());
    // (empty to start with: `append` adds the text piece by piece)
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let mut in_fence = false;
    let mut byte = 0;
    let mut chr = 0;
    for raw in text.split_inclusive('\n') {
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        let n_chars = raw.chars().count();
        let shown = reveal.as_ref().map(|r| r.start <= chr + n_chars && r.end >= chr).unwrap_or(false);
        let (block, spans) = parse_line(line, &mut in_fence);
        let size = match block {
            Block::Heading(1) => body.size * 1.6,
            Block::Heading(2) => body.size * 1.35,
            Block::Heading(3) => body.size * 1.15,
            _ => body.size,
        };
        for (r, fx) in spans {
            let mut f = TextFormat {
                font_id: FontId::new(size, body.family.clone()),
                color: v.text_color(),
                ..Default::default()
            };
            if fx.bold || matches!(block, Block::Heading(_)) {
                f.font_id.family = bold_fam.clone();
            }
            if fx.italic || block == Block::Quote {
                f.italics = true;
            }
            if block == Block::Quote {
                f.color = v.weak_text_color();
            }
            if fx.strike {
                f.strikethrough = Stroke::new(1.0_f32, f.color);
            }
            if fx.code || block == Block::Code {
                f.font_id = FontId::new(mono.size, mono.family.clone());
                f.background = v.faint_bg_color;
            }
            if fx.link {
                f.color = v.hyperlink_color;
            }
            if fx.done {
                f.color = v.weak_text_color();
                f.strikethrough = Stroke::new(1.0_f32, v.weak_text_color());
            }
            if fx.bullet {
                f.color = v.weak_text_color();
                f.font_id = FontId::new(size, body.family.clone());
                f.strikethrough = Stroke::NONE;
            }
            if fx.deco == 2 {
                // a checkbox is drawn over "[ ]", which needs a little room
                f.font_id = FontId::new(mono.size, mono.family.clone());
            }
            if fx.deco != 0 && !shown {
                f.color = Color32::TRANSPARENT;
            }
            if fx.marker {
                if shown {
                    f.color = v.weak_text_color().gamma_multiply(0.8);
                    f.font_id = FontId::new(size.min(body.size), body.family.clone());
                    f.italics = false;
                    f.strikethrough = Stroke::NONE;
                    f.background = Color32::TRANSPARENT;
                } else {
                    // hidden: keep the characters (the cursor still moves over them) but tiny
                    f.color = Color32::TRANSPARENT;
                    f.font_id = FontId::new(1.0, body.family.clone());
                    f.background = Color32::TRANSPARENT;
                    f.strikethrough = Stroke::NONE;
                }
            }
            // every format sits at the top of its row, so the extra height given to a line's
            // last character (below) becomes a small gap after the paragraph
            f.valign = egui::Align::TOP;
            let piece = &line[r.clone()];
            if r.end == line.len() && raw.ends_with('\n') && block != Block::Code && !piece.is_empty() {
                // split off the last character to carry the paragraph gap
                let last = piece.char_indices().last().map(|(i, _)| i).unwrap_or(0);
                if last > 0 {
                    job.append(&piece[..last], 0.0, f.clone());
                }
                let mut g = f.clone();
                // measured with the line's own text size (the last character may be a tiny
                // hidden mark)
                let row = ui.fonts_mut(|fo| fo.row_height(&FontId::new(size, body.family.clone())));
                g.line_height = Some(row + body.size * PARA_GAP);
                job.append(&piece[last..], 0.0, g);
            } else {
                job.append(piece, 0.0, f);
            }
        }
        if raw.ends_with('\n') {
            job.append(
                "\n",
                0.0,
                TextFormat {
                    font_id: FontId::new(size, body.family.clone()),
                    color: v.text_color(),
                    ..Default::default()
                },
            );
        }
        byte += raw.len();
        chr += n_chars;
    }
    let _ = byte;
    job
}

// ---------------------------------------------------------------------------
// Editing helpers (toolbar and shortcuts)
// ---------------------------------------------------------------------------

fn char_to_byte(s: &str, c: usize) -> usize {
    s.char_indices().nth(c).map(|(b, _)| b).unwrap_or(s.len())
}
fn byte_to_char(s: &str, b: usize) -> usize {
    s[..b.min(s.len())].chars().count()
}

/// Remember where the cursor was while the box had focus (clicking a toolbar button takes
/// the focus away, and the box may forget its selection). Call after drawing the box.
pub fn remember_selection(ctx: &egui::Context, id: egui::Id) {
    if ctx.memory(|m| m.has_focus(id)) {
        if let Some(r) = TextEditState::load(ctx, id).and_then(|s| s.cursor.char_range()) {
            ctx.data_mut(|d| d.insert_temp(id.with("last_sel"), r.as_sorted_char_range()));
        }
    }
}

/// Give the focus back to the box after a toolbar click (asked for again on the next frame,
/// because the click itself can take it away at the end of this one). Call before drawing.
pub fn keep_focus(ctx: &egui::Context, id: egui::Id) {
    let key = id.with("refocus");
    let n: u8 = ctx.data(|d| d.get_temp(key)).unwrap_or(0);
    if n > 0 {
        ctx.memory_mut(|m| m.request_focus(id));
        ctx.data_mut(|d| d.insert_temp(key, n - 1));
    }
}

/// Selection as byte range (end of text if the editor was never focused).
fn selection(ctx: &egui::Context, id: egui::Id, text: &str) -> Range<usize> {
    let focused = ctx.memory(|m| m.has_focus(id));
    let live = if focused {
        TextEditState::load(ctx, id)
            .and_then(|s| s.cursor.char_range())
            .map(|r| r.as_sorted_char_range())
    } else {
        None
    };
    let r = live.or_else(|| ctx.data(|d| d.get_temp::<Range<usize>>(id.with("last_sel"))));
    match r {
        Some(r) => char_to_byte(text, r.start)..char_to_byte(text, r.end),
        None => text.len()..text.len(),
    }
}

fn set_selection(ctx: &egui::Context, id: egui::Id, text: &str, r: Range<usize>) {
    let mut st = TextEditState::load(ctx, id).unwrap_or_default();
    let (a, b) = (byte_to_char(text, r.start), byte_to_char(text, r.end));
    st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
    st.store(ctx, id);
    ctx.data_mut(|d| d.insert_temp(id.with("last_sel"), a.min(b)..a.max(b)));
    ctx.memory_mut(|m| m.request_focus(id));
    ctx.data_mut(|d| d.insert_temp(id.with("refocus"), 2u8));
}

/// Wrap the selection in `mark` (or unwrap it if it's already wrapped).
fn toggle_wrap(text: &mut String, sel: Range<usize>, mark: &str) -> Range<usize> {
    let m = mark.len();
    let (s, e) = (sel.start, sel.end);
    // already wrapped just outside the selection
    if s >= m && e + m <= text.len() && &text[s - m..s] == mark && &text[e..e + m] == mark {
        text.replace_range(e..e + m, "");
        text.replace_range(s - m..s, "");
        return s - m..e - m;
    }
    // the selection includes the marks
    if e - s >= 2 * m && text[s..e].starts_with(mark) && text[s..e].ends_with(mark) {
        text.replace_range(e - m..e, "");
        text.replace_range(s..s + m, "");
        return s..e - 2 * m;
    }
    // don't wrap surrounding spaces ("bold " → "**bold** ")
    let inner = &text[s..e];
    let lead = inner.len() - inner.trim_start().len();
    let trail = inner.len() - inner.trim_end().len();
    let (s2, e2) = if lead + trail < inner.len() { (s + lead, e - trail) } else { (s, e) };
    text.insert_str(e2, mark);
    text.insert_str(s2, mark);
    s2 + m..e2 + m
}

/// Byte range of the whole lines touched by the selection.
fn line_span(text: &str, sel: &Range<usize>) -> Range<usize> {
    let start = text[..sel.start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = text[sel.end..].find('\n').map(|i| sel.end + i).unwrap_or(text.len());
    start..end
}

/// Remove any list / heading / quote prefix from a line; returns (indent, rest).
fn strip_prefix(line: &str) -> (&str, &str) {
    let t = line.trim_start();
    let indent = &line[..line.len() - t.len()];
    let hashes = t.bytes().take_while(|b| *b == b'#').count();
    if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
        return (indent, &t[hashes + 1..]);
    }
    for p in ["- [ ] ", "- [x] ", "- [X] ", "- ", "* ", "+ ", "> "] {
        if let Some(r) = t.strip_prefix(p) {
            return (indent, r);
        }
    }
    let d = t.bytes().take_while(|b| b.is_ascii_digit()).count();
    if d > 0 && (t[d..].starts_with(". ") || t[d..].starts_with(") ")) {
        return (indent, &t[d + 2..]);
    }
    (indent, t)
}

/// Give every selected line `prefix` (numbered when `prefix` is "1. "), or take it away if
/// they all have it already. Other prefixes are replaced.
fn toggle_lines(text: &mut String, sel: Range<usize>, prefix: &str) -> Range<usize> {
    let span = line_span(text, &sel);
    let lines: Vec<String> = text[span.clone()].split('\n').map(|s| s.to_string()).collect();
    let numbered = prefix == "1. ";
    let has = |l: &str| {
        let t = l.trim_start();
        if numbered {
            let d = t.bytes().take_while(|b| b.is_ascii_digit()).count();
            d > 0 && t[d..].starts_with(". ")
        } else {
            t.starts_with(prefix)
        }
    };
    let all = lines.iter().filter(|l| !l.trim().is_empty()).all(|l| has(l)) && lines.iter().any(|l| !l.trim().is_empty());
    let mut n = 0;
    let new: Vec<String> = lines
        .iter()
        .map(|l| {
            let (indent, rest) = strip_prefix(l);
            if all || (l.trim().is_empty() && lines.len() > 1) {
                format!("{indent}{rest}")
            } else {
                n += 1;
                let p = if numbered { format!("{n}. ") } else { prefix.to_string() };
                format!("{indent}{p}{rest}")
            }
        })
        .collect();
    let joined = new.join("\n");
    let len = joined.len();
    text.replace_range(span.clone(), &joined);
    // put the cursor at the end of the last changed line
    span.start + len..span.start + len
}

#[derive(Clone, Copy)]
enum Cmd {
    Wrap(&'static str),
    Lines(&'static str),
    Heading(u8),
    Link,
}

fn apply(text: &mut String, sel: Range<usize>, cmd: Cmd) -> Range<usize> {
    match cmd {
        Cmd::Wrap(m) => toggle_wrap(text, sel, m),
        Cmd::Lines(p) => toggle_lines(text, sel, p),
        Cmd::Heading(level) => {
            let span = line_span(text, &sel);
            let line = text[span.clone()].to_string();
            let (indent, rest) = strip_prefix(&line);
            let already = line.trim_start().starts_with(&format!("{} ", "#".repeat(level as usize)));
            let new = if level == 0 || already {
                format!("{indent}{rest}")
            } else {
                format!("{indent}{} {rest}", "#".repeat(level as usize))
            };
            let end = span.start + new.len();
            text.replace_range(span, &new);
            end..end
        }
        Cmd::Link => {
            if sel.is_empty() {
                text.insert_str(sel.start, "[[]]");
                sel.start + 2..sel.start + 2
            } else {
                toggle_wrap_pair(text, sel, "[[", "]]")
            }
        }
    }
}

fn toggle_wrap_pair(text: &mut String, sel: Range<usize>, open: &str, close: &str) -> Range<usize> {
    let (s, e) = (sel.start, sel.end);
    if s >= open.len() && e + close.len() <= text.len() && &text[s - open.len()..s] == open && &text[e..e + close.len()] == close {
        text.replace_range(e..e + close.len(), "");
        text.replace_range(s - open.len()..s, "");
        return s - open.len()..e - open.len();
    }
    text.insert_str(e, close);
    text.insert_str(s, open);
    s + open.len()..e + open.len()
}

fn run(ui: &Ui, id: egui::Id, text: &mut String, cmd: Cmd) {
    let sel = selection(ui.ctx(), id, text);
    let sel = sel.start.min(text.len())..sel.end.min(text.len());
    let new_sel = apply(text, sel, cmd);
    set_selection(ui.ctx(), id, text, new_sel);
}

/// The formatting toolbar. Returns true if the text changed. `id` is the text box's id.
pub fn toolbar(ui: &mut Ui, id: egui::Id, text: &mut String) -> bool {
    let before = text.len();
    let snapshot = text.clone();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let bf = bold(ui.ctx());
        let btn = |ui: &mut Ui, label: RichText, tip: &str| ui.add(egui::Button::new(label).min_size(egui::vec2(26.0, 0.0))).on_hover_text(tip).clicked();
        if btn(ui, RichText::new("B").family(bf.clone()), "Bold (Ctrl+B)") {
            run(ui, id, text, Cmd::Wrap("**"));
        }
        if btn(ui, RichText::new("I").italics(), "Italic (Ctrl+I)") {
            run(ui, id, text, Cmd::Wrap("*"));
        }
        if btn(ui, RichText::new("S").strikethrough(), "Strikethrough") {
            run(ui, id, text, Cmd::Wrap("~~"));
        }
        ui.menu_button(RichText::new("H").family(bf.clone()), |ui| {
            for (lvl, label, size) in [(1u8, "Big heading", 1.5f32), (2, "Heading", 1.25), (3, "Small heading", 1.1)] {
                let body = egui::TextStyle::Body.resolve(ui.style()).size;
                if ui.button(RichText::new(label).family(bf.clone()).size(body * size)).clicked() {
                    run(ui, id, text, Cmd::Heading(lvl));
                    ui.close();
                }
            }
            if ui.button("Normal text").clicked() {
                run(ui, id, text, Cmd::Heading(0));
                ui.close();
            }
        })
        .response
        .on_hover_text("Heading");
        ui.separator();
        if btn(ui, RichText::new("•"), "Bullet list") {
            run(ui, id, text, Cmd::Lines("- "));
        }
        if btn(ui, RichText::new("1."), "Numbered list") {
            run(ui, id, text, Cmd::Lines("1. "));
        }
        if btn(ui, RichText::new("☑"), "Checklist") {
            run(ui, id, text, Cmd::Lines("- [ ] "));
        }
        if btn(ui, RichText::new("❝"), "Quote") {
            run(ui, id, text, Cmd::Lines("> "));
        }
        ui.separator();
        if btn(ui, RichText::new("[[ ]]"), "Link to a card, NPC, place or wiki page by name") {
            run(ui, id, text, Cmd::Link);
        }
        if btn(ui, RichText::new("{ }").monospace(), "Code / exact text") {
            run(ui, id, text, Cmd::Wrap("`"));
        }
        ui.separator();
        let mut md = show_markdown();
        if ui
            .checkbox(&mut md, "Markdown")
            .on_hover_text("Show the Markdown marks while writing (a plain text box)")
            .changed()
        {
            user_set_show_markdown(md);
        }
    });
    text.len() != before || *text != snapshot
}

/// Ctrl+B / Ctrl+I while the box has focus. Call before drawing the box.
pub fn shortcuts(ui: &Ui, id: egui::Id, text: &mut String) -> bool {
    if !ui.ctx().memory(|m| m.has_focus(id)) {
        return false;
    }
    let (b, i) = ui.input_mut(|inp| {
        (
            inp.consume_key(egui::Modifiers::COMMAND, egui::Key::B),
            inp.consume_key(egui::Modifiers::COMMAND, egui::Key::I),
        )
    });
    if b {
        run(ui, id, text, Cmd::Wrap("**"));
    }
    if i {
        run(ui, id, text, Cmd::Wrap("*"));
    }
    b || i
}

/// Bullets and checkboxes to draw: (char range, kind, ticked, char range of the line).
fn decorations(text: &str) -> Vec<(Range<usize>, u8, bool, Range<usize>)> {
    let mut out = Vec::new();
    let mut in_fence = false;
    let mut chr = 0;
    for raw in text.split_inclusive('\n') {
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        let n = raw.chars().count();
        let (_, spans) = parse_line(line, &mut in_fence);
        for (r, fx) in spans {
            if fx.deco != 0 {
                let a = chr + line[..r.start].chars().count();
                let b = chr + line[..r.end].chars().count();
                let done = fx.deco == 2 && line[r.clone()].to_ascii_lowercase() == "[x]";
                out.push((a..b, fx.deco, done, chr..chr + n));
            }
        }
        chr += n;
    }
    out
}

/// What Enter should do on a list line: continue the list ("- ", "2. ", "- [ ] ") or, on an
/// empty item, end it. Returns (text to insert at the cursor, bytes to delete before it).
fn enter_in_list(text: &str, cursor: usize) -> Option<(String, usize)> {
    let line_start = text[..cursor].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = text[cursor..].find('\n').map(|i| cursor + i).unwrap_or(text.len());
    let line = &text[line_start..line_end];
    let t = line.trim_start();
    let indent = &line[..line.len() - t.len()];
    let prefix: String = if let Some(p) = ["- [ ] ", "- [x] ", "- [X] "].iter().find(|p| t.starts_with(**p)) {
        let _ = p;
        format!("{indent}- [ ] ")
    } else if let Some(p) = ["- ", "* ", "+ ", "> "].iter().find(|p| t.starts_with(**p)) {
        format!("{indent}{p}")
    } else {
        let d = t.bytes().take_while(|b| b.is_ascii_digit()).count();
        if d > 0 && (t[d..].starts_with(". ") || t[d..].starts_with(") ")) {
            let n: u64 = t[..d].parse().unwrap_or(0);
            format!("{indent}{}{}", n + 1, &t[d..d + 2])
        } else {
            return None;
        }
    };
    let own_prefix_len = line.len() - strip_prefix(line).1.len();
    if line[own_prefix_len..].trim().is_empty() && cursor >= line_start + own_prefix_len {
        // empty item: end the list (remove the bullet), like a word processor
        return Some((String::new(), cursor - line_start));
    }
    if cursor < line_start + own_prefix_len {
        return None; // cursor inside the bullet itself: plain newline
    }
    Some((format!("\n{prefix}"), 0))
}

/// Indent (or outdent) the list item the cursor is on by two spaces. None if not a list line.
fn indent_list_item(text: &mut String, sel: Range<usize>, indent: bool) -> Option<Range<usize>> {
    let line_start = text[..sel.start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = text[sel.start..].find('\n').map(|i| sel.start + i).unwrap_or(text.len());
    let line = &text[line_start..line_end];
    let t = line.trim_start();
    let d = t.bytes().take_while(|b| b.is_ascii_digit()).count();
    let is_list = ["- ", "* ", "+ "].iter().any(|p| t.starts_with(p)) || (d > 0 && (t[d..].starts_with(". ") || t[d..].starts_with(") ")));
    if !is_list {
        return None;
    }
    if indent {
        text.insert_str(line_start, "  ");
        Some(sel.start + 2..sel.end + 2)
    } else {
        let spaces = (line.len() - t.len()).min(2);
        if spaces == 0 {
            return Some(sel);
        }
        text.replace_range(line_start..line_start + spaces, "");
        Some(sel.start.saturating_sub(spaces).max(line_start)..sel.end.saturating_sub(spaces).max(line_start))
    }
}

/// The formatted text box.
pub fn formatted_edit(ui: &mut Ui, id: egui::Id, text: &mut String, rows: usize, hint: &str) -> egui::Response {
    let focused = ui.ctx().memory(|m| m.has_focus(id));
    // reveal the marks on the line(s) the cursor or selection is on
    let reveal = if focused {
        TextEditState::load(ui.ctx(), id)
            .and_then(|s| s.cursor.char_range())
            .map(|r| r.as_sorted_char_range())
    } else {
        None
    };
    // Enter on a list line continues the list (or ends it on an empty item)
    let mut list_enter = false;
    if focused && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.is_none()) {
        let sel = selection(ui.ctx(), id, text);
        if sel.is_empty() {
            if let Some((insert, delete)) = enter_in_list(text, sel.start) {
                ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                let at = sel.start - delete;
                text.replace_range(at..sel.start, &insert);
                let end = at + insert.len();
                set_selection(ui.ctx(), id, text, end..end);
                list_enter = true;
            }
        }
    }
    // Tab / Shift+Tab on a list line indents / outdents the item (nested lists)
    if focused && !list_enter {
        let (tab, shift) = ui.input(|i| (i.key_pressed(egui::Key::Tab), i.modifiers.shift));
        if tab {
            let sel = selection(ui.ctx(), id, text);
            if let Some(new_sel) = indent_list_item(text, sel.clone(), !shift) {
                ui.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
                    i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
                });
                set_selection(ui.ctx(), id, text, new_sel);
                list_enter = true;
            } else {
                // elsewhere a tab would start a Markdown code block: ignore it
                ui.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
                    i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
                });
            }
        }
    }
    let reveal = if list_enter {
        Some(selection(ui.ctx(), id, text)).map(|r| byte_to_char(text, r.start)..byte_to_char(text, r.end))
    } else {
        reveal
    };
    let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| -> Arc<egui::Galley> {
        let job = layout(ui, buf.as_str(), wrap, reveal.clone());
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let mut out = egui::TextEdit::multiline(text)
        .id(id)
        .lock_focus(true)
        .desired_rows(rows)
        .desired_width(f32::INFINITY)
        .hint_text(hint)
        .layouter(&mut layouter)
        .show(ui);
    if list_enter {
        out.response.mark_changed();
    }
    // draw bullets and checkboxes on lines that aren't being edited
    let now_focused = ui.ctx().memory(|m| m.has_focus(id));
    let shown = if now_focused {
        out.cursor_range.map(|r| r.as_sorted_char_range())
    } else {
        None
    };
    let decos = decorations(text);
    let painter = ui.painter_at(out.text_clip_rect);
    let v = ui.visuals().clone();
    let mut toggle: Option<usize> = None;
    let clicked_at = if out.response.clicked() {
        out.cursor_range.map(|r| r.primary.index)
    } else {
        None
    };
    for (chars, kind, done, line) in &decos {
        // a click on a checkbox ticks it
        if *kind == 2 {
            if let Some(c) = clicked_at {
                if c >= chars.start && c <= chars.end {
                    toggle = Some(chars.start + 1);
                }
            }
        }
        let editing = shown.as_ref().map(|r| r.start <= line.end && r.end >= line.start).unwrap_or(false);
        if editing {
            continue;
        }
        let a = out.galley.pos_from_cursor(CCursor::new(chars.start)).translate(out.galley_pos.to_vec2());
        let b = out.galley.pos_from_cursor(CCursor::new(chars.end)).translate(out.galley_pos.to_vec2());
        // the row may include the paragraph gap below the text: centre on the text itself
        let text_h = ui.fonts_mut(|f| f.row_height(&egui::TextStyle::Body.resolve(ui.style())));
        let rect = egui::Rect::from_min_max(a.min, egui::pos2(b.max.x, a.min.y + text_h.min(a.height())));
        if *kind == 1 {
            // nested items get a hollow bullet, as in Preview
            let nested = chars.start > line.start;
            let r = (rect.height() * 0.13).max(2.0);
            if nested {
                painter.circle_stroke(rect.center(), r, Stroke::new(1.0_f32, v.text_color()));
            } else {
                painter.circle_filled(rect.center(), r, v.text_color());
            }
        } else {
            let side = (rect.height() * 0.62).min(rect.width());
            let bx = egui::Rect::from_center_size(rect.center(), egui::vec2(side, side));
            let stroke = Stroke::new(1.3_f32, if *done { v.hyperlink_color } else { v.weak_text_color() });
            painter.rect(
                bx,
                2.0,
                if *done {
                    v.hyperlink_color.gamma_multiply(0.25)
                } else {
                    Color32::TRANSPARENT
                },
                stroke,
                egui::StrokeKind::Inside,
            );
            if *done {
                let p = |x: f32, y: f32| bx.min + egui::vec2(bx.width() * x, bx.height() * y);
                painter.line_segment([p(0.22, 0.52), p(0.43, 0.74)], Stroke::new(1.8_f32, v.hyperlink_color));
                painter.line_segment([p(0.43, 0.74), p(0.8, 0.27)], Stroke::new(1.8_f32, v.hyperlink_color));
            }
        }
    }
    if let Some(ci) = toggle {
        let b = char_to_byte(text, ci);
        if let Some(ch) = text[b..].chars().next() {
            let new = if ch == ' ' { "x" } else { " " };
            text.replace_range(b..b + ch.len_utf8(), new);
            out.response.mark_changed();
        }
    }
    out.response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(line: &str) -> Vec<(String, Fx)> {
        let mut fence = false;
        parse_line(line, &mut fence).1.into_iter().map(|(r, f)| (line[r].to_string(), f)).collect()
    }

    #[test]
    fn spans_cover_the_line() {
        for line in [
            "plain",
            "**bold** and *it*",
            "# Head **x**",
            "- [x] done",
            "snake_case_name",
            "a ** b",
            "[[Baron]] at [site](http://x)",
            "`co*de`",
            "1. one",
            "> quote",
            "unclosed **bold",
        ] {
            let mut fence = false;
            let (_, spans) = parse_line(line, &mut fence);
            let mut pos = 0;
            for (r, _) in &spans {
                assert_eq!(r.start, pos, "{line}");
                pos = r.end;
            }
            assert_eq!(pos, line.len(), "{line}");
        }
    }

    #[test]
    fn recognises_marks() {
        let k = kinds("**bold** and *it*");
        assert!(k.iter().any(|(t, f)| t == "bold" && f.bold));
        assert!(k.iter().any(|(t, f)| t == "it" && f.italic));
        assert!(k.iter().filter(|(_, f)| f.marker).count() == 4);
        assert!(!kinds("snake_case_name").iter().any(|(_, f)| f.italic));
        assert!(!kinds("a ** b").iter().any(|(_, f)| f.bold));
        assert!(!kinds("unclosed **bold").iter().any(|(_, f)| f.bold));
        assert!(kinds("[[Baron]] x").iter().any(|(t, f)| t == "Baron" && f.link));
        assert!(kinds("see [site](http://x)").iter().any(|(t, f)| t == "site" && f.link));
        assert!(kinds("- [x] done").iter().any(|(t, f)| t == "done" && f.done));
    }

    #[test]
    fn toolbar_commands() {
        let mut t = "make this bold".to_string();
        let r = apply(&mut t, 10..14, Cmd::Wrap("**"));
        assert_eq!(t, "make this **bold**");
        assert_eq!(&t[r.clone()], "bold");
        let r = apply(&mut t, r, Cmd::Wrap("**"));
        assert_eq!(t, "make this bold");
        assert_eq!(&t[r], "bold");
        // trailing space stays outside
        let mut t = "a word here".to_string();
        apply(&mut t, 2..7, Cmd::Wrap("*"));
        assert_eq!(t, "a *word* here");
        // lists
        let mut t = "one\ntwo\nthree".to_string();
        let n = t.len();
        apply(&mut t, 0..n, Cmd::Lines("- "));
        assert_eq!(t, "- one\n- two\n- three");
        let n = t.len();
        apply(&mut t, 0..n, Cmd::Lines("1. "));
        assert_eq!(t, "1. one\n2. two\n3. three");
        let n = t.len();
        apply(&mut t, 0..n, Cmd::Lines("1. "));
        assert_eq!(t, "one\ntwo\nthree");
        // headings
        let mut t = "intro\ntitle".to_string();
        apply(&mut t, 8..8, Cmd::Heading(2));
        assert_eq!(t, "intro\n## title");
        apply(&mut t, 8..8, Cmd::Heading(2));
        assert_eq!(t, "intro\ntitle");
        let mut t = "the Baron".to_string();
        apply(&mut t, 4..9, Cmd::Link);
        assert_eq!(t, "the [[Baron]]");
    }

    #[test]
    fn enter_continues_lists() {
        let t = "- one";
        assert_eq!(enter_in_list(t, t.len()), Some(("\n- ".to_string(), 0)));
        let t = "  3. three";
        assert_eq!(enter_in_list(t, t.len()), Some(("\n  4. ".to_string(), 0)));
        let t = "- [x] done";
        assert_eq!(enter_in_list(t, t.len()), Some(("\n- [ ] ".to_string(), 0)));
        // an empty item ends the list
        let t = "- one\n- ";
        assert_eq!(enter_in_list(t, t.len()), Some((String::new(), 2)));
        assert_eq!(enter_in_list("plain text", 10), None);
        assert_eq!(enter_in_list("## Heading", 10), None);
    }

    #[test]
    fn bullets_and_boxes_are_found() {
        let d = decorations("intro\n- a\n- [x] b\n```\n- not\n```");
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].0.clone(), d[0].1), (6..7, 1));
        assert_eq!((d[1].0.clone(), d[1].1, d[1].2), (12..15, 2, true));
    }

    #[test]
    fn tab_nests_list_items() {
        let mut t = "- a\n- b".to_string();
        let r = indent_list_item(&mut t, 7..7, true).unwrap();
        assert_eq!(t, "- a\n  - b");
        assert_eq!(r, 9..9);
        indent_list_item(&mut t, 9..9, false).unwrap();
        assert_eq!(t, "- a\n- b");
        assert!(indent_list_item(&mut "plain".to_string(), 2..2, true).is_none());
    }
}
