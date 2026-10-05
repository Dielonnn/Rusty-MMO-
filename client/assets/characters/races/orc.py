"""Orcs: proud desert warriors of the Scorchsand Wastes.

Hulking and hunched: huge shoulders and trapezius rising toward the ears, a
thick neck carried forward from a barrel chest, big arms and hands, thick
slightly short legs. A small heavy head: jutting brow over small deep-set
eyes, a broad flat nose, a wide jaw with an underbite and two lower tusks,
small pointed ears swept back.
"""

import numpy as np

import sculpt

PREVIEW_SKIN = (112, 138, 74)

# Face landmarks of the base heads (model space, before any reshaping).
FACE = {
    "Male": dict(head=(0.0, 1.600, -0.017), chin=(0.0, 1.598, 0.085), lip=(0.0, 1.616, 0.091),
                 mouth=(0.0, 1.622, 0.090), nose=(0.0, 1.655, 0.114), base=(0.0, 1.643, 0.098),
                 eye=(0.034, 1.698, 0.070), brow=(0.030, 1.713, 0.092), ear=(0.084, 1.698, -0.016),
                 jaw=(0.058, 1.606, -0.005), top=1.81),
    "Female": dict(head=(0.0, 1.550, -0.011), chin=(0.0, 1.562, 0.083), lip=(0.0, 1.576, 0.091),
                   mouth=(0.0, 1.584, 0.088), nose=(0.0, 1.615, 0.111), base=(0.0, 1.604, 0.094),
                   eye=(0.035, 1.656, 0.063), brow=(0.032, 1.672, 0.088), ear=(0.084, 1.652, -0.020),
                   jaw=(0.055, 1.566, -0.008), top=1.767),
}


def v(*a):
    return np.array(a, float)


def softplus(x, w):
    """A smooth ramp: 0 well below zero, x well above it."""
    return w * np.logaddexp(0.0, x / w)


