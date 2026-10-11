//! The 3D picture: a clearing in the woods, and creatures assembled from the
//! parts they carry. A creature is a body (a sphere sized by its mass) with
//! eyes, and each part's model placed at its socket; two parts in one socket
//! stand side by side, and a creature turns its parts toward its foe.
//!
//! Local frame of a creature: +x is forward, +y up, +z its left side. Model
//! numbers in parts.json are hundredths of the body's radius.

use crate::combat::{Fight, Fighter, Shot, FOE, PLAYER};
use crate::content::{ActionKind, Content, Rgb};
use crate::creature::Creature;
use crate::fx::{self, deg, int, ratio, ONE};
use crate::geom::{v3, Affine, V3};
use crate::mesh::Mesh;
use crate::render::{boxed, mix, shade, Batch};
use crate::rng::Rng;

/// Where the two fighters stand, metres from the middle of the clearing.
pub const STAND_X: i32 = int(2) + ONE * 3 / 10;
/// How long a thrown thing takes to cross, in ticks.
pub const SHOT_TICKS: i32 = 20;
const PULSE_TICKS: i32 = 14;
const LUNGE_TICKS: i32 = 18;
const HURT_TICKS: i32 = 10;
const HOP_TICKS: i32 = 18;
const FALL_TICKS: i32 = 40;

/// The body's radius in metres: bigger with more mass.
pub fn radius(mass: i32) -> i32 {
    ratio(55, 100) + mass * ratio(7, 100)
}

fn h(v: i32) -> i32 {
    v * ONE / 100
}

fn socket(name: &str) -> [i32; 3] {
    match name {
        "mouth" => [92, -10, 0],
        "head" => [35, 78, 0],
        "back" => [-12, 82, 0],
        "legs" => [0, -84, 0],
        "tail" => [-95, 6, 0],
        _ => [30, -18, 82],
    }
}

/// Where the k-th part in a socket moves to, so two parts there both show.
fn socket_shift(name: &str, k: i32) -> [i32; 3] {
    match name {
        "mouth" => [0, -18 * k, 0],
        "head" => [-26 * k, -6 * k, 0],
        "back" => [-28 * k, -10 * k, 0],
        "legs" => [-22 * k, 4 * k, 0],
        "tail" => [0, 26 * k, 0],
        // the side socket fills the right side, then the left, then further back
        _ => [-34 * (k / 2), 0, 0],
    }
}

/// How a creature stands this frame: each a Q16 fraction from 0 to 1.
#[derive(Debug, Clone, Default)]
pub struct Pose {
    pub bob: i32,
    pub lunge: i32,
    pub hurt: i32,
    pub hop: i32,
    pub fallen: i32,
    pub pulses: Vec<i32>,
}

fn fade(since: i32, span: i32) -> i32 {
    if since < 0 || since >= span {
        0
    } else {
        ONE - since * ONE / span
    }
}

/// The pose of a fighter at the fight's current tick.
pub fn pose_of(f: &Fighter, tick: i32, ended_at: Option<i32>, lost: bool, anim: i32) -> Pose {
    let arc = |since: i32, span: i32| {
        if since < 0 || since >= span {
            0
        } else {
            fx::sin(since * fx::HALF / span)
        }
    };
    Pose {
        bob: fx::sin(anim * deg(4)) / 30,
        lunge: arc(tick - f.lunge_at, LUNGE_TICKS),
        hurt: fade(tick - f.hurt_at, HURT_TICKS),
        hop: arc(tick - f.dodge_at, HOP_TICKS),
        fallen: match (lost, ended_at) {
            (true, Some(t)) => ((tick - t) * ONE / FALL_TICKS).clamp(0, ONE),
            _ => 0,
        },
        pulses: f.fired_at.iter().map(|&t| fade(tick - t, PULSE_TICKS)).collect(),
    }
}

pub fn idle_pose(c: &Creature, anim: i32) -> Pose {
    Pose {
        bob: fx::sin(anim * deg(4)) / 30,
        pulses: vec![0; c.parts.len()],
        ..Pose::default()
    }
}

