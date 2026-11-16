//! What is on screen: the board in 3D, and beside it (when there is room)
//! whose turn it is, the moves, what has been taken and what is en prise.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::app::{App, MENU, Mode};
use crate::game::{self, Status, glyph};
use crate::raster::Canvas;
use crate::scene::Scene;

const AMBER: Color = Color::Rgb(255, 176, 0);
const GREEN: Color = Color::Rgb(80, 250, 123);
const RED: Color = Color::Rgb(255, 90, 100);
const DIM: Color = Color::Rgb(120, 130, 140);
const TEXT: Color = Color::Rgb(210, 214, 220);
const PANEL_BG: Color = Color::Rgb(12, 11, 16);
pub const PANEL_W: u16 = 28;

fn put(buf: &mut Buffer, x: u16, y: u16, s: &str, style: Style, max: u16) {
    let area = buf.area;
    if y >= area.y + area.height || x >= area.x + area.width {
        return;
    }
    let width = max.min(area.x + area.width - x) as usize;
    buf.set_stringn(x, y, s, width, style);
}

pub fn draw(app: &mut App, area: Rect, buf: &mut Buffer) {
    let with_panel = area.width >= 76;
    let view = if with_panel {
        Rect { width: area.width - PANEL_W, ..area }
    } else {
        Rect { height: area.height.saturating_sub(2), ..area }
    };
    app.view_area = view;
    let (w, h) = (view.width.max(1) as usize, view.height.max(1) as usize * 2);
    if app.canvas.as_ref().is_none_or(|c| c.w != w || c.h != h) {
        app.canvas = Some(Canvas::new(w, h));
    }
    let v = app.view();
    let mut scene = Scene::new(&v.cam, w, h, app.y_scale);
    scene.pulse = 0.5 + 0.5 * (app.time * 4.0).sin();
    let canvas = app.canvas.as_mut().expect("a canvas");
    scene.draw(canvas, &v.figures, &v.marks, &v.fx);
    canvas.blit(buf, view);
    put(buf, view.x + 2, view.y + 1, "EN PRISE", Style::default().fg(AMBER).add_modifier(Modifier::BOLD), view.width);
    if let Some(t) = app.fight_title() {
        let x = view.x + view.width.saturating_sub(t.chars().count() as u16) / 2;
        put(buf, x, view.y + view.height.saturating_sub(2), &t, Style::default().fg(AMBER).add_modifier(Modifier::BOLD), view.width);
    }
    if with_panel {
        side(app, Rect { x: area.x + area.width - PANEL_W, width: PANEL_W, ..area }, buf);
    } else {
        let line = status_line(app).0;
        put(buf, area.x + 1, area.y + area.height.saturating_sub(2), &line, Style::default().fg(TEXT), area.width.saturating_sub(2));
        put(buf, area.x + 1, area.y + area.height.saturating_sub(1), "Esc menu  ? help  u take back", Style::default().fg(DIM), area.width.saturating_sub(2));
    }
    if app.promoting.is_some() {
        let r = boxed(area, 36, 5, buf);
        put(buf, r.x + 2, r.y + 1, "Crown your pawn:", Style::default().fg(AMBER).add_modifier(Modifier::BOLD).bg(PANEL_BG), r.width - 3);
        put(buf, r.x + 2, r.y + 3, "Q queen R rook B bishop N knight", Style::default().fg(TEXT).bg(PANEL_BG), r.width - 3);
    }
    if let Some(sel) = app.menu {
        menu(app, sel, area, buf);
    }
    if app.help {
        help(area, buf);
    }
}

fn status_line(app: &App) -> (String, Color) {
    let b = &app.game.board;
    match app.status() {
        _ if app.anim.is_some() => {
            let a = app.anim.as_ref().unwrap();
            (format!("{} plays {}", game::side(a.p.colour), a.p.san), TEXT)
        }
        Status::Playing => {
            let side = game::side(b.side_to_move());
            let check = if app.game.in_check() { ", in check!" } else { "" };
            if app.human_turn() {
                (format!("{side} to move{check}"), if check.is_empty() { GREEN } else { AMBER })
            } else {
                (format!("{side} is thinking{check}"), DIM)
            }
        }
        s => (s.describe(), AMBER),
    }
}

