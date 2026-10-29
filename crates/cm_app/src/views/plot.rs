//! Plot line: a grid of plot threads (rows) × time (columns). Cards can be dragged between
//! cells to change their thread and, in session mode, which session they're aimed at.

use std::collections::BTreeMap;

use cm_core::{CardStatus, CardTiming, Id, Link, StoryCard};
use egui::{Color32, RichText};

use crate::app::{Action, Doc, Shared};
use crate::ui_util;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Session,
    RealDate,
    GameDate,
}

#[derive(Default)]
pub struct State {
    mode: Mode,
    hide_done: bool,
    hide_discarded: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Col {
    Unscheduled,
    Key(i64),
}

#[derive(Clone, Debug)]
struct DragCard {
    card: Id,
    from_thread: Option<Id>,
}

fn column_of(c: &cm_core::Campaign, card: &StoryCard, mode: Mode) -> Col {
    match mode {
        Mode::Session => match &card.timing {
            CardTiming::Session(n) => Col::Key(*n as i64),
            CardTiming::RealDate(d) => match c.settings.session_schedule.session_for_date(*d) {
                Some(n) => Col::Key(n as i64),
                None => Col::Unscheduled,
            },
            _ => c
                .sessions_for_card(card.id)
                .iter()
                .map(|s| s.number)
                .min()
                .map(|n| Col::Key(n as i64))
                .unwrap_or(Col::Unscheduled),
        },
        Mode::RealDate => match &card.timing {
            CardTiming::RealDate(d) => {
                use chrono::Datelike;
                Col::Key(d.year() as i64 * 12 + d.month0() as i64)
            }
            CardTiming::Session(n) => match c.settings.session_schedule.date_for_session(*n) {
                Some(d) => {
                    use chrono::Datelike;
                    Col::Key(d.year() as i64 * 12 + d.month0() as i64)
                }
                None => Col::Unscheduled,
            },
            _ => Col::Unscheduled,
        },
        Mode::GameDate => match &card.timing {
            CardTiming::GameDate(d) => {
                let cal = &c.settings.game_calendar;
                Col::Key(d.year as i64 * cal.months.len().max(1) as i64 + (d.month as i64 - 1))
            }
            _ => Col::Unscheduled,
        },
    }
}

fn col_label(c: &cm_core::Campaign, col: Col, mode: Mode) -> String {
    match col {
        Col::Unscheduled => "Unscheduled".into(),
        Col::Key(k) => match mode {
            Mode::Session => {
                let n = k as u32;
                match c.sessions.iter().find(|s| s.number == n) {
                    Some(s) if !s.title.trim().is_empty() => {
                        format!("Session {n}\n{}", s.title.trim())
                    }
                    _ => format!("Session {n}"),
                }
            }
            Mode::RealDate => {
                let y = k.div_euclid(12);
                let m = k.rem_euclid(12) as u32 + 1;
                chrono::NaiveDate::from_ymd_opt(y as i32, m, 1)
                    .map(|d| d.format("%B %Y").to_string())
                    .unwrap_or_default()
            }
            Mode::GameDate => {
                let cal = &c.settings.game_calendar;
                let n = cal.months.len().max(1) as i64;
                let y = k.div_euclid(n);
                let m = k.rem_euclid(n) as usize;
                format!(
                    "{} {}{}",
                    cal.months.get(m).map(|x| x.name.as_str()).unwrap_or("?"),
                    y,
                    if cal.era.is_empty() { String::new() } else { format!(" {}", cal.era) }
                )
            }
        },
    }
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    ui.horizontal(|ui| {
        ui.heading("Plot line");
        ui.separator();
        ui.label("Columns:");
        ui.selectable_value(&mut st.mode, Mode::Session, "Sessions");
        ui.selectable_value(&mut st.mode, Mode::RealDate, "Real-world months");
        ui.selectable_value(&mut st.mode, Mode::GameDate, "In-world months");
        ui.separator();
        ui.checkbox(&mut st.hide_done, "Hide done");
        ui.checkbox(&mut st.hide_discarded, "Hide discarded");
    });
    ui.weak(match st.mode {
        Mode::Session => "Drag cards between cells to change their plot thread or target session. Rows are plot threads.",
        _ => "Drag cards between rows to change their plot thread. Set dates on the card itself.",
    });
    ui.separator();

    let c = &doc.c;
    let cards: Vec<&StoryCard> = c
        .story_cards
        .iter()
        .filter(|x| !(st.hide_done && x.status == CardStatus::Done))
        .filter(|x| !(st.hide_discarded && x.status == CardStatus::Discarded))
        .collect();

    // columns
    let mut cols: Vec<Col> = vec![Col::Unscheduled];
    let mut keys: Vec<i64> = cards
        .iter()
        .filter_map(|x| if let Col::Key(k) = column_of(c, x, st.mode) { Some(k) } else { None })
        .collect();
    if st.mode == Mode::Session {
        keys.extend(c.sessions.iter().map(|s| s.number as i64));
        let next = keys.iter().max().copied().unwrap_or(0) + 1;
        keys.push(next.max(1));
    }
    keys.sort();
    keys.dedup();
    cols.extend(keys.into_iter().map(Col::Key));

    // rows: threads, then "no thread"
    let mut rows: Vec<(Option<Id>, String, Option<[u8; 3]>)> = c
        .threads
        .iter()
        .filter(|t| t.status != cm_core::ThreadStatus::Abandoned)
        .map(|t| (Some(t.id), format!("{}\n{}", t.name, t.status.label()), t.color))
        .collect();
    rows.push((None, "No thread".into(), None));

    // bucket cards
    let mut cells: BTreeMap<(Option<Id>, Col), Vec<&StoryCard>> = BTreeMap::new();
    for card in &cards {
        let col = column_of(c, card, st.mode);
        if card.threads.is_empty() {
            cells.entry((None, col)).or_default().push(card);
        } else {
            for t in &card.threads {
                if c.thread(*t).map(|th| th.status != cm_core::ThreadStatus::Abandoned).unwrap_or(false) {
                    cells.entry((Some(*t), col)).or_default().push(card);
                }
            }
        }
    }

    let mut dropped: Option<(DragCard, Option<Id>, Col)> = None;
    let mut new_card: Option<(Option<Id>, Col)> = None;
    let col_w = 170.0;
    let row_head_w = 140.0;
    let grid_stroke = ui.visuals().widgets.noninteractive.bg_stroke;

    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        // header
        ui.horizontal(|ui| {
            ui.add_sized([row_head_w, 34.0], egui::Label::new(RichText::new("Thread").strong()));
            for col in &cols {
                let label = col_label(c, *col, st.mode);
                let r = ui.add_sized([col_w, 34.0], egui::Label::new(RichText::new(label).strong()).sense(egui::Sense::click()));
                if let (Mode::Session, Col::Key(k)) = (st.mode, col) {
                    if let Some(s) = c.sessions.iter().find(|s| s.number as i64 == *k) {
                        if r.on_hover_text("Open session plan").clicked() {
                            sh.actions.push(Action::Open(Link::Session(s.id)));
                        }
                    }
                }
            }
        });
        ui.separator();
        for (ri, (tid, tname, tcolor)) in rows.iter().enumerate() {
            let row_bg = if ri % 2 == 0 { ui.visuals().faint_bg_color } else { Color32::TRANSPARENT };
            egui::Frame::new().fill(row_bg).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(egui::vec2(row_head_w, 40.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(row_head_w);
                        ui.horizontal(|ui| {
                            ui_util::swatch(ui, *tcolor);
                            let r = ui.add(egui::Label::new(RichText::new(tname.lines().next().unwrap_or("")).strong()).sense(egui::Sense::click()));
                            if let Some(t) = tid {
                                if r.clicked() {
                                    sh.actions.push(Action::Open(Link::Thread(*t)));
                                }
                            }
                        });
                        if let Some(status) = tname.lines().nth(1) {
                            ui.weak(status);
                        }
                    });
                    for col in &cols {
                        ui.allocate_ui_with_layout(egui::vec2(col_w, 40.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.set_width(col_w);
                            let frame = egui::Frame::new().inner_margin(4).stroke(grid_stroke);
                            let (_, payload) = ui.dnd_drop_zone::<DragCard, ()>(frame, |ui| {
                                ui.set_width(col_w - 10.0);
                                ui.set_min_height(36.0);
                                ui.vertical(|ui| {
                                    if let Some(list) = cells.get(&(*tid, *col)) {
                                        for card in list {
                                            card_chip(ui, c, card, *tid, sh);
                                        }
                                    }
                                    if ui
                                        .add(egui::Button::new(RichText::new("+ card").small().weak()).frame(false))
                                        .on_hover_text("New card here")
                                        .clicked()
                                    {
                                        new_card = Some((*tid, *col));
                                    }
                                });
                            });
                            if let Some(p) = payload {
                                dropped = Some(((*p).clone(), *tid, *col));
                            }
                        });
                    }
                });
            });
        }
    });

    if let Some((drag, to_thread, col)) = dropped {
        let mode = st.mode;
        if mode == Mode::Session {
            // also moves it between session plans, so it doesn't stay in the old one
            doc.c.move_card_to_session(
                drag.card,
                match col {
                    Col::Key(n) => Some(n as u32),
                    Col::Unscheduled => None,
                },
            );
        }
        if let Some(card) = doc.c.card_mut(drag.card) {
            if drag.from_thread != to_thread {
                if let Some(f) = drag.from_thread {
                    card.threads.retain(|t| *t != f);
                }
                if let Some(t) = to_thread {
                    if !card.threads.contains(&t) {
                        card.threads.push(t);
                    }
                }
            }
            card.modified = chrono::Utc::now();
        }
    }
    if let Some((tid, col)) = new_card {
        let mut card = StoryCard::default();
        if let Some(t) = tid {
            card.threads.push(t);
        }
        if let (Mode::Session, Col::Key(n)) = (st.mode, col) {
            card.timing = CardTiming::Session(n as u32);
        }
        let id = card.id;
        doc.c.story_cards.push(card);
        sh.actions.push(Action::Open(Link::Card(id)));
    }
}

