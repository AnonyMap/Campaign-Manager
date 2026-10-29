//! Search across everything in a campaign (cards, NPCs, PCs, factions, sessions...).

use crate::model::{Campaign, Link};

#[derive(Debug, Clone, PartialEq)]
pub struct CampaignHit {
    pub link: Link,
    pub kind: &'static str,
    pub name: String,
    pub score: u32,
    pub context: String,
}

fn score_fields(words: &[String], name: &str, fields: &[&str]) -> Option<(u32, String)> {
    let name_l = name.to_lowercase();
    let mut score = 0;
    let mut context = String::new();
    for w in words {
        let mut hit = false;
        if name_l == *w {
            score += 100;
            hit = true;
        }
        if name_l.contains(w.as_str()) {
            score += 50;
            hit = true;
        }
        for f in fields {
            let fl = f.to_lowercase();
            if let Some(pos) = fl.find(w.as_str()) {
                score += 5;
                hit = true;
                if context.is_empty() {
                    let char_pos = fl[..pos].chars().count();
                    let s: String = f.chars().skip(char_pos.saturating_sub(30)).take(90).collect();
                    context = s.replace('\n', " ");
                }
            }
        }
        if !hit {
            return None;
        }
    }
    Some((score, context))
}

pub fn search(c: &Campaign, query: &str) -> Vec<CampaignHit> {
    let words: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let mut push = |link: Link, kind: &'static str, name: String, fields: &[&str]| {
        if let Some((score, context)) = score_fields(&words, &name, fields) {
            hits.push(CampaignHit {
                link,
                kind,
                name,
                score,
                context,
            });
        }
    };
    for x in &c.story_cards {
        let tags = x.tags.join(" ");
        let quest = x
            .quest
            .as_ref()
            .map(|q| {
                format!(
                    "{} {}",
                    q.objectives.iter().map(|o| o.text.as_str()).collect::<Vec<_>>().join(" "),
                    q.reward_text
                )
            })
            .unwrap_or_default();
        push(
            Link::Card(x.id),
            if x.quest.is_some() { "Quest" } else { "Card" },
            x.display_title(),
            &[&x.body, &tags, &quest],
        );
    }
    for x in &c.npcs {
        let tags = x.core.tags.join(" ");
        push(
            Link::Npc(x.id),
            "NPC",
            x.core.display_name(),
            &[&x.core.notes, &x.core.species, &x.core.class_or_type, &x.role, &x.location, &tags],
        );
    }
    for x in &c.players {
        let tags = x.core.tags.join(" ");
        push(
            Link::Player(x.id),
            "PC",
            x.core.display_name(),
            &[&x.player_name, &x.core.notes, &x.core.species, &x.core.class_or_type, &tags],
        );
    }
    for x in &c.factions {
        push(Link::Faction(x.id), "Faction", x.name.clone(), &[&x.description, &x.goals, &x.notes]);
    }
    for x in &c.encounters {
        push(Link::Encounter(x.id), "Encounter", x.name.clone(), &[&x.description, &x.location, &x.notes]);
    }
    for x in &c.sessions {
        push(Link::Session(x.id), "Session", x.display_title(), &[&x.narrative, &x.recap]);
    }
    for x in &c.threads {
        push(Link::Thread(x.id), "Thread", x.name.clone(), &[&x.description]);
    }
    for x in &c.maps {
        push(Link::Map(x.id), "Map", x.name.clone(), &[&x.notes]);
    }
    for x in &c.locations {
        let tags = x.tags.join(" ");
        push(Link::Location(x.id), "Place", x.display_name(), &[&x.kind, &x.description, &x.notes, &tags]);
    }
    for x in &c.items {
        let tags = x.tags.join(" ");
        push(Link::Item(x.id), "Item", x.name.clone(), &[&x.description, &tags]);
    }
    for x in &c.statblocks {
        push(Link::StatBlock(x.id), "Stat block", x.name(), &[&x.core.notes, &x.core.class_or_type]);
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.name.cmp(&b.name)));
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn finds_across_types() {
        let mut c = Campaign::new("x");
        let mut n = Npc::default();
        n.core.name = "Mira".into();
        n.core.notes = "Smuggles wine through the docks".into();
        c.npcs.push(n);
        c.story_cards.push(StoryCard {
            title: "Dock fire".into(),
            ..Default::default()
        });
        c.factions.push(Faction {
            name: "Dockhands Guild".into(),
            ..Default::default()
        });
        let hits = search(&c, "dock");
        assert_eq!(hits.len(), 3);
        assert!(hits[0].kind == "Card" || hits[0].kind == "Faction");
        assert_eq!(hits.last().unwrap().kind, "NPC");
        assert!(hits.last().unwrap().context.contains("docks"));
        assert!(search(&c, "mira wine").len() == 1);
        assert!(search(&c, "mira guild").is_empty());
        assert!(search(&c, "  ").is_empty());
    }
}
