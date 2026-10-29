//! Story cards: free-form plot ideas. Nothing is required — a card can be just a line of
//! markdown — but cards can also carry timing, threads, map pins, sessions and links.

use cm_core::calendar::GameDate;
use cm_core::{CardStatus, CardTiming, Id, Link, StoryCard};
use egui::RichText;

use crate::app::{Action, Doc, Shared};
use crate::ui_util::{self, section};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Modified,
    Title,
    Status,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum SecretFilter {
    #[default]
    Any,
    Secrets,
    Unrevealed,
    NotSecret,
}

impl SecretFilter {
    fn label(self) -> &'static str {
        match self {
            SecretFilter::Any => "Secret or not",
            SecretFilter::Secrets => "🔒 Secrets",
            SecretFilter::Unrevealed => "🔒 Not yet revealed",
            SecretFilter::NotSecret => "Not secret",
        }
    }
    fn keep(self, c: &StoryCard) -> bool {
        match self {
            SecretFilter::Any => true,
            SecretFilter::Secrets => c.secret,
            SecretFilter::Unrevealed => c.is_unrevealed_secret(),
            SecretFilter::NotSecret => !c.secret,
        }
    }
}

#[derive(Default)]
pub struct State {
    pub selected: Option<Id>,
    filter: String,
    status: Option<CardStatus>,
    thread: Option<Id>,
    ideas_only: bool,
    sort: Sort,
    secret: SecretFilter,
    pub quest_ui: crate::views::quests::QuestUi,
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    egui::SidePanel::left("cards_list").resizable(true).default_width(270.0).show_inside(ui, |ui| {
        ui.heading("Story cards");
        ui.horizontal(|ui| {
            if ui.button("➕ New card").clicked() {
                let c = StoryCard::default();
                st.selected = Some(c.id);
                doc.c.story_cards.push(c);
            }
            ui.checkbox(&mut st.ideas_only, "Ideas pile only");
        });
        ui_util::filter_box(ui, &mut st.filter, "Filter cards…");
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("card_status_f")
                .width(100.0)
                .selected_text(st.status.map(|s| s.label()).unwrap_or("Any status"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut st.status, None, "Any status");
                    for s in CardStatus::ALL {
                        ui.selectable_value(&mut st.status, Some(s), s.label());
                    }
                });
            egui::ComboBox::from_id_salt("card_thread_f")
                .width(110.0)
                .selected_text(st.thread.and_then(|t| doc.c.thread(t)).map(|t| t.name.clone()).unwrap_or("Any thread".into()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut st.thread, None, "Any thread");
                    for t in &doc.c.threads {
                        ui.selectable_value(&mut st.thread, Some(t.id), &t.name);
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.weak("Sort:");
            ui.selectable_value(&mut st.sort, Sort::Modified, "Recent");
            ui.selectable_value(&mut st.sort, Sort::Title, "Title");
            ui.selectable_value(&mut st.sort, Sort::Status, "Status");
        });
        let n_secret = doc.c.story_cards.iter().filter(|c| c.secret).count();
        if n_secret > 0 || st.secret != SecretFilter::Any {
            egui::ComboBox::from_id_salt("card_secret_f")
                .width(150.0)
                .selected_text(st.secret.label())
                .show_ui(ui, |ui| {
                    for f in [SecretFilter::Any, SecretFilter::Secrets, SecretFilter::Unrevealed, SecretFilter::NotSecret] {
                        ui.selectable_value(&mut st.secret, f, f.label());
                    }
                });
        }
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let f = st.filter.to_lowercase();
            let mut list: Vec<&StoryCard> = doc
                .c
                .story_cards
                .iter()
                .filter(|c| !st.ideas_only || c.in_ideas_pile)
                .filter(|c| st.status.map(|s| c.status == s).unwrap_or(true))
                .filter(|c| st.thread.map(|t| c.threads.contains(&t)).unwrap_or(true))
                .filter(|c| st.secret.keep(c))
                .filter(|c| {
                    f.is_empty()
                        || c.title.to_lowercase().contains(&f)
                        || c.body.to_lowercase().contains(&f)
                        || c.tags.iter().any(|t| t.to_lowercase().contains(&f))
                })
                .collect();
            match st.sort {
                Sort::Modified => list.sort_by(|a, b| b.modified.cmp(&a.modified)),
                Sort::Title => list.sort_by_key(|c| c.display_title().to_lowercase()),
                Sort::Status => list.sort_by_key(|c| c.status as u8),
            }
            if list.is_empty() {
                ui.weak("No cards match.");
            }
            for c in list {
                ui.horizontal(|ui| {
                    ui_util::swatch(ui, c.color.or_else(|| c.threads.first().and_then(|t| doc.c.thread(*t)).and_then(|t| t.color)));
                    let lock = if c.is_unrevealed_secret() {
                        "🔒 "
                    } else if c.secret {
                        "🔓 "
                    } else {
                        ""
                    };
                    let mut text = RichText::new(format!("{lock}{}", c.display_title()));
                    if c.title.trim().is_empty() {
                        text = text.italics();
                    }
                    if c.status == CardStatus::Discarded {
                        text = text.strikethrough().weak();
                    }
                    let r = ui_util::list_row(ui, st.selected == Some(c.id), text);
                    if r.clicked() {
                        st.selected = Some(c.id);
                    }
                    r.on_hover_text(format!("{}{}", c.status.label(), if c.in_ideas_pile { " · ideas pile" } else { "" }));
                });
            }
        });
    });

    let Some(id) = st.selected else {
        ui.centered_and_justified(|ui| {
            ui.weak("Select a card, or click “New card”. Cards need no information at all — jot an idea and come back later.");
        });
        return;
    };
    if doc.c.card(id).is_none() {
        st.selected = None;
        return;
    }
    ui_util::pinned_scroll(ui, ("card_edit", id), |ui| editor(ui, doc, sh, id, &mut st.quest_ui));
}

