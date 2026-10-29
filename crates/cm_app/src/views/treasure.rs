//! Shared pieces for items and treasure: the SRD item list, an item picker, a character
//! picker, the "hand out" window (loot and quest rewards), the inventory editor and the
//! read-only item card.

use std::sync::OnceLock;

use cm_core::items::{self, Coins, InventoryEntry, Item, Rarity, format_gp};
use cm_core::{Id, Link};
use egui::RichText;

use crate::app::{Action, Doc, Shared};
use crate::ui_util;

/// The bundled SRD items, parsed once.
pub fn srd_items() -> &'static [Item] {
    static SRD: OnceLock<Vec<Item>> = OnceLock::new();
    SRD.get_or_init(items::srd_items)
}

/// Campaign items first, then SRD items whose name isn't already in the campaign.
pub fn library(doc: &Doc) -> Vec<Item> {
    let mut v = doc.c.items.clone();
    for i in srd_items() {
        if !v.iter().any(|x| x.name.eq_ignore_ascii_case(&i.name)) {
            v.push(i.clone());
        }
    }
    v
}

pub fn rarity_color(ui: &egui::Ui, r: Rarity) -> egui::Color32 {
    let dark = ui.visuals().dark_mode;
    match r {
        Rarity::Mundane => ui.visuals().text_color(),
        Rarity::Common => ui.visuals().weak_text_color(),
        Rarity::Uncommon => {
            if dark {
                egui::Color32::from_rgb(110, 200, 120)
            } else {
                egui::Color32::from_rgb(30, 120, 40)
            }
        }
        Rarity::Rare => {
            if dark {
                egui::Color32::from_rgb(110, 160, 240)
            } else {
                egui::Color32::from_rgb(30, 80, 190)
            }
        }
        Rarity::VeryRare => {
            if dark {
                egui::Color32::from_rgb(190, 130, 240)
            } else {
                egui::Color32::from_rgb(120, 50, 170)
            }
        }
        Rarity::Legendary | Rarity::Artifact => {
            if dark {
                egui::Color32::from_rgb(240, 170, 80)
            } else {
                egui::Color32::from_rgb(170, 90, 0)
            }
        }
    }
}

/// Searchable item picker (campaign + SRD). Returns a copy of the chosen item.
pub fn item_picker(ui: &mut egui::Ui, doc: &Doc, filter: &mut String) -> Option<Item> {
    ui.set_min_width(320.0);
    let r = ui.add(
        egui::TextEdit::singleline(filter)
            .hint_text("Search items… (name, type, rarity)")
            .desired_width(f32::INFINITY),
    );
    if ui.memory(|m| m.focused().is_none()) {
        r.request_focus();
    }
    let f = filter.trim().to_lowercase();
    let hit =
        |i: &Item| f.is_empty() || i.name.to_lowercase().contains(&f) || i.kind.label().to_lowercase().contains(&f) || i.rarity.label().to_lowercase() == f;
    let mut chosen = None;
    egui::ScrollArea::vertical().max_height(320.0).auto_shrink([false, false]).show(ui, |ui| {
        let mine: Vec<&Item> = doc.c.items.iter().filter(|i| hit(i)).collect();
        ui.label(RichText::new("This campaign").small().strong());
        if mine.is_empty() {
            ui.weak(if doc.c.items.is_empty() { "No items yet (see Items)" } else { "No match" });
        }
        for i in mine.iter().take(60) {
            if item_button(ui, i).clicked() {
                chosen = Some((*i).clone());
            }
        }
        ui.add_space(4.0);
        ui.label(RichText::new("SRD 5.2").small().strong());
        if f.chars().count() < 2 {
            ui.weak("Type to search 700+ SRD items");
        } else {
            let mut n = 0;
            for i in srd_items().iter().filter(|i| hit(i)) {
                n += 1;
                if n > 50 {
                    ui.weak("…more — refine the search");
                    break;
                }
                if item_button(ui, i).clicked() {
                    chosen = Some(i.clone());
                }
            }
            if n == 0 {
                ui.weak("No match");
            }
        }
    });
    chosen
}

fn item_button(ui: &mut egui::Ui, i: &Item) -> egui::Response {
    let price = format_gp(i.value_gp());
    ui.button(RichText::new(format!("{}  ·  {price}", i.name)).color(rarity_color(ui, i.rarity)))
        .on_hover_text(i.subtitle())
}

