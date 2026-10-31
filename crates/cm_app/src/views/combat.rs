//! Combat tracker: initiative order, HP with a calculator and undo, conditions with rules
//! text, and the selected combatant's rolls / sheet. Edits to max HP, AC, initiative
//! modifier and custom rolls are written back to the PC/NPC record so they stick.

use cm_core::conditions::ConditionDef;
use cm_core::encounter::{AddOptions, DeathState, HealthState, HpChangeKind, Portion, RunningEncounter};
use cm_core::{CombatantSource, Id, Npc, NpcKind, fmt_mod};
use egui::{Color32, RichText};

use crate::app::{Action, Doc, Shared};
use crate::ui_util::{self, section};

#[derive(Default)]
pub struct State {
    pub selected: Option<Id>,
    npc_filter: String,
    adhoc_name: String,
    adhoc_hp: Option<i32>,
    adhoc_ac: Option<i32>,
    only_missing: bool,
    cond_rounds: Option<u32>,
    custom_cond_name: String,
    custom_cond_text: String,
    new_roll_label: String,
    new_roll_expr: String,
    show_log: bool,
    sb_filter: String,
    /// Combatants ticked for a group roll, with how much each takes.
    targets: Vec<(Id, Portion)>,
    group_amount: String,
    add: AddState,
    /// Scroll the turn order to whoever's turn it is (after Next / Back).
    scroll_to_current: bool,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum AddTab {
    #[default]
    StatBlock,
    Npc,
    OneOff,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum AddInit {
    /// No initiative: drag the row into place.
    #[default]
    Blank,
    Roll,
    Value,
}

/// The "Add to combat" window (also used for summons).
#[derive(Default)]
struct AddState {
    open: bool,
    tab: AddTab,
    count: u32,
    ally: bool,
    together: bool,
    init: AddInit,
    init_value: Option<i32>,
    adhoc_init_mod: Option<i32>,
    /// Adding summons for this combatant.
    summon_for: Option<Id>,
}

impl AddState {
    fn open_for(&mut self, summon_for: Option<Id>) {
        if summon_for != self.summon_for || !self.open {
            self.count = self.count.min(1);
        }
        self.open = true;
        self.summon_for = summon_for;
        if self.count == 0 {
            self.count = 1;
            self.together = true;
            self.init = AddInit::Roll;
        }
    }
}

fn heal_color(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(110, 200, 120)
    } else {
        Color32::from_rgb(20, 120, 40)
    }
}

/// A leading `+` means heal, a leading `-` means damage; returns the rest as the amount.
fn split_sign(buf: &str) -> (Option<HpChangeKind>, String) {
    let t = buf.trim();
    if let Some(rest) = t.strip_prefix('+') {
        (Some(HpChangeKind::Heal), rest.trim().to_string())
    } else if let Some(rest) = t.strip_prefix('-').or_else(|| t.strip_prefix('−')) {
        (Some(HpChangeKind::Damage), rest.trim().to_string())
    } else {
        (None, t.to_string())
    }
}

/// "= 11" for plain maths, "2d6 (2–12)" for dice.
fn hp_preview(amount: &str) -> Result<String, String> {
    let e = cm_core::dice::DiceExpr::parse(amount).map_err(|e| e.to_string())?;
    if e.is_constant() {
        let v = e.average().map_err(|e| e.to_string())?;
        if v < 0.0 {
            return Err("amount can't be negative".into());
        }
        Ok(format!("= {}", cm_core::dice::fmt_num(v.floor())))
    } else {
        let lo = e.minimum().map_err(|e| e.to_string())?;
        let hi = e.maximum().map_err(|e| e.to_string())?;
        Ok(format!("roll {}–{}", cm_core::dice::fmt_num(lo.max(0.0)), cm_core::dice::fmt_num(hi)))
    }
}

fn bloodied_color(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(235, 90, 90)
    } else {
        Color32::from_rgb(190, 20, 20)
    }
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    let was_none = doc.c.active_encounter.is_none();
    let mut enc = doc.c.active_encounter.take().unwrap_or_else(|| RunningEncounter::new("Encounter"));
    let shown_before = st.selected;
    toolbar(ui, doc, sh, st, &mut enc);
    turn_bar(ui, doc, sh, st, &mut enc);
    ui.separator();

    // If Next / Back just moved the selection, draw the old combatant's details one last
    // time: a number typed into one of its boxes is saved when the box loses focus, which
    // only happens if the box is drawn again.
    let moved_to = st.selected;
    if moved_to != shown_before && shown_before.map(|id| enc.get(id).is_some()).unwrap_or(false) {
        st.selected = shown_before;
    }
    egui::SidePanel::right("combat_detail")
        .resizable(true)
        .default_width(330.0)
        .width_range(240.0..=700.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("combat_detail_scroll")
                .show(ui, |ui| detail(ui, doc, sh, st, &mut enc));
        });
    if st.selected == shown_before && moved_to != shown_before {
        st.selected = moved_to;
        ui.ctx().request_repaint();
    }

    add_window(ui.ctx(), doc, sh, st, &mut enc);
    st.targets.retain(|t| enc.get(t.0).is_some());
    if !enc.combatants.is_empty() {
        group_bar(ui, &doc.c, sh, st, &mut enc);
    }
    egui::ScrollArea::both()
        .id_salt("combat_table")
        .auto_shrink([false, true])
        .show(ui, |ui| table(ui, doc, sh, st, &mut enc));

    if st.show_log || !enc.log.is_empty() {
        egui::CollapsingHeader::new("Combat log")
            .id_salt("combatlog")
            .default_open(false)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(140.0).stick_to_bottom(true).show(ui, |ui| {
                    for l in &enc.log {
                        ui.label(l);
                    }
                });
            });
    }
    // just looking at an empty tracker shouldn't count as an edit
    let untouched = was_none && enc.combatants.is_empty() && enc.log.is_empty() && enc.name == "Encounter" && !enc.started;
    doc.c.active_encounter = if untouched { None } else { Some(enc) };
}

/// Whose turn it is, with big Back / Next buttons (N and B on the keyboard), or the
/// Start button before combat begins.
fn turn_bar(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, enc: &mut RunningEncounter) {
    if enc.combatants.is_empty() {
        return;
    }
    let mut go: i32 = 0;
    // keyboard: N = next turn, B = back (only when not typing in a box)
    if enc.started && !ui.ctx().wants_keyboard_input() {
        ui.input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, egui::Key::N) {
                go = 1;
            } else if i.consume_key(egui::Modifiers::NONE, egui::Key::B) {
                go = -1;
            }
        });
    }
    let accent = ui.visuals().selection.stroke.color;
    let big = |text: RichText| egui::Button::new(text.size(16.0)).min_size(egui::vec2(110.0, 34.0));
    egui::Frame::new()
        .fill(ui.visuals().selection.bg_fill.gamma_multiply(0.25))
        .stroke(egui::Stroke::new(1.0_f32, accent.gamma_multiply(0.6)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !enc.started {
                ui.horizontal_wrapped(|ui| {
                    let missing = enc.combatants.iter().filter(|c| c.initiative.is_none()).count();
                    if ui
                        .add(big(RichText::new("▶ Start combat").strong()))
                        .on_hover_text("Sorts by initiative and starts round 1 with whoever rolled highest")
                        .clicked()
                    {
                        enc.start(&doc.c);
                        st.selected = enc.current().map(|c| c.id);
                        st.scroll_to_current = true;
                        sh.toast("Round 1");
                    }
                    if missing > 0 {
                        ui.label(format!(
                            "{} without initiative yet — roll NPC initiative, type the players' rolls, or drag them into place.",
                            ui_util::plural(missing, "combatant")
                        ));
                    } else {
                        ui.label("Everyone has initiative.");
                    }
                });
                return;
            }
            ui.horizontal(|ui| {
                if ui
                    .add(big(RichText::new("◀ Back")))
                    .on_hover_text("Previous turn (B) — going back a round also puts condition timers back")
                    .clicked()
                {
                    go = -1;
                }
                ui.add_space(6.0);
                let (pos, of) = enc.turn_position();
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("Round {}", enc.round)).size(18.0).strong());
                    let secs = (enc.round.saturating_sub(1)) * 6;
                    ui.weak(format!(
                        "Turn {pos} of {of} · {}",
                        if secs < 60 {
                            format!("{secs} s in")
                        } else {
                            format!("{}:{:02} in", secs / 60, secs % 60)
                        }
                    ))
                    .on_hover_text("Each round is about 6 seconds of game time");
                });
                ui.separator();
                // who's acting now
                let dir = doc.dir.clone();
                if let Some(c) = enc.current().cloned() {
                    let image = doc
                        .c
                        .core_for(&c.source)
                        .and_then(|core| core.image.clone())
                        .or_else(|| doc.c.statblock_for(&c.source).and_then(|s| s.core.image.clone()));
                    if token(ui, &dir, image.as_deref(), &c.name, c.is_player(), c.friendly(), 34.0).clicked() {
                        st.selected = Some(c.id);
                    }
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.weak("Now:");
                            if ui
                                .add(egui::Label::new(RichText::new(&c.name).size(18.0).strong().color(accent)).sense(egui::Sense::click()))
                                .clicked()
                            {
                                st.selected = Some(c.id);
                                st.scroll_to_current = true;
                            }
                        });
                        let mut notes: Vec<String> = c
                            .conditions
                            .iter()
                            .map(|x| match x.rounds_remaining {
                                Some(r) => format!("{} ({r})", x.name),
                                None => x.name.clone(),
                            })
                            .collect();
                        if c.is_player() && c.health() == HealthState::Down {
                            notes.insert(0, "at 0 HP — death save!".into());
                        }
                        if !notes.is_empty() {
                            ui.label(RichText::new(notes.join(" · ")).small().color(ui.visuals().warn_fg_color));
                        }
                    });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let next = big(RichText::new("Next ▶").strong().color(ui.visuals().strong_text_color())).fill(ui.visuals().selection.bg_fill);
                    if ui
                        .add(next)
                        .on_hover_text("Next turn (N) — passes over downed monsters; a new round counts condition timers down")
                        .clicked()
                    {
                        go = 1;
                    }
                    if let Some(n) = enc.up_next() {
                        ui.add(egui::Label::new(RichText::new(format!("Up next: {}", n.name)).weak()).truncate());
                    }
                });
            });
        });
    if go != 0 {
        let round = enc.round;
        if go > 0 {
            enc.next_turn();
        } else {
            enc.prev_turn();
        }
        if enc.round != round {
            sh.toast(format!("Round {}", enc.round));
        }
        st.selected = enc.current().map(|c| c.id);
        st.scroll_to_current = true;
    }
}

