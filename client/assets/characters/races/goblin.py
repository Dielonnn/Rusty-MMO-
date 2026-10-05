"""Goblins: clever, greedy little folk of the glowing Grubdeep Caverns.

Small and wiry, hunched forward with a round potbelly: skinny arms and
legs ending in big knobbly hands and long feet. A big head with huge ears
sticking out sideways, a long hooked nose, a wide sly mouth over a small
pointed chin, and big yellow eyes.
"""

import numpy as np

from elf import _brow_mask, _ear, _landmarks, _masked, _press_ear
from orc import bake_cavities, even_limb, fit_hair, shift, smoothstep, v

PREVIEW_SKIN = (128, 178, 76)

SHAPE = {
    "Male": dict(
        face=1.0, size=0.72, posture=1.0,
        length={"thigh_*": 0.96, "calf_*": 0.98, "spine_0*": 0.92, "neck_01": 0.9,
                "clavicle_*": 0.85, "upperarm_*": 1.04, "lowerarm_*": 1.08, "foot_*": 1.3,
                "index*": 1.2, "middle*": 1.2, "ring*": 1.2, "pinky*": 1.2},
        girth={"pelvis": 0.95, "spine_01": 1.0, "spine_02": 0.92, "spine_03": 0.86,
               "neck_01": 0.78, "clavicle_*": 0.9, "upperarm_*": 0.72, "lowerarm_*": 0.74,
               "thigh_*": 0.76, "calf_*": 0.74},
        sizes={"Head": 1.55, "hand_*": 1.32, "foot_*": 1.1, "ball_*": 1.2}),
    "Female": dict(
        face=0.7, size=0.72, posture=0.6,
        length={"thigh_*": 0.97, "calf_*": 0.98, "spine_0*": 0.94, "neck_01": 0.95,
                "clavicle_*": 0.88, "upperarm_*": 1.03, "lowerarm_*": 1.06, "foot_*": 1.25,
                "index*": 1.15, "middle*": 1.15, "ring*": 1.15, "pinky*": 1.15},
        girth={"pelvis": 0.98, "spine_01": 0.92, "spine_02": 0.9, "spine_03": 0.9,
               "neck_01": 0.8, "clavicle_*": 0.92, "upperarm_*": 0.76, "lowerarm_*": 0.78,
               "thigh_*": 0.82, "calf_*": 0.8},
        sizes={"Head": 1.5, "hand_*": 1.22, "foot_*": 1.08, "ball_*": 1.15}),
}


def sculpt_head(body, k):
    m = _landmarks(body)
    eye, tip, chin, H, ear = m["eye"], m["tip"], m["chin"], m["H"], m["ear"]
    head = "Head"
    _press_ear(body, ear)

    # A small pointed chin under a wide mouth; narrow jaw.
    body.scale(chin + v(0, 0.015, -0.02), (0.08, 0.042, 0.08), (0.72, 1.0, 1.0), sym=False,
               bones=head)
    body.grab(chin, 0.026, (0, -0.006 * k, 0.008 * k), sym=False, bones=head)
    mouth = v(0, (tip[1] + chin[1]) / 2 + 0.004, chin[2] + 0.004)
    body.scale(mouth, (0.045, 0.014, 0.04), (1.0 + 0.45 * k, 1.0, 1.0), sym=False, bones=head)
    # The corners of the mouth pulled up in a sly grin.
    body.grab(mouth + v(0.026, 0.0, -0.01), 0.012, (0.003, 0.004 * k, -0.002), bones=head)

    # A long hooked nose: the tip drawn out and down, a hump on the bridge.
    body.scale(tip + v(0, 0.02, -0.02), (0.03, 0.045, 0.04), (0.78, 1.0, 1.0), sym=False,
               bones=head)
    body.grab(tip, (0.022, 0.024, 0.032), (0, -0.014 * k, 0.04 * k), sym=False, bones=head)
    body.grab(v(0, eye[1] - 0.018, tip[2] - 0.016), 0.016, (0, 0, 0.007 * k), sym=False,
              bones=head)

    # Big eyes under a low brow, cheekbones high and sharp.
    body.scale(eye + v(0.002, 0, -0.004), (0.024, 0.02, 0.022), (1.25, 1.2, 1.08),
               parts=["Body", "Eyes"])
    body.inflate(eye + v(0.024, -0.025, -0.018), (0.03, 0.013, 0.03), 0.005, bones=head)
    body.inflate(eye + v(0.017, -0.056, -0.012), (0.02, 0.018, 0.025), -0.004 * k, bones=head)
    bm = _brow_mask(body, eye)
    _masked(body, "Brows", bm, eye + v(0.028, 0.018, -0.01), 0.022,
            delta=(0.002, 0.007, -0.001))
    # A flatter, wider cranium.
    body.scale(H + v(0, 0.1, -0.01), (0.14, 0.1, 0.14), (1.06, 0.94, 1.0), sym=False, bones=head)

    # Huge ears sticking out sideways and a little up.
    p = body.parts["Body"]["positions"]
    near = (np.abs(p[:, 1] - ear[1]) < 0.012) & (np.abs(p[:, 2] - ear[2]) < 0.012)
    skull = p[near, 0].max()
    root = v(skull - 0.016, ear[1] + 0.002, ear[2] + 0.004)
    reach = v(0.15, 0.045, -0.05) * (1.0 if k >= 1.0 else 0.88)
    for side, nm in ((1, "Extra_EarL"), (-1, "Extra_EarR")):
        _ear(body, nm, root, root + reach, upper=0.026, lower=0.048,
             thick=0.01, cup=0.009, face=(0.4, 0.15, 0.9), side=side)


