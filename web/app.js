// The page draws what the core returns and decides nothing. It loads the data
// files, starts the core through the shim, passes the clock in each display
// frame and draws what comes out. The core says which screen is showing; the
// page shows it, and each button hands one choice back to the core.
import init, { Game, numbers, mesh } from "./pkg/gw_shim.js";
import { makeRenderer } from "./gl.js";
import { makeScreens } from "./screens.js";
import { makeHud } from "./hud.js";
import { setup as setupSound, wake, toggleMute, play } from "./sound.js";

const BUILD = "__BUILD__";
const SAVE_KEY = "graftwood.record";
const el = (id) => document.getElementById(id);
const file = (path) => fetch(`${path}?v=${BUILD}`).then((r) => r.text());

// Browser storage can be missing or refused; the game works without it.
const storage = {
  load() {
    try {
      return localStorage.getItem(SAVE_KEY) || "";
    } catch {
      return "";
    }
  },
  save(text) {
    try {
      localStorage.setItem(SAVE_KEY, text);
    } catch {
      // nothing to do: the record is simply not kept
    }
  },
};

function fillCopy(copy) {
  for (const node of document.querySelectorAll("[data-copy]")) {
    node.textContent = copy[node.dataset.copy];
  }
  document.title = copy.title;
  el("build").textContent = BUILD;
  for (const line of copy.how) {
    const li = document.createElement("li");
    li.textContent = line;
    el("how").append(li);
  }
}

async function start() {
  const names = ["parts", "creatures", "encounters", "rules", "palette", "copy", "sounds"];
  const texts = await Promise.all(
    names.map((n) => file(`data/${n}.json`)).concat([file("data/shaders/scene.vert"), file("data/shaders/scene.frag")]),
  );
  const [parts, creatures, encounters, rules, palette, copyText, soundsText, vertex, fragment] = texts;
  const copy = JSON.parse(copyText);
  fillCopy(copy);
  setupSound(JSON.parse(soundsText));
  await init();
  const N = JSON.parse(numbers());

  let game;
  try {
    game = new Game(parts, creatures, encounters, rules, palette, copyText, storage.load());
  } catch (e) {
    el("message").textContent = String(e);
    document.body.dataset.ready = "true";
    return;
  }

  const canvas = el("view");
  let renderer = null;
  try {
    renderer = makeRenderer(canvas, N, { vertex, fragment }, (id) => mesh(id));
  } catch (e) {
    renderer = null;
  }
  if (!renderer) el("message").textContent = copy.no_webgl;

  const save = () => storage.save(game.saved());
  let shownMode = null;

  // A button: wake the sound, hand the choice to the core, keep the record,
  // and draw the screen again from what the core now says.
  const act = (fn) => () => {
    wake();
    fn();
    if (document.activeElement && document.activeElement.blur) document.activeElement.blur();
    save();
    refresh(true);
  };

  const screens = makeScreens(game, copy, act);
  const hud = makeHud(game, copy, N, act);

  el("begin").addEventListener("click", act(() => game.begin()));
  el("choose-go").addEventListener("click", act(() => game.choose(screens.preview())));
  el("conclude").addEventListener("click", act(() => game.conclude()));
  el("skip").addEventListener("click", act(() => game.skip()));
  el("reward-hp").addEventListener("click", act(() => game.reward_hp()));
  el("reward-skip").addEventListener("click", act(() => game.reward_skip()));
  el("encounter-done").addEventListener("click", act(() => game.encounter_done()));
  el("shop-mass").addEventListener("click", act(() => game.shop_mass()));
  el("shop-heal").addEventListener("click", act(() => game.shop_heal()));
  el("shop-leave").addEventListener("click", act(() => game.shop_leave()));
  el("rest-heal").addEventListener("click", act(() => game.rest_heal()));
  el("rest-grow").addEventListener("click", act(() => game.rest_grow()));
  el("graft-decline").addEventListener("click", act(() => game.graft_decline()));
  el("to-title").addEventListener("click", act(() => game.to_title()));
  const abandon = el("abandon");
  abandon.addEventListener("click", () => {
    if (!abandon.classList.contains("armed")) {
      abandon.classList.add("armed");
      abandon.textContent = copy.abandon_confirm;
      return;
    }
    abandon.classList.remove("armed");
    abandon.textContent = copy.abandon;
    act(() => game.abandon())();
  });

  window.addEventListener("keydown", (e) => {
    if (e.code === "KeyN") toggleMute();
    wake();
  });
  window.addEventListener("pointerdown", () => wake());

  function resize() {
    const [w, h] = game.resize(canvas.clientWidth, canvas.clientHeight, window.devicePixelRatio || 1);
    if (renderer) renderer.resize(w, h);
  }
  window.addEventListener("resize", resize);
  resize();

  // which page elements each screen shows
  const layout = {
    title: ["title"],
    choose: ["choose"],
    map: ["map", "sheet"],
    battle: ["battle"],
    reward: ["reward", "sheet"],
    encounter: ["encounter", "sheet"],
    shop: ["shop", "sheet"],
    rest: ["rest", "sheet"],
    graft: ["graft", "sheet"],
    dead: ["end"],
    won: ["end"],
  };
  const all = ["title", "choose", "map", "sheet", "battle", "reward", "encounter", "shop", "rest", "graft", "end"];

  function refresh(again) {
    const mode = game.mode();
    if (mode === shownMode && !again) return;
    document.body.dataset.mode = mode;
    const shown = layout[mode] || [];
    for (const id of all) el(id).hidden = !shown.includes(id);
    screens.show(mode, JSON.parse(game.screen()));
    if (mode === "battle") hud.reset(JSON.parse(game.screen()));
    if (mode !== shownMode) save();
    shownMode = mode;
  }

  let sceneVersion = null;
  function frame(now) {
    game.advance(now);
    refresh(false);
    if (shownMode === "battle") hud.update(JSON.parse(game.hud()));
    for (const s of JSON.parse(game.take_sounds())) play(s);
    if (renderer) {
      if (game.scene_version() !== sceneVersion) {
        sceneVersion = game.scene_version();
        renderer.setScene(game.static_instances(), game.uniforms(), game.clear_color());
      }
      renderer.frame(game.view(), game.instances(), game.draws());
    }
    requestAnimationFrame(frame);
  }

  refresh(true);
  document.body.dataset.ready = "true";
  requestAnimationFrame(frame);
}

start();
