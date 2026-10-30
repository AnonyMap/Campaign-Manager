//! Bestiary: the campaign's pool of stat blocks (independent of NPCs), the bundled SRD 5.2
//! library, and importing stat blocks converted by an AI or exported from another campaign.

use std::sync::OnceLock;

use cm_core::bestiary::{self, ImportReport, StatBlock};
use cm_core::{CombatantSource, Id, Link, Npc, NpcKind};
use egui::RichText;

use crate::app::{Action, Doc, Shared};
use crate::ui_util::{self, section};
use crate::views::characters;
use crate::views::statblock;

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum Tab {
    #[default]
    Campaign,
    Srd,
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum Sort {
    #[default]
    Name,
    Cr,
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum Duplicates {
    /// Replace the existing stat block with the same name (keeps NPC assignments).
    #[default]
    Replace,
    KeepBoth,
    Skip,
}

#[derive(Default)]
pub struct ImportUi {
    pub text: String,
    pub report: Option<ImportReport>,
    pub include: Vec<bool>,
    pub error: Option<String>,
    pub duplicates: Duplicates,
    pub file_name: String,
    /// Bestiary collection the imported stat blocks go into.
    pub collection: String,
}

#[derive(Default)]
pub struct State {
    pub tab: Tab,
    pub selected: Option<Id>,
    srd_selected: Option<usize>,
    filter: String,
    type_filter: Option<String>,
    sort: Sort,
    pub editing: bool,
    cst: characters::State,
    pub import: Option<ImportUi>,
    /// Show only this collection (None = all).
    collection: Option<String>,
    /// Multi-select mode for bulk actions.
    select_mode: bool,
    checked: std::collections::HashSet<Id>,
    last_clicked: Option<usize>,
    confirm_delete: Option<Vec<Id>>,
    new_collection: String,
    /// Collection rename box (None until the menu is first shown for a collection).
    rename_to: Option<String>,
    /// SRD list for the current filter / type / sort (worked out when those change).
    srd_cache: Option<(String, Option<String>, Sort, Vec<usize>)>,
    /// Filter for the "Give to an NPC" menu.
    npc_filter: String,
}

impl State {
    /// Open the import window pre-filled with a dropped file.
    pub fn import_text(&mut self, text: String, path: &std::path::Path) {
        let mut imp = ImportUi {
            text,
            file_name: path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(),
            ..Default::default()
        };
        parse(&mut imp);
        self.import = Some(imp);
    }
}

/// The bundled SRD list, parsed once.
pub fn srd() -> &'static [StatBlock] {
    static SRD: OnceLock<Vec<StatBlock>> = OnceLock::new();
    SRD.get_or_init(bestiary::srd_statblocks)
}

fn main_type(sb: &StatBlock) -> String {
    let t = sb.core.class_or_type.trim();
    let t = t.split('(').next().unwrap_or("").trim();
    let mut c = t.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

fn matches(sb: &StatBlock, f: &str, ty: &Option<String>) -> bool {
    if let Some(t) = ty {
        if &main_type(sb) != t {
            return false;
        }
    }
    if f.is_empty() {
        return true;
    }
    // "cr 2" / "cr 1/4" filters by challenge rating
    if let Some(cr) = f.strip_prefix("cr ") {
        return sb.core.cr.trim() == cr.trim();
    }
    sb.name().to_lowercase().contains(f)
        || sb.core.class_or_type.to_lowercase().contains(f)
        || sb.source.to_lowercase().contains(f)
        || sb.core.tags.iter().any(|t| t.to_lowercase().contains(f))
}

/// Case-insensitive compare where digit runs compare as numbers ("NPC 2" < "NPC 10").
#[cfg(test)]
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn chunks(s: &str) -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for ch in s.to_lowercase().chars() {
            let d = ch.is_ascii_digit();
            match out.last_mut() {
                Some((was, buf)) if *was == d => buf.push(ch),
                _ => out.push((d, ch.to_string())),
            }
        }
        out
    }
    let (ca, cb) = (chunks(a), chunks(b));
    for (x, y) in ca.iter().zip(cb.iter()) {
        let o = match (x.0, y.0) {
            (true, true) => {
                let (nx, ny) = (x.1.trim_start_matches('0'), y.1.trim_start_matches('0'));
                nx.len().cmp(&ny.len()).then_with(|| nx.cmp(ny))
            }
            _ => x.1.cmp(&y.1),
        };
        if o != std::cmp::Ordering::Equal {
            return o;
        }
    }
    ca.len().cmp(&cb.len())
}

