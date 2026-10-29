//! Colour themes. Every theme (built-in or saved by the user) is a full palette applied
//! the same way, so all surfaces — panels, fields, buttons, selection, borders — match.

use egui::{Color32, Stroke, Visuals};
use serde::{Deserialize, Serialize};

pub type Rgb = [u8; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Palette {
    /// Text fields, map canvas and other "paper" areas.
    pub background: Rgb,
    /// Main panels.
    pub panel: Rgb,
    /// Floating windows, menus and popups.
    pub window: Rgb,
    /// Striped rows, code blocks.
    pub faint: Rgb,
    pub text: Rgb,
    /// Selection, links, the active navigation tab.
    pub accent: Rgb,
    /// Buttons.
    pub widget: Rgb,
    pub widget_hover: Rgb,
    /// Lines and frames.
    pub border: Rgb,
}

impl Default for Palette {
    fn default() -> Self {
        LIGHT.palette
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub name: String,
    pub dark: bool,
    pub palette: Palette,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            name: "Light".into(),
            dark: false,
            palette: LIGHT.palette,
        }
    }
}

pub struct BuiltinTheme {
    pub name: &'static str,
    pub dark: bool,
    pub palette: Palette,
}

impl BuiltinTheme {
    pub fn to_theme(&self) -> Theme {
        Theme {
            name: self.name.to_string(),
            dark: self.dark,
            palette: self.palette,
        }
    }
}

const fn g(v: u8) -> Rgb {
    [v, v, v]
}

pub const LIGHT: BuiltinTheme = BuiltinTheme {
    name: "Light",
    dark: false,
    palette: Palette {
        background: g(255),
        panel: g(246),
        window: g(250),
        faint: g(237),
        text: g(28),
        accent: [40, 120, 220],
        widget: g(229),
        widget_hover: g(216),
        border: g(198),
    },
};

pub const DARK: BuiltinTheme = BuiltinTheme {
    name: "Dark",
    dark: true,
    palette: Palette {
        background: g(16),
        panel: g(29),
        window: g(34),
        faint: g(37),
        text: g(214),
        accent: [50, 145, 210],
        widget: g(58),
        widget_hover: g(72),
        border: g(64),
    },
};

