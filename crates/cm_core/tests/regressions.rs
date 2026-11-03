//! One test per bug found in the October 2026 review, so they stay fixed.

use std::time::{Duration, Instant};

use chrono::NaiveDate;
use cm_core::calendar::SessionSchedule;
use cm_core::dice::{self, SequenceRng};
use cm_core::encounter::*;
use cm_core::items::*;
use cm_core::model::*;
use cm_core::{storage, transfer};

/// Run `f` on another thread and fail if it takes longer than `secs`.
fn within<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs)).expect("took too long")
}

#[test]
fn item_text_with_curly_quotes_and_accents_imports() {
    for desc in [
        "a’’’’’’’’’’’’’’’ the target takes 2d6 fire damage".to_string(),
        format!("You have a +1 bonus to {}é and more", "x".repeat(59)),
        "İ regains éé 2d4 hit points".to_string(),
        "İİİİ—takes 3d6 cold damage—İ regains 1d4 + 1 hit points; extra 1d6 radiant damage".to_string(),
    ] {
        let it = Item {
            name: "Thing".into(),
            description: desc.clone(),
            ..Default::default()
        };
        let _ = suggest_rolls(&it);
        let _ = suggest_modifiers(&it);
        let json = serde_json::json!([{ "name": "Thing", "type": "weapon", "description": desc }]).to_string();
        assert_eq!(import_json(&json).unwrap().items.len(), 1);
    }
    // and the rolls are still found
    let it = Item {
        name: "Potion".into(),
        description: "You regain 2d4 + 2 Hit Points when you drink this “potion”.".into(),
        ..Default::default()
    };
    assert!(suggest_rolls(&it).iter().any(|r| r.notation.replace(' ', "") == "2d4+2"));
}

#[test]
fn packages_cannot_write_outside_the_campaign() {
    for bad in [
        "assets/../x.png",
        "assets/C:\\Users\\Public\\x.bat",
        "assets/C:/Users/x.bat",
        "assets/\\\\server\\share\\x",
        "assets/",
        "assets//x.png",
        "/etc/passwd",
        "assets/a/./b.png",
        "assets/x\u{0}.png",
    ] {
        assert!(!storage::is_safe_asset_path(bad), "{bad:?} should be refused");
    }
    for good in ["assets/map.png", "assets/maps/the old bridge-1a2b.jpg", "assets/Bärbel.png"] {
        assert!(storage::is_safe_asset_path(good), "{good:?} should be allowed");
    }
    // a campaign referring to such a path doesn't list it as an image to copy
    let mut c = Campaign::new("x");
    let mut n = Npc::default();
    n.core.image = Some("assets/C:\\Windows\\evil.exe".into());
    c.npcs.push(n);
    assert!(transfer::asset_paths(&c).is_empty());
}

#[test]
fn exploding_pools_and_macro_loops_stop_quickly() {
    for e in ["{10000d6x>0}x>0", "{100d6x>0}x>0", "{{10d6x>0}x>0}x>0", "{{{2d6x>0}x>0}x>0}x>0"] {
        let s = e.to_string();
        let t = within(5, move || {
            let t = Instant::now();
            let _ = dice::roll(&s);
            t.elapsed()
        });
        assert!(t < Duration::from_secs(3), "{e} took {t:?}");
    }
    let mut m = std::collections::BTreeMap::new();
    m.insert("a".to_string(), "#a".repeat(16));
    let r = within(5, move || dice::expand_macros("#a", &m));
    assert!(r.is_err());
    // ordinary macros still work
    let mut m = std::collections::BTreeMap::new();
    m.insert("atk".to_string(), "1d20+5".to_string());
    m.insert("dmg".to_string(), "#atk+1d8".to_string());
    assert_eq!(dice::expand_macros("#dmg", &m).unwrap(), "((1d20+5)+1d8)");
}

