//! Per-user application settings (appearance, recent campaigns). Stored as JSON in the OS
//! config directory, separate from campaign data.

use std::path::PathBuf;
use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, TextStyle};

use crate::themes::{self, Theme};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
    /// Follow the operating system.
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FontChoice {
    /// egui's built-in font.
    #[default]
    Default,
    /// Try to load the operating system's UI font (Segoe UI, Helvetica/SF, DejaVu/Noto).
    System,
    /// Monospace everywhere.
    Monospace,
    /// A TTF/OTF chosen by the user.
    Custom,
}

impl FontChoice {
    pub const ALL: [FontChoice; 4] = [FontChoice::Default, FontChoice::System, FontChoice::Monospace, FontChoice::Custom];
    pub fn label(self) -> &'static str {
        match self {
            FontChoice::Default => "Built-in",
            FontChoice::System => "System UI font",
            FontChoice::Monospace => "Monospace",
            FontChoice::Custom => "Custom font file…",
        }
    }
}

/// Optional colour overrides on top of the light/dark base theme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CustomColors {
    pub enabled: bool,
    pub background: [u8; 3],
    pub panel: [u8; 3],
    pub text: [u8; 3],
    pub accent: [u8; 3],
    pub widget: [u8; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// Selected theme: a built-in name, one of `user_themes`, or "Follow system".
    pub theme_name: Option<String>,
    /// Themes the user customised and saved.
    pub user_themes: Vec<Theme>,
    /// Legacy (v0.1.0) fields, migrated into `theme_name` / `user_themes` on load.
    pub theme: ThemeMode,
    pub custom_colors: CustomColors,
    /// Base body text size in points.
    pub font_size: f32,
    /// Whole-UI zoom (1.0 = 100%).
    pub ui_scale: f32,
    pub font: FontChoice,
    pub custom_font_path: Option<String>,
    /// Use striped rows and tighter spacing.
    pub compact: bool,
    pub recent: Vec<String>,
    pub last_campaign: Option<String>,
    pub reopen_last: bool,
    /// Seconds after the last change before autosaving (0 = off).
    pub autosave_secs: u32,
    pub show_dice_panel: bool,
    pub dice_panel_width: f32,
    /// Navigation shows icons only.
    pub nav_collapsed: bool,
    /// Calendar grids start on Sunday (US) or Monday.
    pub week_start: cm_core::calendar::WeekStart,
    /// How real-world dates are written.
    pub date_style: cm_core::calendar::DateStyle,
    /// Write in Markdown fields with the marks showing (plain text box) instead of formatted.
    pub markdown_source: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            theme_name: Some("Light".into()),
            user_themes: Vec::new(),
            theme: ThemeMode::Light,
            custom_colors: CustomColors::default(),
            font_size: 14.0,
            ui_scale: 1.0,
            font: FontChoice::Default,
            custom_font_path: None,
            compact: false,
            recent: Vec::new(),
            last_campaign: None,
            reopen_last: true,
            autosave_secs: 5,
            show_dice_panel: true,
            dice_panel_width: 320.0,
            nav_collapsed: false,
            week_start: cm_core::calendar::WeekStart::Sunday,
            date_style: cm_core::calendar::DateStyle::Us,
            markdown_source: false,
        }
    }
}

pub fn config_dir() -> Option<PathBuf> {
    // tests (and portable setups) can keep settings somewhere else
    if let Some(d) = std::env::var_os("CAMPAIGN_MANAGER_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    directories::ProjectDirs::from("org", "CampaignManager", "Campaign Manager").map(|d| d.config_dir().to_path_buf())
}

pub fn default_campaigns_dir() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|u| u.document_dir().map(|d| d.to_path_buf()))
        .or_else(|| directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Campaign Manager")
}

impl AppSettings {
    pub fn load() -> Self {
        let Some(dir) = config_dir() else {
            return Self::default();
        };
        let path = dir.join("settings.json");
        let mut s: AppSettings = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(s) => s,
                Err(e) => {
                    // keep the broken file (and the themes in it) rather than silently losing it
                    let backup = dir.join(format!("settings.broken-{}.json", chrono::Local::now().format("%Y%m%d-%H%M%S")));
                    let _ = std::fs::copy(&path, &backup);
                    eprintln!("settings.json couldn't be read ({e}); saved a copy as {}", backup.display());
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        };
        s.migrate();
        s
    }

