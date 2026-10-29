//! Campaign data model. Everything serialises to plain JSON so the campaign file can be
//! read on any platform (and by other tools). All structs use `#[serde(default)]` so that
//! files written by older versions still load after new fields are added.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::bestiary::StatBlock;
use crate::calendar::{GameCalendar, GameDate, SessionSchedule};
use crate::conditions::ConditionDef;
use crate::dice::RollRecord;
use crate::encounter::RunningEncounter;
use crate::items::{Coins, InventoryEntry, Item};
use crate::names::NamePool;
use crate::places::Location;

pub type Id = Uuid;

pub const SCHEMA_VERSION: u32 = 1;

pub fn new_id() -> Id {
    Uuid::new_v4()
}

fn now() -> DateTime<Utc> {
    Utc::now()
}

// ---------------------------------------------------------------------------
// Campaign root
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Campaign {
    pub schema_version: u32,
    pub id: Id,
    pub name: String,
    pub description: String,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub settings: CampaignSettings,

    pub story_cards: Vec<StoryCard>,
    pub threads: Vec<PlotThread>,
    pub sessions: Vec<Session>,
    pub maps: Vec<CampaignMap>,

    pub players: Vec<PlayerCharacter>,
    pub npcs: Vec<Npc>,
    pub factions: Vec<Faction>,
    /// Bestiary: stat blocks independent of NPCs (SRD imports, your own, AI-converted).
    pub statblocks: Vec<StatBlock>,
    /// Item library (your own, imported, or copied from the SRD list).
    pub items: Vec<Item>,
    /// Places: towns, shops, inns, dungeons…
    pub locations: Vec<Location>,

    pub encounters: Vec<Encounter>,
    /// Combat in progress (persisted so closing the app mid-fight is safe).
    pub active_encounter: Option<RunningEncounter>,

    pub dice: DiceState,
    pub custom_conditions: Vec<ConditionDef>,
}

impl Default for Campaign {
    fn default() -> Self {
        Campaign {
            schema_version: SCHEMA_VERSION,
            id: new_id(),
            name: "New Campaign".into(),
            description: String::new(),
            created: now(),
            modified: now(),
            settings: CampaignSettings::default(),
            story_cards: Vec::new(),
            threads: Vec::new(),
            sessions: Vec::new(),
            maps: Vec::new(),
            players: Vec::new(),
            npcs: Vec::new(),
            factions: Vec::new(),
            statblocks: Vec::new(),
            items: Vec::new(),
            locations: Vec::new(),
            encounters: Vec::new(),
            active_encounter: None,
            dice: DiceState::default(),
            custom_conditions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CampaignSettings {
    pub game_calendar: GameCalendar,
    pub session_schedule: SessionSchedule,
    /// Path to an Obsidian vault folder (absolute, or relative to the campaign folder).
    pub wiki_path: Option<String>,
    /// Sub-folder of the vault used when exporting cards/NPCs/etc.
    pub wiki_export_folder: String,
    pub name_pools: Vec<NamePool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DiceState {
    pub history: Vec<RollRecord>,
    pub macros: BTreeMap<String, String>,
    /// Previously typed inputs (for Up/Down recall).
    pub input_history: Vec<String>,
}

pub const MAX_ROLL_HISTORY: usize = 500;

impl DiceState {
    pub fn push(&mut self, rec: RollRecord) {
        self.history.push(rec);
        if self.history.len() > MAX_ROLL_HISTORY {
            let excess = self.history.len() - MAX_ROLL_HISTORY;
            self.history.drain(..excess);
        }
    }
    pub fn remember_input(&mut self, input: &str) {
        let input = input.trim();
        if input.is_empty() {
            return;
        }
        if self.input_history.last().map(|s| s.as_str()) != Some(input) {
            self.input_history.push(input.to_string());
        }
        if self.input_history.len() > 200 {
            self.input_history.remove(0);
        }
    }
}

// ---------------------------------------------------------------------------
// Story / plot
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum CardStatus {
    #[default]
    Idea,
    Planned,
    InPlay,
    Done,
    Discarded,
}

impl CardStatus {
    pub const ALL: [CardStatus; 5] = [
        CardStatus::Idea,
        CardStatus::Planned,
        CardStatus::InPlay,
        CardStatus::Done,
        CardStatus::Discarded,
    ];
    pub fn label(self) -> &'static str {
        match self {
            CardStatus::Idea => "Idea",
            CardStatus::Planned => "Planned",
            CardStatus::InPlay => "In play",
            CardStatus::Done => "Done",
            CardStatus::Discarded => "Discarded",
        }
    }
}

/// How a card is placed in time. The DM picks per card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CardTiming {
    #[default]
    Unscheduled,
    RealDate(NaiveDate),
    GameDate(GameDate),
    Session(u32),
}

impl CardTiming {
    pub fn kind_label(&self) -> &'static str {
        match self {
            CardTiming::Unscheduled => "Unscheduled",
            CardTiming::RealDate(_) => "Real-world date",
            CardTiming::GameDate(_) => "Game-world date",
            CardTiming::Session(_) => "Session number",
        }
    }
}

/// Something a story card (or other entity) points at.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Link {
    Card(Id),
    Npc(Id),
    Player(Id),
    Faction(Id),
    Encounter(Id),
    Session(Id),
    Thread(Id),
    Map(Id),
    StatBlock(Id),
    Item(Id),
    Location(Id),
    Url {
        url: String,
        label: String,
    },
    /// Path of a note relative to the Obsidian vault root.
    Wiki(String),
    /// Path relative to the campaign folder (normally `assets/...`).
    Image(String),
}

