//! A creature: a body of some mass carrying body parts, one part to a unit of
//! mass. The numbers that follow from what it carries (its slowness, its
//! evasion, its health) are worked out here and nowhere else.

use crate::content::{Content, CreatureDef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Creature {
    pub name: String,
    pub mass: i32,
    /// indices into `Content::parts`, in socket order of arrival
    pub parts: Vec<usize>,
    /// health before parts add to it
    pub base_hp: i32,
    pub body: String,
    pub accent: String,
    pub shape: [i32; 3],
}

impl Creature {
    pub fn from_def(c: &Content, d: &CreatureDef) -> Creature {
        Creature {
            name: d.name.clone(),
            mass: d.mass,
            parts: d.parts.iter().filter_map(|p| c.part_index(p)).collect(),
            base_hp: d.hp,
            body: d.body.clone(),
            accent: d.accent.clone(),
            shape: d.shape,
        }
    }

    pub fn empty_slots(&self) -> i32 {
        (self.mass - self.parts.len() as i32).max(0)
    }

    pub fn has_room(&self) -> bool {
        self.empty_slots() > 0
    }

    /// The sum of one modifier over every part.
    pub fn sum(&self, c: &Content, name: &str) -> i32 {
        self.parts.iter().map(|&p| c.parts[p].modifier(name)).sum()
    }

    pub fn max_hp(&self, c: &Content) -> i32 {
        (self.base_hp + self.sum(c, "max_hp")).max(1)
    }

    /// The brief's slow: past `free_mass`, each further unit of mass and each
    /// further body part slows every part by `slow_pct_per` percent.
    pub fn slow_pct(&self, c: &Content) -> i32 {
        let r = &c.rules.combat;
        let over_mass = (self.mass - r.free_mass).max(0);
        let over_parts = (self.parts.len() as i32 - r.free_mass).max(0);
        r.slow_pct_per * (over_mass + over_parts)
    }

    /// Evasion before anything gained in a fight: flat evasion plus the
    /// wings' share for each empty slot. One point is one percent to dodge.
    pub fn base_evasion(&self, c: &Content) -> i32 {
        self.sum(c, "evasion") + self.sum(c, "evasion_per_empty") * self.empty_slots()
    }

    /// Adds a unit of mass, up to the rules' cap. False if already at it.
    pub fn grow(&mut self, c: &Content) -> bool {
        if self.mass >= c.rules.max_mass {
            return false;
        }
        self.mass += 1;
        true
    }

    /// Takes a unit of mass away, never below one. A part that no longer
    /// fits falls off: the newest part, since it sits on the outermost slot.
    /// Returns the part that fell, if one did.
    pub fn shrink(&mut self) -> Option<usize> {
        if self.mass <= 1 {
            return None;
        }
        self.mass -= 1;
        if self.parts.len() as i32 > self.mass {
            return self.parts.pop();
        }
        None
    }

    /// Grafts a part into an empty slot. False if there is none.
    pub fn graft(&mut self, part: usize) -> bool {
        if !self.has_room() {
            return false;
        }
        self.parts.push(part);
        true
    }

    /// Puts `part` where the part at `slot` was.
    pub fn replace(&mut self, slot: usize, part: usize) -> bool {
        if slot >= self.parts.len() {
            return false;
        }
        self.parts[slot] = part;
        true
    }

    pub fn remove(&mut self, slot: usize) -> Option<usize> {
        if slot >= self.parts.len() {
            return None;
        }
        Some(self.parts.remove(slot))
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
        let teeth = c.part_index("teeth").unwrap();
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
        assert_eq!(wasp.empty_slots(), 1);
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
        assert_eq!(k.shrink(), Some(last));
        assert_eq!((k.mass, k.parts.len()), (2, 2));
        let mut wasp = make(&c, "wasp");
        assert_eq!(wasp.shrink(), None, "an empty slot is lost first");
        assert_eq!(wasp.parts.len(), 2);
    }

    #[test]
    fn mass_never_falls_below_one_or_rises_past_the_cap() {
        let c = content();
        let mut k = make(&c, "rabbit");
        for _ in 0..10 {
            k.shrink();
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
        let stinger = c.part_index("stinger").unwrap();
        assert!(!k.graft(stinger));
        assert!(k.replace(0, stinger));
        assert_eq!(k.parts[0], stinger);
        k.grow(&c);
        assert!(k.graft(stinger));
    }
}