pub const BUILTIN: &[BuiltinTheme] = &[
    LIGHT,
    BuiltinTheme {
        name: "Paper",
        dark: false,
        palette: Palette {
            background: [255, 253, 248],
            panel: [245, 242, 235],
            window: [250, 248, 242],
            faint: [236, 232, 222],
            text: [40, 36, 30],
            accent: [180, 90, 40],
            widget: [230, 225, 214],
            widget_hover: [219, 212, 197],
            border: [200, 192, 175],
        },
    },
    BuiltinTheme {
        name: "Parchment",
        dark: false,
        palette: Palette {
            background: [253, 248, 235],
            panel: [240, 230, 205],
            window: [246, 238, 218],
            faint: [232, 220, 190],
            text: [55, 38, 20],
            accent: [150, 80, 30],
            widget: [228, 214, 182],
            widget_hover: [216, 198, 160],
            border: [190, 170, 130],
        },
    },
    BuiltinTheme {
        name: "Sage",
        dark: false,
        palette: Palette {
            background: [250, 252, 248],
            panel: [234, 240, 232],
            window: [242, 246, 240],
            faint: [224, 232, 221],
            text: [30, 42, 32],
            accent: [60, 130, 80],
            widget: [218, 228, 214],
            widget_hover: [205, 218, 200],
            border: [180, 196, 176],
        },
    },
    BuiltinTheme {
        name: "Sky",
        dark: false,
        palette: Palette {
            background: [255, 255, 255],
            panel: [238, 244, 250],
            window: [245, 249, 253],
            faint: [226, 236, 246],
            text: [25, 40, 60],
            accent: [30, 110, 200],
            widget: [220, 232, 244],
            widget_hover: [205, 222, 240],
            border: [180, 200, 225],
        },
    },
    BuiltinTheme {
        name: "Lavender",
        dark: false,
        palette: Palette {
            background: [252, 250, 255],
            panel: [240, 236, 248],
            window: [246, 243, 252],
            faint: [230, 224, 242],
            text: [40, 32, 60],
            accent: [120, 80, 190],
            widget: [226, 220, 240],
            widget_hover: [214, 206, 234],
            border: [190, 180, 215],
        },
    },
    BuiltinTheme {
        name: "High contrast (light)",
        dark: false,
        palette: Palette {
            background: g(255),
            panel: g(255),
            window: g(255),
            faint: g(232),
            text: g(0),
            accent: [0, 70, 200],
            widget: g(222),
            widget_hover: g(196),
            border: g(0),
        },
    },
    DARK,
    BuiltinTheme {
        name: "Midnight",
        dark: true,
        palette: Palette {
            background: [14, 18, 30],
            panel: [24, 29, 44],
            window: [28, 34, 52],
            faint: [33, 39, 58],
            text: [214, 222, 236],
            accent: [90, 130, 230],
            widget: [44, 52, 76],
            widget_hover: [58, 68, 98],
            border: [58, 68, 96],
        },
    },
    BuiltinTheme {
        name: "Slate",
        dark: true,
        palette: Palette {
            background: [22, 23, 25],
            panel: [37, 38, 41],
            window: [42, 43, 46],
            faint: [47, 48, 52],
            text: [222, 222, 224],
            accent: [120, 160, 200],
            widget: [58, 60, 64],
            widget_hover: [74, 76, 81],
            border: [70, 72, 77],
        },
    },
    BuiltinTheme {
        name: "Forest",
        dark: true,
        palette: Palette {
            background: [16, 24, 18],
            panel: [26, 36, 28],
            window: [30, 42, 32],
            faint: [34, 47, 36],
            text: [214, 228, 210],
            accent: [110, 170, 90],
            widget: [44, 60, 46],
            widget_hover: [58, 78, 60],
            border: [58, 78, 60],
        },
    },
    BuiltinTheme {
        name: "Ember",
        dark: true,
        palette: Palette {
            background: [24, 16, 14],
            panel: [36, 25, 22],
            window: [42, 29, 25],
            faint: [46, 32, 28],
            text: [232, 218, 206],
            accent: [205, 85, 50],
            widget: [62, 42, 36],
            widget_hover: [80, 55, 47],
            border: [80, 56, 48],
        },
    },
    BuiltinTheme {
        name: "Nightshade",
        dark: true,
        palette: Palette {
            background: [22, 18, 30],
            panel: [34, 28, 46],
            window: [39, 32, 52],
            faint: [44, 36, 58],
            text: [226, 218, 240],
            accent: [170, 120, 230],
            widget: [56, 46, 76],
            widget_hover: [72, 60, 96],
            border: [72, 60, 96],
        },
    },
    BuiltinTheme {
        name: "Amber terminal",
        dark: true,
        palette: Palette {
            background: [12, 10, 6],
            panel: [22, 18, 10],
            window: [26, 21, 12],
            faint: [31, 25, 14],
            text: [255, 190, 90],
            accent: [220, 140, 30],
            widget: [44, 34, 18],
            widget_hover: [60, 46, 24],
            border: [92, 68, 30],
        },
    },
    BuiltinTheme {
        name: "High contrast (dark)",
        dark: true,
        palette: Palette {
            background: g(0),
            panel: g(0),
            window: g(0),
            faint: g(28),
            text: g(255),
            accent: [255, 200, 0],
            widget: g(40),
            widget_hover: g(72),
            border: g(255),
        },
    },
];

pub const FOLLOW_SYSTEM: &str = "Follow system";

pub fn builtin(name: &str) -> Option<&'static BuiltinTheme> {
    BUILTIN.iter().find(|t| t.name.eq_ignore_ascii_case(name))
}

pub fn c(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Linear mix: t = 0 gives `a`, t = 1 gives `b`.
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2])]
}

/// Relative luminance 0..1 (sRGB approximation), used for contrast decisions.
pub fn luminance(rgb: Rgb) -> f32 {
    (0.2126 * rgb[0] as f32 + 0.7152 * rgb[1] as f32 + 0.0722 * rgb[2] as f32) / 255.0
}

