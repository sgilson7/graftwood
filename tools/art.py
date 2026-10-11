#!/usr/bin/env python3
"""Graftwood's 2D art, drawn as TikZ and compiled, not generated as pixels
(Sam's diagrams-as-code method, as Pursuer's and Long Time Comin's
tools/art.py are made; the drawing machinery here is Pursuer's).

Each picture is composed from sub-pictures defined once below (a pine, a
round tree, a mushroom, a cloud, the grafted creature, the Old Stag, a
beetle) and placed by the picture's own function. The look follows
Sam_Comm_final.png in the vagrancy repository: a warm yellow sky with
outlined clouds, salmon, coral and pink, sage and teal greens, flat fills,
everything outlined in one plum ink, and a giant in the middle distance
with debris (here, leaves) flying.

    python3 tools/art.py            # every picture: web/art/**.png, tools/art/**.tex
    python3 tools/art.py title      # one picture
    python3 tools/art.py nodes      # one folder

Facts each picture must get right (the list of what an image generator gets
wrong), and where this file enforces each:

- text: no words or letters in any picture; the page sets every word over
  the art. `document` refuses any TeX that could typeset text (\\node,
  \\text, \\put, \\char ...), so no picture can carry a letter.
- sizes: title, won and dead are 1920x1080 and opaque; the panel pictures
  (encounter, shop, rest) 1024x384 and opaque; each map icon 256x256 and the
  page icon 128x128, both with a transparent background (`SIZE`; `check`
  reads every PNG's header and refuses a wrong size, a missing alpha channel
  on a cutout, or a file over 1.5 MB).
- the creature is the same creature everywhere: `chimera` draws it (a mud
  brown body, a cream belly, one rabbit ear, a crab's pincer, a wasp's wings
  and stinger) and every picture that shows the player calls it.
- the Old Stag is the same stag everywhere: `stag` draws it, antlers, moss
  and all, standing, kneeling or as a head on the boss icon.
- calm lower left: the page sets the title and the end text there, so
  `leaves` refuses to throw a leaf into the left 40% of a scene's lower
  two thirds, and nothing tall is placed there.
- icons read at 38 px: each map icon is one bold shape with a thick outline,
  centred, clear of every edge (`check`).
- colour is never the only cue: every filled shape has the plum outline,
  and each map icon is a different shape, not only a different colour.
- repeatable: every random choice comes from a seeded generator, so the
  same script draws the same pictures.
"""
import math, random, shutil, struct, subprocess, sys, tempfile, zlib
from contextlib import contextmanager
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "web" / "art"
SRC = ROOT / "tools" / "art"
PDFLATEX = shutil.which("pdflatex") or "/Library/TeX/texbin/pdflatex"
PDFTOCAIRO = shutil.which("pdftocairo") or "/opt/homebrew/bin/pdftocairo"

# Sam_Comm_final's palette, by eye from the picture, and the woods' own
# (the same hex values as data/palette.json where a colour is shared).
COLOURS = {
    "ink": "3B2340", "sky": "EFC46A", "skyhi": "F5DA92", "skylow": "E9B060", "cloud": "F2D58A",
    "dusk": "E99A6A", "duskhi": "F0B07E", "dusklow": "C9746A",
    "coral": "E88A73", "salmon": "EE9F7E", "rose": "E47A7E", "peach": "F0B07E",
    "grass": "8DBB7E", "grassdark": "6FA77A", "grasslight": "A9C97F", "pine": "5E9C6A", "teal": "4FA3A5",
    "farpine": "B7C98A", "hill": "C9C078", "hillfar": "E3C483",
    "clearing": "B9CF86", "dirt": "D3A77D", "dirtdark": "B98A64", "stone": "A8B0A4",
    "trunk": "B9835A", "trunkdark": "8C5A44", "mushroom": "F26B38", "silk": "F6E7C4",
    "flower": "FF6F91", "flowergold": "FFB000", "leafgold": "F2B53A",
    "mud": "B9835A", "muddark": "8C5A44", "cream": "EBC592", "blush": "F08A8A", "bone": "FBF4E4",
    "crabshell": "D2463C", "crabdark": "A8323A", "wing": "CDE8E0", "goldbelt": "F2B53A",
    "stag": "A8693C", "stagdark": "7E4A2C", "antler": "E2C79A", "moss": "6FA77A", "eyeglow": "FFE38A",
    "chitin": "6B4E71", "chitinhi": "8E6E94", "acorn": "C98A55", "acorncap": "8C5A44",
    "fire": "FF8A3D", "firehi": "FFE38A", "jar": "CDE8E0", "white": "FFFFFF",
}

PREAMBLE = r"""\documentclass[tikz,border=0pt]{standalone}
\usepackage{xcolor}
% Palette: Sam_Comm_final.png by eye, and the woods' own colours.
""" + "\n".join(f"\\definecolor{{{k}}}{{HTML}}{{{v}}}" for k, v in COLOURS.items()) + "\n"

# Canvas sizes in cm and in pixels.
SIZE = {"scene": ((32.0, 18.0), (1920, 1080)), "panel": ((32.0, 12.0), (1024, 384)), "nodes": ((4.0, 4.0), (256, 256)), "icon": ((4.0, 4.0), (128, 128))}
OPAQUE = ("scene", "panel")
CX = 2.0
def f(x):
    return f"{x:.3f}".rstrip("0").rstrip(".") if abs(x) > 0.0005 else "0"


def mirror(pts, c=CX):
    return [(2 * c - x, y) for x, y in pts]


