//! Saving, loading, exporting and importing must never lose or corrupt anything, whatever
//! combination of parts and duplicate handling is chosen.

use std::fs;

use cm_core::transfer::{self, Duplicates, Part};
use cm_core::{Campaign, Link, storage};
use proptest::prelude::*;

/// The sample campaign plus a few extra cross-references (NPC forms, a map with pins, an
/// item in an inventory, a quest card), so every kind of link gets exercised.
fn rich() -> Campaign {
    static RICH: std::sync::OnceLock<Campaign> = std::sync::OnceLock::new();
    RICH.get_or_init(build_rich).clone()
}

fn build_rich() -> Campaign {
    let mut c = cm_core::sample_campaign();
    // a second stat block as another form of the first NPC that has one
    if let Some(sid) = c.statblocks.first().map(|s| s.id) {
        let mut wolf = c.statblocks[0].clone();
        wolf.id = cm_core::new_id();
        wolf.core.name = "Other form".into();
        let wid = wolf.id;
        c.statblocks.push(wolf);
        if let Some(n) = c.npcs.iter_mut().find(|n| n.statblock == Some(sid)) {
            n.add_statblock(wid);
        }
    }
    let map = cm_core::model::CampaignMap {
        name: "Region".into(),
        image: "assets/region.png".into(),
        ..Default::default()
    };
    let mid = map.id;
    c.maps.push(map);
    if let Some(card) = c.story_cards.first_mut() {
        card.map_pins.push(cm_core::model::MapPin { map_id: mid, x: 0.5, y: 0.25 });
    }
    c
}

/// Every reference in the campaign points at something that exists.
fn no_dangling(c: &Campaign) -> Result<(), TestCaseError> {
    let mut copy = c.clone();
    let n = transfer::prune_dangling(&mut copy);
    prop_assert_eq!(n, 0, "{} dangling references", n);
    Ok(())
}

fn subsets() -> impl Strategy<Value = Vec<Part>> {
    prop::sample::subsequence(Part::ALL.to_vec(), 0..=Part::ALL.len())
}

fn dup() -> impl Strategy<Value = Duplicates> {
    prop_oneof![Just(Duplicates::Skip), Just(Duplicates::Replace), Just(Duplicates::KeepBoth)]
}

fn json(c: &Campaign) -> serde_json::Value {
    let mut v = serde_json::to_value(c).unwrap();
    v.as_object_mut().unwrap().remove("modified");
    v
}

#[test]
fn save_and_load_is_exact() {
    let dir = tempfile::tempdir().unwrap();
    let c = rich();
    storage::save(&c, dir.path()).unwrap();
    let back = storage::load(dir.path()).unwrap();
    assert_eq!(json(&back), json(&c));
    // and saving again keeps a backup of the previous save
    storage::save(&back, dir.path()).unwrap();
    let back2 = storage::load(dir.path()).unwrap();
    assert_eq!(json(&back2), json(&c));
}

#[test]
fn damaged_files_are_errors_not_crashes() {
    let dir = tempfile::tempdir().unwrap();
    let c = rich();
    storage::save(&c, dir.path()).unwrap();
    let file = dir.path().join("campaign.json");
    let text = fs::read_to_string(&file).unwrap();
    for cut in [0, 1, 10, text.len() / 3, text.len() / 2, text.len() - 2] {
        let cut = (0..=cut).rev().find(|i| text.is_char_boundary(*i)).unwrap();
        fs::write(&file, &text[..cut]).unwrap();
        let _ = storage::load(dir.path()); // an error (or the backup), never a panic
    }
    for junk in [
        "null",
        "[]",
        "{}",
        "{\"npcs\": 5}",
        "{\"npcs\": [{\"id\": 7}]}",
        "\u{feff}{}",
        "{\"schema_version\": 99999}",
    ] {
        fs::write(&file, junk).unwrap();
        let _ = storage::load(dir.path());
        let _ = storage::from_json(junk);
    }
}

