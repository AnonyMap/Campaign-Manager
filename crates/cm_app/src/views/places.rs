//! Places: towns, shops, inns, temples, dungeons… arranged in a tree (a shop is *in* a town).
//! Each place can list its NPCs, factions and related places, stand in as a story card, get
//! a generated name, and — for shops — carry generated stock to sell to the party.

use std::collections::HashSet;

use cm_core::items::format_gp;
use cm_core::loot::{self, ShopQuality, ShopTheme, StockEntry};
use cm_core::places::{self, Location, PLACE_KINDS, Shop};
use cm_core::{Id, Link, Npc, StoryCard};
use egui::RichText;

use crate::app::{Action, Doc, Shared};
use crate::ui_util::{self, section};
use crate::views::{quests, treasure};

#[derive(Default)]
pub struct State {
    pub selected: Option<Id>,
    filter: String,
    kind: Option<String>,
    collapsed: HashSet<Id>,
    names: Vec<String>,
    names_for: Option<Id>,
    npc_filter: String,
    item_filter: String,
    char_filter: String,
    /// Selling gives the item away without taking payment.
    free: bool,
    stock_size: usize,
}

fn kinds(doc: &Doc) -> Vec<String> {
    let mut v: Vec<String> = PLACE_KINDS.iter().map(|s| s.to_string()).collect();
    for l in &doc.c.locations {
        let k = l.kind.trim();
        if !k.is_empty() && !v.iter().any(|x| x.eq_ignore_ascii_case(k)) {
            v.push(k.to_string());
        }
    }
    v
}

fn kind_icon(kind: &str) -> &'static str {
    let k = kind.to_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| k.contains(x));
    if has(&["city", "town", "village", "hamlet"]) {
        "🏠"
    } else if has(&["inn", "tavern", "pub"]) {
        "🍺"
    } else if has(&["temple", "shrine", "chapel", "church", "abbey"]) {
        "⛪"
    } else if has(&["castle", "keep", "fort", "tower", "manor"]) {
        "🏰"
    } else if has(&["dungeon", "ruin", "crypt", "tomb", "cave", "lair", "mine"]) {
        "💀"
    } else if has(&["wild", "forest", "region", "mountain", "swamp", "lake", "river", "landmark"]) {
        "🌲"
    } else if places::theme_for_kind(&k).is_some() || has(&["shop", "store", "market", "smith"]) {
        "🏪"
    } else {
        "📍"
    }
}

fn new_place(doc: &mut Doc, sh: &mut Shared, kind: &str, parent: Option<Id>) -> Id {
    let name = places::place_name(kind, &doc.c.settings.name_pools, &mut sh.rng);
    let mut l = Location {
        name,
        kind: kind.to_string(),
        parent,
        ..Default::default()
    };
    if let Some(theme) = places::theme_for_kind(kind) {
        let mut shop = Shop { theme, ..Default::default() };
        shop.stock = loot::stock_shop(&treasure::library(doc), theme, shop.quality, theme.default_size(), &mut sh.rng);
        l.shop = Some(shop);
    }
    let id = l.id;
    doc.c.locations.push(l);
    id
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    egui::SidePanel::left("places_list")
        .resizable(true)
        .default_width(260.0)
        .show_inside(ui, |ui| list(ui, doc, sh, st));
    let Some(id) = st.selected else {
        ui.centered_and_justified(|ui| {
            ui.weak("Select a place, or create one: a town, an inn, a shop, a temple, a dungeon — any kind you like.");
        });
        return;
    };
    if doc.c.location(id).is_none() {
        st.selected = None;
        return;
    }
    ui_util::pinned_scroll(ui, ("place_edit", id), |ui| editor(ui, doc, sh, st, id));
}

