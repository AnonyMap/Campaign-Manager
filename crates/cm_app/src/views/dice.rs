//! Always-available dice roller panel (5etools-style syntax), with history, reroll,
//! macros and "save this roll to a character".

use cm_core::dice::{self, Command, RollRecord, SegKind};
use cm_core::{Campaign, CombatantSource, Id};
use egui::{Color32, Key, RichText};

use crate::app::{Action, Shared};

#[derive(Default)]
pub struct State {
    pub input: String,
    hist_pos: Option<usize>,
    pub focus: bool,
    show_help: bool,
    show_macros: bool,
    scroll_to_bottom: bool,
    /// Show the whole roll log, not just the latest rolls.
    show_older: bool,
}

/// Roll and append to the campaign's history. Returns the record on success.
pub fn roll_into_history(c: &mut Campaign, label: &str, expr: &str, roller: &str, sh: &mut Shared) -> Option<RollRecord> {
    match dice::roll_to_record(label, expr, roller, &c.dice.macros, &mut sh.rng) {
        Ok(rec) => {
            c.dice.push(rec.clone());
            Some(rec)
        }
        Err(e) => {
            sh.toast(format!("Dice: {e}"));
            None
        }
    }
}

fn seg_color(kind: SegKind, ui: &egui::Ui) -> Option<Color32> {
    let dark = ui.visuals().dark_mode;
    match kind {
        SegKind::DieMax => Some(if dark {
            Color32::from_rgb(110, 210, 110)
        } else {
            Color32::from_rgb(0, 130, 0)
        }),
        SegKind::DieMin => Some(if dark {
            Color32::from_rgb(240, 110, 110)
        } else {
            Color32::from_rgb(190, 0, 0)
        }),
        SegKind::Exploded => Some(Color32::from_rgb(220, 130, 0)),
        SegKind::Success => Some(if dark {
            Color32::from_rgb(120, 170, 255)
        } else {
            Color32::from_rgb(20, 80, 200)
        }),
        SegKind::Dropped => Some(ui.visuals().weak_text_color()),
        _ => None,
    }
}

/// Show a roll's breakdown with coloured dice.
pub fn breakdown_label(ui: &mut egui::Ui, rec: &RollRecord) {
    let mut job = egui::text::LayoutJob::default();
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let normal = ui.visuals().text_color();
    for s in &rec.breakdown {
        let color = seg_color(s.kind, ui).unwrap_or(normal);
        let mut fmt = egui::TextFormat::simple(font.clone(), color);
        if s.kind == SegKind::Dropped {
            fmt.strikethrough = egui::Stroke::new(1.0_f32, color);
        }
        job.append(&s.text, 0.0, fmt);
    }
    job.wrap.max_width = ui.available_width();
    ui.label(job);
}

/// Characters a roll can be saved to: (source, display name). Selected combatant first.
fn save_targets(c: &Campaign, selected: Option<Id>) -> Vec<(CombatantSource, String)> {
    let mut out: Vec<(CombatantSource, String)> = Vec::new();
    let push = |src: CombatantSource, name: String, out: &mut Vec<(CombatantSource, String)>| {
        if src != CombatantSource::AdHoc && !out.iter().any(|(s, _)| *s == src) {
            out.push((src, name));
        }
    };
    if let Some(r) = &c.active_encounter {
        if let Some(sel) = selected.and_then(|id| r.get(id)) {
            push(sel.source.clone(), format!("{} (selected)", sel.name), &mut out);
        }
        for cb in &r.combatants {
            if let Some(core) = c.core_for(&cb.source) {
                push(cb.source.clone(), core.display_name(), &mut out);
            }
        }
    }
    for p in c.players.iter().filter(|p| p.active) {
        push(CombatantSource::Player(p.id), p.core.display_name(), &mut out);
    }
    for n in &c.npcs {
        push(CombatantSource::Npc(n.id), n.core.display_name(), &mut out);
    }
    out
}

