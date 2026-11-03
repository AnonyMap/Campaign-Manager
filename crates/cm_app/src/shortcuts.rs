//! Every keyboard shortcut in one place: the keys, what they do, and how to show them.
//!
//! Shortcuts use egui's "command" modifier, which is ⌘ on macOS and Ctrl on Windows and
//! Linux, and labels come from [`label`], so menus and tooltips read "⌘S" on a Mac and
//! "Ctrl+S" elsewhere.

use egui::{Key, KeyboardShortcut, Modifiers};

const CMD: Modifiers = Modifiers::COMMAND;
const CMD_SHIFT: Modifiers = Modifiers::COMMAND.plus(Modifiers::SHIFT);
const CMD_ALT: Modifiers = Modifiers::COMMAND.plus(Modifiers::ALT);

const fn cmd(k: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(CMD, k)
}
const fn cmd_shift(k: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(CMD_SHIFT, k)
}
const fn cmd_alt(k: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(CMD_ALT, k)
}

// -- everywhere ---------------------------------------------------------------
pub const SAVE: KeyboardShortcut = cmd(Key::S);
pub const UNDO: KeyboardShortcut = cmd(Key::Z);
pub const REDO: KeyboardShortcut = cmd_shift(Key::Z);
pub const REDO_ALT: KeyboardShortcut = cmd(Key::Y);
pub const FIND: KeyboardShortcut = cmd(Key::F);
pub const GO_TO: KeyboardShortcut = cmd(Key::P);
pub const DICE: KeyboardShortcut = cmd(Key::D);
pub const NEW_CARD: KeyboardShortcut = cmd(Key::N);
pub const NEW_NPC: KeyboardShortcut = cmd_shift(Key::N);
pub const OPEN: KeyboardShortcut = cmd(Key::O);
pub const SETTINGS: KeyboardShortcut = cmd(Key::Comma);
pub const BACK: KeyboardShortcut = cmd(Key::OpenBracket);
pub const FORWARD: KeyboardShortcut = cmd(Key::CloseBracket);
pub const BIGGER: KeyboardShortcut = cmd(Key::Equals);
pub const BIGGER_ALT: KeyboardShortcut = cmd(Key::Plus);
pub const SMALLER: KeyboardShortcut = cmd(Key::Minus);
pub const NORMAL_SIZE: KeyboardShortcut = cmd(Key::Num0);
pub const SHORTCUTS: KeyboardShortcut = cmd(Key::Slash);
pub const HELP: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F1);

/// ⌘/Ctrl + 1…9 jump straight to these screens.
pub const VIEW_KEYS: [Key; 9] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
];

// -- writing (in a formatted or Markdown text box) ------------------------------
pub const BOLD: KeyboardShortcut = cmd(Key::B);
pub const ITALIC: KeyboardShortcut = cmd(Key::I);
pub const STRIKE: KeyboardShortcut = cmd_shift(Key::X);
pub const CODE: KeyboardShortcut = cmd(Key::E);
pub const LINK: KeyboardShortcut = cmd(Key::K);
pub const NUMBERED: KeyboardShortcut = cmd_shift(Key::Num7);
pub const BULLETS: KeyboardShortcut = cmd_shift(Key::Num8);
pub const CHECKLIST: KeyboardShortcut = cmd_shift(Key::Num9);
pub const HEADING_1: KeyboardShortcut = cmd_alt(Key::Num1);
pub const HEADING_2: KeyboardShortcut = cmd_alt(Key::Num2);
pub const HEADING_3: KeyboardShortcut = cmd_alt(Key::Num3);
pub const PLAIN_TEXT: KeyboardShortcut = cmd_alt(Key::Num0);

/// How a shortcut is written on this computer ("⌘⇧Z" on a Mac, "Ctrl+Shift+Z" elsewhere).
pub fn label(ctx: &egui::Context, sc: &KeyboardShortcut) -> String {
    let text = ctx.format_shortcut(sc);
    // egui spells some keys out ("OpenBracket"): show the key itself
    let name = sc.logical_key.name();
    let symbol = match sc.logical_key {
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Equals => "=",
        Key::Plus => "+",
        Key::Minus => "−",
        Key::Comma => ",",
        Key::Slash => "/",
        Key::Period => ".",
        _ => return text,
    };
    match text.strip_suffix(name) {
        Some(head) => format!("{head}{symbol}"),
        None => text,
    }
}

/// "Bold (⌘B)" / "Bold (Ctrl+B)".
pub fn with(ctx: &egui::Context, what: &str, sc: &KeyboardShortcut) -> String {
    format!("{what} ({})", label(ctx, sc))
}

/// Take a shortcut out of this frame's input if it was pressed. The modifiers must match
/// exactly (egui's own matching lets ⌘Z fire while Shift is held, so ⌘⇧8 would also be ⌘8).
pub fn pressed(ctx: &egui::Context, sc: &KeyboardShortcut) -> bool {
    ctx.input_mut(|i| take_exact(i, sc))
}

