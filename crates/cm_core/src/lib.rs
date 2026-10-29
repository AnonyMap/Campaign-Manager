//! Campaign Manager core: data model, persistence, dice engine and game logic for a
//! Dungeon Master's campaign tool. The GUI lives in the `cm_app` crate; everything here is
//! UI-independent and unit tested.

pub mod bestiary;
pub mod calendar;
pub mod conditions;
pub mod dice;
pub mod encounter;
pub mod items;
pub mod loot;
pub mod model;
pub mod names;
pub mod places;
pub mod search;
pub mod storage;
pub mod transfer;
pub mod wiki;

pub use model::*;

/// Application name (working title).
pub const APP_NAME: &str = "Campaign Manager";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Build a small sample campaign, used for the "New sample campaign" menu item and tests.
pub fn sample_campaign() -> Campaign {
    use calendar::GameDate;
    let mut c = Campaign::new("The Crimson Crown (sample)");
    c.description = "A sample campaign showing what each section can hold. Delete anything you like.".into();
    c.settings.game_calendar.era = "AR".into();
    c.settings.game_calendar.current = GameDate { year: 1203, month: 3, day: 12 };

    let thread = PlotThread {
        name: "The Baron's secret".into(),
        description: "Baron Vask is funding the bandits to justify new taxes.".into(),
        color: Some([180, 60, 60]),
        ..Default::default()
    };
    let thread2 = PlotThread {
        name: "The missing miller".into(),
        color: Some([60, 110, 180]),
        ..Default::default()
    };
    let faction = Faction {
        name: "Red Hand Bandits".into(),
        description: "Ex-soldiers turned highwaymen.".into(),
        goals: "- Get paid\n- Stay hidden in the Thornwood".into(),
        attitude: Attitude::Hostile,
        color: Some([170, 40, 40]),
        ..Default::default()
    };
    let mut baron = Npc::default();
    baron.core.name = "Baron Aldric Vask".into();
    baron.core.species = "Human".into();
    baron.core.class_or_type = "Humanoid (noble)".into();
    baron.core.ac = Some(15);
    baron.core.max_hp = Some(27);
    baron.core.hp_formula = "5d8+5".into();
    baron.core.cr = "1".into();
    baron.core.abilities = AbilityScores {
        str_: Some(11),
        dex: Some(12),
        con: Some(12),
        int: Some(14),
        wis: Some(13),
        cha: Some(16),
    };
    baron.core.custom_rolls.push(CustomRoll {
        label: "Rapier".into(),
        notation: "1d20+3".into(),
        ..Default::default()
    });
    baron.core.custom_rolls.push(CustomRoll {
        label: "Rapier damage".into(),
        notation: "1d8+1".into(),
        ..Default::default()
    });
    baron.core.notes = "Smiles too much. Owes money to the **Red Hand**.".into();
    baron.role = "Lord of Millbrook".into();
    baron.location = "Millbrook Keep".into();

    let mut bandit = Npc::default();
    bandit.kind = NpcKind::Monster;
    bandit.core.name = "Bandit".into();
    bandit.core.class_or_type = "Humanoid".into();
    bandit.core.ac = Some(12);
    bandit.core.max_hp = Some(11);
    bandit.core.hp_formula = "2d8+2".into();
    bandit.core.initiative_mod = Some(1);
    bandit.core.cr = "1/8".into();
    bandit.core.custom_rolls.push(CustomRoll {
        label: "Scimitar".into(),
        notation: "1d20+3".into(),
        ..Default::default()
    });
    bandit.core.custom_rolls.push(CustomRoll {
        label: "Scimitar damage".into(),
        notation: "1d6+1".into(),
        ..Default::default()
    });
    bandit.factions.push(faction.id);

    let mut captain = Npc::default();
    captain.kind = NpcKind::Monster;
    captain.core.name = "Bandit Captain".into();
    captain.core.ac = Some(15);
    captain.core.max_hp = Some(52);
    captain.core.hp_formula = "8d8+16".into();
    captain.core.initiative_mod = Some(3);
    captain.core.cr = "2".into();
    captain.factions.push(faction.id);

    let mut pc = PlayerCharacter::default();
    pc.player_name = "Sam".into();
    pc.core.name = "Brielle Ashdown".into();
    pc.core.species = "Half-Elf".into();
    pc.core.class_or_type = "Ranger".into();
    pc.core.level = Some(3);
    pc.core.ac = Some(15);
    pc.core.max_hp = Some(28);
    pc.core.abilities.dex = Some(16);
    pc.core.passive_perception = Some(14);
    let mut pc2 = PlayerCharacter::default();
    pc2.player_name = "Jo".into();
    pc2.core.name = "Tordek Fireforge".into();
    pc2.core.species = "Dwarf".into();
    pc2.core.class_or_type = "Cleric".into();
    pc2.core.level = Some(3);
    pc2.core.ac = Some(18);
    pc2.core.max_hp = Some(27);
    pc2.core.abilities.dex = Some(10);

    // A stat block from the bundled SRD, used directly in the encounter and by a named NPC.
    let wolf = bestiary::srd_statblocks().into_iter().find(|s| s.name() == "Wolf");
    let mut grey = Npc::default();
    grey.kind = NpcKind::Monster;
    grey.core.name = "Old Grey".into();
    grey.role = "The bandit captain's wolf".into();
    grey.core.notes = "Scarred muzzle, one blind eye. Fights to the death if the captain falls.".into();
    grey.statblock = wolf.as_ref().map(|w| w.id);
    grey.factions.push(faction.id);

    let mut entries = vec![
        EncounterEntry {
            source: CombatantSource::Npc(bandit.id),
            count: 4,
            ..Default::default()
        },
        EncounterEntry {
            source: CombatantSource::Npc(captain.id),
            count: 1,
            ..Default::default()
        },
    ];
    if let Some(w) = &wolf {
        entries.push(EncounterEntry {
            source: CombatantSource::StatBlock(w.id),
            count: 2,
            ..Default::default()
        });
    }
    let enc = Encounter {
        name: "Ambush at the old bridge".into(),
        location: "Thornwood road".into(),
        description: "Bandits hide under the bridge and in the trees.".into(),
        entries,
        ..Default::default()
    };
    let secret = StoryCard {
        title: "Who really wrote the letter".into(),
        body: "The seal is the Baron's, but the handwriting is his steward **Mirela**'s. She is skimming the bandits' payments.".into(),
        status: CardStatus::Planned,
        in_ideas_pile: false,
        threads: vec![thread.id],
        secret: true,
        ..Default::default()
    };

    let card1 = StoryCard {
        title: "Bandits on the road".into(),
        body: "The party is ambushed at the old bridge.\n\n- The captain carries a letter sealed with the Baron's crest.\n- If captured, a bandit begs for mercy and names [[Baron Aldric Vask]].".into(),
        status: CardStatus::Planned,
        in_ideas_pile: false,
        threads: vec![thread.id],
        timing: CardTiming::Session(1),
        links: vec![Link::Encounter(enc.id), Link::Faction(faction.id), Link::Card(secret.id)],
        ..Default::default()
    };
    let card2 = StoryCard {
        title: "The Baron's feast".into(),
        body: "Vask invites the heroes to dinner to find out what they know.".into(),
        threads: vec![thread.id],
        timing: CardTiming::GameDate(GameDate { year: 1203, month: 3, day: 20 }),
        links: vec![Link::Npc(baron.id), Link::Card(card1.id)],
        ..Default::default()
    };
    let card3 = StoryCard {
        title: String::new(),
        body: "What if the miller faked his own disappearance?".into(),
        threads: vec![thread2.id],
        ..Default::default()
    };
    let session = Session {
        number: 1,
        title: "Into the Thornwood".into(),
        narrative: "Open in the tavern. Hook: the miller's daughter asks for help.\n\nThen the road and the ambush.".into(),
        card_ids: vec![card1.id],
        encounter_ids: vec![enc.id],
        ..Default::default()
    };
    let session2 = Session {
        number: 2,
        title: "Dinner with the Baron".into(),
        ..Default::default()
    };

    // A village with an inn and a potion shop, and a quest from the miller's daughter.
    let mut wenna = Npc::default();
    wenna.core.name = "Wenna Brook".into();
    wenna.core.species = "Human".into();
    wenna.role = "The missing miller's daughter".into();
    wenna.location = "Millbrook".into();
    let mut hesk = Npc::default();
    hesk.core.name = "Hesk Tallowmere".into();
    hesk.core.species = "Halfling".into();
    hesk.role = "Herbalist, sells potions".into();
    hesk.location = "Millbrook".into();
    let village = places::Location {
        name: "Millbrook".into(),
        kind: "Village".into(),
        description: "A sleepy mill village on the edge of the Thornwood.".into(),
        npcs: vec![wenna.id],
        ..Default::default()
    };
    let inn = places::Location {
        name: "The Drowsy Badger".into(),
        kind: "Inn".into(),
        parent: Some(village.id),
        description: "Low beams, a smoky hearth and the best stew for ten miles.".into(),
        ..Default::default()
    };
    let lib = items::srd_items();
    let mut rng = fastrand::Rng::with_seed(1203);
    let shop = places::Location {
        name: "Hesk's Remedies".into(),
        kind: "Potion shop".into(),
        parent: Some(village.id),
        npcs: vec![hesk.id],
        shop: Some(places::Shop {
            theme: loot::ShopTheme::Alchemist,
            quality: loot::ShopQuality::Poor,
            stock: loot::stock_shop(&lib, loot::ShopTheme::Alchemist, loot::ShopQuality::Poor, 6, &mut rng),
            keeper: Some(hesk.id),
        }),
        ..Default::default()
    };
    let healing = lib.iter().find(|i| i.name == "Potion of Healing").cloned();
    let quest = StoryCard {
        title: "Find the missing miller".into(),
        body: "Old Tam Brook went to the Thornwood for timber three days ago and never came back.".into(),
        status: CardStatus::InPlay,
        in_ideas_pile: false,
        threads: vec![thread2.id],
        links: vec![Link::Location(village.id)],
        quest: Some(Quest {
            status: QuestStatus::Active,
            giver: Some(Link::Npc(wenna.id)),
            objectives: vec![
                Objective {
                    text: "Search the mill".into(),
                    done: true,
                },
                Objective {
                    text: "Follow the cart tracks into the Thornwood".into(),
                    done: false,
                },
                Objective {
                    text: "Bring Tam home".into(),
                    done: false,
                },
            ],
            reward_coins: items::Coins::gp(50),
            reward_items: healing.into_iter().collect(),
            reward_text: "Free bread at the mill for life".into(),
            started_session: Some(1),
            ..Default::default()
        }),
        ..Default::default()
    };

    c.threads = vec![thread, thread2];
    c.factions = vec![faction];
    c.locations = vec![village, inn, shop];
    c.npcs = vec![baron, bandit, captain, grey, wenna, hesk];
    c.statblocks = wolf.into_iter().collect();
    c.players = vec![pc, pc2];
    c.encounters = vec![enc];
    c.story_cards = vec![card1, card2, card3, secret, quest];
    c.sessions = vec![session, session2];
    c
}

#[cfg(test)]
mod tests {
    #[test]
    fn sample_is_consistent() {
        let c = super::sample_campaign();
        let json = super::storage::to_json(&c).unwrap();
        let back = super::storage::from_json(&json).unwrap();
        assert_eq!(back.story_cards.len(), 5);
        assert_eq!(back.npcs.len(), 6);
        assert_eq!(back.quests().len(), 1);
        assert_eq!(back.child_locations(back.locations[0].id).len(), 2);
        assert!(!back.locations[2].shop.as_ref().unwrap().stock.is_empty());
        assert_eq!(back.statblocks.len(), 1);
        assert_eq!(back.session_secrets(&back.sessions[0]).len(), 1);
        let s = back.sessions[0].clone();
        assert_eq!(back.cards_for_session(&s).len(), 1);
        let est = super::encounter::estimate_difficulty(&back.encounters[0], &back, &[3, 3]);
        assert_eq!(est.monster_xp, 4 * 25 + 450 + 2 * 50);
        let run = super::encounter::RunningEncounter::from_premade(&back.encounters[0], &back);
        assert_eq!(run.combatants.len(), 7);
        assert!(run.combatants.iter().any(|c| c.name == "Wolf 2" && c.max_hp == Some(11)));
    }
}
