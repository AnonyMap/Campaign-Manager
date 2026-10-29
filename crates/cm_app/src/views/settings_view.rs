//! Settings: appearance (per user) and campaign settings (calendar, schedule, wiki,
//! custom conditions, name pools).

use cm_core::calendar::{DateStyle, GameCalendar, Month, WeekStart};
use cm_core::conditions::ConditionDef;
use cm_core::names;
use egui::RichText;

use crate::app::{Doc, Shared};
use crate::settings::{AppSettings, FontChoice};
use crate::ui_util::{self, section};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Appearance,
    Calendar,
    Wiki,
    Conditions,
    NamePools,
}

#[derive(Default)]
pub struct State {
    tab: Tab,
    font_path: String,
    pool_first: String,
    pool_last: String,
    pool_name: String,
    skip_date: Option<chrono::NaiveDate>,
    calc_session: u32,
    calc_date: Option<chrono::NaiveDate>,
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, app: &mut AppSettings) {
    ui.horizontal(|ui| {
        ui.heading("Settings");
        ui.separator();
        ui.selectable_value(&mut st.tab, Tab::Appearance, "Appearance");
        ui.selectable_value(&mut st.tab, Tab::Calendar, "Calendar & schedule");
        ui.selectable_value(&mut st.tab, Tab::Wiki, "Wiki");
        ui.selectable_value(&mut st.tab, Tab::Conditions, "Custom conditions");
        ui.selectable_value(&mut st.tab, Tab::NamePools, "Name pools");
    });
    ui.separator();
    egui::ScrollArea::vertical().show(ui, |ui| match st.tab {
        Tab::Appearance => appearance(ui, st, app),
        Tab::Calendar => calendar(ui, doc, st),
        Tab::Wiki => wiki(ui, doc),
        Tab::Conditions => conditions(ui, doc, sh),
        Tab::NamePools => pools(ui, doc, st),
    });
}

fn appearance(ui: &mut egui::Ui, st: &mut State, s: &mut AppSettings) {
    ui.weak("Appearance settings are saved for you on this computer, not in the campaign.");
    section(ui, "Theme");
    crate::theme_ui::gallery(ui, s);
    section(ui, "Text & layout");
    egui::Grid::new("appear").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
        ui.label("Text size");
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut s.font_size, 9.0..=32.0).step_by(1.0).suffix(" pt"));
            if ui.button("Reset").clicked() {
                s.font_size = 14.0;
            }
        });
        ui.end_row();

        ui.label("Interface scale");
        ui.horizontal(|ui| {
            // zoom only when the slider is let go: zooming while dragging moves it under the mouse
            let tmp_id = egui::Id::new("ui_scale_drag");
            let mut v = ui.data(|d| d.get_temp::<f32>(tmp_id)).unwrap_or(s.ui_scale);
            let r = ui.add(
                egui::Slider::new(&mut v, 0.6..=2.5)
                    .step_by(0.05)
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
            );
            if r.dragged() {
                ui.data_mut(|d| d.insert_temp(tmp_id, v));
            } else {
                ui.data_mut(|d| d.remove::<f32>(tmp_id));
                if r.changed() || r.drag_stopped() {
                    s.ui_scale = v;
                }
            }
            if ui.button("Reset").clicked() {
                s.ui_scale = 1.0;
            }
        });
        ui.end_row();

        ui.label("Font");
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                for f in FontChoice::ALL {
                    ui.selectable_value(&mut s.font, f, f.label());
                }
            });
            if s.font == FontChoice::Custom {
                ui.horizontal(|ui| {
                    if st.font_path.is_empty() {
                        st.font_path = s.custom_font_path.clone().unwrap_or_default();
                    }
                    ui.add_sized(
                        [300.0, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(&mut st.font_path)
                            .hint_text("/path/to/font.ttf")
                            .desired_width(300.0),
                    );
                    if ui.button("Browse…").clicked() {
                        if let Some(p) = ui_util::pick_font_file() {
                            st.font_path = p.to_string_lossy().to_string();
                        }
                    }
                    if ui.button("Apply").clicked() {
                        s.custom_font_path = Some(st.font_path.trim().to_string());
                    }
                });
                ui.weak("Any TrueType/OpenType font works, e.g. a dyslexia-friendly font like OpenDyslexic or Atkinson Hyperlegible.");
            }
        });
        ui.end_row();

        ui.label("Dates");
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Week starts on");
                ui.selectable_value(&mut s.week_start, WeekStart::Sunday, "Sunday (US)");
                ui.selectable_value(&mut s.week_start, WeekStart::Monday, "Monday");
            });
            ui.horizontal(|ui| {
                ui.label("Date format");
                let today = chrono::Local::now().date_naive();
                for style in [DateStyle::Us, DateStyle::International, DateStyle::Iso] {
                    ui.selectable_value(&mut s.date_style, style, cm_core::calendar::format_date(today, style));
                }
            });
        });
        ui.end_row();

        ui.label("Density");
        ui.checkbox(&mut s.compact, "Compact (tighter spacing)");
        ui.end_row();

        ui.label("Saving");
        ui.horizontal(|ui| {
            ui.label("Autosave");
            ui.add(egui::DragValue::new(&mut s.autosave_secs).range(0..=600).suffix(" s after a change"));
            ui.weak("(0 = off; Ctrl+S always saves)");
        });
        ui.end_row();
        ui.label("Startup");
        ui.checkbox(&mut s.reopen_last, "Reopen the last campaign");
        ui.end_row();
    });
}

