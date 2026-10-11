//! The game: which screen is showing, the clock, the camera, and everything
//! the page reads (the 3D draws, the HUD, the screen's contents and the sounds
//! to play), as values and JSON. Each button the page shows is one method
//! here, which hands the choice to the run and refuses what it refuses.

use serde_json::{json, Value};

use crate::combat::{period_ticks, Ev, Fight, FOE, PLAYER, TICKS_PER_SECOND};
use crate::content::{fill, ActionKind, Content, Part};
use crate::creature::Creature;
use crate::fx::{self, deg, int, ONE};
use crate::geom::{v3, V3};
use crate::mesh::{self, Mesh};
use crate::render::{self, Batch, Camera, DYNAMIC_BUFFER, STATIC_BUFFER};
use crate::run::{Run, Stage};
use crate::scene;

/// The page's clock is in microseconds; a frame longer than this (a hidden
/// tab) counts as this long.
const MAX_FRAME_US: i64 = 100_000;
const TICK_UNITS: i64 = 1_000_000;
/// Fight speeds the player can pick.
pub const SPEEDS: [i32; 3] = [1, 2, 4];
/// The drawing buffer's most pixels, so a large screen stays smooth.
const MAX_PIXELS: i64 = 2_600_000;
/// How long a number floats over a creature, in ticks.
const FLOAT_TICKS: i32 = 54;
/// The seed of the woods behind the title.
const TITLE_SEED: u64 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Title,
    Choose,
    Run,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    pub runs: i32,
    pub wins: i32,
    pub best_floor: i32,
}

pub struct Game {
    pub content: Content,
    pub mode: Mode,
    pub run: Option<Run>,
    pub preview: usize,
    pub record: Record,
    pub speed: i32,
    /// the last fight, kept for the picture after it ends
    last_fight: Option<Fight>,
    last_us: Option<i64>,
    clock_us: i64,
    acc: i64,
    anim: i32,
    css: (i32, i32),
    pixels: (i32, i32),
    scene_seed: u64,
    scene_version: u32,
    static_values: Vec<i32>,
    static_draws: Vec<Vec<u32>>,
    values: Vec<i32>,
    draws: Vec<u32>,
    camera: Camera,
    sounds: Vec<&'static str>,
}

fn sound_of(ev: Ev) -> &'static str {
    match ev {
        Ev::Hit(ActionKind::Ranged) => "thwack",
        Ev::Hit(_) => "bite",
        Ev::Dodge => "dodge",
        Ev::Poisoned => "sting",
        Ev::PoisonTick => "poison",
        Ev::Heal => "heal",
        Ev::Guard | Ev::Blocked => "guard",
        Ev::Evade => "hop",
        Ev::Thorns => "thorn",
        Ev::Fatigue => "fatigue",
    }
}

/// A Q16 percentage as CSS: "37.50%".
fn css_pct(q: i32) -> String {
    let hundredths = (q as i64 * 100 / ONE as i64) as i32;
    format!("{}.{:02}%", hundredths / 100, hundredths.rem_euclid(100))
}

/// Ticks as seconds to a tenth: "2.5".
pub fn secs(ticks: i32) -> String {
    let tenths = (ticks * 10 + TICKS_PER_SECOND / 2) / TICKS_PER_SECOND;
    format!("{}.{}", tenths / 10, tenths % 10)
}

impl Game {
    pub fn new(
        parts: &str,
        creatures: &str,
        encounters: &str,
        rules: &str,
        palette: &str,
        copy: &str,
        saved: &str,
    ) -> Result<Game, String> {
        let content = Content::load(parts, creatures, encounters, rules, palette, copy)?;
        let record = serde_json::from_str::<Value>(saved)
            .map(|v| Record {
                runs: v["runs"].as_i64().unwrap_or(0) as i32,
                wins: v["wins"].as_i64().unwrap_or(0) as i32,
                best_floor: v["best_floor"].as_i64().unwrap_or(0) as i32,
            })
            .unwrap_or_default();
        let ink = content.palette.ink;
        let mut g = Game {
            content,
            mode: Mode::Title,
            run: None,
            preview: 0,
            record,
            speed: 1,
            last_fight: None,
            last_us: None,
            clock_us: 0,
            acc: 0,
            anim: 0,
            css: (1, 1),
            pixels: (1, 1),
            scene_seed: u64::MAX,
            scene_version: 0,
            static_values: Vec::new(),
            static_draws: Vec::new(),
            values: Vec::new(),
            draws: Vec::new(),
            camera: Camera::looking_at(v3(0, int(3), int(9)), V3::ZERO, deg(50)),
            sounds: Vec::new(),
        };
        let _ = ink;
        g.draw();
        Ok(g)
    }

    // ---- the clock ----------------------------------------------------

