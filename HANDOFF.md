# Handoff

Stage 9, written by the agent. A fresh session starts from this file.

## Where the work is

Graftwood is a Builder project built in one session on 2026-10-10 from `BRIEF.md` (Sam's request and his friend's design, word for word) and `PLAN.md`. Repository `sgilson7/graftwood`, public; a push to `main` runs `./scripts/check.sh` in Chromium on GitHub Actions and deploys `dist/` to Pages only if it passes. Sam approved the repository, Pages, the deploys and the website listing in advance.

## Live (2026-10-10)

- Game: https://sgilson7.github.io/graftwood/ (Actions run 38101767495: check and deploy passed). `BUILDER_ORIGIN=https://sgilson7.github.io/graftwood .venv/bin/python tests/browser/check.py` passed in Chromium, Firefox and WebKit against the live site.
- Website: https://sgilson7.github.io/projects/graftwood/, commit "Add Graftwood to the games" in `~/Documents/website` (record in `content/projects.yaml`, page in `content/projects/graftwood.md`, `/graftwood/` in `OFF_BUILD_ROUTES`, tile still and clip in `static/img/tiles/`). `make check` (no findings), `make packet` (44 routes clean) and `make gate` passed, then `make deploy`; `make gate-live` passed after the publish.

## The playtest edits (2026-10-11)

The designer's edits (BRIEF.md) are in: drag and drop building with a held part drawn on the 3D body, a 2-part bag, part levels and the Old Grafter, mass at 20 acorns, 16 new creatures and 47 parts of theirs, 7 wild parts with Two-Headed, foraging and cultivating, regeneration capped at 3 a second, and the four sets with bonuses. Readings are SECOND-ORDER D16 to D24; balance B6. New art: four set badges and four interface icons (`tools/art.py` `sets/`, `ui/`).

## What exists

- **Core** (`crates/core/src`): `content` (data files, checked on load), `creature` (mass, parts, slow, evasion, grafting), `combat` (the auto-battle, 60 ticks a second, seeded dodges), `map` (the seeded 12-floor path), `run` (a run's state and every choice: battles, rewards, encounters, the Trader, rests, grafting), `scene` (the woods and creatures built from their parts), `render` (solid, ink and glow passes), `game` (screens, clock, camera, HUD and screen JSON). `fx`, `geom`, `rng`, `mesh` are Pursuer's.
- **Data**: 89 parts, 38 creatures (3 starters, 34 in three tiers, the Old Stag), 18 encounters (9 good, 9 bad), the rules (sets, bag, upgrades, forage), the palette, copy, sounds, shaders.
- **Page**: `app.js` (loop and screens), `screens.js`, `drag.js` (drag and drop; the core decides every drop), `hud.js`, `gl.js` (Long Time Comin's renderer, three passes), `sound.js`.
- **Art**: `tools/art.py` → `web/art/` (title, won, dead, encounter, shop, rest, six map icons, the page icon), TikZ kept in `tools/art/`.
- **Tools**: `cargo run --release --example balance` (matchups and 600 whole runs by two automatic players); `.venv/bin/python tools/shots.py [dir]` (screenshots through a run).

## What passed

`./scripts/check.sh` on 2026-10-11 after the playtest edits: fmt, clippy with warnings as errors, 96 core tests and 11 boundary tests, the release wasm build, the wasm import audit, and the browser check (now dragging a part into the bag and back onto the creature, and foraging) in Chromium, Firefox and WebKit (`BUILDER CHECK PASSED`).

Before:

`./scripts/check.sh` on 2026-10-10 before the first push: fmt, clippy with warnings as errors, 73 core tests and 11 boundary tests, the release wasm build, the wasm import audit, and the browser check in Chromium, Firefox and WebKit (`BUILDER CHECK PASSED`). Two behaviours were broken on purpose and each test failed (SECOND-ORDER B3).

## What remains

- Every number is a guess until a person plays it (SECOND-ORDER T1). The balance probe gives a sensible bot about 13% wins, a careless one 3%.
- The designer's readings to confirm: SECOND-ORDER T3.
- The friend's readings to confirm: slow, the encounter deal, fatigue (SECOND-ORDER T2).
- Not in the MVP: a run saved part way, more encounters with choices, more boss variety, part icons on cards, a codex of creatures.