fn list(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    ui.heading("Places");
    let mut create: Option<String> = None;
    ui.horizontal(|ui| {
        ui.menu_button("➕ New place", |ui| {
            ui.weak("Pick a kind (you can type any kind later)");
            for k in PLACE_KINDS {
                if ui.button(format!("{} {k}", kind_icon(k))).clicked() {
                    create = Some(k.to_string());
                    ui.close();
                }
            }
            ui.separator();
            ui.label(RichText::new("Shops").small().strong());
            for k in ["General store", "Blacksmith", "Potion shop", "Magic shop", "Bookshop", "Bowyer", "Jeweler"] {
                if ui.button(format!("🏪 {k}")).clicked() {
                    create = Some(k.to_string());
                    ui.close();
                }
            }
        });
    });
    if let Some(k) = create {
        // new places go inside the selected town/city by default
        let parent = st.selected.filter(|s| doc.c.location(*s).map(|l| is_settlement(&l.kind)).unwrap_or(false));
        st.selected = Some(new_place(doc, sh, &k, parent));
    }
    ui_util::filter_box(ui, &mut st.filter, "Filter places…");
    let all_kinds: Vec<String> = {
        let mut v: Vec<String> = doc.c.locations.iter().map(|l| l.kind.trim().to_string()).filter(|k| !k.is_empty()).collect();
        v.sort_by_key(|k| k.to_lowercase());
        v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        v
    };
    if all_kinds.len() > 1 {
        egui::ComboBox::from_id_salt("place_kind_f")
            .width(160.0)
            .truncate()
            .selected_text(st.kind.clone().unwrap_or("Any kind".into()))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut st.kind, None, "Any kind");
                for k in &all_kinds {
                    ui.selectable_value(&mut st.kind, Some(k.clone()), k);
                }
            });
    }
    ui.separator();
    egui::ScrollArea::vertical().id_salt("places_scroll").show(ui, |ui| {
        if doc.c.locations.is_empty() {
            ui.weak("No places yet.");
            return;
        }
        let f = st.filter.to_lowercase();
        let filtering = !f.is_empty() || st.kind.is_some();
        if filtering {
            // flat list of matches with their path
            let mut hits: Vec<(Id, String, String)> = doc
                .c
                .locations
                .iter()
                .filter(|l| st.kind.as_ref().map(|k| l.kind.trim().eq_ignore_ascii_case(k)).unwrap_or(true))
                .filter(|l| {
                    f.is_empty() || l.name.to_lowercase().contains(&f) || l.kind.to_lowercase().contains(&f) || l.description.to_lowercase().contains(&f)
                })
                .map(|l| (l.id, format!("{} {}", kind_icon(&l.kind), l.display_name()), doc.c.location_path(l.id)))
                .collect();
            hits.sort_by_key(|h| h.1.to_lowercase());
            if hits.is_empty() {
                ui.weak("No places match.");
            }
            for (id, name, path) in hits {
                let r = ui_util::list_row(ui, st.selected == Some(id), name);
                if r.clicked() {
                    st.selected = Some(id);
                }
                r.on_hover_text(path);
            }
        } else {
            let mut roots: Vec<&Location> = doc.c.locations.iter().filter(|l| l.parent.and_then(|p| doc.c.location(p)).is_none()).collect();
            roots.sort_by_key(|l| l.display_name().to_lowercase());
            let mut click = None;
            for r in roots {
                tree_row(ui, doc, st, r, 0, &mut click);
            }
            if let Some(c) = click {
                st.selected = Some(c);
            }
        }
    });
}

fn is_settlement(kind: &str) -> bool {
    let k = kind.to_lowercase();
    ["city", "town", "village", "hamlet", "region", "district", "castle"]
        .iter()
        .any(|x| k.contains(x))
}

fn tree_row(ui: &mut egui::Ui, doc: &Doc, st: &mut State, l: &Location, depth: usize, click: &mut Option<Id>) {
    let kids = doc.c.child_locations(l.id);
    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 14.0);
        if kids.is_empty() {
            ui.add_space(18.0);
        } else {
            let open = !st.collapsed.contains(&l.id);
            if ui.small_button(if open { "⏷" } else { "⏵" }).clicked() {
                if open {
                    st.collapsed.insert(l.id);
                } else {
                    st.collapsed.remove(&l.id);
                }
            }
        }
        let mut text = format!("{} {}", kind_icon(&l.kind), l.display_name());
        if !kids.is_empty() && st.collapsed.contains(&l.id) {
            text.push_str(&format!("  ({})", kids.len()));
        }
        let r = ui_util::list_row(ui, st.selected == Some(l.id), text);
        if r.clicked() {
            *click = Some(l.id);
        }
        if !l.kind.is_empty() {
            r.on_hover_text(&l.kind);
        }
    });
    if !st.collapsed.contains(&l.id) && depth < 12 {
        for k in kids {
            tree_row(ui, doc, st, k, depth + 1, click);
        }
    }
}