impl Palette {
    /// Fill for selected items: the accent softened toward the panel so text stays readable.
    pub fn selection(&self, dark: bool) -> Rgb {
        mix(self.panel, self.accent, if dark { 0.55 } else { 0.38 })
    }
    /// Background of the navigation strip: a little apart from the panels.
    pub fn nav(&self, dark: bool) -> Rgb {
        mix(self.panel, self.text, if dark { 0.05 } else { 0.06 })
    }
}

/// Build egui visuals from a theme.
pub fn visuals(theme: &Theme) -> Visuals {
    let p = &theme.palette;
    let mut v = if theme.dark { Visuals::dark() } else { Visuals::light() };
    let text = c(p.text);
    v.override_text_color = None;
    v.panel_fill = c(p.panel);
    v.window_fill = c(p.window);
    v.extreme_bg_color = c(p.background);
    v.text_edit_bg_color = Some(c(p.background));
    v.faint_bg_color = c(p.faint);
    v.code_bg_color = c(p.faint);
    v.window_stroke = Stroke::new(1.0_f32, c(p.border));
    v.hyperlink_color = c(if theme.dark {
        mix(p.accent, p.text, 0.25)
    } else {
        mix(p.accent, p.text, 0.15)
    });
    v.selection.bg_fill = c(p.selection(theme.dark));
    v.selection.stroke = Stroke::new(1.0_f32, text);

    let r = egui::CornerRadius::same(2);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = c(p.panel);
    w.noninteractive.weak_bg_fill = c(p.panel);
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, c(p.border));
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, text);
    w.inactive.bg_fill = c(p.widget);
    w.inactive.weak_bg_fill = c(p.widget);
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0_f32, text);
    w.hovered.bg_fill = c(p.widget_hover);
    w.hovered.weak_bg_fill = c(p.widget_hover);
    w.hovered.bg_stroke = Stroke::new(1.0_f32, c(mix(p.border, p.accent, 0.6)));
    w.hovered.fg_stroke = Stroke::new(1.5_f32, text);
    w.active.bg_fill = c(p.selection(theme.dark));
    w.active.weak_bg_fill = c(p.selection(theme.dark));
    w.active.bg_stroke = Stroke::new(1.0_f32, c(p.accent));
    w.active.fg_stroke = Stroke::new(2.0_f32, text);
    w.open.bg_fill = c(p.widget_hover);
    w.open.weak_bg_fill = c(p.widget_hover);
    w.open.bg_stroke = Stroke::new(1.0_f32, c(p.border));
    w.open.fg_stroke = Stroke::new(1.0_f32, text);
    for x in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        x.corner_radius = r;
    }
    v.window_corner_radius = egui::CornerRadius::same(3);
    v.menu_corner_radius = r;
    v.window_shadow = egui::Shadow::NONE;
    v.popup_shadow = egui::Shadow {
        offset: [0, 2],
        blur: 6,
        spread: 0,
        color: Color32::from_black_alpha(if theme.dark { 110 } else { 40 }),
    };
    v.striped = true;
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_unique_and_readable() {
        let mut names: Vec<_> = BUILTIN.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), BUILTIN.len());
        for t in BUILTIN {
            let p = &t.palette;
            // text must contrast with panels and fields
            let d1 = (luminance(p.text) - luminance(p.panel)).abs();
            let d2 = (luminance(p.text) - luminance(p.background)).abs();
            let d3 = (luminance(p.text) - luminance(p.selection(t.dark))).abs();
            assert!(d1 > 0.5 && d2 > 0.5, "{} text contrast {d1} {d2}", t.name);
            assert!(d3 > 0.3, "{} selected-text contrast {d3}", t.name);
            assert_eq!(t.dark, luminance(p.panel) < 0.5, "{} dark flag", t.name);
        }
    }

    #[test]
    fn mixing() {
        assert_eq!(mix([0, 0, 0], [255, 255, 255], 0.5), [128, 128, 128]);
        assert_eq!(mix([10, 20, 30], [10, 20, 30], 0.7), [10, 20, 30]);
    }
}
