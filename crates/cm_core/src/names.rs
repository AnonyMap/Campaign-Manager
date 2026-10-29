//! NPC name generator. Built-in pools per species plus background flavour (titles,
//! epithets, occupational surnames), a syllable generator for effectively unlimited
//! variety, and DM-defined custom pools stored in the campaign.

use serde::{Deserialize, Serialize};

/// A DM-defined pool of names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NamePool {
    pub name: String,
    pub first: Vec<String>,
    pub last: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinPool {
    pub name: &'static str,
    pub first: &'static [&'static str],
    pub last: &'static [&'static str],
    /// Syllables used to invent new names: (starts, middles, ends).
    pub syl: (&'static [&'static str], &'static [&'static str], &'static [&'static str]),
}

pub const HUMAN: BuiltinPool = BuiltinPool {
    name: "Human",
    first: &[
        "Aldric",
        "Bram",
        "Cedric",
        "Doran",
        "Edmund",
        "Fenwick",
        "Garrett",
        "Hollis",
        "Ivo",
        "Jory",
        "Kendrick",
        "Lucan",
        "Marten",
        "Niall",
        "Osric",
        "Percival",
        "Quentin",
        "Roderick",
        "Silas",
        "Tobin",
        "Ulric",
        "Vance",
        "Wendel",
        "Yorick",
        "Alistair",
        "Benedict",
        "Corwin",
        "Dashiel",
        "Emeric",
        "Gideon",
        "Adela",
        "Brienne",
        "Cecily",
        "Delphine",
        "Elowen",
        "Freya",
        "Gwendolyn",
        "Helena",
        "Isolde",
        "Johanna",
        "Katrin",
        "Lisbet",
        "Maren",
        "Nessa",
        "Odette",
        "Petra",
        "Rosalind",
        "Sabine",
        "Tamsin",
        "Ursula",
        "Vivienne",
        "Wilhelmina",
        "Ysolde",
        "Agnes",
        "Beatrix",
        "Clemence",
        "Dagny",
        "Edda",
        "Fiora",
        "Greta",
        "Hale",
        "Rowan",
        "Ashby",
        "Quinn",
        "Sorrel",
        "Linden",
        "Wren",
        "Morgan",
        "Emery",
        "Avery",
    ],
    last: &[
        "Ashdown",
        "Blackwood",
        "Brightwater",
        "Calloway",
        "Cresswell",
        "Dunmore",
        "Elsworth",
        "Fairbanks",
        "Galloway",
        "Hawthorne",
        "Ironside",
        "Kettering",
        "Langley",
        "Marlowe",
        "Northcott",
        "Oakhurst",
        "Pemberton",
        "Ravenscroft",
        "Redfern",
        "Stanmore",
        "Thornbury",
        "Underhill",
        "Vale",
        "Whitlock",
        "Winterbourne",
        "Yarrow",
        "Holloway",
        "Merriweather",
        "Penhallow",
        "Greaves",
        "Fletcher",
        "Cooper",
        "Thatcher",
        "Mercer",
        "Chandler",
        "Carver",
        "Tanner",
        "Weaver",
        "Brewer",
        "Miller",
        "Harrow",
        "Stroud",
    ],
    syl: (
        &[
            "Al", "Ber", "Cor", "Dal", "Ed", "Gar", "Hal", "Jor", "Mar", "Ro", "Sel", "Tor", "Wil", "El", "An", "Is",
        ],
        &["a", "e", "i", "o", "an", "en", "ri", "la", "mo"],
        &["ric", "wyn", "da", "na", "son", "ton", "mund", "bert", "lyn", "ette", "ard", "is"],
    ),
};

