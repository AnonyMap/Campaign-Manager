//! Top-level application: window layout, menus, navigation, saving, dialogs and the
//! deferred-action queue that views use to talk to each other.

use std::path::{Path, PathBuf};

use cm_core::storage;
use cm_core::wiki::Vault;
use cm_core::{Campaign, CombatantSource, Id, Link, StoryCard};
use egui::{Color32, Key, Modifiers, RichText};
use egui_commonmark::CommonMarkCache;

use crate::settings::AppSettings;
use crate::ui_util;
use crate::views;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Home,
    Run,
    Cards,
    Board,
    Quests,
    Plot,
    Maps,
    Sessions,
    Threads,
    Players,
    Npcs,
    Bestiary,
    Factions,
    Places,
    Items,
    Loot,
    Encounters,
    Combat,
    Wiki,
    Search,
    Settings,
}

/// Marker for the hand-drawn plot-line icon (no font glyph looks like a story arc).
pub const PLOT_ICON: &str = "plot-arc";

/// Navigation groups: (heading, [(view, icon, label)]).
pub const NAV_GROUPS: &[(&str, &[(View, &str, &str)])] = &[
    ("CAMPAIGN", &[(View::Home, "🏠", "Overview"), (View::Run, "▶", "Run session")]),
    (
        "STORY",
        &[
            (View::Cards, "🃏", "Story cards"),
            (View::Board, "🕸", "Story board"),
            (View::Plot, PLOT_ICON, "Plot line"),
            (View::Threads, "📜", "Plot threads"),
            (View::Quests, "❗", "Quests"),
            (View::Sessions, "📅", "Sessions"),
        ],
    ),
    (
        "WORLD",
        &[(View::Places, "📍", "Places"), (View::Maps, "🗺", "Maps"), (View::Factions, "🏰", "Factions")],
    ),
    (
        "PEOPLE",
        &[
            (View::Players, "🛡", "Player characters"),
            (View::Npcs, "👤", "NPCs & monsters"),
            (View::Bestiary, "🐉", "Bestiary"),
        ],
    ),
    ("TREASURE", &[(View::Items, "💎", "Items"), (View::Loot, "💰", "Loot generator")]),
    ("COMBAT", &[(View::Encounters, "📋", "Encounters"), (View::Combat, "⚔", "Combat tracker")]),
    ("REFERENCE", &[(View::Wiki, "📖", "Campaign wiki")]),
];

impl View {
    pub const NAV: [(View, &'static str); 20] = [
        (View::Home, "Overview"),
        (View::Run, "Run session"),
        (View::Cards, "Story cards"),
        (View::Board, "Story board"),
        (View::Plot, "Plot line"),
        (View::Threads, "Plot threads"),
        (View::Quests, "Quests"),
        (View::Sessions, "Sessions"),
        (View::Places, "Places"),
        (View::Maps, "Maps"),
        (View::Factions, "Factions"),
        (View::Players, "Player characters"),
        (View::Npcs, "NPCs & monsters"),
        (View::Bestiary, "Bestiary"),
        (View::Items, "Items"),
        (View::Loot, "Loot generator"),
        (View::Encounters, "Encounters"),
        (View::Combat, "Combat tracker"),
        (View::Wiki, "Campaign wiki"),
        (View::Settings, "Settings"),
    ];
}

/// Things a view asks the app to do after the frame's UI has been drawn.
#[derive(Debug, Clone)]
pub enum Action {
    Open(Link),
    /// A `[[target]]` link was clicked.
    WikiLink(String),
    OpenWiki(String),
    Roll {
        label: String,
        expr: String,
        roller: String,
    },
    ConfirmDelete(Link),
    ViewImage(String),
    StartEncounter(Id),
    AddToCombat(CombatantSource),
    PlaceOnMap(Id),
    NewCardInSession(Id),
    ExportToWiki(Link),
    ExportAll,
    GoView(View),
    /// Select a quest in the Quests view.
    GoQuest(Id),
    /// Centre the story board on a card (putting it on the board if needed).
    ShowOnBoard(Id),
    /// Open the loot generator for an encounter (None = the combat tracker's fight).
    LootFor(Option<Id>),
    /// Open (switch to) a campaign folder.
    OpenCampaign(PathBuf),
    /// Open the import / export window.
    Transfer(views::transfer::Mode),
    ShowDice,
    Toast(String),
}

/// State shared by all views that isn't part of the campaign file.
pub struct Shared {
    pub md: CommonMarkCache,
    pub rng: fastrand::Rng,
    pub actions: Vec<Action>,
    pub vault: Option<Vault>,
    pub vault_error: Option<String>,
    pub status: Option<(String, f64)>,
    pub now: f64,
}

impl Shared {
    pub fn toast(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), self.now));
    }
}

/// An open campaign.
pub struct Doc {
    pub c: Campaign,
    pub dir: PathBuf,
    /// Snapshot (without dice history) at the last save / load.
    saved_json: String,
    saved_dice: (usize, Option<uuid::Uuid>),
    /// Snapshot at the last check.
    last_json: String,
    last_check: f64,
    last_change: f64,
    /// Time of the current frame (set by the app before views run).
    pub now: f64,
    /// Something may have changed since the last check (input arrived or an action ran).
    pub touched: bool,
    /// The next change starts a new undo step even if it follows quickly on the last one.
    new_step: bool,
    pub dirty: bool,
    undo: Vec<String>,
    redo: Vec<String>,
    pub save_error: Option<String>,
    last_save_attempt: f64,
}

/// Undo steps kept. Typing bursts are merged into one step and dice rolls aren't steps.
const UNDO_STEPS: usize = 100;

impl Doc {
    fn new(c: Campaign, dir: PathBuf) -> Self {
        let mut d = Doc {
            c,
            dir,
            saved_json: String::new(),
            saved_dice: (0, None),
            last_json: String::new(),
            last_check: 0.0,
            last_change: f64::NEG_INFINITY,
            now: 0.0,
            touched: false,
            new_step: true,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            save_error: None,
            last_save_attempt: f64::NEG_INFINITY,
        };
        d.last_json = d.snapshot();
        d.saved_json = d.last_json.clone();
        d.saved_dice = d.dice_marker();
        d
    }

    /// The campaign as JSON without the dice roll log: rolling dice isn't an edit you'd want
    /// to undo, and undoing an edit shouldn't wipe the rolls made since.
    fn snapshot(&mut self) -> String {
        let hist = std::mem::take(&mut self.c.dice.history);
        let inputs = std::mem::take(&mut self.c.dice.input_history);
        let json = storage::to_json(&self.c).unwrap_or_default();
        self.c.dice.history = hist;
        self.c.dice.input_history = inputs;
        json
    }

    fn dice_marker(&self) -> (usize, Option<uuid::Uuid>) {
        (self.c.dice.history.len(), self.c.dice.history.last().map(|r| r.id))
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.last_save_attempt = self.now;
        self.c.touch();
        if let Err(e) = storage::save(&self.c, &self.dir) {
            let e = e.to_string();
            self.save_error = Some(e.clone());
            return Err(e);
        }
        self.last_json = self.snapshot();
        self.saved_json = self.last_json.clone();
        self.saved_dice = self.dice_marker();
        self.dirty = false;
        self.save_error = None;
        Ok(())
    }

    /// Detect edits by comparing serialised snapshots (impossible to forget, unlike per-widget
    /// change flags). Only runs after input or an action, at most every 0.75 s unless forced.
    /// Also feeds undo history.
    fn track(&mut self, force: bool) {
        if !force && (!self.touched || self.now - self.last_check < 0.75) {
            return;
        }
        self.last_check = self.now;
        self.touched = false;
        let json = self.snapshot();
        if json != self.last_json {
            let prev = std::mem::replace(&mut self.last_json, json);
            // merge a burst of typing into one undo step
            let merge = !self.new_step && self.now - self.last_change < 1.5 && !self.undo.is_empty();
            if !merge {
                self.undo.push(prev);
                if self.undo.len() > UNDO_STEPS {
                    self.undo.remove(0);
                }
            }
            self.redo.clear();
            self.last_change = self.now;
            self.new_step = false;
        }
        self.dirty = self.last_json != self.saved_json || self.dice_marker() != self.saved_dice;
    }

