//! Player characters and NPCs/monsters. Both share the character editor (stats, images,
//! custom rolls, markdown notes); NPCs add factions, role, status and a name generator.

use cm_core::bestiary::StatBlock;
use cm_core::names::{self, NameRequest};
use cm_core::{AbilityScores, CharacterCore, CombatantSource, CustomRoll, Id, Link, Npc, NpcKind, PlayerCharacter, ability_mod, fmt_mod};
use egui::RichText;

use crate::app::{Action, Doc, Shared};
use crate::ui_util::{self, section};
use crate::views::{quests, treasure};

#[derive(Default)]
pub struct State {
    pub selected: Option<Id>,
    filter: String,
    kind_filter: Option<NpcKind>,
    faction_filter: Option<Id>,
    gen_pool: Option<String>,
    gen_background: Option<String>,
    gen_invented: bool,
    gen_no_surname: bool,
    generated: Vec<String>,
    new_roll_label: String,
    new_roll_expr: String,
    sb_filter: String,
    inv: crate::views::treasure::InventoryUi,
}

// ---------------------------------------------------------------------------
// Shared character editor
// ---------------------------------------------------------------------------

/// Editor for fields common to PCs and NPCs. `is_pc` switches labels (class vs type, level vs CR).
pub fn core_editor(
    ui: &mut egui::Ui,
    core: &mut CharacterCore,
    owner: Link,
    is_pc: bool,
    doc_dir: &std::path::Path,
    sh: &mut Shared,
    st: &mut State,
    history: &[cm_core::dice::RollRecord],
) {
    let id = owner.target_id().unwrap_or_default();
    ui.horizontal_top(|ui| {
        // portrait
        ui.vertical(|ui| {
            ui.set_width(130.0);
            match core.image.clone() {
                Some(p) => {
                    if ui_util::image_thumb(ui, doc_dir, &p, egui::vec2(120.0, 140.0)).clicked() {
                        sh.actions.push(Action::ViewImage(p));
                    }
                    ui.horizontal(|ui| {
                        if let Some(rel) = ui_util::add_image_button(ui, ("portrait", id), doc_dir, "Change", sh) {
                            core.image = Some(rel);
                        }
                        if ui.small_button("Remove").clicked() {
                            core.image = None;
                        }
                    });
                }
                None => {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 120.0), egui::Sense::hover());
                    ui.painter()
                        .rect_stroke(rect, 2.0, ui.visuals().widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "No portrait",
                        egui::FontId::proportional(12.0),
                        ui.visuals().weak_text_color(),
                    );
                    if let Some(rel) = ui_util::add_image_button(ui, ("portrait", id), doc_dir, "Add portrait / token", sh) {
                        core.image = Some(rel);
                    }
                }
            }
        });
        // basic stats grid
        ui.vertical(|ui| {
            egui::Grid::new(("core", id)).num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label(if is_pc { "Character name" } else { "Name" });
                ui.add_sized([220.0, ui.spacing().interact_size.y], egui::TextEdit::singleline(&mut core.name));
                ui.end_row();
                ui.label("Species");
                ui.add_sized([220.0, ui.spacing().interact_size.y], egui::TextEdit::singleline(&mut core.species));
                ui.end_row();
                ui.label(if is_pc { "Class" } else { "Type" });
                ui.add_sized(
                    [220.0, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut core.class_or_type).hint_text(if is_pc { "Fighter 3 / Rogue 1" } else { "Humanoid, Beast…" }),
                );
                ui.end_row();
                if is_pc {
                    ui.label("Level");
                    ui_util::opt_u32(ui, ("lvl", id), &mut core.level, 50.0);
                } else {
                    ui.label("CR");
                    ui.add_sized([50.0, ui.spacing().interact_size.y], egui::TextEdit::singleline(&mut core.cr).hint_text("1/4"));
                }
                ui.end_row();

                ui.label("AC");
                ui_util::opt_int(ui, ("ac", id), &mut core.ac, 50.0);
                ui.end_row();
                ui.label("Max HP");
                ui.horizontal(|ui| {
                    ui_util::opt_int(ui, ("hp", id), &mut core.max_hp, 50.0);
                    ui.add_sized(
                        [80.0, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(&mut core.hp_formula).hint_text("2d8+2"),
                    )
                    .on_hover_text("Hit dice formula. Used to roll or average HP.");
                    if !core.hp_formula.trim().is_empty() {
                        if ui.small_button("avg").on_hover_text("Set Max HP to the formula's average").clicked() {
                            if let Ok(e) = cm_core::dice::DiceExpr::parse(&core.hp_formula) {
                                if let Ok(a) = e.average() {
                                    core.max_hp = Some(a.floor() as i32);
                                }
                            }
                        }
                        if ui.small_button("roll").on_hover_text("Roll Max HP from the formula").clicked() {
                            match cm_core::dice::DiceExpr::parse(&core.hp_formula).and_then(|e| e.roll(&mut sh.rng)) {
                                Ok(o) => {
                                    core.max_hp = Some(o.total.max(1.0) as i32);
                                    sh.toast(format!("Rolled HP {} = {}", o.breakdown(), o.total));
                                }
                                Err(e) => sh.toast(format!("HP formula: {e}")),
                            }
                        }
                    }
                });
                ui.end_row();

                ui.label("Initiative mod");
                ui.horizontal(|ui| {
                    ui_util::opt_int(ui, ("init", id), &mut core.initiative_mod, 50.0)
                        .on_hover_text("Assigned initiative modifier. If blank, the DEX modifier is used when auto-rolling.");
                    if core.initiative_mod.is_none() {
                        ui.weak(core.initiative_source().describe());
                    }
                });
                ui.end_row();
                ui.label("Speed");
                ui.add_sized(
                    [120.0, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut core.speed).hint_text("30 ft."),
                );
                ui.end_row();
                ui.label("Passive Perception");
                ui_util::opt_int(ui, ("pp", id), &mut core.passive_perception, 50.0);
                ui.end_row();
            });
            ui.add_space(4.0);
            abilities_grid(ui, &mut core.abilities, id);
        });
    });

    section(ui, "Custom rolls");
    custom_rolls(ui, core, id, sh, st, history);

    section(ui, if is_pc { "Character sheet images" } else { "Stat block images" });
    sheet_images(ui, core, id, doc_dir, sh, is_pc);

    section(ui, "Notes");
    ui_util::markdown_editor(ui, ("notes", id), &mut core.notes, &mut sh.md, 8);

    egui::CollapsingHeader::new("Extra fields")
        .id_salt(("extra", id))
        .default_open(!core.extra_fields.is_empty())
        .show(ui, |ui| {
            let mut remove = None;
            egui::Grid::new(("xf", id)).num_columns(3).show(ui, |ui| {
                for (i, (k, v)) in core.extra_fields.iter_mut().enumerate() {
                    ui.add_sized(
                        [140.0, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(k).desired_width(140.0).hint_text("Field"),
                    );
                    ui.add_sized(
                        [260.0, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(v).desired_width(260.0).hint_text("Value"),
                    );
                    if ui.small_button("✖").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
            if let Some(i) = remove {
                core.extra_fields.remove(i);
            }
            if ui.button("➕ Add field").clicked() {
                core.extra_fields.push((String::new(), String::new()));
            }
        });
    ui.horizontal(|ui| {
        ui.label("Tags");
        ui_util::tags_edit(ui, ("tags", id), &mut core.tags);
    });
}

fn abilities_grid(ui: &mut egui::Ui, a: &mut AbilityScores, id: Id) {
    egui::Grid::new(("abil", id)).num_columns(6).spacing([6.0, 2.0]).show(ui, |ui| {
        for n in AbilityScores::NAMES {
            ui.label(RichText::new(n).small().strong());
        }
        ui.end_row();
        for i in 0..6 {
            ui_util::opt_int(ui, ("ab", id, i), a.get_mut(i), 40.0);
        }
        ui.end_row();
        for i in 0..6 {
            match a.get(i) {
                Some(v) => ui.weak(fmt_mod(ability_mod(v))),
                None => ui.weak(" "),
            };
        }
        ui.end_row();
    });
}

fn custom_rolls(ui: &mut egui::Ui, core: &mut CharacterCore, id: Id, sh: &mut Shared, st: &mut State, history: &[cm_core::dice::RollRecord]) {
    let roller = core.display_name();
    let mut remove = None;
    let mut up = None;
    egui::Grid::new(("rolls", id)).num_columns(4).striped(true).show(ui, |ui| {
        for (i, r) in core.custom_rolls.iter_mut().enumerate() {
            if ui.button("🎲").on_hover_text(format!("Roll {}", r.notation)).clicked() {
                sh.actions.push(Action::Roll {
                    label: r.label.clone(),
                    expr: r.notation.clone(),
                    roller: roller.clone(),
                });
            }
            let lw = (ui.available_width() * 0.35).clamp(160.0, 300.0);
            ui.add_sized(
                [lw, ui.spacing().interact_size.y],
                egui::TextEdit::singleline(&mut r.label).desired_width(lw).hint_text("Label"),
            )
            .on_hover_text(r.label.clone());
            let valid = cm_core::dice::DiceExpr::parse(&r.notation).is_ok() || r.notation.contains('#');
            let mut te = egui::TextEdit::singleline(&mut r.notation)
                .desired_width(140.0)
                .font(egui::TextStyle::Monospace);
            if !valid {
                te = te.text_color(ui.visuals().error_fg_color);
            }
            ui.add(te);
            ui.horizontal(|ui| {
                if i > 0 && ui.small_button("⬆").clicked() {
                    up = Some(i);
                }
                if ui.small_button("✖").clicked() {
                    remove = Some(i);
                }
            });
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        core.custom_rolls.remove(i);
    }
    if let Some(i) = up {
        core.custom_rolls.swap(i, i - 1);
    }
    ui.horizontal(|ui| {
        ui.add_sized(
            [140.0, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut st.new_roll_label)
                .hint_text("Label (e.g. Bite)")
                .desired_width(140.0),
        );
        ui.add_sized(
            [100.0, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut st.new_roll_expr)
                .hint_text("1d6+2")
                .desired_width(100.0)
                .font(egui::TextStyle::Monospace),
        );
        let ok = cm_core::dice::DiceExpr::parse(&st.new_roll_expr).is_ok();
        if ui.add_enabled(ok, egui::Button::new("➕ Add roll")).clicked() {
            core.custom_rolls.push(CustomRoll {
                label: if st.new_roll_label.trim().is_empty() {
                    st.new_roll_expr.trim().to_string()
                } else {
                    st.new_roll_label.trim().to_string()
                },
                notation: st.new_roll_expr.trim().to_string(),
                ..Default::default()
            });
            st.new_roll_label.clear();
            st.new_roll_expr.clear();
        }
        ui.menu_button("From dice history ⏷", |ui| {
            if history.is_empty() {
                ui.weak("No rolls yet");
            }
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for r in history.iter().rev().take(40) {
                    let text = if r.label.is_empty() {
                        r.expression.clone()
                    } else {
                        format!("{} — {}", r.label, r.expression)
                    };
                    if ui.button(text).clicked() {
                        let label = if st.new_roll_label.trim().is_empty() {
                            if r.label.is_empty() { r.expression.clone() } else { r.label.clone() }
                        } else {
                            st.new_roll_label.trim().to_string()
                        };
                        core.upsert_custom_roll(&label, &r.expression);
                        st.new_roll_label.clear();
                        ui.close();
                    }
                }
            });
        });
    });
}

fn sheet_images(ui: &mut egui::Ui, core: &mut CharacterCore, id: Id, dir: &std::path::Path, sh: &mut Shared, is_pc: bool) {
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (i, p) in core.sheet_images.iter().enumerate() {
            ui.vertical(|ui| {
                if ui_util::image_thumb(ui, dir, p, egui::vec2(220.0, 260.0)).clicked() {
                    sh.actions.push(Action::ViewImage(p.clone()));
                }
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
    });
    if let Some(i) = remove {
        core.sheet_images.remove(i);
    }
    let label = if is_pc { "➕ Add sheet image" } else { "➕ Add stat block image" };
    if let Some(rel) = ui_util::add_image_button(ui, ("sheet", id), dir, label, sh) {
        core.sheet_images.push(rel);
    }
}

// ---------------------------------------------------------------------------
// Player characters
// ---------------------------------------------------------------------------

pub fn show_players(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    egui::SidePanel::left("pc_list").resizable(true).default_width(240.0).show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Player characters");
        });
        if ui.button("➕ New player character").clicked() {
            let pc = PlayerCharacter::default();
            st.selected = Some(pc.id);
            doc.c.players.push(pc);
        }
        ui_util::filter_box(ui, &mut st.filter, "Filter…");
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let f = st.filter.to_lowercase();
            for p in &doc.c.players {
                let name = p.core.display_name();
                if !f.is_empty() && !name.to_lowercase().contains(&f) && !p.player_name.to_lowercase().contains(&f) {
                    continue;
                }
                let mut text = RichText::new(format!(
                    "{name}{}",
                    if p.player_name.is_empty() {
                        String::new()
                    } else {
                        format!("  ({})", p.player_name)
                    }
                ));
                if !p.active {
                    text = text.weak();
                }
                if ui_util::list_row(ui, st.selected == Some(p.id), text).clicked() {
                    st.selected = Some(p.id);
                }
            }
        });
    });
    let Some(id) = st.selected else {
        ui.centered_and_justified(|ui| ui.weak("Select or create a player character"));
        return;
    };
    // only the recent rolls are offered ("From dice history"), so only those are copied
    let history: Vec<cm_core::dice::RollRecord> = {
        let h = &doc.c.dice.history;
        h[h.len().saturating_sub(40)..].to_vec()
    };
    if doc.c.player(id).is_none() {
        st.selected = None;
        return;
    }
    let dir = doc.dir.clone();
    ui_util::pinned_scroll(ui, ("pc_edit", id), |ui| {
        let p = doc.c.player_mut(id).unwrap();
        ui.horizontal(|ui| {
            ui.heading(p.core.display_name());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui_util::confirm_delete_button(ui, Link::Player(id), sh);
                if ui.button("Export to wiki").clicked() {
                    sh.actions.push(Action::ExportToWiki(Link::Player(id)));
                }
                if ui.button("⚔ Add to combat").clicked() {
                    sh.actions.push(Action::AddToCombat(CombatantSource::Player(id)));
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Player");
            ui.add_sized(
                [200.0, ui.spacing().interact_size.y],
                egui::TextEdit::singleline(&mut p.player_name).desired_width(200.0).hint_text("Player's name"),
            );
            ui.checkbox(&mut p.active, "Active in campaign");
        });
        ui.separator();
        core_editor(ui, &mut p.core, Link::Player(id), true, &dir, sh, st, &history);
        inventory(ui, doc, sh, st, Link::Player(id), true);
        quests::quests_given(ui, doc, &Link::Player(id), sh);
        section(ui, "Links");
        let p = doc.c.player_mut(id).unwrap();
        let mut links = std::mem::take(&mut p.core.links);
        ui_util::link_images(ui, &dir, &links, sh);
        ui_util::link_chips(ui, &doc.c, &mut links, sh);
        let vault = sh.vault.take();
        ui_util::link_picker(ui, ("pclinks", id), &doc.c, &mut links, Some(&Link::Player(id)), &dir, vault.as_ref(), sh);
        sh.vault = vault;
        if let Some(p) = doc.c.player_mut(id) {
            p.core.links = links;
        }
        ui_util::backlinks(ui, &doc.c, &Link::Player(id), sh);
    });
}

