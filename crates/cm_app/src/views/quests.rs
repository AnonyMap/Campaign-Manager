//! Quests: story cards flagged as quests, tracked by status with who gave them, objectives
//! and rewards. The quest panel is also shown inside the story card editor.

use cm_core::items::{Coins, format_gp};
use cm_core::{CardStatus, Id, Link, Objective, Quest, QuestStatus, StoryCard};
use egui::RichText;

use crate::app::{Action, Doc, Shared};
use crate::ui_util::{self, section};
use crate::views::treasure::{self, Handout, rarity_color};

/// Editing state for the quest panel (shared by the Quests view and the card editor).
#[derive(Default)]
pub struct QuestUi {
    item_filter: String,
    giver_filter: String,
    new_objective: String,
    handout: Handout,
    handout_for: Option<Id>,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum Show {
    #[default]
    Open,
    All,
    Only(QuestStatus),
}

#[derive(Default)]
pub struct State {
    pub selected: Option<Id>,
    filter: String,
    show: Show,
    giver: Option<Link>,
    pub ui: QuestUi,
}

pub fn status_color(ui: &egui::Ui, s: QuestStatus) -> egui::Color32 {
    let dark = ui.visuals().dark_mode;
    match s {
        QuestStatus::Available => ui.visuals().weak_text_color(),
        QuestStatus::Active => {
            if dark {
                egui::Color32::from_rgb(110, 170, 240)
            } else {
                egui::Color32::from_rgb(30, 90, 190)
            }
        }
        QuestStatus::Completed => {
            if dark {
                egui::Color32::from_rgb(110, 200, 120)
            } else {
                egui::Color32::from_rgb(30, 120, 40)
            }
        }
        QuestStatus::Failed => ui.visuals().error_fg_color,
        QuestStatus::Abandoned => ui.visuals().weak_text_color(),
    }
}

pub fn status_icon(s: QuestStatus) -> &'static str {
    match s {
        QuestStatus::Available => "❔",
        QuestStatus::Active => "❗",
        QuestStatus::Completed => "✔",
        QuestStatus::Failed => "✖",
        QuestStatus::Abandoned => "⊘",
    }
}