fn colour_of(c: &Content, k: &Creature, name: &str, part_colour: &str) -> Rgb {
    let name = if name == "part" { part_colour } else { name };
    match name {
        "body" => c.palette.get(&k.body),
        "accent" => c.palette.get(&k.accent),
        n => c.palette.get(n),
    }
}

/// The body's centre above its feet, metres.
pub fn centre_height(k: &Creature) -> i32 {
    fx::mul(radius(k.mass), h(k.shape[1]) + ONE / 8)
}

/// The creature's transform: standing at `pos`, facing `yaw`, in a pose.
fn frame(k: &Creature, pos: V3, yaw: i32, p: &Pose) -> Affine {
    let r = radius(k.mass);
    let forward = v3(fx::cos(yaw), 0, -fx::sin(yaw));
    let lift = centre_height(k) + p.bob + fx::mul(p.hop, ONE / 2) - fx::mul(p.fallen, centre_height(k) / 2);
    let at = pos
        .add(v3(0, lift, 0))
        .add(forward.scale(fx::mul(p.lunge, ONE * 6 / 10)));
    Affine::translate(at)
        .then(&Affine::rot_y(yaw))
        .then(&Affine::rot_x(fx::mul(p.fallen, deg(90))))
        .then(&Affine::scale(v3(r, r, r)))
}

/// A point on a creature in world space: `local` in hundredths of its radius.
pub fn point_on(k: &Creature, pos: V3, yaw: i32, p: &Pose, local: [i32; 3]) -> V3 {
    frame(k, pos, yaw, p).apply(v3(h(local[0]), h(local[1]), h(local[2])))
}

fn rotation(rot: [i32; 3]) -> Affine {
    Affine::rot_y(deg(rot[1]))
        .then(&Affine::rot_x(deg(rot[0])))
        .then(&Affine::rot_z(deg(rot[2])))
}

/// Draws a creature from its body and parts.
pub fn draw_creature(b: &mut Batch, c: &Content, k: &Creature, pos: V3, yaw: i32, p: &Pose) {
    let t = frame(k, pos, yaw, p);
    let flash = c.palette.get("hit");
    let tint = |rgb: Rgb| mix(rgb, flash, p.hurt * 6 / 10);
    let [sx, sy, sz] = k.shape;
    let body = c.palette.get(&k.body);
    let accent = c.palette.get(&k.accent);
    // body, belly
    b.inked(
        Mesh::Sphere,
        &t.then(&Affine::scale(v3(h(sx * 2), h(sy * 2), h(sz * 2)))),
        tint(body),
    );
    let belly = Affine::translate(v3(h(sx * 34 / 100), h(-sy * 26 / 100), 0)).then(&Affine::scale(v3(
        h(sx * 130 / 100),
        h(sy * 110 / 100),
        h(sz * 130 / 100),
    )));
    b.inked(Mesh::Sphere, &t.then(&belly), tint(accent));
    // eyes
    for side in [1, -1] {
        let eye = v3(h(sx * 72 / 100), h(sy * 32 / 100), h(side * sz * 34 / 100));
        b.inked(
            Mesh::Sphere,
            &t.then(&boxed(eye, v3(h(34), h(38), h(34)))),
            c.palette.get("bone"),
        );
        let pupil = eye.add(v3(h(14), h(3), h(side * 3)));
        b.plain(
            Mesh::Sphere,
            &t.then(&boxed(pupil, v3(h(15), h(20), h(15)))),
            c.palette.get("ink"),
        );
    }
    // a body with nothing in its legs socket still stands on four nubs
    if !k.parts.iter().any(|&p| c.parts[p].socket == "legs") {
        for (fx_, fz) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let at = v3(h(fx_ * 38 * sx / 100), h(-86 * sy / 100), h(fz * 42 * sz / 100));
            b.inked(
                Mesh::Sphere,
                &t.then(&boxed(at, v3(h(38), h(24), h(32)))),
                tint(shade(body, 80)),
            );
        }
    }
    // parts
    let mut seen: Vec<&str> = Vec::new();
    for (i, &pi) in k.parts.iter().enumerate() {
        let part = &c.parts[pi];
        let kth = seen.iter().filter(|s| **s == part.socket.as_str()).count() as i32;
        seen.push(part.socket.as_str());
        let base = socket(&part.socket);
        let shift = socket_shift(&part.socket, kth);
        let flip_side = part.socket == "side" && kth % 2 == 1;
        let origin = [
            base[0] * sx / 100 + shift[0],
            base[1] * sy / 100 + shift[1],
            base[2] * sz / 100 + shift[2],
        ];
        let grow = ONE + fx::mul(p.pulses.get(i).copied().unwrap_or(0), ONE * 35 / 100);
        for m in &part.model {
            let mesh = Mesh::from_name(&m.mesh).unwrap_or(Mesh::Cube);
            let rgb = tint(colour_of(c, k, &m.colour, &part.colour));
            let copies: &[bool] = if m.mirror { &[false, true] } else { &[false] };
            for &mirrored in copies {
                let flip = mirrored != flip_side;
                let z_sign = if flip { -1 } else { 1 };
                let at = v3(
                    h(origin[0] + m.at[0]),
                    h(origin[1] + m.at[1]),
                    h(z_sign * (origin[2] + m.at[2])),
                );
                let rot = if flip {
                    [-m.rot[0], -m.rot[1], m.rot[2]]
                } else {
                    m.rot
                };
                let size = v3(
                    fx::mul(h(m.size[0]), grow),
                    fx::mul(h(m.size[1]), grow),
                    fx::mul(h(m.size[2]), grow),
                );
                let local = Affine::translate(at)
                    .then(&rotation(rot))
                    .then(&Affine::scale(size));
                b.inked(mesh, &t.then(&local), rgb);
            }
        }
    }
}

