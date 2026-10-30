//! Run session: the screen to keep open at the table. Today's plan, the scenes in running
//! order with a check-off, encounters ready to launch, secrets to reveal, the people
//! involved, and live notes that become the recap when the session ends.

use cm_core::encounter::estimate_difficulty;
use cm_core::{CardStatus, CombatantSource, Id, Link, SessionStatus, StoryCard};
use egui::RichText;

use crate::app::{Action, Doc, Shared, View};
use crate::ui_util::{self, section};

#[derive(Default)]
pub struct State {
    pub session: Option<Id>,
    edit_plan: bool,
    idea: String,
    confirm_end: bool,
    carry_over: bool,
    expanded: std::collections::HashSet<Id>,
}

fn done_color(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(110, 200, 120)
    } else {
        egui::Color32::from_rgb(20, 120, 40)
    }
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    if st.session.and_then(|id| doc.c.session(id)).is_none() {
        st.session = doc.c.upcoming_session().map(|s| s.id);
        st.carry_over = true;
    }
    let Some(sid) = st.session else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.heading("Run session");
            ui.weak("Plan a session first: its cards, encounters and notes show up here on game day.");
            if ui.button("➕ Plan a session").clicked() {
                let n = doc.c.next_session_number();
                let s = cm_core::Session {
                    number: n,
                    real_date: doc.c.settings.session_schedule.date_for_session(n),
                    ..Default::default()
                };
                st.session = Some(s.id);
                doc.c.sessions.push(s);
            }
        });
        return;
    };
    header(ui, doc, sh, st, sid);
    ui.separator();
    egui::SidePanel::right("run_side")
        .resizable(true)
        .default_width(340.0)
        .width_range(240.0..=640.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("run_side_scroll")
                .show(ui, |ui| side(ui, doc, sh, st, sid));
        });
    egui::CentralPanel::default().frame(egui::Frame::new()).show_inside(ui, |ui| {
        // A vertical scroll area grows to fit last frame's content width; wrapped rows fill
        // whatever width they get, so pin the width or the column never shrinks back.
        let w = ui.available_width() - 14.0;
        egui::ScrollArea::vertical().id_salt("run_main").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(w);
            main(ui, doc, sh, st, sid)
        });
    });
}

fn header(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, sid: Id) {
    let mut sessions: Vec<(Id, u32, String, SessionStatus)> = doc.c.sessions.iter().map(|s| (s.id, s.number, s.display_title(), s.status)).collect();
    sessions.sort_by_key(|s| s.1);
    let s = doc.c.session(sid).unwrap();
    let title = s.display_title();
    let status = s.status;
    let date = s.real_date.or_else(|| doc.c.settings.session_schedule.date_for_session(s.number));
    let game_date = s.game_date.map(|d| doc.c.settings.game_calendar.format(&d));
    ui.horizontal(|ui| {
        ui.label(RichText::new("▶").size(20.0).color(ui.visuals().hyperlink_color));
        egui::ComboBox::from_id_salt("run_session")
            .selected_text(RichText::new(&title).heading())
            .width(320.0)
            .show_ui(ui, |ui| {
                for (id, _, t, stt) in &sessions {
                    let label = if *stt == SessionStatus::Planned {
                        t.clone()
                    } else {
                        format!("{t}  ({})", stt.label())
                    };
                    if ui.selectable_label(*id == sid, label).clicked() {
                        st.session = Some(*id);
                        st.confirm_end = false;
                    }
                }
            });
        if let Some(d) = date {
            ui.label(ui_util::fmt_date(ui, d));
        }
        if let Some(g) = game_date {
            ui.weak(format!("· in game: {g}"));
        }
        let chip = match status {
            SessionStatus::Planned => RichText::new("Planned"),
            SessionStatus::Played => RichText::new("✔ Played").color(done_color(ui)),
            SessionStatus::Cancelled => RichText::new("Cancelled").weak(),
        };
        ui.label(chip);
    });
    ui.horizontal_wrapped(|ui| {
        if ui.button("⚔ Combat tracker").clicked() {
            sh.actions.push(Action::GoView(View::Combat));
        }
        if ui.button("Open in planner").clicked() {
            sh.actions.push(Action::Open(Link::Session(sid)));
        }
        if status == SessionStatus::Planned {
            let end = ui_util::form_menu(ui, RichText::new("■ End session…").strong().text().to_string(), |ui| {
                ui.set_min_width(300.0);
                ui.label("Mark this session as played and write a recap draft from your live notes, the scenes you checked off and the secrets you revealed.");
                let unplayed = doc
                    .c
                    .cards_for_session(doc.c.session(sid).unwrap())
                    .iter()
                    .filter(|c| !matches!(c.status, CardStatus::Done | CardStatus::Discarded) && !c.secret)
                    .count();
                if unplayed > 0 {
                    ui.checkbox(
                        &mut st.carry_over,
                        format!("Move {} to the next session", ui_util::plural(unplayed, "unplayed card")),
                    );
                }
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("End session").strong()).clicked() {
                        if let Some(r) = doc.c.end_session(sid, st.carry_over) {
                            let mut msg = "Session marked as played — recap drafted".to_string();
                            if let Some(n) = r.moved_to {
                                msg.push_str(&format!("; {} moved to session {n}", ui_util::plural(r.moved_cards, "card")));
                            }
                            sh.toast(msg);
                            sh.actions.push(Action::Open(Link::Session(sid)));
                        }
                        ui.close();
                    }
                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                });
            });
            end.on_hover_text("Wrap up: recap draft, mark played, carry over unplayed cards");
        }
    });
}

