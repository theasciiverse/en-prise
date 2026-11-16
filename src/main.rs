mod ai;
mod anim;
mod app;
mod audio;
mod game;
mod model;
mod raster;
mod rng;
mod scene;
mod ui;

use ai::Strength;
use anim::Style;
use app::{App, MENU, Mode};
use cozy_chess::Piece;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, MouseButton, MouseEvent,
    MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{supports_keyboard_enhancement, window_size};
use std::io::{self, Write};
use std::time::{Duration, Instant, SystemTime};

const SNAPSHOT_EVERY: Duration = Duration::from_millis(500);

const HELP: &str = "en-prise [--black | --two | --watch] [--easy | --hard] [--quick | --no-fights]

En Prise: chess where the pieces fight, on an isometric board in the
terminal. Every capture is played out: the pawn's spear, the knight's
charge, the bishop's light, the rook's stones, the queen's ring, the
king's sword. Needs a terminal with 24-bit colour.

Click a piece and then where it goes, or move the cursor with the arrows
and press Enter. Space skips a fight, u takes back a move, , and . turn the
board, d sets fights to full, quick or off, p turns the en prise marks
off or on, Esc opens the menu, ? shows help, m turns the sound off or on,
q quits. Drag the board to turn and tilt it.

  --black        play Black (default: White, against the computer)
  --two          two players at one keyboard
  --watch        the computer against itself
  --easy, --hard the computer's strength (default: normal)
  --quick        fights at double speed
  --no-fights    captures without fights
  --seed N       fixed random seed
  --mute         start with the sound off
  --fps N        frames a second while things move (default 30)

Logging: RUST_LOG=en_prise=debug en-prise 2>en-prise.log
";

