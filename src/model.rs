//! The pieces as little figures: each is a few turned (lathe) shapes and
//! boxes, with a weapon or tool held at a shoulder that can swing. A pose
//! places, turns, tips and squashes a figure, and can break it apart.
//!
//! Local space: the foot's centre at the origin, y up, +z the way the
//! piece faces (toward the enemy), +x its right hand. One square is 1 wide.

use crate::raster::{Mat3, Vec3};
use cozy_chess::{Color, Piece};
use std::f32::consts::TAU;

/// How big a figure is on its square.
pub const SCALE: f32 = 0.95;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint {
    Body,
    Accent,
    Metal,
    Wood,
    Dark,
}

pub fn paint(p: Paint, c: Color) -> [f32; 3] {
    match (c, p) {
        (Color::White, Paint::Body) => [236.0, 228.0, 208.0],
        (Color::White, Paint::Accent) => [222.0, 176.0, 58.0],
        (Color::Black, Paint::Body) => [84.0, 78.0, 100.0],
        (Color::Black, Paint::Accent) => [196.0, 46.0, 56.0],
        (_, Paint::Metal) => [196.0, 202.0, 214.0],
        (_, Paint::Wood) => [128.0, 86.0, 50.0],
        (_, Paint::Dark) => [26.0, 22.0, 26.0],
    }
}