fn main(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, sid: Id) {
    let number = doc.c.session(sid).unwrap().number;
    // ---- plan -------------------------------------------------------------
    ui.horizontal(|ui| {
        ui.label(RichText::new("Plan").strong().size(16.0));
        if ui.small_button(if st.edit_plan { "✔ Done" } else { "✏ Edit" }).clicked() {
            st.edit_plan = !st.edit_plan;
        }
    });
    {
        let s = doc.c.session_mut(sid).unwrap();
        if st.edit_plan {
            ui_util::markdown_editor(ui, ("run_plan", sid), &mut s.narrative, &mut sh.md, 6);
        } else if s.narrative.trim().is_empty() {
            ui.weak("No plan written yet. Click Edit to jot down the outline for today.");
        } else {
            let text = s.narrative.clone();
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui_util::render_markdown(ui, &mut sh.md, &text);
            });
        }
    }

    // ---- scenes -----------------------------------------------------------
    let cards: Vec<StoryCard> = doc.c.cards_for_session(doc.c.session(sid).unwrap()).into_iter().cloned().collect();
    let done = cards.iter().filter(|c| c.status == CardStatus::Done).count();
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Scenes").strong().size(16.0));
        if !cards.is_empty() {
            ui.weak(format!("{done} of {} done", cards.len()));
            ui.add(
                egui::ProgressBar::new(done as f32 / cards.len() as f32)
                    .desired_width(120.0)
                    .desired_height(6.0),
            );
        }
        if ui.small_button("➕ Card for this session").clicked() {
            sh.actions.push(Action::NewCardInSession(sid));
        }
    });
    if cards.is_empty() {
        ui.weak("No story cards planned. Add some in the session planner, or with the button above.");
    }
    let first_open = cards
        .iter()
        .find(|c| !matches!(c.status, CardStatus::Done | CardStatus::Discarded))
        .map(|c| c.id);
    let mut set_status: Option<(Id, CardStatus)> = None;
    let mut reveal: Option<(Id, bool)> = None;
    for card in &cards {
        let is_now = card.status == CardStatus::InPlay;
        let is_done = card.status == CardStatus::Done;
        let fill = if is_now {
            ui.visuals().selection.bg_fill.gamma_multiply(0.3)
        } else {
            ui.visuals().faint_bg_color
        };
        egui::Frame::group(ui.style()).fill(fill).inner_margin(6).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let mut checked = is_done;
                if ui.checkbox(&mut checked, "").on_hover_text("Played").changed() {
                    set_status = Some((card.id, if checked { CardStatus::Done } else { CardStatus::Planned }));
                }
                let mut t = RichText::new(card.display_title()).strong();
                if is_done {
                    t = t.strikethrough().weak();
                }
                let expanded = st.expanded.contains(&card.id) || (Some(card.id) == first_open && !st.expanded.contains(&Id::nil()));
                let arrow = if expanded { "⏷" } else { "⏵" };
                if ui.add(egui::Button::new(RichText::new(arrow).small()).frame(false)).clicked() {
                    // first click on the auto-opened card collapses it for good
                    if Some(card.id) == first_open {
                        st.expanded.insert(Id::nil());
                    }
                    if expanded {
                        st.expanded.remove(&card.id);
                    } else {
                        st.expanded.insert(card.id);
                    }
                }
                if ui.link(t).on_hover_text("Open card").clicked() {
                    sh.actions.push(Action::Open(Link::Card(card.id)));
                }
                if card.secret {
                    secret_badge(ui, card);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !is_done {
                        if is_now {
                            ui.label(RichText::new("▶ in play").color(ui.visuals().hyperlink_color));
                        } else if ui.small_button("▶ Now").on_hover_text("Mark as the scene in play").clicked() {
                            set_status = Some((card.id, CardStatus::InPlay));
                        }
                    }
                    if card.secret {
                        reveal_button(ui, card, &mut reveal);
                    }
                });
            });
            let expanded = st.expanded.contains(&card.id) || (Some(card.id) == first_open && !st.expanded.contains(&Id::nil()));
            if expanded {
                if !card.body.trim().is_empty() {
                    ui_util::render_markdown(ui, &mut sh.md, &card.body);
                }
                link_row(ui, doc, sh, &card.links);
            }
        });
    }
    if let Some((id, status)) = set_status {
        // only one scene "in play" at a time
        if status == CardStatus::InPlay {
            for c in &cards {
                if c.status == CardStatus::InPlay {
                    if let Some(x) = doc.c.card_mut(c.id) {
                        x.status = CardStatus::Planned;
                    }
                }
            }
        }
        if let Some(c) = doc.c.card_mut(id) {
            c.status = status;
            c.modified = chrono::Utc::now();
        }
    }

    // ---- encounters -------------------------------------------------------
    let s = doc.c.session(sid).unwrap().clone();
    if !s.encounter_ids.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new("Encounters").strong().size(16.0));
        let levels: Vec<u32> = doc.c.players.iter().filter(|p| p.active).filter_map(|p| p.core.level).collect();
        for eid in &s.encounter_ids {
            let Some(e) = doc.c.encounter(*eid) else {
                continue;
            };
            let est = estimate_difficulty(e, &doc.c, &levels);
            let n: u32 = e
                .entries
                .iter()
                .filter(|x| !matches!(x.source, CombatantSource::Player(_)))
                .map(|x| x.count.max(1))
                .sum();
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(RichText::new("⚔ Run").strong())
                    .on_hover_text("Load into the combat tracker")
                    .clicked()
                {
                    sh.actions.push(Action::StartEncounter(*eid));
                }
                if ui.link(&e.name).clicked() {
                    sh.actions.push(Action::Open(Link::Encounter(*eid)));
                }
                let loc = if e.location.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", e.location)
                };
                ui.weak(format!("{} · {}{loc}", ui_util::plural(n as usize, "creature"), est.rating));
            });
        }
    }

    // ---- secrets ----------------------------------------------------------
    let mine = doc.c.session_secrets(&s);
    let others: Vec<Id> = doc
        .c
        .story_cards
        .iter()
        .filter(|c| c.is_unrevealed_secret() && !mine.contains(&c.id))
        .map(|c| c.id)
        .collect();
    if !mine.is_empty() || !others.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new("🔒 Secrets & clues").strong().size(16.0));
        if mine.is_empty() {
            ui.weak("None tied to this session's cards.");
        }
        for id in &mine {
            if let Some(c) = doc.c.card(*id).cloned() {
                secret_row(ui, &c, sh, &mut reveal);
            }
        }
        if !others.is_empty() {
            egui::CollapsingHeader::new(format!("Other unrevealed secrets ({})", others.len()))
                .id_salt("run_other_secrets")
                .show(ui, |ui| {
                    for id in &others {
                        if let Some(c) = doc.c.card(*id).cloned() {
                            secret_row(ui, &c, sh, &mut reveal);
                        }
                    }
                });
        }
    }
    if let Some((id, yes)) = reveal {
        if let Some(c) = doc.c.card_mut(id) {
            if yes {
                c.reveal(Some(number));
                sh.toast(format!("Revealed “{}” in session {number}", c.display_title()));
            } else {
                c.revealed = None;
            }
        }
    }
}