class Fig:
    """A list of inked shapes under an affine transform, so a character can
    be drawn once and placed anywhere, and its silhouette drawn again as the
    standee's white border."""

    def __init__(self, ink=1.1, T=(1, 0, 0, 1, 0, 0)):
        self.items = []
        self.ink = ink
        self.T = T

    # transforms -------------------------------------------------------
    def pt(self, x, y):
        a, b, c, d, e, g = self.T
        return (a * x + b * y + e, c * x + d * y + g)

    def k(self):
        a, b, c, d, _, _ = self.T
        return math.sqrt(abs(a * d - b * c))

    @contextmanager
    def frame(self, dx=0.0, dy=0.0, s=1.0, rot=0.0, sx=1.0, sy=1.0):
        old = self.T
        r = math.radians(rot)
        la, lb, lc, ld = s * sx * math.cos(r), -s * sy * math.sin(r), s * sx * math.sin(r), s * sy * math.cos(r)
        a, b, c, d, e, g = old
        self.T = (a * la + b * lc, a * lb + b * ld, c * la + d * lc, c * lb + d * ld, a * dx + b * dy + e, c * dx + d * dy + g)
        try:
            yield
        finally:
            self.T = old

    def P(self, x, y):
        X, Y = self.pt(x, y)
        return f"({f(X)},{f(Y)})"

    # shapes ------------------------------------------------------------
    def _add(self, kind, path, colour, opts="", sil=True, outline=True, lw=None):
        self.items.append({"kind": kind, "path": path, "colour": colour, "opts": opts, "sil": sil, "outline": outline, "lw": lw})

    def poly(self, pts, fill, **kw):
        self._add("fill", " -- ".join(self.P(x, y) for x, y in pts) + " -- cycle", fill, **kw)

    def blob(self, pts, fill, tension=0.6, **kw):
        coords = " ".join(self.P(x, y) for x, y in pts)
        self._add("fill", f"plot[smooth cycle, tension={tension}] coordinates {{{coords}}}", fill, **kw)

    def circle(self, x, y, r, fill, **kw):
        self._add("fill", f"{self.P(x, y)} circle ({f(r * self.k())})", fill, **kw)

    def ellipse(self, x, y, rx, ry, fill, **kw):
        n = 20
        self.blob([(x + rx * math.cos(2 * math.pi * i / n), y + ry * math.sin(2 * math.pi * i / n)) for i in range(n)], fill, tension=0.55, **kw)

    def rect(self, x0, y0, x1, y1, fill, **kw):
        self.poly([(x0, y0), (x1, y0), (x1, y1), (x0, y1)], fill, **kw)

    def stroke(self, pts, lw=1.0, colour="ink", smooth=False, opts="", sil=False):
        if smooth:
            path = "plot[smooth, tension=0.6] coordinates {" + " ".join(self.P(x, y) for x, y in pts) + "}"
        else:
            path = " -- ".join(self.P(x, y) for x, y in pts)
        self._add("stroke", path, colour, opts=opts, sil=sil, lw=lw * self.k())

    def raw(self, tex):
        self.items.append({"kind": "raw", "tex": tex, "sil": False})

    # output ------------------------------------------------------------
    def render(self):
        out = []
        for it in self.items:
            if it["kind"] == "raw":
                out.append(it["tex"])
            elif it["kind"] == "fill":
                if it["outline"]:
                    lw = self.ink * (it["lw"] or 1.0)
                    out.append(f"\\filldraw[draw=ink, line width={f(lw)}pt, line join=round, fill={it['colour']}{it['opts']}] {it['path']};\n")
                else:
                    out.append(f"\\fill[{it['colour']}{it['opts']}] {it['path']};\n")
            else:
                out.append(f"\\draw[{it['colour']}, line width={f(it['lw'])}pt, line cap=round, line join=round{it['opts']}] {it['path']};\n")
        return "".join(out)

    def silhouette(self, border_pt, edge_pt):
        """The standee's border: every shape again, stroked fat, first in the
        cut-edge colour and then in white, so a white band of `border_pt`
        with a thin cut line round it follows the whole character."""
        out = []
        for colour, extra in (("cutedge", 2 * edge_pt), ("white", 0.0)):
            for it in self.items:
                if not it["sil"]:
                    continue
                if it["kind"] == "fill":
                    w = 2 * border_pt + extra + self.ink
                    out.append(f"\\filldraw[draw={colour}, fill={colour}, line width={f(w)}pt, line join=round] {it['path']};\n")
                else:
                    w = it["lw"] + 2 * border_pt + extra
                    out.append(f"\\draw[{colour}, line width={f(w)}pt, line cap=round, line join=round] {it['path']};\n")
        return "".join(out)


# ---- shared parts ---------------------------------------------------------

def limb(g, pts, w, fill, **kw):
    """An arm, leg or strap: a polyline thickened to width w (a number or one
    per point), rounded."""
    ws = w if isinstance(w, (list, tuple)) else [w] * len(pts)
    left, right = [], []
    n = len(pts)
    for i, (x, y) in enumerate(pts):
        if i == 0:
            dx, dy = pts[1][0] - x, pts[1][1] - y
        elif i == n - 1:
            dx, dy = x - pts[i - 1][0], y - pts[i - 1][1]
        else:
            dx, dy = pts[i + 1][0] - pts[i - 1][0], pts[i + 1][1] - pts[i - 1][1]
        L = math.hypot(dx, dy) or 1
        nx, ny = -dy / L, dx / L
        left.append((x + nx * ws[i] / 2, y + ny * ws[i] / 2))
        right.append((x - nx * ws[i] / 2, y - ny * ws[i] / 2))
    g.blob(left + right[::-1], fill, tension=0.3, **kw)




def stare(g, x, y, r, look=(0.0, 0.0)):
    """A round eye: a white disc, a pupil looking `look`, a glint."""
    g.circle(x, y, r, "bone")
    g.circle(x + look[0] * r * 0.4, y + look[1] * r * 0.4, r * 0.42, "ink", outline=False)
    g.circle(x + look[0] * r * 0.4 + r * 0.14, y + look[1] * r * 0.4 + r * 0.16, r * 0.12, "white", outline=False)


def shut(g, x, y, r):
    """A closed eye: one curved line."""
    g.stroke([(x - r, y), (x, y - r * 0.45), (x + r, y)], lw=1.6, smooth=True)


def crossed(g, x, y, r):
    """A fainted eye: two crossed strokes."""
    g.stroke([(x - r, y - r), (x + r, y + r)], lw=1.8)
    g.stroke([(x - r, y + r), (x + r, y - r)], lw=1.8)


