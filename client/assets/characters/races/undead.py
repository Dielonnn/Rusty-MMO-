"""Undead: risen from the graves of the dying Witherwood.

Gaunt and stooped: wasted limbs with bony knees and elbows, long bony
fingers, a sunken belly under ribs that show through the skin, the head
hanging forward from bent shoulders. A skull-like face: hollow cheeks
under sharp cheekbones, deep dark sockets with glowing eyes, a jutting
jaw, a nose rotted down to a stub, and patches of rot; on the left side
the ribs and the jawbone show through holes rotted in the flesh.
"""

import numpy as np

from elf import _brow_mask, _landmarks, _masked, _soften
from orc import bake_cavities, even_limb, fit_hair, shift, smoothstep, v

PREVIEW_SKIN = (150, 160, 150)

SHAPE = {
    "Male": dict(
        face=1.0, size=0.97, posture=1.0,
        length={"neck_01": 1.1, "index*": 1.15, "middle*": 1.15, "ring*": 1.15,
                "pinky*": 1.15, "thumb*": 1.1},
        girth={"pelvis": 0.84, "spine_01": 0.76, "spine_02": 0.84, "spine_03": 0.9,
               "neck_01": 0.72, "clavicle_*": 0.95, "upperarm_*": 0.66, "lowerarm_*": 0.7,
               "thigh_*": 0.7, "calf_*": 0.72, "index*": 0.85, "middle*": 0.85,
               "ring*": 0.85, "pinky*": 0.85},
        sizes={"Head": 0.98}),
    "Female": dict(
        face=0.8, size=0.97, posture=0.8,
        length={"neck_01": 1.1, "index*": 1.12, "middle*": 1.12, "ring*": 1.12,
                "pinky*": 1.12, "thumb*": 1.08},
        girth={"pelvis": 0.88, "spine_01": 0.78, "spine_02": 0.86, "spine_03": 0.9,
               "neck_01": 0.76, "clavicle_*": 0.95, "upperarm_*": 0.72, "lowerarm_*": 0.74,
               "thigh_*": 0.76, "calf_*": 0.76, "index*": 0.88, "middle*": 0.88,
               "ring*": 0.88, "pinky*": 0.88},
        sizes={"Head": 0.98}),
}


def sculpt_head(body, k):
    m = _landmarks(body)
    eye, tip, chin, H = m["eye"], m["tip"], m["chin"], m["H"]
    head = "Head"
    # Hollow cheeks under sharp cheekbones; temples sunken.
    cheek = eye + v(0.024, -0.026, -0.018)
    body.inflate(cheek, (0.028, 0.012, 0.028), 0.006, bones=head)
    body.inflate(eye + v(0.026, -0.058, -0.02), (0.026, 0.024, 0.03), -0.01 * k, bones=head)
    body.inflate(eye + v(0.05, 0.02, -0.04), (0.02, 0.025, 0.025), -0.005 * k, bones=head)
    # Deep sockets: the eyes sink in, the brow ridge stands out over them.
    body.grab(eye, (0.026, 0.022, 0.03), (0, 0, -0.006 * k), parts=["Body", "Eyes"])
    body.grab(eye + v(0, 0.02, 0.012), (0.04, 0.014, 0.03), (0, -0.002, 0.005 * k), bones=head)
    # A thin pinched nose, a jutting bony jaw and chin.
    body.scale(tip + v(0, 0.02, -0.02), (0.03, 0.045, 0.04), (0.75, 1.0, 1.0), sym=False,
               bones=head)
    body.grab(tip, 0.016, (0, 0.002, -0.004), sym=False, bones=head)
    body.grab(chin, 0.03, (0, -0.006 * k, 0.006 * k), sym=False, bones=head)
    body.scale(chin + v(0.05, 0.02, -0.05), (0.03, 0.04, 0.04), (1.08, 1.0, 1.0), bones=head)
    bm = _brow_mask(body, eye)
    _masked(body, "Brows", bm, eye + v(0, 0.018, 0.006), (0.05, 0.03, 0.05),
            factors=(1.0, 0.6, 1.0))
    # The nose has rotted down to a flattened stub.
    body.scale(tip + v(0, 0.004, -0.014), (0.026, 0.03, 0.032), (0.85, 0.9, 0.45), sym=False,
               bones=head)
    # Deeper hollows under the cheekbones.
    body.inflate(eye + v(0.03, -0.05, -0.02), (0.02, 0.02, 0.025), -0.006 * k, bones=head)


