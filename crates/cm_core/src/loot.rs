//! Procedural treasure: loot drops by challenge rating / encounter / gp value, and themed shop
//! stock. No AI involved — everything is dice tables and the item library.

use serde::{Deserialize, Serialize};

use crate::dice::DiceRng;
use crate::items::{Coins, Item, ItemKind, Rarity, format_gp, plus_bonus};
use crate::model::{Id, new_id};

// ---------------------------------------------------------------------------
// Dice helpers
// ---------------------------------------------------------------------------

fn d(rng: &mut dyn DiceRng, n: u32, sides: u32) -> i64 {
    (0..n).map(|_| rng.roll_die(sides) as i64).sum()
}

fn pick<'a, T>(rng: &mut dyn DiceRng, list: &'a [T]) -> Option<&'a T> {
    if list.is_empty() {
        return None;
    }
    let i = rng.roll_die(list.len() as u32) as usize - 1;
    list.get(i)
}

/// Pick with weights (weight 0 entries are never picked).
fn pick_weighted<T: Copy>(rng: &mut dyn DiceRng, list: &[(T, u32)]) -> Option<T> {
    let total: u32 = list.iter().map(|(_, w)| *w).sum();
    if total == 0 {
        return None;
    }
    let mut r = rng.roll_die(total);
    for (v, w) in list {
        if r <= *w {
            return Some(*v);
        }
        r -= w;
    }
    None
}

// ---------------------------------------------------------------------------
// Gemstones and art objects
// ---------------------------------------------------------------------------

/// Gemstones by value (gp).
pub const GEMS: &[(f64, &[&str])] = &[
    (
        10.0,
        &[
            "Azurite",
            "Banded agate",
            "Blue quartz",
            "Eye agate",
            "Hematite",
            "Lapis lazuli",
            "Malachite",
            "Moss agate",
            "Obsidian",
            "Rhodochrosite",
            "Tiger eye",
            "Turquoise",
        ],
    ),
    (
        50.0,
        &[
            "Bloodstone",
            "Carnelian",
            "Chalcedony",
            "Chrysoprase",
            "Citrine",
            "Jasper",
            "Moonstone",
            "Onyx",
            "Rock crystal",
            "Sardonyx",
            "Star rose quartz",
            "Zircon",
        ],
    ),
    (
        100.0,
        &[
            "Amber",
            "Amethyst",
            "Chrysoberyl",
            "Coral",
            "Garnet",
            "Jade",
            "Jet",
            "Pearl",
            "Spinel",
            "Tourmaline",
        ],
    ),
    (500.0, &["Alexandrite", "Aquamarine", "Black pearl", "Blue spinel", "Peridot", "Topaz"]),
    (
        1000.0,
        &[
            "Black opal",
            "Blue sapphire",
            "Emerald",
            "Fire opal",
            "Opal",
            "Star ruby",
            "Star sapphire",
            "Yellow sapphire",
        ],
    ),
    (5000.0, &["Black sapphire", "Diamond", "Jacinth", "Ruby"]),
];

/// Art objects by value (gp). Our own descriptions.
pub const ART: &[(f64, &[&str])] = &[
    (
        25.0,
        &[
            "Silver ewer",
            "Carved bone statuette of a heron",
            "Small gold bracelet",
            "Embroidered silk handkerchief",
            "Copper chalice with silver filigree",
            "Painted wooden festival mask",
            "Brass hand mirror",
            "Pewter tankard etched with a hunting scene",
            "Pair of engraved bone dice",
            "Lacquered wooden jewelry box",
        ],
    ),
    (
        250.0,
        &[
            "Gold ring set with bloodstones",
            "Carved ivory statuette of a dancer",
            "Silver-chased longsword scabbard",
            "Bronze crown of an old river-king",
            "Silk robe with gold embroidery",
            "Tapestry of a sea battle",
            "Brass mug with jade inlay",
            "Set of turquoise animal figurines",
            "Silver comb set with moonstones",
        ],
    ),
    (
        750.0,
        &[
            "Silver chalice set with moonstones",
            "Gold music box that plays a lullaby",
            "Gold circlet set with aquamarines",
            "Portrait by a celebrated court painter",
            "Obsidian statuette with gold fittings",
            "Ceremonial electrum dagger with a black pearl pommel",
            "Silk and velvet cloak clasped with a fire opal",
        ],
    ),
    (
        2500.0,
        &[
            "Fine gold chain set with a fire opal",
            "Old masterpiece painting in a gilt frame",
            "Platinum bracelet set with a sapphire",
            "Embroidered mantle set with star rubies",
            "Jeweled gold reliquary",
            "Gold-plated astrolabe with emerald stars",
        ],
    ),
    (
        7500.0,
        &[
            "Jeweled platinum signet ring",
            "Gold idol with ruby eyes",
            "Jewel-encrusted gold crown",
            "Small mithral statuette of a dragon with diamond eyes",
            "Platinum and sapphire scepter",
        ],
    ),
];

