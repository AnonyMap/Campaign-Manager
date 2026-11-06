# Campaign Manager

A desktop notebook for Dungeon Masters running D&D 5e (2024 rules). Plan the story, keep
track of the world, and run sessions and combat from one place. It runs on Windows, macOS
and Linux, works entirely offline, and keeps your campaign in an ordinary folder on your
own computer.

The look is deliberately plain and office-like (closer to a spreadsheet than a fantasy
game), with light and dark themes, your own color themes, and adjustable text size.

*Campaign Manager is a working title.*

---

## Install

Download the latest version from the
**[Releases page](https://github.com/AnonyMap/Campaign-Manager/releases)**.

| System | Download | First run |
|---|---|---|
| **Windows** (10/11, 64-bit) | `…-windows-x64.zip` | Unzip anywhere and run `Campaign Manager.exe`. Nothing is installed. The app isn't code-signed, so Windows SmartScreen may warn you the first time: click **More info → Run anyway**. |
| **macOS** (Intel and Apple Silicon) | `…-macos-universal.dmg` | Open the .dmg and drag the app to Applications. The app isn't notarized by Apple, so macOS blocks it the first time. Go to **System Settings → Privacy & Security** and click **Open Anyway** (on older macOS: right-click the app → **Open**). |
| **Linux** (x86-64) | `….AppImage` | Make it executable (`chmod +x Campaign_Manager-*.AppImage`) and run it. It needs no extra libraries on a normal desktop. If your system lacks FUSE, run it with `--appimage-extract-and-run`. |

To build it yourself instead, see [Building from source](#building-from-source).

---

## What it does

Campaign Manager brings together the things a DM usually keeps in separate notebooks, apps
and browser tabs:

- **The story:** plot ideas, quests, secrets and plot threads, planned session by session.
- **The world:** places, shops, NPCs, factions, maps, and an Obsidian wiki if you use one.
- **The rules you need at the table:** a bestiary with every SRD 5.2 monster, an item
  library with every SRD 5.2 item, a loot generator and a dice roller.
- **The game itself:** a "Run session" screen to keep open while you play, and a combat
  tracker that handles initiative, turns, HP, conditions and death saves.

---

## How it works

### The idea

Everything in a campaign is a **record**: a story card, a session, an NPC, a place, an item,
a stat block, an encounter. Records **link** to each other. A card can mention an NPC, an
NPC can live in a place, a place can be on a map, a session can plan which cards and
encounters it will use. Each record shows everything that links to it, so you can always
follow a thread from one thing to the next.

### A typical flow

This is one way to use it. You don't have to (see below).

1. **Jot ideas as story cards.** A card can be a single line: *"What if the miller faked
   his own disappearance?"* New cards wait in the ideas pile.
2. **Shape the story.** Give cards a plot thread, link them to the NPCs and places involved,
   arrange them on the story board, mark some as **quests** or **secrets**.
3. **Plan a session.** Drag cards into a session's running order and add the encounters
   you expect to run.
4. **Run it at the table.** The **Run session** screen shows tonight's scenes in order, the
   encounters with Run buttons, the secrets you might reveal, the people involved and the
   party's key numbers. Fights go to the **combat tracker**.
5. **End the session.** The app marks it played, drafts a recap from your notes, the scenes
   you played and the secrets you revealed, and can move unplayed scenes to next time.

Meanwhile the world fills in around the story: NPCs with stat blocks, towns with shops and
generated wares, factions, maps with your cards pinned on them.

### You don't have to use it that way

The app is deliberately lenient. It suggests a structure but never enforces one.

- **Almost every field is optional.** An NPC can be just a name. A card can be one line
  with no title, thread or date.
- **Use only the parts you want.** Plenty of DMs will only want the combat tracker, or the
  bestiary and dice roller, or a place to keep notes. The rest stays out of the way.
- **There's no required order.** Run a combat with creatures you add on the spot, start a
  session with nothing planned, or put a card in a session without giving it a thread.
- **Write the way you like.** Notes are formatted text you can type into like a word
  processor. Underneath it's plain Markdown, and you can switch to editing that directly.
- **Imports forgive mess.** Pasted stat blocks and items can have chat text around them,
  odd formatting or missing fields. You get a preview with warnings before anything is
  added.
- **Mistakes are cheap.** Almost every change can be undone (Ctrl/⌘ Z), the app autosaves,
  and the previous save is always kept as a backup.

---

## Features

### Story and planning

- **Story cards.** Ideas, scenes, rumors, reveals. Each can have a status, color, plot
  threads, timing, tags, links, images and map pins, all optional. Writing `[[Name]]` in
  the text links to any card, NPC or wiki page. Mark a card **secret** for things the
  players don't know yet; revealing it records the session they found out.
- **Quests.** Any card can be a quest, with a giver, objectives to check off, rewards and
  status. The quest log groups them by status, and **Hand out rewards** shares coins and
  items with the party.
- **Story board.** A table top for your cards: drag them around and draw links between them.
- **Plot line.** A grid of plot threads × sessions (or months). Drag cards to reschedule them.
- **Sessions.** A planner for each session: notes, running order, encounters, recap.
- **Timing and calendars.** Date cards by session number, by real date or by an in-world
  date on your own calendar. A play schedule (say, every other Friday) converts between
  session numbers and real dates.

### The world

- **NPCs and player characters.** Stats, portraits, notes, custom rolls, inventory and coins.
  NPCs can use a **stat block** from the bestiary. Their own numbers override it, and blank
  ones are filled from it. An NPC can have **several stat blocks** for different forms: a
  druid's wild shape, or a villain's second phase. A **name generator** covers many species
  and backgrounds, and you can add your own name lists.
- **Places.** Towns, inns, temples, dungeons, nested inside each other (a shop *in* a town).
  Shops get **generated wares** by type and quality, with prices you can edit and a **Sell
  to…** button that takes the coins from a character's purse.
- **Factions.** Goals, attitude to the party, members, and where they're present.
- **Maps.** Any image, of any size. Pin cards to it, pan and zoom.
- **Campaign wiki.** Point the app at an Obsidian vault to browse, search, edit and link
  notes. You can also export cards, NPCs, places and more as notes. Text you add below the
  marker line in an exported note is kept when you export again.

### Rules and reference

- **Bestiary.** All 331 SRD 5.2 monsters plus your own, shown like a printed stat block,
  with a roll button for every attack, save and check. **Import** stat blocks from any
  book: give an AI chatbot the page together with the instructions the app provides
  (`docs/statblock-import-spec.md`), paste its reply back, and check the preview. Imports
  go into collections, so a whole book stays separate from your homebrew.
- **Items.** All 739 SRD 5.2 items plus your own, with rolls and bonuses (+1 weapons, AC)
  worked out automatically. Imports work the same way as for stat blocks
  (`docs/item-import-spec.md`).
- **Inventory.** Coins and items for every character, with equipped and attuned items, and
  item rolls added to the character's custom rolls.
- **Loot generator.** Treasure for an encounter, a challenge rating or a target value, made
  from built-in coin, gem and art tables plus magic items. No AI is involved. Reroll lines,
  then hand the loot out, save it as a card or copy it as text.
- **Dice roller.** A side panel that's always available. It uses 5etools-style notation
  (advantage, keep/drop, rerolls, exploding dice, pools), keeps a history and macros, and
  can save any roll to a character.

### Combat

- **Encounter builder.** Plan fights with PCs, NPCs, monsters ("Goblin ×4") and allies,
  with a difficulty estimate from the 2024 DMG XP budgets.
- **Combat tracker.**
  - **Setup:** add creatures several at a time, as enemies or allies. Roll initiative
    separately or as a group, type it in, or drag anyone into place.
  - **Turns:** a turn bar tracks the round, whose turn it is and who's next. **Next** and
    **Back** step through the order (Back exactly undoes Next), and in-game time is counted
    for you.
  - **HP:** a calculator (`2d6+3`, or `+5` to heal), group damage with full, half, double
    or none per target, temporary HP, and undo for every change.
  - **Conditions:** the 15 conditions of the 2024 rules, with their effects shown on hover,
    plus timers and concentration reminders.
  - **Death saves:** handled following the 2024 rules.
  - **Summons and forms:** summons act right after their summoner, and NPCs can switch form
    mid-fight.
  - **Saving:** a fight in progress is saved with the campaign.

### Everywhere

- **Go to…** (Ctrl/⌘ P): jump to any card, NPC, place, item or screen by typing part of
  its name. **Search** (Ctrl/⌘ F) looks through the whole campaign and wiki.
- **Back and forward** (Ctrl/⌘ [ and ]): move between the screens and records you opened,
  like in a browser.
- **Formatted writing.** Bold looks bold, lists continue when you press Enter, and checkboxes
  can be clicked. Underneath it's Markdown.
- **Import and export.** Start a new campaign from an existing one ("same world, new
  story"). You can also share all or part of a campaign as a `.cmpack` file, or import
  parts of another campaign.
- **Keyboard shortcuts** for nearly everything (below). On a Mac they use ⌘.
- **Autosave and undo**, with a backup of the previous save.

<details>
<summary><b>Keyboard shortcuts</b></summary>

On a Mac, use **⌘ Command** wherever this says Ctrl. The app shows the right key in its
menus, and **Ctrl/⌘ /** lists every shortcut.

| Keys | Does |
|---|---|
| Ctrl P | Go to… anything by name |
| Ctrl F | Search |
| Ctrl [ / Ctrl ] (or mouse back/forward) | Back / forward |
| Ctrl 1 … 9 | Overview, Run session, Story cards, Story board, NPCs, Places, Encounters, Combat, Wiki |
| Ctrl N / Ctrl Shift N | New story card / new NPC |
| Ctrl S, Ctrl Z, Ctrl Shift Z | Save, undo, redo |
| Ctrl D | Dice roller |
| Ctrl = / Ctrl − / Ctrl 0 | Larger / smaller / normal text |
| ↑ ↓ Home End | Move through the list on screen |
| Esc | Close the window on top |
| **While writing:** Ctrl B / I / E / K, Ctrl Shift X | Bold, italic, code, [[link]], strikethrough |
| **While writing:** Ctrl Shift 7 / 8 / 9 | Numbered, bulleted, check list |
| **While writing:** Ctrl Alt 1 / 2 / 3 / 0 | Heading 1–3, normal text (Alt is ⌥ Option on a Mac) |
| **In combat:** N or →, B or ←, ↑ ↓, R, A | Next turn, back, select, roll NPC initiative, add creatures |

</details>

<details>
<summary><b>Dice notation</b></summary>

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

Rerolls happen once, explosions chain (up to 999 extra dice), `cs` counts successes and
`ms` gives the margin, as in 5etools.

</details>

---

## Your data

A campaign is a folder you can copy, back up, sync with Dropbox or OneDrive, or put in Git:

```
My Campaign/
  campaign.json       everything except images, as readable JSON
  campaign.json.bak   the previous save
  assets/             images you added (maps, portraits, character sheets)
```

There's no account, no server and no tracking. The app never connects to the internet; web
links you add open in your browser. Files from older versions keep opening in newer ones.
App settings (theme, fonts, recent campaigns) are kept in your system's settings folder,
apart from your campaigns.

---

## Development

### How it's built

- **Language and UI:** written in [Rust](https://www.rust-lang.org/) with the
  [egui](https://github.com/emilk/egui) interface library. It compiles to a single native
  program per platform with no runtime to install.
- **Two parts:**
  - `crates/cm_core` holds all the logic (campaign data, dice, combat rules, calendars,
    import/export, loot, the wiki) and doesn't depend on the interface.
  - `crates/cm_app` is the desktop app, with one file per screen in `views/`.
- **Testing:** 179 automated tests, of three kinds:
  - **Unit tests** for the rules and data handling.
  - **Randomized ("property") tests.** These throw thousands of generated inputs at the
    dice roller, importers, editor, calendars, combat tracker and import/export, checking
    that nothing crashes or loses data, and that Back always undoes Next.
  - **Headless app tests.** These drive the real app without a screen, including a
    "monkey" that clicks, types and presses shortcuts at random on every screen.
- **Bug hunting:** besides the tests, there have been several rounds of independent code
  review, each followed by fixes and a regression test for every bug found.
- **Continuous builds:** every change is built and tested on Linux, macOS and Windows by
  GitHub Actions (`.github/workflows/release.yml`). Tagging a version publishes the
  downloads on the Releases page.
- **Open data:** the SRD monsters and items are bundled as JSON in `crates/cm_core/data`.
  The scripts in `tools/` regenerate them from [Open5e](https://open5e.com).

### Building from source

You need Rust 1.88 or newer ([rustup.rs](https://rustup.rs)).

```sh
cargo run --release -p campaign-manager    # build and run the app
cargo test --workspace                     # run the tests
```

On Linux, nothing beyond a normal desktop (X11 or Wayland, OpenGL) is needed. To build the
release packages, use the scripts in `packaging/`:

- `build-appimage.sh` for Linux.
- `build-macos.sh` on a Mac, for a universal .dmg.
- `build-windows.ps1` on Windows, or `build-windows-cross.sh` from Linux.
- `make-source-tar.sh` for a source archive.

Longer test runs:

```sh
PROPTEST_CASES=20000 cargo test -p cm_core                            # more random inputs
MONKEY_SEED=3 MONKEY_STEPS=5000 cargo test -p campaign-manager monkey  # a longer monkey run
cargo test --release -p campaign-manager big_campaign -- --ignored --nocapture   # speed check
```

---

## Built with AI

**This app was coded with AI.** The code was written by Claude, an AI model made by
Anthropic. The project's author directed the work: what to build, how it should behave,
play-testing, and feedback on every version. The code reviews mentioned above were also
done by AI, alongside the automated tests.

The code is open and auditable. Every line of it is in this repository under the MIT
license, along with the tests and the build pipeline that produces the downloads. Anyone
can read it, check it, build it themselves, change it or reuse it. If you find a bug or
something that looks wrong, please open an issue.

---

## Troubleshooting

- **If the app crashes,** details are written to `crash.log` in the settings folder. Please
  include it when you report the problem. The settings folder is:
  - Linux: `~/.config/campaignmanager/`
  - macOS: `~/Library/Application Support/org.CampaignManager.Campaign-Manager/`
  - Windows: `%APPDATA%\CampaignManager\Campaign Manager\config\`
- **If a file picker doesn't open** (some Linux desktops lack one), every picker also
  accepts a pasted path or a file dragged onto the window.

## Known limitations

- There's no player-facing initiative display yet, and no two-way sync with Obsidian.
- Maps hold card pins only (no free markers or fog of war).
- The dice roller is a side panel, not a separate window.
- Loot and shop stock aren't weighted by region or setting.

---

## Credits and license

This work includes material from the System Reference Document 5.2 ("SRD 5.2") by Wizards
of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2 is licensed under
the Creative Commons Attribution 4.0 International License, available at
https://creativecommons.org/licenses/by/4.0/legalcode. The monster and item data was
structured by the [Open5e](https://open5e.com) project. Condition summaries paraphrase the
2024 rules glossary.

The bold font in formatted text is Ubuntu Bold, under the Ubuntu Font Licence
(`crates/cm_app/assets/UFL.txt`).

Campaign Manager is released under the [MIT License](LICENSE).