def yellow_eyes(image, draw, body):
    """Big gold irises with a slit of dark pupil."""
    import sculpt
    x0, y0, w, h = sculpt.EYES_RECT
    a = np.asarray(image.crop((x0, y0, x0 + w, y0 + h))).astype(np.float32)
    yy, xx = np.mgrid[0:h, 0:w]
    cx, cy = 127.0 * w / 256, 128.0 * h / 256
    r = np.hypot(xx - cx, yy - cy) / (26.5 * w / 256)
    lum = a.mean(2, keepdims=True) / 255.0
    t = np.clip(r, 0, 1)[..., None]
    col = np.array([250.0, 220.0, 70.0]) * (1 - t) + np.array([200.0, 120.0, 20.0]) * t
    col = col * (0.75 + 0.5 * lum)
    slit = np.clip(1.0 - np.hypot((xx - cx) / 3.0, (yy - cy) / 13.0), 0, 1)[..., None]
    col = col * (1 - 0.85 * slit)
    inside = np.clip((1.08 - r) / 0.12, 0, 1)[..., None]
    out = a * (1 - inside) * np.array([1.0, 0.97, 0.85]) + col * inside
    from PIL import Image
    image.paste(Image.fromarray(np.clip(out, 0, 255).astype(np.uint8)), (x0, y0))


def build(body):
    male = body.sex == "Male"
    sh = SHAPE[body.sex]
    k = sh["face"]

    sculpt_head(body, k)
    body.paint(yellow_eyes)
    body.proportions(size={"*": sh["size"]})
    body.proportions(length=sh["length"], girth=sh["girth"], size=sh["sizes"])

    # Wiry limbs: relax the base body's big muscles.
    mid = lambda a, b: (body.joint(a) + body.joint(b)) / 2
    for c, r in [(body.at("spine_03", (0.07, 0.05, 0.06)), 0.1),
                 (body.at("spine_03", (0.08, 0.04, -0.08)), 0.12),
                 (mid("upperarm_l", "lowerarm_l"), 0.09),
                 (mid("lowerarm_l", "hand_l"), 0.09),
                 (mid("thigh_l", "calf_l"), 0.15)]:
        body.smooth(c, r, 0.5, 4)
    for side in ("_l", "_r"):
        even_limb(body, "upperarm" + side, "lowerarm" + side, 0.75, 0.035)
        even_limb(body, "lowerarm" + side, "hand" + side, 0.75, 0.03)
    # Knobbly knees and elbows.
    for j, r, a in (("calf_l", 0.035, 0.008), ("lowerarm_l", 0.025, 0.005)):
        body.inflate(body.joint(j), r, a)

    # A round potbelly.
    sp1 = body.joint("spine_01")
    body.grab(sp1 + v(0, 0.02, 0.08), (0.12, 0.1, 0.08), (0, -0.006, 0.055 if male else 0.026),
              sym=False)
    body.smooth(sp1 + v(0, 0.02, 0.09), (0.13, 0.11, 0.07), 0.3, 3, sym=False)

    # Hunched: head and neck carried low and forward, the upper back round.
    s = sh["posture"]
    lo = body.joint("neck_01")[1] - 0.06
    hi = body.joint("Head")[1] - 0.01

    def neck_field(name, p):
        up = smoothstep((p[:, 1] - lo) / (hi - lo))
        side = 1.0 - smoothstep((np.abs(p[:, 0]) - 0.07) / 0.08)
        return np.maximum(up * side, body.weight_of(name, "Head"))
    shift(body, ["neck_01"], (0, -0.03 * s, 0.08 * s), neck_field)
    sp3 = body.joint("spine_03")
    body.grab(sp3 + v(0, 0.08, -0.1), (0.18, 0.13, 0.1), (0, 0.025 * s, -0.05 * s), sym=False)
    body.smooth(sp3 + v(0, 0.08, -0.1), (0.16, 0.12, 0.08), 0.3, 3, sym=False)
    body.smooth(body.joint("neck_01") + v(0, 0.0, 0.04), (0.08, 0.05, 0.05), 0.4, 4, sym=False)

    fit_hair(body)
    bake_cavities(body)
