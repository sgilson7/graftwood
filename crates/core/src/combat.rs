//! The auto-battle. Two creatures stand facing each other; each body part with
//! an action counts down and acts when it reaches zero, then starts again,
//! until one creature falls. Nothing here is chosen by the player once the
//! fight begins: the build decides it, and the seeded roll decides each dodge.
//!
//! Order inside one tick, so that a fight replays exactly: the player's parts
//! in slot order, then the foe's; then poison; then fatigue; then deaths. Both
//! falling on the same tick counts as the player's win.

use crate::content::{ActionKind, Content};
use crate::creature::Creature;
use crate::rng::Rng;

pub const TICKS_PER_SECOND: i32 = 60;
pub const PLAYER: usize = 0;
pub const FOE: usize = 1;

pub fn ms_to_ticks(ms: i32) -> i32 {
    ms * TICKS_PER_SECOND / 1000
}

/// An action's period in ticks on a creature this slowed and this hasted:
/// slow lengthens it, haste shortens it, and nothing acts faster than the
/// rules' fastest.
pub fn period_ticks(c: &Content, every_ms: i32, slow_pct: i32, haste_pct: i32) -> i32 {
    let factor = (100 + slow_pct - haste_pct).max(25);
    let fastest = ms_to_ticks(c.rules.combat.fastest_ms).max(1);
    (ms_to_ticks(every_ms) * factor / 100).max(fastest)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ev {
    /// damage from an attack, with the action that dealt it
    Hit(ActionKind),
    Dodge,
    Poisoned,
    PoisonTick,
    Heal,
    Guard,
    Evade,
    Thorns,
    Fatigue,
    /// damage soaked up by guard
    Blocked,
}

/// Something that happened to `side` on `tick`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub tick: i32,
    pub side: usize,
    pub ev: Ev,
    pub amount: i32,
}

/// A thrown thing in flight, for the picture: who threw it, what it is, when.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shot {
    pub from: usize,
    pub kind: ActionKind,
    pub at: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fighter {
    pub creature: Creature,
    pub hp: i32,
    pub max_hp: i32,
    /// for each part, its action's period in ticks after slow and haste, or 0
    pub period: Vec<i32>,
    pub timer: Vec<i32>,
    pub slow_pct: i32,
    pub evasion_gained: i32,
    pub poison: i32,
    pub poison_timer: i32,
    pub guard: i32,
    base_evasion: i32,
    ranged_less: i32,
    melee_less: i32,
    ranged_more: i32,
    melee_more: i32,
    thorns: i32,
    // for the picture: the tick each happened last
    pub fired_at: Vec<i32>,
    pub lunge_at: i32,
    pub hurt_at: i32,
    pub dodge_at: i32,
}

const LONG_AGO: i32 = -100_000;

impl Fighter {
    pub fn new(c: &Content, creature: &Creature, hp: i32, max_hp: i32) -> Fighter {
        let r = &c.rules.combat;
        let slow = creature.slow_pct(c);
        let haste = creature.sum(c, "haste");
        let period: Vec<i32> = creature
            .parts
            .iter()
            .map(|&p| match &c.parts[p].action {
                Some(a) => period_ticks(c, a.every_ms, slow, haste),
                None => 0,
            })
            .collect();
        Fighter {
            hp: hp.clamp(0, max_hp),
            max_hp,
            timer: period.clone(),
            fired_at: vec![LONG_AGO; period.len()],
            period,
            slow_pct: slow,
            evasion_gained: 0,
            poison: 0,
            poison_timer: ms_to_ticks(r.poison_every_ms),
            guard: 0,
            base_evasion: creature.base_evasion(c),
            ranged_less: creature.sum(c, "ranged_less"),
            melee_less: creature.sum(c, "melee_less"),
            ranged_more: creature.sum(c, "ranged_more"),
            melee_more: creature.sum(c, "melee_more"),
            thorns: creature.sum(c, "thorns"),
            creature: creature.clone(),
            lunge_at: LONG_AGO,
            hurt_at: LONG_AGO,
            dodge_at: LONG_AGO,
        }
    }