def sky(g, W, H, top="skyhi", mid="sky", low="skylow"):
    g.raw(f"\\fill[{mid}] (0,0) rectangle ({f(W)},{f(H)});\n")
    g.raw(f"\\fill[{top}] (0,{f(H * 0.8)}) rectangle ({f(W)},{f(H)});\n")
    g.raw(f"\\fill[{low}] (0,0) rectangle ({f(W)},{f(H * 0.4)});\n")


def cloud(g, x, y, s):
    """Puffs, outlined, as the clouds over Sam_Comm_final's city."""
    for dx, dy, r in [(-1.0, 0, 0.55), (-0.45, 0.25, 0.7), (0.2, 0.35, 0.8), (0.85, 0.15, 0.6), (1.35, -0.05, 0.45)]:
        g.circle(x + dx * s, y + dy * s, r * s, "cloud")
    g.rect(x - 1.0 * s, y - 0.15 * s, x + 1.35 * s, y + 0.05 * s, "cloud", outline=False)
    g.stroke([(x - 1.25 * s, y - 0.15 * s), (x + 1.6 * s, y - 0.15 * s)], lw=1.1)


def hills(g, rng, W, y, colour, r=(2.0, 3.6)):
    x = -1.0
    while x < W + 1:
        rr = rng.uniform(*r)
        g.circle(x, y, rr, colour)
        x += rr * rng.uniform(1.0, 1.5)
    g.rect(-1, y - 4, W + 1, y, colour, outline=False)


def ground(g, W, y, colour="grass"):
    g.raw(f"\\filldraw[draw=ink, line width=1.1pt, fill={colour}] (-0.5,-0.5) rectangle ({f(W + 0.5)},{f(y)});\n")


def pine(g, x, y, s, colour):
    g.rect(x - 0.12 * s, y, x + 0.12 * s, y + 0.7 * s, "trunkdark", lw=0.8)
    for w, h0 in [(1.0, 0.5), (0.8, 1.15), (0.56, 1.8)]:
        g.poly([(x - w * s, y + h0 * s), (x + w * s, y + h0 * s), (x, y + (h0 + 1.15) * s)], colour)


def round_tree(g, x, y, s, crown):
    g.rect(x - 0.13 * s, y, x + 0.13 * s, y + 1.4 * s, "trunk", lw=0.8)
    for dx, dy, r in [(-0.5, 1.55, 0.62), (0.5, 1.65, 0.66), (0, 2.2, 0.74)]:
        g.circle(x + dx * s, y + dy * s, r * s, crown)


def treeline(g, rng, W, y, s=(1.3, 2.0), skip=(), crowns=("coral", "salmon", "rose", "peach"), pines=("pine", "grassdark", "teal")):
    x = -0.5
    while x < W + 1:
        size = rng.uniform(*s)
        if not any(a <= x <= b for a, b in skip):
            if rng.random() < 0.45:
                round_tree(g, x, y, size, rng.choice(crowns))
            else:
                pine(g, x, y, size, rng.choice(pines))
        x += size * rng.uniform(0.9, 1.4)


def mushroom(g, x, y, s, cap="mushroom"):
    g.blob([(x - 0.13 * s, y), (x + 0.13 * s, y), (x + 0.1 * s, y + 0.5 * s), (x - 0.1 * s, y + 0.5 * s)], "silk", tension=0.3)
    pts = [(x + 0.48 * s * math.cos(math.radians(a)), y + 0.45 * s + 0.36 * s * math.sin(math.radians(a))) for a in range(0, 181, 15)]
    g.poly(pts, cap)
    for dx, dy, r in [(-0.2, 0.6, 0.07), (0.12, 0.7, 0.06), (0.26, 0.55, 0.05)]:
        g.circle(x + dx * s, y + dy * s, r * s, "silk", lw=0.6)


def tuft(g, x, y, s, colour="grassdark"):
    for dx, h in [(-0.15, 0.35), (0, 0.5), (0.15, 0.32)]:
        g.poly([(x + dx * s - 0.06 * s, y), (x + dx * s + 0.06 * s, y), (x + dx * s * 1.6, y + h * s)], colour, lw=0.7)


def flower(g, x, y, s, colour):
    g.stroke([(x, y), (x, y + 0.4 * s)], lw=1.0, colour="grassdark")
    for k in range(5):
        a = math.radians(90 + k * 72)
        g.circle(x + 0.12 * s * math.cos(a), y + 0.45 * s + 0.12 * s * math.sin(a), 0.09 * s, colour, lw=0.6)
    g.circle(x, y + 0.45 * s, 0.06 * s, "flowergold", lw=0.5)


def leaf(g, x, y, s, rot, colour):
    with g.frame(x, y, s, rot):
        g.blob([(-0.32, 0), (0, 0.13), (0.32, 0), (0, -0.13)], colour, tension=0.5, lw=0.8)
        g.stroke([(-0.3, 0), (0.3, 0)], lw=0.6)


def leaves(g, rng, W, H, n, box, colours=("coral", "leafgold", "rose", "grasslight")):
    """Leaves flying, after the debris round Sam_Comm_final's monster; none
    in the calm lower left, where the page sets its words."""
    x0, y0, x1, y1 = box
    placed = 0
    while placed < n:
        x, y = rng.uniform(x0, x1), rng.uniform(y0, y1)
        if x < 0.4 * W and y < H * 2 / 3:
            continue
        leaf(g, x, y, rng.uniform(0.6, 1.2), rng.uniform(0, 360), rng.choice(colours))
        placed += 1