fn calendar(ui: &mut egui::Ui, doc: &mut Doc, st: &mut State) {
    let set = &mut doc.c.settings;
    section(ui, "Play schedule (session-number ↔ real date)");
    let sched = &mut set.session_schedule;
    egui::Grid::new("sched").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("First session date");
        ui_util::opt_date(ui, "sched_first", &mut sched.first_date);
        ui.end_row();
        ui.label("First session number");
        ui.add(egui::DragValue::new(&mut sched.first_number).range(0..=9999));
        ui.end_row();
        ui.label("Every");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut sched.interval_days).range(1..=365).suffix(" days"));
            if ui.small_button("weekly").clicked() {
                sched.interval_days = 7;
            }
            if ui.small_button("fortnightly").clicked() {
                sched.interval_days = 14;
            }
            if ui.small_button("4-weekly").clicked() {
                sched.interval_days = 28;
            }
        });
        ui.end_row();
        ui.label("Skipped dates");
        ui.vertical(|ui| {
            let mut remove = None;
            for (i, d) in sched.skipped_dates.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(ui_util::fmt_date(ui, *d));
                    if ui.small_button("✖").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                sched.skipped_dates.remove(i);
            }
            ui.horizontal(|ui| {
                ui_util::opt_date(ui, "skipdate", &mut st.skip_date);
                if let Some(d) = st.skip_date {
                    if ui.button("Skip this date").clicked() {
                        sched.skipped_dates.push(d);
                        sched.skipped_dates.sort();
                        st.skip_date = None;
                    }
                }
            });
        });
        ui.end_row();
    });
    ui.add_space(4.0);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(RichText::new("Session calculator").strong());
        if sched.first_date.is_none() {
            ui.weak("Set the first session date to use the calculator.");
        }
        ui.horizontal(|ui| {
            ui.label("Session");
            ui.add(egui::DragValue::new(&mut st.calc_session).range(0..=9999));
            ui.label("falls on");
            let txt = sched.date_for_session(st.calc_session).map(|d| ui_util::fmt_date(ui, d)).unwrap_or("—".into());
            ui.strong(txt);
        });
        ui.horizontal(|ui| {
            ui.label("Date");
            ui_util::opt_date(ui, "calc_date", &mut st.calc_date);
            if let Some(d) = st.calc_date {
                ui.label("» session");
                ui.strong(sched.session_for_date(d).map(|n| n.to_string()).unwrap_or("before the first session".into()));
                if let Some(n) = sched.sessions_until(chrono::Local::now().date_naive(), d) {
                    ui.weak(format!("({n} sessions from today)"));
                }
            }
        });
    });

    section(ui, "In-world calendar");
    let cal = &mut set.game_calendar;
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.add_sized(
            [200.0, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut cal.name).desired_width(200.0),
        );
        ui.label("Era suffix");
        ui.add_sized(
            [60.0, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut cal.era).desired_width(60.0).hint_text("DR"),
        );
        ui.menu_button("Load preset", |ui| {
            if ui.button("Simple (12 × 30 days)").clicked() {
                let cur = cal.current;
                *cal = GameCalendar::simple();
                cal.current = cal.clamp(cur);
                ui.close();
            }
            if ui.button("Gregorian-style").clicked() {
                let cur = cal.current;
                *cal = GameCalendar::gregorian_like();
                cal.current = cal.clamp(cur);
                ui.close();
            }
        });
    });
    ui.horizontal(|ui| {
        ui.label("Today in the game world");
        let c2 = cal.clone();
        ui_util::game_date_edit(ui, "calcur", &c2, &mut cal.current);
        if ui.button("+1 day").clicked() {
            cal.current = cal.add_days(&cal.current, 1);
        }
        if ui.button("+1 week").clicked() {
            cal.current = cal.add_days(&cal.current, cal.weekdays.len().max(1) as i64);
        }
    });
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label(RichText::new("Months").strong());
        let mut remove = None;
        for (i, m) in cal.months.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}.", i + 1));
                ui.add_sized(
                    [140.0, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut m.name).desired_width(140.0),
                );
                ui.add(egui::DragValue::new(&mut m.days).range(1..=400).suffix(" days"));
                if ui.small_button("✖").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            if cal.months.len() > 1 {
                cal.months.remove(i);
            }
        }
        if ui.button("➕ Month").clicked() {
            cal.months.push(Month {
                name: format!("Month {}", cal.months.len() + 1),
                days: 30,
            });
        }
        ui.weak(format!("{} days per year", cal.days_in_year()));

        let ui = &mut cols[1];
        ui.label(RichText::new("Weekdays").strong());
        let mut remove = None;
        for (i, w) in cal.weekdays.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add_sized([140.0, ui.spacing().interact_size.y], egui::TextEdit::singleline(w).desired_width(140.0));
                if ui.small_button("✖").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            cal.weekdays.remove(i);
        }
        if ui.button("➕ Weekday").clicked() {
            cal.weekdays.push(format!("Day {}", cal.weekdays.len() + 1));
        }
    });
    cal.current = cal.clamp(cal.current);
}

