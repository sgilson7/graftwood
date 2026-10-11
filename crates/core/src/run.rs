//! One run: the map, the creature you have grown, its health and acorns, and
//! where on the path you stand. Every choice the player makes between fights
//! is a method here, and each one refuses what the rules do not allow.

use crate::combat::{Fight, PLAYER};
use crate::content::{fill, Content, Outcome};
use crate::creature::Creature;
use crate::map::{Kind, Map};
use crate::rng::Rng;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub part: usize,
    pub price: i32,
    pub sold: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shop {
    pub offers: Vec<Offer>,
    pub mass_bought: bool,
    pub shed_used: bool,
}

/// Where a graft returns to once it is settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Back {
    Map,
    Shop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    Map,
    Battle {
        fight: Box<Fight>,
        foe: usize,
        kind: Kind,
    },
    Reward {
        foe: usize,
        parts: Vec<usize>,
        acorns: i32,
    },
    Encounter {
        index: usize,
        lines: Vec<String>,
        pending: Option<usize>,
    },
    Shop,
    Rest,
    /// a part that needs a slot: replace one, or let it go
    Graft {
        part: usize,
        back: Back,
    },
    Dead,
    Won,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub map: Map,
    pub at: Option<usize>,
    pub path: Vec<usize>,
    pub me: Creature,
    pub starter: usize,
    pub hp: i32,
    pub acorns: i32,
    pub stage: Stage,
    pub shop: Option<Shop>,
    pub battles_won: i32,
    rng: Rng,
    deck: Vec<usize>,
    drawn: usize,
}

/// The encounter deck: the good and the bad each shuffled, then dealt in
/// pairs of one of each in a random order, so every two encounters drawn
/// from the start hold one good and one bad.
pub fn deal_encounters(c: &Content, rng: &mut Rng) -> Vec<usize> {
    let shuffle = |mut v: Vec<usize>, rng: &mut Rng| {
        for i in (1..v.len()).rev() {
            let j = rng.range(0, i as i32) as usize;
            v.swap(i, j);
        }
        v
    };
    let good: Vec<usize> = (0..c.encounters.len())
        .filter(|&i| c.encounters[i].good)
        .collect();
    let bad: Vec<usize> = (0..c.encounters.len())
        .filter(|&i| !c.encounters[i].good)
        .collect();
    let (good, bad) = (shuffle(good, rng), shuffle(bad, rng));
    let mut deck = Vec::new();
    for (g, b) in good.iter().zip(bad.iter()) {
        if rng.chance(50) {
            deck.extend([*g, *b]);
        } else {
            deck.extend([*b, *g]);
        }
    }
    deck
}

impl Run {
    pub fn new(c: &Content, starter: usize, seed: u64) -> Run {
        let mut rng = Rng::new(seed);
        let map = Map::generate(&c.rules, &mut rng);
        let deck = deal_encounters(c, &mut rng);
        let def = &c.creatures[c.creature_index(&c.rules.starters[starter]).unwrap_or(0)];
        let me = Creature::from_def(c, def);
        let hp = me.max_hp(c);
        Run {
            map,
            at: None,
            path: Vec::new(),
            me,
            starter,
            hp,
            acorns: c.rules.start_acorns,
            stage: Stage::Map,
            shop: None,
            battles_won: 0,
            rng,
            deck,
            drawn: 0,
        }
    }

    pub fn max_hp(&self, c: &Content) -> i32 {
        self.me.max_hp(c)
    }

    pub fn floor(&self) -> usize {
        self.path.len()
    }

    /// After the parts change: health rises with a higher maximum and is
    /// kept inside the new one.
    fn after_change(&mut self, c: &Content, old_max: i32) {
        let new_max = self.max_hp(c);
        if new_max > old_max {
            self.hp += new_max - old_max;
        }
        self.hp = self.hp.clamp(1, new_max);
    }