def chimera(g, eyes="open", raised=False):
    """The player's creature, grafted from what it beat: a round mud-brown
    body and cream belly, one rabbit ear, a wasp's wings and stinger, and a
    crab's pincer. Facing +x, feet at y 0, the body a unit in radius."""
    # wings behind
    with g.frame(-0.45, 2.0, 1.0, 32):
        g.ellipse(0, 0, 1.15, 0.42, "wing")
        g.stroke([(-0.9, 0), (0.9, 0)], lw=0.6)
    with g.frame(-0.75, 1.75, 1.0, 12):
        g.ellipse(0, 0, 0.85, 0.32, "wing")
    # stinger
    limb(g, [(-0.7, 1.0), (-1.35, 0.85), (-1.95, 0.62)], [0.62, 0.4, 0.04], "goldbelt")
    for x0 in (-1.05, -1.4):
        g.stroke([(x0, 1.1 + (x0 + 1.05) * 0.3), (x0 - 0.12, 0.62 + (x0 + 1.05) * 0.2)], lw=1.2)
    # feet
    g.ellipse(-0.45, 0.12, 0.32, 0.14, "muddark")
    g.ellipse(0.5, 0.12, 0.34, 0.14, "muddark")
    # the back ear, the body, the belly
    with g.frame(-0.3, 2.2, 1.0, 14):
        g.ellipse(0, 0, 0.2, 0.62, "mud")
    g.circle(0, 1.1, 1.0, "mud")
    g.ellipse(0.3, 0.8, 0.62, 0.5, "cream")
    with g.frame(0.12, 2.35, 1.0, -14):
        g.ellipse(0, 0, 0.23, 0.78, "mud")
        g.ellipse(0.02, -0.02, 0.1, 0.55, "blush", lw=0.6)
    # the eye and the mouth
    if eyes == "open":
        stare(g, 0.55, 1.45, 0.3, look=(0.6, 0.1))
    elif eyes == "shut":
        shut(g, 0.55, 1.42, 0.22)
    else:
        crossed(g, 0.55, 1.45, 0.17)
    g.blob([(0.62, 1.0), (0.98, 1.06), (0.92, 0.82), (0.7, 0.8)], "ink", tension=0.4)
    g.rect(0.7, 0.92, 0.79, 1.03, "bone", lw=0.5)
    g.rect(0.82, 0.93, 0.91, 1.04, "bone", lw=0.5)
    # the pincer, held out or raised
    tip = (1.55, 1.75) if raised else (1.55, 1.05)
    limb(g, [(0.55, 0.85), (1.05, 0.9 if not raised else 1.25), (tip[0] - 0.15, tip[1] - 0.1)], 0.24, "crabshell")
    with g.frame(tip[0] + 0.1, tip[1], 1.0, 45 if raised else 10):
        g.blob([(-0.3, -0.2), (0.15, -0.32), (0.55, -0.12), (0.35, 0.02), (-0.2, 0.05)], "crabshell", tension=0.5)
        g.blob([(-0.25, 0.1), (0.2, 0.08), (0.5, 0.35), (0.12, 0.38), (-0.3, 0.3)], "crabshell", tension=0.5)
        g.circle(-0.28, 0.06, 0.24, "crabdark")


def stag(g, kneel=False, eyes_open=True):
    """The Old Stag, oldest body in the wood and carrying too much: antlers
    like a crown, moss and mushrooms on its back. Facing −x, hooves at y 0."""
    lift = 0.75 if kneel else 2.4
    if not kneel:
        for x0 in (-2.3, -1.7, 1.5, 2.1):
            g.rect(x0 - 0.18, 0.15, x0 + 0.18, lift + 0.4, "stagdark")
            g.rect(x0 - 0.2, 0.0, x0 + 0.2, 0.22, "ink", lw=0.6)
    body_y = lift + 0.8
    hy = body_y + (0.6 if kneel else 2.4)
    hx = -3.4 if not kneel else -3.6
    # the neck first, so the body covers its root
    limb(g, [(-1.6, body_y + 0.2), (-2.6, body_y + 0.9), (hx + 0.2, hy - 0.2)], [1.6, 1.15, 0.8], "stag")
    g.circle(2.95, body_y + 0.45, 0.28, "cream")
    g.ellipse(0, body_y, 2.9, 1.3, "stag")
    g.ellipse(-2.0, body_y + 0.15, 0.75, 0.62, "cream")
    for dx, dy, r in [(-1.6, 1.05, 0.5), (-0.6, 1.25, 0.56), (0.5, 1.2, 0.5), (1.5, 0.95, 0.42)]:
        g.circle(dx, body_y + dy, r, "moss")
    for dx in (-0.9, 0.9):
        mushroom(g, dx, body_y + 1.45, 0.7)
    if kneel:
        for x0 in (-1.9, 1.7):
            g.ellipse(x0, 0.3, 1.05, 0.32, "stagdark")
    # antlers: two beams curving up and back, tines off each
    for (bx, beam) in ((hx + 0.35, [(0, 0), (0.45, 1.1), (1.0, 2.1), (1.8, 2.7)]), (hx - 0.25, [(0, 0), (-0.7, 1.0), (-1.1, 2.1), (-0.8, 2.9)])):
        pts = [(bx + dx, hy + 0.45 + dy) for dx, dy in beam]
        limb(g, pts, [0.34, 0.28, 0.2, 0.1], "antler")
        for (px, py), (tx, ty) in zip(pts[1:3], [(-0.55, 0.85), (0.6, 0.75)] if beam[1][0] > 0 else [(-0.6, 0.6), (0.55, 0.9)]):
            limb(g, [(px, py), (px + tx, py + ty)], [0.2, 0.06], "antler")
    with g.frame(hx, hy, 1.0, -18):
        g.ellipse(0, 0, 0.95, 0.62, "stag")
        g.ellipse(-0.75, -0.25, 0.48, 0.36, "cream")
        g.circle(-1.15, -0.2, 0.13, "ink", outline=False)
        with g.frame(0.55, 0.5, 1.0, 35):
            g.ellipse(0, 0, 0.45, 0.18, "stag")
        if eyes_open:
            g.circle(-0.2, 0.15, 0.17, "eyeglow")
            g.circle(-0.24, 0.15, 0.07, "ink", outline=False)
        else:
            shut(g, -0.2, 0.15, 0.16)


