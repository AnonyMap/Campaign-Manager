//! Stat blocks: reusable monster / NPC game statistics, independent of NPC records.
//! An NPC can point at a stat block; encounters can use stat blocks directly.
//!
//! This module also implements the **Campaign Manager stat block import format**
//! (see `docs/statblock-import-spec.md`). The importer is deliberately lenient so files
//! produced by an AI from a PDF import cleanly: code fences, numbers written as strings,
//! "15 (natural armor)", "1/4", missing optional fields and trailing commas are accepted,
//! and every problem is reported as a warning instead of failing the whole file.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{CharacterCore, CustomRoll, Id, new_id};

pub const FORMAT_NAME: &str = "campaign-manager-statblocks";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DamageRoll {
    /// Dice notation, e.g. "2d6+3".
    pub dice: String,
    /// "slashing", "fire", …
    pub damage_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Attack {
    /// Bonus to hit, e.g. 4 for "+4 to hit".
    pub bonus: i32,
    /// "melee", "ranged" or "melee or ranged".
    pub kind: String,
    /// Reach or range text, e.g. "5 ft." or "80/320 ft.".
    pub range: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SaveInfo {
    /// "STR", "DEX", "CON", "INT", "WIS" or "CHA".
    pub ability: String,
    pub dc: i32,
}

/// A trait, action, bonus action, reaction or legendary action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Feature {
    pub name: String,
    /// Full rules text as written.
    pub text: String,
    pub attack: Option<Attack>,
    pub save: Option<SaveInfo>,
    pub damage: Vec<DamageRoll>,
    /// "3/Day", "Recharge 5–6", "Costs 2 Actions", …
    pub usage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StatBlock {
    pub id: Id,
    /// Name, AC, HP, speed, abilities, CR, initiative, passive Perception, rolls, notes,
    /// images. `class_or_type` holds the creature type ("Humanoid (Goblinoid)").
    pub core: CharacterCore,
    pub size: String,
    pub alignment: String,
    /// Text after the AC number, e.g. "natural armor".
    pub ac_note: String,
    pub saves: String,
    pub skills: String,
    pub senses: String,
    pub languages: String,
    pub damage_vulnerabilities: String,
    pub damage_resistances: String,
    pub damage_immunities: String,
    pub condition_immunities: String,
    pub proficiency_bonus: Option<i32>,
    pub traits: Vec<Feature>,
    pub actions: Vec<Feature>,
    pub bonus_actions: Vec<Feature>,
    pub reactions: Vec<Feature>,
    pub legendary_actions: Vec<Feature>,
    /// Intro paragraph for legendary actions ("uses: 3 …").
    pub legendary_intro: String,
    /// Where it came from: "SRD 5.2", "Essential NPCs p. 12", …
    pub source: String,
    /// The group it belongs to in the Bestiary ("Essential NPCs", "SRD 5.2"); empty for
    /// your own. Imports go into a collection so they don't mix with homebrew.
    pub collection: String,
}

/// Collection label for stat blocks you made yourself.
pub const MY_COLLECTION: &str = "My stat blocks";

impl Default for StatBlock {
    fn default() -> Self {
        StatBlock {
            id: new_id(),
            core: CharacterCore::default(),
            size: String::new(),
            alignment: String::new(),
            ac_note: String::new(),
            saves: String::new(),
            skills: String::new(),
            senses: String::new(),
            languages: String::new(),
            damage_vulnerabilities: String::new(),
            damage_resistances: String::new(),
            damage_immunities: String::new(),
            condition_immunities: String::new(),
            proficiency_bonus: None,
            traits: Vec::new(),
            actions: Vec::new(),
            bonus_actions: Vec::new(),
            reactions: Vec::new(),
            legendary_actions: Vec::new(),
            legendary_intro: String::new(),
            source: String::new(),
            collection: String::new(),
        }
    }
}

impl StatBlock {
    /// The collection shown in the Bestiary (stat blocks without one are "My stat blocks";
    /// SRD copies from before collections existed count as "SRD 5.2").
    pub fn collection_label(&self) -> String {
        let c = self.collection.trim();
        if !c.is_empty() {
            c.to_string()
        } else if self.source.trim() == "SRD 5.2" {
            "SRD 5.2".to_string()
        } else {
            MY_COLLECTION.to_string()
        }
    }

    pub fn name(&self) -> String {
        self.core.display_name()
    }

    /// "Medium Humanoid (Goblinoid), Neutral Evil"
    pub fn subtitle(&self) -> String {
        let mut s = String::new();
        if !self.size.trim().is_empty() {
            s.push_str(&title_case(self.size.trim()));
            s.push(' ');
        }
        s.push_str(self.core.class_or_type.trim());
        if !self.alignment.trim().is_empty() {
            if !s.trim().is_empty() {
                s.push_str(", ");
            }
            s.push_str(&title_case(self.alignment.trim()));
        }
        s.trim().to_string()
    }

    /// Every feature list with its heading, for display.
    pub fn sections(&self) -> [(&'static str, &Vec<Feature>); 5] {
        [
            ("Traits", &self.traits),
            ("Actions", &self.actions),
            ("Bonus Actions", &self.bonus_actions),
            ("Reactions", &self.reactions),
            ("Legendary Actions", &self.legendary_actions),
        ]
    }

    /// Rolls implied by structured attacks and damage: "Scimitar (to hit)" = 1d20+4,
    /// "Scimitar damage" = 1d6+2.
    pub fn derived_rolls(&self) -> Vec<CustomRoll> {
        let mut out = Vec::new();
        for (_, list) in self.sections() {
            for f in list {
                if let Some(a) = &f.attack {
                    let n = if a.bonus >= 0 {
                        format!("1d20+{}", a.bonus)
                    } else {
                        format!("1d20{}", a.bonus)
                    };
                    out.push(CustomRoll {
                        label: format!("{} (to hit)", f.name),
                        notation: n,
                        ..Default::default()
                    });
                }
                let dmg: Vec<&str> = f.damage.iter().map(|d| d.dice.trim()).filter(|d| !d.is_empty()).collect();
                if !dmg.is_empty() {
                    let label = if f.save.is_some() && f.attack.is_none() {
                        format!("{} damage (DC {} {})", f.name, f.save.as_ref().unwrap().dc, f.save.as_ref().unwrap().ability)
                    } else {
                        format!("{} damage", f.name)
                    };
                    out.push(CustomRoll {
                        label,
                        notation: dmg.join("+"),
                        ..Default::default()
                    });
                }
            }
        }
        out.retain(|r| crate::dice::DiceExpr::parse(&r.notation).is_ok());
        out
    }

    /// Fill `core.custom_rolls` from structured actions (keeps any rolls already there).
    pub fn ensure_rolls(&mut self) {
        for r in self.derived_rolls() {
            if !self.core.custom_rolls.iter().any(|x| x.label.eq_ignore_ascii_case(&r.label)) {
                self.core.custom_rolls.push(r);
            }
        }
    }
}

fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Import (lenient JSON → StatBlock)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ImportReport {
    pub statblocks: Vec<StatBlock>,
    /// Problems that didn't stop the import (shown to the user).
    pub warnings: Vec<String>,
    /// The file's own "source" label, if any.
    pub source: String,
    /// The file's "collection" (else its "source"): where the stat blocks will be filed.
    pub collection: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("not valid JSON: {0}")]
    Json(String),
    #[error("no stat blocks found (expected a list, or an object with a \"statblocks\" list)")]
    Empty,
}

/// Remove ``` fences, a leading "json" tag and trailing commas — common in AI output.
pub fn clean_json_text(text: &str) -> String {
    let mut t = text.trim().trim_start_matches('\u{feff}').to_string();
    // AI replies often wrap the JSON in prose and a ``` fence: keep what's inside the fence.
    // (Text that already starts as JSON is left alone: "```" inside it is just a string.)
    let starts_as_json = t.starts_with('{') || t.starts_with('[');
    if let Some(start) = t.find("```").filter(|_| !starts_as_json) {
        let after = &t[start + 3..];
        if let Some(nl) = after.find('\n') {
            let body = &after[nl + 1..];
            t = match body.rfind("```") {
                Some(end) => body[..end].to_string(),
                None => body.to_string(),
            };
        }
    }
    // Otherwise drop any chatter before the first { or [ and after the last } or ].
    if let (Some(a), Some(b)) = (t.find(['{', '[']), t.rfind(['}', ']'])) {
        if b > a {
            t = t[a..=b].to_string();
        }
    }
    // strip trailing commas before } or ] (outside strings)
    let mut out = String::with_capacity(t.len());
    let chars: Vec<char> = t.chars().collect();
    let mut in_str = false;
    let mut esc = false;
    for (i, &c) in chars.iter().enumerate() {
        if in_str {
            out.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push(c);
            continue;
        }
        if c == ',' {
            let next = chars[i + 1..].iter().find(|ch| !ch.is_whitespace());
            if matches!(next, Some('}') | Some(']')) {
                continue;
            }
        }
        out.push(c);
    }
    out.trim().to_string()
}

/// Parse an import file.
pub fn import_json(text: &str) -> Result<ImportReport, ImportError> {
    let cleaned = clean_json_text(text);
    let v: Value = serde_json::from_str(&cleaned).map_err(|e| ImportError::Json(e.to_string()))?;
    let mut report = ImportReport::default();
    let list: Vec<Value> = match &v {
        Value::Array(a) => a.clone(),
        Value::Object(o) => {
            report.source = o.get("source").map(as_text).unwrap_or_default();
            report.collection = o
                .get("collection")
                .map(as_text)
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| report.source.clone());
            if let Some(fmt) = o.get("format").and_then(|f| f.as_str()) {
                if fmt != FORMAT_NAME {
                    report.warnings.push(format!("format is \"{fmt}\", expected \"{FORMAT_NAME}\" — trying anyway"));
                }
            }
            match o.get("statblocks").or_else(|| o.get("monsters")).or_else(|| o.get("creatures")) {
                Some(Value::Array(a)) => a.clone(),
                // a single stat block object
                _ if o.contains_key("name") => vec![v.clone()],
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    if list.is_empty() {
        return Err(ImportError::Empty);
    }
    for (i, item) in list.iter().enumerate() {
        match statblock_from_value(item, &report.source) {
            Ok((mut sb, mut w)) => {
                if sb.collection.is_empty() {
                    sb.collection = report.collection.clone();
                }
                let label = sb.name();
                report.warnings.extend(w.drain(..).map(|m| format!("{label}: {m}")));
                report.statblocks.push(sb);
            }
            Err(e) => report.warnings.push(format!("entry {}: skipped — {e}", i + 1)),
        }
    }
    if report.statblocks.is_empty() {
        return Err(ImportError::Empty);
    }
    Ok(report)
}

pub(crate) fn as_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(a) => a.iter().map(as_text).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", "),
        Value::Object(o) => o.iter().map(|(k, v)| format!("{k} {}", as_text(v))).collect::<Vec<_>>().join(", "),
        Value::Null => String::new(),
    }
}

/// First integer in a value: 15, "15", "15 (natural armor)", "+4", "−1".
pub(crate) fn as_int(v: &Value) -> Option<i32> {
    match v {
        Value::Number(n) => n.as_f64().map(|f| f.round() as i32),
        Value::String(s) => first_int(s),
        Value::Object(o) => o.get("value").or_else(|| o.get("ac")).or_else(|| o.get("average")).and_then(as_int),
        Value::Array(a) => a.first().and_then(as_int),
        _ => None,
    }
}

pub(crate) fn first_int(s: &str) -> Option<i32> {
    let s = s.replace('−', "-").replace('–', "-");
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let neg = i > 0 && chars[i - 1] == '-';
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let n: i32 = chars[start..i].iter().collect::<String>().parse().ok()?;
            return Some(if neg { -n } else { n });
        }
        i += 1;
    }
    None
}

