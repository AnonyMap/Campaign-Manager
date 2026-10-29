#!/usr/bin/env python3
"""Convert Open5e's SRD 5.2 (2024) equipment and magic items into the Campaign Manager item
import format (docs/item-import-spec.md).

Usage:
    git clone --depth 1 https://github.com/open5e/open5e-api.git
    python3 tools/convert_open5e_items.py open5e-api/data/v2/wizards-of-the-coast/srd-2024 \
        crates/cm_core/data/srd-5.2-items.json

Rolls and modifiers are not written here: the app derives them from names and rules text when
it loads the list (so the same logic serves imported books).

SRD 5.2 content is (c) Wizards of the Coast, licensed CC-BY-4.0. See ATTRIBUTION in the output.
"""
import json
import re
import sys
from collections import defaultdict

ATTRIBUTION = ("This work includes material from the System Reference Document 5.2 (\"SRD 5.2\") by "
               "Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2 is "
               "licensed under the Creative Commons Attribution 4.0 International License, available at "
               "https://creativecommons.org/licenses/by/4.0/legalcode. Data structured by the Open5e project.")

KIND = {
    "adventuring-gear": "Adventuring gear", "weapon": "Weapon", "tools": "Tool", "armor": "Armor",
    "equipment-pack": "Adventuring gear", "mount": "Mount", "waterborne-vehicle": "Vehicle",
    "land-vehicle": "Vehicle", "ammunition": "Ammunition", "potion": "Potion",
    "spellcasting-focus": "Adventuring gear", "wondrous-item": "Wondrous item", "scroll": "Scroll",
    "ring": "Ring", "wand": "Wand", "staff": "Staff", "rod": "Rod",
}
RARITY = {"common": "Common", "uncommon": "Uncommon", "rare": "Rare", "very-rare": "Very rare",
          "legendary": "Legendary", "artifact": "Artifact"}


def num(s):
    try:
        f = float(s)
    except (TypeError, ValueError):
        return None
    return f if f > 0 else None


