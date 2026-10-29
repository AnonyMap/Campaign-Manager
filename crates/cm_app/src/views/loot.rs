//! Loot generator: treasure for an encounter (pocket change or a hoard), for a challenge
//! rating, or worth a set amount of gold. Reroll lines, add items, then hand it out, save it
//! as a story card or copy it.

use cm_core::items::{Coins, Item, ItemKind, Rarity, format_gp};
use cm_core::loot::{self, Loot, LootLine, ValueOptions};
use cm_core::{CombatantSource, Id, Link, StoryCard};
use egui::RichText;

use crate::app::{Action, Doc, Shared};
use crate::ui_util;
use crate::views::treasure::{self, Handout, rarity_color};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Encounter,
    Cr,
    Value,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EncSource {
    Premade(Id),
    Active,
}

pub struct State {
    pub mode: Mode,
    pub source: Option<EncSource>,
    hoard: bool,
    cr: f32,
    creatures: u32,
    value: ValueOptions,
    srd_too: bool,
    current: Option<Loot>,
    from_encounter: Option<Id>,
    history: Vec<Loot>,
    handout: Handout,
    item_filter: String,
    add_gp: i64,
}

impl Default for State {
    fn default() -> Self {
        State {
            mode: Mode::Encounter,
            source: None,
            hoard: false,
            cr: 1.0,
            creatures: 4,
            value: ValueOptions::default(),
            srd_too: true,
            current: None,
            from_encounter: None,
            history: Vec::new(),
            handout: Handout::default(),
            item_filter: String::new(),
            add_gp: 10,
        }
    }
}

const CRS: [f32; 34] = [
    0.0, 0.125, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0, 21.0, 22.0, 23.0,
    24.0, 25.0, 26.0, 27.0, 28.0, 29.0, 30.0,
];

fn cr_label(c: f32) -> String {
    match c {
        x if (x - 0.125).abs() < 1e-3 => "1/8".into(),
        x if (x - 0.25).abs() < 1e-3 => "1/4".into(),
        x if (x - 0.5).abs() < 1e-3 => "1/2".into(),
        x => format!("{}", x as i32),
    }
}

/// Challenge ratings of the enemies in an encounter (allies and PCs left out), and how many
/// creatures had no CR.
fn encounter_crs(doc: &Doc, src: EncSource) -> (Vec<f32>, usize, String) {
    let mut crs = Vec::new();
    let mut unknown = 0;
    let name;
    match src {
        EncSource::Premade(id) => {
            let Some(e) = doc.c.encounter(id) else { return (crs, 0, String::new()) };
            name = e.name.clone();
            for en in e.entries.iter().filter(|en| !en.ally && !matches!(en.source, CombatantSource::Player(_))) {
                match doc.c.cr_for(&en.source).and_then(|c| loot::cr_number(&c)) {
                    Some(c) => crs.extend(std::iter::repeat_n(c, en.count.max(1) as usize)),
                    None => unknown += en.count.max(1) as usize,
                }
            }
        }
        EncSource::Active => {
            let Some(r) = &doc.c.active_encounter else { return (crs, 0, String::new()) };
            name = r.name.clone();
            for c in r.combatants.iter().filter(|c| !c.friendly()) {
                match doc.c.cr_for(&c.source).and_then(|c| loot::cr_number(&c)) {
                    Some(x) => crs.push(x),
                    None => unknown += 1,
                }
            }
        }
    }
    (crs, unknown, name)
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    if treasure::handout_window(ui.ctx(), doc, sh, &mut st.handout) {
        if let Some(l) = &mut st.current {
            l.title = format!("{} (handed out)", l.title.trim_end_matches(" (handed out)"));
        }
    }
    egui::SidePanel::left("loot_opts").resizable(true).default_width(300.0).show_inside(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("loot_opts_scroll").show(ui, |ui| options(ui, doc, sh, st));
    });
    egui::CentralPanel::default().frame(egui::Frame::new()).show_inside(ui, |ui| {
        ui_util::pinned_scroll(ui, "loot_result", |ui| result(ui, doc, sh, st));
    });
}