/// Who can give quests: NPCs, factions, places and PCs.
fn givers(doc: &Doc) -> Vec<(Link, String)> {
    let mut v: Vec<(Link, String)> = Vec::new();
    let mut npcs: Vec<_> = doc.c.npcs.iter().map(|n| (Link::Npc(n.id), n.core.display_name())).collect();
    npcs.sort_by_key(|x| x.1.to_lowercase());
    v.extend(npcs);
    v.extend(doc.c.factions.iter().map(|f| (Link::Faction(f.id), f.name.clone())));
    let mut places: Vec<_> = doc.c.locations.iter().map(|l| (Link::Location(l.id), l.display_name())).collect();
    places.sort_by_key(|x| x.1.to_lowercase());
    v.extend(places);
    v.extend(doc.c.players.iter().map(|p| (Link::Player(p.id), p.core.display_name())));
    v
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    egui::SidePanel::left("quest_list").resizable(true).default_width(290.0).show_inside(ui, |ui| {
        ui.heading("Quests");
        ui.horizontal(|ui| {
            if ui.button("➕ New quest").clicked() {
                let mut c = StoryCard {
                    title: "New quest".into(),
                    in_ideas_pile: false,
                    ..Default::default()
                };
                c.quest = Some(Quest {
                    status: QuestStatus::Available,
                    ..Default::default()
                });
                st.selected = Some(c.id);
                doc.c.story_cards.push(c);
            }
            ui.menu_button("Make a card a quest…", |ui| {
                let mut chosen = None;
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    let mut any = false;
                    for c in doc.c.story_cards.iter().filter(|c| c.quest.is_none()) {
                        any = true;
                        if ui.button(ui_util::card_title(c)).clicked() {
                            chosen = Some(c.id);
                            ui.close();
                        }
                    }
                    if !any {
                        ui.weak("Every card is a quest already.");
                    }
                });
                if let Some(id) = chosen {
                    if let Some(c) = doc.c.card_mut(id) {
                        c.quest = Some(Quest::default());
                    }
                    st.selected = Some(id);
                }
            });
        });
        ui_util::filter_box(ui, &mut st.filter, "Filter quests…");
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut st.show, Show::Open, "Open");
            ui.selectable_value(&mut st.show, Show::All, "All");
            egui::ComboBox::from_id_salt("quest_status_f")
                .width(100.0)
                .truncate()
                .selected_text(match st.show {
                    Show::Only(s) => s.label(),
                    _ => "Status…",
                })
                .show_ui(ui, |ui| {
                    for s in QuestStatus::ALL {
                        ui.selectable_value(&mut st.show, Show::Only(s), format!("{} {}", status_icon(s), s.label()));
                    }
                });
        });
        let giver_list = givers(doc);
        let used: Vec<Link> = doc.c.quests().iter().filter_map(|c| c.quest.as_ref().and_then(|q| q.giver.clone())).collect();
        if !used.is_empty() {
            egui::ComboBox::from_id_salt("quest_giver_f")
                .width(200.0)
                .truncate()
                .selected_text(
                    st.giver
                        .as_ref()
                        .and_then(|g| doc.c.link_name(g))
                        .map(|n| format!("From {n}"))
                        .unwrap_or("From anyone".into()),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut st.giver, None, "From anyone");
                    for (l, n) in giver_list.iter().filter(|(l, _)| used.contains(l)) {
                        ui.selectable_value(&mut st.giver, Some(l.clone()), format!("{n} ({})", l.kind_label()));
                    }
                });
        }
        ui.separator();
        let f = st.filter.to_lowercase();
        let mut list: Vec<&StoryCard> = doc
            .c
            .quests()
            .into_iter()
            .filter(|c| {
                let q = c.quest.as_ref().unwrap();
                let s_ok = match st.show {
                    Show::Open => q.status.open(),
                    Show::All => true,
                    Show::Only(s) => q.status == s,
                };
                s_ok && st.giver.as_ref().map(|g| q.giver.as_ref() == Some(g)).unwrap_or(true)
            })
            .filter(|c| f.is_empty() || c.title.to_lowercase().contains(&f) || c.body.to_lowercase().contains(&f))
            .collect();
        list.sort_by_key(|c| (c.quest.as_ref().unwrap().status as u8, c.display_title().to_lowercase()));
        egui::ScrollArea::vertical().id_salt("quest_list_scroll").show(ui, |ui| {
            if list.is_empty() {
                ui.weak(if doc.c.quests().is_empty() {
                    "No quests yet. Create one, or tick “This is a quest” on any story card."
                } else {
                    "No quests match."
                });
            }
            let mut last = None;
            for c in list {
                let q = c.quest.as_ref().unwrap();
                if last != Some(q.status) {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(q.status.label().to_uppercase())
                            .small()
                            .strong()
                            .color(status_color(ui, q.status)),
                    );
                    last = Some(q.status);
                }
                let (done, total) = q.progress();
                let mut text = format!("{} {}", status_icon(q.status), ui_util::card_title(c));
                if total > 0 {
                    text.push_str(&format!("  ({done}/{total})"));
                }
                let r = ui_util::list_row(ui, st.selected == Some(c.id), RichText::new(text).color(status_color(ui, q.status)));
                if r.clicked() {
                    st.selected = Some(c.id);
                }
                let giver = q.giver.as_ref().and_then(|g| doc.c.link_name(g));
                let reward = q.reward_summary();
                r.on_hover_ui(|ui| {
                    if let Some(g) = &giver {
                        ui.label(format!("From {g}"));
                    }
                    if !reward.is_empty() {
                        ui.label(format!("Reward: {reward}"));
                    }
                });
            }
        });
    });
    let Some(id) = st.selected else {
        ui.centered_and_justified(|ui| ui.weak("Select a quest, or create one."));
        return;
    };
    if doc.c.card(id).and_then(|c| c.quest.as_ref()).is_none() {
        st.selected = None;
        return;
    }
    ui_util::pinned_scroll(ui, ("quest_edit", id), |ui| {
        let card = doc.c.card_mut(id).unwrap();
        ui.horizontal(|ui| {
            ui.weak("Quest");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui_util::confirm_delete_button(ui, Link::Card(id), sh);
                if ui
                    .button("🃏 Open as story card")
                    .on_hover_text("Timing, threads, map pins, session plans and links")
                    .clicked()
                {
                    sh.actions.push(Action::Open(Link::Card(id)));
                }
            });
        });
        ui.add(
            egui::TextEdit::singleline(&mut card.title)
                .hint_text("Quest name")
                .font(egui::TextStyle::Heading)
                .desired_width(f32::INFINITY),
        );
        ui_util::markdown_editor(ui, ("questbody", id), &mut card.body, &mut sh.md, 6);
        quest_panel(ui, doc, sh, id, &mut st.ui);
        section(ui, "Links");
        let dir = doc.dir.clone();
        let mut links = doc.c.card_mut(id).map(|c| std::mem::take(&mut c.links)).unwrap_or_default();
        ui_util::link_chips(ui, &doc.c, &mut links, sh);
        let vault = sh.vault.take();
        ui_util::link_picker(ui, ("questlinks", id), &doc.c, &mut links, Some(&Link::Card(id)), &dir, vault.as_ref(), sh);
        sh.vault = vault;
        if let Some(c) = doc.c.card_mut(id) {
            c.links = links;
        }
        ui_util::backlinks(ui, &doc.c, &Link::Card(id), sh);
    });
}

