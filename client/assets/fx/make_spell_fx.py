"""Paints spell_fx.png, the atlas every spell effect is drawn from.

Run with plain Python 3 (no packages needed) from this folder:

    python3 make_spell_fx.py

Every pixel is computed here, so the picture is our own work (CC0, like the
rest of this folder). It's white everywhere; the alpha channel holds how
brightly each pixel glows, and the game tints it with the spell's school
color. The atlas is 4 by 2 cells of 128 pixels, in this order (keep it in
step with `Cell` in client/src/vfx.rs):

    0 glow   1 flare  2 flame  3 wisp
    4 ring   5 runes  6 band   7 shard
"""

import math
import struct
import zlib

CELL = 128
COLS, ROWS = 4, 2


def hash2(x, y, seed):
    h = (x * 374761393 + y * 668265263 + seed * 2246822519) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
    return ((h ^ (h >> 16)) & 0xFFFF) / 65535.0


def noise(x, y, seed):
    """Smooth value noise, 0..1."""
    xi, yi = math.floor(x), math.floor(y)
    fx, fy = x - xi, y - yi
    fx, fy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
    a = hash2(xi, yi, seed)
    b = hash2(xi + 1, yi, seed)
    c = hash2(xi, yi + 1, seed)
    d = hash2(xi + 1, yi + 1, seed)
    return (a + (b - a) * fx) + ((c + (d - c) * fx) - (a + (b - a) * fx)) * fy


def fbm(x, y, seed, octaves=4):
    total, amp, norm = 0.0, 0.5, 0.0
    for o in range(octaves):
        total += noise(x, y, seed + o * 17) * amp
        norm += amp
        x, y, amp = x * 2.03, y * 2.03, amp * 0.5
    return total / norm


def clamp(v, lo=0.0, hi=1.0):
    return lo if v < lo else hi if v > hi else v


def smooth_edge(r):
    """1 in the middle, easing to 0 at the cell's rim, so nothing bleeds."""
    return clamp((1.0 - r) / 0.12)


