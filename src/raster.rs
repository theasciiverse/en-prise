//! (Copied from Ratty's Cube, games/rattys-cube.) A small software
//! rasteriser: flat-coloured triangles in, half-block
//! terminal cells out.
//!
//! Everything is drawn at `SS` x `SS` samples per output pixel with a depth
//! per sample, then averaged down, so edges come out smooth. An output pixel
//! is half a terminal cell (the upper or lower half of a `▀`), so a cell
//! holds two freely coloured pixels. A pick buffer says which square each
//! sample shows, for the mouse.

#![allow(dead_code)] // copied whole from Ratty's Cube

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Samples per output pixel, across and down.
pub const SS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Vec3 {
        Vec3 { x, y, z }
    }
    pub fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    pub fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    pub fn scale(self, k: f32) -> Vec3 {
        Vec3::new(self.x * k, self.y * k, self.z * k)
    }
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn norm(self) -> Vec3 {
        let l = self.len();
        if l > 0.0 { self.scale(1.0 / l) } else { self }
    }
}

/// A rotation (or any linear map), rows times column vectors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3(pub [[f32; 3]; 3]);

impl Mat3 {
    pub const IDENTITY: Mat3 = Mat3([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

    pub fn rot_x(a: f32) -> Mat3 {
        let (s, c) = a.sin_cos();
        Mat3([[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]])
    }
    pub fn rot_y(a: f32) -> Mat3 {
        let (s, c) = a.sin_cos();
        Mat3([[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]])
    }
    pub fn rot_z(a: f32) -> Mat3 {
        let (s, c) = a.sin_cos();
        Mat3([[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]])
    }
    pub fn mul(self, v: Vec3) -> Vec3 {
        let m = self.0;
        Vec3::new(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
        )
    }
    pub fn then(self, o: Mat3) -> Mat3 {
        // o * self: apply self first
        let (a, b) = (o.0, self.0);
        let mut r = [[0.0; 3]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
            }
        }
        Mat3(r)
    }
}

/// A triangle in view space (the camera at the origin looking down -z is
/// handled by the projection), already lit.
#[derive(Clone, Copy, Debug)]
pub struct Tri {
    pub v: [Vec3; 3],
    pub rgb: [f32; 3],
    /// what the mouse finds here: 0 nothing, otherwise square + 1
    pub pick: u16,
}

pub struct Projection {
    /// camera distance from the cube's centre
    pub dist: f32,
    /// focal length in output pixels
    pub focal: f32,
    /// centre of the picture in output pixels
    pub cx: f32,
    pub cy: f32,
    /// output pixels are this much wider than tall (corrects the cell shape)
    pub y_scale: f32,
}

impl Projection {
    /// View-space point to output pixels and inverse depth.
    pub fn project(&self, p: Vec3) -> (f32, f32, f32) {
        let d = self.dist - p.z;
        let inv = 1.0 / d.max(0.01);
        (self.cx + self.focal * p.x * inv, self.cy - self.focal * p.y * inv * self.y_scale, inv)
    }

    /// A view-space direction at a point, as an output-pixel direction
    /// (for working out which way a drag turns a face).
    pub fn screen_dir(&self, at: Vec3, dir: Vec3) -> (f32, f32) {
        let (x0, y0, _) = self.project(at);
        let (x1, y1, _) = self.project(at.add(dir.scale(0.05)));
        (x1 - x0, y1 - y0)
    }
}

pub struct Canvas {
    /// output pixels
    pub w: usize,
    pub h: usize,
    sw: usize,
    sh: usize,
    rgb: Vec<[f32; 3]>,
    depth: Vec<f32>,
    pick: Vec<u16>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        let (sw, sh) = (w * SS, h * SS);
        Canvas { w, h, sw, sh, rgb: vec![[0.0; 3]; sw * sh], depth: vec![0.0; sw * sh], pick: vec![0; sw * sh] }
    }

    /// A vertical gradient from `top` to `bottom`.
    pub fn clear(&mut self, top: [f32; 3], bottom: [f32; 3]) {
        for y in 0..self.sh {
            let k = y as f32 / self.sh.max(1) as f32;
            let c = [top[0] + (bottom[0] - top[0]) * k, top[1] + (bottom[1] - top[1]) * k, top[2] + (bottom[2] - top[2]) * k];
            let row = y * self.sw;
            self.rgb[row..row + self.sw].fill(c);
        }
        self.depth.fill(0.0);
        self.pick.fill(0);
    }