fn execute(input: &str, c: &mut Campaign, sh: &mut Shared, st: &mut State) {
    let cmd = match dice::parse_command(input) {
        Ok(cmd) => cmd,
        Err(e) => {
            sh.toast(format!("Dice: {e}"));
            return;
        }
    };
    c.dice.remember_input(input);
    match cmd {
        Command::Help => st.show_help = !st.show_help,
        Command::Clear => c.dice.history.clear(),
        Command::MacroList => st.show_macros = true,
        Command::MacroAdd { name, body } => {
            if let Err(e) = dice::DiceExpr::parse(&dice::expand_macros(&body, &c.dice.macros).unwrap_or(body.clone())) {
                sh.toast(format!("Macro not saved — {e}"));
            } else {
                c.dice.macros.insert(name.clone(), body);
                st.show_macros = true;
                sh.toast(format!("Saved macro #{name}"));
            }
        }
        Command::MacroRemove { name } => {
            if c.dice.macros.remove(&name).is_some() {
                sh.toast(format!("Removed macro #{name}"));
            } else {
                sh.toast(format!("No macro named #{name}"));
            }
        }
        Command::Roll { label, expression, target } => {
            let Some(rec) = roll_into_history(c, &label, &expression, "", sh) else {
                return;
            };
            if let Some(t) = target {
                match c.find_character_by_name(&t) {
                    Some(src) => {
                        let name = c.core_for(&src).map(|x| x.display_name()).unwrap_or_default();
                        let lbl = if label.trim().is_empty() { expression.clone() } else { label.clone() };
                        if let Some(core) = c.core_for_mut(&src) {
                            core.upsert_custom_roll(&lbl, &rec.expression);
                        }
                        sh.toast(format!("Saved “{lbl}” to {name}"));
                    }
                    None => sh.toast(format!("No PC/NPC matches “{t}” — roll not saved")),
                }
            }
        }
        Command::IterRoll { expression, count, target } => {
            let expanded = match dice::expand_macros(&expression, &c.dice.macros) {
                Ok(e) => e,
                Err(e) => {
                    sh.toast(format!("Dice: {e}"));
                    return;
                }
            };
            let expr = match dice::DiceExpr::parse(&expanded) {
                Ok(e) => e,
                Err(e) => {
                    sh.toast(format!("Dice: {e}"));
                    return;
                }
            };
            let mut totals = Vec::new();
            for _ in 0..count {
                match expr.roll(&mut sh.rng) {
                    Ok(o) => totals.push(o.total),
                    Err(e) => {
                        sh.toast(format!("Dice: {e}"));
                        return;
                    }
                }
            }
            let mut text = totals.iter().map(|t| dice::fmt_num(*t)).collect::<Vec<_>>().join(", ");
            let sum: f64 = totals.iter().sum();
            let total = if let Some(tg) = target {
                let hits = totals.iter().filter(|t| **t >= tg).count();
                text.push_str(&format!("  » {hits}/{count} meet {}", dice::fmt_num(tg)));
                hits as f64
            } else {
                text.push_str(&format!("  » sum {}", dice::fmt_num(sum)));
                sum
            };
            c.dice.push(RollRecord {
                id: uuid::Uuid::new_v4(),
                expression: format!("{expression} ×{count}"),
                label: "Repeated roll".into(),
                roller: String::new(),
                total,
                breakdown: vec![dice::Segment { text, kind: SegKind::Text }],
                nat20: false,
                nat1: false,
                timestamp: chrono::Utc::now(),
            });
        }
    }
    st.scroll_to_bottom = true;
}

