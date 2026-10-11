//! One run: the map, the creature you have grown, its health and acorns, and
//! where on the path you stand. Every choice the player makes between fights
//! is a method here, and each one refuses what the rules do not allow.

use crate::combat::{Fight, PLAYER};
use crate::content::{fill, Content, Outcome};
use crate::creature::{Creature, Graft};
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
}

/// Where a body part can be picked up from or put down: a slot on the body,
/// the body as a whole (the first slot it fits), a place in the bag, a part
/// on offer (a reward, the Trader's stock, a find), the bin, or the old
/// grafter's stump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Body(usize),
    Creature,
    Bag(usize),
    Offer(usize),
    Bin,
    Upgrade,
}

impl Place {
    /// The page names a place by a word and a number.
    pub fn parse(kind: &str, i: usize) -> Option<Place> {
        match kind {
            "body" => Some(Place::Body(i)),
            "creature" => Some(Place::Creature),
            "bag" => Some(Place::Bag(i)),
            "offer" => Some(Place::Offer(i)),
            "bin" => Some(Place::Bin),
            "upgrade" => Some(Place::Upgrade),
            _ => None,
        }
    }
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
        /// what this win's forage added, once the player has foraged
        foraged: Option<i32>,
        /// the health or the part has been taken
        taken: bool,
    },
    Encounter {
        index: usize,
        lines: Vec<String>,
        /// a part found and not yet placed
        found: Option<Graft>,
        /// an upgrade waiting for the player to choose a part
        upgrade: bool,
    },
    Shop,
    Rest,
    Dead,
    Won,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub map: Map,
    pub at: Option<usize>,
    pub path: Vec<usize>,
    pub me: Creature,
    /// parts carried but not grafted: they do nothing in a fight
    pub bag: Vec<Graft>,
    pub starter: usize,
    pub hp: i32,
    pub acorns: i32,
    /// the forage bar, from 0 to the rules' `full`
    pub forage: i32,
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
            bag: Vec::new(),
            starter,
            hp,
            acorns: c.rules.start_acorns,
            forage: 0,
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
        let parts = Creature::from_def(c, &c.creatures[foe])
            .parts
            .iter()
            .map(|g| g.part)
            .collect();
        self.stage = Stage::Reward {
            foe,
            parts,
            acorns,
            foraged: None,
            taken: false,
        };
        true
    }

    /// Raises max health: one of the two rewards for a win.
    pub fn reward_hp(&mut self, c: &Content) -> bool {
        let Stage::Reward { taken, .. } = &mut self.stage else {
            return false;
        };
        if *taken {
            return false;
        }
        *taken = true;
        self.me.base_hp += c.rules.max_hp_reward;
        self.hp += c.rules.max_hp_reward;
        self.hp = self.hp.min(self.max_hp(c));
        true
    }

    /// Takes the slain creature's part at `i`: grafted if it fits, else
    /// into the bag.
    pub fn reward_part(&mut self, c: &Content, i: usize) -> bool {
        matches!(self.stage, Stage::Reward { .. }) && self.take(c, i)
    }

    /// Forages once after a win: the bar fills by a roll, up to full.
    pub fn reward_forage(&mut self, c: &Content) -> bool {
        let Stage::Reward { foraged: None, .. } = self.stage else {
            return false;
        };
        let [lo, hi] = c.rules.forage.per_forage;
        let found = self.rng.range(lo, hi);
        let before = self.forage;
        self.forage = (self.forage + found).min(c.rules.forage.full);
        if let Stage::Reward { foraged, .. } = &mut self.stage {
            *foraged = Some(self.forage - before);
        }
        true
    }

    /// Leaves the reward, taken or not.
    pub fn reward_done(&mut self) -> bool {
        if !matches!(self.stage, Stage::Reward { .. }) {
            return false;
        }
        self.stage = Stage::Map;
        true
    }

    pub fn can_cultivate(&self, c: &Content) -> bool {
        self.can_edit() && self.forage >= c.rules.forage.full && self.me.mass < c.rules.max_mass
    }

    /// Spends a full forage bar on one unit of mass.
    pub fn cultivate(&mut self, c: &Content) -> bool {
        if !self.can_cultivate(c) {
            return false;
        }
        self.forage -= c.rules.forage.full;
        self.me.grow(c);
        true
    }

    /// Whether parts can be moved about: on every screen between fights.
    pub fn can_edit(&self) -> bool {
        matches!(
            self.stage,
            Stage::Map | Stage::Reward { .. } | Stage::Encounter { .. } | Stage::Shop | Stage::Rest
        )
    }

    pub fn bag_has_room(&self, c: &Content) -> bool {
        self.bag.len() < c.rules.bag_size
    }

    /// The part on offer at `i`, if it may be taken now: a reward not yet
    /// chosen, a find not yet placed, or stock not sold and affordable.
    pub fn offer(&self, i: usize) -> Option<Graft> {
        match &self.stage {
            Stage::Reward {
                parts, taken: false, ..
            } => parts.get(i).map(|&p| Graft::new(p)),
            Stage::Encounter { found: Some(g), .. } if i == 0 => Some(*g),
            Stage::Shop => {
                let o = self.shop.as_ref()?.offers.get(i)?;
                (!o.sold && self.acorns >= o.price).then_some(Graft::new(o.part))
            }
            _ => None,
        }
    }

    /// Marks the offer at `i` as taken, paying for it at the Trader.
    fn spend(&mut self, i: usize) {
        match &mut self.stage {
            Stage::Reward { taken, .. } => *taken = true,
            Stage::Encounter { found, .. } => *found = None,
            Stage::Shop => {
                if let Some(o) = self.shop.as_mut().and_then(|s| s.offers.get_mut(i)) {
                    o.sold = true;
                    self.acorns -= o.price;
                }
            }
            _ => {}
        }
    }

    /// Takes offer `i` the plain way: onto the body if it fits, else into
    /// the bag. False if neither has room.
    pub fn take(&mut self, c: &Content, i: usize) -> bool {
        self.move_part(c, Place::Offer(i), Place::Creature)
            || self.move_part(c, Place::Offer(i), Place::Bag(self.bag.len()))
    }

    /// Whether a move would be allowed, without making it.
    pub fn can_move(&self, c: &Content, from: Place, to: Place) -> bool {
        self.clone().move_part(c, from, to)
    }

    /// Moves a part from one place to another, if the rules allow it:
    ///
    /// - onto the body: into a free slot, or onto one of its parts to swap
    ///   (the part it replaces goes where the new one came from, or into
    ///   the bag when it came from an offer);
    /// - into the bag: into a free place, or onto a part there to swap;
    /// - into the bin: gone;
    /// - onto the grafter's stump, when an encounter offers it: up a level.
    ///
    /// Nothing is ever lost by a move but what goes in the bin: a move that
    /// would leave a part without a slot is refused.
    pub fn move_part(&mut self, c: &Content, from: Place, to: Place) -> bool {
        if !self.can_edit() {
            return false;
        }
        let old = self.max_hp(c);
        let before = (self.me.clone(), self.bag.clone());
        let ok = self.try_move(c, from, to) && self.me.fits(c) && self.bag.len() <= c.rules.bag_size;
        if !ok {
            (self.me, self.bag) = before;
            return false;
        }
        if let Place::Offer(i) = from {
            self.spend(i);
        }
        self.after_change(c, old);
        true
    }

    fn try_move(&mut self, c: &Content, from: Place, to: Place) -> bool {
        let body = self.me.parts.len();
        let bag = self.bag.len();
        match (from, to) {
            (Place::Body(i), Place::Body(j)) if i < body && j < body && i != j => {
                self.me.parts.swap(i, j);
                true
            }
            (Place::Body(i), Place::Bag(j)) if i < body && j < bag => {
                std::mem::swap(&mut self.me.parts[i], &mut self.bag[j]);
                true
            }
            (Place::Body(i), Place::Bag(_)) if i < body && self.bag_has_room(c) => {
                let g = self.me.parts.remove(i);
                self.bag.push(g);
                true
            }
            (Place::Body(i), Place::Bin) if i < body => {
                self.me.parts.remove(i);
                true
            }
            (Place::Bag(i), Place::Creature) if i < bag => {
                let g = self.bag.remove(i);
                self.me.parts.push(g);
                true
            }
            (Place::Bag(i), Place::Body(j)) if i < bag && j < body => {
                std::mem::swap(&mut self.bag[i], &mut self.me.parts[j]);
                true
            }
            (Place::Bag(i), Place::Bag(j)) if i < bag && j < bag && i != j => {
                self.bag.swap(i, j);
                true
            }
            (Place::Bag(i), Place::Bin) if i < bag => {
                self.bag.remove(i);
                true
            }
            (Place::Offer(i), Place::Creature) => match self.offer(i) {
                Some(g) => {
                    self.me.parts.push(g);
                    true
                }
                None => false,
            },
            (Place::Offer(i), Place::Bag(_)) => match self.offer(i) {
                Some(g) if self.bag_has_room(c) => {
                    self.bag.push(g);
                    true
                }
                _ => false,
            },
            (Place::Offer(i), Place::Body(j)) if j < body => match self.offer(i) {
                Some(g) if self.bag_has_room(c) => {
                    let old = std::mem::replace(&mut self.me.parts[j], g);
                    self.bag.push(old);
                    true
                }
                _ => false,
            },
            (Place::Body(i), Place::Upgrade) if i < body => {
                let g = self.me.parts[i];
                self.upgrade(c, g).map(|up| self.me.parts[i] = up).is_some()
            }
            (Place::Bag(i), Place::Upgrade) if i < bag => {
                let g = self.bag[i];
                self.upgrade(c, g).map(|up| self.bag[i] = up).is_some()
            }
            _ => false,
        }
    }

    /// Spends an encounter's upgrade on `g`, if one is waiting and `g` can
    /// rise. Returns the part a level up.
    fn upgrade(&mut self, c: &Content, g: Graft) -> Option<Graft> {
        let Stage::Encounter { upgrade, .. } = &mut self.stage else {
            return None;
        };
        if !*upgrade || !c.parts[g.part].can_upgrade() || g.level >= c.rules.upgrade.max_level {
            return None;
        }
        *upgrade = false;
        Some(Graft {
            part: g.part,
            level: g.level + 1,
        })
    }

    /// A part's name, with its level when it has one.
    pub fn graft_name(c: &Content, g: Graft) -> String {
        let name = c.parts[g.part].name.clone();
        match g.level {
            0 => name,
            n if n > 0 => fill(&c.word("level_up"), &[("part", name), ("n", n.to_string())]),
            n => fill(&c.word("level_down"), &[("part", name), ("n", (-n).to_string())]),
        }
    }

    fn random_part(&mut self, c: &Content, max_tier: i32, wild: bool) -> usize {
        let pool: Vec<usize> = (0..c.parts.len())
            .filter(|&p| {
                let part = &c.parts[p];
                if wild {
                    part.set.is_empty()
                } else {
                    part.tier >= 1 && part.tier <= max_tier.max(1)
                }
            })
            .collect();
        pool[self.rng.range(0, pool.len() as i32 - 1) as usize]
    }

    fn draw_encounter(&mut self, c: &Content) -> Stage {
        let index = self.deck[self.drawn % self.deck.len()];
        self.drawn += 1;
        let mut lines = Vec::new();
        let mut found = None;
        let mut upgrade = false;
        let old = self.max_hp(c);
        let named = |key: &str, g: Graft| fill(&c.word(key), &[("part", Run::graft_name(c, g))]);
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
                        let fell = self.me.shrink(c);
                        if fell.is_empty() && self.me.mass < before {
                            lines.push(c.word("out_mass_down"));
                        } else if fell.is_empty() {
                            lines.push(c.word("out_mass_floor"));
                        }
                        for g in fell {
                            lines.push(named("out_mass_down_part", g));
                        }
                    }
                }
                Outcome::GainPart { tier, wild } => {
                    let g = Graft::new(self.random_part(c, tier, wild));
                    lines.push(named("out_gain_part", g));
                    found = Some(g);
                }
                Outcome::LosePart => {
                    if self.me.parts.is_empty() {
                        lines.push(c.word("out_lose_none"));
                    } else {
                        let slot = self.rng.range(0, self.me.parts.len() as i32 - 1) as usize;
                        if let Some((g, fell)) = self.me.remove(c, slot) {
                            lines.push(named("out_lose_part", g));
                            for f in fell {
                                lines.push(named("out_falls_off", f));
                            }
                        }
                    }
                }
                Outcome::LoseBagPart => {
                    if self.bag.is_empty() {
                        lines.push(c.word("out_lose_bag_none"));
                    } else {
                        let i = self.rng.range(0, self.bag.len() as i32 - 1) as usize;
                        let g = self.bag.remove(i);
                        lines.push(named("out_lose_bag_part", g));
                    }
                }
                Outcome::UpgradePart => {
                    let max = c.rules.upgrade.max_level;
                    let any = self
                        .me
                        .parts
                        .iter()
                        .chain(self.bag.iter())
                        .any(|g| c.parts[g.part].can_upgrade() && g.level < max);
                    upgrade = any;
                    lines.push(c.word(if any { "out_upgrade" } else { "out_upgrade_none" }));
                }
                Outcome::DowngradePart => {
                    let min = c.rules.upgrade.min_level;
                    let slots: Vec<usize> = (0..self.me.parts.len())
                        .filter(|&i| {
                            let g = self.me.parts[i];
                            c.parts[g.part].can_upgrade() && g.level > min
                        })
                        .collect();
                    if slots.is_empty() {
                        lines.push(c.word("out_downgrade_none"));
                    } else {
                        let i = slots[self.rng.range(0, slots.len() as i32 - 1) as usize];
                        let was = self.me.parts[i];
                        self.me.parts[i].level -= 1;
                        lines.push(fill(
                            &c.word("out_downgrade"),
                            &[
                                ("part", Run::graft_name(c, was)),
                                ("now", Run::graft_name(c, self.me.parts[i])),
                            ],
                        ));
                    }
                }
                Outcome::Forage { amount } => {
                    let before = self.forage;
                    self.forage = (self.forage + amount).clamp(0, c.rules.forage.full);
                    let key = if amount >= 0 {
                        "out_forage_up"
                    } else {
                        "out_forage_down"
                    };
                    lines.push(fill(
                        &c.word(key),
                        &[("n", (self.forage - before).abs().to_string())],
                    ));
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
            found,
            upgrade,
        }
    }

    /// Walks on from an encounter. A find not placed and an upgrade not
    /// spent are left behind.
    pub fn encounter_done(&mut self) -> bool {
        if !matches!(self.stage, Stage::Encounter { .. }) {
            return false;
        }
        self.stage = Stage::Map;
        true
    }

    /// Takes the encounter's find the plain way.
    pub fn encounter_take(&mut self, c: &Content) -> bool {
        matches!(self.stage, Stage::Encounter { .. }) && self.take(c, 0)
    }

    fn stock_shop(&mut self, c: &Content, floor: usize) -> Shop {
        let tier = c.rules.tiers[floor].min(3);
        let s = &c.rules.shop;
        let mut offers: Vec<Offer> = Vec::new();
        let mut tries = 0;
        while offers.len() < s.offers && tries < 100 {
            tries += 1;
            let part = self.random_part(c, tier, false);
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
        }
    }

    fn in_shop(&self) -> bool {
        self.stage == Stage::Shop && self.shop.is_some()
    }

    /// Buys the Trader's offer at `i` the plain way.
    pub fn shop_buy(&mut self, c: &Content, i: usize) -> bool {
        self.in_shop() && self.take(c, i)
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

    fn part(c: &Content, id: &str) -> Graft {
        Graft::new(c.part_index(id).unwrap())
    }

    fn reward(parts: Vec<usize>) -> Stage {
        Stage::Reward {
            foe: 0,
            parts,
            acorns: 0,
            foraged: None,
            taken: false,
        }
    }

    #[test]
    fn a_run_starts_on_the_map_with_a_three_mass_starter_at_full_health() {
        let c = content();
        let r = Run::new(&c, 0, 1);
        assert_eq!(r.stage, Stage::Map);
        assert_eq!(r.me.mass, 3);
        assert_eq!(r.hp, r.max_hp(&c));
        assert_eq!(r.choices(), r.map.floors[0]);
        assert!(r.bag.is_empty());
        assert_eq!(r.forage, 0);
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
        assert!(!r.reward_hp(&c), "one reward a win");
        assert!(!r.reward_part(&c, 0), "health or a part, not both");
        assert!(r.reward_done());
        assert_eq!(r.stage, Stage::Map);
    }

    #[test]
    fn a_rewarded_part_is_grafted_if_it_fits_and_bagged_if_not() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        let stinger = c.part_index("stinger").unwrap();
        r.stage = reward(vec![stinger]);
        let n = r.me.parts.len();
        assert!(r.reward_part(&c, 0));
        assert_eq!(r.me.parts.len(), n + 1);
        // now full: three parts on three mass
        r.stage = reward(vec![stinger]);
        assert!(r.reward_part(&c, 0));
        assert_eq!(r.me.parts.len(), n + 1);
        assert_eq!(r.bag, vec![Graft::new(stinger)]);
    }

    #[test]
    fn the_bag_holds_two_parts_and_they_do_nothing_in_a_fight() {
        let c = content();
        assert_eq!(c.rules.bag_size, 2);
        let mut r = Run::new(&c, 0, 1);
        let evasion = r.me.base_evasion(&c);
        r.bag = vec![part(&c, "eye_stalk"), part(&c, "third_eye")];
        assert_eq!(r.me.base_evasion(&c), evasion, "bagged parts change nothing");
        let fight = Fight::new(&c, &r.me, r.hp, &r.me.clone(), 100, 1);
        assert_eq!(fight.sides[PLAYER].creature.parts.len(), r.me.parts.len());
        // full: a part from the body has nowhere to go
        assert!(!r.move_part(&c, Place::Body(0), Place::Bag(2)));
        // but a swap is fine
        let first = r.me.parts[0];
        assert!(r.move_part(&c, Place::Body(0), Place::Bag(1)));
        assert_eq!(r.bag[1], first);
    }

    #[test]
    fn parts_move_between_the_body_the_bag_and_the_bin() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        let teeth = r.me.parts[0];
        // off the body into the bag, and back on
        assert!(r.move_part(&c, Place::Body(0), Place::Bag(0)));
        assert_eq!((r.me.parts.len(), r.bag.clone()), (1, vec![teeth]));
        assert!(r.move_part(&c, Place::Bag(0), Place::Creature));
        assert_eq!(r.me.parts.last(), Some(&teeth));
        assert!(r.bag.is_empty());
        // a full body takes nothing more from the bag, but swaps
        r.me.parts.push(part(&c, "antennae"));
        r.bag.push(part(&c, "stinger"));
        assert!(!r.move_part(&c, Place::Bag(0), Place::Creature));
        assert!(r.move_part(&c, Place::Bag(0), Place::Body(2)));
        assert_eq!(r.me.parts[2], part(&c, "stinger"));
        assert_eq!(r.bag, vec![part(&c, "antennae")]);
        // the bin takes anything for good
        assert!(r.move_part(&c, Place::Bag(0), Place::Bin));
        assert!(r.bag.is_empty());
        // nothing moves in a fight
        let first = r.choices()[0];
        r.pick(&c, first);
        assert!(!r.move_part(&c, Place::Body(0), Place::Bag(0)));
    }

    #[test]
    fn a_move_that_would_leave_a_part_without_a_slot_is_refused() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        r.me.parts = vec![part(&c, "teeth"), part(&c, "thick_fur"), part(&c, "lucky_feet")];
        r.me.parts.push(part(&c, "two_headed"));
        r.me.parts.push(part(&c, "antennae"));
        assert!(r.me.fits(&c));
        // the second head holds the antennae up
        assert!(!r.move_part(&c, Place::Body(3), Place::Bin));
        assert!(!r.move_part(&c, Place::Body(3), Place::Bag(0)));
        assert!(r.move_part(&c, Place::Body(4), Place::Bag(0)));
        assert!(r.move_part(&c, Place::Body(3), Place::Bag(1)));
    }

    #[test]
    fn health_follows_a_part_that_raises_it_on_and_off() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        // the Burrowling's Thick Fur: +6
        let full = r.max_hp(&c);
        assert!(r.move_part(&c, Place::Body(1), Place::Bag(0)));
        assert_eq!((r.max_hp(&c), r.hp), (full - 6, full - 6));
        assert!(r.move_part(&c, Place::Bag(0), Place::Creature));
        assert_eq!((r.max_hp(&c), r.hp), (full, full));
    }

    #[test]
    fn foraging_after_wins_fills_the_bar_and_a_full_bar_cultivates_mass() {
        let c = content();
        let f = &c.rules.forage;
        let mut r = Run::new(&c, 0, 1);
        assert!(!r.reward_forage(&c), "only after a win");
        let mut wins = 0;
        while r.forage < f.full {
            r.stage = reward(vec![]);
            assert!(r.reward_forage(&c));
            assert!(!r.reward_forage(&c), "once a win");
            r.reward_done();
            wins += 1;
        }
        assert!((3..=4).contains(&wins), "a full bar every 3 or 4 wins: {wins}");
        let mass = r.me.mass;
        assert!(r.cultivate(&c));
        assert_eq!(r.me.mass, mass + 1);
        assert!(r.forage < f.full);
        assert!(!r.cultivate(&c));
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

    fn encounter(c: &Content, r: &mut Run, id: &str) -> Stage {
        let i = c.encounters.iter().position(|e| e.id == id).unwrap();
        r.deck = vec![i];
        r.drawn = 0;
        let s = r.draw_encounter(c);
        r.stage = s.clone();
        s
    }

    #[test]
    fn every_encounter_applies_without_killing() {
        let c = content();
        for e in &c.encounters {
            let mut r = Run::new(&c, 0, 1);
            r.hp = 1;
            let s = encounter(&c, &mut r, &e.id);
            assert!(r.hp >= 1, "{}", e.id);
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
    fn the_old_grafter_upgrades_the_part_the_player_chooses() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        let s = encounter(&c, &mut r, "old_grafter");
        assert!(matches!(s, Stage::Encounter { upgrade: true, .. }));
        assert!(r.move_part(&c, Place::Body(0), Place::Upgrade));
        assert_eq!(r.me.parts[0].level, 1);
        assert!(!r.move_part(&c, Place::Body(1), Place::Upgrade), "one upgrade");
        // an upgrade can go to a part in the bag too
        let mut r = Run::new(&c, 0, 1);
        r.bag.push(part(&c, "stinger"));
        encounter(&c, &mut r, "old_grafter");
        assert!(r.move_part(&c, Place::Bag(0), Place::Upgrade));
        assert_eq!(r.bag[0].level, 1);
    }

    #[test]
    fn blight_takes_a_level_off_a_part() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        encounter(&c, &mut r, "blight");
        let levels: Vec<i32> = r.me.parts.iter().map(|g| g.level).collect();
        assert_eq!(levels.iter().sum::<i32>(), -1, "{levels:?}");
    }

    #[test]
    fn a_find_waits_to_be_placed_and_is_left_behind_if_not() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        let s = encounter(&c, &mut r, "strange_spores");
        let Stage::Encounter { found: Some(g), .. } = s else {
            panic!("nothing found")
        };
        assert!(c.parts[g.part].set.is_empty(), "spores grow a wild part");
        assert!(r.move_part(&c, Place::Offer(0), Place::Bag(0)));
        assert_eq!(r.bag, vec![g]);
        assert!(!r.move_part(&c, Place::Offer(0), Place::Bag(1)), "taken once");
        let mut r = Run::new(&c, 0, 1);
        encounter(&c, &mut r, "strange_spores");
        assert!(r.encounter_done());
        assert!(r.bag.is_empty());
    }

    #[test]
    fn the_packrat_takes_from_the_bag() {
        let c = content();
        let mut r = Run::new(&c, 0, 1);
        r.bag = vec![part(&c, "stinger")];
        encounter(&c, &mut r, "packrat");
        assert!(r.bag.is_empty());
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
        let price = r.shop.as_ref().unwrap().offers[0].price;
        assert!(r.shop_buy(&c, 0));
        assert_eq!(r.acorns, 500 - price);
        assert_eq!(r.stage, Stage::Shop);
        assert!(!r.shop_buy(&c, 0), "sold");
        let mass = r.me.mass;
        let acorns = r.acorns;
        assert!(r.shop_mass(&c));
        assert!(!r.shop_mass(&c), "once a visit");
        assert_eq!(r.me.mass, mass + 1);
        assert_eq!(acorns - r.acorns, 20, "a unit of mass costs 20 acorns");
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
    /// forages, takes health or a part after each win and rests to heal:
    /// every run ends, in victory or death, on the path.
    fn autoplay(c: &Content, seed: u64) -> Stage {
        let mut r = Run::new(c, (seed % 3) as usize, seed);
        for _ in 0..300 {
            r.cultivate(c);
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
                    r.reward_forage(c);
                    if seed.is_multiple_of(2) {
                        r.reward_hp(c);
                    } else {
                        r.reward_part(c, parts.len() - 1);
                        r.move_part(c, Place::Bag(0), Place::Body(0));
                    }
                    r.reward_done();
                }
                Stage::Encounter { .. } => {
                    r.encounter_take(c);
                    r.move_part(c, Place::Body(0), Place::Upgrade);
                    r.encounter_done();
                }
                Stage::Shop => {
                    r.shop_heal(c);
                    r.shop_leave();
                }
                Stage::Rest => {
                    r.rest_heal(c);
                }
                s @ (Stage::Dead | Stage::Won) => return s,
            }
            assert!(r.me.fits(c), "seed {seed}: a part without a slot");
            assert!(r.bag.len() <= c.rules.bag_size);
        }
        panic!("seed {seed}: the run never ended")
    }

    #[test]
    fn every_run_ends() {
        let c = content();
        let mut won = 0;
        for seed in 0..200 {
            if autoplay(&c, seed) == Stage::Won {
                won += 1;
            }
        }
        // a careless player wins now and then and loses more often
        assert!(won >= 1, "an autoplayer never won ({won}/200)");
        assert!(won <= 100, "an autoplayer won too often ({won}/200)");
    }
}
