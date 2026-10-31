//! Randomised tests: every function that reads text typed by a user, pasted from an AI or
//! loaded from a file must cope with anything (multi-byte characters, stray Markdown,
//! half-finished input) without panicking, and a few must keep simple promises.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use cm_core::dice::{self, DiceExpr, DiceRng};
use cm_core::{bestiary, items, loot, wiki};
use proptest::prelude::*;

/// Text built from pieces that tend to break parsers: Markdown marks, dice syntax,
/// brackets, newlines and characters that take more than one byte in UTF-8.
fn tricky() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        Just("d".to_string()),
        Just("d20".to_string()),
        Just("+".to_string()),
        Just("-".to_string()),
        Just("*".to_string()),
        Just("**".to_string()),
        Just("/".to_string()),
        Just("(".to_string()),
        Just(")".to_string()),
        Just("[[".to_string()),
        Just("]]".to_string()),
        Just("[".to_string()),
        Just("](".to_string()),
        Just("|".to_string()),
        Just("#".to_string()),
        Just("- ".to_string()),
        Just("- [ ] ".to_string()),
        Just("1. ".to_string()),
        Just("> ".to_string()),
        Just("```".to_string()),
        Just("`".to_string()),
        Just("~~".to_string()),
        Just("_".to_string()),
        Just("\n".to_string()),
        Just("\r\n".to_string()),
        Just("  ".to_string()),
        Just("\t".to_string()),
        Just("é".to_string()),
        Just("日本".to_string()),
        Just("🐉".to_string()),
        Just("e\u{301}".to_string()),
        Just("½".to_string()),
        Just("—".to_string()),
        Just("kh".to_string()),
        Just("!".to_string()),
        Just("{".to_string()),
        Just("}".to_string()),
        Just("\"".to_string()),
        Just(":".to_string()),
        Just(",".to_string()),
        Just("@".to_string()),
        Just("%".to_string()),
        Just("0".to_string()),
        Just("99999999999".to_string()),
        Just("CR".to_string()),
        Just("gp".to_string()),
        "[a-z]{1,6}",
        "[0-9]{1,4}",
        "\\PC{1,3}",
    ];
    prop::collection::vec(piece, 0..24).prop_map(|v| v.concat())
}