/// Text in parentheses after a number: "15 (natural armor)" -> "natural armor".
fn paren_note(s: &str) -> String {
    match (s.find('('), s.rfind(')')) {
        (Some(a), Some(b)) if b > a => s[a + 1..b].trim().to_string(),
        _ => String::new(),
    }
}

/// Dice formula inside text: "52 (8d8 + 16)" -> "8d8+16".
pub fn find_dice(s: &str) -> Option<String> {
    let t = s.replace('−', "-").replace('–', "-");
    let lower = t.to_lowercase();
    let bytes: Vec<char> = lower.chars().collect();
    for i in 0..bytes.len() {
        if bytes[i] == 'd' && i > 0 && bytes[i - 1].is_ascii_digit() && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            // walk back to the start of the count
            let mut start = i;
            while start > 0 && bytes[start - 1].is_ascii_digit() {
                start -= 1;
            }
            let mut end = i + 1;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            // optional " + 16" / "-1"
            let mut j = end;
            while j < bytes.len() && bytes[j] == ' ' {
                j += 1;
            }
            if j < bytes.len() && (bytes[j] == '+' || bytes[j] == '-') {
                let sign = bytes[j];
                let mut k = j + 1;
                while k < bytes.len() && bytes[k] == ' ' {
                    k += 1;
                }
                let ns = k;
                while k < bytes.len() && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                if k > ns && !(k < bytes.len() && bytes[k] == 'd') {
                    let num: String = bytes[ns..k].iter().collect();
                    let base: String = bytes[start..end].iter().collect();
                    return Some(format!("{base}{sign}{num}"));
                }
            }
            return Some(bytes[start..end].iter().collect());
        }
    }
    None
}