fn editor(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, id: Id) {
    let dir = doc.dir.clone();
    let kinds = kinds(doc);
    let pools = doc.c.settings.name_pools.clone();
    // possible parents: anything that wouldn't make a loop
    let parents: Vec<(Id, String)> = {
        let mut v: Vec<(Id, String)> = doc
            .c
            .locations
            .iter()
            .filter(|p| p.id != id && !doc.c.location_would_loop(id, p.id))
            .map(|p| (p.id, doc.c.location_path(p.id)))
            .collect();
        v.sort_by_key(|x| x.1.to_lowercase());
        v
    };
    let path = doc.c.location_path(id);
    if st.names_for != Some(id) {
        st.names.clear();
        st.names_for = Some(id);
    }

    let l = doc.c.location_mut(id).unwrap();
    ui.horizontal(|ui| {
        ui.weak(if path.contains('›') { path.clone() } else { "Place".into() });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui_util::confirm_delete_button(ui, Link::Location(id), sh);
            if ui.button("Export to wiki").clicked() {
                sh.actions.push(Action::ExportToWiki(Link::Location(id)));
            }
        });
    });
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut l.name)
                .hint_text("Name")
                .font(egui::TextStyle::Heading)
                .desired_width(ui.available_width() - 140.0),
        );
        if ui.button("🎲 Name").on_hover_text("Suggest names for this kind of place").clicked() {
            let kind = if l.kind.trim().is_empty() { "Town".to_string() } else { l.kind.clone() };
            st.names = places::place_names(&kind, &pools, 8, &mut sh.rng);
        }
    });
    if !st.names.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.weak("Pick one:");
            let mut chosen = None;
            for n in &st.names {
                if ui.small_button(n).clicked() {
                    chosen = Some(n.clone());
                }
            }
            if ui.small_button("🎲 More").clicked() {
                let kind = if l.kind.trim().is_empty() { "Town".to_string() } else { l.kind.clone() };
                st.names = places::place_names(&kind, &pools, 8, &mut sh.rng);
            }
            if ui.small_button("✖").clicked() {
                st.names.clear();
            }
            if let Some(n) = chosen {
                l.name = n;
                st.names.clear();
            }
        });
        ui.weak("Tip: add a name pool in Settings named after a kind (e.g. “Inn”) to mix in your own names.");
    }

    egui::Grid::new(("place_grid", id)).num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Kind");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut l.kind)
                    .id(egui::Id::new(("place_kind", id)))
                    .hint_text("Town, inn, potion shop…")
                    .desired_width(180.0),
            );
            egui::ComboBox::from_id_salt(("place_kind_pick", id))
                .selected_text("")
                .width(24.0)
                .truncate()
                .show_ui(ui, |ui| {
                    for k in &kinds {
                        if ui.selectable_label(l.kind.eq_ignore_ascii_case(k), format!("{} {k}", kind_icon(k))).clicked() {
                            l.kind = k.clone();
                        }
                    }
                });
        });
        ui.end_row();

        ui.label("Inside");
        let current = l
            .parent
            .and_then(|p| parents.iter().find(|x| x.0 == p))
            .map(|x| x.1.clone())
            .unwrap_or("— nowhere (top level) —".into());
        egui::ComboBox::from_id_salt(("place_parent", id))
            .selected_text(current)
            .width(260.0)
            .truncate()
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut l.parent, None, "— nowhere (top level) —");
                for (pid, name) in &parents {
                    ui.selectable_value(&mut l.parent, Some(*pid), name);
                }
            });
        ui.end_row();

        ui.label("Tags");
        ui_util::tags_edit(ui, ("place_tags", id), &mut l.tags);
        ui.end_row();
    });
    ui.horizontal_top(|ui| match l.image.clone() {
        Some(p) => {
            ui.vertical(|ui| {
                if ui_util::image_thumb(ui, &dir, &p, egui::vec2(220.0, 160.0)).clicked() {
                    sh.actions.push(Action::ViewImage(p));
                }
                if ui.small_button("Remove image").clicked() {
                    l.image = None;
                }
            });
        }
        None => {
            if let Some(rel) = ui_util::add_image_button(ui, ("placeimg", id), &dir, "Add a picture", sh) {
                l.image = Some(rel);
            }
        }
    });
    let is_shop = l.shop.is_some();
    if is_shop {
        shop(ui, doc, sh, st, id);
    }
    let l = doc.c.location_mut(id).unwrap();
    section(ui, "Description");
    ui_util::markdown_editor(ui, ("placedesc", id), &mut l.description, &mut sh.md, 4);
    section(ui, "DM notes");
    ui_util::markdown_editor(ui, ("placenotes", id), &mut l.notes, &mut sh.md, 3);

    story_card(ui, doc, sh, id);
    places_here(ui, doc, sh, st, id);
    people(ui, doc, sh, st, id);
    if !is_shop {
        shop(ui, doc, sh, st, id);
    }
    quests::quests_given(ui, doc, &Link::Location(id), sh);

    section(ui, "Links");
    let mut links = doc.c.location_mut(id).map(|l| std::mem::take(&mut l.links)).unwrap_or_default();
    ui_util::link_images(ui, &dir, &links, sh);
    ui_util::link_chips(ui, &doc.c, &mut links, sh);
    let vault = sh.vault.take();
    ui_util::link_picker(ui, ("placelinks", id), &doc.c, &mut links, Some(&Link::Location(id)), &dir, vault.as_ref(), sh);
    sh.vault = vault;
    if let Some(l) = doc.c.location_mut(id) {
        l.links = links;
    }
    ui_util::backlinks(ui, &doc.c, &Link::Location(id), sh);
}