/// Characters you can give things to: active PCs, then other PCs and NPCs.
pub fn characters(doc: &Doc) -> Vec<(Link, String)> {
    let mut v: Vec<(Link, String)> = doc
        .c
        .players
        .iter()
        .filter(|p| p.active)
        .map(|p| (Link::Player(p.id), p.core.display_name()))
        .collect();
    v.extend(
        doc.c
            .players
            .iter()
            .filter(|p| !p.active)
            .map(|p| (Link::Player(p.id), format!("{} (inactive)", p.core.display_name()))),
    );
    let mut npcs: Vec<(Link, String)> = doc.c.npcs.iter().map(|n| (Link::Npc(n.id), n.core.display_name())).collect();
    npcs.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    v.extend(npcs);
    v
}

/// A menu body listing characters; returns the one clicked.
pub fn character_menu(ui: &mut egui::Ui, doc: &Doc, filter: &mut String) -> Option<Link> {
    ui.set_min_width(220.0);
    let all = characters(doc);
    if all.len() > 12 {
        ui.add(egui::TextEdit::singleline(filter).hint_text("Find a character…").desired_width(f32::INFINITY));
    }
    let f = filter.to_lowercase();
    let mut out = None;
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        let mut shown_npc_header = false;
        for (l, name) in &all {
            if !f.is_empty() && !name.to_lowercase().contains(&f) {
                continue;
            }
            if matches!(l, Link::Npc(_)) && !shown_npc_header {
                ui.separator();
                ui.label(RichText::new("NPCs").small().strong());
                shown_npc_header = true;
            }
            if ui.button(name).clicked() {
                out = Some(l.clone());
            }
        }
        if all.is_empty() {
            ui.weak("No characters yet.");
        }
    });
    out
}

/// Read-only item card with roll buttons.
pub fn item_card(ui: &mut egui::Ui, it: &Item, sh: &mut Shared, roller: &str) {
    ui.label(RichText::new(&it.name).heading().color(rarity_color(ui, it.rarity)));
    ui.label(RichText::new(it.subtitle()).italics());
    ui.horizontal_wrapped(|ui| {
        let v = it.value_gp();
        if v > 0.0 {
            let tip = if it.cost_gp.is_none() { " (typical for its rarity)" } else { "" };
            ui.label(RichText::new(format!("Value {}", format_gp(v))).strong())
                .on_hover_text(format!("Price in gp{tip}"));
        }
        if let Some(w) = it.weight_lb {
            ui.weak(format!("· {w} lb"));
        }
    });
    if !it.damage.is_empty() {
        ui.label(format!("Damage: {}", it.damage));
    }
    if !it.armor_class.is_empty() {
        ui.label(format!("Armor class: {}", it.armor_class));
    }
    if !it.properties.is_empty() {
        ui.label(format!("Properties: {}", it.properties));
    }
    if !it.modifiers.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Bonuses").strong());
            for m in &it.modifiers {
                ui.label(RichText::new(m.text()).monospace());
            }
        });
    }
    if !it.rolls.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Rolls").strong());
            for r in &it.rolls {
                if ui.small_button(format!("🎲 {}  {}", r.label, r.notation)).clicked() {
                    sh.actions.push(Action::Roll {
                        label: r.label.clone(),
                        expr: r.notation.clone(),
                        roller: roller.to_string(),
                    });
                }
            }
        });
    }
    if !it.description.trim().is_empty() {
        ui.add_space(4.0);
        ui_util::render_markdown(ui, &mut sh.md, &it.description);
    }
    let src = [it.source.trim(), it.collection.trim()]
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ");
    if !src.is_empty() {
        ui.weak(format!("Source: {src}"));
    }
}

// ---------------------------------------------------------------------------
// Handing out treasure
// ---------------------------------------------------------------------------

/// What to hand out, and to whom.
#[derive(Default)]
pub struct Handout {
    pub open: bool,
    pub title: String,
    pub coins: Coins,
    pub items: Vec<(Item, u32)>,
    /// Who shares the coins.
    pub recipients: Vec<Link>,
    /// For each item: who gets it.
    pub item_to: Vec<Option<Link>>,
    pub with_rolls: bool,
    /// Set when done (the caller marks a quest rewarded, etc.).
    pub done: bool,
}

impl Handout {
    pub fn start(&mut self, doc: &Doc, title: &str, coins: Coins, items: Vec<(Item, u32)>) {
        let party: Vec<Link> = doc.c.players.iter().filter(|p| p.active).map(|p| Link::Player(p.id)).collect();
        self.open = true;
        self.done = false;
        self.title = title.to_string();
        self.coins = coins;
        self.item_to = vec![party.first().cloned(); items.len()];
        self.items = items;
        self.recipients = party;
        self.with_rolls = true;
    }
}

