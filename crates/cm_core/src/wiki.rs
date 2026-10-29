//! Obsidian vault support: scan a vault folder of Markdown notes, parse front matter,
//! tags and `[[wiki links]]`, search, create notes, and export campaign entities to notes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::{Campaign, CardTiming, CombatantSource, Encounter, Faction, Npc, PlayerCharacter, StoryCard, fmt_mod};
use crate::storage::sanitize_file_name;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Note {
    /// Path relative to the vault root, with forward slashes ("NPCs/Baron Vask.md").
    pub rel_path: String,
    /// File stem ("Baron Vask"), which is how Obsidian resolves `[[links]]`.
    pub title: String,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    /// Targets of `[[links]]` in the note (without `|alias` or `#heading`).
    pub links: Vec<String>,
    pub content: String,
    pub modified: Option<std::time::SystemTime>,
}

impl Note {
    pub fn folder(&self) -> &str {
        match self.rel_path.rfind('/') {
            Some(i) => &self.rel_path[..i],
            None => "",
        }
    }
    /// Note body without front matter.
    pub fn body(&self) -> &str {
        split_front_matter(&self.content).1
    }
}

#[derive(Debug, Clone, Default)]
pub struct Vault {
    pub root: PathBuf,
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub rel_path: String,
    pub title: String,
    pub score: u32,
    pub snippet: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WikiError {
    #[error("vault folder not found: {0}")]
    NoVault(PathBuf),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("a note named '{0}' already exists")]
    Exists(String),
}

/// Split `---\nyaml\n---\nbody` into (front matter, body).
pub fn split_front_matter(text: &str) -> (Option<&str>, &str) {
    let t = text.trim_start_matches('\u{feff}');
    let Some(rest) = t.strip_prefix("---") else {
        return (None, t);
    };
    let rest = rest.trim_start_matches([' ', '\t']);
    let Some(rest) = rest.strip_prefix('\n').or_else(|| rest.strip_prefix("\r\n")) else {
        return (None, t);
    };
    // find closing --- on its own line
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            let fm = &rest[..offset];
            let body = &rest[offset + line.len()..];
            return (Some(fm), body);
        }
        offset += line.len();
    }
    (None, t)
}

/// Minimal YAML front-matter reader for the keys Obsidian cares about (tags, aliases).
/// Handles `key: value`, `key: [a, b]`, and block lists (`key:\n  - a`).
pub fn parse_front_matter(fm: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut current: Option<String> = None;
    let clean = |s: &str| s.trim().trim_matches('"').trim_matches('\'').trim().to_string();
    for line in fm.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let trimmed = line.trim_start();
        if let Some(item) = trimmed.strip_prefix("- ") {
            if let Some(k) = &current {
                out.entry(k.clone()).or_default().push(clean(item));
            }
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            if line.starts_with(' ') || line.starts_with('\t') {
                continue; // nested mapping we don't need
            }
            let key = k.trim().to_lowercase();
            let v = v.trim();
            current = Some(key.clone());
            let entry = out.entry(key).or_default();
            if v.is_empty() {
                continue;
            }
            if v.starts_with('[') && v.ends_with(']') {
                for item in v[1..v.len() - 1].split(',') {
                    let c = clean(item);
                    if !c.is_empty() {
                        entry.push(c);
                    }
                }
            } else {
                entry.push(clean(v));
            }
        }
    }
    out
}

/// Extract `[[target|alias]]` / `![[embed]]` targets.
pub fn extract_wikilinks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else { break };
        let inner = &after[..end];
        let target = inner.split('|').next().unwrap_or("").split('#').next().unwrap_or("").trim();
        if !target.is_empty() && !target.contains('\n') {
            out.push(target.to_string());
        }
        rest = &after[end + 2..];
    }
    out
}

/// Extract inline `#tags` (not headings, not inside words, not pure numbers).
pub fn extract_inline_tags(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_code = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '#' && (i == 0 || chars[i - 1].is_whitespace()) {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_alphanumeric() || matches!(chars[j], '_' | '-' | '/')) {
                    j += 1;
                }
                let tag: String = chars[i + 1..j].iter().collect();
                if !tag.is_empty() && !tag.chars().all(|c| c.is_ascii_digit()) {
                    out.push(tag);
                }
                i = j;
            } else {
                i += 1;
            }
        }
    }
    out
}