fn secret_badge(ui: &mut egui::Ui, card: &StoryCard) {
    match &card.revealed {
        None => {
            ui.label(RichText::new("🔒 secret").small().color(ui.visuals().warn_fg_color))
                .on_hover_text("The players don't know this yet");
        }
        Some(r) => {
            let when = r.session.map(|n| format!("in session {n}")).unwrap_or_else(|| "earlier".into());
            ui.label(RichText::new("🔓 revealed").small().weak()).on_hover_text(format!("Revealed {when}"));
        }
    }
}

fn reveal_button(ui: &mut egui::Ui, card: &StoryCard, reveal: &mut Option<(Id, bool)>) {
    if card.revealed.is_none() {
        if ui.small_button("🔓 Reveal").on_hover_text("The players learned this now").clicked() {
            *reveal = Some((card.id, true));
        }
    } else if ui.small_button("↺").on_hover_text("Undo reveal (still secret)").clicked() {
        *reveal = Some((card.id, false));
    }
}

fn secret_row(ui: &mut egui::Ui, c: &StoryCard, sh: &mut Shared, reveal: &mut Option<(Id, bool)>) {
    ui.horizontal_wrapped(|ui| {
        reveal_button(ui, c, reveal);
        let mut t = RichText::new(c.display_title());
        if c.revealed.is_some() {
            t = t.weak();
        }
        let r = ui.link(t);
        if r.clicked() {
            sh.actions.push(Action::Open(Link::Card(c.id)));
        }
        if !c.body.trim().is_empty() {
            let body = c.body.clone();
            r.on_hover_ui(|ui| {
                ui.set_max_width(360.0);
                ui.label(body);
            });
        }
        secret_badge(ui, c);
    });
}

