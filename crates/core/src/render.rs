//! What the page draws: one 3×4 matrix, a colour and a glow per shape,
//! grouped by mesh and by pass so each (mesh, pass) is one instanced draw.
//!
//! Long Time Comin's contract, without its facades and see-through cut.
//! Solid shapes are lit in bands like the TikZ art's flat fills; the ink pass
//! draws each inked shape again a little larger with its front faces culled,
//! which leaves a plum outline round it (the inverted hull); the glow pass
//! adds a soft halo round thrown things. The core orders the draws solid,
//! ink, glow, and the page sets the GL state each pass needs.

use crate::content::Rgb;
use crate::fx::{self, ONE};
use crate::geom::{Affine, Mat4, V3};
use crate::mesh::Mesh;

pub const INSTANCE_VALUES: usize = 16;
pub const DRAW_VALUES: usize = 5;
pub const BYTES_PER_VALUE: usize = 4;
pub const STATIC_BUFFER: u32 = 0;
pub const DYNAMIC_BUFFER: u32 = 1;

pub const PASS_SOLID: u32 = 0;
pub const PASS_INK: u32 = 1;
pub const PASS_GLOW: u32 = 2;
pub const PASSES: usize = 3;

pub const NEAR: i32 = ONE / 10;
pub const FAR: i32 = fx::int(400);

/// The ink line is this thick on a small shape, and grows with a shape's
/// size so a tree far off still has one.
const INK_MIN: i32 = ONE * 2 / 100;
const INK_PER_METRE: i32 = ONE / 90;
const INK_MAX: i32 = ONE / 5;

#[derive(Debug, Clone)]
pub struct Batch {
    lists: Vec<Vec<i32>>,
    ink: Rgb,
}

fn slot(mesh: Mesh, pass: u32) -> usize {
    pass as usize * Mesh::ALL.len() + mesh as usize
}

impl Batch {
    pub fn new(ink: Rgb) -> Batch {
        Batch {
            lists: vec![Vec::new(); PASSES * Mesh::ALL.len()],
            ink,
        }
    }

    pub fn push(&mut self, mesh: Mesh, a: &Affine, rgb: Rgb, glow: i32, pass: u32) {
        self.lists[slot(mesh, pass)].extend_from_slice(&[
            a.c0.x, a.c0.y, a.c0.z, a.t.x, a.c1.x, a.c1.y, a.c1.z, a.t.y, a.c2.x, a.c2.y, a.c2.z, a.t.z,
            rgb[0], rgb[1], rgb[2], glow,
        ]);
    }

    /// A solid shape with its outline.
    pub fn inked(&mut self, mesh: Mesh, a: &Affine, rgb: Rgb) {
        self.push(mesh, a, rgb, 0, PASS_SOLID);
        let ink = self.ink;
        self.push(mesh, &inflate(a), ink, 0, PASS_INK);
    }

    /// A solid shape with no outline (the ground, far hills).
    pub fn plain(&mut self, mesh: Mesh, a: &Affine, rgb: Rgb) {
        self.push(mesh, a, rgb, 0, PASS_SOLID);
    }

    /// A shape lit from inside: `glow` mixes it toward its own bright colour.
    pub fn lit(&mut self, mesh: Mesh, a: &Affine, rgb: Rgb, glow: i32) {
        self.push(mesh, a, rgb, glow, PASS_SOLID);
        let ink = self.ink;
        self.push(mesh, &inflate(a), ink, 0, PASS_INK);
    }

    pub fn halo(&mut self, mesh: Mesh, a: &Affine, rgb: Rgb, strength: i32) {
        self.push(mesh, a, rgb, strength, PASS_GLOW);
    }

    pub fn count(&self) -> usize {
        self.lists.iter().map(|m| m.len() / INSTANCE_VALUES).sum()
    }

    /// The instance values, and one draw per (mesh, pass) that has any,
    /// in pass order.
    pub fn finish(self, buffer: u32) -> (Vec<i32>, Vec<Vec<u32>>) {
        let mut values = Vec::new();
        let mut draws = vec![Vec::new(); PASSES];
        for pass in 0..PASSES as u32 {
            for mesh in Mesh::ALL {
                let v = &self.lists[slot(mesh, pass)];
                if v.is_empty() {
                    continue;
                }
                let offset = (values.len() * BYTES_PER_VALUE) as u32;
                let count = (v.len() / INSTANCE_VALUES) as u32;
                draws[pass as usize].extend_from_slice(&[buffer, mesh as u32, offset, count, pass]);
                values.extend_from_slice(v);
            }
        }
        (values, draws)
    }
}