// ---------------------------------------------------------------------------
// NPCs
// ---------------------------------------------------------------------------

pub fn show_npcs(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    // the name generator's pool list includes the campaign's custom pools
    let pools: Vec<String> = doc.c.settings.name_pools.iter().map(|p| p.name.clone()).collect();
    ui.data_mut(|d| d.insert_temp(egui::Id::new("custom_pools"), pools));
    egui::SidePanel::left("npc_list").resizable(true).default_width(250.0).show_inside(ui, |ui| {
        ui.heading("NPCs & monsters");
        ui.horizontal(|ui| {
            if ui.button("➕ NPC").clicked() {
                let n = Npc::default();
                st.selected = Some(n.id);
                doc.c.npcs.push(n);
            }
            if ui.button("➕ Monster").clicked() {
                let n = Npc {
                    kind: NpcKind::Monster,
                    ..Default::default()
                };
                st.selected = Some(n.id);
                doc.c.npcs.push(n);
            }
        });
        ui_util::filter_box(ui, &mut st.filter, "Filter by name, role, place…");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut st.kind_filter, None, "All");
            ui.selectable_value(&mut st.kind_filter, Some(NpcKind::Character), "Characters");
            ui.selectable_value(&mut st.kind_filter, Some(NpcKind::Monster), "Monsters");
        });
        if !doc.c.factions.is_empty() {
            egui::ComboBox::from_id_salt("npc_fac_filter")
                .selected_text(
                    st.faction_filter
                        .and_then(|f| doc.c.faction(f))
                        .map(|f| f.name.clone())
                        .unwrap_or("Any faction".into()),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut st.faction_filter, None, "Any faction");
                    for f in &doc.c.factions {
                        ui.selectable_value(&mut st.faction_filter, Some(f.id), &f.name);
                    }
                });
        }
        ui.separator();
        {
            let f = st.filter.to_lowercase();
            let mut list: Vec<&Npc> = doc
                .c
                .npcs
                .iter()
                .filter(|n| st.kind_filter.map(|k| n.kind == k).unwrap_or(true))
                .filter(|n| st.faction_filter.map(|fid| n.factions.contains(&fid)).unwrap_or(true))
                .filter(|n| {
                    f.is_empty()
                        || n.core.name.to_lowercase().contains(&f)
                        || n.role.to_lowercase().contains(&f)
                        || n.location.to_lowercase().contains(&f)
                        || n.core.tags.iter().any(|t| t.to_lowercase().contains(&f))
                })
                .collect();
            list.sort_by_key(|n| n.core.name.to_lowercase());
            // only the rows on screen are laid out (campaigns can have hundreds of NPCs)
            let row_h = ui.spacing().interact_size.y;
            egui::ScrollArea::vertical().id_salt("npc_rows").show_rows(ui, row_h, list.len(), |ui, range| {
                for n in &list[range] {
                    let mut text = RichText::new(format!(
                        "{}{}",
                        n.core.display_name(),
                        if let (NpcKind::Monster, Some(cr)) = (n.kind, doc.c.cr_for(&CombatantSource::Npc(n.id))) {
                            format!("  CR {cr}")
                        } else {
                            String::new()
                        }
                    ));
                    if n.status == cm_core::LifeStatus::Dead {
                        text = text.strikethrough().weak();
                    }
                    let r = ui_util::list_row(ui, st.selected == Some(n.id), text);
                    if r.clicked() {
                        st.selected = Some(n.id);
                    }
                    if !n.role.is_empty() {
                        r.on_hover_text(&n.role);
                    }
                }
            });
        }
    });

    let Some(id) = st.selected else {
        name_generator(ui, doc, sh, st, None);
        return;
    };
    // only the recent rolls are offered ("From dice history"), so only those are copied
    let history: Vec<cm_core::dice::RollRecord> = {
        let h = &doc.c.dice.history;
        h[h.len().saturating_sub(40)..].to_vec()
    };
    let factions: Vec<(Id, String)> = doc.c.factions.iter().map(|f| (f.id, f.name.clone())).collect();
    let dir = doc.dir.clone();
    if doc.c.npc(id).is_none() {
        st.selected = None;
        return;
    }
    ui_util::pinned_scroll(ui, ("npc_edit", id), |ui| {
        let n = doc.c.npc_mut(id).unwrap();
        ui.horizontal(|ui| {
            ui.heading(n.core.display_name());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui_util::confirm_delete_button(ui, Link::Npc(id), sh);
                if ui.button("Export to wiki").clicked() {
                    sh.actions.push(Action::ExportToWiki(Link::Npc(id)));
                }
                if ui.button("⚔ Add to combat").clicked() {
                    sh.actions.push(Action::AddToCombat(CombatantSource::Npc(id)));
                }
                if ui.button("Duplicate").clicked() {
                    let mut copy = n.clone();
                    copy.id = cm_core::new_id();
                    copy.core.name = format!("{} (copy)", copy.core.name);
                    sh.actions.push(Action::Toast("Duplicated".into()));
                    st.selected = Some(copy.id);
                    // pushed after the borrow ends
                    ui.data_mut(|d| d.insert_temp(egui::Id::new("npc_dup"), Some(copy)));
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Kind");
            ui.selectable_value(&mut n.kind, NpcKind::Character, "Character");
            ui.selectable_value(&mut n.kind, NpcKind::Monster, "Monster");
            ui.separator();
            ui.label("Status");
            egui::ComboBox::from_id_salt(("npcstatus", id))
                .selected_text(n.status.label())
                .show_ui(ui, |ui| {
                    for s in cm_core::LifeStatus::ALL {
                        ui.selectable_value(&mut n.status, s, s.label());
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("Role");
            ui.add_sized(
                [200.0, ui.spacing().interact_size.y],
                egui::TextEdit::singleline(&mut n.role).desired_width(200.0).hint_text("Innkeeper, villain…"),
            );
            ui.label("Location");
            ui.add_sized(
                [200.0, ui.spacing().interact_size.y],
                egui::TextEdit::singleline(&mut n.location).desired_width(200.0),
            );
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Factions");
            let mut remove = None;
            for f in &n.factions {
                let name = factions.iter().find(|(i, _)| i == f).map(|(_, n)| n.clone()).unwrap_or("(deleted)".into());
                if ui.small_button(format!("{name} ✖")).on_hover_text("Remove from faction").clicked() {
                    remove = Some(*f);
                }
            }
            if let Some(r) = remove {
                n.factions.retain(|f| *f != r);
            }
            ui.menu_button("➕", |ui| {
                if factions.is_empty() {
                    ui.weak("No factions yet — create one under Factions");
                }
                for (fid, name) in &factions {
                    if !n.factions.contains(fid) && ui.button(name).clicked() {
                        n.factions.push(*fid);
                        ui.close();
                    }
                }
            });
        });
        statblock_row(ui, doc, sh, st, id);
        let n = doc.c.npc_mut(id).unwrap();
        ui.horizontal(|ui| {
            ui_util::form_menu(ui, "🎲 Name generator", |ui| {
                ui.set_min_width(320.0);
                gen_controls(ui, st, &doc_pools(&n.core.species));
                if st.generated.is_empty() {
                    ui.weak("Click Generate");
                }
                for g in st.generated.clone() {
                    if ui.button(&g).on_hover_text("Use this name").clicked() {
                        n.core.name = g;
                        ui.close();
                    }
                }
            });
        });
        ui.separator();
        core_editor(ui, &mut n.core, Link::Npc(id), false, &dir, sh, st, &history);
        inventory(ui, doc, sh, st, Link::Npc(id), false);
        places_of(ui, doc, sh, id);
        quests::quests_given(ui, doc, &Link::Npc(id), sh);
        section(ui, "Links");
        let n = doc.c.npc_mut(id).unwrap();
        let mut links = std::mem::take(&mut n.core.links);
        ui_util::link_images(ui, &dir, &links, sh);
        ui_util::link_chips(ui, &doc.c, &mut links, sh);
        let vault = sh.vault.take();
        ui_util::link_picker(ui, ("npclinks", id), &doc.c, &mut links, Some(&Link::Npc(id)), &dir, vault.as_ref(), sh);
        sh.vault = vault;
        if let Some(n) = doc.c.npc_mut(id) {
            n.core.links = links;
        }
        ui_util::backlinks(ui, &doc.c, &Link::Npc(id), sh);
    });
    // Handle generator "apply" from a fresh name request and duplicates.
    if let Some(Some(copy)) = ui.data_mut(|d| d.remove_temp::<Option<Npc>>(egui::Id::new("npc_dup"))) {
        doc.c.npcs.push(copy);
    }
    // custom pools are needed by the generator
    let pools = doc.c.settings.name_pools.clone();
    if let Some(req) = ui.data_mut(|d| d.remove_temp::<bool>(egui::Id::new("gen_req"))) {
        let _ = req;
        st.generated = names::generate_many(&request(st), &pools, 10, &mut sh.rng);
    }
}

/// Coins and carried items, collapsed for NPCs that carry nothing.
fn inventory(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, owner: Link, is_pc: bool) {
    let Some(core) = doc.c.character_core(&owner) else { return };
    let n: u32 = core.inventory.iter().map(|e| e.qty.max(1)).sum();
    let summary = if n == 0 { String::new() } else { ui_util::plural(n as usize, "item") };
    let coins = if core.coins.is_zero() { String::new() } else { core.coins.label() };
    let empty = core.inventory.is_empty() && core.coins.is_zero();
    let full = treasure::inventory_summary(&core.inventory);
    let mut title = "🎒 Inventory".to_string();
    for part in [coins, summary] {
        if !part.is_empty() {
            title.push_str(" · ");
            title.push_str(&part);
        }
    }
    ui.add_space(6.0);
    egui::CollapsingHeader::new(RichText::new(title).strong())
        .id_salt(("inv", owner.target_id()))
        .default_open(is_pc || !empty)
        .show(ui, |ui| treasure::inventory_editor(ui, doc, &owner, sh, &mut st.inv))
        .header_response
        .on_hover_text(if full.is_empty() { "Nothing carried".to_string() } else { full });
}

/// Places that list this NPC.
fn places_of(ui: &mut egui::Ui, doc: &Doc, sh: &mut Shared, id: Id) {
    let places: Vec<(Id, String, bool)> = doc
        .c
        .locations
        .iter()
        .filter(|l| l.npcs.contains(&id) || l.shop.as_ref().and_then(|s| s.keeper) == Some(id))
        .map(|l| (l.id, doc.c.location_path(l.id), l.shop.as_ref().and_then(|s| s.keeper) == Some(id)))
        .collect();
    if places.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.weak("Found at:");
        for (lid, path, keeper) in places {
            let text = if keeper { format!("{path} (shopkeeper)") } else { path };
            if ui.link(text).clicked() {
                sh.actions.push(Action::Open(Link::Location(lid)));
            }
        }
    });
}

/// The stat blocks an NPC uses: a main one (fills in the NPC's blank numbers) and any
/// number of other forms (wild shape, a second phase, an older version).
fn statblock_row(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, id: Id) {
    enum Op {
        Add(Id),
        Replace(Id, Id),
        Remove(Id),
        Main(Id),
    }
    let ids = doc.c.npc(id).map(|n| n.statblock_ids()).unwrap_or_default();
    let blocks: Vec<StatBlock> = ids.iter().filter_map(|s| doc.c.statblock(*s).cloned()).collect();
    let mut op: Option<Op> = None;
    let cr_of = |sb: &StatBlock| {
        if sb.core.cr.trim().is_empty() {
            String::new()
        } else {
            format!("  (CR {})", sb.core.cr.trim())
        }
    };
    // a label column, so a second stat block lines up under the first
    let label_w = {
        let font = egui::TextStyle::Body.resolve(ui.style());
        ui.fonts_mut(|f| f.layout_no_wrap("Stat blocks".into(), font, egui::Color32::WHITE).size().x)
    };
    let label_cell = |ui: &mut egui::Ui, text: &str| {
        let h = ui.spacing().interact_size.y;
        ui.allocate_ui_with_layout(egui::vec2(label_w, h), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(label_w);
            ui.label(text);
        });
    };
    if blocks.is_empty() {
        ui.horizontal(|ui| {
            ui.label("Stat block");
            ui_util::form_menu(ui, "Assign…", |ui| {
                if let Some(sid) = crate::views::statblock::picker(ui, doc, &mut st.sb_filter) {
                    op = Some(Op::Add(sid));
                    ui.close();
                }
            })
            .on_hover_text("Use a bestiary / SRD stat block for this NPC's combat stats");
            ui.weak("none — this NPC's own numbers are used");
        });
    }
    for (i, sb) in blocks.iter().enumerate() {
        ui.horizontal(|ui| {
            label_cell(
                ui,
                match (i, blocks.len()) {
                    (0, 1) => "Stat block",
                    (0, _) => "Stat blocks",
                    _ => "",
                },
            );
            if ui
                .link(RichText::new(format!("🐉 {}{}", sb.name(), cr_of(sb))).strong())
                .on_hover_text("Open in the Bestiary")
                .clicked()
            {
                sh.actions.push(Action::Open(Link::StatBlock(sb.id)));
            }
            if blocks.len() > 1 {
                if i == 0 {
                    ui.weak("main")
                        .on_hover_text("Fills in this NPC's blank AC, HP, initiative and CR, and is used when it joins a fight");
                } else if ui
                    .small_button("Make main")
                    .on_hover_text("Use this one by default (the current main one becomes another form)")
                    .clicked()
                {
                    op = Some(Op::Main(sb.id));
                }
            }
            ui_util::form_menu(ui, "Change…", |ui| {
                if let Some(sid) = crate::views::statblock::picker(ui, doc, &mut st.sb_filter) {
                    op = Some(Op::Replace(sb.id, sid));
                    ui.close();
                }
            });
            if ui
                .small_button("✖")
                .on_hover_text("Stop using this stat block (it stays in the Bestiary)")
                .clicked()
            {
                op = Some(Op::Remove(sb.id));
            }
        });
    }
    if !blocks.is_empty() {
        ui.horizontal(|ui| {
            label_cell(ui, "");
            ui_util::form_menu(ui, "➕ Add another stat block…", |ui| {
                if let Some(sid) = crate::views::statblock::picker(ui, doc, &mut st.sb_filter) {
                    op = Some(Op::Add(sid));
                    ui.close();
                }
            })
            .on_hover_text("Another form, phase or version of this NPC: a druid's wild shape, a villain's second phase, a younger self. Switch between them in the combat tracker.");
        });
        if blocks.len() > 1 {
            ui.weak("Blank AC, Max HP, initiative and CR below fall back to the main stat block. The other forms use their own numbers; pick one when adding this NPC to a fight, or switch mid-fight in the combat tracker.");
        } else {
            ui.weak("Blank AC, Max HP, initiative and CR below fall back to the stat block. Fill them in to override.");
        }
        let roller = doc.c.npc(id).map(|n| n.core.display_name()).unwrap_or_default();
        for sb in &blocks {
            let title = if blocks.len() > 1 {
                format!("Show {} · {}", sb.name(), crate::views::statblock::summary(sb))
            } else {
                format!("Show stat block · {}", crate::views::statblock::summary(sb))
            };
            egui::CollapsingHeader::new(title)
                .id_salt(("npc_sb", id, sb.id))
                .default_open(false)
                .show(ui, |ui| {
                    ui.set_max_width(ui.available_width().min(620.0));
                    crate::views::statblock::render(ui, sb, &roller, sh, true);
                });
        }
    }
    if let (Some(op), Some(n)) = (op, doc.c.npc_mut(id)) {
        match op {
            Op::Add(s) => n.add_statblock(s),
            Op::Replace(old, new) => n.replace_statblock(old, new),
            Op::Remove(s) => n.remove_statblock(s),
            Op::Main(s) => n.set_main_statblock(s),
        }
    }
}

