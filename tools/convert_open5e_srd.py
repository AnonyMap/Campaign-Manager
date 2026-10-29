#!/usr/bin/env python3
"""Convert Open5e's SRD 5.2 (2024) creature data into the Campaign Manager stat block
import format (docs/statblock-import-spec.md).

Usage:
    git clone --depth 1 https://github.com/open5e/open5e-api.git
    python3 tools/convert_open5e_srd.py open5e-api/data/v2/wizards-of-the-coast/srd-2024 \
        crates/cm_core/data/srd-5.2-monsters.json

SRD 5.2 content is (c) Wizards of the Coast, licensed CC-BY-4.0. See ATTRIBUTION in the output.
"""
import json
import re
import sys
from collections import defaultdict

ABILS = ["strength", "dexterity", "constitution", "intelligence", "wisdom", "charisma"]
SHORT = {"strength": "Str", "dexterity": "Dex", "constitution": "Con",
         "intelligence": "Int", "wisdom": "Wis", "charisma": "Cha"}
SKILLS = ["acrobatics", "animal_handling", "arcana", "athletics", "deception", "history", "insight",
          "intimidation", "investigation", "medicine", "nature", "perception", "performance",
          "persuasion", "religion", "sleight_of_hand", "stealth", "survival"]
ATTRIBUTION = ("This work includes material from the System Reference Document 5.2 (\"SRD 5.2\") by "
               "Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2 is "
               "licensed under the Creative Commons Attribution 4.0 International License, available at "
               "https://creativecommons.org/licenses/by/4.0/legalcode. Data structured by the Open5e project.")

SAVE_RE = re.compile(r"(Strength|Dexterity|Constitution|Intelligence|Wisdom|Charisma) Saving Throw: DC (\d+)")
ATTACK_RE = re.compile(r"(Melee or Ranged|Melee|Ranged) Attack Roll: ([+\-−]\d+)(?:,\s*(reach [^.,]+|range [^.,]+))?")
DMG_RE = re.compile(r"\((\d+d\d+(?:\s*[+\-−]\s*\d+)?)\)\s+([A-Za-z]+) damage")


def mod(score):
    return (score - 10) // 2


def fmt_mod(m):
    return f"+{m}" if m >= 0 else str(m)


def cr_text(s):
    f = float(s)
    return {0.125: "1/8", 0.25: "1/4", 0.5: "1/2"}.get(f, str(int(f)) if f == int(f) else str(f))


def pb_for(cr):
    f = float(cr)
    for limit, pb in [(4, 2), (8, 3), (12, 4), (16, 5), (20, 6), (24, 7), (28, 8)]:
        if f <= limit:
            return pb
    return 9


def dice(count, die, bonus):
    if not count or not die:
        return None
    s = f"{count}{die.lower()}"
    if bonus:
        s += f"+{bonus}" if bonus > 0 else str(bonus)
    return s


def speed_text(f):
    parts = []
    if f.get("walk") is not None:
        parts.append(f"{f['walk']} ft.")
    for k in ["burrow", "climb", "fly", "swim"]:
        if f.get(k):
            extra = " (hover)" if k == "fly" and f.get("hover") else ""
            parts.append(f"{k.capitalize()} {f[k]} ft.{extra}")
    return ", ".join(parts)


def senses_text(f):
    parts = []
    for key, label in [("blindsight_range", "Blindsight"), ("darkvision_range", "Darkvision"),
                       ("tremorsense_range", "Tremorsense"), ("truesight_range", "Truesight")]:
        if f.get(key):
            parts.append(f"{label} {f[key]} ft.")
    if f.get("passive_perception") is not None:
        parts.append(f"Passive Perception {f['passive_perception']}")
    return "; ".join(parts)


def usage_text(a):
    t, p = a.get("uses_type"), a.get("uses_param")
    u = ""
    if t == "PER_DAY":
        u = f"{p}/Day"
    elif t in ("RECHARGE_ON_ROLL",):
        u = f"Recharge {p}–6" if p and p < 6 else "Recharge 6"
    elif t == "RECHARGE":
        u = "Recharges after a Short or Long Rest"
    cost = a.get("legendary_action_cost")
    if cost and cost > 1:
        u = (u + "; " if u else "") + f"Costs {cost} Actions"
    return u


