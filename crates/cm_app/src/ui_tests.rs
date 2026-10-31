//! Headless tests of the whole app (egui_kittest): every screen draws, and a "monkey" clicks
//! buttons and types awkward text at random on every screen without crashing anything.

use std::path::PathBuf;
use std::sync::OnceLock;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable, by};

use super::*;

const ALL_VIEWS: [View; 21] = [
    View::Home,
    View::Run,
    View::Cards,
    View::Board,
    View::Quests,
    View::Plot,
    View::Maps,
    View::Sessions,
    View::Threads,
    View::Players,
    View::Npcs,
    View::Bestiary,
    View::Factions,
    View::Places,
    View::Items,
    View::Loot,
    View::Encounters,
    View::Combat,
    View::Wiki,
    View::Search,
    View::Settings,
];

/// Settings go to a throwaway folder, never the real one.
fn sandbox() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = std::env::temp_dir().join(format!("cm-ui-tests-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        // SAFETY: set once, before any test reads it, to the same value for every test
        unsafe { std::env::set_var("CAMPAIGN_MANAGER_CONFIG_DIR", d.join("config")) };
        d
    })
}

/// A sample campaign on disk, with a wiki vault and some awkward data.
fn campaign(name: &str) -> PathBuf {
    let dir = sandbox().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let mut c = cm_core::sample_campaign();
    c.description = "Line one\n- [ ] a task\n- **bold** é ’ 🐉\n\n> quote".into();
    if let Some(sid) = c.statblocks.first().map(|s| s.id) {
        let mut other = c.statblocks[0].clone();
        other.id = cm_core::new_id();
        other.core.name = "Other form".into();
        let oid = other.id;
        c.statblocks.push(other);
        if let Some(n) = c.npcs.iter_mut().find(|n| n.statblock == Some(sid)) {
            n.add_statblock(oid);
        }
    }
    let vault = dir.join("vault");
    std::fs::create_dir_all(vault.join("People")).unwrap();
    std::fs::write(vault.join("People/Vask.md"), "---\ntags: [noble]\n---\n# Vask\nSee [[Thornwood]].").unwrap();
    std::fs::write(vault.join("Thornwood.md"), "A forest. #place").unwrap();
    c.settings.wiki_path = Some(vault.to_string_lossy().to_string());
    cm_core::storage::save(&c, &dir).unwrap();
    dir
}

fn harness(dir: PathBuf) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1400.0, 900.0))
        .build_eframe(move |cc| App::new(cc, Some(dir)))
}

const CLICKABLE: [Role; 8] = [
    Role::Button,
    Role::Link,
    Role::CheckBox,
    Role::RadioButton,
    Role::Tab,
    Role::MenuItem,
    Role::ComboBox,
    Role::Switch,
];

/// Things the monkey shouldn't press: they end the test session rather than test anything.
fn off_limits(label: &str) -> bool {
    let l = label.to_lowercase();
    ["quit", "exit", "close campaign", "open in obsidian", "show in folder", "open folder"]
        .iter()
        .any(|x| l.contains(x))
}

#[test]
fn every_screen_draws() {
    let mut h = harness(campaign("screens"));
    h.run_steps(3);
    assert!(h.state().doc.is_some(), "the campaign didn't open");
    for v in ALL_VIEWS {
        h.state_mut().view = v;
        h.run_steps(3);
    }
}