pub const ELF: BuiltinPool = BuiltinPool {
    name: "Elf",
    first: &[
        "Aelar",
        "Aerendyl",
        "Caelynn",
        "Elandor",
        "Faelar",
        "Galinndan",
        "Haldir",
        "Ilphas",
        "Ivellios",
        "Lorien",
        "Mindartis",
        "Naeris",
        "Paelias",
        "Quarion",
        "Riardon",
        "Soveliss",
        "Thamior",
        "Thervan",
        "Varis",
        "Erevan",
        "Adrie",
        "Althaea",
        "Anastrianna",
        "Bethrynna",
        "Birel",
        "Drusilia",
        "Enna",
        "Felosial",
        "Ielenia",
        "Jelenneth",
        "Keyleth",
        "Leshanna",
        "Lia",
        "Meriele",
        "Mialee",
        "Naivara",
        "Quelenna",
        "Sariel",
        "Shanairra",
        "Silaqui",
        "Thiala",
        "Valanthe",
        "Xanaphia",
        "Arannis",
        "Celeborn",
        "Elrohen",
        "Faenya",
        "Gaelira",
        "Ilyndra",
        "Lyriel",
        "Nimeth",
        "Oriel",
        "Sylvaris",
        "Taelion",
        "Vaelith",
        "Yllanis",
        "Zentha",
        "Aerith",
        "Ellisar",
        "Myrdin",
    ],
    last: &[
        "Amakiir",
        "Amastacia",
        "Galanodel",
        "Holimion",
        "Ilphelkiir",
        "Liadon",
        "Meliamne",
        "Nailo",
        "Siannodel",
        "Xiloscient",
        "Brightleaf",
        "Moonwhisper",
        "Starbloom",
        "Silverfrond",
        "Dawnstrider",
        "Evenwood",
        "Gemblossom",
        "Mistvale",
        "Nightbreeze",
        "Oakenshade",
        "Riverwind",
        "Sunfire",
        "Thistledown",
        "Willowmere",
        "Ashgrove",
        "Duskmantle",
        "Feyheart",
        "Glimmerdew",
        "Lightsong",
        "Shadowmoss",
        "Stillwater",
        "Windrunner",
    ],
    syl: (
        &["Ae", "Ca", "El", "Fae", "Gal", "Il", "Lae", "Myr", "Nae", "Syl", "Tha", "Va", "Xi", "Ye", "Ri"],
        &["la", "ri", "na", "the", "lia", "ae", "ly", "ra", "ne", "si"],
        &["ndra", "rion", "thil", "wyn", "lis", "nor", "riel", "dor", "las", "nya", "ssa", "thas"],
    ),
};

pub const DWARF: BuiltinPool = BuiltinPool {
    name: "Dwarf",
    first: &[
        "Adrik", "Baern", "Barendd", "Brottor", "Bruenor", "Dain", "Darrak", "Eberk", "Einkil", "Fargrim", "Flint", "Gardain", "Harbek", "Kildrak", "Morgran",
        "Orsik", "Oskar", "Rangrim", "Rurik", "Taklinn", "Thoradin", "Thorin", "Tordek", "Traubon", "Travok", "Ulfgar", "Veit", "Vondal", "Balin", "Gimrik",
        "Amber", "Artin", "Audhild", "Bardryn", "Dagnal", "Diesa", "Eldeth", "Falkrunn", "Finellen", "Gunnloda", "Gurdis", "Helja", "Hlin", "Kathra",
        "Kristryd", "Ilde", "Liftrasa", "Mardred", "Riswynn", "Sannl", "Torbera", "Torgga", "Vistra", "Brunhild", "Dagmar", "Hilde", "Ragna", "Sigrun",
        "Thyra", "Yrsa",
    ],
    last: &[
        "Balderk",
        "Battlehammer",
        "Brawnanvil",
        "Dankil",
        "Fireforge",
        "Frostbeard",
        "Gorunn",
        "Holderhek",
        "Ironfist",
        "Loderr",
        "Lutgehr",
        "Rumnaheim",
        "Strakeln",
        "Torunn",
        "Ungart",
        "Stonebreaker",
        "Deepdelver",
        "Coppervein",
        "Granitejaw",
        "Hammerfall",
        "Anvilborn",
        "Blackforge",
        "Goldhand",
        "Ironvein",
        "Mithrilbrow",
        "Oakenshield",
        "Rockseeker",
        "Steelbeard",
        "Thunderhelm",
        "Bronzebottom",
        "Graniteheart",
    ],
    syl: (
        &[
            "Bal", "Bor", "Dur", "Gim", "Gron", "Har", "Kil", "Mor", "Thor", "Ulf", "Brun", "Dag", "Hel", "Tor",
        ],
        &["a", "i", "o", "u", "ra", "ri", "gr"],
        &["din", "dek", "rak", "grim", "nar", "rik", "li", "dis", "hild", "gga", "unn", "ek"],
    ),
};