    pub fn choices(&self) -> Vec<usize> {
        if self.stage != Stage::Map {
            return Vec::new();
        }
        self.map.choices(self.at)
    }

    /// Steps onto a node the path allows. False if it does not.
    pub fn pick(&mut self, c: &Content, node: usize) -> bool {
        if !self.choices().contains(&node) {
            return false;
        }
        self.at = Some(node);
        self.path.push(node);
        let n = self.map.nodes[node].clone();
        self.stage = match n.kind {
            Kind::Battle | Kind::Elite | Kind::Boss => self.start_battle(c, n.floor, n.kind),
            Kind::Encounter => self.draw_encounter(c),
            Kind::Shop => {
                self.shop = Some(self.stock_shop(c, n.floor));
                Stage::Shop
            }
            Kind::Rest => Stage::Rest,
        };
        true
    }

    fn start_battle(&mut self, c: &Content, floor: usize, kind: Kind) -> Stage {
        let foe = if kind == Kind::Boss {
            c.creature_index(&c.rules.boss).unwrap_or(0)
        } else {
            let mut tier = c.rules.tiers[floor];
            if kind == Kind::Elite {
                tier = (tier + 1).min(3);
            }
            let pool: Vec<usize> = (0..c.creatures.len())
                .filter(|&i| c.creatures[i].tier == tier)
                .collect();
            pool[self.rng.range(0, pool.len() as i32 - 1) as usize]
        };
        let pct = if kind == Kind::Elite {
            c.rules.elite_hp_pct
        } else {
            100
        };
        let foe_body = Creature::from_def(c, &c.creatures[foe]);
        let seed = self.rng.next();
        Stage::Battle {
            fight: Box::new(Fight::new(c, &self.me, self.hp, &foe_body, pct, seed)),
            foe,
            kind,
        }
    }

    /// Once the fight is over: a win pays acorns and offers a reward (or ends
    /// the run, after the boss); a loss ends the run.
    pub fn conclude(&mut self, c: &Content) -> bool {
        let Stage::Battle { fight, foe, kind } = &self.stage else {
            return false;
        };
        let Some(winner) = fight.winner else {
            return false;
        };
        let (foe, kind) = (*foe, *kind);
        if winner != PLAYER {
            self.hp = 0;
            self.stage = Stage::Dead;
            return true;
        }
        self.hp = fight.sides[PLAYER].hp.clamp(1, self.max_hp(c));
        self.battles_won += 1;
        if kind == Kind::Boss {
            self.stage = Stage::Won;
            return true;
        }
        let [lo, hi] = if kind == Kind::Elite {
            c.rules.elite_acorns
        } else {
            c.rules.battle_acorns
        };
        let acorns = self.rng.range(lo, hi);
        self.acorns += acorns;
        let parts = Creature::from_def(c, &c.creatures[foe]).parts;
        self.stage = Stage::Reward { foe, parts, acorns };
        true
    }

    pub fn reward_hp(&mut self, c: &Content) -> bool {
        if !matches!(self.stage, Stage::Reward { .. }) {
            return false;
        }
        self.me.base_hp += c.rules.max_hp_reward;
        self.hp += c.rules.max_hp_reward;
        self.hp = self.hp.min(self.max_hp(c));
        self.stage = Stage::Map;
        true
    }

    /// Adopts the slain creature's part at `i`. With no empty slot, the
    /// graft screen asks which part it replaces.
    pub fn reward_part(&mut self, c: &Content, i: usize) -> bool {
        let Stage::Reward { parts, .. } = &self.stage else {
            return false;
        };
        let Some(&part) = parts.get(i) else {
            return false;
        };
        self.offer_part(c, part, Back::Map);
        true
    }

    pub fn reward_skip(&mut self) -> bool {
        if !matches!(self.stage, Stage::Reward { .. }) {
            return false;
        }
        self.stage = Stage::Map;
        true
    }

