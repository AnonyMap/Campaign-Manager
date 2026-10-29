//! Item library: your items, imported books and the bundled SRD 5.2 list. Give items to
//! characters (optionally with their rolls), or just add an item's rolls to someone.

use cm_core::items::{self, ImportReport, Item, ItemKind, ItemModifier, Rarity, format_gp};
use cm_core::{CustomRoll, Id, Link};
use egui::RichText;

use crate::app::{Doc, Shared};
use crate::ui_util::{self, section};
use crate::views::treasure::{self, rarity_color, srd_items};

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum Tab {
    #[default]
    Campaign,
    Srd,
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum Sort {
    #[default]
    Name,
    Value,
    Rarity,
}

#[derive(Default)]
pub struct ImportUi {
    text: String,
    report: Option<ImportReport>,
    include: Vec<bool>,
    error: Option<String>,
    collection: String,
    file_name: String,
    replace: bool,
}

#[derive(Default)]
pub struct State {
    pub tab: Tab,
    pub selected: Option<Id>,
    srd_selected: Option<usize>,
    filter: String,
    kind: Option<ItemKind>,
    rarity: Option<Rarity>,
    magic_only: bool,
    sort: Sort,
    collection: Option<String>,
    pub editing: bool,
    import: Option<ImportUi>,
    select_mode: bool,
    checked: std::collections::HashSet<Id>,
    confirm_delete: bool,
    new_collection: String,
    char_filter: String,
    /// Giving an item leaves its rolls off the character's custom rolls.
    give_without_rolls: bool,
    new_roll_label: String,
    new_roll_expr: String,
}

impl State {
    /// Select one of the campaign's items (clearing filters that could hide it).
    pub fn show_campaign_item(&mut self, id: Id) {
        self.tab = Tab::Campaign;
        self.selected = Some(id);
        self.editing = false;
        self.collection = None;
    }
}

fn matches(i: &Item, f: &str, st: &State) -> bool {
    if st.kind.map(|k| i.kind != k).unwrap_or(false) || st.rarity.map(|r| i.rarity != r).unwrap_or(false) {
        return false;
    }
    if st.magic_only && !i.is_magic() {
        return false;
    }
    f.is_empty() || i.name.to_lowercase().contains(f) || i.description.to_lowercase().contains(f) || i.tags.iter().any(|t| t.to_lowercase().contains(f))
}

fn sort(v: &mut [&Item], s: Sort) {
    match s {
        Sort::Name => v.sort_by_key(|i| i.name.to_lowercase()),
        Sort::Value => v.sort_by(|a, b| a.value_gp().partial_cmp(&b.value_gp()).unwrap_or(std::cmp::Ordering::Equal)),
        Sort::Rarity => v.sort_by(|a, b| a.rarity.cmp(&b.rarity).then(a.name.cmp(&b.name))),
    }
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    list_panel(ui, doc, sh, st);
    import_window(ui.ctx(), doc, sh, st);
    egui::CentralPanel::default().frame(egui::Frame::new()).show_inside(ui, |ui| match st.tab {
        Tab::Srd => srd_detail(ui, doc, sh, st),
        Tab::Campaign => detail(ui, doc, sh, st),
    });
}

fn list_panel(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    egui::SidePanel::left("items_list").resizable(true).default_width(280.0).show_inside(ui, |ui| {
        ui.heading("Items");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut st.tab, Tab::Campaign, format!("This campaign ({})", doc.c.items.len()));
            ui.selectable_value(&mut st.tab, Tab::Srd, format!("SRD 5.2 ({})", srd_items().len()));
        });
        if st.tab == Tab::Campaign {
            let cols = doc.c.item_collections();
            if st.collection.as_ref().map(|c| !cols.iter().any(|(n, _)| n == c)).unwrap_or(false) {
                st.collection = None;
            }
            if cols.len() > 1 {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.weak("Show");
                    if ui.selectable_label(st.collection.is_none(), format!("All ({})", doc.c.items.len())).clicked() {
                        st.collection = None;
                    }
                    for (name, n) in &cols {
                        if ui.selectable_label(st.collection.as_deref() == Some(name.as_str()), format!("{name} ({n})")).clicked() {
                            st.collection = Some(name.clone());
                            st.checked.clear();
                        }
                    }
                });
            }
        }
        ui.horizontal_wrapped(|ui| {
            if st.tab == Tab::Campaign && ui.button("➕ New").clicked() {
                let mut it = Item { name: "New item".into(), ..Default::default() };
                if let Some(c) = st.collection.clone().filter(|c| c != items::MY_COLLECTION) {
                    it.collection = c;
                }
                st.selected = Some(it.id);
                st.editing = true;
                doc.c.items.push(it);
            }
            if ui.button("📥 Import…").on_hover_text("Import items from a file or pasted AI output").clicked() {
                st.import = Some(ImportUi::default());
            }
            if st.tab == Tab::Campaign && !doc.c.items.is_empty() {
                if ui.button("📤 Export…").clicked() {
                    let shown: Vec<Item> = doc.c.items.iter().filter(|i| st.collection.as_ref().map(|c| i.collection_label() == *c).unwrap_or(true)).cloned().collect();
                    let what = st.collection.clone().unwrap_or_else(|| doc.c.name.clone());
                    let name = format!("{} items.json", cm_core::storage::sanitize_file_name(&what));
                    if let Some(p) = ui_util::save_file_dialog("Export items", &name, ("JSON", &["json"])) {
                        match std::fs::write(&p, items::export_json(&shown, &what)) {
                            Ok(()) => sh.toast(format!("Exported {}", ui_util::plural(shown.len(), "item"))),
                            Err(e) => sh.toast(format!("Export failed: {e}")),
                        }
                    }
                }
                let label = if st.select_mode { "✔ Done selecting" } else { "☑ Select" };
                if ui.add(egui::Button::new(label).selected(st.select_mode)).clicked() {
                    st.select_mode = !st.select_mode;
                    st.checked.clear();
                    st.confirm_delete = false;
                }
            }
        });
        ui_util::filter_box(ui, &mut st.filter, "Filter: name, text, tag…");
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("item_kind_f").width(110.0).truncate().selected_text(st.kind.map(|k| k.label()).unwrap_or("Any type")).show_ui(ui, |ui| {
                ui.selectable_value(&mut st.kind, None, "Any type");
                for k in ItemKind::ALL {
                    ui.selectable_value(&mut st.kind, Some(k), k.label());
                }
            });
            egui::ComboBox::from_id_salt("item_rarity_f").width(100.0).truncate().selected_text(st.rarity.map(|k| k.label()).unwrap_or("Any rarity")).show_ui(ui, |ui| {
                ui.selectable_value(&mut st.rarity, None, "Any rarity");
                for r in Rarity::ALL {
                    ui.selectable_value(&mut st.rarity, Some(r), r.label());
                }
            });
            ui.checkbox(&mut st.magic_only, "Magic");
        });
        ui.horizontal(|ui| {
            ui.weak("Sort");
            ui.selectable_value(&mut st.sort, Sort::Name, "Name");
            ui.selectable_value(&mut st.sort, Sort::Value, "Value");
            ui.selectable_value(&mut st.sort, Sort::Rarity, "Rarity");
        });
        let f = st.filter.trim().to_lowercase();
        if st.tab == Tab::Campaign && st.select_mode {
            let shown: Vec<Id> = doc.c.items.iter().filter(|i| st.collection.as_ref().map(|c| i.collection_label() == *c).unwrap_or(true) && matches(i, &f, st)).map(|i| i.id).collect();
            bulk_bar(ui, doc, sh, st, &shown);
        }
        ui.separator();
        match st.tab {
            Tab::Campaign => {
                let mut list: Vec<&Item> = doc.c.items.iter().filter(|i| st.collection.as_ref().map(|c| i.collection_label() == *c).unwrap_or(true) && matches(i, &f, st)).collect();
                sort(&mut list, st.sort);
                if doc.c.items.is_empty() {
                    ui.weak("No items yet. Make one, import a book's items, or copy some from the SRD 5.2 tab. The loot generator and shops use the SRD list as well as yours.");
                }
                let row_h = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
                egui::ScrollArea::vertical().id_salt("items_rows").show_rows(ui, row_h, list.len(), |ui, range| {
                    for it in &list[range] {
                        let text = RichText::new(format!("{}  ·  {}", it.name, format_gp(it.value_gp()))).color(rarity_color(ui, it.rarity));
                        if st.select_mode {
                            let mut on = st.checked.contains(&it.id);
                            if ui.checkbox(&mut on, text).changed() {
                                if on {
                                    st.checked.insert(it.id);
                                } else {
                                    st.checked.remove(&it.id);
                                }
                            }
                        } else if ui_util::list_row(ui, st.selected == Some(it.id), text).on_hover_text(it.subtitle()).clicked() {
                            st.selected = Some(it.id);
                            st.editing = false;
                        }
                    }
                });
            }
            Tab::Srd => {
                let all = srd_items();
                let mut list: Vec<(usize, &Item)> = all.iter().enumerate().filter(|(_, i)| matches(i, &f, st)).collect();
                match st.sort {
                    Sort::Name => list.sort_by_key(|(_, i)| i.name.to_lowercase()),
                    Sort::Value => list.sort_by(|a, b| a.1.value_gp().partial_cmp(&b.1.value_gp()).unwrap_or(std::cmp::Ordering::Equal)),
                    Sort::Rarity => list.sort_by(|a, b| a.1.rarity.cmp(&b.1.rarity).then(a.1.name.cmp(&b.1.name))),
                }
                ui.weak(format!("{} shown", list.len()));
                let row_h = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
                egui::ScrollArea::vertical().id_salt("srd_items_rows").show_rows(ui, row_h, list.len(), |ui, range| {
                    for (idx, it) in &list[range] {
                        let text = RichText::new(format!("{}  ·  {}", it.name, format_gp(it.value_gp()))).color(rarity_color(ui, it.rarity));
                        if ui_util::list_row(ui, st.srd_selected == Some(*idx), text).on_hover_text(it.subtitle()).clicked() {
                            st.srd_selected = Some(*idx);
                        }
                    }
                });
            }
        }
    });
}