    /// Record the current state as its own undo step (before something drastic like a delete).
    pub fn checkpoint(&mut self) {
        self.track(true);
        self.new_step = true;
    }

    fn restore(&mut self, json: String) -> bool {
        match storage::from_json(&json) {
            Ok(mut c) => {
                c.dice.history = std::mem::take(&mut self.c.dice.history);
                c.dice.input_history = std::mem::take(&mut self.c.dice.input_history);
                self.c = c;
                self.last_json = json;
                self.new_step = true;
                self.dirty = self.last_json != self.saved_json || self.dice_marker() != self.saved_dice;
                true
            }
            Err(_) => false,
        }
    }

    pub fn undo(&mut self) -> bool {
        self.track(true);
        let Some(prev) = self.undo.pop() else {
            return false;
        };
        let cur = self.last_json.clone();
        if self.restore(prev) {
            self.redo.push(cur);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        // an edit made in the last moment counts first (and, being new, ends redo)
        self.track(true);
        let Some(next) = self.redo.pop() else {
            return false;
        };
        let cur = self.last_json.clone();
        if self.restore(next) {
            self.undo.push(cur);
            true
        } else {
            false
        }
    }
}

#[derive(Default)]
pub struct ViewState {
    pub cards: views::cards::State,
    pub plot: views::plot::State,
    pub maps: views::maps::State,
    pub sessions: views::sessions::State,
    pub threads: views::threads::State,
    pub players: views::characters::State,
    pub npcs: views::characters::State,
    pub bestiary: views::bestiary::State,
    pub run: views::run::State,
    pub factions: views::factions::State,
    pub board: views::storyboard::State,
    pub quests: views::quests::State,
    pub places: views::places::State,
    pub items: views::items::State,
    pub loot: views::loot::State,
    pub transfer: views::transfer::State,
    pub encounters: views::encounters::State,
    pub combat: views::combat::State,
    pub wiki: views::wiki::State,
    pub dice: views::dice::State,
    pub search: views::search::State,
    pub settings: views::settings_view::State,
}

enum Dialog {
    None,
    NewCampaign {
        name: String,
        parent: String,
        sample: bool,
    },
    OpenPath {
        path: String,
    },
    ConfirmDelete(Link),
    About,
    DiceHelp,
    /// Closing the window but the campaign couldn't be saved.
    SaveFailedOnClose(String),
}

pub struct App {
    pub settings: AppSettings,
    style_applied: Option<AppSettings>,
    pub doc: Option<Doc>,
    pub view: View,
    pub vs: ViewState,
    pub sh: Shared,
    dialog: Dialog,
    image_viewer: Option<(String, f32)>,
    last_title: String,
    font_error: Option<String>,
    /// The user chose to quit even though saving failed.
    allow_close: bool,
    search_text: String,
    focus_search: bool,
    vault_loaded_for: Option<PathBuf>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, open_path: Option<PathBuf>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        // tried first: shrinks images too big for the graphics card instead of crashing
        cc.egui_ctx
            .add_image_loader(std::sync::Arc::new(crate::image_loader::FitImageLoader::default()));
        let settings = AppSettings::load();
        let mut app = App {
            settings,
            style_applied: None,
            doc: None,
            view: View::Home,
            vs: ViewState::default(),
            sh: Shared {
                md: CommonMarkCache::default(),
                rng: fastrand::Rng::new(),
                actions: Vec::new(),
                vault: None,
                vault_error: None,
                status: None,
                now: 0.0,
            },
            dialog: Dialog::None,
            image_viewer: None,
            last_title: String::new(),
            font_error: None,
            allow_close: false,
            search_text: String::new(),
            focus_search: false,
            vault_loaded_for: None,
        };
        let target = open_path.or_else(|| {
            if app.settings.reopen_last {
                app.settings.last_campaign.as_ref().map(PathBuf::from).filter(|p| storage::is_campaign_dir(p))
            } else {
                None
            }
        });
        if let Some(p) = target {
            app.open_campaign(&p);
        }
        app
    }

    // -- campaign file operations --------------------------------------------

    /// Before leaving this campaign: save a wiki note being edited. If that isn't possible,
    /// stay (on the wiki page, which says why) rather than lose the edit.
    fn leave_wiki_note(&mut self) -> bool {
        if self.vs.wiki.has_unsaved().is_none() || views::wiki::save_open_note(&mut self.sh, &mut self.vs.wiki) {
            return true;
        }
        self.view = View::Wiki;
        self.sh.toast("Save or discard your wiki note first");
        false
    }

    pub fn open_campaign(&mut self, path: &Path) {
        if !self.leave_wiki_note() || !self.save_if_dirty() {
            return; // keep the current campaign open rather than lose its changes
        }
        let dir = storage::campaign_dir(path);
        match storage::load(&dir) {
            Ok(c) => {
                self.set_doc(c, dir);
            }
            Err(e) => self.sh.toast(format!("Couldn't open campaign: {e}")),
        }
    }

    fn set_doc(&mut self, c: Campaign, dir: PathBuf) {
        let p = dir.to_string_lossy().to_string();
        self.settings.push_recent(&p);
        self.settings.save();
        self.sh.toast(format!("Opened {}", c.name));
        self.doc = Some(Doc::new(c, dir));
        self.vs = ViewState::default();
        self.view = View::Home;
        self.sh.vault = None;
        self.sh.vault_error = None;
        self.vault_loaded_for = None;
    }

    fn create_campaign(&mut self, name: &str, parent: &str, sample: bool) {
        let name = if name.trim().is_empty() { "New Campaign" } else { name.trim() };
        let mut dir = PathBuf::from(parent.trim()).join(storage::sanitize_file_name(name));
        let mut i = 2;
        while dir.exists() {
            dir = PathBuf::from(parent.trim()).join(format!("{} {i}", storage::sanitize_file_name(name)));
            i += 1;
        }
        let mut c = if sample { cm_core::sample_campaign() } else { Campaign::new(name) };
        c.name = name.to_string();
        match storage::save(&c, &dir) {
            Ok(_) => {
                if self.leave_wiki_note() && self.save_if_dirty() {
                    self.set_doc(c, dir);
                }
            }
            Err(e) => self.sh.toast(format!("Couldn't create campaign: {e}")),
        }
    }

    pub fn save_now(&mut self) {
        if let Some(doc) = &mut self.doc {
            match doc.save() {
                Ok(()) => self.sh.toast("Saved"),
                Err(e) => {
                    doc.save_error = Some(e.clone());
                    self.sh.toast(format!("Save failed: {e}"));
                }
            }
        }
    }

    /// Save pending changes. False if there were changes and they couldn't be written.
    fn save_if_dirty(&mut self) -> bool {
        if let Some(doc) = &mut self.doc {
            doc.track(true);
            if doc.dirty {
                if let Err(e) = doc.save() {
                    self.sh.toast(format!("Couldn't save: {e}"));
                    return false;
                }
            }
        }
        true
    }

    fn close_campaign(&mut self) {
        if !self.leave_wiki_note() || !self.save_if_dirty() {
            return;
        }
        self.doc = None;
        self.settings.last_campaign = None;
        self.settings.save();
        self.view = View::Home;
    }

    fn ensure_vault(&mut self) {
        let Some(doc) = &self.doc else { return };
        let root = doc.c.wiki_root(&doc.dir);
        if root == self.vault_loaded_for {
            return;
        }
        self.vault_loaded_for = root.clone();
        match root {
            Some(r) => match Vault::open(&r) {
                Ok(v) => {
                    self.sh.vault = Some(v);
                    self.sh.vault_error = None;
                }
                Err(e) => {
                    self.sh.vault = None;
                    self.sh.vault_error = Some(e.to_string());
                }
            },
            None => {
                self.sh.vault = None;
                self.sh.vault_error = None;
            }
        }
    }

