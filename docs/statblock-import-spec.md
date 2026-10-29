# Campaign Manager stat block import format (v1)

This document tells an AI model (or a person) how to turn creature stat blocks from a
PDF, a screenshot, a web page or typed notes into a file that Campaign Manager can import
into its **Bestiary** (the campaign's pool of stat blocks). Stat blocks are templates:
in the app you can assign one to any number of NPCs and add them to encounters.

## How to use it (for the person)

1. In Campaign Manager open **Bestiary → Import…** and click **Copy AI instructions**
   (or attach this file to the chat).
2. Give the AI your source: attach the PDF or images, or paste the text. Then say e.g.
   *"Convert every stat block in the attached file using these instructions."*
3. Copy the AI's whole reply, go back to **Bestiary → Import…**, paste it into the box and
   click **Read**. (Or save the reply as a `.json` file and use **Choose file…**.)
4. Check the preview and any warnings, untick anything you don't want, and click **Import**.
   They go into their own **collection** (named after the file's `source`, e.g. "Essential
   NPCs"; you can rename it in the import window), so a whole book stays separate from your own
   stat blocks. In the Bestiary, filter by collection, and use **Select** to move or delete many
   at once.

Long books: ask for one chapter or about 15 stat blocks per reply. Each reply must be a
complete file on its own; import them one after another.

---

## Instructions for the AI

You are converting tabletop RPG (D&D 5th edition, 2014 or 2024 rules) creature stat
blocks into JSON for import. Follow these rules exactly.

### Output rules

1. Reply with **one JSON document inside a single ```json code block**. Nothing else is
   required; a one-line note before the block is fine. No comments inside the JSON.
2. The document is an object with `"format": "campaign-manager-statblocks"`,
   `"version": 1`, a `"source"` naming the book or file, and a `"statblocks"` array.
3. Make **one entry per stat block** in the source, in the order they appear. Include
   every stat block, not just the first few. If you must stop because the reply is too long,
   close the JSON properly, then say after the code block which creature to continue from.
4. **Copy, don't invent.** Use the numbers and wording printed in the source. If the source
   doesn't give a value, leave the field out. Never guess AC, HP, scores or bonuses.
5. Keep rules text (`text` of traits and actions) as printed, as plain text. Remove page
   furniture (headers, footers, page numbers) and rejoin words split across lines with a
   hyphen. Use plain `+`/`-` for modifiers.
6. When a stat block says it uses another creature's statistics with changes (for example
   "uses the Veteran stat block, except…"), and that other stat block is in the source,
   write out the complete, changed stat block. Otherwise put the instruction in `notes`.
7. Named characters with their own stat block (common in NPC books): use the character's
   name as `name`, put their role, personality and plot hooks in `notes` (short markdown),
   and add useful `tags` such as the faction or location.

### Stat block fields

| Field | Type | Example | Notes |
|---|---|---|---|
| `name` | string | `"Harbor Smuggler"` | **Required.** |
| `size` | string | `"Medium"` | Tiny, Small, Medium, Large, Huge, Gargantuan, or e.g. `"Medium or Small"`. |
| `type` | string | `"Humanoid (Human)"` | Creature type with any tag in parentheses. |
| `alignment` | string | `"Neutral Evil"` | |
| `ac` | integer | `14` | The number only. |
| `ac_note` | string | `"leather armor"` | The text in parentheses after AC, if any. |
| `hp` | integer | `27` | The average shown. |
| `hp_formula` | string | `"5d8+5"` | Hit dice as dice notation, no spaces. |
| `speed` | string | `"30 ft., Swim 30 ft."` | As printed. |
| `initiative` | integer | `2` | Initiative **modifier** (2024 blocks print it, e.g. "Initiative +2 (12)" → `2`). Leave out if not printed. |
| `abilities` | object | `{"str": 10, "dex": 15, "con": 12, "int": 11, "wis": 13, "cha": 14}` | Scores, not modifiers. |
| `saves` | string | `"Dex +4, Wis +3"` | Saving throw bonuses as printed. |
| `skills` | string | `"Deception +4, Stealth +6"` | |
| `damage_vulnerabilities` | string | `"Fire"` | |
| `damage_resistances` | string | `"Cold; Bludgeoning from nonmagical attacks"` | |
| `damage_immunities` | string | `"Poison"` | 2024 blocks list damage and condition immunities together: split them. |
| `condition_immunities` | string | `"Charmed, Poisoned"` | |
| `senses` | string | `"Darkvision 60 ft."` | Without passive Perception. |
| `passive_perception` | integer | `13` | |
| `languages` | string | `"Common, Thieves' Cant"` | |
| `cr` | string | `"1/2"` | Challenge rating as printed: `"0"`, `"1/8"`, `"1/4"`, `"1/2"`, `"1"`… `"30"`. |
| `proficiency_bonus` | integer | `2` | Only if printed. |
| `traits` | array of features | | Passive abilities listed before Actions. |
| `actions` | array of features | | |
| `bonus_actions` | array of features | | |
| `reactions` | array of features | | |
| `legendary_actions` | array of features | | |
| `legendary_intro` | string | `"Legendary Action Uses: 3 (4 in Lair)…"` | The paragraph before the legendary actions. |
| `notes` | string | `"Smiling dock boss. **Hook:** …"` | Markdown. Description, tactics, lore, or the "uses X stats" instruction from rule 6. |
| `tags` | array of strings | `["smugglers", "port town"]` | Optional, for filtering. |
| `source` | string | `"Essential NPCs p. 42"` | Book/file and page if known. Defaults to the document's `source`. |
| `collection` | string | `"Villains"` | Optional. Files this stat block in a different Bestiary collection than the document's `source`. Usually leave it out. |

### Feature objects (traits, actions, reactions…)

| Field | Type | Example | Notes |
|---|---|---|---|
| `name` | string | `"Cutlass"` | **Required.** Without the usage note. |
| `usage` | string | `"Recharge 5–6"` | `"3/Day"`, `"Recharge 5–6"`, `"1/Day each"`, `"Costs 2 Actions"`… Leave out if none. |
| `text` | string | `"Melee Attack Roll: +4, reach 5 ft. Hit: 6 (1d8 + 2) Slashing damage."` | Full text as printed. |
| `attack` | object | `{"bonus": 4, "kind": "melee", "range": "reach 5 ft."}` | **Only for attack rolls.** `bonus` is the number after "Attack Roll:" or "to hit". `kind` is `"melee"`, `"ranged"` or `"melee or ranged"`. |
| `save` | object | `{"ability": "DEX", "dc": 13}` | **Only when the feature forces a saving throw.** `ability` is one of STR, DEX, CON, INT, WIS, CHA. |
| `damage` | array | `[{"dice": "1d8+2", "type": "slashing"}]` | One entry per damage type, using the **dice in parentheses**, not the average: "6 (1d8 + 2) Slashing damage plus 3 (1d6) Poison damage" → two entries. No spaces in `dice`. Use the main option when a weapon has a two-handed or alternative damage; mention the other in `text` only. |

Features without attacks, saves or damage (Multiattack, Pack Tactics…) have just `name`
and `text` (and `usage` if any). Spellcasting is a trait or action whose `text` lists the
spells as printed; give it a `save` only if the block states a single save DC for it.

### Checklist before you answer

- Every stat block in the source has an entry; names are spelled as printed.
- Numbers match the source; no field is guessed.
- `ac`, `hp`, `initiative`, `passive_perception`, `proficiency_bonus`, ability scores,
  `bonus` and `dc` are numbers, not strings.
- `damage[].dice` is dice notation like `2d6+3` (the part in parentheses), never an average.
- The JSON is valid: double quotes, no trailing commas, no comments.

### Example

<!-- example-start -->
```json
{
  "format": "campaign-manager-statblocks",
  "version": 1,
  "source": "Example homebrew",
  "statblocks": [
    {
      "name": "Harbor Smuggler",
      "size": "Medium",
      "type": "Humanoid (Human)",
      "alignment": "Neutral",
      "ac": 13,
      "ac_note": "leather armor",
      "hp": 27,
      "hp_formula": "5d8+5",
      "speed": "30 ft., Swim 20 ft.",
      "initiative": 2,
      "abilities": {"str": 12, "dex": 15, "con": 12, "int": 11, "wis": 12, "cha": 14},
      "skills": "Deception +4, Stealth +4",
      "senses": "",
      "passive_perception": 11,
      "languages": "Common, Thieves' Cant",
      "cr": "1",
      "proficiency_bonus": 2,
      "traits": [
        {"name": "Slippery", "text": "The smuggler has Advantage on ability checks and saving throws made to escape a grapple."}
      ],
      "actions": [
        {"name": "Multiattack", "text": "The smuggler makes two Cutlass attacks."},
        {
          "name": "Cutlass",
          "text": "Melee Attack Roll: +4, reach 5 ft. Hit: 6 (1d8 + 2) Slashing damage.",
          "attack": {"bonus": 4, "kind": "melee", "range": "reach 5 ft."},
          "damage": [{"dice": "1d8+2", "type": "slashing"}]
        },
        {
          "name": "Hand Crossbow",
          "text": "Ranged Attack Roll: +4, range 30/120 ft. Hit: 5 (1d6 + 2) Piercing damage plus 3 (1d6) Poison damage.",
          "attack": {"bonus": 4, "kind": "ranged", "range": "range 30/120 ft."},
          "damage": [{"dice": "1d6+2", "type": "piercing"}, {"dice": "1d6", "type": "poison"}]
        }
      ],
      "bonus_actions": [
        {"name": "Duck and Weave", "text": "The smuggler takes the Disengage or Hide action."}
      ],
      "notes": "Runs contraband through the sea caves. **Hook:** will trade a map for safe passage.",
      "tags": ["smugglers"],
      "source": "Example homebrew p. 3"
    },
    {
      "name": "Hedge Mage",
      "size": "Medium",
      "type": "Humanoid (Elf)",
      "alignment": "Chaotic Neutral",
      "ac": 12,
      "hp": 33,
      "hp_formula": "6d8+6",
      "speed": "30 ft.",
      "abilities": {"str": 9, "dex": 14, "con": 12, "int": 16, "wis": 13, "cha": 10},
      "saves": "Int +5, Wis +3",
      "senses": "Darkvision 60 ft.",
      "passive_perception": 11,
      "languages": "Common, Elvish, Sylvan",
      "cr": "2",
      "actions": [
        {
          "name": "Thorn Bolt",
          "text": "Ranged Attack Roll: +5, range 120 ft. Hit: 12 (2d8 + 3) Piercing damage.",
          "attack": {"bonus": 5, "kind": "ranged", "range": "range 120 ft."},
          "damage": [{"dice": "2d8+3", "type": "piercing"}]
        },
        {
          "name": "Bramble Burst",
          "usage": "Recharge 5–6",
          "text": "Dexterity Saving Throw: DC 13, each creature in a 15-foot Cube originating from the mage. Failure: 14 (4d6) Piercing damage, and the target's Speed is halved until the end of its next turn. Success: Half damage only.",
          "save": {"ability": "DEX", "dc": 13},
          "damage": [{"dice": "4d6", "type": "piercing"}]
        },
        {
          "name": "Spellcasting",
          "text": "The mage casts one of the following spells, using Intelligence as the spellcasting ability (spell save DC 13): At will: Druidcraft, Mage Hand. 1/Day each: Entangle, Misty Step."
        }
      ],
      "reactions": [
        {"name": "Bark Skin", "usage": "1/Day", "text": "Trigger: The mage is hit by an attack roll. Response: The mage gains a +4 bonus to AC against that attack, possibly causing it to miss."}
      ]
    }
  ]
}
```
<!-- example-end -->

### Tolerances

The importer is forgiving: it also accepts a bare array of stat blocks, `"hp": "27 (5d8 + 5)"`,
`"ac": "13 (leather armor)"`, top-level `str`/`dex`/… keys, `"desc"` instead of `"text"`,
and trailing commas. But following the format above exactly gives the best results, and the
app will list anything it couldn't read as a warning before importing.
