//! A balance probe, run by hand: `cargo run --release --example balance`.
//! Prints how each starter fares against each creature, and how far two
//! automatic players get through whole runs. It reads the data files from
//! disk, which the core itself never does: this is a tool, not the game.

use gw_core::combat::{Fight, PLAYER};
use gw_core::content::Content;
use gw_core::creature::Creature;
use gw_core::map::Kind;
use gw_core::run::{Place, Run, Stage};

fn load() -> Content {
    let r = |n: &str| std::fs::read_to_string(format!("data/{n}.json")).unwrap();
    Content::load(
        &r("parts"),
        &r("creatures"),
        &r("encounters"),
        &r("rules"),
        &r("palette"),
        &r("copy"),
    )
    .unwrap()
}

fn value(c: &Content, p: usize) -> i32 {
    let part = &c.parts[p];
    let mut v = 0;
    if let Some(a) = &part.action {
        v += a.amount * 10_000 / a.every_ms;
    }
    v + part.mods.values().sum::<i32>() * 2
}

/// The body slot holding the weakest part, and its value.
fn weakest(c: &Content, r: &Run) -> Option<(usize, i32)> {
    r.me.parts
        .iter()
        .enumerate()
        .map(|(i, g)| (i, value(c, g.part) + g.level * 3))
        .min_by_key(|x| x.1)
}

/// Puts the offer at `i` on the body: into a free slot, or over the
/// weakest part (which is then binned) when the offer is better.
fn place(c: &Content, r: &mut Run, i: usize, part: usize) -> bool {
    if r.move_part(c, Place::Offer(i), Place::Creature) {
        return true;
    }
    match weakest(c, r) {
        Some((slot, worst)) if value(c, part) > worst + 3 => {
            r.bag.clear();
            if r.move_part(c, Place::Offer(i), Place::Body(slot)) {
                r.bag.clear();
                return true;
            }
            false
        }
        _ => false,
    }
}

fn play(c: &Content, starter: usize, seed: u64, smart: bool) -> (Stage, usize) {
    let mut r = Run::new(c, starter, seed);
    for _ in 0..400 {
        r.cultivate(c);
        match r.stage.clone() {
            Stage::Map => {
                let ch = r.choices();
                let hp_pct = r.hp * 100 / r.max_hp(c);
                let pick = if smart {
                    let score = |k: &usize| {
                        let kind = r.map.nodes[*k].kind;
                        match kind {
                            Kind::Rest => {
                                if hp_pct < 60 {
                                    100
                                } else {
                                    10
                                }
                            }
                            Kind::Shop => {
                                if r.acorns >= 20 {
                                    60
                                } else {
                                    5
                                }
                            }
                            Kind::Elite => {
                                if hp_pct > 75 {
                                    50
                                } else {
                                    -50
                                }
                            }
                            Kind::Battle => 40,
                            Kind::Encounter => 30,
                            Kind::Boss => 0,
                        }
                    };
                    *ch.iter().max_by_key(|k| score(k)).unwrap()
                } else {
                    ch[0]
                };
                r.pick(c, pick);
            }
            Stage::Battle { .. } => {
                r.fight_mut().unwrap().finish(c);
                r.conclude(c);
            }
            Stage::Reward { parts, .. } => {
                r.reward_forage(c);
                if smart {
                    let best = (0..parts.len()).max_by_key(|&i| value(c, parts[i])).unwrap();
                    if !place(c, &mut r, best, parts[best]) {
                        r.reward_hp(c);
                    }
                } else if seed.is_multiple_of(2) {
                    r.reward_hp(c);
                } else {
                    r.reward_part(c, parts.len() - 1);
                }
                r.reward_done();
            }
            Stage::Encounter { found, .. } => {
                if smart {
                    if let Some(g) = found {
                        place(c, &mut r, 0, g.part);
                    }
                    let best =
                        r.me.parts
                            .iter()
                            .enumerate()
                            .max_by_key(|(_, g)| value(c, g.part))
                            .map(|(i, _)| i);
                    if let Some(i) = best {
                        r.move_part(c, Place::Body(i), Place::Upgrade);
                    }
                } else {
                    r.encounter_take(c);
                }
                r.encounter_done();
            }
            Stage::Shop => {
                if smart {
                    r.shop_mass(c);
                    while r.shop_heal(c) {}
                    for i in 0..3 {
                        let part = r.shop.as_ref().unwrap().offers[i].part;
                        place(c, &mut r, i, part);
                    }
                }
                r.shop_leave();
            }
            Stage::Rest => {
                if smart && r.hp * 100 / r.max_hp(c) > 85 {
                    r.rest_grow(c);
                } else {
                    r.rest_heal(c);
                }
            }
            s @ (Stage::Dead | Stage::Won) => return (s, r.floor()),
        }
    }
    (Stage::Dead, 0)
}

fn main() {
    let c = load();
    println!("starter vs creature: wins/20, mean health lost");
    for sid in &c.rules.starters {
        let me = Creature::from_def(&c, &c.creatures[c.creature_index(sid).unwrap()]);
        print!("{sid:>12}:");
        for d in c.creatures.iter().filter(|d| d.tier >= 1) {
            let foe = Creature::from_def(&c, d);
            let (mut w, mut lost) = (0, 0);
            for s in 0..20 {
                let mut f = Fight::new(&c, &me, me.max_hp(&c), &foe, 100, s);
                f.finish(&c);
                if f.winner == Some(PLAYER) {
                    w += 1;
                }
                lost += me.max_hp(&c) - f.sides[PLAYER].hp.max(0);
            }
            print!(" {}:{}/{}", &d.id[..4.min(d.id.len())], w, lost / 20);
        }
        println!();
    }
    for smart in [false, true] {
        let mut wins = [0; 3];
        let mut floors = vec![0; c.rules.floors + 1];
        let n = 300;
        for seed in 0..n {
            let st = (seed % 3) as usize;
            let (s, f) = play(&c, st, seed * 7919 + 1, smart);
            if s == Stage::Won {
                wins[st] += 1;
            }
            floors[f.min(c.rules.floors)] += 1;
        }
        println!(
            "{} players: wins by starter {:?} of {} each; runs ending on each floor {:?}",
            if smart { "sensible" } else { "careless" },
            wins,
            n / 3,
            floors
        );
    }
}