/// Sort key equivalent to `natural_cmp` (digit runs zero-padded), computed once per item.
fn natural_key(s: &str) -> Vec<String> {
    let mut out: Vec<(bool, String)> = Vec::new();
    for ch in s.to_lowercase().chars() {
        let d = ch.is_ascii_digit();
        match out.last_mut() {
            Some((was, buf)) if *was == d => buf.push(ch),
            _ => out.push((d, ch.to_string())),
        }
    }
    out.into_iter()
        .map(|(d, t)| if d { format!("{:0>24}", t.trim_start_matches('0')) } else { t })
        .collect()
}

fn sorted<'a>(list: impl Iterator<Item = (usize, &'a StatBlock)>, sort: Sort) -> Vec<(usize, &'a StatBlock)> {
    let mut v: Vec<_> = list.collect();
    match sort {
        Sort::Name => v.sort_by_cached_key(|a| natural_key(&a.1.name())),
        Sort::Cr => v.sort_by(|a, b| {
            statblock::cr_value(&a.1.core.cr)
                .partial_cmp(&statblock::cr_value(&b.1.core.cr))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.name().cmp(&b.1.name()))
        }),
    }
    v
}

/// Copy an SRD block into the campaign (or find the copy already there).
pub fn adopt_srd(doc: &mut Doc, sb: &StatBlock) -> (Id, bool) {
    if let Some(existing) = doc.c.statblocks.iter().find(|s| s.name() == sb.name() && s.source == sb.source) {
        return (existing.id, false);
    }
    let mut copy = sb.clone();
    copy.id = cm_core::new_id();
    let id = copy.id;
    doc.c.statblocks.push(copy);
    (id, true)
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    list_panel(ui, doc, sh, st);
    import_window(ui.ctx(), doc, sh, st);
    match st.tab {
        Tab::Srd => srd_detail(ui, doc, sh, st),
        Tab::Campaign => campaign_detail(ui, doc, sh, st),
    }
}