def glowing_eyes(image, draw, body):
    """Pale glowing eyes: no whites, a cold light with a bright core."""
    import sculpt
    x0, y0, w, h = sculpt.EYES_RECT
    yy, xx = np.mgrid[0:h, 0:w]
    cx, cy = 127.0 * w / 256, 128.0 * h / 256
    r = np.hypot(xx - cx, yy - cy) / (26.5 * w / 256)
    t = np.clip(r / 2.2, 0, 1)[..., None]
    col = np.array([235.0, 255.0, 250.0]) * (1 - t) + np.array([70.0, 200.0, 190.0]) * t
    from PIL import Image
    image.paste(Image.fromarray(np.clip(col, 0, 255).astype(np.uint8)), (x0, y0))


def texel_positions(body):
    """Where on the body each texel of the skin texture lies (model space;
    NaN where no triangle uses it), so paint can follow the body's shape
    finer than its few vertices."""
    import sculpt
    d = body.parts["Body"]
    p = d["positions"]
    tris = d["indices"].reshape(-1, 3)
    uv = d["uvs"] * sculpt.ATLAS - 0.5
    out = np.full((sculpt.ATLAS, sculpt.ATLAS, 3), np.nan)
    for t in tris:
        a, b, c = uv[t]
        x0, y0 = np.floor(np.minimum(np.minimum(a, b), c)).astype(int)
        x1, y1 = np.ceil(np.maximum(np.maximum(a, b), c)).astype(int) + 1
        x0, y0 = max(x0 - 1, 0), max(y0 - 1, 0)
        x1, y1 = min(x1 + 1, sculpt.ATLAS), min(y1 + 1, sculpt.ATLAS)
        if x1 <= x0 or y1 <= y0:
            continue
        m = np.array([[b[0] - a[0], c[0] - a[0]], [b[1] - a[1], c[1] - a[1]]])
        if abs(np.linalg.det(m)) < 1e-9:
            continue
        inv = np.linalg.inv(m)
        yy, xx = np.mgrid[y0:y1, x0:x1]
        rx, ry = xx - a[0], yy - a[1]
        l1 = inv[0, 0] * rx + inv[0, 1] * ry
        l2 = inv[1, 0] * rx + inv[1, 1] * ry
        l0 = 1.0 - l1 - l2
        inside = (l0 > -0.05) & (l1 > -0.05) & (l2 > -0.05)
        q = l0[..., None] * p[t[0]] + l1[..., None] * p[t[1]] + l2[..., None] * p[t[2]]
        sub = out[y0:y1, x0:x1]
        sub[inside] = q[inside]
    return out