fn bulk_bar(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, shown: &[Id]) {
    st.checked.retain(|id| doc.c.item(*id).is_some());
    let n = st.checked.len();
    egui::Frame::group(ui.style())
        .fill(ui.visuals().selection.bg_fill.gamma_multiply(0.15))
        .inner_margin(6)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("{n} selected")).strong());
                if ui.small_button("All shown").clicked() {
                    st.checked.extend(shown.iter().copied());
                }
                if ui.small_button("None").clicked() {
                    st.checked.clear();
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled_ui(n > 0, |ui| {
                    ui_util::form_menu(ui, "Move to…", |ui| {
                        let mut target: Option<String> = None;
                        for (name, _) in doc.c.item_collections() {
                            if ui.button(&name).clicked() {
                                target = Some(name);
                            }
                        }
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut st.new_collection)
                                    .hint_text("New collection")
                                    .desired_width(130.0),
                            );
                            if ui.add_enabled(!st.new_collection.trim().is_empty(), egui::Button::new("Move")).clicked() {
                                target = Some(st.new_collection.trim().to_string());
                            }
                        });
                        if let Some(t) = target {
                            for i in doc.c.items.iter_mut().filter(|i| st.checked.contains(&i.id)) {
                                i.collection = if t == items::MY_COLLECTION { String::new() } else { t.clone() };
                            }
                            sh.toast(format!("Moved {} to “{t}”", ui_util::plural(st.checked.len(), "item")));
                            st.new_collection.clear();
                            ui.close();
                        }
                    });
                    if ui.button(RichText::new(format!("🗑 Delete {n}…")).color(ui.visuals().error_fg_color)).clicked() {
                        st.confirm_delete = true;
                    }
                });
            });
            if st.confirm_delete && n > 0 {
                ui.label(
                    RichText::new(format!(
                        "Delete {}? Items characters already carry stay with them. Edit » Undo brings them back.",
                        ui_util::plural(n, "item")
                    ))
                    .strong(),
                );
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Delete").color(ui.visuals().error_fg_color)).clicked() {
                        let ids: Vec<Id> = st.checked.iter().copied().collect();
                        let k = doc.c.delete_items(&ids);
                        sh.toast(format!("Deleted {}", ui_util::plural(k, "item")));
                        st.checked.clear();
                        st.confirm_delete = false;
                        st.select_mode = false;
                    }
                    if ui.button("Cancel").clicked() {
                        st.confirm_delete = false;
                    }
                });
            }
        });
}