fn rng() -> fastrand::Rng {
    fastrand::Rng::with_seed(7)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 2000, ..ProptestConfig::default() })]

    #[test]
    fn dice_never_panic_and_stay_fast(s in tricky()) {
        let start = Instant::now();
        if let Ok(e) = DiceExpr::parse(&s) {
            let _ = e.source();
            if let Ok(o) = e.roll(&mut rng()) {
                let _ = o.breakdown();
                prop_assert!(!o.total.is_nan(), "NaN total for {s:?}");
            }
        }
        let _ = dice::parse_command(&s);
        let _ = dice::expand_macros(&s, &BTreeMap::new());
        prop_assert!(start.elapsed() < Duration::from_secs(2), "slow: {s:?} took {:?}", start.elapsed());
    }

    #[test]
    fn dice_text_and_macros(s in tricky(), name in "[a-z]{1,5}") {
        let mut m = BTreeMap::new();
        m.insert(name.clone(), s.clone());
        let _ = dice::expand_macros(&format!("{name}+1"), &m);
        let _ = dice::roll_to_record(&s, &s, &s, &m, &mut rng());
    }

    #[test]
    fn stat_block_and_item_import_never_panic(s in tricky()) {
        let _ = bestiary::clean_json_text(&s);
        let _ = bestiary::import_json(&s);
        let _ = bestiary::find_dice(&s);
        let _ = items::import_json(&s);
        let _ = items::parse_cost_text(&s);
        let _ = items::plus_bonus(&s);
        let _ = items::Rarity::parse(&s);
        let _ = items::ItemKind::parse(&s);
        let _ = loot::cr_number(&s);
    }

    /// Item descriptions mixing the phrases the roll/bonus finder looks for with
    /// characters whose lowercase or UTF-8 form is a different length.
    #[test]
    fn item_text_scanning_never_panics(parts in prop::collection::vec(prop_oneof![
        Just(" damage"), Just(" bonus to "), Just("regains "), Just("regain "), Just("extra "),
        Just(" hit points"), Just("+1"), Just("+2 "), Just("2d6"), Just("1d4 + 1"), Just("."), Just(";"), Just("("),
        Just("İ"), Just("’"), Just("—"), Just("é"), Just("ß"), Just("🐉"), Just("x"), Just(" "), Just("attack and damage rolls"),
        Just("armor class"), Just("saving throws"), Just(" while "), Just(" that "),
    ], 0..40)) {
        let desc = parts.concat();
        for kind in [items::ItemKind::Weapon, items::ItemKind::Armor, items::ItemKind::Potion, items::ItemKind::Wondrous] {
            let it = items::Item { name: "+1 Thing".into(), kind, description: desc.clone(), damage: desc.clone(), ..Default::default() };
            let _ = items::suggest_rolls(&it);
            let _ = items::suggest_modifiers(&it);
        }
    }

    #[test]
    fn wiki_helpers_never_panic(s in tricky()) {
        let (fm, body) = wiki::split_front_matter(&s);
        if let Some(fm) = fm {
            let _ = wiki::parse_front_matter(fm);
        }
        let _ = body.len();
        let _ = wiki::extract_wikilinks(&s);
        let _ = wiki::extract_inline_tags(&s);
        let _ = wiki::parse_note("folder/note.md", s.clone());
        let _ = wiki::hard_line_breaks(&s);
        let _ = wiki::wikilinks_to_markdown(&s);
        let _ = wiki::note_rel_path(&s, &s);
        prop_assert_eq!(wiki::decode_target(&wiki::encode_target(&s)), s.clone());
        let _ = cm_core::storage::sanitize_file_name(&s);
    }

    /// Single line breaks become hard breaks, but no text is lost or changed.
    #[test]
    fn hard_line_breaks_keep_the_words(s in tricky()) {
        let out = wiki::hard_line_breaks(&s);
        let squash = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
        prop_assert_eq!(squash(&out), squash(&s));
    }

    /// Stat blocks and items survive export → import unchanged in their main fields.
    #[test]
    fn json_export_round_trips(name in "\\PC{1,30}", cr in "[0-9]{1,2}|1/2|1/4", ac in 1i32..30, hp in 1i32..500) {
        let mut sb = bestiary::StatBlock::default();
        sb.core.name = name.trim().to_string();
        prop_assume!(!sb.core.name.is_empty());
        sb.core.cr = cr.clone();
        sb.core.ac = Some(ac);
        sb.core.max_hp = Some(hp);
        let text = bestiary::export_json(&[sb.clone()], "Mine");
        let back = bestiary::import_json(&text).unwrap();
        prop_assert_eq!(back.statblocks.len(), 1);
        let b = &back.statblocks[0];
        prop_assert_eq!(&b.core.name, &sb.core.name);
        prop_assert_eq!(&b.core.cr, &cr);
        prop_assert_eq!(b.core.ac, Some(ac));
        prop_assert_eq!(b.core.max_hp, Some(hp));

        let it = items::Item::named(name.trim(), items::ItemKind::Weapon, items::Rarity::Rare, Some(hp as f64));
        let text = items::export_json(&[it.clone()], "Mine");
        let back = items::import_json(&text).unwrap();
        prop_assert_eq!(back.items.len(), 1);
        prop_assert_eq!(&back.items[0].name, &it.name);
        prop_assert_eq!(back.items[0].rarity, it.rarity);
        prop_assert_eq!(back.items[0].cost_gp, it.cost_gp);
    }

    /// Any calendar the settings screen can produce (even one with no months or 0-day
    /// months) and any stored date: nothing panics, and clamped dates validate.
    #[test]
    fn calendars_never_panic(
        months in prop::collection::vec(("[A-Za-z]{0,6}", 0u32..40), 0..5),
        weekdays in prop::collection::vec("[A-Za-z]{1,5}", 0..8),
        epoch in any::<u32>(),
        year in any::<i32>(), month in any::<u32>(), day in any::<u32>(), shift in any::<i64>(),
    ) {
        let cal = cm_core::calendar::GameCalendar {
            months: months.into_iter().map(|(name, days)| cm_core::calendar::Month { name, days }).collect(),
            weekdays,
            epoch_weekday: epoch,
            ..Default::default()
        };
        let d = cm_core::calendar::GameDate { year, month, day };
        let _ = cal.validate(&d);
        let c = cal.clamp(d);
        if !cal.months.is_empty() && cal.months.iter().all(|m| m.days > 0) {
            prop_assert!(cal.validate(&c).is_ok(), "{c:?} doesn't fit after clamping");
        }
        let _ = cal.ordinal(&d);
        let _ = cal.format(&d);
        let _ = cal.weekday(&d);
        let _ = cal.add_days(&d, shift);
        let _ = cal.days_between(&d, &c);
        let _ = cal.days_in_year();
    }

    #[test]
    fn campaign_search_never_panics(s in tricky()) {
        static SAMPLE: std::sync::OnceLock<cm_core::Campaign> = std::sync::OnceLock::new();
        let c = SAMPLE.get_or_init(cm_core::sample_campaign);
        let _ = cm_core::search::search(c, &s);
    }

    #[test]
    fn hp_input_never_panics(s in tricky()) {
        let mut c = cm_core::encounter::Combatant {
            max_hp: Some(20),
            current_hp: Some(20),
            ..Default::default()
        };
        for kind in [cm_core::encounter::HpChangeKind::Damage, cm_core::encounter::HpChangeKind::Heal, cm_core::encounter::HpChangeKind::Set] {
            let _ = c.apply_hp(kind, &s, &mut rng(), 1);
        }
    }
}

/// Inputs that once caused trouble elsewhere, or obviously might.
#[test]
fn nasty_dice_are_refused_or_quick() {
    for s in [
        "99999999999d6",
        "10000d6+10000d6+10000d6+10000d6",
        "10000d1!",
        "10000d2!",
        "1d0",
        "0d6",
        "1d-6",
        "d",
        "1d6/0",
        "1d6%0",
        "(((((((((((((((((((((((((((((((1)))))))))))))))))))))))))))))))",
        "1d6kh99999",
        "-2147483648d6",
        "2147483647+2147483647",
        "1e308*1e308",
    ] {
        let start = Instant::now();
        if let Ok(e) = DiceExpr::parse(s) {
            if let Ok(o) = e.roll(&mut rng()) {
                assert!(o.total.is_finite(), "{s:?} gave {}", o.total);
            }
        }
        assert!(start.elapsed() < Duration::from_secs(2), "{s:?} took {:?}", start.elapsed());
    }
    // deep nesting must not overflow the stack
    let deep = format!("{}1{}", "(".repeat(5000), ")".repeat(5000));
    let _ = DiceExpr::parse(&deep);
    let _ = rng().roll_die(1);
}