pub const HALFLING: BuiltinPool = BuiltinPool {
    name: "Halfling",
    first: &[
        "Alton",
        "Ander",
        "Cade",
        "Corrin",
        "Eldon",
        "Errich",
        "Finnan",
        "Garret",
        "Lindal",
        "Lyle",
        "Merric",
        "Milo",
        "Osborn",
        "Perrin",
        "Reed",
        "Roscoe",
        "Wellby",
        "Bingo",
        "Dodo",
        "Fosco",
        "Andry",
        "Bree",
        "Callie",
        "Cora",
        "Euphemia",
        "Jillian",
        "Kithri",
        "Lavinia",
        "Lidda",
        "Merla",
        "Nedda",
        "Paela",
        "Portia",
        "Seraphina",
        "Shaena",
        "Trym",
        "Vani",
        "Verna",
        "Daisy",
        "Pansy",
        "Poppy",
        "Rosie",
        "Tansy",
        "Myrtle",
        "Clover",
        "Hazel",
        "Pip",
        "Tobold",
        "Wilcome",
        "Hamfast",
    ],
    last: &[
        "Brushgather",
        "Goodbarrel",
        "Greenbottle",
        "High-hill",
        "Hilltopple",
        "Leagallow",
        "Tealeaf",
        "Thorngage",
        "Tosscobble",
        "Underbough",
        "Applebrook",
        "Bramblefoot",
        "Butterbur",
        "Cherrycheeks",
        "Copperkettle",
        "Dewdrop",
        "Fairweather",
        "Goldfound",
        "Honeypot",
        "Longbottom",
        "Mossbank",
        "Puddifoot",
        "Quickstep",
        "Riverhopper",
        "Swiftwhistle",
        "Thistlewood",
        "Warmhearth",
        "Wheatfield",
    ],
    syl: (
        &["Al", "Bil", "Cor", "Dro", "Fen", "Hal", "Lil", "Mer", "Pip", "Ros", "Tam", "Wil", "Bun", "Mol"],
        &["a", "i", "o", "y", "le", "ri"],
        &["bo", "do", "lin", "mo", "ric", "ry", "wise", "sy", "la", "bell", "ly", "ton"],
    ),
};

pub const GNOME: BuiltinPool = BuiltinPool {
    name: "Gnome",
    first: &[
        "Alston",
        "Alvyn",
        "Boddynock",
        "Brocc",
        "Burgell",
        "Dimble",
        "Eldon",
        "Erky",
        "Fonkin",
        "Frug",
        "Gerbo",
        "Gimble",
        "Glim",
        "Jebeddo",
        "Kellen",
        "Namfoodle",
        "Orryn",
        "Roondar",
        "Seebo",
        "Sindri",
        "Warryn",
        "Wrenn",
        "Zook",
        "Bimpnottin",
        "Breena",
        "Caramip",
        "Carlin",
        "Donella",
        "Duvamil",
        "Ella",
        "Ellyjobell",
        "Ellywick",
        "Lilli",
        "Loopmottin",
        "Lorilla",
        "Mardnab",
        "Nissa",
        "Nyx",
        "Oda",
        "Orla",
        "Roywyn",
        "Shamil",
        "Tana",
        "Waywocket",
        "Zanna",
        "Fizzwick",
        "Tinkerbo",
        "Quillby",
        "Sprocket",
        "Nim",
    ],
    last: &[
        "Beren",
        "Daergel",
        "Folkor",
        "Garrick",
        "Nackle",
        "Murnig",
        "Ningel",
        "Raulnor",
        "Scheppen",
        "Timbers",
        "Turen",
        "Cogspinner",
        "Fizzlebang",
        "Gearwhistle",
        "Glitterglow",
        "Nackleberry",
        "Pocketfuzz",
        "Quickcrank",
        "Sparkletoe",
        "Springsprocket",
        "Tinkertop",
        "Whirligig",
        "Bafflestone",
        "Copperpot",
    ],
    syl: (
        &["Bim", "Bod", "Fiz", "Gim", "Nim", "Pim", "Quil", "Tink", "Wib", "Zook", "Fon", "Ell"],
        &["ble", "dle", "ka", "li", "mo", "ni", "py", "ty"],
        &["bo", "nock", "wick", "kin", "pip", "zle", "lo", "na", "bell", "ry", "dle", "nottin"],
    ),
};