impl Link {
    pub fn kind_label(&self) -> &'static str {
        match self {
            Link::Card(_) => "Card",
            Link::Npc(_) => "NPC",
            Link::Player(_) => "PC",
            Link::Faction(_) => "Faction",
            Link::Encounter(_) => "Encounter",
            Link::Session(_) => "Session",
            Link::Thread(_) => "Thread",
            Link::Map(_) => "Map",
            Link::StatBlock(_) => "Stat block",
            Link::Item(_) => "Item",
            Link::Location(_) => "Place",
            Link::Url { .. } => "URL",
            Link::Wiki(_) => "Wiki",
            Link::Image(_) => "Image",
        }
    }
    pub fn target_id(&self) -> Option<Id> {
        match self {
            Link::Card(i)
            | Link::Npc(i)
            | Link::Player(i)
            | Link::Faction(i)
            | Link::Encounter(i)
            | Link::Session(i)
            | Link::Thread(i)
            | Link::Map(i)
            | Link::StatBlock(i)
            | Link::Item(i)
            | Link::Location(i) => Some(*i),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MapPin {
    pub map_id: Id,
    /// Normalised position, 0..1 across the image.
    pub x: f32,
    pub y: f32,
}

impl Default for MapPin {
    fn default() -> Self {
        MapPin {
            map_id: Uuid::nil(),
            x: 0.5,
            y: 0.5,
        }
    }
}

/// A plot idea. Every field is optional — an empty card is valid.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StoryCard {
    pub id: Id,
    pub title: String,
    /// Free-form markdown. A DM can ignore every other field and just write here.
    pub body: String,
    pub status: CardStatus,
    /// RGB accent colour.
    pub color: Option<[u8; 3]>,
    pub in_ideas_pile: bool,
    pub threads: Vec<Id>,
    pub timing: CardTiming,
    pub map_pins: Vec<MapPin>,
    pub links: Vec<Link>,
    pub tags: Vec<String>,
    /// Information the players shouldn't know yet.
    pub secret: bool,
    /// When a secret was revealed to the players.
    pub revealed: Option<Reveal>,
    /// Set when the card is a quest: who gave it, objectives, rewards, progress.
    pub quest: Option<Quest>,
    /// Position on the story board (None = not on the board).
    pub board: Option<[f32; 2]>,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum QuestStatus {
    /// Known to exist / on offer, not taken yet.
    #[default]
    Available,
    Active,
    Completed,
    Failed,
    Abandoned,
}

impl QuestStatus {
    pub const ALL: [QuestStatus; 5] = [
        QuestStatus::Available,
        QuestStatus::Active,
        QuestStatus::Completed,
        QuestStatus::Failed,
        QuestStatus::Abandoned,
    ];
    pub fn label(self) -> &'static str {
        match self {
            QuestStatus::Available => "Available",
            QuestStatus::Active => "Active",
            QuestStatus::Completed => "Completed",
            QuestStatus::Failed => "Failed",
            QuestStatus::Abandoned => "Abandoned",
        }
    }
    pub fn open(self) -> bool {
        matches!(self, QuestStatus::Available | QuestStatus::Active)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Objective {
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Quest {
    pub status: QuestStatus,
    /// Who gave it: an NPC, a faction, a place (a notice board)…
    pub giver: Option<Link>,
    pub objectives: Vec<Objective>,
    /// Promised payment.
    pub reward_coins: Coins,
    pub reward_items: Vec<Item>,
    /// Anything else: favours, titles, land, XP…
    pub reward_text: String,
    pub xp: Option<u32>,
    /// Session numbers when it was taken and when it ended.
    pub started_session: Option<u32>,
    pub ended_session: Option<u32>,
    /// Rewards have been handed out.
    pub rewarded: bool,
}

impl Quest {
    pub fn progress(&self) -> (usize, usize) {
        (self.objectives.iter().filter(|o| o.done).count(), self.objectives.len())
    }
    pub fn reward_summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.reward_coins.is_zero() {
            parts.push(self.reward_coins.label());
        }
        for i in &self.reward_items {
            parts.push(i.name.clone());
        }
        if let Some(x) = self.xp {
            parts.push(format!("{x} XP"));
        }
        if !self.reward_text.trim().is_empty() {
            parts.push(self.reward_text.trim().to_string());
        }
        parts.join(", ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Reveal {
    pub session: Option<u32>,
    pub date: Option<NaiveDate>,
    pub note: String,
}

impl Default for StoryCard {
    fn default() -> Self {
        StoryCard {
            id: new_id(),
            title: String::new(),
            body: String::new(),
            status: CardStatus::Idea,
            color: None,
            in_ideas_pile: true,
            threads: Vec::new(),
            timing: CardTiming::Unscheduled,
            map_pins: Vec::new(),
            links: Vec::new(),
            tags: Vec::new(),
            secret: false,
            revealed: None,
            quest: None,
            board: None,
            created: now(),
            modified: now(),
        }
    }
}

impl StoryCard {
    pub fn display_title(&self) -> String {
        display_or_untitled(&self.title, &self.body)
    }

    /// A secret the players haven't learned yet.
    pub fn is_unrevealed_secret(&self) -> bool {
        self.secret && self.revealed.is_none()
    }

    pub fn reveal(&mut self, session: Option<u32>) {
        self.revealed = Some(Reveal {
            session,
            date: Some(chrono::Local::now().date_naive()),
            note: String::new(),
        });
        self.modified = now();
    }
}

/// A title for display: the title, else the first line of the body, else "(untitled)".
pub fn display_or_untitled(title: &str, body: &str) -> String {
    let t = title.trim();
    if !t.is_empty() {
        return t.to_string();
    }
    let first = body.lines().map(|l| l.trim().trim_start_matches('#').trim()).find(|l| !l.is_empty());
    match first {
        Some(l) => {
            let mut s: String = l.chars().take(60).collect();
            if l.chars().count() > 60 {
                s.push('…');
            }
            s
        }
        None => "(untitled)".into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ThreadStatus {
    #[default]
    Open,
    Dormant,
    Resolved,
    Abandoned,
}

impl ThreadStatus {
    pub const ALL: [ThreadStatus; 4] = [ThreadStatus::Open, ThreadStatus::Dormant, ThreadStatus::Resolved, ThreadStatus::Abandoned];
    pub fn label(self) -> &'static str {
        match self {
            ThreadStatus::Open => "Open",
            ThreadStatus::Dormant => "Dormant",
            ThreadStatus::Resolved => "Resolved",
            ThreadStatus::Abandoned => "Abandoned",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlotThread {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub status: ThreadStatus,
    pub color: Option<[u8; 3]>,
    pub links: Vec<Link>,
}

impl Default for PlotThread {
    fn default() -> Self {
        PlotThread {
            id: new_id(),
            name: "New thread".into(),
            description: String::new(),
            status: ThreadStatus::Open,
            color: None,
            links: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SessionStatus {
    #[default]
    Planned,
    Played,
    Cancelled,
}

impl SessionStatus {
    pub const ALL: [SessionStatus; 3] = [SessionStatus::Planned, SessionStatus::Played, SessionStatus::Cancelled];
    pub fn label(self) -> &'static str {
        match self {
            SessionStatus::Planned => "Planned",
            SessionStatus::Played => "Played",
            SessionStatus::Cancelled => "Cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub id: Id,
    pub number: u32,
    pub title: String,
    pub status: SessionStatus,
    pub real_date: Option<NaiveDate>,
    pub game_date: Option<GameDate>,
    /// The DM's plan / narrative for the session (markdown).
    pub narrative: String,
    /// Story cards planned for the session, in running order.
    pub card_ids: Vec<Id>,
    pub encounter_ids: Vec<Id>,
    /// What actually happened (markdown).
    pub recap: String,
    /// Scratch notes taken during play on the Run session screen.
    pub live_notes: String,
    pub links: Vec<Link>,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            id: new_id(),
            number: 1,
            title: String::new(),
            status: SessionStatus::Planned,
            real_date: None,
            game_date: None,
            narrative: String::new(),
            card_ids: Vec::new(),
            encounter_ids: Vec::new(),
            recap: String::new(),
            live_notes: String::new(),
            links: Vec::new(),
        }
    }
}

impl Session {
    pub fn display_title(&self) -> String {
        if self.title.trim().is_empty() {
            format!("Session {}", self.number)
        } else {
            format!("Session {}: {}", self.number, self.title.trim())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CampaignMap {
    pub id: Id,
    pub name: String,
    /// Path relative to the campaign folder.
    pub image: String,
    pub notes: String,
}

impl Default for CampaignMap {
    fn default() -> Self {
        CampaignMap {
            id: new_id(),
            name: "New map".into(),
            image: String::new(),
            notes: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Characters
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomRoll {
    pub id: Id,
    pub label: String,
    pub notation: String,
}

impl Default for CustomRoll {
    fn default() -> Self {
        CustomRoll {
            id: new_id(),
            label: String::new(),
            notation: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AbilityScores {
    pub str_: Option<i32>,
    pub dex: Option<i32>,
    pub con: Option<i32>,
    pub int: Option<i32>,
    pub wis: Option<i32>,
    pub cha: Option<i32>,
}

impl AbilityScores {
    pub const NAMES: [&'static str; 6] = ["STR", "DEX", "CON", "INT", "WIS", "CHA"];

    pub fn get_mut(&mut self, i: usize) -> &mut Option<i32> {
        match i {
            0 => &mut self.str_,
            1 => &mut self.dex,
            2 => &mut self.con,
            3 => &mut self.int,
            4 => &mut self.wis,
            _ => &mut self.cha,
        }
    }
    pub fn get(&self, i: usize) -> Option<i32> {
        match i {
            0 => self.str_,
            1 => self.dex,
            2 => self.con,
            3 => self.int,
            4 => self.wis,
            _ => self.cha,
        }
    }
}

/// Ability modifier per the 5e rules: floor((score - 10) / 2).
pub fn ability_mod(score: i32) -> i32 {
    (score - 10).div_euclid(2)
}

pub fn fmt_mod(m: i32) -> String {
    if m >= 0 { format!("+{m}") } else { m.to_string() }
}

/// Fields shared by player characters and NPCs/monsters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterCore {
    pub name: String,
    /// Portrait / token (path relative to campaign folder).
    pub image: Option<String>,
    /// Character sheet or stat block images.
    pub sheet_images: Vec<String>,
    pub species: String,
    /// Class for PCs, creature type for NPCs ("Humanoid (goblinoid)").
    pub class_or_type: String,
    pub level: Option<u32>,
    /// Challenge rating as typed ("1/4", "5").
    pub cr: String,
    pub ac: Option<i32>,
    pub max_hp: Option<i32>,
    /// Hit dice formula, e.g. "2d6+2" — used to roll HP for monsters.
    pub hp_formula: String,
    pub speed: String,
    /// Assigned initiative modifier. When unset, DEX mod is used if known.
    pub initiative_mod: Option<i32>,
    pub abilities: AbilityScores,
    pub passive_perception: Option<i32>,
    pub custom_rolls: Vec<CustomRoll>,
    /// Extra labelled fields the DM adds (e.g. "Alignment" = "CN").
    pub extra_fields: Vec<(String, String)>,
    /// Free-form markdown notes.
    pub notes: String,
    pub tags: Vec<String>,
    pub links: Vec<Link>,
    /// What they carry.
    pub inventory: Vec<InventoryEntry>,
    pub coins: Coins,
}

/// Where an initiative modifier came from — shown to the DM when auto-rolling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitSource {
    Assigned(i32),
    Dex(i32),
    None,
}

impl InitSource {
    pub fn value(self) -> i32 {
        match self {
            InitSource::Assigned(v) | InitSource::Dex(v) => v,
            InitSource::None => 0,
        }
    }
    pub fn describe(self) -> String {
        match self {
            InitSource::Assigned(v) => format!("assigned init {}", fmt_mod(v)),
            InitSource::Dex(v) => format!("DEX mod {} (no init assigned)", fmt_mod(v)),
            InitSource::None => "no modifier (+0)".into(),
        }
    }
}

impl CharacterCore {
    pub fn initiative_source(&self) -> InitSource {
        if let Some(v) = self.initiative_mod {
            InitSource::Assigned(v)
        } else if let Some(dex) = self.abilities.dex {
            InitSource::Dex(ability_mod(dex))
        } else {
            InitSource::None
        }
    }

    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() {
            "(unnamed)".into()
        } else {
            self.name.trim().to_string()
        }
    }

    /// Add a custom roll, or update the notation if a roll with the same label exists.
    pub fn upsert_custom_roll(&mut self, label: &str, notation: &str) -> Id {
        let label = label.trim();
        if !label.is_empty() {
            if let Some(r) = self.custom_rolls.iter_mut().find(|r| r.label.eq_ignore_ascii_case(label)) {
                r.notation = notation.trim().to_string();
                return r.id;
            }
        }
        let roll = CustomRoll {
            id: new_id(),
            label: if label.is_empty() { notation.trim().to_string() } else { label.to_string() },
            notation: notation.trim().to_string(),
        };
        let id = roll.id;
        self.custom_rolls.push(roll);
        id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerCharacter {
    pub id: Id,
    pub player_name: String,
    pub core: CharacterCore,
    /// Inactive PCs (retired, dead, absent) are hidden from quick pickers.
    pub active: bool,
}

impl Default for PlayerCharacter {
    fn default() -> Self {
        PlayerCharacter {
            id: new_id(),
            player_name: String::new(),
            core: CharacterCore::default(),
            active: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum NpcKind {
    #[default]
    Character,
    Monster,
}

impl NpcKind {
    pub fn label(self) -> &'static str {
        match self {
            NpcKind::Character => "Character",
            NpcKind::Monster => "Monster",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LifeStatus {
    #[default]
    Alive,
    Dead,
    Missing,
    Unknown,
}

impl LifeStatus {
    pub const ALL: [LifeStatus; 4] = [LifeStatus::Alive, LifeStatus::Dead, LifeStatus::Missing, LifeStatus::Unknown];
    pub fn label(self) -> &'static str {
        match self {
            LifeStatus::Alive => "Alive",
            LifeStatus::Dead => "Dead",
            LifeStatus::Missing => "Missing",
            LifeStatus::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Npc {
    pub id: Id,
    pub kind: NpcKind,
    pub core: CharacterCore,
    pub factions: Vec<Id>,
    pub location: String,
    pub role: String,
    pub status: LifeStatus,
    /// Game statistics from the bestiary. The NPC's own fields override it; blank fields
    /// fall back to the stat block.
    pub statblock: Option<Id>,
}

impl Default for Npc {
    fn default() -> Self {
        Npc {
            id: new_id(),
            kind: NpcKind::Character,
            core: CharacterCore::default(),
            factions: Vec::new(),
            location: String::new(),
            role: String::new(),
            status: LifeStatus::Alive,
            statblock: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Attitude {
    Allied,
    Friendly,
    #[default]
    Neutral,
    Unfriendly,
    Hostile,
    Unknown,
}

impl Attitude {
    pub const ALL: [Attitude; 6] = [
        Attitude::Allied,
        Attitude::Friendly,
        Attitude::Neutral,
        Attitude::Unfriendly,
        Attitude::Hostile,
        Attitude::Unknown,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Attitude::Allied => "Allied",
            Attitude::Friendly => "Friendly",
            Attitude::Neutral => "Neutral",
            Attitude::Unfriendly => "Unfriendly",
            Attitude::Hostile => "Hostile",
            Attitude::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Faction {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub goals: String,
    pub attitude: Attitude,
    pub color: Option<[u8; 3]>,
    pub image: Option<String>,
    pub notes: String,
    pub links: Vec<Link>,
}

impl Default for Faction {
    fn default() -> Self {
        Faction {
            id: new_id(),
            name: "New faction".into(),
            description: String::new(),
            goals: String::new(),
            attitude: Attitude::Neutral,
            color: None,
            image: None,
            notes: String::new(),
            links: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Encounters (premade)
// ---------------------------------------------------------------------------

/// Who a combatant is. PCs and NPCs reference the campaign records so changes stick.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CombatantSource {
    Player(Id),
    Npc(Id),
    /// A creature straight from the bestiary (e.g. "Goblin Warrior" ×4).
    StatBlock(Id),
    /// One-off creature that exists only in this encounter.
    AdHoc,
}

impl CombatantSource {
    /// The campaign record behind this combatant, if any.
    pub fn link(&self) -> Option<Link> {
        match self {
            CombatantSource::Player(id) => Some(Link::Player(*id)),
            CombatantSource::Npc(id) => Some(Link::Npc(*id)),
            CombatantSource::StatBlock(id) => Some(Link::StatBlock(*id)),
            CombatantSource::AdHoc => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EncounterEntry {
    pub id: Id,
    pub source: CombatantSource,
    pub count: u32,
    /// For AdHoc entries.
    pub name: String,
    pub max_hp: Option<i32>,
    pub initiative_mod: Option<i32>,
    pub ac: Option<i32>,
    /// Fights on the party's side (not counted towards difficulty).
    pub ally: bool,
    /// When count > 1, all copies share one initiative roll.
    pub group_initiative: bool,
}

impl Default for EncounterEntry {
    fn default() -> Self {
        EncounterEntry {
            id: new_id(),
            source: CombatantSource::AdHoc,
            count: 1,
            name: String::new(),
            max_hp: None,
            initiative_mod: None,
            ac: None,
            ally: false,
            group_initiative: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Encounter {
    pub id: Id,
    pub name: String,
    pub location: String,
    pub description: String,
    pub entries: Vec<EncounterEntry>,
    pub notes: String,
    pub links: Vec<Link>,
}

impl Default for Encounter {
    fn default() -> Self {
        Encounter {
            id: new_id(),
            name: "New encounter".into(),
            location: String::new(),
            description: String::new(),
            entries: Vec::new(),
            notes: String::new(),
            links: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Lookups and mutations on the campaign
// ---------------------------------------------------------------------------

/// A reference to any named entity, for pickers and search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityRef {
    pub link: Link,
    pub name: String,
}

impl Campaign {
    pub fn new(name: &str) -> Self {
        Campaign {
            name: name.to_string(),
            ..Default::default()
        }
    }

    pub fn touch(&mut self) {
        self.modified = Utc::now();
    }

    pub fn card(&self, id: Id) -> Option<&StoryCard> {
        self.story_cards.iter().find(|c| c.id == id)
    }
    pub fn card_mut(&mut self, id: Id) -> Option<&mut StoryCard> {
        self.story_cards.iter_mut().find(|c| c.id == id)
    }
    pub fn npc(&self, id: Id) -> Option<&Npc> {
        self.npcs.iter().find(|c| c.id == id)
    }
    pub fn npc_mut(&mut self, id: Id) -> Option<&mut Npc> {
        self.npcs.iter_mut().find(|c| c.id == id)
    }
    pub fn player(&self, id: Id) -> Option<&PlayerCharacter> {
        self.players.iter().find(|c| c.id == id)
    }
    pub fn player_mut(&mut self, id: Id) -> Option<&mut PlayerCharacter> {
        self.players.iter_mut().find(|c| c.id == id)
    }
    pub fn faction(&self, id: Id) -> Option<&Faction> {
        self.factions.iter().find(|c| c.id == id)
    }
    pub fn encounter(&self, id: Id) -> Option<&Encounter> {
        self.encounters.iter().find(|c| c.id == id)
    }
    pub fn session(&self, id: Id) -> Option<&Session> {
        self.sessions.iter().find(|c| c.id == id)
    }
    pub fn session_mut(&mut self, id: Id) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|c| c.id == id)
    }
    pub fn thread(&self, id: Id) -> Option<&PlotThread> {
        self.threads.iter().find(|c| c.id == id)
    }
    pub fn map(&self, id: Id) -> Option<&CampaignMap> {
        self.maps.iter().find(|c| c.id == id)
    }
    pub fn statblock(&self, id: Id) -> Option<&StatBlock> {
        self.statblocks.iter().find(|c| c.id == id)
    }
    pub fn item(&self, id: Id) -> Option<&Item> {
        self.items.iter().find(|c| c.id == id)
    }
    pub fn item_mut(&mut self, id: Id) -> Option<&mut Item> {
        self.items.iter_mut().find(|c| c.id == id)
    }
    pub fn location(&self, id: Id) -> Option<&Location> {
        self.locations.iter().find(|c| c.id == id)
    }
    pub fn location_mut(&mut self, id: Id) -> Option<&mut Location> {
        self.locations.iter_mut().find(|c| c.id == id)
    }

    /// The character (PC or NPC) behind a link.
    pub fn character_core(&self, link: &Link) -> Option<&CharacterCore> {
        match link {
            Link::Player(id) => self.player(*id).map(|p| &p.core),
            Link::Npc(id) => self.npc(*id).map(|n| &n.core),
            _ => None,
        }
    }
    pub fn character_core_mut(&mut self, link: &Link) -> Option<&mut CharacterCore> {
        match link {
            Link::Player(id) => self.player_mut(*id).map(|p| &mut p.core),
            Link::Npc(id) => self.npc_mut(*id).map(|n| &mut n.core),
            _ => None,
        }
    }

    /// Put an item in a character's inventory (stacking), optionally adding its rolls to
    /// their custom rolls. Returns false if there's no such character.
    pub fn give_item(&mut self, to: &Link, item: &Item, qty: u32, with_rolls: bool) -> bool {
        let Some(core) = self.character_core_mut(to) else { return false };
        crate::items::add_to_inventory(&mut core.inventory, item, qty);
        if with_rolls {
            add_item_rolls(core, item);
        }
        true
    }

    pub fn give_coins(&mut self, to: &Link, coins: &Coins) -> bool {
        let Some(core) = self.character_core_mut(to) else { return false };
        core.coins.add(coins);
        true
    }

    /// Item library collections with counts ("My items" first).
    pub fn item_collections(&self) -> Vec<(String, usize)> {
        let mut m: BTreeMap<String, usize> = BTreeMap::new();
        for s in &self.items {
            *m.entry(s.collection_label()).or_default() += 1;
        }
        let mut v: Vec<(String, usize)> = m.into_iter().collect();
        v.sort_by_key(|(name, _)| (name != crate::items::MY_COLLECTION, name.to_lowercase()));
        v
    }

    pub fn delete_items(&mut self, ids: &[Id]) -> usize {
        let mut n = 0;
        for id in ids {
            if self.item(*id).is_some() {
                self.delete(&Link::Item(*id));
                n += 1;
            }
        }
        n
    }

    /// Story cards that are quests.
    pub fn quests(&self) -> Vec<&StoryCard> {
        self.story_cards.iter().filter(|c| c.quest.is_some()).collect()
    }

    /// Places inside a place (direct children), by name.
    pub fn child_locations(&self, id: Id) -> Vec<&Location> {
        let mut v: Vec<&Location> = self.locations.iter().filter(|l| l.parent == Some(id)).collect();
        v.sort_by_key(|l| l.display_name().to_lowercase());
        v
    }

    /// "Thornwood › Millbrook › The Rusty Anchor"
    pub fn location_path(&self, id: Id) -> String {
        let mut parts = Vec::new();
        let mut cur = Some(id);
        let mut guard = 0;
        while let (Some(c), true) = (cur, guard < 20) {
            let Some(l) = self.location(c) else { break };
            parts.push(l.display_name());
            cur = l.parent;
            guard += 1;
        }
        parts.reverse();
        parts.join(" › ")
    }

    /// Would making `parent` the parent of `child` create a loop?
    pub fn location_would_loop(&self, child: Id, parent: Id) -> bool {
        let mut cur = Some(parent);
        let mut guard = 0;
        while let Some(c) = cur {
            if c == child || guard > 50 {
                return true;
            }
            cur = self.location(c).and_then(|l| l.parent);
            guard += 1;
        }
        false
    }

    /// Bestiary collections with how many stat blocks each holds ("My stat blocks" first).
    pub fn statblock_collections(&self) -> Vec<(String, usize)> {
        let mut m: BTreeMap<String, usize> = BTreeMap::new();
        for s in &self.statblocks {
            *m.entry(s.collection_label()).or_default() += 1;
        }
        let mut v: Vec<(String, usize)> = m.into_iter().collect();
        v.sort_by_key(|(name, _)| (name != crate::bestiary::MY_COLLECTION, name.to_lowercase()));
        v
    }

    /// Delete several stat blocks (and their uses in NPCs and encounters). Returns how many.
    pub fn delete_statblocks(&mut self, ids: &[Id]) -> usize {
        let mut n = 0;
        for id in ids {
            if self.statblock(*id).is_some() {
                self.delete(&Link::StatBlock(*id));
                n += 1;
            }
        }
        n
    }

    pub fn statblock_mut(&mut self, id: Id) -> Option<&mut StatBlock> {
        self.statblocks.iter_mut().find(|c| c.id == id)
    }

    /// The stat block behind a combatant: itself, or the one an NPC points at.
    pub fn statblock_for(&self, src: &CombatantSource) -> Option<&StatBlock> {
        match src {
            CombatantSource::StatBlock(id) => self.statblock(*id),
            CombatantSource::Npc(id) => self.npc(*id).and_then(|n| n.statblock).and_then(|s| self.statblock(s)),
            _ => None,
        }
    }

    /// Initiative modifier for a source, falling back from the NPC to its stat block.
    pub fn initiative_source_for(&self, src: &CombatantSource) -> Option<InitSource> {
        let core = self.core_for(src)?;
        let own = core.initiative_source();
        if own == InitSource::None {
            if let (CombatantSource::Npc(_), Some(sb)) = (src, self.statblock_for(src)) {
                return Some(sb.core.initiative_source());
            }
        }
        Some(own)
    }

    /// Challenge rating, falling back from the NPC to its stat block.
    pub fn cr_for(&self, src: &CombatantSource) -> Option<String> {
        let own = self.core_for(src).map(|c| c.cr.trim().to_string()).filter(|c| !c.is_empty());
        own.or_else(|| self.statblock_for(src).map(|s| s.core.cr.trim().to_string()).filter(|c| !c.is_empty()))
    }

    /// Character data for a combatant source (PC or NPC).
    pub fn core_for(&self, src: &CombatantSource) -> Option<&CharacterCore> {
        match src {
            CombatantSource::Player(id) => self.player(*id).map(|p| &p.core),
            CombatantSource::Npc(id) => self.npc(*id).map(|n| &n.core),
            CombatantSource::StatBlock(id) => self.statblock(*id).map(|s| &s.core),
            CombatantSource::AdHoc => None,
        }
    }
    pub fn core_for_mut(&mut self, src: &CombatantSource) -> Option<&mut CharacterCore> {
        match src {
            CombatantSource::Player(id) => self.player_mut(*id).map(|p| &mut p.core),
            CombatantSource::Npc(id) => self.npc_mut(*id).map(|n| &mut n.core),
            CombatantSource::StatBlock(id) => self.statblock_mut(*id).map(|s| &mut s.core),
            CombatantSource::AdHoc => None,
        }
    }

    /// Display name for a link target, or None if it's been deleted.
    pub fn link_name(&self, link: &Link) -> Option<String> {
        Some(match link {
            Link::Card(id) => self.card(*id)?.display_title(),
            Link::Npc(id) => self.npc(*id)?.core.display_name(),
            Link::Player(id) => self.player(*id)?.core.display_name(),
            Link::Faction(id) => self.faction(*id)?.name.clone(),
            Link::Encounter(id) => self.encounter(*id)?.name.clone(),
            Link::Session(id) => self.session(*id)?.display_title(),
            Link::Thread(id) => self.thread(*id)?.name.clone(),
            Link::Map(id) => self.map(*id)?.name.clone(),
            Link::StatBlock(id) => self.statblock(*id)?.name(),
            Link::Item(id) => self.item(*id)?.name.clone(),
            Link::Location(id) => self.location(*id)?.display_name(),
            Link::Url { url, label } => {
                if label.trim().is_empty() {
                    url.clone()
                } else {
                    label.clone()
                }
            }
            Link::Wiki(p) => p.rsplit('/').next().unwrap_or(p).trim_end_matches(".md").to_string(),
            Link::Image(p) => p.rsplit('/').next().unwrap_or(p).to_string(),
        })
    }

    /// Every linkable campaign entity (for pickers / [[wiki-link]] resolution).
    pub fn all_entities(&self) -> Vec<EntityRef> {
        let mut v = Vec::new();
        for c in &self.story_cards {
            v.push(EntityRef {
                link: Link::Card(c.id),
                name: c.display_title(),
            });
        }
        for c in &self.npcs {
            v.push(EntityRef {
                link: Link::Npc(c.id),
                name: c.core.display_name(),
            });
        }
        for c in &self.players {
            v.push(EntityRef {
                link: Link::Player(c.id),
                name: c.core.display_name(),
            });
        }
        for c in &self.factions {
            v.push(EntityRef {
                link: Link::Faction(c.id),
                name: c.name.clone(),
            });
        }
        for c in &self.encounters {
            v.push(EntityRef {
                link: Link::Encounter(c.id),
                name: c.name.clone(),
            });
        }
        for c in &self.sessions {
            v.push(EntityRef {
                link: Link::Session(c.id),
                name: c.display_title(),
            });
        }
        for c in &self.threads {
            v.push(EntityRef {
                link: Link::Thread(c.id),
                name: c.name.clone(),
            });
        }
        for c in &self.maps {
            v.push(EntityRef {
                link: Link::Map(c.id),
                name: c.name.clone(),
            });
        }
        for c in &self.statblocks {
            v.push(EntityRef {
                link: Link::StatBlock(c.id),
                name: c.name(),
            });
        }
        for c in &self.locations {
            v.push(EntityRef {
                link: Link::Location(c.id),
                name: c.display_name(),
            });
        }
        for c in &self.items {
            v.push(EntityRef {
                link: Link::Item(c.id),
                name: c.name.clone(),
            });
        }
        v
    }

    /// Resolve a `[[Name]]`-style reference against campaign entities (case-insensitive).
    pub fn resolve_name(&self, name: &str) -> Option<Link> {
        let n = name.trim();
        self.all_entities().into_iter().find(|e| e.name.eq_ignore_ascii_case(n)).map(|e| e.link)
    }

    /// Find a PC or NPC by name: exact match first, then unique prefix match.
    pub fn find_character_by_name(&self, name: &str) -> Option<CombatantSource> {
        let n = name.trim().to_lowercase();
        if n.is_empty() {
            return None;
        }
        let mut all: Vec<(String, CombatantSource)> = Vec::new();
        for p in &self.players {
            all.push((p.core.name.to_lowercase(), CombatantSource::Player(p.id)));
        }
        for npc in &self.npcs {
            all.push((npc.core.name.to_lowercase(), CombatantSource::Npc(npc.id)));
        }
        if let Some((_, s)) = all.iter().find(|(name, _)| *name == n) {
            return Some(s.clone());
        }
        let prefix: Vec<_> = all.iter().filter(|(name, _)| name.starts_with(&n)).collect();
        if prefix.len() == 1 {
            return Some(prefix[0].1.clone());
        }
        None
    }

    /// Everything that links to `target` (backlinks).
    pub fn backlinks(&self, target: &Link) -> Vec<EntityRef> {
        let mut out = Vec::new();
        for c in &self.story_cards {
            let via_quest = c.quest.as_ref().map(|q| q.giver.as_ref() == Some(target)).unwrap_or(false);
            if c.links.contains(target) || via_quest || matches!(target, Link::Thread(t) if c.threads.contains(t)) {
                out.push(EntityRef {
                    link: Link::Card(c.id),
                    name: c.display_title(),
                });
            }
        }
        for s in &self.sessions {
            let via_cards = matches!(target, Link::Card(id) if s.card_ids.contains(id));
            let via_enc = matches!(target, Link::Encounter(id) if s.encounter_ids.contains(id));
            if s.links.contains(target) || via_cards || via_enc {
                out.push(EntityRef {
                    link: Link::Session(s.id),
                    name: s.display_title(),
                });
            }
        }
        for n in &self.npcs {
            let via_faction = matches!(target, Link::Faction(f) if n.factions.contains(f));
            let via_statblock = matches!(target, Link::StatBlock(s) if n.statblock == Some(*s));
            if n.core.links.contains(target) || via_faction || via_statblock {
                out.push(EntityRef {
                    link: Link::Npc(n.id),
                    name: n.core.display_name(),
                });
            }
        }
        for p in &self.players {
            if p.core.links.contains(target) {
                out.push(EntityRef {
                    link: Link::Player(p.id),
                    name: p.core.display_name(),
                });
            }
        }
        for f in &self.factions {
            if f.links.contains(target) {
                out.push(EntityRef {
                    link: Link::Faction(f.id),
                    name: f.name.clone(),
                });
            }
        }
        for l in &self.locations {
            // NPCs, factions and places inside show their places on their own pages
            // ("Found at", "Present at", "Places here"), so only plain links count here.
            let via = match target {
                Link::Location(o) => l.related.contains(o),
                Link::Card(c) => l.card == Some(*c),
                _ => false,
            };
            if l.links.contains(target) || via {
                out.push(EntityRef {
                    link: Link::Location(l.id),
                    name: l.display_name(),
                });
            }
        }
        for e in &self.encounters {
            let via_entry = e.entries.iter().any(|en| match (&en.source, target) {
                (CombatantSource::Npc(a), Link::Npc(b)) => a == b,
                (CombatantSource::Player(a), Link::Player(b)) => a == b,
                (CombatantSource::StatBlock(a), Link::StatBlock(b)) => a == b,
                _ => false,
            });
            if e.links.contains(target) || via_entry {
                out.push(EntityRef {
                    link: Link::Encounter(e.id),
                    name: e.name.clone(),
                });
            }
        }
        out
    }

    /// Delete an entity and scrub every reference to it.
    pub fn delete(&mut self, target: &Link) {
        match target {
            Link::Card(id) => self.story_cards.retain(|c| c.id != *id),
            Link::Npc(id) => {
                self.npcs.retain(|c| c.id != *id);
                for e in &mut self.encounters {
                    e.entries.retain(|en| en.source != CombatantSource::Npc(*id));
                }
            }
            Link::Player(id) => {
                self.players.retain(|c| c.id != *id);
                for e in &mut self.encounters {
                    e.entries.retain(|en| en.source != CombatantSource::Player(*id));
                }
            }
            Link::Faction(id) => {
                self.factions.retain(|c| c.id != *id);
                for n in &mut self.npcs {
                    n.factions.retain(|f| f != id);
                }
            }
            Link::Encounter(id) => {
                self.encounters.retain(|c| c.id != *id);
                for s in &mut self.sessions {
                    s.encounter_ids.retain(|e| e != id);
                }
            }
            Link::Session(id) => self.sessions.retain(|c| c.id != *id),
            Link::Thread(id) => {
                self.threads.retain(|c| c.id != *id);
                for c in &mut self.story_cards {
                    c.threads.retain(|t| t != id);
                }
            }
            Link::Map(id) => {
                self.maps.retain(|c| c.id != *id);
                for c in &mut self.story_cards {
                    c.map_pins.retain(|p| p.map_id != *id);
                }
            }
            Link::StatBlock(id) => {
                self.statblocks.retain(|c| c.id != *id);
                for n in &mut self.npcs {
                    if n.statblock == Some(*id) {
                        n.statblock = None;
                    }
                }
                for e in &mut self.encounters {
                    e.entries.retain(|en| en.source != CombatantSource::StatBlock(*id));
                }
            }
            Link::Item(id) => self.items.retain(|c| c.id != *id),
            Link::Location(id) => {
                // places inside it move up a level
                let grand = self.location(*id).and_then(|l| l.parent);
                self.locations.retain(|c| c.id != *id);
                for l in &mut self.locations {
                    if l.parent == Some(*id) {
                        l.parent = grand;
                    }
                    l.related.retain(|r| r != id);
                }
            }
            Link::Url { .. } | Link::Wiki(_) | Link::Image(_) => {}
        }
        match target {
            Link::Npc(id) => {
                for l in &mut self.locations {
                    l.npcs.retain(|n| n != id);
                    if let Some(s) = &mut l.shop {
                        if s.keeper == Some(*id) {
                            s.keeper = None;
                        }
                    }
                }
            }
            Link::Faction(id) => {
                for l in &mut self.locations {
                    l.factions.retain(|f| f != id);
                }
            }
            Link::Card(id) => {
                for l in &mut self.locations {
                    if l.card == Some(*id) {
                        l.card = None;
                    }
                }
            }
            _ => {}
        }
        for c in &mut self.story_cards {
            if let Some(q) = &mut c.quest {
                if q.giver.as_ref() == Some(target) {
                    q.giver = None;
                }
            }
        }
        if let Link::Card(id) = target {
            for s in &mut self.sessions {
                s.card_ids.retain(|c| c != id);
            }
        }
        // scrub generic links everywhere
        for c in &mut self.story_cards {
            c.links.retain(|l| l != target);
        }
        for s in &mut self.sessions {
            s.links.retain(|l| l != target);
        }
        for t in &mut self.threads {
            t.links.retain(|l| l != target);
        }
        for n in &mut self.npcs {
            n.core.links.retain(|l| l != target);
        }
        for p in &mut self.players {
            p.core.links.retain(|l| l != target);
        }
        for f in &mut self.factions {
            f.links.retain(|l| l != target);
        }
        for e in &mut self.encounters {
            e.links.retain(|l| l != target);
        }
        for l in &mut self.locations {
            l.links.retain(|x| x != target);
        }
        self.touch();
    }

    /// Cards that belong in the ideas pile.
    pub fn ideas_pile(&self) -> Vec<&StoryCard> {
        self.story_cards.iter().filter(|c| c.in_ideas_pile).collect()
    }

    /// Cards assigned to a session (via the planner) in running order, then cards
    /// whose timing points at that session number but aren't in the planner.
    pub fn cards_for_session(&self, session: &Session) -> Vec<&StoryCard> {
        let mut out: Vec<&StoryCard> = session.card_ids.iter().filter_map(|id| self.card(*id)).collect();
        for c in &self.story_cards {
            if c.timing == CardTiming::Session(session.number) && !session.card_ids.contains(&c.id) {
                out.push(c);
            }
        }
        out
    }

    /// Sessions a card has been assigned to.
    pub fn sessions_for_card(&self, card: Id) -> Vec<&Session> {
        self.sessions.iter().filter(|s| s.card_ids.contains(&card)).collect()
    }

    pub fn next_session_number(&self) -> u32 {
        self.sessions.iter().map(|s| s.number).max().map(|n| n + 1).unwrap_or(1)
    }

    /// Assign a card to a session planner (idempotent). Removes it from the ideas pile.
    pub fn assign_card_to_session(&mut self, card: Id, session: Id) {
        if let Some(s) = self.session_mut(session) {
            if !s.card_ids.contains(&card) {
                s.card_ids.push(card);
            }
        }
        if let Some(c) = self.card_mut(card) {
            c.in_ideas_pile = false;
            if c.status == CardStatus::Idea {
                c.status = CardStatus::Planned;
            }
        }
    }

    /// Move a card to session `number` (None = unscheduled): sets its timing and takes it
    /// out of other sessions' plans, adding it to that session's plan if the session exists.
    pub fn move_card_to_session(&mut self, card: Id, number: Option<u32>) {
        let target = number.and_then(|n| self.sessions.iter().find(|s| s.number == n).map(|s| s.id));
        for s in &mut self.sessions {
            if Some(s.id) != target {
                s.card_ids.retain(|c| *c != card);
            }
        }
        if let Some(c) = self.card_mut(card) {
            c.timing = match number {
                Some(n) => CardTiming::Session(n),
                None if matches!(c.timing, CardTiming::Session(_)) => CardTiming::Unscheduled,
                None => c.timing.clone(),
            };
        }
        if let Some(t) = target {
            self.assign_card_to_session(card, t);
        }
    }

    pub fn unassign_card_from_session(&mut self, card: Id, session: Id) {
        if let Some(s) = self.session_mut(session) {
            s.card_ids.retain(|c| *c != card);
        }
    }

    /// NPCs that are members of a faction.
    pub fn faction_members(&self, faction: Id) -> Vec<&Npc> {
        self.npcs.iter().filter(|n| n.factions.contains(&faction)).collect()
    }

    /// All conditions available (standard 2024 rules + campaign custom ones).
    pub fn all_conditions(&self) -> Vec<ConditionDef> {
        let mut v = crate::conditions::standard_conditions();
        v.extend(self.custom_conditions.iter().cloned());
        v
    }

    pub fn condition(&self, name: &str) -> Option<ConditionDef> {
        self.all_conditions().into_iter().find(|c| c.name.eq_ignore_ascii_case(name.trim()))
    }

    /// Find the vault-relative path for a wiki link (resolving `[[Note]]` names is done by the vault).
    pub fn wiki_root(&self, campaign_dir: &std::path::Path) -> Option<std::path::PathBuf> {
        let p = self.settings.wiki_path.as_ref()?;
        if p.trim().is_empty() {
            return None;
        }
        let path = std::path::PathBuf::from(p);
        Some(if path.is_absolute() { path } else { campaign_dir.join(path) })
    }
}

// ---------------------------------------------------------------------------
// Running a session at the table
// ---------------------------------------------------------------------------

/// What `end_session` did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EndSessionReport {
    pub recap: String,
    pub moved_cards: usize,
    pub moved_to: Option<u32>,
}

impl Campaign {
    /// The session to run today: the lowest-numbered planned session, else the latest one.
    pub fn upcoming_session(&self) -> Option<&Session> {
        self.sessions
            .iter()
            .filter(|s| s.status == SessionStatus::Planned)
            .min_by_key(|s| s.number)
            .or_else(|| self.sessions.iter().max_by_key(|s| s.number))
    }

    /// NPCs, PCs and factions involved in a session: linked from the session, its cards
    /// and its encounters. In first-seen order, without duplicates.
    pub fn session_people(&self, s: &Session) -> Vec<Link> {
        let mut out: Vec<Link> = Vec::new();
        let mut push = |l: Link| {
            if matches!(l, Link::Npc(_) | Link::Player(_) | Link::Faction(_)) && !out.contains(&l) {
                out.push(l);
            }
        };
        for l in &s.links {
            push(l.clone());
        }
        for c in self.cards_for_session(s) {
            for l in &c.links {
                push(l.clone());
            }
        }
        for eid in &s.encounter_ids {
            if let Some(e) = self.encounter(*eid) {
                for en in &e.entries {
                    if let CombatantSource::Npc(n) = en.source {
                        push(Link::Npc(n));
                    }
                }
            }
        }
        out
    }

    /// Secret cards that belong to a session: secret cards planned for it, and secret cards
    /// linked from its cards (clues the scene could reveal).
    pub fn session_secrets(&self, s: &Session) -> Vec<Id> {
        let mut out = Vec::new();
        for c in self.cards_for_session(s) {
            if c.secret && !out.contains(&c.id) {
                out.push(c.id);
            }
            for l in &c.links {
                if let Link::Card(id) = l {
                    if self.card(*id).map(|x| x.secret).unwrap_or(false) && !out.contains(id) {
                        out.push(*id);
                    }
                }
            }
        }
        out
    }

    /// Mark a session played, write a recap draft from the live notes, the scenes that were
    /// played and the secrets revealed, and optionally carry unplayed cards to the next
    /// session (creating it if needed).
    pub fn end_session(&mut self, id: Id, carry_over: bool) -> Option<EndSessionReport> {
        let s = self.session(id)?.clone();
        let cards: Vec<StoryCard> = self.cards_for_session(&s).into_iter().cloned().collect();
        let played: Vec<&StoryCard> = cards.iter().filter(|c| c.status == CardStatus::Done).collect();
        let unplayed: Vec<Id> = cards
            .iter()
            .filter(|c| !matches!(c.status, CardStatus::Done | CardStatus::Discarded) && !c.secret)
            .map(|c| c.id)
            .collect();
        let revealed: Vec<String> = self
            .story_cards
            .iter()
            .filter(|c| c.revealed.as_ref().map(|r| r.session == Some(s.number)).unwrap_or(false))
            .map(|c| c.display_title())
            .collect();
        let mut draft = String::new();
        if !s.live_notes.trim().is_empty() {
            draft.push_str("**Notes from the table**\n\n");
            draft.push_str(s.live_notes.trim());
            draft.push_str("\n\n");
        }
        if !played.is_empty() {
            draft.push_str("**Scenes played**\n\n");
            for c in &played {
                draft.push_str(&format!("- [[{}]]\n", c.display_title()));
            }
            draft.push('\n');
        }
        if !revealed.is_empty() {
            draft.push_str("**Secrets revealed**\n\n");
            for t in &revealed {
                draft.push_str(&format!("- [[{t}]]\n"));
            }
            draft.push('\n');
        }
        let mut report = EndSessionReport::default();
        if carry_over && !unplayed.is_empty() {
            let next_num = self
                .sessions
                .iter()
                .filter(|x| x.number > s.number && x.status == SessionStatus::Planned)
                .map(|x| x.number)
                .min();
            let next_id = match next_num.and_then(|n| self.sessions.iter().find(|x| x.number == n)) {
                Some(n) => n.id,
                None => {
                    let n = Session {
                        number: self.next_session_number(),
                        ..Default::default()
                    };
                    let nid = n.id;
                    self.sessions.push(n);
                    nid
                }
            };
            let next_number = self.session(next_id).map(|x| x.number);
            for cid in &unplayed {
                if let Some(sess) = self.session_mut(id) {
                    sess.card_ids.retain(|c| c != cid);
                }
                if let Some(card) = self.card_mut(*cid) {
                    if card.timing == CardTiming::Session(s.number) {
                        card.timing = CardTiming::Session(next_number.unwrap_or(s.number + 1));
                    }
                }
                self.assign_card_to_session(*cid, next_id);
            }
            report.moved_cards = unplayed.len();
            report.moved_to = next_number;
            draft.push_str(&format!(
                "_{} carried over to session {}._\n",
                if unplayed.len() == 1 {
                    "1 unplayed card".to_string()
                } else {
                    format!("{} unplayed cards", unplayed.len())
                },
                next_number.unwrap_or(0)
            ));
        }
        let sess = self.session_mut(id)?;
        sess.status = SessionStatus::Played;
        if sess.real_date.is_none() {
            sess.real_date = Some(chrono::Local::now().date_naive());
        }
        let draft = draft.trim().to_string();
        if !draft.is_empty() {
            sess.recap = if sess.recap.trim().is_empty() {
                draft
            } else {
                format!("{}\n\n{}", sess.recap.trim_end(), draft)
            };
        }
        report.recap = sess.recap.clone();
        Some(report)
    }
}

/// Add an item's rolls to a character's custom rolls (by label, so it's safe to repeat).
/// Returns how many rolls were added or updated.
pub fn add_item_rolls(core: &mut CharacterCore, item: &Item) -> usize {
    for r in &item.rolls {
        core.upsert_custom_roll(&r.label, &r.notation);
    }
    item.rolls.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untitled_cards() {
        let mut c = StoryCard::default();
        assert_eq!(c.display_title(), "(untitled)");
        c.body = "\n\n## The baron lies\nmore".into();
        assert_eq!(c.display_title(), "The baron lies");
        c.title = "Real title".into();
        assert_eq!(c.display_title(), "Real title");
    }

    #[test]
    fn ability_mods() {
        assert_eq!(ability_mod(10), 0);
        assert_eq!(ability_mod(11), 0);
        assert_eq!(ability_mod(9), -1);
        assert_eq!(ability_mod(8), -1);
        assert_eq!(ability_mod(1), -5);
        assert_eq!(ability_mod(20), 5);
        assert_eq!(fmt_mod(3), "+3");
        assert_eq!(fmt_mod(-1), "-1");
        assert_eq!(fmt_mod(0), "+0");
    }

    #[test]
    fn init_source_priority() {
        let mut core = CharacterCore::default();
        assert_eq!(core.initiative_source(), InitSource::None);
        core.abilities.dex = Some(14);
        assert_eq!(core.initiative_source(), InitSource::Dex(2));
        core.initiative_mod = Some(5);
        assert_eq!(core.initiative_source(), InitSource::Assigned(5));
        assert!(core.initiative_source().describe().contains("assigned"));
    }

    #[test]
    fn custom_roll_upsert() {
        let mut core = CharacterCore::default();
        let a = core.upsert_custom_roll("Bite", "1d6+2");
        let b = core.upsert_custom_roll("bite", "1d8+2");
        assert_eq!(a, b);
        assert_eq!(core.custom_rolls.len(), 1);
        assert_eq!(core.custom_rolls[0].notation, "1d8+2");
        core.upsert_custom_roll("", "2d4");
        assert_eq!(core.custom_rolls.len(), 2);
        assert_eq!(core.custom_rolls[1].label, "2d4");
    }

    #[test]
    fn delete_scrubs_references() {
        let mut c = Campaign::new("Test");
        let npc = Npc::default();
        let npc_id = npc.id;
        c.npcs.push(npc);
        let mut card = StoryCard::default();
        card.links.push(Link::Npc(npc_id));
        let card_id = card.id;
        c.story_cards.push(card);
        let mut enc = Encounter::default();
        enc.entries.push(EncounterEntry {
            source: CombatantSource::Npc(npc_id),
            ..Default::default()
        });
        c.encounters.push(enc);
        let s = Session::default();
        let sid = s.id;
        c.sessions.push(s);
        c.assign_card_to_session(card_id, sid);

        assert_eq!(c.backlinks(&Link::Npc(npc_id)).len(), 2);
        c.delete(&Link::Npc(npc_id));
        assert!(c.card(card_id).unwrap().links.is_empty());
        assert!(c.encounters[0].entries.is_empty());

        c.delete(&Link::Card(card_id));
        assert!(c.sessions[0].card_ids.is_empty());
    }

    #[test]
    fn session_assignment() {
        let mut c = Campaign::new("Test");
        let card = StoryCard::default();
        let cid = card.id;
        c.story_cards.push(card);
        let mut timed = StoryCard::default();
        timed.timing = CardTiming::Session(2);
        c.story_cards.push(timed);
        let s = Session {
            number: 2,
            ..Default::default()
        };
        let sid = s.id;
        c.sessions.push(s);
        assert_eq!(c.ideas_pile().len(), 2);
        c.assign_card_to_session(cid, sid);
        c.assign_card_to_session(cid, sid);
        assert_eq!(c.session(sid).unwrap().card_ids.len(), 1);
        assert_eq!(c.ideas_pile().len(), 1);
        assert_eq!(c.card(cid).unwrap().status, CardStatus::Planned);
        let sess = c.session(sid).unwrap().clone();
        assert_eq!(c.cards_for_session(&sess).len(), 2);
        assert_eq!(c.next_session_number(), 3);
        c.unassign_card_from_session(cid, sid);
        assert!(c.sessions_for_card(cid).is_empty());
    }

    #[test]
    fn find_by_name() {
        let mut c = Campaign::new("Test");
        let mut n = Npc::default();
        n.core.name = "Goblin Boss".into();
        c.npcs.push(n);
        let mut n2 = Npc::default();
        n2.core.name = "Goblin".into();
        c.npcs.push(n2);
        let mut p = PlayerCharacter::default();
        p.core.name = "Aria".into();
        c.players.push(p);
        assert!(matches!(c.find_character_by_name("goblin"), Some(CombatantSource::Npc(_))));
        assert_eq!(c.find_character_by_name("goblin b").map(|s| matches!(s, CombatantSource::Npc(_))), Some(true));
        assert!(matches!(c.find_character_by_name("ari"), Some(CombatantSource::Player(_))));
        assert_eq!(c.find_character_by_name("zz"), None);
        let mut n3 = Npc::default();
        n3.core.name = "Goblin Shaman".into();
        c.npcs.push(n3);
        // "goblin " (ambiguous prefix) -> exact match wins for "goblin"
        assert!(c.find_character_by_name("goblin").is_some());
        assert_eq!(c.find_character_by_name("goblin s").is_some(), true);
    }

    #[test]
    fn json_is_forward_compatible() {
        // A minimal / old file with missing fields still loads.
        let json = r#"{"name":"Old","story_cards":[{"title":"x"}],"npcs":[{"core":{"name":"Bob"}}]}"#;
        let c: Campaign = serde_json::from_str(json).unwrap();
        assert_eq!(c.name, "Old");
        assert_eq!(c.story_cards[0].title, "x");
        assert!(c.story_cards[0].in_ideas_pile);
        assert_eq!(c.npcs[0].core.name, "Bob");
    }
    #[test]
    fn items_places_and_quests() {
        use crate::items::{Coins, Item, ItemKind, Rarity};
        use crate::places::Location;
        let mut c = Campaign::new("x");
        let pc = PlayerCharacter::default();
        let pid = pc.id;
        c.players.push(pc);
        let mut sword = Item::named("Flame Tongue", ItemKind::Weapon, Rarity::Rare, None);
        sword.rolls.push(CustomRoll {
            label: "Flame Tongue extra fire damage".into(),
            notation: "2d6".into(),
            ..Default::default()
        });
        assert!(c.give_item(&Link::Player(pid), &sword, 1, true));
        assert!(c.give_coins(&Link::Player(pid), &Coins::gp(25)));
        assert!(!c.give_item(&Link::Faction(new_id()), &sword, 1, false));
        let core = &c.player(pid).unwrap().core;
        assert_eq!(core.inventory.len(), 1);
        assert_eq!(core.custom_rolls[0].notation, "2d6");
        assert_eq!(core.coins.gp, 25);
        // giving the rolls again doesn't duplicate them
        add_item_rolls(&mut c.player_mut(pid).unwrap().core, &sword);
        assert_eq!(c.player(pid).unwrap().core.custom_rolls.len(), 1);

        // places: nesting, loops, deletion scrubs references
        let town = Location {
            name: "Millbrook".into(),
            kind: "Town".into(),
            ..Default::default()
        };
        let inn = Location {
            name: "The Rusty Anchor".into(),
            kind: "Inn".into(),
            parent: Some(town.id),
            ..Default::default()
        };
        let (tid, iid) = (town.id, inn.id);
        let mut npc = Npc::default();
        npc.core.name = "Greta".into();
        let nid = npc.id;
        c.npcs.push(npc);
        c.locations = vec![town, inn];
        c.location_mut(iid).unwrap().npcs.push(nid);
        assert_eq!(c.location_path(iid), "Millbrook › The Rusty Anchor");
        assert!(c.location_would_loop(tid, iid));
        assert!(!c.location_would_loop(iid, tid));
        assert_eq!(c.child_locations(tid).len(), 1);
        assert!(
            !c.backlinks(&Link::Npc(nid)).iter().any(|e| e.link == Link::Location(iid)),
            "shown as Found at instead"
        );
        assert_eq!(c.resolve_name("the rusty anchor"), Some(Link::Location(iid)));

        // quests: giver shows up in backlinks; deleting the giver clears it
        let mut card = StoryCard {
            title: "Clear the cellar".into(),
            ..Default::default()
        };
        card.quest = Some(Quest {
            giver: Some(Link::Npc(nid)),
            reward_coins: Coins::gp(50),
            xp: Some(100),
            ..Default::default()
        });
        let cid = card.id;
        c.story_cards.push(card);
        assert_eq!(c.quests().len(), 1);
        assert_eq!(c.card(cid).unwrap().quest.as_ref().unwrap().reward_summary(), "50 gp, 100 XP");
        assert!(c.backlinks(&Link::Npc(nid)).iter().any(|e| e.link == Link::Card(cid)));
        c.delete(&Link::Npc(nid));
        assert!(c.location(iid).unwrap().npcs.is_empty());
        assert_eq!(c.card(cid).unwrap().quest.as_ref().unwrap().giver, None);
        c.delete(&Link::Location(tid));
        assert_eq!(c.location(iid).unwrap().parent, None);
        // old files without the new fields still load
        let json = crate::storage::to_json(&c).unwrap();
        let back = crate::storage::from_json(&json).unwrap();
        assert_eq!(back.player(pid).unwrap().core.inventory.len(), 1);
    }

    #[test]
    fn statblock_collections_and_bulk_delete() {
        let mut c = Campaign::new("x");
        let mut mine = StatBlock::default();
        mine.core.name = "Homebrew".into();
        let mut a = StatBlock::default();
        a.collection = "Essential NPCs".into();
        let mut b = StatBlock::default();
        b.collection = "Essential NPCs".into();
        let (aid, bid) = (a.id, b.id);
        c.statblocks = vec![a, mine, b];
        let mut npc = Npc::default();
        npc.statblock = Some(aid);
        c.npcs.push(npc);
        c.encounters.push(Encounter {
            entries: vec![EncounterEntry {
                source: CombatantSource::StatBlock(bid),
                ..Default::default()
            }],
            ..Default::default()
        });
        assert_eq!(
            c.statblock_collections(),
            vec![("My stat blocks".to_string(), 1), ("Essential NPCs".to_string(), 2)]
        );
        assert_eq!(c.delete_statblocks(&[aid, bid, new_id()]), 2);
        assert_eq!(c.statblocks.len(), 1);
        assert_eq!(c.npcs[0].statblock, None);
        assert!(c.encounters[0].entries.is_empty());
    }

    #[test]
    fn run_session_helpers() {
        let mut c = crate::sample_campaign();
        let s1 = c.sessions[0].clone();
        assert_eq!(c.upcoming_session().unwrap().number, 1);
        let people = c.session_people(&s1);
        // bandit + captain from the encounter, the faction from the card
        assert!(people.contains(&Link::Faction(c.factions[0].id)));
        assert!(people.iter().filter(|l| matches!(l, Link::Npc(_))).count() >= 2);
        // a secret clue linked from the session's card
        let mut secret = StoryCard {
            title: "The letter".into(),
            secret: true,
            ..Default::default()
        };
        let secret_id = secret.id;
        secret.in_ideas_pile = false;
        c.story_cards.push(secret);
        c.story_cards[0].links.push(Link::Card(secret_id));
        assert!(c.session_secrets(&s1).contains(&secret_id));
        assert_eq!(c.session_secrets(&s1).len(), 2);
        c.card_mut(secret_id).unwrap().reveal(Some(1));
        assert!(!c.card(secret_id).unwrap().is_unrevealed_secret());
        // one card played, one extra unplayed card carried over to session 2
        let extra = StoryCard {
            title: "Tavern brawl".into(),
            status: CardStatus::Planned,
            ..Default::default()
        };
        let extra_id = extra.id;
        c.story_cards.push(extra);
        let sid = c.sessions[0].id;
        c.assign_card_to_session(extra_id, sid);
        let first = c.sessions[0].card_ids[0];
        c.card_mut(first).unwrap().status = CardStatus::Done;
        c.session_mut(sid).unwrap().live_notes = "Brielle pocketed the letter.".into();
        let r = c.end_session(sid, true).unwrap();
        assert_eq!(r.moved_cards, 1);
        assert_eq!(r.moved_to, Some(2));
        assert!(r.recap.contains("Brielle pocketed the letter."));
        assert!(r.recap.contains("[[Bandits on the road]]"));
        assert!(r.recap.contains("[[The letter]]"));
        let s = c.session(sid).unwrap();
        assert_eq!(s.status, SessionStatus::Played);
        assert!(s.real_date.is_some());
        assert!(!s.card_ids.contains(&extra_id));
        assert!(c.sessions[1].card_ids.contains(&extra_id));
        assert_eq!(c.upcoming_session().unwrap().number, 2);
        // ending the last session creates the next one for carried-over cards
        let s2 = c.sessions[1].id;
        let r = c.end_session(s2, true).unwrap();
        assert_eq!(r.moved_to, Some(3));
        assert_eq!(c.sessions.len(), 3);
    }

    #[test]
    fn moving_cards_between_sessions() {
        let mut c = Campaign::new("x");
        let s1 = Session {
            number: 1,
            ..Default::default()
        };
        let s2 = Session {
            number: 2,
            ..Default::default()
        };
        let (i1, i2) = (s1.id, s2.id);
        c.sessions = vec![s1, s2];
        let card = StoryCard::default();
        let cid = card.id;
        c.story_cards.push(card);
        c.assign_card_to_session(cid, i1);
        c.move_card_to_session(cid, Some(2));
        assert!(!c.session(i1).unwrap().card_ids.contains(&cid));
        assert!(c.session(i2).unwrap().card_ids.contains(&cid));
        assert_eq!(c.card(cid).unwrap().timing, CardTiming::Session(2));
        c.move_card_to_session(cid, None);
        assert!(c.sessions.iter().all(|s| !s.card_ids.contains(&cid)));
        assert_eq!(c.card(cid).unwrap().timing, CardTiming::Unscheduled);
        assert_eq!(c.cards_for_session(c.session(i2).unwrap()).len(), 0);
    }
}
