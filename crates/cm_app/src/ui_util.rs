//! Reusable widgets: markdown editor, optional number fields, dates, link chips and
//! pickers, image thumbnails and file dialogs.

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use cm_core::calendar::{GameCalendar, GameDate};
use cm_core::storage;
use cm_core::wiki::Vault;
use cm_core::{Campaign, Link};
use egui::{Color32, RichText, Ui};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use crate::app::{Action, Shared};

pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MdMode {
    Edit,
    Preview,
    Split,
}

/// Render markdown, turning `[[wiki links]]` into clickable links.
pub fn render_markdown(ui: &mut Ui, cache: &mut CommonMarkCache, text: &str) {
    if text.trim().is_empty() {
        ui.weak("(empty)");
        return;
    }
    let converted = cm_core::wiki::wikilinks_to_markdown(text);
    // egui_commonmark lays text out at max(max_image_width, available width), so a fixed
    // image width wider than the column makes every paragraph overflow. Keep it in bounds.
    let w = ui.available_width().max(60.0);
    let img = w.min(600.0) as usize;
    CommonMarkViewer::new().max_image_width(Some(img)).show(ui, cache, &converted);
}

/// A markdown field with Edit / Preview / Split modes (mode remembered per field).
pub fn markdown_editor(ui: &mut Ui, id_salt: impl std::hash::Hash, text: &mut String, cache: &mut CommonMarkCache, rows: usize) -> bool {
    let id = ui.make_persistent_id(("md", id_salt));
    let mut mode = ui
        .data(|d| d.get_temp::<MdMode>(id))
        .unwrap_or(if text.trim().is_empty() { MdMode::Edit } else { MdMode::Preview });
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.selectable_value(&mut mode, MdMode::Edit, "Edit");
        ui.selectable_value(&mut mode, MdMode::Preview, "Preview");
        ui.selectable_value(&mut mode, MdMode::Split, "Split");
        ui.add(egui::Label::new(RichText::new("Markdown · [[Name]] links to cards, NPCs or wiki pages").weak()).truncate());
    });
    // An explicit id: egui keeps a text box's undo history by id, and without one every
    // record's editor would share it (Ctrl+Z could paste another card's text in here).
    let te_id = id.with("te");
    let edit = |ui: &mut Ui, text: &mut String| -> bool {
        ui.add(
            egui::TextEdit::multiline(text)
                .id(te_id)
                .desired_rows(rows)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Monospace)
                .hint_text("Write anything here…"),
        )
        .changed()
    };
    match mode {
        MdMode::Edit => changed |= edit(ui, text),
        MdMode::Preview => {
            let resp = egui::Frame::group(ui.style())
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    render_markdown(ui, cache, text);
                })
                .response;
            if resp.interact(egui::Sense::click()).double_clicked() {
                mode = MdMode::Edit;
            }
        }
        MdMode::Split => {
            ui.columns(2, |cols| {
                changed |= edit(&mut cols[0], text);
                egui::Frame::group(cols[1].style()).show(&mut cols[1], |ui| {
                    ui.set_min_width(ui.available_width());
                    render_markdown(ui, cache, text);
                });
            });
        }
    }
    ui.data_mut(|d| d.insert_temp(id, mode));
    changed
}

/// A vertical scroll area whose content can't grow wider than the space it was given.
/// (egui widens a vertical scroll area to last frame's content width, and wrapped text then
/// fills that width, so one wide row would otherwise make the column overflow for good.)
pub fn pinned_scroll<R>(ui: &mut Ui, id_salt: impl std::hash::Hash, add: impl FnOnce(&mut Ui) -> R) -> R {
    let w = ui.available_width() - ui.spacing().scroll.allocated_width();
    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .show(ui, |ui| {
            ui.set_max_width(w);
            add(ui)
        })
        .inner
}

/// A dropdown menu that stays open while the user types or clicks inside it
/// (egui menus close on any click by default). Call `ui.close()` to close it.
pub fn form_menu<R>(ui: &mut Ui, text: impl Into<String>, add: impl FnOnce(&mut Ui) -> R) -> egui::Response {
    use egui::containers::menu::{MenuButton, MenuConfig};
    let cfg = MenuConfig::new().close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    MenuButton::new(text.into()).config(cfg).ui(ui, add).0
}