    fn back_to(&mut self, back: Back) {
        self.stage = match back {
            Back::Map => Stage::Map,
            Back::Shop => Stage::Shop,
        };
    }

    /// Grafts a part if there is room, else asks; then returns to `back`.
    fn offer_part(&mut self, c: &Content, part: usize, back: Back) {
        let old = self.max_hp(c);
        if self.me.graft(part) {
            self.after_change(c, old);
            self.back_to(back);
        } else {
            self.stage = Stage::Graft { part, back };
        }
    }

    pub fn graft_replace(&mut self, c: &Content, slot: usize) -> bool {
        let Stage::Graft { part, back } = self.stage else {
            return false;
        };
        let old = self.max_hp(c);
        if !self.me.replace(slot, part) {
            return false;
        }
        self.after_change(c, old);
        self.back_to(back);
        true
    }

    pub fn graft_decline(&mut self) -> bool {
        let Stage::Graft { back, .. } = self.stage else {
            return false;
        };
        self.back_to(back);
        true
    }

    fn random_part(&mut self, c: &Content, max_tier: i32) -> usize {
        let pool: Vec<usize> = (0..c.parts.len())
            .filter(|&p| c.parts[p].tier >= 1 && c.parts[p].tier <= max_tier.max(1))
            .collect();
        pool[self.rng.range(0, pool.len() as i32 - 1) as usize]
    }

    fn draw_encounter(&mut self, c: &Content) -> Stage {
        let index = self.deck[self.drawn % self.deck.len()];
        self.drawn += 1;
        let mut lines = Vec::new();
        let mut pending = None;
        let old = self.max_hp(c);
        for o in c.encounters[index].outcomes.clone() {
            match o {
                Outcome::Mass { amount } if amount > 0 => {
                    for _ in 0..amount {
                        if self.me.grow(c) {
                            lines.push(c.word("out_mass_up"));
                        } else {
                            lines.push(c.word("out_mass_capped"));
                        }
                    }
                }
                Outcome::Mass { amount } => {
                    for _ in 0..(-amount) {
                        let before = self.me.mass;
                        match self.me.shrink() {
                            Some(p) => lines.push(fill(
                                &c.word("out_mass_down_part"),
                                &[("part", c.parts[p].name.clone())],
                            )),
                            None if self.me.mass < before => lines.push(c.word("out_mass_down")),
                            None => lines.push(c.word("out_mass_floor")),
                        }
                    }
                }
                Outcome::GainPart { tier } => {
                    let part = self.random_part(c, tier);
                    let name = c.parts[part].name.clone();
                    if self.me.graft(part) {
                        lines.push(fill(&c.word("out_gain_part"), &[("part", name)]));
                    } else {
                        lines.push(fill(&c.word("out_gain_part_full"), &[("part", name)]));
                        pending = Some(part);
                    }
                }
                Outcome::LosePart => {
                    if self.me.parts.is_empty() {
                        lines.push(c.word("out_lose_none"));
                    } else {
                        let slot = self.rng.range(0, self.me.parts.len() as i32 - 1) as usize;
                        let p = self.me.remove(slot).unwrap_or(0);
                        lines.push(fill(
                            &c.word("out_lose_part"),
                            &[("part", c.parts[p].name.clone())],
                        ));
                    }
                }
                Outcome::MaxHp { amount } => {
                    self.me.base_hp = (self.me.base_hp + amount).max(1);
                    let key = if amount >= 0 {
                        "out_max_hp_up"
                    } else {
                        "out_max_hp_down"
                    };
                    lines.push(fill(&c.word(key), &[("n", amount.abs().to_string())]));
                }
                Outcome::Heal { amount } => {
                    self.hp += amount;
                    lines.push(fill(&c.word("out_heal"), &[("n", amount.to_string())]));
                }
                Outcome::HealFull => {
                    self.hp = self.max_hp(c);
                    lines.push(c.word("out_heal_full"));
                }
                Outcome::Hurt { amount } => {
                    // an encounter wounds but never kills
                    let dealt = amount.min(self.hp - 1).max(0);
                    self.hp -= dealt;
                    lines.push(fill(&c.word("out_hurt"), &[("n", dealt.to_string())]));
                }
                Outcome::Acorns { amount } => {
                    let before = self.acorns;
                    self.acorns = (self.acorns + amount).max(0);
                    let key = if amount >= 0 {
                        "out_acorns_up"
                    } else {
                        "out_acorns_down"
                    };
                    lines.push(fill(
                        &c.word(key),
                        &[("n", (self.acorns - before).abs().to_string())],
                    ));
                }
            }
        }
        // max health may have moved either way; a lower maximum takes health with it
        let new_max = self.max_hp(c);
        if new_max < old {
            self.hp = self.hp.min(new_max);
        }
        self.hp = self.hp.clamp(1, new_max);
        Stage::Encounter {
            index,
            lines,
            pending,
        }
    }

