//! Plot threads: long-running storylines that story cards belong to.

use cm_core::{CardStatus, Id, Link, PlotThread, ThreadStatus};
use egui::RichText;

use crate::app::{Action, Doc, Shared};
use crate::ui_util::{self, section};

#[derive(Default)]
pub struct State {
    pub selected: Option<Id>,
    show_closed: bool,
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    egui::SidePanel::left("thread_list").resizable(true).default_width(240.0).show_inside(ui, |ui| {
        ui.heading("Plot threads");
        ui.horizontal(|ui| {
            if ui.button("➕ New thread").clicked() {
                let t = PlotThread::default();
                st.selected = Some(t.id);
                doc.c.threads.push(t);
            }
            ui.checkbox(&mut st.show_closed, "Show resolved");
        });
        ui.separator();
        egui::ScrollArea::vertical().id_salt("thread_list_scroll").show(ui, |ui| {
            for status in ThreadStatus::ALL {
                if !st.show_closed && matches!(status, ThreadStatus::Resolved | ThreadStatus::Abandoned) {
                    continue;
                }
                let list: Vec<&PlotThread> = doc.c.threads.iter().filter(|t| t.status == status).collect();
                if list.is_empty() {
                    continue;
                }
                ui.label(RichText::new(status.label()).small().strong());
                for t in list {
                    let n = doc.c.story_cards.iter().filter(|c| c.threads.contains(&t.id)).count();
                    ui.horizontal(|ui| {
                        ui_util::swatch(ui, t.color);
                        if ui_util::list_row(ui, st.selected == Some(t.id), format!("{}  ({n})", t.name)).clicked() {
                            st.selected = Some(t.id);
                        }
                    });
                }
            }
        });
    });
    let Some(id) = st.selected else {
        ui.centered_and_justified(|ui| ui.weak("Threads group story cards into storylines and become rows on the Plot line."));
        return;
    };
    if doc.c.thread(id).is_none() {
        st.selected = None;
        return;
    }
    let dir = doc.dir.clone();
    ui_util::pinned_scroll(ui, ("thread_edit", id), |ui| {
        let t = doc.c.threads.iter_mut().find(|t| t.id == id).unwrap();
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut t.name).font(egui::TextStyle::Heading).desired_width(320.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui_util::confirm_delete_button(ui, Link::Thread(id), sh);
            });
        });
        ui.horizontal(|ui| {
            ui.label("Status");
            for s in ThreadStatus::ALL {
                ui.selectable_value(&mut t.status, s, s.label());
            }
            ui.separator();
            ui.label("Colour");
            ui_util::color_opt(ui, &mut t.color);
        });
        section(ui, "Description");
        ui_util::markdown_editor(ui, ("threaddesc", id), &mut t.description, &mut sh.md, 6);

        section(ui, "Cards in this thread");
        let mut cards: Vec<_> = doc.c.story_cards.iter().filter(|c| c.threads.contains(&id)).collect();
        cards.sort_by_key(|c| c.status as u8);
        if cards.is_empty() {
            ui.weak("None yet. Add a thread to a card from the card's Details.");
        }
        egui::Grid::new(("tcards", id)).striped(true).num_columns(3).show(ui, |ui| {
            for c in cards {
                if ui.link(c.display_title()).clicked() {
                    sh.actions.push(Action::Open(Link::Card(c.id)));
                }
                ui.label(c.status.label());
                ui.weak(c.timing.kind_label());
                ui.end_row();
            }
        });
        let done = doc
            .c
            .story_cards
            .iter()
            .filter(|c| c.threads.contains(&id) && c.status == CardStatus::Done)
            .count();
        let total = doc.c.story_cards.iter().filter(|c| c.threads.contains(&id)).count();
        if total > 0 {
            ui.add(
                egui::ProgressBar::new(done as f32 / total as f32)
                    .text(format!("{done}/{total} cards done"))
                    .desired_width(240.0),
            );
        }
        if ui.button("➕ New card in this thread").clicked() {
            let mut card = cm_core::StoryCard::default();
            card.threads.push(id);
            let cid = card.id;
            doc.c.story_cards.push(card);
            sh.actions.push(Action::Open(Link::Card(cid)));
        }
        section(ui, "Links");
        let mut links = doc
            .c
            .threads
            .iter_mut()
            .find(|t| t.id == id)
            .map(|t| std::mem::take(&mut t.links))
            .unwrap_or_default();
        ui_util::link_chips(ui, &doc.c, &mut links, sh);
        let vault = sh.vault.take();
        ui_util::link_picker(ui, ("threadlinks", id), &doc.c, &mut links, Some(&Link::Thread(id)), &dir, vault.as_ref(), sh);
        sh.vault = vault;
        if let Some(t) = doc.c.threads.iter_mut().find(|t| t.id == id) {
            t.links = links;
        }
    });
}
