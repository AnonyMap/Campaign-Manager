//! Items: the campaign's item library (mundane gear, magic items, treasure), what characters
//! carry, coins, and a lenient import format (so an AI can convert a book's item lists).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::bestiary::{as_text, clean_json_text, find_dice, get, text_of};
use crate::model::{CustomRoll, Id, new_id};

pub const FORMAT_NAME: &str = "campaign-manager-items";
pub const FORMAT_VERSION: u32 = 1;
/// Collection label for items you made yourself.
pub const MY_COLLECTION: &str = "My items";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum Rarity {
    /// Not magical.
    #[default]
    Mundane,
    Common,
    Uncommon,
    Rare,
    VeryRare,
    Legendary,
    Artifact,
}

impl Rarity {
    pub const ALL: [Rarity; 7] = [
        Rarity::Mundane,
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::VeryRare,
        Rarity::Legendary,
        Rarity::Artifact,
    ];
    pub const MAGIC: [Rarity; 6] = [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::VeryRare,
        Rarity::Legendary,
        Rarity::Artifact,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Rarity::Mundane => "Mundane",
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::VeryRare => "Very rare",
            Rarity::Legendary => "Legendary",
            Rarity::Artifact => "Artifact",
        }
    }

    /// Typical market value of a magic item of this rarity (2024 rules), in gp.
    pub fn typical_value(self) -> Option<f64> {
        match self {
            Rarity::Mundane => None,
            Rarity::Common => Some(100.0),
            Rarity::Uncommon => Some(400.0),
            Rarity::Rare => Some(4000.0),
            Rarity::VeryRare => Some(40000.0),
            Rarity::Legendary => Some(200000.0),
            Rarity::Artifact => None,
        }
    }

    pub fn parse(s: &str) -> Option<Rarity> {
        let t = s.trim().to_lowercase().replace(['-', '_'], " ");
        Some(match t.as_str() {
            "" | "mundane" | "none" | "nonmagical" | "non magical" | "standard" => Rarity::Mundane,
            "common" => Rarity::Common,
            "uncommon" => Rarity::Uncommon,
            "rare" => Rarity::Rare,
            "very rare" | "veryrare" => Rarity::VeryRare,
            "legendary" => Rarity::Legendary,
            "artifact" => Rarity::Artifact,
            "varies" => Rarity::Uncommon,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ItemKind {
    Weapon,
    Armor,
    Ammunition,
    Potion,
    Scroll,
    Wand,
    Staff,
    Rod,
    Ring,
    #[default]
    Wondrous,
    Gear,
    Tool,
    Gem,
    Art,
    TradeGood,
    Food,
    Mount,
    Vehicle,
    Other,
}

impl ItemKind {
    pub const ALL: [ItemKind; 19] = [
        ItemKind::Weapon,
        ItemKind::Armor,
        ItemKind::Ammunition,
        ItemKind::Potion,
        ItemKind::Scroll,
        ItemKind::Wand,
        ItemKind::Staff,
        ItemKind::Rod,
        ItemKind::Ring,
        ItemKind::Wondrous,
        ItemKind::Gear,
        ItemKind::Tool,
        ItemKind::Gem,
        ItemKind::Art,
        ItemKind::TradeGood,
        ItemKind::Food,
        ItemKind::Mount,
        ItemKind::Vehicle,
        ItemKind::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ItemKind::Weapon => "Weapon",
            ItemKind::Armor => "Armor",
            ItemKind::Ammunition => "Ammunition",
            ItemKind::Potion => "Potion",
            ItemKind::Scroll => "Scroll",
            ItemKind::Wand => "Wand",
            ItemKind::Staff => "Staff",
            ItemKind::Rod => "Rod",
            ItemKind::Ring => "Ring",
            ItemKind::Wondrous => "Wondrous item",
            ItemKind::Gear => "Adventuring gear",
            ItemKind::Tool => "Tool",
            ItemKind::Gem => "Gemstone",
            ItemKind::Art => "Art object",
            ItemKind::TradeGood => "Trade good",
            ItemKind::Food => "Food & drink",
            ItemKind::Mount => "Mount",
            ItemKind::Vehicle => "Vehicle",
            ItemKind::Other => "Other",
        }
    }

    pub fn parse(s: &str) -> ItemKind {
        let t = s.trim().to_lowercase().replace(['-', '_'], " ");
        // whole words (plural allowed), so "Adventuring gear" isn't a ring and "Cartography" isn't art
        let has = |w: &str| {
            t.split(|c: char| !c.is_alphanumeric())
                .any(|x| x == w || x.strip_suffix('s') == Some(w) || (w.len() > 5 && x.starts_with(w)))
        };
        if has("ammunition") || has("arrow") || has("bolt") {
            ItemKind::Ammunition
        } else if has("weapon") {
            ItemKind::Weapon
        } else if has("armor") || has("armour") || has("shield") {
            ItemKind::Armor
        } else if has("potion") || has("elixir") || has("oil") || has("philter") {
            ItemKind::Potion
        } else if has("scroll") {
            ItemKind::Scroll
        } else if has("wand") {
            ItemKind::Wand
        } else if has("staff") {
            ItemKind::Staff
        } else if has("rod") {
            ItemKind::Rod
        } else if has("ring") {
            ItemKind::Ring
        } else if has("wondrous") {
            ItemKind::Wondrous
        } else if has("tool") || has("kit") || has("instrument") {
            ItemKind::Tool
        } else if has("gem") {
            ItemKind::Gem
        } else if has("art") {
            ItemKind::Art
        } else if has("trade") {
            ItemKind::TradeGood
        } else if has("food") || has("drink") || has("meal") {
            ItemKind::Food
        } else if has("mount") || has("animal") {
            ItemKind::Mount
        } else if has("vehicle") || has("ship") || has("boat") {
            ItemKind::Vehicle
        } else if has("gear") || has("equipment") || has("pack") || has("focus") {
            ItemKind::Gear
        } else if t.is_empty() {
            ItemKind::Gear
        } else {
            ItemKind::Other
        }
    }

    /// Used up when used (2024 rules price these at half).
    pub fn consumable(self) -> bool {
        matches!(self, ItemKind::Potion | ItemKind::Scroll | ItemKind::Ammunition | ItemKind::Food)
    }
}

/// A numeric bonus an item grants: "+1 AC", "+2 attack rolls".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ItemModifier {
    /// What it modifies: "AC", "attack rolls", "damage rolls", "saving throws", "spell save DC"…
    pub label: String,
    pub value: i32,
}

impl ItemModifier {
    pub fn text(&self) -> String {
        format!("{} {}", crate::model::fmt_mod(self.value), self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Item {
    pub id: Id,
    pub name: String,
    pub kind: ItemKind,
    pub rarity: Rarity,
    /// Price in gp. None = unknown (magic items fall back to their rarity's typical value).
    pub cost_gp: Option<f64>,
    pub weight_lb: Option<f64>,
    /// "" = no attunement; otherwise the requirement ("requires attunement by a wizard").
    pub attunement: String,
    /// Weapon damage ("1d8 slashing") or armor class ("14 + Dex (max 2)").
    pub damage: String,
    pub armor_class: String,
    pub properties: String,
    /// Rules / description (markdown).
    pub description: String,
    /// Rolls the item gives its wielder ("Flame Tongue extra damage" = 2d6).
    pub rolls: Vec<CustomRoll>,
    pub modifiers: Vec<ItemModifier>,
    pub image: Option<String>,
    pub tags: Vec<String>,
    /// Where it came from: "SRD 5.2", "Xanathar's p. 12"…
    pub source: String,
    /// Library collection ("SRD 5.2", a book's name); empty = "My items".
    pub collection: String,
}

impl Default for Item {
    fn default() -> Self {
        Item {
            id: new_id(),
            name: String::new(),
            kind: ItemKind::Gear,
            rarity: Rarity::Mundane,
            cost_gp: None,
            weight_lb: None,
            attunement: String::new(),
            damage: String::new(),
            armor_class: String::new(),
            properties: String::new(),
            description: String::new(),
            rolls: Vec::new(),
            modifiers: Vec::new(),
            image: None,
            tags: Vec::new(),
            source: String::new(),
            collection: String::new(),
        }
    }
}

impl Item {
    pub fn named(name: &str, kind: ItemKind, rarity: Rarity, cost: Option<f64>) -> Item {
        Item {
            name: name.to_string(),
            kind,
            rarity,
            cost_gp: cost,
            ..Default::default()
        }
    }

    pub fn is_magic(&self) -> bool {
        self.rarity != Rarity::Mundane
    }

    /// Value in gp: its price, else its rarity's typical value (half for consumables).
    pub fn value_gp(&self) -> f64 {
        if let Some(c) = self.cost_gp.filter(|c| *c > 0.0) {
            return c;
        }
        match self.rarity.typical_value() {
            Some(v) if self.kind.consumable() => v / 2.0,
            Some(v) => v,
            None => 0.0,
        }
    }

    pub fn collection_label(&self) -> String {
        let c = self.collection.trim();
        if c.is_empty() { MY_COLLECTION.to_string() } else { c.to_string() }
    }

    /// "Uncommon wondrous item (requires attunement)"
    pub fn subtitle(&self) -> String {
        let mut s = if self.is_magic() {
            format!("{} {}", self.rarity.label(), self.kind.label().to_lowercase())
        } else {
            self.kind.label().to_string()
        };
        if !self.attunement.trim().is_empty() {
            s.push_str(&format!(" ({})", self.attunement.trim()));
        }
        s
    }

    /// Fill in rolls and modifiers from the name and description when it has none.
    pub fn derive_rolls(&mut self) {
        if self.rolls.is_empty() {
            self.rolls = suggest_rolls(self);
        }
        if self.modifiers.is_empty() {
            self.modifiers = suggest_modifiers(self);
        }
    }
}

/// "+1" from "Longsword (+1)", "+2 Shield", "Wand of the War Mage +3".
pub fn plus_bonus(name: &str) -> Option<i32> {
    let n = name.replace('−', "-");
    let b = n.as_bytes();
    for i in 0..b.len().saturating_sub(1) {
        if b[i] == b'+' && b[i + 1].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
            let v = (b[i + 1] - b'0') as i32;
            if (1..=5).contains(&v) && (i + 2 >= b.len() || !b[i + 2].is_ascii_digit()) {
                return Some(v);
            }
        }
    }
    None
}

/// Modifiers the item grants, read from "+N" in its name and "a +N bonus to …" in its text.
pub fn suggest_modifiers(item: &Item) -> Vec<ItemModifier> {
    let mut out: Vec<ItemModifier> = Vec::new();
    let mut push = |label: &str, value: i32| {
        if !out.iter().any(|m| m.label == label) {
            out.push(ItemModifier {
                label: label.to_string(),
                value,
            });
        }
    };
    if let Some(n) = plus_bonus(&item.name) {
        match item.kind {
            ItemKind::Weapon | ItemKind::Ammunition => {
                push("attack rolls", n);
                push("damage rolls", n);
            }
            ItemKind::Armor => push("AC", n),
            _ => {}
        }
    }
    // ASCII-only lowercasing keeps byte positions the same as in the original text
    let text = item.description.to_ascii_lowercase();
    let mut from = 0;
    while let Some(pos) = text[from..].find(" bonus to ") {
        let at = from + pos;
        // the "+N" just before " bonus to"
        let before = &text[..at];
        let value = before
            .rfind('+')
            .filter(|p| at - p <= 3)
            .and_then(|p| before[p + 1..].trim().parse::<i32>().ok());
        let rest = &text[at + " bonus to ".len()..];
        let end = crate::model::floor_char(rest, rest.find(['.', ';', '(']).unwrap_or(rest.len()).min(60));
        let phrase = rest[..end].split(" while ").next().unwrap_or("").split(" that ").next().unwrap_or("").trim();
        if let Some(v) = value {
            if phrase.contains("attack and damage rolls") {
                push("attack rolls", v);
                push("damage rolls", v);
            } else if phrase.starts_with("armor class") || phrase.starts_with("ac") {
                push("AC", v);
            } else if phrase.contains("saving throws") && phrase.contains("ability checks") {
                push("saving throws", v);
                push("ability checks", v);
            } else if phrase.contains("spell attack rolls") {
                push("spell attack rolls", v);
            } else if phrase.contains("saving throw") {
                push("saving throws", v);
            } else if phrase.contains("spell save dc") {
                push("spell save DC", v);
            } else if phrase.contains("attack rolls") {
                push("attack rolls", v);
            } else if phrase.contains("damage rolls") {
                push("damage rolls", v);
            } else if !phrase.is_empty() && phrase.len() <= 40 {
                push(phrase, v);
            }
        }
        from = at + 1;
    }
    out
}

/// Rolls an item gives: weapon damage (with its +N), healing, extra damage riders and
/// damage it deals ("takes 4d4 Fire damage"). At most four.
pub fn suggest_rolls(item: &Item) -> Vec<CustomRoll> {
    let mut out: Vec<CustomRoll> = Vec::new();
    let mut push = |label: String, notation: String| {
        if crate::dice::DiceExpr::parse(&notation).is_ok() && !out.iter().any(|r| r.notation == notation && r.label == label) && out.len() < 4 {
            out.push(CustomRoll {
                label,
                notation,
                ..Default::default()
            });
        }
    };
    let name = item.name.trim();
    if item.kind == ItemKind::Weapon {
        if let Some(d) = find_dice(&item.damage) {
            // "+1 Longsword", or "a +2 bonus to attack and damage rolls" in the text
            let bonus = plus_bonus(name).or_else(|| {
                let mods = if item.modifiers.is_empty() {
                    suggest_modifiers(item)
                } else {
                    item.modifiers.clone()
                };
                mods.iter().find(|m| m.label == "damage rolls").map(|m| m.value)
            });
            let notation = match bonus {
                Some(n) if n > 0 => format!("{d}+{n}"),
                Some(n) if n < 0 => format!("{d}{n}"),
                _ => d,
            };
            push(format!("{name} damage"), notation);
        }
    }
    // ASCII-only lowercasing keeps byte positions the same as in the original text
    let lower = item.description.to_ascii_lowercase();
    // healing: "regains 2d4 + 2 Hit Points"
    for key in ["regain ", "regains "] {
        if let Some(p) = lower.find(key) {
            let tail = &item.description[p + key.len()..];
            let tail_l = &lower[p + key.len()..];
            if let (Some(d), Some(hp)) = (find_dice(tail), tail_l.find("hit point")) {
                if tail_l.find(&d.replace(' ', "")).map(|i| i < hp).unwrap_or(hp < 30) {
                    push(format!("{name} (healing)"), d);
                }
            }
        }
    }
    // "extra 2d6 Fire damage" / "an extra 1d6 Necrotic damage"
    let mut from = 0;
    while let Some(p) = lower[from..].find("extra ") {
        let at = from + p + "extra ".len();
        let tail = &item.description[at..];
        let first = tail.split_whitespace().next().unwrap_or("");
        if first.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            if let Some(d) = find_dice(first) {
                let ty = tail
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .trim_matches(|c: char| !c.is_alphabetic())
                    .to_lowercase();
                let label = if ty.is_empty() || ty == "damage" {
                    format!("{name} extra damage")
                } else {
                    format!("{name} extra {ty} damage")
                };
                push(label, d);
            }
        }
        from = at;
    }
    // "takes 4d4 Fire damage" / "8d6 Fire damage" (first two)
    let mut found = 0;
    let mut from = 0;
    while let Some(p) = lower[from..].find(" damage") {
        let at = from + p;
        let window_start = lower[..at]
            .rfind(['.', ',', ';'])
            .map(|i| i + 1)
            .unwrap_or(0)
            .max(crate::model::ceil_char(&lower, at.saturating_sub(40)));
        let seg = &item.description[window_start..at];
        if let Some(d) = find_dice(seg) {
            if !seg.to_lowercase().contains("extra") {
                let ty = seg
                    .split_whitespace()
                    .last()
                    .unwrap_or("")
                    .trim_matches(|c: char| !c.is_alphabetic())
                    .to_lowercase();
                let ty = if ty.contains('d') && ty.chars().any(|c| c.is_ascii_digit()) {
                    String::new()
                } else {
                    ty
                };
                let label = if ty.is_empty() {
                    format!("{name} damage")
                } else {
                    format!("{name} {ty} damage")
                };
                push(label, d);
                found += 1;
                if found >= 2 {
                    break;
                }
            }
        }
        from = at + 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Coins
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Coins {
    pub pp: i64,
    pub gp: i64,
    pub ep: i64,
    pub sp: i64,
    pub cp: i64,
}

impl Coins {
    pub fn gp(n: i64) -> Coins {
        Coins { gp: n, ..Default::default() }
    }
    pub fn total_gp(&self) -> f64 {
        self.pp as f64 * 10.0 + self.gp as f64 + self.ep as f64 * 0.5 + self.sp as f64 * 0.1 + self.cp as f64 * 0.01
    }
    pub fn is_zero(&self) -> bool {
        self.pp == 0 && self.gp == 0 && self.ep == 0 && self.sp == 0 && self.cp == 0
    }
    pub fn add(&mut self, o: &Coins) {
        self.pp += o.pp;
        self.gp += o.gp;
        self.ep += o.ep;
        self.sp += o.sp;
        self.cp += o.cp;
    }
    /// "12 gp, 30 sp"
    pub fn label(&self) -> String {
        let parts: Vec<String> = [(self.pp, "pp"), (self.gp, "gp"), (self.ep, "ep"), (self.sp, "sp"), (self.cp, "cp")]
            .iter()
            .filter(|(n, _)| *n != 0)
            .map(|(n, u)| format!("{n} {u}"))
            .collect();
        if parts.is_empty() { "0 gp".into() } else { parts.join(", ") }
    }
    pub fn total_cp(&self) -> i64 {
        self.pp * 1000 + self.gp * 100 + self.ep * 50 + self.sp * 10 + self.cp
    }
    /// Pay `price_gp` from this purse, spending small coins first and taking change in gold,
    /// silver and copper. Returns false (and changes nothing) if there isn't enough.
    pub fn pay(&mut self, price_gp: f64) -> bool {
        let price = (price_gp * 100.0).round() as i64;
        let total = self.total_cp();
        if price > total {
            return false;
        }
        let mut left = total - price;
        // keep the biggest coins we can, up to what we had
        let mut out = Coins::default();
        for (have, val, slot) in [(self.pp, 1000, 0), (self.gp, 100, 1), (self.ep, 50, 2), (self.sp, 10, 3)] {
            let keep = have.min(left / val);
            left -= keep * val;
            match slot {
                0 => out.pp = keep,
                1 => out.gp = keep,
                2 => out.ep = keep,
                _ => out.sp = keep,
            }
        }
        // change
        out.gp += left / 100;
        left %= 100;
        out.sp += left / 10;
        out.cp = left % 10;
        *self = out;
        true
    }
    /// Split as evenly as possible between `n` people (remainders go to the first ones),
    /// changing larger coins into smaller ones when needed so nothing is lost.
    pub fn split(&self, n: usize) -> Vec<Coins> {
        let n = n.max(1) as i64;
        let cp_total = self.pp * 1000 + self.gp * 100 + self.ep * 50 + self.sp * 10 + self.cp;
        let mut out = vec![Coins::default(); n as usize];
        let share = cp_total / n;
        let mut extra = cp_total % n;
        for c in out.iter_mut() {
            let mut v = share + if extra > 0 { 1 } else { 0 };
            if extra > 0 {
                extra -= 1;
            }
            c.gp = v / 100;
            v %= 100;
            c.sp = v / 10;
            c.cp = v % 10;
        }
        out
    }
}

/// "50 gp", "5 sp", "1,500 gp", "2 pp", 25 → gp.
pub fn parse_cost(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => parse_cost_text(s),
        Value::Object(o) => {
            let amount = get(o, &["amount", "value", "quantity"]).and_then(|a| a.as_f64().or_else(|| as_text(a).replace(',', "").parse().ok()))?;
            let unit = text_of(o, &["unit", "currency"]);
            Some(amount * unit_factor(&unit))
        }
        _ => None,
    }
}

fn unit_factor(unit: &str) -> f64 {
    match unit.trim().to_lowercase().as_str() {
        "pp" | "platinum" => 10.0,
        "ep" | "electrum" => 0.5,
        "sp" | "silver" => 0.1,
        "cp" | "copper" => 0.01,
        _ => 1.0,
    }
}

pub fn parse_cost_text(s: &str) -> Option<f64> {
    let t = s.trim().to_lowercase().replace(',', "");
    if t.is_empty() || t == "—" || t == "-" || t == "varies" {
        return None;
    }
    let num: String = t.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let n: f64 = num.parse().ok()?;
    let unit = t[num.len()..].trim().split_whitespace().next().unwrap_or("gp");
    Some(n * unit_factor(unit))
}

/// "50 gp", "5 sp", "1,500 gp".
pub fn format_gp(v: f64) -> String {
    if v <= 0.0 {
        return "—".into();
    }
    if v < 1.0 {
        let sp = v * 10.0;
        if (sp - sp.round()).abs() < 1e-6 {
            return format!("{} sp", sp.round() as i64);
        }
        return format!("{} cp", (v * 100.0).round() as i64);
    }
    let whole = v.round() as i64;
    if (v - whole as f64).abs() < 1e-6 {
        let s = whole.to_string();
        let mut out = String::new();
        for (i, ch) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(ch);
        }
        format!("{out} gp")
    } else {
        format!("{v:.1} gp")
    }
}

// ---------------------------------------------------------------------------
// Inventory
// ---------------------------------------------------------------------------

/// One line of what a character carries. Holds its own copy of the item so later library
/// edits don't change what's in someone's backpack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InventoryEntry {
    pub id: Id,
    pub item: Item,
    pub qty: u32,
    pub equipped: bool,
    pub attuned: bool,
    pub notes: String,
}

impl Default for InventoryEntry {
    fn default() -> Self {
        InventoryEntry {
            id: new_id(),
            item: Item::default(),
            qty: 1,
            equipped: false,
            attuned: false,
            notes: String::new(),
        }
    }
}

impl InventoryEntry {
    pub fn of(item: &Item, qty: u32) -> InventoryEntry {
        let mut it = item.clone();
        it.id = new_id();
        InventoryEntry {
            item: it,
            qty: qty.max(1),
            ..Default::default()
        }
    }
}

/// Add to an inventory, stacking with an identical (same name, not magic-tweaked) entry.
pub fn add_to_inventory(inv: &mut Vec<InventoryEntry>, item: &Item, qty: u32) {
    if let Some(e) = inv
        .iter_mut()
        .find(|e| e.item.name.eq_ignore_ascii_case(&item.name) && e.item.rarity == item.rarity && e.item.description == item.description)
    {
        e.qty += qty.max(1);
        return;
    }
    inv.push(InventoryEntry::of(item, qty));
}

// ---------------------------------------------------------------------------
// Import / export
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ImportReport {
    pub items: Vec<Item>,
    pub warnings: Vec<String>,
    pub source: String,
    pub collection: String,
}

pub fn import_json(text: &str) -> Result<ImportReport, crate::bestiary::ImportError> {
    use crate::bestiary::ImportError;
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
            match o.get("items").or_else(|| o.get("loot")).or_else(|| o.get("magic_items")) {
                Some(Value::Array(a)) => a.clone(),
                _ if o.contains_key("name") => vec![v.clone()],
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    for (i, entry) in list.iter().enumerate() {
        match item_from_value(entry, &report.source) {
            Ok((mut it, w)) => {
                if it.collection.is_empty() {
                    it.collection = report.collection.clone();
                }
                let n = it.name.clone();
                report.warnings.extend(w.into_iter().map(|m| format!("{n}: {m}")));
                report.items.push(it);
            }
            Err(e) => report.warnings.push(format!("entry {}: skipped — {e}", i + 1)),
        }
    }
    if report.items.is_empty() {
        return Err(ImportError::Empty);
    }
    Ok(report)
}

pub fn item_from_value(v: &Value, default_source: &str) -> Result<(Item, Vec<String>), String> {
    let o = v.as_object().ok_or("not an object")?;
    let mut w = Vec::new();
    let name = text_of(o, &["name", "title"]);
    if name.is_empty() {
        return Err("missing \"name\"".into());
    }
    let mut it = Item { name, ..Default::default() };
    it.kind = ItemKind::parse(&text_of(o, &["type", "kind", "category"]));
    let rarity_text = text_of(o, &["rarity"]);
    it.rarity = match Rarity::parse(&rarity_text) {
        Some(r) => r,
        None => {
            w.push(format!("unknown rarity “{rarity_text}” — treated as uncommon"));
            Rarity::Uncommon
        }
    };
    if let Some(c) = get(o, &["cost", "price", "value", "cost_gp"]) {
        it.cost_gp = parse_cost(c);
        if it.cost_gp.is_none() && !as_text(c).trim().is_empty() {
            w.push(format!("couldn't read cost “{}”", as_text(c)));
        }
    }
    it.weight_lb = get(o, &["weight", "weight_lb"]).and_then(|x| x.as_f64().or_else(|| crate::bestiary::first_int(&as_text(x)).map(|n| n as f64)));
    it.attunement = match get(o, &["attunement", "requires_attunement"]) {
        Some(Value::Bool(true)) => "requires attunement".into(),
        Some(Value::Bool(false)) | None => String::new(),
        Some(x) => {
            let t = as_text(x);
            if t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes") {
                "requires attunement".into()
            } else if t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no") {
                String::new()
            } else {
                t
            }
        }
    };
    it.damage = text_of(o, &["damage"]);
    it.armor_class = text_of(o, &["armor_class", "ac"]);
    it.properties = text_of(o, &["properties"]);
    it.description = text_of(o, &["description", "desc", "text", "entries"]);
    it.source = {
        let s = text_of(o, &["source"]);
        if s.is_empty() { default_source.to_string() } else { s }
    };
    it.collection = text_of(o, &["collection"]);
    it.tags = match get(o, &["tags"]) {
        Some(Value::Array(a)) => a.iter().map(as_text).filter(|s| !s.is_empty()).collect(),
        Some(other) => as_text(other).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        None => Vec::new(),
    };
    if let Some(Value::Array(rolls)) = get(o, &["rolls"]) {
        for r in rolls {
            let (label, dice) = match r {
                Value::Object(ro) => (text_of(ro, &["label", "name"]), text_of(ro, &["dice", "roll", "notation", "formula"])),
                other => (String::new(), as_text(other)),
            };
            let dice = dice.replace(' ', "");
            if crate::dice::DiceExpr::parse(&dice).is_ok() {
                it.rolls.push(CustomRoll {
                    label: if label.is_empty() { it.name.clone() } else { label },
                    notation: dice,
                    ..Default::default()
                });
            } else {
                w.push(format!("roll “{dice}” isn't dice notation — skipped"));
            }
        }
    }
    if let Some(Value::Array(mods)) = get(o, &["modifiers", "bonuses"]) {
        for m in mods {
            match m {
                Value::Object(mo) => {
                    let label = text_of(mo, &["label", "target", "name"]);
                    match get(mo, &["value", "bonus"]).and_then(|x| crate::bestiary::as_int(x)) {
                        Some(v) if !label.is_empty() => it.modifiers.push(ItemModifier { label, value: v }),
                        _ => w.push("a modifier needs a label and a number — skipped".into()),
                    }
                }
                other => {
                    // "+1 AC"
                    let t = as_text(other);
                    if let Some(v) = crate::bestiary::first_int(&t) {
                        let label = t
                            .trim_start_matches(|c: char| c == '+' || c == '-' || c.is_ascii_digit() || c == ' ')
                            .trim()
                            .to_string();
                        it.modifiers.push(ItemModifier { label, value: v });
                    }
                }
            }
        }
    }
    it.derive_rolls();
    Ok((it, w))
}

