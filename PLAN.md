# Plan

Stage 2. The decisions, and the reason for each. What the brief leaves open is decided here and logged in `SECOND-ORDER.md`.

## Parents

| From | Taken |
| --- | --- |
| Builder (`builder-setting`) | the three layers, `BUILDER-SPEC.md`, the boundary tests, `check.sh`, the browser check, the procedure |
| Long Time Comin' | the renderer (solid, ink and glow passes; the inverted-hull outline; toon bands), the shim's shape, the build script with cache busting, the sound synthesizer, the run-on-a-graph roguelike shape |
| Pursuer | Q16.16 fixed point, vectors and matrices, seeded RNG, meshes, the TikZ drawing machinery in `tools/art.py`, the Pages workflow |
| Sam_Comm_final.png | the palette and line: warm yellow sky, salmon, coral, sage, teal, one plum ink |

## Architecture

- `crates/core` (`gw_core`): every rule, integers only, no I/O. Modules: `content` (data files), `creature` (mass, parts, slow, evasion), `combat` (the auto-battle), `map` (the branching path), `run` (a run's state and every choice), `scene` (the 3D woods and creature models), `render` (instanced draws and passes), `game` (screens, clock, camera, HUD and screen JSON).
- `crates/shim` (`gw_shim`): one call per method, no branches.
- `web/`: WebGL 2 canvas under HTML panels. `screens.js` draws the screens between fights, `hud.js` the fight, `gl.js` the 3D.
- `data/`: parts, creatures, encounters, rules, palette, copy, sounds, shaders.

## The game

- **Creatures.** A body of some mass; one part per unit of mass. A part sits in a socket (mouth, head, back, legs, tail, side) and either acts on a timer (melee, ranged, poison, evade, heal, guard, drain) or changes the creature for good (less damage taken, evasion, health, thorns, haste, more damage), or both. 35 parts, 18 creatures of the woods in three tiers (6 a tier), 3 starters, 1 boss.
- **Combat.** 60 ticks a second. Each part's timer counts down and acts at zero. Slow: past 5, each unit of mass and each part adds 10% to every timer. Evasion is percent to dodge an attack, capped at 60. Poison stacks to 10, each stack 1 damage every 2 s. Guard soaks attack damage. After 30 s, fatigue hurts both sides each second, growing, so every fight ends. Speeds 1×, 2×, 4×, and skip.
- **The run.** 12 floors on a seeded graph of 2 to 4 nodes a floor: battles, elites (from floor 5), encounters, the Trader, rests (from floor 5), all rests on floor 11, the Old Stag on floor 12. Health carries between fights.
- **Rewards.** A win pays acorns and offers +8 max health or one of the loser's parts. With no empty slot, a part replaces one of yours or is let go.
- **Encounters.** Six good, six bad, dealt in pairs of one of each, so the run's encounters are half and half.
- **Trader.** Three parts, one unit of mass, a poultice, and shedding a part, for acorns.
- **Rest.** Heal 50% of max health, or grow one unit of mass.

## 3D

Fights, the creatures, and the clearing behind every screen are 3D: each creature is built from its body and the models of the parts it carries, so a grafted part shows on the body. Parts pulse when they act, melee lunges, ranged and poison shots fly in an arc with a glow, a dodge hops, a hit flashes, a loser falls over. The map and menus are panels over the live 3D clearing, where your creature turns slowly.

## Art

TikZ through `tools/art.py`, after `Sam_Comm_final.png` and the title screens of Pursuer and Long Time Comin': title, victory, defeat, three panel pictures (encounter, Trader, rest), six map icons and the page icon.

## Order of work

1. Core: content, creature, combat, map, run, with tests; a balance probe (`examples/balance.rs`).
2. Scene, render, game; shim; page.
3. Art and sound.
4. Check, repository, Pages, website.