    pub fn encounter_done(&mut self, c: &Content) -> bool {
        let Stage::Encounter { pending, .. } = &self.stage else {
            return false;
        };
        match *pending {
            Some(part) => self.offer_part(c, part, Back::Map),
            None => self.stage = Stage::Map,
        }
        true
    }

    fn stock_shop(&mut self, c: &Content, floor: usize) -> Shop {
        let tier = c.rules.tiers[floor].min(3);
        let s = &c.rules.shop;
        let mut offers: Vec<Offer> = Vec::new();
        let mut tries = 0;
        while offers.len() < s.offers && tries < 100 {
            tries += 1;
            let part = self.random_part(c, tier);
            if offers.iter().any(|o| o.part == part) {
                continue;
            }
            let price = self.rng.range(s.part_price[0], s.part_price[1]) + (c.parts[part].tier - 1) * 5;
            offers.push(Offer {
                part,
                price,
                sold: false,
            });
        }
        Shop {
            offers,
            mass_bought: false,
            shed_used: false,
        }
    }

    fn in_shop(&self) -> bool {
        self.stage == Stage::Shop && self.shop.is_some()
    }

    pub fn shop_buy(&mut self, c: &Content, i: usize) -> bool {
        if !self.in_shop() {
            return false;
        }
        let Some(o) = self.shop.as_ref().and_then(|s| s.offers.get(i)).cloned() else {
            return false;
        };
        if o.sold || self.acorns < o.price {
            return false;
        }
        self.acorns -= o.price;
        if let Some(s) = self.shop.as_mut() {
            s.offers[i].sold = true;
        }
        self.offer_part(c, o.part, Back::Shop);
        true
    }

    pub fn shop_mass(&mut self, c: &Content) -> bool {
        let price = c.rules.shop.mass_price;
        let bought = self.shop.as_ref().map(|s| s.mass_bought).unwrap_or(true);
        if !self.in_shop() || bought || self.acorns < price || self.me.mass >= c.rules.max_mass {
            return false;
        }
        self.acorns -= price;
        self.me.grow(c);
        if let Some(s) = self.shop.as_mut() {
            s.mass_bought = true;
        }
        true
    }

    pub fn shop_heal(&mut self, c: &Content) -> bool {
        let s = &c.rules.shop;
        if !self.in_shop() || self.acorns < s.heal_price || self.hp >= self.max_hp(c) {
            return false;
        }
        self.acorns -= s.heal_price;
        self.hp = (self.hp + s.heal_amount).min(self.max_hp(c));
        true
    }

    pub fn shop_shed(&mut self, c: &Content, slot: usize) -> bool {
        let price = c.rules.shop.shed_price;
        let used = self.shop.as_ref().map(|s| s.shed_used).unwrap_or(true);
        if !self.in_shop() || used || self.acorns < price || slot >= self.me.parts.len() {
            return false;
        }
        let old = self.max_hp(c);
        self.acorns -= price;
        self.me.remove(slot);
        self.after_change(c, old);
        if let Some(s) = self.shop.as_mut() {
            s.shed_used = true;
        }
        true
    }

