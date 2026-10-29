//! Campaign overview: description, next session, open threads, recent and unscheduled cards.

use cm_core::{Link, SessionStatus, ThreadStatus};
use egui::RichText;

use crate::app::{Action, Doc, Shared};
use crate::ui_util;

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut doc.c.name)
                .font(egui::TextStyle::Heading)
                .desired_width(f32::INFINITY),
        );
        ui_util::markdown_editor(ui, "campaign_desc", &mut doc.c.description, &mut sh.md, 4);
        ui.separator();
        let c = &doc.c;
        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            ui.label(RichText::new("Next session").strong());
            let mut upcoming: Vec<_> = c.sessions.iter().filter(|s| s.status == SessionStatus::Planned).collect();
            upcoming.sort_by_key(|s| s.number);
            match upcoming.first() {
                Some(s) => {
                    if ui.link(s.display_title()).clicked() {
                        sh.actions.push(Action::Open(Link::Session(s.id)));
                    }
                    if let Some(d) = s.real_date.or_else(|| c.settings.session_schedule.date_for_session(s.number)) {
                        let days = (d - chrono::Local::now().date_naive()).num_days();
                        ui.weak(format!(
                            "{} ({})",
                            ui_util::fmt_date(ui, d),
                            match days {
                                0 => "today".to_string(),
                                1 => "tomorrow".to_string(),
                                d if d > 0 => format!("in {d} days"),
                                d => format!("{} days ago", -d),
                            }
                        ));
                    }
                    let cards = c.cards_for_session(s);
                    ui.weak(format!(
                        "{}, {} planned",
                        ui_util::plural(cards.len(), "card"),
                        ui_util::plural(s.encounter_ids.len(), "encounter")
                    ));
                    if ui
                        .button(RichText::new("▶ Run this session").strong())
                        .on_hover_text("Open the table view: plan, scenes, encounters, secrets, live notes")
                        .clicked()
                    {
                        sh.actions.push(Action::GoView(crate::app::View::Run));
                    }
                }
                None => {
                    ui.weak("No session planned.");
                }
            }
            ui.add_space(8.0);
            ui.label(RichText::new("In-world date").strong());
            ui.label(c.settings.game_calendar.format(&c.settings.game_calendar.current));
            ui.add_space(8.0);
            ui.label(RichText::new("Open plot threads").strong());
            for t in c.threads.iter().filter(|t| t.status == ThreadStatus::Open) {
                ui.horizontal(|ui| {
                    ui_util::swatch(ui, t.color);
                    if ui.link(&t.name).clicked() {
                        sh.actions.push(Action::Open(Link::Thread(t.id)));
                    }
                    let n = c.story_cards.iter().filter(|x| x.threads.contains(&t.id)).count();
                    ui.weak(ui_util::plural(n, "card"));
                });
            }
            if !c.threads.iter().any(|t| t.status == ThreadStatus::Open) {
                ui.weak("None.");
            }
            if let Some(r) = &c.active_encounter {
                if !r.combatants.is_empty() {
                    ui.add_space(8.0);
                    ui.label(RichText::new("Combat in progress").strong());
                    if ui.link(format!("{} — round {}", r.name, r.round)).clicked() {
                        sh.actions.push(Action::GoView(crate::app::View::Combat));
                    }
                }
            }

            let ui = &mut cols[1];
            ui.label(RichText::new("Ideas pile").strong());
            let mut ideas = c.ideas_pile();
            ideas.sort_by(|a, b| b.modified.cmp(&a.modified));
            for card in ideas.iter().take(12) {
                if ui.link(ui_util::card_title(card)).clicked() {
                    sh.actions.push(Action::Open(Link::Card(card.id)));
                }
            }
            if ideas.len() > 12 {
                ui.weak(format!("…and {} more", ideas.len() - 12));
            }
            if ideas.is_empty() {
                ui.weak("Empty.");
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Recently edited cards").strong());
            let mut recent: Vec<_> = c.story_cards.iter().collect();
            recent.sort_by(|a, b| b.modified.cmp(&a.modified));
            for card in recent.iter().take(8) {
                ui.horizontal(|ui| {
                    if ui.link(ui_util::card_title(card)).clicked() {
                        sh.actions.push(Action::Open(Link::Card(card.id)));
                    }
                    let m = card.modified.with_timezone(&chrono::Local);
                    ui.weak(format!("{} {}", ui_util::fmt_date_short(ui, m.date_naive()), m.format("%H:%M")));
                });
            }
            ui.add_space(8.0);
            ui.label(RichText::new("At a glance").strong());
            let p = ui_util::plural;
            ui.weak(
                [
                    p(c.story_cards.len(), "card"),
                    p(c.sessions.len(), "session"),
                    format!("{} PCs", c.players.len()),
                    format!("{} NPCs", c.npcs.len()),
                    p(c.factions.len(), "faction"),
                    p(c.encounters.len(), "encounter"),
                    p(c.maps.len(), "map"),
                    p(c.locations.len(), "place"),
                    p(c.items.len(), "item"),
                ]
                .join(" · "),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                use crate::views::transfer::Mode;
                if ui
                    .button("✨ New campaign from this one…")
                    .on_hover_text("Same world, new story: reuse NPCs, places, factions, items, the bestiary…")
                    .clicked()
                {
                    sh.actions.push(crate::app::Action::Transfer(Mode::NewFromThis));
                }
                if ui.button("📤 Export…").clicked() {
                    sh.actions.push(crate::app::Action::Transfer(Mode::Export));
                }
                if ui.button("📥 Import…").clicked() {
                    sh.actions.push(crate::app::Action::Transfer(Mode::Import));
                }
            });
        });
    });
}