fn wiki(ui: &mut egui::Ui, doc: &mut Doc) {
    let dir = doc.dir.clone();
    let set = &mut doc.c.settings;
    egui::Grid::new("wikiset").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Vault folder");
        ui.horizontal(|ui| {
            let mut p = set.wiki_path.clone().unwrap_or_default();
            if ui
                .add_sized(
                    [380.0, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut p).desired_width(380.0).hint_text("(none)"),
                )
                .changed()
            {
                set.wiki_path = if p.trim().is_empty() { None } else { Some(p) };
            }
            if ui.button("Browse…").clicked() {
                if let Some(f) = ui_util::pick_folder("Choose your Obsidian vault folder") {
                    set.wiki_path = Some(ui_util::vault_setting(&dir, &f));
                }
            }
        });
        ui.end_row();
        ui.label("Export folder");
        ui.add_sized(
            [240.0, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut set.wiki_export_folder)
                .hint_text("Campaign Manager")
                .desired_width(240.0),
        );
        ui.end_row();
    });
    ui.weak("Exported notes carry a cm_id in their front matter, so exporting again updates the same note even if you rename it.");
}

fn conditions(ui: &mut egui::Ui, doc: &mut Doc, _sh: &mut Shared) {
    ui.label("Custom conditions appear alongside the standard ones in the combat tracker.");
    let mut remove = None;
    for (i, c) in doc.c.custom_conditions.iter_mut().enumerate() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [200.0, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut c.name).hint_text("Name").desired_width(200.0),
                );
                ui.add_sized(
                    [360.0, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut c.summary).hint_text("One-line summary").desired_width(360.0),
                );
                if ui.small_button("✖").clicked() {
                    remove = Some(i);
                }
            });
            // keep the typed text while editing, so Enter at the end can start a new line
            let buf_id = ui.make_persistent_id(("cond_fx", i));
            let mut text = ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or_else(|| c.effects.join("\n"));
            let r = ui.add(
                egui::TextEdit::multiline(&mut text)
                    .id(buf_id.with("te"))
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .hint_text("Effects, one per line"),
            );
            if r.changed() {
                c.effects = text.lines().map(|l| l.trim_end().to_string()).filter(|l| !l.trim().is_empty()).collect();
            }
            if r.has_focus() {
                ui.data_mut(|d| d.insert_temp(buf_id, text));
            } else {
                ui.data_mut(|d| d.remove::<String>(buf_id));
            }
        });
    }
    if let Some(i) = remove {
        doc.c.custom_conditions.remove(i);
    }
    if ui.button("➕ Add custom condition").clicked() {
        doc.c.custom_conditions.push(ConditionDef {
            name: "New condition".into(),
            custom: true,
            ..Default::default()
        });
    }
    ui.separator();
    egui::CollapsingHeader::new("Standard conditions (2024 rules)").show(ui, |ui| {
        for c in cm_core::conditions::standard_conditions() {
            ui.label(RichText::new(&c.name).strong());
            for e in &c.effects {
                ui.label(format!("  • {e}"));
            }
        }
    });
}