    pub fn evasion(&self, cap: i32) -> i32 {
        (self.base_evasion + self.evasion_gained).clamp(0, cap)
    }

    pub fn alive(&self) -> bool {
        self.hp > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fight {
    pub sides: [Fighter; 2],
    pub tick: i32,
    rng: Rng,
    pub events: Vec<Event>,
    pub shots: Vec<Shot>,
    pub winner: Option<usize>,
    /// the tick the fight was decided
    pub ended_at: Option<i32>,
}

impl Fight {
    pub fn new(c: &Content, me: &Creature, my_hp: i32, foe: &Creature, foe_hp_pct: i32, seed: u64) -> Fight {
        let foe_max = (foe.max_hp(c) * foe_hp_pct / 100).max(1);
        Fight {
            sides: [
                Fighter::new(c, me, my_hp, me.max_hp(c)),
                Fighter::new(c, foe, foe_max, foe_max),
            ],
            tick: 0,
            rng: Rng::new(seed),
            events: Vec::new(),
            shots: Vec::new(),
            winner: None,
            ended_at: None,
        }
    }

    pub fn over(&self) -> bool {
        self.winner.is_some()
    }

    fn note(&mut self, side: usize, ev: Ev, amount: i32) {
        self.events.push(Event {
            tick: self.tick,
            side,
            ev,
            amount,
        });
    }

    /// Damage that guard can soak, then health. Returns what reached health.
    fn wound(&mut self, side: usize, amount: i32) -> i32 {
        let f = &mut self.sides[side];
        let soaked = f.guard.min(amount);
        f.guard -= soaked;
        let dealt = amount - soaked;
        f.hp -= dealt;
        if dealt > 0 {
            f.hurt_at = self.tick;
        }
        if soaked > 0 {
            self.note(side, Ev::Blocked, soaked);
        }
        dealt
    }

    /// Whether the defender dodges, from its evasion and the seeded roll.
    fn dodges(&mut self, c: &Content, defender: usize) -> bool {
        let chance = self.sides[defender].evasion(c.rules.combat.evasion_cap);
        if chance > 0 && self.rng.range(0, 99) < chance {
            self.sides[defender].dodge_at = self.tick;
            self.note(defender, Ev::Dodge, 0);
            return true;
        }
        false
    }

    fn fire(&mut self, c: &Content, side: usize, slot: usize) {
        let part = self.sides[side].creature.parts[slot];
        let Some(action) = c.parts[part].action.clone() else {
            return;
        };
        let foe = 1 - side;
        self.sides[side].fired_at[slot] = self.tick;
        let kind = action.kind;
        if kind.is_melee() {
            self.sides[side].lunge_at = self.tick;
        }
        if kind == ActionKind::Ranged || kind == ActionKind::Poison {
            self.shots.push(Shot {
                from: side,
                kind,
                at: self.tick,
            });
        }
        if kind.is_attack() && self.dodges(c, foe) {
            return;
        }
        let me = &self.sides[side];
        let them = &self.sides[foe];
        match kind {
            ActionKind::Melee | ActionKind::Drain | ActionKind::Ranged => {
                let amount = if kind == ActionKind::Ranged {
                    action.amount + me.ranged_more - them.ranged_less
                } else {
                    action.amount + me.melee_more - them.melee_less
                }
                .max(0);
                let dealt = self.wound(foe, amount);
                self.note(foe, Ev::Hit(kind), dealt);
                if kind == ActionKind::Drain && dealt > 0 {
                    let f = &mut self.sides[side];
                    let healed = dealt.min(f.max_hp - f.hp.max(0));
                    f.hp += healed;
                    if healed > 0 {
                        self.note(side, Ev::Heal, healed);
                    }
                }
                let thorns = self.sides[foe].thorns;
                if kind.is_melee() && thorns > 0 {
                    let back = self.wound(side, thorns);
                    self.note(side, Ev::Thorns, back);
                }
            }
            ActionKind::Poison => {
                let cap = c.rules.combat.poison_cap;
                let f = &mut self.sides[foe];
                let before = f.poison;
                f.poison = (f.poison + action.amount).min(cap);
                let added = f.poison - before;
                self.note(foe, Ev::Poisoned, added);
            }
            ActionKind::Evade => {
                self.sides[side].evasion_gained += action.amount;
                self.note(side, Ev::Evade, action.amount);
            }
            ActionKind::Heal => {
                let f = &mut self.sides[side];
                let healed = action.amount.min(f.max_hp - f.hp.max(0)).max(0);
                f.hp += healed;
                self.note(side, Ev::Heal, healed);
            }
            ActionKind::Guard => {
                let cap = c.rules.combat.guard_cap;
                let f = &mut self.sides[side];
                f.guard = (f.guard + action.amount).min(cap);
                self.note(side, Ev::Guard, action.amount);
            }
        }
    }

    /// One tick of the fight. Does nothing once it is over.
    pub fn step(&mut self, c: &Content) {
        if self.over() {
            return;
        }
        let r = c.rules.combat.clone();
        self.tick += 1;
        for side in [PLAYER, FOE] {
            for slot in 0..self.sides[side].period.len() {
                let f = &mut self.sides[side];
                if f.period[slot] == 0 {
                    continue;
                }
                f.timer[slot] -= 1;
                if f.timer[slot] <= 0 {
                    f.timer[slot] = f.period[slot];
                    self.fire(c, side, slot);
                }
            }
        }
        let poison_period = ms_to_ticks(r.poison_every_ms).max(1);
        for side in [PLAYER, FOE] {
            let f = &mut self.sides[side];
            if f.poison == 0 {
                f.poison_timer = poison_period;
                continue;
            }
            f.poison_timer -= 1;
            if f.poison_timer <= 0 {
                f.poison_timer = poison_period;
                let stacks = f.poison;
                f.hp -= stacks;
                f.hurt_at = self.tick;
                self.note(side, Ev::PoisonTick, stacks);
            }
        }
        let after = ms_to_ticks(r.fatigue_after_ms);
        let every = ms_to_ticks(r.fatigue_every_ms).max(1);
        if self.tick > after && (self.tick - after) % every == 0 {
            let amount = 1 + (self.tick - after) / ms_to_ticks(r.fatigue_growth_ms).max(1);
            for side in [PLAYER, FOE] {
                self.sides[side].hp -= amount;
                self.sides[side].hurt_at = self.tick;
                self.note(side, Ev::Fatigue, amount);
            }
        }
        let (me, foe) = (self.sides[PLAYER].alive(), self.sides[FOE].alive());
        if !foe {
            self.winner = Some(PLAYER);
        } else if !me {
            self.winner = Some(FOE);
        } else if self.tick >= ms_to_ticks(r.max_fight_ms) {
            // a last resort fatigue makes all but unreachable
            let share = |f: &Fighter| f.hp * 1000 / f.max_hp.max(1);
            let mine = share(&self.sides[PLAYER]);
            self.winner = Some(if mine >= share(&self.sides[FOE]) {
                PLAYER
            } else {
                FOE
            });
        }
        if self.winner.is_some() {
            self.ended_at = Some(self.tick);
        }
        // keep only what the picture still needs
        let keep = self.tick - TICKS_PER_SECOND * 2;
        self.shots.retain(|s| s.at > keep);
        if self.events.len() > 400 {
            self.events.retain(|e| e.tick > keep);
        }
    }

    /// Runs the fight to its end, as the skip button does.
    pub fn finish(&mut self, c: &Content) {
        let limit = ms_to_ticks(c.rules.combat.max_fight_ms) + 1;
        while !self.over() && self.tick <= limit {
            self.step(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::tests::content;

    fn make(c: &Content, id: &str) -> Creature {
        Creature::from_def(c, &c.creatures[c.creature_index(id).unwrap()])
    }

    fn bare(c: &Content, parts: &[&str], mass: i32, hp: i32) -> Creature {
        Creature {
            name: "test".into(),
            mass,
            parts: parts.iter().map(|p| c.part_index(p).unwrap()).collect(),
            base_hp: hp,
            body: "fur".into(),
            accent: "cream".into(),
            shape: [100, 100, 100],
        }
    }

    fn run_ticks(f: &mut Fight, c: &Content, n: i32) {
        for _ in 0..n {
            f.step(c);
        }
    }

    #[test]
    fn teeth_bite_for_two_every_two_seconds() {
        let c = content();
        let biter = bare(&c, &["teeth"], 3, 50);
        let post = bare(&c, &["thick_fur"], 3, 50);
        let mut f = Fight::new(&c, &biter, 50, &post, 100, 1);
        let start = f.sides[FOE].hp;
        run_ticks(&mut f, &c, 119);
        assert_eq!(f.sides[FOE].hp, start, "nothing before two seconds");
        f.step(&c);
        assert_eq!(f.sides[FOE].hp, start - 2);
        run_ticks(&mut f, &c, 120);
        assert_eq!(f.sides[FOE].hp, start - 4);
    }

    #[test]
    fn hearing_takes_one_off_each_ranged_hit_and_none_off_a_bite() {
        let c = content();
        let quills = bare(&c, &["quills"], 3, 50);
        let ears = bare(&c, &["bunny_ears"], 3, 50);
        let mut f = Fight::new(&c, &quills, 50, &ears, 100, 1);
        let amount = c.parts[c.part_index("quills").unwrap()]
            .action
            .clone()
            .unwrap()
            .amount;
        let start = f.sides[FOE].hp;
        f.finish_first_action(&c);
        assert_eq!(f.sides[FOE].hp, start - (amount - 1));
        let teeth = bare(&c, &["teeth"], 3, 50);
        let mut g = Fight::new(&c, &teeth, 50, &ears, 100, 1);
        g.finish_first_action(&c);
        assert_eq!(g.sides[FOE].hp, start - 2);
    }

    impl Fight {
        fn finish_first_action(&mut self, c: &Content) {
            let n = self.events.len();
            while self.events.len() == n {
                self.step(c);
            }
        }
    }

    #[test]
    fn a_stinger_stacks_poison_to_ten_and_each_stack_hurts_every_two_seconds() {
        let c = content();
        let wasp = bare(&c, &["stinger"], 2, 50);
        let post = bare(&c, &["thick_fur"], 3, 500);
        let mut f = Fight::new(&c, &wasp, 50, &post, 100, 1);
        // the first sting lands at 4 s: one stack
        run_ticks(&mut f, &c, 240);
        assert_eq!(f.sides[FOE].poison, 1);
        let hp = f.sides[FOE].hp;
        run_ticks(&mut f, &c, 120);
        assert_eq!(f.sides[FOE].hp, hp - 1, "one stack: one damage in two seconds");
        // long after: capped at ten
        run_ticks(&mut f, &c, 60 * 60);
        assert_eq!(f.sides[FOE].poison, 10);
    }

    #[test]
    fn lucky_feet_add_one_evasion_every_five_seconds() {
        let c = content();
        let feet = bare(&c, &["lucky_feet"], 3, 50);
        let post = bare(&c, &["thick_fur"], 3, 50);
        let mut f = Fight::new(&c, &feet, 50, &post, 100, 1);
        run_ticks(&mut f, &c, 300);
        assert_eq!(f.sides[PLAYER].evasion(75), 1);
        run_ticks(&mut f, &c, 600);
        assert_eq!(f.sides[PLAYER].evasion(75), 3);
    }

    #[test]
    fn a_heavy_body_acts_slower() {
        let c = content();
        let light = bare(&c, &["teeth"], 5, 50);
        let heavy = bare(&c, &["teeth"], 7, 50);
        let a = Fighter::new(&c, &light, 50, 50);
        let b = Fighter::new(&c, &heavy, 50, 50);
        assert_eq!(a.period[0], 120);
        assert_eq!(b.period[0], 144, "two mass over five: 20% slower");
    }

    #[test]
    fn dodges_follow_evasion() {
        let c = content();
        let biter = bare(&c, &["claws"], 3, 500);
        // wings on a body of nine mass with one part: 8 empty slots, 24 evasion
        let flier = bare(&c, &["wings"], 9, 500);
        let mut f = Fight::new(&c, &biter, 500, &flier, 100, 7);
        run_ticks(&mut f, &c, 60 * 30);
        let dodges = f.events.iter().filter(|e| e.ev == Ev::Dodge).count() as i32;
        let hits = f.events.iter().filter(|e| matches!(e.ev, Ev::Hit(_))).count() as i32;
        let pct = dodges * 100 / (dodges + hits);
        assert!(
            (12..=36).contains(&pct),
            "about 24% dodged: {pct}% of {}",
            dodges + hits
        );
    }

    #[test]
    fn guard_soaks_damage_before_health() {
        let c = content();
        let biter = bare(&c, &["teeth"], 3, 50);
        let slime = bare(&c, &["slime_coat"], 3, 50);
        let mut f = Fight::new(&c, &biter, 50, &slime, 100, 1);
        f.sides[FOE].guard = 5;
        let start = f.sides[FOE].hp;
        run_ticks(&mut f, &c, 120);
        assert_eq!(f.sides[FOE].hp, start, "the bite went into guard");
        assert_eq!(f.sides[FOE].guard, 3);
    }

    #[test]
    fn spines_hurt_whoever_bites() {
        let c = content();
        let biter = bare(&c, &["teeth"], 3, 50);
        let spiny = bare(&c, &["spines"], 3, 50);
        let mut f = Fight::new(&c, &biter, 50, &spiny, 100, 1);
        let thorns = c.parts[c.part_index("spines").unwrap()].modifier("thorns");
        run_ticks(&mut f, &c, 120);
        assert_eq!(f.sides[PLAYER].hp, 50 - thorns);
    }

    #[test]
    fn fatigue_ends_a_fight_between_two_who_cannot_hurt_each_other() {
        let c = content();
        let a = bare(&c, &["thick_fur"], 3, 40);
        let b = bare(&c, &["thick_fur"], 3, 40);
        let mut f = Fight::new(&c, &a, a.max_hp(&c), &b, 100, 1);
        f.finish(&c);
        assert!(f.over());
        assert!(f.tick < ms_to_ticks(c.rules.combat.max_fight_ms));
        assert_eq!(f.winner, Some(PLAYER), "falling together is the player's win");
    }

    #[test]
    fn the_same_seed_replays_the_same_fight() {
        let c = content();
        let me = make(&c, "spider");
        let foe = make(&c, "rabbit");
        let mut a = Fight::new(&c, &me, 20, &foe, 100, 42);
        let mut b = Fight::new(&c, &me, 20, &foe, 100, 42);
        a.finish(&c);
        b.finish(&c);
        assert_eq!(a.tick, b.tick);
        assert_eq!(a.winner, b.winner);
        assert_eq!(a.sides[PLAYER].hp, b.sides[PLAYER].hp);
    }

    #[test]
    fn every_creature_can_finish_a_fight_against_every_other() {
        let c = content();
        for x in &c.creatures {
            for y in &c.creatures {
                let (p, q) = (Creature::from_def(&c, x), Creature::from_def(&c, y));
                let mut f = Fight::new(&c, &p, p.max_hp(&c), &q, 100, 3);
                f.finish(&c);
                assert!(f.over(), "{} v {}", x.id, y.id);
            }
        }
    }
}