pub(crate) fn get<'a>(o: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    for k in keys {
        if let Some(v) = o.get(*k) {
            if !v.is_null() {
                return Some(v);
            }
        }
        // case-insensitive fallback
        if let Some((_, v)) = o.iter().find(|(kk, _)| kk.eq_ignore_ascii_case(k)) {
            if !v.is_null() {
                return Some(v);
            }
        }
    }
    None
}

pub(crate) fn text_of(o: &serde_json::Map<String, Value>, keys: &[&str]) -> String {
    get(o, keys).map(as_text).unwrap_or_default()
}

fn normalise_ability(a: &str) -> String {
    let a = a.trim().to_lowercase();
    let short = match a.get(..3).unwrap_or(&a) {
        "str" => "STR",
        "dex" => "DEX",
        "con" => "CON",
        "int" => "INT",
        "wis" => "WIS",
        "cha" => "CHA",
        _ => return a.to_uppercase(),
    };
    short.into()
}

/// Read a CR given as "1/4", 0.25, "2 (XP 450; PB +2)".
fn cr_text(v: &Value) -> String {
    let s = match v {
        Value::Number(n) => {
            let f = n.as_f64().unwrap_or(0.0);
            return match f {
                x if (x - 0.125).abs() < 1e-6 => "1/8".into(),
                x if (x - 0.25).abs() < 1e-6 => "1/4".into(),
                x if (x - 0.5).abs() < 1e-6 => "1/2".into(),
                x => crate::dice::fmt_num(x),
            };
        }
        other => as_text(other),
    };
    let s = s.trim();
    let head: String = s.chars().take_while(|c| c.is_ascii_digit() || *c == '/' || *c == '.').collect();
    match head.as_str() {
        "0.125" => "1/8".into(),
        "0.25" => "1/4".into(),
        "0.5" => "1/2".into(),
        "" => s.to_string(),
        h => h.trim_end_matches(".000").trim_end_matches(".0").to_string(),
    }
}

