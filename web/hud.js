// The fight's HUD: each creature's health, poison, guard, evasion and slow,
// each part's countdown, the numbers floating over them, the speed buttons
// and the result. The core sends every value already worked out; this file
// only writes them into the page.

const el = (id) => document.getElementById(id);

function make(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

export function makeHud(game, copy, N, act) {
  let panels = [];
  const speedButtons = [];
  const speeds = el("speeds");
  for (const s of N.speeds) {
    const b = make("button", "speed", `${s}×`);
    b.addEventListener("click", act(() => game.set_speed(s)));
    speeds.append(b);
    speedButtons.push([s, b]);
  }

  // one fighter's panel, built once a fight
  function panel(root, f) {
    while (root.firstChild) root.firstChild.remove();
    root.append(make("div", "f-name", f.name));
    const bar = make("div", "bar health");
    const fill = make("div", "fill");
    const text = make("span");
    bar.append(fill, text);
    root.append(bar);
    const chips = make("div", "chips");
    const chip = (cls, label) => {
      const c = make("span", `chip ${cls}`);
      const v = make("b");
      c.append(make("span", "", label), v);
      chips.append(c);
      return { c, v };
    };
    const poison = chip("poison", copy.poison);
    const guard = chip("guard", copy.guard);
    const evasion = chip("evasion", copy.evasion);
    const slow = chip("slow", copy.slow);
    const regen = chip("regen", copy.regen);
    root.append(chips);
    const list = make("ul", "timers");
    const rows = f.parts.map((p) => {
      const li = make("li", `kind-${p.kind}`);
      const name = make("span", "t-name", p.name);
      const track = make("span", "t-track");
      const t = make("span", "t-fill");
      track.append(t);
      const secs = make("span", "t-secs");
      li.append(name, track, secs);
      list.append(li);
      return { li, t, secs };
    });
    root.append(list);
    return { fill, text, poison, guard, evasion, slow, regen, rows };
  }

  function fillPanel(p, f) {
    p.fill.style.width = f.fill;
    p.text.textContent = `${f.hp} / ${f.max}`;
    p.poison.v.textContent = f.poison;
    p.poison.c.hidden = !f.poison;
    p.guard.v.textContent = f.guard;
    p.guard.c.hidden = !f.guard;
    p.evasion.v.textContent = `${f.evasion}%`;
    p.evasion.c.hidden = !f.evasion;
    p.slow.v.textContent = `${f.slow}%`;
    p.slow.c.hidden = !f.slow;
    p.regen.v.textContent = f.regen;
    p.regen.c.hidden = !f.regen;
    f.parts.forEach((part, i) => {
      const r = p.rows[i];
      if (!r) return;
      r.t.style.width = part.fill;
      r.secs.textContent = part.secs;
      r.li.classList.toggle("fired", part.fired);
    });
  }

  return {
    reset(screen) {
      const h = JSON.parse(game.hud());
      if (!h.fighters) return;
      panels = h.fighters.map((f, i) => panel(el(`f${i}`), f));
      el("b-kind").textContent = screen.foe ? screen.foe.kind : "";
      el("b-kind").title = screen.foe ? screen.foe.blurb : "";
    },
    update(h) {
      if (!h.fighters) return;
      h.fighters.forEach((f, i) => panels[i] && fillPanel(panels[i], f));
      el("b-clock").textContent = h.clock;
      el("b-fatigue").hidden = !h.fatigue;
      for (const [s, b] of speedButtons) b.classList.toggle("on", s === h.speed);
      el("skip").disabled = h.over;
      el("result").hidden = !h.over;
      el("r-word").textContent = h.result || "";
      el("result").className = h.over && h.result === copy.fight_won ? "won" : "lost";
      const box = el("floaters");
      while (box.firstChild) box.firstChild.remove();
      for (const f of h.floaters) {
        const d = make("div", `floater ${f.kind}`, f.text);
        d.style.left = f.x;
        d.style.top = f.y;
        d.style.setProperty("--fade", f.fade);
        box.append(d);
      }
    },
  };
}