#[test]
fn huge_hp_numbers_dont_overflow() {
    let mut rng = SequenceRng::new(vec![1]);
    let mut c = Combatant {
        max_hp: Some(20),
        current_hp: Some(10),
        ..Default::default()
    };
    c.apply_hp(HpChangeKind::Heal, "3000000000", &mut rng, 1).unwrap();
    assert_eq!(c.current_hp, Some(20));
    c.apply_hp(HpChangeKind::Set, "2000000000", &mut rng, 1).unwrap();
    let _ = c.health();
    c.apply_hp(HpChangeKind::Set, "-2000000000", &mut rng, 1).unwrap();
    c.apply_hp(HpChangeKind::Damage, "2000000000", &mut rng, 1).unwrap();
    assert_eq!(c.current_hp, Some(0));
    let mut none = Combatant {
        current_hp: Some(10),
        ..Default::default()
    };
    none.apply_hp(HpChangeKind::Heal, "2147483647", &mut rng, 1).unwrap();
    assert!(none.current_hp.unwrap() > 0);
    let mut r = RunningEncounter::new("x");
    let id = r.add_adhoc("Ogre", Some(50), None);
    r.apply_hp_group(&[(id, Portion::Double)], HpChangeKind::Damage, "1500000000", &mut rng)
        .unwrap();
    assert_eq!(r.get(id).unwrap().current_hp, Some(0));
}

fn downed_pc() -> (RunningEncounter, Id) {
    let mut c = Campaign::new("x");
    let pc = PlayerCharacter::default();
    let pid = pc.id;
    c.players.push(pc);
    let mut r = RunningEncounter::new("x");
    let id = r.add_from_campaign(&c, CombatantSource::Player(pid)).unwrap();
    let cb = r.get_mut(id).unwrap();
    cb.max_hp = Some(10);
    cb.current_hp = Some(0);
    (r, id)
}

#[test]
fn death_saves_follow_the_rules() {
    let mut rng = SequenceRng::new(vec![1]);
    // stable + damaged = dying again, with one failure
    let (mut r, id) = downed_pc();
    let cb = r.get_mut(id).unwrap();
    cb.death_successes = 3;
    cb.death_failures = 1;
    assert_eq!(cb.death_state(), Some(DeathState::Stable));
    cb.apply_hp(HpChangeKind::Damage, "3", &mut rng, 1).unwrap();
    assert_eq!(cb.death_state(), Some(DeathState::Dying));
    assert_eq!((cb.death_successes, cb.death_failures), (0, 1));
    // dead stays dead when healed; Set HP revives
    cb.death_failures = 3;
    cb.apply_hp(HpChangeKind::Heal, "5", &mut rng, 1).unwrap();
    assert_eq!(cb.death_state(), Some(DeathState::Dead));
    cb.apply_hp(HpChangeKind::Set, "1", &mut rng, 1).unwrap();
    assert_eq!(cb.death_state(), None);
    assert_eq!(cb.death_failures, 0);
    // undoing an older hit taken while down takes its failure back
    let (mut r, id) = downed_pc();
    let cb = r.get_mut(id).unwrap();
    let hit = cb.apply_hp(HpChangeKind::Damage, "2", &mut rng, 1).unwrap();
    cb.apply_hp(HpChangeKind::TempHp, "3", &mut rng, 1).unwrap();
    assert_eq!(cb.death_failures, 1);
    cb.undo_hp(hit.id);
    assert_eq!(cb.death_failures, 0);
}

#[test]
fn removing_the_last_combatant_on_their_turn_starts_the_next_round() {
    let mut r = RunningEncounter::new("x");
    let a = r.add_adhoc("A", Some(5), None);
    let b = r.add_adhoc("B", Some(5), None);
    let c = r.add_adhoc("C", Some(5), None);
    for (i, id) in [a, b, c].iter().enumerate() {
        r.set_initiative(*id, Some(20 - i as i32));
    }
    r.next_turn(); // start: A
    r.next_turn(); // B
    r.next_turn(); // C
    assert_eq!((r.round, r.current().unwrap().id), (1, c));
    r.remove(c);
    assert_eq!((r.round, r.current().unwrap().id), (2, a));
    // removing someone on their turn passes it on: with only A left, that's A in round 3
    r.next_turn(); // B
    r.remove(b);
    assert_eq!((r.round, r.current().unwrap().id), (3, a));
    // and in the middle of the order it goes to whoever was next
    let mut r = RunningEncounter::new("x");
    let ids: Vec<Id> = (0..4).map(|i| r.add_adhoc(&format!("M{i}"), Some(5), None)).collect();
    for (i, id) in ids.iter().enumerate() {
        r.set_initiative(*id, Some(20 - i as i32));
    }
    r.next_turn();
    r.next_turn(); // M1
    r.remove(ids[1]);
    assert_eq!((r.round, r.current().unwrap().id), (1, ids[2]));
}