fn toolbar(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, enc: &mut RunningEncounter) {
    ui.horizontal_wrapped(|ui| {
        // Size the title to fit its text (up to most of the row) so long names stay readable.
        let font = egui::TextStyle::Heading.resolve(ui.style());
        let text_w = ui.painter().layout_no_wrap(enc.name.clone(), font, Color32::WHITE).size().x;
        let max_w = (ui.available_width() * 0.6).max(220.0);
        ui.add(
            egui::TextEdit::singleline(&mut enc.name)
                .font(egui::TextStyle::Heading)
                .desired_width((text_w + 12.0).clamp(220.0, max_w)),
        )
        .on_hover_text("Encounter name (click to rename)");
        ui.separator();
        if ui
            .button("🎲 Roll NPC initiative")
            .on_hover_text("d20 + assigned initiative (or DEX mod) for every non-player combatant")
            .clicked()
        {
            let rolls = enc.roll_npc_initiative(&doc.c, st.only_missing, &mut sh.rng);
            let assigned = rolls.iter().filter(|r| matches!(r.source, cm_core::InitSource::Assigned(_))).count();
            sh.toast(format!("Rolled initiative for {} (used assigned modifier for {assigned})", rolls.len()));
        }
        ui.checkbox(&mut st.only_missing, "only empty");
        if ui.button("Sort").on_hover_text("Sort by initiative").clicked() {
            enc.sort_by_initiative(&doc.c);
        }
    });
    ui.horizontal(|ui| {
        if ui.button("➕ All active PCs").clicked() {
            let _ = &st.npc_filter;
            for p in doc.c.players.iter().filter(|p| p.active) {
                if !enc.combatants.iter().any(|c| c.source == CombatantSource::Player(p.id)) {
                    enc.add_from_campaign(&doc.c, CombatantSource::Player(p.id));
                }
            }
        }
        if ui
            .button(RichText::new("➕ Add creatures…").strong())
            .on_hover_text("NPCs, stat blocks / SRD monsters or one-offs — several at once, enemies or allies, with or without rolling initiative")
            .clicked()
        {
            st.add.open_for(None);
        }
        ui.separator();
        if ui.button("Save as premade").on_hover_text("Save this roster as a reusable encounter").clicked() {
            let e = enc.to_premade();
            let id = e.id;
            doc.c.encounters.push(e);
            sh.toast("Saved to Encounters");
            let _ = id;
        }
        if ui.button("💰 Loot").on_hover_text("Generate treasure for the enemies in this fight").clicked() {
            sh.actions.push(Action::LootFor(None));
        }
        if ui
            .button("End combat")
            .on_hover_text("Clear the tracker (Edit » Undo brings it back)")
            .clicked()
        {
            *enc = RunningEncounter::new("Encounter");
            st.selected = None;
        }
    });
}

const DRAG_W: f32 = 12.0;
const SEL_W: f32 = 18.0;
const TOKEN_W: f32 = 40.0;
const MARK_W: f32 = 14.0;
const INIT_W: f32 = 40.0;
const NAME_W: f32 = 170.0;
const AC_W: f32 = 30.0;
const HP_W: f32 = 165.0;

/// A fixed-width column cell.
fn cell<R>(ui: &mut egui::Ui, w: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.allocate_ui_with_layout(egui::vec2(w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(w);
        add(ui)
    })
    .inner
}