    /// Convert v0.1.0 theme settings (light/dark + optional custom colours).
    pub fn migrate(&mut self) {
        if self.theme_name.is_some() {
            return;
        }
        if self.custom_colors.enabled {
            let cc = &self.custom_colors;
            let dark = self.theme == ThemeMode::Dark;
            let base = if dark { themes::DARK.palette } else { themes::LIGHT.palette };
            let t = Theme {
                name: "My colours".into(),
                dark,
                palette: themes::Palette {
                    background: cc.background,
                    panel: cc.panel,
                    window: cc.panel,
                    faint: themes::mix(cc.panel, cc.text, 0.06),
                    text: cc.text,
                    accent: cc.accent,
                    widget: cc.widget,
                    widget_hover: themes::mix(cc.widget, cc.text, 0.1),
                    border: base.border,
                },
            };
            self.user_themes.push(t);
            self.theme_name = Some("My colours".into());
        } else {
            self.theme_name = Some(
                match self.theme {
                    ThemeMode::Light => "Light",
                    ThemeMode::Dark => "Dark",
                    ThemeMode::System => themes::FOLLOW_SYSTEM,
                }
                .into(),
            );
        }
        self.custom_colors.enabled = false;
    }

    pub fn theme_name(&self) -> &str {
        self.theme_name.as_deref().unwrap_or("Light")
    }

    /// The theme to draw with (resolving "Follow system").
    pub fn active_theme(&self, ctx: &egui::Context) -> Theme {
        let name = self.theme_name();
        if name == themes::FOLLOW_SYSTEM {
            let dark = ctx.system_theme().map(|t| t == egui::Theme::Dark).unwrap_or(false);
            return if dark { themes::DARK.to_theme() } else { themes::LIGHT.to_theme() };
        }
        if let Some(t) = self.user_themes.iter().find(|t| t.name == name) {
            return t.clone();
        }
        themes::builtin(name).map(|b| b.to_theme()).unwrap_or_else(|| themes::LIGHT.to_theme())
    }

    pub fn is_user_theme(&self, name: &str) -> bool {
        self.user_themes.iter().any(|t| t.name == name)
    }

    /// Copy the active theme into a new editable user theme and select it.
    pub fn customize_current(&mut self, ctx: &egui::Context) -> String {
        let mut t = self.active_theme(ctx);
        let base = if self.is_user_theme(&t.name) {
            t.name.clone()
        } else {
            format!("My {}", t.name)
        };
        let mut name = base.clone();
        let mut i = 2;
        while self.is_user_theme(&name) || themes::builtin(&name).is_some() {
            name = format!("{base} {i}");
            i += 1;
        }
        t.name = name.clone();
        self.user_themes.push(t);
        self.theme_name = Some(name.clone());
        name
    }

    pub fn save(&self) {
        if let Some(dir) = config_dir() {
            let _ = std::fs::create_dir_all(&dir);
            if let Ok(s) = serde_json::to_string_pretty(self) {
                // write a temporary file and rename it over the old one, so a crash or power
                // cut mid-write never leaves a half-written settings file
                let tmp = dir.join("settings.json.tmp");
                if std::fs::write(&tmp, s).is_ok() && std::fs::rename(&tmp, dir.join("settings.json")).is_err() {
                    let _ = std::fs::remove_file(&tmp);
                }
            }
        }
    }

    pub fn push_recent(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(10);
        self.last_campaign = Some(path.to_string());
    }

    /// Apply theme, colours and text sizes to the egui context.
    pub fn apply_style(&self, ctx: &egui::Context) {
        let t = self.active_theme(ctx);
        let dark = t.dark;
        let v = themes::visuals(&t);
        let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
        ctx.set_theme(theme);
        ctx.set_visuals_of(theme, v);

        let base = self.font_size.clamp(8.0, 40.0);
        let compact = self.compact;
        ctx.style_mut_of(theme, |style| {
            use egui::FontId;
            style.text_styles = [
                (TextStyle::Small, FontId::new(base * 0.8, FontFamily::Proportional)),
                (TextStyle::Body, FontId::new(base, FontFamily::Proportional)),
                (TextStyle::Button, FontId::new(base, FontFamily::Proportional)),
                (TextStyle::Monospace, FontId::new(base * 0.95, FontFamily::Monospace)),
                (TextStyle::Heading, FontId::new(base * 1.45, FontFamily::Proportional)),
            ]
            .into();
            style.spacing.item_spacing = if compact { egui::vec2(6.0, 3.0) } else { egui::vec2(8.0, 5.0) };
            style.spacing.button_padding = if compact { egui::vec2(4.0, 1.0) } else { egui::vec2(6.0, 3.0) };
            style.spacing.interact_size.y = base + if compact { 4.0 } else { 8.0 };
            style.url_in_tooltip = true;
        });
        ctx.set_zoom_factor(self.ui_scale.clamp(0.5, 3.0));
    }