/// The hand-out window: split coins between chosen characters and give each item to one of
/// them. Returns true on the frame it was applied.
pub fn handout_window(ctx: &egui::Context, doc: &mut Doc, sh: &mut Shared, h: &mut Handout) -> bool {
    if !h.open {
        return false;
    }
    let chars = characters(doc);
    let name_of = |l: &Link| chars.iter().find(|(x, _)| x == l).map(|(_, n)| n.clone()).unwrap_or("?".into());
    let mut open = true;
    let mut apply = false;
    egui::Window::new(format!("Hand out: {}", h.title))
        .id(egui::Id::new("handout_window"))
        .open(&mut open)
        .collapsible(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            if !h.coins.is_zero() {
                ui.label(RichText::new(format!("Coins: {}", h.coins.label())).strong());
                ui.horizontal_wrapped(|ui| {
                    ui.weak("Split between");
                    for (l, n) in chars.iter().filter(|(l, _)| matches!(l, Link::Player(_))) {
                        let mut on = h.recipients.contains(l);
                        if ui.checkbox(&mut on, n).changed() {
                            if on {
                                h.recipients.push(l.clone());
                            } else {
                                h.recipients.retain(|x| x != l);
                            }
                        }
                    }
                });
                if !h.recipients.is_empty() {
                    let share = h.coins.split(h.recipients.len());
                    ui.weak(format!("Each gets about {}", share[0].label()));
                }
                ui.separator();
            }
            if !h.items.is_empty() {
                ui.label(RichText::new("Items").strong());
                egui::Grid::new("handout_items").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    for (i, (it, qty)) in h.items.iter().enumerate() {
                        let label = if *qty > 1 { format!("{qty} × {}", it.name) } else { it.name.clone() };
                        ui.label(RichText::new(label).color(rarity_color(ui, it.rarity)));
                        egui::ComboBox::from_id_salt(("handout_to", i))
                            .selected_text(h.item_to[i].as_ref().map(|l| name_of(l)).unwrap_or("— keep in the pile —".into()))
                            .width(180.0)
                            .truncate()
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut h.item_to[i], None, "— keep in the pile —");
                                for (l, n) in &chars {
                                    ui.selectable_value(&mut h.item_to[i], Some(l.clone()), n);
                                }
                            });
                        ui.end_row();
                    }
                });
                ui.checkbox(&mut h.with_rolls, "Also add the items' rolls to their custom rolls");
                ui.separator();
            }
            if chars.is_empty() {
                ui.weak("Add player characters first.");
            }
            ui.horizontal(|ui| {
                let can = (!h.coins.is_zero() && !h.recipients.is_empty()) || h.item_to.iter().any(|t| t.is_some());
                if ui.add_enabled(can, egui::Button::new(RichText::new("Hand out").strong())).clicked() {
                    apply = true;
                }
                if ui.button("Cancel").clicked() {
                    h.open = false;
                }
            });
        });
    if !open {
        h.open = false;
    }
    if apply {
        let mut msg = Vec::new();
        if !h.coins.is_zero() && !h.recipients.is_empty() {
            for (l, share) in h.recipients.iter().zip(h.coins.split(h.recipients.len())) {
                doc.c.give_coins(l, &share);
            }
            msg.push(format!("{} split {} ways", h.coins.label(), h.recipients.len()));
        }
        let mut given = 0;
        for ((it, qty), to) in h.items.iter().zip(&h.item_to) {
            if let Some(l) = to {
                if doc.c.give_item(l, it, *qty, h.with_rolls) {
                    given += 1;
                }
            }
        }
        if given > 0 {
            msg.push(ui_util::plural(given, "item") + " given");
        }
        sh.toast(msg.join("; "));
        h.open = false;
        h.done = true;
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Inventory editor (PCs and NPCs)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct InventoryUi {
    filter: String,
    custom_name: String,
    expanded: Option<Id>,
}

/// Coins, carried items (qty, equipped, attuned), bonuses from equipped items, and buttons to
/// add an item's rolls to this character.
pub fn inventory_editor(ui: &mut egui::Ui, doc: &mut Doc, owner: &Link, sh: &mut Shared, st: &mut InventoryUi) {
    let roller = doc.c.link_name(owner).unwrap_or_default();
    let mut add: Option<Item> = None;
    let mut add_rolls: Option<Item> = None;
    {
        let Some(core) = doc.c.character_core_mut(owner) else { return };
        // coins
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Coins").strong());
            for (label, v) in [
                ("pp", &mut core.coins.pp),
                ("gp", &mut core.coins.gp),
                ("ep", &mut core.coins.ep),
                ("sp", &mut core.coins.sp),
                ("cp", &mut core.coins.cp),
            ] {
                ui.add(egui::DragValue::new(v).range(0..=10_000_000).suffix(format!(" {label}")));
            }
            ui.weak(format!("= {}", format_gp(core.coins.total_gp())));
        });
        let attuned = core.inventory.iter().filter(|e| e.attuned).count();
        let mut remove = None;
        if core.inventory.is_empty() {
            ui.weak("Nothing carried yet.");
        } else {
            egui::Grid::new(("inv", owner.target_id()))
                .num_columns(6)
                .striped(true)
                .spacing([8.0, 3.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Item").small().strong());
                    ui.label(RichText::new("Qty").small().strong());
                    ui.label(RichText::new("Equipped").small().strong());
                    ui.label(RichText::new(format!("Attuned {attuned}/3")).small().strong());
                    ui.label(RichText::new("Value").small().strong());
                    ui.label("");
                    ui.end_row();
                    for (i, e) in core.inventory.iter_mut().enumerate() {
                        let open = st.expanded == Some(e.id);
                        let name = RichText::new(format!("{} {}", if open { "⏷" } else { "⏵" }, e.item.name)).color(rarity_color(ui, e.item.rarity));
                        if ui.add(egui::Button::new(name).frame(false)).on_hover_text(e.item.subtitle()).clicked() {
                            st.expanded = if open { None } else { Some(e.id) };
                        }
                        ui.add(egui::DragValue::new(&mut e.qty).range(0..=9999));
                        ui.checkbox(&mut e.equipped, "");
                        if e.item.attunement.trim().is_empty() {
                            ui.label("");
                        } else {
                            ui.checkbox(&mut e.attuned, "");
                        }
                        ui.weak(format_gp(e.item.value_gp() * e.qty as f64));
                        ui.horizontal(|ui| {
                            if !e.item.rolls.is_empty()
                                && ui
                                    .small_button("🎲+")
                                    .on_hover_text("Add this item's rolls to the character's custom rolls")
                                    .clicked()
                            {
                                add_rolls = Some(e.item.clone());
                            }
                            if ui.small_button("✖").on_hover_text("Remove").clicked() {
                                remove = Some(i);
                            }
                        });
                        ui.end_row();
                    }
                });
        }
        if let Some(i) = remove {
            core.inventory.remove(i);
        }
        if let Some(id) = st.expanded {
            if let Some(e) = core.inventory.iter_mut().find(|e| e.id == id) {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let it = e.item.clone();
                    item_card(ui, &it, sh, &roller);
                    ui.add(
                        egui::TextEdit::singleline(&mut e.notes)
                            .hint_text("Notes (charges left, who it belonged to…)")
                            .desired_width(f32::INFINITY),
                    );
                });
            }
        }
        // bonuses from what's worn / attuned
        let bonuses: Vec<String> = core
            .inventory
            .iter()
            .filter(|e| e.equipped || e.attuned)
            .flat_map(|e| e.item.modifiers.iter().map(move |m| format!("{} ({})", m.text(), e.item.name)))
            .collect();
        if !bonuses.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Bonuses from equipped items:").strong());
                ui.label(bonuses.join(" · "));
            });
        }
        let total: f64 = core.inventory.iter().map(|e| e.item.value_gp() * e.qty as f64).sum();
        if total > 0.0 {
            ui.weak(format!("Items worth about {}", format_gp(total)));
        }
    }
    ui.horizontal_wrapped(|ui| {
        ui_util::form_menu(ui, "➕ Add item…", |ui| {
            if let Some(it) = item_picker(ui, doc, &mut st.filter) {
                add = Some(it);
                ui.close();
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut st.custom_name)
                .hint_text("or type a custom item")
                .desired_width(160.0),
        );
        if ui.add_enabled(!st.custom_name.trim().is_empty(), egui::Button::new("Add")).clicked() {
            add = Some(Item::named(st.custom_name.trim(), cm_core::items::ItemKind::Gear, Rarity::Mundane, None));
            st.custom_name.clear();
        }
    });
    if let Some(it) = add {
        let has_rolls = !it.rolls.is_empty();
        doc.c.give_item(owner, &it, 1, false);
        if has_rolls {
            sh.toast(format!("Added {} — use 🎲+ to add its rolls", it.name));
        }
    }
    if let Some(it) = add_rolls {
        if let Some(core) = doc.c.character_core_mut(owner) {
            let n = cm_core::model::add_item_rolls(core, &it);
            sh.toast(format!("Added {} from {}", ui_util::plural(n, "roll"), it.name));
        }
    }
}

/// Inventory lines for display elsewhere.
pub fn inventory_summary(inv: &[InventoryEntry]) -> String {
    inv.iter()
        .map(|e| {
            if e.qty > 1 {
                format!("{} × {}", e.qty, e.item.name)
            } else {
                e.item.name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}
