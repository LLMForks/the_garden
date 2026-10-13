#!/usr/bin/env python3
"""Procedural pixel-art generator for The Garden.

Generates every sprite the game uses into assets/sprites and the island
layout into assets/map/island.txt. Deterministic (fixed seeds), so running
it again reproduces the same files.

    python3 tools/gen_art.py
"""
import math
import os
import random

import numpy as np
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SPR = os.path.join(ROOT, "assets", "sprites")
MAPDIR = os.path.join(ROOT, "assets", "map")
os.makedirs(SPR, exist_ok=True)
os.makedirs(MAPDIR, exist_ok=True)

T = 16  # tile size in pixels


def hx(s, a=255):
    s = s.lstrip("#")
    return (int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16), a)


OUTLINE = hx("#2a2033")


def save(img, name):
    img.save(os.path.join(SPR, name))


def new(w, h):
    return Image.new("RGBA", (w, h), (0, 0, 0, 0))


def put(img, x, y, c):
    if 0 <= x < img.width and 0 <= y < img.height and c is not None:
        img.putpixel((int(x), int(y)), c)


def rect(img, x0, y0, x1, y1, c):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            put(img, x, y, c)


def disc(img, cx, cy, r, c, sq=1.0):
    for y in range(int(cy - r - 1), int(cy + r + 2)):
        for x in range(int(cx - r - 1), int(cx + r + 2)):
            if ((x - cx) ** 2 + ((y - cy) / sq) ** 2) <= r * r:
                put(img, x, y, c)


def outline(img, c=OUTLINE, diag=False):
    """Adds a 1px outline around every opaque pixel."""
    src = img.copy()
    w, h = img.size
    px = src.load()
    out = img.load()
    nb = [(1, 0), (-1, 0), (0, 1), (0, -1)]
    if diag:
        nb += [(1, 1), (-1, 1), (1, -1), (-1, -1)]
    for y in range(h):
        for x in range(w):
            if px[x, y][3] == 0:
                for dx, dy in nb:
                    nx, ny = x + dx, y + dy
                    if 0 <= nx < w and 0 <= ny < h and px[nx, ny][3] > 0:
                        out[x, y] = c
                        break
    return img


def from_template(rows, pal):
    h = len(rows)
    w = max(len(r) for r in rows)
    img = new(w, h)
    for y, r in enumerate(rows):
        for x, ch in enumerate(r):
            if ch in pal:
                put(img, x, y, pal[ch])
    return img


def mirror_rows(half):
    return [r + r[::-1] for r in half]


def strip(frames):
    w = sum(f.width for f in frames)
    h = max(f.height for f in frames)
    img = new(w, h)
    x = 0
    for f in frames:
        img.paste(f, (x, 0), f)
        x += f.width
    return img


def grid(rows_of_frames):
    rows = [strip(r) for r in rows_of_frames]
    img = new(max(r.width for r in rows), sum(r.height for r in rows))
    y = 0
    for r in rows:
        img.paste(r, (0, y), r)
        y += r.height
    return img


def shade(c, f):
    return (max(0, min(255, int(c[0] * f))), max(0, min(255, int(c[1] * f))),
            max(0, min(255, int(c[2] * f))), c[3])


# ---------------------------------------------------------------- noise
def value_noise(w, h, scale, seed):
    rng = np.random.RandomState(seed)
    gw, gh = int(w / scale) + 3, int(h / scale) + 3
    g = rng.rand(gh, gw)
    ys, xs = np.mgrid[0:h, 0:w].astype(float)
    u, v = xs / scale, ys / scale
    x0, y0 = np.floor(u).astype(int), np.floor(v).astype(int)
    fx, fy = u - x0, v - y0
    fx = fx * fx * (3 - 2 * fx)
    fy = fy * fy * (3 - 2 * fy)
    a = g[y0, x0]
    b = g[y0, x0 + 1]
    c = g[y0 + 1, x0]
    d = g[y0 + 1, x0 + 1]
    return (a * (1 - fx) + b * fx) * (1 - fy) + (c * (1 - fx) + d * fx) * fy


def fbm(w, h, scale, seed, octaves=4):
    tot = np.zeros((h, w))
    amp, norm = 1.0, 0.0
    for o in range(octaves):
        tot += amp * value_noise(w, h, scale / (2 ** o), seed + o * 17)
        norm += amp
        amp *= 0.5
    return tot / norm


# ================================================================ ISLAND MAP
W, H = 64, 48
ZONES = {
    "shared": (24, 17, 40, 32),
    "you": (9, 29, 20, 38),
    "mila": (44, 9, 55, 18),
    "tyoma": (44, 29, 55, 38),
    "ksyu": (9, 9, 20, 18),
}
# buildings: kind, x, y (top-left of footprint), fw, fh
BUILDINGS = [
    ("kitchen", 30, 17, 4, 3),
    ("well", 26, 19, 1, 1),
    ("shop", 36, 19, 2, 1),
    ("chest", 28, 21, 1, 1),
    ("crate", 35, 21, 1, 1),
    ("board", 39, 21, 1, 1),
    ("tent_you", 10, 30, 2, 2),
    ("tent_mila", 53, 10, 2, 2),
    ("tent_tyoma", 53, 30, 2, 2),
    ("tent_ksyu", 10, 10, 2, 2),
]
SIGNS = [("you", 21, 29), ("mila", 43, 18), ("tyoma", 43, 29), ("ksyu", 21, 18)]
DOCK_X = (31, 32)


def build_map():
    rng = random.Random(7)
    ys, xs = np.mgrid[0:H, 0:W].astype(float)
    cx, cy = W / 2, H / 2 + 0.5
    base = 1 - ((xs + 0.5 - cx) / 31.0) ** 2 - ((ys + 0.5 - cy) / 22.0) ** 2
    n = fbm(W, H, 6, 3) - 0.5
    base = base + 0.55 * n
    # guarantee land around zones
    for (x0, y0, x1, y1) in ZONES.values():
        dx = np.maximum(np.maximum(x0 - xs, xs - x1), 0)
        dy = np.maximum(np.maximum(y0 - ys, ys - y1), 0)
        d = np.sqrt(dx * dx + dy * dy)
        base = np.maximum(base, 0.34 - d / 3.0)
    # always keep open sea around the island
    edge = np.minimum(np.minimum(xs, W - 1 - xs), np.minimum(ys, H - 1 - ys))
    base = np.minimum(base, (edge - 3.5) / 3.0)
    # land bridge toward the dock
    for y in range(32, 41):
        for x in range(29, 35):
            base[y, x] = max(base[y, x], 0.2)
    ground = np.full((H, W), "~", dtype="<U1")
    ground[base > 0] = ":"
    ground[base > 0.16] = ","
    for (x0, y0, x1, y1) in ZONES.values():
        ground[y0:y1 + 1, x0:x1 + 1] = ","

    def hline(x0, x1, y):
        for x in range(min(x0, x1), max(x0, x1) + 1):
            for k in (0, 1):
                if ground[y + k, x] in ",:":
                    ground[y + k, x] = "="

    def vline(x, y0, y1):
        for y in range(min(y0, y1), max(y0, y1) + 1):
            for k in (0, 1):
                if ground[y, x + k] in ",:":
                    ground[y, x + k] = "="

    hline(15, 49, 22)      # main street
    vline(15, 19, 28)      # west: ksyu <-> you
    vline(48, 19, 28)      # east: mila <-> tyoma
    vline(31, 20, 46)      # kitchen door -> dock
    # dock: first water tile below the land bridge, down to the map edge
    for x in DOCK_X:
        y = 34
        while y < H and ground[y, x] != "~":
            y += 1
        for yy in range(y, H - 1):
            ground[yy, x] = "#"
    # blocked footprint
    blocked = np.zeros((H, W), dtype=bool)
    for (_, x, y, fw, fh) in BUILDINGS:
        blocked[y:y + fh, x:x + fw] = True
    for (_, x, y) in SIGNS:
        blocked[y, x] = True
    objs = []
    for (k, x, y, fw, fh) in BUILDINGS:
        objs.append(("B", k, x, y))
    for (o, x, y) in SIGNS:
        objs.append(("S", o, x, y))

    def in_zone(x, y, pad=1):
        for (x0, y0, x1, y1) in ZONES.values():
            if x0 - pad <= x <= x1 + pad and y0 - pad <= y <= y1 + pad:
                return True
        return False

    def near_path(x, y):
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                yy, xx = y + dy, x + dx
                if 0 <= yy < H and 0 <= xx < W and ground[yy, xx] in "=#":
                    return True
        return False

    trees = []
    cand = [(x, y) for y in range(H) for x in range(W)
            if ground[y, x] == "," and not in_zone(x, y) and not near_path(x, y)]
    rng.shuffle(cand)
    for (x, y) in cand:
        if all(abs(x - tx) + abs(y - ty) >= 3 for (_, tx, ty) in trees):
            kind = "tree%d" % rng.randint(0, 1) if rng.random() < 0.7 else (
                "bush" if rng.random() < 0.6 else "rock")
            trees.append((kind, x, y))
            blocked[y, x] = True
    # a few rocks on beaches
    sand = [(x, y) for y in range(H) for x in range(W) if ground[y, x] == ":" and not near_path(x, y)]
    rng.shuffle(sand)
    for (x, y) in sand[:10]:
        if not blocked[y, x]:
            trees.append(("rock", x, y))
            blocked[y, x] = True
    for (k, x, y) in trees:
        objs.append(("N", k, x, y))
    with open(os.path.join(MAPDIR, "island.txt"), "w") as f:
        f.write("# The Garden island map. ~ water, : sand, , grass, = path, # dock\n")
        f.write("SIZE %d %d\n" % (W, H))
        for row in ground:
            f.write("G " + "".join(row) + "\n")
        for name, (x0, y0, x1, y1) in ZONES.items():
            f.write("ZONE %s %d %d %d %d\n" % (name, x0, y0, x1, y1))
        for (t, k, x, y) in objs:
            f.write("OBJ %s %s %d %d\n" % (t, k, x, y))
    return ground