/// The story card that stands for this place.
fn story_card(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, id: Id) {
    let card = doc.c.location(id).and_then(|l| l.card).filter(|c| doc.c.card(*c).is_some());
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| match card {
        Some(cid) => {
            ui.label("🃏 Story card:");
            let title = doc.c.card(cid).map(ui_util::card_title).unwrap_or_default();
            if ui.link(title).clicked() {
                sh.actions.push(Action::Open(Link::Card(cid)));
            }
            if ui.small_button("🕸 Story board").clicked() {
                sh.actions.push(Action::ShowOnBoard(cid));
            }
            if ui
                .small_button("Unlink")
                .on_hover_text("Keep the card, but it no longer stands for this place")
                .clicked()
            {
                if let Some(l) = doc.c.location_mut(id) {
                    l.card = None;
                }
            }
        }
        None => {
            if ui
                .button("🃏 Use as a story card")
                .on_hover_text("Make a story card for this place, so it can go on the plot line, the story board, session plans and maps")
                .clicked()
            {
                let l = doc.c.location(id).unwrap();
                let card = StoryCard {
                    title: l.display_name(),
                    body: l.description.clone(),
                    links: vec![Link::Location(id)],
                    tags: vec!["place".into()],
                    in_ideas_pile: false,
                    ..Default::default()
                };
                let cid = card.id;
                doc.c.story_cards.push(card);
                if let Some(l) = doc.c.location_mut(id) {
                    l.card = Some(cid);
                }
                sh.toast("Story card created");
            }
        }
    });
}

