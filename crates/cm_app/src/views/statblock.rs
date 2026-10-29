//! Stat block rendering (the classic monster-manual layout with roll buttons) and the
//! stat block editor. Shared by the Bestiary, NPC pages and the combat tracker.

use cm_core::bestiary::{Attack, DamageRoll, Feature, SaveInfo, StatBlock};
use cm_core::{AbilityScores, ability_mod, fmt_mod};
use egui::text::LayoutJob;
use egui::{RichText, TextFormat};

use crate::app::{Action, Shared};

fn rule(ui: &mut egui::Ui) {
    let c = ui.visuals().hyperlink_color.gamma_multiply(0.6);
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(egui::vec2(w, 6.0), egui::Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, egui::Stroke::new(1.5_f32, c));
}

fn heading_color(ui: &egui::Ui) -> egui::Color32 {
    ui.visuals().hyperlink_color
}

/// "**Label** value" on one wrapped line.
fn prop(ui: &mut egui::Ui, label: &str, value: &str) {
    if value.trim().is_empty() {
        return;
    }
    let mut job = LayoutJob::default();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let strong = ui.visuals().strong_text_color();
    let text = ui.visuals().text_color();
    job.append(
        &format!("{label} "),
        0.0,
        TextFormat {
            font_id: font.clone(),
            color: strong,
            ..Default::default()
        },
    );
    job.append(
        value.trim(),
        0.0,
        TextFormat {
            font_id: font,
            color: text,
            ..Default::default()
        },
    );
    job.wrap.max_width = ui.available_width();
    ui.label(job);
}

/// Roll a d20 with a modifier via the dice panel.
fn roll(sh: &mut Shared, roller: &str, label: String, m: i32) {
    let expr = if m >= 0 { format!("1d20+{m}") } else { format!("1d20{m}") };
    sh.actions.push(Action::Roll {
        label,
        expr,
        roller: roller.to_string(),
    });
}

