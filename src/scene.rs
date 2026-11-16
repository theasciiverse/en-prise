//! The board in an isometric view: the squares on a wooden frame, the
//! figures standing on them, and the effects of a fight, as lit triangles
//! for the rasteriser.
//!
//! World space: the board's middle at the origin, y up, files along +x
//! (a to h), ranks along -z (1 nearest White's camera, 8 furthest).

use crate::model::{self, Pose, WTri};
use crate::raster::{Canvas, Mat3, Projection, Tri, Vec3};
use cozy_chess::{Color, Piece, Square};
use std::f32::consts::{FRAC_PI_4, PI};

/// The true isometric angle looks down at about 35 degrees; a little
/// steeper shows the squares better between the figures.
pub const PITCH: f32 = 0.68;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub target: Vec3,
}

impl Camera {
    /// Looking from `side`'s corner, as an isometric view does.
    pub fn home(side: Color) -> Camera {
        Camera { yaw: home_yaw(side), pitch: PITCH, zoom: 1.0, target: Vec3::new(0.0, 0.3, 0.0) }
    }
}

pub fn home_yaw(side: Color) -> f32 {
    if side == Color::White { -FRAC_PI_4 } else { PI - FRAC_PI_4 }
}

pub fn square_pos(sq: Square) -> Vec3 {
    Vec3::new(sq.file() as usize as f32 - 3.5, 0.0, 3.5 - sq.rank() as usize as f32)
}

/// Pieces face the other side's back rank.
pub fn facing(c: Color) -> f32 {
    if c == Color::White { PI } else { 0.0 }
}