#[test]
fn schedules_with_odd_dates_answer_quickly() {
    let s = SessionSchedule {
        first_date: NaiveDate::from_ymd_opt(1026, 1, 3), // a typo'd year
        first_number: 1,
        interval_days: 7,
        skipped_dates: vec![],
    };
    let t = within(5, move || s.session_for_date(NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()));
    assert!(t.is_some());
    let s = SessionSchedule {
        first_date: NaiveDate::from_ymd_opt(2026, 1, 3),
        first_number: 1,
        interval_days: 1_000_000,
        skipped_dates: vec![],
    };
    let _ = s.date_for_session(200);
    // and normal use still works, skipping cancelled dates
    let s = SessionSchedule {
        first_date: NaiveDate::from_ymd_opt(2026, 1, 3),
        first_number: 1,
        interval_days: 7,
        skipped_dates: vec![NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()],
    };
    assert_eq!(s.date_for_session(2), NaiveDate::from_ymd_opt(2026, 1, 17));
    assert_eq!(s.session_for_date(NaiveDate::from_ymd_opt(2026, 1, 16).unwrap()), Some(1));
    assert_eq!(s.session_for_date(NaiveDate::from_ymd_opt(2026, 1, 17).unwrap()), Some(2));
    assert_eq!(s.session_for_date(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()), None);
}

#[test]
fn deleting_scrubs_stat_block_links() {
    let mut c = Campaign::new("x");
    let npc = Npc::default();
    let nid = npc.id;
    c.npcs.push(npc);
    let mut sb = cm_core::bestiary::StatBlock::default();
    sb.core.links.push(Link::Npc(nid));
    c.statblocks.push(sb);
    c.delete(&Link::Npc(nid));
    assert!(c.statblocks[0].core.links.is_empty());
}

#[test]
fn one_off_initiative_survives_save_as_premade() {
    let c = Campaign::new("x");
    let enc = Encounter {
        entries: vec![EncounterEntry {
            source: CombatantSource::AdHoc,
            name: "Wolf".into(),
            initiative_mod: Some(3),
            ..Default::default()
        }],
        ..Default::default()
    };
    let r = RunningEncounter::from_premade(&enc, &c);
    assert_eq!(r.to_premade().entries[0].initiative_mod, Some(3));
}

#[test]
fn rerolling_forgets_where_a_row_was_dragged() {
    let c = Campaign::new("x");
    let mut r = RunningEncounter::new("x");
    let g = r.add_adhoc("Goblin", None, None);
    r.get_mut(g).unwrap().tiebreak = 5;
    r.roll_initiative_for(&c, g, &mut SequenceRng::new(vec![10])).unwrap();
    assert_eq!(r.get(g).unwrap().tiebreak, 0);
    r.get_mut(g).unwrap().tiebreak = 5;
    r.set_initiative(g, Some(12));
    assert_eq!(r.get(g).unwrap().tiebreak, 0);
}

#[test]
fn exhaustion_lowers_initiative() {
    let c = Campaign::new("x");
    let mut r = RunningEncounter::new("x");
    let g = r.add_adhoc("Goblin", None, None);
    r.get_mut(g).unwrap().add_condition("Exhaustion", None);
    r.get_mut(g).unwrap().add_condition("Exhaustion", None); // level 2
    let roll = r.roll_initiative_for(&c, g, &mut SequenceRng::new(vec![10])).unwrap();
    assert_eq!(roll.total, 10 - 4);
    assert!(r.get(g).unwrap().initiative_note.contains("Exhaustion"));
}

