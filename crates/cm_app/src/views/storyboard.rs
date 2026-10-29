//! Story board: a table top for story cards. Drag cards around, drag from a card's ● handle
//! onto another card to link them, right-click a line to remove the link. Kept deliberately
//! plain — boxes and arrows.

use std::collections::HashMap;

use cm_core::{CardStatus, Id, Link, QuestStatus, StoryCard};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2, pos2, vec2};

use crate::app::{Action, Doc, Shared};
use crate::ui_util;

const NODE: Vec2 = vec2(190.0, 68.0);

pub struct State {
    pan: Vec2,
    zoom: f32,
    filter: String,
    selected: Option<Id>,
    /// Dragging a new link from this card.
    linking: Option<Id>,
    /// Link under the pointer when the context menu opened.
    edge_menu: Option<(Id, Id)>,
    /// Board position of the last right-click (for "new card here").
    menu_pos: Option<Pos2>,
    hide_done: bool,
    /// Size of the board area last frame (to centre on a card).
    view_size: Vec2,
}

impl Default for State {
    fn default() -> Self {
        State {
            pan: vec2(40.0, 40.0),
            zoom: 1.0,
            filter: String::new(),
            selected: None,
            linking: None,
            edge_menu: None,
            menu_pos: None,
            hide_done: false,
            view_size: vec2(900.0, 600.0),
        }
    }
}

impl State {
    /// Select a card and centre the board on it, putting it on the board first if needed.
    pub fn focus(&mut self, doc: &mut Doc, id: Id) {
        let mid = vec2(self.view_size.x * 0.5, self.view_size.y * 0.5);
        if let Some(card) = doc.c.card_mut(id) {
            let [x, y] = *card.board.get_or_insert_with(|| {
                let c = (mid - self.pan) / self.zoom - NODE * 0.5;
                [c.x, c.y]
            });
            self.pan = mid - (vec2(x, y) + NODE * 0.5) * self.zoom;
            self.selected = Some(id);
        }
    }
}

fn node_color(c: &cm_core::Campaign, card: &StoryCard) -> Option<Color32> {
    card.color
        .or_else(|| card.threads.first().and_then(|t| c.thread(*t)).and_then(|t| t.color))
        .map(ui_util::rgb)
}

/// Point where the segment from `a` (inside `r`) towards `b` leaves `r`.
fn edge_exit(r: Rect, b: Pos2) -> Pos2 {
    let c = r.center();
    let d = b - c;
    if d.length() < 1e-3 {
        return c;
    }
    let tx = if d.x.abs() > 1e-3 { (r.width() / 2.0) / d.x.abs() } else { f32::INFINITY };
    let ty = if d.y.abs() > 1e-3 { (r.height() / 2.0) / d.y.abs() } else { f32::INFINITY };
    c + d * tx.min(ty).min(1.0)
}

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t - p).length()
}

fn arrow(painter: &egui::Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    painter.line_segment([from, to], stroke);
    let dir = (to - from).normalized();
    let side = vec2(-dir.y, dir.x);
    let size = 9.0;
    let tip = to;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, tip - dir * size + side * size * 0.5, tip - dir * size - side * size * 0.5],
        stroke.color,
        Stroke::NONE,
    ));
}

pub fn show(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    side_list(ui, doc, sh, st);
    egui::CentralPanel::default().frame(egui::Frame::new()).show_inside(ui, |ui| {
        toolbar(ui, doc, st);
        canvas(ui, doc, sh, st);
    });
}