pub const ORC: BuiltinPool = BuiltinPool {
    name: "Orc / Half-Orc",
    first: &[
        "Dench", "Feng", "Gell", "Henk", "Holg", "Imsh", "Keth", "Krusk", "Mhurren", "Ront", "Shump", "Thokk", "Grukk", "Morg", "Durth", "Brakk", "Ghazat",
        "Uzgash", "Varg", "Zorn", "Baggi", "Emen", "Engong", "Kansif", "Myev", "Neega", "Ovak", "Ownka", "Shautha", "Sutha", "Vola", "Volen", "Yevelda",
        "Grisha", "Ugra", "Murza", "Rasha", "Thrak", "Gorza", "Krenna",
    ],
    last: &[
        "Bonecrusher",
        "Bloodfang",
        "Skullsplitter",
        "Ironhide",
        "Ashmaw",
        "Blackthorn",
        "Gorefist",
        "Grimtusk",
        "Rageborn",
        "Redaxe",
        "Stonejaw",
        "Thunderfoot",
        "Wolfscar",
        "Ember-eye",
        "Halfhorn",
        "Frostscar",
        "Deadeye",
        "Dreadmane",
        "Gutripper",
        "Splitlip",
    ],
    syl: (
        &["Gr", "Kr", "Th", "Ur", "Mo", "Dr", "Ba", "Zu", "Gh", "Sh", "Va", "Ru"],
        &["a", "o", "u", "ag", "ug", "or"],
        &["k", "gash", "zat", "rok", "th", "nak", "ga", "sha", "ra", "dul", "mok", "z"],
    ),
};

pub const TIEFLING: BuiltinPool = BuiltinPool {
    name: "Tiefling",
    first: &[
        "Akmenos",
        "Amnon",
        "Barakas",
        "Damakos",
        "Ekemon",
        "Iados",
        "Kairon",
        "Leucis",
        "Melech",
        "Mordai",
        "Morthos",
        "Pelaios",
        "Skamos",
        "Therai",
        "Zariel",
        "Malcer",
        "Azrakel",
        "Cassiel",
        "Vexx",
        "Xyrius",
        "Akta",
        "Anakis",
        "Bryseis",
        "Criella",
        "Damaia",
        "Ea",
        "Kallista",
        "Lerissa",
        "Makaria",
        "Nemeia",
        "Orianna",
        "Phelaia",
        "Rieta",
        "Lilith",
        "Seraxa",
        "Vashti",
        "Ziva",
        "Nyxara",
        "Belladonna",
        "Morrigan",
        "Ambition",
        "Art",
        "Carrion",
        "Chant",
        "Creed",
        "Despair",
        "Excellence",
        "Fear",
        "Glory",
        "Hope",
        "Ideal",
        "Music",
        "Nowhere",
        "Open",
        "Poetry",
        "Quest",
        "Random",
        "Reverence",
        "Sorrow",
        "Torment",
    ],
    last: &[
        "Ashborne",
        "Cindervale",
        "Duskmourn",
        "Embercloak",
        "Grimsong",
        "Hellfeather",
        "Nightsorrow",
        "Shadowhorn",
        "Smokewreath",
        "Thornheart",
        "Vesperhall",
        "Wraithwood",
        "Blackbrand",
        "Brimstone",
        "Crowe",
        "Morn",
    ],
    syl: (
        &["Ak", "Az", "Bel", "Dam", "Kai", "Mal", "Mor", "Nyx", "Ser", "Vex", "Xy", "Zar", "Lil", "Mak"],
        &["a", "e", "i", "ra", "ri", "the"],
        &["mos", "kas", "ion", "ael", "iel", "is", "ia", "xa", "ith", "ora", "ek", "os"],
    ),
};