fn list_panel(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    egui::SidePanel::left("bestiary_list")
        .resizable(true)
        .default_width(270.0)
        .show_inside(ui, |ui| {
            ui.heading("Bestiary");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut st.tab, Tab::Campaign, format!("This campaign ({})", doc.c.statblocks.len()));
                ui.selectable_value(&mut st.tab, Tab::Srd, format!("SRD 5.2 ({})", srd().len()));
            });
            if st.tab == Tab::Campaign {
                collections_row(ui, doc, sh, st);
            }
            ui.horizontal_wrapped(|ui| {
                if st.tab == Tab::Campaign && ui.button("➕ New").on_hover_text("Blank stat block").clicked() {
                    let mut sb = StatBlock::default();
                    sb.core.name = "New creature".into();
                    if let Some(c) = st.collection.clone().filter(|c| c != bestiary::MY_COLLECTION) {
                        sb.collection = c;
                    }
                    st.selected = Some(sb.id);
                    st.editing = true;
                    doc.c.statblocks.push(sb);
                }
                if ui
                    .button("📥 Import…")
                    .on_hover_text("Import stat blocks from a file or pasted AI output")
                    .clicked()
                {
                    st.import = Some(ImportUi::default());
                }
                if st.tab == Tab::Campaign && !doc.c.statblocks.is_empty() {
                    let what = st.collection.clone().unwrap_or_else(|| doc.c.name.clone());
                    if ui
                        .button("📤 Export…")
                        .on_hover_text(match &st.collection {
                            Some(c) => format!("Save the “{c}” collection as an import file"),
                            None => "Save all of this campaign's stat blocks as an import file".to_string(),
                        })
                        .clicked()
                    {
                        let shown: Vec<StatBlock> = doc.c.statblocks.iter().filter(|s| in_collection(s, &st.collection)).cloned().collect();
                        let name = format!("{} stat blocks.json", cm_core::storage::sanitize_file_name(&what));
                        if let Some(p) = ui_util::save_file_dialog("Export stat blocks", &name, ("JSON", &["json"])) {
                            let json = bestiary::export_json(&shown, &what);
                            match std::fs::write(&p, json) {
                                Ok(()) => sh.toast(format!("Exported {}", ui_util::plural(shown.len(), "stat block"))),
                                Err(e) => sh.toast(format!("Export failed: {e}")),
                            }
                        }
                    }
                    let label = if st.select_mode { "✔ Done selecting" } else { "☑ Select" };
                    if ui
                        .add(egui::Button::new(label).selected(st.select_mode))
                        .on_hover_text("Tick several stat blocks to delete them or move them to another collection")
                        .clicked()
                    {
                        st.select_mode = !st.select_mode;
                        st.checked.clear();
                        st.confirm_delete = None;
                    }
                }
            });
            ui_util::filter_box(ui, &mut st.filter, "Filter: name, type, tag, “cr 2”…");
            // creature type filter
            let mut types: Vec<String> = match st.tab {
                Tab::Campaign => doc.c.statblocks.iter().filter(|s| in_collection(s, &st.collection)).map(main_type).collect(),
                Tab::Srd => srd().iter().map(main_type).collect(),
            };
            types.retain(|t| !t.is_empty());
            types.sort();
            types.dedup();
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("sb_type")
                    .selected_text(st.type_filter.clone().unwrap_or("Any type".into()))
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut st.type_filter, None, "Any type");
                        for t in &types {
                            ui.selectable_value(&mut st.type_filter, Some(t.clone()), t);
                        }
                    });
                ui.label("Sort");
                ui.selectable_value(&mut st.sort, Sort::Name, "Name");
                ui.selectable_value(&mut st.sort, Sort::Cr, "CR");
            });
            let f = st.filter.trim().to_lowercase();
            // what the list shows, in order (grouped by collection when showing all)
            let order: Vec<String> = doc.c.statblock_collections().into_iter().map(|(n, _)| n).collect();
            let shown: Vec<Id> = if st.tab == Tab::Campaign {
                let mut v: Vec<&StatBlock> = doc
                    .c
                    .statblocks
                    .iter()
                    .filter(|s| in_collection(s, &st.collection) && matches(s, &f, &st.type_filter))
                    .collect();
                let sorted_ids = sorted(v.drain(..).enumerate().map(|(i, s)| (i, s)), st.sort);
                let mut out: Vec<(usize, Id)> = sorted_ids
                    .iter()
                    .map(|(_, s)| (order.iter().position(|o| *o == s.collection_label()).unwrap_or(usize::MAX), s.id))
                    .collect();
                out.sort_by_key(|(g, _)| *g); // stable: keeps the name/CR order inside a group
                out.into_iter().map(|(_, id)| id).collect()
            } else {
                Vec::new()
            };
            if st.tab == Tab::Campaign && st.select_mode {
                bulk_bar(ui, doc, sh, st, &shown);
            }
            ui.separator();
            match st.tab {
                Tab::Campaign => {
                    egui::ScrollArea::vertical().id_salt("sb_scroll").show(ui, |ui| {
                        if doc.c.statblocks.is_empty() {
                            ui.weak("No stat blocks yet. Create one, import a file, or add some from the SRD 5.2 tab.");
                        } else if shown.is_empty() {
                            ui.weak("Nothing matches.");
                        }
                        let grouped = st.collection.is_none() && order.len() > 1;
                        let mut last_group = String::new();
                        let shift = ui.input(|i| i.modifiers.shift);
                        for (idx, id) in shown.iter().enumerate() {
                            let Some(s) = doc.c.statblock(*id) else { continue };
                            if grouped && s.collection_label() != last_group {
                                last_group = s.collection_label();
                                ui.add_space(4.0);
                                ui.label(RichText::new(&last_group).small().strong().color(ui.visuals().hyperlink_color));
                            }
                            let cr = if s.core.cr.trim().is_empty() {
                                String::new()
                            } else {
                                format!("  CR {}", s.core.cr.trim())
                            };
                            let text = format!("{}{cr}", s.name());
                            if st.select_mode {
                                let mut on = st.checked.contains(id);
                                let r = ui.checkbox(&mut on, text);
                                if r.clicked() {
                                    // shift-click ticks everything between this and the last click
                                    if let (true, Some(last)) = (shift, st.last_clicked) {
                                        let (a, b) = if last <= idx { (last, idx) } else { (idx, last) };
                                        for j in a..=b {
                                            if on {
                                                st.checked.insert(shown[j]);
                                            } else {
                                                st.checked.remove(&shown[j]);
                                            }
                                        }
                                    } else if on {
                                        st.checked.insert(*id);
                                    } else {
                                        st.checked.remove(id);
                                    }
                                    st.last_clicked = Some(idx);
                                }
                            } else {
                                let r = ui_util::list_row(ui, st.selected == Some(s.id), text);
                                if r.clicked() {
                                    st.selected = Some(s.id);
                                    st.editing = false;
                                }
                                r.on_hover_text(format!("{}\n{} · {}", s.subtitle(), s.collection_label(), s.source));
                            }
                        }
                    });
                }
                Tab::Srd => {
                    let fresh = st
                        .srd_cache
                        .as_ref()
                        .map(|(cf, ct, cs, _)| *cf == f && *ct == st.type_filter && *cs == st.sort)
                        .unwrap_or(false);
                    if !fresh {
                        let list: Vec<usize> = sorted(srd().iter().enumerate().filter(|(_, s)| matches(s, &f, &st.type_filter)), st.sort)
                            .into_iter()
                            .map(|(i, _)| i)
                            .collect();
                        st.srd_cache = Some((f.clone(), st.type_filter.clone(), st.sort, list));
                    }
                    let list = st.srd_cache.as_ref().map(|c| c.3.clone()).unwrap_or_default();
                    ui.weak(format!("{} shown", list.len()));
                    // only the visible rows are laid out
                    let row_h = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
                    egui::ScrollArea::vertical()
                        .id_salt("srd_rows")
                        .auto_shrink([false, false])
                        .show_rows(ui, row_h, list.len(), |ui, range| {
                            for &i in &list[range] {
                                let s = &srd()[i];
                                let cr = if s.core.cr.trim().is_empty() {
                                    String::new()
                                } else {
                                    format!("  CR {}", s.core.cr.trim())
                                };
                                if ui_util::list_row(ui, st.srd_selected == Some(i), format!("{}{cr}", s.name()))
                                    .on_hover_text(s.subtitle())
                                    .clicked()
                                {
                                    st.srd_selected = Some(i);
                                }
                            }
                        });
                }
            }
        });
}

