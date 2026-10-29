//! Moving content between campaigns: export some or all of a campaign to a package file,
//! import some or all of a package (or another campaign folder) into a campaign, and start a
//! new campaign from parts of an existing one ("same world, new story").
//!
//! A package (`.cmpack`) is a zip file holding `campaign.json` (the chosen parts, in the
//! normal campaign format) and the `assets/` images those parts use. Everything keeps its
//! id, so importing the same thing twice can be recognised (skip, replace or keep both), and
//! links between imported things stay intact. Links to things that weren't brought along are
//! removed after an import.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::model::{Campaign, CombatantSource, Id, Link, new_id};
use crate::storage::{self, ASSETS_DIR, CAMPAIGN_FILE};

pub const PACKAGE_EXT: &str = "cmpack";

/// The parts a campaign is made of, as far as moving content around is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Part {
    /// Story cards (and quests) and plot threads.
    Story,
    Sessions,
    Encounters,
    Players,
    Npcs,
    Factions,
    Places,
    Items,
    Bestiary,
    Maps,
    /// Calendar, name pools, custom conditions, dice macros, wiki settings.
    Rules,
}

impl Part {
    pub const ALL: [Part; 11] = [
        Part::Story,
        Part::Sessions,
        Part::Encounters,
        Part::Players,
        Part::Npcs,
        Part::Factions,
        Part::Places,
        Part::Items,
        Part::Bestiary,
        Part::Maps,
        Part::Rules,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Part::Story => "Story cards, quests & plot threads",
            Part::Sessions => "Sessions & play schedule",
            Part::Encounters => "Encounters",
            Part::Players => "Player characters",
            Part::Npcs => "NPCs & monsters",
            Part::Factions => "Factions",
            Part::Places => "Places & shops",
            Part::Items => "Items",
            Part::Bestiary => "Bestiary (stat blocks)",
            Part::Maps => "Maps",
            Part::Rules => "Calendar, name pools, conditions & dice macros",
        }
    }

    /// How many things of this part a campaign has (for the checkboxes).
    pub fn count(self, c: &Campaign) -> usize {
        match self {
            Part::Story => c.story_cards.len() + c.threads.len(),
            Part::Sessions => c.sessions.len(),
            Part::Encounters => c.encounters.len(),
            Part::Players => c.players.len(),
            Part::Npcs => c.npcs.len(),
            Part::Factions => c.factions.len(),
            Part::Places => c.locations.len(),
            Part::Items => c.items.len(),
            Part::Bestiary => c.statblocks.len(),
            Part::Maps => c.maps.len(),
            Part::Rules => {
                c.settings.name_pools.len() + c.custom_conditions.len() + c.dice.macros.len() + usize::from(c.settings.game_calendar != Default::default())
            }
        }
    }
}

/// Ready-made selections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Everything,
    /// The world without the story: people, places, factions, items, bestiary, maps, rules.
    Setting,
    /// Things you'd reuse in any campaign: items, bestiary, rules.
    Library,
    /// The player characters (with their inventories).
    Party,
}