#[test]
fn monkey_clicks_and_types_everywhere() {
    let seed: u64 = std::env::var("MONKEY_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(7);
    let steps: usize = std::env::var("MONKEY_STEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(1200);
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut h = harness(campaign("monkey"));
    h.run_steps(3);
    let texts = ["12", "-3", "2d6+1", "é’🐉 **x** [[Vask]]", "\n- item\n", "", "999999999", "a\tb"];
    let mut trail: Vec<String> = Vec::new();
    let mut seen: std::collections::BTreeMap<String, usize> = Default::default();
    for step in 0..steps {
        if step % 40 == 0 || h.state().doc.is_none() {
            if h.state().doc.is_none() {
                let dir = sandbox().join("monkey");
                h.state_mut().open_campaign(&dir);
            }
            let v = ALL_VIEWS[rng.usize(..ALL_VIEWS.len())];
            trail.push(format!("view {v:?}"));
            h.state_mut().view = v;
            h.run_steps(2);
            continue;
        }
        // what can be clicked or typed into right now
        let (targets, inputs): (Vec<(String, egui::Pos2)>, Vec<(String, egui::Pos2)>) = {
            let mut t = Vec::new();
            let mut i = Vec::new();
            for n in h.query_all(by()) {
                let node = n.accesskit_node();
                if node.is_disabled() || node.is_hidden() {
                    continue;
                }
                let label = node.label().unwrap_or_default();
                let Some(b) = node.bounding_box() else { continue };
                let rect = egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32));
                if !rect.is_positive() || rect.max.x < 0.0 || rect.max.y < 0.0 || rect.min.x > 1400.0 || rect.min.y > 900.0 {
                    continue;
                }
                if CLICKABLE.contains(&node.role()) && !off_limits(&label) {
                    t.push((label, rect.center()));
                } else if matches!(node.role(), Role::TextInput | Role::MultilineTextInput) {
                    i.push((label, rect.center()));
                }
            }
            (t, i)
        };
        let click = |h: &mut Harness<'_, App>, pos: egui::Pos2| {
            h.event(egui::Event::PointerMoved(pos));
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            });
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            });
        };
        if !inputs.is_empty() && rng.u8(..4) == 0 {
            let (label, pos) = inputs[rng.usize(..inputs.len())].clone();
            let text = texts[rng.usize(..texts.len())];
            trail.push(format!("type {text:?} into {label:?}"));
            click(&mut h, pos);
            h.run_steps(1);
            h.event(egui::Event::Text(text.to_string()));
            if rng.bool() {
                h.event(egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                });
            }
        } else if !targets.is_empty() {
            let (label, pos) = targets[rng.usize(..targets.len())].clone();
            trail.push(format!("click {label:?}"));
            *seen.entry(label.clone()).or_default() += 1;
            click(&mut h, pos);
        } else {
            h.event(egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            });
        }
        if trail.len() > 25 {
            trail.remove(0);
        }
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| h.run_steps(2)));
        if let Err(e) = r {
            eprintln!("monkey (seed {seed}) crashed at step {step}. Last actions:\n  {}", trail.join("\n  "));
            std::panic::resume_unwind(e);
        }
    }
    eprintln!("monkey (seed {seed}): {} different things clicked", seen.len());
    if std::env::var("MONKEY_VERBOSE").is_ok() {
        eprintln!("{seen:?}");
    }
    // whatever happened, the campaign still saves and loads
    if let Some(doc) = h.state_mut().doc.as_mut() {
        doc.save().unwrap();
        cm_core::storage::load(&doc.dir).unwrap();
    }
}

/// A campaign far bigger than most: run with
/// `cargo test --release -p campaign-manager big_campaign -- --ignored --nocapture`.
#[test]
#[ignore]
fn big_campaign_frame_times() {
    use std::time::Instant;
    let dir = sandbox().join("big");
    let _ = std::fs::remove_dir_all(&dir);
    let mut c = cm_core::sample_campaign();
    for i in 0..1500 {
        let mut n = cm_core::Npc::default();
        n.core.name = format!("Person {i}");
        n.core.notes = "Some **notes** with a [[link]] and a list:\n- one\n- two\n".repeat(3);
        c.npcs.push(n);
    }
    for i in 0..800 {
        let mut card = cm_core::StoryCard::default();
        card.title = format!("Scene {i}");
        card.body = "A scene with **bold** and _italic_ text.\n".repeat(5);
        card.in_ideas_pile = i % 3 == 0;
        c.story_cards.push(card);
    }
    for i in 0..400 {
        let mut l = cm_core::places::Location::default();
        l.name = format!("Place {i}");
        l.parent = c.locations.first().map(|p| p.id).filter(|_| i % 2 == 0);
        c.locations.push(l);
    }
    for i in 0..2000 {
        c.items.push(cm_core::items::Item::named(
            &format!("Item {i}"),
            cm_core::items::ItemKind::Weapon,
            cm_core::items::Rarity::Common,
            Some(1.0),
        ));
    }
    for i in 0..600 {
        let mut sb = cm_core::bestiary::StatBlock::default();
        sb.core.name = format!("Beast {i}");
        sb.core.cr = format!("{}", i % 20);
        c.statblocks.push(sb);
    }
    let t = Instant::now();
    let json = cm_core::storage::to_json(&c).unwrap();
    eprintln!("campaign.json: {} KB, serialise {:?}", json.len() / 1024, t.elapsed());
    cm_core::storage::save(&c, &dir).unwrap();
    let mut h = harness(dir);
    h.run_steps(3);
    for v in ALL_VIEWS {
        h.state_mut().view = v;
        h.run_steps(2);
        let t = Instant::now();
        h.run_steps(10);
        eprintln!("{v:?}: {:.1} ms/frame", t.elapsed().as_secs_f64() * 100.0);
    }
    // typing: snapshotting for undo
    let t = Instant::now();
    for _ in 0..10 {
        let doc = h.state_mut().doc.as_mut().unwrap();
        doc.touched = true;
        doc.last_check = -10.0;
        doc.track(false);
    }
    eprintln!("change tracking: {:.1} ms per check", t.elapsed().as_secs_f64() * 100.0);
}
