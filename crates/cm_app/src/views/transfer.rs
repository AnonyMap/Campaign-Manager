//! Import / export window: export parts of this campaign to a package file, import parts of
//! a package (or another campaign folder) into this campaign, or start a new campaign from
//! parts of this one or of a package ("same world, new story").

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use cm_core::transfer::{self, Duplicates, Part, Preset, Source};
use egui::RichText;

use crate::app::{Action, Doc, Shared};
use crate::ui_util;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Export,
    Import,
    /// New campaign from the open one.
    NewFromThis,
    /// New campaign from a package file or another campaign folder.
    NewFromFile,
}

#[derive(Default)]
pub struct State {
    pub open: bool,
    mode: Mode,
    source: Option<Source>,
    parts: BTreeSet<Part>,
    name: String,
    parent: String,
    dup: Duplicates,
    error: Option<String>,
    path_buf: String,
}

impl State {
    /// Open the window. `doc` is the open campaign, if any.
    pub fn start(&mut self, mode: Mode, doc: Option<&Doc>) {
        *self = State {
            open: true,
            mode,
            ..Default::default()
        };
        if let (Some(d), Mode::Export | Mode::NewFromThis) = (doc, mode) {
            self.source = Some(Source {
                campaign: d.c.clone(),
                assets: transfer::AssetSource::Dir(d.dir.clone()),
                label: d.c.name.clone(),
            });
        }
        if let Some(d) = doc {
            self.parent = d.dir.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        }
        if self.parent.is_empty() {
            self.parent = crate::settings::default_campaigns_dir().to_string_lossy().to_string();
        }
        self.apply_preset(if mode == Mode::Export { Preset::Everything } else { Preset::Setting });
        if mode == Mode::NewFromThis {
            if let Some(d) = doc {
                self.name = format!("{} — new campaign", d.c.name);
            }
        }
    }

    /// Open the import window with a file (e.g. one dropped on the window).
    pub fn start_import(&mut self, path: &Path, doc: Option<&Doc>) {
        self.start(if doc.is_some() { Mode::Import } else { Mode::NewFromFile }, doc);
        self.load(path);
    }

    fn load(&mut self, path: &Path) {
        match Source::open(path) {
            Ok(src) => {
                self.error = None;
                if self.mode == Mode::Import {
                    // offer what's in it
                    self.parts = transfer::parts_present(&src.campaign).into_iter().collect();
                } else {
                    self.parts.retain(|p| p.count(&src.campaign) > 0);
                    if self.parts.is_empty() {
                        self.parts = transfer::parts_present(&src.campaign).into_iter().collect();
                    }
                }
                if self.name.trim().is_empty() {
                    self.name = src.campaign.name.clone();
                }
                self.source = Some(src);
            }
            Err(e) => self.error = Some(format!("Couldn't read {}: {e}", path.display())),
        }
    }

    fn apply_preset(&mut self, p: Preset) {
        self.parts = p.parts().into_iter().collect();
    }

    fn matches(&self, p: Preset) -> bool {
        let want: BTreeSet<Part> = p.parts().into_iter().collect();
        let avail = |x: &Part| self.source.as_ref().map(|s| x.count(&s.campaign) > 0).unwrap_or(true);
        want.iter().filter(|x| avail(x)).collect::<Vec<_>>() == self.parts.iter().filter(|x| avail(x)).collect::<Vec<_>>()
    }
}

fn pick_source() -> Option<PathBuf> {
    ui_util::safe_dialog(|| {
        rfd::FileDialog::new()
            .set_title("Choose a campaign package or campaign.json")
            .add_filter("Campaign packages", &[transfer::PACKAGE_EXT, "zip", "json"])
            .add_filter("All files", &["*"])
            .pick_file()
    })
}

