"""The project's own browser interactions, run by check.py in every engine.

check.py has already loaded the page, waited for it to report ready, and let
it idle. These drive the game the way a person would and assert what the page
shows. Expected values are worked out by hand from the data files, never
copied from the game's own output:

- rules.json lists 3 starters; the first, the Burrowling (creatures.json),
  has 46 health, 3 mass and 2 parts, one of them Thick Fur (+6 max health,
  parts.json): 52 / 52 health, two filled slots and one empty.
- rules.json has 12 floors, the last a single boss node; the first floor has
  2 to 4 nodes (min_nodes, max_nodes), all battles, and they are the only
  ones open at the start.
- a battle shows the player's creature on the left, named Burrowling, with
  one countdown row per part (2).
- dragging the Teeth card from the sheet into the first bag place moves it:
  one part on the body, one in the bag (rules.json bag_size 2 places). The
  Thick Fur alone gives 46 + 6 = 52, so health stays 52 / 52 — and dragging
  the Teeth back onto the creature in the clearing puts two parts on again.
- winning a battle offers forage (rules.json forage.per_forage, 26 to 34 of
  100) and +8 max health (max_hp_reward): taken, the sheet reads 60 at most,
  and the screen waits for Continue.
- giving up a run ends it; back on the title the record counts 1 run, and a
  reload keeps it.
"""

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
COPY = json.loads((ROOT / "data" / "copy.json").read_text())
RULES = json.loads((ROOT / "data" / "rules.json").read_text())


def click(page, selector):
    page.click(selector, timeout=5000)
    page.wait_for_timeout(250)


def drag(page, source, target):
    """A part picked up and put down with the mouse, as a person drags it."""
    a = page.locator(source).first.bounding_box()
    page.mouse.move(a["x"] + a["width"] / 2, a["y"] + a["height"] / 2)
    page.mouse.down()
    page.mouse.move(a["x"] + a["width"] / 2 + 6, a["y"] + a["height"] / 2 + 6)
    b = page.locator(target).first.bounding_box()
    page.mouse.move(b["x"] + b["width"] / 2, b["y"] + b["height"] / 2, steps=8)
    page.mouse.up()
    page.wait_for_timeout(250)


def mode(page):
    return page.evaluate("() => document.body.dataset.mode")


def run(page, check, engine):
    message = page.inner_text("#message")
    check(message in ("", COPY["no_webgl"]), f"no error message is shown ({message!r})")
    check(mode(page) == "title", f"the game opens on the title ({mode(page)})")
    check(page.text_content("#title h1") == COPY["title"], "the title from data/copy.json is drawn")
    how = page.query_selector_all("#how li")
    check(len(how) == len(COPY["how"]), f"the rules are listed ({len(how)})")

    click(page, "#begin")
    check(mode(page) == "choose", f"then the choice of creature ({mode(page)})")
    starters = page.query_selector_all("#starters .starter")
    check(len(starters) == 3, f"three starters ({len(starters)})")
    click(page, "#starters .starter:nth-child(2)")
    chosen = page.inner_text("#starters .starter.chosen .starter-name")
    check(chosen.lower() == "flitterling", f"clicking a starter chooses it ({chosen!r})")
    click(page, "#starters .starter:nth-child(1)")

    click(page, "#choose-go")
    check(mode(page) == "map", f"starting a run shows the map ({mode(page)})")
    check(page.inner_text("#s-name").lower() == "burrowling", f"as the Burrowling ({page.inner_text('#s-name')!r})")
    check(page.inner_text("#s-hp") == "52 / 52", f"with 52 / 52 health ({page.inner_text('#s-hp')!r})")
    used = page.query_selector_all("#s-slots i.used")
    empty = page.query_selector_all("#s-slots i.empty")
    check(len(used) == 2 and len(empty) == 1, f"two parts on three mass ({len(used)}, {len(empty)})")
    bosses = page.query_selector_all("#nodes .node.boss")
    check(len(bosses) == 1, f"one boss at the end ({len(bosses)})")
    opened = page.query_selector_all("#nodes .node.open")
    lo, hi = RULES["min_nodes"], RULES["max_nodes"]
    check(lo <= len(opened) <= hi, f"{lo} to {hi} ways in ({len(opened)})")
    check(all("battle" in n.get_attribute("class") for n in opened), "the first floor is all battles")

    drag(page, "#s-parts .card:nth-child(1)", "#s-bag .card:nth-child(1)")
    body = page.query_selector_all("#s-parts .card:not(.empty-slot)")
    bag = page.query_selector_all("#s-bag .card:not(.empty-slot)")
    check(len(body) == 1 and len(bag) == 1, f"a part dragged into the bag leaves the body ({len(body)}, {len(bag)})")
    check(len(page.query_selector_all("#s-bag .card")) == RULES["bag_size"], "the bag has two places")
    check(page.inner_text("#s-hp") == "52 / 52", f"the Thick Fur's health stays ({page.inner_text('#s-hp')!r})")
    drag(page, "#s-bag .card:nth-child(1)", "#creature-zone")
    body = page.query_selector_all("#s-parts .card:not(.empty-slot)")
    check(len(body) == 2, f"dragged onto the creature, it is grafted again ({len(body)})")

    click(page, "#nodes .node.open")
    check(mode(page) == "battle", f"a battle starts ({mode(page)})")
    page.wait_for_timeout(600)
    check(page.inner_text("#f0 .f-name").lower() == "burrowling", "the player fights on the left")
    rows = page.query_selector_all("#f0 .timers li")
    check(len(rows) == 2, f"one countdown per part ({len(rows)})")
    click(page, "#skip")
    page.wait_for_selector("#result:not([hidden])", timeout=10000)
    word = page.text_content("#r-word")
    check(word in (COPY["fight_won"], COPY["fight_lost"]), f"the fight ends ({word!r})")
    click(page, "#conclude")
    after = mode(page)
    check(after in ("reward", "dead"), f"then a reward or the end ({after})")
    if after == "reward":
        parts = page.query_selector_all("#w-parts .card")
        check(2 <= len(parts) <= 4, f"the loser's parts are offered ({len(parts)})")
        click(page, "#reward-forage")
        forage = page.inner_text("#s-forage")
        lo, hi = RULES["forage"]["per_forage"]
        got = forage.split(" / ")[0]
        check(got.isdigit() and lo <= int(got) <= hi, f"foraging fills the bar by {lo} to {hi} ({forage!r})")
        check(page.is_disabled("#reward-forage"), "once a win")
        click(page, "#reward-hp")
        check(mode(page) == "reward", f"the reward waits for Continue ({mode(page)})")
        click(page, "#reward-done")
        check(mode(page) == "map", f"then back to the map ({mode(page)})")
        hp = page.inner_text("#s-hp")
        check(hp.endswith("/ 60"), f"max health rose by 8 ({hp!r})")
        click(page, "#abandon")
        click(page, "#abandon")

    check(mode(page) == "dead", f"the run is over ({mode(page)})")
    stats = page.query_selector_all("#x-stats li")
    check(len(stats) == 3, f"three lines of the run's record ({len(stats)})")
    click(page, "#to-title")
    check(mode(page) == "title", "back to the title")
    record = page.inner_text("#record")
    want = COPY["record_line"].replace("{runs}", "1").replace("{wins}", "0").replace("{best}", "1")
    check(record == want, f"the record counts one run, no wins, floor one ({record!r})")

    page.reload()
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=20000)
    check(page.inner_text("#record") == record, "and keeps it after a reload")