fn table(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, enc: &mut RunningEncounter) {
    if enc.combatants.is_empty() {
        ui.add_space(20.0);
        ui.weak("Add combatants above, or open an encounter under Encounters and click “Run encounter”.");
        return;
    }
    ui.weak(
        "Damage / heal: type an amount — maths and dice work (5+6 → 11, 2d6 → rolled) — then click −Dmg or +Heal. Enter = damage; start with + to heal (+6).",
    );
    let current = if enc.started { enc.combatants.get(enc.turn).map(|c| c.id) } else { None };
    let conds: Vec<ConditionDef> = doc.c.all_conditions();
    let mut remove = None;
    let mut resort = false;
    let mut pending_hp: Option<(Id, HpChangeKind, String)> = None;
    let mut death_msg: Option<String> = None;
    let ids: Vec<Id> = enc.combatants.iter().map(|c| c.id).collect();
    let dir = doc.dir.clone();
    // token image per combatant (an NPC without its own picture uses its stat block's)
    let tokens: std::collections::HashMap<Id, Option<String>> = enc
        .combatants
        .iter()
        .map(|c| {
            let own = doc.c.core_for(&c.source).and_then(|core| core.image.clone());
            (c.id, own.or_else(|| doc.c.statblock_for(&c.source).and_then(|s| s.core.image.clone())))
        })
        .collect();
    let group_names: std::collections::HashMap<Id, Vec<String>> = enc
        .combatants
        .iter()
        .filter(|c| c.init_group.is_some())
        .map(|c| {
            (
                c.id,
                enc.group_members(c.id).iter().filter_map(|m| enc.get(*m)).map(|m| m.name.clone()).collect(),
            )
        })
        .collect();
    let summoner_names: std::collections::HashMap<Id, String> = enc
        .combatants
        .iter()
        .filter_map(|c| Some((c.id, enc.get(c.summoned_by?)?.name.clone())))
        .collect();
    // For "drop below row i": the next row that isn't one of row i's summons.
    let next_top: Vec<Option<Id>> = (0..ids.len())
        .map(|i| enc.combatants[i + 1..].iter().find(|c| c.summoned_by != Some(ids[i])).map(|c| c.id))
        .collect();
    let mut pending_move: Option<(Id, Option<Id>)> = None;
    let mut pending_init: Option<(Id, Option<i32>)> = None;
    let mut summon_for: Option<Id> = None;
    // Two-line rows: stats on top, damage/heal controls underneath (fits narrow screens).
    let h = ui.spacing().interact_size.y;
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        cell(ui, DRAG_W, |_| {});
        cell(ui, SEL_W, |ui| {
            let all_selected = !enc.combatants.is_empty() && enc.combatants.iter().all(|c| st.targets.iter().any(|t| t.0 == c.id));
            let mut v = all_selected;
            if ui.checkbox(&mut v, "").on_hover_text("Select everyone for a group roll").changed() {
                st.targets = if v {
                    enc.combatants.iter().map(|c| (c.id, Portion::Full)).collect()
                } else {
                    Vec::new()
                };
            }
        });
        cell(ui, MARK_W, |_| {});
        cell(ui, INIT_W, |ui| {
            ui.label(RichText::new("Init").small().strong());
        });
        cell(ui, TOKEN_W, |_| {});
        cell(ui, NAME_W, |ui| {
            ui.label(RichText::new("Name").small().strong());
        });
        cell(ui, AC_W, |ui| {
            ui.label(RichText::new("AC").small().strong());
        });
        cell(ui, HP_W, |ui| {
            ui.label(RichText::new("HP").small().strong());
        });
    });
    for (row, cid) in ids.into_iter().enumerate() {
        let Some(c) = enc.get_mut(cid) else { continue };
        let is_current = current == Some(cid);
        let selected = st.selected == Some(cid);
        let fill = if is_current {
            ui.visuals().selection.bg_fill.gamma_multiply(0.6)
        } else if row % 2 == 0 {
            ui.visuals().faint_bg_color
        } else {
            Color32::TRANSPARENT
        };
        let is_summon = summoner_names.contains_key(&cid);
        let row_resp = egui::Frame::new().fill(fill).inner_margin(egui::Margin::symmetric(4, 4)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                cell(ui, DRAG_W, |ui| {
                    if is_summon {
                        return; // summons move with their summoner
                    }
                    let r = ui
                        .dnd_drag_source(egui::Id::new(("cdrag", cid)), cid, |ui| {
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 22.0), egui::Sense::hover());
                            let col = ui.visuals().weak_text_color();
                            for dx in [3.0, 7.0] {
                                for dy in [5.0, 11.0, 17.0] {
                                    ui.painter().circle_filled(rect.min + egui::vec2(dx, dy), 1.4, col);
                                }
                            }
                        })
                        .response;
                    if r.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    }
                    r.on_hover_text("Drag to move in the turn order (sets initiative from the neighbours; no roll needed)");
                });
                cell(ui, SEL_W, |ui| {
                    let mut on = st.targets.iter().any(|t| t.0 == cid);
                    if ui.checkbox(&mut on, "").on_hover_text("Select for a group roll (Fireball, Bless…)").changed() {
                        if on {
                            st.targets.push((cid, Portion::Full));
                        } else {
                            st.targets.retain(|t| t.0 != cid);
                        }
                    }
                });
                cell(ui, MARK_W, |ui| {
                    ui.label(if is_current {
                        RichText::new("▶").strong().color(ui.visuals().hyperlink_color)
                    } else {
                        RichText::new(" ")
                    });
                });
                cell(ui, INIT_W, |ui| {
                    // initiative
                    let mut init = c.initiative;
                    let r = if is_summon {
                        ui.add_enabled_ui(false, |ui| ui_util::opt_int(ui, ("init", cid), &mut init, 36.0)).inner
                    } else {
                        ui_util::opt_int(ui, ("init", cid), &mut init, 36.0)
                    };
                    let mut tip = c.initiative_note.clone();
                    if let Some(members) = group_names.get(&cid) {
                        tip = format!("Shares one initiative roll with: {}\n{tip}", members.join(", "));
                    }
                    if !tip.trim().is_empty() {
                        r.clone().on_hover_text(tip.trim());
                    }
                    if init != c.initiative {
                        pending_init = Some((cid, init));
                    }
                    if r.lost_focus() {
                        resort = true;
                    }
                });
                cell(ui, TOKEN_W, |ui| {
                    let size = if is_summon { 28.0 } else { 36.0 };
                    let image = tokens.get(&cid).cloned().flatten();
                    let clicked = ui
                        .horizontal(|ui| {
                            if is_summon {
                                // a little elbow from the summoner's token above
                                let p = ui.cursor().min;
                                let stroke = egui::Stroke::new(1.5_f32, ui.visuals().weak_text_color());
                                ui.painter().line_segment([p + egui::vec2(3.0, -12.0), p + egui::vec2(3.0, 14.0)], stroke);
                                ui.painter().line_segment([p + egui::vec2(3.0, 14.0), p + egui::vec2(9.0, 14.0)], stroke);
                                ui.add_space(8.0);
                            }
                            token(ui, &dir, image.as_deref(), &c.name, c.is_player(), c.friendly(), size).clicked()
                        })
                        .inner;
                    if clicked {
                        match image {
                            Some(p) => sh.actions.push(Action::ViewImage(p)),
                            None => st.selected = Some(cid),
                        }
                    }
                });
                // name + conditions
                cell(ui, NAME_W, |ui| {
                    let mut name = RichText::new(&c.name);
                    if c.is_player() {
                        name = name.strong();
                    }
                    if c.health() == HealthState::Down {
                        name = name.strikethrough();
                    }
                    if is_current {
                        name = name.color(ui.visuals().hyperlink_color);
                    }
                    let resp = ui
                        .horizontal(|ui| {
                            if is_summon {
                                ui.label(RichText::new("↳").weak());
                            }
                            ui.add(egui::Button::selectable(selected, name).truncate())
                        })
                        .inner;
                    if resp.clicked() {
                        st.selected = Some(cid);
                    }
                    resp.context_menu(|ui| {
                        if ui.button("➕ Add summon…").clicked() {
                            summon_for = Some(cid);
                            ui.close();
                        }
                    });
                    if let Some(by) = summoner_names.get(&cid) {
                        ui.label(
                            RichText::new("summon")
                                .small()
                                .color(if c.friendly() { heal_color(ui) } else { bloodied_color(ui) }),
                        )
                        .on_hover_text(format!("Summoned by {by}; acts right after them"));
                    } else if c.ally {
                        ui.label(RichText::new("ally").small().color(heal_color(ui)));
                    }
                    if !c.conditions.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            let mut rm = None;
                            for (i, ac) in c.conditions.iter().enumerate() {
                                let def = conds.iter().find(|d| d.name.eq_ignore_ascii_case(&ac.name));
                                let mut label = ac.name.clone();
                                if ac.level > 0 {
                                    label = format!("{label} {}", ac.level);
                                }
                                if let Some(r) = ac.rounds_remaining {
                                    label = format!("{label} ({r})");
                                }
                                let chip = ui.add(egui::Button::new(RichText::new(label).small()).corner_radius(8));
                                let chip = chip.on_hover_ui(|ui| {
                                    ui.label(RichText::new(&ac.name).strong());
                                    match def {
                                        Some(d) => {
                                            for e in &d.effects {
                                                ui.label(format!("• {e}"));
                                            }
                                        }
                                        None => {
                                            ui.weak("Custom status");
                                        }
                                    }
                                    if !ac.note.is_empty() {
                                        ui.label(&ac.note);
                                    }
                                    ui.weak("Right-click to remove");
                                });
                                if chip.secondary_clicked() {
                                    rm = Some(i);
                                }
                                if chip.clicked() {
                                    st.selected = Some(cid);
                                }
                            }
                            if let Some(i) = rm {
                                c.conditions.remove(i);
                            }
                        });
                    }
                });
                cell(ui, AC_W, |ui| {
                    ui.label(c.ac.map(|a| a.to_string()).unwrap_or("—".into()));
                });
                // HP
                cell(ui, HP_W, |ui| {
                    let health = c.health();
                    let text = match (c.current_hp, c.max_hp) {
                        (Some(h), Some(m)) => format!("{h} / {m}"),
                        (Some(h), None) => format!("{h}"),
                        (None, Some(m)) => format!("{m} / {m}"),
                        (None, None) => "—".into(),
                    };
                    let temp = if c.temp_hp > 0 { format!("  +{} temp", c.temp_hp) } else { String::new() };
                    let color = match health {
                        HealthState::Bloodied => Some(bloodied_color(ui)),
                        HealthState::Down => Some(ui.visuals().weak_text_color()),
                        _ => None,
                    };
                    ui.horizontal(|ui| {
                        let mut t = RichText::new(format!("{text}{temp}")).monospace();
                        if let Some(col) = color {
                            t = t.color(col);
                        }
                        ui.label(t);
                        match health {
                            HealthState::Bloodied => {
                                ui.label(RichText::new("🔻 Bloodied").small().color(bloodied_color(ui)))
                                    .on_hover_text("At or below half HP");
                            }
                            HealthState::Down => {
                                ui.label(RichText::new("💀 Down").small().strong());
                            }
                            _ => {}
                        }
                    });
                    if let Some(f) = c.hp_fraction() {
                        let fill = match health {
                            HealthState::Bloodied => bloodied_color(ui),
                            HealthState::Down => ui.visuals().weak_text_color(),
                            _ => Color32::from_rgb(60, 150, 70),
                        };
                        ui.add(egui::ProgressBar::new(f).desired_width(90.0).desired_height(5.0).fill(fill));
                    }
                    if let Some(ds) = c.death_state() {
                        death_saves(ui, c, ds, &mut sh.rng, &mut death_msg);
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if ui.small_button("✖").on_hover_text("Remove from combat").clicked() {
                        remove = Some(cid);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.add_space(DRAG_W + SEL_W + MARK_W + INIT_W + TOKEN_W + 5.0 * ui.spacing().item_spacing.x);
                // HP change input: the box is an AMOUNT (math/dice evaluate to one number);
                // the button (or a leading +/-) decides whether it damages or heals.
                {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    let id = egui::Id::new(("hpin", cid));
                    let mut buf: String = ui.data(|d| d.get_temp(id)).unwrap_or_default();
                    let r = ui.add_sized(
                        [64.0, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(&mut buf).hint_text("amount").font(egui::TextStyle::Monospace),
                    );
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let (sign_kind, amount_text) = split_sign(&buf);
                    let mut kind = None;
                    if ui
                        .button(RichText::new("−Dmg").color(bloodied_color(ui)))
                        .on_hover_text("Subtract the amount from HP (temp HP first)")
                        .clicked()
                    {
                        kind = Some(HpChangeKind::Damage);
                    }
                    if ui
                        .button(RichText::new("+Heal").color(heal_color(ui)))
                        .on_hover_text("Add the amount to HP (up to max)")
                        .clicked()
                    {
                        kind = Some(HpChangeKind::Heal);
                    }
                    if ui.button("Temp").on_hover_text("Set temporary HP (doesn't stack: keeps the higher)").clicked() {
                        kind = Some(HpChangeKind::TempHp);
                    }
                    if enter {
                        kind = Some(sign_kind.unwrap_or(HpChangeKind::Damage));
                    }
                    // live preview of what will happen
                    if !amount_text.is_empty() {
                        let k = sign_kind.unwrap_or(HpChangeKind::Damage);
                        match hp_preview(&amount_text) {
                            Ok(p) => {
                                let color = if k == HpChangeKind::Heal { heal_color(ui) } else { bloodied_color(ui) };
                                ui.label(
                                    RichText::new(format!("{p} {}", if k == HpChangeKind::Heal { "heal" } else { "dmg" }))
                                        .small()
                                        .color(color),
                                )
                                .on_hover_text("What Enter will do. Or click a button to choose.");
                            }
                            Err(e) => {
                                ui.label(RichText::new("?").color(ui.visuals().error_fg_color)).on_hover_text(e);
                            }
                        }
                    }
                    if let Some(k) = kind {
                        if !amount_text.is_empty() {
                            match cm_core::dice::DiceExpr::parse(&amount_text) {
                                Ok(_) => {
                                    pending_hp = Some((cid, k, amount_text.clone()));
                                    buf.clear();
                                }
                                Err(e) => sh.toast(format!("HP: {e}")),
                            }
                        }
                    }
                    ui.data_mut(|d| d.insert_temp(id, buf));
                }
                // last change + undo
                {
                    if let Some(last) = c.hp_log.last() {
                        ui.add(egui::Label::new(RichText::new(last.note()).weak()).truncate())
                            .on_hover_text(&last.breakdown);
                        if ui.small_button("↺").on_hover_text("Undo this HP change").clicked() {
                            c.undo_last_hp();
                        }
                    }
                }
            });
        });
        // Drop target: top half = before this row, bottom half = after it.
        let rr = row_resp.response;
        if is_current {
            // a thick accent bar marks whose turn it is
            let bar = egui::Rect::from_min_max(rr.rect.left_top() - egui::vec2(2.0, 0.0), rr.rect.left_bottom() + egui::vec2(4.0, 0.0));
            ui.painter().rect_filled(bar, 2.0, ui.visuals().selection.stroke.color);
            if st.scroll_to_current {
                rr.scroll_to_me(Some(egui::Align::Center));
                st.scroll_to_current = false;
            }
        }
        // A coloured bar down the left edge joins combatants that share an initiative roll.
        if let Some(g) = enc.get(cid).and_then(|c| c.init_group) {
            let h = (g.as_u128() % 360) as f32 / 360.0;
            let col: Color32 = egui::ecolor::Hsva::new(h, 0.75, if ui.visuals().dark_mode { 0.85 } else { 0.6 }, 1.0).into();
            let bar = egui::Rect::from_min_max(rr.rect.left_top(), rr.rect.left_bottom() + egui::vec2(3.0, 0.0));
            ui.painter().rect_filled(bar, 0.0, col);
        }
        if let Some(dragged) = rr.dnd_hover_payload::<Id>() {
            if *dragged != cid {
                let y = ui.ctx().pointer_hover_pos().map(|p| p.y).unwrap_or(rr.rect.center().y);
                let above = y < rr.rect.center().y;
                let line_y = if above { rr.rect.top() } else { rr.rect.bottom() };
                ui.painter()
                    .hline(rr.rect.x_range(), line_y, egui::Stroke::new(3.0_f32, ui.visuals().selection.stroke.color));
            }
        }
        if let Some(dragged) = rr.dnd_release_payload::<Id>() {
            if *dragged != cid {
                let y = ui.ctx().pointer_interact_pos().map(|p| p.y).unwrap_or(rr.rect.center().y);
                let before = if y < rr.rect.center().y { Some(cid) } else { next_top[row] };
                pending_move = Some((*dragged, before));
            }
        }
    }
    // drop below the last row = move to the end
    let (end_rect, end_resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 18.0), egui::Sense::hover());
    if end_resp.dnd_hover_payload::<Id>().is_some() {
        ui.painter().hline(
            end_rect.x_range(),
            end_rect.top(),
            egui::Stroke::new(3.0_f32, ui.visuals().selection.stroke.color),
        );
    }
    if let Some(dragged) = end_resp.dnd_release_payload::<Id>() {
        pending_move = Some((*dragged, None));
    }
    if let Some((id, before)) = pending_move {
        enc.move_before(&doc.c, id, before);
        if let Some(c) = enc.get(id) {
            sh.toast(format!(
                "{} now at initiative {}",
                c.name,
                c.initiative.map(|v| v.to_string()).unwrap_or("—".into())
            ));
        }
    }
    if let Some((id, value)) = pending_init {
        // typing a value for one member of a shared group sets the whole group
        for m in enc.group_members(id) {
            if let Some(c) = enc.get_mut(m) {
                c.initiative = value;
                c.initiative_note = if value.is_some() { "entered manually".into() } else { String::new() };
            }
        }
    }
    if let Some(s) = summon_for {
        st.add.open_for(Some(s));
    }
    let _ = h;
    if let Some(m) = death_msg {
        sh.toast(m);
    }
    if let Some(r) = remove {
        enc.remove(r);
        st.targets.retain(|t| t.0 != r);
        if st.selected == Some(r) {
            st.selected = None;
        }
    }
    if resort {
        enc.sort_by_initiative(&doc.c);
    }
    if let Some((cid, kind, input)) = pending_hp {
        let log_len = enc.log.len();
        match enc.apply_hp(cid, kind, &input, &mut sh.rng) {
            Ok(ch) => {
                let name = enc.get(cid).map(|c| c.name.clone()).unwrap_or_default();
                // surface reminders (e.g. concentration saves) added to the log
                let extra: Vec<String> = enc.log[log_len..].iter().skip(1).cloned().collect();
                let mut msg = format!("{name}: {}", ch.note());
                if !extra.is_empty() {
                    msg = format!("{msg} — {}", extra.join("; "));
                }
                sh.toast(msg);
            }
            Err(e) => sh.toast(format!("HP: {e}")),
        }
    }
}