pub fn panel(ui: &mut egui::Ui, c: &mut Campaign, sh: &mut Shared, st: &mut State, selected_combatant: Option<Id>) {
    ui.horizontal(|ui| {
        ui.heading("Dice");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.selectable_label(st.show_help, "?").on_hover_text("Syntax help").clicked() {
                st.show_help = !st.show_help;
            }
            if ui.selectable_label(st.show_macros, "Macros").clicked() {
                st.show_macros = !st.show_macros;
            }
            if ui.small_button("Clear").on_hover_text("Clear history").clicked() {
                c.dice.history.clear();
            }
        });
    });
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for (label, expr) in [
            ("d4", "1d4"),
            ("d6", "1d6"),
            ("d8", "1d8"),
            ("d10", "1d10"),
            ("d12", "1d12"),
            ("d20", "1d20"),
            ("d100", "1d100"),
            ("Adv", "2d20kh1"),
            ("Dis", "2d20kl1"),
        ] {
            if ui.button(label).on_hover_text(expr).clicked() {
                if ui.input(|i| i.modifiers.shift) {
                    st.input.push_str(&format!("+{expr}"));
                    st.focus = true;
                } else {
                    roll_into_history(c, "", expr, "", sh);
                    st.scroll_to_bottom = true;
                }
            }
        }
    });
    if st.show_help {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            egui::ScrollArea::horizontal().id_salt("dice_help").show(ui, |ui| {
                ui.label(RichText::new(dice::HELP_TEXT).monospace().small());
            });
        });
    }
    if st.show_macros {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new("Macros").strong());
            if c.dice.macros.is_empty() {
                ui.weak("None yet. Type: /macro add sneak 3d6");
            }
            let mut remove = None;
            let mut roll = None;
            for (k, v) in &c.dice.macros {
                ui.horizontal(|ui| {
                    if ui.button(format!("#{k}")).on_hover_text(v).clicked() {
                        roll = Some((k.clone(), v.clone()));
                    }
                    ui.monospace(v);
                    if ui.small_button("✖").clicked() {
                        remove = Some(k.clone());
                    }
                });
            }
            if let Some(k) = remove {
                c.dice.macros.remove(&k);
            }
            if let Some((k, _)) = roll {
                roll_into_history(c, &k, &format!("#{k}"), "", sh);
                st.scroll_to_bottom = true;
            }
        });
    }
    ui.separator();

    // Input pinned to the bottom of the panel; history fills whatever space is left.
    egui::TopBottomPanel::bottom("dice_input_panel")
        .frame(egui::Frame::new().inner_margin(egui::Margin {
            left: 0,
            right: 0,
            top: 6,
            bottom: 6,
        }))
        .show_separator_line(true)
        .show_inside(ui, |ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut st.input)
                    .hint_text("1d20+5   ·   Label: 2d6+3 @Goblin   ·   /help")
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
            if st.focus {
                resp.request_focus();
                st.focus = false;
            }
            if resp.has_focus() {
                let (up, down) = ui.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown)));
                let hist = &c.dice.input_history;
                if up && !hist.is_empty() {
                    let p = st.hist_pos.map(|p| p.saturating_sub(1)).unwrap_or(hist.len() - 1);
                    st.hist_pos = Some(p);
                    st.input = hist[p].clone();
                }
                if down {
                    if let Some(p) = st.hist_pos {
                        if p + 1 < hist.len() {
                            st.hist_pos = Some(p + 1);
                            st.input = hist[p + 1].clone();
                        } else {
                            st.hist_pos = None;
                            st.input.clear();
                        }
                    }
                }
            }
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            ui.horizontal(|ui| {
                let roll_clicked = ui.button("Roll").clicked();
                if (enter || roll_clicked) && !st.input.trim().is_empty() {
                    let input = std::mem::take(&mut st.input);
                    execute(&input, c, sh, st);
                    st.hist_pos = None;
                    st.focus = true;
                }
                if let Some(r) = c.dice.history.last() {
                    ui.weak(format!("last: {}", r.total_text()));
                }
            });
        });
    let targets = save_targets(c, selected_combatant);
    let mut reroll: Option<(String, String, String)> = None;
    let mut to_input: Option<String> = None;
    let mut save_to: Option<(CombatantSource, String, String)> = None;
    egui::ScrollArea::vertical()
        .id_salt("dice_hist")
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if c.dice.history.is_empty() {
                ui.weak("Type a roll below, e.g. 1d20+5, 4d6kh3, Fireball: 8d6\nAdd “@Name” to save the roll to a character.");
            }
            let n = c.dice.history.len();
            // laying out hundreds of rolls every frame is slow; older ones on request
            let start = if st.show_older { 0 } else { n.saturating_sub(60) };
            if start > 0 && ui.small_button(format!("Show {} older rolls", start)).clicked() {
                st.show_older = true;
            }
            for (i, rec) in c.dice.history.iter().enumerate().skip(start) {
                let frame = egui::Frame::new().inner_margin(egui::Margin::symmetric(4, 3)).fill(if i % 2 == 0 {
                    ui.visuals().faint_bg_color
                } else {
                    Color32::TRANSPARENT
                });
                frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        if !rec.roller.is_empty() {
                            ui.label(RichText::new(&rec.roller).strong());
                        }
                        if !rec.label.is_empty() {
                            ui.label(RichText::new(&rec.label).italics());
                        }
                        ui.weak(&rec.expression);
                    });
                    ui.horizontal(|ui| {
                        let mut total = RichText::new(rec.total_text())
                            .size(ui.style().text_styles[&egui::TextStyle::Body].size * 1.5)
                            .strong();
                        if rec.nat20 {
                            total = total.color(seg_color(SegKind::DieMax, ui).unwrap());
                        } else if rec.nat1 {
                            total = total.color(seg_color(SegKind::DieMin, ui).unwrap());
                        }
                        ui.label(total).on_hover_text(if rec.nat20 {
                            "Natural 20"
                        } else if rec.nat1 {
                            "Natural 1"
                        } else {
                            ""
                        });
                        ui.vertical(|ui| breakdown_label(ui, rec));
                    });
                    ui.horizontal(|ui| {
                        if ui.small_button("↻ Reroll").clicked() {
                            reroll = Some((rec.label.clone(), rec.expression.clone(), rec.roller.clone()));
                        }
                        if ui.small_button("✏").on_hover_text("Copy to input").clicked() {
                            let txt = if rec.label.is_empty() {
                                rec.expression.clone()
                            } else {
                                format!("{}: {}", rec.label, rec.expression)
                            };
                            to_input = Some(txt);
                        }
                        let id = ui.make_persistent_id(("savelabel", rec.id));
                        crate::ui_util::form_menu(ui, "➕ Save to…", |ui| {
                            let mut label: String = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| rec.label.clone());
                            ui.horizontal(|ui| {
                                ui.label("Label");
                                ui.add_sized(
                                    [150.0, ui.spacing().interact_size.y],
                                    egui::TextEdit::singleline(&mut label).hint_text(&rec.expression).desired_width(150.0),
                                );
                            });
                            ui.separator();
                            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                                if targets.is_empty() {
                                    ui.weak("No PCs or NPCs yet");
                                }
                                for (src, name) in &targets {
                                    if ui.button(name).clicked() {
                                        save_to = Some((src.clone(), label.clone(), rec.expression.clone()));
                                        ui.close();
                                    }
                                }
                            });
                            ui.data_mut(|d| d.insert_temp(id, label));
                        });
                        if i + 1 == n {
                            ui.weak(rec.timestamp.with_timezone(&chrono::Local).format("%H:%M").to_string());
                        }
                    });
                });
            }
            if st.scroll_to_bottom {
                ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                st.scroll_to_bottom = false;
            }
        });
    if let Some((l, e, r)) = reroll {
        roll_into_history(c, &l, &e, &r, sh);
        st.scroll_to_bottom = true;
    }
    if let Some(t) = to_input {
        st.input = t;
        st.focus = true;
    }
    if let Some((src, label, expr)) = save_to {
        let lbl = if label.trim().is_empty() { expr.clone() } else { label };
        let name = c.core_for(&src).map(|x| x.display_name()).unwrap_or_default();
        if let Some(core) = c.core_for_mut(&src) {
            core.upsert_custom_roll(&lbl, &expr);
            sh.toast(format!("Saved “{lbl}” to {name}"));
        }
    }

    let _ = Action::ShowDice;
}
