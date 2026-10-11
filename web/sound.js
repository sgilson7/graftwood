// Sound, synthesized from data/sounds.json. The core says what happened;
// this file only performs it. White noise comes from the core's generator,
// since the page may not draw random numbers.
import { noise } from "./pkg/gw_shim.js";

let ctx = null;
let master = null;
let noiseBuffer = null;
let recipes = null;
let muted = false;
const lastAt = new Map();

export function setup(sounds) {
  recipes = sounds;
}

// Browsers start audio only after a person presses something.
export function wake() {
  if (!recipes) return;
  if (!ctx) {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return;
    ctx = new AC({ latencyHint: "interactive" });
    master = ctx.createGain();
    master.gain.value = recipes.master;
    master.connect(ctx.destination);
    const data = noise(ctx.sampleRate * recipes.noise_seconds, recipes.seed);
    noiseBuffer = ctx.createBuffer(1, data.length, ctx.sampleRate);
    noiseBuffer.copyToChannel(data, 0);
  }
  if (ctx.state === "suspended") ctx.resume().catch(() => {});
}

export function toggleMute() {
  muted = !muted;
  if (master) master.gain.value = muted ? 0 : recipes.master;
  return muted;
}

function layer(l, t) {
  const t0 = t + l.at;
  const g = ctx.createGain();
  g.gain.setValueAtTime(recipes.floor, t0);
  g.gain.linearRampToValueAtTime(l.gain, t0 + l.attack);
  g.gain.exponentialRampToValueAtTime(recipes.floor, t0 + l.dur);
  g.connect(master);
  let src;
  if (l.type === "osc") {
    src = ctx.createOscillator();
    src.type = l.wave;
    src.frequency.setValueAtTime(l.from, t0);
    src.frequency.exponentialRampToValueAtTime(l.to, t0 + l.dur);
    src.connect(g);
  } else {
    src = ctx.createBufferSource();
    src.buffer = noiseBuffer;
    src.loop = true;
    const f = ctx.createBiquadFilter();
    f.type = l.filter;
    f.frequency.value = l.freq;
    f.Q.value = l.q;
    src.connect(f);
    f.connect(g);
  }
  src.start(t0);
  src.stop(t0 + l.dur + recipes.tail);
}

// A sound by name. A sound asked for again sooner than its `gap` is
// dropped, so a fast gun or a burst of hits does not pile up.
export function play(kind) {
  if (!ctx || muted || !recipes) return;
  const recipe = recipes.recipes[kind];
  if (!recipe) return;
  const now = ctx.currentTime;
  const gap = recipes.gaps[kind] || recipes.gap;
  if (now - (lastAt.get(kind) || -gap) < gap) return;
  lastAt.set(kind, now);
  for (const l of recipe) layer(l, now);
}

window.addEventListener("pagehide", () => {
  if (ctx) {
    const c = ctx;
    ctx = null;
    c.close().catch(() => {});
  }
});