fn places_here(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, id: Id) {
    section(ui, "Places here");
    let kids: Vec<(Id, String)> = doc
        .c
        .child_locations(id)
        .iter()
        .map(|k| (k.id, format!("{} {}", kind_icon(&k.kind), k.display_name())))
        .collect();
    let mut create = None;
    ui.horizontal_wrapped(|ui| {
        for (kid, name) in &kids {
            if ui.link(name).clicked() {
                st.selected = Some(*kid);
            }
        }
        if kids.is_empty() {
            ui.weak("Nothing inside yet.");
        }
        ui.menu_button("➕ Add a place here", |ui| {
            for k in [
                "Inn",
                "Tavern",
                "General store",
                "Blacksmith",
                "Potion shop",
                "Magic shop",
                "Temple",
                "Guild hall",
                "Bookshop",
                "Jeweler",
            ] {
                if ui.button(format!("{} {k}", kind_icon(k))).clicked() {
                    create = Some(k.to_string());
                    ui.close();
                }
            }
            ui.separator();
            for k in PLACE_KINDS.iter().filter(|k| !["Inn", "Tavern", "Temple", "Guild hall", "Shop"].contains(k)) {
                if ui.button(format!("{} {k}", kind_icon(k))).clicked() {
                    create = Some(k.to_string());
                    ui.close();
                }
            }
        });
    });
    if let Some(k) = create {
        let new = new_place(doc, sh, &k, Some(id));
        st.collapsed.remove(&id);
        sh.toast(format!("Added {}", doc.c.location(new).map(|l| l.display_name()).unwrap_or_default()));
    }
    // related places (not inside)
    let related: Vec<(Id, String)> = doc
        .c
        .location(id)
        .map(|l| l.related.iter().filter_map(|r| doc.c.location(*r).map(|x| (x.id, x.display_name()))).collect())
        .unwrap_or_default();
    let mut remove = None;
    let mut add = None;
    ui.horizontal_wrapped(|ui| {
        ui.weak("Related:");
        for (rid, name) in &related {
            if ui.link(name).clicked() {
                st.selected = Some(*rid);
            }
            if ui.small_button("✖").clicked() {
                remove = Some(*rid);
            }
        }
        ui.menu_button("➕", |ui| {
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for l in doc.c.locations.iter().filter(|l| l.id != id && !related.iter().any(|r| r.0 == l.id)) {
                    if ui.button(l.display_name()).clicked() {
                        add = Some(l.id);
                        ui.close();
                    }
                }
            });
        });
    });
    if let Some(l) = doc.c.location_mut(id) {
        if let Some(r) = remove {
            l.related.retain(|x| *x != r);
        }
        if let Some(a) = add {
            l.related.push(a);
        }
    }
}