/// Give to… / Add rolls to… buttons for an item.
fn give_buttons(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, it: &Item) {
    let mut give: Option<Link> = None;
    let mut rolls_to: Option<Link> = None;
    ui_util::form_menu(ui, "🎁 Give to…", |ui| {
        let mut with = !st.give_without_rolls;
        if ui.checkbox(&mut with, "and add its rolls to their custom rolls").changed() {
            st.give_without_rolls = !with;
        }
        if let Some(l) = treasure::character_menu(ui, doc, &mut st.char_filter) {
            give = Some(l);
            ui.close();
        }
    })
    .on_hover_text("Put it in a character's inventory");
    if !it.rolls.is_empty() {
        ui_util::form_menu(ui, "🎲 Add rolls to…", |ui| {
            ui.weak("Adds the rolls only — the item isn't put in their inventory.");
            if let Some(l) = treasure::character_menu(ui, doc, &mut st.char_filter) {
                rolls_to = Some(l);
                ui.close();
            }
        });
    }
    if let Some(l) = give {
        if doc.c.give_item(&l, it, 1, !st.give_without_rolls) {
            sh.toast(format!("Gave {} to {}", it.name, doc.c.link_name(&l).unwrap_or_default()));
        }
    }
    if let Some(l) = rolls_to {
        if let Some(core) = doc.c.character_core_mut(&l) {
            let n = cm_core::add_item_rolls(core, it);
            let who = core.display_name();
            sh.toast(format!("Added {} to {who}", ui_util::plural(n, "roll")));
        }
    }
}

