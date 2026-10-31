//! Running combat: initiative, turn order, HP changes (with dice-calculator input and
//! undo), conditions, and write-back of edits to the campaign's PC/NPC records.

use serde::{Deserialize, Serialize};

use crate::conditions::{AppliedCondition, concentration_dc};
use crate::dice::{DiceError, DiceExpr, DiceRng, fmt_num};
use crate::model::{Campaign, CharacterCore, CombatantSource, Encounter, Id, InitSource, new_id};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HpChangeKind {
    Damage,
    Heal,
    TempHp,
    /// Directly set current HP.
    Set,
}

impl HpChangeKind {
    pub fn label(self) -> &'static str {
        match self {
            HpChangeKind::Damage => "damage",
            HpChangeKind::Heal => "healing",
            HpChangeKind::TempHp => "temp HP",
            HpChangeKind::Set => "set HP",
        }
    }
}

/// One HP change, kept for display and undo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HpChange {
    pub id: Id,
    pub kind: HpChangeKind,
    /// What the DM typed ("5+6", "2d6").
    pub input: String,
    /// The calculation breakdown, e.g. "[3]+[5] = 8".
    pub breakdown: String,
    pub amount: i32,
    /// True when the input needed calculating (not a plain number).
    pub calculated: bool,
    pub before_hp: Option<i32>,
    pub before_temp: i32,
    pub after_hp: Option<i32>,
    pub after_temp: i32,
    pub round: u32,
    /// Set when one roll was applied to several combatants (undo them together).
    #[serde(default)]
    pub group: Option<Id>,
    /// "half", "double"… when a group roll was adjusted for this target.
    #[serde(default)]
    pub adjustment: String,
    /// The rolled total before a group adjustment.
    #[serde(default)]
    pub rolled: Option<i32>,
    /// Death saves before the change (for undo).
    #[serde(default)]
    pub before_death_saves: (u8, u8),
}