def seg_dist(px, py, ax, ay, bx, by):
    dx, dy = bx - ax, by - ay
    t = clamp(((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy or 1e-9))
    return math.hypot(px - ax - dx * t, py - ay - dy * t)


def line_glow(d, width):
    return math.exp(-((d / width) ** 2)) + 0.35 * math.exp(-((d / (width * 3.5)) ** 2))


# Each painter takes x right and y up, both -1..1, and returns 0..1.


def glow(x, y):
    r = math.hypot(x, y)
    soft = (math.exp(-r * r * 4.0) - math.exp(-4.0)) / (1 - math.exp(-4.0))
    return clamp(soft) * smooth_edge(r)


def flare(x, y):
    r = math.hypot(x, y)
    core = math.exp(-r * r * 40.0)
    straight = math.exp(-abs(y) * 45.0) * (1 - abs(x)) ** 2 + math.exp(-abs(x) * 45.0) * (
        1 - abs(y)
    ) ** 2
    u, v = (x + y) * 0.7071, (x - y) * 0.7071
    diagonal = 0.45 * (
        math.exp(-abs(v) * 70.0) * clamp(1 - abs(u) * 1.6) ** 2
        + math.exp(-abs(u) * 70.0) * clamp(1 - abs(v) * 1.6) ** 2
    )
    halo = 0.35 * glow(x * 1.3, y * 1.3)
    return clamp(core + straight + diagonal + halo) * smooth_edge(r)


def flame(x, y):
    t = (y + 0.92) / 1.8  # 0 at the bottom, 1 at the tip, inside the cell
    if t <= 0.0 or t >= 1.0:
        return 0.0
    if t < 0.28:
        hw = 0.62 * math.sqrt(t / 0.28)
    else:
        hw = 0.62 * ((1 - t) / 0.72) ** 0.85
    if hw <= 1e-4:
        return 0.0
    sway = (fbm(x * 2.0, y * 2.5 - 3.0, 11) - 0.5) * 0.5 * t
    d = abs(x + sway) / hw
    tongues = 0.65 + 0.7 * fbm(x * 4.0, y * 3.0, 23)
    a = clamp(1 - d) ** 1.2 * clamp(tongues - t * 0.45)
    hot = math.exp(-((x / 0.18) ** 2) - (((t - 0.3) / 0.22) ** 2))
    return clamp(a * 1.25 + hot * 0.5) * smooth_edge(math.hypot(x, y))


def wisp(x, y):
    r = math.hypot(x, y)
    n = fbm(x * 2.2 + 5.0, y * 2.2, 31, 5)
    body = clamp(1 - r) ** 1.3
    return clamp((n - 0.28) * 3.2 * body + body * 0.35) * smooth_edge(r)


def ring(x, y):
    r = math.hypot(x, y)
    ang = math.atan2(y, x)
    streaks = 0.7 + 0.3 * noise(math.cos(ang) * 5 + 9, math.sin(ang) * 5, 41)
    band = math.exp(-(((r - 0.8) / 0.075) ** 2)) * streaks
    inner = 0.18 * clamp(r / 0.8) ** 4 * (1.0 if r < 0.8 else 0.0)
    return clamp(band + inner) * smooth_edge(r)


def rune_segments():
    """The rune circle's strokes: two rims, a six-pointed star, and a
    glyph in each of twelve slots between the rims."""
    segs = []
    for rim in (0.93, 0.7):
        n = 64
        for i in range(n):
            a0, a1 = math.tau * i / n, math.tau * (i + 1) / n
            segs.append((rim * math.cos(a0), rim * math.sin(a0), rim * math.cos(a1), rim * math.sin(a1)))
    for start in (0.0, math.pi / 3):
        pts = [
            (0.68 * math.cos(start + k * math.tau / 3 + math.pi / 2), 0.68 * math.sin(start + k * math.tau / 3 + math.pi / 2))
            for k in range(3)
        ]
        for k in range(3):
            segs.append((*pts[k], *pts[(k + 1) % 3]))
    for slot in range(12):
        a = math.tau * slot / 12
        c, s = math.cos(a), math.sin(a)
        # Strokes in the slot's own frame: u along the circle, v outward.
        strokes = []
        for k in range(3):
            pick = int(hash2(slot, k, 7) * 6)
            strokes.append(
                [
                    ((-0.05, -0.05), (-0.05, 0.05)),
                    ((0.05, -0.05), (0.05, 0.05)),
                    ((-0.05, 0.05), (0.05, 0.05)),
                    ((-0.05, -0.05), (0.05, 0.05)),
                    ((0.0, -0.06), (0.0, 0.06)),
                    ((-0.05, 0.0), (0.05, 0.0)),
                ][pick]
            )
        for (u0, v0), (u1, v1) in strokes:
            def place(u, v):
                rr = 0.815 + v
                return (rr * c - u * s, rr * s + u * c)

            segs.append((*place(u0, v0), *place(u1, v1)))
    return segs


RUNES = rune_segments()


def runes(x, y):
    r = math.hypot(x, y)
    if r > 1.0:
        return 0.0
    d = min(seg_dist(x, y, *s) for s in RUNES)
    return clamp(line_glow(d, 0.014)) * smooth_edge(r)


def band(x, y):
    """A streak running left to right that tiles along its length, so
    beams can be built from many pieces without seams."""
    u = (x + 1) * 0.5
    v = abs(y)
    flicker = 0.6 + 0.4 * (
        0.5 * math.sin(math.tau * (3 * u) + y * 7)
        + 0.3 * math.sin(math.tau * (7 * u) - y * 13 + 1.3)
        + 0.2 * math.sin(math.tau * (11 * u) + 2.1)
    )
    core = math.exp(-((v / 0.1) ** 2))
    sheath = 0.55 * math.exp(-((v / 0.38) ** 2)) * flicker
    return clamp(core + sheath) * clamp((1.0 - v) / 0.12)


def shard(x, y):
    r = math.hypot(x, y)
    d = 9.0
    for k in range(6):
        a = math.tau * k / 6 + math.pi / 2
        c, s = math.cos(a), math.sin(a)
        d = min(d, seg_dist(x, y, 0, 0, 0.85 * c, 0.85 * s))
        for at, length in ((0.42, 0.22), (0.64, 0.15)):
            bx, by = at * c, at * s
            for turn in (-1, 1):
                b = a + turn * math.pi / 3
                d = min(d, seg_dist(x, y, bx, by, bx + length * math.cos(b), by + length * math.sin(b)))
    core = math.exp(-r * r * 30.0)
    return clamp(line_glow(d, 0.022) + core) * smooth_edge(r)


PAINTERS = [glow, flare, flame, wisp, ring, runes, band, shard]


def main():
    width, height = CELL * COLS, CELL * ROWS
    alpha = bytearray(width * height)
    for index, paint in enumerate(PAINTERS):
        ox, oy = (index % COLS) * CELL, (index // COLS) * CELL
        for py in range(CELL):
            y = 1 - (py + 0.5) / CELL * 2
            for px in range(CELL):
                x = (px + 0.5) / CELL * 2 - 1
                alpha[(oy + py) * width + ox + px] = round(clamp(paint(x, y)) * 255)
    rows = bytearray()
    for py in range(height):
        rows.append(0)
        for px in range(width):
            rows += bytes((255, 255, 255, alpha[py * width + px]))

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(rows), 9))
    png += chunk(b"IEND", b"")
    with open("spell_fx.png", "wb") as f:
        f.write(png)


if __name__ == "__main__":
    main()
