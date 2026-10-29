//! Saving and loading. A campaign is a folder:
//!
//! ```text
//! My Campaign/
//!   campaign.json        <- everything except images, pretty-printed UTF-8 JSON
//!   campaign.json.bak    <- previous save
//!   assets/              <- images copied in by the app (maps, portraits, sheets)
//! ```
//!
//! Paths inside the JSON always use forward slashes and are relative to the folder, so the
//! folder can be copied between Windows, macOS and Linux (or synced via Dropbox/Git).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::model::{Campaign, SCHEMA_VERSION};

pub const CAMPAIGN_FILE: &str = "campaign.json";
pub const ASSETS_DIR: &str = "assets";

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("couldn't read campaign file: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no campaign.json found in {0}")]
    NotACampaign(PathBuf),
    #[error("this campaign was saved by a newer version (schema {found}, this app supports {supported})")]
    TooNew { found: u32, supported: u32 },
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> StorageError + '_ {
    move |source| StorageError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Accept either the folder or the campaign.json inside it.
pub fn campaign_dir(path: &Path) -> PathBuf {
    if path.file_name().map(|f| f == CAMPAIGN_FILE).unwrap_or(false) {
        path.parent().map(Path::to_path_buf).unwrap_or_default()
    } else {
        path.to_path_buf()
    }
}

pub fn is_campaign_dir(path: &Path) -> bool {
    campaign_dir(path).join(CAMPAIGN_FILE).is_file()
}

/// Serialise to pretty JSON.
pub fn to_json(c: &Campaign) -> Result<String, StorageError> {
    Ok(serde_json::to_string_pretty(c)?)
}

pub fn from_json(s: &str) -> Result<Campaign, StorageError> {
    // Peek at schema version first so a newer file gives a clear error.
    let v: serde_json::Value = serde_json::from_str(s)?;
    let found = v.get("schema_version").and_then(|x| x.as_u64()).unwrap_or(SCHEMA_VERSION as u64) as u32;
    if found > SCHEMA_VERSION {
        return Err(StorageError::TooNew {
            found,
            supported: SCHEMA_VERSION,
        });
    }
    let mut c: Campaign = serde_json::from_value(v)?;
    migrate(&mut c);
    Ok(c)
}

/// Upgrade older data in place. (Version 1 is the first format.)
fn migrate(c: &mut Campaign) {
    c.schema_version = SCHEMA_VERSION;
}

/// Save atomically: write a temp file, keep the previous save as `.bak`, then rename.
pub fn save(c: &Campaign, dir: &Path) -> Result<PathBuf, StorageError> {
    let dir = campaign_dir(dir);
    fs::create_dir_all(&dir).map_err(io_err(&dir))?;
    fs::create_dir_all(dir.join(ASSETS_DIR)).map_err(io_err(&dir))?;
    let json = to_json(c)?;
    let target = dir.join(CAMPAIGN_FILE);
    let tmp = dir.join(format!("{CAMPAIGN_FILE}.tmp"));
    {
        let mut f = fs::File::create(&tmp).map_err(io_err(&tmp))?;
        f.write_all(json.as_bytes()).map_err(io_err(&tmp))?;
        f.sync_all().map_err(io_err(&tmp))?;
    }
    if target.exists() {
        let bak = dir.join(format!("{CAMPAIGN_FILE}.bak"));
        // best effort; failure to back up shouldn't block saving
        let _ = fs::copy(&target, &bak);
    }
    fs::rename(&tmp, &target).map_err(io_err(&target))?;
    Ok(target)
}

pub fn load(path: &Path) -> Result<Campaign, StorageError> {
    let dir = campaign_dir(path);
    let file = dir.join(CAMPAIGN_FILE);
    if !file.is_file() {
        return Err(StorageError::NotACampaign(dir));
    }
    let text = fs::read_to_string(&file).map_err(io_err(&file))?;
    // Tolerate a UTF-8 BOM (some Windows editors add one).
    let text = text.trim_start_matches('\u{feff}');
    from_json(text)
}