fn people(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, id: Id) {
    section(ui, "People and factions");
    let Some(l) = doc.c.location(id) else { return };
    let npcs: Vec<(Id, String, String)> = l
        .npcs
        .iter()
        .filter_map(|n| doc.c.npc(*n).map(|x| (x.id, x.core.display_name(), x.role.clone())))
        .collect();
    let facs: Vec<(Id, String)> = l.factions.iter().filter_map(|f| doc.c.faction(*f).map(|x| (x.id, x.name.clone()))).collect();
    let name = l.display_name();
    // NPCs whose free-text location mentions this place but aren't listed here or in a place inside it
    let mut inside: Vec<Id> = Vec::new();
    let mut stack = vec![id];
    while let Some(p) = stack.pop() {
        for k in doc.c.child_locations(p) {
            if !inside.contains(&k.id) {
                inside.push(k.id);
                stack.push(k.id);
            }
        }
    }
    let placed_below: Vec<Id> = doc
        .c
        .locations
        .iter()
        .filter(|x| inside.contains(&x.id))
        .flat_map(|x| x.npcs.iter().copied())
        .collect();
    let mentioned: Vec<(Id, String)> = doc
        .c
        .npcs
        .iter()
        .filter(|n| !l.npcs.contains(&n.id) && !placed_below.contains(&n.id) && !n.location.trim().is_empty() && !l.name.trim().is_empty())
        .filter(|n| n.location.to_lowercase().contains(&l.name.trim().to_lowercase()))
        .map(|n| (n.id, n.core.display_name()))
        .collect();
    let mut remove_npc = None;
    let mut add_npc = None;
    let mut new_npc = false;
    let mut remove_fac = None;
    let mut add_fac = None;
    ui.horizontal_wrapped(|ui| {
        ui.weak("NPCs:");
        for (nid, n, role) in &npcs {
            let r = ui.link(n);
            if r.clicked() {
                sh.actions.push(Action::Open(Link::Npc(*nid)));
            }
            if !role.is_empty() {
                r.on_hover_text(role);
            }
            if ui.small_button("✖").clicked() {
                remove_npc = Some(*nid);
            }
        }
        ui.menu_button("➕", |ui| {
            ui.set_min_width(220.0);
            if ui.button("New NPC here").clicked() {
                new_npc = true;
                ui.close();
            }
            ui.separator();
            ui.add(egui::TextEdit::singleline(&mut st.npc_filter).hint_text("Find…").desired_width(f32::INFINITY));
            let f = st.npc_filter.to_lowercase();
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for n in doc.c.npcs.iter().filter(|n| !npcs.iter().any(|x| x.0 == n.id)) {
                    let nm = n.core.display_name();
                    if (f.is_empty() || nm.to_lowercase().contains(&f)) && ui.button(nm).clicked() {
                        add_npc = Some(n.id);
                        ui.close();
                    }
                }
            });
        });
    });
    if !mentioned.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.weak("Their location says they're here:");
            for (nid, n) in &mentioned {
                if ui.small_button(format!("➕ {n}")).on_hover_text("Add to this place").clicked() {
                    add_npc = Some(*nid);
                }
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        ui.weak("Factions:");
        for (fid, n) in &facs {
            if ui.link(n).clicked() {
                sh.actions.push(Action::Open(Link::Faction(*fid)));
            }
            if ui.small_button("✖").clicked() {
                remove_fac = Some(*fid);
            }
        }
        ui.menu_button("➕", |ui| {
            for f in doc.c.factions.iter().filter(|f| !facs.iter().any(|x| x.0 == f.id)) {
                if ui.button(&f.name).clicked() {
                    add_fac = Some(f.id);
                    ui.close();
                }
            }
            if doc.c.factions.is_empty() {
                ui.weak("No factions yet.");
            }
        });
    });
    if new_npc {
        let mut n = Npc::default();
        n.core.name = "New NPC".into();
        n.location = name.clone();
        add_npc = Some(n.id);
        doc.c.npcs.push(n);
        sh.actions.push(Action::Open(Link::Npc(add_npc.unwrap())));
    }
    if let Some(l) = doc.c.location_mut(id) {
        if let Some(r) = remove_npc {
            l.npcs.retain(|x| *x != r);
        }
        if let Some(a) = add_npc {
            if !l.npcs.contains(&a) {
                l.npcs.push(a);
            }
        }
        if let Some(r) = remove_fac {
            l.factions.retain(|x| *x != r);
        }
        if let Some(a) = add_fac {
            l.factions.push(a);
        }
    }
}