fn in_collection(s: &StatBlock, c: &Option<String>) -> bool {
    match c {
        None => true,
        Some(c) => s.collection_label() == *c,
    }
}

/// "All · My stat blocks · Essential NPCs · SRD 5.2" filter buttons, plus collection actions.
fn collections_row(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    let cols = doc.c.statblock_collections();
    if st.collection.as_ref().map(|c| !cols.iter().any(|(n, _)| n == c)).unwrap_or(false) {
        st.collection = None;
    }
    if cols.len() < 2 && st.collection.is_none() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.weak("Show");
        if ui
            .selectable_label(st.collection.is_none(), format!("All ({})", doc.c.statblocks.len()))
            .clicked()
        {
            st.collection = None;
        }
        for (name, n) in &cols {
            if ui
                .selectable_label(st.collection.as_deref() == Some(name.as_str()), format!("{name} ({n})"))
                .on_hover_text("Show only this collection")
                .clicked()
            {
                st.collection = Some(name.clone());
                st.checked.clear();
                st.rename_to = None;
            }
        }
        if let Some(c) = st.collection.clone() {
            ui_util::form_menu(ui, "⋯", |ui| {
                ui.set_min_width(240.0);
                ui.label(RichText::new(&c).strong());
                if c != bestiary::MY_COLLECTION {
                    ui.horizontal(|ui| {
                        let buf = st.rename_to.get_or_insert_with(|| c.clone());
                        ui.add(egui::TextEdit::singleline(buf).desired_width(150.0));
                        let new = buf.trim().to_string();
                        if ui.add_enabled(!new.is_empty() && new != c, egui::Button::new("Rename")).clicked() {
                            for s in doc.c.statblocks.iter_mut().filter(|s| s.collection_label() == c) {
                                s.collection = if new == bestiary::MY_COLLECTION { String::new() } else { new.clone() };
                            }
                            st.collection = Some(new);
                            st.rename_to = None;
                            ui.close();
                        }
                    });
                }
                let ids: Vec<Id> = doc.c.statblocks.iter().filter(|s| s.collection_label() == c).map(|s| s.id).collect();
                if ui
                    .button(RichText::new(format!("🗑 Delete all {} in “{c}”…", ids.len())).color(ui.visuals().error_fg_color))
                    .clicked()
                {
                    st.select_mode = true;
                    st.checked = ids.iter().copied().collect();
                    st.confirm_delete = Some(ids);
                    ui.close();
                }
                let _ = &sh;
            });
        }
    });
}