fn pools(ui: &mut egui::Ui, doc: &mut Doc, st: &mut State) {
    ui.label(format!(
        "Built-in pools: {}. Add your own below — one name per line (or comma separated).",
        names::BUILTIN_POOLS.iter().map(|p| p.name).collect::<Vec<_>>().join(", ")
    ));
    let mut remove = None;
    for (i, p) in doc.c.settings.name_pools.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&p.name).strong());
            ui.weak(format!("{} first names, {} surnames", p.first.len(), p.last.len()));
            if ui.small_button("Edit").clicked() {
                st.pool_name = p.name.clone();
                st.pool_first = p.first.join("\n");
                st.pool_last = p.last.join("\n");
            }
            if ui.small_button("✖").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        doc.c.settings.name_pools.remove(i);
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Pool name");
        ui.add_sized(
            [200.0, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut st.pool_name)
                .desired_width(200.0)
                .hint_text("e.g. Pirates of the Shattered Isles"),
        );
    });
    ui.columns(2, |cols| {
        cols[0].label("First names");
        cols[0].add(egui::TextEdit::multiline(&mut st.pool_first).desired_rows(10).desired_width(f32::INFINITY));
        cols[1].label("Surnames (optional)");
        cols[1].add(egui::TextEdit::multiline(&mut st.pool_last).desired_rows(10).desired_width(f32::INFINITY));
    });
    if ui.add_enabled(!st.pool_name.trim().is_empty(), egui::Button::new("Save pool")).clicked() {
        let pool = names::pool_from_text(&st.pool_name, &st.pool_first, &st.pool_last);
        match doc.c.settings.name_pools.iter_mut().find(|p| p.name.eq_ignore_ascii_case(&pool.name)) {
            Some(p) => *p = pool,
            None => doc.c.settings.name_pools.push(pool),
        }
        st.pool_name.clear();
        st.pool_first.clear();
        st.pool_last.clear();
    }
}
