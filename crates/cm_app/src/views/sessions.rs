//! Session planner: narrative, running order of story cards, encounters, and recap.

use cm_core::{CardTiming, Id, Link, Session, SessionStatus};
use egui::RichText;

use crate::app::{Action, Doc, Shared};
use crate::ui_util::{self, section};

#[derive(Default)]
pub struct State {
    pub selected: Option<Id>,
    card_filter: String,
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    egui::SidePanel::left("sess_list").resizable(true).default_width(260.0).show_inside(ui, |ui| {
        ui.heading("Sessions");
        if ui.button("➕ Plan next session").clicked() {
            let n = doc.c.next_session_number();
            let s = Session {
                number: n,
                real_date: doc.c.settings.session_schedule.date_for_session(n),
                ..Default::default()
            };
            st.selected = Some(s.id);
            doc.c.sessions.push(s);
        }
        ui.separator();
        let mut list: Vec<&Session> = doc.c.sessions.iter().collect();
        list.sort_by_key(|s| s.number);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for s in list {
                let date = s
                    .real_date
                    .or_else(|| doc.c.settings.session_schedule.date_for_session(s.number))
                    .map(|d| ui_util::fmt_date_short(ui, d))
                    .unwrap_or_default();
                let mut t = RichText::new(format!("{}   {date}", s.display_title()));
                if s.status == SessionStatus::Played {
                    t = t.weak();
                }
                if s.status == SessionStatus::Cancelled {
                    t = t.strikethrough().weak();
                }
                if ui_util::list_row(ui, st.selected == Some(s.id), t).clicked() {
                    st.selected = Some(s.id);
                }
            }
        });
    });

    let Some(id) = st.selected else {
        ui.centered_and_justified(|ui| ui.weak("Select a session or plan the next one."));
        return;
    };
    if doc.c.session(id).is_none() {
        st.selected = None;
        return;
    }
    ui_util::pinned_scroll(ui, ("sess_edit", id), |ui| editor(ui, doc, sh, st, id));
}

