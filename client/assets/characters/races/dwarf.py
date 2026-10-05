"""Dwarves: stout and stubborn mountain folk of the Frostcog Peaks.

Short and very broad: a barrel chest and a deep belly on short thick legs,
huge shoulders, thick arms with big hands, big feet. A large head on a
neck that barely shows: a heavy brow over deep-set eyes, a big bulbous
nose, broad cheeks and a wide jaw. The men's beard is long and full.
"""

import numpy as np

import sculpt
from elf import _brow_mask, _landmarks, _masked
from orc import bake_cavities, even_limb, fit_hair, smoothstep, v

PREVIEW_SKIN = (232, 178, 150)

SHAPE = {
    "Male": dict(
        face=1.0, size=0.8,
        length={"thigh_*": 0.76, "calf_*": 0.74, "spine_0*": 0.96, "neck_01": 0.55,
                "clavicle_*": 1.35, "upperarm_*": 0.9, "lowerarm_*": 0.86, "foot_*": 1.05},
        girth={"pelvis": 1.25, "spine_01": 1.38, "spine_02": 1.48, "spine_03": 1.45,
               "neck_01": 1.35, "clavicle_*": 1.15, "upperarm_*": 1.38, "lowerarm_*": 1.45,
               "thigh_*": 1.38, "calf_*": 1.42},
        sizes={"Head": 1.22, "hand_*": 1.3, "index*": 1.3, "middle*": 1.3, "ring*": 1.3,
               "pinky*": 1.3, "thumb*": 1.3, "foot_*": 1.25, "ball_*": 1.25}),
    "Female": dict(
        face=0.55, size=0.8,
        length={"thigh_*": 0.79, "calf_*": 0.77, "spine_0*": 0.97, "neck_01": 0.65,
                "clavicle_*": 1.25, "upperarm_*": 0.92, "lowerarm_*": 0.88, "foot_*": 1.04},
        girth={"pelvis": 1.22, "spine_01": 1.22, "spine_02": 1.28, "spine_03": 1.32,
               "neck_01": 1.25, "clavicle_*": 1.1, "upperarm_*": 1.28, "lowerarm_*": 1.3,
               "thigh_*": 1.3, "calf_*": 1.3},
        sizes={"Head": 1.17, "hand_*": 1.18, "index*": 1.18, "middle*": 1.18, "ring*": 1.18,
               "pinky*": 1.18, "thumb*": 1.18, "foot_*": 1.15, "ball_*": 1.15}),
}


def sculpt_head(body, k):
    m = _landmarks(body)
    eye, tip, chin, H = m["eye"], m["tip"], m["chin"], m["H"]
    head = "Head"
    # A broad face: wider cheeks and jaw, a square chin.
    body.scale(H + v(0, 0.03, 0.04), (0.13, 0.1, 0.12), (1.0 + 0.1 * k, 1.0, 1.0), sym=False,
               bones=head)
    body.scale(chin + v(0, 0.01, -0.02), (0.07, 0.04, 0.06), (1.0 + 0.3 * k, 1.0, 1.0), sym=False,
               bones=head)
    body.inflate(eye + v(0.026, -0.03, -0.015), (0.03, 0.02, 0.03), 0.006 * (0.5 + 0.5 * k),
                 bones=head)
    # A big bulbous nose: a round tip, wide nostrils, a broad bridge.
    body.scale(tip + v(0, 0.004, -0.014), (0.032, 0.034, 0.034),
               (1.0 + 0.7 * k, 1.0 + 0.45 * k, 1.0 + 0.45 * k), sym=False, bones=head)
    body.inflate(tip + v(0, -0.002, -0.004), 0.02, 0.009 * k, sym=False, bones=head)
    body.grab(tip, 0.024, (0, -0.005 * k, 0.006 * k), sym=False, bones=head)
    # A heavy jutting brow; the eyes sit deep beneath it.
    brow = eye + v(0.0, 0.018, 0.016)
    body.grab(brow, (0.045, 0.016, 0.035), (0, -0.006 * k, 0.012 * k), bones=head)
    body.grab(v(0, brow[1] - 0.004, brow[2]), (0.026, 0.016, 0.03), (0, -0.004 * k, 0.01 * k),
              sym=False, bones=head)
    body.smooth(brow, (0.05, 0.03, 0.04), 0.2, 2)
    bm = _brow_mask(body, eye)
    _masked(body, "Brows", bm, brow, (0.05, 0.03, 0.05), delta=(0.0, -0.004 * k, 0.01 * k))
    _masked(body, "Brows", bm, brow, (0.05, 0.03, 0.05), factors=(1.08, 1.0 + 0.4 * k, 1.0))
    body.grab(eye, (0.024, 0.02, 0.03), (0, 0, -0.003 * k), parts=["Body", "Eyes"])


def beard(body):
    """The men's beard grows long and full, down over the chest in a
    broad spade."""
    d = body.parts["Hair_Beard"]
    p = d["positions"]
    m = _landmarks(body)
    mouth_y = m["chin"][1] + 0.03
    below = np.clip((mouth_y - p[:, 1]) / 0.05, 0.0, None)
    t = smoothstep(below)
    out = p.copy()
    # Longer and fuller the lower it hangs.
    out[:, 1] = p[:, 1] - below * 0.06
    out[:, 0] = p[:, 0] * (1.0 + 0.7 * t)
    out[:, 2] = p[:, 2] + t * 0.03 + below * 0.012
    d["positions"] = out


def build(body):
    male = body.sex == "Male"
    sh = SHAPE[body.sex]
    k = sh["face"]

    sculpt_head(body, k)
    body.proportions(size={"*": sh["size"]})
    body.proportions(length=sh["length"], girth=sh["girth"], size=sh["sizes"])

    # Clean shoulders and limbs on the thick frame.
    ua = body.joint("upperarm_l")
    body.smooth(ua + v(0.0, 0.02, 0), (0.14, 0.1, 0.12), 0.6, 10)
    body.inflate(ua + v(0.035, 0.015, 0.0), (0.075, 0.065, 0.08), 0.01)
    for side in ("_l", "_r"):
        even_limb(body, "upperarm" + side, "lowerarm" + side, 0.75, 0.04)
        even_limb(body, "lowerarm" + side, "hand" + side, 0.75, 0.035)
        even_limb(body, "thigh" + side, "calf" + side, 0.6, 0.05)
    body.smooth(body.at("hand_l", (-0.012, 0, 0)), (0.045, 0.05, 0.05), 0.5, 6)

    # A barrel chest and a deep round belly.
    sp3, sp1 = body.joint("spine_03"), body.joint("spine_01")
    body.grab(sp3 + v(0, 0.02, 0.12), (0.2, 0.13, 0.1), (0, 0.004, 0.024), sym=False)
    body.grab(sp1 + v(0, 0.02, 0.12), (0.18, 0.13, 0.1), (0, -0.005, 0.03 if male else 0.016),
              sym=False)
    body.smooth(sp1 + v(0, 0.06, 0.12), (0.2, 0.18, 0.1), 0.3, 3, sym=False)
    # Bull neck: the trapezius rises toward the ears.
    neck = body.joint("neck_01")
    body.grab(neck + v(0.08, -0.04, -0.06), (0.12, 0.09, 0.08), (0.0, 0.04 * k + 0.015, 0.0),
              bones=["spine_03", "neck_01", "clavicle_*"])
    body.smooth(neck + v(0.09, -0.02, -0.03), (0.11, 0.08, 0.1), 0.4, 4)

    fit_hair(body)
    if male:
        beard(body)
    bake_cavities(body)