// ---------------------------------------------------------------------------
// Fields
// ---------------------------------------------------------------------------

/// Text field bound to an optional integer; blank = None.
pub fn opt_int(ui: &mut Ui, id_salt: impl std::hash::Hash, value: &mut Option<i32>, width: f32) -> egui::Response {
    let id = ui.make_persistent_id(("optint", id_salt));
    let mut buf = ui
        .data(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| value.map(|v| v.to_string()).unwrap_or_default());
    let resp = ui.add_sized(
        [width, ui.spacing().interact_size.y],
        egui::TextEdit::singleline(&mut buf).id(id.with("te")).hint_text("—"),
    );
    if resp.changed() {
        let t = buf.trim();
        if t.is_empty() {
            *value = None;
        } else if let Ok(v) = t.trim_start_matches('+').parse::<i32>() {
            *value = Some(v);
        }
    }
    if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, buf));
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
    }
    resp
}

/// Like `opt_int`, but the value is only written when the box loses focus or Enter is
/// pressed (for fields where a half-typed number would do harm, like hit points).
/// Returns true when the value was committed.
pub fn opt_int_commit(ui: &mut Ui, id_salt: impl std::hash::Hash, value: &mut Option<i32>, width: f32) -> bool {
    let id = ui.make_persistent_id(("optint_c", id_salt));
    let mut buf = ui
        .data(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| value.map(|v| v.to_string()).unwrap_or_default());
    let resp = ui.add_sized(
        [width, ui.spacing().interact_size.y],
        egui::TextEdit::singleline(&mut buf).id(id.with("te")).hint_text("—"),
    );
    let mut committed = false;
    if resp.lost_focus() {
        let t = buf.trim();
        let new = if t.is_empty() {
            None
        } else {
            t.trim_start_matches('+').parse::<i32>().ok().or(*value)
        };
        if new != *value {
            *value = new;
            committed = true;
        }
    }
    if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, buf));
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
    }
    committed
}

pub fn opt_u32(ui: &mut Ui, id_salt: impl std::hash::Hash, value: &mut Option<u32>, width: f32) -> egui::Response {
    let mut v = value.map(|x| x as i32);
    let r = opt_int(ui, id_salt, &mut v, width);
    *value = v.and_then(|x| u32::try_from(x).ok());
    r
}

// ---------------------------------------------------------------------------
// Dates (week start and date style come from the user's settings each frame)
// ---------------------------------------------------------------------------

use cm_core::calendar::{DateStyle, WeekStart};

fn prefs_id() -> egui::Id {
    egui::Id::new("date_prefs")
}

pub fn set_date_prefs(ctx: &egui::Context, week: WeekStart, style: DateStyle) {
    ctx.data_mut(|d| d.insert_temp(prefs_id(), (week, style)));
}

fn date_prefs(ui: &Ui) -> (WeekStart, DateStyle) {
    ui.data(|d| d.get_temp(prefs_id())).unwrap_or_default()
}

/// Full date in the user's preferred style ("Sat, Oct 31, 2026").
pub fn fmt_date(ui: &Ui, d: NaiveDate) -> String {
    cm_core::calendar::format_date(d, date_prefs(ui).1)
}

/// Short date without the year ("Oct 31").
pub fn fmt_date_short(ui: &Ui, d: NaiveDate) -> String {
    cm_core::calendar::format_date_short(d, date_prefs(ui).1)
}