/// A small square token: the portrait if there is one, else initials. The border shows the
/// side: accent for PCs, green for allies, red for enemies.
fn token(ui: &mut egui::Ui, dir: &std::path::Path, image: Option<&str>, name: &str, pc: bool, friendly: bool, size: f32) -> egui::Response {
    let ring = if pc {
        ui.visuals().hyperlink_color
    } else if friendly {
        heal_color(ui)
    } else {
        bloodied_color(ui)
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let radius = 6.0;
    match image {
        Some(rel) => {
            egui::Image::new(ui_util::file_uri(dir, rel))
                .corner_radius(radius)
                .maintain_aspect_ratio(false)
                .paint_at(ui, rect.shrink(1.5));
        }
        None => {
            ui.painter().rect_filled(rect.shrink(1.5), radius, ring.gamma_multiply(0.18));
            let initials: String = name
                .split_whitespace()
                .filter(|w| !w.chars().all(|c| c.is_ascii_digit()))
                .take(2)
                .filter_map(|w| w.chars().next())
                .collect::<String>()
                .to_uppercase();
            let number: String = name
                .rsplit(' ')
                .next()
                .filter(|n| n.chars().all(|c| c.is_ascii_digit()))
                .unwrap_or("")
                .to_string();
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                initials,
                egui::FontId::proportional(size * 0.38),
                ui.visuals().strong_text_color(),
            );
            if !number.is_empty() {
                ui.painter().text(
                    rect.right_bottom() - egui::vec2(3.0, 2.0),
                    egui::Align2::RIGHT_BOTTOM,
                    number,
                    egui::FontId::proportional(size * 0.28),
                    ui.visuals().text_color(),
                );
            }
        }
    }
    ui.painter()
        .rect_stroke(rect.shrink(1.0), radius, egui::Stroke::new(2.0_f32, ring), egui::StrokeKind::Inside);
    resp.on_hover_text(if image.is_some() {
        "Click to view the image"
    } else {
        "No portrait / token yet — add one on the character's or stat block's page"
    })
}

