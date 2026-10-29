//! Conditions from the 2024 (5.5e) rules, summarised in plain language, plus support for
//! DM-defined custom conditions. Summaries are short paraphrases for at-the-table reference.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ConditionDef {
    pub name: String,
    /// One-line reminder shown in tooltips.
    pub summary: String,
    /// Bullet-point effects (markdown allowed).
    pub effects: Vec<String>,
    /// True for conditions defined by the DM.
    pub custom: bool,
    /// Uses levels (Exhaustion).
    pub leveled: bool,
}

fn std(name: &str, summary: &str, effects: &[&str]) -> ConditionDef {
    ConditionDef {
        name: name.into(),
        summary: summary.into(),
        effects: effects.iter().map(|s| s.to_string()).collect(),
        custom: false,
        leveled: false,
    }
}

/// The 15 conditions of the 2024 Player's Handbook rules glossary.
pub fn standard_conditions() -> Vec<ConditionDef> {
    let mut v = vec![
        std(
            "Blinded",
            "Can't see; attacks against it have Advantage, its attacks have Disadvantage.",
            &[
                "Can't see; automatically fails any ability check that requires sight.",
                "Attack rolls against it have Advantage.",
                "Its attack rolls have Disadvantage.",
            ],
        ),
        std(
            "Charmed",
            "Can't attack the charmer; charmer has Advantage on social checks against it.",
            &[
                "Can't attack the charmer or target them with damaging abilities or magical effects.",
                "The charmer has Advantage on ability checks to interact with it socially.",
            ],
        ),
        std(
            "Deafened",
            "Can't hear; fails checks that require hearing.",
            &["Can't hear; automatically fails any ability check that requires hearing."],
        ),
        ConditionDef {
            name: "Exhaustion".into(),
            summary: "Cumulative levels: −2 × level to D20 Tests, −5 ft × level Speed; dies at level 6.".into(),
            effects: vec![
                "Levels stack (1–6). At level 6 the creature dies.".into(),
                "D20 Tests (attacks, checks, saves) are reduced by 2 × the exhaustion level.".into(),
                "Speed is reduced by 5 feet × the exhaustion level.".into(),
                "A Long Rest removes one level.".into(),
            ],
            custom: false,
            leveled: true,
        },
        std(
            "Frightened",
            "Disadvantage on checks and attacks while the source is in sight; can't move closer to it.",
            &[
                "Disadvantage on ability checks and attack rolls while the source of fear is within line of sight.",
                "Can't willingly move closer to the source of fear.",
            ],
        ),
        std(
            "Grappled",
            "Speed 0; Disadvantage on attacks against anyone but the grappler; can be dragged.",
            &[
                "Speed is 0 and can't increase.",
                "Disadvantage on attack rolls against any target other than the grappler.",
                "The grappler can move it, but each foot costs an extra foot unless it is Tiny or two+ sizes smaller.",
            ],
        ),
        std(
            "Incapacitated",
            "No actions, Bonus Actions, or Reactions; Concentration ends; can't speak.",
            &[
                "Can't take any action, Bonus Action, or Reaction.",
                "Concentration is broken.",
                "Can't speak.",
                "If Incapacitated when rolling Initiative, it rolls with Disadvantage.",
            ],
        ),
        std(
            "Invisible",
            "Advantage on Initiative; attacks against it have Disadvantage, its attacks have Advantage.",
            &[
                "Has Advantage on Initiative rolls.",
                "Isn't affected by effects that require the target to be seen, unless the source can somehow see it.",
                "Attack rolls against it have Disadvantage; its attack rolls have Advantage (unless the other creature can see it).",
            ],
        ),
        std(
            "Paralyzed",
            "Incapacitated, Speed 0, fails STR/DEX saves; hits from within 5 ft are crits.",
            &[
                "Has the Incapacitated condition.",
                "Speed is 0 and can't increase.",
                "Automatically fails Strength and Dexterity saving throws.",
                "Attack rolls against it have Advantage.",
                "Any attack that hits it is a Critical Hit if the attacker is within 5 feet.",
            ],
        ),
        std(
            "Petrified",
            "Turned to stone: Incapacitated, Speed 0, resists all damage, fails STR/DEX saves.",
            &[
                "Transformed (with its nonmagical gear) into an inanimate substance; weight ×10; stops aging.",
                "Has the Incapacitated condition; Speed is 0 and can't increase.",
                "Attack rolls against it have Advantage.",
                "Automatically fails Strength and Dexterity saving throws.",
                "Has Resistance to all damage and Immunity to the Poisoned condition.",
            ],
        ),
        std(
            "Poisoned",
            "Disadvantage on attack rolls and ability checks.",
            &["Disadvantage on attack rolls and ability checks."],
        ),
        std(
            "Prone",
            "Crawl or spend half Speed to stand; its attacks have Disadvantage; melee within 5 ft has Advantage against it.",
            &[
                "Only movement options are crawling or spending half its Speed (round down) to stand up.",
                "Disadvantage on its attack rolls.",
                "Attack rolls against it have Advantage if the attacker is within 5 feet; otherwise Disadvantage.",
            ],
        ),
        std(
            "Restrained",
            "Speed 0; attacks against it have Advantage; its attacks and DEX saves have Disadvantage.",
            &[
                "Speed is 0 and can't increase.",
                "Attack rolls against it have Advantage.",
                "Its attack rolls have Disadvantage.",
                "Disadvantage on Dexterity saving throws.",
            ],
        ),
        std(
            "Stunned",
            "Incapacitated; fails STR/DEX saves; attacks against it have Advantage.",
            &[
                "Has the Incapacitated condition.",
                "Automatically fails Strength and Dexterity saving throws.",
                "Attack rolls against it have Advantage.",
            ],
        ),
        std(
            "Unconscious",
            "Incapacitated and Prone, drops held items; fails STR/DEX saves; hits within 5 ft are crits.",
            &[
                "Has the Incapacitated and Prone conditions and drops whatever it's holding.",
                "Speed is 0 and can't increase.",
                "Attack rolls against it have Advantage.",
                "Automatically fails Strength and Dexterity saving throws.",
                "Any attack that hits it is a Critical Hit if the attacker is within 5 feet.",
                "Unaware of its surroundings.",
            ],
        ),
    ];
    // Common table-side statuses that aren't formal conditions but DMs track constantly.
    v.push(ConditionDef {
        name: "Concentrating".into(),
        summary: "Maintaining a spell; taking damage forces a CON save (DC 10 or half damage, max 30).".into(),
        effects: vec![
            "When it takes damage, it makes a Constitution saving throw: DC 10 or half the damage taken (round down), whichever is higher, up to DC 30.".into(),
            "Ends if it becomes Incapacitated or dies, or casts another Concentration spell.".into(),
        ],
        custom: false,
        leveled: false,
    });
    v
}

