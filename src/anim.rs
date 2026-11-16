//! A move, played out. A plain move walks (a knight leaps); a capture is a
//! fight on the square, each kind of attacker with its own: the pawn's
//! spear, the knight's charge, the bishop's light, the rook's stones, the
//! queen's ring, the king's sword. Chess has decided the result; the fight
//! only shows it.
//!
//! While a move plays, the board is drawn as it was before it, without the
//! pieces taking part, and this module draws those.

use crate::audio::Sound;
use crate::game::Played;
use crate::model::{Break, Pose};
use crate::raster::Vec3;
use crate::scene::{Figure, Fx, facing, square_pos, yaw_toward};
use cozy_chess::{Piece, Square};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// the fight in full, the camera close
    Full,
    /// the fight at double speed, the camera a little closer
    Quick,
    /// no fights: the taken piece just breaks
    Off,
}

impl Style {
    pub fn name(self) -> &'static str {
        match self {
            Style::Full => "full",
            Style::Quick => "quick",
            Style::Off => "off",
        }
    }
    pub fn next(self) -> Style {
        match self {
            Style::Full => Style::Quick,
            Style::Quick => Style::Off,
            Style::Off => Style::Full,
        }
    }
}

fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// 0 to 1 over [a, b], eased at both ends.
fn ease(x: f32, a: f32, b: f32) -> f32 {
    let k = clamp01((x - a) / (b - a));
    k * k * (3.0 - 2.0 * k)
}

/// A hump from 0 up to 1 and back over `w` starting at `a`.
fn bump(x: f32, a: f32, w: f32) -> f32 {
    let k = (x - a) / w;
    if (0.0..=1.0).contains(&k) { (k * PI).sin() } else { 0.0 }
}