fn editor(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, id: Id, qu: &mut crate::views::quests::QuestUi) {
    let dir = doc.dir.clone();
    let before = doc.c.card(id).cloned();
    // Snapshot data needed while the card is mutably borrowed.
    let threads: Vec<(Id, String, Option<[u8; 3]>)> = doc.c.threads.iter().map(|t| (t.id, t.name.clone(), t.color)).collect();
    let sessions: Vec<(Id, u32, String)> = {
        let mut s: Vec<_> = doc.c.sessions.iter().map(|s| (s.id, s.number, s.display_title())).collect();
        s.sort_by_key(|x| x.1);
        s
    };
    let maps: Vec<(Id, String)> = doc.c.maps.iter().map(|m| (m.id, m.name.clone())).collect();
    let assigned: Vec<Id> = doc.c.sessions_for_card(id).iter().map(|s| s.id).collect();
    let cal = doc.c.settings.game_calendar.clone();
    let sched = doc.c.settings.session_schedule.clone();
    let mut assign: Option<Id> = None;
    let mut unassign: Option<Id> = None;
    let upcoming = doc.c.upcoming_session().map(|s| s.number);

    let card = doc.c.card_mut(id).unwrap();
    ui.horizontal(|ui| {
        ui.weak(format!("Card · {}", card.status.label()));
        if card.is_unrevealed_secret() {
            ui.label(RichText::new("🔒 Secret").color(ui.visuals().warn_fg_color))
                .on_hover_text("The players don't know this yet");
        } else if card.secret {
            ui.label(RichText::new("🔓 Revealed secret").weak());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui_util::confirm_delete_button(ui, Link::Card(id), sh);
            if ui.button("Export to wiki").clicked() {
                sh.actions.push(Action::ExportToWiki(Link::Card(id)));
            }
            if ui.button("📌 Place on map").clicked() {
                sh.actions.push(Action::PlaceOnMap(id));
            }
            if ui.button("🕸 Story board").on_hover_text("Show this card on the story board").clicked() {
                sh.actions.push(Action::ShowOnBoard(id));
            }
        });
    });
    ui.add(
        egui::TextEdit::singleline(&mut card.title)
            .hint_text("Untitled card (a title is optional)")
            .font(egui::TextStyle::Heading)
            .desired_width(f32::INFINITY),
    );

    ui_util::markdown_editor(ui, ("cardbody", id), &mut card.body, &mut sh.md, 10);

    egui::CollapsingHeader::new(RichText::new("Details").strong())
        .id_salt(("card_details", id))
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new(("cardgrid", id)).num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label("Status");
                ui.horizontal_wrapped(|ui| {
                    for s in CardStatus::ALL {
                        ui.selectable_value(&mut card.status, s, s.label());
                    }
                });
                ui.end_row();

                ui.label("Ideas pile");
                ui.checkbox(&mut card.in_ideas_pile, "Keep in the ideas pile");
                ui.end_row();

                ui.label("Secret");
                ui.vertical(|ui| {
                    ui.checkbox(&mut card.secret, "🔒 Players don't know this yet")
                        .on_hover_text("Secrets are marked in lists and listed on the Run session screen until you reveal them");
                    if card.secret {
                        match card.revealed.clone() {
                            None => {
                                ui.horizontal(|ui| {
                                    ui.weak("Not revealed yet.");
                                    let label = match upcoming {
                                        Some(n) => format!("🔓 Reveal now (session {n})"),
                                        None => "🔓 Reveal now".to_string(),
                                    };
                                    if ui.button(label).clicked() {
                                        card.reveal(upcoming);
                                    }
                                });
                            }
                            Some(r) => {
                                ui.horizontal(|ui| {
                                    let when = match (r.session, r.date) {
                                        (Some(n), Some(d)) => format!("Revealed in session {n} ({})", ui_util::fmt_date_short(ui, d)),
                                        (Some(n), None) => format!("Revealed in session {n}"),
                                        (None, Some(d)) => format!("Revealed {}", ui_util::fmt_date_short(ui, d)),
                                        (None, None) => "Revealed".to_string(),
                                    };
                                    ui.label(when);
                                    if ui.small_button("↺ Undo").on_hover_text("Mark as still secret").clicked() {
                                        card.revealed = None;
                                    }
                                });
                                if let Some(rv) = &mut card.revealed {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut rv.note)
                                            .hint_text("How they found out (optional)")
                                            .desired_width(300.0),
                                    );
                                }
                            }
                        }
                    }
                });
                ui.end_row();

                ui.label("Colour");
                ui_util::color_opt(ui, &mut card.color);
                ui.end_row();

                ui.label("Plot threads");
                ui.horizontal_wrapped(|ui| {
                    let mut remove = None;
                    for t in &card.threads {
                        let (name, color) = threads
                            .iter()
                            .find(|x| x.0 == *t)
                            .map(|x| (x.1.clone(), x.2))
                            .unwrap_or(("(deleted)".into(), None));
                        ui_util::swatch(ui, color);
                        if ui.small_button(format!("{name} ✖")).clicked() {
                            remove = Some(*t);
                        }
                    }
                    if let Some(r) = remove {
                        card.threads.retain(|t| *t != r);
                    }
                    ui.menu_button("➕", |ui| {
                        if threads.is_empty() {
                            ui.weak("No threads yet — create them under Plot threads");
                        }
                        for (tid, name, _) in &threads {
                            if !card.threads.contains(tid) && ui.button(name).clicked() {
                                card.threads.push(*tid);
                                ui.close();
                            }
                        }
                    });
                });
                ui.end_row();

                ui.label("When");
                ui.vertical(|ui| timing_editor(ui, id, &mut card.timing, &cal, &sched, &sessions));
                ui.end_row();

                ui.label("Session plans");
                ui.horizontal_wrapped(|ui| {
                    for sid in &assigned {
                        let name = sessions.iter().find(|s| s.0 == *sid).map(|s| s.2.clone()).unwrap_or_default();
                        if ui.link(&name).clicked() {
                            sh.actions.push(Action::Open(Link::Session(*sid)));
                        }
                        if ui.small_button("✖").on_hover_text("Remove from this session's plan").clicked() {
                            unassign = Some(*sid);
                        }
                    }
                    ui.menu_button("➕ Add to session", |ui| {
                        if sessions.is_empty() {
                            ui.weak("No sessions yet");
                        }
                        for (sid, _, name) in &sessions {
                            if !assigned.contains(sid) && ui.button(name).clicked() {
                                assign = Some(*sid);
                                ui.close();
                            }
                        }
                    });
                });
                ui.end_row();

                ui.label("Map pins");
                ui.horizontal_wrapped(|ui| {
                    let mut remove = None;
                    for (i, p) in card.map_pins.iter().enumerate() {
                        let name = maps.iter().find(|m| m.0 == p.map_id).map(|m| m.1.clone()).unwrap_or("(deleted map)".into());
                        if ui.link(format!("📌 {name}")).clicked() {
                            sh.actions.push(Action::Open(Link::Map(p.map_id)));
                        }
                        if ui.small_button("✖").clicked() {
                            remove = Some(i);
                        }
                    }
                    if let Some(i) = remove {
                        card.map_pins.remove(i);
                    }
                    if ui.button("Place on map…").clicked() {
                        sh.actions.push(Action::PlaceOnMap(id));
                    }
                });
                ui.end_row();

                ui.label("Tags");
                ui_util::tags_edit(ui, ("cardtags", id), &mut card.tags);
                ui.end_row();
            });
        });

    let mut links = std::mem::take(&mut card.links);
    if card.quest.is_some() {
        let mut remove = false;
        egui::CollapsingHeader::new(RichText::new("❗ Quest").strong())
            .id_salt(("card_quest", id))
            .default_open(true)
            .show(ui, |ui| {
                crate::views::quests::quest_panel(ui, doc, sh, id, qu);
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.small_button("Show in Quests").clicked() {
                        sh.actions.push(Action::GoQuest(id));
                    }
                    if ui
                        .small_button("Not a quest any more")
                        .on_hover_text("Removes the quest details (objectives, rewards) from this card")
                        .clicked()
                    {
                        remove = true;
                    }
                });
            });
        if remove {
            if let Some(c) = doc.c.card_mut(id) {
                c.quest = None;
            }
        }
    } else if ui
        .button("❗ This is a quest…")
        .on_hover_text("Track who gave it, objectives and rewards in Quests")
        .clicked()
    {
        card.quest = Some(cm_core::Quest::default());
    }
    section(ui, "Links (NPCs, factions, places, encounters, other cards, wiki pages, URLs, images)");
    ui_util::link_images(ui, &dir, &links, sh);
    ui_util::link_chips(ui, &doc.c, &mut links, sh);
    let vault = sh.vault.take();
    ui_util::link_picker(ui, ("cardlinks", id), &doc.c, &mut links, Some(&Link::Card(id)), &dir, vault.as_ref(), sh);
    sh.vault = vault;
    // [[links]] found in the body that resolve to campaign entities
    if let Some(card) = doc.c.card(id) {
        let mentioned: Vec<String> = cm_core::wiki::extract_wikilinks(&card.body);
        let resolved: Vec<(String, Link)> = mentioned.iter().filter_map(|m| doc.c.resolve_name(m).map(|l| (m.clone(), l))).collect();
        if !resolved.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.weak("Mentioned in text:");
                for (n, l) in resolved {
                    if ui.link(&n).clicked() {
                        sh.actions.push(Action::Open(l));
                    }
                }
            });
        }
    }
    if let Some(card) = doc.c.card_mut(id) {
        card.links = links;
    }
    ui_util::backlinks(ui, &doc.c, &Link::Card(id), sh);

    if let Some(s) = assign {
        doc.c.assign_card_to_session(id, s);
    }
    if let Some(s) = unassign {
        doc.c.unassign_card_from_session(id, s);
    }
    // bump modified time on edits
    if let (Some(b), Some(card)) = (before, doc.c.card_mut(id)) {
        if b.title != card.title
            || b.body != card.body
            || b.links != card.links
            || b.timing != card.timing
            || b.status != card.status
            || b.threads != card.threads
            || b.tags != card.tags
            || b.secret != card.secret
            || b.revealed != card.revealed
            || b.quest != card.quest
        {
            card.modified = chrono::Utc::now();
        }
    }
}