/// Concentration save DC after taking `damage` (2024 rules: max(10, damage/2), capped at 30).
pub fn concentration_dc(damage: i32) -> i32 {
    (damage / 2).clamp(10, 30)
}

/// A condition applied to a combatant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppliedCondition {
    pub name: String,
    pub note: String,
    /// Remaining rounds; `None` = until removed.
    pub rounds_remaining: Option<u32>,
    /// Exhaustion level (1–6) when the condition is leveled.
    pub level: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_all_fifteen() {
        let c = standard_conditions();
        let names: Vec<_> = c.iter().map(|c| c.name.as_str()).collect();
        for n in [
            "Blinded",
            "Charmed",
            "Deafened",
            "Exhaustion",
            "Frightened",
            "Grappled",
            "Incapacitated",
            "Invisible",
            "Paralyzed",
            "Petrified",
            "Poisoned",
            "Prone",
            "Restrained",
            "Stunned",
            "Unconscious",
        ] {
            assert!(names.contains(&n), "missing {n}");
        }
        assert!(c.iter().all(|c| !c.summary.is_empty() && !c.effects.is_empty()));
        assert!(c.iter().find(|c| c.name == "Exhaustion").unwrap().leveled);
    }

    #[test]
    fn concentration() {
        assert_eq!(concentration_dc(5), 10);
        assert_eq!(concentration_dc(22), 11);
        assert_eq!(concentration_dc(100), 30);
    }
}