/// Read-only stat block. `roller` names who's rolling in the dice history.
pub fn render(ui: &mut egui::Ui, sb: &StatBlock, roller: &str, sh: &mut Shared, show_name: bool) {
    let c = &sb.core;
    if show_name {
        ui.label(RichText::new(sb.name()).heading().strong().color(heading_color(ui)));
    }
    let sub = sb.subtitle();
    if !sub.is_empty() {
        ui.label(RichText::new(sub).italics());
    }
    rule(ui);
    ui.horizontal_wrapped(|ui| {
        if let Some(ac) = c.ac {
            prop(
                ui,
                "AC",
                &if sb.ac_note.is_empty() {
                    ac.to_string()
                } else {
                    format!("{ac} ({})", sb.ac_note)
                },
            );
            ui.add_space(12.0);
        }
        let init = c.initiative_source();
        let m = match init {
            cm_core::InitSource::Assigned(v) | cm_core::InitSource::Dex(v) => Some(v),
            _ => None,
        };
        if let Some(m) = m {
            ui.label(RichText::new("Initiative").strong());
            if ui
                .small_button(format!("{} ({})", fmt_mod(m), 10 + m))
                .on_hover_text("Roll initiative")
                .clicked()
            {
                roll(sh, roller, "Initiative".into(), m);
            }
        }
    });
    let hp = match (c.max_hp, c.hp_formula.trim()) {
        (Some(h), "") => h.to_string(),
        (Some(h), f) => format!("{h} ({f})"),
        (None, f) => f.to_string(),
    };
    prop(ui, "HP", &hp);
    prop(ui, "Speed", &c.speed);
    rule(ui);
    // abilities: click to roll a check
    let has_abilities = (0..6).any(|i| c.abilities.get(i).is_some());
    if has_abilities {
        egui::Grid::new(("sb_abil", sb.id)).num_columns(6).spacing([10.0, 2.0]).show(ui, |ui| {
            for n in AbilityScores::NAMES {
                ui.label(RichText::new(n).small().strong());
            }
            ui.end_row();
            for i in 0..6 {
                match c.abilities.get(i) {
                    Some(v) => {
                        let m = ability_mod(v);
                        if ui
                            .small_button(format!("{v} ({})", fmt_mod(m)))
                            .on_hover_text(format!("Roll a {} check", AbilityScores::NAMES[i]))
                            .clicked()
                        {
                            roll(sh, roller, format!("{} check", AbilityScores::NAMES[i]), m);
                        }
                    }
                    None => {
                        ui.weak("—");
                    }
                }
            }
            ui.end_row();
        });
        rule(ui);
    }
    prop(ui, "Saving Throws", &sb.saves);
    prop(ui, "Skills", &sb.skills);
    prop(ui, "Vulnerabilities", &sb.damage_vulnerabilities);
    prop(ui, "Resistances", &sb.damage_resistances);
    prop(ui, "Immunities", &join_nonempty(&[&sb.damage_immunities, &sb.condition_immunities], "; "));
    let senses = match (sb.senses.trim(), c.passive_perception) {
        (s, Some(pp)) if !s.to_lowercase().contains("passive") => join_nonempty(&[s, &format!("Passive Perception {pp}")], "; "),
        (s, _) => s.to_string(),
    };
    prop(ui, "Senses", &senses);
    prop(ui, "Languages", &sb.languages);
    if !c.cr.trim().is_empty() {
        let xp = cm_core::encounter::xp_for_cr(&c.cr).map(|x| format!(" (XP {x}")).unwrap_or_else(|| " (".into());
        let pb = sb.proficiency_bonus.map(|p| format!("; PB {}", fmt_mod(p))).unwrap_or_default();
        let tail = if xp == " (" && pb.is_empty() {
            String::new()
        } else {
            format!("{xp}{pb})").replace("(; ", "(")
        };
        prop(ui, "CR", &format!("{}{tail}", c.cr.trim()));
    }
    for (title, list) in sb.sections() {
        if list.is_empty() {
            continue;
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(title)
                .size(ui.style().text_styles[&egui::TextStyle::Body].size + 3.0)
                .color(heading_color(ui)),
        );
        rule(ui);
        if title == "Legendary Actions" && !sb.legendary_intro.trim().is_empty() {
            ui.label(RichText::new(sb.legendary_intro.trim()).italics());
        }
        for f in list {
            feature(ui, f, roller, sh);
        }
    }
    if !sb.source.trim().is_empty() {
        ui.add_space(4.0);
        ui.weak(format!("Source: {}", sb.source));
    }
}

fn join_nonempty(parts: &[&str], sep: &str) -> String {
    parts.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(sep)
}

/// One trait/action: bold-italic name, text, then roll buttons for structured attacks.
pub fn feature(ui: &mut egui::Ui, f: &Feature, roller: &str, sh: &mut Shared) {
    let mut job = LayoutJob::default();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let strong = ui.visuals().strong_text_color();
    let text = ui.visuals().text_color();
    let name = if f.usage.trim().is_empty() {
        format!("{}. ", f.name.trim())
    } else {
        format!("{} ({}). ", f.name.trim(), f.usage.trim())
    };
    job.append(
        &name,
        0.0,
        TextFormat {
            font_id: font.clone(),
            color: strong,
            italics: true,
            ..Default::default()
        },
    );
    // Source text sometimes carries markdown emphasis (the SRD spell lists do); show it plain.
    let body = f.text.trim().replace("**", "").replace("__", "");
    job.append(
        &body,
        0.0,
        TextFormat {
            font_id: font,
            color: text,
            ..Default::default()
        },
    );
    job.wrap.max_width = ui.available_width();
    ui.label(job);
    let dmg: Vec<&DamageRoll> = f.damage.iter().filter(|d| cm_core::dice::DiceExpr::parse(&d.dice).is_ok()).collect();
    if f.attack.is_none() && dmg.is_empty() {
        ui.add_space(3.0);
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if let Some(a) = &f.attack {
            if ui
                .small_button(format!("🎲 to hit {}", fmt_mod(a.bonus)))
                .on_hover_text(format!("{} attack roll", f.name))
                .clicked()
            {
                roll(sh, roller, format!("{} (to hit)", f.name), a.bonus);
            }
        }
        if let Some(s) = &f.save {
            ui.label(RichText::new(format!("DC {} {}", s.dc, s.ability)).small().strong());
        }
        for d in &dmg {
            let label = if d.damage_type.is_empty() {
                d.dice.clone()
            } else {
                format!("{} {}", d.dice, d.damage_type)
            };
            if ui.small_button(format!("🎲 {label}")).on_hover_text("Roll damage").clicked() {
                sh.actions.push(Action::Roll {
                    label: format!("{} damage", f.name),
                    expr: d.dice.clone(),
                    roller: roller.to_string(),
                });
            }
        }
        if dmg.len() > 1 {
            let all: Vec<&str> = dmg.iter().map(|d| d.dice.as_str()).collect();
            if ui.small_button("🎲 all").on_hover_text("Roll every damage die together").clicked() {
                sh.actions.push(Action::Roll {
                    label: format!("{} damage", f.name),
                    expr: all.join("+"),
                    roller: roller.to_string(),
                });
            }
        }
    });
    ui.add_space(3.0);
}