#[test]
fn huge_creature_counts_are_capped() {
    let mut c = Campaign::new("x");
    let mut sb = cm_core::bestiary::StatBlock::default();
    sb.core.cr = "30".into();
    let sid = sb.id;
    c.statblocks.push(sb);
    let enc = Encounter {
        entries: vec![EncounterEntry {
            source: CombatantSource::StatBlock(sid),
            count: 4_000_000_000,
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_eq!(estimate_difficulty(&enc, &c, &[5]).monster_xp, u32::MAX);
    let r = within(5, move || RunningEncounter::from_premade(&enc, &c).combatants.len());
    assert_eq!(r, MAX_COPIES as usize);
}

#[test]
fn imported_sessions_get_free_numbers_and_cards_follow() {
    let mut src = Campaign::new("src");
    let s1 = Session {
        number: 1,
        title: "Arrival".into(),
        ..Default::default()
    };
    src.sessions.push(s1);
    let card = StoryCard {
        title: "Ambush".into(),
        timing: CardTiming::Session(1),
        ..Default::default()
    };
    let cid = card.id;
    src.story_cards.push(card);
    src.settings.wiki_path = Some("vault".into());

    let mut target = Campaign::new("t");
    target.sessions.push(Session {
        number: 1,
        ..Default::default()
    });
    transfer::merge(
        &mut target,
        &src,
        &[transfer::Part::Story, transfer::Part::Sessions, transfer::Part::Rules],
        transfer::Duplicates::Skip,
    )
    .unwrap();
    let mut numbers: Vec<u32> = target.sessions.iter().map(|s| s.number).collect();
    numbers.sort();
    assert_eq!(numbers, vec![1, 2]);
    assert_eq!(target.card(cid).unwrap().timing, CardTiming::Session(2));
    // a relative vault path doesn't come along
    assert_eq!(target.settings.wiki_path, None);
}

#[test]
fn deep_or_long_dice_are_refused_not_crashing() {
    let deep = format!("{}1{}", "(".repeat(5000), ")".repeat(5000));
    assert!(dice::roll(&deep).is_err());
    let long = vec!["1"; 5000].join("+");
    assert!(dice::roll(&long).is_err());
    let neg = format!("{}1", "-".repeat(5000));
    let _ = dice::roll(&neg); // fine either way, as long as it doesn't crash
    assert_eq!(dice::roll("((((2))))+3").unwrap().total, 5.0);
}

#[test]
fn a_campaign_with_an_infinite_number_still_opens() {
    let mut c = Campaign::new("x");
    let mut it = Item::named("Godslayer", ItemKind::Weapon, Rarity::Legendary, Some(1.0));
    it.weight_lb = Some(2.0);
    c.items.push(it);
    let mut sb = cm_core::bestiary::StatBlock::default();
    sb.core.name = "Ogre".into();
    c.statblocks.push(sb);
    let mut json: serde_json::Value = serde_json::to_value(&c).unwrap();
    // what serde_json writes for f64::INFINITY in a plain f64 field
    let place = cm_core::places::Location::default();
    let mut pv = serde_json::to_value(&place).unwrap();
    if let Some(obj) = pv.as_object_mut() {
        obj.insert("x".into(), serde_json::Value::Null);
    }
    json["locations"] = serde_json::json!([pv]);
    // find any f64 field in the dice history and null it, as an infinite total would
    json["dice"]["history"] = serde_json::json!([{ "id": cm_core::new_id(), "label": "", "expression": "1", "total": null, "breakdown": [] }]);
    let text = serde_json::to_string(&json).unwrap();
    let back = cm_core::storage::from_json(&text).expect("should open with the bad numbers set to 0");
    assert_eq!(back.dice.history.len(), 1);
    assert_eq!(back.dice.history[0].total, 0.0);
}

// ---- second review (November 2026) -------------------------------------------------

fn three_in_order() -> (Campaign, RunningEncounter, [Id; 3]) {
    let c = Campaign::new("x");
    let mut r = RunningEncounter::new("x");
    let a = r.add_adhoc("A", Some(5), None);
    let b = r.add_adhoc("B", Some(5), None);
    let cc = r.add_adhoc("C", Some(5), None);
    r.set_initiative(a, Some(20));
    r.set_initiative(b, Some(15));
    r.set_initiative(cc, Some(10));
    r.start(&c);
    (c, r, [a, b, cc])
}

#[test]
fn back_after_reordering_doesnt_skip_anyone() {
    let (c, mut r, [a, b, cc]) = three_in_order();
    r.next_turn(); // B
    r.move_before(&c, a, None); // A dragged to the end: B, C, A
    r.prev_turn();
    r.next_turn();
    // whatever Back did, C must still get a turn in round 1
    let mut seen = vec![r.current().unwrap().id];
    while r.round == 1 {
        r.next_turn();
        if r.round == 1 {
            seen.push(r.current().unwrap().id);
        }
    }
    assert!(seen.contains(&cc), "C skipped: {seen:?} (b={b:?})");
}

#[test]
fn back_never_lands_on_a_downed_monster_that_had_no_turn() {
    // start with the top creature already down
    let c = Campaign::new("x");
    let mut r = RunningEncounter::new("x");
    let m = r.add_adhoc("M", Some(5), None);
    let a = r.add_adhoc("A", Some(5), None);
    r.set_initiative(m, Some(20));
    r.set_initiative(a, Some(10));
    r.get_mut(m).unwrap().current_hp = Some(0);
    r.start(&c);
    assert_eq!(r.current().unwrap().id, a);
    r.prev_turn();
    assert_eq!(r.current().unwrap().id, a);
    // removing the current one after a downed monster
    let mut r = RunningEncounter::new("x");
    let ids: Vec<Id> = ["X", "M", "R", "Y"].iter().map(|n| r.add_adhoc(n, Some(5), None)).collect();
    for (i, id) in ids.iter().enumerate() {
        r.set_initiative(*id, Some(20 - i as i32));
    }
    r.get_mut(ids[1]).unwrap().current_hp = Some(0);
    r.start(&c); // X
    r.next_turn(); // R (M is down)
    r.remove(ids[2]); // turn passes to Y
    assert_eq!(r.current().unwrap().id, ids[3]);
    r.prev_turn();
    assert_ne!(r.current().unwrap().id, ids[1], "Back landed on the downed monster");
}

#[test]
fn importing_sessions_leaves_your_own_cards_alone() {
    // A: same campaign imported again with "keep both", sessions only
    let mut mine = Campaign::new("x");
    mine.sessions.push(Session {
        number: 1,
        ..Default::default()
    });
    let card = StoryCard {
        timing: CardTiming::Session(1),
        ..Default::default()
    };
    let cid = card.id;
    mine.story_cards.push(card);
    let copy = mine.clone();
    transfer::merge(&mut mine, &copy, &[transfer::Part::Sessions], transfer::Duplicates::KeepBoth).unwrap();
    assert_eq!(mine.card(cid).unwrap().timing, CardTiming::Session(1));
    // B: both have their own session 3; a shared card is skipped as a duplicate
    let mut mine = Campaign::new("x");
    mine.sessions.push(Session {
        number: 3,
        ..Default::default()
    });
    let card = StoryCard {
        timing: CardTiming::Session(3),
        ..Default::default()
    };
    let cid = card.id;
    mine.story_cards.push(card.clone());
    let mut other = Campaign::new("y");
    other.sessions.push(Session {
        number: 3,
        ..Default::default()
    });
    other.story_cards.push(card);
    transfer::merge(
        &mut mine,
        &other,
        &[transfer::Part::Story, transfer::Part::Sessions],
        transfer::Duplicates::Skip,
    )
    .unwrap();
    assert_eq!(mine.card(cid).unwrap().timing, CardTiming::Session(3));
    // Replace never leaves two sessions with one number
    let mut mine = Campaign::new("x");
    let s1 = Session {
        number: 1,
        ..Default::default()
    };
    let s2 = Session {
        number: 2,
        ..Default::default()
    };
    let mut other = Campaign::new("y");
    let mut s1b = s1.clone();
    s1b.number = 2;
    other.sessions.push(s1b);
    mine.sessions.push(s1);
    mine.sessions.push(s2);
    transfer::merge(&mut mine, &other, &[transfer::Part::Sessions], transfer::Duplicates::Replace).unwrap();
    let mut n: Vec<u32> = mine.sessions.iter().map(|s| s.number).collect();
    n.sort();
    n.dedup();
    assert_eq!(n.len(), 2);
}

fn npc_with_forms() -> (Campaign, Id, Id, Id, Id) {
    let mut c = Campaign::new("x");
    let mut blocks = Vec::new();
    for (name, ac, hp) in [("Human", 12, 40), ("Wolf", 13, 11), ("Bear", 11, 34)] {
        let mut sb = cm_core::bestiary::StatBlock::default();
        sb.core.name = name.into();
        sb.core.ac = Some(ac);
        sb.core.max_hp = Some(hp);
        blocks.push(sb.id);
        c.statblocks.push(sb);
    }
    let mut n = Npc::default();
    n.core.max_hp = Some(40);
    for b in &blocks {
        n.add_statblock(*b);
    }
    let nid = n.id;
    c.npcs.push(n);
    (c, nid, blocks[0], blocks[1], blocks[2])
}

#[test]
fn deleting_the_form_in_use_mid_fight_keeps_the_npc_record_intact() {
    let (mut c, nid, _human, wolf, _bear) = npc_with_forms();
    let mut r = RunningEncounter::new("x");
    let id = r.add_from_campaign_as(&c, CombatantSource::Npc(nid), Some(wolf)).unwrap();
    c.active_encounter = Some(r);
    c.delete(&Link::StatBlock(wolf));
    let mut r = c.active_encounter.take().unwrap();
    assert_eq!(r.get(id).unwrap().form, None);
    assert_eq!(r.get(id).unwrap().max_hp, Some(40), "back to the main form's numbers");
    r.set_max_hp(&mut c, id, Some(11));
    assert_eq!(c.npc(nid).unwrap().core.max_hp, Some(11)); // main form: edits do go to the NPC now
}

#[test]
fn changing_the_main_stat_block_to_one_of_its_forms_keeps_it_main() {
    let (mut c, nid, human, wolf, bear) = npc_with_forms();
    let n = c.npc_mut(nid).unwrap();
    n.replace_statblock(human, bear);
    assert_eq!(n.statblock, Some(bear));
    assert_eq!(n.statblock_ids(), vec![bear, wolf]);
}

#[test]
fn undoing_old_hits_respects_death_saves() {
    let mut rng = SequenceRng::new(vec![1]);
    // dead stays dead
    let (mut r, id) = downed_pc();
    let cb = r.get_mut(id).unwrap();
    cb.death_failures = 3;
    let hit = cb.apply_hp(HpChangeKind::Damage, "5", &mut rng, 1).unwrap();
    cb.apply_hp(HpChangeKind::Damage, "2", &mut rng, 1).unwrap();
    cb.undo_hp(hit.id);
    assert_eq!(cb.death_state(), Some(DeathState::Dead));
    // undoing the hit that broke a stable character makes them stable again
    let (mut r, id) = downed_pc();
    let cb = r.get_mut(id).unwrap();
    cb.death_successes = 3;
    let hit = cb.apply_hp(HpChangeKind::Damage, "3", &mut rng, 1).unwrap();
    cb.apply_hp(HpChangeKind::TempHp, "4", &mut rng, 1).unwrap();
    cb.undo_hp(hit.id);
    assert_eq!(cb.death_state(), Some(DeathState::Stable));
}

#[test]
fn a_forms_initiative_modifier_doesnt_touch_the_notes() {
    let (mut c, nid, _h, wolf, _b) = npc_with_forms();
    let mut r = RunningEncounter::new("x");
    let id = r.add_from_campaign_as(&c, CombatantSource::Npc(nid), Some(wolf)).unwrap();
    r.get_mut(id).unwrap().notes = "Bit the cleric".into();
    r.set_initiative_mod(&mut c, id, Some(3));
    assert_eq!(r.get(id).unwrap().notes, "Bit the cleric");
    assert_eq!(r.init_source(&c, r.get(id).unwrap()), cm_core::InitSource::Assigned(3));
    r.set_initiative_mod(&mut c, id, None);
    assert_eq!(r.get(id).unwrap().notes, "Bit the cleric");
    // old saves kept it in the notes: still read
    let mut r = RunningEncounter::new("x");
    let g = r.add_adhoc("Rat", None, None);
    r.get_mut(g).unwrap().notes = "init +2".into();
    assert_eq!(r.init_source(&c, r.get(g).unwrap()), cm_core::InitSource::Assigned(2));
}

#[test]
fn two_big_dice_terms_still_roll() {
    assert!(dice::roll("10000d6+10000d6").is_ok());
}

#[test]
fn removing_a_card_from_a_plan_and_renumbering_sessions() {
    let mut c = Campaign::new("x");
    let s = Session {
        number: 3,
        ..Default::default()
    };
    let sid = s.id;
    c.sessions.push(s);
    let card = StoryCard::default();
    let cid = card.id;
    c.story_cards.push(card);
    c.move_card_to_session(cid, Some(3));
    c.unassign_card_from_session(cid, sid);
    assert_eq!(c.card(cid).unwrap().timing, CardTiming::Unscheduled);
    assert!(c.cards_for_session(c.session(sid).unwrap()).is_empty());
    c.move_card_to_session(cid, Some(3));
    c.renumber_session(sid, 4);
    assert_eq!(c.card(cid).unwrap().timing, CardTiming::Session(4));
}

#[test]
fn impossible_prices_become_no_price() {
    assert_eq!(parse_cost_text(&"9".repeat(400)), None);
    assert_eq!(parse_cost(&serde_json::json!({"amount": "1e999", "unit": "gp"})), None);
    assert!(cm_core::loot::shop_price(f64::INFINITY).is_finite());
    assert!(cm_core::loot::shop_price(f64::NAN).is_finite());
}

#[test]
fn re_exporting_to_the_wiki_keeps_your_notes() {
    let tmp = tempfile::tempdir().unwrap();
    let mut v = cm_core::wiki::Vault::open(tmp.path()).unwrap();
    let rel = v.export("People", "Vask", "id-1", "---\ncm_id: id-1\n---\n# Vask\nOld text").unwrap();
    let path = v.abs_path(&rel);
    let mut text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(cm_core::wiki::KEEP_MARKER));
    text.push_str("\nMy own notes about Vask.\n");
    std::fs::write(&path, text).unwrap();
    let mut v = cm_core::wiki::Vault::open(tmp.path()).unwrap();
    v.export("People", "Vask", "id-1", "---\ncm_id: id-1\n---\n# Vask\nNew text").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("New text") && !text.contains("Old text"));
    assert!(text.contains("My own notes about Vask."));
    // a note from an older version (no marker) is kept as a copy before it's replaced
    std::fs::write(tmp.path().join("People/Greta.md"), "---\ncm_id: id-2\n---\nHand-edited").unwrap();
    let mut v = cm_core::wiki::Vault::open(tmp.path()).unwrap();
    v.export("People", "Greta", "id-2", "---\ncm_id: id-2\n---\nFresh").unwrap();
    let backup = std::fs::read_to_string(tmp.path().join("People/Greta (before re-export).md")).unwrap();
    assert!(backup.contains("Hand-edited") && !backup.contains("cm_id"));
    // names starting with a dot would be hidden from the vault
    assert!(!cm_core::wiki::note_rel_path(".hidden", ".38 Special").split('/').any(|p| p.starts_with('.')));
}