fn card_chip(ui: &mut egui::Ui, c: &cm_core::Campaign, card: &StoryCard, row_thread: Option<Id>, sh: &mut Shared) {
    let id = egui::Id::new(("chip", card.id, row_thread));
    let payload = DragCard {
        card: card.id,
        from_thread: row_thread,
    };
    let color = card.color.or_else(|| row_thread.and_then(|t| c.thread(t)).and_then(|t| t.color));
    let resp = ui
        .dnd_drag_source(id, payload, |ui| {
            egui::Frame::new()
                .fill(ui.visuals().extreme_bg_color)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .corner_radius(2)
                .inner_margin(egui::Margin::symmetric(4, 2))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui_util::swatch(ui, color);
                        let mut t = RichText::new(ui_util::card_title(card));
                        if card.status == CardStatus::Done {
                            t = t.weak();
                        }
                        if card.status == CardStatus::Discarded {
                            t = t.strikethrough().weak();
                        }
                        ui.add(egui::Label::new(t).truncate());
                    });
                });
        })
        .response;
    let resp = resp.interact(egui::Sense::click());
    if resp.clicked() {
        sh.actions.push(Action::Open(Link::Card(card.id)));
    }
    let preview: String = card.body.chars().take(240).collect();
    resp.on_hover_text(format!("{} · {}\n\n{}", card.status.label(), card.timing.kind_label(), preview));
}