    /// One display frame: the page's clock in microseconds.
    pub fn advance(&mut self, now_us: i64) {
        let dt = match self.last_us {
            Some(last) => (now_us - last).clamp(0, MAX_FRAME_US),
            None => 0,
        };
        self.last_us = Some(now_us);
        self.clock_us += dt;
        self.acc += dt * TICKS_PER_SECOND as i64;
        let mut frames = 0;
        while self.acc >= TICK_UNITS {
            self.acc -= TICK_UNITS;
            frames += 1;
        }
        self.anim = (self.anim + frames) % (TICKS_PER_SECOND * 3600);
        let speed = self.speed;
        let c = &self.content;
        if let Some(f) = self.run.as_mut().and_then(|r| r.fight_mut()) {
            let before = f.tick;
            let was_over = f.over();
            for _ in 0..frames * speed {
                f.step(c);
            }
            // after the end, the picture keeps moving at its own pace
            if f.over() {
                f.tick += if was_over { frames } else { 0 };
            }
            for e in f.events.iter().filter(|e| e.tick > before) {
                let s = sound_of(e.ev);
                if !self.sounds.contains(&s) {
                    self.sounds.push(s);
                }
            }
            if f.over() && !was_over {
                self.sounds
                    .push(if f.winner == Some(PLAYER) { "win" } else { "lose" });
            }
        }
        self.draw();
    }

    pub fn resize(&mut self, css_w: i32, css_h: i32, dpr_pct: i32) -> [i32; 2] {
        let (cw, ch) = (css_w.max(1), css_h.max(1));
        let mut w = cw as i64 * dpr_pct.clamp(25, 300) as i64 / 100;
        let mut h = ch as i64 * dpr_pct.clamp(25, 300) as i64 / 100;
        if w * h > MAX_PIXELS {
            let k = fx::isqrt((MAX_PIXELS * 1_000_000 / (w * h)) as u64) as i64;
            w = w * k / 1000;
            h = h * k / 1000;
        }
        self.css = (cw, ch);
        self.pixels = (w.max(1) as i32, h.max(1) as i32);
        self.draw();
        [self.pixels.0, self.pixels.1]
    }

    fn aspect(&self) -> i32 {
        fx::div(self.css.0 * ONE, self.css.1.max(1) * ONE)
    }

    // ---- the buttons --------------------------------------------------

    pub fn begin(&mut self) -> bool {
        if self.mode != Mode::Title {
            return false;
        }
        self.mode = Mode::Choose;
        self.sounds.push("click");
        true
    }

    pub fn set_preview(&mut self, i: usize) -> bool {
        if self.mode != Mode::Choose || i >= self.content.rules.starters.len() {
            return false;
        }
        self.preview = i;
        true
    }

    /// Starts a run with starter `i`. The seed comes from the page's clock, so
    /// every run differs while the same inputs still give the same run.
    pub fn choose(&mut self, i: usize) -> bool {
        if self.mode != Mode::Choose || i >= self.content.rules.starters.len() {
            return false;
        }
        let seed = (self.clock_us as u64) ^ ((self.record.runs as u64) << 40) ^ 0x0067_7261_6674;
        self.start_with_seed(i, seed);
        true
    }

    pub fn start_with_seed(&mut self, i: usize, seed: u64) {
        self.run = Some(Run::new(&self.content, i, seed));
        self.mode = Mode::Run;
        self.last_fight = None;
        self.record.runs += 1;
        self.sounds.push("click");
    }

    fn with_run(&mut self, f: impl FnOnce(&mut Run, &Content) -> bool, sound: &'static str) -> bool {
        let Some(run) = self.run.as_mut() else {
            return false;
        };
        let ok = f(run, &self.content);
        if ok {
            self.sounds.push(sound);
        }
        ok
    }

    pub fn pick(&mut self, node: usize) -> bool {
        let ok = self.with_run(|r, c| r.pick(c, node), "step");
        if ok {
            self.last_fight = None;
            self.speed = self.speed.clamp(1, 4);
        }
        ok
    }

    pub fn set_speed(&mut self, n: i32) -> bool {
        if !SPEEDS.contains(&n) {
            return false;
        }
        self.speed = n;
        true
    }

    /// Runs the fight to its end at once.
    pub fn skip(&mut self) -> bool {
        let c = &self.content;
        let Some(f) = self.run.as_mut().and_then(|r| r.fight_mut()) else {
            return false;
        };
        if f.over() {
            return false;
        }
        f.finish(c);
        self.sounds
            .push(if f.winner == Some(PLAYER) { "win" } else { "lose" });
        true
    }

    /// After a fight has ended: on to the reward, or the end of the run.
    pub fn conclude(&mut self) -> bool {
        let fight = self.run.as_ref().and_then(|r| r.fight()).cloned();
        let ok = self.with_run(|r, c| r.conclude(c), "click");
        if ok {
            self.last_fight = fight;
            self.settle_record();
        }
        ok
    }

    fn settle_record(&mut self) {
        let Some(r) = &self.run else {
            return;
        };
        self.record.best_floor = self.record.best_floor.max(r.floor() as i32);
        if r.stage == Stage::Won {
            self.record.wins += 1;
        }
    }