/// Bulk actions for ticked stat blocks: select all/none, move to a collection, delete.
fn bulk_bar(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, shown: &[Id]) {
    st.checked.retain(|id| doc.c.statblock(*id).is_some());
    let n = st.checked.len();
    egui::Frame::group(ui.style())
        .fill(ui.visuals().selection.bg_fill.gamma_multiply(0.15))
        .inner_margin(6)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("{n} selected")).strong());
                if ui
                    .small_button("All shown")
                    .on_hover_text("Tick everything in the list (shift-click ticks a range)")
                    .clicked()
                {
                    st.checked.extend(shown.iter().copied());
                }
                if ui.small_button("None").clicked() {
                    st.checked.clear();
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled_ui(n > 0, |ui| {
                    ui_util::form_menu(ui, "Move to…", |ui| {
                        ui.set_min_width(220.0);
                        let mut target: Option<String> = None;
                        for (name, _) in doc.c.statblock_collections() {
                            if ui.button(&name).clicked() {
                                target = Some(name);
                            }
                        }
                        ui.separator();
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
                            for s in doc.c.statblocks.iter_mut().filter(|s| st.checked.contains(&s.id)) {
                                s.collection = if t == bestiary::MY_COLLECTION { String::new() } else { t.clone() };
                            }
                            sh.toast(format!("Moved {} to “{t}”", ui_util::plural(st.checked.len(), "stat block")));
                            st.new_collection.clear();
                            ui.close();
                        }
                    });
                    if ui.button(RichText::new(format!("🗑 Delete {n}…")).color(ui.visuals().error_fg_color)).clicked() {
                        st.confirm_delete = Some(st.checked.iter().copied().collect());
                    }
                });
            });
            if let Some(ids) = st.confirm_delete.clone() {
                let users = doc.c.npcs.iter().filter(|npc| ids.iter().any(|s| npc.uses_statblock(*s))).count();
                ui.separator();
                ui.label(RichText::new(format!("Delete {}?", ui_util::plural(ids.len(), "stat block"))).strong());
                if users > 0 {
                    ui.weak(format!(
                        "{} using them will keep their own stats but lose the stat block.",
                        ui_util::plural(users, "NPC")
                    ));
                }
                ui.weak("They're also removed from premade encounters. Edit » Undo brings everything back.");
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Delete").color(ui.visuals().error_fg_color).strong()).clicked() {
                        let n = doc.c.delete_statblocks(&ids);
                        sh.toast(format!("Deleted {} (Ctrl+Z to undo)", ui_util::plural(n, "stat block")));
                        st.checked.clear();
                        st.confirm_delete = None;
                        st.select_mode = false;
                        if st.selected.map(|s| ids.contains(&s)).unwrap_or(false) {
                            st.selected = None;
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        st.confirm_delete = None;
                    }
                });
            }
        });
}

fn srd_detail(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    let Some(sb) = st.srd_selected.and_then(|i| srd().get(i)) else {
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);
            ui.weak(format!("{} monsters from the System Reference Document 5.2 (2024 rules).", srd().len()));
            ui.weak("Select one to read it; add it to this campaign to use it in encounters or assign it to NPCs.");
            ui.add_space(20.0);
            ui.label(RichText::new(bestiary::SRD_ATTRIBUTION).small().weak());
        });
        return;
    };
    let name = sb.name();
    ui.horizontal(|ui| {
        let already = doc.c.statblocks.iter().find(|s| s.name() == name && s.source == sb.source).map(|s| s.id);
        if let Some(id) = already {
            if ui.button("Open campaign copy").clicked() {
                st.tab = Tab::Campaign;
                st.selected = Some(id);
            }
        } else if ui
            .button(RichText::new("➕ Add to campaign").strong())
            .on_hover_text("Copy into this campaign's bestiary so you can edit it, assign it to NPCs and use it in encounters")
            .clicked()
        {
            let (id, _) = adopt_srd(doc, sb);
            sh.toast(format!("Added {name} to the campaign bestiary"));
            st.selected = Some(id);
        }
        if ui.button("⚔ Add to combat").on_hover_text("Adds it to the campaign bestiary too").clicked() {
            let (id, _) = adopt_srd(doc, sb);
            sh.actions.push(Action::AddToCombat(CombatantSource::StatBlock(id)));
        }
        if ui.button("👤 Create NPC").on_hover_text("A named NPC that uses this stat block").clicked() {
            let (id, _) = adopt_srd(doc, sb);
            create_npc(doc, sh, id);
        }
    });
    ui.separator();
    ui_util::pinned_scroll(ui, "srd_detail", |ui| {
        ui.set_max_width(ui.available_width().min(640.0));
        statblock::render(ui, sb, &name, sh, true);
        ui.add_space(10.0);
        ui.label(RichText::new("SRD 5.2 · CC-BY-4.0 · see Help » About for attribution").small().weak());
    });
}

fn create_npc(doc: &mut Doc, sh: &mut Shared, sid: Id) {
    let Some(sb) = doc.c.statblock(sid) else {
        return;
    };
    let mut n = Npc::default();
    n.core.name = sb.name();
    n.kind = if sb.core.class_or_type.to_lowercase().starts_with("humanoid") {
        NpcKind::Character
    } else {
        NpcKind::Monster
    };
    n.statblock = Some(sid);
    let nid = n.id;
    doc.c.npcs.push(n);
    sh.actions.push(Action::Open(Link::Npc(nid)));
    sh.toast("Created an NPC using this stat block — rename them and add their story");
}