/// Draw the window. Returns nothing; actions (open the new campaign, toasts) go through `sh`.
pub fn window(ctx: &egui::Context, doc: Option<&mut Doc>, sh: &mut Shared, st: &mut State) {
    if !st.open {
        return;
    }
    let title = match st.mode {
        Mode::Export => "Export",
        Mode::Import => "Import into this campaign",
        Mode::NewFromThis => "New campaign from this one",
        Mode::NewFromFile => "New campaign from a file",
    };
    let mut open = true;
    let mut doc = doc;
    egui::Window::new(title)
        .id(egui::Id::new("transfer_window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(540.0)
        .max_width(720.0)
        .show(ctx, |ui| body(ui, doc.as_deref_mut(), sh, st));
    if !open {
        st.open = false;
    }
}

fn body(ui: &mut egui::Ui, doc: Option<&mut Doc>, sh: &mut Shared, st: &mut State) {
    ui.label(match st.mode {
        Mode::Export => "Save some or all of this campaign as a package file — to back it up, share it, or bring it into another campaign.",
        Mode::Import => "Bring things from a package or another campaign into this one. Links to things you leave out are dropped.",
        Mode::NewFromThis => "Start a new campaign that reuses parts of this one — for example the same world with a new story.",
        Mode::NewFromFile => "Start a new campaign from a package or another campaign folder.",
    });
    ui.add_space(4.0);

    // the source
    if matches!(st.mode, Mode::Import | Mode::NewFromFile) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("📂 Choose a file…").on_hover_text("A .cmpack package, or a campaign.json").clicked() {
                if let Some(p) = pick_source() {
                    st.load(&p);
                }
            }
            if ui.button("📁 Choose a campaign folder…").on_hover_text("Another campaign's folder").clicked() {
                if let Some(p) = ui_util::pick_folder("Choose a campaign folder") {
                    st.load(&p);
                }
            }
        });
        // right to left: the button first, the box gets exactly what's left (so the window can't creep wider)
        let h = ui.spacing().interact_size.y;
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), h), egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let open = ui.add_enabled(!st.path_buf.trim().is_empty(), egui::Button::new("Open")).clicked();
            let r = ui.add(
                egui::TextEdit::singleline(&mut st.path_buf)
                    .hint_text("…or paste a path")
                    .desired_width(f32::INFINITY),
            );
            if open || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !st.path_buf.trim().is_empty()) {
                let p = PathBuf::from(st.path_buf.trim().trim_matches('"'));
                st.load(&p);
            }
        });
        if let Some(e) = &st.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
    }
    let Some(src) = st.source.as_ref() else {
        ui.weak("Nothing chosen yet.");
        return;
    };
    if matches!(st.mode, Mode::Import | Mode::NewFromFile) {
        ui.label(RichText::new(format!("From: {} ({})", src.campaign.name, src.label)).strong());
    }
    ui.separator();

    // what to take
    ui.horizontal_wrapped(|ui| {
        ui.label("Take:");
        for p in Preset::ALL {
            let on = st.matches(p);
            if ui.selectable_label(on, p.label()).on_hover_text(p.description()).clicked() {
                st.apply_preset(p);
            }
        }
    });
    let src = st.source.as_ref().unwrap();
    let mut toggles: Vec<(Part, bool)> = Vec::new();
    egui::Grid::new("transfer_parts").num_columns(2).spacing([16.0, 2.0]).show(ui, |ui| {
        for (i, p) in Part::ALL.into_iter().enumerate() {
            let n = p.count(&src.campaign);
            let mut on = st.parts.contains(&p) && n > 0;
            let text = if n > 0 {
                format!("{} ({n})", p.label())
            } else {
                format!("{} (none)", p.label())
            };
            if ui.add_enabled(n > 0, egui::Checkbox::new(&mut on, text)).changed() {
                toggles.push((p, on));
            }
            if i % 2 == 1 {
                ui.end_row();
            }
        }
    });
    for (p, on) in toggles {
        if on {
            st.parts.insert(p);
        } else {
            st.parts.remove(&p);
        }
    }
    let parts: Vec<Part> = st.parts.iter().copied().filter(|p| p.count(&src.campaign) > 0).collect();
    let images = transfer::asset_paths(&transfer::extract(&src.campaign, &parts)).len();
    ui.weak(format!(
        "{} selected{}",
        ui_util::plural(parts.iter().filter(|p| **p != Part::Rules).map(|p| p.count(&src.campaign)).sum(), "thing"),
        if images > 0 {
            format!(", with {}", ui_util::plural(images, "image"))
        } else {
            String::new()
        }
    ));
    if parts.contains(&Part::Npcs) && !parts.contains(&Part::Bestiary) && src.campaign.npcs.iter().any(|n| n.statblock.is_some()) {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            "Some NPCs use bestiary stat blocks — tick the bestiary too, or they'll lose them.",
        );
    }
    if parts.contains(&Part::Sessions) && !parts.contains(&Part::Story) && !src.campaign.story_cards.is_empty() {
        ui.weak("Sessions without the story cards keep their notes but lose their planned cards.");
    }
    ui.separator();

    match st.mode {
        Mode::Export => {
            let Some(doc) = doc else { return };
            if ui
                .add_enabled(!parts.is_empty(), egui::Button::new(RichText::new("💾 Export…").strong()))
                .clicked()
            {
                let preset = Preset::ALL.into_iter().find(|p| st.matches(*p)).filter(|p| *p != Preset::Everything);
                let file = match preset {
                    Some(p) => format!(
                        "{} - {}.{}",
                        cm_core::storage::sanitize_file_name(&doc.c.name),
                        p.label(),
                        transfer::PACKAGE_EXT
                    ),
                    None => format!("{}.{}", cm_core::storage::sanitize_file_name(&doc.c.name), transfer::PACKAGE_EXT),
                };
                if let Some(dest) = ui_util::save_file_dialog("Export campaign", &file, ("Campaign package", &[transfer::PACKAGE_EXT])) {
                    let dest = if dest.extension().is_none() {
                        dest.with_extension(transfer::PACKAGE_EXT)
                    } else {
                        dest
                    };
                    match transfer::export_package(&doc.c, &doc.dir, &parts, &dest) {
                        Ok(r) => {
                            let mut msg = format!(
                                "Exported {} and {} to {}",
                                ui_util::plural(r.things, "thing"),
                                ui_util::plural(r.images, "image"),
                                dest.display()
                            );
                            if r.missing_images > 0 {
                                msg.push_str(&format!(" ({} missing from the assets folder)", r.missing_images));
                            }
                            sh.toast(msg);
                            st.open = false;
                        }
                        Err(e) => st.error = Some(format!("Export failed: {e}")),
                    }
                }
            }
            if let Some(e) = &st.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
        }
        Mode::Import => {
            let Some(doc) = doc else {
                ui.weak("Open a campaign first.");
                return;
            };
            let dups = transfer::count_duplicates(&doc.c, &src.campaign, &parts);
            if dups > 0 {
                ui.label(if dups == 1 {
                    "1 thing is already in this campaign (it came from the same place):".to_string()
                } else {
                    format!("{dups} things are already in this campaign (they came from the same place):")
                });
                ui.horizontal_wrapped(|ui| {
                    ui.radio_value(&mut st.dup, Duplicates::Skip, "Keep mine")
                        .on_hover_text("Leave what's here as it is");
                    ui.radio_value(&mut st.dup, Duplicates::Replace, "Replace with the imported version");
                    ui.radio_value(&mut st.dup, Duplicates::KeepBoth, "Keep both")
                        .on_hover_text("Import separate copies");
                });
            }
            if ui
                .add_enabled(!parts.is_empty(), egui::Button::new(RichText::new("📥 Import").strong()))
                .clicked()
            {
                doc.checkpoint();
                let dir = doc.dir.clone();
                let src = st.source.clone().unwrap();
                match transfer::import_into(&mut doc.c, &dir, &src, &parts, st.dup) {
                    Ok((r, imgs)) => {
                        doc.touched = true;
                        doc.checkpoint();
                        let img = if imgs > 0 {
                            format!(", {} copied", ui_util::plural(imgs, "image"))
                        } else {
                            String::new()
                        };
                        sh.toast(format!("Imported: {}{img}. Edit » Undo takes it back.", r.summary()));
                        st.open = false;
                    }
                    Err(e) => st.error = Some(format!("Import failed: {e}")),
                }
            }
            if let Some(e) = &st.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
        }
        Mode::NewFromThis | Mode::NewFromFile => {
            egui::Grid::new("transfer_new").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(320.0));
                ui.end_row();
                ui.label("In folder");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut st.parent).desired_width(260.0));
                    if ui.button("Browse…").clicked() {
                        if let Some(p) = ui_util::pick_folder("Where to create the campaign") {
                            st.parent = p.to_string_lossy().to_string();
                        }
                    }
                });
                ui.end_row();
            });
            let ok = !parts.is_empty() && !st.parent.trim().is_empty();
            if ui.add_enabled(ok, egui::Button::new(RichText::new("✨ Create and open").strong())).clicked() {
                // build from the latest state of the open campaign
                let src = match (st.mode, doc) {
                    (Mode::NewFromThis, Some(d)) => Source {
                        campaign: d.c.clone(),
                        assets: transfer::AssetSource::Dir(d.dir.clone()),
                        label: d.c.name.clone(),
                    },
                    _ => st.source.clone().unwrap(),
                };
                match transfer::new_campaign_from(&src, &parts, &st.name, Path::new(st.parent.trim())) {
                    Ok((dir, r)) => {
                        sh.actions.push(Action::OpenCampaign(dir));
                        sh.actions.push(Action::Toast(format!("New campaign created: {}", r.summary())));
                        st.open = false;
                    }
                    Err(e) => st.error = Some(format!("Couldn't create the campaign: {e}")),
                }
            }
            if let Some(e) = &st.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
        }
    }
}