pub const DRAGONBORN: BuiltinPool = BuiltinPool {
    name: "Dragonborn",
    first: &[
        "Arjhan", "Balasar", "Bharash", "Donaar", "Ghesh", "Heskan", "Kriv", "Medrash", "Mehen", "Nadarr", "Pandjed", "Patrin", "Rhogar", "Shamash", "Shedinn",
        "Tarhun", "Torinn", "Vrakos", "Zorvath", "Kaldrax", "Akra", "Biri", "Daar", "Farideh", "Harann", "Havilar", "Jheri", "Kava", "Korinn", "Mishann",
        "Nala", "Perra", "Raiann", "Sora", "Surina", "Thava", "Uadjit", "Vyrsa", "Zhaira", "Kyrra",
    ],
    last: &[
        "Clethtinthiallor",
        "Daardendrian",
        "Delmirev",
        "Drachedandion",
        "Fenkenkabradon",
        "Kepeshkmolik",
        "Kerrhylon",
        "Kimbatuul",
        "Linxakasendalor",
        "Myastan",
        "Nemmonis",
        "Norixius",
        "Ophinshtalajiir",
        "Prexijandilin",
        "Shestendeliath",
        "Turnuroth",
        "Verthisathurgiesh",
        "Yarjerit",
    ],
    syl: (
        &["Ar", "Bal", "Dra", "Kri", "Med", "Rho", "Sha", "Tor", "Vra", "Zor", "Kal", "Ny"],
        &["a", "ra", "ka", "sha", "the", "dri"],
        &["jhan", "sar", "rash", "gar", "vath", "drax", "rinn", "ra", "ann", "ith", "us", "kos"],
    ),
};

pub const GOLIATH: BuiltinPool = BuiltinPool {
    name: "Goliath",
    first: &[
        "Aukan", "Eglath", "Gae-Al", "Gauthak", "Ilikan", "Keothi", "Kuori", "Lo-Kag", "Manneo", "Maveith", "Nalla", "Orilo", "Paavu", "Pethani", "Thalai",
        "Thotham", "Uthal", "Vaunea", "Vimak", "Kavaki",
    ],
    last: &[
        "Bearkiller",
        "Dawncaller",
        "Fearless",
        "Flintfinder",
        "Horncarver",
        "Keeneye",
        "Lonehunter",
        "Longleaper",
        "Rootsmasher",
        "Skywatcher",
        "Steadyhand",
        "Threadtwister",
        "Twice-Orphaned",
        "Twistedlimb",
        "Wordpainter",
        "Stormclimber",
        "Peakstrider",
        "Cloudbreaker",
    ],
    syl: (
        &["Au", "Eg", "Ga", "Ka", "Ko", "Lo", "Ma", "Na", "Pa", "Tha", "Va", "Vi"],
        &["u", "ka", "la", "o", "ve", "thi"],
        &["kan", "lath", "thak", "kag", "neo", "veith", "la", "lo", "vu", "mak", "ki", "tham"],
    ),
};

