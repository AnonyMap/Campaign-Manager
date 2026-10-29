//! End-to-end tests of DM workflows through the public API (no GUI).

use cm_core::dice::{self, Command, SequenceRng};
use cm_core::encounter::{HealthState, HpChangeKind, RunningEncounter};
use cm_core::wiki::Vault;
use cm_core::*;

/// Plan a session from the ideas pile, then save and reload it.
#[test]
fn plan_session_and_persist() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Camp");
    let mut c = Campaign::new("Test campaign");

    // An empty card is allowed and goes to the ideas pile.
    let empty = StoryCard::default();
    let empty_id = empty.id;
    c.story_cards.push(empty);
    assert_eq!(c.ideas_pile().len(), 1);
    assert_eq!(c.card(empty_id).unwrap().display_title(), "(untitled)");

    let thread = PlotThread {
        name: "Cult".into(),
        ..Default::default()
    };
    let tid = thread.id;
    c.threads.push(thread);
    let card = StoryCard {
        title: "Cultists at the docks".into(),
        threads: vec![tid],
        timing: CardTiming::GameDate(calendar::GameDate { year: 1203, month: 4, day: 2 }),
        ..Default::default()
    };
    let cid = card.id;
    c.story_cards.push(card);
    let s = Session {
        number: c.next_session_number(),
        title: "Docks".into(),
        narrative: "Open at night.".into(),
        ..Default::default()
    };
    let sid = s.id;
    c.sessions.push(s);
    c.assign_card_to_session(cid, sid);
    assert_eq!(c.ideas_pile().len(), 1);

    storage::save(&c, &dir).unwrap();
    let back = storage::load(&dir).unwrap();
    let s = back.session(sid).unwrap();
    assert_eq!(back.cards_for_session(s)[0].title, "Cultists at the docks");
    assert_eq!(back.card(cid).unwrap().threads, vec![tid]);
    assert_eq!(back.backlinks(&Link::Thread(tid)).len(), 1);
    // JSON is human-readable
    let text = std::fs::read_to_string(dir.join("campaign.json")).unwrap();
    assert!(text.contains("\"title\": \"Cultists at the docks\""));
}