/// [`pressed`] for code that already holds the input.
pub fn take_exact(i: &mut egui::InputState, sc: &KeyboardShortcut) -> bool {
    let want = sc.modifiers;
    let pos = i.events.iter().position(|e| {
        matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. }
            if *key == sc.logical_key
                && modifiers.command == want.command
                && modifiers.shift == want.shift
                && modifiers.alt == want.alt
                // (on a Mac, Ctrl is separate from ⌘: Ctrl+B is not ⌘B)
                && (want.command || !modifiers.ctrl))
    });
    match pos {
        Some(p) => {
            i.events.remove(p);
            true
        }
        None => false,
    }
}

/// True when the keyboard belongs to a text box, so single-key shortcuts (N, ↑…) must not fire.
pub fn typing(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.focused().is_some()) && ctx.wants_keyboard_input()
}

/// One row of the cheat sheet.
pub enum Keys {
    Sc(&'static [KeyboardShortcut]),
    Text(&'static str),
}

pub const SHEET: &[(&str, &[(Keys, &str)])] = &[
    (
        "Everywhere",
        &[
            (Keys::Sc(&[GO_TO]), "Go to… any card, person, place, item or screen by name"),
            (Keys::Sc(&[FIND]), "Search the campaign and wiki"),
            (Keys::Sc(&[SAVE]), "Save now"),
            (Keys::Sc(&[UNDO]), "Undo"),
            (Keys::Sc(&[REDO, REDO_ALT]), "Redo"),
            (Keys::Sc(&[BACK]), "Back to the previous screen or record"),
            (Keys::Sc(&[FORWARD]), "Forward again"),
            (
                Keys::Text("⌘/Ctrl + 1 … 9"),
                "Overview, Run session, Story cards, Story board, NPCs, Places, Encounters, Combat, Wiki",
            ),
            (Keys::Sc(&[NEW_CARD]), "New story card"),
            (Keys::Sc(&[NEW_NPC]), "New NPC"),
            (Keys::Sc(&[DICE]), "Show / hide the dice roller"),
            (Keys::Sc(&[BIGGER, SMALLER, NORMAL_SIZE]), "Larger / smaller / normal text"),
            (Keys::Sc(&[OPEN]), "Open another campaign"),
            (Keys::Sc(&[SETTINGS]), "Settings"),
            (Keys::Sc(&[SHORTCUTS]), "This list of shortcuts"),
            (Keys::Sc(&[HELP]), "Dice syntax help"),
            (Keys::Text("Mouse back / forward buttons"), "Back / forward"),
            (Keys::Text("Esc"), "Close the window or menu on top"),
        ],
    ),
    (
        "Lists (when not typing)",
        &[
            (Keys::Text("↑ / ↓"), "Previous / next item in the list"),
            (Keys::Text("Home / End"), "First / last item"),
        ],
    ),
    (
        "Writing (in a text box)",
        &[
            (Keys::Sc(&[BOLD]), "Bold"),
            (Keys::Sc(&[ITALIC]), "Italic"),
            (Keys::Sc(&[STRIKE]), "Strikethrough"),
            (Keys::Sc(&[CODE]), "Code"),
            (Keys::Sc(&[LINK]), "[[Link]] to a card, NPC or wiki page"),
            (Keys::Sc(&[HEADING_1, HEADING_2, HEADING_3]), "Heading 1 / 2 / 3"),
            (Keys::Sc(&[PLAIN_TEXT]), "Normal text (no heading)"),
            (Keys::Sc(&[BULLETS]), "Bulleted list"),
            (Keys::Sc(&[NUMBERED]), "Numbered list"),
            (Keys::Sc(&[CHECKLIST]), "Checklist"),
            (Keys::Text("Enter"), "In a list: next item (Enter on an empty item ends the list)"),
            (Keys::Text("Tab / Shift+Tab"), "In a list: indent / outdent the item"),
        ],
    ),
    (
        "Combat tracker (when not typing)",
        &[
            (Keys::Text("N  or  →"), "Next turn (starts combat)"),
            (Keys::Text("B  or  ←"), "Back a turn"),
            (Keys::Text("↑ / ↓"), "Select the previous / next combatant"),
            (Keys::Text("R"), "Roll initiative for the NPCs"),
            (Keys::Text("A"), "Add creatures…"),
            (Keys::Text("Enter in an HP box"), "Damage (start with + to heal)"),
        ],
    ),
    (
        "Dice roller",
        &[(Keys::Text("Enter"), "Roll"), (Keys::Text("↑ / ↓"), "Earlier / later rolls you typed")],
    ),
];

/// The cheat sheet window.
pub fn sheet_window(ctx: &egui::Context, open: &mut bool) {
    let mac = ctx.os().is_mac();
    egui::Window::new("Keyboard shortcuts")
        .open(open)
        .collapsible(false)
        .default_width(560.0)
        .show(ctx, |ui| {
            ui.weak(if mac {
                "On this Mac, ⌘ is the Command key, ⇧ Shift and ⌥ Option."
            } else {
                "On a Mac, use ⌘ Command wherever this list says Ctrl."
            });
            egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                for (group, rows) in SHEET {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(*group).strong());
                    egui::Grid::new(("sc_grid", *group))
                        .num_columns(2)
                        .spacing([18.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for (keys, what) in rows.iter() {
                                let k = match keys {
                                    Keys::Sc(list) => list.iter().map(|s| label(ctx, s)).collect::<Vec<_>>().join("  /  "),
                                    Keys::Text(t) if mac => t.replace("⌘/Ctrl", "⌘").replace("Shift+", "⇧"),
                                    Keys::Text(t) => t.replace("⌘/Ctrl", "Ctrl"),
                                };
                                ui.monospace(k);
                                ui.label(*what);
                                ui.end_row();
                            }
                        });
                }
            });
        });
}