fn doc_pools(species: &str) -> Option<String> {
    names::BUILTIN_POOLS
        .iter()
        .find(|p| species.to_lowercase().contains(&p.name.split(' ').next().unwrap_or("").to_lowercase()) && !species.is_empty())
        .map(|p| p.name.to_string())
}

fn request(st: &State) -> NameRequest {
    NameRequest {
        pool: st.gen_pool.clone(),
        background: st.gen_background.clone(),
        invented: st.gen_invented,
        surname: !st.gen_no_surname,
    }
}

fn gen_controls(ui: &mut egui::Ui, st: &mut State, species_hint: &Option<String>) {
    if st.gen_pool.is_none() {
        if let Some(h) = species_hint {
            st.gen_pool = Some(h.clone());
        }
    }
    egui::Grid::new("gen_grid").num_columns(2).show(ui, |ui| {
        ui.label("Species / pool");
        egui::ComboBox::from_id_salt("gen_pool")
            .selected_text(st.gen_pool.clone().unwrap_or("Any".into()))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut st.gen_pool, None, "Any");
                for p in names::BUILTIN_POOLS {
                    ui.selectable_value(&mut st.gen_pool, Some(p.name.to_string()), p.name);
                }
                let custom: Vec<String> = ui.data(|d| d.get_temp::<Vec<String>>(egui::Id::new("custom_pools"))).unwrap_or_default();
                if !custom.is_empty() {
                    ui.separator();
                    for c in custom {
                        ui.selectable_value(&mut st.gen_pool, Some(c.clone()), format!("{c} (custom)"));
                    }
                }
            });
        ui.end_row();
        ui.label("Background");
        egui::ComboBox::from_id_salt("gen_bg")
            .selected_text(st.gen_background.clone().unwrap_or("None".into()))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut st.gen_background, None, "None");
                for b in names::BACKGROUNDS {
                    ui.selectable_value(&mut st.gen_background, Some(b.name.to_string()), b.name);
                }
            });
        ui.end_row();
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut st.gen_invented, "Invent new first names");
        ui.checkbox(&mut st.gen_no_surname, "No surname");
    });
    if ui.button("Generate").clicked() {
        ui.data_mut(|d| d.insert_temp(egui::Id::new("gen_req"), true));
    }
}

