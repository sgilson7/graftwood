#!/usr/bin/env python3
"""Screenshots of the built game, for checking the picture by eye and for the
website's tile. Serves dist/, drives the page in Chromium, and saves PNGs.

    .venv/bin/python tools/shots.py [outdir]
"""
import functools, http.server, pathlib, sys, threading
from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "shots"
OUT.mkdir(parents=True, exist_ok=True)


class Quiet(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm", ".js": "text/javascript"}

    def log_message(self, *a):
        pass


server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Quiet, directory=str(ROOT / "dist")))
threading.Thread(target=server.serve_forever, daemon=True).start()
base = f"http://127.0.0.1:{server.server_address[1]}"

with sync_playwright() as p:
    b = p.chromium.launch(args=["--use-angle=metal", "--enable-gpu", "--ignore-gpu-blocklist"])
    page = b.new_page(viewport={"width": 1600, "height": 900})
    errors = []
    page.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
    page.on("pageerror", lambda e: errors.append(str(e)))
    page.goto(base + "/index.html")
    page.wait_for_selector("body[data-ready='true']", state="attached")
    page.wait_for_timeout(1200)
    shot = lambda name: page.screenshot(path=str(OUT / f"{name}.png"))
    mode = lambda: page.evaluate("document.body.dataset.mode")
    shot("01-title")
    page.click("#begin")
    page.wait_for_timeout(800)
    shot("02-choose")
    page.click("#choose-go")
    page.wait_for_timeout(800)
    shot("03-map")
    page.click("#nodes .node.open")
    page.wait_for_timeout(2500)
    shot("04-battle")
    page.click("#speeds .speed:nth-of-type(3)")
    page.wait_for_function("() => !document.getElementById('result').hidden", timeout=60000)
    page.wait_for_timeout(1200)
    shot("05-result")
    page.click("#conclude")
    page.wait_for_timeout(600)
    shot("06-" + mode())
    print("errors:", errors)
    b.close()
