//! Theme picker widgets: toolbar menu, preview gallery and the editor for user themes.

use egui::{Rect, RichText, Sense, Stroke, Vec2};

use crate::settings::AppSettings;
use crate::themes::{self, Theme, c, mix};

/// Compact menu for the toolbar.
pub fn theme_menu(ui: &mut egui::Ui, s: &mut AppSettings) {
    let current = s.theme_name().to_string();
    ui.menu_button(format!("🎨 {current}"), |ui| {
        ui.set_min_width(200.0);
        let mut pick = |ui: &mut egui::Ui, name: &str, swatch: Option<&Theme>| {
            ui.horizontal(|ui| {
                if let Some(t) = swatch {
                    mini_swatch(ui, t);
                }
                if ui.selectable_label(current == name, name).clicked() {
                    s.theme_name = Some(name.to_string());
                    ui.close();
                }
            });
        };
        ui.label(RichText::new("Light").small().strong());
        for b in themes::BUILTIN.iter().filter(|b| !b.dark) {
            pick(ui, b.name, Some(&b.to_theme()));
        }
        ui.separator();
        ui.label(RichText::new("Dark").small().strong());
        for b in themes::BUILTIN.iter().filter(|b| b.dark) {
            pick(ui, b.name, Some(&b.to_theme()));
        }
        let users = s.user_themes.clone();
        if !users.is_empty() {
            ui.separator();
            ui.label(RichText::new("Your themes").small().strong());
            for t in &users {
                pick(ui, &t.name, Some(t));
            }
        }
        ui.separator();
        pick(ui, themes::FOLLOW_SYSTEM, None);
    });
}

fn mini_swatch(ui: &mut egui::Ui, t: &Theme) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(28.0, 14.0), Sense::hover());
    let p = ui.painter();
    let half = Rect::from_min_max(rect.min, rect.center_bottom());
    p.rect_filled(rect, 2.0, c(t.palette.panel));
    p.rect_filled(half, 2.0, c(t.palette.background));
    p.rect_filled(
        Rect::from_min_size(rect.center_top() + Vec2::new(3.0, 4.0), Vec2::new(9.0, 6.0)),
        1.0,
        c(t.palette.accent),
    );
    p.rect_stroke(rect, 2.0, Stroke::new(1.0_f32, c(t.palette.border)), egui::StrokeKind::Inside);
}

/// A clickable preview card. Returns true if clicked.
fn preview_card(ui: &mut egui::Ui, t: &Theme, selected: bool) -> bool {
    let size = Vec2::new(138.0, 96.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let pal = &t.palette;
    let screen = Rect::from_min_size(rect.min, Vec2::new(size.x, 74.0));
    p.rect_filled(screen, 3.0, c(pal.panel));
    // nav strip with active tab
    let nav = Rect::from_min_size(screen.min, Vec2::new(30.0, screen.height()));
    p.rect_filled(nav, 3.0, c(pal.nav(t.dark)));
    for i in 0..4 {
        let y = nav.min.y + 8.0 + i as f32 * 15.0;
        let r = Rect::from_min_size(egui::pos2(nav.min.x + 4.0, y), Vec2::new(22.0, 9.0));
        if i == 1 {
            p.rect_filled(r.expand2(Vec2::new(4.0, 2.0)), 0.0, c(pal.panel));
            p.rect_filled(
                Rect::from_min_size(egui::pos2(nav.min.x, r.min.y - 2.0), Vec2::new(3.0, 13.0)),
                0.0,
                c(pal.accent),
            );
        }
        p.rect_filled(
            Rect::from_min_size(r.min + Vec2::new(2.0, 3.0), Vec2::new(16.0, 3.0)),
            1.0,
            c(mix(pal.text, pal.panel, 0.35)),
        );
    }
    // a text field with text, a selected row and a button
    let field = Rect::from_min_size(egui::pos2(nav.max.x + 8.0, screen.min.y + 8.0), Vec2::new(92.0, 18.0));
    p.rect_filled(field, 2.0, c(pal.background));
    p.rect_stroke(field, 2.0, Stroke::new(1.0_f32, c(pal.border)), egui::StrokeKind::Inside);
    p.text(
        field.left_center() + Vec2::new(5.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "Aa Text",
        egui::FontId::proportional(11.0),
        c(pal.text),
    );
    let sel = Rect::from_min_size(egui::pos2(field.min.x, field.max.y + 7.0), Vec2::new(92.0, 14.0));
    p.rect_filled(sel, 2.0, c(pal.selection(t.dark)));
    p.text(
        sel.left_center() + Vec2::new(5.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "Selected",
        egui::FontId::proportional(10.0),
        c(pal.text),
    );
    let btn = Rect::from_min_size(egui::pos2(field.min.x, sel.max.y + 6.0), Vec2::new(40.0, 13.0));
    p.rect_filled(btn, 2.0, c(pal.widget));
    let link_pos = btn.right_center() + Vec2::new(8.0, 0.0);
    p.text(
        link_pos,
        egui::Align2::LEFT_CENTER,
        "link",
        egui::FontId::proportional(10.0),
        c(mix(pal.accent, pal.text, if t.dark { 0.25 } else { 0.15 })),
    );
    // outline
    let outline = if selected {
        Stroke::new(3.0_f32, ui.visuals().hyperlink_color)
    } else if resp.hovered() {
        Stroke::new(2.0_f32, ui.visuals().widgets.hovered.bg_stroke.color)
    } else {
        Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color)
    };
    p.rect_stroke(screen, 3.0, outline, egui::StrokeKind::Outside);
    let label = if selected {
        RichText::new(format!("✔ {}", t.name)).strong()
    } else {
        RichText::new(&t.name)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.text().to_string(), egui::FontId::proportional(12.5), ui.visuals().text_color());
    ui.painter()
        .galley(egui::pos2(rect.min.x + 2.0, screen.max.y + 4.0), galley, ui.visuals().text_color());
    resp.on_hover_text(if t.dark { "Dark theme" } else { "Light theme" }).clicked()
}

/// Gallery of all themes plus the editor for the selected user theme.
pub fn gallery(ui: &mut egui::Ui, s: &mut AppSettings) {
    let current = s.theme_name().to_string();
    let mut chosen: Option<String> = None;
    let mut section = |ui: &mut egui::Ui, title: &str, list: Vec<Theme>| {
        if list.is_empty() {
            return;
        }
        ui.label(RichText::new(title).strong());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(12.0, 10.0);
            for t in &list {
                if preview_card(ui, t, current == t.name) {
                    chosen = Some(t.name.clone());
                }
            }
        });
        ui.add_space(6.0);
    };
    section(ui, "Light themes", themes::BUILTIN.iter().filter(|b| !b.dark).map(|b| b.to_theme()).collect());
    section(ui, "Dark themes", themes::BUILTIN.iter().filter(|b| b.dark).map(|b| b.to_theme()).collect());
    section(ui, "Your themes", s.user_themes.clone());
    ui.horizontal(|ui| {
        if ui.selectable_label(current == themes::FOLLOW_SYSTEM, "Follow system (Light / Dark)").clicked() {
            chosen = Some(themes::FOLLOW_SYSTEM.to_string());
        }
    });
    if let Some(n) = chosen {
        s.theme_name = Some(n);
    }
    ui.add_space(8.0);
    editor(ui, s);
}