# ---------------------------------------------------------------- terrain
SEASONS = ["spring", "summer", "autumn", "winter"]
GRASS = {
    "spring": ["#4f9a45", "#62b04c", "#7cc457", "#9fd96a"],
    "summer": ["#3f8a3e", "#4f9d42", "#66b34a", "#86c95a"],
    "autumn": ["#7a8a3c", "#8f9a42", "#a8a64c", "#c2b25a"],
    "winter": ["#c8d6e6", "#dbe6f0", "#e9f0f7", "#f7fbff"],
}
FLOWERS = {
    "spring": ["#ffffff", "#f7a8c8", "#ffe36e", "#c9a8f7"],
    "summer": ["#ffe36e", "#ffffff", "#ff9e6e"],
    "autumn": ["#e0782e", "#c74a2a", "#f2b23a", "#8c4a2a"],
    "winter": ["#9fc4a0", "#a8b8c8"],
}
SAND = {"spring": ["#e2c58c", "#ecd59f", "#f4e4b8"],
        "summer": ["#e8c984", "#f0d99a", "#f8e9b8"],
        "autumn": ["#d8bb86", "#e4c998", "#eed8b0"],
        "winter": ["#d8dde6", "#e6ebf1", "#f2f5f9"]}
PATH = {"spring": ["#9a6a44", "#ab7a50", "#bd8c5e"],
        "summer": ["#9e6c42", "#b07e4e", "#c2905c"],
        "autumn": ["#8e6040", "#a0704a", "#b28258"],
        "winter": ["#a89a96", "#bdb0aa", "#d0c6c2"]}


def bilinear(arr, px_w, px_h):
    ys, xs = np.mgrid[0:px_h, 0:px_w].astype(float)
    u = (xs + 0.5) / T - 0.5
    v = (ys + 0.5) / T - 0.5
    u = np.clip(u, 0, arr.shape[1] - 1.001)
    v = np.clip(v, 0, arr.shape[0] - 1.001)
    x0, y0 = np.floor(u).astype(int), np.floor(v).astype(int)
    fx, fy = u - x0, v - y0
    a = arr[y0, x0]
    b = arr[y0, np.minimum(x0 + 1, arr.shape[1] - 1)]
    c = arr[np.minimum(y0 + 1, arr.shape[0] - 1), x0]
    d = arr[np.minimum(y0 + 1, arr.shape[0] - 1), np.minimum(x0 + 1, arr.shape[1] - 1)]
    return (a * (1 - fx) + b * fx) * (1 - fy) + (c * (1 - fx) + d * fx) * fy


def render_terrain(ground):
    pw, ph = W * T, H * T
    land = np.where(np.isin(ground, list(":,=")), 1.0, -1.0)
    grass = np.where(np.isin(ground, list(",=")), 1.0, -1.0)
    pathm = np.where(ground == "=", 1.0, -1.0)
    n1 = fbm(pw, ph, 10, 11) - 0.5
    n2 = fbm(pw, ph, 6, 23) - 0.5
    n3 = fbm(pw, ph, 3, 41)
    nlow = fbm(pw, ph, 22, 57)
    L = bilinear(land, pw, ph) + 0.9 * n1
    G = bilinear(grass, pw, ph) + 0.8 * n2 - 0.15
    P = bilinear(pathm, pw, ph) + 0.6 * n2 - 0.1
    rng = np.random.RandomState(5)
    dither = rng.rand(ph, pw)
    for season in SEASONS:
        img = np.zeros((ph, pw, 4), dtype=np.uint8)
        gcol = [hx(c) for c in GRASS[season]]
        scol = [hx(c) for c in SAND[season]]
        pcol = [hx(c) for c in PATH[season]]
        # grass shades from noise
        v = nlow + (dither - 0.5) * 0.10
        idx = np.digitize(v, [0.30, 0.63, 0.80])
        gimg = np.array(gcol, dtype=np.uint8)[idx]
        sidx = np.clip((n3 * 2.2 + dither * 0.7 - 0.4).astype(int), 0, 2)
        simg = np.array(scol, dtype=np.uint8)[sidx]
        pidx = np.clip((n3 * 2.0 + dither * 0.8 - 0.5).astype(int), 0, 2)
        pimg = np.array(pcol, dtype=np.uint8)[pidx]
        landm = L > 0
        grassm = landm & (G > 0)
        sandm = landm & ~grassm
        pathm_px = landm & (P > 0.05)
        img[sandm] = simg[sandm]
        img[grassm] = gimg[grassm]
        img[pathm_px] = pimg[pathm_px]
        # darker rim where sand meets water (wet sand)
        wet = landm & (L < 0.18)
        img[wet] = (img[wet].astype(int) * np.array([0.86, 0.86, 0.9, 1])).astype(np.uint8)
        # grass edge shade toward sand
        gedge = grassm & (G < 0.12)
        img[gedge] = (img[gedge].astype(int) * np.array([0.82, 0.86, 0.82, 1])).astype(np.uint8)
        pedge = pathm_px & (P < 0.17)
        img[pedge] = (img[pedge].astype(int) * np.array([0.85, 0.85, 0.85, 1])).astype(np.uint8)
        pil = Image.fromarray(img, "RGBA")
        r = random.Random(99)
        # grass tufts and flowers
        tuft_dark = shade(gcol[0], 0.85)
        tuft_light = gcol[3]
        for _ in range(int(pw * ph / 70)):
            x, y = r.randrange(pw), r.randrange(ph)
            if grassm[y, x] and not pathm_px[y, x] and (y + 3 < ph and grassm[y + 2, x]):
                c = tuft_dark if r.random() < 0.6 else tuft_light
                put(pil, x, y, c)
                put(pil, x - 1, y - 1, c)
                put(pil, x + 1, y - 1, c)
        fl = [hx(c) for c in FLOWERS[season]]
        nfl = {"spring": 2600, "summer": 1900, "autumn": 2000, "winter": 500}[season]
        for _ in range(nfl):
            x, y = r.randrange(pw), r.randrange(ph)
            if grassm[y, x] and not pathm_px[y, x]:
                c = r.choice(fl)
                if season == "autumn" and r.random() < 0.5:
                    put(pil, x, y, c)
                    put(pil, x + 1, y, shade(c, 0.85))
                else:
                    put(pil, x, y, c)
                    put(pil, x, y + 1, shade(gcol[0], 0.8))
                    if r.random() < 0.4:
                        put(pil, x + 1, y, c)
                        put(pil, x - 1, y, c)
                        put(pil, x, y - 1, c)
                        put(pil, x, y, hx("#ffe36e") if c != hx("#ffe36e") else hx("#ffffff"))
        # pebbles on paths and sand
        for _ in range(int(pw * ph / 160)):
            x, y = r.randrange(pw), r.randrange(ph)
            if pathm_px[y, x]:
                put(pil, x, y, shade(pcol[0], 0.8))
                put(pil, x + 1, y, shade(pcol[2], 1.08))
            elif sandm[y, x] and r.random() < 0.3:
                put(pil, x, y, shade(scol[0], 0.9))
        if season == "winter":
            for _ in range(int(pw * ph / 90)):
                x, y = r.randrange(pw), r.randrange(ph)
                if grassm[y, x] and not pathm_px[y, x]:
                    put(pil, x, y, hx("#ffffff"))
        save(pil, "terrain_%s.png" % season)
    # foam / shallow water overlay frames (transparent elsewhere)
    for f in range(3):
        img = np.zeros((ph, pw, 4), dtype=np.uint8)
        ys, xs = np.mgrid[0:ph, 0:pw].astype(float)
        wob = 0.10 * np.sin(xs * 0.21 + ys * 0.17 + f * 2.09) + 0.06 * np.sin(xs * 0.07 - ys * 0.11 + f * 2.09)
        water = L <= 0
        shallow = water & (L > -0.75 + wob)
        foam = water & (L > -0.20 + wob)
        foam2 = water & (L > -0.52 + wob) & (L < -0.44 + wob)
        img[shallow] = (120, 200, 222, 120)
        img[foam2] = (210, 240, 248, 150)
        img[foam] = (236, 250, 252, 230)
        save(Image.fromarray(img, "RGBA"), "foam_%d.png" % f)