/// Add creatures (or summons) to the tracker: several at once, enemy or ally, with the
/// initiative rolled (together or separately), typed in, or left blank to drag into place.
fn add_window(ctx: &egui::Context, doc: &mut Doc, sh: &mut Shared, st: &mut State, enc: &mut RunningEncounter) {
    if !st.add.open {
        return;
    }
    let summoner = st.add.summon_for.and_then(|id| enc.get(id)).map(|c| (c.id, c.name.clone()));
    if st.add.summon_for.is_some() && summoner.is_none() {
        st.add.summon_for = None;
    }
    let title = match &summoner {
        Some((_, n)) => format!("Add summon for {n}"),
        None => "Add to combat".to_string(),
    };
    let mut open = true;
    let mut chosen: Option<CombatantSource> = None;
    let mut chosen_form: Option<Id> = None;
    egui::Window::new(title)
        .id(egui::Id::new("combat_add_window"))
        .open(&mut open)
        .collapsible(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            let a = &mut st.add;
            egui::Grid::new("add_opts").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("How many");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut a.count).range(1..=40));
                    if a.count > 1 && summoner.is_none() {
                        ui.checkbox(&mut a.together, "roll initiative together");
                    }
                });
                ui.end_row();
                match &summoner {
                    Some((_, n)) => {
                        ui.label("Turn");
                        ui.weak(format!("acts right after {n}, on their side"));
                        ui.end_row();
                    }
                    None => {
                        ui.label("Side");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut a.ally, false, RichText::new("Enemy").color(bloodied_color(ui)));
                            ui.selectable_value(&mut a.ally, true, RichText::new("Ally").color(heal_color(ui)));
                        });
                        ui.end_row();
                        ui.label("Initiative");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut a.init, AddInit::Roll, "Roll");
                            ui.selectable_value(&mut a.init, AddInit::Value, "Set");
                            if a.init == AddInit::Value {
                                ui_util::opt_int(ui, "add_init_val", &mut a.init_value, 40.0);
                            }
                            ui.selectable_value(&mut a.init, AddInit::Blank, "None — I'll drag it");
                        });
                        ui.end_row();
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.selectable_value(&mut a.tab, AddTab::StatBlock, "Stat blocks & SRD");
                ui.selectable_value(&mut a.tab, AddTab::Npc, "NPCs");
                ui.selectable_value(&mut a.tab, AddTab::OneOff, "One-off");
            });
            match a.tab {
                AddTab::StatBlock => {
                    if let Some(id) = crate::views::statblock::picker(ui, doc, &mut st.sb_filter) {
                        chosen = Some(CombatantSource::StatBlock(id));
                    }
                }
                AddTab::Npc => {
                    ui.add(
                        egui::TextEdit::singleline(&mut st.npc_filter)
                            .hint_text("Filter NPCs…")
                            .desired_width(f32::INFINITY),
                    );
                    let f = st.npc_filter.to_lowercase();
                    egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
                        let mut list: Vec<&Npc> = doc.c.npcs.iter().filter(|n| f.is_empty() || n.core.name.to_lowercase().contains(&f)).collect();
                        list.sort_by_key(|n| (n.kind != NpcKind::Monster, n.core.name.to_lowercase()));
                        if list.is_empty() {
                            ui.weak("No NPCs match.");
                        }
                        for n in list {
                            let src = CombatantSource::Npc(n.id);
                            let cr = doc.c.cr_for(&src).map(|c| format!("  (CR {c})")).unwrap_or_default();
                            ui.horizontal_wrapped(|ui| {
                                if ui.button(format!("{}{cr}", n.core.display_name())).clicked() {
                                    chosen = Some(src.clone());
                                }
                                // other forms: "as Wolf", "as Phase 2"
                                for f in &n.other_statblocks {
                                    let Some(sb) = doc.c.statblock(*f) else { continue };
                                    let cr = doc.c.cr_as(&src, Some(*f)).map(|c| format!(" (CR {c})")).unwrap_or_default();
                                    if ui
                                        .small_button(format!("as {}{cr}", sb.name()))
                                        .on_hover_text(format!("Add {} using the {} stat block", n.core.display_name(), sb.name()))
                                        .clicked()
                                    {
                                        chosen = Some(src.clone());
                                        chosen_form = Some(*f);
                                    }
                                }
                            });
                        }
                    });
                }
                AddTab::OneOff => {
                    egui::Grid::new("adhoc").num_columns(2).show(ui, |ui| {
                        ui.label("Name");
                        ui.add_sized(
                            [180.0, ui.spacing().interact_size.y],
                            egui::TextEdit::singleline(&mut st.adhoc_name).hint_text("Creature"),
                        );
                        ui.end_row();
                        ui.label("Max HP");
                        ui_util::opt_int(ui, "adhochp", &mut st.adhoc_hp, 60.0);
                        ui.end_row();
                        ui.label("AC");
                        ui_util::opt_int(ui, "adhocac", &mut st.adhoc_ac, 60.0);
                        ui.end_row();
                        ui.label("Init modifier");
                        ui_util::opt_int(ui, "adhocinit", &mut st.add.adhoc_init_mod, 60.0);
                        ui.end_row();
                    });
                    if ui.button(RichText::new("Add").strong()).clicked() {
                        chosen = Some(CombatantSource::AdHoc);
                    }
                }
            }
            ui.weak("Click again to add more. This window stays open until you close it.");
        });
    if let Some(src) = chosen {
        let a = &st.add;
        let opts = AddOptions {
            count: a.count.max(1),
            ally: a.ally,
            roll_together: a.together,
            summoned_by: summoner.as_ref().map(|s| s.0),
            form: chosen_form,
        };
        let name = st.adhoc_name.trim().to_string();
        let adhoc = if src == CombatantSource::AdHoc {
            Some((name.as_str(), st.adhoc_hp, st.adhoc_ac))
        } else {
            None
        };
        let ids = enc.add_with(&doc.c, src.clone(), adhoc, &opts);
        if src == CombatantSource::AdHoc {
            if let Some(m) = a.adhoc_init_mod {
                for id in &ids {
                    if let Some(c) = enc.get_mut(*id) {
                        c.notes = format!("init {}", fmt_mod(m));
                    }
                }
            }
        }
        let label = ids
            .first()
            .and_then(|i| enc.get(*i))
            .map(|c| c.name.trim_end_matches(|ch: char| ch.is_ascii_digit()).trim().to_string())
            .unwrap_or_default();
        let mut msg = if ids.len() == 1 {
            format!("Added {label}")
        } else {
            format!("Added {} × {label}", ids.len())
        };
        if summoner.is_none() {
            match a.init {
                AddInit::Roll => {
                    if a.together && ids.len() > 1 {
                        if let Some(r) = enc.roll_shared(&doc.c, &ids, &mut sh.rng) {
                            msg.push_str(&format!(" — initiative {} for all", r.total));
                        }
                    } else {
                        let totals: Vec<String> = ids
                            .iter()
                            .filter_map(|id| enc.roll_initiative_for(&doc.c, *id, &mut sh.rng))
                            .map(|r| r.total.to_string())
                            .collect();
                        msg.push_str(&format!(" — initiative {}", totals.join(", ")));
                    }
                    enc.sort_by_initiative(&doc.c);
                }
                AddInit::Value => {
                    for id in &ids {
                        if let Some(c) = enc.get_mut(*id) {
                            c.initiative = a.init_value;
                            c.initiative_note = "entered when added".into();
                        }
                    }
                    enc.sort_by_initiative(&doc.c);
                }
                AddInit::Blank => msg.push_str(" — drag the dotted handle on the left to place them in the turn order"),
            }
        }
        st.selected = ids.first().copied();
        sh.toast(msg);
    }
    if !open {
        st.add.open = false;
        st.add.summon_for = None;
    }
}