fn editor(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, id: Id) {
    let dir = doc.dir.clone();
    let cal = doc.c.settings.game_calendar.clone();
    let sched = doc.c.settings.session_schedule.clone();
    let s = doc.c.session_mut(id).unwrap();
    ui.horizontal(|ui| {
        ui.heading(format!("Session {}", s.number));
        ui.add(
            egui::TextEdit::singleline(&mut s.title)
                .hint_text("Title")
                .font(egui::TextStyle::Heading)
                .desired_width(300.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui_util::confirm_delete_button(ui, Link::Session(id), sh);
        });
    });
    egui::Grid::new(("sessgrid", id)).num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Number");
        ui.add(egui::DragValue::new(&mut s.number).range(0..=9999));
        ui.end_row();
        ui.label("Status");
        ui.horizontal(|ui| {
            for x in SessionStatus::ALL {
                ui.selectable_value(&mut s.status, x, x.label());
            }
        });
        ui.end_row();
        ui.label("Real date");
        ui.horizontal(|ui| {
            ui_util::opt_date(ui, &format!("sessdate{id}"), &mut s.real_date);
            if let Some(d) = sched.date_for_session(s.number) {
                if s.real_date != Some(d) && ui.button(format!("Use schedule ({})", ui_util::fmt_date(ui, d))).clicked() {
                    s.real_date = Some(d);
                }
            }
        });
        ui.end_row();
        ui.label("In-world date");
        ui.horizontal(|ui| match &mut s.game_date {
            Some(gd) => {
                ui_util::game_date_edit(ui, ("sessgd", id), &cal, gd);
                if ui.small_button("✖").clicked() {
                    s.game_date = None;
                }
            }
            None => {
                if ui.button("Set in-world date").clicked() {
                    s.game_date = Some(cal.current);
                }
            }
        });
        ui.end_row();
    });

    section(ui, "Plan / narrative");
    ui_util::markdown_editor(ui, ("sessnarr", id), &mut s.narrative, &mut sh.md, 8);

    // ---- planned cards
    section(ui, "Story cards (running order)");
    let number = s.number;
    let card_ids = s.card_ids.clone();
    let mut move_up = None;
    let mut remove = None;
    for (i, cid) in card_ids.iter().enumerate() {
        let Some(card) = doc.c.card(*cid) else {
            continue;
        };
        ui.horizontal(|ui| {
            ui.label(format!("{}.", i + 1));
            ui_util::swatch(ui, card.color);
            if ui.link(ui_util::card_title(card)).clicked() {
                sh.actions.push(Action::Open(Link::Card(*cid)));
            }
            ui.weak(card.status.label());
            if i > 0 && ui.small_button("⬆").clicked() {
                move_up = Some(i);
            }
            if ui.small_button("✖").on_hover_text("Remove from plan (the card is kept)").clicked() {
                remove = Some(*cid);
            }
        });
        if !card.body.trim().is_empty() {
            ui.indent(("cardprev", cid), |ui| {
                let first: String = card.body.lines().filter(|l| !l.trim().is_empty()).take(2).collect::<Vec<_>>().join(" ");
                ui.weak(first.chars().take(160).collect::<String>());
            });
        }
    }
    if card_ids.is_empty() {
        ui.weak("No cards planned yet.");
    }
    if let Some(i) = move_up {
        if let Some(s) = doc.c.session_mut(id) {
            s.card_ids.swap(i, i - 1);
        }
    }
    if let Some(c) = remove {
        doc.c.unassign_card_from_session(c, id);
    }
    let mut add_card = None;
    ui.horizontal(|ui| {
        ui_util::form_menu(ui, "➕ Add card", |ui| {
            ui.set_min_width(300.0);
            ui.add(egui::TextEdit::singleline(&mut st.card_filter).hint_text("Filter…"));
            let f = st.card_filter.to_lowercase();
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                let mut cards: Vec<_> = doc.c.story_cards.iter().filter(|c| !card_ids.contains(&c.id)).collect();
                cards.sort_by_key(|c| (!c.in_ideas_pile, c.display_title().to_lowercase()));
                let mut shown_header = None;
                for c in cards {
                    if !f.is_empty() && !c.display_title().to_lowercase().contains(&f) && !c.body.to_lowercase().contains(&f) {
                        continue;
                    }
                    if shown_header != Some(c.in_ideas_pile) {
                        ui.label(RichText::new(if c.in_ideas_pile { "Ideas pile" } else { "Other cards" }).small().strong());
                        shown_header = Some(c.in_ideas_pile);
                    }
                    if ui.button(c.display_title()).clicked() {
                        add_card = Some(c.id);
                        ui.close();
                    }
                }
            });
        });
        if ui.button("➕ New card for this session").clicked() {
            sh.actions.push(Action::NewCardInSession(id));
        }
    });
    if let Some(c) = add_card {
        doc.c.assign_card_to_session(c, id);
    }
    // Cards aimed at this session by number/date but not in the plan.
    let aimed: Vec<(Id, String)> = doc
        .c
        .story_cards
        .iter()
        .filter(|c| !card_ids.contains(&c.id))
        .filter(|c| match &c.timing {
            CardTiming::Session(n) => *n == number,
            CardTiming::RealDate(d) => sched.session_for_date(*d) == Some(number),
            _ => false,
        })
        .map(|c| (c.id, c.display_title()))
        .collect();
    if !aimed.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.weak("Also aimed at this session:");
            for (cid, name) in aimed {
                if ui.link(&name).clicked() {
                    sh.actions.push(Action::Open(Link::Card(cid)));
                }
                if ui.small_button("add").clicked() {
                    doc.c.assign_card_to_session(cid, id);
                }
            }
        });
    }

    // ---- encounters
    section(ui, "Encounters");
    let enc_ids = doc.c.session(id).map(|s| s.encounter_ids.clone()).unwrap_or_default();
    let mut remove_enc = None;
    for eid in &enc_ids {
        if let Some(e) = doc.c.encounter(*eid) {
            ui.horizontal(|ui| {
                if ui.link(&e.name).clicked() {
                    sh.actions.push(Action::Open(Link::Encounter(*eid)));
                }
                let n: u32 = e.entries.iter().map(|x| x.count.max(1)).sum();
                ui.weak(format!("{n} combatants"));
                if ui.button("⚔ Run").clicked() {
                    sh.actions.push(Action::StartEncounter(*eid));
                }
                if ui.small_button("✖").clicked() {
                    remove_enc = Some(*eid);
                }
            });
        }
    }
    let mut add_enc = None;
    ui.menu_button("➕ Add encounter", |ui| {
        if doc.c.encounters.is_empty() {
            ui.weak("No encounters yet");
        }
        for e in &doc.c.encounters {
            if !enc_ids.contains(&e.id) && ui.button(&e.name).clicked() {
                add_enc = Some(e.id);
                ui.close();
            }
        }
    });
    if let Some(s) = doc.c.session_mut(id) {
        if let Some(r) = remove_enc {
            s.encounter_ids.retain(|e| *e != r);
        }
        if let Some(a) = add_enc {
            s.encounter_ids.push(a);
        }
    }

    // ---- recap & links
    section(ui, "Recap (what actually happened)");
    if let Some(s) = doc.c.session_mut(id) {
        ui_util::markdown_editor(ui, ("sessrecap", id), &mut s.recap, &mut sh.md, 6);
    }
    section(ui, "Links");
    let mut links = doc.c.session_mut(id).map(|s| std::mem::take(&mut s.links)).unwrap_or_default();
    ui_util::link_images(ui, &dir, &links, sh);
    ui_util::link_chips(ui, &doc.c, &mut links, sh);
    let vault = sh.vault.take();
    ui_util::link_picker(ui, ("sesslinks", id), &doc.c, &mut links, Some(&Link::Session(id)), &dir, vault.as_ref(), sh);
    sh.vault = vault;
    if let Some(s) = doc.c.session_mut(id) {
        s.links = links;
    }
}