// ---------------------------------------------------------------------------
// Editor for the parts of a stat block that aren't in CharacterCore
// ---------------------------------------------------------------------------

pub fn details_editor(ui: &mut egui::Ui, sb: &mut StatBlock) {
    let h = ui.spacing().interact_size.y;
    egui::Grid::new(("sb_details", sb.id)).num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
        let row = |ui: &mut egui::Ui, label: &str, v: &mut String, hint: &str| {
            ui.label(label);
            ui.add_sized([320.0, h], egui::TextEdit::singleline(v).hint_text(hint));
            ui.end_row();
        };
        row(ui, "Size", &mut sb.size, "Medium");
        row(ui, "Alignment", &mut sb.alignment, "Neutral Evil");
        row(ui, "AC note", &mut sb.ac_note, "natural armor");
        row(ui, "Saving throws", &mut sb.saves, "Dex +4, Wis +2");
        row(ui, "Skills", &mut sb.skills, "Perception +4, Stealth +6");
        row(ui, "Vulnerabilities", &mut sb.damage_vulnerabilities, "");
        row(ui, "Resistances", &mut sb.damage_resistances, "");
        row(ui, "Damage immunities", &mut sb.damage_immunities, "");
        row(ui, "Condition immunities", &mut sb.condition_immunities, "");
        row(ui, "Senses", &mut sb.senses, "Darkvision 60 ft.");
        row(ui, "Languages", &mut sb.languages, "Common, Goblin");
        row(ui, "Source", &mut sb.source, "Monster Manual p. 12");
        row(ui, "Collection", &mut sb.collection, "blank = My stat blocks");
        ui.label("Proficiency bonus");
        crate::ui_util::opt_int(ui, ("pb", sb.id), &mut sb.proficiency_bonus, 50.0);
        ui.end_row();
    });
}