/// Standalone name generator shown when no NPC is selected.
fn name_generator(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, _sel: Option<Id>) {
    ui.heading("Name generator");
    ui.weak(format!(
        "{} built-in species pools, {} backgrounds, {}+ name combinations, plus invented names and your own pools (Settings » Name pools).",
        names::BUILTIN_POOLS.len(),
        names::BACKGROUNDS.len(),
        names::builtin_combinations()
    ));
    let custom: Vec<String> = doc.c.settings.name_pools.iter().map(|p| p.name.clone()).collect();
    ui.data_mut(|d| d.insert_temp(egui::Id::new("custom_pools"), custom));
    gen_controls(ui, st, &None);
    if ui.data_mut(|d| d.remove_temp::<bool>(egui::Id::new("gen_req"))).is_some() {
        st.generated = names::generate_many(&request(st), &doc.c.settings.name_pools, 15, &mut sh.rng);
    }
    ui.separator();
    for g in st.generated.clone() {
        ui.horizontal(|ui| {
            ui.label(&g);
            if ui.small_button("Create NPC").clicked() {
                let mut n = Npc::default();
                n.core.name = g.clone();
                if let Some(p) = &st.gen_pool {
                    if names::builtin_pool(p).is_some() {
                        n.core.species = p.split(' ').next().unwrap_or("").to_string();
                    }
                }
                st.selected = Some(n.id);
                doc.c.npcs.push(n);
            }
            if ui.small_button("Copy").clicked() {
                ui.ctx().copy_text(g.clone());
            }
        });
    }
    ui.add_space(20.0);
    ui.weak("Select an NPC on the left to edit it.");
}