fn treasure_item(rng: &mut dyn DiceRng, table: &[(f64, &[&str])], value: f64, kind: ItemKind) -> Item {
    let names = table.iter().find(|(v, _)| (*v - value).abs() < 0.01).map(|(_, n)| *n).unwrap_or(&["Curio"]);
    let name = pick(rng, names).copied().unwrap_or("Curio");
    let mut it = Item::named(name, kind, Rarity::Mundane, Some(value));
    it.description = format!("Worth about {}.", format_gp(value));
    it.tags = vec![if kind == ItemKind::Gem { "gem" } else { "art" }.into()];
    it
}

pub fn gem(rng: &mut dyn DiceRng, value: f64) -> Item {
    treasure_item(rng, GEMS, value, ItemKind::Gem)
}

pub fn art(rng: &mut dyn DiceRng, value: f64) -> Item {
    treasure_item(rng, ART, value, ItemKind::Art)
}

// ---------------------------------------------------------------------------
// Loot
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LootLine {
    pub id: Id,
    /// An item (or gem / art object). None = a pile of coins.
    pub item: Option<Item>,
    pub coins: Coins,
    pub qty: u32,
}

impl Default for LootLine {
    fn default() -> Self {
        LootLine {
            id: new_id(),
            item: None,
            coins: Coins::default(),
            qty: 1,
        }
    }
}

