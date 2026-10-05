"""Gnomes: tiny tinkerers of the snowy Frostcog Peaks.

Tiny and round: a big round head on a short soft body, short arms and
legs, small hands. A friendly face: big bright blue eyes, a round button
nose, full round cheeks, a small round chin, and little pointed ears.
"""

import numpy as np

from elf import _brow_mask, _ear, _landmarks, _masked, _press_ear, _soften
from orc import bake_cavities, even_limb, fit_hair, v

PREVIEW_SKIN = (246, 205, 182)

SHAPE = {
    "Male": dict(
        face=1.0, size=0.64,
        length={"thigh_*": 0.76, "calf_*": 0.74, "spine_0*": 0.9, "neck_01": 0.7,
                "clavicle_*": 0.92, "upperarm_*": 0.85, "lowerarm_*": 0.82},
        girth={"pelvis": 1.15, "spine_01": 1.18, "spine_02": 1.1, "spine_03": 1.0,
               "neck_01": 0.95, "upperarm_*": 0.95, "lowerarm_*": 0.95,
               "thigh_*": 1.05, "calf_*": 1.0},
        sizes={"Head": 1.82, "hand_*": 1.1, "foot_*": 1.12, "ball_*": 1.12}),
    "Female": dict(
        face=0.8, size=0.64,
        length={"thigh_*": 0.8, "calf_*": 0.78, "spine_0*": 0.92, "neck_01": 0.75,
                "clavicle_*": 0.94, "upperarm_*": 0.87, "lowerarm_*": 0.84},
        girth={"pelvis": 1.05, "spine_01": 1.02, "spine_02": 1.0, "spine_03": 0.98,
               "neck_01": 0.92, "upperarm_*": 0.95, "lowerarm_*": 0.95,
               "thigh_*": 1.02, "calf_*": 1.0},
        sizes={"Head": 1.76, "hand_*": 1.05, "foot_*": 1.08, "ball_*": 1.08}),
}


def sculpt_head(body, k):
    m = _landmarks(body)
    eye, tip, chin, H, ear = m["eye"], m["tip"], m["chin"], m["H"], m["ear"]
    head = "Head"
    _press_ear(body, ear)

    # A rounder skull and a short soft lower face.
    body.scale(H + v(0, 0.1, -0.01), (0.14, 0.11, 0.14), (1.06, 1.0, 1.04), sym=False, bones=head)
    body.scale(eye * v(0, 1, 1) + v(0, -0.03, -0.04), (0.12, 0.06, 0.1), (1.1, 1.0, 1.0), sym=False,
               bones=head)
    body.scale(chin + v(0, 0.02, -0.02), (0.08, 0.05, 0.08), (0.9, 0.8, 1.0), sym=False,
               bones=head)
    body.grab(chin, 0.03, (0, 0.006, 0.002), sym=False, bones=head)
    # Full round cheeks.
    body.inflate(eye + v(0.026, -0.034, -0.008), (0.034, 0.03, 0.03), 0.014, bones=head)
    body.smooth(eye + v(0.026, -0.034, -0.008), 0.04, 0.3, 2)
    # A round button nose.
    body.scale(tip + v(0, 0.004, -0.012), (0.026, 0.026, 0.028), (1.25, 1.1, 1.0), sym=False,
               bones=head)
    body.grab(tip, 0.03, (0, 0.004, -0.008), sym=False, bones=head)
    body.inflate(tip + v(0, -0.004, -0.008), 0.026, 0.017 * (0.6 + 0.4 * k), sym=False, bones=head)
    body.smooth(tip + v(0, -0.004, -0.008), 0.026, 0.3, 2, sym=False)
    body.grab(v(0, eye[1] - 0.006, tip[2] - 0.03), 0.02, (0, 0, -0.004), sym=False, bones=head)
    # Big eyes, brows raised high and curved.
    body.scale(eye + v(0.002, 0, -0.004), (0.024, 0.021, 0.022), (1.35, 1.32, 1.1),
               parts=["Body", "Eyes"])
    bm = _brow_mask(body, eye)
    _masked(body, "Brows", bm, eye + v(0.0, 0.02, 0.006), (0.05, 0.03, 0.05),
            delta=(0.0, 0.006, 0.0))
    body.smooth(eye + v(0.0, 0.03, 0.0), (0.05, 0.03, 0.04), 0.3, 2)

    # Little pointed ears.
    p = body.parts["Body"]["positions"]
    near = (np.abs(p[:, 1] - ear[1]) < 0.012) & (np.abs(p[:, 2] - ear[2]) < 0.012)
    skull = p[near, 0].max()
    root = v(skull - 0.016, ear[1] - 0.004, ear[2])
    for side, nm in ((1, "Extra_EarL"), (-1, "Extra_EarR")):
        _ear(body, nm, root, root + v(0.075, 0.06, -0.06), upper=0.02, lower=0.036,
             thick=0.009, cup=0.006, face=(0.95, 0.1, 0.3), side=side)