/// Links on a card as compact chips: people open their record, encounters can be run.
fn link_row(ui: &mut egui::Ui, doc: &Doc, sh: &mut Shared, links: &[Link]) {
    if links.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for l in links {
            let Some(name) = doc.c.link_name(l) else {
                continue;
            };
            let name = match l {
                Link::Card(id) => doc.c.card(*id).map(ui_util::card_title).unwrap_or(name),
                _ => name,
            };
            match l {
                Link::Encounter(id) => {
                    if ui.small_button(format!("⚔ {name}")).on_hover_text("Run this encounter").clicked() {
                        sh.actions.push(Action::StartEncounter(*id));
                    }
                }
                Link::Image(p) => {
                    if ui.small_button(format!("🖼 {name}")).clicked() {
                        sh.actions.push(Action::ViewImage(p.clone()));
                    }
                }
                other => {
                    if ui.small_button(format!("{} {name}", link_icon(other))).clicked() {
                        sh.actions.push(Action::Open(other.clone()));
                    }
                }
            }
        }
    });
}

fn link_icon(l: &Link) -> &'static str {
    match l {
        Link::Npc(_) => "👤",
        Link::Player(_) => "🛡",
        Link::Faction(_) => "🏰",
        Link::Card(_) => "🃏",
        Link::Map(_) => "🗺",
        Link::Wiki(_) => "📖",
        Link::Url { .. } => "🔗",
        Link::StatBlock(_) => "🐉",
        Link::Session(_) => "📅",
        Link::Thread(_) => "📜",
        Link::Location(_) => "📍",
        Link::Item(_) => "💎",
        _ => "•",
    }
}