fn feature_from_value(v: &Value, warnings: &mut Vec<String>) -> Option<Feature> {
    let o = match v {
        Value::Object(o) => o,
        Value::String(s) => {
            // "Name. Text" shorthand
            let (n, t) = s.split_once(". ").unwrap_or(("", s));
            return Some(Feature {
                name: n.to_string(),
                text: t.to_string(),
                ..Default::default()
            });
        }
        _ => return None,
    };
    let mut f = Feature {
        name: text_of(o, &["name", "title"]),
        text: text_of(o, &["text", "desc", "description", "entries"]),
        usage: text_of(o, &["usage", "uses", "recharge"]),
        ..Default::default()
    };
    if let Some(a) = get(o, &["attack"]) {
        match a {
            Value::Object(ao) => {
                let bonus = get(ao, &["bonus", "to_hit", "hit"]).and_then(as_int);
                match bonus {
                    Some(b) => {
                        f.attack = Some(Attack {
                            bonus: b,
                            kind: text_of(ao, &["kind", "type"]).to_lowercase(),
                            range: text_of(ao, &["range", "reach"]),
                        })
                    }
                    None => warnings.push(format!("“{}” attack has no bonus — ignored", f.name)),
                }
            }
            other => {
                if let Some(b) = as_int(other) {
                    f.attack = Some(Attack {
                        bonus: b,
                        ..Default::default()
                    });
                }
            }
        }
    } else if let Some(b) = get(o, &["attack_bonus", "to_hit", "bonus"]).and_then(as_int) {
        f.attack = Some(Attack {
            bonus: b,
            ..Default::default()
        });
    }
    if let Some(s) = get(o, &["save"]) {
        if let Value::Object(so) = s {
            let dc = get(so, &["dc"]).and_then(as_int);
            let ab = text_of(so, &["ability", "stat"]);
            if let Some(dc) = dc {
                f.save = Some(SaveInfo {
                    ability: normalise_ability(&ab),
                    dc,
                });
            }
        }
    } else if let Some(dc) = get(o, &["save_dc", "dc"]).and_then(as_int) {
        f.save = Some(SaveInfo {
            ability: normalise_ability(&text_of(o, &["save_ability"])),
            dc,
        });
    }
    if let Some(d) = get(o, &["damage"]) {
        let items: Vec<Value> = match d {
            Value::Array(a) => a.clone(),
            other => vec![other.clone()],
        };
        for item in items {
            let (dice, ty) = match &item {
                Value::Object(dobj) => (text_of(dobj, &["dice", "roll", "formula"]), text_of(dobj, &["type", "damage_type"])),
                other => {
                    let t = as_text(other);
                    (t.clone(), String::new())
                }
            };
            let dice_clean = find_dice(&dice).or_else(|| first_int(&dice).map(|n| n.to_string()));
            match dice_clean {
                Some(dc) if crate::dice::DiceExpr::parse(&dc).is_ok() => f.damage.push(DamageRoll {
                    dice: dc,
                    damage_type: ty.to_lowercase(),
                }),
                _ => warnings.push(format!("“{}” damage “{dice}” isn't dice notation — skipped", f.name)),
            }
        }
    }
    if f.name.is_empty() && f.text.is_empty() {
        return None;
    }
    Some(f)
}