pub const GOBLINOID: BuiltinPool = BuiltinPool {
    name: "Goblinoid / Kobold",
    first: &[
        "Snik",
        "Grib",
        "Nackle",
        "Rizz",
        "Skulk",
        "Pog",
        "Drek",
        "Murk",
        "Yip",
        "Zat",
        "Klik",
        "Snarl",
        "Vrik",
        "Gnash",
        "Skab",
        "Fetch",
        "Twitch",
        "Mog",
        "Nub",
        "Sprak",
        "Kurtulmak",
        "Meepo",
        "Irrik",
        "Draxi",
        "Vex",
        "Tazz",
        "Kriss",
        "Zikka",
        "Iska",
        "Rakka",
    ],
    last: &[
        "the Sneaky",
        "Ratbiter",
        "Knifewit",
        "Mudfoot",
        "Shinkicker",
        "Tunnelrat",
        "Scabface",
        "Twotoes",
        "Gutgnaw",
        "Sootcheek",
        "of the Pit",
        "Trapmaker",
    ],
    syl: (
        &["Sn", "Gr", "Kr", "Sk", "Z", "Dr", "Gn", "Vr", "Kl", "Ts"],
        &["i", "a", "u", "ee", "o"],
        &["k", "b", "zz", "x", "g", "rk", "ka", "po", "t", "ck"],
    ),
};

pub const BUILTIN_POOLS: &[BuiltinPool] = &[HUMAN, ELF, DWARF, HALFLING, GNOME, ORC, TIEFLING, DRAGONBORN, GOLIATH, GOBLINOID];

/// D&D-style backgrounds that colour the generated name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Background {
    pub name: &'static str,
    pub titles: &'static [&'static str],
    pub epithets: &'static [&'static str],
    /// Occupational surnames used instead of the species surname sometimes.
    pub surnames: &'static [&'static str],
}

pub const BACKGROUNDS: &[Background] = &[
    Background {
        name: "Noble",
        titles: &["Lord", "Lady", "Baron", "Baroness", "Count", "Countess", "Sir", "Dame", "Duke", "Duchess"],
        epithets: &["of Highmere", "of Ravensgate", "the Elder", "the Younger", "of House Valen", "of Thornwall"],
        surnames: &["Ashcombe", "Delacroix", "Montclair", "Vandermere", "Wexley", "Harrowgate"],
    },
    Background {
        name: "Criminal",
        titles: &[],
        epithets: &[
            "'Quickfingers'",
            "'the Knife'",
            "'Two-Coins'",
            "'Smiles'",
            "'the Rat'",
            "'Ghost'",
            "'Lucky'",
            "'Ash'",
        ],
        surnames: &["Blackhand", "Cutpurse", "Shade", "Locke", "Fenn", "Crowley"],
    },
    Background {
        name: "Sailor",
        titles: &["Captain", "Bosun", "First Mate", "Old"],
        epithets: &["'Saltbeard'", "'Barnacle'", "of the Gull's Wake", "'Stormy'", "'Tidewater'"],
        surnames: &["Saltgrave", "Brinemoor", "Tidewell", "Seaworth", "Keelhaul", "Harbor"],
    },
    Background {
        name: "Acolyte",
        titles: &["Brother", "Sister", "Father", "Mother", "Deacon", "Abbot", "Prioress"],
        epithets: &["the Pious", "of the Dawn", "the Penitent", "of the Silent Order", "the Devout"],
        surnames: &["Candlewick", "Pryor", "Bell", "Chapel", "Graceford"],
    },
    Background {
        name: "Soldier",
        titles: &["Sergeant", "Corporal", "Captain", "Commander", "Private", "Lieutenant"],
        epithets: &["'Ironjaw'", "the Unbroken", "of the Third Company", "'Shield'", "'Grim'"],
        surnames: &["Shieldwall", "Spearman", "Bowyer", "Steele", "Marshall"],
    },
    Background {
        name: "Sage",
        titles: &["Master", "Archivist", "Loremaster", "Magister", "Scholar"],
        epithets: &["the Learned", "of the Grey Library", "the Wise", "the Bookish"],
        surnames: &["Inkwell", "Quill", "Parchment", "Scrivener", "Lorewright"],
    },
    Background {
        name: "Merchant",
        titles: &["Master", "Mistress", "Guildmaster"],
        epithets: &["the Shrewd", "'Silvertongue'", "of the Coin Guild", "'Fair-Price'"],
        surnames: &["Goldsworth", "Coinwell", "Trader", "Chapman", "Mercer", "Silvermark"],
    },
    Background {
        name: "Farmer / Folk Hero",
        titles: &["Goodman", "Goodwife", "Old"],
        epithets: &["of the Hollow", "the Brave", "'Oxheart'", "of Millbrook", "'Haybale'"],
        surnames: &["Fields", "Barley", "Plowright", "Shepherd", "Thresher", "Haywood"],
    },
    Background {
        name: "Entertainer",
        titles: &["Maestro", "the Great", "Madame"],
        epithets: &["the Magnificent", "'Songbird'", "the Fabulous", "'Lark'", "the Masked"],
        surnames: &["Lute", "Harper", "Piper", "Fiddler", "Jester", "Starling"],
    },
    Background {
        name: "Hermit",
        titles: &["Old", "Mother", "Father"],
        epithets: &["of the Woods", "the Hermit", "who Waits", "of the Lonely Hill", "the Mad"],
        surnames: &["Moss", "Stone", "Fern", "Hollow", "Cairn"],
    },
];

