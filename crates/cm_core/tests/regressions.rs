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