fn options(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    ui.heading("Loot generator");
    ui.weak("Dice tables and your item library — no AI needed.");
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut st.mode, Mode::Encounter, "For an encounter");
        ui.selectable_value(&mut st.mode, Mode::Cr, "By challenge");
        ui.selectable_value(&mut st.mode, Mode::Value, "By value");
    });
    ui.separator();
    let lib = if st.srd_too { treasure::library(doc) } else { doc.c.items.clone() };
    let mut generated: Option<Loot> = None;
    match st.mode {
        Mode::Encounter => {
            if st.source.is_none() {
                st.source = if doc.c.active_encounter.as_ref().map(|r| !r.combatants.is_empty()).unwrap_or(false) {
                    Some(EncSource::Active)
                } else {
                    doc.c.encounters.first().map(|e| EncSource::Premade(e.id))
                };
            }
            let label = match st.source {
                Some(EncSource::Active) => format!(
                    "Combat tracker: {}",
                    doc.c.active_encounter.as_ref().map(|r| r.name.clone()).unwrap_or_default()
                ),
                Some(EncSource::Premade(id)) => doc.c.encounter(id).map(|e| e.name.clone()).unwrap_or("(deleted)".into()),
                None => "Choose an encounter".into(),
            };
            egui::ComboBox::from_id_salt("loot_enc")
                .selected_text(label)
                .width(ui.available_width() - 30.0)
                .truncate()
                .show_ui(ui, |ui| {
                    if doc.c.active_encounter.as_ref().map(|r| !r.combatants.is_empty()).unwrap_or(false) {
                        ui.selectable_value(&mut st.source, Some(EncSource::Active), "The fight in the combat tracker");
                    }
                    for e in &doc.c.encounters {
                        ui.selectable_value(&mut st.source, Some(EncSource::Premade(e.id)), &e.name);
                    }
                });
            if doc.c.encounters.is_empty() && doc.c.active_encounter.is_none() {
                ui.weak("No encounters yet — use “By challenge” instead.");
            }
            ui.radio_value(&mut st.hoard, false, "Each enemy's pocket change")
                .on_hover_text("Coins per creature (by its CR), now and then a gem or a magic item");
            ui.radio_value(&mut st.hoard, true, "One treasure hoard")
                .on_hover_text("Coins, gems or art, and magic items, sized by the toughest enemy");
            if let Some(src) = st.source {
                let (crs, unknown, name) = encounter_crs(doc, src);
                if !crs.is_empty() {
                    let top = crs.iter().cloned().fold(0.0_f32, f32::max);
                    ui.weak(format!("{} enemies, toughest CR {}", crs.len(), cr_label(top)));
                }
                if unknown > 0 {
                    ui.weak(format!("{} without a CR are left out", ui_util::plural(unknown, "creature")));
                }
                if ui
                    .add_enabled(!crs.is_empty(), egui::Button::new(RichText::new("🎲 Generate loot").strong()))
                    .clicked()
                {
                    let mut l = loot::for_creatures(&lib, &crs, st.hoard, &mut sh.rng);
                    l.title = format!("Loot: {name}");
                    l.tidy();
                    st.from_encounter = if let EncSource::Premade(id) = src { Some(id) } else { None };
                    generated = Some(l);
                }
            }
        }
        Mode::Cr => {
            ui.horizontal(|ui| {
                ui.label("Challenge rating");
                egui::ComboBox::from_id_salt("loot_cr")
                    .selected_text(cr_label(st.cr))
                    .width(70.0)
                    .truncate()
                    .show_ui(ui, |ui| {
                        for c in CRS {
                            ui.selectable_value(&mut st.cr, c, cr_label(c));
                        }
                    });
            });
            ui.weak("Use the party's level for a hoard at the end of an adventure.");
            ui.radio_value(&mut st.hoard, true, "Treasure hoard");
            ui.radio_value(&mut st.hoard, false, "Pocket change for");
            if !st.hoard {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut st.creatures).range(1..=50));
                    ui.label("creatures");
                });
            }
            ui.weak(format!("Tier: {}", loot::TIER_LABELS[loot::tier_for(st.cr)]));
            if ui.button(RichText::new("🎲 Generate loot").strong()).clicked() {
                let crs = if st.hoard { vec![st.cr] } else { vec![st.cr; st.creatures as usize] };
                let mut l = loot::for_creatures(&lib, &crs, st.hoard, &mut sh.rng);
                l.tidy();
                st.from_encounter = None;
                generated = Some(l);
            }
        }
        Mode::Value => {
            ui.horizontal(|ui| {
                ui.label("Worth about");
                ui.add(egui::DragValue::new(&mut st.value.target_gp).range(1.0..=1_000_000.0).speed(10.0).suffix(" gp"));
            });
            ui.horizontal_wrapped(|ui| {
                for v in [50.0, 250.0, 1000.0, 5000.0, 20000.0] {
                    if ui.small_button(format_gp(v)).clicked() {
                        st.value.target_gp = v;
                    }
                }
            });
            ui.checkbox(&mut st.value.include_magic, "Include magic items");
            if st.value.include_magic {
                ui.horizontal(|ui| {
                    ui.label("up to");
                    egui::ComboBox::from_id_salt("loot_max_rarity")
                        .selected_text(st.value.max_rarity.label())
                        .show_ui(ui, |ui| {
                            for r in Rarity::MAGIC {
                                ui.selectable_value(&mut st.value.max_rarity, r, r.label());
                            }
                        });
                });
            }
            ui.checkbox(&mut st.value.include_valuables, "Include gems and art objects");
            if ui.button(RichText::new("🎲 Generate loot").strong()).clicked() {
                let mut l = loot::by_value(&lib, &st.value, &mut sh.rng);
                l.tidy();
                st.from_encounter = None;
                generated = Some(l);
            }
        }
    }
    ui.separator();
    ui.checkbox(&mut st.srd_too, "Use SRD 5.2 items as well as mine")
        .on_hover_text("Untick to draw magic items only from your own item library");
    if doc.c.items.is_empty() && !st.srd_too {
        ui.colored_label(ui.visuals().warn_fg_color, "Your item library is empty, so no magic items can be picked.");
    }
    if let Some(l) = generated {
        if let Some(old) = st.current.take() {
            st.history.insert(0, old);
            st.history.truncate(10);
        }
        st.current = Some(l);
    }
    if !st.history.is_empty() {
        ui.separator();
        ui.label(RichText::new("Earlier this session").small().strong());
        let mut restore = None;
        for (i, l) in st.history.iter().enumerate() {
            if ui.link(format!("{} · {}", l.title, format_gp(l.total_gp()))).clicked() {
                restore = Some(i);
            }
        }
        if let Some(i) = restore {
            let l = st.history.remove(i);
            if let Some(cur) = st.current.replace(l) {
                st.history.insert(0, cur);
            }
        }
    }
}