    // -- actions ---------------------------------------------------------------

    fn process_actions(&mut self, ctx: &egui::Context) {
        let actions = std::mem::take(&mut self.sh.actions);
        for a in actions {
            self.process(ctx, a);
        }
    }

    fn process(&mut self, ctx: &egui::Context, a: Action) {
        match a {
            Action::Open(link) => self.navigate(ctx, link),
            Action::WikiLink(target) => {
                let target = cm_core::wiki::decode_target(&target);
                if let Some(doc) = &self.doc {
                    if let Some(link) = doc.c.resolve_name(&target) {
                        self.navigate(ctx, link);
                        return;
                    }
                }
                if let Some(v) = &self.sh.vault {
                    if let Some(n) = v.resolve(&target) {
                        let p = n.rel_path.clone();
                        self.navigate(ctx, Link::Wiki(p));
                        return;
                    }
                }
                if target.starts_with("http://") || target.starts_with("https://") {
                    ctx.open_url(egui::OpenUrl::new_tab(target));
                    return;
                }
                self.view = View::Wiki;
                self.vs.wiki.offer_create(&target);
                self.sh.toast(format!("No card, character or wiki page named “{target}”"));
            }
            Action::OpenWiki(p) => self.navigate(ctx, Link::Wiki(p)),
            Action::Roll { label, expr, roller } => {
                if let Some(doc) = &mut self.doc {
                    views::dice::roll_into_history(&mut doc.c, &label, &expr, &roller, &mut self.sh);
                    self.settings.show_dice_panel = true;
                }
            }
            Action::ConfirmDelete(l) => self.dialog = Dialog::ConfirmDelete(l),
            Action::ViewImage(p) => self.image_viewer = Some((p, 1.0)),
            Action::StartEncounter(id) => {
                if let Some(doc) = &mut self.doc {
                    if let Some(enc) = doc.c.encounter(id).cloned() {
                        if doc.c.active_encounter.as_ref().map(|r| !r.combatants.is_empty()).unwrap_or(false) {
                            self.sh.toast("Replaced the previous combat in the tracker (use Undo to get it back)");
                        }
                        let running = cm_core::encounter::RunningEncounter::from_premade(&enc, &doc.c);
                        doc.c.active_encounter = Some(running);
                        self.view = View::Combat;
                    }
                }
            }
            Action::AddToCombat(src) => {
                if let Some(doc) = &mut self.doc {
                    let c = &doc.c;
                    let mut r = doc
                        .c
                        .active_encounter
                        .clone()
                        .unwrap_or_else(|| cm_core::encounter::RunningEncounter::new("Encounter"));
                    r.add_from_campaign(c, src);
                    doc.c.active_encounter = Some(r);
                    self.sh.toast("Added to combat tracker");
                }
            }
            Action::PlaceOnMap(card) => {
                self.view = View::Maps;
                self.vs.maps.placing = Some(card);
                if self.vs.maps.selected.is_none() {
                    self.vs.maps.selected = self.doc.as_ref().and_then(|d| d.c.maps.first().map(|m| m.id));
                }
            }
            Action::NewCardInSession(sid) => {
                if let Some(doc) = &mut self.doc {
                    let card = StoryCard::default();
                    let cid = card.id;
                    doc.c.story_cards.push(card);
                    doc.c.assign_card_to_session(cid, sid);
                    self.vs.cards.selected = Some(cid);
                    self.view = View::Cards;
                }
            }
            Action::ExportToWiki(link) => self.export_to_wiki(&link),
            Action::ExportAll => self.export_all_to_wiki(),
            Action::GoView(v) => self.view = v,
            Action::GoQuest(id) => {
                self.view = View::Quests;
                self.vs.quests.selected = Some(id);
            }
            Action::ShowOnBoard(id) => {
                if let Some(doc) = &mut self.doc {
                    self.vs.board.focus(doc, id);
                    self.view = View::Board;
                }
            }
            Action::OpenCampaign(dir) => self.open_campaign(&dir),
            Action::Transfer(mode) => self.vs.transfer.start(mode, self.doc.as_ref()),
            Action::LootFor(enc) => {
                self.vs.loot.mode = views::loot::Mode::Encounter;
                self.vs.loot.source = Some(match enc {
                    Some(id) => views::loot::EncSource::Premade(id),
                    None => views::loot::EncSource::Active,
                });
                self.view = View::Loot;
            }
            Action::ShowDice => self.settings.show_dice_panel = true,
            Action::Toast(m) => self.sh.toast(m),
        }
    }

    pub fn navigate(&mut self, ctx: &egui::Context, link: Link) {
        match link {
            Link::Card(id) => {
                // quests stay in the quest log when opened from there
                let quest = self.doc.as_ref().and_then(|d| d.c.card(id)).map(|c| c.quest.is_some()).unwrap_or(false);
                if quest && self.view == View::Quests {
                    self.vs.quests.selected = Some(id);
                } else {
                    self.view = View::Cards;
                    self.vs.cards.selected = Some(id);
                }
            }
            Link::Npc(id) => {
                self.view = View::Npcs;
                self.vs.npcs.selected = Some(id);
            }
            Link::Player(id) => {
                self.view = View::Players;
                self.vs.players.selected = Some(id);
            }
            Link::Faction(id) => {
                self.view = View::Factions;
                self.vs.factions.selected = Some(id);
            }
            Link::Encounter(id) => {
                self.view = View::Encounters;
                self.vs.encounters.selected = Some(id);
            }
            Link::Session(id) => {
                self.view = View::Sessions;
                self.vs.sessions.selected = Some(id);
            }
            Link::Thread(id) => {
                self.view = View::Threads;
                self.vs.threads.selected = Some(id);
            }
            Link::Map(id) => {
                self.view = View::Maps;
                self.vs.maps.selected = Some(id);
            }
            Link::Wiki(p) => {
                self.view = View::Wiki;
                self.vs.wiki.open(&p);
            }
            Link::Url { url, .. } => ctx.open_url(egui::OpenUrl::new_tab(url)),
            Link::Image(p) => self.image_viewer = Some((p, 1.0)),
            Link::StatBlock(id) => {
                self.view = View::Bestiary;
                self.vs.bestiary.tab = views::bestiary::Tab::Campaign;
                self.vs.bestiary.selected = Some(id);
                self.vs.bestiary.editing = false;
            }
            Link::Item(id) => {
                self.view = View::Items;
                self.vs.items.show_campaign_item(id);
            }
            Link::Location(id) => {
                self.view = View::Places;
                self.vs.places.selected = Some(id);
            }
        }
    }

    fn export_to_wiki(&mut self, link: &Link) {
        match self.export_one(link) {
            Ok(Some(rel)) => self.sh.toast(format!("Exported to wiki: {rel}")),
            Ok(None) => {}
            Err(e) => self.sh.toast(e),
        }
    }

    /// Export one entry. Ok(None) when there's nothing to export for that link.
    fn export_one(&mut self, link: &Link) -> Result<Option<String>, String> {
        self.ensure_vault();
        let Some(doc) = &self.doc else { return Ok(None) };
        let Some(v) = &mut self.sh.vault else {
            return Err("Set a wiki (Obsidian vault) folder first in Campaign wiki or Settings".into());
        };
        let c = &doc.c;
        let base = if c.settings.wiki_export_folder.trim().is_empty() {
            "Campaign Manager".to_string()
        } else {
            c.settings.wiki_export_folder.trim().to_string()
        };
        use cm_core::wiki::*;
        let res = match link {
            Link::Card(id) => c.card(*id).map(|x| (format!("{base}/Story Cards"), x.display_title(), card_to_markdown(c, x))),
            Link::Npc(id) => c.npc(*id).map(|x| (format!("{base}/NPCs"), x.core.display_name(), npc_to_markdown(c, x))),
            Link::Player(id) => c
                .player(*id)
                .map(|x| (format!("{base}/Player Characters"), x.core.display_name(), player_to_markdown(c, x))),
            Link::Faction(id) => c.faction(*id).map(|x| (format!("{base}/Factions"), x.name.clone(), faction_to_markdown(c, x))),
            Link::Encounter(id) => c
                .encounter(*id)
                .map(|x| (format!("{base}/Encounters"), x.name.clone(), encounter_to_markdown(c, x))),
            Link::Location(id) => c
                .location(*id)
                .map(|x| (format!("{base}/Places"), x.display_name(), location_to_markdown(c, x))),
            _ => None,
        };
        let Some((folder, title, md)) = res else {
            return Ok(None);
        };
        let id = link.target_id().map(|i| i.to_string()).unwrap_or_default();
        v.export(&folder, &title, &id, &md).map(Some).map_err(|e| format!("Export failed: {e}"))
    }