def main(src, out_path):
    load = lambda n: json.load(open(f"{src}/{n}"))
    items, magic = load("Item.json"), load("MagicItem.json")
    weapons = {w["pk"]: w["fields"] for w in load("Weapon.json")}
    armors = {a["pk"]: a["fields"] for a in load("Armor.json")}
    props = {p["pk"]: p["fields"]["name"] for p in load("WeaponProperty.json")}
    wprops = defaultdict(list)
    for a in load("WeaponPropertyAssignment.json"):
        f = a["fields"]
        name = props.get(f["property"], "")
        if name.endswith("Mastery") or f["property"].endswith("-mastery"):
            name = "Mastery: " + props.get(f["property"], f["property"].split("_")[-1].replace("-mastery", "").title())
        wprops[f["weapon"]].append(f"{name} ({f['detail']})" if f.get("detail") else name)

    def weapon_bits(pk):
        w = weapons.get(pk)
        if not w:
            return "", ""
        dmg = f"{w['damage_dice']} {w['damage_type']}".strip()
        return dmg, ", ".join(wprops.get(pk, []))

    def armor_ac(pk):
        a = armors.get(pk)
        if not a:
            return ""
        if a["name"] == "Shield":
            return "+2"
        if not a["ac_add_dexmod"]:
            return str(a["ac_base"])
        cap = a.get("ac_cap_dexmod")
        return f"{a['ac_base']} + Dex modifier" + (f" (max {cap})" if cap else "")

    out = []
    seen = set()

    def add(entry):
        # tool names in the source carry their price: "Smith's Tools (20 GP)"
        entry["name"] = re.sub(r" \(\d+ GP\)$", "", entry["name"])
        if entry.get("description", "").endswith(" abo"):  # truncated source text
            entry["description"] = entry["description"][:-4]
        if entry["name"] in seen:
            return
        seen.add(entry["name"])
        out.append({k: v for k, v in entry.items() if v not in (None, "", [])})

    magic_names = {m["fields"]["name"] for m in magic}
    for x in sorted(items, key=lambda x: x["fields"]["name"]):
        f = x["fields"]
        cost = num(f["cost"])
        if f["name"] in magic_names or (cost is None and f["category"] in ("potion", "wondrous-item")):
            continue  # generic placeholders ("Potions of Healing") or duplicates of magic items
        dmg, prop = weapon_bits(f["weapon"]) if f.get("weapon") else ("", "")
        rarity = "Common" if f["name"] in ("Potion of Healing",) else "Mundane"
        add({"name": f["name"], "type": KIND.get(f["category"], "Other"), "rarity": rarity, "cost": cost,
             "weight": num(f["weight"]), "damage": dmg, "properties": prop,
             "armor_class": armor_ac(f["armor"]) if f.get("armor") else "",
             "description": f.get("desc") or "", "tags": [f["category"]]})

    # Magic items: keep "+1/+2/+3" variants and small variant groups; collapse large groups of
    # same-rarity variants ("Defender (Longsword)", ×39) into one entry listing the choices.
    groups = defaultdict(list)
    for m in magic:
        name = m["fields"]["name"]
        mt = re.match(r"^(.*) \((.*)\)$", name)
        if mt and not mt.group(2).startswith("+"):
            groups[mt.group(1)].append(m)
        else:
            groups[name].append(m)
    for base, ms in sorted(groups.items()):
        rarities = {m["fields"]["rarity"] for m in ms}
        if len(ms) > 3 and len(rarities) == 1:
            f = ms[0]["fields"]
            variants = sorted(mt.group(1) for mt in (re.match(r"^.* \((.*)\)$", m["fields"]["name"]) for m in ms) if mt)
            desc = (f.get("desc") or "") + "\n\nAvailable as: " + ", ".join(variants) + "."
            add({"name": base, "type": KIND.get(f["category"], "Wondrous item"), "rarity": RARITY.get(f["rarity"], "Uncommon"),
                 "attunement": f.get("requires_attunement") and "requires attunement" or "",
                 "weight": num(f["weight"]), "description": desc, "tags": [f["category"]]})
            continue
        for m in ms:
            f = m["fields"]
            dmg, prop = weapon_bits(f["weapon"]) if f.get("weapon") else ("", "")
            add({"name": f["name"], "type": KIND.get(f["category"], "Wondrous item"), "rarity": RARITY.get(f["rarity"], "Uncommon"),
                 "attunement": f.get("requires_attunement") and "requires attunement" or "",
                 "cost": num(f["cost"]), "weight": num(f["weight"]), "damage": dmg, "properties": prop,
                 "armor_class": armor_ac(f["armor"]) if f.get("armor") else "",
                 "description": f.get("desc") or "", "tags": [f["category"]]})

    # Stronger healing potions and spell scrolls by level (game values; wording is ours).
    for name, rarity, dice in [("Potion of Healing (Greater)", "Uncommon", "4d4 + 4"),
                               ("Potion of Healing (Superior)", "Rare", "8d4 + 8"),
                               ("Potion of Healing (Supreme)", "Very rare", "10d4 + 20")]:
        add({"name": name, "type": "Potion", "rarity": rarity, "weight": 0.5,
             "description": f"A magic potion: you regain {dice} Hit Points when you drink it. Drinking it or giving it to someone takes a Bonus Action.",
             "tags": ["potion", "healing"]})
    for lvl, rarity in [(0, "Common"), (1, "Common"), (2, "Uncommon"), (3, "Uncommon"), (4, "Rare"), (5, "Rare"),
                        (6, "Very rare"), (7, "Very rare"), (8, "Very rare"), (9, "Legendary")]:
        label = "Cantrip" if lvl == 0 else f"Level {lvl}"
        add({"name": f"Spell Scroll ({label})", "type": "Scroll", "rarity": rarity,
             "description": f"A scroll holding one {'cantrip' if lvl == 0 else f'level {lvl} spell'}. A creature who has the spell on its spell list can read it to cast the spell without material components; the scroll then crumbles to dust. Choose the spell when the scroll is found.",
             "tags": ["scroll", "spell scroll"]})

    doc = {"format": "campaign-manager-items", "version": 1, "source": "SRD 5.2",
           "attribution": ATTRIBUTION, "items": out}
    with open(out_path, "w") as fh:
        json.dump(doc, fh, indent=1, ensure_ascii=False)
    print(f"wrote {len(out)} items to {out_path}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