/// Button showing a date; opens a month calendar that honours the week-start setting.
pub fn date_picker(ui: &mut Ui, id_salt: impl std::hash::Hash, date: &mut NaiveDate) -> bool {
    use chrono::Datelike;
    let (week, _) = date_prefs(ui);
    let id = ui.make_persistent_id(("datepick", id_salt));
    let mut changed = false;
    let label = format!("📅 {}", fmt_date(ui, *date));
    form_menu(ui, label, |ui| {
        // which month is on show (defaults to the selected date's month)
        let (mut y, mut m): (i32, u32) = ui.data(|d| d.get_temp(id)).unwrap_or((date.year(), date.month()));
        ui.horizontal(|ui| {
            if ui.button("‹").on_hover_text("Previous month").clicked() {
                if m == 1 {
                    m = 12;
                    y -= 1;
                } else {
                    m -= 1;
                }
            }
            let title = NaiveDate::from_ymd_opt(y, m, 1).map(|d| d.format("%B %Y").to_string()).unwrap_or_default();
            ui.add_sized([150.0, ui.spacing().interact_size.y], egui::Label::new(RichText::new(title).strong()));
            if ui.button("›").on_hover_text("Next month").clicked() {
                if m == 12 {
                    m = 1;
                    y += 1;
                } else {
                    m += 1;
                }
            }
        });
        let today = chrono::Local::now().date_naive();
        egui::Grid::new(("datepick_grid", id)).striped(false).spacing([2.0, 2.0]).show(ui, |ui| {
            for h in week.headers() {
                ui.add_sized([28.0, 18.0], egui::Label::new(RichText::new(h).small().weak()));
            }
            ui.end_row();
            for (i, d) in cm_core::calendar::month_grid(y, m, week).into_iter().enumerate() {
                let mut t = RichText::new(d.day().to_string());
                if d.month() != m {
                    t = t.weak();
                }
                if d == today {
                    t = t.underline().strong();
                }
                let r = ui.add_sized([28.0, 22.0], egui::Button::selectable(d == *date, t));
                if r.clicked() {
                    *date = d;
                    changed = true;
                    ui.close();
                }
                if i % 7 == 6 {
                    ui.end_row();
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Today").clicked() {
                *date = today;
                changed = true;
                ui.close();
            }
            ui.weak(match week {
                WeekStart::Sunday => "Weeks start on Sunday",
                WeekStart::Monday => "Weeks start on Monday",
            })
            .on_hover_text("Change in Settings → Appearance → Dates");
        });
        if changed {
            ui.data_mut(|d| d.remove::<(i32, u32)>(id));
        } else {
            ui.data_mut(|d| d.insert_temp(id, (y, m)));
        }
    });
    changed
}

/// Optional real-world date with a calendar popup.
pub fn opt_date(ui: &mut Ui, id_salt: &str, value: &mut Option<NaiveDate>) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| match value {
        Some(d) => {
            changed |= date_picker(ui, id_salt, d);
            if ui.small_button("✖").on_hover_text("Clear date").clicked() {
                *value = None;
                changed = true;
            }
        }
        None => {
            if ui.button("Set date…").clicked() {
                *value = Some(chrono::Local::now().date_naive());
                changed = true;
            }
        }
    });
    changed
}

/// Freytag's pyramid: exposition, rising action, climax, falling action, resolution.
pub fn paint_plot_icon(painter: &egui::Painter, center: egui::Pos2, size: f32, color: Color32) {
    let w = size * 1.35;
    let h = size * 0.95;
    let o = center - egui::vec2(w / 2.0, h / 2.0);
    let pt = |x: f32, y: f32| o + egui::vec2(x * w, y * h);
    let pts = vec![pt(0.0, 1.0), pt(0.22, 1.0), pt(0.58, 0.0), pt(0.80, 0.55), pt(1.0, 0.55)];
    painter.add(egui::Shape::line(pts, egui::Stroke::new((size / 7.5).max(1.6), color)));
    painter.circle_filled(pt(0.58, 0.0), (size / 6.5).max(2.0), color);
}

pub fn game_date_edit(ui: &mut Ui, id_salt: impl std::hash::Hash, cal: &GameCalendar, d: &mut GameDate) -> bool {
    let before = *d;
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(&mut d.day)
                .range(1..=cal.months.get(d.month as usize - 1).map(|m| m.days).unwrap_or(30).max(1))
                .prefix("day "),
        );
        egui::ComboBox::from_id_salt(("gdm", id_salt))
            .selected_text(cal.months.get(d.month as usize - 1).map(|m| m.name.clone()).unwrap_or("?".into()))
            .show_ui(ui, |ui| {
                for (i, m) in cal.months.iter().enumerate() {
                    ui.selectable_value(&mut d.month, i as u32 + 1, &m.name);
                }
            });
        ui.add(egui::DragValue::new(&mut d.year).prefix("year "));
        if !cal.era.trim().is_empty() {
            ui.label(cal.era.trim());
        }
        if let Some(wd) = cal.weekday(d) {
            ui.weak(wd);
        }
    });
    *d = cal.clamp(*d);
    *d != before
}