def decay(body):
    """Paints rot and bone on the skin, only darkening it so the game's
    skin tint still applies: dark sunken sockets, shadowed ribs, a hollow
    belly, and blotches of rot."""
    import sculpt
    q = texel_positions(body)
    m = _landmarks(body)
    eye = m["eye"]
    sp3, sp2, sp1 = body.joint("spine_03"), body.joint("spine_02"), body.joint("spine_01")
    rng = np.random.default_rng(7 if body.sex == "Male" else 11)
    x, y, z = q[..., 0], q[..., 1], q[..., 2]
    blob = lambda c, r: smoothstep(1 - np.linalg.norm((q - c) / r, axis=-1))

    dark = np.zeros(q.shape[:2])
    for side in (1, -1):
        c = eye * v(side, 1, 1) + v(0, -0.002, -0.006)
        dark = np.maximum(dark, blob(c, v(0.046, 0.036, 0.06)) * 1.0)
        dark = np.maximum(dark, blob(c + v(0.008 * side, -0.034, -0.006), v(0.032, 0.024, 0.045)) * 0.8)
    # The rotted nose.
    dark = np.maximum(dark, blob(m["tip"], v(0.014, 0.012, 0.016)) * 0.7)
    # Ribs: curved bands round the chest and flanks.
    ax = np.abs(x)
    y0, y1 = sp2[1] - 0.04, sp3[1] + 0.06
    band = np.cos((y - y0 + ax * ax * 3.0) / 0.036 * 2 * np.pi) * 0.5 + 0.5
    inside = (smoothstep((y - y0) / 0.03) * smoothstep((y1 - y) / 0.03)
              * smoothstep((ax - 0.03) / 0.03) * smoothstep((0.2 - ax) / 0.03)
              * (z > -0.06))
    dark = np.maximum(dark, band ** 2 * inside * 0.55)
    # Bared bone: on the left flank the flesh has rotted off the ribs, and
    # the jawbone shows through the left cheek. Painted lighter (keeping
    # the skin's hue, so the game still reads it as skin, and its grey
    # tint turns it bone-pale), ringed with dark rot.
    light = np.zeros(q.shape[:2])
    flank = sp2 + v(0.12, 0.05, 0.05)
    open_ = blob(flank, v(0.07, 0.075, 0.09)) * (x > 0.04)
    rib = np.clip((band - 0.45) / 0.3, 0, 1)
    light = np.maximum(light, rib * smoothstep(open_ * 2.0))
    dark = np.maximum(dark, (1 - rib) * smoothstep(open_ * 2.0) * 0.95)
    dark = np.maximum(dark, blob(flank, v(0.09, 0.095, 0.11)) * (x > 0.03) * (1 - open_) * 0.9)
    tip, chin = m["tip"], m["chin"]
    jaw = v(0.034, (tip[1] + chin[1]) / 2, chin[2] - 0.03)
    bone = blob(jaw, v(0.018, 0.013, 0.02))
    light = np.maximum(light, smoothstep(bone * 2.5))
    dark = np.maximum(dark, blob(jaw, v(0.028, 0.022, 0.03)) * (1 - smoothstep(bone * 2.5)) * 0.9)
    light = np.nan_to_num(light)
    # A hollow belly under the ribs.
    dark = np.maximum(dark, blob(sp1 + v(0, 0.05, 0.1), v(0.1, 0.06, 0.1)) * 0.45)
    # Blotches of rot.
    pts = body.parts["Body"]["positions"]
    for _ in range(14):
        c = pts[rng.integers(len(pts))]
        n = rng.uniform(0.04, 0.08)
        dark = np.maximum(dark, blob(c, v(n, n * 1.4, n)) * rng.uniform(0.35, 0.6))
    dark = np.nan_to_num(dark)

    from PIL import Image, ImageFilter
    f = Image.fromarray((np.clip(dark, 0, 1) * 255).astype(np.uint8)).filter(
        ImageFilter.GaussianBlur(2))
    f = np.asarray(f, np.float32)[..., None] / 255.0
    a = np.asarray(body.texture).astype(np.float32)
    x0, y0_, w, h = sculpt.BODY_RECT
    region = np.zeros_like(f)
    region[y0_:y0_ + h, x0:x0 + w] = 1.0
    # Skin only: the grey underwear is left alone.
    sat = (a.max(2) - a.min(2)) / np.maximum(a.max(2), 1.0)
    region = region * smoothstep((sat[..., None] - 0.12) / 0.08)
    lit = Image.fromarray((np.clip(light, 0, 1) * 255).astype(np.uint8)).filter(
        ImageFilter.GaussianBlur(1.5))
    lit = np.asarray(lit, np.float32)[..., None] / 255.0
    a = a * (1 - f * region * 0.9) * (1 + lit * region * 0.55)
    body.texture.paste(Image.fromarray(np.clip(a, 0, 255).astype(np.uint8)))


