//! gw_shim: moves values between the page and `gw_core::game`.
//!
//! Builder requirement: the shim decides nothing. Each function is one call
//! into the core and a conversion of its arguments or result. The core speaks
//! Q16.16 integers; WebGL wants `f32`, so values on the way out are divided by
//! the core's own `ONE`, and the page's float clock is turned into the
//! microseconds the core expects. `crates/core/tests/boundary.rs` fails if a
//! branch, a loop or the `?` operator appears here.

use gw_core::{fx, game};
use wasm_bindgen::prelude::*;

const ONE: f32 = fx::ONE as f32;

fn floats(v: Vec<i32>) -> Vec<f32> {
    v.into_iter().map(|x| x as f32 / ONE).collect()
}

#[wasm_bindgen]
pub struct Game {
    inner: game::Game,
}

#[wasm_bindgen]
impl Game {
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(constructor)]
    pub fn new(
        parts: &str,
        creatures: &str,
        encounters: &str,
        rules: &str,
        palette: &str,
        copy: &str,
        saved: &str,
    ) -> Result<Game, JsValue> {
        game::Game::new(parts, creatures, encounters, rules, palette, copy, saved)
            .map(|inner| Game { inner })
            .map_err(|e| JsValue::from_str(&e))
    }

    /// CSS size and device pixel ratio in; drawing-buffer size out.
    pub fn resize(&mut self, css_w: u32, css_h: u32, dpr: f64) -> Vec<i32> {
        self.inner
            .resize(css_w as i32, css_h as i32, (dpr * 100.0) as i32)
            .to_vec()
    }

    /// The page's clock in milliseconds.
    pub fn advance(&mut self, now_ms: f64) {
        self.inner.advance((now_ms * 1000.0) as i64)
    }

    pub fn view(&self) -> Vec<f32> {
        floats(self.inner.view())
    }
    pub fn instances(&self) -> Vec<f32> {
        floats(self.inner.instances())
    }
    pub fn static_instances(&self) -> Vec<f32> {
        floats(self.inner.static_instances())
    }
    pub fn draws(&self) -> Vec<u32> {
        self.inner.draws()
    }
    pub fn uniforms(&self) -> Vec<f32> {
        floats(self.inner.scene_uniforms())
    }
    pub fn clear_color(&self) -> Vec<f32> {
        floats(self.inner.clear_color())
    }
    pub fn scene_version(&self) -> u32 {
        self.inner.scene_version()
    }
    pub fn mode(&self) -> String {
        self.inner.mode_key().to_string()
    }
    pub fn hud(&self) -> String {
        self.inner.hud_json()
    }
    pub fn screen(&self) -> String {
        self.inner.screen_json()
    }
    pub fn take_sounds(&mut self) -> String {
        self.inner.take_sounds()
    }
    pub fn saved(&self) -> String {
        self.inner.saved()
    }

    pub fn begin(&mut self) -> bool {
        self.inner.begin()
    }
    pub fn set_preview(&mut self, i: u32) -> bool {
        self.inner.set_preview(i as usize)
    }
    pub fn choose(&mut self, i: u32) -> bool {
        self.inner.choose(i as usize)
    }
    pub fn pick(&mut self, node: u32) -> bool {
        self.inner.pick(node as usize)
    }
    pub fn set_speed(&mut self, n: i32) -> bool {
        self.inner.set_speed(n)
    }
    pub fn skip(&mut self) -> bool {
        self.inner.skip()
    }
    pub fn conclude(&mut self) -> bool {
        self.inner.conclude()
    }
    pub fn reward_hp(&mut self) -> bool {
        self.inner.reward_hp()
    }
    pub fn reward_part(&mut self, i: u32) -> bool {
        self.inner.reward_part(i as usize)
    }
    pub fn reward_forage(&mut self) -> bool {
        self.inner.reward_forage()
    }
    pub fn reward_done(&mut self) -> bool {
        self.inner.reward_done()
    }
    pub fn cultivate(&mut self) -> bool {
        self.inner.cultivate()
    }
    pub fn encounter_take(&mut self) -> bool {
        self.inner.encounter_take()
    }
    pub fn encounter_done(&mut self) -> bool {
        self.inner.encounter_done()
    }
    /// A drag and drop: a part from one place to another, each named by a
    /// word ("body", "bag", "offer", "creature", "bin", "upgrade") and a
    /// number.
    pub fn move_part(&mut self, from: &str, i: u32, to: &str, j: u32) -> bool {
        self.inner.move_part(from, i as usize, to, j as usize)
    }
    pub fn can_move(&self, from: &str, i: u32, to: &str, j: u32) -> bool {
        self.inner.can_move(from, i as usize, to, j as usize)
    }
    pub fn hover(&mut self, from: &str, i: u32) -> bool {
        self.inner.hover(from, i as usize)
    }
    pub fn hover_end(&mut self) -> bool {
        self.inner.hover_end()
    }
    pub fn zone(&self) -> String {
        self.inner.zone_json()
    }
    pub fn shop_buy(&mut self, i: u32) -> bool {
        self.inner.shop_buy(i as usize)
    }
    pub fn shop_mass(&mut self) -> bool {
        self.inner.shop_mass()
    }
    pub fn shop_heal(&mut self) -> bool {
        self.inner.shop_heal()
    }
    pub fn shop_leave(&mut self) -> bool {
        self.inner.shop_leave()
    }
    pub fn rest_heal(&mut self) -> bool {
        self.inner.rest_heal()
    }
    pub fn rest_grow(&mut self) -> bool {
        self.inner.rest_grow()
    }
    pub fn abandon(&mut self) -> bool {
        self.inner.abandon()
    }
    pub fn to_title(&mut self) -> bool {
        self.inner.to_title()
    }
}

#[wasm_bindgen]
pub fn numbers() -> String {
    game::numbers()
}

#[wasm_bindgen]
pub fn mesh(id: u32) -> Vec<f32> {
    floats(game::mesh_vertices(id))
}

/// White noise for the page's sound, from the core's seeded generator.
#[wasm_bindgen]
pub fn noise(n: u32, seed: u32) -> Vec<f32> {
    floats(game::noise(n, seed))
}