pub fn color_opt(ui: &mut Ui, value: &mut Option<[u8; 3]>) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| match value {
        Some(c) => {
            changed |= ui.color_edit_button_srgb(c).changed();
            if ui.small_button("✖").on_hover_text("No colour").clicked() {
                *value = None;
                changed = true;
            }
        }
        None => {
            if ui.button("Add colour").clicked() {
                *value = Some([90, 130, 200]);
                changed = true;
            }
        }
    });
    changed
}

/// "1 card", "3 cards".
pub fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("1 {word}")
    } else if word.ends_with('s') || word.ends_with('x') || word.ends_with("ch") || word.ends_with("sh") {
        format!("{n} {word}es")
    } else {
        format!("{n} {word}s")
    }
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Comma-separated tag editor.
pub fn tags_edit(ui: &mut Ui, id_salt: impl std::hash::Hash, tags: &mut Vec<String>) -> bool {
    let id = ui.make_persistent_id(("tags", id_salt));
    let mut buf = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| tags.join(", "));
    let r = ui.add(
        egui::TextEdit::singleline(&mut buf)
            .id(id.with("te"))
            .hint_text("comma, separated, tags")
            .desired_width(f32::INFINITY),
    );
    let mut changed = false;
    if r.changed() {
        let new: Vec<String> = buf.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if new != *tags {
            *tags = new;
            changed = true;
        }
    }
    if r.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, buf));
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
    }
    changed
}

/// Heading-style section label.
pub fn section(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).strong());
}

/// A card's title with a lock for secrets (🔒 unrevealed, 🔓 revealed).
pub fn card_title(c: &cm_core::StoryCard) -> String {
    if c.is_unrevealed_secret() {
        format!("🔒 {}", c.display_title())
    } else if c.secret {
        format!("🔓 {}", c.display_title())
    } else {
        c.display_title()
    }
}

/// A list row that looks like a selectable table cell.
pub fn list_row(ui: &mut Ui, selected: bool, text: impl Into<egui::WidgetText>) -> egui::Response {
    let w = ui.available_width();
    ui.allocate_ui_with_layout(egui::vec2(w, 0.0), egui::Layout::top_down_justified(egui::Align::Min), |ui| {
        ui.add(egui::Button::selectable(selected, text).truncate())
    })
    .inner
}

/// Small coloured square.
pub fn swatch(ui: &mut Ui, color: Option<[u8; 3]>) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    if let Some(c) = color {
        ui.painter().rect_filled(rect, 1.0, rgb(c));
    } else {
        ui.painter()
            .rect_stroke(rect, 1.0, egui::Stroke::new(1.0_f32, ui.visuals().weak_text_color()), egui::StrokeKind::Inside);
    }
}

// ---------------------------------------------------------------------------
// Files and images
// ---------------------------------------------------------------------------

/// Run a native file dialog, never letting a dialog-backend failure crash the app.
/// (If the desktop has no file-dialog service, the user can still paste a path or drag files.)
pub fn safe_dialog<T>(f: impl FnOnce() -> Option<T>) -> Option<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => v,
        Err(_) => {
            log::error_dialog_failed();
            None
        }
    }
}

pub mod log {
    use std::sync::atomic::{AtomicBool, Ordering};
    static DIALOG_FAILED: AtomicBool = AtomicBool::new(false);
    pub fn error_dialog_failed() {
        DIALOG_FAILED.store(true, Ordering::Relaxed);
    }
    /// True once if a file dialog failed since the last call.
    pub fn take_dialog_failed() -> bool {
        DIALOG_FAILED.swap(false, Ordering::Relaxed)
    }
}

/// Programs that open a file, folder or URL with the user's default app.
fn os_openers() -> &'static [(&'static str, &'static [&'static str])] {
    if cfg!(target_os = "macos") {
        &[("open", &[])]
    } else if cfg!(target_os = "windows") {
        &[("explorer", &[])]
    } else {
        &[("xdg-open", &[]), ("gio", &["open"]), ("kde-open5", &[]), ("kde-open", &[])]
    }
}