fn side_list(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    egui::SidePanel::left("board_list").resizable(true).default_width(230.0).show_inside(ui, |ui| {
        ui.heading("Story board");
        ui.weak("Lay cards out on a table and draw links between them.");
        ui.add_space(4.0);
        ui_util::filter_box(ui, &mut st.filter, "Find a card…");
        let f = st.filter.to_lowercase();
        let off: Vec<(Id, String)> = doc
            .c
            .story_cards
            .iter()
            .filter(|c| c.board.is_none())
            .filter(|c| f.is_empty() || c.display_title().to_lowercase().contains(&f) || c.body.to_lowercase().contains(&f))
            .map(|c| (c.id, ui_util::card_title(c)))
            .collect();
        let on: Vec<(Id, String)> = doc
            .c
            .story_cards
            .iter()
            .filter(|c| c.board.is_some())
            .filter(|c| f.is_empty() || c.display_title().to_lowercase().contains(&f))
            .map(|c| (c.id, ui_util::card_title(c)))
            .collect();
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("➕ New card")
                .on_hover_text("A new card in the middle of the board (or double-click the board)")
                .clicked()
            {
                let center = board_center(ui, st);
                let card = StoryCard {
                    title: "New card".into(),
                    board: Some([center.x, center.y]),
                    ..Default::default()
                };
                st.selected = Some(card.id);
                doc.c.story_cards.push(card);
            }
            if !off.is_empty() && ui.button("Add all").on_hover_text("Put every card on the board, in a grid").clicked() {
                let cols = ((st.view_size.x - 40.0) / ((NODE.x + 40.0) * st.zoom)).floor().max(1.0) as usize;
                place_grid(doc, off.iter().map(|x| x.0).collect(), cols);
                fit(doc, st);
            }
        });
        ui.separator();
        if let Some(sel) = st.selected {
            selected_card(ui, doc, sh, st, sel);
        }
        egui::ScrollArea::vertical().id_salt("board_list_scroll").show(ui, |ui| {
            ui.label(RichText::new(format!("Not on the board ({})", off.len())).small().strong());
            if off.is_empty() {
                ui.weak("Every card is on the board.");
            }
            for (id, title) in &off {
                ui.horizontal(|ui| {
                    if ui.small_button("➕").on_hover_text("Put on the board").clicked() {
                        let c = board_center(ui, st);
                        let n = doc.c.story_cards.iter().filter(|c| c.board.is_some()).count() as f32;
                        if let Some(card) = doc.c.card_mut(*id) {
                            card.board = Some([c.x + (n % 5.0) * 14.0, c.y + (n % 5.0) * 14.0]);
                        }
                        st.selected = Some(*id);
                    }
                    ui.add(egui::Label::new(title).truncate());
                });
            }
            ui.add_space(8.0);
            ui.label(RichText::new(format!("On the board ({})", on.len())).small().strong());
            for (id, title) in &on {
                let r = ui_util::list_row(ui, st.selected == Some(*id), title.as_str());
                if r.clicked() {
                    st.focus(doc, *id);
                }
                if r.double_clicked() {
                    sh.actions.push(Action::Open(Link::Card(*id)));
                }
            }
        });
    });
}

/// The selected card's links, both ways, so they can be read and followed from the board.
fn selected_card(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State, id: Id) {
    let Some(card) = doc.c.card(id) else {
        st.selected = None;
        return;
    };
    let title = ui_util::card_title(card);
    let outgoing: Vec<Link> = card.links.iter().filter(|l| !matches!(l, Link::Url { .. } | Link::Image(_))).cloned().collect();
    let incoming: Vec<Id> = doc
        .c
        .story_cards
        .iter()
        .filter(|c| c.id != id && c.links.contains(&Link::Card(id)) && !outgoing.contains(&Link::Card(c.id)))
        .map(|c| c.id)
        .collect();
    let others: Vec<Link> = doc
        .c
        .backlinks(&Link::Card(id))
        .into_iter()
        .map(|e| e.link)
        .filter(|l| !matches!(l, Link::Card(_)))
        .collect();
    let mut rows: Vec<(&str, Link)> = Vec::new();
    for l in &outgoing {
        let two_way = matches!(l, Link::Card(c) if doc.c.card(*c).map(|o| o.links.contains(&Link::Card(id))).unwrap_or(false));
        rows.push((if two_way { "↔" } else { "→" }, l.clone()));
    }
    rows.extend(incoming.iter().map(|c| ("←", Link::Card(*c))));
    rows.extend(others.into_iter().map(|l| ("←", l)));
    let mut focus = None;
    let mut take_off = false;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(RichText::new(&title).strong()).truncate());
        ui.horizontal(|ui| {
            if ui.small_button("Open card").clicked() {
                sh.actions.push(Action::Open(Link::Card(id)));
            }
            if ui.small_button("Take off board").clicked() {
                take_off = true;
            }
        });
        if rows.is_empty() {
            ui.weak("No links yet — drag its ● onto another card.");
        }
        egui::ScrollArea::vertical().id_salt("board_sel_links").max_height(160.0).show(ui, |ui| {
            for (arrow, l) in &rows {
                let name = doc.c.link_name(l).unwrap_or("(deleted)".into());
                let text = if matches!(l, Link::Card(_)) {
                    name
                } else {
                    format!("{name} ({})", l.kind_label())
                };
                ui.horizontal(|ui| {
                    ui.weak(*arrow);
                    let link_col = ui.visuals().hyperlink_color;
                    if ui
                        .add(egui::Label::new(RichText::new(text).color(link_col)).truncate().sense(Sense::click()))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        match l {
                            Link::Card(c) if doc.c.card(*c).and_then(|x| x.board).is_some() => focus = Some(*c),
                            other => sh.actions.push(Action::Open(other.clone())),
                        }
                    }
                });
            }
        });
    });
    if take_off {
        if let Some(c) = doc.c.card_mut(id) {
            c.board = None;
        }
    }
    if let Some(c) = focus {
        st.focus(doc, c);
    }
}

