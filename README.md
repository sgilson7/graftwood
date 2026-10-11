# Graftwood

A 3D auto-battling roguelike that runs in the browser. Your creature walks a branching path through the woods, fights bugs, crustaceans and woodland creatures, and grafts their body parts onto its own body. Fights play themselves: every part counts down and acts when it reaches zero, so the build you grow decides who wins.

**Play:** https://sgilson7.github.io/graftwood/

- **Mass and parts.** A creature has mass, and one body part fits on each unit of it. You start with 3 mass and 2 parts. Past 5, each extra unit of mass and each extra part slows every part by 10%.
- **Parts.** 35 of them, each in a socket (mouth, head, back, legs, tail, side), drawn on the 3D body when grafted. Some act on a timer: Teeth bite for 2 every 2 s, a Stinger adds poison every 4 s (each stack deals 1 every 2 s, up to 10), Lucky Feet add 1% to dodge every 5 s, Slime Coat soaks damage. Others change the creature for good: Bunny Ears take 1 off every ranged hit, Wings add 3% dodge for each empty slot.
- **The path.** 12 floors of battles, elites, encounters, the Trader and rests, rolled for each run, ending at the Old Stag: seven mass, six parts, and slow.
- **Winning a fight** pays acorns and offers +8 max health or one of the loser's parts. With no slot free, the part replaces one of yours.
- **Encounters** are half good and half bad: more mass, a found part and full healing, or brambles that take mass (and the part that no longer fits), a heron that takes a part, and a magpie that takes your acorns.
- **Three starters**: the Burrowling (Teeth, Thick Fur), the Flitterling (Stinger, Wings) and the Sprigling (Spore Cap, Slime Coat).

Fights run at 1×, 2× or 4×, or skip to the end. N turns the sound on or off.

## How it is built

Graftwood is a [Builder](https://github.com/sgilson7/builder-setting) project, built from a friend's design (`BRIEF.md`), and its parents are Sam's other games.

- **From Long Time Comin':** the renderer (solid, ink and glow passes, the inverted-hull outline and toon-banded light), the shim, the build script and the synthesized sound.
- **From Pursuer:** Q16.16 fixed point with no floats in the core, the meshes, the seeded generator, the Pages workflow, and the TikZ machinery of `tools/art.py`, after `Sam_Comm_final.png`.
- **New here:** creatures assembled from the parts they carry, an integer auto-battler that replays exactly from its seed, a seeded branching map, and a balance probe that plays hundreds of runs (`cargo run --release --example balance`).

The layers:

- **`crates/core`** holds every rule: creatures, combat, the map, the run, the 3D scene and the camera, and the HUD. No input or output, no floats, no clock.
- **`crates/shim`** moves values across the WebAssembly boundary and decides nothing.
- **`web/`** is a static page with a WebGL 2 canvas under HTML panels. It draws what the core returns.
- **`data/`** holds every part, creature, encounter, rule, colour, word, sound recipe and shader.
- **`tools/art.py`** draws every picture as TikZ; the sources are in `tools/art/`.

```bash
./scripts/setup.sh   # once: Playwright and three browser engines
./scripts/check.sh   # fmt, clippy, the core tests, the wasm build and audit, the browser check
python3 scripts/serve.py 8000
```

A push to `main` runs the same check in Chromium and deploys `dist/` to GitHub Pages if it passes.

`BRIEF.md` holds the request and the design, `PLAN.md` the decisions, `SECOND-ORDER.md` what was decided and found while building, and `HANDOFF.md` where the work stands.

## Credits

Designed by a friend of Sam's; built by Sam Gilson with a coding agent. Every picture is TikZ drawn in this repository; every sound is synthesized.

## Licence

MIT. See `LICENSE`.