static OPEN_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Open a folder, file or URL (e.g. `obsidian://…`) with the operating system.
/// Errors that only show up after launch (the opener exits with failure) are reported
/// through [`take_open_error`] on a later frame.
pub fn open_with_os(target: &str) -> Result<(), String> {
    use std::process::{Command, Stdio};
    for (cmd, pre) in os_openers() {
        let spawned = Command::new(cmd)
            .args(*pre)
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match spawned {
            Ok(mut child) => {
                let what = target.to_string();
                let cmd = cmd.to_string();
                std::thread::spawn(move || {
                    if let Ok(status) = child.wait() {
                        if !status.success() {
                            *OPEN_ERROR.lock().unwrap() = Some(format!("{cmd} couldn't open {what} (is a file manager / handler installed?)"));
                        }
                    }
                });
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("couldn't run {cmd}: {e}")),
        }
    }
    Err("no program found to open it (on Linux, install xdg-utils)".into())
}

pub fn take_open_error() -> Option<String> {
    OPEN_ERROR.lock().ok()?.take()
}

pub fn pick_image_file() -> Option<PathBuf> {
    safe_dialog(|| {
        rfd::FileDialog::new()
            .set_title("Choose an image")
            .add_filter("Images", IMAGE_EXTS)
            .add_filter("All files", &["*"])
            .pick_file()
    })
}

pub fn pick_json_file(title: &str) -> Option<PathBuf> {
    safe_dialog(|| {
        rfd::FileDialog::new()
            .set_title(title)
            .add_filter("JSON / text", &["json", "txt", "md"])
            .add_filter("All files", &["*"])
            .pick_file()
    })
}

pub fn save_file_dialog(title: &str, file_name: &str, filter: (&str, &[&str])) -> Option<PathBuf> {
    safe_dialog(|| {
        rfd::FileDialog::new()
            .set_title(title)
            .set_file_name(file_name)
            .add_filter(filter.0, filter.1)
            .save_file()
    })
}

pub fn pick_font_file() -> Option<PathBuf> {
    safe_dialog(|| {
        rfd::FileDialog::new()
            .set_title("Choose a font")
            .add_filter("Fonts", &["ttf", "otf", "ttc"])
            .pick_file()
    })
}

pub fn pick_folder(title: &str) -> Option<PathBuf> {
    safe_dialog(|| rfd::FileDialog::new().set_title(title).pick_folder())
}

