//! The game's state: the board, whose turn, the computer in its thread, the
//! move being played out, the camera, and what the player has picked up.

use crate::ai::{self, Strength};
use crate::anim::{Anim, Style};
use crate::audio::{Audio, Sound};
use crate::game::{self, Game, Status, find_move, landing, legal_moves};
use crate::raster::{Canvas, Vec3};
use crate::rng::Rng;
use crate::scene::{Camera, Figure, Fx, Mark, facing, home_yaw, square_pos};
use crate::model::Pose;
use cozy_chess::{Color, Move, Piece, Square};
use ratatui::layout::Rect;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    White,
    Black,
    Two,
    Watch,
}

pub const MENU: [&str; 8] = ["New game as White", "New game as Black", "Two players", "Watch the computer", "Strength", "Fights", "Resume", "Quit"];

pub struct App {
    pub game: Game,
    pub mode: Mode,
    pub strength: Strength,
    pub style: Style,
    rng: Rng,
    pub audio: Audio,
    pub cam: Camera,
    yaw_goal: f32,
    pub anim: Option<Anim>,
    pub selected: Option<Square>,
    pub cursor: Square,
    pub promoting: Option<(Square, Square)>,
    thinking: Option<(Receiver<Option<Move>>, u64)>,
    generation: u64,
    pub menu: Option<usize>,
    pub help: bool,
    pub quit: bool,
    pub canvas: Option<Canvas>,
    pub view_area: Rect,
    pub y_scale: f32,
    pub show_en_prise: bool,
    pub time: f32,
    pub drag: Option<(u16, u16)>,
    pub frame: Duration,
    /// seconds since the game ended, for the fallen king
    pub ended: f32,
    last_input: Instant,
}

/// What the view draws this frame.
pub struct View {
    pub figures: Vec<Figure>,
    pub marks: [Mark; 64],
    pub fx: Vec<Fx>,
    pub cam: Camera,
}

