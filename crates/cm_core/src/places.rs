//! Places: towns, shops, inns, temples, dungeons… with the NPCs and factions found there,
//! a name generator per kind of place, and (for shops) generated stock.

use serde::{Deserialize, Serialize};

use crate::loot::{ShopQuality, ShopTheme, StockEntry};
use crate::model::{Id, Link, new_id};
use crate::names::NamePool;

/// Suggested kinds of place. Any text is allowed; these get tailored name generators.
pub const PLACE_KINDS: &[&str] = &[
    "City",
    "Town",
    "Village",
    "Region",
    "Inn",
    "Tavern",
    "Shop",
    "Temple",
    "Guild hall",
    "Castle",
    "Dungeon",
    "Ruins",
    "Landmark",
    "Wilderness",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shop {
    pub theme: ShopTheme,
    pub quality: ShopQuality,
    pub stock: Vec<StockEntry>,
    /// The shopkeeper (an NPC), if any.
    pub keeper: Option<Id>,
}

impl Default for Shop {
    fn default() -> Self {
        Shop {
            theme: ShopTheme::General,
            quality: ShopQuality::Average,
            stock: Vec::new(),
            keeper: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Location {
    pub id: Id,
    pub name: String,
    /// "Town", "Inn", "Potion shop" — free text.
    pub kind: String,
    /// The place this one is in (a shop's town).
    pub parent: Option<Id>,
    pub description: String,
    pub notes: String,
    pub npcs: Vec<Id>,
    pub factions: Vec<Id>,
    /// Other related places (not children — those point here with `parent`).
    pub related: Vec<Id>,
    pub image: Option<String>,
    pub links: Vec<Link>,
    pub tags: Vec<String>,
    pub shop: Option<Shop>,
    /// A story card that stands for this place (on the plot line / story board).
    pub card: Option<Id>,
}

impl Default for Location {
    fn default() -> Self {
        Location {
            id: new_id(),
            name: String::new(),
            kind: String::new(),
            parent: None,
            description: String::new(),
            notes: String::new(),
            npcs: Vec::new(),
            factions: Vec::new(),
            related: Vec::new(),
            image: None,
            links: Vec::new(),
            tags: Vec::new(),
            shop: None,
            card: None,
        }
    }
}

impl Location {
    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() {
            "(unnamed place)".into()
        } else {
            self.name.trim().to_string()
        }
    }
}

/// The shop theme a kind of place suggests ("Potion shop" → Alchemist).
pub fn theme_for_kind(kind: &str) -> Option<ShopTheme> {
    let k = kind.to_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| k.contains(x));
    let rules: [(&[&str], ShopTheme); 9] = [
        (&["potion", "alchem", "apothec", "herbal"], ShopTheme::Alchemist),
        (&["smith", "armor", "armour", "weapon", "forge"], ShopTheme::Blacksmith),
        (&["magic", "curio", "arcane", "enchant"], ShopTheme::Magic),
        (&["scribe", "scroll", "book", "library"], ShopTheme::Scribe),
        (&["bow", "fletch", "archer"], ShopTheme::Fletcher),
        (&["jewel", "gem"], ShopTheme::Jeweler),
        (&["inn", "tavern", "pub", "alehouse"], ShopTheme::Tavern),
        (&["general", "store", "market", "trading post", "shop", "provision"], ShopTheme::General),
        (&["temple", "shrine", "chapel"], ShopTheme::Temple),
    ];
    rules.iter().find(|(words, _)| has(words)).map(|(_, t)| *t)
}

// ---------------------------------------------------------------------------
// Name generator (word lists are our own)
// ---------------------------------------------------------------------------

const INN_ADJ: &[&str] = &[
    "Prancing",
    "Gilded",
    "Rusty",
    "Sleeping",
    "Drunken",
    "Laughing",
    "Silver",
    "Crooked",
    "Wandering",
    "Jolly",
    "Salty",
    "Golden",
    "Green",
    "Howling",
    "Merry",
    "Wild",
    "Lucky",
    "Broken",
    "Hungry",
    "Red",
    "Blind",
    "Dancing",
    "Leaky",
    "Grinning",
    "Lonely",
    "Thirsty",
    "Copper",
    "Black",
    "Brave",
    "Stumbling",
];
const INN_NOUN: &[&str] = &[
    "Pony", "Dragon", "Goblet", "Stag", "Barrel", "Lantern", "Griffon", "Mermaid", "Anchor", "Boar", "Kettle", "Raven", "Hearth", "Crown", "Fox", "Wyvern",
    "Owl", "Goose", "Tankard", "Hound", "Bard", "Badger", "Wheel", "Rose", "Unicorn", "Ogre", "Pig", "Swan", "Cask", "Lamb",
];
const SHOP_NOUN: &[(ShopTheme, &[&str])] = &[
    (
        ShopTheme::General,
        &[
            "Goods",
            "Provisions",
            "Supplies",
            "Trading Post",
            "Sundries",
            "General Store",
            "Outfitters",
            "Wares",
        ],
    ),
    (
        ShopTheme::Blacksmith,
        &["Forge", "Anvil", "Smithy", "Arms", "Armory", "Ironworks", "Blades", "Steelworks"],
    ),
    (
        ShopTheme::Alchemist,
        &["Apothecary", "Elixirs", "Remedies", "Tinctures", "Potions", "Draughts", "Alchemy", "Brews"],
    ),
    (
        ShopTheme::Magic,
        &["Curios", "Emporium", "Arcana", "Oddities", "Wonders", "Relics", "Trinkets", "Enchantments"],
    ),
    (
        ShopTheme::Scribe,
        &["Scriptorium", "Quills", "Inkworks", "Books", "Scrolls", "Pages", "Bindery"],
    ),
    (ShopTheme::Fletcher, &["Arrows", "Bows", "Fletchery", "Bowyer", "Quivers", "Longbows"]),
    (ShopTheme::Jeweler, &["Gems", "Jewels", "Facets", "Settings", "Gemcutter", "Treasures"]),
    (ShopTheme::Temple, &["Offerings", "Candles", "Blessings", "Reliquary"]),
    (ShopTheme::Tavern, &["Taproom", "Alehouse"]),
];
const SHOP_ADJ: &[&str] = &[
    "Honest", "Golden", "Old", "Silver", "Hidden", "Twisted", "Lucky", "Humble", "Gleaming", "Ancient", "Bright", "Curious", "Crimson", "Moonlit", "Copper",
];
const OWNER: &[&str] = &[
    "Brindle", "Hob", "Mirela", "Tamsin", "Orrin", "Greta", "Fenwick", "Ysolde", "Bram", "Lark", "Quill", "Jory", "Thessaly", "Dunstan", "Ivo", "Marrow",
    "Pell", "Wren", "Corwin", "Hazel", "Odo", "Sabine", "Tobias", "Elspeth", "Garrick", "Nim", "Rook", "Vesna",
];
const TEMPLE_CONCEPT: &[&str] = &[
    "Dawn",
    "Morning Light",
    "Silver Moon",
    "Harvest",
    "Eternal Flame",
    "Tides",
    "Stars",
    "Open Hand",
    "Last Watch",
    "Quiet Mercy",
    "Iron Oath",
    "Green Mother",
    "Seven Lanterns",
    "Rising Sun",
    "Hidden Path",
    "First Rain",
    "Deep Earth",
    "Wandering Wind",
];
const TEMPLE_ADJ: &[&str] = &[
    "Radiant", "Holy", "Silent", "Golden", "Merciful", "Watchful", "Undying", "Blessed", "Shining", "Patient",
];
const TOWN_PRE: &[&str] = &[
    "Oak", "Raven", "Ash", "Mill", "Stone", "Thorn", "Elder", "Wolf", "Black", "Gold", "Red", "Frost", "Briar", "Hollow", "Kings", "Salt", "Iron", "Willow",
    "Deer", "Bright", "Marsh", "High", "Low", "Fox", "Copper", "Silver", "Moss", "Hazel", "Crow", "Heather", "Sheep", "Fern", "Cold", "Hawk", "Bram", "Lark",
];
const TOWN_SUF: &[&str] = &[
    "ford", "hollow", "bury", "brook", "wick", "stead", "vale", "haven", "field", "moor", "ridge", "wood", "mere", "dale", "gate", "watch", "hold", "fall",
    "crossing", "well", "ton", "by", "cliff", "barrow", "march",
];
const PLACE_ADJ: &[&str] = &[
    "Sunken",
    "Forgotten",
    "Shattered",
    "Whispering",
    "Hollow",
    "Drowned",
    "Burning",
    "Silent",
    "Howling",
    "Black",
    "Crimson",
    "Lost",
    "Cursed",
    "Broken",
    "Hidden",
    "Frozen",
    "Sleeping",
    "Weeping",
    "Ashen",
    "Gloom",
];
const DUNGEON_NOUN: &[&str] = &[
    "Crypt",
    "Vault",
    "Halls",
    "Catacombs",
    "Pit",
    "Warrens",
    "Sanctum",
    "Tomb",
    "Mines",
    "Labyrinth",
    "Barrow",
    "Undercroft",
    "Grotto",
    "Citadel",
];
const DUNGEON_OF: &[&str] = &[
    "the Lich King",
    "Ten Thousand Bones",
    "the Serpent Queen",
    "Lost Souls",
    "the Mad Mage",
    "Ashes",
    "the Deep",
    "Endless Night",
    "the Forgotten God",
    "Chains",
];
const WILD_NOUN: &[&str] = &[
    "Woods", "Hills", "Marsh", "Peaks", "Fen", "Moors", "Gorge", "Falls", "Wastes", "Glade", "Downs", "Lake", "Coast", "Forest", "Pass", "Heath",
];
const KEEP_NOUN: &[&str] = &["Keep", "Castle", "Tower", "Hold", "Fortress", "Citadel", "Bastion", "Manor"];
const GUILD_NOUN: &[&str] = &["Guild", "Company", "Brotherhood", "Lodge", "Society", "Circle", "Union", "Fellowship"];
const GUILD_TRADE: &[&str] = &[
    "Merchants",
    "Masons",
    "Weavers",
    "Smiths",
    "Shipwrights",
    "Alchemists",
    "Cartographers",
    "Brewers",
    "Lamplighters",
    "Coin-changers",
];

fn p<'a>(rng: &mut fastrand::Rng, list: &'a [&'a str]) -> &'a str {
    list[rng.usize(..list.len())]
}

fn town(rng: &mut fastrand::Rng) -> String {
    let suf = p(rng, TOWN_SUF);
    let pre = p(rng, TOWN_PRE);
    if suf == "crossing" || suf == "watch" && rng.bool() {
        format!("{pre} {}", capital(suf))
    } else {
        format!("{pre}{suf}")
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn shop_name(theme: ShopTheme, rng: &mut fastrand::Rng) -> String {
    let nouns = SHOP_NOUN.iter().find(|(t, _)| *t == theme).map(|(_, n)| *n).unwrap_or(SHOP_NOUN[0].1);
    match rng.u8(0..4) {
        0 => format!("{}'s {}", p(rng, OWNER), p(rng, nouns)),
        1 => format!("The {} {}", p(rng, SHOP_ADJ), p(rng, nouns)),
        2 => format!("{} & {} {}", p(rng, OWNER), p(rng, OWNER), p(rng, nouns)),
        _ => format!("The {} {}", p(rng, INN_NOUN), p(rng, nouns)),
    }
}

/// One name for a kind of place. A custom name pool whose name matches the kind (e.g. a pool
/// named "Inn") is used too: "first part + last part".
pub fn place_name(kind: &str, custom: &[NamePool], rng: &mut fastrand::Rng) -> String {
    if let Some(cp) = custom.iter().find(|c| c.name.trim().eq_ignore_ascii_case(kind.trim()) && !c.first.is_empty()) {
        if rng.u8(0..3) > 0 {
            let f = &cp.first[rng.usize(..cp.first.len())];
            return if cp.last.is_empty() {
                f.clone()
            } else {
                format!("{f} {}", cp.last[rng.usize(..cp.last.len())])
            };
        }
    }
    let k = kind.to_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| k.contains(x));
    if has(&["inn", "tavern", "pub", "alehouse"]) {
        return match rng.u8(0..5) {
            0 | 1 => format!("The {} {}", p(rng, INN_ADJ), p(rng, INN_NOUN)),
            2 => format!("The {} and {}", p(rng, INN_NOUN), p(rng, INN_NOUN)),
            3 => format!("The {}'s Rest", p(rng, INN_NOUN)),
            _ => format!("{}'s {}", p(rng, OWNER), if rng.bool() { "Inn" } else { "Tavern" }),
        };
    }
    if let Some(theme) = theme_for_kind(&k).filter(|t| *t != ShopTheme::Tavern && *t != ShopTheme::Temple) {
        return shop_name(theme, rng);
    }
    if has(&["temple", "shrine", "chapel", "church", "monastery", "abbey"]) {
        return match rng.u8(0..4) {
            0 => format!("Temple of the {}", p(rng, TEMPLE_CONCEPT)),
            1 => format!("Shrine of the {} {}", p(rng, TEMPLE_ADJ), p(rng, TEMPLE_CONCEPT)),
            2 => format!("Chapel of the {}", p(rng, TEMPLE_CONCEPT)),
            _ => format!(
                "The {} {} Abbey",
                p(rng, TEMPLE_ADJ),
                p(rng, TEMPLE_CONCEPT).split(' ').last().unwrap_or("Light")
            ),
        };
    }
    if has(&["dungeon", "ruin", "crypt", "tomb", "cave", "lair", "mine"]) {
        return match rng.u8(0..3) {
            0 => format!("The {} {}", p(rng, PLACE_ADJ), p(rng, DUNGEON_NOUN)),
            1 => format!("{} of {}", p(rng, DUNGEON_NOUN), p(rng, DUNGEON_OF)),
            _ => format!("The {} of {}", p(rng, DUNGEON_NOUN), town(rng)),
        };
    }
    if has(&["castle", "keep", "fort", "tower", "manor", "citadel"]) {
        return format!("{} {}", town(rng), p(rng, KEEP_NOUN));
    }
    if has(&["guild", "hall", "lodge", "society"]) {
        return format!("The {} {} of {}", p(rng, SHOP_ADJ), p(rng, GUILD_NOUN), p(rng, GUILD_TRADE));
    }
    if has(&["wild", "forest", "landmark", "region", "hill", "mountain", "swamp", "lake", "river"]) {
        return format!("The {} {}", p(rng, PLACE_ADJ), p(rng, WILD_NOUN));
    }
    // towns, villages, cities and anything else
    town(rng)
}

pub fn place_names(kind: &str, custom: &[NamePool], count: usize, rng: &mut fastrand::Rng) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut tries = 0;
    while out.len() < count && tries < count * 20 {
        let n = place_name(kind, custom, rng);
        if !out.contains(&n) {
            out.push(n);
        }
        tries += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_fit_their_kind() {
        let mut rng = fastrand::Rng::with_seed(3);
        let inns = place_names("Inn", &[], 20, &mut rng);
        assert_eq!(inns.len(), 20);
        assert!(inns.iter().all(|n| n.starts_with("The ") || n.contains("'s ")), "{inns:?}");
        let potions = place_names("Potion shop", &[], 10, &mut rng);
        let nouns = SHOP_NOUN.iter().find(|(t, _)| *t == ShopTheme::Alchemist).unwrap().1;
        assert!(potions.iter().all(|n| nouns.iter().any(|w| n.ends_with(w))), "{potions:?}");
        assert!(
            place_names("Temple", &[], 5, &mut rng)
                .iter()
                .all(|n| ["Temple", "Shrine", "Chapel", "Abbey"].iter().any(|w| n.contains(w)))
        );
        let towns = place_names("Village", &[], 5, &mut rng);
        assert!(towns.iter().all(|n| !n.is_empty() && n.chars().next().unwrap().is_uppercase()));
        let weird = place_name("Floating sky-whale", &[], &mut rng);
        assert!(!weird.is_empty());
        // a custom pool named like the kind is used
        let pool = NamePool {
            name: "Brothel".into(),
            first: vec!["The Velvet".into()],
            last: vec!["Rose".into()],
        };
        let names = place_names("brothel", &[pool], 10, &mut rng);
        assert!(names.contains(&"The Velvet Rose".to_string()));
    }

    #[test]
    fn kinds_suggest_shop_themes() {
        assert_eq!(theme_for_kind("Potion shop"), Some(ShopTheme::Alchemist));
        assert_eq!(theme_for_kind("Blacksmith"), Some(ShopTheme::Blacksmith));
        assert_eq!(theme_for_kind("Inn"), Some(ShopTheme::Tavern));
        assert_eq!(theme_for_kind("Village"), None);
    }
}