impl LootLine {
    pub fn coins(c: Coins) -> LootLine {
        LootLine {
            coins: c,
            ..Default::default()
        }
    }
    pub fn item(i: Item, qty: u32) -> LootLine {
        LootLine {
            item: Some(i),
            qty: qty.max(1),
            ..Default::default()
        }
    }
    pub fn value_gp(&self) -> f64 {
        match &self.item {
            Some(i) => i.value_gp() * self.qty as f64,
            None => self.coins.total_gp(),
        }
    }
    pub fn label(&self) -> String {
        match &self.item {
            Some(i) if self.qty > 1 => format!("{} × {}", self.qty, i.name),
            Some(i) => i.name.clone(),
            None => self.coins.label(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Loot {
    pub title: String,
    pub lines: Vec<LootLine>,
}

impl Loot {
    pub fn total_gp(&self) -> f64 {
        self.lines.iter().map(|l| l.value_gp()).sum()
    }
    pub fn coins(&self) -> Coins {
        let mut c = Coins::default();
        for l in self.lines.iter().filter(|l| l.item.is_none()) {
            c.add(&l.coins);
        }
        c
    }
    /// Merge all coin lines into one (keeps items as they are).
    pub fn tidy(&mut self) {
        let c = self.coins();
        self.lines.retain(|l| l.item.is_some());
        if !c.is_zero() {
            self.lines.insert(0, LootLine::coins(c));
        }
    }
    /// Markdown list, for a story card or the clipboard.
    pub fn as_markdown(&self) -> String {
        let mut s = String::new();
        for l in &self.lines {
            match &l.item {
                Some(i) => {
                    let extra = if i.is_magic() { format!(" — {}", i.subtitle()) } else { String::new() };
                    s.push_str(&format!("- {}{extra} ({})\n", l.label(), format_gp(l.value_gp())));
                }
                None => s.push_str(&format!("- {}\n", l.label())),
            }
        }
        s.push_str(&format!("\n**Total value:** about {}", format_gp(self.total_gp())));
        s
    }
}

/// Treasure tier from a challenge rating (or party level): 0 = CR 0–4, 1 = 5–10, 2 = 11–16, 3 = 17+.
pub fn tier_for(cr: f32) -> usize {
    match cr {
        c if c <= 4.0 => 0,
        c if c <= 10.0 => 1,
        c if c <= 16.0 => 2,
        _ => 3,
    }
}

pub const TIER_LABELS: [&str; 4] = ["CR 0–4", "CR 5–10", "CR 11–16", "CR 17+"];

/// Pocket change for one creature.
pub fn individual_coins(cr: f32, rng: &mut dyn DiceRng) -> Coins {
    match tier_for(cr) {
        0 => Coins {
            cp: d(rng, 5, 6),
            sp: d(rng, 4, 6),
            gp: d(rng, 1, 6) - 1,
            ..Default::default()
        },
        1 => Coins {
            sp: d(rng, 4, 6) * 10,
            gp: d(rng, 2, 6) * 10,
            ..Default::default()
        },
        2 => Coins {
            gp: d(rng, 4, 6) * 10,
            pp: d(rng, 1, 6) * 10,
            ..Default::default()
        },
        _ => Coins {
            gp: d(rng, 2, 6) * 100,
            pp: d(rng, 2, 6) * 10,
            ..Default::default()
        },
    }
}

fn hoard_coins(tier: usize, rng: &mut dyn DiceRng) -> Coins {
    match tier {
        0 => Coins {
            cp: d(rng, 6, 6) * 100,
            sp: d(rng, 3, 6) * 100,
            gp: d(rng, 2, 6) * 10,
            ..Default::default()
        },
        1 => Coins {
            sp: d(rng, 2, 6) * 100,
            gp: d(rng, 6, 6) * 100,
            pp: d(rng, 3, 6) * 10,
            ..Default::default()
        },
        2 => Coins {
            gp: d(rng, 4, 6) * 1000,
            pp: d(rng, 5, 6) * 100,
            ..Default::default()
        },
        _ => Coins {
            gp: d(rng, 12, 6) * 1000,
            pp: d(rng, 8, 6) * 1000,
            ..Default::default()
        },
    }
}

/// (gem/art values for the tier, how many: dice count, sides, minus)
fn hoard_valuables(tier: usize) -> (&'static [(f64, ItemKind)], u32, u32, i64) {
    match tier {
        0 => (&[(10.0, ItemKind::Gem), (25.0, ItemKind::Art)], 1, 4, 1),
        1 => (&[(50.0, ItemKind::Gem), (100.0, ItemKind::Gem), (250.0, ItemKind::Art)], 1, 6, 0),
        2 => (&[(500.0, ItemKind::Gem), (750.0, ItemKind::Art), (1000.0, ItemKind::Gem)], 1, 6, 0),
        _ => (
            &[
                (1000.0, ItemKind::Gem),
                (2500.0, ItemKind::Art),
                (5000.0, ItemKind::Gem),
                (7500.0, ItemKind::Art),
            ],
            1,
            6,
            0,
        ),
    }
}

fn hoard_magic(tier: usize) -> (u32, u32, i64, &'static [(Rarity, u32)]) {
    match tier {
        0 => (1, 4, 1, &[(Rarity::Common, 50), (Rarity::Uncommon, 45), (Rarity::Rare, 5)]),
        1 => (
            1,
            4,
            0,
            &[(Rarity::Common, 10), (Rarity::Uncommon, 50), (Rarity::Rare, 35), (Rarity::VeryRare, 5)],
        ),
        2 => (
            1,
            4,
            0,
            &[(Rarity::Uncommon, 15), (Rarity::Rare, 45), (Rarity::VeryRare, 35), (Rarity::Legendary, 5)],
        ),
        _ => (1, 6, 0, &[(Rarity::Rare, 25), (Rarity::VeryRare, 45), (Rarity::Legendary, 30)]),
    }
}

/// Family key so "+1 weapons" (×38) don't crowd out other items of the same rarity.
fn family(i: &Item) -> String {
    match (plus_bonus(&i.name), i.kind) {
        (Some(n), ItemKind::Weapon) => format!("weapon+{n}"),
        (Some(n), ItemKind::Armor) => format!("armor+{n}"),
        (Some(n), ItemKind::Ammunition) => format!("ammo+{n}"),
        _ => i.name.to_lowercase(),
    }
}

/// A random magic item of a rarity from the library (falls back to the nearest rarity).
pub fn random_magic_item(library: &[Item], rarity: Rarity, rng: &mut dyn DiceRng) -> Option<Item> {
    let order = [rarity, Rarity::Uncommon, Rarity::Rare, Rarity::Common, Rarity::VeryRare, Rarity::Legendary];
    for r in order {
        let pool: Vec<&Item> = library.iter().filter(|i| i.rarity == r && r != Rarity::Mundane).collect();
        if pool.is_empty() {
            continue;
        }
        // potions and scrolls turn up more often than one-of-a-kind wonders
        let consumables: Vec<&&Item> = pool.iter().filter(|i| i.kind.consumable()).collect();
        if !consumables.is_empty() && rng.roll_die(10) <= 3 {
            return pick(rng, &consumables).map(|i| (**i).clone());
        }
        let mut fams: Vec<String> = pool.iter().map(|i| family(i)).collect();
        fams.sort();
        fams.dedup();
        let f = pick(rng, &fams)?.clone();
        let members: Vec<&&Item> = pool.iter().filter(|i| family(i) == f).collect();
        return pick(rng, &members).map(|i| (**i).clone());
    }
    None
}

fn fresh(mut i: Item) -> Item {
    i.id = new_id();
    i
}

/// A hoard for a tier: coins, gems/art objects and magic items.
pub fn hoard(library: &[Item], tier: usize, rng: &mut dyn DiceRng) -> Loot {
    let tier = tier.min(3);
    let mut loot = Loot {
        title: format!("Treasure hoard ({})", TIER_LABELS[tier]),
        lines: vec![LootLine::coins(hoard_coins(tier, rng))],
    };
    let (vals, n, sides, minus) = hoard_valuables(tier);
    let count = (d(rng, n, sides) - minus).max(0);
    if count > 0 {
        // a hoard's gems tend to come in sets
        let (value, kind) = *pick(rng, vals).unwrap();
        let mut same: Vec<(String, u32)> = Vec::new();
        for _ in 0..count {
            let it = if kind == ItemKind::Gem { gem(rng, value) } else { art(rng, value) };
            match same.iter_mut().find(|(n, _)| *n == it.name) {
                Some(e) => e.1 += 1,
                None => same.push((it.name.clone(), 1)),
            }
        }
        for (name, q) in same {
            let mut it = Item::named(&name, kind, Rarity::Mundane, Some(value));
            it.description = format!("Worth about {} each.", format_gp(value));
            it.tags = vec![if kind == ItemKind::Gem { "gem" } else { "art" }.into()];
            loot.lines.push(LootLine::item(it, q));
        }
    }
    let (n, sides, minus, weights) = hoard_magic(tier);
    let items = (d(rng, n, sides) - minus).max(0);
    for _ in 0..items {
        if let Some(r) = pick_weighted(rng, weights) {
            if let Some(it) = random_magic_item(library, r, rng) {
                loot.lines.push(LootLine::item(fresh(it), 1));
            }
        }
    }
    loot
}

/// Loot for a fight: each creature's pocket change, or one hoard sized by the toughest
/// creature. `crs` are the non-player combatants' challenge ratings.
pub fn for_creatures(library: &[Item], crs: &[f32], as_hoard: bool, rng: &mut dyn DiceRng) -> Loot {
    if as_hoard {
        let top = crs.iter().cloned().fold(0.0_f32, f32::max);
        let mut l = hoard(library, tier_for(top), rng);
        l.title = format!("Hoard for an encounter up to CR {}", fmt_cr(top));
        return l;
    }
    let mut c = Coins::default();
    for cr in crs {
        c.add(&individual_coins(*cr, rng));
    }
    let mut loot = Loot {
        title: format!("Pocket change from {} creature{}", crs.len(), if crs.len() == 1 { "" } else { "s" }),
        lines: Vec::new(),
    };
    if !c.is_zero() {
        loot.lines.push(LootLine::coins(c));
    }
    // now and then someone carries a trinket or a potion
    for cr in crs {
        if rng.roll_die(20) == 20 {
            let t = tier_for(*cr);
            let r = [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::VeryRare][t];
            if let Some(it) = random_magic_item(library, r, rng) {
                loot.lines.push(LootLine::item(fresh(it), 1));
            }
        } else if rng.roll_die(6) == 6 {
            let v = [10.0, 50.0, 100.0, 500.0][tier_for(*cr)];
            loot.lines.push(LootLine::item(gem(rng, v), 1));
        }
    }
    loot
}

fn fmt_cr(cr: f32) -> String {
    match cr {
        c if (c - 0.125).abs() < 1e-3 => "1/8".into(),
        c if (c - 0.25).abs() < 1e-3 => "1/4".into(),
        c if (c - 0.5).abs() < 1e-3 => "1/2".into(),
        c => format!("{}", c as i32),
    }
}

/// "1/4" → 0.25
pub fn cr_number(cr: &str) -> Option<f32> {
    let t = cr.trim();
    if let Some((a, b)) = t.split_once('/') {
        let (a, b): (f32, f32) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
        return if b == 0.0 { None } else { Some(a / b) };
    }
    t.parse().ok()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValueOptions {
    pub target_gp: f64,
    pub include_magic: bool,
    pub max_rarity: Rarity,
    pub include_valuables: bool,
}

impl Default for ValueOptions {
    fn default() -> Self {
        ValueOptions {
            target_gp: 500.0,
            include_magic: true,
            max_rarity: Rarity::Rare,
            include_valuables: true,
        }
    }
}

/// Loot worth roughly `target_gp`: some magic items that fit the budget, some gems / art,
/// the rest in coins. The total lands within a few gp of the target.
pub fn by_value(library: &[Item], o: &ValueOptions, rng: &mut dyn DiceRng) -> Loot {
    let target = o.target_gp.max(0.0);
    let mut left = target;
    let mut loot = Loot {
        title: format!("Loot worth about {}", format_gp(target)),
        lines: Vec::new(),
    };
    if o.include_magic && target >= 50.0 {
        let max_items = (1.0 + target / 2500.0).min(4.0) as usize;
        let mut taken = 0;
        while taken < max_items && left > target * 0.35 {
            let cap = left.min(target * 0.6);
            let pool: Vec<&Item> = library
                .iter()
                .filter(|i| i.is_magic() && i.rarity <= o.max_rarity && i.value_gp() <= cap && i.value_gp() >= (target * 0.04).min(cap))
                .collect();
            let Some(it) = pick(rng, &pool) else { break };
            left -= it.value_gp();
            loot.lines.push(LootLine::item(fresh((*it).clone()), 1));
            taken += 1;
            if rng.roll_die(3) == 1 {
                break;
            }
        }
    }
    if o.include_valuables && left >= 20.0 {
        let budget = left * 0.5;
        let mut spent = 0.0;
        let mut count = 0;
        let denoms: Vec<(f64, ItemKind)> = GEMS
            .iter()
            .map(|(v, _)| (*v, ItemKind::Gem))
            .chain(ART.iter().map(|(v, _)| (*v, ItemKind::Art)))
            .filter(|(v, _)| *v <= budget)
            .collect();
        while count < 6 {
            let fits: Vec<(f64, ItemKind)> = denoms.iter().copied().filter(|(v, _)| spent + v <= budget && *v >= budget / 12.0).collect();
            let Some((v, k)) = pick(rng, &fits).copied() else { break };
            let it = if k == ItemKind::Gem { gem(rng, v) } else { art(rng, v) };
            spent += v;
            count += 1;
            loot.lines.push(LootLine::item(it, 1));
        }
        left -= spent;
    }
    if left >= 0.01 {
        let cp_total = (left * 100.0).round() as i64;
        let mut c = Coins::default();
        let mut gp_part = cp_total / 100;
        if gp_part >= 200 {
            c.pp = gp_part / 40; // a quarter of it in platinum
            gp_part -= c.pp * 10;
        }
        c.gp = gp_part;
        let rest = cp_total % 100;
        c.sp = rest / 10;
        c.cp = rest % 10;
        loot.lines.insert(0, LootLine::coins(c));
    }
    loot
}

// ---------------------------------------------------------------------------
// Shops
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ShopTheme {
    #[default]
    General,
    Blacksmith,
    Alchemist,
    Magic,
    Scribe,
    Fletcher,
    Jeweler,
    Temple,
    Tavern,
}

impl ShopTheme {
    pub const ALL: [ShopTheme; 9] = [
        ShopTheme::General,
        ShopTheme::Blacksmith,
        ShopTheme::Alchemist,
        ShopTheme::Magic,
        ShopTheme::Scribe,
        ShopTheme::Fletcher,
        ShopTheme::Jeweler,
        ShopTheme::Temple,
        ShopTheme::Tavern,
    ];
    pub fn label(self) -> &'static str {
        match self {
            ShopTheme::General => "General store",
            ShopTheme::Blacksmith => "Blacksmith / armory",
            ShopTheme::Alchemist => "Alchemist / potions",
            ShopTheme::Magic => "Magic shop",
            ShopTheme::Scribe => "Scribe / scrolls",
            ShopTheme::Fletcher => "Bowyer & fletcher",
            ShopTheme::Jeweler => "Jeweler",
            ShopTheme::Temple => "Temple goods",
            ShopTheme::Tavern => "Tavern / inn menu",
        }
    }
    pub fn default_size(self) -> usize {
        match self {
            ShopTheme::Magic | ShopTheme::Jeweler => 6,
            ShopTheme::Tavern => 8,
            _ => 10,
        }
    }
    /// Things a shop of this kind nearly always has.
    fn staples(self) -> &'static [&'static str] {
        match self {
            ShopTheme::General => &["Rope", "Torch", "Rations", "Backpack", "Bedroll", "Waterskin"],
            ShopTheme::Blacksmith => &["Dagger", "Longsword", "Shield", "Chain Mail", "Spear"],
            ShopTheme::Alchemist => &["Potion of Healing", "Antitoxin", "Alchemist's Fire"],
            ShopTheme::Magic => &["Potion of Healing"],
            ShopTheme::Scribe => &["Paper", "Ink", "Ink Pen", "Parchment"],
            ShopTheme::Fletcher => &["Arrows (20)", "Shortbow", "Longbow", "Bolts (20)"],
            ShopTheme::Temple => &["Potion of Healing", "Holy Water", "Healer's Kit", "Candle"],
            ShopTheme::Jeweler | ShopTheme::Tavern => &[],
        }
    }
    fn wants(self, i: &Item, q: ShopQuality) -> bool {
        let name = i.name.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
        let magic_ok = i.rarity <= q.max_rarity();
        match self {
            ShopTheme::General => {
                !i.is_magic() && matches!(i.kind, ItemKind::Gear | ItemKind::Tool | ItemKind::Ammunition) && i.value_gp() > 0.0 && i.value_gp() <= 50.0
            }
            ShopTheme::Blacksmith => {
                matches!(i.kind, ItemKind::Weapon | ItemKind::Armor)
                    && (!i.is_magic() || (q == ShopQuality::Fine && plus_bonus(&i.name) == Some(1)))
                    && !has(&["musket", "pistol"])
            }
            ShopTheme::Alchemist => {
                (i.kind == ItemKind::Potion && magic_ok)
                    || (!i.is_magic() && has(&["acid", "alchemist", "antitoxin", "oil", "perfume", "herbalism", "healer", "vial", "poison"]))
            }
            ShopTheme::Magic => {
                i.is_magic()
                    && magic_ok
                    && i.rarity != Rarity::Artifact
                    && plus_bonus(&i.name).is_none()
                    && matches!(
                        i.kind,
                        ItemKind::Ring | ItemKind::Wand | ItemKind::Wondrous | ItemKind::Staff | ItemKind::Rod | ItemKind::Potion | ItemKind::Scroll
                    )
            }
            ShopTheme::Scribe => {
                (i.kind == ItemKind::Scroll && magic_ok)
                    || (!i.is_magic()
                        && has(&[
                            "paper",
                            "parchment",
                            "ink",
                            "book",
                            "calligrapher",
                            "case, map",
                            "map or scroll",
                            "sealing wax",
                            "spellbook",
                        ]))
            }
            ShopTheme::Fletcher => {
                (!i.is_magic() || (q == ShopQuality::Fine && plus_bonus(&i.name) == Some(1)))
                    && (i.kind == ItemKind::Ammunition
                        || (i.kind == ItemKind::Weapon && (has(&["bow", "crossbow", "sling", "dart", "javelin"]) || i.properties.contains("Ammunition"))))
                    && !has(&["musket", "pistol", "firearm"])
            }
            ShopTheme::Temple => {
                (i.kind == ItemKind::Potion && name.contains("healing") && magic_ok)
                    || (i.kind == ItemKind::Scroll && magic_ok && i.rarity <= Rarity::Uncommon)
                    || (!i.is_magic() && has(&["holy", "amulet", "emblem", "reliquary", "healer", "candle", "incense", "censer", "prayer"]))
            }
            ShopTheme::Jeweler | ShopTheme::Tavern => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ShopQuality {
    Poor,
    #[default]
    Average,
    Fine,
}

impl ShopQuality {
    pub const ALL: [ShopQuality; 3] = [ShopQuality::Poor, ShopQuality::Average, ShopQuality::Fine];
    pub fn label(self) -> &'static str {
        match self {
            ShopQuality::Poor => "Poor",
            ShopQuality::Average => "Average",
            ShopQuality::Fine => "Fine",
        }
    }
    pub fn markup(self) -> f64 {
        match self {
            ShopQuality::Poor => 0.9,
            ShopQuality::Average => 1.0,
            ShopQuality::Fine => 1.25,
        }
    }
    pub fn max_rarity(self) -> Rarity {
        match self {
            ShopQuality::Poor => Rarity::Common,
            ShopQuality::Average => Rarity::Uncommon,
            ShopQuality::Fine => Rarity::Rare,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StockEntry {
    pub id: Id,
    pub item: Item,
    pub price_gp: f64,
    pub qty: u32,
}

impl Default for StockEntry {
    fn default() -> Self {
        StockEntry {
            id: new_id(),
            item: Item::default(),
            price_gp: 0.0,
            qty: 1,
        }
    }
}

/// Round a price the way a shopkeeper would.
pub fn shop_price(v: f64) -> f64 {
    if v >= 1000.0 {
        (v / 50.0).round() * 50.0
    } else if v >= 100.0 {
        (v / 5.0).round() * 5.0
    } else if v >= 10.0 {
        v.round()
    } else if v >= 1.0 {
        (v * 10.0).round() / 10.0
    } else {
        ((v * 100.0).round() / 100.0).max(0.01)
    }
}

fn price_for(base: f64, q: ShopQuality, rng: &mut dyn DiceRng) -> f64 {
    // ±10% haggle room
    let wobble = 0.9 + (rng.roll_die(21) as f64 - 1.0) / 100.0;
    shop_price(base * q.markup() * wobble)
}

/// Tavern fare (our own list): (name, kind, price in gp by quality poor/average/fine).
const MENU: &[(&str, ItemKind, [f64; 3])] = &[
    ("Mug of ale", ItemKind::Food, [0.02, 0.04, 0.1]),
    ("Pitcher of dark stout", ItemKind::Food, [0.1, 0.2, 0.5]),
    ("Cup of mulled cider", ItemKind::Food, [0.03, 0.05, 0.1]),
    ("Glass of house wine", ItemKind::Food, [0.1, 0.2, 1.0]),
    ("Bottle of fine wine", ItemKind::Food, [5.0, 10.0, 25.0]),
    ("Dwarven firewhisky (shot)", ItemKind::Food, [0.2, 0.5, 1.0]),
    ("Bowl of barley stew and bread", ItemKind::Food, [0.03, 0.1, 0.3]),
    ("Roast chicken with root vegetables", ItemKind::Food, [0.1, 0.3, 0.8]),
    ("Venison pie", ItemKind::Food, [0.2, 0.4, 1.0]),
    ("Smoked river trout", ItemKind::Food, [0.1, 0.3, 0.8]),
    ("Honey cakes", ItemKind::Food, [0.02, 0.05, 0.2]),
    ("Cheese, bread and pickles", ItemKind::Food, [0.02, 0.05, 0.2]),
    ("Spit-roasted boar (feast for four)", ItemKind::Food, [1.0, 2.0, 5.0]),
    ("Bed in the common room (night)", ItemKind::Other, [0.07, 0.2, 0.5]),
    ("Private room (night)", ItemKind::Other, [0.3, 0.5, 2.0]),
    ("Hot bath", ItemKind::Other, [0.05, 0.1, 0.5]),
    ("Stabling and feed (night)", ItemKind::Other, [0.05, 0.05, 0.1]),
];

/// Generate a shop's wares from the item library (or built-in tables for jewelers and inns).
pub fn stock_shop(library: &[Item], theme: ShopTheme, quality: ShopQuality, size: usize, rng: &mut dyn DiceRng) -> Vec<StockEntry> {
    let size = size.max(1);
    let qi = match quality {
        ShopQuality::Poor => 0,
        ShopQuality::Average => 1,
        ShopQuality::Fine => 2,
    };
    let mut out: Vec<StockEntry> = Vec::new();
    match theme {
        ShopTheme::Tavern => {
            let mut idx: Vec<usize> = (0..MENU.len()).collect();
            // lodging always on offer; then a few dishes and drinks
            let mut chosen: Vec<usize> = idx
                .iter()
                .copied()
                .filter(|i| MENU[*i].1 == ItemKind::Other && !MENU[*i].0.contains("bath"))
                .collect();
            idx.retain(|i| !chosen.contains(i));
            while chosen.len() < size && !idx.is_empty() {
                let k = rng.roll_die(idx.len() as u32) as usize - 1;
                chosen.push(idx.remove(k));
            }
            chosen.sort();
            for i in chosen {
                let (name, kind, prices) = MENU[i];
                let item = Item::named(name, kind, Rarity::Mundane, Some(prices[qi]));
                out.push(StockEntry {
                    item,
                    price_gp: prices[qi],
                    qty: 99,
                    ..Default::default()
                });
            }
        }
        ShopTheme::Jeweler => {
            let gem_vals: &[f64] = match quality {
                ShopQuality::Poor => &[10.0, 50.0],
                ShopQuality::Average => &[10.0, 50.0, 100.0, 500.0],
                ShopQuality::Fine => &[100.0, 500.0, 1000.0, 5000.0],
            };
            let art_vals: &[f64] = match quality {
                ShopQuality::Poor => &[25.0],
                ShopQuality::Average => &[25.0, 250.0],
                ShopQuality::Fine => &[250.0, 750.0, 2500.0],
            };
            for k in 0..size {
                let it = if k % 3 == 2 {
                    let v = *pick(rng, art_vals).unwrap();
                    art(rng, v)
                } else {
                    let v = *pick(rng, gem_vals).unwrap();
                    gem(rng, v)
                };
                if out.iter().any(|e| e.item.name == it.name) {
                    continue;
                }
                let price = price_for(it.value_gp(), quality, rng);
                out.push(StockEntry {
                    item: it,
                    price_gp: price,
                    qty: 1 + (rng.roll_die(3) - 1),
                    ..Default::default()
                });
            }
        }
        _ => {
            let pool: Vec<&Item> = library.iter().filter(|i| theme.wants(i, quality) && i.value_gp() > 0.0).collect();
            // group by family so +1 weapons or 12 armor-of-resistance variants don't flood a shop
            let mut fams: Vec<String> = pool.iter().map(|i| family(i)).collect();
            fams.sort();
            fams.dedup();
            // a few staples every shop of its kind keeps (up to half the stock), then random picks
            let mut staples: Vec<String> = theme
                .staples()
                .iter()
                .filter_map(|n| pool.iter().find(|i| i.name.eq_ignore_ascii_case(n)).map(|i| family(i)))
                .collect();
            staples.truncate((size / 2).max(1));
            while out.len() < size && !fams.is_empty() {
                let f = if !staples.is_empty() {
                    let f = staples.remove(0);
                    fams.retain(|x| *x != f);
                    f
                } else {
                    let k = rng.roll_die(fams.len() as u32) as usize - 1;
                    fams.remove(k)
                };
                let members: Vec<&&Item> = pool.iter().filter(|i| family(i) == f).collect();
                let Some(it) = pick(rng, &members) else { continue };
                let qty = if it.is_magic() {
                    if it.kind.consumable() { rng.roll_die(3) } else { 1 }
                } else if it.kind.consumable() || it.value_gp() < 5.0 {
                    rng.roll_die(6) + 2
                } else {
                    rng.roll_die(3)
                };
                let price = price_for(it.value_gp(), quality, rng);
                out.push(StockEntry {
                    item: fresh((**it).clone()),
                    price_gp: price,
                    qty,
                    ..Default::default()
                });
            }
            out.sort_by(|a, b| {
                a.item
                    .rarity
                    .cmp(&b.item.rarity)
                    .then(a.price_gp.partial_cmp(&b.price_gp).unwrap_or(std::cmp::Ordering::Equal))
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::srd_items;

    fn rng() -> fastrand::Rng {
        fastrand::Rng::with_seed(7)
    }

    #[test]
    fn hoards_scale_with_tier() {
        let lib = srd_items();
        let mut r = rng();
        let avg = |tier: usize, r: &mut fastrand::Rng| (0..60).map(|_| hoard(&lib, tier, r).total_gp()).sum::<f64>() / 60.0;
        let (a, b, c) = (avg(0, &mut r), avg(1, &mut r), avg(2, &mut r));
        assert!(a < b && b < c, "{a} {b} {c}");
        // a tier-2 hoard has magic items and they come from the library
        let h = (0..20)
            .map(|_| hoard(&lib, 2, &mut r))
            .find(|h| h.lines.iter().any(|l| l.item.as_ref().map(|i| i.is_magic()).unwrap_or(false)))
            .unwrap();
        let magic = h.lines.iter().filter_map(|l| l.item.as_ref()).find(|i| i.is_magic()).unwrap();
        assert!(lib.iter().any(|i| i.name == magic.name));
        assert!(h.as_markdown().contains("Total value"));
    }

    #[test]
    fn value_loot_hits_target() {
        let lib = srd_items();
        let mut r = rng();
        for target in [5.0, 75.0, 500.0, 3000.0, 25000.0] {
            for _ in 0..20 {
                let o = ValueOptions {
                    target_gp: target,
                    ..Default::default()
                };
                let l = by_value(&lib, &o, &mut r);
                let t = l.total_gp();
                assert!(
                    (t - target).abs() <= target * 0.02 + 0.05,
                    "target {target} got {t}: {:?}",
                    l.lines.iter().map(|x| x.label()).collect::<Vec<_>>()
                );
                assert!(l.lines.iter().filter_map(|x| x.item.as_ref()).all(|i| i.rarity <= Rarity::Rare));
            }
        }
        let none = by_value(
            &lib,
            &ValueOptions {
                target_gp: 1000.0,
                include_magic: false,
                include_valuables: false,
                ..Default::default()
            },
            &mut r,
        );
        assert_eq!(none.lines.len(), 1);
        assert_eq!(none.coins().total_gp(), 1000.0);
    }

    #[test]
    fn creature_loot() {
        let lib = srd_items();
        let mut r = rng();
        let l = for_creatures(&lib, &[0.25, 0.25, 2.0], false, &mut r);
        assert!(l.total_gp() > 0.0);
        assert!(l.title.contains("3 creatures"));
        let h = for_creatures(&lib, &[0.25, 12.0], true, &mut r);
        assert!(h.title.contains("CR 12"));
        assert_eq!(cr_number("1/4"), Some(0.25));
        assert_eq!(cr_number("10"), Some(10.0));
        assert_eq!(tier_for(4.0), 0);
        assert_eq!(tier_for(17.0), 3);
    }

    #[test]
    fn shops_stock_their_theme() {
        let lib = srd_items();
        let mut r = rng();
        for theme in ShopTheme::ALL {
            for q in ShopQuality::ALL {
                let s = stock_shop(&lib, theme, q, theme.default_size(), &mut r);
                assert!(!s.is_empty(), "{:?} {:?} is empty", theme, q);
                assert!(s.iter().all(|e| e.price_gp > 0.0), "{theme:?}");
                assert!(
                    s.iter().all(|e| e.item.rarity <= q.max_rarity() || theme == ShopTheme::Jeweler),
                    "{theme:?} {q:?}"
                );
            }
        }
        let potions = stock_shop(&lib, ShopTheme::Alchemist, ShopQuality::Average, 10, &mut r);
        assert!(potions.iter().any(|e| e.item.kind == ItemKind::Potion));
        assert!(potions.iter().any(|e| e.item.name == "Potion of Healing"));
        let general = stock_shop(&lib, ShopTheme::General, ShopQuality::Poor, 10, &mut r);
        assert!(general.iter().any(|e| e.item.name == "Rope"));
        let smith = stock_shop(&lib, ShopTheme::Blacksmith, ShopQuality::Average, 10, &mut r);
        assert!(
            smith
                .iter()
                .all(|e| matches!(e.item.kind, ItemKind::Weapon | ItemKind::Armor) && !e.item.is_magic())
        );
        let inn = stock_shop(&lib, ShopTheme::Tavern, ShopQuality::Average, 8, &mut r);
        assert!(inn.iter().any(|e| e.item.name.contains("room")));
        assert_eq!(shop_price(1234.0), 1250.0);
        assert_eq!(shop_price(0.437), 0.44);
    }
}