fn srd_detail(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    let Some(it) = st.srd_selected.and_then(|i| srd_items().get(i)).cloned() else {
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);
            ui.weak(format!(
                "{} items from the System Reference Document 5.2: equipment, weapons, armor and magic items.",
                srd_items().len()
            ));
            ui.weak("The loot generator and shops draw from this list and your own items.");
            ui.add_space(12.0);
            ui.label(RichText::new(cm_core::bestiary::SRD_ATTRIBUTION).small().weak());
        });
        return;
    };
    ui.horizontal_wrapped(|ui| {
        let existing = doc.c.items.iter().find(|i| i.name == it.name && i.collection == it.collection).map(|i| i.id);
        match existing {
            Some(id) => {
                if ui.button("Open campaign copy").clicked() {
                    st.tab = Tab::Campaign;
                    st.selected = Some(id);
                }
            }
            None => {
                if ui
                    .button(RichText::new("➕ Add to campaign").strong())
                    .on_hover_text("Copy into your item library to edit it")
                    .clicked()
                {
                    let mut copy = it.clone();
                    copy.id = cm_core::new_id();
                    st.selected = Some(copy.id);
                    doc.c.items.push(copy);
                    sh.toast(format!("Added {} to your items", it.name));
                }
            }
        }
        give_buttons(ui, doc, sh, st, &it);
    });
    ui.separator();
    ui_util::pinned_scroll(ui, "srd_item_detail", |ui| {
        treasure::item_card(ui, &it, sh, "");
    });
}