def beetle(g, horn=False):
    """A beetle on six legs, facing −x, feet at y 0, about 2.4 long."""
    for x0 in (-0.7, 0.0, 0.7):
        g.stroke([(x0, 0.6), (x0 - 0.25, 0.3), (x0 - 0.1, 0.0)], lw=1.8)
    g.ellipse(0.2, 0.75, 1.15, 0.65, "chitin")
    g.stroke([(0.2, 1.4), (0.2, 0.25)], lw=1.0)
    g.ellipse(0.55, 1.0, 0.35, 0.18, "chitinhi", outline=False)
    g.circle(-1.05, 0.7, 0.42, "chitin")
    if horn:
        limb(g, [(-1.3, 0.95), (-1.7, 1.45), (-1.6, 1.85)], [0.24, 0.14, 0.04], "chitin")
    stare(g, -1.2, 0.82, 0.13, look=(-0.6, 0))
    g.stroke([(-1.35, 1.05), (-1.7, 1.5)], lw=1.0)
    g.stroke([(-1.2, 1.1), (-1.4, 1.6)], lw=1.0)


# ---- the pictures -----------------------------------------------------------

def title():
    """The clearing in Sam_Comm_final's light. A grafted creature squares up
    to a beetle while the Old Stag looms over the treeline behind, antlers in
    the clouds, leaves flying. The lower left stays calm for the title."""
    W, H = 32.0, 18.0
    rng = random.Random(2026)
    g = Fig(ink=1.1)
    sky(g, W, H)
    for cx, cy, cs in [(3.5, 15.8, 1.3), (9.6, 13.6, 0.85), (15.0, 16.4, 1.0), (27.6, 16.2, 1.4), (30.8, 12.8, 0.9)]:
        cloud(g, cx, cy, cs)
    hills(g, rng, W, 6.6, "hillfar")
    treeline(g, rng, W, 6.0, s=(0.8, 1.1), crowns=("farpine", "hill"), pines=("farpine",))
    with g.frame(22.0, 4.4, 1.15):
        stag(g)
    treeline(g, rng, W, 4.6, s=(1.4, 2.1), skip=((17.2, 26.8),))
    for x in (17.6, 19.3, 24.4, 26.2):
        pine(g, x, 4.6, 1.0, rng.choice(("pine", "grassdark", "teal")))
    ground(g, W, 4.8)
    g.ellipse(21.5, 2.3, 9.0, 2.0, "clearing")
    g.poly([(19.2, -0.5), (24.0, -0.5), (22.6, 1.6), (20.8, 1.6)], "dirt")
    for x in rng.sample(range(1, 12), 6):
        tuft(g, x + rng.random(), rng.uniform(0.5, 3.8), 1.0)
    for (x, y, s) in [(27.4, 1.0, 1.0), (28.3, 1.6, 0.7), (13.8, 3.0, 0.8), (30.5, 2.6, 0.9), (15.0, 1.0, 0.6)]:
        mushroom(g, x, y, s)
    for k in range(9):
        flower(g, rng.uniform(14, 31), rng.uniform(0.6, 3.8), 1.0, rng.choice(("flower", "flowergold", "bone")))
    with g.frame(19.2, 1.3, 1.45):
        chimera(g)
    with g.frame(25.2, 1.4, 1.05):
        beetle(g, horn=True)
    leaves(g, rng, W, H, 34, (13.0, 4.5, 31.5, 16.5))
    return g, "scene"


def won():
    """The Old Stag kneels, eyes closed; the grafted creature stands on a
    stump with its pincer raised, sun rays behind, gold leaves falling."""
    W, H = 32.0, 18.0
    rng = random.Random(77)
    g = Fig(ink=1.1)
    sky(g, W, H)
    for k in range(14):
        a0, a1 = math.radians(k * 360 / 14), math.radians(k * 360 / 14 + 360 / 28)
        g.poly([(21, 7), (21 + 40 * math.cos(a0), 7 + 40 * math.sin(a0)), (21 + 40 * math.cos(a1), 7 + 40 * math.sin(a1))], "skyhi", outline=False)
    for cx, cy, cs in [(5.0, 15.5, 1.2), (28.5, 15.8, 1.3)]:
        cloud(g, cx, cy, cs)
    hills(g, rng, W, 6.4, "hillfar")
    treeline(g, rng, W, 5.0, s=(1.3, 2.0), skip=((15.0, 30.0),))
    ground(g, W, 5.2)
    g.ellipse(21.5, 2.5, 9.5, 2.3, "clearing")
    with g.frame(26.5, 2.0, 0.95):
        stag(g, kneel=True, eyes_open=False)
    g.blob([(15.3, 0.8), (18.3, 0.8), (18.1, 2.6), (15.5, 2.6)], "trunk", tension=0.2)
    g.ellipse(16.8, 2.6, 1.35, 0.35, "dirt")
    for r in (0.9, 0.55, 0.25):
        g.ellipse(16.8, 2.6, r * 1.35, r * 0.35, "dirt", lw=0.6)
    with g.frame(16.7, 2.8, 1.5):
        chimera(g, raised=True)
    for (x, y, s) in [(14.6, 1.2, 0.8), (29.5, 1.0, 1.0), (30.6, 1.8, 0.7)]:
        mushroom(g, x, y, s)
    leaves(g, rng, W, H, 30, (13.0, 3.0, 31.5, 17.0), colours=("leafgold", "flowergold", "peach"))
    return g, "scene"