/// Toolbar for a roll that hits several combatants at once.
fn group_bar(ui: &mut egui::Ui, campaign: &cm_core::Campaign, sh: &mut Shared, st: &mut State, enc: &mut RunningEncounter) {
    let last_group = enc.last_group;
    if st.targets.is_empty() {
        if let Some(g) = last_group {
            ui.horizontal(|ui| {
                ui.weak("Tick combatants to damage or heal several at once.");
                if ui.small_button("↺ Undo last group roll").clicked() {
                    let n = enc.undo_group(g);
                    sh.toast(format!("Undid the group roll on {}", ui_util::plural(n, "combatant")));
                }
            });
        } else {
            ui.weak("Tip: tick several combatants to damage or heal them all with one roll. Drag the dotted handle to move someone in the turn order.");
        }
        return;
    }
    egui::Frame::group(ui.style())
        .fill(ui.visuals().selection.bg_fill.gamma_multiply(0.15))
        .inner_margin(6)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("Group roll · {}", ui_util::plural(st.targets.len(), "target"))).strong());
                let r = ui.add_sized(
                    [90.0, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut st.group_amount)
                        .hint_text("8d6")
                        .font(egui::TextStyle::Monospace),
                );
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let (sign, amount) = split_sign(&st.group_amount);
                let mut kind = None;
                if ui
                    .button(RichText::new("−Dmg all").color(bloodied_color(ui)))
                    .on_hover_text("Roll once, damage every ticked combatant")
                    .clicked()
                {
                    kind = Some(HpChangeKind::Damage);
                }
                if ui
                    .button(RichText::new("+Heal all").color(heal_color(ui)))
                    .on_hover_text("Roll once, heal every ticked combatant")
                    .clicked()
                {
                    kind = Some(HpChangeKind::Heal);
                }
                if enter {
                    kind = Some(sign.unwrap_or(HpChangeKind::Damage));
                }
                if !amount.is_empty() {
                    match hp_preview(&amount) {
                        Ok(p) => {
                            ui.weak(p);
                        }
                        Err(e) => {
                            ui.label(RichText::new("?").color(ui.visuals().error_fg_color)).on_hover_text(e);
                        }
                    }
                }
                if ui.small_button("Clear").clicked() {
                    st.targets.clear();
                }
                if let Some(g) = last_group {
                    if ui.small_button("↺ Undo last group").clicked() {
                        enc.undo_group(g);
                    }
                }
                if let (Some(k), false) = (kind, amount.is_empty()) {
                    match enc.apply_hp_group(&st.targets, k, &amount, &mut sh.rng) {
                        Ok((_, total)) => {
                            let hit: Vec<String> = st
                                .targets
                                .iter()
                                .filter_map(|(id, p)| {
                                    Some(format!(
                                        "{} {}",
                                        enc.get(*id)?.name,
                                        p.apply(total).map(|v| v.to_string()).unwrap_or("–".into())
                                    ))
                                })
                                .collect();
                            let extra: Vec<&String> = enc.log.iter().rev().skip(1).take_while(|l| l.contains("concentrating")).collect();
                            let mut msg = format!("{amount} = {total} {}: {}", k.label(), hit.join(", "));
                            if !extra.is_empty() {
                                msg.push_str(&format!(" — {} concentration check(s)", extra.len()));
                            }
                            sh.toast(msg);
                            st.group_amount.clear();
                            st.targets.clear();
                        }
                        Err(e) => sh.toast(format!("Group roll: {e}")),
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.weak("Each takes:");
                let mut drop = None;
                for (i, (id, portion)) in st.targets.iter_mut().enumerate() {
                    let Some(c) = enc.get(*id) else { continue };
                    let text = match portion {
                        Portion::Full => RichText::new(format!("{} · full", c.name)),
                        Portion::Half => RichText::new(format!("{} · half", c.name)).italics(),
                        Portion::Double => RichText::new(format!("{} · ×2", c.name)).strong(),
                        Portion::None => RichText::new(format!("{} · none", c.name)).strikethrough().weak(),
                    };
                    let r = ui
                        .add(egui::Button::new(text.small()).corner_radius(8))
                        .on_hover_text("Click: full → half (made the save / resistant) → none → ×2 (vulnerable). Right-click: untick.");
                    if r.clicked() {
                        *portion = match portion {
                            Portion::Full => Portion::Half,
                            Portion::Half => Portion::None,
                            Portion::None => Portion::Double,
                            Portion::Double => Portion::Full,
                        };
                    }
                    if r.secondary_clicked() {
                        drop = Some(i);
                    }
                }
                if let Some(i) = drop {
                    st.targets.remove(i);
                }
                if ui.small_button("all half").on_hover_text("Everyone made the save").clicked() {
                    st.targets.iter_mut().for_each(|t| t.1 = Portion::Half);
                }
                if ui.small_button("all full").clicked() {
                    st.targets.iter_mut().for_each(|t| t.1 = Portion::Full);
                }
            });
            if st.targets.len() > 1 {
                ui.horizontal_wrapped(|ui| {
                    ui.weak("Initiative:");
                    let ids: Vec<Id> = st.targets.iter().map(|t| t.0).collect();
                    if ui
                        .small_button("🔗 Roll together")
                        .on_hover_text("One initiative roll shared by all ticked combatants")
                        .clicked()
                    {
                        enc.link_initiative(&ids);
                        if let Some(r) = enc.roll_shared(campaign, &ids, &mut sh.rng) {
                            sh.toast(format!("Initiative {} for {}", r.total, ui_util::plural(ids.len(), "combatant")));
                        }
                        enc.sort_by_initiative(campaign);
                    }
                    if ui
                        .small_button("🔗 Link")
                        .on_hover_text("Share initiative from now on (keeps the first one's value)")
                        .clicked()
                    {
                        enc.link_initiative(&ids);
                        enc.sort_by_initiative(campaign);
                    }
                    if ui.small_button("Unlink").on_hover_text("Each rolls their own initiative").clicked() {
                        for id in &ids {
                            enc.unlink_initiative(*id);
                        }
                    }
                });
            }
        });
    ui.add_space(4.0);
}

/// Three success and three failure boxes for a PC at 0 HP (click to mark), plus a roll button.
fn death_saves(ui: &mut egui::Ui, c: &mut cm_core::encounter::Combatant, ds: DeathState, rng: &mut fastrand::Rng, msg: &mut Option<String>) {
    let good = heal_color(ui);
    let bad = bloodied_color(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let pips = |ui: &mut egui::Ui, n: &mut u8, color: egui::Color32, tip: &str| {
            for i in 0..3u8 {
                let (rect, r) = ui.allocate_exact_size(egui::vec2(11.0, 11.0), egui::Sense::click());
                let filled = i < *n;
                let center = rect.center();
                if filled {
                    ui.painter().circle_filled(center, 4.5, color);
                } else {
                    ui.painter().circle_stroke(center, 4.0, egui::Stroke::new(1.0_f32, color));
                }
                if r.on_hover_text(tip).clicked() {
                    *n = if filled { i } else { i + 1 };
                }
            }
        };
        pips(ui, &mut c.death_successes, good, "Death save successes (click to mark)");
        ui.add_space(4.0);
        pips(ui, &mut c.death_failures, bad, "Death save failures (click to mark)");
        ui.add_space(4.0);
        match ds {
            DeathState::Dying => {
                if ui
                    .small_button("🎲")
                    .on_hover_text("Roll a death save (d20: 10+ succeeds, 1 = two failures, 20 = up with 1 HP)")
                    .clicked()
                {
                    let (d, m) = c.roll_death_save(rng);
                    *msg = Some(format!("{} death save: {d} — {m}", c.name));
                }
            }
            DeathState::Stable => {
                ui.label(RichText::new("Stable").small().color(good));
            }
            DeathState::Dead => {
                ui.label(RichText::new("Dead").small().strong().color(bad));
            }
        }
    });
}