pub fn is_image_path(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn file_uri(dir: &Path, rel: &str) -> String {
    let abs = storage::resolve_asset(dir, rel);
    let s = abs.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') { format!("file://{s}") } else { format!("file:///{s}") }
}

/// Thumbnail of a campaign image; click to enlarge.
pub fn image_thumb(ui: &mut Ui, dir: &Path, rel: &str, max: egui::Vec2) -> egui::Response {
    let abs = storage::resolve_asset(dir, rel);
    if !abs.exists() {
        return ui.add(egui::Label::new(RichText::new(format!("⚠ missing image: {rel}")).color(Color32::RED)).sense(egui::Sense::click()));
    }
    ui.add(
        egui::Image::new(file_uri(dir, rel))
            .max_size(max)
            .maintain_aspect_ratio(true)
            .corner_radius(2)
            .sense(egui::Sense::click()),
    )
    .on_hover_text("Click to enlarge")
}

/// "Add image" control: browse, or paste a path. Copies into the campaign's assets folder.
/// Returns the campaign-relative path of an added image.
pub fn add_image_button(ui: &mut Ui, id_salt: impl std::hash::Hash, dir: &Path, label: &str, sh: &mut Shared) -> Option<String> {
    let id = ui.make_persistent_id(("addimg", id_salt));
    let mut out = None;
    form_menu(ui, label, |ui| {
        if ui.button("Browse…").clicked() {
            if let Some(p) = pick_image_file() {
                out = import(dir, &p, sh);
            }
            ui.close();
        }
        ui.separator();
        ui.label("…or paste a file path:");
        let mut buf = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_default();
        let r = ui.add_sized(
            [260.0, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut buf).desired_width(260.0).hint_text("/path/to/image.png"),
        );
        let go = ui.button("Add").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
        if go && !buf.trim().is_empty() {
            let p = PathBuf::from(buf.trim().trim_matches('"'));
            out = import(dir, &p, sh);
            buf.clear();
            ui.close();
        }
        ui.data_mut(|d| d.insert_temp(id, buf));
        ui.weak("Tip: you can also drag image files onto the window.");
    });
    out
}

pub fn import(dir: &Path, p: &Path, sh: &mut Shared) -> Option<String> {
    match storage::import_asset(dir, p) {
        Ok(rel) => Some(rel),
        Err(e) => {
            sh.toast(format!("Couldn't add file: {e}"));
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Links
// ---------------------------------------------------------------------------

/// Show links as clickable chips with remove buttons. Returns true if one was removed.
pub fn link_chips(ui: &mut Ui, c: &Campaign, links: &mut Vec<Link>, sh: &mut Shared) -> bool {
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (i, l) in links.iter().enumerate() {
            let name = c.link_name(l);
            let missing = name.is_none();
            let text = format!("{}: {}", l.kind_label(), name.unwrap_or_else(|| "(deleted)".into()));
            let resp = egui::Frame::new()
                .fill(ui.visuals().faint_bg_color)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .corner_radius(3)
                .inner_margin(egui::Margin::symmetric(4, 1))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let t = if missing { RichText::new(text).strikethrough() } else { RichText::new(text) };
                        let r = ui.add(egui::Label::new(t).sense(egui::Sense::click()));
                        if r.clicked() && !missing {
                            sh.actions.push(Action::Open(l.clone()));
                        }
                        if let Link::Url { url, .. } = l {
                            r.on_hover_text(url);
                        }
                        if ui.small_button("✖").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            let _ = resp;
        }
    });
    if let Some(i) = remove {
        links.remove(i);
        return true;
    }
    false
}

#[derive(Default, Clone)]
struct PickerState {
    filter: String,
    url: String,
    url_label: String,
}

/// "+ Link" dropdown that lets the DM attach any campaign entity, a wiki page, a URL or an image.
pub fn link_picker(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash,
    c: &Campaign,
    links: &mut Vec<Link>,
    exclude: Option<&Link>,
    dir: &Path,
    vault: Option<&Vault>,
    sh: &mut Shared,
) -> bool {
    let id = ui.make_persistent_id(("linkpick", id_salt));
    let mut st: PickerState = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let mut changed = false;
    let add = |l: Link, links: &mut Vec<Link>| {
        if !links.contains(&l) {
            links.push(l);
        }
    };
    form_menu(ui, "🔗 Add link…", |ui| {
        ui.set_min_width(280.0);
        ui.horizontal(|ui| {
            ui.label("Find:");
            ui.add_sized(
                [200.0, ui.spacing().interact_size.y],
                egui::TextEdit::singleline(&mut st.filter).desired_width(200.0).hint_text("name…"),
            );
        });
        let f = st.filter.to_lowercase();
        egui::ScrollArea::vertical().max_height(260.0).id_salt("lp_scroll").show(ui, |ui| {
            let mut last_kind = "";
            for e in c.all_entities() {
                if Some(&e.link) == exclude || links.contains(&e.link) {
                    continue;
                }
                if !f.is_empty() && !e.name.to_lowercase().contains(&f) {
                    continue;
                }
                let kind = e.link.kind_label();
                if kind != last_kind {
                    ui.label(RichText::new(kind).small().strong());
                    last_kind = kind;
                }
                if ui.button(&e.name).clicked() {
                    add(e.link.clone(), links);
                    changed = true;
                    ui.close();
                }
            }
            if let Some(v) = vault {
                let hits: Vec<_> = if f.is_empty() {
                    v.notes.iter().take(30).map(|n| (n.rel_path.clone(), n.title.clone())).collect()
                } else {
                    v.notes
                        .iter()
                        .filter(|n| n.title.to_lowercase().contains(&f) || n.rel_path.to_lowercase().contains(&f))
                        .take(30)
                        .map(|n| (n.rel_path.clone(), n.title.clone()))
                        .collect()
                };
                if !hits.is_empty() {
                    ui.label(RichText::new("Wiki pages").small().strong());
                }
                for (rel, title) in hits {
                    if ui.button(&title).on_hover_text(&rel).clicked() {
                        add(Link::Wiki(rel), links);
                        changed = true;
                        ui.close();
                    }
                }
            }
        });
        ui.separator();
        ui.label(RichText::new("Web link").small().strong());
        ui.horizontal(|ui| {
            ui.add_sized(
                [170.0, ui.spacing().interact_size.y],
                egui::TextEdit::singleline(&mut st.url).hint_text("https://…").desired_width(170.0),
            );
            ui.add_sized(
                [80.0, ui.spacing().interact_size.y],
                egui::TextEdit::singleline(&mut st.url_label).hint_text("label").desired_width(80.0),
            );
            if ui.button("Add").clicked() && !st.url.trim().is_empty() {
                add(
                    Link::Url {
                        url: st.url.trim().to_string(),
                        label: st.url_label.trim().to_string(),
                    },
                    links,
                );
                st.url.clear();
                st.url_label.clear();
                changed = true;
                ui.close();
            }
        });
        ui.separator();
        if ui.button("🖼 Attach image…").clicked() {
            if let Some(p) = pick_image_file() {
                if let Some(rel) = import(dir, &p, sh) {
                    add(Link::Image(rel), links);
                    changed = true;
                }
            }
            ui.close();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, st));
    changed
}

/// Images among a list of links, shown as thumbnails.
pub fn link_images(ui: &mut Ui, dir: &Path, links: &[Link], sh: &mut Shared) {
    let imgs: Vec<&String> = links.iter().filter_map(|l| if let Link::Image(p) = l { Some(p) } else { None }).collect();
    if imgs.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for p in imgs {
            if image_thumb(ui, dir, p, egui::vec2(140.0, 110.0)).clicked() {
                sh.actions.push(Action::ViewImage(p.clone()));
            }
        }
    });
}

/// Show "Referenced by" backlinks.
pub fn backlinks(ui: &mut Ui, c: &Campaign, target: &Link, sh: &mut Shared) {
    let bl = c.backlinks(target);
    if bl.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.weak("Referenced by:");
        for e in bl {
            if ui.link(format!("{} ({})", e.name, e.link.kind_label())).clicked() {
                sh.actions.push(Action::Open(e.link));
            }
        }
    });
}