pub fn parse_note(rel_path: &str, content: String) -> Note {
    let title = rel_path.rsplit('/').next().unwrap_or(rel_path).trim_end_matches(".md").to_string();
    let (fm, body) = split_front_matter(&content);
    let mut tags = BTreeSet::new();
    let mut aliases = Vec::new();
    if let Some(fm) = fm {
        let map = parse_front_matter(fm);
        for k in ["tags", "tag"] {
            for t in map.get(k).into_iter().flatten() {
                for t in t.split_whitespace() {
                    tags.insert(t.trim_start_matches('#').to_string());
                }
            }
        }
        for k in ["aliases", "alias"] {
            aliases.extend(map.get(k).into_iter().flatten().cloned());
        }
    }
    for t in extract_inline_tags(body) {
        tags.insert(t);
    }
    let links = extract_wikilinks(body);
    Note {
        rel_path: rel_path.to_string(),
        title,
        aliases,
        tags: tags.into_iter().collect(),
        links,
        content,
        modified: None,
    }
}

impl Vault {
    /// Scan a folder for `.md` notes, skipping `.obsidian`, `.trash` and other dot-folders.
    pub fn open(root: &Path) -> Result<Vault, WikiError> {
        if !root.is_dir() {
            return Err(WikiError::NoVault(root.to_path_buf()));
        }
        let mut notes = Vec::new();
        let walker = walkdir::WalkDir::new(root).follow_links(false).into_iter().filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            e.depth() == 0 || !(name.starts_with('.') || name == "node_modules")
        });
        for entry in walker.flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let p = entry.path();
            if p.extension().map(|e| e.eq_ignore_ascii_case("md")).unwrap_or(false) {
                let rel = rel_string(root, p);
                let content = fs::read_to_string(p).unwrap_or_default();
                let mut note = parse_note(&rel, content);
                note.modified = entry.metadata().ok().and_then(|m| m.modified().ok());
                notes.push(note);
            }
        }
        notes.sort_by(|a, b| a.rel_path.to_lowercase().cmp(&b.rel_path.to_lowercase()));
        Ok(Vault {
            root: root.to_path_buf(),
            notes,
        })
    }

    pub fn reload(&mut self) -> Result<(), WikiError> {
        *self = Vault::open(&self.root.clone())?;
        Ok(())
    }

    pub fn note(&self, rel_path: &str) -> Option<&Note> {
        self.notes.iter().find(|n| n.rel_path == rel_path)
    }

    /// Resolve a `[[link]]` target the way Obsidian does: exact path, then file name, then alias.
    pub fn resolve(&self, target: &str) -> Option<&Note> {
        let t = target.trim().trim_end_matches(".md").replace('\\', "/");
        let tl = t.to_lowercase();
        self.notes
            .iter()
            .find(|n| n.rel_path.trim_end_matches(".md").to_lowercase() == tl)
            .or_else(|| self.notes.iter().find(|n| n.title.to_lowercase() == tl))
            .or_else(|| {
                // path suffix ("Places/Town" matches "World/Places/Town.md")
                self.notes
                    .iter()
                    .find(|n| n.rel_path.trim_end_matches(".md").to_lowercase().ends_with(&format!("/{tl}")))
            })
            .or_else(|| self.notes.iter().find(|n| n.aliases.iter().any(|a| a.to_lowercase() == tl)))
    }

    /// Notes that link to `rel_path`.
    pub fn backlinks(&self, rel_path: &str) -> Vec<&Note> {
        let Some(target) = self.note(rel_path) else { return Vec::new() };
        self.notes
            .iter()
            .filter(|n| n.rel_path != rel_path)
            .filter(|n| n.links.iter().any(|l| self.resolve(l).map(|r| r.rel_path == target.rel_path).unwrap_or(false)))
            .collect()
    }

    /// Distinct folders (for a tree view), sorted.
    pub fn folders(&self) -> Vec<String> {
        let mut set = BTreeSet::new();
        for n in &self.notes {
            let mut f = n.folder();
            while !f.is_empty() {
                set.insert(f.to_string());
                f = match f.rfind('/') {
                    Some(i) => &f[..i],
                    None => "",
                };
            }
        }
        set.into_iter().collect()
    }

    pub fn all_tags(&self) -> Vec<(String, usize)> {
        let mut m: BTreeMap<String, usize> = BTreeMap::new();
        for n in &self.notes {
            for t in &n.tags {
                *m.entry(t.clone()).or_default() += 1;
            }
        }
        m.into_iter().collect()
    }

    /// Case-insensitive search. All words must match (title, alias, tag or content).
    /// `tag:foo` restricts to a tag; `path:folder` restricts to a folder.
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        let mut words = Vec::new();
        let mut tag_filters = Vec::new();
        let mut path_filters = Vec::new();
        for w in query.split_whitespace() {
            let lw = w.to_lowercase();
            if let Some(t) = lw.strip_prefix("tag:").or_else(|| lw.strip_prefix('#')) {
                if !t.is_empty() {
                    tag_filters.push(t.to_string());
                }
            } else if let Some(p) = lw.strip_prefix("path:") {
                path_filters.push(p.to_string());
            } else {
                words.push(lw);
            }
        }
        let mut hits = Vec::new();
        for n in &self.notes {
            let tags_l: Vec<String> = n.tags.iter().map(|t| t.to_lowercase()).collect();
            if !tag_filters.iter().all(|t| tags_l.iter().any(|nt| nt == t || nt.starts_with(&format!("{t}/")))) {
                continue;
            }
            let path_l = n.rel_path.to_lowercase();
            if !path_filters.iter().all(|p| path_l.contains(p)) {
                continue;
            }
            let title_l = n.title.to_lowercase();
            let content_l = n.content.to_lowercase();
            let mut score = 0u32;
            let mut all = true;
            for w in &words {
                let in_title = title_l.contains(w.as_str());
                let in_alias = n.aliases.iter().any(|a| a.to_lowercase().contains(w.as_str()));
                let in_tag = tags_l.iter().any(|t| t.contains(w.as_str()));
                let count = content_l.matches(w.as_str()).count() as u32;
                if !(in_title || in_alias || in_tag || count > 0) {
                    all = false;
                    break;
                }
                if title_l == *w {
                    score += 100;
                }
                if in_title {
                    score += 50;
                }
                if in_alias {
                    score += 30;
                }
                if in_tag {
                    score += 20;
                }
                score += count.min(20);
            }
            if !all {
                continue;
            }
            if words.is_empty() && tag_filters.is_empty() && path_filters.is_empty() {
                continue;
            }
            let snippet = words
                .first()
                .map(|w| snippet(n.body(), w, 80))
                .unwrap_or_else(|| n.body().chars().take(120).collect::<String>().replace('\n', " "));
            hits.push(SearchHit {
                rel_path: n.rel_path.clone(),
                title: n.title.clone(),
                score: score.max(1),
                snippet,
            });
        }
        hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        hits
    }

    /// Create a new note. Fails if a note with that path already exists.
    pub fn create_note(&mut self, folder: &str, title: &str, content: &str) -> Result<String, WikiError> {
        let rel = note_rel_path(folder, title);
        let abs = self.abs_path(&rel);
        if abs.exists() {
            return Err(WikiError::Exists(rel));
        }
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&abs, content)?;
        self.upsert_cached(&rel, content.to_string());
        Ok(rel)
    }

    /// Write (create or overwrite) a note.
    pub fn write_note(&mut self, rel_path: &str, content: &str) -> Result<(), WikiError> {
        let abs = self.abs_path(rel_path);
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&abs, content)?;
        self.upsert_cached(rel_path, content.to_string());
        Ok(())
    }

    fn upsert_cached(&mut self, rel: &str, content: String) {
        let mut note = parse_note(rel, content);
        note.modified = Some(std::time::SystemTime::now());
        match self.notes.iter_mut().find(|n| n.rel_path == rel) {
            Some(n) => *n = note,
            None => {
                self.notes.push(note);
                self.notes.sort_by(|a, b| a.rel_path.to_lowercase().cmp(&b.rel_path.to_lowercase()));
            }
        }
    }

    pub fn abs_path(&self, rel: &str) -> PathBuf {
        let mut p = self.root.clone();
        for part in rel.split('/') {
            if !part.is_empty() && part != ".." {
                p.push(part);
            }
        }
        p
    }

    /// Find a previously exported note for a campaign entity by its `cm_id` front matter.
    pub fn find_by_cm_id(&self, id: &str) -> Option<&Note> {
        self.notes.iter().find(|n| {
            split_front_matter(&n.content)
                .0
                .map(|fm| parse_front_matter(fm).get("cm_id").map(|v| v.iter().any(|x| x == id)).unwrap_or(false))
                .unwrap_or(false)
        })
    }

    /// Export (create or update) a note for a campaign entity. Returns its path.
    pub fn export(&mut self, folder: &str, title: &str, cm_id: &str, content: &str) -> Result<String, WikiError> {
        let rel = match self.find_by_cm_id(cm_id) {
            Some(n) => n.rel_path.clone(),
            None => {
                let mut rel = note_rel_path(folder, title);
                let mut i = 2;
                while self.abs_path(&rel).exists() {
                    rel = note_rel_path(folder, &format!("{title} {i}"));
                    i += 1;
                }
                rel
            }
        };
        self.write_note(&rel, content)?;
        Ok(rel)
    }
}