fn shop(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, id: Id) {
    let has_shop = doc.c.location(id).map(|l| l.shop.is_some()).unwrap_or(false);
    if !has_shop {
        ui.add_space(6.0);
        if ui
            .button("🏪 Sells things…")
            .on_hover_text("Give this place a shop: generated wares with prices, sold straight to the party")
            .clicked()
        {
            let kind = doc.c.location(id).map(|l| l.kind.clone()).unwrap_or_default();
            let theme = places::theme_for_kind(&kind).unwrap_or(ShopTheme::General);
            if let Some(l) = doc.c.location_mut(id) {
                l.shop = Some(Shop { theme, ..Default::default() });
            }
        }
        return;
    }
    section(ui, "🏪 Shop");
    let lib = treasure::library(doc);
    let keeper_name = doc
        .c
        .location(id)
        .and_then(|l| l.shop.as_ref())
        .and_then(|s| s.keeper)
        .and_then(|k| doc.c.npc(k))
        .map(|n| n.core.display_name());
    let npcs_here: Vec<(Id, String)> = doc
        .c
        .location(id)
        .map(|l| l.npcs.iter().filter_map(|n| doc.c.npc(*n).map(|x| (x.id, x.core.display_name()))).collect())
        .unwrap_or_default();
    let all_npcs: Vec<(Id, String)> = doc.c.npcs.iter().map(|n| (n.id, n.core.display_name())).collect();
    let mut generate: Option<bool> = None; // Some(replace)
    let mut remove_shop = false;
    {
        let shop = doc.c.location_mut(id).unwrap().shop.as_mut().unwrap();
        if st.stock_size == 0 {
            st.stock_size = shop.theme.default_size();
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Sells");
            egui::ComboBox::from_id_salt(("shop_theme", id))
                .selected_text(shop.theme.label())
                .show_ui(ui, |ui| {
                    for t in ShopTheme::ALL {
                        if ui.selectable_value(&mut shop.theme, t, t.label()).clicked() {
                            st.stock_size = t.default_size();
                        }
                    }
                });
            ui.label("Quality");
            egui::ComboBox::from_id_salt(("shop_quality", id))
                .selected_text(shop.quality.label())
                .show_ui(ui, |ui| {
                    for q in ShopQuality::ALL {
                        ui.selectable_value(&mut shop.quality, q, q.label());
                    }
                })
                .response
                .on_hover_text("Poor shops are cheap with little magic; fine shops charge more and stock rarer things");
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Shopkeeper");
            egui::ComboBox::from_id_salt(("shop_keeper", id))
                .selected_text(keeper_name.clone().unwrap_or("— none —".into()))
                .width(180.0)
                .truncate()
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut shop.keeper, None, "— none —");
                    if !npcs_here.is_empty() {
                        ui.label(RichText::new("Here").small().strong());
                        for (nid, n) in &npcs_here {
                            ui.selectable_value(&mut shop.keeper, Some(*nid), n);
                        }
                        ui.separator();
                    }
                    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                        for (nid, n) in all_npcs.iter().filter(|x| !npcs_here.iter().any(|h| h.0 == x.0)) {
                            ui.selectable_value(&mut shop.keeper, Some(*nid), n);
                        }
                    });
                });
            if let Some(k) = shop.keeper {
                if ui.small_button("Open").clicked() {
                    sh.actions.push(Action::Open(Link::Npc(k)));
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::DragValue::new(&mut st.stock_size).range(1..=60).suffix(" wares"));
            if ui
                .button(RichText::new("🎲 Restock").strong())
                .on_hover_text("Replace the stock with freshly generated wares")
                .clicked()
            {
                generate = Some(true);
            }
            if ui
                .button("🎲 Add more")
                .on_hover_text("Generate more wares without removing the current ones")
                .clicked()
            {
                generate = Some(false);
            }
            if !shop.stock.is_empty() && ui.button("Clear").clicked() {
                shop.stock.clear();
            }
            if ui.small_button("Close shop").on_hover_text("Remove the shop from this place").clicked() {
                remove_shop = true;
            }
        });
    }
    if remove_shop {
        if let Some(l) = doc.c.location_mut(id) {
            l.shop = None;
        }
        return;
    }
    if let Some(replace) = generate {
        let (theme, quality) = {
            let s = doc.c.location(id).unwrap().shop.as_ref().unwrap();
            (s.theme, s.quality)
        };
        let new = loot::stock_shop(&lib, theme, quality, st.stock_size, &mut sh.rng);
        let shop = doc.c.location_mut(id).unwrap().shop.as_mut().unwrap();
        if replace {
            shop.stock = new;
        } else {
            for e in new {
                match shop.stock.iter_mut().find(|x| x.item.name == e.item.name) {
                    Some(x) => x.qty += e.qty,
                    None => shop.stock.push(e),
                }
            }
        }
    }
    stock_table(ui, doc, sh, st, id);
}