/// A text filter box with a clear button.
pub fn filter_box(ui: &mut Ui, text: &mut String, hint: &str) {
    // Lay out right-to-left so the clear button takes its space first and the text box gets
    // exactly what's left: the box can never push its panel wider (which made panels creep).
    // The text box needs a fixed id: its automatic id depends on what was added before it,
    // and the ✖ appearing after the first letter would otherwise steal its keyboard focus.
    let id = ui.make_persistent_id(("filter_box", hint));
    let h = ui.spacing().interact_size.y;
    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), h), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if !text.is_empty() && ui.small_button("✖").on_hover_text("Clear").clicked() {
            text.clear();
        }
        ui.add(egui::TextEdit::singleline(text).id(id).hint_text(hint).desired_width(f32::INFINITY));
    });
}

pub fn confirm_delete_button(ui: &mut Ui, link: Link, sh: &mut Shared) {
    if ui.button(RichText::new("🗑 Delete").color(ui.visuals().error_fg_color)).clicked() {
        sh.actions.push(Action::ConfirmDelete(link));
    }
}

/// How to store a chosen vault folder: relative to the campaign folder when it's inside it
/// (so the campaign can be moved or synced), absolute otherwise.
pub fn vault_setting(campaign_dir: &Path, chosen: &Path) -> String {
    let dir = campaign_dir.canonicalize().unwrap_or_else(|_| campaign_dir.to_path_buf());
    let p = chosen.canonicalize().unwrap_or_else(|_| chosen.to_path_buf());
    match p.strip_prefix(&dir) {
        Ok(rel) if !rel.as_os_str().is_empty() => rel.to_string_lossy().replace('\\', "/"),
        _ => chosen.to_string_lossy().to_string(),
    }
}