fn rel_string(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn note_rel_path(folder: &str, title: &str) -> String {
    let name = format!("{}.md", sanitize_file_name(title));
    let folder = folder
        .split(['/', '\\'])
        .filter(|s| !s.is_empty() && *s != "..")
        .map(sanitize_file_name)
        .collect::<Vec<_>>()
        .join("/");
    if folder.is_empty() { name } else { format!("{folder}/{name}") }
}

fn snippet(body: &str, word: &str, width: usize) -> String {
    let lower = body.to_lowercase();
    let Some(pos) = lower.find(word) else {
        return body.chars().take(width * 2).collect::<String>().replace('\n', " ");
    };
    // convert byte index in lowercase to char index (approximately same for ASCII-ish text)
    let char_pos = lower[..pos].chars().count();
    let chars: Vec<char> = body.chars().collect();
    let start = char_pos.saturating_sub(width / 2);
    let end = (char_pos + word.chars().count() + width).min(chars.len());
    let mut s: String = chars[start.min(chars.len())..end].iter().collect();
    s = s.replace('\n', " ");
    if start > 0 {
        s = format!("…{s}");
    }
    if end < chars.len() {
        s.push('…');
    }
    s
}

/// Turn `[[Target|Label]]` into Markdown links with a custom scheme, so a renderer can
/// make them clickable: `[Label](wiki:Target)`.
pub fn wikilinks_to_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        let embed = start > 0 && rest.as_bytes()[start - 1] == b'!';
        let (before, after) = rest.split_at(start);
        let Some(end) = after[2..].find("]]") else { break };
        let inner = &after[2..2 + end];
        if inner.contains('\n') {
            out.push_str(&rest[..start + 2]);
            rest = &rest[start + 2..];
            continue;
        }
        let (target, label) = match inner.split_once('|') {
            Some((t, l)) => (t.trim(), l.trim()),
            None => (inner.trim(), inner.trim()),
        };
        let before = if embed { &before[..before.len() - 1] } else { before };
        out.push_str(before);
        out.push_str(&format!("[{}](wiki:{})", label, encode_target(target)));
        rest = &after[2 + end + 2..];
    }
    out.push_str(rest);
    out
}

