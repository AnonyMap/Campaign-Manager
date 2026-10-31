//! The combat tracker under random sequences of everything a DM can do at the table.
//! After every step: no panic, the turn points at someone, references are valid, and
//! Next followed by Back puts the fight back exactly as it was.

use cm_core::dice::SequenceRng;
use cm_core::encounter::*;
use cm_core::{Campaign, CombatantSource, Id, Npc, PlayerCharacter, new_id};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    AddPc(usize),
    AddNpc(bool),
    AddBlock(u32, bool),
    AddAdhoc,
    Summon(usize),
    Remove(usize),
    Next,
    Back,
    NextBack,
    Start,
    RollAll,
    SetInit(usize, i32),
    Hp(usize, u8, String),
    Group(Vec<usize>, u8, String),
    UndoLast(usize),
    UndoOldest(usize),
    Cond(usize, u8, Option<u32>),
    Uncond(usize, u8),
    Form(usize, bool),
    Drag(usize, Option<usize>),
    Link(usize, usize),
    Unlink(usize),
    DeathSave(usize),
    MaxHp(usize, Option<i32>),
}

fn amount() -> impl Strategy<Value = String> {
    prop_oneof![
        (0i32..40).prop_map(|n| n.to_string()),
        Just("2d6+3".to_string()),
        Just("0".to_string()),
        Just("999999999".to_string()),
        Just("-5".to_string()),
        Just("1d4-10".to_string()),
        Just("".to_string()),
        Just("abc".to_string()),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    let i = 0usize..12;
    prop_oneof![
        (0usize..3).prop_map(Op::AddPc),
        any::<bool>().prop_map(Op::AddNpc),
        (1u32..5, any::<bool>()).prop_map(|(n, t)| Op::AddBlock(n, t)),
        Just(Op::AddAdhoc),
        i.clone().prop_map(Op::Summon),
        i.clone().prop_map(Op::Remove),
        Just(Op::Next),
        Just(Op::Next),
        Just(Op::Next),
        Just(Op::Back),
        Just(Op::NextBack),
        Just(Op::Start),
        Just(Op::RollAll),
        (i.clone(), -5i32..30).prop_map(|(a, b)| Op::SetInit(a, b)),
        (i.clone(), 0u8..4, amount()).prop_map(|(a, k, s)| Op::Hp(a, k, s)),
        (prop::collection::vec(0usize..12, 0..4), 0u8..4, amount()).prop_map(|(v, k, s)| Op::Group(v, k, s)),
        i.clone().prop_map(Op::UndoLast),
        i.clone().prop_map(Op::UndoOldest),
        (i.clone(), 0u8..5, prop::option::of(0u32..4)).prop_map(|(a, c, r)| Op::Cond(a, c, r)),
        (i.clone(), 0u8..5).prop_map(|(a, c)| Op::Uncond(a, c)),
        (i.clone(), any::<bool>()).prop_map(|(a, f)| Op::Form(a, f)),
        (i.clone(), prop::option::of(0usize..12)).prop_map(|(a, b)| Op::Drag(a, b)),
        (i.clone(), i.clone()).prop_map(|(a, b)| Op::Link(a, b)),
        i.clone().prop_map(Op::Unlink),
        i.clone().prop_map(Op::DeathSave),
        (i, prop::option::of(-3i32..60)).prop_map(|(a, m)| Op::MaxHp(a, m)),
    ]
}

const CONDS: [&str; 5] = ["Poisoned", "Exhaustion", "Invisible", "Incapacitated", "Hexed"];

struct World {
    c: Campaign,
    pcs: Vec<Id>,
    npc: Id,
    blocks: Vec<Id>,
}

fn world() -> World {
    let mut c = Campaign::new("x");
    let mut pcs = Vec::new();
    for name in ["Ana", "Bo", "Cy"] {
        let mut p = PlayerCharacter::default();
        p.core.name = name.into();
        p.core.max_hp = Some(20);
        p.core.initiative_mod = Some(2);
        pcs.push(p.id);
        c.players.push(p);
    }
    let mut blocks = Vec::new();
    for (name, hp, ac) in [("Goblin", 7, 15), ("Dragon", 180, 18)] {
        let mut sb = cm_core::bestiary::StatBlock::default();
        sb.core.name = name.into();
        sb.core.max_hp = Some(hp);
        sb.core.ac = Some(ac);
        blocks.push(sb.id);
        c.statblocks.push(sb);
    }
    let mut n = Npc::default();
    n.core.name = "Vex".into();
    n.add_statblock(blocks[0]);
    n.add_statblock(blocks[1]);
    let npc = n.id;
    c.npcs.push(n);
    World { c, pcs, npc, blocks }
}

/// What Back must restore after Next.
fn turn_state(r: &RunningEncounter) -> (u32, usize, bool, Vec<(Id, Vec<(String, Option<u32>)>)>) {
    (
        r.round,
        r.turn,
        r.started,
        r.combatants
            .iter()
            .map(|c| (c.id, c.conditions.iter().map(|x| (x.name.clone(), x.rounds_remaining)).collect()))
            .collect(),
    )
}

fn check(r: &RunningEncounter) -> Result<(), TestCaseError> {
    let ids: std::collections::HashSet<Id> = r.combatants.iter().map(|c| c.id).collect();
    prop_assert_eq!(ids.len(), r.combatants.len(), "duplicate ids");
    if !r.combatants.is_empty() {
        prop_assert!(r.turn < r.combatants.len(), "turn {} of {}", r.turn, r.combatants.len());
    }
    for c in &r.combatants {
        prop_assert!(c.temp_hp >= 0);
        prop_assert!(c.death_failures <= 3 && c.death_successes <= 3);
        if let Some(s) = c.summoned_by {
            prop_assert!(ids.contains(&s), "summoned by someone who left");
        }
        if let Some(hp) = c.current_hp {
            prop_assert!(hp.abs() <= HP_LIMIT, "hp {}", hp);
        }
        for x in &c.conditions {
            prop_assert!(x.level <= 6);
        }
    }
    // summons come straight after their summoner (or another of its summons)
    for (k, c) in r.combatants.iter().enumerate() {
        if let Some(s) = c.summoned_by {
            let pos = r.combatants.iter().position(|x| x.id == s).unwrap();
            prop_assert!(pos < k, "summon listed before its summoner");
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 400, max_shrink_iters: 4000, ..ProptestConfig::default() })]

    #[test]
    fn random_fights_stay_consistent(ops in prop::collection::vec(op(), 1..60), seed in prop::collection::vec(1u32..21, 8)) {
        let mut w = world();
        let mut r = RunningEncounter::new("Fight");
        let mut rng = SequenceRng::new(seed);
        let pick = |r: &RunningEncounter, i: usize| r.combatants.get(i % r.combatants.len().max(1)).map(|c| c.id);
        for o in ops {
            match o {
                Op::AddPc(i) => {
                    r.add_from_campaign(&w.c, CombatantSource::Player(w.pcs[i]));
                }
                Op::AddNpc(dragon) => {
                    r.add_with(&w.c, CombatantSource::Npc(w.npc), None, &AddOptions { count: 1, form: dragon.then_some(w.blocks[1]), ..Default::default() });
                }
                Op::AddBlock(n, together) => {
                    r.add_with(&w.c, CombatantSource::StatBlock(w.blocks[0]), None, &AddOptions { count: n, roll_together: together, ..Default::default() });
                }
                Op::AddAdhoc => {
                    r.add_adhoc("Rat", Some(3), Some(10));
                }
                Op::Summon(i) => {
                    if let Some(s) = pick(&r, i) {
                        r.add_with(&w.c, CombatantSource::AdHoc, Some(("Spirit", Some(10), Some(12))), &AddOptions { count: 1, summoned_by: Some(s), ..Default::default() });
                        r.sort_by_initiative(&w.c);
                    }
                }
                Op::Remove(i) => {
                    if let Some(id) = pick(&r, i) {
                        r.remove(id);
                    }
                }
                Op::Next => r.next_turn(),
                Op::Back => r.prev_turn(),
                Op::NextBack => {
                    if r.started && !r.combatants.is_empty() {
                        let before = turn_state(&r);
                        r.next_turn();
                        r.prev_turn();
                        prop_assert_eq!(turn_state(&r), before, "Next then Back changed the fight");
                    }
                }
                Op::Start => r.start(&w.c),
                Op::RollAll => {
                    r.roll_npc_initiative(&w.c, false, &mut rng);
                }
                Op::SetInit(i, v) => {
                    if let Some(id) = pick(&r, i) {
                        r.set_initiative(id, Some(v));
                        r.sort_by_initiative(&w.c);
                    }
                }
                Op::Hp(i, k, s) => {
                    if let Some(id) = pick(&r, i) {
                        let kind = [HpChangeKind::Damage, HpChangeKind::Heal, HpChangeKind::TempHp, HpChangeKind::Set][k as usize];
                        let _ = r.apply_hp(id, kind, &s, &mut rng);
                    }
                }
                Op::Group(v, k, s) => {
                    let targets: Vec<(Id, Portion)> = v.iter().filter_map(|i| pick(&r, *i)).enumerate().map(|(j, id)| (id, Portion::ALL[j % 4])).collect();
                    let kind = [HpChangeKind::Damage, HpChangeKind::Heal, HpChangeKind::TempHp, HpChangeKind::Damage][k as usize];
                    if let Ok((g, _)) = r.apply_hp_group(&targets, kind, &s, &mut rng) {
                        if k == 3 {
                            r.undo_group(g);
                        }
                    }
                }
                Op::UndoLast(i) => {
                    if let Some(id) = pick(&r, i) {
                        r.get_mut(id).unwrap().undo_last_hp();
                    }
                }
                Op::UndoOldest(i) => {
                    if let Some(id) = pick(&r, i) {
                        let c = r.get_mut(id).unwrap();
                        if let Some(first) = c.hp_log.first().map(|h| h.id) {
                            c.undo_hp(first);
                        }
                    }
                }
                Op::Cond(i, n, rounds) => {
                    if let Some(id) = pick(&r, i) {
                        r.get_mut(id).unwrap().add_condition(CONDS[n as usize], rounds);
                    }
                }
                Op::Uncond(i, n) => {
                    if let Some(id) = pick(&r, i) {
                        r.get_mut(id).unwrap().remove_condition(CONDS[n as usize]);
                    }
                }
                Op::Form(i, dragon) => {
                    if let Some(id) = pick(&r, i) {
                        r.switch_form(&w.c, id, dragon.then_some(w.blocks[1]));
                    }
                }
                Op::Drag(i, before) => {
                    if let Some(id) = pick(&r, i) {
                        let b = before.and_then(|b| pick(&r, b));
                        r.move_before(&w.c, id, b);
                    }
                }
                Op::Link(a, b) => {
                    if let (Some(x), Some(y)) = (pick(&r, a), pick(&r, b)) {
                        r.link_initiative(&[x, y]);
                    }
                }
                Op::Unlink(i) => {
                    if let Some(id) = pick(&r, i) {
                        r.unlink_initiative(id);
                    }
                }
                Op::DeathSave(i) => {
                    if let Some(id) = pick(&r, i) {
                        r.get_mut(id).unwrap().roll_death_save(&mut rng);
                    }
                }
                Op::MaxHp(i, m) => {
                    if let Some(id) = pick(&r, i) {
                        r.set_max_hp(&mut w.c, id, m);
                    }
                }
            }
            check(&r)?;
            let _ = r.current();
            let _ = r.up_next();
            let _ = r.turn_position();
            let _ = r.to_premade();
            let _ = estimate_difficulty(&r.to_premade(), &w.c, &[3, 3]);
        }
        let _ = new_id();
    }
}