def blue_eyes(image, draw, body):
    """Big bright blue irises with a white highlight."""
    import sculpt
    x0, y0, w, h = sculpt.EYES_RECT
    a = np.asarray(image.crop((x0, y0, x0 + w, y0 + h))).astype(np.float32)
    yy, xx = np.mgrid[0:h, 0:w]
    cx, cy = 127.0 * w / 256, 128.0 * h / 256
    r = np.hypot(xx - cx, yy - cy) / (26.5 * w / 256)
    lum = a.mean(2, keepdims=True) / 255.0
    t = np.clip(r, 0, 1)[..., None]
    col = np.array([150.0, 210.0, 255.0]) * (1 - t) + np.array([40.0, 90.0, 190.0]) * t
    col = col * (0.7 + 0.6 * lum)
    pupil = np.clip(1.0 - r / 0.4, 0, 1)[..., None] ** 0.5
    col = col * (1 - 0.8 * pupil)
    inside = np.clip((1.08 - r) / 0.12, 0, 1)[..., None]
    out = a * (1 - inside) + col * inside
    from PIL import Image
    image.paste(Image.fromarray(np.clip(out, 0, 255).astype(np.uint8)), (x0, y0))


def build(body):
    male = body.sex == "Male"
    sh = SHAPE[body.sex]
    k = sh["face"]

    sculpt_head(body, k)
    body.paint(blue_eyes)
    body.proportions(size={"*": sh["size"]})
    body.proportions(length=sh["length"], girth=sh["girth"], size=sh["sizes"])

    # Soft, not muscled: relax the base body's big muscle forms.
    mid = lambda a, b: (body.joint(a) + body.joint(b)) / 2
    for c, r in [(body.at("spine_03", (0.07, 0.05, 0.06)), 0.1),
                 (body.at("spine_02", (0.0, -0.01, 0.07)), 0.12),
                 (body.at("spine_03", (0.08, 0.04, -0.08)), 0.12),
                 (body.at("upperarm_l", (0.02, 0.02, 0.0)), 0.07),
                 (mid("upperarm_l", "lowerarm_l"), 0.09),
                 (mid("lowerarm_l", "hand_l"), 0.09),
                 (mid("thigh_l", "calf_l"), 0.15)]:
        body.smooth(c, r, 0.5, 4)
    for side in ("_l", "_r"):
        even_limb(body, "upperarm" + side, "lowerarm" + side, 0.75, 0.035)
        even_limb(body, "lowerarm" + side, "hand" + side, 0.75, 0.03)
    # A little round tummy.
    sp1 = body.joint("spine_01")
    body.grab(sp1 + v(0, 0.02, 0.08), (0.11, 0.09, 0.08), (0, -0.004, 0.022 if male else 0.01),
              sym=False)
    if male:
        lo, hi = body.joint("spine_01")[1] + 0.01, body.joint("spine_03")[1] + 0.08
        _soften(body, lambda p: (p[:, 2] > 0.0) & (np.abs(p[:, 0]) < 0.1)
                & (p[:, 1] > lo) & (p[:, 1] < hi), amount=0.7, radius=7)

    fit_hair(body)
    bake_cavities(body, dark=0.18)