fn detail(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    let Some(id) = st.selected.filter(|id| doc.c.item(*id).is_some()) else {
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);
            ui.label(RichText::new("Your item library").strong());
            ui.weak("Homebrew and imported items. Give them to characters, use them as quest rewards, and the loot generator and shops will pick from them.");
            ui.add_space(10.0);
            if ui.button("📥 Import a book's items with AI…").clicked() {
                st.import = Some(ImportUi::default());
            }
            if ui.button("Browse the SRD 5.2 items").clicked() {
                st.tab = Tab::Srd;
            }
        });
        return;
    };
    let it = doc.c.item(id).unwrap().clone();
    ui.horizontal_wrapped(|ui| {
        let label = if st.editing { "✔ Done editing" } else { "✏ Edit" };
        if ui.add(egui::Button::new(label).selected(st.editing)).clicked() {
            st.editing = !st.editing;
        }
        give_buttons(ui, doc, sh, st, &it);
        if ui.button("Duplicate").clicked() {
            let mut copy = it.clone();
            copy.id = cm_core::new_id();
            copy.name = format!("{} (copy)", copy.name);
            st.selected = Some(copy.id);
            st.editing = true;
            doc.c.items.push(copy);
        }
        ui_util::confirm_delete_button(ui, Link::Item(id), sh);
    });
    ui.weak(format!("Collection: {}", it.collection_label()));
    ui.separator();
    ui_util::pinned_scroll(ui, ("item_detail", st.editing, st.selected), |ui| {
        if st.editing {
            let i = doc.c.item_mut(id).unwrap();
            editor(ui, i, st, sh);
        } else {
            treasure::item_card(ui, &it, sh, "");
        }
        ui_util::backlinks(ui, &doc.c, &Link::Item(id), sh);
    });
}