def smoothstep(t):
    t = np.clip(t, 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


def descendants(body, names):
    ks = {body.index[n] for n in names}
    grew = True
    while grew:
        grew = False
        for k, p in enumerate(body.parent):
            if p in ks and k not in ks:
                ks.add(k)
                grew = True
    return sorted(ks)


def shift(body, names, delta, field=None):
    """Moves joints (and their children) by `delta`, carrying the mesh
    along: each vertex by its skin weight on them, or by `field(part,
    positions)` (0..1) when given, for a smooth blend over a long stretch
    (a neck carried forward without folds)."""
    delta = np.asarray(delta, float)
    ks = descendants(body, names)
    for k in ks:
        body.world[k] = body.world[k].copy()
        body.world[k][:3, 3] += delta
    for k in range(len(body.names)):
        p = body.parent[k]
        body.local[k] = body.world[k] if p is None else np.linalg.inv(body.world[p]) @ body.world[k]
    for name, d in body.parts.items():
        w = (d["weights"] * np.isin(d["joints"], ks)).sum(1)
        if field is not None:
            w = field(name, d["positions"])
        d["positions"] = d["positions"] + w[:, None] * delta
    body._axes()


def masked(body, part, mask, fn):
    """Moves the vertices of `part` by fn(positions) scaled by `mask`
    (0..1 per vertex)."""
    d = body.parts[part]
    p = d["positions"]
    d["positions"] = p + mask[:, None] * fn(p)


def sweep_ears(body, ear, k):
    """Small pointed ears swept back: each ear's rim is levered out from
    where it meets the skull (nothing on the skull moves), its top drawn
    up and back into a point."""
    for name in body.parts:
        if name != "Body":
            continue
        for side in (1, -1):
            p = body.parts[name]["positions"]
            c = v(ear[0] * side, ear[1], ear[2])
            near = sculpt.falloff(np.linalg.norm((p - c) / v(0.04, 0.06, 0.05), axis=1))
            lever = np.clip((p[:, 0] * side - 0.074) / 0.016, 0.0, 1.2)
            m = near * lever * body.weight_of(name, "Head")
            t = smoothstep((p[:, 1] - (ear[1] - 0.025)) / 0.045)
            d = np.zeros_like(p)
            d[:, 0] = side * (0.006 * t - 0.002)
            d[:, 1] = 0.016 * t * t
            d[:, 2] = -0.012 - 0.03 * t * t
            body.parts[name]["positions"] = p + (m * k)[:, None] * d


def even_limb(body, bone, child, strength=0.7, sigma=0.04):
    """Evens out a limb along its length: each vertex's distance from the
    bone is drawn toward the average of its neighbours along the bone (at
    the same angle round it). Big muscle shapes stay, the sharp peaks that
    thinning would turn into spikes go."""
    d = body.parts["Body"]
    p = d["positions"]
    a, b = body.joint(bone), body.joint(child)
    ax = (b - a) / np.linalg.norm(b - a)
    L = np.linalg.norm(b - a)
    rel = p - a
    t = rel @ ax
    radial = rel - t[:, None] * ax
    r = np.linalg.norm(radial, axis=1)
    w = body.weight_of("Body", bone)
    sel = np.where((w > 0.05) & (t > -0.05) & (t < L + 0.05))[0]
    if len(sel) == 0:
        return
    u = np.cross(ax, [0.0, 0.0, 1.0])
    if np.linalg.norm(u) < 1e-3:
        u = np.cross(ax, [0.0, 1.0, 0.0])
    u /= np.linalg.norm(u)
    u2 = np.cross(ax, u)
    th = np.arctan2(radial[sel] @ u2, radial[sel] @ u)
    ts, rs = t[sel], r[sel]
    dt = (ts[:, None] - ts[None, :]) / sigma
    dth = np.angle(np.exp(1j * (th[:, None] - th[None, :]))) / 0.35
    K = np.exp(-0.5 * (dt ** 2 + dth ** 2))
    smooth_r = (K @ rs) / K.sum(1)
    ends = smoothstep(ts / 0.04) * smoothstep((L - ts) / 0.04)
    m = strength * w[sel] * ends
    new_r = rs + m * (smooth_r - rs)
    scale = new_r / np.maximum(rs, 1e-6)
    out = p.copy()
    out[sel] = a + ts[:, None] * ax + radial[sel] * scale[:, None]
    d["positions"] = out


def nearest_rows(a, b, chunk=512):
    """Index of the nearest row of `b` for each row of `a`."""
    out = np.empty(len(a), np.int64)
    for s in range(0, len(a), chunk):
        d = ((a[s:s + chunk, None, :] - b[None, :, :]) ** 2).sum(2)
        out[s:s + chunk] = d.argmin(1)
    return out


def fit_hair(body, margin=0.003, reach=0.025):
    """Lifts each hairstyle off the reshaped head and neck wherever skin
    would poke through it, moving whole patches so the hair keeps its
    thickness."""
    B = body.parts["Body"]
    bn = body.normals("Body")
    for name in list(body.parts):
        if not name.startswith("Hair_"):
            continue
        d = body.parts[name]
        ph = d["positions"]
        lo, hi = ph.min(0) - 0.05, ph.max(0) + 0.05
        near = np.where(np.all((B["positions"] > lo) & (B["positions"] < hi), axis=1))[0]
        if len(near) == 0:
            continue
        k = near[nearest_rows(ph, B["positions"][near])]
        n = bn[k]
        s = ((ph - B["positions"][k]) * n).sum(1)
        need = np.clip(margin - s, 0.0, None)
        hit = np.where(need > 0)[0]
        if len(hit) == 0:
            continue
        push = np.zeros(len(ph))
        for c in range(0, len(ph), 512):
            dist = np.linalg.norm(ph[c:c + 512, None, :] - ph[None, hit, :], axis=2)
            push[c:c + 512] = (need[hit][None, :] * sculpt.falloff(dist / reach)).max(1)
        d["positions"] = ph + push[:, None] * n


def bake_cavities(body, bones=("Head",), depth=0.0025, dark=0.26, light=0.0, iterations=12):
    """Paints the head's creases darker and its ridges a touch lighter
    (under the brow, round the eyes, beside the nose and mouth, under the
    jaw), so the sculpted shapes read even in flat light. Only multiplies
    the texture, so the skin keeps its colour and takes the game's tint."""
    d = body.parts["Body"]
    p = d["positions"]
    n = body.normals("Body")
    weld, nbrs = body._graph("Body")
    q = body._welded(p, weld)
    cnt = np.bincount(nbrs[:, 0], minlength=len(q))[:, None]
    sm = q.copy()
    for _ in range(iterations):
        avg = np.zeros_like(sm)
        np.add.at(avg, nbrs[:, 0], sm[nbrs[:, 1]])
        sm = 0.5 * sm + 0.5 * np.where(cnt > 0, avg / np.maximum(cnt, 1), sm)
    cav = ((sm[weld] - p) * n).sum(1) / depth
    region = smoothstep(body.weight_of("Body", list(bones)) * 1.4 - 0.2)
    k = 1.0 - dark * smoothstep(cav) * region + light * smoothstep(-cav - 0.6) * region
    tris = d["indices"].reshape(-1, 3)
    tris = tris[(np.abs(k[tris] - 1.0) > 1e-3).any(1)]
    img = np.asarray(body.texture).astype(np.float32)
    gain = np.ones(img.shape[:2], np.float32)
    uv = d["uvs"] * sculpt.ATLAS - 0.5
    for t in tris:
        a, b, c = uv[t]
        x0, y0 = np.floor(np.minimum(np.minimum(a, b), c)).astype(int)
        x1, y1 = np.ceil(np.maximum(np.maximum(a, b), c)).astype(int) + 1
        x0, y0 = max(x0 - 1, 0), max(y0 - 1, 0)
        x1, y1 = min(x1 + 1, sculpt.ATLAS), min(y1 + 1, sculpt.ATLAS)
        if x1 <= x0 or y1 <= y0:
            continue
        yy, xx = np.mgrid[y0:y1, x0:x1]
        m = np.array([[b[0] - a[0], c[0] - a[0]], [b[1] - a[1], c[1] - a[1]]])
        det = np.linalg.det(m)
        if abs(det) < 1e-9:
            continue
        inv = np.linalg.inv(m)
        rx, ry = xx - a[0], yy - a[1]
        l1 = inv[0, 0] * rx + inv[0, 1] * ry
        l2 = inv[1, 0] * rx + inv[1, 1] * ry
        l0 = 1.0 - l1 - l2
        inside = (l0 > -0.08) & (l1 > -0.08) & (l2 > -0.08)
        val = l0 * k[t[0]] + l1 * k[t[1]] + l2 * k[t[2]]
        sub = gain[y0:y1, x0:x1]
        sub[inside] = val[inside]
    from PIL import Image, ImageFilter
    g = Image.fromarray(np.clip(gain * 127.5, 0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(1.2))
    gain = np.asarray(g).astype(np.float32) / 127.5
    x, y, w, h = sculpt.BODY_RECT
    img[y:y + h, x:x + w] *= gain[y:y + h, x:x + w, None]
    body.texture.paste(Image.fromarray(np.clip(img, 0, 255).astype(np.uint8)))


def smooth_off(body, center, radius, strength, iterations, spare="Head", sym=True):
    """Relaxes the body near `center`, leaving what is skinned to `spare`
    (the jaw above a neck being smoothed) where it is."""
    d = body.parts["Body"]
    before = d["positions"].copy()
    body.smooth(center, radius, strength, iterations, sym=sym)
    keep = body.weight_of("Body", spare)[:, None]
    d["positions"] = before + (d["positions"] - before) * (1.0 - keep)


# ---- The head ----

def sculpt_head(body, f, k):
    """`f` the landmarks, `k` how strong (1 male, less female)."""
    H = "Head"
    chin, lip, mouth, nose, base = (v(*f[n]) for n in ("chin", "lip", "mouth", "nose", "base"))
    eye, brow, ear, jaw = (v(*f[n]) for n in ("eye", "brow", "ear", "jaw"))
    top = f["top"]

    # A low cranium with a forehead sloping back from the brow.
    body.scale(v(0, brow[1], -0.01), (0.16, 0.13, 0.16), (1.0, 1.0 - 0.08 * k, 1.0), sym=False, bones=H)
    body.grab(v(0, top - 0.035, 0.07), (0.13, 0.08, 0.09), (0, -0.006 * k, -0.016 * k), sym=False, bones=H)

    # The muzzle juts: mouth, lips and jaw come forward.
    body.grab(v(0, mouth[1] - 0.004, mouth[2]), (0.06, 0.04, 0.06), (0, 0, 0.008 * k), sym=False, bones=H)

    # Jaw: the angles flare out and down into a sharp corner, the jawline
    # runs straight to a broad square chin.
    body.grab(jaw + v(0.0, -0.004, -0.004), (0.026, 0.026, 0.032), (0.022 * k, -0.014 * k, -0.002 * k), bones=H)
    body.grab(v(jaw[0] * 0.7, (jaw[1] + chin[1]) / 2 - 0.01, (jaw[2] + chin[2]) / 2),
              (0.026, 0.02, 0.03), (0.013 * k, -0.010 * k, 0.004 * k), bones=H)
    body.scale(chin + v(0, -0.004, -0.012), (0.045, 0.03, 0.04), (1.0 + 0.5 * k, 1.0, 1.0), sym=False, bones=H)
    body.grab(chin + v(0.022, -0.012, -0.008), (0.018, 0.014, 0.02), (0.006 * k, -0.006 * k, 0.002 * k), bones=H)
    # Underbite: chin and lower lip thrust forward past the upper lip.
    body.grab(v(0, lip[1] - 0.012, chin[2]), (0.065, 0.03, 0.06), (0, -0.008 * k, 0.02 * k), sym=False, bones=H)
    body.grab(lip, (0.038, 0.010, 0.03), (0, -0.002 * k, 0.008 * k), sym=False, bones=H)
    body.grab(v(0, mouth[1] + 0.012, mouth[2]), (0.035, 0.012, 0.03), (0, 0.001, -0.004 * k), sym=False, bones=H)
    body.smooth(v(0, chin[1] + 0.01, chin[2]), (0.08, 0.04, 0.06), 0.25, 2, sym=False)

    # Broad flat nose with wide flared nostrils and a low bridge.
    nc = v(0, (nose[1] + base[1]) / 2, nose[2] - 0.018)
    body.scale(nc, (0.045, 0.03, 0.045), (1.0 + 0.6 * k, 0.88, 0.6), sym=False, bones=H)
    body.grab(v(0.014, base[1] + 0.003, base[2] - 0.004), (0.016, 0.012, 0.016), (0.011 * k, 0.003 * k, 0.0), bones=H)
    body.grab(v(0, nose[1], nose[2] - 0.01), (0.016, 0.014, 0.02), (0, 0.005 * k, -0.003 * k), sym=False, bones=H)
    body.grab(v(0, nose[1] + 0.025, nose[2] - 0.02), (0.018, 0.022, 0.02), (0, 0, -0.006 * k), sym=False, bones=H)

    # Heavy jutting brow ridge (the eyebrows ride along), a groove above.
    body.grab(brow + v(0, 0.002, 0), (0.044, 0.016, 0.035), (0, -0.009 * k, 0.021 * k), bones=H)
    body.grab(v(0, brow[1] - 0.004, brow[2]), (0.026, 0.016, 0.03), (0, -0.006 * k, 0.016 * k), sym=False, bones=H)
    body.inflate(v(0.02, brow[1] + 0.03, brow[2] - 0.01), (0.05, 0.016, 0.04), -0.003 * k, bones=H)
    body.smooth(brow, (0.05, 0.03, 0.04), 0.2, 2)

    # Small deep-set eyes.
    body.scale(eye, (0.026, 0.02, 0.03), (0.82, 0.78, 0.9), parts=["Body", "Eyes"])
    body.grab(eye, (0.026, 0.02, 0.03), (0, 0, -0.005 * k), parts=["Body", "Eyes"])

    # Strong cheekbones, hollows under them.
    body.inflate(v(0.054, eye[1] - 0.024, eye[2] - 0.012), (0.022, 0.014, 0.022), 0.005 * k, bones=H)
    body.inflate(v(0.052, mouth[1] + 0.012, mouth[2] - 0.04), (0.02, 0.018, 0.02), -0.005 * k, bones=H)
    # Creases from the nostrils round the muzzle.
    for t in np.linspace(0.0, 1.0, 4):
        c = v(0.022 + 0.012 * t, base[1] - 0.004 - 0.026 * t, base[2] - 0.016 - 0.008 * t)
        body.inflate(c, 0.008, -0.0015 * k, bones=H)
    body.smooth(v(0.03, mouth[1] + 0.01, mouth[2] - 0.02), (0.03, 0.03, 0.03), 0.15, 1)

    # A shorter skull: the back of the head comes forward, the crown down.
    hz0 = ear[2] + 0.02
    head_y = f["head"][1]
    for name in body.parts:
        if name.startswith("Extra_"):
            continue
        # By height, not skin weight, so hair keeps sitting on the skull.
        w = smoothstep((body.parts[name]["positions"][:, 1] - (head_y - 0.03)) / 0.06)

        def squash(p):
            d = np.zeros_like(p)
            back = softplus(hz0 - p[:, 2], 0.015)
            d[:, 2] = back * 0.16 * k
            up = softplus(p[:, 1] - (eye[1] + 0.02), 0.015)
            d[:, 1] = -up * 0.08 * k
            return d
        masked(body, name, w, squash)

    sweep_ears(body, ear, 0.6 + 0.4 * k)

    # Relax the cranium and temples so the reshaped skull stays clean.
    body.smooth(v(0, top - 0.03, -0.01), (0.1, 0.06, 0.12), 0.4, 4, sym=False)
    body.smooth(v(0.062, brow[1] + 0.04, brow[2] - 0.05), (0.035, 0.03, 0.04), 0.4, 3)


def tusks(body, f, k):
    """Two lower tusks rising out of the jaw past the upper lip, curving
    out and a little back."""
    lip = v(*f["lip"])
    p = body.parts["Body"]["positions"]
    for side, name in ((1, "Extra_TuskL"), (-1, "Extra_TuskR")):
        want = v(0.022 * side, lip[1], lip[2] + 0.02)
        q = p[np.argmin(((p - want) ** 2).sum(1))]
        p0 = q + v(0.0, -0.005, -0.011)
        L = 0.042 * (0.45 + 0.55 * k)
        p1 = p0 + v(0.003 * side, 0.55 * L, 0.013)
        p2 = p0 + v(0.011 * side, 1.0 * L, 0.003)
        t = np.linspace(0.0, 1.0, 7)[:, None]
        path = (1 - t) ** 2 * p0 + 2 * (1 - t) * t * p1 + t ** 2 * p2
        r = 0.0078 * (0.6 + 0.4 * k)
        radii = r * (1.0 - t[:, 0] ** 1.6) * (1.0 - 0.15 * t[:, 0])
        body.tube(name, path, radii, sides=10, up=(0, 0, 1), color="ivory", bone="Head")


def amber_eyes(body):
    x, y, w, h = sculpt.EYES_RECT
    a = np.asarray(body.texture).astype(np.float32)
    eye = a[y:y + h, x:x + w]
    yy, xx = np.mgrid[0:h, 0:w]
    r = np.hypot(xx - w * 0.498, yy - h * 0.498) / (w * 0.106)
    iris = smoothstep((1.08 - r) / 0.12)[..., None]
    lum = eye.mean(2, keepdims=True) / 255.0
    ring = smoothstep((r - 0.55) / 0.4)[..., None]
    amber = np.array([235, 150, 30], np.float32)
    rust = np.array([150, 40, 18], np.float32)
    col = (amber * (1 - ring) + rust * ring) * np.clip(lum * 2.4, 0, 1.4)
    col = np.where(lum < 0.12, eye, col)
    # Yellowed whites.
    tint = np.array([1.0, 0.93, 0.78], np.float32)
    out = eye * (1 - iris) * tint + col * iris
    a[y:y + h, x:x + w] = np.clip(out, 0, 255)
    from PIL import Image
    body.texture.paste(Image.fromarray(a.astype(np.uint8)))


# ---- The body ----

SHAPE = {
    "Male": dict(
        face=1.0, tusk=1.0, size=1.131, posture=1.0,
        length={"spine_0*": 1.12, "thigh_*": 0.92, "calf_*": 0.93, "neck_01": 0.8,
                "clavicle_*": 1.45, "upperarm_*": 1.04, "lowerarm_*": 1.02},
        girth={"pelvis": 1.1, "spine_01": 1.12, "spine_02": 1.28, "spine_03": 1.34,
               "neck_01": 1.25, "clavicle_*": 1.1, "upperarm_*": 1.4, "lowerarm_*": 1.5,
               "thigh_*": 1.3, "calf_*": 1.32},
        sizes={"Head": 0.85, "hand_*": 1.28, "index*": 1.28, "middle*": 1.28, "ring*": 1.28,
               "pinky*": 1.28, "thumb*": 1.28, "foot_*": 1.12, "ball_*": 1.12}),
    "Female": dict(
        face=0.7, tusk=0.3, size=1.073, posture=0.55,
        length={"spine_0*": 1.06, "thigh_*": 0.95, "calf_*": 0.96, "neck_01": 0.9,
                "clavicle_*": 1.38, "upperarm_*": 1.03, "lowerarm_*": 1.02},
        girth={"pelvis": 1.0, "spine_01": 1.0, "spine_02": 1.1, "spine_03": 1.22,
               "neck_01": 1.2, "clavicle_*": 1.05, "upperarm_*": 1.3, "lowerarm_*": 1.32,
               "thigh_*": 1.1, "calf_*": 1.15},
        sizes={"Head": 0.9, "hand_*": 1.15, "index*": 1.15, "middle*": 1.15, "ring*": 1.15,
               "pinky*": 1.15, "thumb*": 1.15, "foot_*": 1.06, "ball_*": 1.06}),
}

THROAT = {"Male": (0.0, 1.545, 0.022), "Female": (0.0, 1.515, 0.004)}


def nearest(body, point):
    p = body.parts["Body"]["positions"]
    return int(np.argmin(((p - np.asarray(point, float)) ** 2).sum(1)))


def build(body):
    male = body.sex == "Male"
    f = FACE[body.sex]
    sh = SHAPE[body.sex]
    P = body.parts["Body"]
    throat_i = nearest(body, THROAT[body.sex])
    throat0 = P["positions"][throat_i].copy()

    sculpt_head(body, f, sh["face"])
    tusks(body, f, sh["tusk"])
    amber_eyes(body)

    # Overall size, then the orc's proportions.
    body.proportions(size={"*": sh["size"]})
    body.proportions(length=sh["length"], girth=sh["girth"], size=sh["sizes"])

    # Round, clean shoulders where the thick arm meets the collarbone: relax
    # the seam, then swell the deltoid back out into a cap.
    ua = body.joint("upperarm_l")
    body.smooth(ua + v(0.0, 0.02, 0), (0.17, 0.12, 0.14), 0.6, 10)
    body.inflate(ua + v(0.04, 0.02, 0.0), (0.09, 0.08, 0.1), 0.012)
    body.smooth(ua + v(0.03, 0.03, 0), (0.12, 0.1, 0.12), 0.3, 3)
    # Relax the arms and legs a little so the thinned game mesh keeps a clean
    # outline.
    for side in ("_l", "_r"):
        even_limb(body, "upperarm" + side, "lowerarm" + side, 0.75, 0.045)
        even_limb(body, "lowerarm" + side, "hand" + side, 0.75, 0.04)
    arm = body.joint("lowerarm_l")[0] - body.joint("upperarm_l")[0]
    fore = body.joint("hand_l")[0] - body.joint("lowerarm_l")[0]
    for c, rx in ((body.at("upperarm_l", (0.55 * arm, 0, 0)), 0.75 * arm),
                  (body.at("lowerarm_l", (0.4 * fore, 0, 0)), 0.7 * fore)):
        body.smooth(c, (rx, 0.11, 0.11), 0.4, 4)
    body.smooth(body.at("upperarm_l", (-0.01, -0.08, 0.0)), (0.09, 0.08, 0.1), 0.6, 10)
    # Blend the thick forearm into the big hand.
    body.smooth(body.at("hand_l", (-0.015, 0, 0)), (0.05, 0.06, 0.06), 0.5, 6)
    for name in ("thigh_l", "calf_l"):
        body.smooth(body.at(name, (0.0, -0.2, 0.0)), (0.13, 0.26, 0.14), 0.3, 3)
    if not male:
        # Athletic: a narrow waist under broad shoulders.
        sp1 = body.joint("spine_01")
        body.scale(sp1 + v(0, 0.06, 0.0), (0.3, 0.12, 0.3), (0.9, 1.0, 0.94), sym=False)
        pel = body.joint("pelvis")
        body.scale(pel + v(0, -0.04, 0.0), (0.32, 0.14, 0.3), (0.93, 1.0, 0.96), sym=False)

    # Posture: the head and neck carried low and forward, the throat kept
    # back under the jaw.
    s = sh["posture"]
    before = P["positions"][throat_i].copy()
    lo = body.joint("neck_01")[1] - 0.08
    hi = body.joint("Head")[1] - 0.015

    def neck_field(name, p):
        up = smoothstep((p[:, 1] - lo) / (hi - lo))
        side = 1.0 - smoothstep((np.abs(p[:, 0]) - 0.09) / 0.1)
        return np.maximum(up * side, body.weight_of(name, "Head"))
    shift(body, ["neck_01"], (0, -0.012 * s, 0.07 * s), neck_field)
    moved = P["positions"][throat_i] - before
    th = P["positions"][throat_i]
    body.grab(th + v(0, -0.01, -0.01), (0.09, 0.07, 0.07), (0, 0, -0.35 * moved[2]), sym=False,
              bones=["neck_01", "spine_03"])
    body.smooth(th + v(0, -0.005, -0.01), (0.06, 0.045, 0.05), 0.5, 5, sym=False)

    neck = body.joint("neck_01")
    sp3 = body.joint("spine_03")
    # Trapezius: huge slopes from the neck out to the shoulders.
    body.grab(neck + v(0.10, -0.05, -0.075), (0.15, 0.11, 0.085), (0.0, 0.075 * s, -0.012 * s),
              bones=["spine_03", "neck_01", "clavicle_*"])
    body.smooth(neck + v(0.12, -0.03, -0.04), (0.14, 0.1, 0.12), 0.4, 4)
    smooth_off(body, neck + v(0.09, 0.035, 0.01), (0.075, 0.065, 0.09), 0.7, 10)
    # Ease the collarbones down where the neck leaves the chest, so the two
    # meet in a soft curve, not a crease.
    body.grab(neck + v(0.05, 0.0, 0.075), (0.09, 0.04, 0.05), (0, -0.012 * s, 0.004 * s),
              bones=["spine_03", "clavicle_*", "neck_01"])
    body.smooth(neck + v(0.0, 0.005, 0.065), (0.13, 0.035, 0.045), 0.6, 8, sym=False)
    # Hunch: the upper back rounds out into a hump.
    body.grab(sp3 + v(0, 0.13, -0.15), (0.28, 0.2, 0.15), (0, 0.035 * s, -0.065 * s), sym=False)
    body.smooth(sp3 + v(0, 0.13, -0.16), (0.24, 0.18, 0.1), 0.3, 3, sym=False)
    # Barrel chest.
    body.grab(sp3 + v(0, 0.03, 0.14), (0.24, 0.16, 0.11), (0, 0.005 * s, 0.032 * s), sym=False)
    body.smooth(sp3 + v(0, 0.03, 0.14), (0.2, 0.14, 0.08), 0.25, 2, sym=False)
    body.smooth(neck + v(0, -0.04, -0.04), (0.2, 0.12, 0.14), 0.3, 3, sym=False)

    # Keep every hairstyle sitting on the reshaped head.
    fit_hair(body)
    bake_cavities(body)