#[test]
fn a_package_round_trips_with_its_images() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Camp");
    let c = rich();
    storage::save(&c, &dir).unwrap();
    fs::write(dir.join("assets/region.png"), b"png bytes").unwrap();
    let pack = tmp.path().join("all.cmpack");
    let report = transfer::export_package(&c, &dir, &Part::ALL, &pack).unwrap();
    assert_eq!(report.missing_images, transfer::asset_paths(&c).len() - 1);
    let src = transfer::Source::open(&pack).unwrap();
    let (new_dir, _) = transfer::new_campaign_from(&src, &Part::ALL, "Copy", tmp.path()).unwrap();
    let copy = storage::load(&new_dir).unwrap();
    for p in Part::ALL {
        assert_eq!(p.count(&copy), p.count(&c), "{p:?}");
    }
    assert_eq!(fs::read(new_dir.join("assets/region.png")).unwrap(), b"png bytes");
    // no half-written leftovers
    assert!(
        fs::read_dir(new_dir.join("assets"))
            .unwrap()
            .all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".partial"))
    );
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 120, ..ProptestConfig::default() })]

    /// Importing any parts into an empty campaign: everything chosen arrives, nothing
    /// dangles, and importing the same again with Skip changes nothing.
    #[test]
    fn importing_any_parts_is_complete_and_clean(parts in subsets(), d in dup()) {
        let src = rich();
        let mut t = Campaign::new("New");
        transfer::merge(&mut t, &src, &parts, d).unwrap();
        no_dangling(&t)?;
        for p in &parts {
            if *p != Part::Rules {
                prop_assert_eq!(p.count(&t), p.count(&src), "{:?} incomplete", p);
            }
        }
        for p in Part::ALL.iter().filter(|p| !parts.contains(p) && **p != Part::Rules) {
            prop_assert_eq!(p.count(&t), 0, "{:?} came along unasked", p);
        }
        let before = json(&t);
        transfer::merge(&mut t, &src, &parts, Duplicates::Skip).unwrap();
        prop_assert_eq!(json(&t), before);
    }

    /// Importing into a campaign that already has the same things.
    #[test]
    fn importing_over_itself(parts in subsets(), d in dup()) {
        let src = rich();
        let mut t = src.clone();
        transfer::merge(&mut t, &src, &parts, d).unwrap();
        no_dangling(&t)?;
        for p in Part::ALL.iter().filter(|p| **p != Part::Rules) {
            let expect = if d == Duplicates::KeepBoth && parts.contains(p) { 2 * p.count(&src) } else { p.count(&src) };
            prop_assert_eq!(p.count(&t), expect, "{:?} with {:?}", p, d);
        }
        // session numbers stay unique
        let mut nums: Vec<u32> = t.sessions.iter().map(|s| s.number).collect();
        let n = nums.len();
        nums.sort();
        nums.dedup();
        prop_assert_eq!(nums.len(), n, "duplicate session numbers");
        // and it still saves and loads
        let back = storage::from_json(&storage::to_json(&t).unwrap()).unwrap();
        prop_assert_eq!(json(&back), json(&t));
    }

    /// Deleting any things, in any order, leaves no reference to them anywhere.
    #[test]
    fn deleting_anything_leaves_nothing_dangling(picks in prop::collection::vec((0usize..10, any::<prop::sample::Index>()), 0..12)) {
        let mut c = rich();
        for (kind, idx) in picks {
            let link = match kind {
                0 if !c.npcs.is_empty() => Link::Npc(c.npcs[idx.index(c.npcs.len())].id),
                1 if !c.players.is_empty() => Link::Player(c.players[idx.index(c.players.len())].id),
                2 if !c.story_cards.is_empty() => Link::Card(c.story_cards[idx.index(c.story_cards.len())].id),
                3 if !c.factions.is_empty() => Link::Faction(c.factions[idx.index(c.factions.len())].id),
                4 if !c.locations.is_empty() => Link::Location(c.locations[idx.index(c.locations.len())].id),
                5 if !c.statblocks.is_empty() => Link::StatBlock(c.statblocks[idx.index(c.statblocks.len())].id),
                6 if !c.encounters.is_empty() => Link::Encounter(c.encounters[idx.index(c.encounters.len())].id),
                7 if !c.sessions.is_empty() => Link::Session(c.sessions[idx.index(c.sessions.len())].id),
                8 if !c.threads.is_empty() => Link::Thread(c.threads[idx.index(c.threads.len())].id),
                9 if !c.maps.is_empty() => Link::Map(c.maps[idx.index(c.maps.len())].id),
                _ => continue,
            };
            c.delete(&link);
            prop_assert!(c.link_name(&link).is_none(), "{:?} still there", link);
            no_dangling(&c)?;
        }
    }
}