fn side(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, sid: Id) {
    let number = doc.c.session(sid).unwrap().number;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Live notes").strong().size(16.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("🕑 time").on_hover_text("Insert the current time").clicked() {
                let s = doc.c.session_mut(sid).unwrap();
                let t = chrono::Local::now().format("%H:%M").to_string();
                if !s.live_notes.is_empty() && !s.live_notes.ends_with('\n') {
                    s.live_notes.push('\n');
                }
                s.live_notes.push_str(&format!("**{t}** "));
            }
        });
    });
    {
        let s = doc.c.session_mut(sid).unwrap();
        ui.add(
            egui::TextEdit::multiline(&mut s.live_notes)
                .id(egui::Id::new(("live_notes", sid)))
                .desired_rows(12)
                .desired_width(f32::INFINITY)
                .hint_text("What happened, names you made up, loot handed out… Becomes the recap when you end the session."),
        );
    }
    ui.add_space(6.0);
    ui.label(RichText::new("Quick idea").strong());
    ui.horizontal(|ui| {
        let r = ui.add_sized(
            [ui.available_width() - 60.0, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut st.idea).hint_text("A thread the players invented…"),
        );
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (ui.button("➕").on_hover_text("Add to the ideas pile").clicked() || enter) && !st.idea.trim().is_empty() {
            let card = StoryCard {
                body: st.idea.trim().to_string(),
                tags: vec![format!("session {number}")],
                ..Default::default()
            };
            doc.c.story_cards.push(card);
            st.idea.clear();
            sh.toast("Added to the ideas pile");
        }
    });

    // people
    let s = doc.c.session(sid).unwrap().clone();
    let people = doc.c.session_people(&s);
    section(ui, "People in this session");
    if people.is_empty() {
        ui.weak("Link NPCs to the session's cards or encounters and they show up here.");
    }
    for l in people {
        let Some(name) = doc.c.link_name(&l) else {
            continue;
        };
        ui.horizontal_wrapped(|ui| {
            let detail = match &l {
                Link::Npc(id) => doc.c.npc(*id).map(|n| {
                    let sb: String = n
                        .statblock_ids()
                        .iter()
                        .filter_map(|s| doc.c.statblock(*s))
                        .map(|s| format!(" · {}", s.name()))
                        .collect();
                    format!("{}{sb}", n.role)
                }),
                Link::Player(id) => doc.c.player(*id).map(|p| p.player_name.clone()),
                Link::Faction(_) => Some("faction".into()),
                _ => None,
            }
            .unwrap_or_default();
            if ui.link(format!("{} {name}", link_icon(&l))).clicked() {
                sh.actions.push(Action::Open(l.clone()));
            }
            if !detail.trim().is_empty() {
                ui.label(RichText::new(detail.trim_start_matches(" · ")).weak().small());
            }
        });
    }
    active_quests(ui, doc, sh);
    // party at a glance
    let party: Vec<_> = doc.c.players.iter().filter(|p| p.active).collect();
    if !party.is_empty() {
        section(ui, "Party");
        egui::Grid::new("run_party").num_columns(4).spacing([10.0, 2.0]).striped(true).show(ui, |ui| {
            ui.label(RichText::new("PC").small().strong());
            ui.label(RichText::new("AC").small().strong());
            ui.label(RichText::new("HP").small().strong());
            ui.label(RichText::new("PP").small().strong()).on_hover_text("Passive Perception");
            ui.end_row();
            for p in party {
                if ui.link(p.core.display_name()).clicked() {
                    sh.actions.push(Action::Open(Link::Player(p.id)));
                }
                ui.label(p.core.ac.map(|v| v.to_string()).unwrap_or("—".into()));
                ui.label(p.core.max_hp.map(|v| v.to_string()).unwrap_or("—".into()));
                ui.label(p.core.passive_perception.map(|v| v.to_string()).unwrap_or("—".into()));
                ui.end_row();
            }
        });
    }
}

/// Quests the party is on, with objectives to tick off during play.
fn active_quests(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared) {
    let ids: Vec<Id> = doc
        .c
        .quests()
        .iter()
        .filter(|c| c.quest.as_ref().map(|q| q.status == cm_core::QuestStatus::Active).unwrap_or(false))
        .map(|c| c.id)
        .collect();
    if ids.is_empty() {
        return;
    }
    section(ui, "Active quests");
    for id in ids {
        let giver = doc
            .c
            .card(id)
            .and_then(|c| c.quest.as_ref())
            .and_then(|q| q.giver.clone())
            .and_then(|g| doc.c.link_name(&g));
        let Some(card) = doc.c.card_mut(id) else { continue };
        let title = crate::views::quests::quest_line(card);
        let Some(q) = card.quest.as_mut() else { continue };
        let r = ui.link(RichText::new(title).strong());
        if r.clicked() {
            sh.actions.push(Action::GoQuest(id));
        }
        if let Some(g) = giver {
            r.on_hover_text(format!("From {g}"));
        }
        ui.indent(("rq", id), |ui| {
            for (i, o) in q.objectives.iter_mut().enumerate() {
                let text = if o.done {
                    RichText::new(&o.text).weak().strikethrough()
                } else {
                    RichText::new(&o.text)
                };
                ui.push_id(i, |ui| ui.checkbox(&mut o.done, text));
            }
            let reward = q.reward_summary();
            if !reward.is_empty() {
                ui.label(RichText::new(format!("Reward: {reward}")).weak().small());
            }
        });
    }
}