def dead():
    """Dusk. The grafted creature lies on its side in the clearing, eyes
    crossed, and mushrooms have begun to grow from it."""
    W, H = 32.0, 18.0
    rng = random.Random(13)
    g = Fig(ink=1.1)
    sky(g, W, H, top="duskhi", mid="dusk", low="dusklow")
    for cx, cy, cs in [(6.0, 15.2, 1.1), (19.0, 16.0, 1.3), (28.0, 14.2, 1.0)]:
        cloud(g, cx, cy, cs)
    hills(g, rng, W, 6.4, "coral")
    treeline(g, rng, W, 5.0, s=(1.4, 2.2), crowns=("rose", "coral", "trunkdark"), pines=("teal", "pine"))
    ground(g, W, 5.2, colour="grassdark")
    g.ellipse(21.5, 2.4, 9.0, 2.1, "grass")
    with g.frame(22.2, 2.3, 1.6, 90):
        chimera(g, eyes="crossed")
    for (x, y, s) in [(19.6, 3.6, 0.7), (20.8, 3.9, 0.55), (21.9, 3.6, 0.65), (24.4, 1.6, 0.8), (17.0, 1.3, 0.6)]:
        mushroom(g, x, y, s)
    for k in range(16):
        x, y = rng.uniform(13.5, 31), rng.uniform(4, 15)
        g.circle(x, y, 0.09, "eyeglow", lw=0.5)
    leaves(g, rng, W, H, 18, (13.0, 2.0, 31.5, 16.0), colours=("trunkdark", "coral", "rose"))
    return g, "scene"


def panel_scene(g, rng, W, H):
    sky(g, W, H)
    cloud(g, 4.0, 10.2, 0.9)
    cloud(g, 27.0, 10.6, 1.0)
    hills(g, rng, W, 5.0, "hillfar", r=(1.6, 2.6))
    treeline(g, rng, W, 3.6, s=(1.2, 1.8))
    ground(g, W, 3.8)


def encounter():
    """A fork in the path: a signpost with two blank boards, fireflies, and a
    ring of mushrooms that might be anything."""
    W, H = 32.0, 12.0
    rng = random.Random(5)
    g = Fig(ink=1.1)
    panel_scene(g, rng, W, H)
    g.poly([(14.0, -0.5), (18.0, -0.5), (16.6, 2.2), (15.4, 2.2)], "dirt")
    g.poly([(15.4, 2.2), (16.6, 2.2), (10.0, 3.8), (8.6, 3.8)], "dirt")
    g.poly([(15.4, 2.2), (16.6, 2.2), (24.2, 3.8), (22.8, 3.8)], "dirt")
    g.rect(15.85, 1.2, 16.15, 6.4, "trunk")
    g.poly([(16.1, 5.6), (19.0, 5.6), (19.7, 5.15), (19.0, 4.7), (16.1, 4.7)], "trunk")
    g.poly([(15.9, 4.3), (13.0, 4.3), (12.3, 3.85), (13.0, 3.4), (15.9, 3.4)], "trunk")
    for k in range(7):
        a = math.radians(k * 360 / 7)
        mushroom(g, 25.5 + 2.2 * math.cos(a), 1.6 + 0.6 * math.sin(a), 0.9)
    for k in range(20):
        g.circle(rng.uniform(1, 31), rng.uniform(4.2, 11), 0.11, "eyeglow", lw=0.5)
    for k in range(6):
        tuft(g, rng.uniform(1, 12), rng.uniform(0.4, 3.2), 1.0)
    with g.frame(6.0, 0.8, 1.0):
        chimera(g)
    return g, "panel"


def shop():
    """The trader: a beetle with a cart of jars, each holding a spare part."""
    W, H = 32.0, 12.0
    rng = random.Random(9)
    g = Fig(ink=1.1)
    panel_scene(g, rng, W, H)
    # the cart
    g.rect(13.0, 1.6, 21.0, 4.2, "trunk")
    for x in (13.0, 16.0, 19.0):
        g.stroke([(x + 0.2, 1.7), (x + 0.2, 4.1)], lw=0.8)
    g.poly([(12.4, 4.2), (21.6, 4.2), (20.8, 5.0), (13.2, 5.0)], "trunkdark")
    for (x, part) in ((14.2, "ear"), (16.6, "claw"), (19.0, "wing")):
        g.blob([(x - 0.8, 4.2), (x + 0.8, 4.2), (x + 0.75, 6.4), (x - 0.75, 6.4)], "jar", tension=0.15)
        g.rect(x - 0.6, 6.4, x + 0.6, 6.75, "acorncap")
        if part == "ear":
            with g.frame(x, 5.3, 1.0, -10):
                g.ellipse(0, 0, 0.22, 0.75, "mud")
                g.ellipse(0, 0, 0.1, 0.55, "blush", lw=0.6)
        elif part == "claw":
            with g.frame(x - 0.1, 5.2, 1.1, 30):
                g.blob([(-0.3, -0.2), (0.15, -0.32), (0.55, -0.12), (0.35, 0.02), (-0.2, 0.05)], "crabshell", tension=0.5)
                g.blob([(-0.25, 0.1), (0.2, 0.08), (0.5, 0.35), (0.12, 0.38), (-0.3, 0.3)], "crabshell", tension=0.5)
        else:
            with g.frame(x, 5.3, 1.0, 30):
                g.ellipse(0, 0, 0.6, 0.25, "wing")
        g.stroke([(x - 0.5, 6.1), (x - 0.5, 4.6)], lw=1.4, colour="white", opts=", opacity=0.8")
    for x in (14.4, 19.6):
        g.circle(x, 1.5, 0.9, "trunkdark")
        g.circle(x, 1.5, 0.25, "acorn")
    with g.frame(24.8, 0.9, 1.5):
        beetle(g, horn=True)
    for (x, y, s) in [(9.5, 1.0, 0.9), (29.5, 1.2, 1.0), (30.5, 2.4, 0.6)]:
        mushroom(g, x, y, s)
    with g.frame(4.2, 0.8, 0.9):
        chimera(g)
    return g, "panel"