def water_tiles():
    frames = []
    base = [hx("#2f73a6"), hx("#3780b5"), hx("#3f8cc0")]
    for f in range(4):
        img = new(32, 32)
        for y in range(32):
            for x in range(32):
                v = 0.5 + 0.25 * math.sin((x + y * 0.5) * 2 * math.pi / 32 * 2 + f * math.pi / 2) \
                    + 0.25 * math.sin((x * 0.5 - y) * 2 * math.pi / 32 + f * math.pi / 2)
                put(img, x, y, base[min(2, int(v * 3))])
        # wave highlights
        r = random.Random(3)
        for k in range(5):
            x0 = r.randrange(32)
            y0 = r.randrange(32)
            ph = (k + f) % 4
            ln = [2, 3, 3, 2][ph]
            for i in range(ln):
                put(img, (x0 + i) % 32, y0, hx("#7cc6e6"))
            if ph == 2:
                put(img, (x0 + 1) % 32, (y0 - 1) % 32, hx("#b8e6f6"))
        frames.append(img)
    save(strip(frames), "water.png")


# ---------------------------------------------------------------- characters
SKIN = {"s": hx("#f4cfa8"), "S": hx("#e0a982"), "k": hx("#f29a9a"), "e": hx("#2a2033"),
        "w": hx("#ffffff")}

CHAR_PALS = {
    "player": dict(h="#7a4a2c", H="#5c3420", c="#4fb3a9", C="#3a8a86", p="#3d4f7a", P="#2e3c60", b="#5c3a28"),
    "mila": dict(h="#e2783a", H="#b85a2a", c="#7cbf5a", C="#5a9a44", p="#8a5a3a", P="#6e442c", b="#4a3020"),
    "tyoma": dict(h="#3a2a28", H="#241a18", c="#f2f2f2", C="#c8c8d0", p="#c74a4a", P="#9a3434", b="#3a2a28"),
    "ksyu": dict(h="#f29ac0", H="#cc6e9c", c="#9a7ad8", C="#7a5cb8", p="#5a4a8a", P="#44386e", b="#3a2a50"),
}

FRONT_HALF = [
    "........",
    "........",
    "....hhhh",
    "...hhhhh",
    "..hhhhhh",
    "..hhhhhh",
    ".hhhhhhh",
    ".hhhHhhH",
    ".hhHssss",
    ".hhsssss",
    ".hhssess",
    ".hsssess",
    "..skssss",
    "...sssss",
    "....SSSS",
    "...ccccc",
    "..cccccc",
    "..cCcccc",
    "..cCcccc",
    "..sppppp",
    "...ppppp",
]
BACK_HALF = [
    "........",
    "........",
    "....hhhh",
    "...hhhhh",
    "..hhhhhh",
    "..hhhhhh",
    ".hhhhhhh",
    ".hhhhhhh",
    ".hhhhhhh",
    ".hhhHhhh",
    ".hhhhhhh",
    ".hhHhhhh",
    "..hhhhhh",
    "...hHHHH",
    "....SSSS",
    "...ccccc",
    "..cccccc",
    "..cCcccc",
    "..cCcccc",
    "..sppppp",
    "...ppppp",
]
SIDE = [
    "................",
    "................",
    "......hhhh......",
    "....hhhhhhhh....",
    "...hhhhhhhhhh...",
    "...hhhhhhhhhhh..",
    "..hhhhhhhhhhhh..",
    "..hhhhhhhhHhhH..",
    "..hhhhhhhHsssss.",
    "..hhhhhhhsssssss",
    "..hhhhhhssssess.",
    "...hhhhhssssess.",
    "...hhhhhskkssss.",
    "....hhhssssss...",
    "......SSSSS.....",
    ".....cccccc.....",
    ".....ccccccc....",
    ".....cccCccc....",
    ".....cccCccc....",
    ".....ppspppp....",
    ".....pppppp.....",
]


def char_frame(kind, pal, view, step, action=False):
    """view: 0 down, 1 up, 2 side(right). step: 0..3 walk phase."""
    p = {k: hx(v) for k, v in pal.items()}
    p.update(SKIN)
    if view == 0:
        rows = mirror_rows(FRONT_HALF)
    elif view == 1:
        rows = mirror_rows(BACK_HALF)
    else:
        rows = list(SIDE)
    body = from_template(rows, p)
    img = new(18, 26)
    bob = 1 if step in (1, 3) else 0
    if action:
        bob = 1
    img.paste(body, (1, 1 + bob), body)
    # legs
    legs = new(18, 26)
    top = 22 + bob
    if view in (0, 1):
        lifts = {0: (0, 0), 1: (1, 0), 2: (0, 0), 3: (0, 1)}[step]
        for li, (x0) in enumerate((5, 10)):
            lift = lifts[li]
            ln = 2 - lift
            for yy in range(ln):
                rect(legs, x0, top + yy, x0 + 2, top + yy, p["p"] if yy == 0 else p["P"])
            rect(legs, x0, top + ln - bob + bob, x0 + 2, top + ln, p["b"])
    else:
        off = {0: (0, 0), 1: (1, -1), 2: (0, 0), 3: (-1, 1)}[step]
        for li, (x0) in enumerate((6, 9)):
            xx = x0 + off[li]
            col = p["P"] if li == 0 else p["p"]
            rect(legs, xx, top, xx + 2, top + 1, col)
            rect(legs, xx, top + 2, xx + 2 + (1 if li == 1 or True else 0), top + 2, p["b"])
    legs.paste(img, (0, 0), img)
    img = legs
    if view == 0:
        # little eye highlights
        put(img, 6, 11 + bob, p["w"])
        put(img, 11, 11 + bob, p["w"])
    if kind == "tyoma":
        # chef toque: puffy top + band over the hair
        hat, hat_d = hx("#ffffff"), hx("#d8d8e4")
        x0 = 4 if view != 2 else 4
        disc(img, x0 + 2, 2 + bob, 2.2, hat)
        disc(img, x0 + 5, 1.5 + bob, 2.6, hat)
        disc(img, x0 + 8, 2 + bob, 2.2, hat)
        rect(img, x0 + 1, 3 + bob, x0 + 9, 4 + bob, hat)
        rect(img, x0 + 1, 5 + bob, x0 + 9, 5 + bob, hat_d)
    if kind == "ksyu":
        # flower hairpin
        fx = 13 if view != 2 else 5
        put(img, fx, 5 + bob, hx("#ffe36e"))
        put(img, fx - 1, 5 + bob, hx("#ffffff"))
        put(img, fx + 1, 5 + bob, hx("#ffffff"))
        put(img, fx, 4 + bob, hx("#ffffff"))
        put(img, fx, 6 + bob, hx("#ffffff"))
    if kind == "mila":
        # pigtail tufts
        sides = (2, 15) if view != 2 else (3,)
        for sx in sides:
            rect(img, sx - 1, 12 + bob, sx, 15 + bob, p["h"])
            put(img, sx, 16 + bob, p["H"])
    outline(img)
    return img


def characters():
    for kind, pal in CHAR_PALS.items():
        rows = []
        for view in (0, 1, 2):
            rows.append([char_frame(kind, pal, view, s) for s in range(4)] +
                        [char_frame(kind, pal, view, 0, action=True)])
        save(grid(rows), "char_%s.png" % kind)


# ---------------------------------------------------------------- crops
CROPS = ["wheat", "cabbage", "carrot", "potato"]
G1, G2, G3, G4 = hx("#2f6e2f"), hx("#46913a"), hx("#63b344"), hx("#8fd45a")


def plant_base(img, cx, by):
    pass