fn detail(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, enc: &mut RunningEncounter) {
    let Some(cid) = st.selected.filter(|id| enc.get(*id).is_some()) else {
        ui.weak("Select a combatant to see their rolls, stats, conditions and sheet.");
        return;
    };
    let dir = doc.dir.clone();
    let src = enc.get(cid).unwrap().source.clone();
    let name = enc.get(cid).unwrap().name.clone();

    ui.horizontal(|ui| {
        ui.heading(&name);
        if let Some(link) = src.link() {
            if ui.small_button("Open record").clicked() {
                sh.actions.push(Action::Open(link));
            }
        }
    });
    if src == CombatantSource::AdHoc {
        ui.horizontal(|ui| {
            let c = enc.get_mut(cid).unwrap();
            ui.label("Name");
            ui.add_sized(
                [160.0, ui.spacing().interact_size.y],
                egui::TextEdit::singleline(&mut c.name).id(egui::Id::new(("cname", cid))).desired_width(160.0),
            );
            if ui
                .button("Save as NPC")
                .on_hover_text("Create an NPC record so rolls, HP and notes stick")
                .clicked()
            {
                let mut n = Npc {
                    kind: NpcKind::Monster,
                    ..Default::default()
                };
                n.core.name = c.name.trim_end_matches(|ch: char| ch.is_ascii_digit()).trim().to_string();
                n.core.max_hp = c.max_hp;
                n.core.ac = c.ac;
                if let Some(rest) = c.notes.strip_prefix("init ") {
                    n.core.initiative_mod = rest.trim().trim_start_matches('+').parse().ok();
                }
                c.source = CombatantSource::Npc(n.id);
                doc.c.npcs.push(n);
                sh.toast("Created an NPC for this creature");
            }
        });
    }

    // an NPC with several stat blocks: which one it's fighting as
    if let CombatantSource::Npc(nid) = &src {
        let forms = doc.c.npc(*nid).map(|n| n.statblock_ids()).unwrap_or_default();
        if forms.len() > 1 {
            let current = enc.get(cid).and_then(|c| c.form);
            let mut pick: Option<Option<Id>> = None;
            ui.horizontal_wrapped(|ui| {
                ui.label("Form").on_hover_text(
                    "Which of this NPC's stat blocks it's using. AC and max HP follow the stat block; current HP carries over \
                     (an unhurt creature starts the new form at full HP).",
                );
                for (i, f) in forms.iter().enumerate() {
                    let Some(sb) = doc.c.statblock(*f) else { continue };
                    let value = if i == 0 { None } else { Some(*f) };
                    if ui.selectable_label(current == value, sb.name()).clicked() && current != value {
                        pick = Some(value);
                    }
                }
            });
            if let Some(form) = pick {
                if enc.switch_form(&doc.c, cid, form) {
                    sh.toast(enc.log.last().cloned().unwrap_or_default());
                }
            }
        }
    }

    // side, shared initiative, summons
    {
        let c = enc.get(cid).unwrap().clone();
        ui.horizontal_wrapped(|ui| {
            if !c.is_player() && c.summoned_by.is_none() {
                ui.label("Side");
                let mut ally = c.ally;
                ui.selectable_value(&mut ally, false, RichText::new("Enemy").color(bloodied_color(ui)));
                ui.selectable_value(&mut ally, true, RichText::new("Ally").color(heal_color(ui)));
                if ally != c.ally {
                    if let Some(m) = enc.get_mut(cid) {
                        m.ally = ally;
                    }
                }
                ui.separator();
            }
            if ui
                .button("➕ Add summon…")
                .on_hover_text("A creature this combatant summoned: listed under it and acts right after it")
                .clicked()
            {
                st.add.open_for(Some(cid));
            }
        });
        if let Some(by) = c.summoned_by.and_then(|s| enc.get(s)).map(|s| s.name.clone()) {
            ui.horizontal_wrapped(|ui| {
                ui.weak(format!("Summoned by {by}; acts right after them."));
                if ui
                    .small_button("Release")
                    .on_hover_text("Make it an ordinary combatant with its own place in the order")
                    .clicked()
                {
                    if let Some(m) = enc.get_mut(cid) {
                        m.summoned_by = None;
                    }
                }
                if ui.small_button("Dismiss").on_hover_text("Remove it from combat").clicked() {
                    enc.remove(cid);
                    st.selected = c.summoned_by;
                }
            });
            if enc.get(cid).is_none() {
                return;
            }
        }
        let summons = enc.summons_of(cid);
        if !summons.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.weak("Summons:");
                for s in &summons {
                    if let Some(sc) = enc.get(*s) {
                        if ui.link(&sc.name).clicked() {
                            st.selected = Some(*s);
                        }
                    }
                }
                if ui.small_button("Dismiss all").clicked() {
                    for s in &summons {
                        enc.remove(*s);
                    }
                }
            });
        }
        let members = enc.group_members(cid);
        if members.len() > 1 {
            let names: Vec<String> = members
                .iter()
                .filter(|m| **m != cid)
                .filter_map(|m| enc.get(*m))
                .map(|m| m.name.clone())
                .collect();
            ui.horizontal_wrapped(|ui| {
                ui.weak(format!("🔗 Shares initiative with {}", names.join(", ")));
                if ui
                    .small_button("Roll separately")
                    .on_hover_text("Take this one out of the shared roll")
                    .clicked()
                {
                    enc.unlink_initiative(cid);
                }
            });
        }
    }

    // portrait (an NPC without one shows its stat block's)
    let portrait = doc
        .c
        .core_for(&src)
        .and_then(|c| c.image.clone())
        .or_else(|| doc.c.statblock_for(&src).and_then(|s| s.core.image.clone()));
    if let Some(img) = portrait {
        if ui_util::image_thumb(ui, &dir, &img, egui::vec2(120.0, 120.0)).clicked() {
            sh.actions.push(Action::ViewImage(img));
        }
    }

    // stats
    section(ui, "Stats");
    egui::Grid::new(("cstats", cid)).num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
        let init_src = enc.init_source(&doc.c, enc.get(cid).unwrap());
        ui.label("Initiative");
        ui.horizontal(|ui| {
            let c = enc.get(cid).unwrap();
            ui.label(c.initiative.map(|v| v.to_string()).unwrap_or("—".into()));
            let is_summon = c.summoned_by.is_some();
            let members = enc.group_members(cid);
            let label = if members.len() > 1 { "🎲 Roll for the group" } else { "🎲 Roll" };
            if !is_summon && ui.small_button(label).clicked() {
                if let Some(r) = enc.roll_shared(&doc.c, &members, &mut sh.rng) {
                    sh.toast(format!("{name}: initiative {} ({})", r.total, r.source.describe()));
                    enc.sort_by_initiative(&doc.c);
                }
            }
        });
        ui.end_row();
        let note = enc.get(cid).unwrap().initiative_note.clone();
        if !note.is_empty() {
            ui.label("");
            ui.weak(note);
            ui.end_row();
        }
        ui.label("Init modifier");
        ui.horizontal(|ui| {
            let mut m = match init_src {
                cm_core::InitSource::Assigned(v) => Some(v),
                _ => None,
            };
            if ui_util::opt_int_commit(ui, ("cinitmod", cid), &mut m, 40.0) {
                enc.set_initiative_mod(&mut doc.c, cid, m);
            }
            // a stat block or an NPC's other form keeps the change on this creature
            let own_record = !matches!(src, CombatantSource::StatBlock(_)) && !doc.c.is_other_form(&src, enc.get(cid).and_then(|c| c.form));
            ui.weak(match init_src {
                cm_core::InitSource::Assigned(_) if own_record => "assigned (saved to record)".to_string(),
                cm_core::InitSource::Assigned(_) => "assigned (this creature only)".to_string(),
                other => other.describe(),
            });
        });
        ui.end_row();

        ui.label("Max HP");
        ui.horizontal(|ui| {
            let mut m = enc.get(cid).unwrap().max_hp;
            // applied when you leave the box, so typing "45" never passes through 4
            if ui_util::opt_int_commit(ui, ("cmaxhp", cid), &mut m, 50.0) {
                enc.set_max_hp(&mut doc.c, cid, m);
            }
            // the HP dice of the form it's in (an NPC's own formula wins for its main form)
            let form = enc.get(cid).and_then(|c| c.form);
            let block = doc
                .c
                .statblock_as(&src, form)
                .map(|s| s.core.hp_formula.clone())
                .filter(|f| !f.trim().is_empty());
            let own = doc.c.core_for(&src).map(|c| c.hp_formula.clone()).filter(|f| !f.trim().is_empty());
            let formula = if doc.c.is_other_form(&src, form) { block.or(own) } else { own.or(block) }.unwrap_or_default();
            if !formula.trim().is_empty()
                && ui
                    .small_button(format!("🎲 {formula}"))
                    .on_hover_text("Roll HP for this combatant only")
                    .clicked()
            {
                if let Ok(o) = cm_core::dice::DiceExpr::parse(&formula).and_then(|e| e.roll(&mut sh.rng)) {
                    let c = enc.get_mut(cid).unwrap();
                    let v = o.total.max(1.0) as i32;
                    c.max_hp = Some(v);
                    c.current_hp = Some(v);
                    sh.toast(format!("{name}: rolled {} = {v} HP", o.breakdown()));
                }
            }
        });
        ui.end_row();
        ui.label("Current HP");
        ui.horizontal(|ui| {
            let r = enc.round;
            let c = enc.get_mut(cid).unwrap();
            let mut cur = c.current_hp;
            if ui_util::opt_int_commit(ui, ("ccur", cid), &mut cur, 50.0) {
                if let Some(v) = cur {
                    c.apply_amount(HpChangeKind::Set, &v.to_string(), "", v, false, r);
                }
            }
            ui.label("Temp");
            ui.add(egui::DragValue::new(&mut c.temp_hp).range(0..=999));
        });
        ui.end_row();
        ui.label("AC");
        let mut ac = enc.get(cid).unwrap().ac;
        if ui_util::opt_int_commit(ui, ("cac", cid), &mut ac, 50.0) {
            enc.set_ac(&mut doc.c, cid, ac);
        }
        ui.end_row();
        if let Some(core) = doc.c.core_for(&src) {
            if let Some(pp) = core.passive_perception {
                ui.label("Passive Perception");
                ui.label(pp.to_string());
                ui.end_row();
            }
            if !core.speed.is_empty() {
                ui.label("Speed");
                ui.label(&core.speed);
                ui.end_row();
            }
            let abil: Vec<String> = (0..6)
                .filter_map(|i| {
                    core.abilities
                        .get(i)
                        .map(|v| format!("{} {} ({})", cm_core::AbilityScores::NAMES[i], v, fmt_mod(cm_core::ability_mod(v))))
                })
                .collect();
            // (the stat block below shows abilities when there is one)
            if !abil.is_empty() && doc.c.statblock_for(&src).is_none() {
                ui.label("Abilities");
                ui.add(egui::Label::new(abil.join("  ")).wrap());
                ui.end_row();
            }
        }
    });

    // custom rolls
    section(ui, "Rolls");
    match doc.c.core_for_mut(&src) {
        Some(core) => {
            let roller = core.display_name();
            let mut remove = None;
            for (i, r) in core.custom_rolls.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.button(format!("🎲 {}", r.label)).on_hover_text(&r.notation).clicked() {
                        sh.actions.push(Action::Roll {
                            label: r.label.clone(),
                            expr: r.notation.clone(),
                            roller: name.clone(),
                        });
                    }
                    ui.monospace(&r.notation);
                    if ui.small_button("✖").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                core.custom_rolls.remove(i);
            }
            if core.custom_rolls.is_empty() {
                ui.weak("No custom rolls yet.");
            }
            ui.horizontal(|ui| {
                ui.add_sized(
                    [100.0, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut st.new_roll_label).hint_text("Label").desired_width(100.0),
                );
                ui.add_sized(
                    [90.0, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut st.new_roll_expr)
                        .hint_text("1d20+4")
                        .desired_width(90.0)
                        .font(egui::TextStyle::Monospace),
                );
                let ok = cm_core::dice::DiceExpr::parse(&st.new_roll_expr).is_ok();
                if ui.add_enabled(ok, egui::Button::new("Add")).clicked() {
                    core.upsert_custom_roll(&st.new_roll_label, &st.new_roll_expr);
                    st.new_roll_label.clear();
                    st.new_roll_expr.clear();
                }
            });
            let _ = roller;
            ui.weak("Tip: in the dice roller use “Label: 1d6+2 @Name” or a roll's “Save to…” button.");
        }
        None => {
            ui.weak("One-off creatures don't keep rolls. Use “Save as NPC” above to give it a record.");
        }
    }
    // add recent dice roll to this character in one click
    if doc.c.core_for(&src).is_some() {
        if let Some(last) = doc.c.dice.history.last().cloned() {
            let label = if last.label.is_empty() { last.expression.clone() } else { last.label.clone() };
            if ui.small_button(format!("➕ Save last roll “{label}” here")).clicked() {
                if let Some(core) = doc.c.core_for_mut(&src) {
                    core.upsert_custom_roll(&label, &last.expression);
                    sh.toast(format!("Saved “{label}” to {name}"));
                }
            }
        }
    }

    // stat block (the combatant's own, or the one its NPC uses)
    let form = enc.get(cid).and_then(|c| c.form);
    if let Some(sb) = doc.c.statblock_as(&src, form).cloned() {
        ui.add_space(6.0);
        egui::CollapsingHeader::new(RichText::new(format!("Stat block · {}", sb.name())).strong())
            .id_salt(("csb", sb.id))
            .default_open(true)
            .show(ui, |ui| {
                crate::views::statblock::render(ui, &sb, &name, sh, false);
            });
    }

    // conditions
    section(ui, "Conditions");
    let conds = doc.c.all_conditions();
    {
        let c = enc.get_mut(cid).unwrap();
        let mut remove = None;
        for (i, ac) in c.conditions.iter_mut().enumerate() {
            let def = conds.iter().find(|d| d.name.eq_ignore_ascii_case(&ac.name));
            egui::Frame::group(ui.style()).inner_margin(4).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&ac.name).strong());
                    if ac.level > 0 {
                        ui.add(egui::DragValue::new(&mut ac.level).range(1..=6).prefix("level "));
                    }
                    let mut has_rounds = ac.rounds_remaining.is_some();
                    if ui.checkbox(&mut has_rounds, "rounds").changed() {
                        ac.rounds_remaining = if has_rounds { Some(1) } else { None };
                    }
                    if let Some(r) = &mut ac.rounds_remaining {
                        ui.add(egui::DragValue::new(r).range(0..=999));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✖").clicked() {
                            remove = Some(i);
                        }
                    });
                });
                if let Some(d) = def {
                    for e in &d.effects {
                        ui.label(RichText::new(format!("• {e}")).small());
                    }
                    if d.name == "Exhaustion" && ac.level > 0 {
                        ui.label(
                            RichText::new(format!("» D20 Tests −{}, Speed −{} ft.", ac.level * 2, ac.level * 5))
                                .small()
                                .strong(),
                        );
                    }
                }
                ui.add(
                    egui::TextEdit::singleline(&mut ac.note)
                        .id(egui::Id::new(("cond_note", cid, i)))
                        .hint_text("note (source, save DC…)")
                        .desired_width(f32::INFINITY),
                );
            });
        }
        if let Some(i) = remove {
            c.conditions.remove(i);
        }
        ui.horizontal(|ui| {
            ui.menu_button("➕ Add condition", |ui| {
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for d in &conds {
                        let label = if d.custom { format!("{} (custom)", d.name) } else { d.name.clone() };
                        if ui.button(label).on_hover_text(&d.summary).clicked() {
                            c.add_condition(&d.name, st.cond_rounds);
                            ui.close();
                        }
                    }
                });
            });
            ui.label("for");
            ui_util::opt_u32(ui, "condrounds", &mut st.cond_rounds, 30.0).on_hover_text("Rounds (blank = until removed)");
            ui.label("rounds");
        });
        ui_util::form_menu(ui, "➕ Custom status / rule…", |ui| {
            ui.add(egui::TextEdit::singleline(&mut st.custom_cond_name).hint_text("Name (e.g. Hexed)"));
            ui.add(egui::TextEdit::multiline(&mut st.custom_cond_text).hint_text("What it does…").desired_rows(3));
            ui.horizontal(|ui| {
                let ok = !st.custom_cond_name.trim().is_empty();
                if ui.add_enabled(ok, egui::Button::new("Add to combatant")).clicked() {
                    c.add_condition(st.custom_cond_name.trim(), st.cond_rounds);
                    if let Some(last) = c.conditions.last_mut() {
                        last.note = st.custom_cond_text.trim().to_string();
                    }
                    st.custom_cond_name.clear();
                    st.custom_cond_text.clear();
                    ui.close();
                }
                if ui.add_enabled(ok, egui::Button::new("Add & save for reuse")).clicked() {
                    let name = st.custom_cond_name.trim().to_string();
                    c.add_condition(&name, st.cond_rounds);
                    doc.c.custom_conditions.push(ConditionDef {
                        name,
                        summary: st.custom_cond_text.lines().next().unwrap_or("").to_string(),
                        effects: st
                            .custom_cond_text
                            .lines()
                            .filter(|l| !l.trim().is_empty())
                            .map(|l| l.trim().to_string())
                            .collect(),
                        custom: true,
                        leveled: false,
                    });
                    st.custom_cond_name.clear();
                    st.custom_cond_text.clear();
                    ui.close();
                }
            });
        });
    }

    // HP history
    let c = enc.get_mut(cid).unwrap();
    if !c.hp_log.is_empty() {
        section(ui, "HP history");
        let mut undo = None;
        for ch in c.hp_log.iter().rev().take(12) {
            ui.horizontal(|ui| {
                ui.weak(format!("R{}", ch.round));
                ui.label(ch.note()).on_hover_text(&ch.breakdown);
                ui.weak(format!(
                    "{} » {}",
                    ch.before_hp.map(|v| v.to_string()).unwrap_or("—".into()),
                    ch.after_hp.map(|v| v.to_string()).unwrap_or("—".into())
                ));
                if ui.small_button("↺").on_hover_text("Undo this change").clicked() {
                    undo = Some(ch.id);
                }
            });
        }
        if let Some(u) = undo {
            c.undo_hp(u);
        }
    }

    // sheet images & notes
    if let Some(core) = doc.c.core_for(&src) {
        if !core.sheet_images.is_empty() {
            section(ui, if src_is_pc(&src) { "Character sheet" } else { "Stat block" });
            ui.horizontal_wrapped(|ui| {
                for p in &core.sheet_images {
                    if ui_util::image_thumb(ui, &dir, p, egui::vec2(ui.available_width().min(340.0), 420.0)).clicked() {
                        sh.actions.push(Action::ViewImage(p.clone()));
                    }
                }
            });
        }
        if !core.notes.trim().is_empty() {
            section(ui, "Notes");
            let notes = core.notes.clone();
            ui_util::render_markdown(ui, &mut sh.md, &notes);
        }
    }
    let c = enc.get_mut(cid).unwrap();
    section(ui, "Combat notes");
    if src == CombatantSource::AdHoc && c.notes.starts_with("init ") {
        ui.weak(format!("({})", c.notes));
    } else {
        // (an id per combatant, so typing and undo in one creature's notes never carry over
        // to the next one shown in the same place)
        ui.add(
            egui::TextEdit::multiline(&mut c.notes)
                .id(egui::Id::new(("cnotes", cid)))
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
    }
}

fn src_is_pc(s: &CombatantSource) -> bool {
    matches!(s, CombatantSource::Player(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_prefix_picks_direction() {
        assert_eq!(split_sign("5+6"), (None, "5+6".to_string()));
        assert_eq!(split_sign("+6"), (Some(HpChangeKind::Heal), "6".to_string()));
        assert_eq!(split_sign(" -2d6 "), (Some(HpChangeKind::Damage), "2d6".to_string()));
        assert_eq!(split_sign("−4"), (Some(HpChangeKind::Damage), "4".to_string()));
    }

    #[test]
    fn preview_shows_total_or_range() {
        assert_eq!(hp_preview("5+6").unwrap(), "= 11");
        assert_eq!(hp_preview("2d6").unwrap(), "roll 2–12");
        assert_eq!(hp_preview("2d4+1").unwrap(), "roll 3–9");
        assert!(hp_preview("1-5").is_err());
        assert!(hp_preview("banana").is_err());
    }
}
