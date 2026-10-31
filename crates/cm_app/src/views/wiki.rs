//! Campaign wiki: browse, search, read, edit and create notes in an Obsidian vault.

use std::collections::BTreeMap;

use cm_core::wiki::Vault;
use egui::RichText;

use crate::app::{Action, Doc, Shared};
use crate::ui_util;

#[derive(Default)]
pub struct State {
    open: Option<String>,
    query: String,
    editing: Option<String>,
    /// The note text when editing started (to spot unsaved changes and edits made in Obsidian).
    editing_base: String,
    /// Asked to open another note while this one has unsaved changes.
    pending_open: Option<String>,
    /// Asked to quit while this note has unsaved changes.
    pub pending_quit: bool,
    /// The file changed on disk since editing started; saving needs a second click.
    conflict: bool,
    /// Cached "Linked from" list: (note, notes in vault, [(title, path)]).
    backlinks: Option<(String, usize, Vec<(String, String)>)>,
    /// Cached search: (query, notes in vault, hits).
    search: Option<(String, usize, Vec<(String, String, String)>)>,
    new_title: String,
    new_folder: String,
    show_new: bool,
    path_buf: String,
    pub pending_reload: bool,
    tag_filter: Option<String>,
}

impl State {
    pub fn open(&mut self, rel: &str) {
        if self.unsaved() && self.open.as_deref() != Some(rel) {
            self.pending_open = Some(rel.to_string());
            return;
        }
        self.open = Some(rel.to_string());
        self.editing = None;
        self.conflict = false;
    }
    fn unsaved(&self) -> bool {
        self.editing.as_ref().map(|b| *b != self.editing_base).unwrap_or(false)
    }
    /// The note being edited, if it has unsaved changes.
    pub fn has_unsaved(&self) -> Option<String> {
        if self.unsaved() { self.open.clone() } else { None }
    }
    fn start_editing(&mut self, content: String) {
        self.editing_base = content.clone();
        self.editing = Some(content);
        self.conflict = false;
    }
    pub fn offer_create(&mut self, title: &str) {
        self.show_new = true;
        self.new_title = title.to_string();
    }
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    if doc.c.settings.wiki_path.as_deref().map(|p| p.trim().is_empty()).unwrap_or(true) || (sh.vault.is_none() && sh.vault_error.is_some()) {
        setup(ui, doc, sh, st);
        return;
    }
    let Some(vault) = sh.vault.take() else {
        ui.spinner();
        return;
    };
    let mut vault = vault;
    egui::SidePanel::left("wiki_tree").resizable(true).default_width(260.0).show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Wiki");
            ui.weak(format!("{} notes", vault.notes.len()));
        });
        ui.horizontal(|ui| {
            if ui.button("➕ New note").clicked() {
                st.show_new = true;
                st.new_folder = st
                    .open
                    .as_deref()
                    .and_then(|p| p.rsplit_once('/').map(|(f, _)| f.to_string()))
                    .unwrap_or_default();
            }
            if ui.button("⟳ Reload").on_hover_text("Re-read the vault from disk").clicked() {
                if let Err(e) = vault.reload() {
                    sh.toast(format!("Reload failed: {e}"));
                }
                st.backlinks = None;
                st.search = None;
            }
            ui.menu_button("…", |ui| {
                if ui.button("Change vault folder…").clicked() {
                    doc.c.settings.wiki_path = None;
                    ui.close();
                }
                if ui.button("Export all campaign items to the wiki").clicked() {
                    sh.actions.push(Action::ExportAll);
                    ui.close();
                }
            });
        });
        ui_util::filter_box(ui, &mut st.query, "Search notes (tag:x, path:y)…");
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            if !st.query.trim().is_empty() {
                let fresh = st.search.as_ref().map(|(q, n, _)| *q == st.query && *n == vault.notes.len()).unwrap_or(false);
                if !fresh {
                    let hits = vault
                        .search(&st.query)
                        .into_iter()
                        .take(200)
                        .map(|h| (h.rel_path, h.title, h.snippet))
                        .collect();
                    st.search = Some((st.query.clone(), vault.notes.len(), hits));
                }
                let hits = st.search.as_ref().map(|s| s.2.clone()).unwrap_or_default();
                if hits.is_empty() {
                    ui.weak("No matches.");
                }
                for (rel, title, snippet) in &hits {
                    let r = ui_util::list_row(ui, st.open.as_deref() == Some(rel), RichText::new(title));
                    if r.clicked() {
                        st.open(rel);
                    }
                    ui.label(RichText::new(snippet).small().weak());
                }
            } else {
                tree(ui, &vault, st);
                ui.separator();
                egui::CollapsingHeader::new("Tags").default_open(false).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (t, n) in vault.all_tags() {
                            if ui.small_button(format!("#{t} ({n})")).clicked() {
                                st.query = format!("tag:{t}");
                            }
                        }
                    });
                });
            }
        });
    });

    if st.show_new {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new("New note").strong());
            egui::Grid::new("newnote").num_columns(2).show(ui, |ui| {
                ui.label("Title");
                ui.add_sized(
                    [260.0, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut st.new_title).desired_width(260.0),
                );
                ui.end_row();
                ui.label("Folder");
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [200.0, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(&mut st.new_folder).desired_width(200.0).hint_text("(vault root)"),
                    );
                    egui::ComboBox::from_id_salt("newfolder").selected_text("pick").width(60.0).show_ui(ui, |ui| {
                        if ui.selectable_label(false, "(root)").clicked() {
                            st.new_folder.clear();
                        }
                        for f in vault.folders() {
                            if ui.selectable_label(st.new_folder == f, &f).clicked() {
                                st.new_folder = f.clone();
                            }
                        }
                    });
                });
                ui.end_row();
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(!st.new_title.trim().is_empty(), egui::Button::new("Create")).clicked() {
                    let content = format!("# {}\n\n", st.new_title.trim());
                    match vault.create_note(&st.new_folder, st.new_title.trim(), &content) {
                        Ok(rel) => {
                            st.open(&rel);
                            st.start_editing(content);
                            st.show_new = false;
                            st.new_title.clear();
                        }
                        Err(e) => sh.toast(e.to_string()),
                    }
                }
                if ui.button("Cancel").clicked() {
                    st.show_new = false;
                }
            });
        });
    }

    match st.open.clone() {
        None => {
            ui.centered_and_justified(|ui| {
                ui.weak("Pick a note on the left, or search. [[Links]] in cards and NPC notes open matching wiki pages.");
            });
        }
        Some(rel) => match vault.note(&rel).cloned() {
            None => {
                ui.weak(format!("Note not found: {rel}"));
            }
            Some(note) => {
                ui.horizontal(|ui| {
                    ui.heading(&note.title);
                    ui.weak(&note.rel_path);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let abs = vault.abs_path(&rel);
                        if ui.button("Open in Obsidian").clicked() {
                            let url = format!("obsidian://open?path={}", percent_encode(&abs.to_string_lossy()));
                            if let Err(e) = ui_util::open_with_os(&url) {
                                sh.toast(format!("Couldn't open Obsidian: {e}"));
                            }
                        }
                        match &mut st.editing {
                            Some(_) => {
                                if ui.button("Cancel").clicked() {
                                    st.editing = None;
                                    st.conflict = false;
                                } else if ui
                                    .button(RichText::new(if st.conflict { "💾 Overwrite" } else { "💾 Save" }).strong())
                                    .clicked()
                                {
                                    save_note(&mut vault, st, &rel, sh);
                                }
                            }
                            None => {
                                if ui.button("✏ Edit").clicked() {
                                    // edit what's on disk now, in case Obsidian changed it
                                    let current = std::fs::read_to_string(vault.abs_path(&rel)).unwrap_or(note.content.clone());
                                    st.start_editing(current);
                                }
                            }
                        }
                    });
                });
                if !note.tags.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        for t in &note.tags {
                            if ui.small_button(format!("#{t}")).clicked() {
                                st.query = format!("tag:{t}");
                                st.tag_filter = Some(t.clone());
                            }
                        }
                    });
                }
                if st.conflict {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        "⚠ This note was changed outside the app (Obsidian?) since you started editing. Saving again overwrites those changes — or Cancel and Edit again to start from the new text.",
                    );
                }
                if let Some(next) = st.pending_open.clone() {
                    let mut choice = 0;
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("You have unsaved changes to this note.").strong());
                            if ui.button("💾 Save").clicked() {
                                choice = 1;
                            }
                            if ui.button("Discard").clicked() {
                                choice = 2;
                            }
                            if ui.button("Keep editing").clicked() {
                                choice = 3;
                            }
                        });
                    });
                    match choice {
                        1 => {
                            if save_note(&mut vault, st, &rel, sh) {
                                st.pending_open = None;
                                st.open(&next);
                            }
                        }
                        2 => {
                            st.editing = None;
                            st.pending_open = None;
                            st.open(&next);
                        }
                        3 => st.pending_open = None,
                        _ => {}
                    }
                }
                ui.separator();
                egui::ScrollArea::vertical().id_salt("wiki_note").show(ui, |ui| {
                    match &mut st.editing {
                        Some(buf) => {
                            ui.add(
                                egui::TextEdit::multiline(buf)
                                    .id(egui::Id::new(("wiki_edit", &rel)))
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(30),
                            );
                        }
                        None => {
                            ui_util::render_markdown(ui, &mut sh.md, note.body());
                        }
                    }
                    // backlinks scan every note, so they're worked out once per note, not per frame
                    let fresh = st.backlinks.as_ref().map(|(r, n, _)| *r == rel && *n == vault.notes.len()).unwrap_or(false);
                    if !fresh {
                        let b = vault.backlinks(&rel).into_iter().map(|b| (b.title.clone(), b.rel_path.clone())).collect();
                        st.backlinks = Some((rel.clone(), vault.notes.len(), b));
                    }
                    let back = st.backlinks.as_ref().map(|b| b.2.clone()).unwrap_or_default();
                    if !back.is_empty() {
                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            ui.weak("Linked from:");
                            for (title, path) in back {
                                if ui.link(&title).clicked() {
                                    st.open(&path);
                                }
                            }
                        });
                    }
                    let mentioned = doc.c.backlinks(&cm_core::Link::Wiki(rel.clone()));
                    if !mentioned.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            ui.weak("Campaign items linking here:");
                            for e in mentioned {
                                if ui.link(&e.name).clicked() {
                                    sh.actions.push(Action::Open(e.link));
                                }
                            }
                        });
                    }
                });
            }
        },
    }
    sh.vault = Some(vault);
}

