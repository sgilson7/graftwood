//! A creature: a body of some mass carrying body parts, one part to a unit of
//! mass. The numbers that follow from what it carries (its slowness, its
//! evasion, its health) are worked out here and nowhere else.

use crate::content::{Content, CreatureDef, UNSCALED_MOD};

/// A body part on a creature or in the bag: which part, and how many times
/// it has been upgraded (below 0 when blight has withered it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Graft {
    pub part: usize,
    pub level: i32,
}

impl Graft {
    pub fn new(part: usize) -> Graft {
        Graft { part, level: 0 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Creature {
    pub name: String,
    pub mass: i32,
    /// the parts it carries, in order of arrival
    pub parts: Vec<Graft>,
    /// health before parts add to it
    pub base_hp: i32,
    pub body: String,
    pub accent: String,
    pub shape: [i32; 3],
}

/// A set's standing on one body: how many of its parts, and the bonus
/// reached (an index into the set's bonuses), if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetStanding {
    pub set: usize,
    pub count: i32,
    pub bonus: Option<usize>,
}

impl Creature {
    pub fn from_def(c: &Content, d: &CreatureDef) -> Creature {
        Creature {
            name: d.name.clone(),
            mass: d.mass,
            parts: d
                .parts
                .iter()
                .filter_map(|p| c.part_index(p))
                .map(Graft::new)
                .collect(),
            base_hp: d.hp,
            body: d.body.clone(),
            accent: d.accent.clone(),
            shape: d.shape,
        }
    }

    /// The slots Two-Headed grows, which only head parts may fill.
    pub fn head_slots(&self, c: &Content) -> i32 {
        self.parts
            .iter()
            .map(|g| c.parts[g.part].modifier(UNSCALED_MOD))
            .sum()
    }

    /// Parts that take a slot: (head parts, the rest).
    fn slot_users(&self, c: &Content) -> (i32, i32) {
        let mut heads = 0;
        let mut rest = 0;
        for g in &self.parts {
            let p = &c.parts[g.part];
            if p.slotless {
                continue;
            }
            if p.socket == "head" {
                heads += 1;
            } else {
                rest += 1;
            }
        }
        (heads, rest)
    }

    /// Parts that take a slot.
    pub fn used_slots(&self, c: &Content) -> i32 {
        let (heads, rest) = self.slot_users(c);
        heads + rest
    }

    /// Every slot: one a unit of mass, and each head-only slot.
    pub fn capacity(&self, c: &Content) -> i32 {
        self.mass + self.head_slots(c)
    }

    /// Whether everything it carries has a slot: the rest within its mass,
    /// and the head parts in what is left over or in the head-only slots.
    pub fn fits(&self, c: &Content) -> bool {
        let (heads, rest) = self.slot_users(c);
        rest <= self.mass && heads + rest <= self.capacity(c)
    }

    pub fn empty_slots(&self, c: &Content) -> i32 {
        (self.capacity(c) - self.used_slots(c)).max(0)
    }

    /// Whether `part` could be grafted on as things stand.
    pub fn can_take(&self, c: &Content, part: Graft) -> bool {
        let mut k = self.clone();
        k.parts.push(part);
        k.fits(c)
    }

    /// How many parts of each set it carries, and the bonus each has
    /// reached, in the order rules.json lists the sets.
    pub fn sets(&self, c: &Content) -> Vec<SetStanding> {
        c.rules
            .sets
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let count = self.parts.iter().filter(|g| c.parts[g.part].set == s.id).count() as i32;
                let bonus = s
                    .bonuses
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| count >= b.count)
                    .max_by_key(|(_, b)| b.count)
                    .map(|(k, _)| k);
                SetStanding { set: i, count, bonus }
            })
            .collect()
    }

    /// The sum of one modifier over every part, at each part's level, and
    /// the set bonuses reached.
    pub fn sum(&self, c: &Content, name: &str) -> i32 {
        let u = &c.rules.upgrade;
        let parts: i32 = self
            .parts
            .iter()
            .map(|g| c.parts[g.part].modifier_at(name, g.level, u))
            .sum();
        let sets: i32 = self
            .sets(c)
            .iter()
            .filter_map(|s| s.bonus.map(|b| (s.set, b)))
            .map(|(s, b)| c.rules.sets[s].bonuses[b].mods.get(name).copied().unwrap_or(0))
            .sum();
        parts + sets
    }

    pub fn max_hp(&self, c: &Content) -> i32 {
        (self.base_hp + self.sum(c, "max_hp")).max(1)
    }

    /// Tenths of health a second in a fight, no more than the rules' cap
    /// (which is whole health a second).
    pub fn regen(&self, c: &Content) -> i32 {
        self.sum(c, "regen").clamp(0, c.rules.combat.regen_cap * 10)
    }

    /// The brief's slow: past `free_mass`, each further unit of mass and each
    /// further body part slows every part by `slow_pct_per` percent.
    pub fn slow_pct(&self, c: &Content) -> i32 {
        let r = &c.rules.combat;
        let over_mass = (self.mass - r.free_mass).max(0);
        let over_parts = (self.used_slots(c) - r.free_mass).max(0);
        r.slow_pct_per * (over_mass + over_parts)
    }

    /// Evasion before anything gained in a fight: flat evasion plus the
    /// wings' share for each empty slot. One point is one percent to dodge.
    pub fn base_evasion(&self, c: &Content) -> i32 {
        self.sum(c, "evasion") + self.sum(c, "evasion_per_empty") * self.empty_slots(c)
    }

    /// Adds a unit of mass, up to the rules' cap. False if already at it.
    pub fn grow(&mut self, c: &Content) -> bool {
        if self.mass >= c.rules.max_mass {
            return false;
        }
        self.mass += 1;
        true
    }

    /// Drops parts until everything left has a slot: the newest first, and
    /// a head part only when the head-only slots are what overflowed.
    /// Returns what fell, in the order it fell.
    pub fn settle(&mut self, c: &Content) -> Vec<Graft> {
        let mut fell = Vec::new();
        while !self.fits(c) {
            let (_, rest) = self.slot_users(c);
            let body_full = rest > self.mass;
            let pick = self.parts.iter().rposition(|g| {
                let p = &c.parts[g.part];
                !p.slotless && (!body_full || p.socket != "head")
            });
            match pick {
                Some(i) => fell.push(self.parts.remove(i)),
                None => break,
            }
        }
        fell
    }

    /// Takes a unit of mass away, never below one. A part that no longer
    /// fits falls off: the newest, since it sits on the outermost slot.
    /// Returns what fell.
    pub fn shrink(&mut self, c: &Content) -> Vec<Graft> {
        if self.mass <= 1 {
            return Vec::new();
        }
        self.mass -= 1;
        self.settle(c)
    }

    /// Grafts a part into an empty slot. False if it has no slot.
    pub fn graft(&mut self, c: &Content, part: Graft) -> bool {
        if !self.can_take(c, part) {
            return false;
        }
        self.parts.push(part);
        true
    }

    /// Takes off the part at `slot`, and anything that loses its slot with
    /// it (a head part, when the second head goes). Returns both.
    pub fn remove(&mut self, c: &Content, slot: usize) -> Option<(Graft, Vec<Graft>)> {
        if slot >= self.parts.len() {
            return None;
        }
        let g = self.parts.remove(slot);
        let fell = self.settle(c);
        Some((g, fell))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::tests::content;

    fn make(c: &Content, id: &str) -> Creature {
        Creature::from_def(c, &c.creatures[c.creature_index(id).unwrap()])
    }

    #[test]
    fn up_to_five_mass_there_is_no_slow() {
        let c = content();
        let mut k = make(&c, "rabbit");
        k.mass = 5;
        assert_eq!(k.slow_pct(&c), 0);
    }

    #[test]
    fn each_mass_and_each_part_past_five_slows_by_ten_percent() {
        let c = content();
        let mut k = make(&c, "rabbit");
        let teeth = Graft::new(c.part_index("teeth").unwrap());
        k.mass = 7; // two past five
        assert_eq!(k.slow_pct(&c), 20);
        while k.parts.len() < 6 {
            k.parts.push(teeth);
        }
        // seven mass (two over) and six parts (one over)
        assert_eq!(k.slow_pct(&c), 30);
    }

    #[test]
    fn wings_give_three_evasion_for_each_empty_slot() {
        let c = content();
        let mut wasp = make(&c, "wasp");
        // the wasp: three mass, two parts, so one empty slot
        assert_eq!(wasp.empty_slots(&c), 1);
        assert_eq!(wasp.base_evasion(&c), 3);
        wasp.mass = 5;
        assert_eq!(wasp.base_evasion(&c), 9);
        wasp.mass = 2;
        assert_eq!(wasp.base_evasion(&c), 0);
    }

    #[test]
    fn losing_mass_drops_the_part_that_no_longer_fits() {
        let c = content();
        let mut k = make(&c, "rabbit");
        let last = *k.parts.last().unwrap();
        assert_eq!(k.shrink(&c), vec![last]);
        assert_eq!((k.mass, k.parts.len()), (2, 2));
        let mut wasp = make(&c, "wasp");
        assert_eq!(wasp.shrink(&c), vec![], "an empty slot is lost first");
        assert_eq!(wasp.parts.len(), 2);
    }

    #[test]
    fn mass_never_falls_below_one_or_rises_past_the_cap() {
        let c = content();
        let mut k = make(&c, "rabbit");
        for _ in 0..10 {
            k.shrink(&c);
        }
        assert_eq!(k.mass, 1);
        for _ in 0..20 {
            k.grow(&c);
        }
        assert_eq!(k.mass, c.rules.max_mass);
    }

    #[test]
    fn a_full_body_takes_no_graft_but_takes_a_replacement() {
        let c = content();
        let mut k = make(&c, "rabbit");
        let stinger = Graft::new(c.part_index("stinger").unwrap());
        assert!(!k.graft(&c, stinger));
        k.grow(&c);
        assert!(k.graft(&c, stinger));
        assert_eq!(k.parts.last(), Some(&stinger));
    }

    fn part(c: &Content, id: &str) -> Graft {
        Graft::new(c.part_index(id).unwrap())
    }

    /// Three mass, full, and no head part.
    fn headless(c: &Content) -> Creature {
        let mut k = make(c, "rabbit");
        k.parts = vec![part(c, "teeth"), part(c, "thick_fur"), part(c, "lucky_feet")];
        k
    }

    #[test]
    fn with_a_head_part_aboard_a_second_head_frees_a_slot_for_anything() {
        let c = content();
        // the rabbit's ears move into the head-only slot
        let mut k = make(&c, "rabbit");
        assert!(k.graft(&c, part(&c, "two_headed")));
        assert!(k.can_take(&c, part(&c, "stinger")));
    }

    #[test]
    fn two_headed_takes_no_slot_and_grows_one_only_a_head_part_fits() {
        let c = content();
        let mut k = headless(&c);
        let two = part(&c, "two_headed");
        assert!(k.can_take(&c, two), "it needs no slot of its own");
        assert!(k.graft(&c, two));
        assert_eq!(k.capacity(&c), 4);
        assert!(!k.can_take(&c, part(&c, "stinger")), "a tail part does not fit");
        assert!(k.can_take(&c, part(&c, "antennae")), "a head part does");
        k.graft(&c, part(&c, "antennae"));
        assert!(k.fits(&c));
        assert_eq!(k.empty_slots(&c), 0);
        // losing the second head drops the head part that sat in its slot
        let at = k.parts.iter().position(|g| *g == two).unwrap();
        let (gone, fell) = k.remove(&c, at).unwrap();
        assert_eq!(gone, two);
        assert_eq!(fell, vec![part(&c, "antennae")]);
        assert!(k.fits(&c));
    }

    #[test]
    fn losing_mass_with_a_second_head_drops_a_body_part_not_a_head() {
        let c = content();
        let mut k = headless(&c);
        assert!(k.graft(&c, part(&c, "two_headed")));
        assert!(k.graft(&c, part(&c, "antennae")));
        let fell = k.shrink(&c);
        assert_eq!(fell.len(), 1);
        assert_ne!(c.parts[fell[0].part].socket, "head");
        assert!(k.fits(&c));
    }

    #[test]
    fn two_parts_of_a_set_earn_its_first_bonus_and_more_earn_more() {
        let c = content();
        let bug = c.set_index("bug").unwrap();
        let mut k = make(&c, "rabbit");
        k.mass = 9;
        k.parts.clear();
        let haste = |k: &Creature| k.sum(&c, "haste");
        k.parts.push(part(&c, "stinger"));
        assert_eq!(k.sets(&c)[bug].bonus, None);
        assert_eq!(haste(&k), 0);
        k.parts.push(part(&c, "mandibles"));
        let first = &c.rules.sets[bug].bonuses[0];
        assert_eq!(first.count, 2);
        assert_eq!(k.sets(&c)[bug].bonus, Some(0));
        assert_eq!(haste(&k), first.mods["haste"]);
        k.parts.push(part(&c, "antennae"));
        assert_eq!(k.sets(&c)[bug].bonus, Some(1));
        assert!(haste(&k) > first.mods["haste"]);
        // a wild part counts toward no set
        k.parts.push(part(&c, "third_eye"));
        assert_eq!(k.sets(&c)[bug].count, 3);
    }

    #[test]
    fn regeneration_never_passes_three_a_second() {
        let c = content();
        let mut k = make(&c, "rabbit");
        k.mass = 9;
        k.parts.clear();
        for _ in 0..9 {
            k.parts.push(part(&c, "shed_skin"));
        }
        // regeneration is counted in tenths of health a second
        assert!(k.sum(&c, "regen") > 30);
        assert_eq!(k.regen(&c), 30);
        assert_eq!(c.rules.combat.regen_cap, 3);
    }
}