fn lerp(a: Vec3, b: Vec3, k: f32) -> Vec3 {
    a.add(b.sub(a).scale(k))
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

fn hash01(i: u32, salt: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

/// How long each attacker's fight takes, before speeding up.
fn duel_length(p: Piece) -> f32 {
    match p {
        Piece::Pawn => 2.35,
        Piece::Knight => 2.15,
        Piece::Bishop => 2.0,
        Piece::Rook => 2.25,
        Piece::Queen => 2.35,
        Piece::King => 2.05,
    }
}

/// When in a fight the taken piece is hit for good (sounds and the like are
/// timed from the fight's own clock, in seconds).
fn duel_events(p: Piece) -> Vec<(f32, Sound)> {
    match p {
        Piece::Pawn => vec![(0.27, Sound::Clash), (0.72, Sound::Clash), (1.17, Sound::Clash), (1.45, Sound::Shatter)],
        Piece::Knight => vec![(0.6, Sound::Boom), (1.2, Sound::Shatter)],
        Piece::Bishop => vec![(0.45, Sound::Zap), (1.2, Sound::Shatter)],
        Piece::Rook => vec![(0.3, Sound::Fire), (0.65, Sound::Boom), (0.95, Sound::Fire), (1.3, Sound::Boom), (1.32, Sound::Shatter)],
        Piece::Queen => vec![(0.8, Sound::Whoosh), (1.4, Sound::Shatter)],
        Piece::King => vec![(0.95, Sound::Chop), (1.25, Sound::Shatter)],
    }
}

pub struct Anim {
    pub p: Played,
    pub t: f32,
    pub dur: f32,
    style: Style,
    /// where the mover starts, where it fights from, where the taken piece
    /// stands, where the mover ends
    a: Vec3,
    s: Vec3,
    v: Vec3,
    end: Vec3,
    /// the walk (or for a fight the approach)
    walk: f32,
    /// the fight: start, and speed (1 normal)
    duel: f32,
    speed: f32,
    /// the last step onto the square after a fight
    step: f32,
    step_len: f32,
    /// a pawn crowned: when its change begins
    promo: f32,
    events: Vec<(f32, Sound)>,
    fired: usize,
    zoom: f32,
}

/// What a frame of the move shows.
#[derive(Clone, Copy, Debug)]
pub struct Focus {
    pub at: Vec3,
    pub zoom: f32,
    pub across: Vec3,
    pub weight: f32,
}

pub struct Stage {
    pub figures: Vec<Figure>,
    /// squares of the board drawn without their piece (the actors stand in)
    pub hidden: Vec<Square>,
    pub fx: Vec<Fx>,
    /// the camera during a fight: what it looks at, how much closer, the
    /// direction of the fight (to look at it side-on), and how much of all
    /// that (0 to 1)
    pub focus: Option<Focus>,
}

impl Anim {
    pub fn new(p: Played, style: Style) -> Anim {
        let a = square_pos(p.from);
        let end = square_pos(p.to);
        let v = p.captured.map_or(end, |(_, sq)| square_pos(sq));
        let fight = p.captured.is_some() && style != Style::Off;
        let speed = if style == Style::Quick { 1.8 } else { 1.0 };
        let mut dir = v.sub(a);
        dir.y = 0.0;
        let dir = dir.norm();
        let s = if fight { v.sub(dir.scale(1.0)) } else { end };
        let dist = s.sub(a).len();
        let walk = if fight { (0.26 * dist).clamp(0.5, 1.3) / speed } else { (0.22 * dist).clamp(0.35, 1.1) * if style == Style::Off { 0.8 } else { 1.0 } };
        let mut events = Vec::new();
        let mut t = walk;
        let (duel, step, step_len);
        if fight {
            duel = t;
            for (at, snd) in duel_events(p.piece) {
                events.push((duel + at / speed, snd));
            }
            t += duel_length(p.piece) / speed;
            step = t - 0.5 / speed;
            step_len = 0.4 / speed;
            t = step + step_len;
            events.push((t, Sound::Step));
        } else {
            duel = t;
            step = t;
            step_len = 0.0;
            events.push((walk, if p.captured.is_some() { Sound::Capture } else { Sound::Move }));
            if p.captured.is_some() {
                events.push((walk + 0.05, Sound::Shatter));
                t += 0.8;
            }
        }
        let promo = t;
        if p.promotion.is_some() {
            events.push((promo + 0.1, Sound::Crown));
            t += 0.6;
        }
        events.sort_by(|x, y| x.0.total_cmp(&y.0));
        let zoom = match style {
            Style::Full => 2.5,
            Style::Quick => 1.7,
            Style::Off => 1.0,
        };
        Anim { p, t: 0.0, dur: t, style, a, s, v, end, walk, duel, speed, step, step_len, promo, events, fired: 0, zoom }
    }

    pub fn done(&self) -> bool {
        self.t >= self.dur
    }

    pub fn fight(&self) -> bool {
        self.p.captured.is_some() && self.style != Style::Off
    }

    /// Moves the clock on and returns the sounds that fall due.
    pub fn advance(&mut self, dt: f32) -> Vec<Sound> {
        self.t = (self.t + dt).min(self.dur);
        let mut out = Vec::new();
        while self.fired < self.events.len() && self.events[self.fired].0 <= self.t {
            out.push(self.events[self.fired].1);
            self.fired += 1;
        }
        out
    }

    /// Jumps to the end (sounds still due are dropped).
    pub fn finish(&mut self) {
        self.t = self.dur;
        self.fired = self.events.len();
    }

    /// A walk from `from` to `to` over [t0, t0 + len]: position, lift, yaw.
    fn walk_pose(&self, piece: Piece, from: Vec3, to: Vec3, t0: f32, len: f32, home_yaw: f32) -> Pose {
        let k = ease(self.t, t0, t0 + len);
        let lin = clamp01((self.t - t0) / len.max(1e-3));
        let pos = lerp(from, to, k);
        let mut pose = Pose::at(pos, home_yaw);
        let dist = to.sub(from).len();
        if dist > 0.01 {
            let toward = yaw_toward(from, to);
            let turn = ease(lin, 0.0, 0.15) * (1.0 - ease(lin, 0.85, 1.0));
            pose.yaw = lerp_angle(home_yaw, toward, turn);
        }
        if piece == Piece::Knight {
            pose.lift = 4.0 * lin * (1.0 - lin) * (0.35 + 0.18 * dist);
            pose.lean = 0.25 * (1.0 - 2.0 * lin);
        } else if lin > 0.0 && lin < 1.0 {
            let steps = (dist * 2.0).round().max(1.0);
            pose.lift = 0.07 * (lin * steps * PI).sin().abs();
            pose.roll = 0.06 * (lin * steps * PI).sin();
        }
        pose
    }

    pub fn stage(&self) -> Stage {
        let p = &self.p;
        let mut figures = Vec::new();
        let mut fx = Vec::new();
        let mut hidden = vec![p.from];
        let home = facing(p.colour);
        let t = self.t;

        // castling: the rook walks beside the king
        if let Some((rf, rt)) = p.rook {
            hidden.push(rf);
            let pose = self.walk_pose(Piece::Rook, square_pos(rf), square_pos(rt), 0.0, self.walk, home);
            figures.push(Figure { piece: Piece::Rook, colour: p.colour, pose, pick: 0 });
        }

        let mut mover;
        let mut victim: Option<Pose> = None;
        let victim_kind = p.captured.map(|(k, _)| k);
        if let Some((_, sq)) = p.captured {
            hidden.push(sq);
            victim = Some(Pose::at(self.v, facing(!p.colour)));
        }

        if !self.fight() {
            mover = self.walk_pose(p.piece, self.a, self.end, 0.0, self.walk, home);
            if let Some(v) = victim.as_mut() {
                let since = t - (self.walk - 0.05);
                if since > 0.0 {
                    v.broken = Break::Shatter(since);
                }
                if since > 0.0 && since < 0.3 {
                    v.flash = 1.0 - since / 0.3;
                }
            }
        } else {
            let to_v = yaw_toward(self.s, self.v);
            if t < self.duel {
                mover = self.walk_pose(p.piece, self.a, self.s, 0.0, self.walk, home);
                // face the enemy at the end of the approach
                mover.yaw = lerp_angle(mover.yaw, to_v, ease(t, self.walk * 0.7, self.walk));
            } else {
                mover = Pose::at(self.s, to_v);
            }
            let v = victim.as_mut().expect("a fight has a victim");
            // the taken piece turns to face its attacker, and trembles
            let face = yaw_toward(self.v, self.a);
            v.yaw = lerp_angle(v.yaw, face, ease(t, self.walk * 0.3, self.walk));
            let u = (t - self.duel) * self.speed;
            self.duel_pose(&mut mover, v, u, &mut fx);
            // after the fight: the last step onto the square
            if t >= self.step {
                let k = ease(t, self.step, self.step + self.step_len);
                let base = mover;
                mover = Pose { pos: lerp(self.s, self.end, k), yaw: lerp_angle(to_v, home, k), lean: base.lean * (1.0 - k), arm: base.arm * (1.0 - k), lift: 0.06 * (k * PI).sin(), ..Pose::at(self.s, home) };
            }
        }

        // a pawn crowned: it shrinks into light and the new piece grows
        if let Some(new) = p.promotion
            && t >= self.promo
        {
            let k = ease(t, self.promo, self.promo + 0.5);
            let mut old = mover;
            old.size = 1.0 - k;
            old.flash = k;
            mover.size = k.max(0.01);
            mover.flash = 1.0 - k;
            if old.size > 0.02 {
                figures.push(Figure { piece: Piece::Pawn, colour: p.colour, pose: old, pick: 0 });
            }
            figures.push(Figure { piece: new, colour: p.colour, pose: mover, pick: 0 });
            for i in 0..10 {
                let a = i as f32 / 10.0 * TAU + t * 3.0;
                let r = 0.45;
                fx.push(Fx::Spark { pos: mover.pos.add(Vec3::new(a.cos() * r, 0.1 + 1.2 * k * hash01(i, 3), a.sin() * r)), size: 0.04, rgb: [255.0, 230.0, 120.0] });
            }
        } else {
            figures.push(Figure { piece: p.piece, colour: p.colour, pose: mover, pick: 0 });
        }
        if let (Some(v), Some(k)) = (victim, victim_kind) {
            figures.push(Figure { piece: k, colour: !p.colour, pose: v, pick: 0 });
        }

        let focus = if self.fight() && self.style != Style::Off {
            let w = ease(t, 0.0, self.walk * 0.9) * (1.0 - ease(t, self.dur - 0.7 / self.speed, self.dur));
            let mid = lerp(self.s, self.v, 0.5).add(Vec3::new(0.0, 0.45, 0.0));
            Some(Focus { at: mid, zoom: self.zoom, across: self.v.sub(self.s), weight: w })
        } else {
            None
        };
        Stage { figures, hidden, fx, focus }
    }

    /// The fight itself at fight time `u` (seconds at normal speed).
    fn duel_pose(&self, m: &mut Pose, v: &mut Pose, u: f32, fx: &mut Vec<Fx>) {
        let dir = {
            let mut d = self.v.sub(self.s);
            d.y = 0.0;
            d.norm()
        };
        let tremble = |v: &mut Pose, a: f32, b: f32| {
            if u > a && u < b {
                v.roll = 0.07 * (u * 45.0).sin();
            }
        };
        let flash_at = |v: &mut Pose, at: f32| {
            let since = u - at;
            if (0.0..0.3).contains(&since) {
                v.flash = v.flash.max(1.0 - since / 0.3);
            }
        };
        let burst = |fx: &mut Vec<Fx>, at: Vec3, since: f32, n: u32, rgb: [f32; 3], speed: f32| {
            if !(0.0..0.6).contains(&since) {
                return;
            }
            for i in 0..n {
                let a = hash01(i, 11) * TAU;
                let up = hash01(i, 12);
                let d = Vec3::new(a.cos(), up * 1.2, a.sin());
                let pos = at.add(d.scale(speed * since)).add(Vec3::new(0.0, -2.0 * since * since, 0.0));
                fx.push(Fx::Spark { pos, size: 0.05 * (1.0 - since / 0.6), rgb });
            }
        };
        if u < -0.4 {
            return;
        }
        match self.p.piece {
            Piece::Pawn => {
                // spear levelled, three jabs, the third one through
                m.arm = 1.45 * ease(u, -0.3, 0.1);
                for k in 0..3 {
                    let at = 0.15 + 0.45 * k as f32;
                    let jab = bump(u, at, 0.25);
                    m.pos = m.pos.add(dir.scale(0.2 * jab));
                    m.lean += 0.15 * jab;
                    flash_at(v, at + 0.12);
                    let push = ease(u, at + 0.1, at + 0.2) * 0.07;
                    v.pos = v.pos.add(dir.scale(push));
                }
                tremble(v, -0.4, 0.27);
                v.lean = -1.5 * ease(u, 1.17, 1.5);
                if u > 1.45 {
                    v.broken = Break::Shatter(u - 1.45);
                }
                m.arm *= 1.0 - ease(u, 1.6, 2.0);
            }
            Piece::Knight => {
                // rears up, crashes down, and the enemy goes flying
                m.lean = -0.65 * ease(u, 0.0, 0.45) + 1.0 * ease(u, 0.45, 0.6) - 0.35 * ease(u, 0.6, 0.95);
                m.lift = 0.3 * ease(u, 0.0, 0.45) * (1.0 - ease(u, 0.45, 0.6));
                tremble(v, -0.4, 0.6);
                let land = self.v.add(dir.scale(1.4));
                if u > 0.6 {
                    let k = clamp01((u - 0.6) / 0.6);
                    v.pos = lerp(self.v, land, k);
                    v.lift = 1.3 * 4.0 * k * (1.0 - k) * 0.5;
                    v.lean = -2.5 * k;
                    v.roll = 3.0 * k;
                }
                flash_at(v, 0.6);
                if u > 1.2 {
                    v.broken = Break::Shatter(u - 1.2);
                }
                // dust where the hooves land
                if (0.6..1.1).contains(&u) {
                    let k = (u - 0.6) / 0.5;
                    for i in 0..14 {
                        let a = i as f32 / 14.0 * TAU;
                        let r = 0.25 + 0.75 * k;
                        fx.push(Fx::Spark { pos: self.s.add(dir.scale(0.3)).add(Vec3::new(a.cos() * r, 0.05 + 0.15 * k, a.sin() * r)), size: 0.05 * (1.0 - k), rgb: [200.0, 180.0, 140.0] });
                    }
                }
                burst(fx, land.add(Vec3::new(0.0, 0.2, 0.0)), u - 1.2, 10, [255.0, 220.0, 160.0], 1.5);
            }
            Piece::Bishop => {
                // staff raised, a bolt of light, the enemy melts away
                m.arm = 0.95 * ease(u, 0.0, 0.4) * (1.0 - ease(u, 1.4, 1.8));
                m.flash = 0.6 * bump(u, 0.25, 0.4);
                tremble(v, -0.4, 0.5);
                if (0.45..1.35).contains(&u) {
                    let tip = m.tip_world(Piece::Bishop);
                    let w = 0.035 + 0.02 * (u * 40.0).sin().abs();
                    fx.push(Fx::Beam { a: tip, b: v.centre(), width: w, rgb: [200.0, 235.0, 255.0] });
                    fx.push(Fx::Spark { pos: tip, size: 0.09, rgb: [255.0, 255.0, 255.0] });
                }
                v.flash = ease(u, 0.5, 0.8);
                if u > 0.7 {
                    v.broken = Break::Dissolve(ease(u, 0.7, 1.55));
                }
                if (0.7..1.9).contains(&u) {
                    for i in 0..10 {
                        let k = ((u - 0.7) * 0.9 + hash01(i, 5)) % 1.0;
                        let a = hash01(i, 6) * TAU;
                        fx.push(Fx::Spark { pos: self.v.add(Vec3::new(a.cos() * 0.25, 0.1 + 1.3 * k, a.sin() * 0.25)), size: 0.04 * (1.0 - k), rgb: [190.0, 230.0, 255.0] });
                    }
                }
            }
            Piece::Rook => {
                // two stones from the battlements; the second one finishes it
                for (fire, hit) in [(0.3, 0.65), (0.95, 1.3)] {
                    m.squash -= 0.16 * bump(u, fire, 0.3);
                    if (fire..hit).contains(&u) {
                        let k = (u - fire) / (hit - fire);
                        let from = m.world(crate::model::shoulder(Piece::Rook));
                        let to = v.centre();
                        let pos = lerp(from, to, k).add(Vec3::new(0.0, 0.9 * 4.0 * k * (1.0 - k), 0.0));
                        fx.push(Fx::Stone { pos, r: 0.07, rgb: [150.0, 140.0, 130.0] });
                    }
                    flash_at(v, hit);
                    burst(fx, v.centre(), u - hit, 8, [255.0, 190.0, 90.0], 1.2);
                }
                tremble(v, -0.4, 0.65);
                v.lean = -0.35 * bump(u, 0.65, 0.4);
                if u > 1.32 {
                    v.broken = Break::Shatter(u - 1.32);
                }
            }
            Piece::Queen => {
                // a spin, a ring of light, the enemy lifted and broken
                m.yaw += 2.0 * TAU * ease(u, 0.0, 0.9);
                m.lift = 0.25 * bump(u, 0.0, 0.9);
                m.arm = 1.2 * ease(u, 0.7, 0.85) * (1.0 - ease(u, 1.6, 1.9));
                if (0.8..1.45).contains(&u) {
                    let k = (u - 0.8) / 0.65;
                    let r = 0.3 + 2.2 * k;
                    for i in 0..22 {
                        let a = i as f32 / 22.0 * TAU + u * 2.0;
                        fx.push(Fx::Spark { pos: self.s.add(Vec3::new(a.cos() * r, 0.45, a.sin() * r)), size: 0.06 * (1.0 - 0.6 * k), rgb: [255.0, 130.0, 235.0] });
                    }
                }
                tremble(v, -0.4, 0.9);
                v.lift = 0.6 * ease(u, 0.9, 1.4);
                v.yaw += 3.0 * ease(u, 0.9, 1.4);
                v.flash = ease(u, 0.95, 1.4);
                if u > 1.4 {
                    v.broken = Break::Shatter(u - 1.4);
                }
                burst(fx, v.centre(), u - 1.4, 12, [255.0, 160.0, 240.0], 1.4);
            }
            Piece::King => {
                // the sword raised high, one blow straight down
                m.arm = -0.7 * ease(u, 0.0, 0.7) + 2.4 * ease(u, 0.83, 0.95) - 1.7 * ease(u, 1.3, 1.8);
                m.lift = 0.14 * ease(u, 0.45, 0.8) * (1.0 - ease(u, 0.83, 0.95));
                m.lean = 0.25 * ease(u, 0.83, 0.95) * (1.0 - ease(u, 1.2, 1.6));
                tremble(v, -0.4, 0.95);
                flash_at(v, 0.95);
                if u > 0.95 {
                    v.broken = Break::Split(u - 0.95, dir);
                }
                // the cut, a line of light down the middle
                if (0.93..1.15).contains(&u) {
                    let k = (u - 0.93) / 0.22;
                    let top = self.v.add(Vec3::new(0.0, 1.25, 0.0));
                    let bottom = self.v.add(Vec3::new(0.0, 0.02, 0.0));
                    fx.push(Fx::Beam { a: top, b: lerp(top, bottom, (k * 2.0).min(1.0)), width: 0.03 * (1.0 - 0.5 * k), rgb: [255.0, 255.0, 240.0] });
                }
                burst(fx, m.tip_world(Piece::King), u - 0.95, 12, [255.0, 240.0, 200.0], 1.3);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Game, describe, find_move};
    use cozy_chess::Board;

    fn played(fen: &str, from: &str, to: &str) -> Played {
        let b: Board = fen.parse().unwrap();
        let m = find_move(&b, from.parse().unwrap(), to.parse().unwrap(), None).unwrap();
        describe(&b, m)
    }

    #[test]
    fn every_attacker_fights_to_the_end() {
        // each kind takes a black knight on d5
        for (fen, from) in [
            ("4k3/8/8/3n4/4P3/8/8/4K3 w - - 0 1", "e4"),
            ("4k3/8/8/3n4/8/4N3/8/4K3 w - - 0 1", "e3"),
            ("4k3/8/8/3n4/8/8/6B1/4K3 w - - 0 1", "g2"),
            ("4k3/8/8/3n4/8/8/8/3RK3 w - - 0 1", "d1"),
            ("4k3/8/8/3n4/8/8/8/3QK3 w - - 0 1", "d1"),
            ("4k3/8/8/3n4/4K3/8/8/8 w - - 0 1", "e4"),
        ] {
            for style in [Style::Full, Style::Quick, Style::Off] {
                let mut a = Anim::new(played(fen, from, "d5"), style);
                let mut sounds = 0;
                let mut was_whole_late = false;
                while !a.done() {
                    sounds += a.advance(1.0 / 30.0).len();
                    let st = a.stage();
                    assert!(st.hidden.contains(&a.p.from) && st.hidden.len() == 2);
                    let victim = st.figures.last().unwrap();
                    if a.t > a.dur - 0.05 {
                        was_whole_late |= victim.pose.broken == Break::Whole;
                    }
                }
                assert!(sounds >= 2, "{from} {style:?}: {sounds} sounds");
                assert!(!was_whole_late, "{from} {style:?}: the taken piece is still whole at the end");
                // the mover ends on its square, facing ahead
                let st = a.stage();
                let m = st.figures[0].pose;
                assert!(m.pos.sub(square_pos("d5".parse().unwrap())).len() < 0.01, "{from} {style:?} ends at {:?}", m.pos);
            }
        }
    }

    #[test]
    fn castling_walks_both() {
        let mut g = Game::new();
        g.board = "4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1".parse().unwrap();
        let m = find_move(&g.board, "e1".parse().unwrap(), "g1".parse().unwrap(), None).unwrap();
        let mut a = Anim::new(g.play(m), Style::Full);
        a.advance(100.0);
        let st = a.stage();
        assert_eq!(st.figures.len(), 2);
        assert!(st.hidden.contains(&"h1".parse().unwrap()));
    }
}