def build(body):
    sh = SHAPE[body.sex]
    k = sh["face"]

    sculpt_head(body, k)
    body.paint(glowing_eyes)
    body.proportions(size={"*": sh["size"]})
    body.proportions(length=sh["length"], girth=sh["girth"], size=sh["sizes"])

    # Wasted: the big muscles melt away.
    mid = lambda a, b: (body.joint(a) + body.joint(b)) / 2
    for c, r in [(body.at("spine_03", (0.09, 0.07, 0.075)), 0.14),
                 (body.at("spine_02", (0.0, -0.01, 0.095)), 0.16),
                 (body.at("spine_03", (0.1, 0.05, -0.105)), 0.16),
                 (body.at("upperarm_l", (0.02, 0.02, 0.0)), 0.09),
                 (mid("upperarm_l", "lowerarm_l"), 0.12),
                 (mid("lowerarm_l", "hand_l"), 0.12),
                 (mid("thigh_l", "calf_l"), 0.2)]:
        body.smooth(c, r, 0.6, 5)
    for side in ("_l", "_r"):
        even_limb(body, "upperarm" + side, "lowerarm" + side, 0.8, 0.04)
        even_limb(body, "lowerarm" + side, "hand" + side, 0.8, 0.035)
        even_limb(body, "thigh" + side, "calf" + side, 0.7, 0.05)
        even_limb(body, "calf" + side, "foot" + side, 0.7, 0.05)
    # Bony knees and elbows stand out on the thin limbs.
    body.inflate(body.joint("calf_l") + v(0, 0.0, 0.02), 0.04, 0.012)
    body.inflate(body.joint("lowerarm_l") + v(0, 0, -0.02), 0.03, 0.008)
    # A sunken belly under the ribcage.
    sp1 = body.joint("spine_01")
    body.grab(sp1 + v(0, 0.04, 0.1), (0.13, 0.08, 0.08), (0, 0, -0.022), sym=False)
    body.smooth(sp1 + v(0, 0.06, 0.1), (0.14, 0.1, 0.07), 0.3, 2, sym=False)

    # Stooped: the head hangs forward off bent shoulders.
    s = sh["posture"]
    lo = body.joint("neck_01")[1] - 0.08
    hi = body.joint("Head")[1] - 0.015

    def neck_field(name, p):
        up = smoothstep((p[:, 1] - lo) / (hi - lo))
        side = 1.0 - smoothstep((np.abs(p[:, 0]) - 0.08) / 0.09)
        return np.maximum(up * side, body.weight_of(name, "Head"))
    shift(body, ["neck_01"], (0, -0.035 * s, 0.085 * s), neck_field)
    sp3 = body.joint("spine_03")
    body.grab(sp3 + v(0, 0.12, -0.13), (0.24, 0.17, 0.12), (0, 0.015 * s, -0.06 * s), sym=False)
    body.smooth(sp3 + v(0, 0.12, -0.14), (0.2, 0.15, 0.09), 0.3, 3, sym=False)
    body.smooth(body.joint("neck_01") + v(0, 0.0, 0.05), (0.09, 0.05, 0.05), 0.4, 4, sym=False)

    if body.sex == "Male":
        lo, hi = body.joint("spine_01")[1] + 0.02, body.joint("spine_03")[1] + 0.12
        _soften(body, lambda p: (p[:, 2] > 0.0) & (np.abs(p[:, 0]) < 0.14)
                & (p[:, 1] > lo) & (p[:, 1] < hi), amount=0.7, radius=7)
    fit_hair(body)
    bake_cavities(body, dark=0.4)
    decay(body)