fn editor(ui: &mut egui::Ui, s: &mut AppSettings) {
    let name = s.theme_name().to_string();
    let Some(idx) = s.user_themes.iter().position(|t| t.name == name) else {
        ui.horizontal(|ui| {
            if ui
                .button("🎨 Customize this theme…")
                .on_hover_text("Make an editable copy you can recolour and keep")
                .clicked()
            {
                let ctx = ui.ctx().clone();
                s.customize_current(&ctx);
            }
            ui.weak("Built-in themes can't be changed — customizing makes your own copy.");
        });
        return;
    };
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(RichText::new("Edit your theme").strong());
        let mut rename: Option<String> = None;
        let mut delete = false;
        {
            let t = &mut s.user_themes[idx];
            ui.horizontal(|ui| {
                ui.label("Name");
                // edit a copy; rename when the box loses focus (so spaces and clearing work)
                let buf_id = ui.make_persistent_id(("theme_name", idx));
                let mut buf = ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or_else(|| t.name.clone());
                let r = ui.add(egui::TextEdit::singleline(&mut buf).id(buf_id.with("te")).desired_width(200.0));
                if r.gained_focus() {
                    ui.data_mut(|d| d.remove::<String>(egui::Id::new("theme_rename_taken")));
                }
                if r.has_focus() {
                    ui.data_mut(|d| d.insert_temp(buf_id, buf.clone()));
                } else {
                    ui.data_mut(|d| d.remove::<String>(buf_id));
                }
                if r.lost_focus() && !buf.trim().is_empty() && buf.trim() != t.name {
                    rename = Some(buf);
                }
                ui.checkbox(&mut t.dark, "Dark theme")
                    .on_hover_text("Tells the app (and Markdown/code colours) this is a dark theme");
            });
            egui::Grid::new("theme_edit").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
                let p = &mut t.palette;
                let rows: [(&str, &mut [u8; 3], &str); 9] = [
                    ("Panels", &mut p.panel, "Main window background"),
                    ("Fields", &mut p.background, "Text boxes, map canvas"),
                    ("Popups", &mut p.window, "Menus and windows"),
                    ("Stripes", &mut p.faint, "Alternate rows"),
                    ("Text", &mut p.text, "All text"),
                    ("Accent", &mut p.accent, "Selection, links, active tab"),
                    ("Buttons", &mut p.widget, "Button background"),
                    ("Button hover", &mut p.widget_hover, "Button under the mouse"),
                    ("Borders", &mut p.border, "Lines and frames"),
                ];
                for (i, (label, col, tip)) in rows.into_iter().enumerate() {
                    ui.label(label).on_hover_text(tip);
                    ui.color_edit_button_srgb(col);
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
            let contrast = (themes::luminance(t.palette.text) - themes::luminance(t.palette.panel)).abs();
            if contrast < 0.45 {
                ui.colored_label(ui.visuals().warn_fg_color, "⚠ Text and panel colours are close — this may be hard to read.");
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Duplicate").clicked() {
                let ctx = ui.ctx().clone();
                s.customize_current(&ctx);
            }
            if ui.button(RichText::new("Delete theme").color(ui.visuals().error_fg_color)).clicked() {
                delete = true;
            }
        });
        if let Some(n) = rename {
            let n = n.trim().to_string();
            if !s.is_user_theme(&n) && themes::builtin(&n).is_none() && n != themes::FOLLOW_SYSTEM {
                s.user_themes[idx].name = n.clone();
                s.theme_name = Some(n);
            } else {
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("theme_rename_taken"), n));
            }
        }
        if let Some(n) = ui.ctx().data(|d| d.get_temp::<String>(egui::Id::new("theme_rename_taken"))) {
            ui.colored_label(ui.visuals().warn_fg_color, format!("There's already a theme called “{n}”."));
        }
        if delete {
            let dark = s.user_themes[idx].dark;
            s.user_themes.remove(idx);
            s.theme_name = Some(if dark { "Dark" } else { "Light" }.into());
        }
    });
}