fn tree(ui: &mut egui::Ui, vault: &Vault, st: &mut State) {
    let mut by_folder: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for n in &vault.notes {
        by_folder.entry(n.folder().to_string()).or_default().push((n.rel_path.clone(), n.title.clone()));
    }
    let folders = vault.folders();
    fn children<'a>(folders: &'a [String], parent: &str) -> Vec<&'a String> {
        folders
            .iter()
            .filter(|f| match f.rsplit_once('/') {
                Some((p, _)) => p == parent,
                None => parent.is_empty(),
            })
            .collect()
    }
    fn show_folder(ui: &mut egui::Ui, folder: &str, folders: &[String], by_folder: &BTreeMap<String, Vec<(String, String)>>, st: &mut State) {
        for child in children(folders, folder) {
            let name = child.rsplit('/').next().unwrap_or(child);
            egui::CollapsingHeader::new(format!("📁 {name}"))
                .id_salt(("wf", child))
                .default_open(false)
                .show(ui, |ui| {
                    show_folder(ui, child, folders, by_folder, st);
                });
        }
        if let Some(notes) = by_folder.get(folder) {
            for (rel, title) in notes {
                if ui_util::list_row(ui, st.open.as_deref() == Some(rel), title.as_str()).clicked() {
                    st.open(rel);
                }
            }
        }
    }
    show_folder(ui, "", &folders, &by_folder, st);
}