/// Dice roller "@Name" notation saves a custom roll that persists on the NPC, and the
/// combat tracker uses it alongside the assigned initiative.
#[test]
fn roll_saved_to_npc_and_used_in_combat() {
    let mut c = Campaign::new("x");
    let mut gob = Npc {
        kind: NpcKind::Monster,
        ..Default::default()
    };
    gob.core.name = "Goblin".into();
    gob.core.max_hp = Some(7);
    gob.core.initiative_mod = Some(2);
    let gid = gob.id;
    c.npcs.push(gob);
    let mut pc = PlayerCharacter::default();
    pc.core.name = "Aria".into();
    pc.core.max_hp = Some(24);
    let pid = pc.id;
    c.players.push(pc);

    // Type a roll with a target, as the DM would in the roller.
    let Command::Roll { label, expression, target } = dice::parse_command("Scimitar: 1d6+2 @gob").unwrap() else {
        panic!()
    };
    let mut rng = SequenceRng::new(vec![4]);
    let rec = dice::roll_to_record(&label, &expression, "", &c.dice.macros, &mut rng).unwrap();
    assert_eq!(rec.total, 6.0);
    c.dice.push(rec);
    let src = c.find_character_by_name(&target.unwrap()).unwrap();
    c.core_for_mut(&src).unwrap().upsert_custom_roll(&label, &expression);
    assert_eq!(c.npc(gid).unwrap().core.custom_rolls[0].notation, "1d6+2");

    // Build and run an encounter.
    let enc = Encounter {
        name: "Ambush".into(),
        entries: vec![
            EncounterEntry {
                source: CombatantSource::Npc(gid),
                count: 2,
                group_initiative: false,
                ..Default::default()
            },
            EncounterEntry {
                source: CombatantSource::Player(pid),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut run = RunningEncounter::from_premade(&enc, &c);
    let ids: Vec<Id> = run.combatants.iter().map(|x| x.id).collect();
    run.set_initiative(ids[2], Some(14));
    let mut rng = SequenceRng::new(vec![10, 18]);
    let rolls = run.roll_npc_initiative(&c, true, &mut rng);
    assert!(rolls.iter().all(|r| r.source == InitSource::Assigned(2)));
    run.start(&c);
    assert_eq!(run.current().unwrap().name, "Goblin 2"); // 18+2 = 20
    // Damage with the calculator, then undo.
    let g1 = run.combatants.iter().find(|x| x.name == "Goblin 1").unwrap().id;
    let ch = run.apply_hp(g1, HpChangeKind::Damage, "2+2", &mut rng).unwrap();
    assert_eq!(ch.note(), "2+2 = 4 damage");
    assert_eq!(run.get(g1).unwrap().health(), HealthState::Bloodied);
    run.get_mut(g1).unwrap().undo_last_hp();
    assert_eq!(run.get(g1).unwrap().current_hp, Some(7));
    // Changing max HP in combat sticks to the NPC record.
    run.set_max_hp(&mut c, g1, Some(9));
    assert_eq!(c.npc(gid).unwrap().core.max_hp, Some(9));
    // The running combat is saved with the campaign.
    c.active_encounter = Some(run);
    let json = storage::to_json(&c).unwrap();
    let back = storage::from_json(&json).unwrap();
    assert_eq!(back.active_encounter.unwrap().combatants.len(), 3);
}

/// Export campaign items into an Obsidian vault and read them back.
#[test]
fn export_to_obsidian_vault() {
    let tmp = tempfile::tempdir().unwrap();
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();
    std::fs::write(vault_dir.join("Millbrook.md"), "# Millbrook\nHome of [[Baron Aldric Vask]].").unwrap();
    let c = sample_campaign();
    let mut v = Vault::open(&vault_dir).unwrap();
    for n in &c.npcs {
        let md = wiki::npc_to_markdown(&c, n);
        v.export("Campaign Manager/NPCs", &n.core.display_name(), &n.id.to_string(), &md).unwrap();
    }
    for f in &c.factions {
        v.export("Campaign Manager/Factions", &f.name, &f.id.to_string(), &wiki::faction_to_markdown(&c, f))
            .unwrap();
    }
    let reopened = Vault::open(&vault_dir).unwrap();
    assert_eq!(reopened.notes.len(), 1 + c.npcs.len() + c.factions.len());
    let baron = reopened.resolve("Baron Aldric Vask").unwrap();
    assert!(baron.tags.contains(&"campaign/npc".to_string()));
    // the pre-existing note now has a backlink target that exists
    assert_eq!(reopened.backlinks(&baron.rel_path).len(), 1);
    let hits = reopened.search("tag:campaign/faction");
    assert_eq!(hits.len(), 1);
}

/// Every advertised syntax example in the help text parses.
#[test]
fn help_examples_parse() {
    for ex in [
        "4d6kh3",
        "4d6dl1",
        "3d4dh1",
        "3d4kl1",
        "2d4r1",
        "2d4r<2",
        "2d4r<=2",
        "2d4r>2",
        "2d4r>=3",
        "2d4x4",
        "2d4x<2",
        "2d4x<=2",
        "2d4x>2",
        "2d4x>=3",
        "2d4cs=4",
        "2d4cs<2",
        "2d4cs<=2",
        "2d4cs>2",
        "2d4cs>=3",
        "2d4ms=4",
        "{2d8, 1d6}",
        "{1d20+7, 10}kh1",
        "floor(1.5)",
        "ceil(1.5)",
        "round(1.5)",
        "avg(8d6)",
        "dmax(8d6)",
        "dmin(8d6)",
        "sign(1d6-3)",
        "abs(1d6-3)",
        "1d20+5",
        "d20",
        "d%",
        "(1+1)d6",
        "2d3d4",
        "2d20kh1",
        "pow(2,3)",
        "max(1d4, 2)",
        "random()",
    ] {
        let e = dice::DiceExpr::parse(ex).unwrap_or_else(|err| panic!("{ex}: {err}"));
        let mut rng = fastrand::Rng::with_seed(9);
        e.roll(&mut rng).unwrap_or_else(|err| panic!("{ex}: {err}"));
    }
}