    pub fn export_all_to_wiki(&mut self) {
        let Some(doc) = &self.doc else { return };
        let mut links = Vec::new();
        links.extend(doc.c.story_cards.iter().map(|x| Link::Card(x.id)));
        links.extend(doc.c.npcs.iter().map(|x| Link::Npc(x.id)));
        links.extend(doc.c.players.iter().map(|x| Link::Player(x.id)));
        links.extend(doc.c.factions.iter().map(|x| Link::Faction(x.id)));
        links.extend(doc.c.encounters.iter().map(|x| Link::Encounter(x.id)));
        links.extend(doc.c.locations.iter().map(|x| Link::Location(x.id)));
        let mut done = 0;
        let mut failed = 0;
        let mut last_err = String::new();
        for l in links {
            match self.export_one(&l) {
                Ok(Some(_)) => done += 1,
                Ok(None) => {}
                Err(e) => {
                    if self.sh.vault.is_none() {
                        self.sh.toast(e);
                        return; // no vault: one message, not one per entry
                    }
                    failed += 1;
                    last_err = e;
                }
            }
        }
        if failed > 0 {
            self.sh.toast(format!("Exported {done} entries; {failed} failed ({last_err})"));
        } else {
            self.sh.toast(format!("Exported {done} entries to the wiki"));
        }
    }

