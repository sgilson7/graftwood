// The screens between fights, drawn from the core's screen JSON. Every word
// and number arrives from the core or copy.json; this file only places them,
// marks which cards can be dragged and where they can be dropped, and wires
// each button to one call into the core.

const el = (id) => document.getElementById(id);

function make(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function clear(node) {
  while (node.firstChild) node.firstChild.remove();
}

function lines(list, items, className) {
  const ul = make("ul", className);
  for (const r of items) ul.append(make("li", "", r));
  list.append(ul);
}

// A set's badge: its TikZ picture and its name, so colour is never the only cue.
function badge(p, copy) {
  if (p.set) {
    const b = make("span", `set-badge set-${p.set.id}`);
    const img = make("img");
    img.src = `art/sets/${p.set.id}.png`;
    img.alt = "";
    b.append(img, make("span", "", p.set.name));
    return b;
  }
  return make("span", "set-badge wild", copy.wild);
}

export function makeScreens(game, copy, act, drag) {
  let last = {};

  // One body part as a card: name, set, socket, rules, flavour, and an
  // optional button underneath.
  function card(p, button) {
    const c = make("div", `card kind-${p.kind}${p.level > 0 ? " upgraded" : ""}${p.level < 0 ? " withered" : ""}`);
    const head = make("div", "card-head");
    head.append(make("span", "card-name", p.name), badge(p, copy), make("span", "card-socket", p.socket));
    c.append(head);
    lines(c, p.rules, "card-rules");
    if (p.flavour) c.append(make("p", "card-flavour", p.flavour));
    if (button) {
      const b = make("button", "card-button", button.label);
      b.disabled = !button.enabled;
      b.addEventListener("click", button.onClick);
      c.append(b);
    }
    return c;
  }

  function sheet(me) {
    el("s-name").textContent = me.name;
    el("s-floor").textContent = `${me.floor} / ${me.floors}`;
    el("s-hp").textContent = `${me.hp} / ${me.max_hp}`;
    el("s-hp-fill").style.width = me.fill;
    el("s-acorns").textContent = me.acorns;
    el("s-slow").textContent = `${me.slow}%`;
    el("s-evasion").textContent = `${me.evasion}%`;
    el("s-regen").textContent = me.regen;
    el("s-forage").textContent = me.forage.text;
    el("s-forage-fill").style.width = me.forage.fill;
    el("cultivate").hidden = !me.forage.full;
    el("cultivate").disabled = !me.forage.can_cultivate;

    const slots = el("s-slots");
    clear(slots);
    for (const s of me.slots) {
      const i = make("i", `${s.used ? "used" : "empty"}${s.head ? " head" : ""}`);
      if (s.head) i.title = copy.head_slot;
      slots.append(i);
    }

    const sets = el("s-sets");
    clear(sets);
    for (const s of me.sets) {
      const chip = make("div", `set-chip set-${s.id}${s.active ? " active" : ""}`);
      const img = make("img");
      img.src = `art/sets/${s.id}.png`;
      img.alt = "";
      chip.append(img, make("b", "", s.line));
      chip.append(make("span", "", s.active ? s.bonus.join(" ") : s.next));
      sets.append(chip);
    }

    // the body: each part can be picked up, and a part dropped on it swaps
    const list = el("s-parts");
    clear(list);
    me.parts.forEach((p, i) => {
      const c = card(p);
      if (me.edit) {
        drag.source(c, "body", i);
        drag.target(c, "body", i);
      }
      list.append(c);
    });
    for (const s of me.slots) {
      if (s.used) continue;
      const empty = make("div", "card empty-slot", s.head ? copy.head_slot : copy.empty_slot);
      if (me.edit) drag.target(empty, "creature", 0);
      list.append(empty);
    }

    // the bag: parts carried but doing nothing
    const bag = el("s-bag");
    clear(bag);
    me.bag.forEach((p, i) => {
      const c = card(p);
      if (me.edit) {
        drag.source(c, "bag", i);
        drag.target(c, "bag", i);
      }
      bag.append(c);
    });
    for (let i = me.bag.length; i < me.bag_size; i += 1) {
      const empty = make("div", "card empty-slot", copy.bag_empty);
      if (me.edit) drag.target(empty, "bag", i);
      bag.append(empty);
    }
  }

  function choose(s) {
    const list = el("starters");
    clear(list);
    s.starters.forEach((k, i) => {
      const c = make("button", i === s.preview ? "starter chosen" : "starter");
      c.append(make("div", "starter-name", k.name), make("p", "starter-blurb", k.blurb));
      c.append(make("div", "starter-stats", `${copy.health} ${k.max_hp} · ${copy.mass} ${k.mass}`));
      const parts = make("div", "cards compact");
      for (const p of k.parts) parts.append(card(p));
      c.append(parts);
      c.addEventListener("click", act(() => game.set_preview(i)));
      list.append(c);
    });
  }

  function map(s) {
    const svg = el("edges");
    clear(svg);
    for (const e of s.map.edges) {
      const line = document.createElementNS(svg.namespaceURI, "line");
      line.setAttribute("x1", e.x1);
      line.setAttribute("y1", e.y1);
      line.setAttribute("x2", e.x2);
      line.setAttribute("y2", e.y2);
      line.setAttribute("class", e.state);
      svg.append(line);
    }
    const nodes = el("nodes");
    clear(nodes);
    for (const n of s.map.nodes) {
      const b = make("button", `node ${n.kind} ${n.state}`);
      b.style.left = n.x;
      b.style.top = n.y;
      b.title = n.label;
      b.dataset.node = n.id;
      const img = make("img");
      img.src = `art/nodes/${n.kind}.png`;
      img.alt = n.label;
      b.append(img);
      b.disabled = n.state !== "open";
      b.addEventListener("click", act(() => game.pick(n.id)));
      nodes.append(b);
    }
  }

  // A part on offer: draggable, with the plain button the core labels.
  function offer(p, i, onClick) {
    const c = card(p, { label: p.take.label, enabled: p.take.can, onClick: act(onClick) });
    if (p.take.can) drag.source(c, "offer", i);
    return c;
  }

  function reward(s) {
    const r = s.reward;
    el("w-heading").textContent = r.heading;
    el("w-acorns").textContent = r.acorns;
    el("reward-forage").textContent = r.forage.label;
    el("reward-forage").disabled = !r.forage.can;
    el("w-forage").textContent = r.forage.line;
    el("reward-hp").textContent = r.hp;
    el("reward-hp").disabled = r.taken;
    el("w-room").textContent = r.room;
    el("reward-done").textContent = r.done;
    const list = el("w-parts");
    clear(list);
    r.parts.forEach((p, i) => list.append(offer(p, i, () => game.reward_part(i))));
  }

  function encounter(s) {
    const e = s.encounter;
    const tag = el("e-tag");
    tag.textContent = e.good ? copy.good : copy.bad;
    tag.className = e.good ? "tag good" : "tag bad";
    el("e-name").textContent = e.name;
    el("e-text").textContent = e.text;
    const items = el("e-lines");
    clear(items);
    for (const l of e.lines) items.append(make("li", "", l));
    const found = el("e-found");
    clear(found);
    if (e.found) found.append(offer(e.found, 0, () => game.encounter_take()));
    el("e-upgrade").hidden = !e.upgrade;
    const choices = el("e-choices");
    clear(choices);
    for (const p of e.choices) {
      const c = card(p, {
        label: copy.upgrade_this,
        enabled: p.can,
        onClick: act(() => game.move_part(p.from.kind, p.from.i, "upgrade", 0)),
      });
      const next = make("div", "card-next");
      next.append(make("b", "", p.next_line));
      lines(next, p.next, "card-rules");
      c.insertBefore(next, c.lastChild);
      if (p.can) drag.source(c, p.from.kind, p.from.i);
      choices.append(c);
    }
    el("encounter-done").textContent = e.button;
  }

  function shop(s) {
    const o = s.shop;
    const list = el("o-offers");
    clear(list);
    o.offers.forEach((p, i) => list.append(offer(p, i, () => game.shop_buy(i))));
    el("shop-mass").textContent = o.mass.label;
    el("shop-mass").disabled = !o.mass.can;
    el("shop-heal").textContent = o.heal.label;
    el("shop-heal").disabled = !o.heal.can;
  }

  function rest(s) {
    el("rest-heal").textContent = s.rest.heal;
    el("rest-grow").textContent = s.rest.grow;
    el("rest-grow").disabled = !s.rest.can_grow;
  }

  function end(s, mode) {
    el("end-art").src = mode === "won" ? "art/won.png" : "art/dead.png";
    el("x-heading").textContent = s.end.heading;
    el("x-text").textContent = s.end.text;
    const stats = el("x-stats");
    clear(stats);
    for (const line of s.end.stats) stats.append(make("li", "", line));
  }

  return {
    preview: () => last.preview || 0,
    show(mode, s) {
      last = s;
      el("record").textContent = s.record.line;
      if (s.me) sheet(s.me);
      if (mode === "choose") choose(s);
      if (mode === "map") map(s);
      if (mode === "reward") reward(s);
      if (mode === "encounter") encounter(s);
      if (mode === "shop") shop(s);
      if (mode === "rest") rest(s);
      if (mode === "dead" || mode === "won") end(s, mode);
    },
  };
}