fn campaign_detail(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    let Some(id) = st.selected.filter(|id| doc.c.statblock(*id).is_some()) else {
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);
            ui.label(RichText::new("Stat blocks are templates").strong());
            ui.weak("Use one for many NPCs (“Captain Vask” and “Old Greta” can both use Veteran),");
            ui.weak("or add them straight to encounters. Edit an NPC's own AC/HP to override the template.");
            ui.weak("An NPC can have several: a druid's wild shape, a villain's second phase.");
            ui.add_space(12.0);
            if ui.button("📥 Import from a book or PDF with AI…").clicked() {
                st.import = Some(ImportUi::default());
            }
            if ui.button("Browse the SRD 5.2 library").clicked() {
                st.tab = Tab::Srd;
            }
        });
        return;
    };
    let users: Vec<(Id, String)> = doc
        .c
        .npcs
        .iter()
        .filter(|n| n.uses_statblock(id))
        .map(|n| {
            let main = n.statblock == Some(id);
            (
                n.id,
                if main {
                    n.core.display_name()
                } else {
                    format!("{} (other form)", n.core.display_name())
                },
            )
        })
        .collect();
    let name = doc.c.statblock(id).unwrap().name();
    ui.heading(&name);
    ui.horizontal_wrapped(|ui| {
        let label = if st.editing { "✔ Done editing" } else { "✏ Edit" };
        if ui.add(egui::Button::new(label).selected(st.editing)).clicked() {
            st.editing = !st.editing;
        }
        if ui.button("⚔ Add to combat").clicked() {
            sh.actions.push(Action::AddToCombat(CombatantSource::StatBlock(id)));
        }
        if ui.button("👤 Create NPC").on_hover_text("A named NPC that uses this stat block").clicked() {
            create_npc(doc, sh, id);
        }
        let mut give: Option<Id> = None;
        ui_util::form_menu(ui, "👤 Give to an NPC…", |ui| {
            ui.set_min_width(260.0);
            ui.set_max_width(320.0);
            let r = ui.add(egui::TextEdit::singleline(&mut st.npc_filter).hint_text("Filter NPCs…").desired_width(260.0));
            if ui.memory(|m| m.focused().is_none()) {
                r.request_focus();
            }
            let f = st.npc_filter.trim().to_lowercase();
            egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                let mut list: Vec<&Npc> = doc.c.npcs.iter().filter(|n| f.is_empty() || n.core.name.to_lowercase().contains(&f)).collect();
                list.sort_by_key(|n| n.core.name.to_lowercase());
                if list.is_empty() {
                    ui.weak("No NPCs match.");
                }
                for n in list {
                    let note = if n.uses_statblock(id) {
                        " ✔"
                    } else if n.statblock.is_some() {
                        "  · as another form"
                    } else {
                        ""
                    };
                    if ui
                        .add_enabled(!n.uses_statblock(id), egui::Button::new(format!("{}{note}", n.core.display_name())))
                        .clicked()
                    {
                        give = Some(n.id);
                        ui.close();
                    }
                }
            });
        })
        .on_hover_text("Add this stat block to an existing NPC. If they already have one, it becomes another form (wild shape, phase 2…).");
        if let Some(nid) = give {
            if let Some(n) = doc.c.npc_mut(nid) {
                let other = n.statblock.is_some();
                n.add_statblock(id);
                let who = n.core.display_name();
                sh.toast(if other {
                    format!("Added to {who} as another form")
                } else {
                    format!("{who} now uses this stat block")
                });
            }
        }
        if ui.button("Duplicate").clicked() {
            let mut copy = doc.c.statblock(id).unwrap().clone();
            copy.id = cm_core::new_id();
            copy.core.name = format!("{} (variant)", copy.core.name);
            st.selected = Some(copy.id);
            st.editing = true;
            doc.c.statblocks.push(copy);
        }
        ui_util::confirm_delete_button(ui, Link::StatBlock(id), sh);
    });
    ui.weak(format!("Collection: {}", doc.c.statblock(id).map(|s| s.collection_label()).unwrap_or_default()));
    if !users.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.weak("Used by");
            for (nid, n) in &users {
                if ui.link(n).clicked() {
                    sh.actions.push(Action::Open(Link::Npc(*nid)));
                }
            }
        });
    }
    ui.separator();
    // only the recent rolls are offered ("From dice history"), so only those are copied
    let history: Vec<cm_core::dice::RollRecord> = {
        let h = &doc.c.dice.history;
        h[h.len().saturating_sub(40)..].to_vec()
    };
    let dir = doc.dir.clone();
    ui_util::pinned_scroll(ui, ("sb_detail", st.editing, st.selected), |ui| {
        if st.editing {
            let sb = doc.c.statblock_mut(id).unwrap();
            characters::core_editor(ui, &mut sb.core, Link::StatBlock(id), false, &dir, sh, &mut st.cst, &history);
            section(ui, "Stat block details");
            statblock::details_editor(ui, sb);
            section(ui, "Traits & actions");
            ui.weak("Fill in Attack / Save / Damage so the stat block gets roll buttons.");
            statblock::features_editor(ui, sb);
            ui.add_space(6.0);
            if ui
                .button("Add rolls from actions")
                .on_hover_text("Create custom rolls (to hit / damage) for any attack that doesn't have one yet")
                .clicked()
            {
                let before = sb.core.custom_rolls.len();
                sb.ensure_rolls();
                sh.toast(format!("Added {} rolls", sb.core.custom_rolls.len() - before));
            }
        } else {
            let sb = doc.c.statblock(id).unwrap().clone();
            ui.horizontal_top(|ui| {
                if let Some(img) = &sb.core.image {
                    if ui_util::image_thumb(ui, &dir, img, egui::vec2(110.0, 130.0)).clicked() {
                        sh.actions.push(Action::ViewImage(img.clone()));
                    }
                }
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width().min(640.0));
                    statblock::render(ui, &sb, &name, sh, false);
                });
            });
            if !sb.core.notes.trim().is_empty() {
                section(ui, "Notes");
                ui_util::render_markdown(ui, &mut sh.md, &sb.core.notes);
            }
            if !sb.core.sheet_images.is_empty() {
                section(ui, "Images");
                ui.horizontal_wrapped(|ui| {
                    for p in &sb.core.sheet_images {
                        if ui_util::image_thumb(ui, &dir, p, egui::vec2(220.0, 260.0)).clicked() {
                            sh.actions.push(Action::ViewImage(p.clone()));
                        }
                    }
                });
            }
            let extra: Vec<_> = sb
                .core
                .custom_rolls
                .iter()
                .filter(|r| !sb.derived_rolls().iter().any(|d| d.label == r.label))
                .cloned()
                .collect();
            if !extra.is_empty() {
                section(ui, "Custom rolls");
                ui.horizontal_wrapped(|ui| {
                    for r in extra {
                        if ui.button(format!("🎲 {}", r.label)).on_hover_text(&r.notation).clicked() {
                            sh.actions.push(Action::Roll {
                                label: r.label.clone(),
                                expr: r.notation.clone(),
                                roller: name.clone(),
                            });
                        }
                    }
                });
            }
            if !sb.core.tags.is_empty() {
                ui.weak(format!("Tags: {}", sb.core.tags.join(", ")));
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

fn import_window(ctx: &egui::Context, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    let Some(imp) = &mut st.import else { return };
    let mut open = true;
    let mut done: Option<Vec<Id>> = None;
    egui::Window::new("Import stat blocks")
        .open(&mut open)
        .default_width(560.0)
        .default_height(520.0)
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            if imp.report.is_none() {
                ui.label(RichText::new("1. Get the stat blocks as JSON").strong());
                ui.label(
                    "Give an AI (Claude, ChatGPT…) your PDF or screenshots together with the instructions below. It replies with a file this app can read.",
                );
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button("📋 Copy AI instructions")
                        .on_hover_text("Paste these into the AI chat along with your PDF")
                        .clicked()
                    {
                        ui.ctx().copy_text(bestiary::AI_SPEC.to_string());
                        sh.toast("AI instructions copied — paste them into the chat with your PDF");
                    }
                    if ui.button("💾 Save instructions as file…").clicked() {
                        if let Some(p) = ui_util::save_file_dialog("Save AI instructions", "statblock-import-spec.md", ("Markdown", &["md"])) {
                            if let Err(e) = std::fs::write(&p, bestiary::AI_SPEC) {
                                sh.toast(format!("Couldn't save: {e}"));
                            }
                        }
                    }
                });
                ui.weak("Also accepts files exported from another campaign's Bestiary.");
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
                        if let Some(p) = ui_util::pick_json_file("Choose a stat block file") {
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
                let report = imp.report.as_ref().unwrap();
                let n = imp.include.iter().filter(|b| **b).count();
                ui.label(
                    RichText::new(format!(
                        "Found {}{}",
                        ui_util::plural(report.statblocks.len(), "stat block"),
                        if report.source.is_empty() {
                            String::new()
                        } else {
                            format!(" from {}", report.source)
                        }
                    ))
                    .strong(),
                );
                ui.horizontal(|ui| {
                    ui.label("Put them in collection");
                    ui.add(egui::TextEdit::singleline(&mut imp.collection).desired_width(180.0))
                        .on_hover_text("Keeps a book's stat blocks together, separate from your own. Filter by it in the Bestiary.");
                    let existing: Vec<String> = doc.c.statblock_collections().into_iter().map(|(n, _)| n).collect();
                    if !existing.is_empty() {
                        egui::ComboBox::from_id_salt("imp_coll")
                            .selected_text("existing…")
                            .width(90.0)
                            .show_ui(ui, |ui| {
                                for n in existing {
                                    if ui.selectable_label(false, &n).clicked() {
                                        imp.collection = n;
                                    }
                                }
                            });
                    }
                });
                let target = target_collection(imp);
                let dup_count = report
                    .statblocks
                    .iter()
                    .zip(&imp.include)
                    .filter(|(s, inc)| {
                        **inc
                            && doc
                                .c
                                .statblocks
                                .iter()
                                .any(|x| x.collection_label() == target && x.name().eq_ignore_ascii_case(&s.name()))
                    })
                    .count();
                if dup_count > 0 {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(if dup_count == 1 {
                            "1 is already in this collection (same name):".to_string()
                        } else {
                            format!("{dup_count} are already in this collection (same name):")
                        });
                        ui.radio_value(&mut imp.duplicates, Duplicates::Replace, "Replace")
                            .on_hover_text("Overwrite the existing stat block; NPCs using it keep using it");
                        ui.radio_value(&mut imp.duplicates, Duplicates::KeepBoth, "Keep both");
                        ui.radio_value(&mut imp.duplicates, Duplicates::Skip, "Skip");
                    });
                }
                if !report.warnings.is_empty() {
                    egui::CollapsingHeader::new(
                        RichText::new(format!("⚠ {}", ui_util::plural(report.warnings.len(), "warning"))).color(ui.visuals().warn_fg_color),
                    )
                    .default_open(report.warnings.len() <= 6)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt("imp_warn").max_height(120.0).show(ui, |ui| {
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
                egui::ScrollArea::vertical().id_salt("imp_list").max_height(280.0).show(ui, |ui| {
                    for (i, s) in report.statblocks.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut imp.include[i], RichText::new(s.name()).strong());
                            ui.weak(format!("{}  {}", s.subtitle(), statblock::summary(s)));
                            let actions = s.actions.len() + s.bonus_actions.len() + s.reactions.len();
                            ui.weak(format!(
                                "· {}, {}",
                                ui_util::plural(actions, "action"),
                                ui_util::plural(s.derived_rolls().len(), "roll")
                            ));
                        });
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(n > 0, egui::Button::new(RichText::new(format!("Import {n}")).strong()))
                        .clicked()
                    {
                        done = Some(apply_import(doc, imp));
                    }
                    if ui.button("◀ Back").clicked() {
                        imp.report = None;
                    }
                });
            }
        });
    if let Some(ids) = done {
        let target = st.import.as_ref().map(target_collection).unwrap_or_default();
        sh.toast(format!("Imported {} into “{target}”", ui_util::plural(ids.len(), "stat block")));
        st.collection = Some(target);
        st.tab = Tab::Campaign;
        st.selected = ids.first().copied();
        st.editing = false;
        st.import = None;
    } else if !open {
        st.import = None;
    }
}