/// The fighters, facing each other, and anything in flight between them.
pub fn draw_fight(b: &mut Batch, c: &Content, f: &Fight, ended_at: Option<i32>, anim: i32) {
    let places = [(v3(-STAND_X, 0, 0), 0), (v3(STAND_X, 0, 0), fx::HALF)];
    let mut poses = Vec::new();
    for side in [PLAYER, FOE] {
        let lost = f.winner.is_some_and(|w| w != side);
        let p = pose_of(&f.sides[side], f.tick, ended_at, lost, anim + side as i32 * 37);
        let (pos, yaw) = places[side];
        draw_creature(b, c, &f.sides[side].creature, pos, yaw, &p);
        poses.push(p);
    }
    for s in &f.shots {
        draw_shot(b, c, f, s, &places, &poses);
    }
}

fn draw_shot(b: &mut Batch, c: &Content, f: &Fight, s: &Shot, places: &[(V3, i32); 2], poses: &[Pose]) {
    let age = f.tick - s.at;
    if !(0..SHOT_TICKS).contains(&age) {
        return;
    }
    let (from, to) = (s.from, 1 - s.from);
    let start = point_on(
        &f.sides[from].creature,
        places[from].0,
        places[from].1,
        &poses[from],
        [100, 10, 0],
    );
    let end = point_on(
        &f.sides[to].creature,
        places[to].0,
        places[to].1,
        &poses[to],
        [0, 0, 0],
    );
    let t = age * ONE / SHOT_TICKS;
    let at = start
        .lerp(end, t)
        .add(v3(0, fx::mul(fx::sin(t * fx::HALF / ONE), ONE * 6 / 10), 0));
    let rgb = match s.kind {
        ActionKind::Poison => c.palette.get("poison"),
        _ => c.palette.get("quill"),
    };
    let size = ONE * 22 / 100;
    b.lit(Mesh::Sphere, &boxed(at, v3(size, size, size)), rgb, ONE / 2);
    b.halo(
        Mesh::Sphere,
        &boxed(at, v3(size * 3, size * 3, size * 3)),
        rgb,
        ONE * 7 / 10,
    );
}

/// Where a fighter's head is, for numbers floating over it.
pub fn head_of(f: &Fight, side: usize) -> V3 {
    let k = &f.sides[side].creature;
    let x = if side == PLAYER { -STAND_X } else { STAND_X };
    v3(x, centre_height(k) + radius(k.mass) + ONE / 3, 0)
}