/// The yaw that faces from `from` toward `to`.
pub fn yaw_toward(from: Vec3, to: Vec3) -> f32 {
    let d = to.sub(from);
    d.x.atan2(d.z)
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Mark {
    #[default]
    None,
    Selected,
    Target,
    LastMove,
    Check,
    Cursor,
    /// a piece of yours that can be taken for profit; the pulse is in `Scene`
    EnPrise,
}

const LIGHT: [f32; 3] = [222.0, 208.0, 176.0];
const DARK: [f32; 3] = [112.0, 86.0, 64.0];
const FRAME: [f32; 3] = [58.0, 38.0, 26.0];

pub fn square_rgb(sq: Square, mark: Mark, pulse: f32) -> [f32; 3] {
    let dark = (sq.file() as usize + sq.rank() as usize) % 2 == 0;
    let base = if dark { DARK } else { LIGHT };
    let tint = |c: [f32; 3], k: f32| [base[0] + (c[0] - base[0]) * k, base[1] + (c[1] - base[1]) * k, base[2] + (c[2] - base[2]) * k];
    match mark {
        Mark::None => base,
        Mark::Selected => tint([255.0, 176.0, 0.0], 0.8),
        Mark::Target => tint([70.0, 210.0, 110.0], 0.55),
        Mark::LastMove => tint([240.0, 220.0, 90.0], 0.45),
        Mark::Check => tint([230.0, 30.0, 30.0], 0.85),
        Mark::Cursor => tint([80.0, 170.0, 255.0], 0.65),
        Mark::EnPrise => tint([235.0, 40.0, 60.0], 0.25 + 0.45 * pulse),
    }
}

/// Things a fight draws that are not figures.
#[derive(Clone, Copy, Debug)]
pub enum Fx {
    /// a glowing dot facing the camera
    Spark { pos: Vec3, size: f32, rgb: [f32; 3] },
    /// a glowing band from a to b
    Beam { a: Vec3, b: Vec3, width: f32, rgb: [f32; 3] },
    /// a lit cube (a stone in flight)
    Stone { pos: Vec3, r: f32, rgb: [f32; 3] },
}

/// A figure to draw: what it is, how it stands, and what a click on it picks.
#[derive(Clone, Copy, Debug)]
pub struct Figure {
    pub piece: Piece,
    pub colour: Color,
    pub pose: Pose,
    /// 0 nothing, else square index + 1
    pub pick: u16,
}

pub struct Scene {
    pub proj: Projection,
    view: Mat3,
    target: Vec3,
    pub pulse: f32,
}

impl Scene {
    pub fn new(cam: &Camera, w: usize, h: usize, y_scale: f32) -> Scene {
        let dist = 30.0;
        // world units to output pixels: the board's diamond across, its
        // height with the figures down
        let ppu = (w as f32 * 0.5 / 6.3).min(h as f32 * 0.5 / (y_scale * 4.3));
        let focal = ppu * dist * cam.zoom;
        let view = Mat3::rot_y(cam.yaw).then(Mat3::rot_x(cam.pitch));
        Scene { proj: Projection { dist, focal, cx: w as f32 / 2.0, cy: h as f32 / 2.0, y_scale }, view, target: cam.target, pulse: 0.0 }
    }

    fn to_view(&self, p: Vec3) -> Vec3 {
        self.view.mul(p.sub(self.target))
    }

    fn lit(&self, w: &[Vec3; 3], rgb: [f32; 3]) -> [f32; 3] {
        let n = w[1].sub(w[0]).cross(w[2].sub(w[0])).norm();
        let l = Vec3::new(-0.45, 0.82, 0.36).norm();
        let k = 0.40 + 0.66 * n.dot(l).abs();
        [rgb[0] * k, rgb[1] * k, rgb[2] * k]
    }

    fn push(&self, out: &mut Vec<Tri>, w: [Vec3; 3], rgb: [f32; 3], lit: bool, pick: u16) {
        let rgb = if lit { self.lit(&w, rgb) } else { rgb };
        out.push(Tri { v: w.map(|p| self.to_view(p)), rgb, pick });
    }

    fn quad(&self, out: &mut Vec<Tri>, q: [Vec3; 4], rgb: [f32; 3], pick: u16) {
        self.push(out, [q[0], q[1], q[2]], rgb, true, pick);
        self.push(out, [q[0], q[2], q[3]], rgb, true, pick);
    }

    fn board(&self, out: &mut Vec<Tri>, marks: &[Mark; 64]) {
        // the frame: a slab under the squares with a rim round them
        let (r, top, bottom) = (4.45, -0.002, -0.32);
        let c = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
        self.quad(out, [c(-r, top, -r), c(r, top, -r), c(r, top, r), c(-r, top, r)], FRAME, 0);
        for (a, b) in [((-r, -r), (r, -r)), ((r, -r), (r, r)), ((r, r), (-r, r)), ((-r, r), (-r, -r))] {
            self.quad(out, [c(a.0, top, a.1), c(b.0, top, b.1), c(b.0, bottom, b.1), c(a.0, bottom, a.1)], [FRAME[0] * 0.8, FRAME[1] * 0.8, FRAME[2] * 0.8], 0);
        }
        for sq in Square::ALL {
            let p = square_pos(sq);
            let rgb = square_rgb(sq, marks[sq as usize], self.pulse);
            let (x0, x1, z0, z1) = (p.x - 0.5, p.x + 0.5, p.z - 0.5, p.z + 0.5);
            self.quad(out, [c(x0, 0.0, z0), c(x1, 0.0, z0), c(x1, 0.0, z1), c(x0, 0.0, z1)], rgb, sq as u16 + 1);
        }
    }

    fn fx(&self, out: &mut Vec<Tri>, f: &Fx) {
        match *f {
            Fx::Spark { pos, size, rgb } => {
                let v = self.to_view(pos);
                let (dx, dy) = (Vec3::new(size, 0.0, 0.0), Vec3::new(0.0, size, 0.0));
                let q = [v.sub(dx).sub(dy), v.add(dx).sub(dy), v.add(dx).add(dy), v.sub(dx).add(dy)];
                out.push(Tri { v: [q[0], q[1], q[2]], rgb, pick: 0 });
                out.push(Tri { v: [q[0], q[2], q[3]], rgb, pick: 0 });
            }
            Fx::Beam { a, b, width, rgb } => {
                let (va, vb) = (self.to_view(a), self.to_view(b));
                let d = vb.sub(va);
                let side = d.cross(Vec3::new(0.0, 0.0, 1.0)).norm().scale(width);
                let q = [va.sub(side), vb.sub(side), vb.add(side), va.add(side)];
                out.push(Tri { v: [q[0], q[1], q[2]], rgb, pick: 0 });
                out.push(Tri { v: [q[0], q[2], q[3]], rgb, pick: 0 });
            }
            Fx::Stone { pos, r, rgb } => {
                let corner = |i: usize| {
                    pos.add(Vec3::new(if i & 1 == 0 { -r } else { r }, if i & 2 == 0 { -r } else { r }, if i & 4 == 0 { -r } else { r }))
                };
                for q in [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]] {
                    self.quad(out, q.map(corner), rgb, 0);
                }
            }
        }
    }

    pub fn draw(&self, canvas: &mut Canvas, figures: &[Figure], marks: &[Mark; 64], fx: &[Fx]) {
        canvas.clear([22.0, 20.0, 30.0], [6.0, 6.0, 10.0]);
        let mut tris = Vec::with_capacity(6000);
        self.board(&mut tris, marks);
        let mut w: Vec<WTri> = Vec::with_capacity(400);
        for f in figures {
            w.clear();
            model::build(f.piece, f.colour, &f.pose, &mut w);
            for t in &w {
                self.push(&mut tris, t.v, t.rgb, t.lit, f.pick);
            }
            // a shadow under a figure in the air
            if f.pose.lift > 0.05 {
                let p = f.pose.pos;
                let r = 0.3 / (1.0 + f.pose.lift);
                let s = [Vec3::new(p.x - r, 0.003, p.z - r), Vec3::new(p.x + r, 0.003, p.z - r), Vec3::new(p.x + r, 0.003, p.z + r), Vec3::new(p.x - r, 0.003, p.z + r)];
                self.push(&mut tris, [s[0], s[1], s[2]], [20.0, 16.0, 14.0], false, 0);
                self.push(&mut tris, [s[0], s[2], s[3]], [20.0, 16.0, 14.0], false, 0);
            }
        }
        for f in fx {
            self.fx(&mut tris, f);
        }
        for t in &tris {
            canvas.draw(&self.proj, t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_sits_nearest_the_camera_from_white_s_side() {
        let sc = Scene::new(&Camera::home(Color::White), 160, 120, 1.0);
        let near = sc.to_view(square_pos(Square::A1)).z.max(sc.to_view(square_pos(Square::H1)).z);
        let far = sc.to_view(square_pos(Square::A8)).z.min(sc.to_view(square_pos(Square::H8)).z);
        assert!(near > far, "rank 1 is nearer: {near} > {far}");
        // and e1 lies lower on the screen than e8
        let (_, y1, _) = sc.proj.project(sc.to_view(square_pos(Square::E1)));
        let (_, y8, _) = sc.proj.project(sc.to_view(square_pos(Square::E8)));
        assert!(y1 > y8);
    }

    #[test]
    fn a_click_finds_the_square_drawn_there() {
        let sc = Scene::new(&Camera::home(Color::White), 160, 120, 1.0);
        let mut c = Canvas::new(160, 120);
        sc.draw(&mut c, &[], &[Mark::None; 64], &[]);
        for sq in [Square::A1, Square::E4, Square::H8, Square::D5] {
            let (x, y, _) = sc.proj.project(sc.to_view(square_pos(sq)));
            assert_eq!(c.pick_at(x as usize, y as usize), sq as u16 + 1, "{sq}");
        }
    }

    #[test]
    fn the_whole_board_fits() {
        for side in [Color::White, Color::Black] {
            let sc = Scene::new(&Camera::home(side), 76, 56, 1.0);
            for sq in [Square::A1, Square::A8, Square::H1, Square::H8] {
                let (x, y, _) = sc.proj.project(sc.to_view(square_pos(sq)));
                assert!(x > 0.0 && x < 76.0 && y > 0.0 && y < 56.0, "{sq} at {x},{y}");
            }
        }
    }
}