/// Move a list selection with ↑ / ↓ / Home / End when nothing is being typed. `ids` is
/// the list in the order shown. Returns the index scrolled to, if the selection moved.
pub fn list_keys(ctx: &egui::Context, ids: &[cm_core::Id], selected: &mut Option<cm_core::Id>) -> Option<usize> {
    if ids.is_empty() || typing(ctx) || egui::Popup::is_any_open(ctx) {
        return None;
    }
    let (up, down, home, end) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::Home),
            i.consume_key(Modifiers::NONE, Key::End),
        )
    });
    if !(up || down || home || end) {
        return None;
    }
    let cur = selected.and_then(|s| ids.iter().position(|x| *x == s));
    let next = if home {
        0
    } else if end {
        ids.len() - 1
    } else {
        match (cur, up) {
            (None, true) => ids.len() - 1,
            (None, false) => 0,
            (Some(c), true) => c.saturating_sub(1),
            (Some(c), false) => (c + 1).min(ids.len() - 1),
        }
    };
    *selected = Some(ids[next]);
    Some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_follow_the_platform() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |_| {}); // fonts load on the first frame
        ctx.set_os(egui::os::OperatingSystem::Windows);
        assert_eq!(label(&ctx, &SAVE), "Ctrl+S");
        assert_eq!(label(&ctx, &REDO), "Ctrl+Shift+Z");
        assert_eq!(label(&ctx, &BACK), "Ctrl+[");
        assert_eq!(label(&ctx, &SETTINGS), "Ctrl+,");
        ctx.set_os(egui::os::OperatingSystem::Mac);
        let mac = label(&ctx, &SAVE);
        assert!(mac.contains('⌘') || mac.contains("Cmd"), "{mac}");
        assert!(!mac.contains("Ctrl"));
    }

    #[test]
    fn no_shortcut_is_used_twice() {
        let all = [
            SAVE,
            UNDO,
            REDO,
            REDO_ALT,
            FIND,
            GO_TO,
            DICE,
            NEW_CARD,
            NEW_NPC,
            OPEN,
            SETTINGS,
            BACK,
            FORWARD,
            BIGGER,
            BIGGER_ALT,
            SMALLER,
            NORMAL_SIZE,
            SHORTCUTS,
            HELP,
            BOLD,
            ITALIC,
            STRIKE,
            CODE,
            LINK,
            NUMBERED,
            BULLETS,
            CHECKLIST,
            HEADING_1,
            HEADING_2,
            HEADING_3,
            PLAIN_TEXT,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert!(a != b, "{a:?} is used twice");
            }
        }
        // ⌘1…9 (views) and ⌘⌥1…3 (headings) differ by the Alt key
        assert!(VIEW_KEYS.iter().all(|k| *k != Key::Num0));
    }

    #[test]
    fn arrow_keys_walk_a_list() {
        let ids: Vec<cm_core::Id> = (0..3).map(|_| cm_core::new_id()).collect();
        let ctx = egui::Context::default();
        let mut sel = None;
        let press = |key| egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            ..Default::default()
        };
        let step = |key, sel: &mut Option<cm_core::Id>| {
            let _ = ctx.run(press(key), |ctx| {
                list_keys(ctx, &ids, sel);
            });
        };
        step(Key::ArrowDown, &mut sel);
        assert_eq!(sel, Some(ids[0]));
        step(Key::ArrowDown, &mut sel);
        step(Key::ArrowDown, &mut sel);
        step(Key::ArrowDown, &mut sel);
        assert_eq!(sel, Some(ids[2]));
        step(Key::Home, &mut sel);
        assert_eq!(sel, Some(ids[0]));
        step(Key::ArrowUp, &mut sel);
        assert_eq!(sel, Some(ids[0]));
    }
}