/// Percent-encode spaces and parens so link targets survive Markdown parsing.
pub fn encode_target(t: &str) -> String {
    t.replace('%', "%25").replace(' ', "%20").replace('(', "%28").replace(')', "%29")
}

pub fn decode_target(t: &str) -> String {
    t.replace("%20", " ").replace("%28", "(").replace("%29", ")").replace("%25", "%")
}

// ---------------------------------------------------------------------------
// Exporting campaign entities to Markdown
// ---------------------------------------------------------------------------

fn fm(kind: &str, id: &str, tags: &[String]) -> String {
    let mut s = format!("---\ncm_type: {kind}\ncm_id: {id}\n");
    let mut all = vec![format!("campaign/{kind}")];
    all.extend(tags.iter().map(|t| t.replace(' ', "-")));
    s.push_str(&format!("tags: [{}]\n", all.join(", ")));
    s.push_str("---\n\n");
    s
}

fn link_line(c: &Campaign, links: &[crate::model::Link]) -> String {
    let mut parts = Vec::new();
    for l in links {
        match l {
            crate::model::Link::Url { url, label } => parts.push(format!("[{}]({})", if label.is_empty() { url } else { label }, url)),
            crate::model::Link::Wiki(p) => parts.push(format!("[[{}]]", p.trim_end_matches(".md"))),
            crate::model::Link::Image(p) => parts.push(format!("`{p}`")),
            other => {
                if let Some(n) = c.link_name(other) {
                    parts.push(format!("[[{n}]]"));
                }
            }
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("**Links:** {}\n\n", parts.join(", "))
    }
}

pub fn card_to_markdown(c: &Campaign, card: &StoryCard) -> String {
    let mut s = fm("story-card", &card.id.to_string(), &card.tags);
    s.push_str(&format!("# {}\n\n", card.display_title()));
    s.push_str(&format!("**Status:** {}", card.status.label()));
    match &card.timing {
        CardTiming::Unscheduled => {}
        CardTiming::RealDate(d) => s.push_str(&format!(" · **Date:** {d}")),
        CardTiming::GameDate(d) => s.push_str(&format!(" · **In-world:** {}", c.settings.game_calendar.format(d))),
        CardTiming::Session(n) => s.push_str(&format!(" · **Session:** {n}")),
    }
    s.push_str("\n\n");
    let threads: Vec<String> = card.threads.iter().filter_map(|t| c.thread(*t)).map(|t| format!("[[{}]]", t.name)).collect();
    if !threads.is_empty() {
        s.push_str(&format!("**Threads:** {}\n\n", threads.join(", ")));
    }
    s.push_str(&link_line(c, &card.links));
    s.push_str(card.body.trim());
    s.push('\n');
    if let Some(q) = &card.quest {
        s.push_str(&format!("\n## Quest\n\n**Status:** {}", q.status.label()));
        if let Some(g) = q.giver.as_ref().and_then(|g| c.link_name(g)) {
            s.push_str(&format!(" · **Given by:** [[{g}]]"));
        }
        s.push_str("\n\n");
        for o in &q.objectives {
            s.push_str(&format!("- [{}] {}\n", if o.done { "x" } else { " " }, o.text.trim()));
        }
        if !q.objectives.is_empty() {
            s.push('\n');
        }
        let reward = q.reward_summary();
        if !reward.is_empty() {
            s.push_str(&format!("**Reward:** {reward}\n"));
        }
    }
    s
}

pub fn location_to_markdown(c: &Campaign, l: &crate::places::Location) -> String {
    let mut s = fm("place", &l.id.to_string(), &l.tags);
    s.push_str(&format!("# {}\n\n", l.display_name()));
    let mut meta = Vec::new();
    if !l.kind.trim().is_empty() {
        meta.push(format!("**Kind:** {}", l.kind.trim()));
    }
    if let Some(p) = l.parent.and_then(|p| c.location(p)) {
        meta.push(format!("**In:** [[{}]]", p.display_name()));
    }
    if !meta.is_empty() {
        s.push_str(&meta.join(" · "));
        s.push_str("\n\n");
    }
    if !l.description.trim().is_empty() {
        s.push_str(l.description.trim());
        s.push_str("\n\n");
    }
    let list = |title: &str, names: Vec<String>, s: &mut String| {
        if !names.is_empty() {
            s.push_str(&format!(
                "**{title}:** {}\n\n",
                names.iter().map(|n| format!("[[{n}]]")).collect::<Vec<_>>().join(", ")
            ));
        }
    };
    list("Places here", c.child_locations(l.id).iter().map(|x| x.display_name()).collect(), &mut s);
    list(
        "People",
        l.npcs.iter().filter_map(|n| c.npc(*n)).map(|n| n.core.display_name()).collect(),
        &mut s,
    );
    list(
        "Factions",
        l.factions.iter().filter_map(|f| c.faction(*f)).map(|f| f.name.clone()).collect(),
        &mut s,
    );
    list(
        "Related",
        l.related.iter().filter_map(|r| c.location(*r)).map(|r| r.display_name()).collect(),
        &mut s,
    );
    if let Some(shop) = &l.shop {
        s.push_str(&format!("## For sale ({})\n\n", shop.theme.label()));
        if let Some(k) = shop.keeper.and_then(|k| c.npc(k)) {
            s.push_str(&format!("Shopkeeper: [[{}]]\n\n", k.core.display_name()));
        }
        for e in &shop.stock {
            s.push_str(&format!(
                "- {} — {}{}\n",
                e.item.name,
                crate::items::format_gp(e.price_gp),
                if e.qty > 1 { format!(" ({} in stock)", e.qty) } else { String::new() }
            ));
        }
        s.push('\n');
    }
    s.push_str(&link_line(c, &l.links));
    if !l.notes.trim().is_empty() {
        s.push_str("## Notes\n\n");
        s.push_str(l.notes.trim());
        s.push('\n');
    }
    s
}

fn core_block(core: &crate::model::CharacterCore) -> String {
    let mut s = String::new();
    let mut row = Vec::new();
    if let Some(ac) = core.ac {
        row.push(format!("**AC** {ac}"));
    }
    if let Some(hp) = core.max_hp {
        if core.hp_formula.trim().is_empty() {
            row.push(format!("**HP** {hp}"));
        } else {
            row.push(format!("**HP** {hp} ({})", core.hp_formula.trim()));
        }
    }
    if !core.speed.trim().is_empty() {
        row.push(format!("**Speed** {}", core.speed.trim()));
    }
    if let Some(i) = core.initiative_mod {
        row.push(format!("**Init** {}", fmt_mod(i)));
    }
    if let Some(pp) = core.passive_perception {
        row.push(format!("**Passive Perception** {pp}"));
    }
    if !row.is_empty() {
        s.push_str(&row.join(" · "));
        s.push_str("\n\n");
    }
    let abil: Vec<(usize, i32)> = (0..6).filter_map(|i| core.abilities.get(i).map(|v| (i, v))).collect();
    if !abil.is_empty() {
        s.push_str("| STR | DEX | CON | INT | WIS | CHA |\n|---|---|---|---|---|---|\n|");
        for i in 0..6 {
            match core.abilities.get(i) {
                Some(v) => s.push_str(&format!(" {} ({}) |", v, fmt_mod(crate::model::ability_mod(v)))),
                None => s.push_str(" — |"),
            }
        }
        s.push_str("\n\n");
    }
    for (k, v) in &core.extra_fields {
        if !k.trim().is_empty() {
            s.push_str(&format!("**{}:** {}\n", k.trim(), v.trim()));
        }
    }
    if !core.extra_fields.is_empty() {
        s.push('\n');
    }
    if !core.custom_rolls.is_empty() {
        s.push_str("## Rolls\n\n");
        for r in &core.custom_rolls {
            s.push_str(&format!("- **{}:** `{}`\n", r.label, r.notation));
        }
        s.push('\n');
    }
    if !core.inventory.is_empty() || !core.coins.is_zero() {
        s.push_str("## Inventory\n\n");
        if !core.coins.is_zero() {
            s.push_str(&format!("**Coins:** {}\n\n", core.coins.label()));
        }
        for e in &core.inventory {
            let mut line = if e.qty > 1 {
                format!("- {} × {}", e.qty, e.item.name)
            } else {
                format!("- {}", e.item.name)
            };
            if e.equipped {
                line.push_str(" (equipped)");
            }
            if e.attuned {
                line.push_str(" (attuned)");
            }
            s.push_str(&line);
            s.push('\n');
        }
        s.push('\n');
    }
    if !core.notes.trim().is_empty() {
        s.push_str("## Notes\n\n");
        s.push_str(core.notes.trim());
        s.push_str("\n\n");
    }
    s
}

pub fn npc_to_markdown(c: &Campaign, npc: &Npc) -> String {
    let mut s = fm(
        if npc.kind == crate::model::NpcKind::Monster { "monster" } else { "npc" },
        &npc.id.to_string(),
        &npc.core.tags,
    );
    s.push_str(&format!("# {}\n\n", npc.core.display_name()));
    let mut meta = Vec::new();
    for (k, v) in [
        ("Species", &npc.core.species),
        ("Type", &npc.core.class_or_type),
        ("Role", &npc.role),
        ("Location", &npc.location),
        ("CR", &npc.core.cr),
    ] {
        if !v.trim().is_empty() {
            meta.push(format!("**{k}:** {}", v.trim()));
        }
    }
    meta.push(format!("**Status:** {}", npc.status.label()));
    s.push_str(&meta.join(" · "));
    s.push_str("\n\n");
    let factions: Vec<String> = npc.factions.iter().filter_map(|f| c.faction(*f)).map(|f| format!("[[{}]]", f.name)).collect();
    if !factions.is_empty() {
        s.push_str(&format!("**Factions:** {}\n\n", factions.join(", ")));
    }
    s.push_str(&link_line(c, &npc.core.links));
    s.push_str(&core_block(&npc.core));
    s
}

pub fn player_to_markdown(c: &Campaign, p: &PlayerCharacter) -> String {
    let mut s = fm("pc", &p.id.to_string(), &p.core.tags);
    s.push_str(&format!("# {}\n\n", p.core.display_name()));
    let mut meta = Vec::new();
    if !p.player_name.trim().is_empty() {
        meta.push(format!("**Player:** {}", p.player_name.trim()));
    }
    if !p.core.species.trim().is_empty() {
        meta.push(format!("**Species:** {}", p.core.species.trim()));
    }
    if !p.core.class_or_type.trim().is_empty() {
        meta.push(format!("**Class:** {}", p.core.class_or_type.trim()));
    }
    if let Some(l) = p.core.level {
        meta.push(format!("**Level:** {l}"));
    }
    if !meta.is_empty() {
        s.push_str(&meta.join(" · "));
        s.push_str("\n\n");
    }
    s.push_str(&link_line(c, &p.core.links));
    s.push_str(&core_block(&p.core));
    s
}

pub fn faction_to_markdown(c: &Campaign, f: &Faction) -> String {
    let mut s = fm("faction", &f.id.to_string(), &[]);
    s.push_str(&format!("# {}\n\n**Attitude toward the party:** {}\n\n", f.name, f.attitude.label()));
    if !f.description.trim().is_empty() {
        s.push_str(f.description.trim());
        s.push_str("\n\n");
    }
    if !f.goals.trim().is_empty() {
        s.push_str("## Goals\n\n");
        s.push_str(f.goals.trim());
        s.push_str("\n\n");
    }
    let members = c.faction_members(f.id);
    if !members.is_empty() {
        s.push_str("## Members\n\n");
        for m in members {
            s.push_str(&format!("- [[{}]]\n", m.core.display_name()));
        }
        s.push('\n');
    }
    s.push_str(&link_line(c, &f.links));
    if !f.notes.trim().is_empty() {
        s.push_str("## Notes\n\n");
        s.push_str(f.notes.trim());
        s.push('\n');
    }
    s
}

pub fn encounter_to_markdown(c: &Campaign, e: &Encounter) -> String {
    let mut s = fm("encounter", &e.id.to_string(), &[]);
    s.push_str(&format!("# {}\n\n", e.name));
    if !e.location.trim().is_empty() {
        s.push_str(&format!("**Location:** {}\n\n", e.location.trim()));
    }
    if !e.description.trim().is_empty() {
        s.push_str(e.description.trim());
        s.push_str("\n\n");
    }
    s.push_str("## Combatants\n\n");
    for en in &e.entries {
        let name = match &en.source {
            CombatantSource::AdHoc => en.name.clone(),
            src => c
                .core_for(src)
                .map(|core| format!("[[{}]]", core.display_name()))
                .unwrap_or_else(|| "(deleted)".into()),
        };
        s.push_str(&format!("- {} × {}\n", en.count.max(1), name));
    }
    s.push('\n');
    s.push_str(&link_line(c, &e.links));
    if !e.notes.trim().is_empty() {
        s.push_str("## Notes\n\n");
        s.push_str(e.notes.trim());
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vault() -> (tempfile::TempDir, Vault) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".obsidian")).unwrap();
        fs::write(root.join(".obsidian/app.json"), "{}").unwrap();
        fs::create_dir_all(root.join("NPCs")).unwrap();
        fs::create_dir_all(root.join("World/Places")).unwrap();
        fs::write(
            root.join("NPCs/Baron Vask.md"),
            "---\ntags: [npc, noble]\naliases:\n  - The Baron\n---\n# Baron Vask\nRules [[Millbrook]] with an iron fist. #villain\n",
        )
        .unwrap();
        fs::write(
            root.join("World/Places/Millbrook.md"),
            "A sleepy mill town. The [[Baron Vask|baron]] taxes it heavily.\n#town\n```\n#notatag\n```\n",
        )
        .unwrap();
        fs::write(root.join("Session 1.md"), "We met [[The Baron]]. Heading 1\n# Heading\n").unwrap();
        fs::write(root.join("image.png"), [0u8, 1, 2]).unwrap();
        let v = Vault::open(root).unwrap();
        (tmp, v)
    }

    #[test]
    fn scan_parse_resolve() {
        let (_t, v) = make_vault();
        assert_eq!(v.notes.len(), 3);
        let baron = v.note("NPCs/Baron Vask.md").unwrap();
        assert_eq!(baron.title, "Baron Vask");
        assert_eq!(baron.aliases, vec!["The Baron"]);
        assert!(baron.tags.contains(&"npc".to_string()));
        assert!(baron.tags.contains(&"villain".to_string()));
        assert_eq!(baron.links, vec!["Millbrook"]);
        let mill = v.note("World/Places/Millbrook.md").unwrap();
        assert!(mill.tags.contains(&"town".to_string()));
        assert!(!mill.tags.contains(&"notatag".to_string()));
        assert_eq!(v.resolve("millbrook").unwrap().rel_path, "World/Places/Millbrook.md");
        assert_eq!(v.resolve("Places/Millbrook").unwrap().rel_path, "World/Places/Millbrook.md");
        assert_eq!(v.resolve("The Baron").unwrap().title, "Baron Vask");
        assert!(v.resolve("Nowhere").is_none());
        let back = v.backlinks("NPCs/Baron Vask.md");
        let names: Vec<_> = back.iter().map(|n| n.title.as_str()).collect();
        assert!(names.contains(&"Millbrook") && names.contains(&"Session 1"));
        assert_eq!(v.folders(), vec!["NPCs", "World", "World/Places"]);
        assert!(v.all_tags().iter().any(|(t, n)| t == "npc" && *n == 1));
    }

    #[test]
    fn search_ranks_titles_first() {
        let (_t, v) = make_vault();
        let hits = v.search("baron");
        assert_eq!(hits[0].title, "Baron Vask");
        assert!(hits.len() >= 2);
        assert!(v.search("baron mill").iter().all(|h| h.title != "Session 1"));
        let tag_hits = v.search("tag:town");
        assert_eq!(tag_hits.len(), 1);
        assert_eq!(tag_hits[0].title, "Millbrook");
        assert_eq!(v.search("#villain").len(), 1);
        assert_eq!(v.search("path:npcs").len(), 1);
        assert!(v.search("zzzz").is_empty());
        assert!(v.search("").is_empty());
        assert!(v.search("sleepy")[0].snippet.contains("sleepy"));
    }

    #[test]
    fn create_and_export() {
        let (_t, mut v) = make_vault();
        let rel = v.create_note("NPCs", "Mira: Smuggler?", "# Mira\n").unwrap();
        assert_eq!(rel, "NPCs/Mira- Smuggler-.md");
        assert!(v.abs_path(&rel).is_file());
        assert!(v.create_note("NPCs", "Mira: Smuggler?", "x").is_err());
        assert!(v.note(&rel).is_some());

        let mut c = Campaign::new("x");
        let mut npc = Npc::default();
        npc.core.name = "Goblin Boss".into();
        npc.core.ac = Some(17);
        npc.core.abilities.dex = Some(14);
        npc.core.custom_rolls.push(crate::model::CustomRoll {
            label: "Scimitar".into(),
            notation: "1d6+2".into(),
            ..Default::default()
        });
        let md = npc_to_markdown(&c, &npc);
        assert!(md.contains("cm_id: "));
        assert!(md.contains("**AC** 17"));
        assert!(md.contains("14 (+2)"));
        assert!(md.contains("`1d6+2`"));
        c.npcs.push(npc.clone());
        let p1 = v.export("Campaign Manager/NPCs", "Goblin Boss", &npc.id.to_string(), &md).unwrap();
        // re-export updates the same file even if renamed
        let p2 = v.export("Campaign Manager/NPCs", "Renamed", &npc.id.to_string(), &md).unwrap();
        assert_eq!(p1, p2);
        let card = StoryCard {
            title: "Ambush".into(),
            body: "Goblins at the bridge".into(),
            ..Default::default()
        };
        let md = card_to_markdown(&c, &card);
        assert!(md.contains("# Ambush") && md.contains("Goblins at the bridge"));
        // path traversal is neutralised
        assert_eq!(note_rel_path("../../etc", "passwd"), "etc/passwd.md");
    }

    #[test]
    fn front_matter_edge_cases() {
        assert_eq!(split_front_matter("no fm").0, None);
        let (fm, body) = split_front_matter("---\na: 1\n---\nbody");
        assert_eq!(fm, Some("a: 1\n"));
        assert_eq!(body, "body");
        assert_eq!(split_front_matter("---\nunterminated").0, None);
        let m = parse_front_matter("tags: one two\nempty:\naliases: ['A', \"B\"]\n");
        assert_eq!(m["tags"], vec!["one two"]);
        assert_eq!(m["aliases"], vec!["A", "B"]);
    }

    #[test]
    fn wikilink_conversion() {
        assert_eq!(
            wikilinks_to_markdown("see [[Baron Vask|the baron]] now"),
            "see [the baron](wiki:Baron%20Vask) now"
        );
        assert_eq!(wikilinks_to_markdown("![[map.png]]"), "[map.png](wiki:map.png)");
        assert_eq!(wikilinks_to_markdown("unclosed [[x"), "unclosed [[x");
        assert_eq!(decode_target(&encode_target("A (b) 100%")), "A (b) 100%");
        assert_eq!(extract_wikilinks("[[A#Heading|x]] [[B]]"), vec!["A", "B"]);
    }
}