enum Shape {
    /// (height, radius) from the bottom up, turned about the y axis
    Lathe(&'static [(f32, f32)]),
    /// centre and half sizes
    Cuboid([f32; 3], [f32; 3]),
}

struct Part {
    shape: Shape,
    paint: Paint,
    /// held: placed relative to the shoulder and swung with the arm
    held: bool,
}

const fn lathe(p: &'static [(f32, f32)], paint: Paint) -> Part {
    Part { shape: Shape::Lathe(p), paint, held: false }
}
const fn cuboid(c: [f32; 3], h: [f32; 3], paint: Paint) -> Part {
    Part { shape: Shape::Cuboid(c, h), paint, held: false }
}
const fn held(c: [f32; 3], h: [f32; 3], paint: Paint) -> Part {
    Part { shape: Shape::Cuboid(c, h), paint, held: true }
}

const BASE: &[(f32, f32)] = &[(0.0, 0.0), (0.0, 0.40), (0.07, 0.40), (0.10, 0.33)];

const PAWN: &[Part] = &[
    lathe(BASE, Paint::Accent),
    lathe(&[(0.10, 0.30), (0.40, 0.17), (0.48, 0.13)], Paint::Body),
    lathe(&[(0.48, 0.06), (0.52, 0.13), (0.60, 0.16), (0.68, 0.13), (0.72, 0.0)], Paint::Body),
    lathe(&[(0.62, 0.17), (0.66, 0.17), (0.80, 0.0)], Paint::Accent),
    // spear
    held([0.0, 0.18, 0.0], [0.022, 0.50, 0.022], Paint::Wood),
    held([0.0, 0.72, 0.0], [0.05, 0.09, 0.05], Paint::Metal),
];

const KNIGHT: &[Part] = &[
    lathe(BASE, Paint::Accent),
    lathe(&[(0.10, 0.32), (0.30, 0.24), (0.50, 0.21), (0.52, 0.0)], Paint::Body),
    // neck, head, muzzle, mane, ears
    cuboid([0.0, 0.64, 0.02], [0.13, 0.16, 0.14], Paint::Body),
    cuboid([0.0, 0.84, 0.12], [0.12, 0.11, 0.20], Paint::Body),
    cuboid([0.0, 0.78, 0.33], [0.09, 0.08, 0.06], Paint::Body),
    cuboid([0.0, 0.80, -0.10], [0.05, 0.20, 0.07], Paint::Accent),
    cuboid([0.07, 0.99, 0.02], [0.03, 0.06, 0.03], Paint::Body),
    cuboid([-0.07, 0.99, 0.02], [0.03, 0.06, 0.03], Paint::Body),
    cuboid([0.125, 0.87, 0.18], [0.01, 0.025, 0.025], Paint::Dark),
    cuboid([-0.125, 0.87, 0.18], [0.01, 0.025, 0.025], Paint::Dark),
];

const BISHOP: &[Part] = &[
    lathe(BASE, Paint::Accent),
    lathe(&[(0.10, 0.31), (0.56, 0.17), (0.62, 0.14)], Paint::Body),
    lathe(&[(0.62, 0.08), (0.68, 0.13), (0.76, 0.11)], Paint::Body),
    lathe(&[(0.74, 0.0), (0.74, 0.13), (0.92, 0.12), (1.06, 0.0)], Paint::Accent),
    cuboid([0.0, 0.90, 0.115], [0.02, 0.10, 0.012], Paint::Dark),
    // crook
    held([0.0, 0.14, 0.0], [0.022, 0.56, 0.022], Paint::Wood),
    held([0.0, 0.72, 0.05], [0.026, 0.04, 0.08], Paint::Accent),
    held([0.0, 0.66, 0.11], [0.026, 0.07, 0.026], Paint::Accent),
];

const ROOK: &[Part] = &[
    lathe(BASE, Paint::Accent),
    lathe(&[(0.10, 0.31), (0.70, 0.26), (0.76, 0.32), (0.84, 0.32), (0.84, 0.0)], Paint::Body),
    cuboid([0.22, 0.92, 0.0], [0.07, 0.08, 0.07], Paint::Body),
    cuboid([-0.22, 0.92, 0.0], [0.07, 0.08, 0.07], Paint::Body),
    cuboid([0.0, 0.92, 0.22], [0.07, 0.08, 0.07], Paint::Body),
    cuboid([0.0, 0.92, -0.22], [0.07, 0.08, 0.07], Paint::Body),
    cuboid([0.0, 0.22, 0.30], [0.09, 0.13, 0.02], Paint::Dark),
    cuboid([0.0, 0.56, 0.285], [0.025, 0.07, 0.02], Paint::Dark),
    cuboid([0.0, 0.79, 0.0], [0.33, 0.025, 0.33], Paint::Accent),
];

const QUEEN: &[Part] = &[
    lathe(BASE, Paint::Accent),
    lathe(&[(0.10, 0.34), (0.46, 0.17), (0.80, 0.12), (0.88, 0.16)], Paint::Body),
    lathe(&[(0.88, 0.07), (0.94, 0.12), (1.02, 0.10)], Paint::Body),
    lathe(&[(0.98, 0.0), (0.98, 0.11), (1.08, 0.14), (1.08, 0.0)], Paint::Accent),
    cuboid([0.11, 1.14, 0.0], [0.02, 0.06, 0.02], Paint::Accent),
    cuboid([-0.11, 1.14, 0.0], [0.02, 0.06, 0.02], Paint::Accent),
    cuboid([0.0, 1.14, 0.11], [0.02, 0.06, 0.02], Paint::Accent),
    cuboid([0.0, 1.14, -0.11], [0.02, 0.06, 0.02], Paint::Accent),
    cuboid([0.0, 1.17, 0.0], [0.03, 0.06, 0.03], Paint::Metal),
    // wand
    held([0.0, 0.14, 0.0], [0.016, 0.24, 0.016], Paint::Accent),
    held([0.0, 0.42, 0.0], [0.055, 0.055, 0.055], Paint::Metal),
];

const KING: &[Part] = &[
    lathe(BASE, Paint::Accent),
    lathe(&[(0.10, 0.36), (0.60, 0.21), (0.90, 0.18), (0.96, 0.21)], Paint::Body),
    lathe(&[(0.96, 0.08), (1.02, 0.14), (1.10, 0.11)], Paint::Body),
    lathe(&[(1.06, 0.0), (1.06, 0.14), (1.18, 0.15), (1.18, 0.0)], Paint::Accent),
    cuboid([0.0, 1.30, 0.0], [0.028, 0.11, 0.028], Paint::Accent),
    cuboid([0.0, 1.32, 0.0], [0.08, 0.028, 0.028], Paint::Accent),
    // sword
    held([0.0, 0.46, 0.0], [0.045, 0.40, 0.016], Paint::Metal),
    held([0.0, 0.05, 0.0], [0.09, 0.022, 0.03], Paint::Accent),
    held([0.0, -0.04, 0.0], [0.022, 0.07, 0.022], Paint::Wood),
];

fn parts(p: Piece) -> &'static [Part] {
    match p {
        Piece::Pawn => PAWN,
        Piece::Knight => KNIGHT,
        Piece::Bishop => BISHOP,
        Piece::Rook => ROOK,
        Piece::Queen => QUEEN,
        Piece::King => KING,
    }
}

/// Where the held thing pivots (the right shoulder), in local space.
pub fn shoulder(p: Piece) -> Vec3 {
    match p {
        Piece::Pawn => Vec3::new(0.22, 0.42, 0.04),
        Piece::Bishop => Vec3::new(0.24, 0.48, 0.04),
        Piece::Queen => Vec3::new(0.22, 0.62, 0.04),
        Piece::King => Vec3::new(0.27, 0.58, 0.06),
        // the knight and the rook hold nothing; this is their "mouth"
        Piece::Knight => Vec3::new(0.0, 0.80, 0.38),
        Piece::Rook => Vec3::new(0.0, 0.88, 0.0),
    }
}