    /// Install fonts according to the font choice. Returns an error message on failure.
    pub fn apply_fonts(&self, ctx: &egui::Context) -> Result<(), String> {
        let mut fonts = FontDefinitions::default();
        let mut load_into_proportional = |bytes: Vec<u8>, name: &str| {
            fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
            if let Some(f) = fonts.families.get_mut(&FontFamily::Proportional) {
                f.insert(0, name.to_owned());
            }
        };
        let mut result = Ok(());
        match self.font {
            FontChoice::Default => {}
            FontChoice::Monospace => {
                let mono = fonts.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
                fonts.families.insert(FontFamily::Proportional, mono);
            }
            FontChoice::System => match system_font_bytes() {
                Some(bytes) => load_into_proportional(bytes, "system-ui"),
                // It's the default, so fall back to the built-in font quietly.
                None => {}
            },
            FontChoice::Custom => match self.custom_font_path.as_deref() {
                Some(p) => match std::fs::read(p) {
                    Ok(bytes) if looks_like_font(&bytes) => load_into_proportional(bytes, "custom"),
                    Ok(_) => result = Err(format!("{p} doesn't look like a TTF/OTF font")),
                    Err(e) => result = Err(format!("Couldn't read {p}: {e}")),
                },
                None => result = Err("No custom font selected".into()),
            },
        }
        // A real bold face for formatted text and headings (the built-in font only has one
        // weight). Matches the system font when there is a bold version of it next to it.
        let bold = match self.font {
            FontChoice::System => system_bold_font_bytes(),
            _ => None,
        }
        .unwrap_or_else(|| BUNDLED_BOLD.to_vec());
        fonts.font_data.insert("ui-bold".to_owned(), Arc::new(FontData::from_owned(bold)));
        let mut bold_family = vec!["ui-bold".to_owned()];
        bold_family.extend(fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
        fonts.families.insert(crate::rich_text::bold_family(), bold_family);
        ctx.set_fonts(fonts);
        result
    }
}

/// Ubuntu Bold (Ubuntu Font Licence, like the built-in Ubuntu Light).
const BUNDLED_BOLD: &[u8] = include_bytes!("../assets/Ubuntu-Bold.ttf");

fn system_bold_font_bytes() -> Option<Vec<u8>> {
    let candidates: &[&str] = if cfg!(target_os = "windows") {
        &[
            "C:\\Windows\\Fonts\\segoeuib.ttf",
            "C:\\Windows\\Fonts\\arialbd.ttf",
            "C:\\Windows\\Fonts\\tahomabd.ttf",
        ]
    } else if cfg!(target_os = "macos") {
        &["/System/Library/Fonts/Supplemental/Arial Bold.ttf", "/Library/Fonts/Arial Bold.ttf"]
    } else {
        &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
            "/usr/share/fonts/noto/NotoSans-Bold.ttf",
            "/usr/share/fonts/google-noto/NotoSans-Bold.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        ]
    };
    candidates.iter().filter_map(|p| std::fs::read(p).ok()).find(|b| looks_like_font(b))
}

/// True if the bytes are a font egui can actually use. egui stops the whole program on a
/// font it can't read, so every font is test-read with the same library first (a damaged
/// or truncated file would otherwise crash the app at every start).
fn looks_like_font(b: &[u8]) -> bool {
    b.len() > 4 && (b[..4] == [0, 1, 0, 0] || &b[..4] == b"OTTO" || &b[..4] == b"true" || &b[..4] == b"ttcf") && ab_glyph::FontRef::try_from_slice(b).is_ok()
}

fn system_font_bytes() -> Option<Vec<u8>> {
    let candidates: &[&str] = if cfg!(target_os = "windows") {
        &[
            "C:\\Windows\\Fonts\\segoeui.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
            "C:\\Windows\\Fonts\\tahoma.ttf",
        ]
    } else if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/Library/Fonts/Arial.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
            "/System/Library/Fonts/Geneva.ttf",
        ]
    } else {
        &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
            "/usr/share/fonts/cantarell/Cantarell-Regular.otf",
        ]
    };
    candidates.iter().filter_map(|p| std::fs::read(p).ok()).find(|b| looks_like_font(b))
}
