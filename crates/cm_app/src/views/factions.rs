//! Factions: organisations, their goals, attitude to the party and member NPCs.

use cm_core::{Attitude, Faction, Id, Link};

use crate::app::{Action, Doc, Shared};
use crate::ui_util::{self, section};

#[derive(Default)]
pub struct State {
    pub selected: Option<Id>,
    filter: String,
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    egui::SidePanel::left("fac_list").resizable(true).default_width(230.0).show_inside(ui, |ui| {
        ui.heading("Factions");
        if ui.button("➕ New faction").clicked() {
            let f = Faction::default();
            st.selected = Some(f.id);
            doc.c.factions.push(f);
        }
        ui_util::filter_box(ui, &mut st.filter, "Filter…");
        ui.separator();
        let f = st.filter.to_lowercase();
        egui::ScrollArea::vertical().id_salt("fac_list_scroll").show(ui, |ui| {
            for fac in &doc.c.factions {
                if !f.is_empty() && !fac.name.to_lowercase().contains(&f) {
                    continue;
                }
                let n = doc.c.faction_members(fac.id).len();
                ui.horizontal(|ui| {
                    ui_util::swatch(ui, fac.color);
                    if ui_util::list_row(ui, st.selected == Some(fac.id), format!("{}  ({n})", fac.name)).clicked() {
                        st.selected = Some(fac.id);
                    }
                });
            }
        });
    });
    let Some(id) = st.selected else {
        ui.centered_and_justified(|ui| ui.weak("Select or create a faction."));
        return;
    };
    if doc.c.faction(id).is_none() {
        st.selected = None;
        return;
    }
    let dir = doc.dir.clone();
    ui_util::pinned_scroll(ui, ("fac_edit", id), |ui| {
        let f = doc.c.factions.iter_mut().find(|f| f.id == id).unwrap();
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut f.name).font(egui::TextStyle::Heading).desired_width(320.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui_util::confirm_delete_button(ui, Link::Faction(id), sh);
                if ui.button("Export to wiki").clicked() {
                    sh.actions.push(Action::ExportToWiki(Link::Faction(id)));
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Attitude toward the party");
            egui::ComboBox::from_id_salt(("att", id)).selected_text(f.attitude.label()).show_ui(ui, |ui| {
                for a in Attitude::ALL {
                    ui.selectable_value(&mut f.attitude, a, a.label());
                }
            });
            ui.separator();
            ui.label("Colour");
            ui_util::color_opt(ui, &mut f.color);
        });
        ui.horizontal_top(|ui| match f.image.clone() {
            Some(p) => {
                ui.vertical(|ui| {
                    if ui_util::image_thumb(ui, &dir, &p, egui::vec2(140.0, 140.0)).clicked() {
                        sh.actions.push(Action::ViewImage(p));
                    }
                    if ui.small_button("Remove image").clicked() {
                        f.image = None;
                    }
                });
            }
            None => {
                if let Some(rel) = ui_util::add_image_button(ui, ("facimg", id), &dir, "Add emblem / image", sh) {
                    f.image = Some(rel);
                }
            }
        });
        section(ui, "Description");
        ui_util::markdown_editor(ui, ("facdesc", id), &mut f.description, &mut sh.md, 5);
        section(ui, "Goals");
        ui_util::markdown_editor(ui, ("facgoals", id), &mut f.goals, &mut sh.md, 4);
        section(ui, "Notes");
        ui_util::markdown_editor(ui, ("facnotes", id), &mut f.notes, &mut sh.md, 4);

        section(ui, "Members");
        let members: Vec<(Id, String)> = doc.c.faction_members(id).iter().map(|n| (n.id, n.core.display_name())).collect();
        let mut remove = None;
        ui.horizontal_wrapped(|ui| {
            for (nid, name) in &members {
                if ui.link(name).clicked() {
                    sh.actions.push(Action::Open(Link::Npc(*nid)));
                }
                if ui.small_button("✖").on_hover_text("Remove from faction").clicked() {
                    remove = Some(*nid);
                }
            }
            if members.is_empty() {
                ui.weak("No members yet.");
            }
        });
        let mut add = None;
        ui.menu_button("➕ Add member", |ui| {
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for n in &doc.c.npcs {
                    if !n.factions.contains(&id) && ui.button(n.core.display_name()).clicked() {
                        add = Some(n.id);
                        ui.close();
                    }
                }
            });
        });
        if let Some(r) = remove {
            if let Some(n) = doc.c.npc_mut(r) {
                n.factions.retain(|x| *x != id);
            }
        }
        if let Some(a) = add {
            if let Some(n) = doc.c.npc_mut(a) {
                n.factions.push(id);
            }
        }
        let places: Vec<(Id, String)> = doc
            .c
            .locations
            .iter()
            .filter(|l| l.factions.contains(&id))
            .map(|l| (l.id, doc.c.location_path(l.id)))
            .collect();
        if !places.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.weak("Present at:");
                for (lid, path) in places {
                    if ui.link(path).clicked() {
                        sh.actions.push(Action::Open(Link::Location(lid)));
                    }
                }
            });
        }
        crate::views::quests::quests_given(ui, doc, &Link::Faction(id), sh);
        section(ui, "Links");
        let mut links = doc
            .c
            .factions
            .iter_mut()
            .find(|f| f.id == id)
            .map(|f| std::mem::take(&mut f.links))
            .unwrap_or_default();
        ui_util::link_chips(ui, &doc.c, &mut links, sh);
        let vault = sh.vault.take();
        ui_util::link_picker(ui, ("faclinks", id), &doc.c, &mut links, Some(&Link::Faction(id)), &dir, vault.as_ref(), sh);
        sh.vault = vault;
        if let Some(f) = doc.c.factions.iter_mut().find(|f| f.id == id) {
            f.links = links;
        }
        ui_util::backlinks(ui, &doc.c, &Link::Faction(id), sh);
    });
}