/// Edit traits, actions, bonus actions, reactions and legendary actions.
pub fn features_editor(ui: &mut egui::Ui, sb: &mut StatBlock) {
    let id = sb.id;
    let lists: [(&str, &mut Vec<Feature>); 5] = [
        ("Traits", &mut sb.traits),
        ("Actions", &mut sb.actions),
        ("Bonus Actions", &mut sb.bonus_actions),
        ("Reactions", &mut sb.reactions),
        ("Legendary Actions", &mut sb.legendary_actions),
    ];
    for (title, list) in lists {
        egui::CollapsingHeader::new(format!("{title} ({})", list.len()))
            .id_salt((title, id))
            .default_open(!list.is_empty() && title != "Legendary Actions")
            .show(ui, |ui| {
                let mut remove = None;
                let mut up = None;
                let n = list.len();
                for (i, f) in list.iter_mut().enumerate() {
                    egui::Frame::group(ui.style()).inner_margin(6).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        feature_editor(ui, f, (title, id, i));
                        ui.horizontal(|ui| {
                            if i > 0 && ui.small_button("⬆ Move up").clicked() {
                                up = Some(i);
                            }
                            if ui.small_button("✖ Remove").clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                }
                let _ = n;
                if let Some(i) = remove {
                    list.remove(i);
                }
                if let Some(i) = up {
                    list.swap(i, i - 1);
                }
                let singular = title.trim_end_matches('s');
                if ui.button(format!("➕ Add {}", singular.to_lowercase())).clicked() {
                    list.push(Feature::default());
                }
            });
    }
    ui.horizontal(|ui| {
        ui.label("Legendary intro");
        ui.add(
            egui::TextEdit::singleline(&mut sb.legendary_intro)
                .hint_text("Legendary Action Uses: 3…")
                .desired_width(f32::INFINITY),
        );
    });
}

fn feature_editor(ui: &mut egui::Ui, f: &mut Feature, salt: (&str, cm_core::Id, usize)) {
    let h = ui.spacing().interact_size.y;
    ui.horizontal(|ui| {
        ui.add_sized([200.0, h], egui::TextEdit::singleline(&mut f.name).hint_text("Name (e.g. Scimitar)"));
        ui.add_sized([150.0, h], egui::TextEdit::singleline(&mut f.usage).hint_text("Usage (Recharge 5–6)"));
    });
    ui.add(
        egui::TextEdit::multiline(&mut f.text)
            .desired_rows(2)
            .desired_width(f32::INFINITY)
            .hint_text("Rules text as printed"),
    );
    ui.horizontal_wrapped(|ui| {
        let mut has_attack = f.attack.is_some();
        if ui.checkbox(&mut has_attack, "Attack").changed() {
            f.attack = if has_attack {
                Some(Attack {
                    bonus: 0,
                    kind: "melee".into(),
                    range: String::new(),
                })
            } else {
                None
            };
        }
        if let Some(a) = &mut f.attack {
            ui.add(egui::DragValue::new(&mut a.bonus).range(-10..=30).prefix("+").speed(0.1));
            egui::ComboBox::from_id_salt(("akind", salt))
                .selected_text(a.kind.clone())
                .width(110.0)
                .show_ui(ui, |ui| {
                    for k in ["melee", "ranged", "melee or ranged"] {
                        ui.selectable_value(&mut a.kind, k.to_string(), k);
                    }
                });
            ui.add_sized([130.0, h], egui::TextEdit::singleline(&mut a.range).hint_text("reach 5 ft."));
        }
        ui.separator();
        let mut has_save = f.save.is_some();
        if ui.checkbox(&mut has_save, "Save").changed() {
            f.save = if has_save { Some(SaveInfo { ability: "DEX".into(), dc: 12 }) } else { None };
        }
        if let Some(s) = &mut f.save {
            ui.label("DC");
            ui.add(egui::DragValue::new(&mut s.dc).range(1..=40));
            egui::ComboBox::from_id_salt(("sabl", salt))
                .selected_text(s.ability.clone())
                .width(60.0)
                .show_ui(ui, |ui| {
                    for a in ["STR", "DEX", "CON", "INT", "WIS", "CHA"] {
                        ui.selectable_value(&mut s.ability, a.to_string(), a);
                    }
                });
        }
    });
    let mut remove = None;
    for (i, d) in f.damage.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label("Damage");
            let ok = cm_core::dice::DiceExpr::parse(&d.dice).is_ok();
            let mut te = egui::TextEdit::singleline(&mut d.dice).hint_text("1d6+2").font(egui::TextStyle::Monospace);
            if !ok {
                te = te.text_color(ui.visuals().error_fg_color);
            }
            ui.add_sized([90.0, h], te);
            ui.add_sized([110.0, h], egui::TextEdit::singleline(&mut d.damage_type).hint_text("slashing"));
            if ui.small_button("✖").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        f.damage.remove(i);
    }
    ui.horizontal(|ui| {
        if ui.small_button("➕ Damage roll").clicked() {
            f.damage.push(DamageRoll::default());
        }
        if f.damage.is_empty() {
            if let Some(d) = cm_core::bestiary::find_dice(&f.text) {
                if ui.small_button(format!("Use {d} from text")).clicked() {
                    f.damage.push(DamageRoll {
                        dice: d,
                        damage_type: String::new(),
                    });
                }
            }
        }
    });
}

/// Compact one-line summary for lists and pickers: "Goblin Warrior — CR 1/4, AC 15, HP 10".
pub fn summary(sb: &StatBlock) -> String {
    let mut parts = Vec::new();
    if !sb.core.cr.trim().is_empty() {
        parts.push(format!("CR {}", sb.core.cr.trim()));
    }
    if let Some(ac) = sb.core.ac {
        parts.push(format!("AC {ac}"));
    }
    if let Some(hp) = sb.core.max_hp {
        parts.push(format!("HP {hp}"));
    }
    parts.join(", ")
}

/// Sort key for challenge ratings ("1/8" < "1/2" < "1" < "10").
pub fn cr_value(cr: &str) -> f32 {
    let t = cr.trim();
    if let Some((a, b)) = t.split_once('/') {
        let a: f32 = a.trim().parse().unwrap_or(0.0);
        let b: f32 = b.trim().parse().unwrap_or(1.0);
        return if b == 0.0 { 0.0 } else { a / b };
    }
    t.parse().unwrap_or(-1.0)
}

/// Searchable picker over the campaign's stat blocks and the SRD library. Returns the chosen
/// campaign stat block (an SRD pick is copied into the campaign's bestiary first).
pub fn picker(ui: &mut egui::Ui, doc: &mut crate::app::Doc, filter: &mut String) -> Option<cm_core::Id> {
    ui.set_min_width(300.0);
    let r = ui.add(
        egui::TextEdit::singleline(filter)
            .hint_text("Search stat blocks… (“cr 2” works)")
            .desired_width(f32::INFINITY),
    );
    // focus the search when the picker opens, without stealing focus from other fields
    if ui.memory(|m| m.focused().is_none()) {
        r.request_focus();
    }
    let f = filter.trim().to_lowercase();
    let hit = |s: &StatBlock| {
        if f.is_empty() {
            return true;
        }
        if let Some(cr) = f.strip_prefix("cr ") {
            return s.core.cr.trim() == cr.trim();
        }
        s.name().to_lowercase().contains(&f) || s.core.class_or_type.to_lowercase().contains(&f)
    };
    let label = |s: &StatBlock| {
        if s.core.cr.trim().is_empty() {
            s.name()
        } else {
            format!("{}  (CR {})", s.name(), s.core.cr.trim())
        }
    };
    let mut chosen = None;
    let mut srd_pick = None;
    egui::ScrollArea::vertical().max_height(320.0).auto_shrink([false, false]).show(ui, |ui| {
        let mut mine: Vec<&StatBlock> = doc.c.statblocks.iter().filter(|s| hit(s)).collect();
        mine.sort_by_key(|s| s.name().to_lowercase());
        ui.label(RichText::new("This campaign").small().strong());
        if mine.is_empty() {
            ui.weak(if doc.c.statblocks.is_empty() {
                "No stat blocks yet (see Bestiary)"
            } else {
                "No match"
            });
        }
        let total = mine.len();
        for s in mine.into_iter().take(60) {
            if ui
                .button(label(s))
                .on_hover_text(format!("{}\n{}", s.subtitle(), s.collection_label()))
                .clicked()
            {
                chosen = Some(s.id);
            }
        }
        if total > 60 {
            ui.weak(format!("…and {} more — type to search", total - 60));
        }
        ui.add_space(4.0);
        ui.label(RichText::new("SRD 5.2").small().strong());
        if f.chars().count() < 2 {
            ui.weak("Type to search 300+ SRD monsters");
        } else {
            let srd = crate::views::bestiary::srd();
            let mut n = 0;
            for s in srd.iter().filter(|s| hit(s)) {
                n += 1;
                if n > 40 {
                    ui.weak("…more — refine the search");
                    break;
                }
                if ui.button(label(s)).on_hover_text(s.subtitle()).clicked() {
                    srd_pick = Some(s.clone());
                }
            }
            if n == 0 {
                ui.weak("No match");
            }
        }
    });
    if let Some(s) = srd_pick {
        chosen = Some(crate::views::bestiary::adopt_srd(doc, &s).0);
    }
    chosen
}