fn ease(x: f32) -> f32 {
    let k = x.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

fn lerp_angle(a: f32, b: f32, k: f32) -> f32 {
    let mut d = (b - a) % TAU;
    if d > PI {
        d -= TAU;
    }
    if d < -PI {
        d += TAU;
    }
    a + d * k
}

/// The camera yaw nearest `now` that sees a fight along `across` from the
/// side (a little turned toward the camera's old side, three-quarters on).
fn side_on(across: Vec3, now: f32) -> f32 {
    let base = across.z.atan2(across.x);
    let mut best = base;
    let mut best_d = f32::MAX;
    for c in [base, base + PI] {
        for off in [0.35f32, -0.35] {
            let y = c + off;
            let d = lerp_angle(now, y, 1.0) - now;
            if d.abs() < best_d {
                best_d = d.abs();
                best = now + d;
            }
        }
    }
    best
}

impl App {
    pub fn new(mode: Mode, strength: Strength, style: Style, seed: u64, audio: Audio) -> App {
        let side = if mode == Mode::Black { Color::Black } else { Color::White };
        let cam = Camera::home(side);
        App {
            game: Game::new(),
            mode,
            strength,
            style,
            rng: Rng::new(seed),
            audio,
            cam,
            yaw_goal: cam.yaw,
            anim: None,
            selected: None,
            cursor: if side == Color::White { Square::E2 } else { Square::E7 },
            promoting: None,
            thinking: None,
            generation: 0,
            menu: None,
            help: false,
            quit: false,
            canvas: None,
            view_area: Rect::default(),
            y_scale: 1.0,
            show_en_prise: true,
            time: 0.0,
            drag: None,
            frame: Duration::from_millis(33),
            ended: 0.0,
            last_input: Instant::now(),
        }
    }

    pub fn new_game(&mut self, mode: Mode) {
        log::info!("new game {mode:?}");
        let audio = std::mem::replace(&mut self.audio, Audio::none());
        let seed = self.rng.next();
        let (y, f, c, a) = (self.y_scale, self.frame, self.canvas.take(), self.view_area);
        *self = App::new(mode, self.strength, self.style, seed, audio);
        self.y_scale = y;
        self.frame = f;
        self.canvas = c;
        self.view_area = a;
    }

    pub fn touch(&mut self) {
        self.last_input = Instant::now();
    }

    pub fn status(&self) -> Status {
        self.game.status()
    }

    pub fn human_turn(&self) -> bool {
        match self.mode {
            Mode::White => self.game.board.side_to_move() == Color::White,
            Mode::Black => self.game.board.side_to_move() == Color::Black,
            Mode::Two => true,
            Mode::Watch => false,
        }
    }

    pub fn thinking(&self) -> bool {
        self.thinking.is_some()
    }

    /// The side whose pieces the en prise warning watches.
    pub fn watched(&self) -> Option<Color> {
        match self.mode {
            Mode::White => Some(Color::White),
            Mode::Black => Some(Color::Black),
            Mode::Two => Some(self.game.board.side_to_move()),
            Mode::Watch => None,
        }
    }

    pub fn busy(&self) -> bool {
        self.anim.is_some() || self.thinking.is_some() || (self.cam.yaw - self.yaw_goal).abs() > 1e-3 || self.drag.is_some() || (self.show_en_prise && self.watched().is_some())
    }

    pub fn idle_for(&self) -> Duration {
        if self.busy() || (self.status().over() && self.ended < 2.0) { self.frame } else { Duration::from_millis(250) }
    }

    pub fn step(&mut self, dt: f32) {
        self.time += dt;
        let k = (dt * 9.0).min(1.0);
        self.cam.yaw += (self.yaw_goal - self.cam.yaw) * k;
        if (self.yaw_goal - self.cam.yaw).abs() < 1e-3 {
            self.cam.yaw = self.yaw_goal;
        }
        if let Some(a) = self.anim.as_mut() {
            for s in a.advance(dt) {
                self.audio.play(s);
            }
            if a.done() {
                self.anim = None;
                self.after_move();
            }
            return;
        }
        if self.status().over() {
            self.ended += dt;
            return;
        }
        if let Some((rx, generation)) = &self.thinking {
            if let Ok(mv) = rx.try_recv() {
                let ours = *generation == self.generation;
                self.thinking = None;
                if ours {
                    match mv {
                        Some(m) => self.play(m),
                        None => log::warn!("the computer found no move"),
                    }
                }
            }
            return;
        }
        if !self.human_turn() {
            self.think();
        }
    }

    fn think(&mut self) {
        let (tx, rx) = channel();
        let board = self.game.board.clone();
        let history = self.game.hashes();
        let strength = self.strength;
        let mut rng = Rng::new(self.rng.next());
        std::thread::spawn(move || {
            let _ = tx.send(ai::choose(&board, &history, strength, &mut rng));
        });
        self.thinking = Some((rx, self.generation));
    }

    pub fn play(&mut self, m: Move) {
        self.selected = None;
        self.promoting = None;
        let p = self.game.play(m);
        log::info!("move {} ({} {} {} -> {}), captured {:?}", p.san, game::side(p.colour), game::name(p.piece), p.from, p.to, p.captured.map(|(k, s)| (game::name(k), s.to_string())));
        self.anim = Some(Anim::new(p, self.style));
        self.ended = 0.0;
    }

    fn after_move(&mut self) {
        let s = self.status();
        log::info!("status {s:?}");
        match s {
            Status::Mate(winner) => {
                let ours = match self.mode {
                    Mode::White => winner == Color::White,
                    Mode::Black => winner == Color::Black,
                    _ => true,
                };
                self.audio.play(Sound::Mate(ours));
            }
            Status::Playing => {
                if self.game.in_check() {
                    self.audio.play(Sound::Check);
                }
            }
            _ => self.audio.play(Sound::Draw),
        }
    }

    /// Skips the move being played out.
    pub fn skip(&mut self) {
        if let Some(a) = self.anim.as_mut() {
            log::info!("skip the animation at {:.2}/{:.2}", a.t, a.dur);
            a.finish();
        }
    }

    /// A square picked, by click or by Enter on the cursor.
    pub fn choose(&mut self, sq: Square) {
        self.cursor = sq;
        if self.anim.is_some() {
            self.skip();
            return;
        }
        if !self.human_turn() || self.status().over() {
            return;
        }
        let b = &self.game.board;
        let us = b.side_to_move();
        if let Some(from) = self.selected {
            if from == sq {
                self.selected = None;
                return;
            }
            let moves: Vec<Move> = legal_moves(b).into_iter().filter(|m| m.from == from && (landing(b, *m) == sq || m.to == sq)).collect();
            if let Some(&m) = moves.first() {
                if m.promotion.is_some() {
                    self.promoting = Some((from, m.to));
                    log::info!("promotion choice for {from}{}", m.to);
                } else {
                    self.play(m);
                }
                return;
            }
        }
        if b.color_on(sq) == Some(us) {
            self.selected = Some(sq);
            self.audio.play(Sound::Pick);
            log::info!("picked up {sq}");
        } else {
            self.selected = None;
        }
    }

    pub fn promote(&mut self, k: Piece) {
        if let Some((from, to)) = self.promoting.take()
            && let Some(m) = find_move(&self.game.board, from, to, Some(k))
        {
            self.play(m);
        }
    }

    pub fn undo(&mut self) {
        if self.mode == Mode::Watch {
            return;
        }
        if let Some(a) = self.anim.as_mut() {
            a.finish();
        }
        self.anim = None;
        self.thinking = None;
        self.generation += 1;
        self.selected = None;
        self.promoting = None;
        if !self.game.undo() {
            return;
        }
        if self.mode != Mode::Two && !self.human_turn() {
            self.game.undo();
        }
        self.ended = 0.0;
        log::info!("take back, {} moves now", self.game.played.len());
    }

    pub fn toggle_sound(&mut self) {
        self.audio.muted = !self.audio.muted;
        self.audio.play(Sound::Toggle);
    }

    pub fn turn_view(&mut self, quarters: f32) {
        self.yaw_goal += quarters * FRAC_PI_2;
    }

    pub fn reset_view(&mut self) {
        let side = if self.mode == Mode::Black { Color::Black } else { Color::White };
        let home = home_yaw(side);
        // the nearest turn that lands on home
        let turns = ((self.cam.yaw - home) / TAU).round();
        self.yaw_goal = home + turns * TAU;
        self.cam.zoom = 1.0;
        self.cam.pitch = crate::scene::PITCH;
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.cam.yaw += dx * 0.05;
        self.yaw_goal = self.cam.yaw;
        self.cam.pitch = (self.cam.pitch + dy * 0.04).clamp(0.25, 1.45);
    }

    /// The arrows move the cursor as the board looks from here: up is away.
    pub fn move_cursor(&mut self, right: i8, up: i8) {
        let q = (((self.yaw_goal - home_yaw(Color::White)) / FRAC_PI_2).round() as i32).rem_euclid(4);
        let (mut df, mut dr) = (right, up);
        for _ in 0..q {
            (df, dr) = (dr, -df);
        }
        if let Some(s) = self.cursor.try_offset(df, dr) {
            self.cursor = s;
        }
    }

    /// The figures, square marks and effects of this frame, and the camera.
    pub fn view(&self) -> View {
        let mut marks = [Mark::None; 64];
        let mut figures = Vec::with_capacity(34);
        let mut fx = Vec::new();
        let mut cam = self.cam;
        let board = self.anim.as_ref().map_or(&self.game.board, |a| &a.p.before);
        let mut hidden: Vec<Square> = Vec::new();
        if let Some(a) = &self.anim {
            let st = a.stage();
            hidden = st.hidden;
            figures.extend(st.figures);
            fx = st.fx;
            if let Some(f) = st.focus {
                let w = f.weight;
                cam.target = cam.target.add(f.at.sub(cam.target).scale(w));
                cam.zoom *= 1.0 + (f.zoom - 1.0) * w;
                cam.pitch += (0.45 - cam.pitch) * w;
                cam.yaw = lerp_angle(cam.yaw, side_on(f.across, self.cam.yaw), w);
            }
        } else {
            if let Some(last) = self.game.played.last() {
                marks[last.from as usize] = Mark::LastMove;
                marks[last.to as usize] = Mark::LastMove;
            }
            if let Some(c) = self.watched()
                && self.show_en_prise
                && !self.status().over()
            {
                for s in game::en_prise(&self.game.board, c) {
                    marks[s as usize] = Mark::EnPrise;
                }
            }
            if self.game.in_check() {
                marks[self.game.board.king(self.game.board.side_to_move()) as usize] = Mark::Check;
            }
            if let Some(sel) = self.selected {
                marks[sel as usize] = Mark::Selected;
                for m in legal_moves(&self.game.board).into_iter().filter(|m| m.from == sel) {
                    marks[landing(&self.game.board, m) as usize] = Mark::Target;
                }
            }
            if self.human_turn() && !self.status().over() {
                marks[self.cursor as usize] = if self.selected == Some(self.cursor) { Mark::Selected } else { Mark::Cursor };
            }
        }
        let mated = match self.status() {
            Status::Mate(w) if self.anim.is_none() => Some(board.king(!w)),
            _ => None,
        };
        for sq in board.occupied() {
            if hidden.contains(&sq) {
                continue;
            }
            let (Some(piece), Some(colour)) = (board.piece_on(sq), board.color_on(sq)) else { continue };
            let mut pose = Pose::at(square_pos(sq), facing(colour));
            if mated == Some(sq) {
                let k = ease(self.ended / 0.9);
                pose.lean = -1.45 * k;
                pose.roll = 0.3 * k;
            }
            figures.push(Figure { piece, colour, pose, pick: sq as u16 + 1 });
        }
        View { figures, marks, fx, cam }
    }

    pub fn snapshot(&self) -> String {
        format!(
            "moves {} turn {} status {:?} human {} thinking {} anim {} selected {:?} cursor {} yaw {:.2} zoom {:.2} style {} strength {}",
            self.game.played.len(),
            game::side(self.game.board.side_to_move()),
            self.status(),
            self.human_turn(),
            self.thinking(),
            self.anim.as_ref().map_or("-".into(), |a| format!("{} {:.2}/{:.2}", a.p.san, a.t, a.dur)),
            self.selected.map(|s| s.to_string()),
            self.cursor,
            self.cam.yaw,
            self.cam.zoom,
            self.style.name(),
            self.strength.name()
        )
    }

    /// For the help line: does a fight fill the view now?
    pub fn fight_title(&self) -> Option<String> {
        let a = self.anim.as_ref()?;
        let (k, _) = a.p.captured?;
        if !a.fight() {
            return None;
        }
        Some(format!("{} {} takes {} {}", game::side(a.p.colour), game::name(a.p.piece), game::side(!a.p.colour).to_lowercase(), game::name(k)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(mode: Mode) -> App {
        App::new(mode, Strength::Easy, Style::Quick, 1, Audio::none())
    }

    fn run_until_idle(a: &mut App) {
        for _ in 0..2000 {
            a.step(0.05);
            if a.anim.is_none() && !a.thinking() && (a.human_turn() || a.status().over()) {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("never settled: {}", a.snapshot());
    }

    #[test]
    fn a_move_by_clicks_then_the_computer_answers() {
        let mut a = app(Mode::White);
        a.choose(Square::E2);
        assert_eq!(a.selected, Some(Square::E2));
        a.choose(Square::E4);
        assert!(a.anim.is_some());
        run_until_idle(&mut a);
        assert_eq!(a.game.played.len(), 2);
        assert!(a.human_turn());
        a.undo();
        assert_eq!(a.game.played.len(), 0);
    }

    #[test]
    fn castling_by_clicking_the_king_s_square() {
        let mut a = app(Mode::Two);
        a.game.board = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1".parse().unwrap();
        a.choose(Square::E1);
        a.choose(Square::G1);
        a.skip();
        a.step(0.01);
        assert_eq!(a.game.board.piece_on(Square::F1), Some(Piece::Rook));
        assert_eq!(a.game.board.piece_on(Square::G1), Some(Piece::King));
    }

    #[test]
    fn a_pawn_asks_what_to_become() {
        let mut a = app(Mode::Two);
        a.game.board = "4k3/1P6/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
        a.choose(Square::B7);
        a.choose(Square::B8);
        assert_eq!(a.promoting, Some((Square::B7, Square::B8)));
        a.promote(Piece::Knight);
        a.skip();
        a.step(0.01);
        assert_eq!(a.game.board.piece_on(Square::B8), Some(Piece::Knight));
    }

    #[test]
    fn the_cursor_follows_the_turned_board() {
        let mut a = app(Mode::White);
        a.cursor = Square::E4;
        a.move_cursor(0, 1);
        assert_eq!(a.cursor, Square::E5);
        a.turn_view(2.0);
        a.move_cursor(0, 1);
        assert_eq!(a.cursor, Square::E4);
    }

    #[test]
    fn the_computer_plays_itself_to_the_end_or_a_while() {
        let mut a = app(Mode::Watch);
        for _ in 0..30_000 {
            a.step(0.1);
            if a.status().over() || a.game.played.len() >= 30 {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(a.game.played.len() >= 10, "{}", a.snapshot());
        let v = a.view();
        assert!(!v.figures.is_empty());
    }
}