fn side(app: &App, area: Rect, buf: &mut Buffer) {
    buf.set_style(area, Style::default().bg(PANEL_BG));
    let w = area.width - 3;
    let x0 = area.x + 2;
    let mut y = area.y + 1;
    let who = match app.mode {
        Mode::White => format!("You play White ({})", app.strength.name()),
        Mode::Black => format!("You play Black ({})", app.strength.name()),
        Mode::Two => "Two players".into(),
        Mode::Watch => format!("Watching, {}", app.strength.name()),
    };
    put(buf, x0, y, &who, Style::default().fg(DIM), w);
    y += 2;
    let (line, colour) = status_line(app);
    // the end of a game can be long: two lines
    let mut words = String::new();
    let mut lines = Vec::new();
    for word in line.split(' ') {
        if words.len() + word.len() + 1 > w as usize {
            lines.push(std::mem::take(&mut words));
        }
        if !words.is_empty() {
            words.push(' ');
        }
        words.push_str(word);
    }
    lines.push(words);
    for l in lines {
        put(buf, x0, y, &l, Style::default().fg(colour).add_modifier(Modifier::BOLD), w);
        y += 1;
    }
    y += 1;

    // taken pieces
    let (by_white, by_black) = app.game.taken();
    let row = |v: &[cozy_chess::Piece], c: cozy_chess::Color| v.iter().map(|&k| glyph(k, c)).collect::<String>();
    put(buf, x0, y, &format!("White took {}", row(&by_white, cozy_chess::Color::Black)), Style::default().fg(TEXT), w);
    put(buf, x0, y + 1, &format!("Black took {}", row(&by_black, cozy_chess::Color::White)), Style::default().fg(TEXT), w);
    y += 3;

    // en prise
    if let Some(c) = app.watched()
        && app.show_en_prise
        && app.anim.is_none()
        && !app.status().over()
    {
        let hanging: Vec<String> = game::en_prise(&app.game.board, c)
            .into_iter()
            .filter_map(|s| app.game.board.piece_on(s).map(|k| format!("{}{}", glyph(k, c), s)))
            .collect();
        if !hanging.is_empty() {
            put(buf, x0, y, "En prise:", Style::default().fg(RED).add_modifier(Modifier::BOLD), w);
            put(buf, x0 + 10, y, &hanging.join(" "), Style::default().fg(RED), w.saturating_sub(10));
            y += 2;
        }
    }

    // the moves, newest at the bottom
    let bottom = area.y + area.height - 4;
    let lines = app.game.lines();
    let room = bottom.saturating_sub(y) as usize;
    for l in lines.iter().skip(lines.len().saturating_sub(room)) {
        put(buf, x0, y, l, Style::default().fg(TEXT), w);
        y += 1;
    }
    let keys = if app.anim.is_some() { "Space: skip the fight" } else { "click, or arrows+Enter" };
    put(buf, x0, area.y + area.height - 3, keys, Style::default().fg(DIM), w);
    put(buf, x0, area.y + area.height - 2, "u back ,. turn d fights", Style::default().fg(DIM), w);
    put(buf, x0, area.y + area.height - 1, "Esc menu ? help q quit", Style::default().fg(DIM), w);
}

fn boxed(area: Rect, w: u16, h: u16, buf: &mut Buffer) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let r = Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height - h) / 2, width: w, height: h };
    for y in r.y..r.y + r.height {
        for x in r.x..r.x + r.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.reset();
                cell.set_bg(PANEL_BG);
            }
        }
    }
    r
}

fn menu(app: &App, sel: usize, area: Rect, buf: &mut Buffer) {
    let r = boxed(area, 32, MENU.len() as u16 + 4, buf);
    put(buf, r.x + 2, r.y + 1, "EN PRISE", Style::default().fg(AMBER).add_modifier(Modifier::BOLD).bg(PANEL_BG), r.width - 3);
    for (i, item) in MENU.iter().enumerate() {
        let text = match *item {
            "Strength" => format!("Strength: {}", app.strength.name()),
            "Fights" => format!("Fights: {}", app.style.name()),
            _ => item.to_string(),
        };
        let style = if i == sel { Style::default().fg(Color::Black).bg(AMBER) } else { Style::default().fg(TEXT).bg(PANEL_BG) };
        put(buf, r.x + 2, r.y + 3 + i as u16, &format!(" {text:<26}"), style, r.width - 3);
    }
}

fn help(area: Rect, buf: &mut Buffer) {
    let lines = [
        "EN PRISE",
        "",
        "Chess, where a capture is a fight. The",
        "pawn's spear, the knight's charge, the",
        "bishop's light, the rook's stones, the",
        "queen's ring and the king's sword.",
        "",
        "Click a piece, then where it goes, or",
        "move the cursor with the arrows (up is",
        "away from you) and press Enter.",
        "",
        "A piece of yours glowing red is en prise:",
        "it can be taken, and it would pay to.",
        "",
        "Space  skip a fight     d  fights full,",
        "u      take back           quick or off",
        ", .    turn the board   p  en prise marks",
        "+ -    zoom             0  look home",
        "m      sound on/off     Esc  menu",
        "drag to turn and tilt   q  quit",
    ];
    let r = boxed(area, 46, lines.len() as u16 + 2, buf);
    for (i, l) in lines.iter().enumerate() {
        let style = if i == 0 { Style::default().fg(AMBER).add_modifier(Modifier::BOLD) } else { Style::default().fg(TEXT) };
        put(buf, r.x + 2, r.y + 1 + i as u16, l, style.bg(PANEL_BG), r.width - 3);
    }
}