/// Status, giver, objectives and rewards of the quest on card `id`.
pub fn quest_panel(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, id: Id, qu: &mut QuestUi) {
    if treasure::handout_window(ui.ctx(), doc, sh, &mut qu.handout) {
        if let Some(q) = qu.handout_for.take().and_then(|qid| doc.c.card_mut(qid)).and_then(|c| c.quest.as_mut()) {
            q.rewarded = true;
        }
    }
    let upcoming = doc.c.upcoming_session().map(|s| s.number);
    let giver_list = givers(doc);
    let Some(card) = doc.c.card_mut(id) else { return };
    let title = card.display_title();
    let Some(q) = card.quest.as_mut() else { return };
    let mut status_changed = None;

    egui::Grid::new(("quest_grid", id)).num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Status");
        ui.horizontal_wrapped(|ui| {
            for s in QuestStatus::ALL {
                let r = ui.selectable_label(
                    q.status == s,
                    RichText::new(format!("{} {}", status_icon(s), s.label())).color(status_color(ui, s)),
                );
                if r.clicked() && q.status != s {
                    status_changed = Some(s);
                }
            }
        });
        ui.end_row();

        ui.label("Given by");
        ui.horizontal_wrapped(|ui| {
            if let Some(g) = q.giver.clone() {
                let name = giver_list.iter().find(|(l, _)| *l == g).map(|(_, n)| n.clone()).unwrap_or("(deleted)".into());
                if ui.link(format!("{name} ({})", g.kind_label())).clicked() {
                    sh.actions.push(Action::Open(g));
                }
                if ui.small_button("✖").on_hover_text("Clear").clicked() {
                    q.giver = None;
                }
            }
            ui_util::form_menu(ui, if q.giver.is_some() { "Change…" } else { "Choose…" }, |ui| {
                ui.set_min_width(240.0);
                ui.add(
                    egui::TextEdit::singleline(&mut qu.giver_filter)
                        .hint_text("Find an NPC, faction or place…")
                        .desired_width(f32::INFINITY),
                );
                let f = qu.giver_filter.to_lowercase();
                egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                    let mut last = "";
                    for (l, n) in &giver_list {
                        if !f.is_empty() && !n.to_lowercase().contains(&f) {
                            continue;
                        }
                        if l.kind_label() != last {
                            last = l.kind_label();
                            ui.label(RichText::new(last).small().strong());
                        }
                        if ui.button(n).clicked() {
                            q.giver = Some(l.clone());
                            ui.close();
                        }
                    }
                    if giver_list.is_empty() {
                        ui.weak("Add NPCs, factions or places first.");
                    }
                });
            });
        });
        ui.end_row();

        ui.label("Sessions");
        ui.horizontal(|ui| {
            ui.weak("taken");
            ui_util::opt_u32(ui, ("q_start", id), &mut q.started_session, 44.0);
            ui.weak("ended");
            ui_util::opt_u32(ui, ("q_end", id), &mut q.ended_session, 44.0);
        });
        ui.end_row();
    });

    section(ui, "Objectives");
    let mut remove = None;
    let mut up = None;
    let row_h = ui.spacing().interact_size.y;
    for (i, o) in q.objectives.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.checkbox(&mut o.done, "");
            // buttons first (right to left) so the text box takes exactly the rest
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), row_h),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui.small_button("✖").clicked() {
                        remove = Some(i);
                    }
                    if i > 0 {
                        if ui.small_button("⏶").on_hover_text("Move up").clicked() {
                            up = Some(i);
                        }
                    } else {
                        ui.add_space(ui.spacing().interact_size.y + ui.spacing().item_spacing.x);
                    }
                    let mut te = egui::TextEdit::singleline(&mut o.text)
                        .id(egui::Id::new(("q_obj", id, i)))
                        .desired_width(f32::INFINITY);
                    if o.done {
                        te = te.text_color(ui.visuals().weak_text_color());
                    }
                    ui.add(te);
                },
            );
        });
    }
    if let Some(i) = remove {
        q.objectives.remove(i);
    }
    if let Some(i) = up {
        q.objectives.swap(i, i - 1);
    }
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), row_h),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            let add = ui.button("Add").clicked();
            let r = ui.add(
                egui::TextEdit::singleline(&mut qu.new_objective)
                    .id(egui::Id::new(("q_new_obj", id)))
                    .hint_text("Add an objective and press Enter")
                    .desired_width(f32::INFINITY),
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (add || enter) && !qu.new_objective.trim().is_empty() {
                q.objectives.push(Objective {
                    text: qu.new_objective.trim().to_string(),
                    done: false,
                });
                qu.new_objective.clear();
                if enter {
                    r.request_focus();
                }
            }
        },
    );
    let (done, total) = q.progress();
    if total > 0 {
        ui.add(
            egui::ProgressBar::new(done as f32 / total as f32)
                .desired_width(240.0)
                .text(format!("{done} of {total} done")),
        );
        if done == total && q.status == QuestStatus::Active {
            if ui.button("✔ All done — mark the quest completed").clicked() {
                status_changed = Some(QuestStatus::Completed);
            }
        }
    }

    section(ui, "Rewards");
    ui.horizontal_wrapped(|ui| {
        ui.label("Coins");
        for (label, v) in [
            ("pp", &mut q.reward_coins.pp),
            ("gp", &mut q.reward_coins.gp),
            ("sp", &mut q.reward_coins.sp),
            ("cp", &mut q.reward_coins.cp),
        ] {
            ui.add(egui::DragValue::new(v).range(0..=100_000_000).suffix(format!(" {label}")));
        }
        ui.separator();
        ui.label("XP");
        ui_util::opt_u32(ui, ("q_xp", id), &mut q.xp, 60.0);
    });
    let mut remove_item = None;
    for (i, it) in q.reward_items.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&it.name).color(rarity_color(ui, it.rarity)))
                .on_hover_text(it.subtitle());
            ui.weak(format_gp(it.value_gp()));
            if ui.small_button("✖").clicked() {
                remove_item = Some(i);
            }
        });
    }
    if let Some(i) = remove_item {
        q.reward_items.remove(i);
    }
    let mut picked = None;
    // The picker needs the whole campaign, so it opens in a window after the quest borrow ends.
    if ui.button("➕ Reward item…").clicked() {
        picked = Some(true);
    }
    ui.add(
        egui::TextEdit::multiline(&mut q.reward_text)
            .id(egui::Id::new(("q_reward_text", id)))
            .hint_text("Other rewards: favours, land, a title…")
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
    let total_gp = q.reward_coins.total_gp() + q.reward_items.iter().map(|i| i.value_gp()).sum::<f64>();
    let has_reward = !q.reward_coins.is_zero() || !q.reward_items.is_empty();
    let rewarded = q.rewarded;
    let coins: Coins = q.reward_coins;
    let items: Vec<(cm_core::items::Item, u32)> = q.reward_items.iter().map(|i| (i.clone(), 1)).collect();
    if let Some(s) = status_changed {
        q.status = s;
        match s {
            QuestStatus::Active => {
                if q.started_session.is_none() {
                    q.started_session = upcoming;
                }
                q.ended_session = None;
            }
            QuestStatus::Completed | QuestStatus::Failed | QuestStatus::Abandoned => {
                if q.ended_session.is_none() {
                    q.ended_session = upcoming;
                }
            }
            QuestStatus::Available => {}
        }
        card.status = match s {
            QuestStatus::Available => CardStatus::Planned,
            QuestStatus::Active => CardStatus::InPlay,
            QuestStatus::Completed | QuestStatus::Failed => CardStatus::Done,
            QuestStatus::Abandoned => CardStatus::Discarded,
        };
        card.modified = chrono::Utc::now();
    }
    let qstatus = card.quest.as_ref().map(|q| q.status).unwrap_or_default();
    ui.horizontal_wrapped(|ui| {
        if total_gp > 0.0 {
            ui.weak(format!("Worth about {}", format_gp(total_gp)));
        }
        if has_reward {
            let label = if rewarded { "🎁 Hand out again…" } else { "🎁 Hand out rewards…" };
            let r = ui.add_enabled(!qu.handout.open, egui::Button::new(label));
            if r.clicked() {
                qu.handout.start(doc, &title, coins, items);
                qu.handout_for = Some(id);
            }
            if rewarded {
                ui.label(RichText::new("✔ Rewards handed out").weak());
            } else if qstatus == QuestStatus::Completed {
                ui.label(RichText::new("Rewards not handed out yet").color(ui.visuals().warn_fg_color));
            }
        }
    });
    // reward item picker (separate popup so it can borrow the campaign)
    let pick_id = ui.make_persistent_id(("q_pick_item", id));
    if picked == Some(true) {
        ui.data_mut(|d| d.insert_temp(pick_id, true));
    }
    let mut open = ui.data(|d| d.get_temp::<bool>(pick_id)).unwrap_or(false);
    if open {
        let mut chosen = None;
        egui::Window::new("Add a reward item")
            .id(pick_id.with("win"))
            .open(&mut open)
            .collapsible(false)
            .default_width(360.0)
            .show(ui.ctx(), |ui| {
                chosen = treasure::item_picker(ui, doc, &mut qu.item_filter);
            });
        if let Some(mut it) = chosen {
            it.id = cm_core::new_id();
            if let Some(q) = doc.c.card_mut(id).and_then(|c| c.quest.as_mut()) {
                q.reward_items.push(it);
            }
            open = false;
        }
        ui.data_mut(|d| d.insert_temp(pick_id, open));
    }
}