def rest():
    """A quiet hollow: a mossy log, and the creature asleep beside it."""
    W, H = 32.0, 12.0
    rng = random.Random(3)
    g = Fig(ink=1.1)
    panel_scene(g, rng, W, H)
    g.rect(12.0, 1.0, 22.0, 4.0, "trunk")
    for y in (1.8, 2.6, 3.3):
        g.stroke([(12.4, y), (21.0, y + 0.1)], lw=0.7)
    g.ellipse(22.0, 2.5, 0.75, 1.5, "trunkdark")
    g.ellipse(22.0, 2.5, 0.5, 1.15, "ink")
    for dx, r in [(-0.3, 0.6), (2.0, 0.75), (4.5, 0.55), (7.0, 0.7), (9.3, 0.5)]:
        g.circle(12.3 + dx, 4.0, r, "moss")
    mushroom(g, 15.0, 4.3, 0.8)
    mushroom(g, 18.6, 4.4, 0.6)
    with g.frame(25.2, 0.5, 1.2):
        chimera(g, eyes="shut")
    for k, (x, y, r) in enumerate([(27.2, 4.4, 0.16), (27.7, 5.0, 0.22), (28.4, 5.7, 0.3)]):
        g.circle(x, y, r, "bone", lw=0.7)
    for (x, y, s) in [(6.0, 1.0, 0.9), (8.0, 1.6, 0.6), (30.0, 1.2, 0.8)]:
        mushroom(g, x, y, s)
    return g, "panel"


def icon_battle():
    """Two pincers crossed."""
    g = Fig(ink=2.6)
    for rot, sx in ((40, 1), (-40, -1)):
        with g.frame(2.0, 2.0, 1.0, rot, sx=sx):
            limb(g, [(-0.2, -1.4), (0.0, -0.2)], 0.36, "crabshell")
            with g.frame(0.05, 0.55, 1.5, 90):
                g.blob([(-0.3, -0.2), (0.15, -0.32), (0.55, -0.12), (0.35, 0.02), (-0.2, 0.05)], "crabshell", tension=0.5)
                g.blob([(-0.25, 0.1), (0.2, 0.08), (0.5, 0.35), (0.12, 0.38), (-0.3, 0.3)], "crabshell", tension=0.5)
    return g, "nodes"


def icon_elite():
    """A horned beetle's head, glaring."""
    g = Fig(ink=2.6)
    for sx in (1, -1):
        limb(g, [(2.0 + sx * 0.55, 2.4), (2.0 + sx * 1.15, 3.0), (2.0 + sx * 0.95, 3.55)], [0.42, 0.26, 0.06], "chitin")
    g.circle(2.0, 1.75, 1.15, "chitin")
    for sx in (1, -1):
        g.poly([(2.0 + sx * 0.2, 1.9), (2.0 + sx * 0.85, 2.2), (2.0 + sx * 0.75, 1.55)], "crabshell")
    g.blob([(1.55, 1.0), (2.45, 1.0), (2.25, 0.75), (1.75, 0.75)], "ink", tension=0.3)
    return g, "nodes"


def sparkle(g, x, y, r, colour):
    pts = []
    for k in range(8):
        a = math.radians(90 + k * 45)
        rr = r if k % 2 == 0 else r * 0.3
        pts.append((x + rr * math.cos(a), y + rr * math.sin(a)))
    g.poly(pts, colour, lw=0.6)


def icon_encounter():
    """A mushroom with sparkles round it: something strange on the path."""
    g = Fig(ink=2.6)
    mushroom(g, 2.0, 0.55, 2.6)
    sparkle(g, 0.75, 2.9, 0.42, "eyeglow")
    sparkle(g, 3.3, 3.05, 0.36, "eyeglow")
    sparkle(g, 3.35, 1.25, 0.28, "eyeglow")
    return g, "nodes"


def icon_shop():
    """An acorn: what the trader takes."""
    g = Fig(ink=2.6)
    g.blob([(1.15, 2.2), (2.85, 2.2), (2.75, 1.3), (2.0, 0.45), (1.25, 1.3)], "acorn", tension=0.55)
    g.blob([(0.95, 2.1), (3.05, 2.1), (2.9, 2.85), (2.0, 3.2), (1.1, 2.85)], "acorncap", tension=0.5)
    for x in (1.5, 2.0, 2.5):
        g.stroke([(x - 0.2, 2.2), (x + 0.2, 3.0)], lw=1.4)
    g.rect(1.88, 3.1, 2.12, 3.6, "acorncap")
    g.stroke([(1.55, 1.9), (1.6, 1.2)], lw=2.0, colour="cream")
    return g, "nodes"


def icon_rest():
    """A small fire on crossed logs."""
    g = Fig(ink=2.6)
    for rot in (20, -20):
        with g.frame(2.0, 0.95, 1.0, rot):
            g.rect(-1.4, -0.22, 1.4, 0.22, "trunk")
    g.blob([(1.15, 1.2), (2.85, 1.2), (2.7, 2.2), (2.25, 2.6), (2.1, 3.5), (1.6, 2.6), (1.25, 2.1)], "fire", tension=0.5)
    g.blob([(1.6, 1.25), (2.4, 1.25), (2.35, 1.9), (2.0, 2.5), (1.65, 1.9)], "firehi", tension=0.5)
    return g, "nodes"


def icon_boss():
    """The Old Stag's head, front on, antlers wide."""
    g = Fig(ink=2.4)
    for sx in (1, -1):
        pts = [(2.0 + sx * 0.4, 2.2), (2.0 + sx * 0.95, 2.8), (2.0 + sx * 1.35, 3.6)]
        limb(g, pts, [0.3, 0.22, 0.1], "antler")
        limb(g, [pts[1], (2.0 + sx * 0.7, 3.5)], [0.2, 0.06], "antler")
        limb(g, [(2.0 + sx * 1.15, 3.2), (2.0 + sx * 1.7, 3.1)], [0.16, 0.05], "antler")
        with g.frame(2.0 + sx * 0.85, 2.15, 1.0, sx * 30):
            g.ellipse(0, 0, 0.4, 0.16, "stag")
    g.blob([(1.35, 2.5), (2.65, 2.5), (2.5, 1.3), (2.0, 0.45), (1.5, 1.3)], "stag", tension=0.5)
    g.ellipse(2.0, 0.8, 0.38, 0.3, "cream")
    g.circle(2.0, 0.88, 0.12, "ink", outline=False)
    for sx in (1, -1):
        g.circle(2.0 + sx * 0.33, 1.8, 0.15, "eyeglow")
    return g, "nodes"