    pub fn shop_leave(&mut self) -> bool {
        if !self.in_shop() {
            return false;
        }
        self.stage = Stage::Map;
        true
    }

    pub fn rest_heal(&mut self, c: &Content) -> bool {
        if self.stage != Stage::Rest {
            return false;
        }
        let max = self.max_hp(c);
        self.hp = (self.hp + max * c.rules.rest_heal_pct / 100).min(max);
        self.stage = Stage::Map;
        true
    }

    /// The rest's other choice: one more unit of mass, and no healing.
    pub fn rest_grow(&mut self, c: &Content) -> bool {
        if self.stage != Stage::Rest || self.me.mass >= c.rules.max_mass {
            return false;
        }
        self.me.grow(c);
        self.stage = Stage::Map;
        true
    }

    pub fn fight_mut(&mut self) -> Option<&mut Fight> {
        match &mut self.stage {
            Stage::Battle { fight, .. } => Some(fight),
            _ => None,
        }
    }

    pub fn fight(&self) -> Option<&Fight> {
        match &self.stage {
            Stage::Battle { fight, .. } => Some(fight),
            _ => None,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::content::tests::content;

    #[test]
    fn a_run_starts_on_the_map_with_a_three_mass_starter_at_full_health() {
        let c = content();
        let r = Run::new(&c, 0, 1);
        assert_eq!(r.stage, Stage::Map);
        assert_eq!(r.me.mass, 3);
        assert_eq!(r.hp, r.max_hp(&c));
        assert_eq!(r.choices(), r.map.floors[0]);
    }

    #[test]
    fn a_node_off_the_path_is_refused() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        let far = r.map.floors[3][0];
        assert!(!r.pick(&c, far));
        assert_eq!(r.at, None);
    }

    #[test]
    fn winning_a_battle_pays_and_offers_health_or_one_of_the_foes_parts() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        let first = r.choices()[0];
        assert!(r.pick(&c, first));
        let f = r.fight_mut().unwrap();
        f.sides[1].hp = 0;
        f.step(&c);
        assert!(r.conclude(&c));
        let Stage::Reward { parts, foe, .. } = r.stage.clone() else {
            panic!("no reward: {:?}", r.stage)
        };
        assert_eq!(parts.len(), c.creatures[foe].parts.len());
        let max = r.max_hp(&c);
        assert!(r.reward_hp(&c));
        assert_eq!(r.max_hp(&c), max + c.rules.max_hp_reward);
        assert_eq!(r.stage, Stage::Map);
    }