impl Preset {
    pub const ALL: [Preset; 4] = [Preset::Everything, Preset::Setting, Preset::Library, Preset::Party];
    pub fn label(self) -> &'static str {
        match self {
            Preset::Everything => "Everything",
            Preset::Setting => "Campaign setting",
            Preset::Library => "Reference library",
            Preset::Party => "The party",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Preset::Everything => "The whole campaign.",
            Preset::Setting => {
                "The world without the story: NPCs, factions, places and shops, items, bestiary, maps, calendar and name pools. No story cards, quests, sessions, encounters or player characters."
            }
            Preset::Library => "Items, the bestiary, calendar, name pools, custom conditions and dice macros.",
            Preset::Party => "The player characters, with their inventories and custom rolls.",
        }
    }
    pub fn parts(self) -> Vec<Part> {
        match self {
            Preset::Everything => Part::ALL.to_vec(),
            Preset::Setting => vec![Part::Npcs, Part::Factions, Part::Places, Part::Items, Part::Bestiary, Part::Maps, Part::Rules],
            Preset::Library => vec![Part::Items, Part::Bestiary, Part::Rules],
            Preset::Party => vec![Part::Players],
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    #[error("{0}")]
    Storage(#[from] storage::StorageError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("couldn't read the package: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Other(String),
}

// ---------------------------------------------------------------------------
// Choosing parts
// ---------------------------------------------------------------------------

/// A copy of the campaign holding only the chosen parts (ids unchanged). The combat in
/// progress and the dice log are never carried over.
pub fn extract(c: &Campaign, parts: &[Part]) -> Campaign {
    let has = |p: Part| parts.contains(&p);
    let mut out = Campaign::new(&c.name);
    out.id = c.id;
    out.description = c.description.clone();
    out.created = c.created;
    if has(Part::Story) {
        out.story_cards = c.story_cards.clone();
        out.threads = c.threads.clone();
    }
    if has(Part::Sessions) {
        out.sessions = c.sessions.clone();
        out.settings.session_schedule = c.settings.session_schedule.clone();
    }
    if has(Part::Encounters) {
        out.encounters = c.encounters.clone();
    }
    if has(Part::Players) {
        out.players = c.players.clone();
    }
    if has(Part::Npcs) {
        out.npcs = c.npcs.clone();
    }
    if has(Part::Factions) {
        out.factions = c.factions.clone();
    }
    if has(Part::Places) {
        out.locations = c.locations.clone();
    }
    if has(Part::Items) {
        out.items = c.items.clone();
    }
    if has(Part::Bestiary) {
        out.statblocks = c.statblocks.clone();
    }
    if has(Part::Maps) {
        out.maps = c.maps.clone();
    }
    if has(Part::Rules) {
        out.settings.game_calendar = c.settings.game_calendar.clone();
        out.settings.name_pools = c.settings.name_pools.clone();
        out.settings.wiki_export_folder = c.settings.wiki_export_folder.clone();
        // a vault inside the old campaign folder doesn't come along; a shared one does
        out.settings.wiki_path = c.settings.wiki_path.clone().filter(|p| Path::new(p).is_absolute());
        out.custom_conditions = c.custom_conditions.clone();
        out.dice.macros = c.dice.macros.clone();
    }
    out
}

/// The parts a campaign actually has something in.
pub fn parts_present(c: &Campaign) -> Vec<Part> {
    Part::ALL.into_iter().filter(|p| p.count(c) > 0).collect()
}

// ---------------------------------------------------------------------------
// Keeping references valid
// ---------------------------------------------------------------------------

/// Remove references to things that aren't in the campaign (after taking only some parts).
/// Returns how many references were removed.
pub fn prune_dangling(c: &mut Campaign) -> usize {
    let cards: HashSet<Id> = c.story_cards.iter().map(|x| x.id).collect();
    let threads: HashSet<Id> = c.threads.iter().map(|x| x.id).collect();
    let npcs: HashSet<Id> = c.npcs.iter().map(|x| x.id).collect();
    let players: HashSet<Id> = c.players.iter().map(|x| x.id).collect();
    let factions: HashSet<Id> = c.factions.iter().map(|x| x.id).collect();
    let encounters: HashSet<Id> = c.encounters.iter().map(|x| x.id).collect();
    let sessions: HashSet<Id> = c.sessions.iter().map(|x| x.id).collect();
    let maps: HashSet<Id> = c.maps.iter().map(|x| x.id).collect();
    let statblocks: HashSet<Id> = c.statblocks.iter().map(|x| x.id).collect();
    let items: HashSet<Id> = c.items.iter().map(|x| x.id).collect();
    let places: HashSet<Id> = c.locations.iter().map(|x| x.id).collect();
    let link_ok = |l: &Link| match l {
        Link::Card(i) => cards.contains(i),
        Link::Npc(i) => npcs.contains(i),
        Link::Player(i) => players.contains(i),
        Link::Faction(i) => factions.contains(i),
        Link::Encounter(i) => encounters.contains(i),
        Link::Session(i) => sessions.contains(i),
        Link::Thread(i) => threads.contains(i),
        Link::Map(i) => maps.contains(i),
        Link::StatBlock(i) => statblocks.contains(i),
        Link::Item(i) => items.contains(i),
        Link::Location(i) => places.contains(i),
        Link::Url { .. } | Link::Wiki(_) | Link::Image(_) => true,
    };
    let mut n = 0;
    let mut keep_links = |links: &mut Vec<Link>| {
        let before = links.len();
        links.retain(|l| link_ok(l));
        n += before - links.len();
    };
    for x in &mut c.story_cards {
        keep_links(&mut x.links);
    }
    for x in &mut c.sessions {
        keep_links(&mut x.links);
    }
    for x in &mut c.threads {
        keep_links(&mut x.links);
    }
    for x in &mut c.npcs {
        keep_links(&mut x.core.links);
    }
    for x in &mut c.players {
        keep_links(&mut x.core.links);
    }
    for x in &mut c.statblocks {
        keep_links(&mut x.core.links);
    }
    for x in &mut c.factions {
        keep_links(&mut x.links);
    }
    for x in &mut c.encounters {
        keep_links(&mut x.links);
    }
    for x in &mut c.locations {
        keep_links(&mut x.links);
    }
    let mut count = |removed: usize| n += removed;
    for x in &mut c.story_cards {
        let b = x.threads.len() + x.map_pins.len();
        x.threads.retain(|t| threads.contains(t));
        x.map_pins.retain(|p| maps.contains(&p.map_id));
        count(b - x.threads.len() - x.map_pins.len());
        if let Some(q) = &mut x.quest {
            if q.giver.as_ref().map(|g| !link_ok(g)).unwrap_or(false) {
                q.giver = None;
                count(1);
            }
        }
    }
    for s in &mut c.sessions {
        let b = s.card_ids.len() + s.encounter_ids.len();
        s.card_ids.retain(|i| cards.contains(i));
        s.encounter_ids.retain(|i| encounters.contains(i));
        count(b - s.card_ids.len() - s.encounter_ids.len());
    }
    for x in &mut c.npcs {
        let b = x.factions.len();
        x.factions.retain(|f| factions.contains(f));
        count(b - x.factions.len());
        if x.statblock.map(|s| !statblocks.contains(&s)).unwrap_or(false) {
            x.statblock = None;
            count(1);
        }
    }
    for e in &mut c.encounters {
        let b = e.entries.len();
        e.entries.retain(|en| match en.source {
            CombatantSource::Player(i) => players.contains(&i),
            CombatantSource::Npc(i) => npcs.contains(&i),
            CombatantSource::StatBlock(i) => statblocks.contains(&i),
            CombatantSource::AdHoc => true,
        });
        count(b - e.entries.len());
    }
    for l in &mut c.locations {
        if l.parent.map(|p| !places.contains(&p)).unwrap_or(false) {
            l.parent = None;
            count(1);
        }
        if l.card.map(|p| !cards.contains(&p)).unwrap_or(false) {
            l.card = None;
            count(1);
        }
        let b = l.npcs.len() + l.factions.len() + l.related.len();
        l.npcs.retain(|i| npcs.contains(i));
        l.factions.retain(|i| factions.contains(i));
        l.related.retain(|i| places.contains(i));
        count(b - l.npcs.len() - l.factions.len() - l.related.len());
        if let Some(s) = &mut l.shop {
            if s.keeper.map(|k| !npcs.contains(&k)).unwrap_or(false) {
                s.keeper = None;
                count(1);
            }
        }
    }
    n
}

// ---------------------------------------------------------------------------
// Merging into a campaign
// ---------------------------------------------------------------------------

/// What to do with things the campaign already has (same id — usually because both came
/// from the same campaign).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Duplicates {
    /// Keep what's here.
    #[default]
    Skip,
    /// Overwrite with the imported version.
    Replace,
    /// Import as separate copies (new ids).
    KeepBoth,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MergeReport {
    pub added: usize,
    pub replaced: usize,
    pub skipped: usize,
    /// References to things that weren't imported, removed.
    pub links_removed: usize,
}

impl MergeReport {
    pub fn summary(&self) -> String {
        let mut parts = vec![format!("{} added", self.added)];
        if self.replaced > 0 {
            parts.push(format!("{} replaced", self.replaced));
        }
        if self.skipped > 0 {
            parts.push(format!("{} already here, skipped", self.skipped));
        }
        if self.links_removed > 0 {
            parts.push(format!("{} links to things left out removed", self.links_removed));
        }
        parts.join(", ")
    }
}

/// Ids of everything in the chosen parts of `c` (to spot duplicates).
fn ids_in(c: &Campaign, parts: &[Part]) -> Vec<Id> {
    let mut v = Vec::new();
    for p in parts {
        match p {
            Part::Story => {
                v.extend(c.story_cards.iter().map(|x| x.id));
                v.extend(c.threads.iter().map(|x| x.id));
            }
            Part::Sessions => v.extend(c.sessions.iter().map(|x| x.id)),
            Part::Encounters => v.extend(c.encounters.iter().map(|x| x.id)),
            Part::Players => v.extend(c.players.iter().map(|x| x.id)),
            Part::Npcs => v.extend(c.npcs.iter().map(|x| x.id)),
            Part::Factions => v.extend(c.factions.iter().map(|x| x.id)),
            Part::Places => v.extend(c.locations.iter().map(|x| x.id)),
            Part::Items => v.extend(c.items.iter().map(|x| x.id)),
            Part::Bestiary => v.extend(c.statblocks.iter().map(|x| x.id)),
            Part::Maps => v.extend(c.maps.iter().map(|x| x.id)),
            Part::Rules => {}
        }
    }
    v
}

/// How many of the chosen things in `src` are already in `target`.
pub fn count_duplicates(target: &Campaign, src: &Campaign, parts: &[Part]) -> usize {
    let have: HashSet<Id> = ids_in(target, &Part::ALL).into_iter().collect();
    ids_in(src, parts).into_iter().filter(|i| have.contains(i)).count()
}

/// Give new ids to everything in `src` that `target` already has, updating every reference
/// inside `src` to match (ids are UUIDs, so a plain text replace in the JSON is exact).
fn reid_duplicates(target: &Campaign, src: &Campaign, parts: &[Part]) -> Result<Campaign, TransferError> {
    let have: HashSet<Id> = ids_in(target, &Part::ALL).into_iter().collect();
    let dups: Vec<Id> = ids_in(src, parts).into_iter().filter(|i| have.contains(i)).collect();
    let mut json = storage::to_json(src)?;
    for old in dups {
        json = json.replace(&old.to_string(), &new_id().to_string());
    }
    Ok(storage::from_json(&json)?)
}

/// Add the chosen parts of `src` to `target`, then drop references that don't resolve.
pub fn merge(target: &mut Campaign, src: &Campaign, parts: &[Part], dup: Duplicates) -> Result<MergeReport, TransferError> {
    let src = if dup == Duplicates::KeepBoth {
        reid_duplicates(target, src, parts)?
    } else {
        src.clone()
    };
    let mut r = MergeReport::default();
    macro_rules! put {
        ($field:ident) => {
            for x in &src.$field {
                match target.$field.iter().position(|y| y.id == x.id) {
                    Some(i) if dup == Duplicates::Replace => {
                        target.$field[i] = x.clone();
                        r.replaced += 1;
                    }
                    Some(_) => r.skipped += 1,
                    None => {
                        target.$field.push(x.clone());
                        r.added += 1;
                    }
                }
            }
        };
    }
    for p in parts {
        match p {
            Part::Story => {
                put!(story_cards);
                put!(threads);
            }
            Part::Sessions => {
                let had_sessions = !target.sessions.is_empty();
                put!(sessions);
                // the play schedule comes along when this campaign had no sessions of its own
                if !had_sessions || dup == Duplicates::Replace {
                    target.settings.session_schedule = src.settings.session_schedule.clone();
                }
            }
            Part::Encounters => put!(encounters),
            Part::Players => put!(players),
            Part::Npcs => put!(npcs),
            Part::Factions => put!(factions),
            Part::Places => put!(locations),
            Part::Items => put!(items),
            Part::Bestiary => put!(statblocks),
            Part::Maps => put!(maps),
            Part::Rules => {
                for p in &src.settings.name_pools {
                    match target.settings.name_pools.iter().position(|x| x.name.eq_ignore_ascii_case(&p.name)) {
                        Some(i) if dup == Duplicates::Replace => target.settings.name_pools[i] = p.clone(),
                        Some(_) => {}
                        None => target.settings.name_pools.push(p.clone()),
                    }
                }
                for cd in &src.custom_conditions {
                    match target.custom_conditions.iter().position(|x| x.name.eq_ignore_ascii_case(&cd.name)) {
                        Some(i) if dup == Duplicates::Replace => target.custom_conditions[i] = cd.clone(),
                        Some(_) => {}
                        None => target.custom_conditions.push(cd.clone()),
                    }
                }
                for (k, v) in &src.dice.macros {
                    if dup == Duplicates::Replace || !target.dice.macros.contains_key(k) {
                        target.dice.macros.insert(k.clone(), v.clone());
                    }
                }
                // the calendar comes along when this campaign still has the default one
                if dup == Duplicates::Replace || target.settings.game_calendar == Default::default() {
                    target.settings.game_calendar = src.settings.game_calendar.clone();
                }
                if target.settings.wiki_path.is_none() {
                    target.settings.wiki_path = src.settings.wiki_path.clone();
                }
            }
        }
    }
    r.links_removed = prune_dangling(target);
    target.touch();
    Ok(r)
}

// ---------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------

/// Every `assets/…` path the campaign refers to (portraits, maps, sheets, link images…).
pub fn asset_paths(c: &Campaign) -> BTreeSet<String> {
    fn walk(v: &serde_json::Value, out: &mut BTreeSet<String>) {
        match v {
            serde_json::Value::String(s) => {
                if let Some(rest) = s.strip_prefix("assets/") {
                    if !rest.is_empty() && !rest.contains("..") {
                        out.insert(s.clone());
                    }
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            serde_json::Value::Object(o) => o.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = BTreeSet::new();
    if let Ok(v) = serde_json::to_value(c) {
        walk(&v, &mut out);
    }
    out
}

// ---------------------------------------------------------------------------
// Package files and other sources
// ---------------------------------------------------------------------------

/// Where an import's images come from.
#[derive(Debug, Clone)]
pub enum AssetSource {
    /// Another campaign's folder.
    Dir(PathBuf),
    /// A package (zip) file.
    Zip(PathBuf),
    None,
}

/// Something to import from: a package file, another campaign's folder, or a campaign.json.
#[derive(Debug, Clone)]
pub struct Source {
    pub campaign: Campaign,
    pub assets: AssetSource,
    /// What to call it ("Hollowmere (export).cmpack").
    pub label: String,
}

impl Source {
    /// Open a package, a campaign folder or a campaign.json.
    pub fn open(path: &Path) -> Result<Source, TransferError> {
        let label = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        if storage::is_campaign_dir(path) {
            let dir = storage::campaign_dir(path);
            let campaign = storage::load(&dir)?;
            return Ok(Source {
                campaign,
                assets: AssetSource::Dir(dir),
                label,
            });
        }
        let mut f = fs::File::open(path)?;
        let mut magic = [0u8; 2];
        let n = f.read(&mut magic)?;
        if n == 2 && &magic == b"PK" {
            let mut zip = zip::ZipArchive::new(fs::File::open(path)?)?;
            let mut json = String::new();
            zip.by_name(CAMPAIGN_FILE)
                .map_err(|_| TransferError::Other(format!("{label} has no {CAMPAIGN_FILE} inside — is it a Campaign Manager package?")))?
                .read_to_string(&mut json)?;
            let campaign = storage::from_json(&json)?;
            return Ok(Source {
                campaign,
                assets: AssetSource::Zip(path.to_path_buf()),
                label,
            });
        }
        // a bare campaign.json somewhere
        let text = fs::read_to_string(path)?;
        let campaign = storage::from_json(&text)?;
        let assets = match path.parent() {
            Some(p) if p.join(ASSETS_DIR).is_dir() => AssetSource::Dir(p.to_path_buf()),
            _ => AssetSource::None,
        };
        Ok(Source { campaign, assets, label })
    }

    /// Copy the given asset paths into `dest_dir` (a campaign folder). Files already there
    /// are left alone (asset names carry a random suffix, so the same name is the same image).
    /// Returns how many were copied; missing ones are skipped.
    pub fn copy_assets(&self, wanted: &BTreeSet<String>, dest_dir: &Path) -> Result<usize, TransferError> {
        let mut n = 0;
        let todo: Vec<&String> = wanted.iter().filter(|rel| !storage::resolve_asset(dest_dir, rel).exists()).collect();
        if todo.is_empty() {
            return Ok(0);
        }
        fs::create_dir_all(dest_dir.join(ASSETS_DIR))?;
        match &self.assets {
            AssetSource::Dir(src) => {
                for rel in todo {
                    let from = storage::resolve_asset(src, rel);
                    if from.is_file() {
                        let to = storage::resolve_asset(dest_dir, rel);
                        if let Some(p) = to.parent() {
                            fs::create_dir_all(p)?;
                        }
                        fs::copy(&from, &to)?;
                        n += 1;
                    }
                }
            }
            AssetSource::Zip(file) => {
                let mut zip = zip::ZipArchive::new(fs::File::open(file)?)?;
                for rel in todo {
                    let Ok(mut entry) = zip.by_name(rel) else { continue };
                    let to = storage::resolve_asset(dest_dir, rel);
                    if let Some(p) = to.parent() {
                        fs::create_dir_all(p)?;
                    }
                    let mut out = fs::File::create(&to)?;
                    std::io::copy(&mut entry, &mut out)?;
                    n += 1;
                }
            }
            AssetSource::None => {}
        }
        Ok(n)
    }
}

/// What an export wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportReport {
    pub things: usize,
    pub images: usize,
    pub missing_images: usize,
}

/// Write the chosen parts of the campaign in `dir` to a package file.
pub fn export_package(c: &Campaign, dir: &Path, parts: &[Part], dest: &Path) -> Result<ExportReport, TransferError> {
    let sub = extract(c, parts);
    let json = storage::to_json(&sub)?;
    let tmp = dest.with_extension("cmpack.tmp");
    let file = fs::File::create(&tmp)?;
    let mut zip = zip::ZipWriter::new(file);
    let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    // images are already compressed: store them as they are
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file(CAMPAIGN_FILE, deflate)?;
    zip.write_all(json.as_bytes())?;
    let mut images = 0;
    let mut missing = 0;
    for rel in asset_paths(&sub) {
        let from = storage::resolve_asset(dir, &rel);
        match fs::read(&from) {
            Ok(bytes) => {
                zip.start_file(rel.as_str(), stored)?;
                zip.write_all(&bytes)?;
                images += 1;
            }
            Err(_) => missing += 1,
        }
    }
    zip.finish()?;
    fs::rename(&tmp, dest)?;
    let things = parts.iter().filter(|p| **p != Part::Rules).map(|p| p.count(&sub)).sum();
    Ok(ExportReport {
        things,
        images,
        missing_images: missing,
    })
}

/// Start a new campaign in `parent/name` from the chosen parts of `src`. Returns its folder.
pub fn new_campaign_from(src: &Source, parts: &[Part], name: &str, parent: &Path) -> Result<(PathBuf, MergeReport), TransferError> {
    let name = if name.trim().is_empty() { "New Campaign" } else { name.trim() };
    let mut dir = parent.join(storage::sanitize_file_name(name));
    let mut i = 2;
    while dir.exists() {
        dir = parent.join(format!("{} {i}", storage::sanitize_file_name(name)));
        i += 1;
    }
    let mut c = Campaign::new(name);
    if parts.contains(&Part::Story) {
        c.description = src.campaign.description.clone();
    }
    let report = merge(&mut c, &src.campaign, parts, Duplicates::Skip)?;
    storage::save(&c, &dir)?;
    src.copy_assets(&asset_paths(&c), &dir)?;
    Ok((dir, report))
}

/// Import the chosen parts of `src` into the campaign stored in `dir`, copying the images
/// the imported things use.
pub fn import_into(target: &mut Campaign, dir: &Path, src: &Source, parts: &[Part], dup: Duplicates) -> Result<(MergeReport, usize), TransferError> {
    let report = merge(target, &src.campaign, parts, dup)?;
    let wanted = asset_paths(&extract(target, parts));
    let images = src.copy_assets(&wanted, dir)?;
    Ok((report, images))
}

/// Count what each part of a source has, for display.
pub fn part_counts(c: &Campaign) -> HashMap<Part, usize> {
    Part::ALL.iter().map(|p| (*p, p.count(c))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Link;

    #[test]
    fn setting_preset_leaves_the_story_behind() {
        let c = crate::sample_campaign();
        let sub = extract(&c, &Preset::Setting.parts());
        assert!(sub.story_cards.is_empty() && sub.sessions.is_empty() && sub.encounters.is_empty() && sub.players.is_empty());
        assert_eq!(sub.npcs.len(), c.npcs.len());
        assert_eq!(sub.locations.len(), c.locations.len());
        assert_eq!(sub.settings.game_calendar, c.settings.game_calendar);
        // new campaign: references into the story are dropped, the world stays linked
        let mut fresh = Campaign::new("Next");
        let r = merge(&mut fresh, &c, &Preset::Setting.parts(), Duplicates::Skip).unwrap();
        assert!(r.added > 0);
        assert!(fresh.npcs.iter().all(|n| n.core.links.iter().all(|l| !matches!(l, Link::Card(_)))));
        assert!(fresh.locations.iter().all(|l| l.card.is_none()));
        assert_eq!(fresh.child_locations(fresh.locations[0].id).len(), 2);
        assert_eq!(fresh.locations[2].shop.as_ref().unwrap().keeper, c.locations[2].shop.as_ref().unwrap().keeper);
        assert!(fresh.npcs.iter().any(|n| n.statblock.is_some()));
    }

    #[test]
    fn duplicates_skip_replace_or_copy() {
        let c = crate::sample_campaign();
        let mut t = c.clone();
        t.npcs[0].core.name = "Renamed".into();
        assert_eq!(count_duplicates(&t, &c, &[Part::Npcs]), c.npcs.len());
        let r = merge(&mut t, &c, &[Part::Npcs], Duplicates::Skip).unwrap();
        assert_eq!((r.added, r.skipped), (0, c.npcs.len()));
        assert_eq!(t.npcs[0].core.name, "Renamed");
        let r = merge(&mut t, &c, &[Part::Npcs], Duplicates::Replace).unwrap();
        assert_eq!(r.replaced, c.npcs.len());
        assert_eq!(t.npcs[0].core.name, c.npcs[0].core.name);
        // keep both: copies get new ids, and links between the copies follow
        let before = t.npcs.len();
        merge(&mut t, &c, &[Part::Npcs, Part::Factions], Duplicates::KeepBoth).unwrap();
        assert_eq!(t.npcs.len(), before * 2);
        let copy = t.npcs.iter().rev().find(|n| !n.factions.is_empty()).unwrap();
        assert!(t.faction(copy.factions[0]).is_some());
        assert_ne!(Some(copy.factions[0]), c.factions.first().map(|f| f.id));
    }

    #[test]
    fn packages_round_trip_with_images() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Old");
        let mut c = crate::sample_campaign();
        fs::create_dir_all(dir.join("assets")).unwrap();
        fs::write(dir.join("assets/baron-1a2b3c4d.png"), b"fake png").unwrap();
        c.npcs[0].core.image = Some("assets/baron-1a2b3c4d.png".into());
        c.story_cards[0].links.push(Link::Image("assets/missing-00000000.png".into()));
        storage::save(&c, &dir).unwrap();

        let pack = tmp.path().join("world.cmpack");
        let rep = export_package(&c, &dir, &Preset::Setting.parts(), &pack).unwrap();
        assert_eq!((rep.images, rep.missing_images), (1, 0)); // the card's image isn't in the setting
        let src = Source::open(&pack).unwrap();
        assert!(src.campaign.story_cards.is_empty());
        let (new_dir, _) = new_campaign_from(&src, &Preset::Setting.parts(), "Next Adventure", tmp.path()).unwrap();
        assert_eq!(fs::read(new_dir.join("assets/baron-1a2b3c4d.png")).unwrap(), b"fake png");
        let loaded = storage::load(&new_dir).unwrap();
        assert_eq!(loaded.name, "Next Adventure");
        assert_eq!(loaded.npcs.len(), c.npcs.len());
        assert!(loaded.story_cards.is_empty());

        // importing straight from another campaign folder works too
        let src2 = Source::open(&dir).unwrap();
        let mut target = Campaign::new("T");
        let (r, imgs) = import_into(&mut target, &new_dir, &src2, &[Part::Story, Part::Npcs], Duplicates::Skip).unwrap();
        assert!(r.added > 0);
        assert_eq!(imgs, 0); // the baron's portrait is already there
        assert_eq!(target.story_cards.len(), c.story_cards.len());
    }
}