fn stock_table(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, id: Id) {
    let stock: Vec<StockEntry> = doc.c.location(id).and_then(|l| l.shop.as_ref()).map(|s| s.stock.clone()).unwrap_or_default();
    let mut remove = None;
    let mut sell: Option<(usize, Link)> = None;
    let mut edits: Vec<(usize, f64, u32)> = Vec::new();
    if stock.is_empty() {
        ui.weak("Nothing for sale. Click Restock to generate wares.");
    } else {
        egui::Grid::new(("stock", id)).num_columns(5).striped(true).spacing([10.0, 3.0]).show(ui, |ui| {
            ui.weak("Item");
            ui.weak("Price");
            ui.weak("In stock");
            ui.label("");
            ui.label("");
            ui.end_row();
            for (i, e) in stock.iter().enumerate() {
                let r = ui.label(RichText::new(&e.item.name).color(treasure::rarity_color(ui, e.item.rarity)));
                let it = e.item.clone();
                r.on_hover_ui(|ui| {
                    ui.set_max_width(360.0);
                    ui.label(RichText::new(it.subtitle()).italics());
                    if it.value_gp() > 0.0 {
                        ui.weak(format!("Book price {}", format_gp(it.value_gp())));
                    }
                    let d: String = it.description.chars().take(400).collect();
                    if !d.is_empty() {
                        ui.label(d);
                    }
                });
                let mut price = e.price_gp;
                let mut qty = e.qty;
                let speed = price.max(1.0) * 0.02;
                let p = ui.add(
                    egui::DragValue::new(&mut price)
                        .range(0.0..=10_000_000.0)
                        .speed(speed)
                        .custom_formatter(|v, _| format_gp(v)),
                );
                let q = ui.add(egui::DragValue::new(&mut qty).range(0..=999));
                if p.changed() || q.changed() {
                    edits.push((i, price, qty));
                }
                ui.menu_button("Sell to…", |ui| {
                    let mut pay = !st.free;
                    if ui.checkbox(&mut pay, format!("Take {} from their purse", format_gp(price))).changed() {
                        st.free = !pay;
                    }
                    ui.separator();
                    if let Some(l) = treasure::character_menu(ui, doc, &mut st.char_filter) {
                        sell = Some((i, l));
                        ui.close();
                    }
                });
                if ui.small_button("✖").clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    }
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        ui_util::form_menu(ui, "➕ Stock an item…", |ui| {
            if let Some(it) = treasure::item_picker(ui, doc, &mut st.item_filter) {
                picked = Some(it);
                ui.close();
            }
        });
        if !stock.is_empty() && ui.button("📄 Copy price list").clicked() {
            let name = doc.c.location(id).map(|l| l.display_name()).unwrap_or_default();
            let mut md = format!("**{name}**\n\n");
            for e in &stock {
                md.push_str(&format!(
                    "- {} — {}{}\n",
                    e.item.name,
                    format_gp(e.price_gp),
                    if e.qty > 1 { format!(" ({} in stock)", e.qty) } else { String::new() }
                ));
            }
            ui.ctx().copy_text(md);
            sh.toast("Price list copied");
        }
    });
    let Some(shop) = doc.c.location_mut(id).and_then(|l| l.shop.as_mut()) else {
        return;
    };
    let quality = shop.quality;
    for (i, p, q) in edits {
        if let Some(e) = shop.stock.get_mut(i) {
            e.price_gp = p;
            e.qty = q;
        }
    }
    if let Some(mut it) = picked {
        it.id = cm_core::new_id();
        let price = loot::shop_price(it.value_gp().max(0.01) * quality.markup());
        shop.stock.push(StockEntry {
            item: it,
            price_gp: price,
            qty: 1,
            ..Default::default()
        });
    }
    if let Some(i) = remove {
        if i < shop.stock.len() {
            shop.stock.remove(i);
        }
    }
    if let Some((i, to)) = sell {
        let Some(e) = shop.stock.get(i).cloned() else { return };
        let who = doc.c.link_name(&to).unwrap_or_default();
        if !st.free {
            let paid = doc.c.character_core_mut(&to).map(|c| c.coins.pay(e.price_gp)).unwrap_or(false);
            if !paid {
                sh.toast(format!("{who} can't afford {} ({})", e.item.name, format_gp(e.price_gp)));
                return;
            }
        }
        doc.c.give_item(&to, &e.item, 1, true);
        if let Some(shop) = doc.c.location_mut(id).and_then(|l| l.shop.as_mut()) {
            if let Some(x) = shop.stock.get_mut(i) {
                if x.qty <= 1 {
                    shop.stock.remove(i);
                } else {
                    x.qty -= 1;
                }
            }
        }
        sh.toast(if !st.free {
            format!("Sold {} to {who} for {}", e.item.name, format_gp(e.price_gp))
        } else {
            format!("Gave {} to {who}", e.item.name)
        });
    }
}