    #[test]
    fn adopting_a_part_with_room_grafts_it_and_without_room_asks_which_to_replace() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        let teeth = c.part_index("stinger").unwrap();
        r.stage = Stage::Reward {
            foe: 0,
            parts: vec![teeth],
            acorns: 0,
        };
        let n = r.me.parts.len();
        assert!(r.me.has_room());
        assert!(r.reward_part(&c, 0));
        assert_eq!(r.me.parts.len(), n + 1);
        // now full: three parts on three mass
        r.stage = Stage::Reward {
            foe: 0,
            parts: vec![teeth],
            acorns: 0,
        };
        assert!(r.reward_part(&c, 0));
        assert!(matches!(r.stage, Stage::Graft { .. }));
        let first = r.me.parts[0];
        assert!(r.graft_replace(&c, 0));
        assert_eq!(r.me.parts[0], teeth);
        assert_ne!(first, teeth);
        assert_eq!(r.stage, Stage::Map);
    }

    #[test]
    fn a_loss_ends_the_run() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        let first = r.choices()[0];
        r.pick(&c, first);
        let f = r.fight_mut().unwrap();
        f.sides[0].hp = 0;
        f.step(&c);
        r.conclude(&c);
        assert_eq!(r.stage, Stage::Dead);
    }

    #[test]
    fn encounters_come_half_good_and_half_bad() {
        let c = content();
        let good = c.encounters.iter().filter(|e| e.good).count();
        assert_eq!(good * 2, c.encounters.len(), "the list is half and half");
        for seed in 0..50 {
            let deck = deal_encounters(&c, &mut Rng::new(seed));
            assert_eq!(deck.len(), c.encounters.len());
            for pair in deck.chunks(2) {
                assert_ne!(c.encounters[pair[0]].good, c.encounters[pair[1]].good);
            }
        }
    }

    #[test]
    fn every_encounter_applies_without_killing() {
        let c = content();
        for i in 0..c.encounters.len() {
            let mut r = Run::new(&c, 0, 1);
            r.hp = 1;
            r.deck = vec![i];
            let s = r.draw_encounter(&c);
            assert!(r.hp >= 1, "{}", c.encounters[i].id);
            let Stage::Encounter { lines, .. } = s else {
                panic!()
            };
            assert!(!lines.is_empty());
            for l in lines {
                assert!(!l.contains('{'), "unfilled: {l}");
            }
        }
    }

    #[test]
    fn the_shop_sells_each_offer_once_and_refuses_what_you_cannot_afford() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        r.shop = Some(r.stock_shop(&c, 2));
        r.stage = Stage::Shop;
        r.acorns = 0;
        assert!(!r.shop_buy(&c, 0));
        r.acorns = 500;
        assert!(r.shop_buy(&c, 0));
        if matches!(r.stage, Stage::Graft { .. }) {
            r.graft_decline();
        }
        assert_eq!(r.stage, Stage::Shop);
        assert!(!r.shop_buy(&c, 0), "sold");
        let mass = r.me.mass;
        assert!(r.shop_mass(&c));
        assert!(!r.shop_mass(&c), "once a visit");
        assert_eq!(r.me.mass, mass + 1);
        assert!(r.shop_leave());
        assert_eq!(r.stage, Stage::Map);
    }

    #[test]
    fn rest_heals_part_of_the_maximum() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        r.stage = Stage::Rest;
        r.hp = 1;
        assert!(r.rest_heal(&c));
        assert_eq!(r.hp, 1 + r.max_hp(&c) * c.rules.rest_heal_pct / 100);
    }

    /// A player who always takes the first way on, fights everything out,
    /// takes health after each win and rests to heal: every run ends, in
    /// victory or death, on the path.
    fn autoplay(c: &Content, seed: u64) -> Stage {
        let mut r = Run::new(c, (seed % 3) as usize, seed);
        for _ in 0..200 {
            match r.stage.clone() {
                Stage::Map => {
                    let k = r.choices()[0];
                    r.pick(c, k);
                }
                Stage::Battle { .. } => {
                    r.fight_mut().unwrap().finish(c);
                    r.conclude(c);
                }
                Stage::Reward { parts, .. } => {
                    if seed.is_multiple_of(2) {
                        r.reward_hp(c);
                    } else {
                        r.reward_part(c, parts.len() - 1);
                    }
                }
                Stage::Encounter { .. } => {
                    r.encounter_done(c);
                }
                Stage::Shop => {
                    r.shop_heal(c);
                    r.shop_leave();
                }
                Stage::Rest => {
                    r.rest_heal(c);
                }
                Stage::Graft { .. } => {
                    r.graft_replace(c, 0);
                }
                s @ (Stage::Dead | Stage::Won) => return s,
            }
        }
        panic!("seed {seed}: the run never ended")
    }

    #[test]
    fn every_run_ends() {
        let c = content();
        let mut won = 0;
        for seed in 0..60 {
            if autoplay(&c, seed) == Stage::Won {
                won += 1;
            }
        }
        // a careless player wins now and then and loses more often
        assert!(won >= 1, "an autoplayer never won ({won}/60)");
        assert!(won <= 45, "an autoplayer won too often ({won}/60)");
    }
}