fn editor(ui: &mut egui::Ui, it: &mut Item, st: &mut State, sh: &mut Shared) {
    let h = ui.spacing().interact_size.y;
    let id = it.id;
    egui::Grid::new(("item_edit", id)).num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
        ui.label("Name");
        ui.add_sized([320.0, h], egui::TextEdit::singleline(&mut it.name).id_salt(("item_name", id)));
        ui.end_row();
        ui.label("Type");
        egui::ComboBox::from_id_salt(("item_kind", id))
            .selected_text(it.kind.label())
            .show_ui(ui, |ui| {
                for k in ItemKind::ALL {
                    ui.selectable_value(&mut it.kind, k, k.label());
                }
            });
        ui.end_row();
        ui.label("Rarity");
        ui.horizontal_wrapped(|ui| {
            for r in Rarity::ALL {
                ui.selectable_value(&mut it.rarity, r, RichText::new(r.label()).color(rarity_color(ui, r)));
            }
        });
        ui.end_row();
        ui.label("Price (gp)");
        ui.horizontal(|ui| {
            let mut has = it.cost_gp.is_some();
            if ui.checkbox(&mut has, "").changed() {
                it.cost_gp = if has { Some(it.value_gp().max(1.0)) } else { None };
            }
            if let Some(c) = &mut it.cost_gp {
                ui.add(egui::DragValue::new(c).range(0.0..=10_000_000.0).speed(1.0).max_decimals(2));
            } else if let Some(v) = it.rarity.typical_value() {
                ui.weak(format!(
                    "typical for {}: {}",
                    it.rarity.label().to_lowercase(),
                    format_gp(if it.kind.consumable() { v / 2.0 } else { v })
                ));
            }
        });
        ui.end_row();
        ui.label("Weight (lb)");
        ui.horizontal(|ui| {
            let mut has = it.weight_lb.is_some();
            if ui.checkbox(&mut has, "").changed() {
                it.weight_lb = if has { Some(1.0) } else { None };
            }
            if let Some(w) = &mut it.weight_lb {
                ui.add(egui::DragValue::new(w).range(0.0..=100000.0).speed(0.1));
            }
        });
        ui.end_row();
        ui.label("Attunement");
        ui.add_sized(
            [320.0, h],
            egui::TextEdit::singleline(&mut it.attunement).hint_text("blank = none, or “requires attunement by a cleric”"),
        );
        ui.end_row();
        ui.label("Damage");
        ui.add_sized([320.0, h], egui::TextEdit::singleline(&mut it.damage).hint_text("1d8 slashing"));
        ui.end_row();
        ui.label("Armor class");
        ui.add_sized(
            [320.0, h],
            egui::TextEdit::singleline(&mut it.armor_class).hint_text("14 + Dex modifier (max 2)"),
        );
        ui.end_row();
        ui.label("Properties");
        ui.add_sized([320.0, h], egui::TextEdit::singleline(&mut it.properties).hint_text("Finesse, Light"));
        ui.end_row();
        ui.label("Source");
        ui.add_sized([320.0, h], egui::TextEdit::singleline(&mut it.source));
        ui.end_row();
        ui.label("Collection");
        ui.add_sized([320.0, h], egui::TextEdit::singleline(&mut it.collection).hint_text("blank = My items"));
        ui.end_row();
    });
    section(ui, "Description");
    ui_util::markdown_editor(ui, ("item_desc", id), &mut it.description, &mut sh.md, 6);
    section(ui, "Rolls");
    ui.weak("Dice the item lets its user roll. They can be added to a character's custom rolls.");
    let mut remove = None;
    for (i, r) in it.rolls.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add_sized([200.0, h], egui::TextEdit::singleline(&mut r.label).id_salt(("irl", r.id)));
            let ok = cm_core::dice::DiceExpr::parse(&r.notation).is_ok();
            let mut te = egui::TextEdit::singleline(&mut r.notation)
                .id_salt(("irn", r.id))
                .font(egui::TextStyle::Monospace);
            if !ok {
                te = te.text_color(ui.visuals().error_fg_color);
            }
            ui.add_sized([110.0, h], te);
            if ui.small_button("✖").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        it.rolls.remove(i);
    }
    ui.horizontal(|ui| {
        ui.add_sized([200.0, h], egui::TextEdit::singleline(&mut st.new_roll_label).hint_text("Label"));
        ui.add_sized(
            [110.0, h],
            egui::TextEdit::singleline(&mut st.new_roll_expr)
                .hint_text("2d6")
                .font(egui::TextStyle::Monospace),
        );
        let ok = cm_core::dice::DiceExpr::parse(&st.new_roll_expr).is_ok();
        if ui.add_enabled(ok, egui::Button::new("➕ Add roll")).clicked() {
            let label = if st.new_roll_label.trim().is_empty() {
                it.name.clone()
            } else {
                st.new_roll_label.trim().to_string()
            };
            it.rolls.push(CustomRoll {
                label,
                notation: st.new_roll_expr.trim().to_string(),
                ..Default::default()
            });
            st.new_roll_label.clear();
            st.new_roll_expr.clear();
        }
        if ui
            .button("Find rolls in the text")
            .on_hover_text("Damage, healing and “+N” bonuses mentioned in the description")
            .clicked()
        {
            let before = (it.rolls.len(), it.modifiers.len());
            for r in items::suggest_rolls(it) {
                if !it.rolls.iter().any(|x| x.notation == r.notation) {
                    it.rolls.push(r);
                }
            }
            for m in items::suggest_modifiers(it) {
                if !it.modifiers.iter().any(|x| x.label == m.label) {
                    it.modifiers.push(m);
                }
            }
            sh.toast(format!(
                "Found {} and {}",
                ui_util::plural(it.rolls.len() - before.0, "roll"),
                ui_util::plural(it.modifiers.len() - before.1, "bonus")
            ));
        }
    });
    section(ui, "Bonuses");
    ui.weak("Flat bonuses it grants (shown on characters who equip or attune it).");
    let mut remove = None;
    for (i, m) in it.modifiers.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut m.value).range(-10..=10).prefix("+"));
            ui.add_sized(
                [200.0, h],
                egui::TextEdit::singleline(&mut m.label)
                    .id_salt(("iml", id, i))
                    .hint_text("AC, attack rolls, saving throws…"),
            );
            if ui.small_button("✖").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        it.modifiers.remove(i);
    }
    if ui.button("➕ Add bonus").clicked() {
        it.modifiers.push(ItemModifier { label: "AC".into(), value: 1 });
    }
    ui.horizontal(|ui| {
        ui.label("Tags");
        ui_util::tags_edit(ui, ("itemtags", id), &mut it.tags);
    });
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