def crop_frames(kind):
    frames = []
    for stage in range(4):
        img = new(16, 24)
        cx, by = 8, 21
        if stage == 0:
            # seeds in a little mound
            for (x, y) in [(6, by), (9, by - 1), (10, by + 1)]:
                put(img, x, y, hx("#e8d0a0"))
                put(img, x + 1, y, hx("#b89868"))
            put(img, 8, by - 1, G3)
        elif stage == 1:
            put(img, cx, by, G2)
            put(img, cx, by - 1, G2)
            put(img, cx - 1, by - 2, G3)
            put(img, cx - 2, by - 3, G3)
            put(img, cx + 1, by - 2, G3)
            put(img, cx + 2, by - 3, G4)
            put(img, cx + 1, by - 3, G4)
            put(img, cx - 1, by - 3, G4)
        else:
            mature = stage == 3
            if kind == "wheat":
                stalks = [-4, -2, 0, 2, 4, -3, 3] if mature else [-3, -1, 1, 3]
                hgt = 15 if mature else 9
                for i, dx in enumerate(stalks):
                    hh = hgt - (i % 3)
                    lean = (1 if dx > 0 else -1 if dx < 0 else 0)
                    for yy in range(hh):
                        x = cx + dx + (lean if yy > hh * 0.6 else 0)
                        col = (hx("#c9a43a") if yy > 3 else hx("#8ea03a")) if mature else (G2 if yy < hh - 3 else G3)
                        put(img, x, by - yy, col)
                    if mature:
                        tx = cx + dx + lean
                        for k in range(4):
                            put(img, tx - 1 + (k % 2) * 2, by - hh + 1 + k, hx("#f2cf5a"))
                            put(img, tx, by - hh + k, hx("#e8b83c"))
                        put(img, tx, by - hh - 1, hx("#fbe48a"))
                    else:
                        put(img, cx + dx - 1, by - hh // 2, G3)
                outline(img, hx("#3a4a20") if not mature else hx("#6a4a1a"))
            elif kind == "cabbage":
                if mature:
                    disc(img, cx, by - 4, 6.5, G2, 0.75)
                    disc(img, cx, by - 5, 4.6, hx("#a8dc7a"), 0.85)
                    disc(img, cx - 1, by - 6, 2.5, hx("#c8eea0"), 0.9)
                    for k in range(-3, 4, 2):
                        put(img, cx + k, by - 3, hx("#7fbf5a"))
                    put(img, cx, by - 8, hx("#e4f8c8"))
                    put(img, cx - 6, by - 5, G1)
                    put(img, cx + 6, by - 5, G1)
                else:
                    disc(img, cx, by - 3, 4.2, G2, 0.7)
                    disc(img, cx, by - 4, 2.4, G4, 0.8)
                outline(img, hx("#24502a"))
            elif kind == "carrot":
                hgt = 12 if mature else 7
                for i, dx in enumerate([-3, -1, 1, 3] if mature else [-2, 0, 2]):
                    for yy in range(hgt - abs(dx)):
                        x = cx + dx + int(dx * yy / 10.0)
                        put(img, x, by - 1 - yy, G2 if yy % 3 else G3)
                        if yy % 3 == 1:
                            put(img, x - 1, by - 1 - yy, G4)
                            put(img, x + 1, by - 2 - yy, G3)
                if mature:
                    rect(img, cx - 2, by - 1, cx + 2, by + 1, hx("#f08a2a"))
                    rect(img, cx - 1, by - 1, cx + 1, by - 1, hx("#ffb05a"))
                    put(img, cx - 2, by + 1, hx("#c8661e"))
                outline(img, hx("#24502a"))
            elif kind == "potato":
                r = 5.5 if mature else 3.8
                disc(img, cx, by - 4, r, G1, 0.75)
                disc(img, cx - 2, by - 5, r - 2, G2, 0.8)
                disc(img, cx + 2, by - 6, r - 2.5, G3, 0.8)
                for (x, y) in [(cx - 3, by - 6), (cx + 1, by - 4), (cx + 3, by - 8)]:
                    put(img, x, y, G4)
                if mature:
                    for (x, y) in [(cx - 3, by - 9), (cx + 2, by - 10), (cx + 5, by - 6), (cx - 5, by - 5)]:
                        put(img, x, y, hx("#ffffff"))
                        put(img, x + 1, y, hx("#e8d8ff"))
                        put(img, x, y + 1, hx("#c8a8f0"))
                        put(img, x, y - 1 if False else y, hx("#ffe36e"))
                    rect(img, cx - 5, by, cx - 3, by + 1, hx("#c89a5a"))
                    put(img, cx - 4, by, hx("#e0b878"))
                outline(img, hx("#1e4024"))
            if not mature:
                pass
        frames.append(img)
    return frames


def crops():
    save(grid([crop_frames(k) for k in CROPS]), "crops.png")


def soil():
    frames = []
    for wet in (False, True):
        img = new(16, 16)
        base = hx("#4a2e1e") if wet else hx("#7a4e32")
        mid = hx("#3a2216") if wet else hx("#6a4028")
        hi = hx("#5a3a26") if wet else hx("#946040")
        rect(img, 1, 1, 14, 14, base)
        for x in (1, 14):
            put(img, x, 1, None)
        r = random.Random(4)
        for y in range(2, 14, 3):
            rect(img, 2, y, 13, y, mid)
            for x in range(2, 14):
                if r.random() < 0.25:
                    put(img, x, y - 1, hi)
        rect(img, 1, 14, 14, 14, shade(base, 0.7))
        rect(img, 1, 1, 14, 1, hi)
        img.putpixel((1, 1), (0, 0, 0, 0))
        img.putpixel((14, 1), (0, 0, 0, 0))
        img.putpixel((1, 14), (0, 0, 0, 0))
        img.putpixel((14, 14), (0, 0, 0, 0))
        if wet:
            for (x, y) in [(4, 4), (10, 7), (6, 11)]:
                put(img, x, y, hx("#6a8ab0"))
        frames.append(img)
    save(strip(frames), "soil.png")


# ---------------------------------------------------------------- items / icons
def icon_canvas():
    return new(16, 16)


def icon_hoe():
    img = icon_canvas()
    for i in range(10):
        put(img, 3 + i, 13 - i, hx("#b07a48"))
        put(img, 4 + i, 13 - i, hx("#8a5a34"))
    rect(img, 10, 2, 14, 3, hx("#b8c0cc"))
    rect(img, 13, 4, 14, 6, hx("#8a94a4"))
    put(img, 10, 2, hx("#e8eef6"))
    return outline(img)


def icon_can():
    img = icon_canvas()
    rect(img, 3, 7, 10, 13, hx("#4f9ad8"))
    rect(img, 3, 7, 10, 7, hx("#8ac8f0"))
    rect(img, 3, 12, 10, 13, hx("#3a78b0"))
    for i in range(4):
        put(img, 11 + i, 8 - i, hx("#4f9ad8"))
        put(img, 11 + i, 9 - i, hx("#3a78b0"))
    rect(img, 14, 3, 15, 4, hx("#8ac8f0"))
    for x in range(4, 10):
        put(img, x, 4 if 5 < x < 8 else 5, hx("#3a78b0"))
    put(img, 4, 6, hx("#3a78b0"))
    put(img, 9, 6, hx("#3a78b0"))
    return outline(img)


def icon_crop(kind):
    img = icon_canvas()
    if kind == "wheat":
        for dx in (-2, 0, 2):
            for y in range(4, 15):
                put(img, 8 + dx + (0 if y > 8 else dx // 2), y, hx("#c9a43a"))
            for k in range(5):
                put(img, 7 + dx + (k % 2) * 2 + dx // 2, 2 + k, hx("#f2cf5a"))
        rect(img, 5, 10, 11, 11, hx("#c74a4a"))
    elif kind == "cabbage":
        disc(img, 8, 9, 6, hx("#5aa64a"))
        disc(img, 8, 8.5, 4.2, hx("#a8dc7a"))
        disc(img, 7, 7.5, 2, hx("#d4f2b0"))
        for k in (5, 8, 11):
            put(img, k, 12, hx("#3f8a3e"))
    elif kind == "carrot":
        for i in range(9):
            w = max(0, 3 - i // 3)
            rect(img, 7 - w + i // 3, 5 + i, 8 + w - i // 4, 5 + i, hx("#f08a2a"))
            if i % 3 == 1:
                put(img, 7 - w + i // 3, 5 + i, hx("#c8661e"))
        put(img, 7, 6, hx("#ffb05a"))
        for (x, y) in [(6, 2), (7, 3), (8, 1), (8, 3), (9, 2), (10, 3), (7, 4), (9, 4)]:
            put(img, x, y, hx("#4f9d42"))
    elif kind == "potato":
        disc(img, 8, 9, 5.4, hx("#c89a5a"), 0.8)
        disc(img, 7, 8, 3.4, hx("#e0b878"), 0.8)
        for (x, y) in [(5, 9), (10, 7), (9, 11)]:
            put(img, x, y, hx("#8a6a3a"))
        put(img, 6, 6, hx("#f4d8a0"))
    return outline(img)


def icon_seeds(kind):
    img = icon_canvas()
    col = {"wheat": "#e8c96a", "cabbage": "#8fcf6a", "carrot": "#f0a050", "potato": "#c8a070"}[kind]
    c = hx(col)
    rect(img, 3, 5, 12, 14, hx("#e8dcc0"))
    rect(img, 3, 13, 12, 14, hx("#cbbd9c"))
    rect(img, 4, 3, 11, 5, hx("#d8caa8"))
    rect(img, 5, 2, 10, 2, hx("#cbbd9c"))
    disc(img, 7.5, 9.5, 2.6, c)
    put(img, 7, 9, shade(c, 1.2))
    rect(img, 3, 6, 12, 6, hx("#b8a888"))
    return outline(img)


DISHES = ["bread", "salad", "mash", "stew", "carrot_pie", "cabbage_pie"]


def icon_dish(kind):
    img = icon_canvas()
    plate = lambda: (disc(img, 8, 11, 7, hx("#e8eef6"), 0.45), disc(img, 8, 11, 5, hx("#c8d4e0"), 0.4))
    if kind == "bread":
        disc(img, 8, 9, 6.2, hx("#c8823a"), 0.65)
        disc(img, 8, 8, 4.8, hx("#e0a050"), 0.6)
        for x in (5, 8, 11):
            put(img, x, 7, hx("#f4d090"))
            put(img, x + 1, 6, hx("#f4d090"))
    elif kind == "salad":
        disc(img, 8, 10, 6.5, hx("#f2f2f2"), 0.55)
        disc(img, 8, 9, 5, hx("#7cc457"), 0.6)
        for (x, y, c) in [(6, 8, "#f08a2a"), (9, 7, "#a8dc7a"), (10, 9, "#f08a2a"), (7, 10, "#c74a4a"), (5, 9, "#a8dc7a")]:
            put(img, x, y, hx(c))
            put(img, x + 1, y, hx(c))
    elif kind == "mash":
        plate()
        disc(img, 8, 9, 4.5, hx("#f4e2b0"), 0.7)
        disc(img, 7, 8, 2.4, hx("#fff4d0"), 0.7)
        put(img, 8, 7, hx("#ffe36e"))
        put(img, 9, 7, hx("#ffd04a"))
    elif kind == "stew":
        rect(img, 3, 8, 12, 13, hx("#6e4a3a"))
        rect(img, 2, 7, 13, 8, hx("#8a5e4a"))
        rect(img, 4, 8, 11, 9, hx("#c8642a"))
        for (x, c) in [(5, "#f08a2a"), (8, "#e0b878"), (10, "#7cc457")]:
            put(img, x, 8, hx(c))
        for x in (5, 8, 11):
            put(img, x, 4, hx("#e8eef6"))
            put(img, x - 1, 5, hx("#e8eef6"))
    elif kind in ("carrot_pie", "cabbage_pie"):
        disc(img, 8, 10, 6.5, hx("#b06a2a"), 0.55)
        disc(img, 8, 9, 5.6, hx("#e0a050"), 0.5)
        fill = "#f08a2a" if kind == "carrot_pie" else "#8fcf6a"
        disc(img, 8, 9, 3.4, hx(fill), 0.5)
        for x in range(3, 14, 2):
            put(img, x, 9, hx("#f4d090"))
    return outline(img)


def icon_misc():
    out = {}
    img = icon_canvas()
    disc(img, 8, 8, 5.5, hx("#f2c23a"))
    disc(img, 8, 8, 3.8, hx("#ffe06a"))
    rect(img, 7, 6, 8, 10, hx("#d89a2a"))
    put(img, 6, 5, hx("#fff4b0"))
    out["coin"] = outline(img)
    img = icon_canvas()
    for y in range(16):
        for x in range(16):
            dx, dy = (x - 7.5) / 6, (y - 6.5) / 6
            if (dx * dx + dy * dy - 0.3) ** 3 - dx * dx * dy ** 3 * 1.4 < 0 and y < 14:
                put(img, x, y, hx("#f25a7a"))
    put(img, 5, 5, hx("#ffb0c0"))
    put(img, 5, 6, hx("#ffb0c0"))
    out["heart"] = outline(img)
    img = icon_canvas()
    disc(img, 8, 10, 4, hx("#4fb3e8"))
    for i in range(5):
        rect(img, 8 - i // 2, 3 + i, 8 + i // 2, 3 + i, hx("#4fb3e8"))
    put(img, 6, 9, hx("#c8ecff"))
    out["drop"] = outline(img)
    img = icon_canvas()
    pts = []
    for i in range(10):
        a = -math.pi / 2 + i * math.pi / 5
        r = 7 if i % 2 == 0 else 3
        pts.append((8 + r * math.cos(a), 8.5 + r * math.sin(a)))
    from PIL import ImageDraw
    ImageDraw.Draw(img).polygon(pts, fill=hx("#ffd84a"))
    put(img, 7, 6, hx("#fff4b0"))
    out["star"] = outline(img)
    img = icon_canvas()  # cozy (house heart)
    rect(img, 4, 8, 12, 13, hx("#e8c090"))
    for i in range(6):
        rect(img, 8 - i, 3 + i, 8 + i, 3 + i, hx("#c74a4a"))
    rect(img, 7, 10, 9, 13, hx("#8a5a34"))
    out["cozy"] = outline(img)
    img = icon_canvas()  # sparkle (small star 4 point)
    for i in range(-4, 5):
        put(img, 8 + i, 8, hx("#ffffff"))
        put(img, 8, 8 + i, hx("#ffffff"))
    for i in (-1, 1):
        put(img, 8 + i, 8 + i, hx("#fff4b0"))
        put(img, 8 - i, 8 + i, hx("#fff4b0"))
    out["sparkle"] = img
    return out


# ---------------------------------------------------------------- objects
def tree(variant, season):
    img = new(32, 44)
    trunk, trunk_d = hx("#8a5a34"), hx("#6a4026")
    rect(img, 13, 30, 18, 41, trunk)
    rect(img, 13, 30, 14, 41, trunk_d)
    put(img, 12, 41, trunk)
    put(img, 19, 41, trunk)
    leaves = {
        "spring": ["#3f8a3e", "#5aa84a", "#7cc457", "#a8dc7a"],
        "summer": ["#2f7a3a", "#3f8f3e", "#58a848", "#7cc457"],
        "autumn": ["#a8462a", "#d06a2a", "#e8902e", "#f4b84a"],
        "winter": ["#3a6a5a", "#4a7a66", "#5a8a74", "#6e9c86"],
    }[season]
    L = [hx(c) for c in leaves]
    if variant == 0:
        blobs = [(16, 20, 12), (9, 22, 7), (23, 22, 7), (16, 12, 9)]
    else:
        blobs = [(16, 24, 11), (16, 15, 9), (16, 8, 6)]
    for (x, y, r) in blobs:
        disc(img, x, y + 1, r, L[0], 0.85)
    for (x, y, r) in blobs:
        disc(img, x - 1, y - 1, r - 1.5, L[1], 0.85)
    for (x, y, r) in blobs:
        disc(img, x - 2, y - 2.5, r - 4.5, L[2], 0.85)
    rr = random.Random(variant * 10 + SEASONS.index(season))
    for _ in range(22):
        x, y = rr.randrange(6, 26), rr.randrange(4, 30)
        if img.getpixel((x, y))[3] and img.getpixel((x, y)) != L[0]:
            put(img, x, y, L[3])
    if season == "spring":
        for _ in range(16):
            x, y = rr.randrange(6, 26), rr.randrange(4, 30)
            if img.getpixel((x, y))[3]:
                put(img, x, y, hx("#ffc8dc"))
                put(img, x + 1, y, hx("#f7a8c8"))
    if season == "summer" and variant == 0:
        for (x, y) in [(10, 22), (20, 16), (15, 26), (23, 24)]:
            put(img, x, y, hx("#e83a3a"))
            put(img, x, y - 1, hx("#ff8a7a"))
    if season == "winter":
        for (x, y, r) in blobs:
            for xx in range(int(x - r), int(x + r)):
                for yy in range(int(y - r) - 2, int(y + r)):
                    if img.getpixel((xx, yy))[3] if 0 <= xx < 32 and 0 <= yy < 44 else 0:
                        above = img.getpixel((xx, yy - 1))[3] if yy > 0 else 0
                        if not above:
                            put(img, xx, yy, hx("#ffffff"))
                            put(img, xx, yy + 1, hx("#e8f0f8"))
                        break
    return outline(img)


def bush(season):
    img = new(16, 16)
    cols = {"spring": ["#3f8a3e", "#5aa84a", "#7cc457"], "summer": ["#2f7a3a", "#3f8f3e", "#58a848"],
            "autumn": ["#8a4a2a", "#b0602a", "#d08a3a"], "winter": ["#4a7a66", "#5a8a74", "#ffffff"]}[season]
    c = [hx(x) for x in cols]
    disc(img, 8, 10, 6.5, c[0], 0.75)
    disc(img, 7, 9, 4.5, c[1], 0.8)
    disc(img, 6, 8, 2.2, c[2], 0.8)
    if season == "summer":
        for (x, y) in [(5, 10), (10, 8), (9, 12)]:
            put(img, x, y, hx("#4a6ad8"))
    if season == "spring":
        for (x, y) in [(5, 10), (10, 8), (9, 12)]:
            put(img, x, y, hx("#ffffff"))
    return outline(img)


def rock():
    img = new(16, 16)
    disc(img, 8, 11, 6, hx("#8a8c9a"), 0.6)
    disc(img, 7, 10, 4, hx("#a8aab8"), 0.6)
    disc(img, 6, 9, 1.8, hx("#c8cad6"), 0.6)
    put(img, 11, 12, hx("#6a6c7a"))
    return outline(img)


def kitchen(night=False):
    img = new(72, 64)
    wall, wall_d = hx("#f0dcb4"), hx("#d8bc8c")
    wood, wood_d = hx("#a0683c"), hx("#7a4a2a")
    roof, roof_d, roof_l = hx("#c75a4a"), hx("#9a3e34"), hx("#e07a62")
    # chimney
    rect(img, 50, 4, 56, 18, hx("#9a8c86"))
    rect(img, 49, 3, 57, 5, hx("#7a6c66"))
    for y in range(6, 18, 3):
        rect(img, 50, y, 56, y, hx("#857872"))
    # walls
    rect(img, 8, 28, 63, 60, wall)
    rect(img, 8, 56, 63, 60, wall_d)
    for x in (8, 63):
        rect(img, x, 28, x + 1, 60, wood)
    rect(img, 8, 28, 63, 29, wood_d)
    # roof
    for i in range(22):
        y = 6 + i
        x0, x1 = 34 - (i * 34) // 21 - 2, 37 + (i * 34) // 21 + 2
        rect(img, max(0, x0), y, min(71, x1), y, roof if i % 4 else roof_d)
    for i in range(22):
        put(img, 34 - (i * 34) // 21 - 2, 6 + i, roof_d)
        put(img, 37 + (i * 34) // 21 + 2, 6 + i, roof_d)
    rect(img, 0, 27, 71, 29, roof_d)
    for i in range(4, 20, 4):
        y = 6 + i
        x0, x1 = 34 - (i * 34) // 21, 37 + (i * 34) // 21
        for x in range(x0, x1, 4):
            put(img, x, y + 1, roof_l)
    # door
    rect(img, 30, 42, 41, 60, wood)
    rect(img, 30, 42, 41, 43, wood_d)
    rect(img, 35, 42, 36, 60, wood_d)
    put(img, 38, 52, hx("#f2c23a"))
    # windows
    glass = hx("#ffd884") if night else hx("#8ac8e8")
    for wx in (14, 48):
        rect(img, wx, 36, wx + 9, 47, wood_d)
        rect(img, wx + 1, 37, wx + 8, 46, glass)
        rect(img, wx + 1, 41, wx + 8, 41, wood_d)
        rect(img, wx + 4, 37, wx + 5, 46, wood_d)
        if not night:
            put(img, wx + 2, 38, hx("#e8f6ff"))
            put(img, wx + 3, 38, hx("#e8f6ff"))
        rect(img, wx - 1, 48, wx + 10, 49, wood)
        for k in range(0, 10, 3):
            put(img, wx + k, 47, hx("#e85a7a"))
            put(img, wx + k + 1, 47, hx("#7cc457"))
    # sign: pot icon above door
    rect(img, 28, 33, 43, 39, wood)
    rect(img, 33, 35, 38, 38, hx("#4a4a5a"))
    rect(img, 32, 35, 39, 35, hx("#6a6a7a"))
    return outline(img)


def well():
    img = new(28, 32)
    stone, stone_d = hx("#9a9cab"), hx("#7a7c8a")
    rect(img, 3, 18, 24, 29, stone)
    for y in (21, 25):
        for x in range(3, 25, 5):
            rect(img, x + (y % 2) * 2, y, min(24, x + 3 + (y % 2) * 2), y, stone_d)
    rect(img, 3, 27, 24, 29, stone_d)
    rect(img, 5, 16, 22, 18, hx("#2f5a86"))
    rect(img, 5, 16, 22, 16, hx("#4f8ac0"))
    rect(img, 4, 4, 5, 18, hx("#8a5a34"))
    rect(img, 22, 4, 23, 18, hx("#8a5a34"))
    for i in range(8):
        rect(img, 2 + i, 6 - i // 2, 25 - i, 6 - i // 2, hx("#c75a4a") if i % 2 else hx("#9a3e34"))
    rect(img, 1, 6, 26, 7, hx("#9a3e34"))
    rect(img, 6, 9, 21, 9, hx("#6a4026"))
    rect(img, 12, 10, 14, 14, hx("#a0683c"))
    rect(img, 11, 13, 15, 15, hx("#8a94a4"))
    return outline(img)


def shop():
    img = new(36, 36)
    rect(img, 3, 18, 32, 32, hx("#a0683c"))
    rect(img, 3, 18, 32, 20, hx("#c98b52"))
    for x in range(5, 32, 6):
        rect(img, x, 22, x, 32, hx("#7a4a2a"))
    rect(img, 3, 6, 4, 32, hx("#7a4a2a"))
    rect(img, 31, 6, 32, 32, hx("#7a4a2a"))
    for x in range(1, 35):
        col = hx("#f2f2f2") if (x // 4) % 2 else hx("#4fb3a9")
        rect(img, x, 4, x, 10, col)
        put(img, x, 11 if (x % 4) < 2 else 10, col)
    rect(img, 1, 3, 34, 3, hx("#3a8a86"))
    # produce on counter
    for i, c in enumerate(["#f08a2a", "#8fcf6a", "#e8c96a", "#c89a5a", "#f25a7a"]):
        disc(img, 7 + i * 5, 16, 2.2, hx(c))
    return outline(img)


def chest():
    img = new(16, 16)
    rect(img, 1, 5, 14, 14, hx("#a0683c"))
    rect(img, 1, 3, 14, 7, hx("#c98b52"))
    rect(img, 1, 8, 14, 8, hx("#6a4026"))
    rect(img, 1, 14, 14, 14, hx("#7a4a2a"))
    for x in (1, 14):
        rect(img, x, 3, x, 14, hx("#8a94a4"))
    rect(img, 7, 7, 8, 10, hx("#f2c23a"))
    return outline(img)


def crate():
    img = new(16, 16)
    rect(img, 1, 4, 14, 14, hx("#c98b52"))
    for y in (4, 9, 14):
        rect(img, 1, y, 14, y, hx("#8a5a34"))
    for i in range(10):
        put(img, 2 + i, 13 - i // 2 * 1 - 0, hx("#a0683c"))
    rect(img, 4, 1, 11, 3, hx("#f2c23a"))  # coin sticking out
    rect(img, 5, 1, 6, 2, hx("#fff4b0"))
    return outline(img)


def board():
    img = new(20, 24)
    rect(img, 3, 12, 4, 23, hx("#7a4a2a"))
    rect(img, 15, 12, 16, 23, hx("#7a4a2a"))
    rect(img, 1, 2, 18, 14, hx("#a0683c"))
    rect(img, 2, 3, 17, 13, hx("#c98b52"))
    for (x, y, c) in [(3, 4, "#f2f2f2"), (9, 5, "#ffe8a0"), (13, 4, "#d8f0ff")]:
        rect(img, x, y, x + 4, y + 5, hx(c))
        put(img, x + 2, y, hx("#c74a4a"))
    return outline(img)


def tent(color):
    img = new(32, 34)
    c = hx(color)
    cd = shade(c, 0.75)
    for i in range(22):
        y = 6 + i
        rect(img, 16 - i * 14 // 21, y, 15 + i * 14 // 21, y, c)
        rect(img, 16 - i * 14 // 21, y, 16 - i * 14 // 21 + 1, y, cd)
    for i in range(10):
        rect(img, 16 - i // 2, 18 + i, 15 + i // 2, 18 + i, hx("#3a2a28"))
    rect(img, 1, 28, 30, 29, cd)
    put(img, 15, 4, hx("#7a4a2a"))
    put(img, 16, 4, hx("#7a4a2a"))
    put(img, 15, 5, hx("#7a4a2a"))
    rect(img, 17, 2, 21, 4, hx("#f2c23a"))
    return outline(img)


def sign():
    img = new(16, 20)
    rect(img, 7, 9, 8, 19, hx("#7a4a2a"))
    rect(img, 1, 3, 14, 10, hx("#c98b52"))
    rect(img, 1, 10, 14, 10, hx("#8a5a34"))
    rect(img, 3, 5, 12, 5, hx("#7a4a2a"))
    rect(img, 3, 7, 10, 7, hx("#7a4a2a"))
    return outline(img)


def dock_tile():
    img = new(16, 16)
    rect(img, 0, 0, 15, 15, hx("#b07a48"))
    for y in (0, 5, 10, 15):
        rect(img, 0, y, 15, y, hx("#7a4a2a"))
    for y in (2, 7, 12):
        put(img, 2, y, hx("#c98b52"))
        put(img, 13, y + 1, hx("#c98b52"))
    rect(img, 0, 0, 0, 15, hx("#6a4026"))
    rect(img, 15, 0, 15, 15, hx("#6a4026"))
    return img


def boat():
    img = new(32, 18)
    for i in range(6):
        rect(img, 2 + i, 8 + i, 29 - i, 8 + i, hx("#a0683c") if i < 4 else hx("#7a4a2a"))
    rect(img, 1, 7, 30, 8, hx("#c98b52"))
    rect(img, 8, 9, 23, 10, hx("#e8eef6"))
    rect(img, 15, 0, 15, 7, hx("#7a4a2a"))
    for i in range(6):
        rect(img, 16, 1 + i, 16 + 6 - i, 1 + i, hx("#f2f2f2"))
    return outline(img)


# --- decor
def decor_fence():
    img = new(16, 16)
    for x in (2, 12):
        rect(img, x, 4, x + 2, 14, hx("#c98b52"))
        put(img, x + 1, 3, hx("#c98b52"))
    rect(img, 0, 6, 15, 7, hx("#a0683c"))
    rect(img, 0, 11, 15, 12, hx("#a0683c"))
    return outline(img)


def fence_tiles():
    """16 autotile variants, mask bits: N=1, E=2, S=4, W=8."""
    frames = []
    wood, wood_d, wood_l = hx("#c98b52"), hx("#a0683c"), hx("#e0a86a")
    for m in range(16):
        img = new(16, 16)
        if m & 8:
            rect(img, 0, 6, 7, 7, wood_d)
            rect(img, 0, 11, 7, 12, wood_d)
        if m & 2:
            rect(img, 8, 6, 15, 7, wood_d)
            rect(img, 8, 11, 15, 12, wood_d)
        if m & 1:
            rect(img, 6, 0, 9, 4, wood_d)
            rect(img, 7, 0, 7, 4, wood_l)
        if m & 4:
            rect(img, 6, 12, 9, 15, wood_d)
            rect(img, 7, 12, 7, 15, wood_l)
        # the post
        rect(img, 6, 3, 9, 14, wood)
        rect(img, 6, 3, 6, 14, wood_l)
        rect(img, 9, 4, 9, 14, wood_d)
        put(img, 7, 2, wood)
        put(img, 8, 2, wood)
        outline(img)
        frames.append(img)
    save(strip(frames), "fence_tiles.png")


def decor_flowerbed():
    img = new(16, 16)
    rect(img, 1, 9, 14, 14, hx("#8a5a34"))
    rect(img, 1, 9, 14, 10, hx("#a0683c"))
    rect(img, 2, 11, 13, 12, hx("#5c3820"))
    for i, (x, c) in enumerate([(3, "#f25a7a"), (6, "#ffe36e"), (9, "#c9a8f7"), (12, "#ff9e6e")]):
        y = 4 + (i % 2) * 2
        rect(img, x, y + 2, x, 10, hx("#4f9d42"))
        put(img, x - 1, y + 1, hx(c))
        put(img, x + 1, y + 1, hx(c))
        put(img, x, y, hx(c))
        put(img, x, y + 2, hx(c))
        put(img, x, y + 1, hx("#fff4b0"))
    return outline(img)


def decor_lantern():
    img = new(16, 24)
    rect(img, 7, 10, 8, 22, hx("#4a4a5a"))
    rect(img, 4, 21, 11, 22, hx("#4a4a5a"))
    rect(img, 4, 2, 11, 3, hx("#4a4a5a"))
    rect(img, 5, 4, 10, 10, hx("#ffd884"))
    rect(img, 6, 5, 7, 8, hx("#fff4c8"))
    rect(img, 4, 10, 11, 11, hx("#4a4a5a"))
    put(img, 7, 1, hx("#4a4a5a"))
    put(img, 8, 1, hx("#4a4a5a"))
    return outline(img)


def decor_bench():
    img = new(32, 16)
    rect(img, 2, 3, 29, 6, hx("#c98b52"))
    rect(img, 2, 5, 29, 5, hx("#a0683c"))
    rect(img, 1, 8, 30, 10, hx("#c98b52"))
    rect(img, 1, 10, 30, 10, hx("#8a5a34"))
    for x in (3, 27):
        rect(img, x, 11, x + 1, 14, hx("#4a4a5a"))
        rect(img, x, 3, x + 1, 8, hx("#4a4a5a"))
    return outline(img)


def decor_scarecrow():
    img = new(16, 24)
    rect(img, 7, 6, 8, 23, hx("#8a5a34"))
    rect(img, 1, 10, 14, 11, hx("#8a5a34"))
    rect(img, 4, 9, 11, 17, hx("#c74a4a"))
    rect(img, 4, 12, 11, 12, hx("#9a3434"))
    rect(img, 1, 12, 2, 13, hx("#e8c96a"))
    rect(img, 13, 12, 14, 13, hx("#e8c96a"))
    disc(img, 7.5, 5, 3.2, hx("#e8d8b0"))
    put(img, 6, 5, hx("#2a2033"))
    put(img, 9, 5, hx("#2a2033"))
    rect(img, 3, 1, 12, 2, hx("#a0683c"))
    rect(img, 5, 0, 10, 1, hx("#a0683c"))
    return outline(img)


def decor_bowl():
    img = new(16, 16)
    disc(img, 8, 11, 5.5, hx("#4fb3e8"), 0.5)
    disc(img, 8, 10, 4.2, hx("#c8642a"), 0.45)
    rect(img, 6, 9, 7, 9, hx("#e0a050"))
    rect(img, 9, 10, 10, 10, hx("#e0a050"))
    return outline(img)


def decor_birdhouse():
    img = new(16, 24)
    rect(img, 7, 12, 8, 23, hx("#8a5a34"))
    rect(img, 3, 5, 12, 13, hx("#f2a8c0"))
    for i in range(5):
        rect(img, 7 - i - 1, 1 + i, 8 + i + 1, 1 + i, hx("#4fb3a9"))
    disc(img, 7.5, 9, 1.6, hx("#3a2a28"))
    rect(img, 7, 12, 8, 12, hx("#8a5a34"))
    return outline(img)


def decor_sunflower():
    img = new(16, 24)
    rect(img, 7, 10, 8, 23, hx("#4f9d42"))
    rect(img, 9, 15, 11, 16, hx("#63b344"))
    rect(img, 4, 18, 6, 19, hx("#63b344"))
    disc(img, 7.5, 6, 5.2, hx("#ffd84a"))
    for a in range(0, 360, 45):
        put(img, 7.5 + 5.6 * math.cos(math.radians(a)), 6 + 5.6 * math.sin(math.radians(a)), hx("#f2b23a"))
    disc(img, 7.5, 6, 2.6, hx("#7a4a2a"))
    put(img, 7, 5, hx("#a0683c"))
    return outline(img)


def decor_pumpkin():
    img = new(16, 16)
    disc(img, 8, 10, 6.2, hx("#e8782e"), 0.75)
    for x in (5, 8, 11):
        rect(img, x, 7, x, 13, hx("#c85a1e"))
    rect(img, 7, 3, 8, 6, hx("#4f9d42"))
    for (x, y) in [(5, 9), (10, 9)]:
        rect(img, x, y, x + 1, y, hx("#ffe36e"))
    rect(img, 6, 12, 10, 12, hx("#ffe36e"))
    return outline(img)


def decor_snowman():
    img = new(16, 24)
    disc(img, 8, 17, 5.5, hx("#f4f8ff"))
    disc(img, 8, 9, 4, hx("#ffffff"))
    rect(img, 4, 14, 12, 14, hx("#d8e2f0"))
    rect(img, 5, 1, 10, 5, hx("#3a3a4a"))
    rect(img, 4, 5, 11, 5, hx("#3a3a4a"))
    put(img, 6, 8, hx("#2a2033"))
    put(img, 9, 8, hx("#2a2033"))
    rect(img, 8, 10, 10, 10, hx("#f08a2a"))
    rect(img, 4, 12, 12, 12, hx("#c74a4a"))
    rect(img, 10, 13, 11, 15, hx("#c74a4a"))
    return outline(img)


def decor_mushroom_lamp():
    img = new(16, 16)
    rect(img, 6, 8, 9, 14, hx("#f4e2c0"))
    disc(img, 7.5, 7, 6, hx("#c74a4a"), 0.6)
    for (x, y) in [(4, 6), (9, 5), (11, 7)]:
        put(img, x, y, hx("#ffffff"))
    put(img, 6, 11, hx("#ffd884"))
    return outline(img)


# ---------------------------------------------------------------- pets
def pet_frames(kind):
    frames = []
    if kind == "cat":
        body, bd, belly = hx("#e8a050"), hx("#c07a34"), hx("#f8dcb0")
    else:
        body, bd, belly = hx("#e86a2a"), hx("#b84a1e"), hx("#fff4e8")
    for f in range(6):  # 0-3 walk, 4 sit, 5 sleep
        img = new(16, 16)
        if f < 4:
            yb = 9 + (1 if f in (1, 3) else 0)
            disc(img, 7, yb + 1, 4.2, body, 0.6)
            rect(img, 3, yb + 1, 10, yb + 2, belly) if kind == "fox" else None
            disc(img, 11, yb - 2, 3, body, 0.9)
            put(img, 10, yb - 5, body)
            put(img, 13, yb - 5, body)
            put(img, 9, yb - 5, body) if kind == "fox" else None
            put(img, 12, yb - 2, hx("#2a2033"))
            put(img, 14, yb - 1, hx("#2a2033") if kind == "cat" else hx("#2a2033"))
            if kind == "fox":
                rect(img, 12, yb - 1, 14, yb, belly)
            # legs
            off = [0, 1, 0, -1][f]
            for lx in (4 + off, 9 - off):
                rect(img, lx, yb + 3, lx, 14, bd)
            # tail
            if kind == "cat":
                rect(img, 1, yb - 3 + (f % 2), 1, yb, bd)
                put(img, 2, yb, bd)
            else:
                disc(img, 2, yb - 1, 2.4, body)
                put(img, 0, yb - 2, belly)
                put(img, 1, yb - 3, belly)
            if kind == "cat":
                for x in (5, 7):
                    put(img, x, yb - 1 + 1, bd)
        elif f == 4:
            disc(img, 8, 11, 4, body, 0.9)
            disc(img, 8, 6, 3.2, body)
            put(img, 6, 2, body)
            put(img, 10, 2, body)
            put(img, 7, 6, hx("#2a2033"))
            put(img, 9, 6, hx("#2a2033"))
            rect(img, 7, 9, 9, 13, belly)
            if kind == "fox":
                rect(img, 7, 7, 9, 8, belly)
            disc(img, 12, 13, 2, bd if kind == "cat" else body)
        else:
            disc(img, 8, 12, 5.5, body, 0.5)
            disc(img, 11, 11, 2.5, body)
            put(img, 10, 9, body)
            put(img, 12, 9, body)
            rect(img, 10, 11, 12, 11, hx("#2a2033"))
            disc(img, 4, 12, 2.2, bd if kind == "cat" else belly)
        frames.append(outline(img))
    return frames


def butterflies():
    frames = []
    for col in ("#ffffff", "#ffe36e", "#9ad0ff", "#f7a8c8"):
        for f in range(2):
            img = new(8, 8)
            c = hx(col)
            if f == 0:
                rect(img, 1, 2, 2, 4, c)
                rect(img, 5, 2, 6, 4, c)
                put(img, 2, 5, c)
                put(img, 5, 5, c)
            else:
                rect(img, 2, 3, 2, 5, c)
                rect(img, 5, 3, 5, 5, c)
            rect(img, 3, 2, 4, 5, hx("#3a2a28"))
            frames.append(img)
    save(strip(frames), "butterfly.png")


# ---------------------------------------------------------------- fx / ui
def soft_light():
    s = 64
    img = new(s, s)
    for y in range(s):
        for x in range(s):
            d = math.hypot(x - s / 2 + 0.5, y - s / 2 + 0.5) / (s / 2)
            a = max(0.0, 1 - d) ** 1.8
            img.putpixel((x, y), (255, 255, 255, int(255 * a)))
    save(img, "light.png")
    img = new(16, 8)
    for y in range(8):
        for x in range(16):
            d = ((x - 7.5) / 7.5) ** 2 + ((y - 3.5) / 3.5) ** 2
            if d < 1:
                img.putpixel((x, y), (20, 16, 40, int(110 * (1 - d * 0.5))))
    save(img, "shadow.png")
    img = new(128, 64)
    n = fbm(128, 64, 14, 77)
    for y in range(64):
        for x in range(128):
            d = ((x - 63.5) / 63.5) ** 2 + ((y - 31.5) / 31.5) ** 2
            v = (1 - d) * 1.4 + (n[y, x] - 0.5) * 1.2
            if v > 0.25:
                img.putpixel((x, y), (30, 30, 60, int(min(1, (v - 0.25) * 2.5) * 255)))
    save(img, "cloud.png")


def cursor():
    img = new(18, 18)
    c = hx("#ffffff")
    for (x, y) in [(0, 0), (17, 0), (0, 17), (17, 17)]:
        sx = 1 if x == 0 else -1
        sy = 1 if y == 0 else -1
        for i in range(4):
            put(img, x + sx * i, y, c)
            put(img, x, y + sy * i, c)
    save(img, "cursor.png")


def ui():
    # 9-slice panel 24x24 border 6
    img = new(24, 24)
    rect(img, 1, 1, 22, 22, hx("#f6ead2"))
    rect(img, 1, 20, 22, 22, hx("#e2cfa8"))
    rect(img, 0, 2, 0, 21, hx("#5c3a28"))
    rect(img, 23, 2, 23, 21, hx("#5c3a28"))
    rect(img, 2, 0, 21, 0, hx("#5c3a28"))
    rect(img, 2, 23, 21, 23, hx("#5c3a28"))
    for (x, y) in [(1, 1), (22, 1), (1, 22), (22, 22)]:
        put(img, x, y, hx("#5c3a28"))
    rect(img, 2, 2, 21, 2, hx("#fffaf0"))
    save(img, "ui_panel.png")
    img = new(24, 24)
    rect(img, 1, 1, 22, 22, hx("#3a2a3a", 225))
    rect(img, 0, 2, 0, 21, hx("#1e1626"))
    rect(img, 23, 2, 23, 21, hx("#1e1626"))
    rect(img, 2, 0, 21, 0, hx("#1e1626"))
    rect(img, 2, 23, 21, 23, hx("#1e1626"))
    save(img, "ui_dark.png")
    # slot 20x20 & selected
    for name, edge, fill in (("ui_slot", "#8a6a4a", "#e8d8b8"), ("ui_slot_sel", "#ffffff", "#fff2c8")):
        img = new(20, 20)
        rect(img, 1, 1, 18, 18, hx(fill))
        rect(img, 1, 1, 18, 1, shade(hx(fill), 0.88))
        rect(img, 0, 1, 0, 18, hx(edge))
        rect(img, 19, 1, 19, 18, hx(edge))
        rect(img, 1, 0, 18, 0, hx(edge))
        rect(img, 1, 19, 18, 19, hx(edge))
        if name == "ui_slot_sel":
            rect(img, 0, 1, 0, 18, hx("#f2a23a"))
            rect(img, 19, 1, 19, 18, hx("#f2a23a"))
            rect(img, 1, 0, 18, 0, hx("#f2a23a"))
            rect(img, 1, 19, 18, 19, hx("#f2a23a"))
        save(img, name + ".png")
    # speech bubble 9-slice 16x16 border 5 (+ tail separate)
    img = new(16, 16)
    rect(img, 1, 1, 14, 14, hx("#ffffff"))
    rect(img, 0, 2, 0, 13, OUTLINE)
    rect(img, 15, 2, 15, 13, OUTLINE)
    rect(img, 2, 0, 13, 0, OUTLINE)
    rect(img, 2, 15, 13, 15, OUTLINE)
    for (x, y) in [(1, 1), (14, 1), (1, 14), (14, 14)]:
        put(img, x, y, OUTLINE)
    save(img, "ui_bubble.png")
    # button
    for name, fill in (("ui_button", "#7cc457"), ("ui_button_off", "#b8aa98")):
        img = new(24, 16)
        rect(img, 1, 1, 22, 14, hx(fill))
        rect(img, 1, 12, 22, 14, shade(hx(fill), 0.8))
        rect(img, 2, 2, 21, 2, shade(hx(fill), 1.2))
        rect(img, 0, 2, 0, 13, hx("#2a3a20"))
        rect(img, 23, 2, 23, 13, hx("#2a3a20"))
        rect(img, 2, 0, 21, 0, hx("#2a3a20"))
        rect(img, 2, 15, 21, 15, hx("#2a3a20"))
        put(img, 1, 1, hx("#2a3a20"))
        put(img, 22, 1, hx("#2a3a20"))
        put(img, 1, 14, hx("#2a3a20"))
        put(img, 22, 14, hx("#2a3a20"))
        save(img, name + ".png")


def logo():
    # small pixel sprout for title & window icon
    img = new(32, 32)
    disc(img, 16, 26, 10, hx("#7a4e32"), 0.35)
    rect(img, 15, 12, 16, 25, hx("#4f9d42"))
    disc(img, 10, 13, 5, hx("#7cc457"), 0.6)
    disc(img, 22, 10, 6, hx("#63b344"), 0.6)
    put(img, 9, 12, hx("#c8eea0"))
    put(img, 21, 8, hx("#c8eea0"))
    outline(img)
    save(img, "logo.png")


def main():
    ground = build_map()
    render_terrain(ground)
    water_tiles()
    characters()
    crops()
    soil()
    icons = [icon_hoe(), icon_can()] + [icon_seeds(k) for k in CROPS] + [icon_crop(k) for k in CROPS] \
        + [icon_dish(k) for k in DISHES]
    misc = icon_misc()
    icons += [misc[k] for k in ("coin", "heart", "drop", "star", "cozy", "sparkle")]
    save(strip(icons), "icons.png")
    for s in SEASONS:
        save(strip([tree(0, s), tree(1, s)]), "trees_%s.png" % s)
        save(bush(s), "bush_%s.png" % s)
    save(rock(), "rock.png")
    save(strip([kitchen(False), kitchen(True)]), "kitchen.png")
    save(well(), "well.png")
    save(shop(), "shop.png")
    save(chest(), "chest.png")
    save(crate(), "crate.png")
    save(board(), "board.png")
    for k, c in (("you", "#4fb3a9"), ("mila", "#7cbf5a"), ("tyoma", "#e86a5a"), ("ksyu", "#b08ae0")):
        save(tent(c), "tent_%s.png" % k)
    save(sign(), "sign.png")
    save(dock_tile(), "dock.png")
    save(boat(), "boat.png")
    decor = [decor_fence(), decor_flowerbed(), decor_lantern(), decor_bench(), decor_scarecrow(),
             decor_bowl(), decor_birdhouse(), decor_sunflower(), decor_pumpkin(), decor_snowman(),
             decor_mushroom_lamp()]
    names = ["fence", "flowerbed", "lantern", "bench", "scarecrow", "bowl", "birdhouse", "sunflower",
             "pumpkin", "snowman", "mushroom"]
    for n, d in zip(names, decor):
        save(d, "decor_%s.png" % n)
    fence_tiles()
    save(strip(pet_frames("cat")), "pet_cat.png")
    save(strip(pet_frames("fox")), "pet_fox.png")
    butterflies()
    soft_light()
    cursor()
    ui()
    logo()
    print("art generated")


if __name__ == "__main__":
    main()