/// Copy an image (or any file) into `assets/` and return its campaign-relative path.
pub fn import_asset(dir: &Path, source: &Path) -> Result<String, StorageError> {
    let dir = campaign_dir(dir);
    let assets = dir.join(ASSETS_DIR);
    fs::create_dir_all(&assets).map_err(io_err(&assets))?;
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .map(sanitize_file_name)
        .unwrap_or_else(|| "file".into());
    let ext = source.extension().and_then(|s| s.to_str()).map(|e| e.to_lowercase()).unwrap_or_default();
    let short = &uuid::Uuid::new_v4().simple().to_string()[..8];
    let name = if ext.is_empty() {
        format!("{stem}-{short}")
    } else {
        format!("{stem}-{short}.{ext}")
    };
    let dest = assets.join(&name);
    fs::copy(source, &dest).map_err(io_err(source))?;
    Ok(format!("{ASSETS_DIR}/{name}"))
}

/// Resolve a campaign-relative path (forward slashes) to an absolute one.
pub fn resolve_asset(dir: &Path, rel: &str) -> PathBuf {
    let p = Path::new(rel);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    let mut out = campaign_dir(dir);
    for part in rel.split('/') {
        if !part.is_empty() {
            out.push(part);
        }
    }
    out
}

/// Make a string safe to use as a file name on Windows, macOS and Linux.
pub fn sanitize_file_name(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    s = s.trim().trim_end_matches(['.', ' ']).to_string();
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
        "LPT7", "LPT8", "LPT9",
    ];
    if s.is_empty() {
        s = "untitled".into();
    }
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(&s)) {
        s.push('_');
    }
    if s.chars().count() > 120 {
        s = s.chars().take(120).collect();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn roundtrip_and_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("My Campaign");
        let mut c = Campaign::new("Curse of the Crimson Crown");
        let mut card = StoryCard::default();
        card.title = "The baron lies".into();
        card.body = "He was at the mill that night.\n\n[[Baron Vask]]".into();
        card.timing = CardTiming::RealDate(chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        c.story_cards.push(card);
        let mut npc = Npc::default();
        npc.core.name = "Baron Vask".into();
        npc.core.custom_rolls.push(CustomRoll {
            label: "Rapier".into(),
            notation: "1d8+3".into(),
            ..Default::default()
        });
        c.npcs.push(npc);
        c.dice.macros.insert("sneak".into(), "3d6".into());

        save(&c, &dir).unwrap();
        assert!(dir.join("campaign.json").is_file());
        assert!(dir.join("assets").is_dir());
        assert!(is_campaign_dir(&dir));
        assert!(is_campaign_dir(&dir.join("campaign.json")));

        let loaded = load(&dir).unwrap();
        assert_eq!(loaded.name, c.name);
        assert_eq!(loaded.story_cards[0].title, "The baron lies");
        assert_eq!(loaded.story_cards[0].timing, c.story_cards[0].timing);
        assert_eq!(loaded.npcs[0].core.custom_rolls[0].notation, "1d8+3");
        assert_eq!(loaded.dice.macros["sneak"], "3d6");

        // second save produces a backup
        save(&loaded, &dir).unwrap();
        assert!(dir.join("campaign.json.bak").is_file());
        assert!(!dir.join("campaign.json.tmp").exists());
    }

    #[test]
    fn rejects_newer_schema_and_missing() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(matches!(load(tmp.path()), Err(StorageError::NotACampaign(_))));
        let json = format!("{{\"schema_version\": {}, \"name\": \"x\"}}", SCHEMA_VERSION + 1);
        assert!(matches!(from_json(&json), Err(StorageError::TooNew { .. })));
        std::fs::write(tmp.path().join("campaign.json"), "\u{feff}{\"name\":\"bom\"}").unwrap();
        assert_eq!(load(tmp.path()).unwrap().name, "bom");
        std::fs::write(tmp.path().join("campaign.json"), "{not json").unwrap();
        assert!(load(tmp.path()).is_err());
    }

    #[test]
    fn assets() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("my map.PNG");
        std::fs::write(&src, b"fake").unwrap();
        let dir = tmp.path().join("camp");
        let rel = import_asset(&dir, &src).unwrap();
        assert!(rel.starts_with("assets/my map-"));
        assert!(rel.ends_with(".png"));
        let abs = resolve_asset(&dir, &rel);
        assert_eq!(std::fs::read(abs).unwrap(), b"fake");
    }

    #[test]
    fn file_names() {
        assert_eq!(sanitize_file_name("What? A/B: test*"), "What- A-B- test-");
        assert_eq!(sanitize_file_name("CON"), "CON_");
        assert_eq!(sanitize_file_name("  "), "untitled");
        assert_eq!(sanitize_file_name("end. "), "end");
    }
}
