// The screens between fights, drawn from the core's screen JSON. Every word
// and number arrives from the core or copy.json; this file only places them
// and wires each button to one call into the core.

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

// One body part as a card: name, socket, rules, flavour, and an optional
// button underneath.
function card(p, button) {
  const c = make("div", `card kind-${p.kind}`);
  const head = make("div", "card-head");
  head.append(make("span", "card-name", p.name), make("span", "card-socket", p.socket));
  c.append(head);
  const rules = make("ul", "card-rules");
  for (const r of p.rules) rules.append(make("li", "", r));
  c.append(rules);
  if (p.flavour) c.append(make("p", "card-flavour", p.flavour));
  if (button) {
    const b = make("button", "card-button", button.label);
    b.disabled = !button.enabled;
    b.addEventListener("click", button.onClick);
    c.append(b);
  }
  return c;
}

export function makeScreens(game, copy, act) {
  let last = {};

  function sheet(me) {
    el("s-name").textContent = me.name;
    el("s-floor").textContent = `${me.floor} / ${me.floors}`;
    el("s-hp").textContent = `${me.hp} / ${me.max_hp}`;
    el("s-hp-fill").style.width = me.fill;
    el("s-acorns").textContent = me.acorns;
    el("s-slow").textContent = `${me.slow}%`;
    el("s-evasion").textContent = `${me.evasion}%`;
    const slots = el("s-slots");
    clear(slots);
    for (const used of me.slots) slots.append(make("i", used ? "used" : "empty"));
    const list = el("s-parts");
    clear(list);
    for (const p of me.parts) list.append(card(p));
    for (const used of me.slots) if (!used) list.append(make("div", "card empty-slot", copy.empty_slot));
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

  function reward(s) {
    const r = s.reward;
    el("w-heading").textContent = r.heading;
    el("w-acorns").textContent = r.acorns;
    el("reward-hp").textContent = r.hp;
    el("w-room").textContent = r.room;
    const list = el("w-parts");
    clear(list);
    r.parts.forEach((p, i) => {
      list.append(card(p, { label: copy.graft_this, enabled: true, onClick: act(() => game.reward_part(i)) }));
    });
  }

  function encounter(s) {
    const e = s.encounter;
    const tag = el("e-tag");
    tag.textContent = e.good ? copy.good : copy.bad;
    tag.className = e.good ? "tag good" : "tag bad";
    el("e-name").textContent = e.name;
    el("e-text").textContent = e.text;
    const lines = el("e-lines");
    clear(lines);
    for (const l of e.lines) lines.append(make("li", "", l));
    el("encounter-done").textContent = e.button;
  }

  function shop(s) {
    const o = s.shop;
    const list = el("o-offers");
    clear(list);
    o.offers.forEach((p, i) => {
      const label = p.sold ? copy.shop_sold : `${copy.shop_buy} · ${p.price}`;
      list.append(card(p, { label, enabled: p.can, onClick: act(() => game.shop_buy(i)) }));
    });
    el("shop-mass").textContent = o.mass.label;
    el("shop-mass").disabled = !o.mass.can;
    el("shop-heal").textContent = o.heal.label;
    el("shop-heal").disabled = !o.heal.can;
    el("o-shed-label").textContent = o.shed.label;
    const shed = el("o-shed");
    clear(shed);
    s.me.parts.forEach((p, i) => {
      const b = make("button", "small", `${copy.shed_this}: ${p.name}`);
      b.disabled = !o.shed.can;
      b.addEventListener("click", act(() => game.shop_shed(i)));
      shed.append(b);
    });
  }

  function rest(s) {
    el("rest-heal").textContent = s.rest.heal;
    el("rest-grow").textContent = s.rest.grow;
    el("rest-grow").disabled = !s.rest.can_grow;
  }

  function graft(s) {
    el("g-heading").textContent = s.graft.heading;
    const one = el("g-part");
    clear(one);
    one.append(card(s.graft.part));
    const mine = el("g-mine");
    clear(mine);
    s.me.parts.forEach((p, i) => {
      mine.append(card(p, { label: copy.graft_replace, enabled: true, onClick: act(() => game.graft_replace(i)) }));
    });
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
      if (mode === "graft") graft(s);
      if (mode === "dead" || mode === "won") end(s, mode);
    },
  };
}