def icon_page():
    """The page's icon: the creature, whole."""
    g = Fig(ink=2.2)
    with g.frame(2.0, 0.85, 0.72):
        chimera(g)
    return g, "icon"


PICTURES = {
    "title": title, "won": won, "dead": dead,
    "encounter": encounter, "shop": shop, "rest": rest,
    "nodes/battle": icon_battle, "nodes/elite": icon_elite, "nodes/encounter": icon_encounter,
    "nodes/shop": icon_shop, "nodes/rest": icon_rest, "nodes/boss": icon_boss,
    "icon": icon_page,
}

FORBIDDEN = ["\\node", "\\text", "\\put", "\\char", "\\pgftext", "label=", "\\Large", "\\huge", "\\LaTeX"]


def document(fig, kind):
    (W, H), _ = SIZE[kind]
    body = fig.render()
    for bad in FORBIDDEN:
        if bad in body:
            raise SystemExit(f"text-setting command {bad!r} in a picture: no words in the art")
    return (PREAMBLE + "\\begin{document}\n\\begin{tikzpicture}\n" + f"\\clip (0,0) rectangle ({f(W)},{f(H)});\n" + body + "\\end{tikzpicture}\n\\end{document}\n")


# ---- checks ----------------------------------------------------------------

def png_read(path):
    """Width, height, colour type and the RGBA rows of a PNG (stdlib only)."""
    data = Path(path).read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path
    pos, idat = 8, b""
    while pos < len(data):
        n, = struct.unpack(">I", data[pos:pos + 4])
        tag = data[pos + 4:pos + 8]
        chunk = data[pos + 8:pos + 8 + n]
        if tag == b"IHDR":
            w, h, depth, ctype = struct.unpack(">IIBB", chunk[:10])
        elif tag == b"IDAT":
            idat += chunk
        pos += 12 + n
    bpp = {6: 4, 2: 3}.get(ctype)
    if depth != 8 or bpp is None:
        return w, h, ctype, None
    raw = zlib.decompress(idat)
    stride = w * bpp
    rows, prev = [], bytearray(stride)
    for y in range(h):
        ft = raw[y * (stride + 1)]
        line = bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if ft == 1:
                line[i] = (line[i] + a) & 255
            elif ft == 2:
                line[i] = (line[i] + b) & 255
            elif ft == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            elif ft == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    return w, h, ctype, rows


def check(name, kind, png, preview):
    """Refuse a picture that breaks the facts in the docstring."""
    _, (PW, PH) = SIZE[kind]
    w, h, ctype, _ = png_read(png)
    problems = []
    if (w, h) != (PW, PH):
        problems.append(f"size {w}x{h}, wanted {PW}x{PH}")
    if Path(png).stat().st_size > 1.5e6:
        problems.append(f"{Path(png).stat().st_size} bytes, over 1.5 MB")
    if kind not in OPAQUE:
        if ctype != 6:
            problems.append("no alpha channel on a cutout")
        pw, ph, _, rows = png_read(preview)
        alpha = [[row[i * 4 + 3] for i in range(pw)] for row in rows]
        solid = [(x, y) for y in range(ph) for x in range(pw) if alpha[y][x] > 40]
        if any(alpha[y][x] for y in (0, ph - 1) for x in (0, pw - 1)) and kind == "items":
            problems.append("a corner is not transparent")
        if max(alpha[0]) > 0:
            problems.append("something touches the top edge (a head or prop is cropped)")
        if max(r[0] for r in alpha) > 0 or max(r[-1] for r in alpha) > 0:
            problems.append("something touches a side edge")
        ys = [y for _, y in solid]
        xs = [x for x, _ in solid]
        if kind == "monsters":
            if max(alpha[-1]) == 0:
                problems.append("the feet do not reach the bottom edge")
            if (max(ys) - min(ys) + 1) < 0.9 * ph:
                problems.append("the figure fills less than 90% of the height")
            if alpha[0][0] or alpha[0][-1]:
                problems.append("a top corner is not transparent")
        else:
            if max(alpha[-1]) > 0:
                problems.append("something touches the bottom edge")
            cx, cy = (min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2
            if abs(cx - pw / 2) > pw * 0.08 or abs(cy - ph / 2) > ph * 0.08:
                problems.append(f"not centred (centre at {cx:.0f},{cy:.0f} of {pw}x{ph})")
    return [f"{name}: {p}" for p in problems]


def main():
    only = set(sys.argv[1:])
    failures = []
    for name, make in PICTURES.items():
        if only and name not in only and name.split("/")[0] not in only:
            continue
        fig, kind = make()
        tex = document(fig, kind)
        _, (PW, PH) = SIZE[kind]
        clear = [] if kind in OPAQUE else ["-transp"]
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            (t / "art.tex").write_text(tex)
            r = subprocess.run([PDFLATEX, "-interaction=nonstopmode", "-halt-on-error", "art.tex"], cwd=t, capture_output=True, text=True)
            if r.returncode != 0:
                print(r.stdout[-3000:])
                raise SystemExit(f"{name}: pdflatex failed")
            subprocess.run([PDFTOCAIRO, "-png", *clear, "-scale-to-x", str(PW), "-scale-to-y", str(PH), "-singlefile", str(t / "art.pdf"), str(t / "art")], check=True)
            subprocess.run([PDFTOCAIRO, "-png", *clear, "-scale-to-x", str(PW // 8), "-scale-to-y", str(PH // 8), "-singlefile", str(t / "art.pdf"), str(t / "small")], check=True)
            out = OUT / f"{name}.png"
            out.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(t / "art.png", out)
            src = SRC / f"{name}.tex"
            src.parent.mkdir(parents=True, exist_ok=True)
            src.write_text(tex)
            # written first so a refused picture can be looked at; the run still fails
            problems = check(name, kind, out, t / "small.png")
        print("wrote", out.relative_to(ROOT), f"{out.stat().st_size // 1024} KB", "REFUSED" if problems else "")
        failures += problems
    if failures:
        raise SystemExit("\n".join(failures))


if __name__ == "__main__":
    main()