/// What to generate from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NameRequest {
    /// Built-in pool name (e.g. "Elf"), custom pool name, or None for any built-in.
    pub pool: Option<String>,
    /// Background name (e.g. "Noble") for titles / epithets.
    pub background: Option<String>,
    /// Use the syllable generator for the first name (invented names).
    pub invented: bool,
    /// Include a surname.
    pub surname: bool,
}

fn pick<'a, T>(rng: &mut fastrand::Rng, items: &'a [T]) -> Option<&'a T> {
    if items.is_empty() { None } else { Some(&items[rng.usize(..items.len())]) }
}

fn invent(rng: &mut fastrand::Rng, p: &BuiltinPool) -> String {
    let (s, m, e) = p.syl;
    let mut name = String::new();
    name.push_str(pick(rng, s).copied().unwrap_or("A"));
    if rng.u8(0..3) == 0 {
        name.push_str(pick(rng, m).copied().unwrap_or(""));
    }
    name.push_str(pick(rng, e).copied().unwrap_or(""));
    name
}

/// Total number of distinct full names the built-in lists can produce (not counting
/// invented names or backgrounds) — shown in the UI to give a sense of pool size.
pub fn builtin_combinations() -> usize {
    BUILTIN_POOLS.iter().map(|p| p.first.len() * p.last.len().max(1)).sum()
}

pub fn builtin_pool(name: &str) -> Option<&'static BuiltinPool> {
    BUILTIN_POOLS.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

pub fn background(name: &str) -> Option<&'static Background> {
    BACKGROUNDS.iter().find(|b| b.name.eq_ignore_ascii_case(name))
}