/// The far end of the held thing, from the shoulder, before the arm turns.
pub fn tip(p: Piece) -> Vec3 {
    match p {
        Piece::Pawn => Vec3::new(0.0, 0.80, 0.0),
        Piece::Bishop => Vec3::new(0.0, 0.76, 0.06),
        Piece::Queen => Vec3::new(0.0, 0.44, 0.0),
        Piece::King => Vec3::new(0.0, 0.74, 0.0),
        _ => Vec3::new(0.0, 0.0, 0.0),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Break {
    Whole,
    /// flying apart, seconds since it began
    Shatter(f32),
    /// melting away into light, 0 to 1
    Dissolve(f32),
    /// cut in two along the given world direction, seconds since
    Split(f32, Vec3),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// the foot, in world space
    pub pos: Vec3,
    /// turned about y: 0 faces +z
    pub yaw: f32,
    /// tipped forward (negative: backward), about the foot
    pub lean: f32,
    /// tipped sideways
    pub roll: f32,
    pub lift: f32,
    /// 1 normal, less is squashed down (and fattened)
    pub squash: f32,
    /// the held thing swung forward from upright, radians
    pub arm: f32,
    /// glow toward white, 0 to 1
    pub flash: f32,
    /// size, 1 normal
    pub size: f32,
    pub broken: Break,
}

impl Pose {
    pub fn at(pos: Vec3, yaw: f32) -> Pose {
        Pose { pos, yaw, lean: 0.0, roll: 0.0, lift: 0.0, squash: 1.0, arm: 0.0, flash: 0.0, size: 1.0, broken: Break::Whole }
    }

    fn rotation(&self) -> Mat3 {
        Mat3::rot_z(self.roll).then(Mat3::rot_x(self.lean)).then(Mat3::rot_y(self.yaw))
    }

    /// A local point to world space (the held thing first turned by `arm`).
    pub fn world(&self, local: Vec3) -> Vec3 {
        let s = SCALE * self.size;
        let fat = 1.0 / self.squash.max(0.2).sqrt();
        let q = Vec3::new(local.x * s * fat, local.y * s * self.squash, local.z * s * fat);
        self.pos.add(Vec3::new(0.0, self.lift, 0.0)).add(self.rotation().mul(q))
    }

    /// Where the held thing's tip is, in world space.
    pub fn tip_world(&self, p: Piece) -> Vec3 {
        let sh = shoulder(p);
        self.world(sh.add(Mat3::rot_x(self.arm).mul(tip(p))))
    }

    pub fn centre(&self) -> Vec3 {
        self.world(Vec3::new(0.0, 0.45, 0.0))
    }
}

/// A finished triangle in world space with its colour.
#[derive(Clone, Copy, Debug)]
pub struct WTri {
    pub v: [Vec3; 3],
    pub rgb: [f32; 3],
    /// lit by the light (false: glows at its own colour)
    pub lit: bool,
}

fn hash01(i: usize, salt: u32) -> f32 {
    let mut x = (i as u32).wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

const SEG: usize = 8;

/// The local triangles of a piece: (corners, paint, held).
fn local_tris(p: Piece) -> Vec<([Vec3; 3], Paint, bool)> {
    let mut out = Vec::new();
    for part in parts(p) {
        match part.shape {
            Shape::Lathe(prof) => {
                let ring = |h: f32, r: f32, i: usize| {
                    let a = i as f32 / SEG as f32 * TAU + 0.2;
                    Vec3::new(a.cos() * r, h, a.sin() * r)
                };
                for k in 0..prof.len() - 1 {
                    let ((h0, r0), (h1, r1)) = (prof[k], prof[k + 1]);
                    for i in 0..SEG {
                        let (a, b, c, d) = (ring(h0, r0, i), ring(h0, r0, i + 1), ring(h1, r1, i + 1), ring(h1, r1, i));
                        if r0 > 0.0 {
                            out.push(([a, b, c], part.paint, false));
                        }
                        if r1 > 0.0 {
                            out.push(([a, c, d], part.paint, false));
                        }
                    }
                }
            }
            Shape::Cuboid(c, h) => {
                // held parts are stored relative to the shoulder
                let corner = |i: usize| {
                    let sx = if i & 1 == 0 { -1.0 } else { 1.0 };
                    let sy = if i & 2 == 0 { -1.0 } else { 1.0 };
                    let sz = if i & 4 == 0 { -1.0 } else { 1.0 };
                    Vec3::new(c[0] + sx * h[0], c[1] + sy * h[1], c[2] + sz * h[2])
                };
                for q in [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]] {
                    let [a, b, cc, d] = q.map(corner);
                    out.push(([a, b, cc], part.paint, part.held));
                    out.push(([a, cc, d], part.paint, part.held));
                }
            }
        }
    }
    out
}

fn mix(a: [f32; 3], b: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k]
}

/// A posed piece as world triangles.
pub fn build(p: Piece, c: Color, pose: &Pose, out: &mut Vec<WTri>) {
    let arm = Mat3::rot_x(pose.arm);
    let sh = shoulder(p);
    let centre = pose.centre();
    for (i, (v, paint_of, is_held)) in local_tris(p).into_iter().enumerate() {
        let mut w = v.map(|q| if is_held { pose.world(sh.add(arm.mul(q))) } else { pose.world(q) });
        let mut rgb = mix(paint(paint_of, c), [255.0, 250.0, 235.0], pose.flash.clamp(0.0, 1.0));
        let mut lit = pose.flash < 0.7;
        match pose.broken {
            Break::Whole => {}
            Break::Shatter(t) => {
                let k = (1.0 - t / 0.9).max(0.0);
                if k <= 0.0 {
                    continue;
                }
                let g = w[0].add(w[1]).add(w[2]).scale(1.0 / 3.0);
                let jitter = Vec3::new(hash01(i, 1) - 0.5, hash01(i, 2) * 0.8, hash01(i, 3) - 0.5);
                let dir = g.sub(centre).norm().add(jitter).norm();
                let speed = 1.2 + hash01(i, 4) * 1.6;
                let mut off = dir.scale(speed * t);
                off.y += 1.5 * t - 4.0 * t * t;
                let ng = g.add(off);
                let ng = Vec3::new(ng.x, ng.y.max(0.02), ng.z);
                w = w.map(|q| ng.add(q.sub(g).scale(k)));
                rgb = mix(rgb, [255.0, 200.0, 120.0], (1.0 - k) * 0.5);
            }
            Break::Dissolve(d) => {
                if hash01(i, 7) < d {
                    continue;
                }
                rgb = mix(rgb, [190.0, 230.0, 255.0], d.min(1.0));
                lit = d < 0.3;
            }
            Break::Split(t, axis) => {
                // the halves part and fall away from each other, tops first
                let g = w[0].add(w[1]).add(w[2]).scale(1.0 / 3.0);
                let side = if g.sub(pose.pos).dot(axis) >= 0.0 { 1.0 } else { -1.0 };
                let k = (1.0 - (t - 0.6).max(0.0) / 0.45).max(0.0);
                if k <= 0.0 {
                    continue;
                }
                let part = (t / 0.5).min(1.0);
                w = w.map(|q| {
                    let h = (q.y - pose.pos.y).max(0.0);
                    let out = axis.scale(side * (0.12 * part + 0.9 * h * part * part));
                    let drop = Vec3::new(0.0, -0.55 * h * part * part, 0.0);
                    let q = q.add(out).add(drop);
                    Vec3::new(q.x, q.y.max(0.01), q.z)
                });
                if k < 1.0 {
                    let g = w[0].add(w[1]).add(w[2]).scale(1.0 / 3.0);
                    w = w.map(|q| g.add(q.sub(g).scale(k)));
                }
            }
        }
        out.push(WTri { v: w, rgb, lit });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cozy_chess::Piece;

    #[test]
    fn every_piece_stands_on_its_square() {
        for p in Piece::ALL {
            let mut v = Vec::new();
            build(p, Color::White, &Pose::at(Vec3::new(0.0, 0.0, 0.0), 0.0), &mut v);
            assert!(!v.is_empty());
            let min_y = v.iter().flat_map(|t| t.v).map(|q| q.y).fold(f32::MAX, f32::min);
            let max_r = v.iter().flat_map(|t| t.v).map(|q| q.x.abs().max(q.z.abs())).fold(0.0, f32::max);
            assert!(min_y > -0.06, "{p:?} goes below the board: {min_y}");
            assert!(max_r < 0.5, "{p:?} spills off its square: {max_r}");
        }
    }

    #[test]
    fn a_shatter_ends_in_nothing() {
        let mut v = Vec::new();
        let mut pose = Pose::at(Vec3::new(0.0, 0.0, 0.0), 0.0);
        pose.broken = Break::Shatter(1.0);
        build(Piece::Queen, Color::Black, &pose, &mut v);
        assert!(v.is_empty());
    }
}