fn timing_editor(
    ui: &mut egui::Ui,
    id: Id,
    timing: &mut CardTiming,
    cal: &cm_core::calendar::GameCalendar,
    sched: &cm_core::calendar::SessionSchedule,
    sessions: &[(Id, u32, String)],
) {
    let mut kind = match timing {
        CardTiming::Unscheduled => 0,
        CardTiming::RealDate(_) => 1,
        CardTiming::GameDate(_) => 2,
        CardTiming::Session(_) => 3,
    };
    let old = kind;
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut kind, 0, "Unscheduled");
        ui.selectable_value(&mut kind, 1, "Real-world date");
        ui.selectable_value(&mut kind, 2, "In-world date");
        ui.selectable_value(&mut kind, 3, "Session #");
    });
    if kind != old {
        *timing = match kind {
            1 => CardTiming::RealDate(match timing {
                CardTiming::Session(n) => sched.date_for_session(*n).unwrap_or_else(|| chrono::Local::now().date_naive()),
                _ => chrono::Local::now().date_naive(),
            }),
            2 => CardTiming::GameDate(cal.current),
            3 => CardTiming::Session(match timing {
                CardTiming::RealDate(d) => sched.session_for_date(*d).unwrap_or(1),
                _ => sessions.iter().map(|s| s.1).max().unwrap_or(1),
            }),
            _ => CardTiming::Unscheduled,
        };
    }
    match timing {
        CardTiming::Unscheduled => {
            ui.weak("Not placed in time. It still shows on the plot line under “Unscheduled”.");
        }
        CardTiming::RealDate(d) => {
            ui.horizontal(|ui| {
                ui_util::date_picker(ui, ("card_date", id), d);
                if let Some(n) = sched.session_for_date(*d) {
                    ui.weak(format!("≈ session {n} by your schedule"));
                }
            });
        }
        CardTiming::GameDate(gd) => {
            ui_util::game_date_edit(ui, ("cardgd", id), cal, gd);
            let diff = cal.days_between(&cal.current, gd);
            ui.weak(match diff {
                0 => "Today in the game world".to_string(),
                d if d > 0 => format!("{d} days after the current in-world date ({})", cal.format(&cal.current)),
                d => format!("{} days before the current in-world date ({})", -d, cal.format(&cal.current)),
            });
        }
        CardTiming::Session(n) => {
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(n).range(0..=9999).prefix("session "));
                if let Some(d) = sched.date_for_session(*n) {
                    ui.weak(format!("≈ {} by your schedule", ui_util::fmt_date(ui, d)));
                } else {
                    ui.weak("(set a play schedule in Settings to see dates)");
                }
                if let Some(s) = sessions.iter().find(|s| s.1 == *n) {
                    ui.weak(format!("» {}", s.2));
                }
            });
        }
    }
    let _ = GameDate::default;
}