    pub fn reward_hp(&mut self) -> bool {
        self.with_run(|r, c| r.reward_hp(c), "heal")
    }
    pub fn reward_part(&mut self, i: usize) -> bool {
        self.with_run(|r, c| r.reward_part(c, i), "graft")
    }
    pub fn reward_skip(&mut self) -> bool {
        self.with_run(|r, _| r.reward_skip(), "click")
    }
    pub fn encounter_done(&mut self) -> bool {
        self.with_run(|r, c| r.encounter_done(c), "click")
    }
    pub fn shop_buy(&mut self, i: usize) -> bool {
        self.with_run(|r, c| r.shop_buy(c, i), "buy")
    }
    pub fn shop_mass(&mut self) -> bool {
        self.with_run(|r, c| r.shop_mass(c), "buy")
    }
    pub fn shop_heal(&mut self) -> bool {
        self.with_run(|r, c| r.shop_heal(c), "heal")
    }
    pub fn shop_shed(&mut self, i: usize) -> bool {
        self.with_run(|r, c| r.shop_shed(c, i), "buy")
    }
    pub fn shop_leave(&mut self) -> bool {
        self.with_run(|r, _| r.shop_leave(), "click")
    }
    pub fn rest_heal(&mut self) -> bool {
        self.with_run(|r, c| r.rest_heal(c), "heal")
    }
    pub fn rest_grow(&mut self) -> bool {
        self.with_run(|r, c| r.rest_grow(c), "graft")
    }
    pub fn graft_replace(&mut self, i: usize) -> bool {
        self.with_run(|r, c| r.graft_replace(c, i), "graft")
    }
    pub fn graft_decline(&mut self) -> bool {
        self.with_run(|r, _| r.graft_decline(), "click")
    }

    /// Gives up the run: it counts as a loss.
    pub fn abandon(&mut self) -> bool {
        let Some(r) = self.run.as_mut() else {
            return false;
        };
        if matches!(r.stage, Stage::Dead | Stage::Won) {
            return false;
        }
        r.stage = Stage::Dead;
        r.hp = 0;
        self.settle_record();
        true
    }

    pub fn to_title(&mut self) -> bool {
        let ended = self
            .run
            .as_ref()
            .is_none_or(|r| matches!(r.stage, Stage::Dead | Stage::Won));
        if !ended {
            return false;
        }
        self.run = None;
        self.last_fight = None;
        self.mode = Mode::Title;
        true
    }

    pub fn saved(&self) -> String {
        json!({
            "runs": self.record.runs,
            "wins": self.record.wins,
            "best_floor": self.record.best_floor,
        })
        .to_string()
    }

    pub fn take_sounds(&mut self) -> String {
        let s = json!(self.sounds).to_string();
        self.sounds.clear();
        s
    }

    // ---- what the page reads -----------------------------------------