fn parse(imp: &mut ImportUi) {
    match items::import_json(&imp.text) {
        Ok(r) => {
            imp.include = vec![true; r.items.len()];
            if imp.collection.trim().is_empty() {
                imp.collection = if !r.collection.trim().is_empty() {
                    r.collection.trim().to_string()
                } else if !imp.file_name.is_empty() {
                    std::path::Path::new(&imp.file_name)
                        .file_stem()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_default()
                } else {
                    format!("Imported {}", chrono::Local::now().format("%b %-d"))
                };
            }
            imp.report = Some(r);
            imp.error = None;
        }
        Err(e) => {
            imp.error = Some(format!(
                "Couldn't read that: {e}. Make sure you copied the whole reply, including the closing brackets."
            ))
        }
    }
}

fn import_window(ctx: &egui::Context, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    let Some(imp) = &mut st.import else { return };
    let mut open = true;
    let mut done: Option<(usize, String)> = None;
    let mut back = false;
    egui::Window::new("Import items")
        .open(&mut open)
        .default_width(560.0)
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            if imp.report.is_none() {
                ui.label(RichText::new("1. Get the items as JSON").strong());
                ui.label("Give an AI your PDF or screenshots together with the instructions below. It replies with a file this app can read.");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("📋 Copy AI instructions").clicked() {
                        ui.ctx().copy_text(items::AI_SPEC.to_string());
                        sh.toast("AI instructions copied — paste them into the chat with your PDF");
                    }
                    if ui.button("💾 Save instructions as file…").clicked() {
                        if let Some(p) = ui_util::save_file_dialog("Save AI instructions", "item-import-spec.md", ("Markdown", &["md"])) {
                            if let Err(e) = std::fs::write(&p, items::AI_SPEC) {
                                sh.toast(format!("Couldn't save: {e}"));
                            }
                        }
                    }
                });
                ui.add_space(6.0);
                ui.label(RichText::new("2. Paste the AI's reply (or choose a file)").strong());
                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut imp.text)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(10)
                            .desired_width(f32::INFINITY)
                            .hint_text("Paste here — extra chat text and ``` fences are fine"),
                    );
                });
                ui.horizontal(|ui| {
                    if ui.button("📂 Choose file…").clicked() {
                        if let Some(p) = ui_util::pick_json_file("Choose an item file") {
                            match std::fs::read_to_string(&p) {
                                Ok(t) => {
                                    imp.text = t;
                                    imp.file_name = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                                    parse(imp);
                                }
                                Err(e) => imp.error = Some(format!("Couldn't read the file: {e}")),
                            }
                        }
                    }
                    if ui
                        .add_enabled(!imp.text.trim().is_empty(), egui::Button::new(RichText::new("Read ▶").strong()))
                        .clicked()
                    {
                        parse(imp);
                    }
                });
                if let Some(e) = &imp.error {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
            } else {
                let report = imp.report.take().unwrap();
                let n = imp.include.iter().filter(|b| **b).count();
                ui.label(RichText::new(format!("Found {}", ui_util::plural(report.items.len(), "item"))).strong());
                ui.horizontal(|ui| {
                    ui.label("Put them in collection");
                    ui.add(egui::TextEdit::singleline(&mut imp.collection).desired_width(200.0));
                });
                let target = if imp.collection.trim().is_empty() {
                    items::MY_COLLECTION.to_string()
                } else {
                    imp.collection.trim().to_string()
                };
                let dups = report
                    .items
                    .iter()
                    .zip(&imp.include)
                    .filter(|(i, inc)| {
                        **inc
                            && doc
                                .c
                                .items
                                .iter()
                                .any(|x| x.collection_label() == target && x.name.eq_ignore_ascii_case(&i.name))
                    })
                    .count();
                if dups > 0 {
                    ui.checkbox(
                        &mut imp.replace,
                        format!("Replace the {dups} already in this collection with the same name (otherwise keep both)"),
                    );
                }
                if !report.warnings.is_empty() {
                    egui::CollapsingHeader::new(
                        RichText::new(format!("⚠ {}", ui_util::plural(report.warnings.len(), "warning"))).color(ui.visuals().warn_fg_color),
                    )
                    .default_open(report.warnings.len() <= 6)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt("item_imp_warn").max_height(120.0).show(ui, |ui| {
                            for w in &report.warnings {
                                ui.label(RichText::new(w).small());
                            }
                        });
                    });
                }
                ui.horizontal(|ui| {
                    if ui.small_button("All").clicked() {
                        imp.include.iter_mut().for_each(|b| *b = true);
                    }
                    if ui.small_button("None").clicked() {
                        imp.include.iter_mut().for_each(|b| *b = false);
                    }
                });
                egui::ScrollArea::vertical().id_salt("item_imp_list").max_height(280.0).show(ui, |ui| {
                    for (i, it) in report.items.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut imp.include[i], RichText::new(&it.name).strong().color(rarity_color(ui, it.rarity)));
                            ui.weak(format!("{} · {}", it.subtitle(), format_gp(it.value_gp())));
                            if !it.rolls.is_empty() || !it.modifiers.is_empty() {
                                ui.weak(format!(
                                    "· {}, {}",
                                    ui_util::plural(it.rolls.len(), "roll"),
                                    ui_util::plural(it.modifiers.len(), "bonus")
                                ));
                            }
                        });
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(n > 0, egui::Button::new(RichText::new(format!("Import {n}")).strong()))
                        .clicked()
                    {
                        let stored = if target == items::MY_COLLECTION { String::new() } else { target.clone() };
                        let mut k = 0;
                        for (it, inc) in report.items.iter().zip(&imp.include) {
                            if !inc {
                                continue;
                            }
                            let mut new = it.clone();
                            new.collection = stored.clone();
                            let existing = doc
                                .c
                                .items
                                .iter()
                                .position(|x| x.collection_label() == target && x.name.eq_ignore_ascii_case(&it.name));
                            match (existing, imp.replace) {
                                (Some(i), true) => {
                                    new.id = doc.c.items[i].id;
                                    doc.c.items[i] = new;
                                }
                                _ => doc.c.items.push(new),
                            }
                            k += 1;
                        }
                        done = Some((k, target.clone()));
                    }
                    if ui.button("◀ Back").clicked() {
                        back = true;
                    }
                });
                if !back {
                    imp.report = Some(report);
                }
            }
        });
    if let Some((k, target)) = done {
        sh.toast(format!("Imported {} into “{target}”", ui_util::plural(k, "item")));
        st.collection = Some(target);
        st.tab = Tab::Campaign;
        st.import = None;
    } else if !open {
        st.import = None;
    }
}