    pub fn draw(&mut self, proj: &Projection, t: &Tri) {
        let s = SS as f32;
        let p: Vec<(f32, f32, f32)> = t
            .v
            .iter()
            .map(|&v| {
                let (x, y, inv) = proj.project(v);
                (x * s, y * s, inv)
            })
            .collect();
        let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
        if area.abs() < 1e-6 {
            return;
        }
        let min_x = p.iter().map(|q| q.0).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let max_x = (p.iter().map(|q| q.0).fold(f32::MIN, f32::max).ceil() as isize).min(self.sw as isize - 1);
        let min_y = p.iter().map(|q| q.1).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let max_y = (p.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil() as isize).min(self.sh as isize - 1);
        if max_x < 0 || max_y < 0 {
            return;
        }
        let inv_area = 1.0 / area;
        for y in min_y..=max_y as usize {
            let py = y as f32 + 0.5;
            for x in min_x..=max_x as usize {
                let px = x as f32 + 0.5;
                let w0 = ((p[1].0 - px) * (p[2].1 - py) - (p[2].0 - px) * (p[1].1 - py)) * inv_area;
                let w1 = ((p[2].0 - px) * (p[0].1 - py) - (p[0].0 - px) * (p[2].1 - py)) * inv_area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let inv = w0 * p[0].2 + w1 * p[1].2 + w2 * p[2].2;
                let i = y * self.sw + x;
                if inv > self.depth[i] {
                    self.depth[i] = inv;
                    self.rgb[i] = t.rgb;
                    self.pick[i] = t.pick;
                }
            }
        }
    }

    /// What the mouse finds at output pixel (x, y).
    pub fn pick_at(&self, x: usize, y: usize) -> u16 {
        let (sx, sy) = (x * SS + SS / 2, y * SS + SS / 2);
        if sx < self.sw && sy < self.sh { self.pick[sy * self.sw + sx] } else { 0 }
    }

    fn pixel(&self, x: usize, y: usize) -> Color {
        let mut acc = [0.0f32; 3];
        for dy in 0..SS {
            let row = (y * SS + dy) * self.sw + x * SS;
            for c in &self.rgb[row..row + SS] {
                acc[0] += c[0];
                acc[1] += c[1];
                acc[2] += c[2];
            }
        }
        let n = (SS * SS) as f32;
        Color::Rgb((acc[0] / n).clamp(0.0, 255.0) as u8, (acc[1] / n).clamp(0.0, 255.0) as u8, (acc[2] / n).clamp(0.0, 255.0) as u8)
    }

    /// Into `area`, two output pixels per cell.
    pub fn blit(&self, buf: &mut Buffer, area: Rect) {
        for cy in 0..area.height.min((self.h / 2) as u16) {
            for cx in 0..area.width.min(self.w as u16) {
                let top = self.pixel(cx as usize, cy as usize * 2);
                let bottom = self.pixel(cx as usize, cy as usize * 2 + 1);
                if let Some(cell) = buf.cell_mut((area.x + cx, area.y + cy)) {
                    cell.set_symbol("▀").set_fg(top).set_bg(bottom);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearer_wins_whichever_is_drawn_first() {
        let proj = Projection { dist: 10.0, focal: 20.0, cx: 10.0, cy: 10.0, y_scale: 1.0 };
        let quad = |z: f32, pick: u16| {
            [
                Tri { v: [Vec3::new(-2.0, -2.0, z), Vec3::new(2.0, -2.0, z), Vec3::new(2.0, 2.0, z)], rgb: [255.0, 0.0, 0.0], pick },
                Tri { v: [Vec3::new(-2.0, -2.0, z), Vec3::new(2.0, 2.0, z), Vec3::new(-2.0, 2.0, z)], rgb: [255.0, 0.0, 0.0], pick },
            ]
        };
        for order in [[1u16, 2], [2, 1]] {
            let mut c = Canvas::new(20, 20);
            c.clear([0.0; 3], [0.0; 3]);
            for &k in &order {
                for t in quad(if k == 1 { 1.0 } else { 0.0 }, k) {
                    c.draw(&proj, &t);
                }
            }
            assert_eq!(c.pick_at(10, 10), 1, "order {order:?}");
        }
    }
}