/// Generate a name.
pub fn generate(req: &NameRequest, custom: &[NamePool], rng: &mut fastrand::Rng) -> String {
    let bg = req.background.as_deref().and_then(background);
    let custom_pool = req.pool.as_deref().and_then(|n| custom.iter().find(|p| p.name.eq_ignore_ascii_case(n)));

    let (mut first, mut last) = if let Some(cp) = custom_pool {
        let f = pick(rng, &cp.first).cloned().unwrap_or_else(|| "Nameless".into());
        let l = pick(rng, &cp.last).cloned();
        (f, l)
    } else {
        let bp = req.pool.as_deref().and_then(builtin_pool).unwrap_or_else(|| pick(rng, BUILTIN_POOLS).unwrap());
        let f = if req.invented {
            invent(rng, bp)
        } else {
            pick(rng, bp.first).unwrap().to_string()
        };
        let l = pick(rng, bp.last).map(|s| s.to_string());
        (f, l)
    };
    if !req.surname {
        last = None;
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some(bg) = bg {
        // occasionally swap surname for an occupational one
        if req.surname && !bg.surnames.is_empty() && rng.u8(0..3) == 0 {
            last = pick(rng, bg.surnames).map(|s| s.to_string());
        }
        let has_title = !bg.titles.is_empty() && rng.bool();
        if has_title {
            parts.push(pick(rng, bg.titles).unwrap().to_string());
        }
        let first_idx = parts.len();
        parts.push(std::mem::take(&mut first));
        if let Some(l) = last.take() {
            parts.push(l);
        }
        if !bg.epithets.is_empty() && rng.u8(0..2) == 0 {
            let ep = pick(rng, bg.epithets).unwrap();
            if ep.starts_with('\'') {
                // a quoted nickname goes right after the first name
                parts.insert(first_idx + 1, ep.to_string());
            } else {
                parts.push(ep.to_string());
            }
        }
    } else {
        parts.push(first);
        if let Some(l) = last {
            parts.push(l);
        }
    }
    parts.join(" ")
}

/// Generate several unique names.
pub fn generate_many(req: &NameRequest, custom: &[NamePool], count: usize, rng: &mut fastrand::Rng) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut tries = 0;
    while out.len() < count && tries < count * 20 {
        let n = generate(req, custom, rng);
        if !out.contains(&n) {
            out.push(n);
        }
        tries += 1;
    }
    out
}

/// Parse a custom pool from text: one name per line; "First Last" lines are split.
pub fn pool_from_text(name: &str, first_text: &str, last_text: &str) -> NamePool {
    let clean = |t: &str| -> Vec<String> {
        let mut v: Vec<String> = t.split(['\n', ',']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        v.dedup();
        v
    };
    NamePool {
        name: name.trim().to_string(),
        first: clean(first_text),
        last: clean(last_text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pools_are_large_and_unique() {
        for p in BUILTIN_POOLS {
            assert!(p.first.len() >= 20, "{} too small", p.name);
            let mut f = p.first.to_vec();
            f.sort();
            let before = f.len();
            f.dedup();
            assert_eq!(before, f.len(), "duplicate first names in {}", p.name);
        }
        assert!(builtin_combinations() > 10_000);
    }

    #[test]
    fn generates_from_requested_pool() {
        let mut rng = fastrand::Rng::with_seed(7);
        let req = NameRequest {
            pool: Some("Dwarf".into()),
            surname: true,
            ..Default::default()
        };
        for _ in 0..50 {
            let n = generate(&req, &[], &mut rng);
            let first = n.split(' ').next().unwrap();
            assert!(DWARF.first.contains(&first), "{n}");
        }
    }

    #[test]
    fn custom_pool_and_backgrounds() {
        let mut rng = fastrand::Rng::with_seed(1);
        let pool = pool_from_text("Pirates", "Anne\nMary, Jack\n\n", "Bonny\nRead");
        assert_eq!(pool.first, vec!["Anne", "Mary", "Jack"]);
        let req = NameRequest {
            pool: Some("pirates".into()),
            surname: true,
            ..Default::default()
        };
        let n = generate(&req, &[pool.clone()], &mut rng);
        assert!(["Anne", "Mary", "Jack"].iter().any(|f| n.starts_with(f)), "{n}");
        let req = NameRequest {
            pool: Some("Human".into()),
            background: Some("Noble".into()),
            surname: true,
            ..Default::default()
        };
        let names = generate_many(&req, &[], 30, &mut rng);
        assert_eq!(names.len(), 30);
        assert!(names.iter().all(|n| !n.trim().is_empty()));
        let req = NameRequest {
            invented: true,
            surname: false,
            ..Default::default()
        };
        let n = generate(&req, &[], &mut rng);
        assert!(!n.contains(' '), "{n}");
        assert!(n.len() >= 2);
    }
}