fn features(o: &serde_json::Map<String, Value>, keys: &[&str], warnings: &mut Vec<String>) -> Vec<Feature> {
    match get(o, keys) {
        Some(Value::Array(a)) => a.iter().filter_map(|v| feature_from_value(v, warnings)).collect(),
        Some(v @ Value::Object(_)) => feature_from_value(v, warnings).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// Convert one JSON object into a stat block, collecting warnings.
pub fn statblock_from_value(v: &Value, default_source: &str) -> Result<(StatBlock, Vec<String>), String> {
    let o = v.as_object().ok_or("not an object")?;
    let mut w = Vec::new();
    let name = text_of(o, &["name"]);
    if name.is_empty() {
        return Err("missing \"name\"".into());
    }
    let mut sb = StatBlock::default();
    sb.core.name = name;
    sb.size = text_of(o, &["size"]);
    sb.core.class_or_type = text_of(o, &["type", "creature_type"]);
    sb.alignment = text_of(o, &["alignment"]);
    if let Some(ac) = get(o, &["ac", "armor_class"]) {
        sb.core.ac = as_int(ac);
        sb.ac_note = match ac {
            Value::Object(ao) => text_of(ao, &["note", "from"]),
            other => paren_note(&as_text(other)),
        };
        if sb.core.ac.is_none() {
            w.push(format!("couldn't read AC “{}”", as_text(ac)));
        }
    }
    if let Some(n) = get(o, &["ac_note", "armor_detail"]) {
        sb.ac_note = as_text(n);
    }
    if let Some(hp) = get(o, &["hp", "hit_points"]) {
        sb.core.max_hp = as_int(hp);
        let t = match hp {
            Value::Object(ho) => text_of(ho, &["formula", "dice"]),
            other => as_text(other),
        };
        if let Some(d) = find_dice(&t) {
            sb.core.hp_formula = d;
        }
    }
    if let Some(f) = get(o, &["hp_formula", "hit_dice"]) {
        if let Some(d) = find_dice(&as_text(f)) {
            sb.core.hp_formula = d;
        }
    }
    if sb.core.max_hp.is_none() && !sb.core.hp_formula.is_empty() {
        sb.core.max_hp = crate::dice::DiceExpr::parse(&sb.core.hp_formula)
            .and_then(|e| e.average())
            .ok()
            .map(|a| a.floor() as i32);
    }
    sb.core.speed = text_of(o, &["speed"]);
    // abilities: {"str":10,...} or {"strength":10} or top-level str/dex/...
    let abil_obj = get(o, &["abilities", "ability_scores", "stats"]).and_then(|v| v.as_object());
    for (i, keys) in [
        ["str", "strength"],
        ["dex", "dexterity"],
        ["con", "constitution"],
        ["int", "intelligence"],
        ["wis", "wisdom"],
        ["cha", "charisma"],
    ]
    .iter()
    .enumerate()
    {
        let val = abil_obj.and_then(|ao| get(ao, keys)).or_else(|| get(o, keys)).and_then(as_int);
        *sb.core.abilities.get_mut(i) = val;
    }
    if let Some(cr) = get(o, &["cr", "challenge", "challenge_rating"]) {
        sb.core.cr = cr_text(cr);
    }
    sb.core.initiative_mod = get(o, &["initiative", "initiative_bonus", "init"]).and_then(as_int);
    sb.core.passive_perception = get(o, &["passive_perception"]).and_then(as_int);
    sb.proficiency_bonus = get(o, &["proficiency_bonus", "pb"]).and_then(as_int);
    sb.saves = text_of(o, &["saves", "saving_throws"]);
    sb.skills = text_of(o, &["skills"]);
    sb.senses = text_of(o, &["senses"]);
    if sb.core.passive_perception.is_none() {
        let s = sb.senses.to_lowercase();
        if let Some(i) = s.find("passive perception") {
            sb.core.passive_perception = first_int(&s[i..]);
        }
    }
    sb.languages = text_of(o, &["languages"]);
    sb.damage_vulnerabilities = text_of(o, &["damage_vulnerabilities", "vulnerabilities"]);
    sb.damage_resistances = text_of(o, &["damage_resistances", "resistances"]);
    sb.damage_immunities = text_of(o, &["damage_immunities", "immunities"]);
    sb.condition_immunities = text_of(o, &["condition_immunities"]);
    sb.traits = features(o, &["traits", "special_abilities", "features"], &mut w);
    sb.actions = features(o, &["actions"], &mut w);
    sb.bonus_actions = features(o, &["bonus_actions"], &mut w);
    sb.reactions = features(o, &["reactions"], &mut w);
    sb.legendary_actions = features(o, &["legendary_actions"], &mut w);
    sb.legendary_intro = text_of(o, &["legendary_intro", "legendary_description"]);
    sb.core.notes = text_of(o, &["notes", "description", "lore"]);
    sb.source = {
        let s = text_of(o, &["source"]);
        if s.is_empty() { default_source.to_string() } else { s }
    };
    sb.collection = text_of(o, &["collection"]);
    sb.core.tags = match get(o, &["tags"]) {
        Some(Value::Array(a)) => a.iter().map(as_text).filter(|s| !s.is_empty()).collect(),
        Some(other) => as_text(other).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        None => Vec::new(),
    };
    if sb.core.ac.is_none() {
        w.push("no AC".into());
    }
    if sb.core.max_hp.is_none() {
        w.push("no HP".into());
    }
    sb.ensure_rolls();
    Ok((sb, w))
}

/// Serialise stat blocks to the import format (for sharing a bestiary).
pub fn export_json(blocks: &[StatBlock], source: &str) -> String {
    let list: Vec<Value> = blocks.iter().map(to_value).collect();
    let doc = serde_json::json!({
        "format": FORMAT_NAME,
        "version": FORMAT_VERSION,
        "source": source,
        "statblocks": list,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_default()
}

fn to_value(sb: &StatBlock) -> Value {
    let c = &sb.core;
    let feat = |l: &Vec<Feature>| -> Value {
        Value::Array(
            l.iter()
                .map(|f| {
                    let mut m = serde_json::Map::new();
                    m.insert("name".into(), f.name.clone().into());
                    m.insert("text".into(), f.text.clone().into());
                    if !f.usage.is_empty() {
                        m.insert("usage".into(), f.usage.clone().into());
                    }
                    if let Some(a) = &f.attack {
                        m.insert("attack".into(), serde_json::json!({"bonus": a.bonus, "kind": a.kind, "range": a.range}));
                    }
                    if let Some(s) = &f.save {
                        m.insert("save".into(), serde_json::json!({"ability": s.ability, "dc": s.dc}));
                    }
                    if !f.damage.is_empty() {
                        m.insert(
                            "damage".into(),
                            Value::Array(f.damage.iter().map(|d| serde_json::json!({"dice": d.dice, "type": d.damage_type})).collect()),
                        );
                    }
                    Value::Object(m)
                })
                .collect(),
        )
    };
    let ab = |i: usize| c.abilities.get(i).map(Value::from).unwrap_or(Value::Null);
    serde_json::json!({
        "name": c.name, "size": sb.size, "type": c.class_or_type, "alignment": sb.alignment,
        "ac": c.ac, "ac_note": sb.ac_note, "hp": c.max_hp, "hp_formula": c.hp_formula, "speed": c.speed,
        "initiative": c.initiative_mod,
        "abilities": {"str": ab(0), "dex": ab(1), "con": ab(2), "int": ab(3), "wis": ab(4), "cha": ab(5)},
        "saves": sb.saves, "skills": sb.skills, "senses": sb.senses, "passive_perception": c.passive_perception,
        "languages": sb.languages,
        "damage_vulnerabilities": sb.damage_vulnerabilities, "damage_resistances": sb.damage_resistances,
        "damage_immunities": sb.damage_immunities, "condition_immunities": sb.condition_immunities,
        "cr": c.cr, "proficiency_bonus": sb.proficiency_bonus,
        "traits": feat(&sb.traits), "actions": feat(&sb.actions), "bonus_actions": feat(&sb.bonus_actions),
        "reactions": feat(&sb.reactions), "legendary_actions": feat(&sb.legendary_actions),
        "legendary_intro": sb.legendary_intro, "notes": c.notes, "tags": c.tags, "source": sb.source, "collection": sb.collection,
    })
}

/// Instructions for an AI model that turns stat blocks (PDF pages, screenshots, text) into
/// an import file. Shown and copied from the Bestiary's Import window.
pub const AI_SPEC: &str = include_str!("../../../docs/statblock-import-spec.md");

/// The bundled SRD 5.2 monsters (CC-BY-4.0), in the import format.
pub const SRD_JSON: &str = include_str!("../data/srd-5.2-monsters.json");

pub const SRD_ATTRIBUTION: &str = "This work includes material from the System Reference Document 5.2 (\"SRD 5.2\") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode. Monster data structured by the Open5e project (open5e.com).";

/// Parse the bundled SRD list.
pub fn srd_statblocks() -> Vec<StatBlock> {
    import_json(SRD_JSON)
        .map(|r| r.statblocks)
        .unwrap_or_default()
        .into_iter()
        .map(|mut s| {
            s.collection = "SRD 5.2".into();
            s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const AI_STYLE: &str = r#"```json
{
  "format": "campaign-manager-statblocks",
  "version": 1,
  "source": "Essential NPCs",
  "statblocks": [
    {
      "name": "Bandit Captain",
      "size": "Medium",
      "type": "Humanoid",
      "alignment": "Neutral",
      "ac": "15 (studded leather)",
      "hp": "52 (8d8 + 16)",
      "speed": "30 ft.",
      "abilities": {"str": 15, "dex": "16", "con": 14, "int": 14, "wis": 11, "cha": 14},
      "senses": "Passive Perception 10",
      "cr": "2",
      "actions": [
        {"name": "Multiattack", "text": "The captain makes three attacks."},
        {"name": "Scimitar", "text": "Melee Attack Roll: +5, reach 5 ft. Hit: 6 (1d6 + 3) Slashing damage.",
         "attack": {"bonus": "+5", "kind": "melee", "range": "5 ft."},
         "damage": [{"dice": "6 (1d6 + 3)", "type": "Slashing"}]},
        {"name": "Fire Breath", "text": "DEX save DC 13", "save": {"ability": "Dexterity", "dc": 13},
         "damage": [{"dice": "4d6", "type": "fire"}]},
      ],
      "reactions": [{"name": "Parry", "text": "Adds 2 to its AC against one melee attack."}],
    },
  ]
}
```"#;

    #[test]
    fn imports_messy_ai_output() {
        let r = import_json(AI_STYLE).unwrap();
        assert_eq!(r.source, "Essential NPCs");
        let sb = &r.statblocks[0];
        assert_eq!(sb.name(), "Bandit Captain");
        assert_eq!(sb.core.ac, Some(15));
        assert_eq!(sb.ac_note, "studded leather");
        assert_eq!(sb.core.max_hp, Some(52));
        assert_eq!(sb.core.hp_formula, "8d8+16");
        assert_eq!(sb.core.abilities.dex, Some(16));
        assert_eq!(sb.core.passive_perception, Some(10));
        assert_eq!(sb.core.cr, "2");
        assert_eq!(sb.actions.len(), 3);
        assert_eq!(sb.actions[1].attack.as_ref().unwrap().bonus, 5);
        assert_eq!(sb.actions[1].damage[0].dice, "1d6+3");
        assert_eq!(sb.actions[2].save.as_ref().unwrap().ability, "DEX");
        assert_eq!(sb.reactions.len(), 1);
        assert_eq!(sb.source, "Essential NPCs");
        let rolls: Vec<_> = sb.core.custom_rolls.iter().map(|r| (r.label.as_str(), r.notation.as_str())).collect();
        assert!(rolls.contains(&("Scimitar (to hit)", "1d20+5")));
        assert!(rolls.contains(&("Scimitar damage", "1d6+3")));
        assert!(rolls.contains(&("Fire Breath damage (DC 13 DEX)", "4d6")));
        assert_eq!(sb.subtitle(), "Medium Humanoid, Neutral");
    }

    #[test]
    fn imports_go_into_a_collection() {
        let r = import_json(r#"{"source": "Essential NPCs", "statblocks": [{"name": "A"}, {"name": "B", "collection": "Villains"}]}"#).unwrap();
        assert_eq!(r.collection, "Essential NPCs");
        assert_eq!(r.statblocks[0].collection_label(), "Essential NPCs");
        assert_eq!(r.statblocks[1].collection_label(), "Villains");
        let bare = import_json(r#"[{"name": "C"}]"#).unwrap();
        assert_eq!(bare.statblocks[0].collection_label(), MY_COLLECTION);
        assert!(srd_statblocks().iter().all(|s| s.collection_label() == "SRD 5.2"));
        // round-trips through export
        let again = import_json(&export_json(&r.statblocks, "x")).unwrap();
        assert_eq!(again.statblocks[1].collection, "Villains");
    }

    #[test]
    fn prose_around_json() {
        let text = "Sure! Here is the file:\n\n```json\n[{\"name\": \"Imp\", \"ac\": 13, \"hp\": 21,}]\n```\nLet me know if you need more.";
        let r = import_json(text).unwrap();
        assert_eq!(r.statblocks[0].name(), "Imp");
        let bare = "Here you go: {\"statblocks\": [{\"name\": \"Imp\"}]} Enjoy";
        assert_eq!(import_json(bare).unwrap().statblocks.len(), 1);
        assert!(AI_SPEC.contains(FORMAT_NAME));
    }

    #[test]
    fn spec_example_imports_cleanly() {
        // The worked example in the AI spec must import without warnings.
        let start = AI_SPEC.find("<!-- example-start -->").expect("example marker");
        let end = AI_SPEC.find("<!-- example-end -->").expect("example end marker");
        let r = import_json(&AI_SPEC[start..end]).unwrap();
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        assert_eq!(r.statblocks.len(), 2);
        let mage = &r.statblocks[1];
        assert!(mage.derived_rolls().iter().any(|x| x.label.contains("DC")));
    }

    #[test]
    fn lenient_shapes() {
        // bare array, CR as number, single object, abilities at top level
        let r = import_json(r#"[{"name":"Kobold","cr":0.125,"ac":12,"hp":5,"str":7,"dex":15}]"#).unwrap();
        assert_eq!(r.statblocks[0].core.cr, "1/8");
        assert_eq!(r.statblocks[0].core.abilities.dex, Some(15));
        let r = import_json(r#"{"name":"Rat","hit_points":"1 (1d4 - 1)","armor_class":10}"#).unwrap();
        assert_eq!(r.statblocks[0].core.hp_formula, "1d4-1");
        let r = import_json(r#"[{"name":"Ghost","hp_formula":"10d8"}]"#).unwrap();
        assert_eq!(r.statblocks[0].core.max_hp, Some(45));
        // bad entries are skipped with warnings, good ones kept
        let r = import_json(r#"[{"name":"Ok","ac":10,"hp":1}, {"ac": 3}, 5]"#).unwrap();
        assert_eq!(r.statblocks.len(), 1);
        assert!(r.warnings.iter().any(|w| w.contains("missing \"name\"")));
        assert!(import_json("[]").is_err());
        assert!(import_json("not json").is_err());
    }

    #[test]
    fn helpers() {
        assert_eq!(find_dice("52 (8d8 + 16)").as_deref(), Some("8d8+16"));
        assert_eq!(find_dice("1d4 − 1").as_deref(), Some("1d4-1"));
        assert_eq!(find_dice("Hit: 7 (2d6) fire").as_deref(), Some("2d6"));
        assert_eq!(find_dice("no dice"), None);
        assert_eq!(first_int("−2"), Some(-2));
        assert_eq!(clean_json_text("[1,2,]"), "[1,2]");
        assert_eq!(clean_json_text(r#"{"a":"x,]"}"#), r#"{"a":"x,]"}"#);
    }

    #[test]
    fn export_roundtrip() {
        let r = import_json(AI_STYLE).unwrap();
        let text = export_json(&r.statblocks, "Test");
        let back = import_json(&text).unwrap();
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
        let (a, b) = (&r.statblocks[0], &back.statblocks[0]);
        assert_eq!(a.core.ac, b.core.ac);
        assert_eq!(a.core.hp_formula, b.core.hp_formula);
        assert_eq!(a.actions, b.actions);
        assert_eq!(a.core.custom_rolls.len(), b.core.custom_rolls.len());
    }

    #[test]
    fn bundled_srd_parses() {
        let r = import_json(SRD_JSON).unwrap();
        assert!(r.statblocks.len() > 300, "{}", r.statblocks.len());
        let names: Vec<String> = r.statblocks.iter().map(|s| s.name()).collect();
        for n in ["Goblin Warrior", "Adult Red Dragon", "Bandit Captain"] {
            assert!(names.iter().any(|x| x == n), "missing {n}");
        }
        assert!(r.statblocks.iter().all(|s| s.core.ac.is_some() && s.core.max_hp.is_some()));
        let gob = r.statblocks.iter().find(|s| s.name() == "Goblin Warrior").unwrap();
        assert!(gob.core.custom_rolls.iter().any(|r| r.notation.starts_with("1d20+")));
        // every derived roll is valid dice notation
        for s in &r.statblocks {
            for roll in &s.core.custom_rolls {
                assert!(crate::dice::DiceExpr::parse(&roll.notation).is_ok(), "{}: {}", s.name(), roll.notation);
            }
        }
    }
}