pub fn export_json(items: &[Item], source: &str) -> String {
    let list: Vec<Value> = items
        .iter()
        .map(|i| {
            serde_json::json!({
                "name": i.name, "type": i.kind.label(), "rarity": i.rarity.label(),
                "cost": i.cost_gp, "weight": i.weight_lb, "attunement": i.attunement,
                "damage": i.damage, "armor_class": i.armor_class, "properties": i.properties,
                "description": i.description,
                "rolls": i.rolls.iter().map(|r| serde_json::json!({"label": r.label, "dice": r.notation})).collect::<Vec<_>>(),
                "modifiers": i.modifiers.iter().map(|m| serde_json::json!({"label": m.label, "value": m.value})).collect::<Vec<_>>(),
                "tags": i.tags, "source": i.source, "collection": i.collection,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "format": FORMAT_NAME, "version": FORMAT_VERSION, "source": source, "items": list,
    }))
    .unwrap_or_default()
}

/// Instructions for an AI model converting item lists, shown in the Items import window.
pub const AI_SPEC: &str = include_str!("../../../docs/item-import-spec.md");

/// The bundled SRD 5.2 items (equipment + magic items), CC-BY-4.0.
pub const SRD_JSON: &str = include_str!("../data/srd-5.2-items.json");

pub fn srd_items() -> Vec<Item> {
    import_json(SRD_JSON)
        .map(|r| r.items)
        .unwrap_or_default()
        .into_iter()
        .map(|mut i| {
            i.collection = "SRD 5.2".into();
            i
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it(name: &str, kind: ItemKind, desc: &str) -> Item {
        Item {
            name: name.into(),
            kind,
            description: desc.into(),
            ..Default::default()
        }
    }

    #[test]
    fn bonuses_from_names_and_text() {
        let mut sword = it(
            "Longsword (+1)",
            ItemKind::Weapon,
            "You have a +1 bonus to attack and damage rolls made with this magic weapon.",
        );
        sword.damage = "1d8 slashing".into();
        sword.derive_rolls();
        assert_eq!(sword.rolls[0].notation, "1d8+1");
        assert_eq!(
            sword.modifiers.iter().map(|m| m.text()).collect::<Vec<_>>(),
            ["+1 attack rolls", "+1 damage rolls"]
        );
        let ring = it(
            "Ring of Protection",
            ItemKind::Ring,
            "You gain a +1 bonus to Armor Class and saving throws while wearing this ring.",
        );
        let m = suggest_modifiers(&ring);
        assert_eq!(m[0].text(), "+1 AC");
        let cloak = it(
            "Cloak of Protection",
            ItemKind::Wondrous,
            "You gain a +1 bonus to AC and saving throws while you wear this cloak.",
        );
        assert_eq!(suggest_modifiers(&cloak)[0].label, "AC");
        let shield = it("Shield (+2)", ItemKind::Armor, "");
        assert_eq!(suggest_modifiers(&shield)[0].text(), "+2 AC");
        let wand = it(
            "Wand of the War Mage +2",
            ItemKind::Wand,
            "While holding this wand, you gain a +2 bonus to spell attack rolls.",
        );
        assert_eq!(suggest_modifiers(&wand)[0].text(), "+2 spell attack rolls");
    }

    #[test]
    fn rolls_from_text() {
        let p = it(
            "Potion of Healing",
            ItemKind::Potion,
            "You regain 2d4 + 2 Hit Points when you drink this potion.",
        );
        assert_eq!(suggest_rolls(&p)[0].notation, "2d4+2");
        let f = it(
            "Flame Tongue",
            ItemKind::Weapon,
            "While the weapon is ablaze, it deals an extra 2d6 Fire damage on a hit.",
        );
        let r = suggest_rolls(&f);
        assert_eq!((r[0].label.as_str(), r[0].notation.as_str()), ("Flame Tongue extra fire damage", "2d6"));
        let w = it(
            "Wand of Fireballs",
            ItemKind::Wand,
            "Each creature in a 20-foot-radius Sphere makes a Dexterity saving throw, taking 8d6 Fire damage on a failed save.",
        );
        assert_eq!(suggest_rolls(&w)[0].notation, "8d6");
        assert_eq!(suggest_rolls(&w)[0].label, "Wand of Fireballs fire damage");
        assert!(suggest_rolls(&it("Rope", ItemKind::Gear, "50 feet of rope.")).is_empty());
    }

    #[test]
    fn coins_and_costs() {
        assert_eq!(parse_cost_text("1,500 gp"), Some(1500.0));
        assert_eq!(parse_cost_text("5 sp"), Some(0.5));
        assert_eq!(parse_cost_text("—"), None);
        assert_eq!(format_gp(1500.0), "1,500 gp");
        assert_eq!(format_gp(0.5), "5 sp");
        assert_eq!(format_gp(0.02), "2 cp");
        let c = Coins {
            gp: 10,
            sp: 5,
            cp: 1,
            ..Default::default()
        };
        let parts = c.split(3);
        let total: f64 = parts.iter().map(|p| p.total_gp()).sum();
        assert!((total - c.total_gp()).abs() < 1e-9);
        assert_eq!(parts[0].label(), "3 gp, 5 sp, 1 cp");
        assert_eq!(
            Coins {
                pp: 2,
                gp: 5,
                ..Default::default()
            }
            .label(),
            "2 pp, 5 gp"
        );
        let potion = Item::named("Potion of X", ItemKind::Potion, Rarity::Rare, None);
        assert_eq!(potion.value_gp(), 2000.0);
    }

    #[test]
    fn lenient_import() {
        let text = r#"Here you go:
```json
{"format": "campaign-manager-items", "source": "Tomb loot", "items": [
  {"name": "Sunblade", "type": "Weapon (longsword)", "rarity": "Rare", "attunement": true, "cost": "5,000 gp",
   "damage": "1d8 radiant", "description": "You gain a +2 bonus to attack and damage rolls made with this weapon."},
  {"name": "Gilded skull", "type": "art object", "cost": {"amount": 25, "unit": "gp"}},
  {"name": "Mystery", "rarity": "sorta rare", "rolls": [{"label": "Zap", "dice": "2d6"}], "modifiers": ["+1 AC"]},
  {"rarity": "rare"},
]}
```"#;
        let r = import_json(text).unwrap();
        assert_eq!(r.items.len(), 3);
        assert_eq!(r.collection, "Tomb loot");
        let sb = &r.items[0];
        assert_eq!((sb.kind, sb.rarity, sb.cost_gp), (ItemKind::Weapon, Rarity::Rare, Some(5000.0)));
        assert_eq!(sb.attunement, "requires attunement");
        assert_eq!(sb.modifiers.len(), 2);
        assert_eq!(r.items[1].kind, ItemKind::Art);
        assert_eq!(r.items[1].cost_gp, Some(25.0));
        assert_eq!(r.items[2].rolls[0].notation, "2d6");
        assert_eq!(r.items[2].modifiers[0].text(), "+1 AC");
        assert!(r.warnings.iter().any(|w| w.contains("sorta rare")));
        assert!(r.warnings.iter().any(|w| w.contains("entry 4")));
        let again = import_json(&export_json(&r.items, "x")).unwrap();
        assert_eq!(again.items[0].modifiers, r.items[0].modifiers);
        assert_eq!(again.items[2].rolls[0].label, "Zap");
    }

    #[test]
    fn inventory_stacks() {
        let mut inv = Vec::new();
        let p = Item::named("Potion of Healing", ItemKind::Potion, Rarity::Common, Some(50.0));
        add_to_inventory(&mut inv, &p, 1);
        add_to_inventory(&mut inv, &p, 2);
        add_to_inventory(&mut inv, &Item::named("Rope", ItemKind::Gear, Rarity::Mundane, Some(1.0)), 1);
        assert_eq!(inv.len(), 2);
        assert_eq!(inv[0].qty, 3);
        assert_ne!(inv[0].item.id, p.id, "inventory holds its own copy");
    }

    #[test]
    fn bundled_srd_items() {
        let items = srd_items();
        assert!(items.len() > 300, "{}", items.len());
        let pot = items.iter().find(|i| i.name == "Potion of Healing").unwrap();
        assert_eq!(pot.rolls[0].notation, "2d4+2");
        let ls = items.iter().find(|i| i.name == "Longsword (+1)").unwrap();
        assert!(ls.rolls.iter().any(|r| r.notation == "1d8+1"));
        assert!(items.iter().all(|i| !i.name.is_empty()));
        assert!(AI_SPEC.contains(FORMAT_NAME));
    }

    #[test]
    fn paying_makes_change() {
        let mut c = Coins {
            pp: 1,
            gp: 3,
            sp: 5,
            ..Default::default()
        };
        assert!(c.pay(2.5));
        assert_eq!(c.total_cp(), 1000 + 300 + 50 - 250);
        assert_eq!(c.pp, 1);
        assert!(!c.pay(100.0));
        assert_eq!(c.total_cp(), 1100);
        let mut d = Coins { pp: 2, ..Default::default() };
        assert!(d.pay(0.37));
        assert_eq!(d.total_cp(), 2000 - 37);
        assert!(d.pp == 1 && d.gp == 9 && d.sp == 6 && d.cp == 3, "{d:?}");
    }

    #[test]
    fn kinds_match_whole_words() {
        assert_eq!(ItemKind::parse("Adventuring gear"), ItemKind::Gear);
        assert_eq!(ItemKind::parse("Ring"), ItemKind::Ring);
        assert_eq!(ItemKind::parse("Rings"), ItemKind::Ring);
        assert_eq!(ItemKind::parse("Art object"), ItemKind::Art);
        assert_eq!(ItemKind::parse("Ammunition"), ItemKind::Ammunition);
        let srd = srd_items();
        let rope = srd.iter().find(|i| i.name == "Rope").unwrap();
        assert_eq!(rope.kind, ItemKind::Gear);
        assert!(srd.iter().all(|i| !i.name.ends_with(" GP)")));
        assert!(
            srd.iter().filter(|i| i.kind == ItemKind::Ring).all(|i| i.name.contains("Ring")),
            "only rings are rings"
        );
    }

    #[test]
    fn weapon_rolls_use_text_bonus() {
        let it = Item {
            name: "Sunblade of Avarre".into(),
            kind: ItemKind::Weapon,
            damage: "1d8 radiant".into(),
            description: "You gain a +2 bonus to attack and damage rolls. Undead take an extra 1d8 Radiant damage.".into(),
            ..Default::default()
        };
        let r = suggest_rolls(&it);
        assert_eq!(r.len(), 2, "{r:?}");
        assert_eq!(r[0].notation, "1d8+2");
        assert_eq!(r[1].notation, "1d8");
        assert!(r[1].label.contains("radiant"));
    }
}