fn result(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    let Some(loot) = &mut st.current else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new("No loot yet").strong());
            ui.weak("Pick how to size it on the left and click Generate.");
        });
        return;
    };
    let lib = if st.srd_too { treasure::library(doc) } else { doc.c.items.clone() };
    ui.add(
        egui::TextEdit::singleline(&mut loot.title)
            .font(egui::TextStyle::Heading)
            .desired_width(f32::INFINITY),
    );
    ui.label(RichText::new(format!("Total value about {}", format_gp(loot.total_gp()))).strong());
    ui.add_space(4.0);
    let mut remove = None;
    let mut reroll = None;
    egui::Grid::new("loot_lines").num_columns(4).striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
        for (i, line) in loot.lines.iter_mut().enumerate() {
            match &line.item {
                Some(it) => {
                    let r = ui.label(RichText::new(line.label()).color(rarity_color(ui, it.rarity)));
                    let it2 = it.clone();
                    r.on_hover_ui(|ui| {
                        ui.set_max_width(380.0);
                        ui.label(RichText::new(it2.subtitle()).italics());
                        let d: String = it2.description.chars().take(400).collect();
                        ui.label(d);
                    });
                }
                None => {
                    ui.horizontal(|ui| {
                        for (label, v) in [
                            ("pp", &mut line.coins.pp),
                            ("gp", &mut line.coins.gp),
                            ("sp", &mut line.coins.sp),
                            ("cp", &mut line.coins.cp),
                        ] {
                            ui.add(egui::DragValue::new(v).range(0..=100_000_000).suffix(format!(" {label}")));
                        }
                    });
                }
            }
            ui.weak(format_gp(line.value_gp()));
            if line.item.is_some() {
                if ui.small_button("🎲").on_hover_text("Reroll this line (same rarity or value)").clicked() {
                    reroll = Some(i);
                }
            } else {
                ui.label("");
            }
            if ui.small_button("✖").clicked() {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        loot.lines.remove(i);
    }
    if let Some(i) = reroll {
        if let Some(old) = loot.lines[i].item.clone() {
            let new = match old.kind {
                ItemKind::Gem if !old.is_magic() => Some(loot::gem(&mut sh.rng, old.value_gp() / loot.lines[i].qty.max(1) as f64)),
                ItemKind::Art if !old.is_magic() => Some(loot::art(&mut sh.rng, old.value_gp() / loot.lines[i].qty.max(1) as f64)),
                _ if old.is_magic() => loot::random_magic_item(&lib, old.rarity, &mut sh.rng),
                _ => None,
            };
            if let Some(n) = new {
                let qty = if n.is_magic() { 1 } else { loot.lines[i].qty };
                loot.lines[i] = LootLine::item(n, qty);
            }
        }
    }
    ui.horizontal_wrapped(|ui| {
        let mut picked = None;
        ui_util::form_menu(ui, "➕ Add item…", |ui| {
            if let Some(it) = treasure::item_picker(ui, doc, &mut st.item_filter) {
                picked = Some(it);
                ui.close();
            }
        });
        if let Some(mut it) = picked {
            it.id = cm_core::new_id();
            loot.lines.push(LootLine::item(it, 1));
        }
        ui.add(egui::DragValue::new(&mut st.add_gp).range(1..=1_000_000).suffix(" gp"));
        if ui.button("➕ Add coins").clicked() {
            loot.lines.push(LootLine::coins(Coins::gp(st.add_gp)));
            loot.tidy();
        }
        if ui.button("➕ Add a gem").on_hover_text("A random gemstone worth 50 gp").clicked() {
            loot.lines.push(LootLine::item(loot::gem(&mut sh.rng, 50.0), 1));
        }
    });
    ui.add_space(8.0);
    ui.separator();
    let items: Vec<(Item, u32)> = loot.lines.iter().filter_map(|l| l.item.clone().map(|i| (i, l.qty))).collect();
    let coins = loot.coins();
    let title = loot.title.clone();
    let md = loot.as_markdown();
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(RichText::new("🎁 Hand out…").strong())
            .on_hover_text("Split the coins between the party and give each item to someone")
            .clicked()
        {
            st.handout.start(doc, &title, coins, items.clone());
        }
        if ui
            .button("🃏 Save as story card")
            .on_hover_text("A card with this loot listed — handy to plan treasure ahead")
            .clicked()
        {
            let card = StoryCard {
                title: title.clone(),
                body: md.clone(),
                status: cm_core::CardStatus::Planned,
                tags: vec!["loot".into()],
                in_ideas_pile: false,
                ..Default::default()
            };
            let id = card.id;
            doc.c.story_cards.push(card);
            sh.actions.push(Action::Open(Link::Card(id)));
        }
        if let Some(eid) = st.from_encounter {
            if ui.button("📋 Add to the encounter's notes").clicked() {
                if let Some(e) = doc.c.encounters.iter_mut().find(|e| e.id == eid) {
                    e.notes = format!("{}\n\n**{title}**\n\n{md}", e.notes.trim_end()).trim_start().to_string();
                    sh.toast("Added to the encounter's notes");
                }
            }
        }
        if ui.button("📄 Copy as text").clicked() {
            ui.ctx().copy_text(format!("{title}\n\n{md}"));
            sh.toast("Copied");
        }
    });
}
