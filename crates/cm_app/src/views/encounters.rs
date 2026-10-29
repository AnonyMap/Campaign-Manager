//! Encounter builder: premade encounters made of PCs, NPCs/monsters and one-off creatures.

use cm_core::encounter::estimate_difficulty;
use cm_core::{CombatantSource, Encounter, EncounterEntry, Id, Link, NpcKind};
use egui::RichText;

use crate::app::{Action, Doc, Shared};
use crate::ui_util::{self, section};

#[derive(Default)]
pub struct State {
    pub selected: Option<Id>,
    npc_filter: String,
    sb_filter: String,
    picking: bool,
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    egui::SidePanel::left("enc_list").resizable(true).default_width(240.0).show_inside(ui, |ui| {
        ui.heading("Encounters");
        if ui.button("➕ New encounter").clicked() {
            let e = Encounter::default();
            st.selected = Some(e.id);
            doc.c.encounters.push(e);
        }
        ui.separator();
        egui::ScrollArea::vertical().id_salt("enc_list_scroll").show(ui, |ui| {
            for e in &doc.c.encounters {
                let n: u32 = e.entries.iter().map(|x| x.count.max(1)).sum();
                if ui_util::list_row(ui, st.selected == Some(e.id), format!("{}  ({n})", e.name)).clicked() {
                    st.selected = Some(e.id);
                }
            }
        });
    });
    let Some(id) = st.selected else {
        ui.centered_and_justified(|ui| ui.weak("Build reusable encounters here, then Run them in the Combat tracker."));
        return;
    };
    if doc.c.encounter(id).is_none() {
        st.selected = None;
        return;
    }
    let dir = doc.dir.clone();
    ui_util::pinned_scroll(ui, ("enc_edit", id), |ui| {
        // snapshot names
        let active_pcs: Vec<CombatantSource> = doc.c.players.iter().filter(|p| p.active).map(|p| CombatantSource::Player(p.id)).collect();
        let names: Vec<(CombatantSource, String, Option<i32>, Option<i32>, String)> = doc
            .c
            .players
            .iter()
            .map(|p| (CombatantSource::Player(p.id), p.core.display_name(), p.core.max_hp, p.core.ac, String::new()))
            .chain(doc.c.npcs.iter().map(|n| {
                let src = CombatantSource::Npc(n.id);
                let sb = doc.c.statblock_for(&src);
                (
                    src.clone(),
                    n.core.display_name(),
                    n.core.max_hp.or(sb.and_then(|s| s.core.max_hp)),
                    n.core.ac.or(sb.and_then(|s| s.core.ac)),
                    doc.c.cr_for(&src).unwrap_or_default(),
                )
            }))
            .chain(
                doc.c
                    .statblocks
                    .iter()
                    .map(|s| (CombatantSource::StatBlock(s.id), s.name(), s.core.max_hp, s.core.ac, s.core.cr.clone())),
            )
            .collect();
        let party_levels: Vec<u32> = doc.c.players.iter().filter(|p| p.active).filter_map(|p| p.core.level).collect();
        let estimate = estimate_difficulty(doc.c.encounter(id).unwrap(), &doc.c, &party_levels);
        let sessions: Vec<(Id, String, bool)> = doc
            .c
            .sessions
            .iter()
            .map(|s| (s.id, s.display_title(), s.encounter_ids.contains(&id)))
            .collect();
        let mut add_to_session = None;

        let e = doc.c.encounters.iter_mut().find(|e| e.id == id).unwrap();
        ui.add(
            egui::TextEdit::singleline(&mut e.name)
                .font(egui::TextStyle::Heading)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal_wrapped(|ui| {
            if ui.button(RichText::new("⚔ Run encounter").strong()).clicked() {
                sh.actions.push(Action::StartEncounter(id));
            }
            if ui.button("💰 Loot").on_hover_text("Generate treasure for this encounter's enemies").clicked() {
                sh.actions.push(Action::LootFor(Some(id)));
            }
            ui.menu_button("Add to session", |ui| {
                if sessions.is_empty() {
                    ui.weak("No sessions yet");
                }
                for (sid, name, has) in &sessions {
                    if !has && ui.button(name).clicked() {
                        add_to_session = Some(*sid);
                        ui.close();
                    }
                }
            });
            if ui.button("Export to wiki").clicked() {
                sh.actions.push(Action::ExportToWiki(Link::Encounter(id)));
            }
            ui_util::confirm_delete_button(ui, Link::Encounter(id), sh);
        });
        ui.horizontal(|ui| {
            ui.label("Location");
            ui.add_sized(
                [300.0, ui.spacing().interact_size.y],
                egui::TextEdit::singleline(&mut e.location).desired_width(300.0),
            );
        });

        section(ui, "Combatants");
        let mut remove = None;
        egui::Grid::new(("entries", id))
            .striped(true)
            .num_columns(8)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Count").small().strong());
                ui.label(RichText::new("Name").small().strong());
                ui.label(RichText::new("HP").small().strong());
                ui.label(RichText::new("AC").small().strong());
                ui.label(RichText::new("Init / CR").small().strong());
                ui.label(RichText::new("Side").small().strong());
                ui.label(RichText::new("Initiative").small().strong());
                ui.label("");
                ui.end_row();
                for (i, en) in e.entries.iter_mut().enumerate() {
                    ui.add(egui::DragValue::new(&mut en.count).range(1..=99));
                    match &en.source {
                        CombatantSource::AdHoc => {
                            ui.add_sized(
                                [180.0, ui.spacing().interact_size.y],
                                egui::TextEdit::singleline(&mut en.name).hint_text("Creature name").desired_width(180.0),
                            );
                            ui_util::opt_int(ui, ("ehp", en.id), &mut en.max_hp, 50.0);
                            ui_util::opt_int(ui, ("eac", en.id), &mut en.ac, 40.0);
                            ui_util::opt_int(ui, ("einit", en.id), &mut en.initiative_mod, 40.0);
                        }
                        src => {
                            let info = names.iter().find(|n| n.0 == *src);
                            match info {
                                Some((_, name, hp, ac, cr)) => {
                                    if ui.link(name).clicked() {
                                        if let Some(l) = src.link() {
                                            sh.actions.push(Action::Open(l));
                                        }
                                    }
                                    ui.label(hp.map(|h| h.to_string()).unwrap_or("—".into()));
                                    ui.label(ac.map(|h| h.to_string()).unwrap_or("—".into()));
                                    ui.weak(if cr.is_empty() { String::new() } else { format!("CR {cr}") });
                                }
                                None => {
                                    ui.weak("(deleted)");
                                    ui.label("");
                                    ui.label("");
                                    ui.label("");
                                }
                            }
                        }
                    }
                    if matches!(en.source, CombatantSource::Player(_)) {
                        ui.weak("party");
                        ui.label("");
                    } else {
                        let ally_color = if ui.visuals().dark_mode {
                            egui::Color32::from_rgb(110, 200, 120)
                        } else {
                            egui::Color32::from_rgb(20, 120, 40)
                        };
                        let label = if en.ally {
                            RichText::new("Ally").color(ally_color)
                        } else {
                            RichText::new("Enemy")
                        };
                        if ui
                            .add(egui::Button::new(label).small())
                            .on_hover_text("Click to switch enemy / ally. Allies don't count towards difficulty.")
                            .clicked()
                        {
                            en.ally = !en.ally;
                        }
                        if en.count > 1 {
                            ui.checkbox(&mut en.group_initiative, "roll together")
                                .on_hover_text("All copies share one initiative roll");
                        } else {
                            ui.label("");
                        }
                    }
                    if ui.small_button("✖").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
        if let Some(i) = remove {
            e.entries.remove(i);
        }
        if e.entries.is_empty() {
            ui.weak("Add player characters, NPCs, stat blocks or one-off creatures below.");
        }
        let mut add: Vec<CombatantSource> = Vec::new();
        let mut add_adhoc = false;
        let mut pick_sb = false;
        ui.horizontal_wrapped(|ui| {
            if ui.button("➕ All active PCs").clicked() {
                for src in &active_pcs {
                    if !e.entries.iter().any(|x| x.source == *src) {
                        add.push(src.clone());
                    }
                }
            }
            ui_util::form_menu(ui, "➕ NPC", |ui| {
                ui.set_min_width(260.0);
                ui.add(egui::TextEdit::singleline(&mut st.npc_filter).hint_text("Filter…"));
                let f = st.npc_filter.to_lowercase();
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    for (src, name, _, _, cr) in names.iter().filter(|n| matches!(n.0, CombatantSource::Npc(_))) {
                        if !f.is_empty() && !name.to_lowercase().contains(&f) {
                            continue;
                        }
                        let label = if cr.is_empty() { name.clone() } else { format!("{name}  (CR {cr})") };
                        if ui.button(label).clicked() {
                            add.push(src.clone());
                        }
                    }
                });
                ui.weak("Click again to add more of the same.");
            });
            if ui
                .button("➕ Stat block")
                .on_hover_text("Monsters from your bestiary or the SRD (Goblin ×4…)")
                .clicked()
            {
                pick_sb = true;
            }
            ui.menu_button("➕ Player character", |ui| {
                for (src, name, ..) in names.iter().filter(|n| matches!(n.0, CombatantSource::Player(_))) {
                    if !e.entries.iter().any(|x| x.source == *src) && ui.button(name).clicked() {
                        add.push(src.clone());
                        ui.close();
                    }
                }
            });
            if ui.button("➕ One-off creature").clicked() {
                add_adhoc = true;
            }
        });
        for src in add {
            if let Some(x) = e.entries.iter_mut().find(|x| x.source == src && !matches!(src, CombatantSource::Player(_))) {
                x.count += 1;
            } else {
                e.entries.push(EncounterEntry {
                    source: src,
                    ..Default::default()
                });
            }
        }
        if add_adhoc {
            e.entries.push(EncounterEntry {
                source: CombatantSource::AdHoc,
                name: "Creature".into(),
                ..Default::default()
            });
        }
        if pick_sb {
            st.picking = true;
        }

        // difficulty
        ui.add_space(6.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("Difficulty: {}", estimate.rating)).strong());
                ui.weak(format!(
                    "Monster XP {} vs party budget Low {} / Moderate {} / High {} ({} active PCs with levels)",
                    estimate.monster_xp, estimate.low, estimate.moderate, estimate.high, estimate.party_size
                ));
                if estimate.unknown_cr > 0 {
                    ui.weak(format!("· {} creatures without a CR aren't counted", estimate.unknown_cr));
                }
            })
            .response
            .on_hover_text("2024 DMG XP-budget method. A rough guide only.");
        });

        section(ui, "Description");
        ui_util::markdown_editor(ui, ("encdesc", id), &mut e.description, &mut sh.md, 4);
        section(ui, "Notes / tactics / treasure");
        ui_util::markdown_editor(ui, ("encnotes", id), &mut e.notes, &mut sh.md, 4);
        section(ui, "Links");
        let mut links = std::mem::take(&mut e.links);
        ui_util::link_chips(ui, &doc.c, &mut links, sh);
        let vault = sh.vault.take();
        ui_util::link_picker(ui, ("enclinks", id), &doc.c, &mut links, Some(&Link::Encounter(id)), &dir, vault.as_ref(), sh);
        sh.vault = vault;
        if let Some(e) = doc.c.encounters.iter_mut().find(|e| e.id == id) {
            e.links = links;
        }
        ui_util::backlinks(ui, &doc.c, &Link::Encounter(id), sh);
        if let Some(s) = add_to_session {
            if let Some(sess) = doc.c.session_mut(s) {
                sess.encounter_ids.push(id);
            }
        }
        let _ = NpcKind::Monster;
    });
    if st.picking {
        let mut open = true;
        let mut chosen = None;
        egui::Window::new("Add stat block")
            .open(&mut open)
            .collapsible(false)
            .default_width(320.0)
            .show(ui.ctx(), |ui| {
                chosen = crate::views::statblock::picker(ui, doc, &mut st.sb_filter);
                ui.weak("Click again to add more of the same.");
            });
        if let Some(sid) = chosen {
            if let Some(e) = doc.c.encounters.iter_mut().find(|e| e.id == id) {
                let src = CombatantSource::StatBlock(sid);
                match e.entries.iter_mut().find(|x| x.source == src) {
                    Some(x) => x.count += 1,
                    None => e.entries.push(EncounterEntry {
                        source: src,
                        ..Default::default()
                    }),
                }
            }
        }
        if !open {
            st.picking = false;
        }
    }
}
