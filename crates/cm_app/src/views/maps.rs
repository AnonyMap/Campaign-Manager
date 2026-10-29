//! Maps: upload an image, pin story cards onto it, pan/zoom, drag pins around.

use cm_core::{CampaignMap, Id, Link, MapPin};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};

use crate::app::{Action, Doc, Shared};
use crate::ui_util;

pub struct State {
    pub selected: Option<Id>,
    /// Card waiting to be placed with the next click.
    pub placing: Option<Id>,
    zoom: f32,
    pan: Vec2,
    dragging: Option<(Id, usize)>,
    focused_card: Option<Id>,
    /// Which of that card's pins (index into its map_pins).
    focused_pin: Option<usize>,
}

impl Default for State {
    fn default() -> Self {
        State {
            selected: None,
            placing: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            dragging: None,
            focused_card: None,
            focused_pin: None,
        }
    }
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    egui::SidePanel::left("maps_list").resizable(true).default_width(200.0).show_inside(ui, |ui| {
        ui.heading("Maps");
        if let Some(rel) = ui_util::add_image_button(ui, "newmap", &doc.dir, "➕ New map from image", sh) {
            let m = CampaignMap {
                name: map_name(&rel),
                image: rel,
                ..Default::default()
            };
            st.selected = Some(m.id);
            doc.c.maps.push(m);
        }
        ui.weak("or drop an image file on this window");
        ui.separator();
        egui::ScrollArea::vertical().id_salt("maps_list_scroll").show(ui, |ui| {
            for m in &doc.c.maps {
                let pins = doc.c.story_cards.iter().filter(|c| c.map_pins.iter().any(|p| p.map_id == m.id)).count();
                if ui_util::list_row(ui, st.selected == Some(m.id), format!("{}  ({pins})", m.name)).clicked() {
                    st.selected = Some(m.id);
                    st.zoom = 1.0;
                    st.pan = Vec2::ZERO;
                }
            }
        });
    });

    if st.selected.is_none() {
        st.selected = doc.c.maps.first().map(|m| m.id);
    }
    let Some(map_id) = st.selected else {
        ui.centered_and_justified(|ui| ui.weak("Add a map image to start pinning story cards to places."));
        return;
    };
    let Some(map) = doc.c.maps.iter_mut().find(|m| m.id == map_id) else {
        st.selected = None;
        return;
    };