fn main() -> io::Result<()> {
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let after = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned();
    if has("--help") || has("-h") {
        print!("{HELP}");
        return Ok(());
    }
    if has("--version") || has("-V") {
        println!("en-prise {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let seed = after("--seed")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64));
    let mode = if has("--black") {
        Mode::Black
    } else if has("--two") {
        Mode::Two
    } else if has("--watch") {
        Mode::Watch
    } else {
        Mode::White
    };
    let strength = if has("--easy") {
        Strength::Easy
    } else if has("--hard") {
        Strength::Hard
    } else {
        Strength::Normal
    };
    let style = if has("--no-fights") {
        Style::Off
    } else if has("--quick") {
        Style::Quick
    } else {
        Style::Full
    };
    if let Some(path) = after("--shot") {
        return shot(&path, after("--fen"), after("--move"), after("--at").and_then(|v| v.parse().ok()).unwrap_or(1.0), after("--size"), style);
    }
    // open audio before the alternate screen, so any ALSA chatter lands above it
    let audio = audio::Audio::new(has("--mute"));
    let mut app = App::new(mode, strength, style, seed, audio);
    let fps: f32 = after("--fps").and_then(|v| v.parse().ok()).unwrap_or(30.0f32).clamp(5.0, 60.0);
    app.frame = Duration::from_secs_f32(1.0 / fps);
    log::info!("start version {} mode {mode:?} strength {} fights {} seed {seed} audio {}", env!("CARGO_PKG_VERSION"), strength.name(), style.name(), app.audio.available());

    let mut terminal = ratatui::init();
    let kitty = supports_keyboard_enhancement().unwrap_or(false);
    if kitty {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES | KeyboardEnhancementFlags::REPORT_EVENT_TYPES;
        execute!(io::stdout(), PushKeyboardEnhancementFlags(flags))?;
    }
    execute!(io::stdout(), EnableMouseCapture)?;
    app.y_scale = cell_y_scale();
    let size = terminal.size()?;
    log::info!("terminal {}x{} kitty_keyboard {kitty} y_scale {:.3}", size.width, size.height, app.y_scale);
    let result = run(&mut terminal, &mut app);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    if kitty {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}

/// How much wider than tall a half-block pixel is (see Ratty's Cube).
fn cell_y_scale() -> f32 {
    match window_size() {
        Ok(ws) if ws.columns > 0 && ws.rows > 0 && ws.width > 0 && ws.height > 0 => {
            let ratio = (ws.height as f32 / ws.rows as f32) / (ws.width as f32 / ws.columns as f32);
            if (1.5..=2.8).contains(&ratio) { 2.0 / ratio } else { 1.0 }
        }
        _ => 1.0,
    }
}

/// A frame drawn without a terminal, as a PPM picture: a position, a move,
/// and how far into it. For checking the fights by eye.
fn shot(path: &str, fen: Option<String>, mv: Option<String>, at: f32, size: Option<String>, style: Style) -> io::Result<()> {
    let (w, h) = size.and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse::<u16>().ok()?, b.parse::<u16>().ok()?)))).unwrap_or((100, 30));
    let mut app = App::new(Mode::Two, Strength::Easy, style, 1, audio::Audio::none());
    if let Some(f) = fen {
        app.game.board = f.parse().map_err(|e| io::Error::other(format!("bad FEN: {e:?}")))?;
    }
    if let Some(m) = mv {
        let from = m[0..2].parse().map_err(|_| io::Error::other("bad move"))?;
        let to = m[2..4].parse().map_err(|_| io::Error::other("bad move"))?;
        let promo = m.chars().nth(4).and_then(|c| match c {
            'q' => Some(Piece::Queen),
            'r' => Some(Piece::Rook),
            'b' => Some(Piece::Bishop),
            'n' => Some(Piece::Knight),
            _ => None,
        });
        let m = game::find_move(&app.game.board, from, to, promo).ok_or_else(|| io::Error::other("not a legal move"))?;
        app.play(m);
        if let Some(a) = app.anim.as_mut() {
            a.advance(at);
        }
    }
    let area = ratatui::layout::Rect::new(0, 0, w, h);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    ui::draw(&mut app, area, &mut buf);
    // two pixels a cell, top and bottom half; text cells as their background
    let mut out = Vec::new();
    write!(out, "P6\n{} {}\n255\n", w, h as usize * 2)?;
    for y in 0..h as usize * 2 {
        for x in 0..w {
            let cell = &buf[(x, (y / 2) as u16)];
            let c = if cell.symbol() == "▀" { if y % 2 == 0 { cell.fg } else { cell.bg } } else if cell.symbol().trim().is_empty() { cell.bg } else { cell.fg };
            let (r, g, b) = match c {
                ratatui::style::Color::Rgb(r, g, b) => (r, g, b),
                ratatui::style::Color::Black | ratatui::style::Color::Reset => (0, 0, 0),
                _ => (200, 200, 200),
            };
            out.extend([r, g, b]);
        }
    }
    std::fs::write(path, out)
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> io::Result<()> {
    let mut last = Instant::now();
    let mut snapshot = Instant::now();
    loop {
        terminal.draw(|f| ui::draw(app, f.area(), f.buffer_mut()))?;
        if event::poll(app.idle_for())? {
            loop {
                match event::read()? {
                    Event::Key(k) => key(app, k),
                    Event::Mouse(m) => mouse(app, m),
                    Event::Resize(w, h) => {
                        app.y_scale = cell_y_scale();
                        log::info!("resize {w}x{h} y_scale {:.3}", app.y_scale);
                    }
                    _ => {}
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
        let now = Instant::now();
        app.step(now.duration_since(last).as_secs_f32().min(0.1));
        last = now;
        if app.quit {
            log::info!("quit");
            return Ok(());
        }
        if snapshot.elapsed() >= SNAPSHOT_EVERY {
            snapshot = Instant::now();
            log::debug!("snapshot {}", app.snapshot());
        }
    }
}

fn key(app: &mut App, k: KeyEvent) {
    log::debug!("key {:?} {:?} {:?}", k.code, k.kind, k.modifiers);
    if k.kind == KeyEventKind::Release {
        return;
    }
    app.touch();
    if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
        app.quit = true;
        return;
    }
    if app.help {
        app.help = false;
        return;
    }
    if let Some(sel) = app.menu {
        match k.code {
            KeyCode::Up => app.menu = Some((sel + MENU.len() - 1) % MENU.len()),
            KeyCode::Down => app.menu = Some((sel + 1) % MENU.len()),
            KeyCode::Esc => app.menu = None,
            KeyCode::Enter | KeyCode::Char(' ') => {
                log::info!("menu: {}", MENU[sel]);
                match sel {
                    0 => app.new_game(Mode::White),
                    1 => app.new_game(Mode::Black),
                    2 => app.new_game(Mode::Two),
                    3 => app.new_game(Mode::Watch),
                    4 => {
                        app.strength = app.strength.next();
                        return;
                    }
                    5 => {
                        app.style = app.style.next();
                        return;
                    }
                    7 => app.quit = true,
                    _ => {}
                }
                app.menu = None;
            }
            KeyCode::Char('q') => app.quit = true,
            _ => {}
        }
        return;
    }
    if app.promoting.is_some() {
        let k = match k.code {
            KeyCode::Char('q' | 'Q') | KeyCode::Enter => Some(Piece::Queen),
            KeyCode::Char('r' | 'R') => Some(Piece::Rook),
            KeyCode::Char('b' | 'B') => Some(Piece::Bishop),
            KeyCode::Char('n' | 'N') => Some(Piece::Knight),
            KeyCode::Esc => {
                app.promoting = None;
                None
            }
            _ => None,
        };
        if let Some(k) = k {
            app.promote(k);
        }
        return;
    }
    match k.code {
        KeyCode::Char('q') => app.quit = true,
        KeyCode::Esc => {
            if app.selected.is_some() {
                app.selected = None;
            } else {
                app.menu = Some(0);
            }
        }
        KeyCode::Char('?') | KeyCode::F(1) => app.help = true,
        KeyCode::Char('u') | KeyCode::Backspace => app.undo(),
        KeyCode::Char('m') => app.toggle_sound(),
        KeyCode::Char('d') => {
            app.style = app.style.next();
            log::info!("fights {}", app.style.name());
        }
        KeyCode::Char('p') => app.show_en_prise = !app.show_en_prise,
        KeyCode::Char(',' | '[' | '<') => app.turn_view(1.0),
        KeyCode::Char('.' | ']' | '>') => app.turn_view(-1.0),
        KeyCode::Char('0') | KeyCode::Home => app.reset_view(),
        KeyCode::Char('+' | '=') => app.cam.zoom = (app.cam.zoom * 1.1).min(3.0),
        KeyCode::Char('-' | '_') => app.cam.zoom = (app.cam.zoom / 1.1).max(0.6),
        KeyCode::Left => app.move_cursor(-1, 0),
        KeyCode::Right => app.move_cursor(1, 0),
        KeyCode::Up => app.move_cursor(0, 1),
        KeyCode::Down => app.move_cursor(0, -1),
        KeyCode::Enter | KeyCode::Char(' ') => {
            if app.anim.is_some() {
                app.skip();
            } else {
                let c = app.cursor;
                app.choose(c);
            }
        }
        _ => {}
    }
}

fn mouse(app: &mut App, m: MouseEvent) {
    log::debug!("mouse {:?} at {},{} {:?}", m.kind, m.column, m.row, m.modifiers);
    app.touch();
    if app.help || app.menu.is_some() || app.promoting.is_some() {
        return;
    }
    let v = app.view_area;
    let inside = m.column >= v.x && m.column < v.x + v.width && m.row >= v.y && m.row < v.y + v.height;
    match m.kind {
        MouseEventKind::Down(MouseButton::Left) if inside => {
            if app.anim.is_some() {
                app.skip();
                return;
            }
            let (px, py) = ((m.column - v.x) as usize, (m.row - v.y) as usize * 2);
            let pick = app.canvas.as_ref().map_or(0, |c| c.pick_at(px, py + 1).max(c.pick_at(px, py)));
            if pick > 0 {
                let sq = cozy_chess::Square::index(pick as usize - 1);
                log::info!("click on {sq}");
                app.choose(sq);
            } else {
                app.drag = Some((m.column, m.row));
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(last) = app.drag {
                app.orbit(m.column as f32 - last.0 as f32, m.row as f32 - last.1 as f32);
                app.drag = Some((m.column, m.row));
            }
        }
        MouseEventKind::Up(_) => app.drag = None,
        MouseEventKind::ScrollUp => app.cam.zoom = (app.cam.zoom * 1.08).min(3.0),
        MouseEventKind::ScrollDown => app.cam.zoom = (app.cam.zoom / 1.08).max(0.6),
        _ => {}
    }
}