/// The clearing, the trees round it, the hills and the clouds: drawn once a
/// run, from the run's seed.
pub fn woods(c: &Content, seed: u64) -> Batch {
    let mut b = Batch::new(c.palette.ink);
    let mut rng = Rng::new(seed);
    let col = |n: &str| c.palette.get(n);
    b.plain(
        Mesh::Ground,
        &boxed(V3::ZERO, v3(int(600), ONE, int(600))),
        col("grass"),
    );
    b.inked(
        Mesh::Cyl,
        &boxed(v3(0, ONE / 100, 0), v3(int(15), ONE / 50, int(15))),
        col("clearing"),
    );
    b.inked(
        Mesh::Cyl,
        &boxed(v3(0, ONE / 40, 0), v3(int(9), ONE / 50, int(6))),
        col("dirt"),
    );
    // stepping stones under the two fighters
    for x in [-STAND_X, STAND_X] {
        b.inked(
            Mesh::Cyl,
            &boxed(v3(x, ONE / 25, 0), v3(int(2), ONE / 20, int(2) - ONE / 3)),
            col("dirtdark"),
        );
    }
    let around = |rng: &mut Rng, lo: i32, hi: i32| {
        let a = rng.range(0, fx::TURN - 1);
        let d = rng.range(lo, hi);
        v3(fx::mul(fx::cos(a), d), 0, fx::mul(fx::sin(a), d))
    };
    // the clearing's edge things keep out of the fight camera's way (+z)
    let edge = |rng: &mut Rng, lo: i32, hi: i32| loop {
        let p = around(rng, lo, hi);
        if p.z < int(2) || p.x.abs() > int(8) {
            break p;
        }
    };
    // trees: pines and round autumn trees, after Sam_Comm_final's greens and salmons
    let crowns = ["coral", "salmon", "rose", "peach", "grasslight"];
    let pines = ["pine", "grassdark", "teal"];
    for i in 0..110 {
        let at = around(&mut rng, int(11), int(46));
        let tall = int(3) + rng.range(0, int(4));
        let trunk_w = ONE / 3 + rng.range(0, ONE / 4);
        if i % 3 == 0 {
            b.inked(
                Mesh::Cyl,
                &boxed(at.add(v3(0, tall / 2, 0)), v3(trunk_w, tall, trunk_w)),
                col("trunk"),
            );
            let crown = col(crowns[rng.range(0, crowns.len() as i32 - 1) as usize]);
            let r = int(2) + rng.range(0, int(1));
            b.inked(
                Mesh::Blob,
                &boxed(at.add(v3(0, tall + r / 3, 0)), v3(r * 2, r * 2 - ONE / 2, r * 2)),
                crown,
            );
            let off = rng.range(-ONE, ONE);
            b.inked(
                Mesh::Blob,
                &boxed(at.add(v3(off, tall + r, off / 2)), v3(r, r, r)),
                shade(crown, 108),
            );
        } else {
            b.inked(
                Mesh::Cyl,
                &boxed(at.add(v3(0, ONE, 0)), v3(trunk_w, int(2), trunk_w)),
                col("trunkdark"),
            );
            let green = col(pines[rng.range(0, pines.len() as i32 - 1) as usize]);
            let w = int(3) + rng.range(0, int(1));
            for tier in 0..3 {
                let y = int(1) + tier * (tall / 3) + tall / 4;
                let s = w - tier * (w / 4);
                b.inked(
                    Mesh::Cone,
                    &boxed(at.add(v3(0, y, 0)), v3(s, tall / 2 + ONE, s)),
                    shade(green, 100 + tier * 6),
                );
            }
        }
    }
    // mushrooms, stones, flowers and ferns round the edge of the clearing
    for _ in 0..26 {
        let at = edge(&mut rng, int(5) + ONE / 2, int(9));
        let s = ONE / 4 + rng.range(0, ONE / 3);
        b.inked(
            Mesh::Cyl,
            &boxed(at.add(v3(0, s, 0)), v3(s / 2, s * 2, s / 2)),
            col("silk"),
        );
        b.inked(
            Mesh::Sphere,
            &boxed(at.add(v3(0, s * 2, 0)), v3(s * 2, s, s * 2)),
            col("mushroom"),
        );
    }
    for _ in 0..18 {
        let at = edge(&mut rng, int(6), int(10));
        let s = ONE / 2 + rng.range(0, ONE / 2);
        b.inked(
            Mesh::Octa,
            &boxed(at.add(v3(0, s / 4, 0)), v3(s * 2, s, s + ONE / 3)),
            col("stone"),
        );
    }
    for _ in 0..40 {
        let at = edge(&mut rng, int(4) + ONE / 2, int(10));
        let name = if rng.chance(50) { "flower" } else { "flowergold" };
        b.inked(
            Mesh::Cyl,
            &boxed(at.add(v3(0, ONE / 6, 0)), v3(ONE / 20, ONE / 3, ONE / 20)),
            col("grassdark"),
        );
        b.inked(
            Mesh::Sphere,
            &boxed(at.add(v3(0, ONE / 3, 0)), v3(ONE / 5, ONE / 8, ONE / 5)),
            col(name),
        );
    }
    for _ in 0..24 {
        let at = edge(&mut rng, int(6), int(10));
        let s = ONE / 2 + rng.range(0, ONE / 2);
        b.inked(
            Mesh::Cone,
            &boxed(at.add(v3(0, s / 2, 0)), v3(s * 2, s, s * 2)),
            col("grassdark"),
        );
    }
    // hills, and clouds in the warm sky
    for _ in 0..16 {
        let at = around(&mut rng, int(70), int(110));
        let w = int(40) + rng.range(0, int(30));
        let name = if rng.chance(50) { "hill" } else { "hillfar" };
        b.inked(Mesh::Blob, &boxed(at, v3(w, w / 2, w)), col(name));
    }
    for _ in 0..14 {
        let at = around(&mut rng, int(60), int(110)).add(v3(0, int(28) + rng.range(0, int(16)), 0));
        let w = int(6) + rng.range(0, int(4));
        for k in -1i32..=1 {
            let lump = w * (3 - k.abs()) / 3;
            b.inked(
                Mesh::Blob,
                &boxed(at.add(v3(k * w * 7 / 10, -lump / 4, 0)), v3(lump * 2, lump, lump)),
                col("cloud"),
            );
        }
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::tests::content;

    #[test]
    fn heavier_creatures_are_bigger() {
        assert!(radius(7) > radius(3));
    }

    #[test]
    fn every_creature_draws_a_body_eyes_and_each_part() {
        let c = content();
        for d in &c.creatures {
            let k = Creature::from_def(&c, d);
            let mut b = Batch::new(c.palette.ink);
            draw_creature(&mut b, &c, &k, V3::ZERO, 0, &idle_pose(&k, 0));
            let prims: usize = k
                .parts
                .iter()
                .map(|&p| {
                    c.parts[p]
                        .model
                        .iter()
                        .map(|m| if m.mirror { 2 } else { 1 })
                        .sum::<usize>()
                })
                .sum();
            let nubs = if k.parts.iter().any(|&p| c.parts[p].socket == "legs") {
                0
            } else {
                4
            };
            // body, belly, two eyes and any nubs inked; two pupils plain
            assert_eq!(b.count(), 2 * (4 + nubs + prims) + 2, "{}", d.id);
        }
    }

    #[test]
    fn a_creature_facing_the_other_way_has_its_mouth_on_the_other_side() {
        let c = content();
        let k = Creature::from_def(&c, &c.creatures[0]);
        let p = idle_pose(&k, 0);
        let right = point_on(&k, V3::ZERO, 0, &p, [100, 0, 0]);
        let left = point_on(&k, V3::ZERO, fx::HALF, &p, [100, 0, 0]);
        assert!(right.x > 0 && left.x < 0);
    }

    #[test]
    fn a_fallen_creature_lies_lower() {
        let c = content();
        let k = Creature::from_def(&c, &c.creatures[0]);
        let up = point_on(&k, V3::ZERO, 0, &idle_pose(&k, 0), [0, 100, 0]);
        let down = Pose {
            fallen: ONE,
            ..idle_pose(&k, 0)
        };
        assert!(point_on(&k, V3::ZERO, 0, &down, [0, 100, 0]).y < up.y);
    }

    #[test]
    fn the_woods_are_the_same_for_the_same_seed() {
        let c = content();
        let (a, _) = woods(&c, 4).finish(0);
        let (b, _) = woods(&c, 4).finish(0);
        let (d, _) = woods(&c, 6).finish(0);
        assert_eq!(a, b);
        assert_ne!(a, d);
    }
}