    // -- dropped files ------------------------------------------------------------

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        for p in dropped {
            if storage::is_campaign_dir(&p) {
                self.open_campaign(&p);
                continue;
            }
            // a campaign package: import it (or start a campaign from it)
            if p.extension().map(|e| e.eq_ignore_ascii_case(cm_core::transfer::PACKAGE_EXT)).unwrap_or(false) {
                self.vs.transfer.start_import(&p, self.doc.as_ref());
                continue;
            }
            let Some(doc) = &mut self.doc else {
                self.sh.toast("Open or create a campaign first");
                continue;
            };
            if p.is_dir() && self.view == View::Wiki {
                doc.c.settings.wiki_path = Some(ui_util::vault_setting(&doc.dir, &p));
                self.vault_loaded_for = None;
                continue;
            }
            if self.view == View::Bestiary
                && p.extension()
                    .map(|e| e.eq_ignore_ascii_case("json") || e.eq_ignore_ascii_case("txt"))
                    .unwrap_or(false)
            {
                match std::fs::read_to_string(&p) {
                    Ok(text) => self.vs.bestiary.import_text(text, &p),
                    Err(e) => self.sh.toast(format!("Couldn't read {}: {e}", p.display())),
                }
                continue;
            }
            if !ui_util::is_image_path(&p) {
                self.sh.toast(format!("Only images can be dropped here ({})", p.display()));
                continue;
            }
            let Some(rel) = ui_util::import(&doc.dir, &p, &mut self.sh) else {
                continue;
            };
            let c = &mut doc.c;
            let msg = match self.view {
                View::Maps => {
                    let m = cm_core::CampaignMap {
                        name: p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or("Map".into()),
                        image: rel,
                        ..Default::default()
                    };
                    self.vs.maps.selected = Some(m.id);
                    c.maps.push(m);
                    "Added a new map".to_string()
                }
                View::Cards if self.vs.cards.selected.is_some() => match c.card_mut(self.vs.cards.selected.unwrap()) {
                    Some(card) => {
                        card.links.push(Link::Image(rel));
                        "Image attached to card".into()
                    }
                    None => "No card selected".into(),
                },
                View::Npcs | View::Players => {
                    let sel = if self.view == View::Npcs {
                        self.vs.npcs.selected
                    } else {
                        self.vs.players.selected
                    };
                    let core = match (self.view, sel) {
                        (View::Npcs, Some(id)) => c.npc_mut(id).map(|n| &mut n.core),
                        (View::Players, Some(id)) => c.player_mut(id).map(|n| &mut n.core),
                        _ => None,
                    };
                    match core {
                        Some(core) if core.image.is_none() => {
                            core.image = Some(rel);
                            "Portrait set".into()
                        }
                        Some(core) => {
                            core.sheet_images.push(rel);
                            "Added to sheet / stat block images".into()
                        }
                        None => "Select a character first".into(),
                    }
                }
                View::Bestiary if self.vs.bestiary.selected.is_some() && self.vs.bestiary.tab == views::bestiary::Tab::Campaign => {
                    match c.statblock_mut(self.vs.bestiary.selected.unwrap()) {
                        Some(sb) if sb.core.image.is_none() => {
                            sb.core.image = Some(rel);
                            "Token image set".into()
                        }
                        Some(sb) => {
                            sb.core.sheet_images.push(rel);
                            "Added to the stat block's images".into()
                        }
                        None => "Select a stat block first".into(),
                    }
                }
                View::Sessions if self.vs.sessions.selected.is_some() => {
                    if let Some(s) = c.session_mut(self.vs.sessions.selected.unwrap()) {
                        s.links.push(Link::Image(rel));
                    }
                    "Image attached to session".into()
                }
                View::Places if self.vs.places.selected.is_some() => {
                    if let Some(l) = c.location_mut(self.vs.places.selected.unwrap()) {
                        if l.image.is_none() {
                            l.image = Some(rel);
                        } else {
                            l.links.push(Link::Image(rel));
                        }
                    }
                    "Image added to the place".into()
                }
                View::Factions if self.vs.factions.selected.is_some() => {
                    if let Some(f) = c.factions.iter_mut().find(|f| Some(f.id) == self.vs.factions.selected) {
                        f.image = Some(rel);
                    }
                    "Faction image set".into()
                }
                _ => "Image copied into the campaign's assets folder. Drop images on a selected card, character or map.".into(),
            };
            self.sh.toast(msg);
        }
    }

    // -- layout ---------------------------------------------------------------------

    fn menu_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New campaign…").clicked() {
                    self.dialog = Dialog::NewCampaign {
                        name: "New Campaign".into(),
                        parent: crate::settings::default_campaigns_dir().to_string_lossy().to_string(),
                        sample: false,
                    };
                    ui.close();
                }
                if ui.button("New sample campaign…").clicked() {
                    self.dialog = Dialog::NewCampaign {
                        name: "Sample Campaign".into(),
                        parent: crate::settings::default_campaigns_dir().to_string_lossy().to_string(),
                        sample: true,
                    };
                    ui.close();
                }
                if ui.button("Open campaign folder…").clicked() {
                    ui.close();
                    match ui_util::pick_folder("Open campaign folder") {
                        Some(p) => self.open_campaign(&p),
                        None => self.dialog = Dialog::OpenPath { path: String::new() },
                    }
                }
                if ui.button("Open by path…").clicked() {
                    self.dialog = Dialog::OpenPath { path: String::new() };
                    ui.close();
                }
                ui.menu_button("Open recent", |ui| {
                    if self.settings.recent.is_empty() {
                        ui.weak("(none)");
                    }
                    for r in self.settings.recent.clone() {
                        if ui.button(&r).clicked() {
                            self.open_campaign(Path::new(&r));
                            ui.close();
                        }
                    }
                });
                ui.separator();
                let has = self.doc.is_some();
                if ui.add_enabled(has, egui::Button::new("Save").shortcut_text("Ctrl+S")).clicked() {
                    self.save_now();
                    ui.close();
                }
                ui.menu_button("Import / export", |ui| {
                    use views::transfer::Mode;
                    if ui
                        .add_enabled(has, egui::Button::new("New campaign from this one…"))
                        .on_hover_text("Same world, new story: reuse the NPCs, places, items, bestiary… of this campaign")
                        .clicked()
                    {
                        self.vs.transfer.start(Mode::NewFromThis, self.doc.as_ref());
                        ui.close();
                    }
                    if ui
                        .button("New campaign from a file…")
                        .on_hover_text("From a .cmpack package or another campaign's folder")
                        .clicked()
                    {
                        self.vs.transfer.start(Mode::NewFromFile, self.doc.as_ref());
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has, egui::Button::new("Export…"))
                        .on_hover_text("Save all or part of this campaign as a .cmpack package")
                        .clicked()
                    {
                        self.vs.transfer.start(Mode::Export, self.doc.as_ref());
                        ui.close();
                    }
                    if ui
                        .add_enabled(has, egui::Button::new("Import into this campaign…"))
                        .on_hover_text("Bring in things from a package or another campaign")
                        .clicked()
                    {
                        self.vs.transfer.start(Mode::Import, self.doc.as_ref());
                        ui.close();
                    }
                });
                ui.separator();
                if ui.add_enabled(has, egui::Button::new("Show campaign folder")).clicked() {
                    if let Some(d) = &self.doc {
                        let dir = d.dir.clone();
                        if !dir.is_dir() {
                            self.sh.toast(format!("Folder not found: {}", dir.display()));
                        } else if let Err(e) = ui_util::open_with_os(&dir.to_string_lossy()) {
                            self.sh.toast(format!("Couldn't open the folder: {e}. It's at {}", dir.display()));
                        }
                    }
                    ui.close();
                }
                if ui.add_enabled(has, egui::Button::new("Close campaign")).clicked() {
                    self.close_campaign();
                    ui.close();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Edit", |ui| {
                let (u, r) = self.doc.as_ref().map(|d| (!d.undo.is_empty(), !d.redo.is_empty())).unwrap_or((false, false));
                if ui.add_enabled(u, egui::Button::new("Undo last change").shortcut_text("Ctrl+Z")).clicked() {
                    self.do_undo();
                    ui.close();
                }
                if ui.add_enabled(r, egui::Button::new("Redo").shortcut_text("Ctrl+Y")).clicked() {
                    self.do_redo();
                    ui.close();
                }
                ui.weak("Undo restores the campaign to how it was\na moment before the last edit.");
            });
            ui.menu_button("View", |ui| {
                for (v, label) in View::NAV {
                    if ui.selectable_label(self.view == v, label).clicked() {
                        self.view = v;
                        ui.close();
                    }
                }
                ui.separator();
                ui.checkbox(&mut self.settings.show_dice_panel, "Dice roller panel (Ctrl+D)");
                ui.separator();
                ui.menu_button("Theme", |ui| {
                    for b in crate::themes::BUILTIN {
                        if ui.selectable_label(self.settings.theme_name() == b.name, b.name).clicked() {
                            self.settings.theme_name = Some(b.name.to_string());
                            ui.close();
                        }
                    }
                    for t in self.settings.user_themes.clone() {
                        if ui.selectable_label(self.settings.theme_name() == t.name, &t.name).clicked() {
                            self.settings.theme_name = Some(t.name.clone());
                            ui.close();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Text size");
                    if ui.button("A−").clicked() {
                        self.settings.font_size = (self.settings.font_size - 1.0).max(9.0);
                    }
                    ui.label(format!("{:.0}", self.settings.font_size));
                    if ui.button("A+").clicked() {
                        self.settings.font_size = (self.settings.font_size + 1.0).min(32.0);
                    }
                });
            });
            ui.menu_button("Tools", |ui| {
                if ui.button("Dice roller").clicked() {
                    self.settings.show_dice_panel = true;
                    self.vs.dice.focus = true;
                    ui.close();
                }
                if ui.button("Dice syntax help").clicked() {
                    self.dialog = Dialog::DiceHelp;
                    ui.close();
                }
                if ui.add_enabled(self.doc.is_some(), egui::Button::new("Export everything to wiki")).clicked() {
                    self.export_all_to_wiki();
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Dice syntax").clicked() {
                    self.dialog = Dialog::DiceHelp;
                    ui.close();
                }
                if ui.button("About").clicked() {
                    self.dialog = Dialog::About;
                    ui.close();
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(doc) = &self.doc {
                ui.label(RichText::new(&doc.c.name).strong());
                ui.separator();
            }
            let has = self.doc.is_some();
            if ui.add_enabled(has, egui::Button::new("➕ Card")).on_hover_text("New story card").clicked() {
                if let Some(doc) = &mut self.doc {
                    let card = StoryCard::default();
                    self.vs.cards.selected = Some(card.id);
                    doc.c.story_cards.push(card);
                    self.view = View::Cards;
                }
            }
            if ui.add_enabled(has, egui::Button::new("➕ NPC")).clicked() {
                if let Some(doc) = &mut self.doc {
                    let npc = cm_core::Npc::default();
                    self.vs.npcs.selected = Some(npc.id);
                    doc.c.npcs.push(npc);
                    self.view = View::Npcs;
                }
            }
            if ui.add_enabled(has, egui::Button::new("⚔ Combat")).clicked() {
                self.view = View::Combat;
            }
            if ui.button("🎲 Dice").on_hover_text("Show/hide the dice roller (Ctrl+D)").clicked() {
                self.settings.show_dice_panel = !self.settings.show_dice_panel;
            }
            ui.separator();
            let r = ui.add_enabled(
                has,
                egui::TextEdit::singleline(&mut self.search_text)
                    .hint_text("🔍 Search campaign & wiki (Ctrl+F)")
                    .desired_width(260.0),
            );
            if self.focus_search {
                r.request_focus();
                self.focus_search = false;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && !self.search_text.trim().is_empty() {
                self.vs.search.query = self.search_text.clone();
                self.view = View::Search;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                crate::theme_ui::theme_menu(ui, &mut self.settings);
                if ui.button("A+").on_hover_text("Larger text").clicked() {
                    self.settings.font_size = (self.settings.font_size + 1.0).min(32.0);
                }
                if ui.button("A−").on_hover_text("Smaller text").clicked() {
                    self.settings.font_size = (self.settings.font_size - 1.0).max(9.0);
                }
            });
        });
    }

    fn nav_panel(&mut self, ui: &mut egui::Ui, theme: &crate::themes::Theme) {
        let Some(doc) = &self.doc else { return };
        let c = &doc.c;
        let collapsed = self.settings.nav_collapsed;
        let count = |v: View| -> Option<String> {
            Some(match v {
                View::Cards => c.story_cards.len().to_string(),
                View::Maps => c.maps.len().to_string(),
                View::Sessions => c.sessions.len().to_string(),
                View::Threads => c.threads.iter().filter(|t| t.status == cm_core::ThreadStatus::Open).count().to_string(),
                View::Players => c.players.len().to_string(),
                View::Npcs => c.npcs.len().to_string(),
                View::Bestiary => c.statblocks.len().to_string(),
                View::Factions => c.factions.len().to_string(),
                View::Places => c.locations.len().to_string(),
                View::Items => c.items.len().to_string(),
                View::Quests => c
                    .story_cards
                    .iter()
                    .filter(|x| x.quest.as_ref().map(|q| q.status.open()).unwrap_or(false))
                    .count()
                    .to_string(),
                View::Encounters => c.encounters.len().to_string(),
                View::Combat => match &c.active_encounter {
                    Some(r) if r.started => format!("R{}", r.round),
                    _ => return None,
                },
                _ => return None,
            })
        };
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            let (icon, tip) = if collapsed {
                ("»", "Show labels")
            } else {
                ("«", "Icons only (more room for content)")
            };
            if ui.add(egui::Button::new(icon).frame(false)).on_hover_text(tip).clicked() {
                self.settings.nav_collapsed = !collapsed;
            }
        });
        let mut clicked = None;
        let theme_fill = crate::themes::c(theme.palette.nav(theme.dark));
        // Settings pinned to the bottom; the tab list scrolls if the window is short.
        egui::TopBottomPanel::bottom("nav_bottom")
            .frame(egui::Frame::new().fill(theme_fill))
            .show_separator_line(false)
            .show_inside(ui, |ui| {
                ui.separator();
                if nav_tab(ui, "⚙", "Settings", None, self.view == View::Settings, collapsed, theme).clicked() {
                    clicked = Some(View::Settings);
                }
                if !collapsed {
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.weak(format!("{} in ideas pile", ui_util::plural(c.ideas_pile().len(), "card")));
                    });
                }
                ui.add_space(4.0);
            });
        egui::ScrollArea::vertical().id_salt("nav_scroll").auto_shrink([false, false]).show(ui, |ui| {
            for (gi, (heading, items)) in NAV_GROUPS.iter().enumerate() {
                if collapsed {
                    if gi > 0 {
                        ui.add_space(4.0);
                        ui.separator();
                    }
                } else {
                    ui.add_space(if gi == 0 { 2.0 } else { 6.0 });
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.label(RichText::new(*heading).small().strong().color(ui.visuals().weak_text_color()));
                    });
                }
                for (v, icon, label) in items.iter() {
                    if nav_tab(ui, icon, label, count(*v), self.view == *v, collapsed, theme).clicked() {
                        clicked = Some(*v);
                    }
                }
            }
        });
        if let Some(v) = clicked {
            self.view = v;
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let mut save = false;
        ui.horizontal(|ui| {
            match &self.doc {
                Some(doc) => {
                    if let Some(e) = &doc.save_error {
                        ui.colored_label(ui.visuals().error_fg_color, format!("⚠ Not saved: {e}"));
                        if ui.small_button("Try again").clicked() {
                            save = true;
                        }
                    } else if doc.dirty {
                        ui.label("⏺ Unsaved changes");
                        if ui.small_button("Save now").clicked() {
                            save = true;
                        }
                    } else {
                        ui.weak("✔ All changes saved");
                    }
                    ui.separator();
                    ui.weak(doc.dir.to_string_lossy());
                }
                None => {
                    ui.weak("No campaign open");
                }
            }
            if let Some((msg, t)) = &self.sh.status {
                if self.sh.now - t < 6.0 && !msg.is_empty() {
                    ui.separator();
                    ui.label(RichText::new(msg).color(ui.visuals().hyperlink_color));
                }
            }
            if let Some(e) = &self.font_error {
                ui.separator();
                ui.colored_label(Color32::from_rgb(200, 120, 0), e);
            }
        });
        if save {
            self.save_now();
        }
    }

    fn start_screen(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.heading(cm_core::APP_NAME);
            ui.label("A campaign notebook for Dungeon Masters.");
            ui.add_space(20.0);
            if ui.add_sized([260.0, 30.0], egui::Button::new("Create a new campaign…")).clicked() {
                self.dialog = Dialog::NewCampaign {
                    name: "New Campaign".into(),
                    parent: crate::settings::default_campaigns_dir().to_string_lossy().to_string(),
                    sample: false,
                };
            }
            if ui.add_sized([260.0, 30.0], egui::Button::new("Open a campaign folder…")).clicked() {
                match ui_util::pick_folder("Open campaign folder") {
                    Some(p) => self.open_campaign(&p),
                    None => self.dialog = Dialog::OpenPath { path: String::new() },
                }
            }
            if ui.add_sized([260.0, 30.0], egui::Button::new("Try the sample campaign")).clicked() {
                self.dialog = Dialog::NewCampaign {
                    name: "Sample Campaign".into(),
                    parent: crate::settings::default_campaigns_dir().to_string_lossy().to_string(),
                    sample: true,
                };
            }
            if ui
                .add_sized([260.0, 30.0], egui::Button::new("New campaign from a file…"))
                .on_hover_text("From a .cmpack package or another campaign's folder")
                .clicked()
            {
                self.vs.transfer.start(views::transfer::Mode::NewFromFile, None);
            }
            ui.add_space(20.0);
            if !self.settings.recent.is_empty() {
                ui.label(RichText::new("Recent campaigns").strong());
                for r in self.settings.recent.clone() {
                    let exists = storage::is_campaign_dir(Path::new(&r));
                    let resp = ui.add_enabled(exists, egui::Link::new(&r));
                    if resp.clicked() {
                        self.open_campaign(Path::new(&r));
                    }
                }
            }
            ui.add_space(20.0);
            ui.weak("Campaigns are folders containing campaign.json (plain JSON) and an assets/ folder for images.\nCopy or sync the folder to use it on another computer.");
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let mut close = false;
        let mut create: Option<(String, String, bool)> = None;
        let mut open: Option<String> = None;
        let mut delete: Option<Link> = None;
        match &mut self.dialog {
            Dialog::None => {}
            Dialog::NewCampaign { name, parent, sample } => {
                egui::Window::new(if *sample { "New sample campaign" } else { "New campaign" })
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        egui::Grid::new("newc").num_columns(2).show(ui, |ui| {
                            ui.label("Name");
                            ui.add_sized([300.0, ui.spacing().interact_size.y], egui::TextEdit::singleline(name).desired_width(300.0));
                            ui.end_row();
                            ui.label("Save in");
                            ui.horizontal(|ui| {
                                ui.add_sized([300.0, ui.spacing().interact_size.y], egui::TextEdit::singleline(parent).desired_width(300.0));
                                if ui.button("Browse…").clicked() {
                                    if let Some(p) = ui_util::pick_folder("Choose where to save the campaign") {
                                        *parent = p.to_string_lossy().to_string();
                                    }
                                }
                            });
                            ui.end_row();
                        });
                        ui.weak(format!(
                            "A folder named “{}” will be created there.",
                            storage::sanitize_file_name(if name.trim().is_empty() { "New Campaign" } else { name.trim() })
                        ));
                        ui.horizontal(|ui| {
                            if ui.button("Create").clicked() {
                                create = Some((name.clone(), parent.clone(), *sample));
                            }
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    });
            }
            Dialog::OpenPath { path } => {
                egui::Window::new("Open campaign")
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label("Path to a campaign folder (the folder that contains campaign.json):");
                        let r = ui.add_sized([420.0, ui.spacing().interact_size.y], egui::TextEdit::singleline(path).desired_width(420.0));
                        ui.horizontal(|ui| {
                            if ui.button("Open").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))) {
                                open = Some(path.trim().trim_matches('"').to_string());
                            }
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    });
            }
            Dialog::ConfirmDelete(link) => {
                let name = self.doc.as_ref().and_then(|d| d.c.link_name(link)).unwrap_or_default();
                egui::Window::new("Delete?")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(format!("Delete {} “{}”?", link.kind_label().to_lowercase(), name));
                        ui.weak("Links to it will be removed. You can undo with Edit » Undo.");
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Delete").color(ui.visuals().error_fg_color)).clicked() {
                                delete = Some(link.clone());
                            }
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    });
            }
            Dialog::SaveFailedOnClose(err) => {
                let mut quit = false;
                let mut retry = false;
                egui::Window::new("Couldn't save")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label("Your latest changes couldn't be written to the campaign folder:");
                        ui.colored_label(ui.visuals().error_fg_color, err.as_str());
                        ui.weak("If a sync app (Dropbox, OneDrive…) is holding the file, wait a moment and try again.");
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Try again").strong()).clicked() {
                                retry = true;
                            }
                            if ui.button("Keep working").clicked() {
                                close = true;
                            }
                            if ui.button(RichText::new("Quit without saving").color(ui.visuals().error_fg_color)).clicked() {
                                quit = true;
                            }
                        });
                    });
                if retry {
                    if self.save_if_dirty() {
                        self.allow_close = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    } else if let Some(e) = self.doc.as_ref().and_then(|d| d.save_error.clone()) {
                        self.dialog = Dialog::SaveFailedOnClose(e);
                        return;
                    }
                }
                if quit {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if retry || quit {
                    close = true;
                }
            }
            Dialog::About => {
                egui::Window::new("About")
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.heading(cm_core::APP_NAME);
                        ui.label(format!("Version {}", cm_core::APP_VERSION));
                        ui.label("A campaign manager for Dungeon Masters.");
                        ui.label("Condition summaries paraphrase the 2024 rules glossary (SRD 5.2, CC-BY-4.0).");
                        ui.add_space(6.0);
                        ui.label(RichText::new("Bundled monsters and items").strong());
                        ui.set_max_width(520.0);
                        ui.label(RichText::new(cm_core::bestiary::SRD_ATTRIBUTION).small());
                        ui.add_space(6.0);
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
            }
            Dialog::DiceHelp => {
                let mut open_w = true;
                egui::Window::new("Dice syntax").open(&mut open_w).default_width(620.0).show(ctx, |ui| {
                    ui.label(RichText::new(cm_core::dice::HELP_TEXT).monospace());
                });
                if !open_w {
                    close = true;
                }
            }
        }
        if let Some((n, p, s)) = create {
            self.dialog = Dialog::None;
            self.create_campaign(&n, &p, s);
        } else if let Some(p) = open {
            self.dialog = Dialog::None;
            self.open_campaign(Path::new(&p));
        } else if let Some(l) = delete {
            self.dialog = Dialog::None;
            if let Some(doc) = &mut self.doc {
                doc.checkpoint();
                doc.c.delete(&l);
                doc.touched = true;
                doc.checkpoint();
                self.sh.toast("Deleted — Edit » Undo brings it back");
            }
        } else if close {
            self.dialog = Dialog::None;
        }
    }

    fn image_window(&mut self, ctx: &egui::Context) {
        let Some((rel, zoom)) = &mut self.image_viewer else {
            return;
        };
        let Some(doc) = &self.doc else { return };
        let mut open = true;
        egui::Window::new(rel.rsplit('/').next().unwrap_or("Image").to_string())
            .id(egui::Id::new("image_viewer"))
            .open(&mut open)
            .default_size([700.0, 560.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("−").clicked() {
                        *zoom = (*zoom / 1.25).max(0.1);
                    }
                    ui.label(format!("{:.0}%", *zoom * 100.0));
                    if ui.button("+").clicked() {
                        *zoom = (*zoom * 1.25).min(8.0);
                    }
                    if ui.button("Fit").clicked() {
                        *zoom = 1.0;
                    }
                });
                egui::ScrollArea::both().show(ui, |ui| {
                    let avail = ui.available_size();
                    let img = egui::Image::new(ui_util::file_uri(&doc.dir, rel));
                    if (*zoom - 1.0).abs() < f32::EPSILON {
                        ui.add(img.max_size(avail).maintain_aspect_ratio(true));
                    } else {
                        ui.add(img.fit_to_original_size(*zoom));
                    }
                });
            });
        if !open {
            self.image_viewer = None;
        }
    }

    fn do_undo(&mut self) {
        if let Some(d) = &mut self.doc {
            if d.undo() {
                self.sh.toast("Undid last change");
            }
        }
    }
    fn do_redo(&mut self) {
        if let Some(d) = &mut self.doc {
            if d.redo() {
                self.sh.toast("Redid change");
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let text_focused = ctx.memory(|m| m.focused().is_some()) && ctx.wants_keyboard_input();
        let (save, dice, find, undo, redo, help) = ctx.input_mut(|i| {
            // Ctrl+Shift+Z first: egui matches Ctrl+Z even with Shift held
            let redo = !text_focused && (i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y));
            (
                i.consume_key(Modifiers::COMMAND, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::D),
                i.consume_key(Modifiers::COMMAND, Key::F),
                !text_focused && i.consume_key(Modifiers::COMMAND, Key::Z),
                redo,
                i.consume_key(Modifiers::NONE, Key::F1),
            )
        });
        if save {
            self.save_now();
        }
        if dice {
            self.settings.show_dice_panel = !self.settings.show_dice_panel;
            if self.settings.show_dice_panel {
                self.vs.dice.focus = true;
            }
        }
        if find {
            self.focus_search = true;
        }
        if undo {
            self.do_undo();
        }
        if redo {
            self.do_redo();
        }
        if help {
            self.dialog = Dialog::DiceHelp;
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sh.now = ctx.input(|i| i.time);
        if let Some(doc) = &mut self.doc {
            doc.now = self.sh.now;
            // edits only happen in frames with input (or actions, marked below)
            if ctx.input(|i| !i.events.is_empty() || i.pointer.any_down() || !i.raw.dropped_files.is_empty()) {
                doc.touched = true;
            }
        }
        // status messages fade after six seconds (and stop the once-a-second repaint)
        if self.sh.status.as_ref().map(|(_, t)| self.sh.now - t > 6.0).unwrap_or(false) {
            self.sh.status = None;
        }

        // Theme / fonts (re-applied only when settings change).
        if self.style_applied.as_ref() != Some(&self.settings) {
            let fonts_changed = self
                .style_applied
                .as_ref()
                .map(|s| s.font != self.settings.font || s.custom_font_path != self.settings.custom_font_path)
                .unwrap_or(true);
            if fonts_changed {
                self.font_error = self.settings.apply_fonts(ctx).err();
            }
            self.settings.apply_style(ctx);
            let prev = self.style_applied.replace(self.settings.clone());
            if prev.is_some() {
                self.settings.save();
            }
        }

        let title = match &self.doc {
            Some(d) => format!("{}{} — {}", if d.dirty { "• " } else { "" }, d.c.name, cm_core::APP_NAME),
            None => cm_core::APP_NAME.to_string(),
        };
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }

        if let Some(e) = ui_util::take_open_error() {
            self.sh.toast(e);
        }
        if ui_util::log::take_dialog_failed() {
            self.sh
                .toast("The system file dialog failed to open — paste a path instead, or drag the file onto the window.");
        }
        ui_util::set_date_prefs(ctx, self.settings.week_start, self.settings.date_style);
        // "Markdown" toggle in the text editors <-> the saved setting
        if crate::rich_text::take_pref_changed() {
            self.settings.markdown_source = crate::rich_text::show_markdown();
        } else {
            crate::rich_text::set_show_markdown(self.settings.markdown_source);
        }
        self.shortcuts(ctx);
        self.handle_dropped_files(ctx);
        if self.view == View::Wiki || matches!(self.vs.wiki.pending_reload, true) {
            self.ensure_vault();
            self.vs.wiki.pending_reload = false;
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            self.menu_bar(ui, ctx);
        });
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(2.0);
            self.toolbar(ui);
            ui.add_space(2.0);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| self.status_bar(ui));

        if self.doc.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| self.start_screen(ui));
        } else {
            let theme = self.settings.active_theme(ctx);
            let nav_frame = egui::Frame::new()
                .fill(crate::themes::c(theme.palette.nav(theme.dark)))
                .inner_margin(egui::Margin {
                    left: 0,
                    right: 0,
                    top: 4,
                    bottom: 0,
                });
            // Size the navigation to its longest label at the current font size.
            let body = egui::TextStyle::Body.resolve(&ctx.style());
            let longest = ctx.fonts_mut(|f| {
                NAV_GROUPS
                    .iter()
                    .flat_map(|g| g.1.iter())
                    .map(|(_, _, l)| f.layout_no_wrap(l.to_string(), body.clone(), Color32::WHITE).size().x)
                    .fold(0.0_f32, f32::max)
            });
            let need = (42.0 + longest + 44.0).ceil();
            let nav = if self.settings.nav_collapsed {
                egui::SidePanel::left("nav_icons").resizable(false).exact_width((body.size * 3.2).max(44.0))
            } else {
                egui::SidePanel::left("nav")
                    .resizable(true)
                    .default_width(need)
                    .width_range(need..=need + 160.0)
            };
            nav.frame(nav_frame).show_separator_line(false).show(ctx, |ui| {
                self.nav_panel(ui, &theme);
            });
            if self.settings.show_dice_panel {
                let resp = egui::SidePanel::right("dice")
                    .resizable(true)
                    .default_width(self.settings.dice_panel_width)
                    .width_range(240.0..=700.0)
                    .show(ctx, |ui| {
                        let doc = self.doc.as_mut().unwrap();
                        views::dice::panel(ui, &mut doc.c, &mut self.sh, &mut self.vs.dice, self.vs.combat.selected);
                    });
                let w = resp.response.rect.width();
                if (w - self.settings.dice_panel_width).abs() > 2.0 && !ctx.input(|i| i.pointer.any_down()) {
                    self.settings.dice_panel_width = w;
                }
            }
            egui::CentralPanel::default().show(ctx, |ui| {
                if self.view == View::Wiki {
                    self.ensure_vault();
                }
                let doc = self.doc.as_mut().unwrap();
                let sh = &mut self.sh;
                let vs = &mut self.vs;
                match self.view {
                    View::Home => views::home::show(ui, doc, sh),
                    View::Run => views::run::show(ui, doc, sh, &mut vs.run),
                    View::Bestiary => views::bestiary::show(ui, doc, sh, &mut vs.bestiary),
                    View::Cards => views::cards::show(ui, doc, sh, &mut vs.cards),
                    View::Plot => views::plot::show(ui, doc, sh, &mut vs.plot),
                    View::Maps => views::maps::show(ui, doc, sh, &mut vs.maps),
                    View::Sessions => views::sessions::show(ui, doc, sh, &mut vs.sessions),
                    View::Threads => views::threads::show(ui, doc, sh, &mut vs.threads),
                    View::Players => views::characters::show_players(ui, doc, sh, &mut vs.players),
                    View::Npcs => views::characters::show_npcs(ui, doc, sh, &mut vs.npcs),
                    View::Factions => views::factions::show(ui, doc, sh, &mut vs.factions),
                    View::Board => views::storyboard::show(ui, doc, sh, &mut vs.board),
                    View::Quests => views::quests::show(ui, doc, sh, &mut vs.quests),
                    View::Places => views::places::show(ui, doc, sh, &mut vs.places),
                    View::Items => views::items::show(ui, doc, sh, &mut vs.items),
                    View::Loot => views::loot::show(ui, doc, sh, &mut vs.loot),
                    View::Encounters => views::encounters::show(ui, doc, sh, &mut vs.encounters),
                    View::Combat => views::combat::show(ui, doc, sh, &mut vs.combat),
                    View::Wiki => views::wiki::show(ui, doc, sh, &mut vs.wiki),
                    View::Search => views::search::show(ui, doc, sh, &mut vs.search),
                    View::Settings => views::settings_view::show(ui, doc, sh, &mut vs.settings, &mut self.settings),
                }
            });
        }

        self.dialogs(ctx);
        self.image_window(ctx);
        views::transfer::window(ctx, self.doc.as_mut(), &mut self.sh, &mut self.vs.transfer);

        // Intercept clicks on [[wiki links]] rendered as `wiki:` URLs.
        let mut wiki_targets = Vec::new();
        ctx.output_mut(|o| {
            o.commands.retain(|cmd| {
                if let egui::OutputCommand::OpenUrl(u) = cmd {
                    if let Some(t) = u.url.strip_prefix("wiki:") {
                        wiki_targets.push(t.to_string());
                        return false;
                    }
                }
                true
            })
        });
        for t in wiki_targets {
            self.sh.actions.push(Action::WikiLink(t));
        }
        let had_actions = !self.sh.actions.is_empty();
        self.process_actions(ctx);

        // Change tracking and autosave.
        let now = self.sh.now;
        let autosave = self.settings.autosave_secs;
        let mut saved_msg = None;
        if let Some(doc) = &mut self.doc {
            if had_actions {
                doc.touched = true;
            }
            doc.track(false);
            if doc.touched {
                // input arrived inside the 0.75 s window: look again shortly
                ctx.request_repaint_after(std::time::Duration::from_millis(800));
            }
            if doc.dirty {
                // after a failed save, retry every 15 s rather than giving up
                let retry_ok = doc.save_error.is_none() || now - doc.last_save_attempt > 15.0;
                if autosave > 0 && now - doc.last_change >= autosave as f64 && retry_ok {
                    let had_error = doc.save_error.is_some();
                    match doc.save() {
                        Err(e) if !had_error => saved_msg = Some(format!("Autosave failed: {e} (will retry)")),
                        Ok(()) if had_error => saved_msg = Some("Saved".to_string()),
                        _ => {}
                    }
                }
                ctx.request_repaint_after(std::time::Duration::from_millis(800));
            }
        }
        if let Some(m) = saved_msg {
            self.sh.toast(m);
        }
        if self.sh.status.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }

        if views::wiki::quit_prompt(ctx, &mut self.sh, &mut self.vs.wiki) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.vs.wiki.has_unsaved().is_some() && self.doc.is_some() {
            // a wiki note is being edited: ask on the wiki page first
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.view = View::Wiki;
            self.vs.wiki.pending_quit = true;
        } else if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            self.settings.save();
            if !self.save_if_dirty() {
                let err = self.doc.as_ref().and_then(|d| d.save_error.clone()).unwrap_or_default();
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.dialog = Dialog::SaveFailedOnClose(err);
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if !self.allow_close {
            self.save_if_dirty();
        }
        self.settings.save();
    }
}

