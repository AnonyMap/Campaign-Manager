# Campaign Manager

A desktop campaign notebook for Dungeon Masters, written in Rust, for Linux, macOS and Windows. Plain, office-style
interface (think Microsoft Project, not a fantasy skin) with 15 built-in light and dark
themes, your own saved themes, and adjustable fonts.

*Working title — the name may change.*

## What it does (v0.4.0)

| Area | What you can do |
|---|---|
| **Run session** | The screen to keep open at the table. Picks the next planned session and shows the plan, the scenes in running order (tick them off, mark the one in play), encounters with Run buttons, the secrets tied to today's scenes with Reveal buttons, the people involved, the party's AC/HP/passive Perception, a quick-idea box and live notes. **End session** marks it played, drafts the recap from your notes, the scenes you played and the secrets you revealed, and can move unplayed cards to the next session. |
| **Story cards** | Jot plot ideas. Nothing is required — a card can be one line of Markdown. Optional title, status, colour, plot threads, timing, map pins, session plans, tags, and links to NPCs, PCs, factions, encounters, other cards, wiki pages, URLs and images. `[[Name]]` in the text links to any card/NPC/wiki page. Shows "Referenced by" backlinks. Mark a card **secret** (🔒) for things the players don't know yet; reveal it later and it records which session they found out in. Filter by secret / not yet revealed. |
| **Timing** | Calendars start weeks on Sunday and show US-style dates by default (Settings → Appearance → Dates switches to Monday / international / ISO). Per card: unscheduled, real-world date, in-world (custom calendar) date, or session number. A play schedule (first date + interval + skipped dates) converts between session numbers and real dates. |
| **Story board** | A plain table top for story cards: put cards on it (one at a time or all at once), drag them around, and drag from a card's ● handle onto another card to link them. Lines show links (two-way ones once, with arrows both ways); right-click a line to remove it. Select a card to list everything it links to and everything that links to it, and click through. Zoom, Fit, Tidy (rows by plot thread), dim finished cards. |
| **Quests** | Tick “This is a quest” on any story card (or create one in Quests). Track status (available, active, completed, failed, abandoned), who gave it (an NPC, faction, place or PC), objectives to tick off, rewards (coins, items, XP, anything else) and the sessions it was taken and finished in. The quest log groups quests by status and filters by giver; **Hand out rewards** splits the coins between the party and gives each item to someone. Active quests and their objectives show on the Run session screen; NPC, faction and place pages list the quests they've given. |
| **Ideas pile** | Cards stay in the pile until you assign them (or untick it). Placing a card on a map doesn't remove it from the pile — it's "and/or". |
| **Plot line** | Grid of plot threads × sessions (or real/in-world months). Drag cards between cells to change thread or target session. "+ card" in any cell. |
| **Places** | Towns, inns, shops, temples, dungeons — any kind you type. Places nest (a shop *in* a town) and list their NPCs, factions and related places. 🎲 **Name** suggests names for the kind of place (inns, shops by trade, temples, dungeons, keeps, guilds, wilds, towns); add a name pool in Settings named after a kind (e.g. “Inn”) to mix in your own. Any place can become a **story card** for the plot line and story board. Shops get **generated wares** by theme (general store, blacksmith, alchemist/potions, magic, scribe, fletcher, jeweler, temple, tavern menu) and quality (poor / average / fine), with prices you can edit and a **Sell to…** button that takes the price from the character's purse. |
| **Maps** | Upload a map image (browse, paste a path, or drag the file onto the window). Pin cards, drag pins, scroll-zoom, pan. |
| **Sessions** | Session planner: narrative (Markdown), running order of cards, encounters (with Run buttons), recap, links. |
| **Plot threads** | Storylines with status (open / dormant / resolved / abandoned) and progress. |
| **Player characters** | Player name, character name, portrait, class/level, AC, HP, initiative mod, ability scores, passive Perception, custom rolls, character-sheet images, Markdown notes, extra fields. |
| **NPCs & monsters** | Same as PCs plus kind, factions, role, location, status, stat-block images and a **name generator** (10 species pools, 10 backgrounds, invented names, your own pools; 12,500+ combinations). Assign a **stat block** from the Bestiary: blank AC/HP/initiative/CR fall back to it, filled-in values override it. |
| **Bestiary** | A pool of stat blocks, separate from NPCs (many NPCs can share one). Rendered like a printed stat block with roll buttons for every attack, damage roll, ability check and initiative. Edit everything, including structured attacks/saves/damage. Includes all **331 SRD 5.2 monsters** to browse and copy in. **Import** stat blocks from any book: give an AI the PDF plus the built-in instructions (Import → *Copy AI instructions*, also in `docs/statblock-import-spec.md`), paste its reply back, check the preview and warnings, import. Imports go into their own **collection** (e.g. "Essential NPCs"), so a whole book stays apart from your homebrew; filter by collection, rename it, export just that collection, or delete it. **Select** mode ticks many stat blocks at once (shift-click for a range) to move them to another collection or delete them. Export your bestiary to share it. |
| **Factions** | Description, goals, attitude to the party, emblem, member NPCs, the places they're present at and the quests they've given. |
| **Items** | Your item library plus all **739 SRD 5.2 items** (equipment and magic items) to browse and copy in. Each item has type, rarity, price, weight, attunement, damage/AC, description, **rolls** (found automatically: weapon damage with its bonus, healing, extra damage) and **bonuses** (+1 to attack and damage, +2 AC…). **Give to…** puts an item in a character's inventory, optionally adding its rolls to their custom rolls; **Add rolls to…** adds just the rolls. **Import** items from any book the same way as stat blocks (AI instructions in `docs/item-import-spec.md`), into their own collection; export, bulk select and delete. |
| **Inventory** | PCs and NPCs carry coins (pp/gp/ep/sp/cp) and items with quantity, equipped and attuned (max 3). Bonuses from equipped items are summed up; 🎲+ adds an item's rolls to the character. |
| **Loot generator** | Procedural treasure, no AI: **for an encounter** (each enemy's pocket change by CR, or one treasure hoard by the toughest enemy — from a premade encounter or the fight in the combat tracker), **by challenge** rating, or **by value** (“about 500 gp”, with or without magic items up to a rarity, gems and art). Coins, gems and art objects from built-in tables, magic items from the SRD list and your own. Reroll or remove any line, add items or coins, then hand it out to the party, save it as a story card, add it to the encounter's notes, or copy it as text. **💰 Loot** buttons in Encounters and the Combat tracker open it for that fight. |
| **Encounter builder** | Premade encounters from PCs, NPCs, stat blocks (with counts — "Goblin Warrior ×4") and one-off creatures; mark entries as allies (left out of the difficulty) and choose whether copies roll initiative together. Difficulty estimate using the 2024 DMG XP budgets. |
| **Combat tracker** | Every row shows the creature's **token** (its portrait, or its stat block's; initials otherwise) with a coloured border: blue for PCs, green for allies, red for enemies. **Add creatures…** adds NPCs, stat blocks/SRD monsters or one-offs several at a time, as enemies or **allies**, with initiative rolled (separately or **together** — one roll for the group, shown by a coloured bar), typed in, or left blank. **Drag** the dotted handle to put anyone anywhere in the turn order, with or without a roll: the app sets their initiative from the new neighbours. **Summons** (right-click a name, or "Add summon…") sit indented under their summoner and act right after them; they stay if the summoner drops out. Auto-roll NPC initiative (uses the assigned initiative modifier, else DEX mod, and says which it used), auto-sorted turn order. **Turns and rounds**: after ▶ Start combat, a turn bar shows the round, “turn 3 of 8”, game time elapsed, who's acting now (with their conditions, or a death-save reminder) and who's up next; **Next ▶** / **◀ Back** (or the N and B keys) move through the order, passing over downed monsters, highlighting and scrolling to the current row. Going back past the top of a round returns to the previous round and puts back any condition timers the new round had counted down. HP with a calculator (type an amount like `5+6` or `2d6`, see a live preview, then click −Dmg or +Heal; Enter = damage, a leading `+` heals), "Bloodied" (≤ 50 %) and "Down" indicators, undo for each HP change, temp HP, the 15 conditions of the 2024 rules with their effects on hover, custom statuses, concentration-save reminders. **Group rolls**: tick several combatants, type one amount (e.g. `8d6`), and each takes full, half, double or none (click their chip to cycle) — one roll, one undo. **Death saves** for PCs at 0 HP: click the pips or roll (10+ succeeds, 1 = two failures, 20 = back up with 1 HP); damage while down adds a failure, healing clears them. Selecting a combatant shows their custom rolls, stats, the full stat block with roll buttons, sheet images and notes. Edits to max HP, AC, initiative modifier and custom rolls are saved to the PC/NPC record. Combat in progress is saved with the campaign. |
| **Dice roller** | Always-available side panel (Ctrl+D), 5etools-style syntax (below), history with reroll, colour-coded dice, macros, and **save any roll to a character** — either with `@Name` in the roll or the "Save to…" button on any history entry. |
| **Campaign wiki** | Point at an Obsidian vault: folder tree, tag list, full-text search (`tag:x`, `path:y`), rendered notes with working `[[links]]` and aliases, backlinks, edit & save (warns before throwing away unsaved edits, and before overwriting a note changed in Obsidian meanwhile), create notes, open in Obsidian. Export cards (with quest details), NPCs and PCs (with inventory), factions, encounters and places (with shop stock) as notes (re-exporting updates the same note). A vault inside the campaign folder is stored as a relative path, so the campaign can move. |
| **Import / export** | **File › Import / export** (also on the Overview): **New campaign from this one** — same world, new story; **Export** all or part of a campaign to a `.cmpack` file; **Import** all or part of a package, or straight from another campaign's folder, into this campaign; **New campaign from a file**. Pick parts with checkboxes (story & quests, sessions, encounters, PCs, NPCs, factions, places & shops, items, bestiary, maps, calendar/name pools/conditions/macros) or a preset: *Everything*, *Campaign setting* (the world without the story, encounters or PCs), *Reference library* (items, bestiary, rules), *The party*. Images come along. Things that came from the same campaign are recognised: keep yours, replace them, or keep both. Links to things you leave out are dropped. An import can be undone. Dropping a `.cmpack` file on the window opens the import. |
| **Everywhere** | Global search (Ctrl+F) across cards, quests, people, places, items and stat blocks; undo/redo of any change (Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y when not typing; bursts of typing are one step, dice rolls aren't undone); autosave that retries if the disk or a sync app blocks it, and asks before quitting with unsaved changes; drag-and-drop images. |

### Dice syntax

```
Basic:        1d20+5   d20   2d6+1d4+3   d%   (1+1)d6   2d3d4 (chained)
Keep/drop:    4d6kh3   4d6dl1   3d4dh1   3d4kl1   2d20kh1 (advantage)
Reroll once:  2d4r1   2d4r<2   2d4r<=2   2d4r>2   2d4r>=3
Explode:      2d4x4   2d4x<2   2d4x<=2   2d4x>2   2d4x>=3
Successes:    2d4cs=4   2d4cs<2   2d4cs<=2   2d4cs>2   2d4cs>=3
Margin:       2d4ms=4
Pools:        {2d8, 1d6}   {1d20+7, 10}kh1
Functions:    floor ceil round trunc abs sign sqrt cbrt exp log pow min max random avg dmax dmin
Labels:       Fireball: 8d6
Save to NPC:  Bite: 1d6+2 @Goblin
Macros:       /macro add sneak 3d6   #sneak   /macro list   /macro remove sneak
Repeat:       /iterroll 1d20+5 10 [target]
Other:        /clear  /help   ↑/↓ recalls earlier input
```

Semantics follow 5etools: rerolls happen once, explosions chain (capped at 999 extra dice),
`cs` turns the total into a success count, `ms` into a margin.

## Your data

A campaign is a **folder**:

```
My Campaign/
  campaign.json       everything except images — pretty-printed UTF-8 JSON
  campaign.json.bak   the previous save
  assets/             images you added (maps, portraits, sheets)
  wiki/               (optional) an Obsidian vault, if you create one here
```

A `.cmpack` export is a zip file holding a `campaign.json` with the parts you chose and the
`assets/` images they use — you can open it with any unzip tool.

Paths inside the JSON are relative with forward slashes, so the folder works unchanged on
Linux, macOS and Windows — copy it, sync it with Dropbox/OneDrive, or put it in Git. Files
from older versions keep loading as fields are added. Appearance settings live in your OS
config folder, separate from campaigns.

## Building

Requires Rust 1.88+.

```sh
cargo test --workspace                     # 110 tests
cargo run --release -p campaign-manager    # run the app
cargo run -p cm_core --example make_sample -- "/tmp/Sample"   # write the sample campaign
```

Linux builds need no system libraries beyond a normal desktop (X11 or Wayland, OpenGL). The
file dialog uses the XDG desktop portal; if your desktop has none, use the "paste a path"
fields or drag-and-drop.

### Release packages

| Package | How |
|---|---|
| Linux AppImage | `packaging/build-appimage.sh` (uses cargo-zigbuild to target glibc 2.28 if installed) |
| macOS .dmg | `packaging/build-macos.sh` on a Mac (universal Intel + Apple Silicon, ad-hoc signed) |
| Windows .zip (portable) | `packaging\build-windows.ps1` on a Windows PC (Rust + Visual Studio C++ Build Tools), or `packaging/build-windows-cross.sh` from Linux (`rustup target add x86_64-pc-windows-gnu` + mingw-w64) |
| Source tarball | `packaging/make-source-tar.sh` |

`.github/workflows/release.yml` builds the AppImage, the .dmg, the Windows .zip and the source
tarball on GitHub's Linux, macOS and Windows runners;
pushing a `v*` tag publishes them as a release.

The macOS build is ad-hoc signed, not notarized: the first time, right-click the app → Open.
The Windows build is a portable folder (unzip, run `Campaign Manager.exe`); it isn't code-signed,
so SmartScreen may warn the first time — click "More info" → "Run anyway".

## Layout

```
crates/cm_core   UI-independent library: model, storage, dice, encounters, conditions,
                 names, calendars, bestiary/import, items/import, loot & shops, places,
                 Obsidian vault, search (all unit tested)
crates/cm_core/data   bundled SRD 5.2 monsters and items (import format)
docs/            stat block and item import specs for AI conversion + JSON Schema
crates/cm_app    egui desktop app (views/ has one file per section)
packaging/       build scripts, .desktop file
```

### Stat block import format

`docs/statblock-import-spec.md` is written for an AI model (and people): rules, field reference,
a worked example and a checklist. `docs/statblock.schema.json` is the matching JSON Schema. The
importer is lenient (it accepts chat text around the JSON, code fences, `"hp": "27 (5d8 + 5)"`,
trailing commas…) and lists anything it couldn't read before you import.
`tools/convert_open5e_srd.py` regenerates the bundled SRD list from Open5e's data.

### Item import format

`docs/item-import-spec.md` does the same for items (weapons, armour, potions, wondrous items,
gear…): name, type, rarity, cost, weight, attunement, damage or AC, description, and optional
`rolls` and `modifiers`. Missing rolls and bonuses are worked out from the text when imported.
`tools/convert_open5e_items.py` regenerates the bundled SRD item list.

## Troubleshooting

- If the app ever crashes, details are written to `crash.log` in the settings folder
  (`~/.config/campaignmanager/` on Linux, `~/Library/Application Support/org.CampaignManager.Campaign-Manager/` on macOS, `%APPDATA%\CampaignManager\Campaign Manager\config\` on Windows) — please send it in.
- File dialogs use your desktop's XDG portal, falling back to `zenity`; if neither is available
  every picker also accepts a pasted path or a dragged file.

## Known limitations / next iterations

- No multi-window: the dice roller is a side panel, not a separate window.
- Wiki editing is a plain Markdown text box (no live preview while editing).
- Maps hold card pins only (no free-form markers or fog of war yet).
- Not yet: a player-facing initiative window, two-way Obsidian sync.
- Loot and shop generation draw from the SRD list and your own items; there's no weighting by
  setting or region yet.

## Credits

Condition summaries paraphrase the 2024 rules glossary, and the bundled monsters come from the
System Reference Document 5.2. This work includes material from the System Reference Document 5.2
("SRD 5.2") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2 is
licensed under the Creative Commons Attribution 4.0 International License, available at
https://creativecommons.org/licenses/by/4.0/legalcode. Monster and item data structured by the
[Open5e](https://open5e.com) project.

License: MIT.