    pub fn mode_key(&self) -> &'static str {
        match (self.mode, self.run.as_ref().map(|r| &r.stage)) {
            (Mode::Title, _) => "title",
            (Mode::Choose, _) => "choose",
            (_, Some(Stage::Map)) => "map",
            (_, Some(Stage::Battle { .. })) => "battle",
            (_, Some(Stage::Reward { .. })) => "reward",
            (_, Some(Stage::Encounter { .. })) => "encounter",
            (_, Some(Stage::Shop)) => "shop",
            (_, Some(Stage::Rest)) => "rest",
            (_, Some(Stage::Graft { .. })) => "graft",
            (_, Some(Stage::Dead)) => "dead",
            (_, Some(Stage::Won)) => "won",
            (_, None) => "title",
        }
    }

    fn word(&self, key: &str) -> String {
        self.content.word(key)
    }

    /// One part as the page shows it: name, socket, its rules in words with
    /// times for a creature this slowed and hasted, and its flavour.
    fn card(&self, p: usize, slow: i32, haste: i32) -> Value {
        let c = &self.content;
        let part: &Part = &c.parts[p];
        let mut rules = Vec::new();
        if let Some(a) = &part.action {
            let t = period_ticks(c, a.every_ms, slow, haste);
            let poison = crate::combat::ms_to_ticks(c.rules.combat.poison_every_ms);
            rules.push(fill(
                &self.word(&format!("act_{}", a.kind.key())),
                &[
                    ("n", a.amount.to_string()),
                    ("s", secs(t)),
                    ("p", secs(poison)),
                    ("cap", c.rules.combat.poison_cap.to_string()),
                ],
            ));
        }
        for (k, v) in &part.mods {
            rules.push(fill(&self.word(&format!("mod_{k}")), &[("n", v.to_string())]));
        }
        json!({
            "id": part.id,
            "name": part.name,
            "socket": self.word(&format!("socket_{}", part.socket)),
            "kind": part.action.as_ref().map(|a| a.kind.key()).unwrap_or("passive"),
            "rules": rules,
            "flavour": part.flavour,
        })
    }

    fn creature_json(&self, k: &Creature, hp: i32) -> Value {
        let c = &self.content;
        let slow = k.slow_pct(c);
        let haste = k.sum(c, "haste");
        json!({
            "name": k.name,
            "hp": hp,
            "max_hp": k.max_hp(c),
            "fill": format!("{}%", hp.max(0) * 100 / k.max_hp(c).max(1)),
            "mass": k.mass,
            "used": k.parts.len(),
            "empty": k.empty_slots(),
            "slow": slow,
            "evasion": k.base_evasion(c).min(c.rules.combat.evasion_cap),
            "parts": k.parts.iter().map(|&p| self.card(p, slow, haste)).collect::<Vec<_>>(),
            "slots": (0..k.mass).map(|i| i < k.parts.len() as i32).collect::<Vec<_>>(),
        })
    }

    fn me_json(&self, r: &Run) -> Value {
        let mut v = self.creature_json(&r.me, r.hp);
        v["acorns"] = json!(r.acorns);
        v["floor"] = json!(r.floor());
        v["floors"] = json!(self.content.rules.floors);
        v
    }

    fn map_json(&self, r: &Run) -> Value {
        let rules = &self.content.rules;
        let choices = r.choices();
        let floors = rules.floors as i32;
        let place = |k: usize| {
            let n = &r.map.nodes[k];
            let x = (n.col * 2 + 1) * ONE * 100 / (rules.columns * 2);
            let y = (94 - n.floor as i32 * 88 / (floors - 1)) * ONE;
            (x, y)
        };
        let nodes: Vec<Value> = r
            .map
            .nodes
            .iter()
            .enumerate()
            .map(|(k, n)| {
                let state = if r.at == Some(k) {
                    "here"
                } else if r.path.contains(&k) {
                    "visited"
                } else if choices.contains(&k) {
                    "open"
                } else if (n.floor as i32) < r.floor() as i32 {
                    "missed"
                } else {
                    "closed"
                };
                let (x, y) = place(k);
                json!({
                    "id": k,
                    "kind": n.kind.key(),
                    "label": self.word(&format!("node_{}", n.kind.key())),
                    "x": css_pct(x),
                    "y": css_pct(y),
                    "state": state,
                })
            })
            .collect();
        let mut edges = Vec::new();
        for (k, n) in r.map.nodes.iter().enumerate() {
            for &t in &n.next {
                let taken = r.path.windows(2).any(|w| w[0] == k && w[1] == t);
                let state = if taken {
                    "taken"
                } else if r.at == Some(k) && r.stage == Stage::Map {
                    "open"
                } else {
                    "plain"
                };
                let ((x1, y1), (x2, y2)) = (place(k), place(t));
                edges.push(json!({
                    "x1": css_pct(x1).trim_end_matches('%'),
                    "y1": css_pct(y1).trim_end_matches('%'),
                    "x2": css_pct(x2).trim_end_matches('%'),
                    "y2": css_pct(y2).trim_end_matches('%'),
                    "state": state,
                }));
            }
        }
        json!({ "nodes": nodes, "edges": edges })
    }

    /// Everything the current screen shows.
    pub fn screen_json(&self) -> String {
        let c = &self.content;
        let mode = self.mode_key();
        let mut out = json!({ "mode": mode });
        let record = json!({
            "runs": self.record.runs,
            "wins": self.record.wins,
            "best": self.record.best_floor,
            "line": fill(&self.word("record_line"), &[
                ("runs", self.record.runs.to_string()),
                ("wins", self.record.wins.to_string()),
                ("best", self.record.best_floor.to_string()),
            ]),
        });
        out["record"] = record;
        if self.mode == Mode::Choose {
            let starters: Vec<Value> = c
                .rules
                .starters
                .iter()
                .filter_map(|id| c.creature_index(id))
                .map(|i| {
                    let d = &c.creatures[i];
                    let k = Creature::from_def(c, d);
                    let mut v = self.creature_json(&k, k.max_hp(c));
                    v["blurb"] = json!(d.blurb);
                    v
                })
                .collect();
            out["starters"] = json!(starters);
            out["preview"] = json!(self.preview);
            return out.to_string();
        }
        let Some(r) = &self.run else {
            return out.to_string();
        };
        out["me"] = self.me_json(r);
        out["map"] = self.map_json(r);
        match &r.stage {
            Stage::Battle { foe, kind, .. } => {
                let d = &c.creatures[*foe];
                out["foe"] = json!({
                    "name": d.name,
                    "blurb": d.blurb,
                    "kind": self.word(&format!("node_{}", kind.key())),
                });
            }
            Stage::Reward { foe, parts, acorns } => {
                let d = &c.creatures[*foe];
                let slow = r.me.slow_pct(c);
                let haste = r.me.sum(c, "haste");
                out["reward"] = json!({
                    "heading": fill(&self.word("reward_heading"), &[("foe", d.name.clone())]),
                    "acorns": fill(&self.word("reward_acorns"), &[("n", acorns.to_string())]),
                    "hp": fill(&self.word("reward_hp"), &[("n", c.rules.max_hp_reward.to_string())]),
                    "room": if r.me.has_room() { self.word("reward_room") } else { self.word("reward_full") },
                    "parts": parts.iter().map(|&p| self.card(p, slow, haste)).collect::<Vec<_>>(),
                });
            }
            Stage::Encounter {
                index,
                lines,
                pending,
            } => {
                let e = &c.encounters[*index];
                out["encounter"] = json!({
                    "name": e.name,
                    "text": e.text,
                    "good": e.good,
                    "lines": lines,
                    "button": if pending.is_some() { self.word("encounter_graft") } else { self.word("encounter_on") },
                });
            }
            Stage::Shop => {
                let s = r.shop.as_ref();
                let rules = &c.rules.shop;
                let slow = r.me.slow_pct(c);
                let haste = r.me.sum(c, "haste");
                let offers: Vec<Value> = s
                    .map(|s| {
                        s.offers
                            .iter()
                            .map(|o| {
                                let mut v = self.card(o.part, slow, haste);
                                v["price"] = json!(o.price);
                                v["sold"] = json!(o.sold);
                                v["can"] = json!(!o.sold && r.acorns >= o.price);
                                v
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let mass_bought = s.map(|s| s.mass_bought).unwrap_or(true);
                let shed_used = s.map(|s| s.shed_used).unwrap_or(true);
                out["shop"] = json!({
                    "offers": offers,
                    "mass": {
                        "label": fill(&self.word("shop_mass"), &[("n", rules.mass_price.to_string())]),
                        "can": !mass_bought && r.acorns >= rules.mass_price && r.me.mass < c.rules.max_mass,
                    },
                    "heal": {
                        "label": fill(&self.word("shop_heal"), &[("n", rules.heal_price.to_string()), ("hp", rules.heal_amount.to_string())]),
                        "can": r.acorns >= rules.heal_price && r.hp < r.max_hp(c),
                    },
                    "shed": {
                        "label": fill(&self.word("shop_shed"), &[("n", rules.shed_price.to_string())]),
                        "can": !shed_used && r.acorns >= rules.shed_price && !r.me.parts.is_empty(),
                    },
                });
            }
            Stage::Rest => {
                let max = r.max_hp(c);
                let heal = (max * c.rules.rest_heal_pct / 100).min(max - r.hp);
                out["rest"] = json!({
                    "heal": fill(&self.word("rest_heal"), &[("n", heal.to_string())]),
                    "grow": self.word("rest_grow"),
                    "can_grow": r.me.mass < c.rules.max_mass,
                });
            }
            Stage::Graft { part, .. } => {
                let slow = r.me.slow_pct(c);
                let haste = r.me.sum(c, "haste");
                out["graft"] = json!({
                    "part": self.card(*part, slow, haste),
                    "heading": fill(&self.word("graft_heading"), &[("part", c.parts[*part].name.clone())]),
                });
            }
            Stage::Dead | Stage::Won => {
                let won = r.stage == Stage::Won;
                out["end"] = json!({
                    "heading": if won { self.word("won_heading") } else { self.word("dead_heading") },
                    "text": if won { self.word("won_text") } else { self.word("dead_text") },
                    "stats": [
                        fill(&self.word("end_floor"), &[("n", r.floor().to_string()), ("of", c.rules.floors.to_string())]),
                        fill(&self.word("end_battles"), &[("n", r.battles_won.to_string())]),
                        fill(&self.word("end_body"), &[("mass", r.me.mass.to_string()), ("parts", r.me.parts.len().to_string())]),
                    ],
                });
            }
            Stage::Map => {}
        }
        out.to_string()
    }

    fn fight_on_screen(&self) -> Option<&Fight> {
        self.run
            .as_ref()
            .and_then(|r| r.fight())
            .or(match self.mode_key() {
                "reward" | "dead" => self.last_fight.as_ref(),
                _ => None,
            })
    }

    /// The fight as the HUD shows it: both creatures' health, poison, guard,
    /// evasion and each part's countdown; numbers floating over them; and
    /// whether it is over.
    pub fn hud_json(&self) -> String {
        let c = &self.content;
        let mode = self.mode_key();
        let Some(f) = self.run.as_ref().and_then(|r| r.fight()) else {
            return json!({ "mode": mode }).to_string();
        };
        let cap = c.rules.combat.evasion_cap;
        let fighters: Vec<Value> = [PLAYER, FOE]
            .iter()
            .map(|&s| {
                let x = &f.sides[s];
                let parts: Vec<Value> = x
                    .creature
                    .parts
                    .iter()
                    .enumerate()
                    .map(|(i, &p)| {
                        let period = x.period[i];
                        let pct = if period == 0 {
                            100
                        } else {
                            (period - x.timer[i]) * 100 / period
                        };
                        json!({
                            "name": c.parts[p].name,
                            "kind": c.parts[p].action.as_ref().map(|a| a.kind.key()).unwrap_or("passive"),
                            "fill": format!("{pct}%"),
                            "secs": if period == 0 { String::new() } else { secs(x.timer[i].max(0)) },
                            "fired": f.tick - x.fired_at[i] < 12,
                        })
                    })
                    .collect();
                json!({
                    "name": x.creature.name,
                    "hp": x.hp.max(0),
                    "max": x.max_hp,
                    "fill": format!("{}%", x.hp.max(0) * 100 / x.max_hp.max(1)),
                    "poison": x.poison,
                    "guard": x.guard,
                    "evasion": x.evasion(cap),
                    "slow": x.slow_pct,
                    "mass": x.creature.mass,
                    "parts": parts,
                })
            })
            .collect();
        let aspect = self.aspect();
        let floaters: Vec<Value> = f
            .events
            .iter()
            .filter(|e| f.tick - e.tick < FLOAT_TICKS)
            .filter_map(|e| {
                let age = f.tick - e.tick;
                let (text, kind) = match e.ev {
                    Ev::Hit(_) | Ev::Thorns | Ev::Fatigue if e.amount == 0 => (self.word("float_none"), "none"),
                    Ev::Hit(_) => (fill(&self.word("float_hit"), &[("n", e.amount.to_string())]), "hit"),
                    Ev::Thorns => (fill(&self.word("float_hit"), &[("n", e.amount.to_string())]), "thorns"),
                    Ev::Fatigue => (fill(&self.word("float_hit"), &[("n", e.amount.to_string())]), "fatigue"),
                    Ev::PoisonTick => (fill(&self.word("float_hit"), &[("n", e.amount.to_string())]), "poison"),
                    Ev::Dodge => (self.word("float_dodge"), "dodge"),
                    Ev::Poisoned => (fill(&self.word("float_poisoned"), &[("n", e.amount.to_string())]), "poison"),
                    Ev::Heal => (fill(&self.word("float_heal"), &[("n", e.amount.to_string())]), "heal"),
                    Ev::Guard => (fill(&self.word("float_guard"), &[("n", e.amount.to_string())]), "guard"),
                    Ev::Blocked => (fill(&self.word("float_blocked"), &[("n", e.amount.to_string())]), "guard"),
                    Ev::Evade => (fill(&self.word("float_evade"), &[("n", e.amount.to_string())]), "evade"),
                };
                // numbers on one creature in one tick stack upward
                let stack = f
                    .events
                    .iter()
                    .filter(|o| o.tick == e.tick && o.side == e.side)
                    .position(|o| o == e)
                    .unwrap_or(0) as i32;
                let lift = age * ONE / 50 + stack * ONE * 4 / 10;
                let at = scene::head_of(f, e.side).add(v3(0, lift, 0));
                let (x, y) = self.camera.screen(aspect, at)?;
                Some(json!({ "x": css_pct(x), "y": css_pct(y), "text": text, "kind": kind, "fade": age * 100 / FLOAT_TICKS }))
            })
            .collect();
        let result = f.winner.map(|w| {
            if w == PLAYER {
                self.word("fight_won")
            } else {
                self.word("fight_lost")
            }
        });
        json!({
            "mode": mode,
            "speed": self.speed,
            "over": f.over(),
            "result": result,
            "clock": secs(f.tick.min(crate::combat::ms_to_ticks(c.rules.combat.max_fight_ms))),
            "fatigue": f.tick > crate::combat::ms_to_ticks(c.rules.combat.fatigue_after_ms),
            "fighters": fighters,
            "floaters": floaters,
        })
        .to_string()
    }

    // ---- the picture ---------------------------------------------------

    fn draw(&mut self) {
        let seed = self
            .run
            .as_ref()
            .map(|r| r.map.nodes.len() as u64 * 31 + r.path.len() as u64 / 4 + self.record.runs as u64)
            .unwrap_or(TITLE_SEED);
        if seed != self.scene_seed {
            self.scene_seed = seed;
            self.scene_version += 1;
            let (values, draws) = scene::woods(&self.content, seed).finish(STATIC_BUFFER);
            self.static_values = values;
            self.static_draws = draws;
        }
        let c = &self.content;
        let mut b = Batch::new(c.palette.ink);
        let anim = self.anim;
        let orbit = |radius: i32, height: i32, look_y: i32, speed: i32, shift: i32| {
            let a = anim * speed;
            let eye = v3(fx::mul(fx::sin(a), radius), height, fx::mul(fx::cos(a), radius));
            let target = v3(0, look_y, 0);
            // look a little to the creature's side, so it stands clear of the panel
            let fwd = target.sub(eye).norm();
            let right = fwd.cross(V3::UP).norm();
            Camera::looking_at(eye, target.sub(right.scale(shift)), deg(50))
        };
        let mode = self.mode_key();
        let fight = self.fight_on_screen().cloned();
        match (&fight, mode) {
            (Some(f), "battle" | "reward" | "dead") => {
                scene::draw_fight(&mut b, c, f, f.ended_at, anim);
                let sway = fx::sin(anim * deg(1) / 3);
                self.camera = Camera::looking_at(
                    v3(
                        fx::mul(sway, ONE / 2),
                        int(2) + ONE * 6 / 10,
                        int(7) + ONE * 8 / 10,
                    ),
                    v3(0, ONE * 9 / 10, 0),
                    deg(46),
                );
            }
            _ => {
                let me = match mode {
                    "title" => None,
                    "choose" => c
                        .rules
                        .starters
                        .get(self.preview)
                        .and_then(|id| c.creature_index(id))
                        .map(|i| (Creature::from_def(c, &c.creatures[i]), true)),
                    _ => self.run.as_ref().map(|r| (r.me.clone(), false)),
                };
                if let Some((k, _)) = &me {
                    scene::draw_creature(&mut b, c, k, V3::ZERO, 0, &scene::idle_pose(k, anim));
                    let shift = if mode == "won" { 0 } else { ONE * 15 / 10 };
                    self.camera = orbit(int(7), int(2) + ONE * 4 / 10, ONE * 85 / 100, deg(1) / 5, shift);
                } else {
                    // the title: the three starters in the clearing
                    for (i, id) in c.rules.starters.iter().enumerate() {
                        if let Some(d) = c.creature_index(id) {
                            let k = Creature::from_def(c, &c.creatures[d]);
                            let x = (i as i32 - 1) * int(2);
                            scene::draw_creature(
                                &mut b,
                                c,
                                &k,
                                v3(x, 0, 0),
                                fx::QUARTER,
                                &scene::idle_pose(&k, anim + i as i32 * 20),
                            );
                        }
                    }
                    self.camera = orbit(int(9), int(3), ONE, deg(1) / 8, 0);
                }
            }
        }
        let (values, dyn_draws) = b.finish(DYNAMIC_BUFFER);
        self.values = values;
        let mut draws = Vec::new();
        for pass in 0..render::PASSES {
            if let Some(d) = self.static_draws.get(pass) {
                draws.extend_from_slice(d);
            }
            draws.extend_from_slice(&dyn_draws[pass]);
        }
        self.draws = draws;
    }

    pub fn view(&self) -> Vec<i32> {
        self.camera.values(self.aspect())
    }
    pub fn instances(&self) -> Vec<i32> {
        self.values.clone()
    }
    pub fn static_instances(&self) -> Vec<i32> {
        self.static_values.clone()
    }
    pub fn draws(&self) -> Vec<u32> {
        self.draws.clone()
    }
    pub fn scene_version(&self) -> u32 {
        self.scene_version
    }

    /// The lighting the shader needs, in the order `numbers()` names.
    pub fn scene_uniforms(&self) -> Vec<i32> {
        let p = &self.content.palette;
        let mut v = Vec::new();
        v.extend(p.light_dir);
        v.extend(p.sky);
        v.extend(p.fog);
        v.extend(p.fog_range);
        v.extend(p.light);
        v.extend(p.ink);
        v
    }

    pub fn clear_color(&self) -> Vec<i32> {
        self.content.palette.sky.to_vec()
    }
}

/// The page's layout numbers: every size and offset its renderer needs.
pub fn numbers() -> String {
    let vertex_bytes = mesh::VERTEX_VALUES * render::BYTES_PER_VALUE;
    let instance_bytes = render::INSTANCE_VALUES * render::BYTES_PER_VALUE;
    let vec4 = 4 * render::BYTES_PER_VALUE;
    json!({
        "frac_bits": fx::FRAC_BITS,
        "meshes": Mesh::ALL.iter().map(|m| json!({
            "id": *m as u32,
            "vertices": mesh::vertices(*m).len() / mesh::VERTEX_VALUES,
        })).collect::<Vec<_>>(),
        "vertex": {
            "stride": vertex_bytes,
            "attributes": [["aPos", 3, 0], ["aNormal", 3, 3 * render::BYTES_PER_VALUE]],
        },
        "instance": {
            "stride": instance_bytes,
            "attributes": [["iA", 4, 0], ["iB", 4, vec4], ["iC", 4, 2 * vec4], ["iD", 4, 3 * vec4]],
        },
        "draw_values": render::DRAW_VALUES,
        "buffers": { "static": render::STATIC_BUFFER, "dynamic": render::DYNAMIC_BUFFER },
        "passes": { "solid": render::PASS_SOLID, "ink": render::PASS_INK, "glow": render::PASS_GLOW },
        "uniforms": [["uLightDir", 3], ["uSky", 3], ["uFog", 3], ["uFogRange", 2], ["uLight", 3], ["uInk", 3]],
        "view": { "matrix": [0, 16], "eye": [16, 19] },
        "speeds": SPEEDS,
        "ms_to_us": 1000,
        "percent": 100,
    })
    .to_string()
}

pub fn mesh_vertices(id: u32) -> Vec<i32> {
    Mesh::ALL
        .iter()
        .find(|m| **m as u32 == id)
        .map(|m| mesh::vertices(*m))
        .unwrap_or_default()
}

/// The core's white noise for the page's sound: `n` samples in [−1, 1] as
/// Q16, from a seed.
pub fn noise(n: u32, seed: u32) -> Vec<i32> {
    let mut r = crate::rng::Rng::new(seed as u64);
    (0..n.min(1 << 20)).map(|_| r.range(-ONE, ONE)).collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::content::tests::files;
    use crate::map::Kind;

    pub fn game_with(saved: &str) -> Game {
        let f = files();
        Game::new(
            f.parts,
            f.creatures,
            f.encounters,
            f.rules,
            f.palette,
            f.copy,
            saved,
        )
        .unwrap()
    }

    fn frames(g: &mut Game, n: i32) {
        for _ in 0..n {
            let now = g.last_us.unwrap_or(0) + 16_667;
            g.advance(now);
        }
    }

    #[test]
    fn the_game_opens_on_the_title_then_the_choice_of_starter() {
        let mut g = game_with("");
        assert_eq!(g.mode_key(), "title");
        assert!(g.begin());
        assert_eq!(g.mode_key(), "choose");
        let s: Value = serde_json::from_str(&g.screen_json()).unwrap();
        assert_eq!(s["starters"].as_array().unwrap().len(), 3);
        assert!(g.choose(1));
        assert_eq!(g.mode_key(), "map");
    }

    #[test]
    fn a_fight_runs_on_the_clock_and_faster_at_higher_speed() {
        let mut g = game_with("");
        g.begin();
        g.start_with_seed(0, 3);
        let first = g.run.as_ref().unwrap().choices()[0];
        assert!(g.pick(first));
        assert_eq!(g.mode_key(), "battle");
        frames(&mut g, 60);
        let t1 = g.run.as_ref().unwrap().fight().unwrap().tick;
        assert!((58..=62).contains(&t1), "a second of fight a second: {t1}");
        assert!(g.set_speed(4));
        frames(&mut g, 30);
        let t2 = g.run.as_ref().unwrap().fight().unwrap().tick;
        assert!(t2 - t1 >= 110, "four times as fast: {}", t2 - t1);
        assert!(!g.set_speed(3));
    }

    #[test]
    fn skipping_ends_the_fight_and_concluding_moves_on() {
        let mut g = game_with("");
        g.begin();
        g.start_with_seed(0, 3);
        let first = g.run.as_ref().unwrap().choices()[0];
        g.pick(first);
        assert!(!g.conclude(), "not before it is over");
        assert!(g.skip());
        let h: Value = serde_json::from_str(&g.hud_json()).unwrap();
        assert_eq!(h["over"], true);
        assert!(g.conclude());
        assert!(["reward", "dead"].contains(&g.mode_key()));
    }

    #[test]
    fn the_record_is_kept_and_read_back() {
        let mut g = game_with("");
        g.begin();
        g.start_with_seed(0, 3);
        g.abandon();
        assert_eq!(g.mode_key(), "dead");
        assert!(g.to_title());
        let back = game_with(&g.saved());
        assert_eq!(back.record.runs, 1);
        let junk = game_with("not json");
        assert_eq!(junk.record, Record::default());
    }

    #[test]
    fn every_screen_fills_every_placeholder() {
        let mut g = game_with("");
        g.begin();
        let check = |g: &Game| {
            let s = g.screen_json();
            assert!(!s.contains('{') || !s.contains("{n}"), "{s}");
            for key in ["{n}", "{s}", "{part}", "{foe}", "{runs}"] {
                assert!(!s.contains(key), "unfilled {key} in {}: {s}", g.mode_key());
            }
            let h = g.hud_json();
            assert!(!h.contains("{n}"), "{h}");
        };
        check(&g);
        g.start_with_seed(2, 9);
        check(&g);
        // walk a whole run, looking at every screen on the way
        for _ in 0..300 {
            check(&g);
            let mode = g.mode_key();
            match mode {
                "map" => {
                    let k = g.run.as_ref().unwrap().choices()[0];
                    g.pick(k);
                }
                "battle" => {
                    frames(&mut g, 20);
                    check(&g);
                    g.skip();
                    check(&g);
                    g.conclude();
                }
                "reward" => {
                    g.reward_part(0);
                }
                "encounter" => {
                    g.encounter_done();
                }
                "shop" => {
                    g.shop_buy(0);
                    if g.mode_key() == "graft" {
                        g.graft_decline();
                    }
                    g.shop_leave();
                }
                "rest" => {
                    g.rest_heal();
                }
                "graft" => {
                    g.graft_replace(0);
                }
                _ => break,
            }
        }
        assert!(["dead", "won"].contains(&g.mode_key()));
    }

    #[test]
    fn every_copy_key_the_core_asks_for_is_present() {
        let c = crate::content::tests::content();
        let sources = [include_str!("game.rs"), include_str!("run.rs")];
        let mut keys: Vec<String> = Vec::new();
        for src in sources {
            for piece in src.split("word(\"").skip(1) {
                if let Some(end) = piece.find('"') {
                    keys.push(piece[..end].to_string());
                }
            }
        }
        for kind in ["melee", "ranged", "poison", "evade", "heal", "guard", "drain"] {
            keys.push(format!("act_{kind}"));
        }
        for m in crate::content::MOD_NAMES {
            keys.push(format!("mod_{m}"));
        }
        for s in crate::content::SOCKETS {
            keys.push(format!("socket_{s}"));
        }
        for k in [
            Kind::Battle,
            Kind::Elite,
            Kind::Encounter,
            Kind::Shop,
            Kind::Rest,
            Kind::Boss,
        ] {
            keys.push(format!("node_{}", k.key()));
        }
        for k in keys {
            assert!(c.copy.contains_key(&k), "copy.json lacks {k}");
        }
    }

    #[test]
    fn the_draws_cover_every_instance_and_name_real_meshes() {
        let mut g = game_with("");
        g.begin();
        g.start_with_seed(0, 3);
        let first = g.run.as_ref().unwrap().choices()[0];
        g.pick(first);
        frames(&mut g, 5);
        let d = g.draws();
        assert_eq!(d.len() % render::DRAW_VALUES, 0);
        let mut counted = [0u32; 2];
        for draw in d.chunks(render::DRAW_VALUES) {
            assert!((draw[1] as usize) < Mesh::ALL.len());
            counted[draw[0] as usize] += draw[3];
        }
        assert_eq!(
            counted[0] as usize * render::INSTANCE_VALUES,
            g.static_instances().len()
        );
        assert_eq!(counted[1] as usize * render::INSTANCE_VALUES, g.instances().len());
    }

    #[test]
    fn secs_rounds_to_a_tenth() {
        assert_eq!(secs(120), "2.0");
        assert_eq!(secs(144), "2.4");
        assert_eq!(secs(0), "0.0");
        assert_eq!(css_pct(ONE * 75 / 2), "37.50%");
    }
}