/// One navigation tab. The active tab takes the content panel's colour and an accent bar,
/// so it reads as "this page is open" rather than as another list item.
fn nav_tab(ui: &mut egui::Ui, icon: &str, label: &str, count: Option<String>, active: bool, collapsed: bool, theme: &crate::themes::Theme) -> egui::Response {
    use crate::themes::{c, mix};
    let pal = &theme.palette;
    let h = ui.spacing().interact_size.y + 5.0;
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    let painter = ui.painter();
    if active {
        painter.rect_filled(rect, 0.0, c(pal.panel));
        painter.rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(4.0, h)), 0.0, c(pal.accent));
    } else if resp.hovered() {
        painter.rect_filled(rect.shrink2(egui::vec2(4.0, 1.0)), 3.0, c(mix(pal.nav(theme.dark), pal.text, 0.08)));
    }
    let text_color = ui.visuals().text_color();
    let weak = ui.visuals().weak_text_color();
    let body = egui::TextStyle::Body.resolve(ui.style());
    let icon_font = egui::FontId::proportional(body.size * 1.05);
    let icon_color = if active { text_color } else { weak };
    let draw_icon = |center: egui::Pos2| {
        if icon == PLOT_ICON {
            ui_util::paint_plot_icon(painter, center, icon_font.size, icon_color);
        } else {
            painter.text(center, egui::Align2::CENTER_CENTER, icon, icon_font.clone(), icon_color);
        }
    };
    if collapsed {
        draw_icon(rect.center());
        return resp.on_hover_text(label);
    }
    draw_icon(egui::pos2(rect.min.x + 24.0, rect.center().y));
    let label_galley = {
        let mut job = egui::text::LayoutJob::simple_singleline(label.to_string(), body.clone(), text_color);
        job.wrap = egui::text::TextWrapping::truncate_at_width((w - 80.0).max(20.0));
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let label_pos = egui::pos2(rect.min.x + 42.0, rect.center().y - label_galley.size().y / 2.0);
    if active {
        // faux-bold: draw twice with a tiny offset (the built-in font has no bold weight)
        painter.galley(label_pos + egui::vec2(0.4, 0.0), label_galley.clone(), text_color);
    }
    painter.galley(label_pos, label_galley, text_color);
    if let Some(n) = count {
        painter.text(
            egui::pos2(rect.max.x - 12.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            n,
            egui::TextStyle::Small.resolve(ui.style()),
            weak,
        );
    }
    resp
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod ui_tests;