/// Where the middle of the visible board is, in board coordinates (approximate).
fn board_center(_ui: &egui::Ui, st: &State) -> Pos2 {
    let c = (st.view_size * 0.5 - st.pan) / st.zoom - NODE * 0.5;
    pos2(c.x, c.y)
}

fn place_grid(doc: &mut Doc, ids: Vec<Id>, cols: usize) {
    let max_y = doc.c.story_cards.iter().filter_map(|c| c.board).map(|p| p[1]).fold(-NODE.y, f32::max);
    let start_y = max_y + NODE.y + 40.0;
    for (i, id) in ids.iter().enumerate() {
        let (col, row) = ((i % cols) as f32, (i / cols) as f32);
        if let Some(card) = doc.c.card_mut(*id) {
            card.board = Some([col * (NODE.x + 40.0), start_y + row * (NODE.y + 50.0)]);
        }
    }
}

fn toolbar(ui: &mut egui::Ui, doc: &mut Doc, st: &mut State) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(6.0);
        if ui.button("−").on_hover_text("Zoom out (or scroll)").clicked() {
            st.zoom = (st.zoom / 1.2).max(0.3);
        }
        if ui.button(format!("{:.0}%", st.zoom * 100.0)).on_hover_text("Reset zoom").clicked() {
            st.zoom = 1.0;
        }
        if ui.button("+").on_hover_text("Zoom in").clicked() {
            st.zoom = (st.zoom * 1.2).min(2.5);
        }
        if ui.button("Fit").on_hover_text("Show every card").clicked() {
            fit(doc, st);
        }
        ui.checkbox(&mut st.hide_done, "Dim done cards");
        if ui.button("Tidy").on_hover_text("Arrange the board's cards in rows by plot thread").clicked() {
            tidy(doc);
            fit(doc, st);
        }
        ui.separator();
        ui.weak("Drag cards to move · drag a card's ● onto another card to link them · right-click a line to remove it · double-click to open");
    });
}

fn fit(doc: &Doc, st: &mut State) {
    let pts: Vec<[f32; 2]> = doc.c.story_cards.iter().filter_map(|c| c.board).collect();
    if pts.is_empty() {
        st.pan = vec2(40.0, 40.0);
        st.zoom = 1.0;
        return;
    }
    let (minx, miny) = pts.iter().fold((f32::MAX, f32::MAX), |a, p| (a.0.min(p[0]), a.1.min(p[1])));
    let (maxx, maxy) = pts.iter().fold((f32::MIN, f32::MIN), |a, p| (a.0.max(p[0] + NODE.x), a.1.max(p[1] + NODE.y)));
    let avail = st.view_size - vec2(40.0, 40.0);
    let z = (avail.x / (maxx - minx).max(1.0)).min(avail.y / (maxy - miny).max(1.0)).clamp(0.3, 1.5);
    st.zoom = z;
    st.pan = vec2(20.0 - minx * z, 20.0 - miny * z);
}

/// Rows by first plot thread, cards in each row by session / status.
fn tidy(doc: &mut Doc) {
    let mut rows: Vec<(Option<Id>, Vec<Id>)> = Vec::new();
    let mut cards: Vec<&StoryCard> = doc.c.story_cards.iter().filter(|c| c.board.is_some()).collect();
    cards.sort_by_key(|c| {
        let session = match c.timing {
            cm_core::CardTiming::Session(n) => n,
            _ => u32::MAX,
        };
        (session, c.status as u8)
    });
    for c in cards {
        let t = c.threads.first().copied();
        match rows.iter_mut().find(|(k, _)| *k == t) {
            Some((_, v)) => v.push(c.id),
            None => rows.push((t, vec![c.id])),
        }
    }
    for (ri, (_, ids)) in rows.iter().enumerate() {
        for (ci, id) in ids.iter().enumerate() {
            if let Some(card) = doc.c.card_mut(*id) {
                card.board = Some([ci as f32 * (NODE.x + 50.0), ri as f32 * (NODE.y + 60.0)]);
            }
        }
    }
}