/// The collection label imports will be filed under.
fn target_collection(imp: &ImportUi) -> String {
    let c = imp.collection.trim();
    if c.is_empty() { bestiary::MY_COLLECTION.to_string() } else { c.to_string() }
}

fn parse(imp: &mut ImportUi) {
    match bestiary::import_json(&imp.text) {
        Ok(r) => {
            imp.include = vec![true; r.statblocks.len()];
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
            ));
        }
    }
}

fn apply_import(doc: &mut Doc, imp: &ImportUi) -> Vec<Id> {
    let Some(report) = &imp.report else {
        return Vec::new();
    };
    let target = target_collection(imp);
    let stored = if target == bestiary::MY_COLLECTION { String::new() } else { target.clone() };
    let mut ids = Vec::new();
    for (s, inc) in report.statblocks.iter().zip(&imp.include) {
        if !inc {
            continue;
        }
        let mut s = s.clone();
        s.collection = stored.clone();
        let s = &s;
        let existing = doc
            .c
            .statblocks
            .iter()
            .position(|x| x.collection_label() == target && x.name().eq_ignore_ascii_case(&s.name()));
        match (existing, imp.duplicates) {
            (Some(_), Duplicates::Skip) => continue,
            (Some(i), Duplicates::Replace) => {
                let keep_id = doc.c.statblocks[i].id;
                let mut new = s.clone();
                new.id = keep_id;
                doc.c.statblocks[i] = new;
                ids.push(keep_id);
            }
            _ => {
                ids.push(s.id);
                doc.c.statblocks.push(s.clone());
            }
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    #[test]
    fn natural_sort() {
        let mut v = vec!["NPC 10", "npc 2", "Bandit", "NPC 1"];
        v.sort_by(|a, b| super::natural_cmp(a, b));
        assert_eq!(v, ["Bandit", "NPC 1", "npc 2", "NPC 10"]);
        let mut w = vec!["NPC 10", "npc 2", "Bandit", "NPC 1", "Goblin 03", "Goblin 3a"];
        let mut x = w.clone();
        w.sort_by(|a, b| super::natural_cmp(a, b));
        x.sort_by_cached_key(|a| super::natural_key(a));
        assert_eq!(w, x);
    }
}