fn setup(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    ui.heading("Campaign wiki");
    ui.label("Point this at an Obsidian vault (any folder of Markdown notes) to browse, search and edit it here. Story cards, NPCs, factions and encounters can be exported into it as notes.");
    if let Some(e) = &sh.vault_error {
        ui.colored_label(ui.visuals().error_fg_color, e);
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Choose vault folder…").clicked() {
            if let Some(p) = ui_util::pick_folder("Choose your Obsidian vault folder") {
                doc.c.settings.wiki_path = Some(ui_util::vault_setting(&doc.dir, &p));
                st.pending_reload = true;
            }
        }
        if ui.button("Create a new vault inside the campaign folder").clicked() {
            let p = doc.dir.join("wiki");
            let _ = std::fs::create_dir_all(&p);
            if !p.join("Welcome.md").exists() {
                let _ = std::fs::write(
                    p.join("Welcome.md"),
                    "# Welcome\n\nThis vault lives inside your campaign folder. Open it in Obsidian too if you like.\n",
                );
            }
            doc.c.settings.wiki_path = Some("wiki".into());
            st.pending_reload = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label("…or paste a path:");
        ui.add_sized(
            [380.0, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut st.path_buf).desired_width(380.0),
        );
        if ui.button("Use").clicked() && !st.path_buf.trim().is_empty() {
            let typed = st.path_buf.trim().trim_matches('"').to_string();
            let p = std::path::Path::new(&typed);
            doc.c.settings.wiki_path = Some(if p.is_absolute() {
                ui_util::vault_setting(&doc.dir, p)
            } else {
                typed.clone()
            });
            st.pending_reload = true;
        }
    });
    ui.weak("Tip: you can also drag the vault folder onto this window. Relative paths are resolved from the campaign folder, which keeps them portable.");
}

