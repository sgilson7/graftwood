// Drag and drop for body parts, as Spore lets you pull parts onto a body.
// screens.js marks what can be picked up (a part on the body, in the bag, or
// on offer) and what can take it (the creature, a part to swap with, a bag
// place, the bin, the grafter's stump). The core decides every move: while a
// part is held over a target the page asks the core whether the drop would
// be allowed and colours the target with the answer, a part held over the
// creature is drawn on it by the core, and on release the move is handed to
// the core. The page keeps no rule of its own.

export function makeDrag(game, act) {
  const sources = new WeakMap();
  const targets = new WeakMap();
  const zone = document.getElementById("creature-zone");
  let held = null;

  function overTarget(x, y) {
    const hit = document.elementFromPoint(x, y);
    const node = hit ? hit.closest("[data-to]") : null;
    return node && targets.has(node) ? node : null;
  }

  function mark(node) {
    if (held.over === node) return;
    if (held.over) held.over.classList.remove("drop-ok", "drop-no");
    held.over = node;
    const to = node ? targets.get(node) : null;
    if (to && to.kind === "creature") game.hover(held.kind, held.i);
    else game.hover_end();
    if (!to) return;
    const ok = game.can_move(held.kind, held.i, to.kind, to.j);
    node.classList.add(ok ? "drop-ok" : "drop-no");
  }

  function placeZone() {
    const z = JSON.parse(game.zone());
    if (!z.left) return;
    zone.style.left = z.left;
    zone.style.top = z.top;
    zone.style.width = z.width;
    zone.style.height = z.height;
  }

  function lift(e) {
    held.ghost = held.node.cloneNode(true);
    held.ghost.classList.add("drag-ghost");
    held.ghost.removeAttribute("data-from");
    document.body.append(held.ghost);
    held.node.classList.add("lifted");
    placeZone();
    document.body.classList.add("dragging");
    move(e);
  }

  function move(e) {
    held.ghost.style.left = `${e.clientX}px`;
    held.ghost.style.top = `${e.clientY}px`;
    mark(overTarget(e.clientX, e.clientY));
  }

  function drop() {
    const node = held.over;
    const ok = node && node.classList.contains("drop-ok");
    const to = node ? targets.get(node) : null;
    const from = held;
    end();
    if (ok) act(() => game.move_part(from.kind, from.i, to.kind, to.j))();
  }

  function end() {
    if (held.over) held.over.classList.remove("drop-ok", "drop-no");
    if (held.ghost) held.ghost.remove();
    held.node.classList.remove("lifted");
    document.body.classList.remove("dragging");
    game.hover_end();
    held = null;
  }

  document.addEventListener("pointerdown", (e) => {
    if (e.button > 0 || held) return;
    if (e.target.closest("button")) return;
    const node = e.target.closest("[data-from]");
    if (!node || !sources.has(node)) return;
    const src = sources.get(node);
    held = { node, kind: src.kind, i: src.i, x: e.clientX, y: e.clientY, ghost: null, over: null };
    e.preventDefault();
  });
  document.addEventListener("pointermove", (e) => {
    if (!held) return;
    if (!held.ghost) {
      if (e.clientX === held.x && e.clientY === held.y) return;
      lift(e);
      return;
    }
    move(e);
  });
  document.addEventListener("pointerup", () => {
    if (!held) return;
    if (held.ghost) drop();
    else end();
  });
  document.addEventListener("pointercancel", () => {
    if (held) end();
  });

  return {
    // a part that can be picked up from `kind` place `i`
    source(node, kind, i) {
      node.dataset.from = kind;
      sources.set(node, { kind, i });
    },
    // a place a part can be put down
    target(node, kind, j) {
      node.dataset.to = kind;
      targets.set(node, { kind, j });
    },
  };
}