/// One-line summary of a quest for lists elsewhere (Run session, places, NPCs).
pub fn quest_line(c: &StoryCard) -> String {
    let Some(q) = &c.quest else { return c.display_title() };
    let (d, t) = q.progress();
    let prog = if t > 0 { format!(" ({d}/{t})") } else { String::new() };
    format!("{} {}{prog}", status_icon(q.status), c.display_title())
}

/// Quests a character, faction or place has given.
pub fn quests_from(doc: &Doc, giver: &Link) -> Vec<(Id, String, QuestStatus)> {
    doc.c
        .quests()
        .iter()
        .filter(|c| c.quest.as_ref().and_then(|q| q.giver.as_ref()) == Some(giver))
        .map(|c| (c.id, quest_line(c), c.quest.as_ref().unwrap().status))
        .collect()
}

/// "Quests given" list with links, for NPC / faction / place pages.
pub fn quests_given(ui: &mut egui::Ui, doc: &Doc, giver: &Link, sh: &mut Shared) {
    let qs = quests_from(doc, giver);
    if qs.is_empty() {
        return;
    }
    section(ui, "Quests given");
    for (id, line, s) in qs {
        if ui.link(RichText::new(line).color(status_color(ui, s))).clicked() {
            sh.actions.push(Action::Open(Link::Card(id)));
        }
    }
}