    // toolbar
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut map.name)
                .id(egui::Id::new(("map_name", map_id)))
                .font(egui::TextStyle::Heading)
                .desired_width(240.0),
        );
        ui.separator();
        if ui.button("−").clicked() {
            st.zoom = (st.zoom / 1.25).max(0.2);
        }
        ui.label(format!("{:.0}%", st.zoom * 100.0));
        if ui.button("+").clicked() {
            st.zoom = (st.zoom * 1.25).min(10.0);
        }
        if ui.button("Fit").clicked() {
            st.zoom = 1.0;
            st.pan = Vec2::ZERO;
        }
        ui.separator();
        if let Some(rel) = ui_util::add_image_button(ui, ("replace", map_id), &doc.dir, "Replace image", sh) {
            map.image = rel;
        }
        ui_util::confirm_delete_button(ui, Link::Map(map_id), sh);
    });

    let cards: Vec<(Id, String, Option<[u8; 3]>)> = doc.c.story_cards.iter().map(|c| (c.id, c.display_title(), c.color)).collect();
    ui.horizontal(|ui| {
        ui.label("Place a card:");
        egui::ComboBox::from_id_salt("place_card")
            .width(240.0)
            .selected_text(
                st.placing
                    .and_then(|p| cards.iter().find(|c| c.0 == p))
                    .map(|c| c.1.clone())
                    .unwrap_or("Choose a card…".into()),
            )
            .show_ui(ui, |ui| {
                for (cid, name, _) in &cards {
                    if ui.selectable_label(st.placing == Some(*cid), name).clicked() {
                        st.placing = Some(*cid);
                    }
                }
            });
        if ui
            .button("➕ New card here")
            .on_hover_text("Create a card, then click the map to pin it")
            .clicked()
        {
            let card = cm_core::StoryCard::default();
            st.placing = Some(card.id);
            doc.c.story_cards.push(card);
        }
        if st.placing.is_some() {
            ui.label(RichText::new("Click on the map to place it (Esc cancels)").color(ui.visuals().hyperlink_color));
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                st.placing = None;
            }
        }
    });

    let map = doc.c.maps.iter().find(|m| m.id == map_id).unwrap().clone();
    let uri = ui_util::file_uri(&doc.dir, &map.image);
    let tex = if map.image.is_empty() {
        None
    } else {
        match ui.ctx().try_load_texture(&uri, egui::TextureOptions::LINEAR, egui::SizeHint::default()) {
            Ok(egui::load::TexturePoll::Ready { texture }) => Some(texture),
            Ok(_) => {
                ui.spinner();
                None
            }
            Err(e) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("Couldn't load map image: {e}"));
                None
            }
        }
    };

    let notes_h = 120.0;
    let avail = ui.available_size() - Vec2::new(0.0, notes_h);
    let (resp, painter) = ui.allocate_painter(avail.max(Vec2::splat(100.0)), Sense::click_and_drag());
    let canvas = resp.rect;
    painter.rect_filled(canvas, 0.0, ui.visuals().extreme_bg_color);
    if tex.is_none() {
        painter.text(
            canvas.center(),
            egui::Align2::CENTER_CENTER,
            "No image (pins and notes are kept)",
            egui::FontId::proportional(16.0),
            ui.visuals().weak_text_color(),
        );
    }
    let mut pins: Vec<(Id, usize, MapPin, String, Option<[u8; 3]>)> = Vec::new();
    for c in &doc.c.story_cards {
        for (i, p) in c.map_pins.iter().enumerate() {
            if p.map_id == map_id {
                pins.push((c.id, i, *p, c.display_title(), c.color));
            }
        }
    }
    if let Some(tex) = tex {
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let mut hovered_pin: Option<(Id, usize)> = None;
        let r = 7.0;
        // fit + zoom + pan
        let img = tex.size;
        let fit = (canvas.width() / img.x).min(canvas.height() / img.y);
        let scale = fit * st.zoom;
        let size = img * scale;
        let origin = canvas.center() - size / 2.0 + st.pan;
        let img_rect = Rect::from_min_size(origin, size);
        let painter = painter.with_clip_rect(canvas);
        painter.image(tex.id, img_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);

        // wheel zoom around the cursor
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                let old = st.zoom;
                st.zoom = (st.zoom * (1.0 + scroll * 0.0015)).clamp(0.2, 10.0);
                if let Some(p) = resp.hover_pos() {
                    let rel = p - canvas.center() - st.pan;
                    st.pan -= rel * (st.zoom / old - 1.0);
                }
            }
        }

        let to_screen = |x: f32, y: f32| Pos2::new(img_rect.min.x + x * img_rect.width(), img_rect.min.y + y * img_rect.height());
        let to_norm = |p: Pos2| ((p.x - img_rect.min.x) / img_rect.width(), (p.y - img_rect.min.y) / img_rect.height());

        // pins
        for (cid, i, p, _, _) in &pins {
            let pos = to_screen(p.x, p.y);
            if let Some(ptr) = pointer {
                if ptr.distance(pos) <= r + 3.0 {
                    hovered_pin = Some((*cid, *i));
                }
            }
        }
        for (cid, i, p, title, color) in &pins {
            let pos = to_screen(p.x, p.y);
            let fill = color.map(ui_util::rgb).unwrap_or(Color32::from_rgb(200, 40, 40));
            let focused = st.focused_card == Some(*cid);
            let hov = hovered_pin == Some((*cid, *i));
            painter.circle_filled(pos + Vec2::new(1.0, 1.5), r, Color32::from_black_alpha(90));
            painter.circle(pos, if hov || focused { r + 2.0 } else { r }, fill, Stroke::new(2.0_f32, Color32::WHITE));
            if hov || focused || st.zoom >= 1.5 {
                let galley = painter.layout_no_wrap(title.clone(), egui::FontId::proportional(13.0), Color32::WHITE);
                let rect = Rect::from_min_size(pos + Vec2::new(r + 4.0, -galley.size().y / 2.0), galley.size()).expand(3.0);
                painter.rect_filled(rect, 2.0, Color32::from_black_alpha(190));
                painter.galley(rect.min + Vec2::splat(3.0), galley, Color32::WHITE);
            }
        }

        // interactions
        if resp.drag_started() {
            st.dragging = hovered_pin;
        }
        if resp.dragged() {
            match st.dragging {
                Some((cid, i)) => {
                    if let Some(ptr) = pointer {
                        let (x, y) = to_norm(ptr);
                        if let Some(card) = doc.c.card_mut(cid) {
                            if let Some(p) = card.map_pins.get_mut(i) {
                                p.x = x.clamp(0.0, 1.0);
                                p.y = y.clamp(0.0, 1.0);
                            }
                        }
                    }
                }
                None => st.pan += resp.drag_delta(),
            }
        }
        if resp.drag_stopped() {
            st.dragging = None;
        }
        if resp.clicked() {
            if let Some(cid) = st.placing {
                if let Some(ptr) = pointer {
                    let (x, y) = to_norm(ptr);
                    if (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) {
                        if let Some(card) = doc.c.card_mut(cid) {
                            card.map_pins.push(MapPin { map_id, x, y });
                            st.focused_pin = Some(card.map_pins.len() - 1);
                        }
                        st.focused_card = Some(cid);
                        st.placing = None;
                    }
                }
            } else if let Some((cid, i)) = hovered_pin {
                st.focused_card = Some(cid);
                st.focused_pin = Some(i);
            } else {
                st.focused_card = None;
                st.focused_pin = None;
            }
        }
        if resp.double_clicked() {
            if let Some((cid, _)) = hovered_pin {
                sh.actions.push(Action::Open(Link::Card(cid)));
            }
        }
        if hovered_pin.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        } else if st.placing.is_some() && resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
    }

    // footer: focused card & map notes
    ui.horizontal(|ui| {
        if let Some(cid) = st.focused_card {
            if let Some(card) = doc.c.card(cid) {
                ui.label(RichText::new(card.display_title()).strong());
                if ui.button("Open card").clicked() {
                    sh.actions.push(Action::Open(Link::Card(cid)));
                }
                if ui.button("Remove pin").clicked() {
                    if let Some(card) = doc.c.card_mut(cid) {
                        // just the pin that was clicked, not every pin of this card on the map
                        match st.focused_pin.filter(|i| card.map_pins.get(*i).map(|p| p.map_id == map_id).unwrap_or(false)) {
                            Some(i) => {
                                card.map_pins.remove(i);
                            }
                            None => {
                                if let Some(i) = card.map_pins.iter().position(|p| p.map_id == map_id) {
                                    card.map_pins.remove(i);
                                }
                            }
                        }
                    }
                    st.focused_card = None;
                    st.focused_pin = None;
                }
            }
        } else {
            ui.weak(format!(
                "{} pins · scroll to zoom · drag to pan · drag pins to move · double-click a pin to open its card",
                pins.len()
            ));
        }
    });
    if let Some(m) = doc.c.maps.iter_mut().find(|m| m.id == map_id) {
        ui.add(
            egui::TextEdit::multiline(&mut m.notes)
                .id(egui::Id::new(("map_notes", map_id)))
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .hint_text("Map notes…"),
        );
    }
}

/// A map's name from its image file: "assets/thornwood-map-3f2a.png" → "thornwood map".
/// (Imported files get a "-xxxx" suffix to keep them unique; only that is dropped.)
fn map_name(rel: &str) -> String {
    let file = rel.rsplit('/').next().unwrap_or("Map");
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
    let stem = match stem.rsplit_once('-') {
        Some((head, tail)) if !head.is_empty() && tail.len() <= 8 && tail.chars().all(|c| c.is_ascii_hexdigit()) => head,
        _ => stem,
    };
    let name = stem.replace(['-', '_'], " ").trim().to_string();
    if name.is_empty() { "Map".into() } else { name }
}