def convert(src_dir):
    creatures = json.load(open(f"{src_dir}/Creature.json"))
    actions = json.load(open(f"{src_dir}/CreatureAction.json"))
    attacks = json.load(open(f"{src_dir}/CreatureActionAttack.json"))
    traits = json.load(open(f"{src_dir}/CreatureTrait.json"))

    by_parent_actions = defaultdict(list)
    for a in actions:
        by_parent_actions[a["fields"]["parent"]].append(a)
    attack_by_action = defaultdict(list)
    for at in attacks:
        attack_by_action[at["fields"]["parent"]].append(at["fields"])
    traits_by_parent = defaultdict(list)
    for t in traits:
        traits_by_parent[t["fields"]["parent"]].append(t["fields"])

    out = []
    for c in sorted(creatures, key=lambda c: c["fields"]["name"]):
        f = c["fields"]
        pk = c["pk"]
        scores = {a: f[f"ability_score_{a}"] for a in ABILS}
        saves = []
        for a in ABILS:
            sv = f.get(f"saving_throw_{a}")
            if sv is not None and sv != mod(scores[a]):
                saves.append(f"{SHORT[a]} {fmt_mod(sv)}")
        skills = []
        for s in SKILLS:
            v = f.get(f"skill_bonus_{s}")
            if v is not None:
                skills.append(f"{s.replace('_', ' ').title()} {fmt_mod(v)}")

        def feature(a, kind):
            af = a["fields"]
            desc = af.get("desc") or ""
            feat = {"name": af["name"], "text": desc}
            u = usage_text(af)
            if u:
                feat["usage"] = u
            recs = attack_by_action.get(a["pk"], [])
            if recs:
                r = recs[0]
                rng = []
                if r.get("reach"):
                    rng.append(f"reach {r['reach']} ft.")
                if r.get("range"):
                    rng.append(f"range {r['range']}{'/' + str(r['long_range']) if r.get('long_range') else ''} ft.")
                kind_txt = "melee or ranged" if r.get("reach") and r.get("range") else ("ranged" if r.get("range") else "melee")
                feat["attack"] = {"bonus": r["to_hit_mod"], "kind": kind_txt, "range": " or ".join(rng)}
                dmg = []
                d1 = dice(r.get("damage_die_count"), r.get("damage_die_type"), r.get("damage_bonus"))
                if d1:
                    dmg.append({"dice": d1, "type": (r.get("damage_type") or "").lower()})
                elif r.get("damage_bonus"):
                    dmg.append({"dice": str(r["damage_bonus"]), "type": (r.get("damage_type") or "").lower()})
                d2 = dice(r.get("extra_damage_die_count"), r.get("extra_damage_die_type"), r.get("extra_damage_bonus"))
                if d2:
                    dmg.append({"dice": d2, "type": (r.get("extra_damage_type") or "").lower()})
                if dmg:
                    feat["damage"] = dmg
            else:
                m = ATTACK_RE.search(desc)
                if m:
                    feat["attack"] = {"bonus": int(m.group(2).replace("−", "-")),
                                      "kind": m.group(1).lower(), "range": (m.group(3) or "").strip()}
            sm = SAVE_RE.search(desc)
            if sm:
                feat["save"] = {"ability": SHORT[sm.group(1).lower()].upper(), "dc": int(sm.group(2))}
            if "damage" not in feat:
                dm = DMG_RE.search(desc)
                if dm and ("attack" in feat or "save" in feat):
                    feat["damage"] = [{"dice": re.sub(r"\s+", "", dm.group(1)).replace("−", "-"),
                                       "type": dm.group(2).lower()}]
            return feat

        acts = sorted(by_parent_actions.get(pk, []), key=lambda a: a["fields"].get("order_in_statblock") or 0)
        groups = defaultdict(list)
        for a in acts:
            groups[a["fields"]["action_type"]].append(feature(a, a["fields"]["action_type"]))
        legendary = groups.get("LEGENDARY_ACTION", [])
        typ = f["type"].title()
        if f.get("subcategory"):
            typ += f" ({f['subcategory'].title()})"
        entry = {
            "name": f["name"],
            "size": f["size"].title(),
            "type": typ,
            "alignment": f["alignment"].title(),
            "ac": f["armor_class"],
            "ac_note": f.get("armor_detail") or "",
            "hp": f["hit_points"],
            "hp_formula": (f.get("hit_dice") or "").replace(" ", ""),
            "speed": speed_text(f),
            "initiative": f.get("initiative_bonus"),
            "abilities": {k[:3]: v for k, v in scores.items()},
            "saves": ", ".join(saves),
            "skills": ", ".join(skills),
            "senses": senses_text(f),
            "passive_perception": f.get("passive_perception"),
            "languages": f.get("languages_desc") or "",
            "damage_vulnerabilities": f.get("damage_vulnerabilities_display") or "",
            "damage_resistances": f.get("damage_resistances_display") or "",
            "damage_immunities": f.get("damage_immunities_display") or "",
            "condition_immunities": f.get("condition_immunities_display") or "",
            "cr": cr_text(f["challenge_rating"]),
            "proficiency_bonus": f.get("proficiency_bonus") or pb_for(f["challenge_rating"]),
            "traits": [{"name": t["name"], "text": t["desc"]} for t in traits_by_parent.get(pk, [])],
            "actions": groups.get("ACTION", []),
            "bonus_actions": groups.get("BONUS_ACTION", []),
            "reactions": groups.get("REACTION", []),
            "legendary_actions": legendary,
            "tags": [f["type"].lower(), f["category"].lower()],
            "source": "SRD 5.2",
        }
        out.append(entry)
    return {
        "format": "campaign-manager-statblocks",
        "version": 1,
        "source": "SRD 5.2",
        "attribution": ATTRIBUTION,
        "statblocks": out,
    }


if __name__ == "__main__":
    doc = convert(sys.argv[1])
    with open(sys.argv[2], "w") as fh:
        json.dump(doc, fh, indent=1, ensure_ascii=False)
    print(f"wrote {len(doc['statblocks'])} stat blocks to {sys.argv[2]}")