impl HpChange {
    /// Short note shown next to the combatant, e.g. "5+6 = 11 damage".
    pub fn note(&self) -> String {
        let adj = if self.adjustment.is_empty() {
            String::new()
        } else {
            format!(" ({})", self.adjustment)
        };
        if let (Some(r), false) = (self.rolled, self.adjustment.is_empty()) {
            if r != self.amount {
                return format!("{} = {r} → {} {}{adj}", self.input, self.amount, self.kind.label());
            }
        }
        if self.calculated {
            format!("{} = {} {}{adj}", self.input, self.amount, self.kind.label())
        } else {
            format!("{} {}{adj}", self.amount, self.kind.label())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Combatant {
    pub id: Id,
    pub source: CombatantSource,
    pub name: String,
    pub initiative: Option<i32>,
    /// How the initiative was obtained ("rolled 14 + 2 (assigned init +2)").
    pub initiative_note: String,
    pub ac: Option<i32>,
    pub max_hp: Option<i32>,
    pub current_hp: Option<i32>,
    pub temp_hp: i32,
    pub conditions: Vec<AppliedCondition>,
    pub hp_log: Vec<HpChange>,
    /// Hidden from players (for a future player-facing view); also greys out in the list.
    pub hidden: bool,
    pub notes: String,
    /// Death saving throws (player characters at 0 HP).
    pub death_successes: u8,
    pub death_failures: u8,
    /// A non-player creature fighting on the party's side.
    pub ally: bool,
    /// Combatants with the same group share one initiative roll ("4 goblins, one roll").
    pub init_group: Option<Id>,
    /// A summoned creature: listed (and acting) right after the combatant that summoned it.
    pub summoned_by: Option<Id>,
    /// Orders combatants with equal initiative (higher first). Set when you drag a row.
    pub tiebreak: i32,
    /// For an NPC with several stat blocks: the one it's fighting as (None = the main one).
    pub form: Option<Id>,
}

impl Default for Combatant {
    fn default() -> Self {
        Combatant {
            id: new_id(),
            source: CombatantSource::AdHoc,
            name: String::new(),
            initiative: None,
            initiative_note: String::new(),
            ac: None,
            max_hp: None,
            current_hp: None,
            temp_hp: 0,
            conditions: Vec::new(),
            hp_log: Vec::new(),
            hidden: false,
            notes: String::new(),
            death_successes: 0,
            death_failures: 0,
            ally: false,
            init_group: None,
            summoned_by: None,
            tiebreak: 0,
            form: None,
        }
    }
}

/// Most copies of one creature a premade encounter puts into a fight.
pub const MAX_COPIES: u32 = 200;

/// Largest HP amount (damage, healing or a set value) the tracker accepts; keeps the
/// arithmetic far from integer overflow.
pub const HP_LIMIT: i32 = 1_000_000;

/// How to add creatures to a running encounter.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AddOptions {
    pub count: u32,
    pub ally: bool,
    /// Copies share one initiative roll.
    pub roll_together: bool,
    pub summoned_by: Option<Id>,
    /// Which of an NPC's stat blocks to fight as (None = the main one).
    pub form: Option<Id>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathState {
    Dying,
    Stable,
    Dead,
}

/// How much of a shared roll each target takes (saves, resistance, vulnerability).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Portion {
    #[default]
    Full,
    Half,
    Double,
    /// Not affected (e.g. succeeded on an "no damage on success" save).
    None,
}

impl Portion {
    pub const ALL: [Portion; 4] = [Portion::Full, Portion::Half, Portion::Double, Portion::None];
    pub fn apply(self, n: i32) -> Option<i32> {
        match self {
            Portion::Full => Some(n),
            Portion::Half => Some(n / 2), // 5e rounds down
            Portion::Double => Some(n.saturating_mul(2)),
            Portion::None => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Portion::Full => "full",
            Portion::Half => "half",
            Portion::Double => "×2",
            Portion::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    Unknown,
    Healthy,
    /// At or below half max HP (the 2024 rules call this "Bloodied").
    Bloodied,
    Down,
}

impl Combatant {
    /// On the party's side: player characters and allies.
    pub fn friendly(&self) -> bool {
        self.is_player() || self.ally
    }

    pub fn is_player(&self) -> bool {
        matches!(self.source, CombatantSource::Player(_))
    }

    pub fn health(&self) -> HealthState {
        match (self.current_hp, self.max_hp) {
            (Some(hp), _) if hp <= 0 => HealthState::Down,
            (Some(hp), Some(max)) if max > 0 && (hp as i64) * 2 <= max as i64 => HealthState::Bloodied,
            (Some(_), Some(_)) => HealthState::Healthy,
            (Some(_), None) => HealthState::Healthy,
            _ => HealthState::Unknown,
        }
    }

    /// 0..1 fraction of max HP, if known.
    pub fn hp_fraction(&self) -> Option<f32> {
        match (self.current_hp, self.max_hp) {
            (Some(hp), Some(max)) if max > 0 => Some((hp as f32 / max as f32).clamp(0.0, 1.0)),
            _ => None,
        }
    }

    pub fn has_condition(&self, name: &str) -> bool {
        self.conditions.iter().any(|c| c.name.eq_ignore_ascii_case(name))
    }

    /// Apply an HP change described by an expression ("5+6", "2d6", "11").
    pub fn apply_hp(&mut self, kind: HpChangeKind, input: &str, rng: &mut dyn DiceRng, round: u32) -> Result<HpChange, DiceError> {
        let expr = DiceExpr::parse(input)?;
        let out = expr.roll(rng)?;
        let amount = out.total.floor();
        if kind != HpChangeKind::Set && amount < 0.0 {
            return Err(DiceError::Invalid("amount can't be negative".into()));
        }
        let amount = amount.clamp(-HP_LIMIT as f64, HP_LIMIT as f64) as i32;
        let plain_number = input.trim().chars().all(|c| c.is_ascii_digit());
        let breakdown = format!("{} = {}", out.breakdown(), fmt_num(amount as f64));
        Ok(self.apply_amount(kind, input.trim(), &breakdown, amount, !plain_number, round))
    }

    /// Apply an already-computed amount.
    pub fn apply_amount(&mut self, kind: HpChangeKind, input: &str, breakdown: &str, amount: i32, calculated: bool, round: u32) -> HpChange {
        let amount = amount.clamp(-HP_LIMIT, HP_LIMIT);
        let before_hp = self.current_hp;
        let was_dead = self.death_state() == Some(DeathState::Dead);
        let was_stable = self.death_state() == Some(DeathState::Stable);
        let before_temp = self.temp_hp;
        let was_down = before_hp.map(|h| h <= 0).unwrap_or(false);
        let before_death_saves = (self.death_successes, self.death_failures);
        match kind {
            HpChangeKind::Damage => {
                let mut remaining = amount;
                let absorbed = remaining.min(self.temp_hp);
                self.temp_hp -= absorbed;
                remaining -= absorbed;
                if remaining > 0 {
                    let hp = self.current_hp.or(self.max_hp).unwrap_or(0);
                    // HP doesn't go below 0 in 5e.
                    self.current_hp = Some(hp.saturating_sub(remaining).max(0));
                } else if self.current_hp.is_none() {
                    self.current_hp = self.max_hp;
                }
            }
            // a dead character isn't brought back by healing (use Set HP for a revival)
            HpChangeKind::Heal if was_dead => {}
            HpChangeKind::Heal => {
                let hp = self.current_hp.or(self.max_hp).unwrap_or(0).max(0);
                let healed = hp.saturating_add(amount);
                self.current_hp = Some(match self.max_hp {
                    Some(max) => healed.min(max),
                    None => healed,
                });
            }
            HpChangeKind::TempHp => {
                // Temporary HP doesn't stack: keep the higher value.
                self.temp_hp = self.temp_hp.max(amount);
                if self.current_hp.is_none() {
                    self.current_hp = self.max_hp;
                }
            }
            HpChangeKind::Set => {
                self.current_hp = Some(amount);
            }
        }
        self.current_hp = self.current_hp.map(|h| h.clamp(-HP_LIMIT, HP_LIMIT));
        // Death saves (2024 rules): damage while at 0 HP is a failed save; any healing
        // brings the creature back and resets them.
        if self.is_player() {
            let now_down = self.current_hp.map(|h| h <= 0).unwrap_or(false);
            if kind == HpChangeKind::Damage && was_down && amount > 0 {
                // a stable character who takes damage starts dying again (2024 rules)
                if was_stable {
                    self.death_successes = 0;
                    self.death_failures = 0;
                }
                self.death_failures = (self.death_failures + 1).min(3);
            }
            if !now_down {
                self.death_successes = 0;
                self.death_failures = 0;
            }
        }
        let change = HpChange {
            id: new_id(),
            kind,
            input: input.to_string(),
            breakdown: breakdown.to_string(),
            amount,
            calculated,
            before_hp,
            before_temp,
            after_hp: self.current_hp,
            after_temp: self.temp_hp,
            round,
            group: None,
            adjustment: String::new(),
            rolled: None,
            before_death_saves,
        };
        self.hp_log.push(change.clone());
        change
    }

    /// Undo the most recent HP change. Returns it if there was one.
    pub fn undo_last_hp(&mut self) -> Option<HpChange> {
        let c = self.hp_log.pop()?;
        self.current_hp = c.before_hp;
        self.temp_hp = c.before_temp;
        (self.death_successes, self.death_failures) = c.before_death_saves;
        Some(c)
    }

    /// Undo a specific HP change: reverses its net effect on current/temp HP, keeping later
    /// changes. (Exact when no clamping happened in between; good enough at the table.)
    pub fn undo_hp(&mut self, change_id: Id) -> Option<HpChange> {
        let idx = self.hp_log.iter().position(|c| c.id == change_id)?;
        if idx == self.hp_log.len() - 1 {
            return self.undo_last_hp();
        }
        let c = self.hp_log.remove(idx);
        let d_hp = c.after_hp.unwrap_or(0).saturating_sub(c.before_hp.unwrap_or(c.after_hp.unwrap_or(0)));
        let d_temp = c.after_temp.saturating_sub(c.before_temp);
        // a hit taken while down added a failed death save: take it back too
        if self.is_player() && c.kind == HpChangeKind::Damage && c.before_hp.map(|h| h <= 0).unwrap_or(false) && c.amount > 0 {
            self.death_failures = self.death_failures.saturating_sub(1);
        }
        if let Some(hp) = self.current_hp {
            let mut v = hp.saturating_sub(d_hp);
            if let Some(max) = self.max_hp {
                v = v.min(max);
            }
            self.current_hp = Some(v.max(0));
        }
        self.temp_hp = self.temp_hp.saturating_sub(d_temp).max(0);
        if self.current_hp.map(|h| h > 0).unwrap_or(false) {
            self.death_successes = 0;
            self.death_failures = 0;
        }
        Some(c)
    }

    /// Death-save state for a player character at 0 HP.
    pub fn death_state(&self) -> Option<DeathState> {
        if !self.is_player() || self.current_hp.map(|h| h > 0).unwrap_or(true) {
            return None;
        }
        Some(if self.death_failures >= 3 {
            DeathState::Dead
        } else if self.death_successes >= 3 {
            DeathState::Stable
        } else {
            DeathState::Dying
        })
    }

    /// Roll a death save: 10+ succeeds, 1 counts as two failures, 20 regains 1 HP.
    pub fn roll_death_save(&mut self, rng: &mut dyn DiceRng) -> (u32, String) {
        let d = rng.roll_die(20);
        let msg = match d {
            20 => {
                self.current_hp = Some(1);
                self.death_successes = 0;
                self.death_failures = 0;
                "natural 20 — back up with 1 HP!".to_string()
            }
            1 => {
                self.death_failures = (self.death_failures + 2).min(3);
                "natural 1 — two failures".to_string()
            }
            n if n >= 10 => {
                self.death_successes = (self.death_successes + 1).min(3);
                "success".to_string()
            }
            _ => {
                self.death_failures = (self.death_failures + 1).min(3);
                "failure".to_string()
            }
        };
        (d, msg)
    }

    pub fn add_condition(&mut self, name: &str, rounds: Option<u32>) {
        // "0 rounds" ends when the next round starts, exactly like 1 round
        let rounds = rounds.map(|r| r.max(1));
        if let Some(c) = self.conditions.iter_mut().find(|c| c.name.eq_ignore_ascii_case(name)) {
            if c.level > 0 {
                c.level = (c.level + 1).min(6);
            }
            c.rounds_remaining = rounds.or(c.rounds_remaining);
            return;
        }
        let level = if name.eq_ignore_ascii_case("Exhaustion") { 1 } else { 0 };
        self.conditions.push(AppliedCondition {
            name: name.to_string(),
            note: String::new(),
            rounds_remaining: rounds,
            level,
        });
    }

    pub fn remove_condition(&mut self, name: &str) {
        self.conditions.retain(|c| !c.name.eq_ignore_ascii_case(name));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RunningEncounter {
    pub id: Id,
    pub name: String,
    /// The premade encounter this was started from, if any.
    pub source_encounter: Option<Id>,
    pub round: u32,
    /// Index into `combatants` of whose turn it is.
    pub turn: usize,
    pub started: bool,
    pub combatants: Vec<Combatant>,
    pub log: Vec<String>,
    /// Most recent group roll (for "undo last group").
    pub last_group: Option<Id>,
    /// What each new round did to condition timers, so going back a round can undo it.
    pub round_ticks: Vec<RoundTick>,
    /// Where each Next came from (from id, from round, to id, to round), so Back returns
    /// to exactly that turn even if it belongs to someone Next would now pass over (say,
    /// a monster that dropped on its own turn).
    pub turn_history: Vec<(Id, u32, Id, u32)>,
}

/// Condition timers changed when a round started: timers counted down, and conditions that ran out.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoundTick {
    pub round: u32,
    pub counted_down: Vec<(Id, String)>,
    pub expired: Vec<(Id, AppliedCondition)>,
    /// Where each expired condition sat in its list (same order as `expired`), so going
    /// back puts it back in place. Missing in saves from before 0.4.4.
    pub expired_at: Vec<usize>,
}

impl Default for RunningEncounter {
    fn default() -> Self {
        RunningEncounter {
            id: new_id(),
            name: "Encounter".into(),
            source_encounter: None,
            round: 1,
            turn: 0,
            started: false,
            combatants: Vec::new(),
            log: Vec::new(),
            last_group: None,
            round_ticks: Vec::new(),
            turn_history: Vec::new(),
        }
    }
}

/// Result of an automatic initiative roll, for display.
#[derive(Debug, Clone, PartialEq)]
pub struct InitRoll {
    pub combatant: Id,
    pub d20: i32,
    pub modifier: i32,
    pub source: InitSource,
    pub total: i32,
}

impl RunningEncounter {
    pub fn new(name: &str) -> Self {
        RunningEncounter {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Build a running encounter from a premade one. Duplicate names get numbers.
    pub fn from_premade(enc: &Encounter, campaign: &Campaign) -> Self {
        let mut r = RunningEncounter::new(&enc.name);
        r.source_encounter = Some(enc.id);
        for entry in &enc.entries {
            // (a hand-edited or imported file could ask for millions)
            let count = entry.count.clamp(1, MAX_COPIES);
            let group = if count > 1 && entry.group_initiative { Some(new_id()) } else { None };
            let first_new = r.combatants.len();
            for _ in 0..count {
                match &entry.source {
                    CombatantSource::AdHoc => {
                        let mut c = Combatant {
                            source: CombatantSource::AdHoc,
                            name: if entry.name.trim().is_empty() {
                                "Creature".into()
                            } else {
                                entry.name.trim().to_string()
                            },
                            ac: entry.ac,
                            max_hp: entry.max_hp,
                            current_hp: entry.max_hp,
                            ..Default::default()
                        };
                        if let Some(m) = entry.initiative_mod {
                            c.notes = format!("init {}", crate::model::fmt_mod(m));
                        }
                        r.combatants.push(c);
                    }
                    src => {
                        if campaign.core_for(src).is_some() {
                            r.add_from_campaign_as(campaign, src.clone(), entry.form);
                        }
                    }
                }
            }
            for c in &mut r.combatants[first_new..] {
                c.ally = entry.ally && !c.is_player();
                c.init_group = group;
            }
        }
        r.renumber_duplicates();
        r
    }

    /// Add `count` copies of a campaign creature (or one-off when `src` is AdHoc, using
    /// `adhoc`). Returns the new ids in order.
    pub fn add_with(&mut self, campaign: &Campaign, src: CombatantSource, adhoc: Option<(&str, Option<i32>, Option<i32>)>, opts: &AddOptions) -> Vec<Id> {
        let count = opts.count.max(1);
        let mut ids = Vec::new();
        for _ in 0..count {
            let id = match (&src, adhoc) {
                (CombatantSource::AdHoc, Some((name, hp, ac))) => Some(self.add_adhoc(if name.trim().is_empty() { "Creature" } else { name }, hp, ac)),
                (CombatantSource::AdHoc, None) => Some(self.add_adhoc("Creature", None, None)),
                (s, _) => self.add_from_campaign_as(campaign, s.clone(), opts.form),
            };
            if let Some(id) = id {
                ids.push(id);
            }
        }
        let group = if ids.len() > 1 && opts.roll_together { Some(new_id()) } else { None };
        let summoner = opts.summoned_by.and_then(|s| self.get(s).cloned());
        for id in &ids {
            if let Some(c) = self.get_mut(*id) {
                c.init_group = group;
                c.ally = (opts.ally || summoner.as_ref().map(|s| s.friendly()).unwrap_or(false)) && !c.is_player();
                if let Some(s) = &summoner {
                    c.summoned_by = Some(s.id);
                    c.initiative = s.initiative;
                    c.initiative_note = format!("summoned by {} — acts right after them", s.name);
                }
            }
        }
        if summoner.is_some() {
            self.sort_by_initiative(campaign);
        }
        ids
    }

    /// Make these combatants share initiative (keeps the first one's value, if any).
    pub fn link_initiative(&mut self, ids: &[Id]) {
        if ids.len() < 2 {
            return;
        }
        let group = new_id();
        let first = ids.iter().filter_map(|i| self.get(*i)).find_map(|c| c.initiative);
        for id in ids {
            if let Some(c) = self.get_mut(*id) {
                c.init_group = Some(group);
                if first.is_some() {
                    c.initiative = first;
                }
            }
        }
    }

    pub fn unlink_initiative(&mut self, id: Id) {
        let Some(g) = self.get(id).and_then(|c| c.init_group) else { return };
        if let Some(c) = self.get_mut(id) {
            c.init_group = None;
        }
        // a group of one isn't a group
        let left: Vec<Id> = self.combatants.iter().filter(|c| c.init_group == Some(g)).map(|c| c.id).collect();
        if left.len() == 1 {
            if let Some(c) = self.get_mut(left[0]) {
                c.init_group = None;
            }
        }
    }

    /// Everyone sharing initiative with `id` (including itself).
    pub fn group_members(&self, id: Id) -> Vec<Id> {
        match self.get(id).and_then(|c| c.init_group) {
            Some(g) => self.combatants.iter().filter(|c| c.init_group == Some(g)).map(|c| c.id).collect(),
            None => vec![id],
        }
    }

    /// Summons of a combatant, in order.
    pub fn summons_of(&self, id: Id) -> Vec<Id> {
        self.combatants.iter().filter(|c| c.summoned_by == Some(id)).map(|c| c.id).collect()
    }

    /// Place `id` in the turn order just before `before` (or last when None), setting its
    /// initiative from its new neighbours. No roll needed; works mid-combat.
    pub fn move_before(&mut self, campaign: &Campaign, id: Id, before: Option<Id>) {
        let Some(moving) = self.get(id).cloned() else { return };
        // The thing being placed is the combatant and its summons; neighbours are the
        // remaining top-level combatants (a summon stands in for its summoner).
        let top = |c: &Combatant| -> Id {
            match c.summoned_by {
                Some(s) if self.get(s).is_some() => s,
                _ => c.id,
            }
        };
        let order: Vec<Id> = self
            .combatants
            .iter()
            .filter(|c| c.id != id && c.summoned_by != Some(id))
            .map(|c| top(c))
            .fold(Vec::new(), |mut v, t| {
                if v.last() != Some(&t) {
                    v.push(t);
                }
                v
            });
        let before_top = before.and_then(|b| self.get(b)).map(|c| top(c)).filter(|t| *t != id);
        let k = match before_top {
            Some(b) => order.iter().position(|x| *x == b).unwrap_or(order.len()),
            None => order.len(),
        };
        let above = if k > 0 {
            order.get(k - 1).and_then(|i| self.get(*i)).map(|c| c.initiative)
        } else {
            None
        };
        let below = order.get(k).and_then(|i| self.get(*i)).map(|c| c.initiative);
        let init = match (above, below) {
            (Some(Some(a)), Some(Some(b))) if a - b >= 2 => Some(a - 1),
            (Some(Some(a)), Some(_)) => Some(a),
            (Some(Some(a)), None) => Some(a - 1),
            (Some(None), _) => None,
            (None, Some(Some(b))) => Some(b + 1),
            (None, Some(None)) => None,
            (None, None) => moving.initiative,
        };
        // Moving out of a shared-initiative group.
        if moving.init_group.is_some() {
            self.unlink_initiative(id);
        }
        if let Some(c) = self.get_mut(id) {
            c.initiative = init;
            c.initiative_note = "placed by hand".into();
        }
        for s in self.summons_of(id) {
            if let Some(c) = self.get_mut(s) {
                c.initiative = init;
            }
        }
        // Order everyone with the same initiative exactly as it now looks.
        let mut tied: Vec<Id> = order.iter().copied().filter(|t| self.get(*t).map(|c| c.initiative) == Some(init)).collect();
        let pos = tied
            .iter()
            .position(|t| order.iter().position(|o| o == t).unwrap_or(0) >= k)
            .unwrap_or(tied.len());
        tied.insert(pos, id);
        let n = tied.len() as i32;
        for (i, t) in tied.iter().enumerate() {
            if let Some(c) = self.get_mut(*t) {
                c.tiebreak = n - i as i32;
            }
        }
        self.sort_by_initiative(campaign);
        self.log.push(format!("{} placed in the turn order by hand", moving.name));
    }

    /// Add a PC/NPC from the campaign. Returns the new combatant's id.
    pub fn add_from_campaign(&mut self, campaign: &Campaign, src: CombatantSource) -> Option<Id> {
        self.add_from_campaign_as(campaign, src, None)
    }

    /// Add a PC/NPC fighting as one of its stat blocks (`form`; None = the main one).
    pub fn add_from_campaign_as(&mut self, campaign: &Campaign, src: CombatantSource, form: Option<Id>) -> Option<Id> {
        let core = campaign.core_for(&src)?;
        // An NPC's blank AC/HP fall back to its stat block.
        let form = form.filter(|_| campaign.is_other_form(&src, form));
        let (ac, max_hp) = campaign.combat_numbers(&src, form);
        let c = Combatant {
            source: src,
            name: core.display_name(),
            ac,
            max_hp,
            current_hp: max_hp,
            form,
            ..Default::default()
        };
        let id = c.id;
        self.combatants.push(c);
        self.renumber_duplicates();
        Some(id)
    }

    pub fn add_adhoc(&mut self, name: &str, max_hp: Option<i32>, ac: Option<i32>) -> Id {
        let c = Combatant {
            source: CombatantSource::AdHoc,
            name: name.trim().to_string(),
            max_hp,
            current_hp: max_hp,
            ac,
            ..Default::default()
        };
        let id = c.id;
        self.combatants.push(c);
        self.renumber_duplicates();
        id
    }

    /// "Goblin, Goblin" -> "Goblin 1, Goblin 2". Existing numbers are kept (so names don't
    /// shuffle after sorting); only un-numbered or clashing duplicates get a new number.
    pub fn renumber_duplicates(&mut self) {
        use std::collections::{HashMap, HashSet};
        fn split(name: &str) -> (String, Option<u32>) {
            let n = name.trim();
            match n.rsplit_once(' ') {
                Some((b, num)) if !num.is_empty() && num.chars().all(|ch| ch.is_ascii_digit()) => (b.trim().to_string(), num.parse().ok()),
                _ => (n.to_string(), None),
            }
        }
        let mut counts: HashMap<String, usize> = HashMap::new();
        for c in self.combatants.iter().filter(|c| !c.is_player()) {
            *counts.entry(split(&c.name).0.to_lowercase()).or_default() += 1;
        }
        let mut used: HashMap<String, HashSet<u32>> = HashMap::new();
        let mut needs: Vec<usize> = Vec::new();
        for (i, c) in self.combatants.iter().enumerate() {
            if c.is_player() {
                continue;
            }
            let (b, num) = split(&c.name);
            let key = b.to_lowercase();
            if counts[&key] <= 1 {
                continue;
            }
            match num {
                Some(n) if used.entry(key.clone()).or_default().insert(n) => {}
                _ => needs.push(i),
            }
        }
        for i in needs {
            let (b, _) = split(&self.combatants[i].name);
            let set = used.entry(b.to_lowercase()).or_default();
            let mut n = 1;
            while set.contains(&n) {
                n += 1;
            }
            set.insert(n);
            self.combatants[i].name = format!("{b} {n}");
        }
    }

    pub fn get(&self, id: Id) -> Option<&Combatant> {
        self.combatants.iter().find(|c| c.id == id)
    }
    pub fn get_mut(&mut self, id: Id) -> Option<&mut Combatant> {
        self.combatants.iter_mut().find(|c| c.id == id)
    }

    pub fn remove(&mut self, id: Id) {
        // Summons outlive their summoner as ordinary combatants at the same place.
        let init = self.get(id).and_then(|c| c.initiative);
        for c in self.combatants.iter_mut().filter(|c| c.summoned_by == Some(id)) {
            c.summoned_by = None;
            c.initiative = init;
        }
        if let Some(idx) = self.combatants.iter().position(|c| c.id == id) {
            let was_current = idx == self.turn;
            self.combatants.remove(idx);
            if idx < self.turn {
                self.turn -= 1;
            }
            if self.combatants.is_empty() {
                self.turn = 0;
            } else if was_current && self.started {
                // the turn passes on as if Next had been pressed: to whoever came after them,
                // or into the next round if they were last
                if idx > 0 {
                    self.turn = idx - 1;
                    self.next_turn();
                } else {
                    self.turn = 0;
                    if self.skips(0) {
                        self.next_turn();
                    }
                }
            } else if self.turn >= self.combatants.len() {
                self.turn = 0;
            }
        }
    }

    /// Initiative modifier for a combatant, and where it came from.
    pub fn init_source(&self, campaign: &Campaign, c: &Combatant) -> InitSource {
        // "init +2" in the notes: ad-hoc creatures, or one stat-block creature set apart
        let noted = c
            .notes
            .trim()
            .strip_prefix("init ")
            .and_then(|rest| rest.trim().trim_start_matches('+').parse::<i32>().ok());
        let from_template = (matches!(c.source, CombatantSource::StatBlock(_)) || campaign.is_other_form(&c.source, c.form)) && noted.is_some();
        match campaign.initiative_source_as(&c.source, c.form) {
            Some(s) if !from_template => s,
            _ => noted.map(InitSource::Assigned).unwrap_or(InitSource::None),
        }
    }

    /// Roll initiative for one combatant (d20 + modifier; Disadvantage if Incapacitated,
    /// Advantage if Invisible, per the 2024 rules).
    pub fn roll_initiative_for(&mut self, campaign: &Campaign, id: Id, rng: &mut dyn DiceRng) -> Option<InitRoll> {
        let c = self.get(id)?.clone();
        let source = self.init_source(campaign, &c);
        let a = rng.roll_die(20) as i32;
        let (d20, adv_note) = if c.has_condition("Invisible") && !c.has_condition("Incapacitated") {
            let b = rng.roll_die(20) as i32;
            (a.max(b), format!(" (Advantage: {a}, {b})"))
        } else if c.has_condition("Incapacitated") && !c.has_condition("Invisible") {
            let b = rng.roll_die(20) as i32;
            (a.min(b), format!(" (Disadvantage: {a}, {b})"))
        } else {
            (a, String::new())
        };
        let modifier = source.value();
        // 2024 rules: Exhaustion takes 2 × its level off every d20 test, initiative included
        let exhaustion = c
            .conditions
            .iter()
            .find(|x| x.name.eq_ignore_ascii_case("Exhaustion"))
            .map(|x| 2 * x.level.max(1) as i32)
            .unwrap_or(0);
        let total = d20 + modifier - exhaustion;
        let cm = self.get_mut(id)?;
        cm.initiative = Some(total);
        // a fresh roll replaces any place it was dragged to
        cm.tiebreak = 0;
        let exhaustion_note = if exhaustion > 0 {
            format!(" − {exhaustion} (Exhaustion)")
        } else {
            String::new()
        };
        cm.initiative_note = format!(
            "rolled {d20}{adv_note} {} {}{exhaustion_note} — {}",
            if modifier >= 0 { "+" } else { "−" },
            modifier.abs(),
            source.describe()
        );
        Some(InitRoll {
            combatant: id,
            d20,
            modifier,
            source,
            total,
        })
    }

    /// Auto-roll initiative for all non-player combatants (optionally only those without one).
    pub fn roll_npc_initiative(&mut self, campaign: &Campaign, only_missing: bool, rng: &mut dyn DiceRng) -> Vec<InitRoll> {
        let ids: Vec<Id> = self
            .combatants
            .iter()
            .filter(|c| !c.is_player() && c.summoned_by.is_none() && (!only_missing || c.initiative.is_none()))
            .map(|c| c.id)
            .collect();
        let mut done: std::collections::HashSet<Id> = std::collections::HashSet::new();
        let mut rolls = Vec::new();
        for id in ids {
            if done.contains(&id) {
                continue;
            }
            let members = self.group_members(id);
            if let Some(r) = self.roll_shared(campaign, &members, rng) {
                rolls.push(r);
            }
            done.extend(members);
        }
        self.sort_by_initiative(campaign);
        rolls
    }

    /// Roll once for the first combatant and give everyone in `ids` (and their summons) the
    /// same result.
    pub fn roll_shared(&mut self, campaign: &Campaign, ids: &[Id], rng: &mut dyn DiceRng) -> Option<InitRoll> {
        let first = *ids.first()?;
        let r = self.roll_initiative_for(campaign, first, rng)?;
        let note = self.get(first).map(|c| c.initiative_note.clone()).unwrap_or_default();
        let lead = self.get(first).map(|c| c.name.clone()).unwrap_or_default();
        for id in ids {
            if *id != first {
                if let Some(c) = self.get_mut(*id) {
                    c.initiative = Some(r.total);
                    c.tiebreak = 0;
                    c.initiative_note = format!("rolled together with {lead}: {note}");
                }
            }
        }
        for id in ids {
            for s in self.summons_of(*id) {
                if let Some(c) = self.get_mut(s) {
                    c.initiative = Some(r.total);
                }
            }
        }
        Some(r)
    }

    pub fn set_initiative(&mut self, id: Id, value: Option<i32>) {
        if let Some(c) = self.get_mut(id) {
            c.initiative = value;
            c.tiebreak = 0;
            c.initiative_note = match value {
                Some(_) => "entered manually".into(),
                None => String::new(),
            };
        }
    }

    /// Sort highest initiative first. Ties: higher modifier, then PCs before NPCs, then name.
    /// Combatants without initiative go last. Keeps the current turn on the same combatant.
    pub fn sort_by_initiative(&mut self, campaign: &Campaign) {
        let current = self.combatants.get(self.turn).map(|c| c.id);
        let mods: std::collections::HashMap<Id, i32> = self.combatants.iter().map(|c| (c.id, self.init_source(campaign, c).value())).collect();
        let ids: std::collections::HashSet<Id> = self.combatants.iter().map(|c| c.id).collect();
        let is_summon = |c: &Combatant| c.summoned_by.map(|s| ids.contains(&s) && s != c.id).unwrap_or(false);
        // summons act on their summoner's initiative
        let inits: std::collections::HashMap<Id, Option<i32>> = self.combatants.iter().map(|c| (c.id, c.initiative)).collect();
        for c in &mut self.combatants {
            if let Some(s) = c.summoned_by {
                if let Some(i) = inits.get(&s) {
                    c.initiative = *i;
                }
            }
        }
        let (summons, mut top): (Vec<Combatant>, Vec<Combatant>) = std::mem::take(&mut self.combatants).into_iter().partition(|c| is_summon(c));
        top.sort_by(|a, b| {
            let ia = a.initiative.map(|v| v as i64).unwrap_or(i64::MIN);
            let ib = b.initiative.map(|v| v as i64).unwrap_or(i64::MIN);
            ib.cmp(&ia)
                .then_with(|| b.tiebreak.cmp(&a.tiebreak))
                .then_with(|| mods[&b.id].cmp(&mods[&a.id]))
                .then_with(|| b.is_player().cmp(&a.is_player()))
                // keep a shared-initiative group together
                .then_with(|| a.init_group.cmp(&b.init_group))
                .then_with(|| natural_cmp(&a.name, &b.name))
        });
        // Summons go right after whoever summoned them (summons of summons too).
        let mut out: Vec<Combatant> = Vec::with_capacity(top.len() + summons.len());
        let mut pending = summons;
        fn place(c: Combatant, out: &mut Vec<Combatant>, pending: &mut Vec<Combatant>) {
            let id = c.id;
            out.push(c);
            let mine: Vec<Combatant> = {
                let (m, rest): (Vec<Combatant>, Vec<Combatant>) = std::mem::take(pending).into_iter().partition(|s| s.summoned_by == Some(id));
                *pending = rest;
                m
            };
            for s in mine {
                place(s, out, pending);
            }
        }
        for c in top {
            place(c, &mut out, &mut pending);
        }
        out.extend(pending); // (cycles — shouldn't happen)
        self.combatants = out;
        if let Some(id) = current {
            if self.started {
                self.turn = self.combatants.iter().position(|c| c.id == id).unwrap_or(0);
            }
        }
    }

    pub fn start(&mut self, campaign: &Campaign) {
        self.sort_by_initiative(campaign);
        self.started = true;
        self.round = 1;
        self.turn = 0;
        self.round_ticks.clear();
        self.log.push(format!("Combat started: {}", self.name));
        // the first creature might already be down
        if self.skips(0) {
            self.next_turn();
        }
    }

    /// Downed monsters and NPCs are passed over (PCs keep their turn for death saves).
    fn skips(&self, i: usize) -> bool {
        self.combatants
            .get(i)
            .map(|c| !c.is_player() && c.health() == HealthState::Down)
            .unwrap_or(false)
    }

    /// Who acts after the current combatant (None before combat starts or if nobody else can act).
    pub fn up_next(&self) -> Option<&Combatant> {
        if !self.started || self.combatants.is_empty() {
            return None;
        }
        let n = self.combatants.len();
        (1..=n)
            .map(|k| (self.turn + k) % n)
            .find(|i| !self.skips(*i))
            .and_then(|i| self.combatants.get(i))
    }

    /// (position of the current turn among those who act this round, how many act).
    pub fn turn_position(&self) -> (usize, usize) {
        let acting: Vec<usize> = (0..self.combatants.len()).filter(|i| !self.skips(*i) || *i == self.turn).collect();
        let pos = acting.iter().position(|i| *i == self.turn).map(|p| p + 1).unwrap_or(0);
        (pos, acting.len())
    }

    pub fn current(&self) -> Option<&Combatant> {
        if !self.started {
            return None;
        }
        self.combatants.get(self.turn)
    }

    /// Advance to the next combatant. Skips combatants that are down (0 HP, not PCs).
    /// Condition durations tick down when a new round starts.
    pub fn next_turn(&mut self) {
        if self.combatants.is_empty() {
            return;
        }
        if !self.started {
            self.started = true;
            self.round = self.round.max(1);
            self.turn = 0;
            if !self.skips(0) {
                return;
            }
        }
        let n = self.combatants.len();
        if self.turn >= n {
            self.turn = n - 1;
        }
        let from = (self.combatants[self.turn].id, self.round);
        for _ in 0..n {
            self.turn += 1;
            if self.turn >= n {
                self.turn = 0;
                self.round += 1;
                self.tick_conditions();
                self.log.push(format!("Round {}", self.round));
            }
            if !self.skips(self.turn) {
                break;
            }
        }
        self.turn_history.push((from.0, from.1, self.combatants[self.turn].id, self.round));
        if self.turn_history.len() > 200 {
            self.turn_history.remove(0);
        }
    }

    /// Step back one turn (passing over downed monsters, like Next). Going back past the top
    /// of a round returns to the previous round and puts back the condition timers that the
    /// new round had counted down.
    pub fn prev_turn(&mut self) {
        if self.combatants.is_empty() || !self.started {
            return;
        }
        let n = self.combatants.len();
        if self.turn >= n {
            self.turn = n - 1;
        }
        // straight back to where the last Next came from, when nothing has moved since
        let here = (self.combatants[self.turn].id, self.round);
        if let Some(&(from_id, from_round, to_id, to_round)) = self.turn_history.last() {
            if (to_id, to_round) == here && from_round <= self.round {
                if let Some(idx) = self.combatants.iter().position(|c| c.id == from_id) {
                    self.turn_history.pop();
                    while self.round > from_round.max(1) {
                        self.untick_conditions();
                        self.log.push(format!("Back to round {}", self.round - 1));
                        self.round -= 1;
                    }
                    self.turn = idx;
                    return;
                }
            }
        }
        self.turn_history.clear();
        let (start_round, start_turn) = (self.round, self.turn);
        for _ in 0..n {
            if self.turn == 0 {
                if self.round <= 1 {
                    // nowhere further back: stay on the first creature who can act
                    if self.skips(0) {
                        self.round = start_round;
                        self.turn = start_turn;
                    }
                    break;
                }
                self.untick_conditions();
                self.log.push(format!("Back to round {}", self.round - 1));
                self.round -= 1;
                self.turn = n - 1;
            } else {
                self.turn -= 1;
            }
            if !self.skips(self.turn) {
                break;
            }
        }
    }

    fn tick_conditions(&mut self) {
        let mut messages = Vec::new();
        let mut tick = RoundTick {
            round: self.round,
            ..Default::default()
        };
        for c in &mut self.combatants {
            for cond in &mut c.conditions {
                if let Some(r) = cond.rounds_remaining.as_mut() {
                    *r = r.saturating_sub(1);
                    if *r > 0 {
                        tick.counted_down.push((c.id, cond.name.clone()));
                    }
                }
            }
            let (id, name) = (c.id, c.name.clone());
            let mut at = 0;
            c.conditions.retain(|cond| {
                let keep = cond.rounds_remaining != Some(0);
                if !keep {
                    messages.push(format!("{} is no longer {}", name, cond.name));
                    tick.expired.push((
                        id,
                        AppliedCondition {
                            rounds_remaining: Some(1),
                            ..cond.clone()
                        },
                    ));
                    tick.expired_at.push(at);
                }
                at += 1;
                keep
            });
        }
        self.log.extend(messages);
        self.round_ticks.push(tick);
        if self.round_ticks.len() > 50 {
            self.round_ticks.remove(0);
        }
    }

    /// Undo what starting the current round did to condition timers.
    fn untick_conditions(&mut self) {
        let Some(pos) = self.round_ticks.iter().rposition(|t| t.round == self.round) else {
            return;
        };
        let tick = self.round_ticks.remove(pos);
        for (id, name) in tick.counted_down {
            if let Some(cond) = self.get_mut(id).and_then(|c| c.conditions.iter_mut().find(|x| x.name == name)) {
                if let Some(r) = cond.rounds_remaining.as_mut() {
                    *r += 1;
                }
            }
        }
        // put expired conditions back where they were (in the order they were removed)
        for (k, (id, cond)) in tick.expired.into_iter().enumerate() {
            let at = tick.expired_at.get(k).copied();
            if let Some(c) = self.get_mut(id) {
                if !c.has_condition(&cond.name) {
                    let at = at.unwrap_or(c.conditions.len()).min(c.conditions.len());
                    c.conditions.insert(at, cond);
                }
            }
        }
    }

    /// Apply an HP change and log it. Adds a concentration reminder if relevant.
    pub fn apply_hp(&mut self, id: Id, kind: HpChangeKind, input: &str, rng: &mut dyn DiceRng) -> Result<HpChange, DiceError> {
        let round = self.round;
        let c = self.get_mut(id).ok_or_else(|| DiceError::Invalid("combatant not found".into()))?;
        let change = c.apply_hp(kind, input, rng, round)?;
        let name = c.name.clone();
        let concentrating = c.has_condition("Concentrating");
        self.log.push(format!("{name}: {}", change.note()));
        if kind == HpChangeKind::Damage && concentrating && change.amount > 0 {
            self.log.push(format!(
                "{name} must make a DC {} Constitution save to keep concentrating",
                concentration_dc(change.amount)
            ));
        }
        Ok(change)
    }

    /// Roll once and apply to several combatants (Fireball on four goblins). Each target can
    /// take full, half, double or no damage. Returns the group id (for undo) and the total.
    pub fn apply_hp_group(&mut self, targets: &[(Id, Portion)], kind: HpChangeKind, input: &str, rng: &mut dyn DiceRng) -> Result<(Id, i32), DiceError> {
        let expr = DiceExpr::parse(input)?;
        let out = expr.roll(rng)?;
        let total = out.total.floor();
        if total < 0.0 {
            return Err(DiceError::Invalid("amount can't be negative".into()));
        }
        let total = total.min(HP_LIMIT as f64) as i32;
        let plain = input.trim().chars().all(|c| c.is_ascii_digit());
        let breakdown = format!("{} = {}", out.breakdown(), total);
        let group = new_id();
        let round = self.round;
        let mut names = Vec::new();
        for (id, portion) in targets {
            let Some(amount) = portion.apply(total) else { continue };
            let Some(c) = self.get_mut(*id) else { continue };
            c.apply_amount(kind, input.trim(), &breakdown, amount, !plain, round);
            if let Some(last) = c.hp_log.last_mut() {
                last.group = Some(group);
                last.rolled = Some(total);
                if *portion != Portion::Full {
                    last.adjustment = portion.label().to_string();
                }
            }
            let concentrating = c.has_condition("Concentrating");
            names.push(format!("{} {}", c.name, amount));
            if kind == HpChangeKind::Damage && concentrating && amount > 0 {
                let n = c.name.clone();
                self.log.push(format!(
                    "{n} must make a DC {} Constitution save to keep concentrating",
                    concentration_dc(amount)
                ));
            }
        }
        self.log.push(format!("{input} = {total} {} → {}", kind.label(), names.join(", ")));
        self.last_group = Some(group);
        Ok((group, total))
    }

    /// Undo every change from one group roll. Returns how many were undone.
    pub fn undo_group(&mut self, group: Id) -> usize {
        let mut n = 0;
        for c in &mut self.combatants {
            let ids: Vec<Id> = c.hp_log.iter().filter(|h| h.group == Some(group)).map(|h| h.id).collect();
            for id in ids {
                if c.undo_hp(id).is_some() {
                    n += 1;
                }
            }
        }
        if self.last_group == Some(group) {
            self.last_group = None;
        }
        n
    }

    /// Change which of an NPC's stat blocks a combatant fights as (None = the main one).
    /// AC and max HP follow the new stat block. Current HP stays as it is (capped at the
    /// new maximum) unless the combatant was unhurt, in which case it starts the new form
    /// at full HP. Returns false if nothing changed.
    pub fn switch_form(&mut self, campaign: &Campaign, id: Id, form: Option<Id>) -> bool {
        let Some(c) = self.get(id) else { return false };
        let src = c.source.clone();
        let form = form.filter(|_| campaign.is_other_form(&src, form));
        if c.form == form || !matches!(src, CombatantSource::Npc(_)) {
            return false;
        }
        let (ac, max_hp) = campaign.combat_numbers(&src, form);
        let label = campaign.statblock_as(&src, form).map(|s| s.name()).unwrap_or_else(|| "its own stats".into());
        let Some(c) = self.get_mut(id) else { return false };
        let unhurt = c.current_hp.is_none() || c.current_hp == c.max_hp;
        c.form = form;
        c.ac = ac;
        c.max_hp = max_hp;
        c.current_hp = match (unhurt, c.current_hp, max_hp) {
            (true, _, m) => m,
            (false, Some(cur), Some(m)) => Some(cur.min(m)),
            (false, cur, _) => cur,
        };
        let name = c.name.clone();
        self.log.push(format!("{name} changed to {label}"));
        true
    }

    // -- write-back to campaign records ------------------------------------

    /// Set max HP on a combatant; for PCs/NPCs this also updates the campaign record so
    /// the value "sticks" (as requested for the encounter manager).
    pub fn set_max_hp(&mut self, campaign: &mut Campaign, id: Id, max: Option<i32>) {
        let Some(c) = self.get_mut(id) else { return };
        let was_full = c.current_hp.is_none() || c.current_hp == c.max_hp;
        c.max_hp = max;
        if was_full {
            c.current_hp = max;
        } else if let (Some(cur), Some(m)) = (c.current_hp, max) {
            c.current_hp = Some(cur.min(m));
        }
        let src = c.source.clone();
        let form = c.form;
        // A stat block is a template: one goblin's rolled HP shouldn't change every goblin.
        // Nor should a dragon form's HP become the NPC's own.
        if matches!(src, CombatantSource::StatBlock(_)) || campaign.is_other_form(&src, form) {
            return;
        }
        if let Some(core) = campaign.core_for_mut(&src) {
            core.max_hp = max;
        }
    }

    pub fn set_initiative_mod(&mut self, campaign: &mut Campaign, id: Id, value: Option<i32>) {
        let Some(c) = self.get(id) else { return };
        let src = c.source.clone();
        // a stat block is a template: keep one creature's change on that creature
        let target = if matches!(src, CombatantSource::StatBlock(_)) || campaign.is_other_form(&src, c.form) {
            None
        } else {
            campaign.core_for_mut(&src)
        };
        match target {
            Some(core) => core.initiative_mod = value,
            None => {
                if let Some(c) = self.get_mut(id) {
                    c.notes = match value {
                        Some(v) => format!("init {}", crate::model::fmt_mod(v)),
                        None => String::new(),
                    };
                }
            }
        }
    }

    pub fn set_ac(&mut self, campaign: &mut Campaign, id: Id, ac: Option<i32>) {
        let Some(c) = self.get_mut(id) else { return };
        c.ac = ac;
        let src = c.source.clone();
        if matches!(src, CombatantSource::StatBlock(_)) || campaign.is_other_form(&src, c.form) {
            return; // one goblin's AC isn't every goblin's (nor is a wolf form's the NPC's)
        }
        if let Some(core) = campaign.core_for_mut(&src) {
            core.ac = ac;
        }
    }

    /// Characters' persistent data for a combatant (custom rolls, sheet etc.).
    pub fn core_of<'a>(&self, campaign: &'a Campaign, id: Id) -> Option<&'a CharacterCore> {
        let c = self.get(id)?;
        campaign.core_for(&c.source)
    }

    /// Save this running encounter's roster back as a premade encounter.
    pub fn to_premade(&self) -> Encounter {
        use crate::model::EncounterEntry;
        let mut enc = Encounter {
            name: self.name.clone(),
            ..Default::default()
        };
        for c in &self.combatants {
            // merge identical sources into counts
            if c.source != CombatantSource::AdHoc {
                if let Some(e) = enc.entries.iter_mut().find(|e| e.source == c.source && e.ally == c.ally && e.form == c.form) {
                    e.count += 1;
                    continue;
                }
            }
            enc.entries.push(EncounterEntry {
                source: c.source.clone(),
                count: 1,
                ally: c.ally,
                group_initiative: c.init_group.is_some(),
                name: if c.source == CombatantSource::AdHoc { c.name.clone() } else { String::new() },
                max_hp: if c.source == CombatantSource::AdHoc { c.max_hp } else { None },
                ac: if c.source == CombatantSource::AdHoc { c.ac } else { None },
                // one-off creatures keep their "init +N" note as the entry's modifier
                initiative_mod: if c.source == CombatantSource::AdHoc {
                    c.notes.trim().strip_prefix("init ").and_then(|r| r.trim().trim_start_matches('+').parse().ok())
                } else {
                    None
                },
                form: c.form,
                ..Default::default()
            });
        }
        enc
    }
}

// ---------------------------------------------------------------------------
// Difficulty (2024 DMG XP budgets)
// ---------------------------------------------------------------------------

/// Compare names so "Goblin 2" sorts before "Goblin 10".
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn split(s: &str) -> (&str, Option<u64>) {
        match s.trim().rsplit_once(' ') {
            Some((base, n)) if n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() => (base, n.parse().ok()),
            _ => (s.trim(), None),
        }
    }
    let (ba, na) = split(a);
    let (bb, nb) = split(b);
    ba.cmp(bb).then(na.cmp(&nb)).then_with(|| a.cmp(b))
}

/// XP for a challenge rating string ("1/4", "5", "0").
pub fn xp_for_cr(cr: &str) -> Option<u32> {
    let cr = cr.trim();
    Some(match cr {
        "0" => 10,
        "1/8" | "0.125" => 25,
        "1/4" | "0.25" => 50,
        "1/2" | "0.5" => 100,
        _ => {
            let n: u32 = cr.parse().ok()?;
            match n {
                1 => 200,
                2 => 450,
                3 => 700,
                4 => 1100,
                5 => 1800,
                6 => 2300,
                7 => 2900,
                8 => 3900,
                9 => 5000,
                10 => 5900,
                11 => 7200,
                12 => 8400,
                13 => 10000,
                14 => 11500,
                15 => 13000,
                16 => 15000,
                17 => 18000,
                18 => 20000,
                19 => 22000,
                20 => 25000,
                21 => 33000,
                22 => 41000,
                23 => 50000,
                24 => 62000,
                25 => 75000,
                26 => 90000,
                27 => 105000,
                28 => 120000,
                29 => 135000,
                30 => 155000,
                _ => return None,
            }
        }
    })
}

/// Per-character XP budget (Low, Moderate, High) by level, 2024 DMG.
pub fn xp_budget_per_character(level: u32) -> Option<(u32, u32, u32)> {
    const T: [(u32, u32, u32); 20] = [
        (50, 75, 100),
        (100, 150, 200),
        (150, 225, 400),
        (250, 375, 500),
        (500, 750, 1100),
        (600, 1000, 1400),
        (750, 1300, 1700),
        (1000, 1700, 2100),
        (1300, 2000, 2600),
        (1600, 2300, 3100),
        (1900, 2900, 4100),
        (2200, 3700, 4700),
        (2600, 4200, 5400),
        (2900, 4900, 6200),
        (3300, 5400, 7800),
        (3800, 6100, 9800),
        (4500, 7200, 11700),
        (5000, 8700, 14200),
        (5500, 10700, 17200),
        (6400, 13200, 22000),
    ];
    if (1..=20).contains(&level) { Some(T[level as usize - 1]) } else { None }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DifficultyEstimate {
    pub monster_xp: u32,
    pub low: u32,
    pub moderate: u32,
    pub high: u32,
    pub rating: &'static str,
    /// Monsters whose CR couldn't be read.
    pub unknown_cr: usize,
    pub party_size: usize,
}

/// Estimate encounter difficulty using the 2024 DMG XP budget method.
pub fn estimate_difficulty(enc: &Encounter, campaign: &Campaign, party_levels: &[u32]) -> DifficultyEstimate {
    let mut monster_xp: u32 = 0;
    let mut unknown = 0;
    for e in enc.entries.iter().filter(|e| !e.ally) {
        if matches!(e.source, CombatantSource::Npc(_) | CombatantSource::StatBlock(_)) {
            match campaign.cr_as(&e.source, e.form).and_then(|cr| xp_for_cr(&cr)) {
                Some(xp) => monster_xp = monster_xp.saturating_add(xp.saturating_mul(e.count.max(1))),
                None => unknown += e.count.max(1) as usize,
            }
        } else if e.source == CombatantSource::AdHoc {
            unknown += e.count.max(1) as usize;
        }
    }
    let (mut low, mut moderate, mut high) = (0, 0, 0);
    for &lvl in party_levels {
        if let Some((l, m, h)) = xp_budget_per_character(lvl) {
            low += l;
            moderate += m;
            high += h;
        }
    }
    let rating = if party_levels.is_empty() || high == 0 {
        "Unknown (no PC levels)"
    } else if monster_xp == 0 {
        "Trivial"
    } else if monster_xp <= low {
        "Low"
    } else if monster_xp <= moderate {
        "Moderate"
    } else if monster_xp <= high {
        "High"
    } else {
        "Beyond High"
    };
    DifficultyEstimate {
        monster_xp,
        low,
        moderate,
        high,
        rating,
        unknown_cr: unknown,
        party_size: party_levels.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dice::SequenceRng;
    use crate::model::{EncounterEntry, Npc, PlayerCharacter};

    fn setup() -> (Campaign, Id, Id) {
        let mut c = Campaign::new("T");
        let mut gob = Npc::default();
        gob.core.name = "Goblin".into();
        gob.core.max_hp = Some(7);
        gob.core.initiative_mod = Some(2);
        gob.core.cr = "1/4".into();
        let gid = gob.id;
        c.npcs.push(gob);
        let mut pc = PlayerCharacter::default();
        pc.core.name = "Aria".into();
        pc.core.max_hp = Some(20);
        pc.core.level = Some(3);
        let pid = pc.id;
        c.players.push(pc);
        (c, gid, pid)
    }

    #[test]
    fn premade_expansion_and_numbering() {
        let (c, gid, pid) = setup();
        let enc = Encounter {
            entries: vec![
                EncounterEntry {
                    source: CombatantSource::Npc(gid),
                    count: 3,
                    ..Default::default()
                },
                EncounterEntry {
                    source: CombatantSource::Player(pid),
                    count: 1,
                    ..Default::default()
                },
                EncounterEntry {
                    source: CombatantSource::AdHoc,
                    count: 1,
                    name: "Wolf".into(),
                    max_hp: Some(11),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let r = RunningEncounter::from_premade(&enc, &c);
        let names: Vec<_> = r.combatants.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Goblin 1", "Goblin 2", "Goblin 3", "Aria", "Wolf"]);
        assert_eq!(r.combatants[0].current_hp, Some(7));
        assert_eq!(r.combatants[4].max_hp, Some(11));
        let back = r.to_premade();
        assert_eq!(back.entries.len(), 3);
        assert_eq!(back.entries[0].count, 3);
    }

    #[test]
    fn initiative_uses_assigned_and_sorts() {
        let (c, gid, pid) = setup();
        let mut r = RunningEncounter::new("Fight");
        let g = r.add_from_campaign(&c, CombatantSource::Npc(gid)).unwrap();
        let p = r.add_from_campaign(&c, CombatantSource::Player(pid)).unwrap();
        let w = r.add_adhoc("Wolf", Some(11), Some(13));
        r.set_initiative(p, Some(12));
        let mut rng = SequenceRng::new(vec![15, 4]);
        let rolls = r.roll_npc_initiative(&c, true, &mut rng);
        assert_eq!(rolls.len(), 2);
        let gr = rolls.iter().find(|x| x.combatant == g).unwrap();
        assert_eq!(gr.total, 17);
        assert_eq!(gr.source, InitSource::Assigned(2));
        assert!(r.get(g).unwrap().initiative_note.contains("assigned"));
        let wr = rolls.iter().find(|x| x.combatant == w).unwrap();
        assert_eq!(wr.total, 4);
        assert_eq!(wr.source, InitSource::None);
        let order: Vec<_> = r.combatants.iter().map(|c| c.id).collect();
        assert_eq!(order, vec![g, p, w]);
    }

    #[test]
    fn tie_break_prefers_modifier_then_pc() {
        let (mut c, gid, pid) = setup();
        c.player_mut(pid).unwrap().core.initiative_mod = Some(2);
        let mut r = RunningEncounter::new("Fight");
        let g = r.add_from_campaign(&c, CombatantSource::Npc(gid)).unwrap();
        let p = r.add_from_campaign(&c, CombatantSource::Player(pid)).unwrap();
        r.set_initiative(g, Some(10));
        r.set_initiative(p, Some(10));
        r.sort_by_initiative(&c);
        assert_eq!(r.combatants[0].id, p);
    }

    #[test]
    fn hp_calculator_and_undo() {
        let (c, gid, _) = setup();
        let mut r = RunningEncounter::new("Fight");
        let g = r.add_from_campaign(&c, CombatantSource::Npc(gid)).unwrap();
        let mut rng = SequenceRng::new(vec![2, 3]);
        let ch = r.apply_hp(g, HpChangeKind::Damage, "2+1", &mut rng).unwrap();
        assert_eq!(ch.amount, 3);
        assert!(ch.calculated);
        assert_eq!(ch.note(), "2+1 = 3 damage");
        assert_eq!(r.get(g).unwrap().current_hp, Some(4));
        assert_eq!(r.get(g).unwrap().health(), HealthState::Healthy);
        let ch2 = r.apply_hp(g, HpChangeKind::Damage, "1d6", &mut rng).unwrap(); // rolls 2
        assert_eq!(ch2.amount, 2);
        assert_eq!(r.get(g).unwrap().current_hp, Some(2));
        assert_eq!(r.get(g).unwrap().health(), HealthState::Bloodied);
        r.apply_hp(g, HpChangeKind::Damage, "50", &mut rng).unwrap();
        assert_eq!(r.get(g).unwrap().current_hp, Some(0));
        assert_eq!(r.get(g).unwrap().health(), HealthState::Down);
        let plain = r.get(g).unwrap().hp_log.last().unwrap().clone();
        assert!(!plain.calculated);
        assert_eq!(plain.note(), "50 damage");
        r.get_mut(g).unwrap().undo_last_hp();
        assert_eq!(r.get(g).unwrap().current_hp, Some(2));
        r.apply_hp(g, HpChangeKind::Heal, "100", &mut rng).unwrap();
        assert_eq!(r.get(g).unwrap().current_hp, Some(7)); // capped
        assert!(r.apply_hp(g, HpChangeKind::Damage, "1-5", &mut rng).is_err());
        assert!(r.apply_hp(g, HpChangeKind::Damage, "banana", &mut rng).is_err());
    }

    #[test]
    fn exactly_half_is_bloodied() {
        let c = Combatant {
            max_hp: Some(10),
            current_hp: Some(5),
            ..Default::default()
        };
        assert_eq!(c.health(), HealthState::Bloodied);
        let c = Combatant {
            max_hp: Some(10),
            current_hp: Some(6),
            ..Default::default()
        };
        assert_eq!(c.health(), HealthState::Healthy);
        let c = Combatant::default();
        assert_eq!(c.health(), HealthState::Unknown);
    }

    #[test]
    fn temp_hp_absorbs_and_doesnt_stack() {
        let mut c = Combatant {
            max_hp: Some(20),
            current_hp: Some(20),
            ..Default::default()
        };
        let mut rng = SequenceRng::new(vec![1]);
        c.apply_hp(HpChangeKind::TempHp, "5", &mut rng, 1).unwrap();
        c.apply_hp(HpChangeKind::TempHp, "3", &mut rng, 1).unwrap();
        assert_eq!(c.temp_hp, 5);
        c.apply_hp(HpChangeKind::Damage, "7", &mut rng, 1).unwrap();
        assert_eq!(c.temp_hp, 0);
        assert_eq!(c.current_hp, Some(18));
    }

    #[test]
    fn undo_specific_change() {
        let mut c = Combatant {
            max_hp: Some(30),
            current_hp: Some(30),
            ..Default::default()
        };
        let mut rng = SequenceRng::new(vec![1]);
        let a = c.apply_hp(HpChangeKind::Damage, "5", &mut rng, 1).unwrap();
        c.apply_hp(HpChangeKind::Damage, "3", &mut rng, 1).unwrap();
        assert_eq!(c.current_hp, Some(22));
        c.undo_hp(a.id).unwrap();
        assert_eq!(c.current_hp, Some(27));
        assert_eq!(c.hp_log.len(), 1);
    }

    #[test]
    fn turns_rounds_and_condition_expiry() {
        let (c, gid, pid) = setup();
        let mut r = RunningEncounter::new("Fight");
        let g = r.add_from_campaign(&c, CombatantSource::Npc(gid)).unwrap();
        let p = r.add_from_campaign(&c, CombatantSource::Player(pid)).unwrap();
        r.set_initiative(g, Some(20));
        r.set_initiative(p, Some(5));
        r.start(&c);
        assert_eq!(r.current().unwrap().id, g);
        r.get_mut(p).unwrap().add_condition("Poisoned", Some(1));
        r.next_turn();
        assert_eq!(r.current().unwrap().id, p);
        assert_eq!(r.round, 1);
        r.next_turn();
        assert_eq!(r.round, 2);
        assert!(!r.get(p).unwrap().has_condition("Poisoned"));
        assert!(r.log.iter().any(|l| l.contains("no longer Poisoned")));
        // downed NPCs are skipped
        r.get_mut(g).unwrap().current_hp = Some(0);
        r.next_turn(); // -> p
        r.next_turn(); // g is down -> skip back to p, round 3
        assert_eq!(r.current().unwrap().id, p);
        assert_eq!(r.round, 3);
        assert_eq!(r.up_next().unwrap().id, p); // g is down, so p again
        r.prev_turn(); // g is down: straight back to p in round 2
        assert_eq!(r.round, 2);
        assert_eq!(r.current().unwrap().id, p);
        r.get_mut(g).unwrap().current_hp = Some(5);
        r.prev_turn();
        assert_eq!((r.round, r.current().unwrap().id), (2, g));
        r.prev_turn(); // into round 1: Poisoned (ran out at the start of round 2) comes back
        assert_eq!((r.round, r.current().unwrap().id), (1, p));
        assert!(r.get(p).unwrap().has_condition("Poisoned"));
        r.prev_turn();
        r.prev_turn(); // can't go before the first turn
        assert_eq!((r.round, r.current().unwrap().id), (1, g));
        r.next_turn();
        r.next_turn(); // round 2 again: it runs out again
        assert!(!r.get(p).unwrap().has_condition("Poisoned"));
        // a lone numbered name is left alone
        let mut solo = RunningEncounter::new("x");
        solo.add_adhoc("Agent 47", None, None);
        assert_eq!(solo.combatants[0].name, "Agent 47");
    }

    #[test]
    fn exhaustion_levels_stack() {
        let mut c = Combatant::default();
        c.add_condition("Exhaustion", None);
        c.add_condition("Exhaustion", None);
        assert_eq!(c.conditions[0].level, 2);
        for _ in 0..10 {
            c.add_condition("Exhaustion", None);
        }
        assert_eq!(c.conditions[0].level, 6);
        c.remove_condition("exhaustion");
        assert!(c.conditions.is_empty());
    }

    #[test]
    fn edits_write_back_to_npc() {
        let (mut c, gid, _) = setup();
        let mut r = RunningEncounter::new("Fight");
        let g = r.add_from_campaign(&c, CombatantSource::Npc(gid)).unwrap();
        r.set_max_hp(&mut c, g, Some(12));
        r.set_initiative_mod(&mut c, g, Some(4));
        r.set_ac(&mut c, g, Some(15));
        let npc = c.npc(gid).unwrap();
        assert_eq!(npc.core.max_hp, Some(12));
        assert_eq!(npc.core.initiative_mod, Some(4));
        assert_eq!(npc.core.ac, Some(15));
        assert_eq!(r.get(g).unwrap().current_hp, Some(12));
        // ad-hoc init mod is stored on the combatant and used by the roller
        let w = r.add_adhoc("Wolf", None, None);
        r.set_initiative_mod(&mut c, w, Some(3));
        let mut rng = SequenceRng::new(vec![10]);
        let roll = r.roll_initiative_for(&c, w, &mut rng).unwrap();
        assert_eq!(roll.total, 13);
    }

    #[test]
    fn concentration_reminder() {
        let (c, gid, _) = setup();
        let mut r = RunningEncounter::new("Fight");
        let g = r.add_from_campaign(&c, CombatantSource::Npc(gid)).unwrap();
        r.get_mut(g).unwrap().add_condition("Concentrating", None);
        let mut rng = SequenceRng::new(vec![1]);
        r.apply_hp(g, HpChangeKind::Damage, "3", &mut rng).unwrap();
        assert!(r.log.last().unwrap().contains("DC 10"));
    }

    #[test]
    fn invisible_rolls_with_advantage() {
        let (c, gid, _) = setup();
        let mut r = RunningEncounter::new("Fight");
        let g = r.add_from_campaign(&c, CombatantSource::Npc(gid)).unwrap();
        r.get_mut(g).unwrap().add_condition("Invisible", None);
        let mut rng = SequenceRng::new(vec![3, 18]);
        let roll = r.roll_initiative_for(&c, g, &mut rng).unwrap();
        assert_eq!(roll.d20, 18);
        assert!(r.get(g).unwrap().initiative_note.contains("Advantage"));
    }

    #[test]
    fn difficulty() {
        let (c, gid, _) = setup();
        assert_eq!(xp_for_cr("1/4"), Some(50));
        assert_eq!(xp_for_cr("5"), Some(1800));
        assert_eq!(xp_for_cr("x"), None);
        let enc = Encounter {
            entries: vec![EncounterEntry {
                source: CombatantSource::Npc(gid),
                count: 4,
                ..Default::default()
            }],
            ..Default::default()
        };
        let d = estimate_difficulty(&enc, &c, &[3, 3, 3, 3]);
        assert_eq!(d.monster_xp, 200);
        assert_eq!(d.low, 600);
        assert_eq!(d.rating, "Low");
        let d = estimate_difficulty(&enc, &c, &[1]);
        assert_eq!(d.rating, "Beyond High");
        let d = estimate_difficulty(&enc, &c, &[]);
        assert!(d.rating.starts_with("Unknown"));
    }

    #[test]
    fn group_damage_with_saves() {
        let (c, gid, pid) = setup();
        let mut r = RunningEncounter::new("Fight");
        let g1 = r.add_from_campaign(&c, CombatantSource::Npc(gid)).unwrap();
        let g2 = r.add_from_campaign(&c, CombatantSource::Npc(gid)).unwrap();
        let g3 = r.add_from_campaign(&c, CombatantSource::Npc(gid)).unwrap();
        let p = r.add_from_campaign(&c, CombatantSource::Player(pid)).unwrap();
        r.get_mut(p).unwrap().add_condition("Concentrating", None);
        // 2d6 rolled once: 3 + 4 = 7 -> full 7, half 3, double 14, none
        let mut rng = SequenceRng::new(vec![3, 4]);
        let (group, total) = r
            .apply_hp_group(
                &[(g1, Portion::Full), (g2, Portion::Half), (g3, Portion::None), (p, Portion::Double)],
                HpChangeKind::Damage,
                "2d6",
                &mut rng,
            )
            .unwrap();
        assert_eq!(total, 7);
        assert_eq!(r.get(g1).unwrap().current_hp, Some(0));
        assert_eq!(r.get(g2).unwrap().current_hp, Some(4));
        assert_eq!(r.get(g3).unwrap().current_hp, Some(7));
        assert_eq!(r.get(p).unwrap().current_hp, Some(6));
        assert_eq!(r.get(g2).unwrap().hp_log[0].note(), "2d6 = 7 → 3 damage (half)");
        assert!(r.log.iter().any(|l| l.contains("DC 10 Constitution")));
        assert_eq!(r.last_group, Some(group));
        assert_eq!(r.undo_group(group), 3);
        assert_eq!(r.get(g1).unwrap().current_hp, Some(7));
        assert_eq!(r.get(p).unwrap().current_hp, Some(20));
        assert!(r.apply_hp_group(&[(g1, Portion::Full)], HpChangeKind::Damage, "x", &mut rng).is_err());
    }

    #[test]
    fn death_saves() {
        let (c, _, pid) = setup();
        let mut r = RunningEncounter::new("Fight");
        let p = r.add_from_campaign(&c, CombatantSource::Player(pid)).unwrap();
        let mut rng = SequenceRng::new(vec![1]);
        let pc = r.get_mut(p).unwrap();
        assert_eq!(pc.death_state(), None);
        pc.apply_hp(HpChangeKind::Damage, "25", &mut rng, 1).unwrap();
        assert_eq!(pc.death_state(), Some(DeathState::Dying));
        // damage while down = a failed save; undo restores it
        pc.apply_hp(HpChangeKind::Damage, "3", &mut rng, 1).unwrap();
        assert_eq!(pc.death_failures, 1);
        pc.undo_last_hp();
        assert_eq!(pc.death_failures, 0);
        let mut seq = SequenceRng::new(vec![12, 15, 1, 11]);
        pc.roll_death_save(&mut seq);
        pc.roll_death_save(&mut seq);
        let (_, msg) = pc.roll_death_save(&mut seq);
        assert!(msg.contains("two failures"));
        assert_eq!((pc.death_successes, pc.death_failures), (2, 2));
        pc.roll_death_save(&mut seq);
        assert_eq!(pc.death_state(), Some(DeathState::Stable));
        // healing resets
        pc.apply_hp(HpChangeKind::Heal, "5", &mut rng, 1).unwrap();
        assert_eq!(pc.death_state(), None);
        assert_eq!((pc.death_successes, pc.death_failures), (0, 0));
        // nat 20 brings you back
        pc.apply_hp(HpChangeKind::Damage, "50", &mut rng, 1).unwrap();
        let mut twenty = SequenceRng::new(vec![20]);
        pc.roll_death_save(&mut twenty);
        assert_eq!(pc.current_hp, Some(1));
        // monsters don't make death saves
        let npc = Combatant {
            current_hp: Some(0),
            max_hp: Some(5),
            ..Default::default()
        };
        assert_eq!(npc.death_state(), None);
    }

    #[test]
    fn statblock_sources_and_fallbacks() {
        let (mut c, gid, _) = setup();
        let sb = crate::bestiary::import_json(
            r#"[{"name":"Ogre","ac":11,"hp":68,"cr":"2","initiative":-1,
            "actions":[{"name":"Greatclub","attack":{"bonus":6},"damage":[{"dice":"2d8+4","type":"bludgeoning"}]}]}]"#,
        )
        .unwrap()
        .statblocks
        .remove(0);
        let sid = sb.id;
        c.statblocks.push(sb);
        // a bare NPC that uses the ogre stat block
        let mut brute = crate::model::Npc::default();
        brute.core.name = "Grug".into();
        brute.statblock = Some(sid);
        let bid = brute.id;
        c.npcs.push(brute);
        c.npc_mut(gid).unwrap().core.cr = String::new();
        let mut r = RunningEncounter::new("x");
        let o = r.add_from_campaign(&c, CombatantSource::StatBlock(sid)).unwrap();
        let g = r.add_from_campaign(&c, CombatantSource::Npc(bid)).unwrap();
        assert_eq!(r.get(o).unwrap().max_hp, Some(68));
        assert_eq!(r.get(g).unwrap().ac, Some(11)); // falls back to stat block
        assert_eq!(r.init_source(&c, r.get(g).unwrap()), InitSource::Assigned(-1));
        let enc = Encounter {
            entries: vec![
                EncounterEntry {
                    source: CombatantSource::StatBlock(sid),
                    count: 2,
                    ..Default::default()
                },
                EncounterEntry {
                    source: CombatantSource::Npc(bid),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(estimate_difficulty(&enc, &c, &[3]).monster_xp, 450 * 3);
        let run = RunningEncounter::from_premade(&enc, &c);
        assert_eq!(run.combatants.len(), 3);
        assert_eq!(run.combatants[0].name, "Ogre 1");
        // editing one ogre in the tracker leaves the template alone
        r.set_ac(&mut c, o, Some(15));
        r.set_initiative_mod(&mut c, o, Some(3));
        assert_eq!(r.get(o).unwrap().ac, Some(15));
        assert_eq!(r.init_source(&c, r.get(o).unwrap()), InitSource::Assigned(3));
        assert_eq!(c.statblock(sid).unwrap().core.ac, Some(11));
        let o2 = r.add_from_campaign(&c, CombatantSource::StatBlock(sid)).unwrap();
        assert_eq!(r.init_source(&c, r.get(o2).unwrap()), InitSource::Assigned(-1));
        // deleting the stat block clears references
        c.delete(&crate::model::Link::StatBlock(sid));
        assert_eq!(c.npc(bid).unwrap().statblock, None);
    }

    #[test]
    fn add_many_and_roll_together() {
        let (c, gid, pid) = setup();
        let mut r = RunningEncounter::new("x");
        r.add_from_campaign(&c, CombatantSource::Player(pid));
        let ids = r.add_with(
            &c,
            CombatantSource::Npc(gid),
            None,
            &AddOptions {
                count: 3,
                roll_together: true,
                ..Default::default()
            },
        );
        assert_eq!(ids.len(), 3);
        let names: Vec<String> = ids.iter().map(|i| r.get(*i).unwrap().name.clone()).collect();
        assert_eq!(names, ["Goblin 1", "Goblin 2", "Goblin 3"]);
        // one d20 for the whole group
        let mut rng = SequenceRng::new(vec![11, 3, 3, 3]);
        let rolls = r.roll_npc_initiative(&c, false, &mut rng);
        assert_eq!(rolls.len(), 1);
        assert!(ids.iter().all(|i| r.get(*i).unwrap().initiative == Some(13)));
        assert!(r.get(ids[2]).unwrap().initiative_note.contains("together with Goblin 1"));
        // grouped creatures stay next to each other in order
        let pos: Vec<usize> = ids.iter().map(|i| r.combatants.iter().position(|c| c.id == *i).unwrap()).collect();
        assert_eq!(pos[2] - pos[0], 2);
        // split one off
        r.unlink_initiative(ids[1]);
        assert_eq!(r.group_members(ids[0]).len(), 2);
        r.unlink_initiative(ids[0]);
        assert_eq!(r.get(ids[2]).unwrap().init_group, None);
        // separate rolls now
        let mut rng = SequenceRng::new(vec![5, 6, 7]);
        assert_eq!(r.roll_npc_initiative(&c, false, &mut rng).len(), 3);
        // one-offs in bulk
        let adhoc = r.add_with(
            &c,
            CombatantSource::AdHoc,
            Some(("Skeleton", Some(13), Some(13))),
            &AddOptions {
                count: 2,
                ally: true,
                ..Default::default()
            },
        );
        assert!(adhoc.iter().all(|i| r.get(*i).unwrap().ally && r.get(*i).unwrap().friendly()));
        assert_eq!(r.get(adhoc[1]).unwrap().name, "Skeleton 2");
    }

    #[test]
    fn summons_follow_their_summoner() {
        let (c, gid, pid) = setup();
        let mut r = RunningEncounter::new("x");
        let pc = r.add_from_campaign(&c, CombatantSource::Player(pid)).unwrap();
        let g = r.add_from_campaign(&c, CombatantSource::Npc(gid)).unwrap();
        r.get_mut(pc).unwrap().initiative = Some(5);
        r.get_mut(g).unwrap().initiative = Some(15);
        r.sort_by_initiative(&c);
        let wolf = r.add_with(
            &c,
            CombatantSource::AdHoc,
            Some(("Spirit Wolf", Some(20), Some(13))),
            &AddOptions {
                summoned_by: Some(pc),
                ..Default::default()
            },
        )[0];
        let order: Vec<Id> = r.combatants.iter().map(|c| c.id).collect();
        assert_eq!(order, vec![g, pc, wolf]);
        let w = r.get(wolf).unwrap();
        assert!(w.ally, "a PC's summon is on the party's side");
        assert_eq!(w.initiative, Some(5));
        // summoner's initiative changes -> summon moves with it
        r.get_mut(pc).unwrap().initiative = Some(20);
        r.sort_by_initiative(&c);
        let order: Vec<Id> = r.combatants.iter().map(|c| c.id).collect();
        assert_eq!(order, vec![pc, wolf, g]);
        // rolling NPC initiative skips summons
        let mut rng = SequenceRng::new(vec![10]);
        assert_eq!(r.roll_npc_initiative(&c, false, &mut rng).len(), 1);
        // summoner leaves: the summon stays where it was
        r.remove(pc);
        let w = r.get(wolf).unwrap();
        assert_eq!((w.summoned_by, w.initiative), (None, Some(20)));
    }

    #[test]
    fn drag_into_turn_order() {
        let (c, gid, _) = setup();
        let mut r = RunningEncounter::new("x");
        let a = r.add_adhoc("A", None, None);
        let b = r.add_adhoc("B", None, None);
        let d = r.add_adhoc("D", None, None);
        r.get_mut(a).unwrap().initiative = Some(18);
        r.get_mut(b).unwrap().initiative = Some(12);
        r.get_mut(d).unwrap().initiative = Some(11);
        r.sort_by_initiative(&c);
        let ids = |r: &RunningEncounter| r.combatants.iter().map(|c| c.id).collect::<Vec<_>>();
        // a newcomer with no initiative, dropped between A (18) and B (12) -> 17
        let x = r.add_with(
            &c,
            CombatantSource::Npc(gid),
            None,
            &AddOptions {
                count: 1,
                ..Default::default()
            },
        )[0];
        r.move_before(&c, x, Some(b));
        assert_eq!(ids(&r), vec![a, x, b, d]);
        assert_eq!(r.get(x).unwrap().initiative, Some(17));
        // between B (12) and D (11): ties with B but goes after it
        r.move_before(&c, x, Some(d));
        assert_eq!(ids(&r), vec![a, b, x, d]);
        assert_eq!(r.get(x).unwrap().initiative, Some(12));
        // re-sorting keeps the hand-made order
        r.sort_by_initiative(&c);
        assert_eq!(ids(&r), vec![a, b, x, d]);
        // and ahead of B within the tie
        r.move_before(&c, x, Some(b));
        assert_eq!(ids(&r), vec![a, x, b, d]);
        // to the top, and to the end
        r.move_before(&c, d, Some(a));
        assert_eq!(r.get(d).unwrap().initiative, Some(19));
        assert_eq!(ids(&r)[0], d);
        r.move_before(&c, d, None);
        assert_eq!(*ids(&r).last().unwrap(), d);
        assert_eq!(r.get(d).unwrap().initiative, Some(11));
        // keeps whose turn it is
        r.started = true;
        r.turn = 0;
        r.move_before(&c, b, Some(a));
        assert_eq!(r.current().unwrap().id, a);
    }

    #[test]
    fn premade_allies_and_groups() {
        let (mut c, gid, _) = setup();
        let mut ally = Npc::default();
        ally.core.name = "Guard".into();
        ally.core.cr = "1".into();
        let aid = ally.id;
        c.npcs.push(ally);
        let enc = Encounter {
            entries: vec![
                EncounterEntry {
                    source: CombatantSource::Npc(gid),
                    count: 3,
                    ..Default::default()
                },
                EncounterEntry {
                    source: CombatantSource::Npc(aid),
                    count: 2,
                    ally: true,
                    group_initiative: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let r = RunningEncounter::from_premade(&enc, &c);
        let gobs: Vec<&Combatant> = r.combatants.iter().filter(|x| x.name.starts_with("Goblin")).collect();
        assert!(gobs.iter().all(|g| g.init_group.is_some() && g.init_group == gobs[0].init_group && !g.ally));
        let guards: Vec<&Combatant> = r.combatants.iter().filter(|x| x.name.starts_with("Guard")).collect();
        assert!(guards.iter().all(|g| g.ally && g.init_group.is_none()));
        // allies don't count towards difficulty
        assert_eq!(estimate_difficulty(&enc, &c, &[1]).monster_xp, 3 * 50);
        // and survive a round-trip back to a premade
        let back = r.to_premade();
        assert!(back.entries.iter().any(|e| e.ally && e.count == 2));
        assert!(back.entries.iter().any(|e| !e.ally && e.count == 3 && e.group_initiative));
    }

    #[test]
    fn natural_name_order() {
        assert_eq!(natural_cmp("Goblin 2", "Goblin 10"), std::cmp::Ordering::Less);
        assert_eq!(natural_cmp("Ant", "Goblin 1"), std::cmp::Ordering::Less);
    }

    #[test]
    fn remove_keeps_turn_valid() {
        let mut r = RunningEncounter::new("x");
        let a = r.add_adhoc("A", None, None);
        let b = r.add_adhoc("B", None, None);
        r.started = true;
        r.turn = 1;
        r.remove(a);
        assert_eq!(r.turn, 0);
        assert_eq!(r.current().unwrap().id, b);
        r.remove(b);
        assert!(r.current().is_none());
    }

    #[test]
    fn going_back_restores_condition_timers() {
        let (c, gid, pid) = setup();
        let mut r = RunningEncounter::new("Fight");
        let g = r.add_from_campaign(&c, CombatantSource::Npc(gid)).unwrap();
        let p = r.add_from_campaign(&c, CombatantSource::Player(pid)).unwrap();
        r.set_initiative(g, Some(20));
        r.set_initiative(p, Some(5));
        r.start(&c);
        r.get_mut(g).unwrap().add_condition("Restrained", Some(3));
        assert_eq!(r.turn_position(), (1, 2));
        r.next_turn();
        r.next_turn(); // round 2: 2 left
        r.next_turn();
        r.next_turn(); // round 3: 1 left
        let left = |r: &RunningEncounter| r.get(g).unwrap().conditions[0].rounds_remaining;
        assert_eq!(left(&r), Some(1));
        r.prev_turn(); // round 2, p
        assert_eq!(left(&r), Some(2));
        r.prev_turn();
        r.prev_turn(); // round 1, p
        assert_eq!((r.round, left(&r)), (1, Some(3)));
        assert_eq!(r.current().unwrap().id, p);
        assert_eq!(r.up_next().unwrap().id, g);
    }

    fn block(json: &str) -> crate::bestiary::StatBlock {
        crate::bestiary::import_json(json).unwrap().statblocks.remove(0)
    }

    /// An NPC (own AC 15, no HP) with a main "Mage" stat block and a "Dragon" form.
    fn shapeshifter() -> (Campaign, Id, Id, Id) {
        let (mut c, _, _) = setup();
        let mage = block(r#"[{"name":"Mage","ac":12,"hp":40,"cr":"6","initiative":2}]"#);
        let dragon = block(r#"[{"name":"Dragon","ac":18,"hp":180,"cr":"10","initiative":0}]"#);
        let (mid, did) = (mage.id, dragon.id);
        c.statblocks.push(mage);
        c.statblocks.push(dragon);
        let mut n = crate::model::Npc::default();
        n.core.name = "Vex".into();
        n.core.ac = Some(15);
        n.add_statblock(mid);
        n.add_statblock(did);
        let nid = n.id;
        c.npcs.push(n);
        (c, nid, mid, did)
    }

    #[test]
    fn npc_statblock_list() {
        let (a, b, d) = (new_id(), new_id(), new_id());
        let mut n = crate::model::Npc::default();
        n.add_statblock(a);
        n.add_statblock(b);
        n.add_statblock(b);
        assert_eq!((n.statblock, n.other_statblocks.clone()), (Some(a), vec![b]));
        n.set_main_statblock(b);
        assert_eq!(n.statblock_ids(), vec![b, a]);
        n.replace_statblock(a, d);
        assert_eq!(n.statblock_ids(), vec![b, d]);
        n.replace_statblock(d, b); // already has b: just drop d
        assert_eq!(n.statblock_ids(), vec![b]);
        n.add_statblock(a);
        n.remove_statblock(b); // main removed: the next one takes over
        assert_eq!((n.statblock, n.other_statblocks.len()), (Some(a), 0));
        assert_eq!(n.retain_statblocks(|_| false), 1);
        assert_eq!(n.statblock, None);
    }

    #[test]
    fn npc_forms_in_combat() {
        let (mut c, nid, _mid, did) = shapeshifter();
        let src = CombatantSource::Npc(nid);
        // main form: the NPC's own AC wins, blanks come from the Mage
        assert_eq!(c.combat_numbers(&src, None), (Some(15), Some(40)));
        // other form: the dragon's numbers win
        assert_eq!(c.combat_numbers(&src, Some(did)), (Some(18), Some(180)));
        assert_eq!(c.cr_as(&src, Some(did)).as_deref(), Some("10"));
        assert_eq!(c.cr_as(&src, None).as_deref(), Some("6"));
        // a stat block the NPC doesn't use is ignored
        assert_eq!(c.combat_numbers(&src, Some(new_id())), (Some(15), Some(40)));

        let mut r = RunningEncounter::new("x");
        let v = r.add_from_campaign(&c, src.clone()).unwrap();
        assert_eq!(r.init_source(&c, r.get(v).unwrap()), InitSource::Assigned(2));
        // hurt, then change into the dragon: HP stays, AC and max follow the form
        r.get_mut(v).unwrap().current_hp = Some(25);
        assert!(r.switch_form(&c, v, Some(did)));
        let cv = r.get(v).unwrap();
        assert_eq!((cv.ac, cv.max_hp, cv.current_hp, cv.form), (Some(18), Some(180), Some(25), Some(did)));
        assert_eq!(r.init_source(&c, cv), InitSource::Assigned(0));
        assert!(r.log.last().unwrap().contains("Dragon"));
        assert!(!r.switch_form(&c, v, Some(did)));
        // editing the dragon's AC in the tracker doesn't touch the NPC's own record
        r.set_ac(&mut c, v, Some(19));
        r.set_max_hp(&mut c, v, Some(200));
        assert_eq!((c.npc(nid).unwrap().core.ac, c.npc(nid).unwrap().core.max_hp), (Some(15), None));
        // back to the main form: HP capped at the new maximum
        r.get_mut(v).unwrap().current_hp = Some(120);
        r.switch_form(&c, v, None);
        let cv = r.get(v).unwrap();
        assert_eq!((cv.ac, cv.max_hp, cv.current_hp), (Some(15), Some(40), Some(40)));
        // an unhurt combatant starts the new form at full HP
        r.switch_form(&c, v, Some(did));
        assert_eq!(r.get(v).unwrap().current_hp, Some(180));
    }

    #[test]
    fn encounter_entries_remember_the_form() {
        let (mut c, nid, _mid, did) = shapeshifter();
        let enc = Encounter {
            entries: vec![EncounterEntry {
                source: CombatantSource::Npc(nid),
                form: Some(did),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(estimate_difficulty(&enc, &c, &[5]).monster_xp, 5900);
        let mut run = RunningEncounter::from_premade(&enc, &c);
        assert_eq!(run.combatants[0].form, Some(did));
        assert_eq!(run.combatants[0].max_hp, Some(180));
        let back = run.to_premade();
        assert_eq!(back.entries[0].form, Some(did));
        // adding through the add dialog
        let ids = run.add_with(
            &c,
            CombatantSource::Npc(nid),
            None,
            &AddOptions {
                count: 1,
                form: Some(did),
                ..Default::default()
            },
        );
        assert_eq!(run.get(ids[0]).unwrap().ac, Some(18));
        // deleting the dragon stat block drops it everywhere
        c.encounters.push(enc);
        c.active_encounter = Some(run);
        c.delete(&crate::model::Link::StatBlock(did));
        assert_eq!(c.npc(nid).unwrap().other_statblocks, Vec::<Id>::new());
        assert_eq!(c.encounters[0].entries[0].form, None);
        assert!(c.active_encounter.as_ref().unwrap().combatants.iter().all(|x| x.form.is_none()));
    }

    #[test]
    fn forms_survive_save_and_older_files() {
        let (c, nid, mid, did) = shapeshifter();
        let json = serde_json::to_string(&c).unwrap();
        let back: Campaign = serde_json::from_str(&json).unwrap();
        assert_eq!(back.npc(nid).unwrap().statblock_ids(), vec![mid, did]);
        // the main one is still stored where older versions of the app look for it
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let npc = v["npcs"].as_array().unwrap().iter().find(|n| n["core"]["name"] == "Vex").unwrap();
        assert_eq!(npc["statblock"], serde_json::json!(mid.to_string()));
        // an older file without the list loads with no other forms
        let old = r#"{"id":"6c5c9c8e-0000-4000-8000-000000000001","statblock":"6c5c9c8e-0000-4000-8000-000000000002"}"#;
        let n: crate::model::Npc = serde_json::from_str(old).unwrap();
        assert_eq!((n.statblock_ids().len(), n.other_statblocks.len()), (1, 0));
    }
}