/// Write the note being edited, unless the file changed on disk since editing started (then
/// warn first; a second save overwrites). Returns true when saved.
/// Save the note being edited (if it has changes) to the vault. False if it couldn't be
/// saved: no vault, a write error, or the file changed on disk meanwhile (then `conflict`
/// is set and the note view explains).
pub fn save_open_note(sh: &mut Shared, st: &mut State) -> bool {
    let Some(rel) = st.has_unsaved() else { return true };
    let Some(mut vault) = sh.vault.take() else {
        sh.toast("The wiki vault isn't loaded, so the note can't be saved");
        return false;
    };
    let ok = save_note(&mut vault, st, &rel, sh);
    sh.vault = Some(vault);
    ok
}

/// Asked to quit while a wiki note has unsaved changes: a small window (shown whatever the
/// wiki page shows, even if the note has since disappeared from the vault). Returns true
/// when the app may now close.
pub fn quit_prompt(ctx: &egui::Context, sh: &mut Shared, st: &mut State) -> bool {
    if !st.pending_quit {
        return false;
    }
    let name = st.open.clone().unwrap_or_default();
    let mut choice = 0;
    egui::Window::new("Unsaved wiki note")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!("“{name}” has changes that aren't saved yet."));
            if st.conflict {
                ui.colored_label(ui.visuals().warn_fg_color, "It was also changed outside the app; saving again overwrites that.");
            }
            ui.horizontal(|ui| {
                if ui.button("💾 Save and quit").clicked() {
                    choice = 1;
                }
                if ui.button("Quit without saving").clicked() {
                    choice = 2;
                }
                if ui.button("Cancel").clicked() {
                    choice = 3;
                }
            });
        });
    let quit = match choice {
        1 => save_open_note(sh, st),
        2 => {
            st.editing = None;
            true
        }
        _ => false,
    };
    if choice == 3 || quit {
        st.pending_quit = false;
    }
    quit
}

fn save_note(vault: &mut Vault, st: &mut State, rel: &str, sh: &mut Shared) -> bool {
    let Some(text) = st.editing.clone() else { return true };
    if !st.conflict {
        if let Ok(disk) = std::fs::read_to_string(vault.abs_path(rel)) {
            if disk != st.editing_base && disk != text {
                st.conflict = true;
                return false;
            }
        }
    }
    match vault.write_note(rel, &text) {
        Ok(()) => {
            st.editing = None;
            st.conflict = false;
            st.backlinks = None;
            st.search = None;
            sh.toast("Note saved");
            true
        }
        Err(e) => {
            sh.toast(format!("Couldn't save: {e}"));
            false
        }
    }
}
