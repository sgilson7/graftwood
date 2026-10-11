//! The data files, parsed and checked. Every part, creature, encounter,
//! balance number, colour and word comes from `data/`; this module turns the
//! JSON into typed values and refuses a file that names something missing.

use serde::Deserialize;
use std::collections::BTreeMap;

use crate::fx::ONE;
use crate::mesh::Mesh;

/// A colour as Q16 red, green, blue.
pub type Rgb = [i32; 3];

/// What a part does when its timer runs out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Melee,
    Ranged,
    Poison,
    Evade,
    Heal,
    Guard,
    Drain,
}

impl ActionKind {
    /// Whether the action is aimed at the foe and so can be dodged.
    pub fn is_attack(self) -> bool {
        matches!(
            self,
            ActionKind::Melee | ActionKind::Ranged | ActionKind::Poison | ActionKind::Drain
        )
    }
    /// Whether the creature closes in to use it (a lunge, and thorns answer it).
    pub fn is_melee(self) -> bool {
        matches!(self, ActionKind::Melee | ActionKind::Drain)
    }
    pub fn key(self) -> &'static str {
        match self {
            ActionKind::Melee => "melee",
            ActionKind::Ranged => "ranged",
            ActionKind::Poison => "poison",
            ActionKind::Evade => "evade",
            ActionKind::Heal => "heal",
            ActionKind::Guard => "guard",
            ActionKind::Drain => "drain",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Action {
    pub kind: ActionKind,
    pub amount: i32,
    pub every_ms: i32,
}

/// The passive modifiers a part may carry. Each is a whole number.
/// `regen` is tenths of health a second in a fight; `toxic` poisons whatever bites;
/// `poison_less` takes stacks off each dose of poison; `head_slots` is the
/// extra slot Two-Headed grows, which only a head part can fill.
pub const MOD_NAMES: [&str; 13] = [
    "ranged_less",
    "melee_less",
    "evasion",
    "evasion_per_empty",
    "max_hp",
    "thorns",
    "haste",
    "ranged_more",
    "melee_more",
    "regen",
    "toxic",
    "poison_less",
    "head_slots",
];

/// The one modifier an upgrade leaves alone: a second head is a second head.
pub const UNSCALED_MOD: &str = "head_slots";

/// The sockets on a body a part can sit in.
pub const SOCKETS: [&str; 6] = ["mouth", "head", "back", "legs", "tail", "side"];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Prim {
    pub mesh: String,
    /// hundredths of the body's radius, from the socket
    pub at: [i32; 3],
    /// full extents, hundredths of the body's radius
    pub size: [i32; 3],
    /// degrees about x, y, z
    #[serde(default)]
    pub rot: [i32; 3],
    /// a palette name, or "body" or "accent" for the creature's own colours
    #[serde(default = "default_colour")]
    pub colour: String,
    /// drawn again on the other side of the body
    #[serde(default)]
    pub mirror: bool,
}

fn default_colour() -> String {
    "part".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Part {
    pub id: String,
    pub name: String,
    pub socket: String,
    /// the colour a primitive marked "part" takes
    pub colour: String,
    #[serde(default)]
    pub action: Option<Action>,
    #[serde(default)]
    pub mods: BTreeMap<String, i32>,
    /// a line of flavour, shown under the rules
    #[serde(default)]
    pub flavour: String,
    pub model: Vec<Prim>,
    /// 0 for parts no shop sells
    pub tier: i32,
    /// the set it counts toward (a `rules.json` set id), or "" for a wild part
    #[serde(default)]
    pub set: String,
    /// takes no slot of its own (Two-Headed)
    #[serde(default)]
    pub slotless: bool,
}

/// A number scaled to an upgrade level: each level adds `pct` percent of the
/// base, at least 1, and a positive number never falls below 1.
pub fn scaled(base: i32, level: i32, pct: i32) -> i32 {
    if base <= 0 || level == 0 {
        return base;
    }
    let step = ((base * pct + 99) / 100).max(1);
    (base + level * step).max(1)
}

impl Part {
    /// The modifier at level 0.
    pub fn modifier(&self, name: &str) -> i32 {
        self.mods.get(name).copied().unwrap_or(0)
    }

    /// The modifier at an upgrade level.
    pub fn modifier_at(&self, name: &str, level: i32, u: &UpgradeRules) -> i32 {
        let base = self.modifier(name);
        if name == UNSCALED_MOD {
            return base;
        }
        scaled(base, level, u.amount_pct)
    }

    /// The action at an upgrade level: a bigger amount and a shorter timer.
    pub fn action_at(&self, level: i32, u: &UpgradeRules) -> Option<Action> {
        self.action.as_ref().map(|a| Action {
            kind: a.kind,
            amount: scaled(a.amount, level, u.amount_pct),
            every_ms: (a.every_ms * (100 - level * u.faster_pct) / 100).max(1),
        })
    }

    /// Whether an upgrade changes anything about it.
    pub fn can_upgrade(&self) -> bool {
        self.action.is_some() || self.mods.keys().any(|k| k != UNSCALED_MOD)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreatureDef {
    pub id: String,
    pub name: String,
    /// 0 for a starter, 1 to 3 for the woods' creatures, 4 for the boss
    pub tier: i32,
    pub mass: i32,
    pub hp: i32,
    pub body: String,
    pub accent: String,
    /// body proportions, percent of the radius along x (forward), y, z
    pub shape: [i32; 3],
    pub parts: Vec<String>,
    /// a line shown before the fight, or under a starter
    #[serde(default)]
    pub blurb: String,
    /// its kind: a `rules.json` set id
    pub set: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Outcome {
    Mass {
        amount: i32,
    },
    /// a part found: from the woods' pool up to a tier, or a wild part
    GainPart {
        tier: i32,
        #[serde(default)]
        wild: bool,
    },
    LosePart,
    /// a part from the bag, if anything is in it
    LoseBagPart,
    /// the player picks one of their parts to raise a level
    UpgradePart,
    /// a random part loses a level
    DowngradePart,
    Forage {
        amount: i32,
    },
    MaxHp {
        amount: i32,
    },
    Heal {
        amount: i32,
    },
    HealFull,
    Hurt {
        amount: i32,
    },
    Acorns {
        amount: i32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Encounter {
    pub id: String,
    pub name: String,
    pub text: String,
    pub good: bool,
    pub outcomes: Vec<Outcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Weights {
    pub battle: i32,
    pub encounter: i32,
    pub shop: i32,
    pub elite: i32,
    pub rest: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ShopRules {
    pub offers: usize,
    pub part_price: [i32; 2],
    pub mass_price: i32,
    pub heal_price: i32,
    pub heal_amount: i32,
}

/// What an upgrade does: each level adds `amount_pct` percent of a part's
/// numbers (at least 1) and takes `faster_pct` percent off its timer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UpgradeRules {
    pub amount_pct: i32,
    pub faster_pct: i32,
    pub max_level: i32,
    pub min_level: i32,
}

/// The forage bar: each won battle's forage adds a roll in `per_forage`;
/// at `full` the player may cultivate one unit of mass.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ForageRules {
    pub full: i32,
    pub per_forage: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SetBonus {
    /// parts of the set on the body needed
    pub count: i32,
    pub mods: BTreeMap<String, i32>,
}

/// A kind of creature. Parts of one set on one body earn its bonus: the
/// highest bonus whose count is reached.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SetDef {
    pub id: String,
    pub colour: String,
    pub bonuses: Vec<SetBonus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CombatRules {
    pub free_mass: i32,
    pub slow_pct_per: i32,
    pub poison_every_ms: i32,
    pub poison_cap: i32,
    pub evasion_cap: i32,
    pub guard_cap: i32,
    pub fastest_ms: i32,
    pub fatigue_after_ms: i32,
    pub fatigue_every_ms: i32,
    pub fatigue_growth_ms: i32,
    pub max_fight_ms: i32,
    /// the most health regeneration can restore in a second, in whole health
    pub regen_cap: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RunRules {
    pub floors: usize,
    pub columns: i32,
    pub min_nodes: i32,
    pub max_nodes: i32,
    pub weights: Weights,
    pub elite_from_floor: usize,
    pub rest_from_floor: usize,
    /// the tier of the battles on each floor (the last floor is the boss)
    pub tiers: Vec<i32>,
    pub max_mass: i32,
    pub start_acorns: i32,
    pub max_hp_reward: i32,
    pub rest_heal_pct: i32,
    pub elite_hp_pct: i32,
    pub battle_acorns: [i32; 2],
    pub elite_acorns: [i32; 2],
    pub boss: String,
    pub starters: Vec<String>,
    pub shop: ShopRules,
    pub combat: CombatRules,
    /// how many parts the bag holds; a part in the bag does nothing
    pub bag_size: usize,
    pub upgrade: UpgradeRules,
    pub forage: ForageRules,
    pub sets: Vec<SetDef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RawPalette {
    colours: BTreeMap<String, String>,
    scene: BTreeMap<String, String>,
    light_dir: [i32; 3],
    fog_range: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    pub colours: BTreeMap<String, Rgb>,
    pub sky: Rgb,
    pub fog: Rgb,
    pub light: Rgb,
    pub ink: Rgb,
    /// a unit vector toward the sun, Q16
    pub light_dir: [i32; 3],
    /// metres, Q16
    pub fog_range: [i32; 2],
}

impl Palette {
    pub fn get(&self, name: &str) -> Rgb {
        self.colours.get(name).copied().unwrap_or([ONE; 3])
    }
}

/// `"E88A73"` to Q16 red, green, blue.
pub fn hex(s: &str) -> Result<Rgb, String> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("not a colour: {s}"));
    }
    let byte = |i: usize| i32::from_str_radix(&s[i..i + 2], 16).unwrap_or(0);
    Ok([byte(0), byte(2), byte(4)].map(|v| v * ONE / 255))
}

#[derive(Debug, Clone)]
pub struct Content {
    pub parts: Vec<Part>,
    pub creatures: Vec<CreatureDef>,
    pub encounters: Vec<Encounter>,
    pub rules: RunRules,
    pub palette: Palette,
    pub copy: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct PartsFile {
    parts: Vec<Part>,
}
#[derive(Deserialize)]
struct CreaturesFile {
    creatures: Vec<CreatureDef>,
}
#[derive(Deserialize)]
struct EncountersFile {
    encounters: Vec<Encounter>,
}

fn parse<T: for<'a> Deserialize<'a>>(name: &str, text: &str) -> Result<T, String> {
    serde_json::from_str(text).map_err(|e| format!("data/{name}.json: {e}"))
}

impl Content {
    pub fn load(
        parts: &str,
        creatures: &str,
        encounters: &str,
        rules: &str,
        palette: &str,
        copy: &str,
    ) -> Result<Content, String> {
        let parts = parse::<PartsFile>("parts", parts)?.parts;
        let creatures = parse::<CreaturesFile>("creatures", creatures)?.creatures;
        let encounters = parse::<EncountersFile>("encounters", encounters)?.encounters;
        let rules: RunRules = parse("rules", rules)?;
        let raw: RawPalette = parse("palette", palette)?;
        let copy: BTreeMap<String, serde_json::Value> = parse("copy", copy)?;

        let mut colours = BTreeMap::new();
        for (k, v) in &raw.colours {
            colours.insert(k.clone(), hex(v).map_err(|e| format!("palette {k}: {e}"))?);
        }
        let scene = |k: &str| -> Result<Rgb, String> {
            hex(raw.scene.get(k).ok_or(format!("palette scene lacks {k}"))?)
        };
        let [lx, ly, lz] = raw.light_dir;
        let dir = crate::geom::v3(lx * ONE / 100, ly * ONE / 100, lz * ONE / 100).norm();
        let palette = Palette {
            colours,
            sky: scene("sky")?,
            fog: scene("fog")?,
            light: scene("light")?,
            ink: scene("ink")?,
            light_dir: [dir.x, dir.y, dir.z],
            fog_range: raw.fog_range.map(|m| m * ONE),
        };
        let c = Content {
            parts,
            creatures,
            encounters,
            rules,
            palette,
            copy,
        };
        c.validate()?;
        Ok(c)
    }

    fn validate(&self) -> Result<(), String> {
        let colour_ok = |name: &str| {
            name == "body" || name == "accent" || name == "part" || self.palette.colours.contains_key(name)
        };
        for p in &self.parts {
            if !SOCKETS.contains(&p.socket.as_str()) {
                return Err(format!("part {}: no socket {}", p.id, p.socket));
            }
            if !colour_ok(&p.colour) {
                return Err(format!("part {}: no colour {}", p.id, p.colour));
            }
            for k in p.mods.keys() {
                if !MOD_NAMES.contains(&k.as_str()) {
                    return Err(format!("part {}: no modifier {k}", p.id));
                }
            }
            if let Some(a) = &p.action {
                if a.every_ms <= 0 || a.amount <= 0 {
                    return Err(format!(
                        "part {}: an action needs a positive amount and time",
                        p.id
                    ));
                }
            }
            if p.action.is_none() && p.mods.is_empty() {
                return Err(format!("part {}: does nothing", p.id));
            }
            if !p.set.is_empty() && self.set_index(&p.set).is_none() {
                return Err(format!("part {}: no set {}", p.id, p.set));
            }
            if p.slotless && p.modifier(UNSCALED_MOD) <= 0 {
                return Err(format!("part {}: a slotless part must grow a slot", p.id));
            }
            for m in &p.model {
                if Mesh::from_name(&m.mesh).is_none() {
                    return Err(format!("part {}: no mesh {}", p.id, m.mesh));
                }
                if !colour_ok(&m.colour) {
                    return Err(format!("part {}: no colour {}", p.id, m.colour));
                }
            }
            if self.parts.iter().filter(|q| q.id == p.id).count() > 1 {
                return Err(format!("part {} is listed twice", p.id));
            }
        }
        for c in &self.creatures {
            for p in &c.parts {
                if self.part_index(p).is_none() {
                    return Err(format!("creature {}: no part {p}", c.id));
                }
            }
            if !crate::creature::Creature::from_def(self, c).fits(self) {
                return Err(format!("creature {}: more parts than mass", c.id));
            }
            if self.set_index(&c.set).is_none() {
                return Err(format!("creature {}: no set {}", c.id, c.set));
            }
            for k in [&c.body, &c.accent] {
                if !self.palette.colours.contains_key(k) {
                    return Err(format!("creature {}: no colour {k}", c.id));
                }
            }
        }
        for id in self.rules.starters.iter().chain([&self.rules.boss]) {
            if self.creature_index(id).is_none() {
                return Err(format!("rules: no creature {id}"));
            }
        }
        if self.rules.tiers.len() != self.rules.floors {
            return Err("rules: one tier per floor".to_string());
        }
        for t in &self.rules.tiers[..self.rules.floors - 1] {
            if !self.creatures.iter().any(|c| c.tier == *t) {
                return Err(format!("rules: no creature of tier {t}"));
            }
        }
        for e in &self.encounters {
            if e.outcomes.is_empty() {
                return Err(format!("encounter {}: no outcome", e.id));
            }
        }
        for set in &self.rules.sets {
            if !self.palette.colours.contains_key(&set.colour) {
                return Err(format!("set {}: no colour {}", set.id, set.colour));
            }
            for b in &set.bonuses {
                for k in b.mods.keys() {
                    if !MOD_NAMES.contains(&k.as_str()) {
                        return Err(format!("set {}: no modifier {k}", set.id));
                    }
                }
            }
        }
        if self.rules.bag_size == 0 {
            return Err("rules: the bag holds at least one part".to_string());
        }
        Ok(())
    }

    pub fn set_index(&self, id: &str) -> Option<usize> {
        self.rules.sets.iter().position(|s| s.id == id)
    }

    pub fn part_index(&self, id: &str) -> Option<usize> {
        self.parts.iter().position(|p| p.id == id)
    }

    pub fn creature_index(&self, id: &str) -> Option<usize> {
        self.creatures.iter().position(|c| c.id == id)
    }

    /// A word or template from copy.json; a missing key shows as itself, and
    /// the tests require every key the core asks for to be present.
    pub fn word(&self, key: &str) -> String {
        match self.copy.get(key) {
            Some(serde_json::Value::String(s)) => s.clone(),
            _ => key.to_string(),
        }
    }
}

/// `template` with each `{name}` replaced.
pub fn fill(template: &str, pairs: &[(&str, String)]) -> String {
    let mut s = template.to_string();
    for (k, v) in pairs {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub struct Files {
        pub parts: &'static str,
        pub creatures: &'static str,
        pub encounters: &'static str,
        pub rules: &'static str,
        pub palette: &'static str,
        pub copy: &'static str,
    }

    pub fn files() -> Files {
        Files {
            parts: include_str!("../../../data/parts.json"),
            creatures: include_str!("../../../data/creatures.json"),
            encounters: include_str!("../../../data/encounters.json"),
            rules: include_str!("../../../data/rules.json"),
            palette: include_str!("../../../data/palette.json"),
            copy: include_str!("../../../data/copy.json"),
        }
    }

    pub fn content() -> Content {
        let f = files();
        Content::load(f.parts, f.creatures, f.encounters, f.rules, f.palette, f.copy).unwrap()
    }

    #[test]
    fn the_data_files_load_and_check() {
        let c = content();
        assert!(c.parts.len() >= 20);
        assert!(c.creatures.len() >= 12);
    }

    #[test]
    fn a_creature_naming_a_missing_part_is_refused() {
        let f = files();
        let bad = f.creatures.replacen("\"teeth\"", "\"no_such_part\"", 1);
        let r = Content::load(f.parts, &bad, f.encounters, f.rules, f.palette, f.copy);
        assert!(r.unwrap_err().contains("no_such_part"));
    }

    #[test]
    fn a_creature_with_more_parts_than_mass_is_refused() {
        let f = files();
        // the rabbit has three parts on three mass
        let bad = f.creatures.replacen("\"mass\": 3", "\"mass\": 1", 1);
        let r = Content::load(f.parts, &bad, f.encounters, f.rules, f.palette, f.copy);
        assert!(r.unwrap_err().contains("more parts than mass"));
    }

    #[test]
    fn the_brief_examples_are_in_the_data_as_written() {
        let c = content();
        let rabbit = &c.creatures[c.creature_index("rabbit").unwrap()];
        assert_eq!(rabbit.mass, 3);
        assert_eq!(rabbit.parts, ["bunny_ears", "teeth", "lucky_feet"]);
        let part = |id: &str| c.parts[c.part_index(id).unwrap()].clone();
        assert_eq!(part("bunny_ears").modifier("ranged_less"), 1);
        let teeth = part("teeth").action.unwrap();
        assert_eq!(
            (teeth.kind, teeth.amount, teeth.every_ms),
            (ActionKind::Melee, 2, 2000)
        );
        let feet = part("lucky_feet").action.unwrap();
        assert_eq!(
            (feet.kind, feet.amount, feet.every_ms),
            (ActionKind::Evade, 1, 5000)
        );
        let wasp = &c.creatures[c.creature_index("wasp").unwrap()];
        assert_eq!(wasp.parts, ["wings", "stinger"]);
        assert_eq!(part("wings").modifier("evasion_per_empty"), 3);
        let sting = part("stinger").action.unwrap();
        assert_eq!(
            (sting.kind, sting.amount, sting.every_ms),
            (ActionKind::Poison, 1, 4000)
        );
        assert_eq!(c.rules.combat.poison_cap, 10);
        assert_eq!(c.rules.combat.poison_every_ms, 2000);
        assert_eq!((c.rules.combat.free_mass, c.rules.combat.slow_pct_per), (5, 10));
    }

    #[test]
    fn every_starter_has_three_mass() {
        let c = content();
        for id in &c.rules.starters {
            assert_eq!(c.creatures[c.creature_index(id).unwrap()].mass, 3, "{id}");
        }
    }

    #[test]
    fn creatures_carry_two_to_four_parts() {
        let c = content();
        for k in c.creatures.iter().filter(|k| k.tier >= 1 && k.tier <= 3) {
            assert!((2..=4).contains(&k.parts.len()), "{}", k.id);
        }
    }

    #[test]
    fn every_creature_and_every_part_but_the_wild_ones_belongs_to_a_set() {
        let c = content();
        for k in &c.creatures {
            assert!(c.set_index(&k.set).is_some(), "{}", k.id);
        }
        let wild: Vec<&str> = c
            .parts
            .iter()
            .filter(|p| p.set.is_empty())
            .map(|p| p.id.as_str())
            .collect();
        let mut want = [
            "fungal_antenna",
            "third_eye",
            "opposable_thumb",
            "prehensile_tail",
            "eye_stalk",
            "fur_patch",
            "two_headed",
        ];
        let mut got = wild.clone();
        want.sort();
        got.sort();
        assert_eq!(got, want);
        let names: Vec<&str> = c.rules.sets.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(names, ["bug", "rodent", "beast", "critter"]);
    }

    #[test]
    fn the_playtest_creatures_are_in_the_woods_with_two_or_three_parts_of_their_own() {
        let c = content();
        let ids = [
            "frog",
            "salmon",
            "bear",
            "bee_swarm",
            "anthill",
            "squirrel",
            "skunk",
            "possum",
            "hawk",
            "turkey",
            "deer",
            "jackrabbit",
            "unicorn",
            "dragon_hatchling",
            "snake",
            "snail",
            "turtle",
            "duck",
            "swan",
        ];
        for id in ids {
            let k = &c.creatures[c.creature_index(id).unwrap_or_else(|| panic!("no {id}"))];
            assert!((1..=3).contains(&k.tier), "{id} is met in the woods");
            // a part of its own: carried by no other creature, and not wild
            let own = k
                .parts
                .iter()
                .filter(|p| !c.parts[c.part_index(p).unwrap()].set.is_empty())
                .filter(|p| c.creatures.iter().filter(|o| o.parts.contains(p)).count() == 1)
                .count();
            assert!((2..=3).contains(&own), "{id} has {own} parts of its own");
        }
    }

    #[test]
    fn an_upgrade_raises_a_parts_numbers_and_shortens_its_timer() {
        let c = content();
        let u = &c.rules.upgrade;
        let teeth = &c.parts[c.part_index("teeth").unwrap()];
        let up = teeth.action_at(1, u).unwrap();
        // 2 damage: half of 2 is 1, so 3; 2 s less a tenth is 1.8 s
        assert_eq!((up.amount, up.every_ms), (3, 1800));
        let down = teeth.action_at(-1, u).unwrap();
        assert_eq!((down.amount, down.every_ms), (1, 2200));
        let fur = &c.parts[c.part_index("thick_fur").unwrap()];
        assert_eq!(fur.modifier_at("max_hp", 2, u), 12);
        let two = &c.parts[c.part_index("two_headed").unwrap()];
        assert_eq!(two.modifier_at("head_slots", 3, u), 1, "a head is a head");
        assert!(!two.can_upgrade());
        assert_eq!(scaled(1, -1, 50), 1, "a positive number stays at least 1");
    }

    #[test]
    fn hex_reads_colours() {
        assert_eq!(hex("FF0000").unwrap(), [ONE, 0, 0]);
        assert!(hex("nope").is_err());
    }
}