fn canvas(ui: &mut egui::Ui, doc: &mut Doc, sh: &mut Shared, st: &mut State) {
    let (rect, bg) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    st.view_size = rect.size();
    let painter = ui.painter_at(rect);
    let vis = ui.visuals().clone();
    painter.rect_filled(rect, 0.0, vis.extreme_bg_color);
    let origin = rect.min.to_vec2();
    let to_screen = |p: [f32; 2], st: &State| pos2(p[0] * st.zoom, p[1] * st.zoom) + st.pan + origin;

    // dotted grid
    let step = 40.0 * st.zoom;
    if step > 12.0 {
        let dot = vis.weak_text_color().gamma_multiply(0.35);
        let mut x = rect.left() + (st.pan.x % step + step) % step;
        while x < rect.right() {
            let mut y = rect.top() + (st.pan.y % step + step) % step;
            while y < rect.bottom() {
                painter.circle_filled(pos2(x, y), 1.0, dot);
                y += step;
            }
            x += step;
        }
    }

    // zoom with the scroll wheel around the pointer
    if bg.hovered() || ui.rect_contains_pointer(rect) {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y + i.zoom_delta().ln() * 200.0);
        if scroll.abs() > 0.1 {
            if let Some(p) = ui.ctx().pointer_hover_pos() {
                let old = st.zoom;
                st.zoom = (st.zoom * (1.0 + scroll * 0.0015)).clamp(0.3, 2.5);
                let k = st.zoom / old;
                let rel = p - origin - st.pan;
                st.pan += rel - rel * k;
            }
        }
    }
    if bg.dragged() && st.linking.is_none() {
        st.pan += bg.drag_delta();
    }

    let size = NODE * st.zoom;
    let rects: HashMap<Id, Rect> = doc
        .c
        .story_cards
        .iter()
        .filter_map(|c| c.board.map(|p| (c.id, Rect::from_min_size(to_screen(p, st), size))))
        .collect();

    // edges
    let mut edges: Vec<(Id, Id, bool)> = Vec::new();
    for c in doc.c.story_cards.iter().filter(|c| c.board.is_some()) {
        for l in &c.links {
            if let Link::Card(t) = l {
                if *t != c.id && rects.contains_key(t) {
                    let both = doc.c.card(*t).map(|o| o.links.contains(&Link::Card(c.id))).unwrap_or(false);
                    if both && c.id > *t {
                        continue; // draw a two-way link once
                    }
                    edges.push((c.id, *t, both));
                }
            }
        }
    }
    let pointer = ui.ctx().pointer_hover_pos();
    let mut hovered_edge: Option<(Id, Id)> = None;
    for (a, b, both) in &edges {
        let (ra, rb) = (rects[a], rects[b]);
        let pa = edge_exit(ra, rb.center());
        let pb = edge_exit(rb, ra.center());
        let near = pointer
            .map(|p| dist_to_segment(p, pa, pb) < 6.0 && !rects.values().any(|r| r.contains(p)))
            .unwrap_or(false);
        if near {
            hovered_edge = Some((*a, *b));
        }
        let sel = st.selected == Some(*a) || st.selected == Some(*b);
        let color = if near {
            vis.selection.stroke.color
        } else if sel {
            vis.hyperlink_color
        } else {
            vis.weak_text_color()
        };
        let stroke = Stroke::new(if near || sel { 2.5_f32 } else { 1.5_f32 }, color);
        arrow(&painter, pa, pb, stroke);
        if *both {
            arrow(&painter, pb, pa, stroke);
        }
    }

    // nodes
    let mut move_by: Option<(Id, Vec2)> = None;
    let mut open: Option<Id> = None;
    let mut remove: Option<Id> = None;
    let mut add_linked: Option<Id> = None;
    let mut link_made: Option<(Id, Id)> = None;
    let cards: Vec<StoryCard> = doc.c.story_cards.iter().filter(|c| c.board.is_some()).cloned().collect();
    for card in &cards {
        let r = rects[&card.id];
        if !r.intersects(rect) {
            continue;
        }
        let resp = ui.interact(r, egui::Id::new(("board_node", card.id)), Sense::click_and_drag());
        if resp.dragged() {
            move_by = Some((card.id, resp.drag_delta() / st.zoom));
        }
        if resp.clicked() || resp.drag_started() {
            st.selected = Some(card.id);
        }
        if resp.double_clicked() {
            open = Some(card.id);
        }
        resp.context_menu(|ui| {
            if ui.button("Open card").clicked() {
                open = Some(card.id);
                ui.close();
            }
            if ui
                .button("Bring linked cards")
                .on_hover_text("Put every card this one links to (or is linked from) on the board around it")
                .clicked()
            {
                add_linked = Some(card.id);
                ui.close();
            }
            if ui.button("Take off the board").clicked() {
                remove = Some(card.id);
                ui.close();
            }
        });
        let selected = st.selected == Some(card.id);
        let done = matches!(card.status, CardStatus::Done | CardStatus::Discarded);
        let dim = st.hide_done && done;
        let fill = if dim { vis.faint_bg_color } else { vis.window_fill };
        let stroke = if selected {
            Stroke::new(2.5_f32, vis.selection.stroke.color)
        } else if resp.hovered() {
            Stroke::new(1.5_f32, vis.hyperlink_color)
        } else {
            vis.widgets.noninteractive.bg_stroke
        };
        painter.rect(r, 6.0 * st.zoom, fill, stroke, egui::StrokeKind::Inside);
        if let Some(col) = node_color(&doc.c, card) {
            let bar = Rect::from_min_size(r.min, vec2(5.0 * st.zoom, r.height()));
            painter.rect_filled(
                bar,
                egui::CornerRadius {
                    nw: (6.0 * st.zoom) as u8,
                    sw: (6.0 * st.zoom) as u8,
                    ne: 0,
                    se: 0,
                },
                col,
            );
        }
        if st.zoom >= 0.45 {
            let pad = 9.0 * st.zoom;
            let text_col = if dim { vis.weak_text_color() } else { vis.strong_text_color() };
            let title_font = egui::FontId::proportional(14.0 * st.zoom);
            let mut job = egui::text::LayoutJob::simple(ui_util::card_title(card), title_font, text_col, r.width() - 2.0 * pad - 8.0 * st.zoom);
            job.wrap.max_rows = 2;
            job.wrap.break_anywhere = false;
            job.wrap.overflow_character = Some('…');
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            let clip = r.shrink(2.0);
            painter.with_clip_rect(clip).galley(r.min + vec2(pad, pad * 0.7), galley, text_col);
            // a second line: quest / status / session
            let mut bits: Vec<String> = Vec::new();
            if let Some(q) = &card.quest {
                bits.push(match q.status {
                    QuestStatus::Completed => "✔ quest done".to_string(),
                    QuestStatus::Failed | QuestStatus::Abandoned => format!("quest {}", q.status.label().to_lowercase()),
                    _ => "❗ quest".to_string(),
                });
            }
            bits.push(card.status.label().to_string());
            if let cm_core::CardTiming::Session(n) = card.timing {
                bits.push(format!("session {n}"));
            }
            painter.with_clip_rect(clip).text(
                r.left_bottom() + vec2(pad, -pad * 0.6),
                egui::Align2::LEFT_BOTTOM,
                bits.join(" · "),
                egui::FontId::proportional(11.0 * st.zoom),
                vis.weak_text_color(),
            );
        }
        // link handle
        let h = pos2(r.right(), r.center().y);
        let hr = Rect::from_center_size(h, vec2(16.0, 16.0));
        let hresp = ui.interact(hr, egui::Id::new(("board_handle", card.id)), Sense::drag());
        let hot = hresp.hovered() || st.linking == Some(card.id);
        painter.circle(
            h,
            if hot { 6.0 } else { 4.5 },
            if hot { vis.selection.stroke.color } else { vis.window_fill },
            Stroke::new(1.5_f32, vis.hyperlink_color),
        );
        if hresp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        hresp.clone().on_hover_text("Drag onto another card to link them");
        if hresp.drag_started() {
            st.linking = Some(card.id);
        }
        if hresp.drag_stopped() {
            if let (Some(from), Some(p)) = (st.linking, ui.ctx().pointer_interact_pos()) {
                if let Some((to, _)) = rects.iter().find(|(id, r)| **id != from && r.contains(p)) {
                    link_made = Some((from, *to));
                }
            }
            st.linking = None;
        }
    }
    // the link being drawn
    if let (Some(from), Some(p)) = (st.linking, pointer) {
        if let Some(r) = rects.get(&from) {
            arrow(&painter, pos2(r.right(), r.center().y), p, Stroke::new(2.0_f32, vis.selection.stroke.color));
        }
    }

    // background: double-click = new card, right-click = edge / board menu
    if bg.double_clicked() {
        if let Some(p) = bg.interact_pointer_pos() {
            let bp = (p - origin - st.pan) / st.zoom;
            let card = StoryCard {
                title: "New card".into(),
                board: Some([bp.x - NODE.x / 2.0, bp.y - NODE.y / 2.0]),
                ..Default::default()
            };
            st.selected = Some(card.id);
            doc.c.story_cards.push(card);
        }
    } else if bg.clicked() && hovered_edge.is_none() {
        st.selected = None;
    }
    if bg.secondary_clicked() {
        st.edge_menu = hovered_edge;
        st.menu_pos = bg.interact_pointer_pos().map(|p| ((p - origin - st.pan).to_vec2() / st.zoom).to_pos2());
    }
    let mut remove_edge: Option<(Id, Id)> = None;
    let mut new_at: Option<Pos2> = None;
    bg.context_menu(|ui| {
        if let Some((a, b)) = st.edge_menu {
            let na = doc.c.card(a).map(|c| c.display_title()).unwrap_or_default();
            let nb = doc.c.card(b).map(|c| c.display_title()).unwrap_or_default();
            if ui.button(format!("Remove link: {na} → {nb}")).clicked() {
                remove_edge = Some((a, b));
                ui.close();
            }
            ui.separator();
        }
        if ui.button("New card here").clicked() {
            new_at = st.menu_pos;
            ui.close();
        }
    });

    // apply changes
    if let Some((id, d)) = move_by {
        if let Some(c) = doc.c.card_mut(id) {
            if let Some(p) = &mut c.board {
                p[0] += d.x;
                p[1] += d.y;
            }
        }
    }
    if let Some((a, b)) = link_made {
        if let Some(c) = doc.c.card_mut(a) {
            if !c.links.contains(&Link::Card(b)) {
                c.links.push(Link::Card(b));
                c.modified = chrono::Utc::now();
                sh.toast("Linked");
            }
        }
    }
    if let Some((a, b)) = remove_edge {
        for (x, y) in [(a, b), (b, a)] {
            if let Some(c) = doc.c.card_mut(x) {
                c.links.retain(|l| *l != Link::Card(y));
            }
        }
        sh.toast("Link removed");
    }
    if let Some(p) = new_at {
        let card = StoryCard {
            title: "New card".into(),
            board: Some([p.x, p.y]),
            ..Default::default()
        };
        st.selected = Some(card.id);
        doc.c.story_cards.push(card);
    }
    if let Some(id) = remove {
        if let Some(c) = doc.c.card_mut(id) {
            c.board = None;
        }
    }
    if let Some(id) = add_linked {
        let Some(base) = doc.c.card(id).and_then(|c| c.board) else { return };
        let mut ids: Vec<Id> = doc
            .c
            .card(id)
            .map(|c| c.links.iter().filter_map(|l| if let Link::Card(t) = l { Some(*t) } else { None }).collect())
            .unwrap_or_default();
        for c in &doc.c.story_cards {
            if c.links.contains(&Link::Card(id)) && !ids.contains(&c.id) {
                ids.push(c.id);
            }
        }
        let mut placed = 0;
        let total = ids.len().max(1) as f32;
        for (i, t) in ids.iter().enumerate() {
            if let Some(c) = doc.c.card_mut(*t) {
                if c.board.is_none() {
                    let ang = i as f32 / total * std::f32::consts::TAU;
                    c.board = Some([base[0] + ang.cos() * 280.0, base[1] + ang.sin() * 160.0]);
                    placed += 1;
                }
            }
        }
        sh.toast(format!("Placed {}", ui_util::plural(placed, "linked card")));
    }
    if let Some(id) = open {
        sh.actions.push(Action::Open(Link::Card(id)));
    }
    if rects.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Put cards on the board from the list on the left, or double-click here to make one.",
            egui::FontId::proportional(14.0),
            vis.weak_text_color(),
        );
    }
}