/// The same shape grown by the ink's thickness on every side.
pub fn inflate(a: &Affine) -> Affine {
    let biggest = [a.c0.len(), a.c1.len(), a.c2.len()]
        .into_iter()
        .max()
        .unwrap_or(0);
    inflate_by(a, (INK_MIN + fx::mul(biggest, INK_PER_METRE)).min(INK_MAX))
}

/// The same shape grown by `t` on every side.
pub fn inflate_by(a: &Affine, t: i32) -> Affine {
    let lens = [a.c0.len(), a.c1.len(), a.c2.len()];
    let grow = |c: V3, l: i32| {
        if l == 0 {
            c
        } else {
            c.scale(fx::div(l + 2 * t, l))
        }
    };
    Affine {
        c0: grow(a.c0, lens[0]),
        c1: grow(a.c1, lens[1]),
        c2: grow(a.c2, lens[2]),
        t: a.t,
    }
}

pub fn boxed(at: V3, size: V3) -> Affine {
    Affine::translate(at).then(&Affine::scale(size))
}

pub fn shade(c: Rgb, pct: i32) -> Rgb {
    c.map(|v| (v * pct / 100).min(ONE))
}

pub fn mix(a: Rgb, b: Rgb, t: i32) -> Rgb {
    [
        fx::lerp(a[0], b[0], t),
        fx::lerp(a[1], b[1], t),
        fx::lerp(a[2], b[2], t),
    ]
}

/// A camera: where it is, where it looks, and its field of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Camera {
    pub eye: V3,
    pub fwd: V3,
    pub fov: i32,
}

impl Camera {
    pub fn looking_at(eye: V3, target: V3, fov: i32) -> Camera {
        Camera {
            eye,
            fwd: target.sub(eye).norm(),
            fov,
        }
    }

    pub fn matrix(&self, aspect: i32) -> Mat4 {
        Mat4::perspective(self.fov, aspect, NEAR, FAR).mul(&Mat4::look(self.eye, self.fwd, V3::UP))
    }

    /// The view values the page uploads: the matrix, then the eye.
    pub fn values(&self, aspect: i32) -> Vec<i32> {
        let mut v = self.matrix(aspect).0.to_vec();
        v.extend([self.eye.x, self.eye.y, self.eye.z]);
        v
    }

    /// Where a world point lands on screen, as percentages (Q16) from the
    /// left and the top; `None` behind the camera.
    pub fn screen(&self, aspect: i32, p: V3) -> Option<(i32, i32)> {
        let [x, y, _, w] = self.matrix(aspect).project(p);
        if w <= ONE / 100 {
            return None;
        }
        let nx = fx::div(x, w);
        let ny = fx::div(y, w);
        Some(((nx + ONE) * 50, (ONE - ny) * 50))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{deg, int};
    use crate::geom::v3;

    #[test]
    fn an_inked_shape_is_drawn_twice_and_its_outline_is_larger() {
        let mut b = Batch::new([0; 3]);
        let a = boxed(V3::ZERO, v3(int(2), int(1), int(4)));
        b.inked(Mesh::Cube, &a, [ONE; 3]);
        assert_eq!(b.count(), 2);
        let big = inflate(&a);
        assert!(big.c0.len() > a.c0.len() && big.c2.len() > a.c2.len());
        let d0 = big.c0.len() - a.c0.len();
        let d2 = big.c2.len() - a.c2.len();
        assert!((d0 - d2).abs() < 8, "{d0} {d2}");
    }

    #[test]
    fn draws_come_in_pass_order() {
        let mut b = Batch::new([0; 3]);
        let a = boxed(V3::ZERO, v3(ONE, ONE, ONE));
        b.halo(Mesh::Sphere, &a, [ONE; 3], ONE);
        b.inked(Mesh::Cube, &a, [ONE; 3]);
        let (_, draws) = b.finish(DYNAMIC_BUFFER);
        assert_eq!(draws[PASS_SOLID as usize][4], PASS_SOLID);
        assert_eq!(draws[PASS_INK as usize][4], PASS_INK);
        assert_eq!(draws[PASS_GLOW as usize][4], PASS_GLOW);
    }

    #[test]
    fn the_point_the_camera_looks_at_is_the_middle_of_the_screen() {
        let c = Camera::looking_at(v3(0, int(2), int(10)), v3(0, ONE, 0), deg(50));
        let p = c.eye.add(c.fwd.scale(int(20)));
        let (x, y) = c.screen(ONE, p).unwrap();
        assert!(
            (x - 50 * ONE).abs() < ONE / 4 && (y - 50 * ONE).abs() < ONE / 4,
            "{x} {y}"
        );
        assert!(c.screen(ONE, c.eye.sub(c.fwd.scale(int(5)))).is_none());
    }
}
