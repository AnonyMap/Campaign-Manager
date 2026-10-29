# Campaign Manager item import format (v1)

This document tells an AI model (or a person) how to turn item lists — magic items, equipment
tables, treasure, a shop's price list — from a PDF, a screenshot, a web page or typed notes into
a file that Campaign Manager can import into its **Item library**. The library feeds the loot
generator, shop stock and character inventories.

## How to use it (for the person)

1. In Campaign Manager open **Items → Import…** and click **Copy AI instructions** (or attach this
   file to the chat).
2. Give the AI your source and say e.g. *"Convert every magic item in the attached chapter using
   these instructions."*
3. Paste the AI's whole reply into the import window and click **Read** (or save it as a `.json`
   file and use **Choose file…**).
4. Check the preview and warnings, choose the collection to file them under, and click **Import**.

Long books: ask for one chapter or about 30 items per reply; each reply must be a complete file.

---

## Instructions for the AI

You are converting tabletop RPG (D&D 5th edition) items into JSON for import.

### Output rules

1. Reply with **one JSON document inside a single ```json code block**. No comments in the JSON.
2. The document is an object: `"format": "campaign-manager-items"`, `"version": 1`, a
   `"source"` naming the book or file, and an `"items"` array.
3. One entry per item, in source order. Include every item. If you must stop early, close the JSON
   properly and say after the block which item to continue from.
4. **Copy, don't invent.** Use the names, numbers and wording in the source. Leave out any field the
   source doesn't give. Never invent prices.
5. Keep rules text as plain text (light markdown is fine). Remove page furniture.
6. An item that comes in variants with different rarities (e.g. "+1, +2 or +3") is one entry per
   variant: "Shield (+1)", "Shield (+2)"… Variants that only differ in weapon or armor type and
   share a rarity can be one entry; list the choices at the end of `description`.

### Item fields

| Field | Type | Example | Notes |
|---|---|---|---|
| `name` | string | `"Cloak of the Owl"` | **Required.** |
| `type` | string | `"Wondrous item"` | One of: Weapon, Armor (shields too), Ammunition, Potion, Scroll, Wand, Staff, Rod, Ring, Wondrous item, Adventuring gear, Tool, Gemstone, Art object, Trade good, Food & drink, Mount, Vehicle, Other. A weapon's subtype goes in parentheses: `"Weapon (longsword)"`. |
| `rarity` | string | `"Uncommon"` | Mundane (not magic), Common, Uncommon, Rare, Very rare, Legendary, Artifact. |
| `attunement` | string or boolean | `"requires attunement by a wizard"` | `true` for plain "requires attunement"; leave out if none. |
| `cost` | number or string | `50`, `"5 sp"`, `"1,500 gp"` | Price as printed. Numbers mean gp. Leave out if the source gives none. |
| `weight` | number | `3` | Pounds. |
| `damage` | string | `"1d8 slashing"` | Weapons: base damage dice and type. |
| `armor_class` | string | `"14 + Dex modifier (max 2)"` | Armor and shields (`"+2"` for a shield). |
| `properties` | string | `"Finesse, Light, Thrown (20/60)"` | Weapon properties as printed. |
| `description` | string | `"While wearing this cloak…"` | Full rules text. |
| `rolls` | array | `[{"label": "Extra fire damage", "dice": "2d6"}]` | Dice the item lets its user roll: extra damage, healing, a breath weapon… `dice` is dice notation like `2d6+3`, no spaces. Optional — the app also finds common ones in the text. |
| `modifiers` | array | `[{"label": "AC", "value": 1}]` | Flat bonuses the item grants: `AC`, `attack rolls`, `damage rolls`, `saving throws`, `spell attack rolls`, `spell save DC`, an ability score… Optional — "+N" items are detected automatically. |
| `tags` | array of strings | `["cursed", "dwarven"]` | Optional, for filtering. |
| `source` | string | `"Tomb of Horrors p. 12"` | Book and page if known. |

### Checklist before you answer

- Every item in the source has an entry; names are spelled as printed.
- No guessed prices, weights or bonuses.
- `cost`, `weight` and `value` are numbers (or a price string like "5 sp").
- `dice` values are dice notation like `2d6+3`.
- The JSON is valid: double quotes, no trailing commas, no comments.

### Example

<!-- example-start -->
```json
{
  "format": "campaign-manager-items",
  "version": 1,
  "source": "Example homebrew",
  "items": [
    {
      "name": "Emberglass Dagger",
      "type": "Weapon (dagger)",
      "rarity": "Uncommon",
      "damage": "1d4 piercing",
      "properties": "Finesse, Light, Thrown (20/60)",
      "description": "You have a +1 bonus to attack and damage rolls made with this magic weapon. On a hit it deals an extra 1d4 Fire damage.",
      "rolls": [{"label": "Emberglass extra fire damage", "dice": "1d4"}],
      "modifiers": [{"label": "attack rolls", "value": 1}, {"label": "damage rolls", "value": 1}],
      "weight": 1,
      "source": "Example homebrew p. 2"
    },
    {
      "name": "Traveler's Tonic",
      "type": "Potion",
      "rarity": "Common",
      "cost": "75 gp",
      "description": "When you drink this tonic you regain 1d6 + 1 Hit Points and ignore one level of Exhaustion until your next Long Rest."
    },
    {
      "name": "Silvered Hand Mirror",
      "type": "Art object",
      "rarity": "Mundane",
      "cost": "25 gp",
      "description": "A palm-sized mirror in a filigreed silver frame."
    }
  ]
}
```
<!-- example-end -->

### Tolerances

The importer also accepts a bare array of items, `"price"` or `"value"` instead of `"cost"`,
`"desc"` instead of `"description"`, `"requires_attunement": true`, modifiers written as text
(`"+1 AC"`), extra chat text around the JSON, and trailing commas. Anything it can't read is
listed as a warning before importing.
