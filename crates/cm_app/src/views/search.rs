//! Global search across the campaign and the linked wiki.

use egui::RichText;

use crate::app::{Action, Doc, Shared};

#[derive(Default)]
pub struct State {
    pub query: String,
    /// Results for (query, when): searched again when the query changes or after a while.
    cache: Option<(String, f64, Vec<cm_core::search::CampaignHit>, Vec<(String, String, String)>)>,
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    ui.horizontal(|ui| {
        ui.heading("Search");
        ui.add_sized(
            [320.0, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut st.query).desired_width(320.0).hint_text("Search…"),
        );
    });
    ui.separator();
    let fresh = st.cache.as_ref().map(|(q, t, _, _)| *q == st.query && sh.now - t < 3.0).unwrap_or(false);
    if !fresh {
        let hits = cm_core::search::search(&doc.c, &st.query);
        let wiki: Vec<(String, String, String)> = sh
            .vault
            .as_ref()
            .map(|v| v.search(&st.query).into_iter().take(200).map(|h| (h.rel_path, h.title, h.snippet)).collect())
            .unwrap_or_default();
        st.cache = Some((st.query.clone(), sh.now, hits, wiki));
    }
    let (_, _, hits, wiki_hits) = st.cache.clone().unwrap_or_default();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.label(RichText::new(format!("Campaign ({})", hits.len())).strong());
        if hits.is_empty() {
            ui.weak("No matches.");
        }
        egui::Grid::new("search_hits").striped(true).num_columns(3).show(ui, |ui| {
            for h in hits.iter().take(200) {
                ui.weak(h.kind);
                if ui.link(&h.name).clicked() {
                    sh.actions.push(Action::Open(h.link.clone()));
                }
                ui.weak(&h.context);
                ui.end_row();
            }
        });
        ui.add_space(10.0);
        if sh.vault.is_some() {
            ui.label(RichText::new(format!("Wiki ({})", wiki_hits.len())).strong());
            egui::Grid::new("search_wiki").striped(true).num_columns(2).show(ui, |ui| {
                for (rel, title, snippet) in &wiki_hits {
                    if ui.link(title).on_hover_text(rel).clicked() {
                        sh.actions.push(Action::OpenWiki(rel.clone()));
                    }
                    ui.weak(snippet);
                    ui.end_row();
                }
            });
        } else {
            ui.weak("No wiki linked — wiki pages aren't included. (Open “Campaign wiki” once to load it.)");
        }
    });
}
