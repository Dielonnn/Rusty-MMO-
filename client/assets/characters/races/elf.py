"""Elves: tall, slender and ancient. Long legs and neck, a narrow waist,
lean limbs instead of the base bodybuilder bulk, an upright proud chest.
The head is finer: high sharp cheekbones, a narrow pointed chin, a long
straight nose, eyes slanting up at the outer corners, and long swept-back
pointed ears (modeled, the human ears pressed flat under them)."""

import numpy as np

PREVIEW_SKIN = (238, 214, 200)


def _landmarks(body):
    """Face points read off the mesh as it is now."""
    p = body.parts["Body"]["positions"]
    head = body.weight_of("Body", "Head") > 0.5
    h = p[head]
    e = body.parts["Eyes"]["positions"]
    eye = e[e[:, 0] > 0].mean(0)
    tip = h[np.argmax(h[:, 2])]
    mid = h[np.abs(h[:, 0]) < 0.006]
    front = mid[mid[:, 2] > eye[2] - 0.03]
    chin = front[np.argmin(front[:, 1])]
    ear = h[(h[:, 0] > 0.068) & (h[:, 2] < 0.012) & (np.abs(h[:, 1] - (eye[1] - 0.015)) < 0.04)]
    return {"eye": eye, "tip": tip, "chin": chin, "ear": ear.mean(0),
            "top": h[np.argmax(h[:, 1])], "H": body.joint("Head")}


def _ear(body, name, root, tip, upper, lower, thick, cup, face, side):
    """A long leaf-shaped ear from `root` (inside the skull) to `tip`:
    rings across its width (a straight upper edge, a curved lower one),
    the front face cupped like an ear's bowl, the rim rolled."""
    root, tip = np.asarray(root, float), np.asarray(tip, float)
    L = tip - root
    length = np.linalg.norm(L)
    L /= length
    n = np.asarray(face, float)
    n = n - n.dot(L) * L
    n /= np.linalg.norm(n)
    w = np.cross(n, L)
    if w[1] < 0:
        w = -w
    ts = np.array([0.0, 0.07, 0.15, 0.24, 0.34, 0.44, 0.54, 0.64, 0.74, 0.83, 0.91, 0.97, 1.0])
    s = np.array([-1.0, -0.88, -0.6, -0.2, 0.2, 0.6, 0.88, 1.0])
    rings = []
    for t in ts:
        hi = upper * (1.0 - t) ** 1.1
        lo = -lower * np.sin(np.pi * (0.22 + 0.78 * t)) ** 1.3
        mid, wd = (hi + lo) / 2, (hi - lo) / 2
        th = thick * (1.0 - t) ** 0.7
        cp = cup * (1.0 - t) ** 1.5
        # The spine bows a little up.
        c = root + L * length * t + w * (0.008 * np.sin(np.pi * t))
        ring = []
        for k in list(range(len(s))) + list(range(len(s) - 1, -1, -1)):
            front = len(ring) < len(s)
            sk = s[k]
            # Thicker at the rim (a rolled helix), a bowl in the front.
            rim = np.sqrt(max(1.0 - sk * sk, 0.0))
            nn = (0.5 if front else -0.5) * th * (0.45 + 0.55 * rim) - cp * (1.0 - sk * sk)
            ring.append(c + w * (mid + wd * sk) + n * nn)
        rings.append(ring)
    rings = np.array(rings)
    if side < 0:
        rings = rings * np.array([-1.0, 1.0, 1.0])
    return body.loft(name, rings, color="skin", bone="Head")


def _press_ear(body, ear):
    """Presses the human ear into the skull: the vertices of the ear are
    put on a smooth surface fitted to the head around it."""
    d = body.parts["Body"]
    p = d["positions"].copy()
    hw = body.weight_of("Body", "Head")
    ry, rz = 0.06, 0.045
    inner, outer = [], []
    for side in (1.0, -1.0):
        x = p[:, 0] * side
        dy, dz = p[:, 1] - ear[1], p[:, 2] - (ear[2] - 0.004)
        e = np.hypot(dy / ry, dz / rz)
        onside = x > 0.035
        ring = onside & (e > 1.25) & (e < 1.9) & (hw > 0.3)
        A = lambda yy, zz: np.stack([np.ones_like(yy), yy, zz, yy * yy, zz * zz, yy * zz], 1)
        coef = np.linalg.lstsq(A(dy[ring], dz[ring]), x[ring], rcond=None)[0]
        target = A(dy, dz) @ coef
        w = np.clip((1.25 - e) / 0.3, 0, 1) * onside
        w = w * w * (3 - 2 * w)
        inner.append(onside & (e < 1.05))
        outer.append(onside & (e > 1.3) & (e < 1.7) & (hw > 0.3))
        p[:, 0] = np.where(w > 0, side * (x + w * (target - x)), p[:, 0])
    d["positions"] = p
    body.smooth(ear, (0.04, 0.065, 0.05), 0.8, 20)

    # The ear is painted into the skin texture too: paint it out with the
    # skin around it.
    inner, outer = inner[0] | inner[1], outer[0] | outer[1]
    tris = d["indices"].reshape(-1, 3)
    ear_tris = tris[inner[tris].all(1)]
    uv = d["uvs"] * 1024

    def paint(image, draw, body):
        from PIL import Image, ImageDraw, ImageFilter
        px = image.load()
        ring = np.array([px[int(u), int(v)] for u, v in uv[outer]], float)
        color = tuple(int(c) for c in np.median(ring, 0))
        mask = Image.new("L", image.size, 0)
        md = ImageDraw.Draw(mask)
        for t in ear_tris:
            md.polygon([tuple(uv[k]) for k in t], fill=255)
        mask = mask.filter(ImageFilter.MaxFilter(5)).filter(ImageFilter.GaussianBlur(3))
        image.paste(Image.composite(Image.new("RGB", image.size, color), image, mask))
    body.paint(paint)


def _pieces(d):
    """Connected piece id of every vertex of a part (welded by position)."""
    p = d["positions"]
    _, weld = np.unique(np.round(p, 5), axis=0, return_inverse=True)
    weld = weld.reshape(-1)
    par = np.arange(weld.max() + 1)

    def find(a):
        while par[a] != a:
            par[a] = par[par[a]]
            a = par[a]
        return a
    for a, b, c in weld[d["indices"].reshape(-1, 3)]:
        for u, v in ((a, b), (b, c)):
            ru, rv = find(u), find(v)
            if ru != rv:
                par[ru] = rv
    roots = np.array([find(a) for a in range(len(par))])
    return roots[weld]


def _brow_mask(body, eye):
    """1 for the eyebrow pieces of "Brows", 0 for the eyelashes."""
    d = body.parts["Brows"]
    ids = _pieces(d)
    mask = np.zeros(len(ids))
    for r in np.unique(ids):
        sel = ids == r
        if d["positions"][sel, 1].mean() > eye[1] + 0.012:
            mask[sel] = 1.0
    return mask


def _masked(body, part, mask, center, radius, delta=None, factors=None):
    """A mirrored grab (`delta`) or scale (`factors`) on a part, only where
    `mask` is 1."""
    import sculpt
    d = body.parts[part]
    r = np.broadcast_to(np.asarray(radius, float), (3,))
    for side in (1.0, -1.0):
        c = np.asarray(center, float) * np.array([side, 1.0, 1.0])
        p = d["positions"]
        f = sculpt.falloff(np.linalg.norm((p - c) / r, axis=1)) * mask
        if delta is not None:
            d["positions"] = p + f[:, None] * (np.asarray(delta, float) * np.array([side, 1, 1]))
        else:
            d["positions"] = p + f[:, None] * (p - c) * (np.asarray(factors, float) - 1.0)


def _soften(body, select, amount=0.6, radius=7):
    """Softens the painted muscle shading of the skin texture where
    `select(positions)` is true (underwear texels are left alone and
    don't bleed in)."""
    from PIL import Image, ImageDraw, ImageFilter
    d = body.parts["Body"]
    tris = d["indices"].reshape(-1, 3)
    inside = select(d["positions"])
    tris = tris[inside[tris].all(1)]
    uv = d["uvs"] * 1024

    def paint(image, draw, body):
        mask = Image.new("L", image.size, 0)
        md = ImageDraw.Draw(mask)
        for t in tris:
            md.polygon([tuple(uv[k]) for k in t], fill=255)
        mask = mask.filter(ImageFilter.GaussianBlur(radius / 2))
        a = np.asarray(image).astype(np.float32)
        lum = a.mean(2)
        x0, y0, w, h = 0, 0, 768, 768
        skin = np.zeros(lum.shape, np.float32)
        ref = np.median(lum[y0:y0 + h, x0:x0 + w])
        skin[y0:y0 + h, x0:x0 + w] = lum[y0:y0 + h, x0:x0 + w] > ref * 0.5
        k = lambda img: np.asarray(Image.fromarray(img).filter(ImageFilter.GaussianBlur(radius)),
                                   np.float32)
        wsum = k((skin * 255).astype(np.uint8)) / 255.0
        blur = np.stack([k((a[..., c] * skin).astype(np.uint8)) for c in range(3)], 2)
        blur = blur / np.maximum(wsum[..., None], 1e-3)
        m = np.asarray(mask, np.float32)[..., None] / 255.0 * amount * skin[..., None]
        out = a * (1 - m) + blur * m
        image.paste(Image.fromarray(np.clip(out, 0, 255).astype(np.uint8)))
    body.paint(paint)


def _eyes(image, draw, body):
    """Luminous pale silver-green irises with a soft glow, the pupil a
    faint darker green."""
    import sculpt
    x0, y0, w, h = sculpt.EYES_RECT
    a = np.asarray(image.crop((x0, y0, x0 + w, y0 + h))).astype(np.float32)
    yy, xx = np.mgrid[0:h, 0:w]
    cx, cy = 127.0 * w / 256, 128.0 * h / 256
    r = np.hypot(xx - cx, yy - cy) / (26.5 * w / 256)
    lum = a.mean(2, keepdims=True) / 255.0
    iris = np.array([178.0, 228.0, 206.0])
    core = np.array([238.0, 252.0, 242.0])
    # Iris: bright, keeping a little of the painted streaks.
    t = np.clip(r, 0, 1)[..., None]
    col = core * (1 - t) + iris * t
    col = col * (0.75 + 0.5 * lum)
    pupil = np.clip(1.0 - r / 0.38, 0, 1)[..., None] ** 0.6
    col = col * (1 - 0.45 * pupil)
    inside = np.clip((1.08 - r) / 0.12, 0, 1)[..., None]
    # A soft glow spilling onto the white around the iris.
    glow = np.clip(1.0 - (r - 1.0) / 0.6, 0, 1)[..., None] * (r > 1.0)[..., None] * 0.35
    out = a * (1 - inside) + col * inside
    out = out * (1 - glow) + np.array([200.0, 245.0, 225.0]) * glow
    from PIL import Image
    image.paste(Image.fromarray(np.clip(out, 0, 255).astype(np.uint8)), (x0, y0))


def build(body):
    male = body.sex == "Male"

    # ---- Proportions: tall and slender ----
    legs = 1.08 if male else 1.075
    body.proportions(
        length={"thigh_*": legs, "calf_*": legs, "neck_01": 1.35, "spine_0*": 1.02,
                "clavicle_*": 0.92, "upperarm_*": 1.03, "lowerarm_*": 1.05, "foot_*": 1.04,
                "index*": 1.08, "middle*": 1.08, "ring*": 1.08, "pinky*": 1.08},
        girth={"upperarm_*": 0.82, "lowerarm_*": 0.86, "thigh_*": 0.86, "calf_*": 0.88,
               "spine_03": 0.9, "spine_02": 0.86, "spine_01": 0.86, "pelvis": 0.9,
               "neck_01": 0.82, "hand_*": 0.9, "foot_*": 0.93, "ball_*": 0.93},
        size={"Head": 0.97},
    )

    # Less bodybuilder: relax the big muscle forms.
    mid = lambda a, b: (body.joint(a) + body.joint(b)) / 2
    for c, r in [(body.at("spine_03", (0.09, 0.07, 0.075)), 0.14),
                 (body.at("spine_02", (0.0, -0.01, 0.095)), 0.16),
                 (body.at("spine_03", (0.1, 0.05, -0.105)), 0.16),
                 (body.at("upperarm_l", (0.02, 0.02, 0.0)), 0.09),
                 (mid("upperarm_l", "lowerarm_l"), 0.12),
                 (mid("lowerarm_l", "hand_l"), 0.12),
                 (mid("thigh_l", "calf_l"), 0.2)]:
        body.smooth(c, r, 0.5, 4)
    # Smaller deltoid caps, so the shoulder flows into the slim arm.
    body.scale(body.at("upperarm_l", (0.03, 0.01, 0.0)), (0.08, 0.09, 0.09), (1.0, 0.86, 0.86),
               bones=["upperarm_l", "upperarm_r", "clavicle_l", "clavicle_r"])

    # ---- Head ----
    m = _landmarks(body)
    eye, tip, chin, H, ear = m["eye"], m["tip"], m["chin"], m["H"], m["ear"]
    head = "Head"

    # Ears: press the human ear flat to the skull (long ones go on later).
    _press_ear(body, ear)

    # A finer, narrower skull and face.
    body.scale(H + np.array([0, 0.07, 0.04]), (0.2, 0.2, 0.2), (0.95, 1.0, 1.0), sym=False)
    # Narrow pointed chin, tapered jaw.
    body.scale(chin + np.array([0, 0.015, -0.02]), (0.08, 0.042, 0.08),
               (0.78 if male else 0.74, 1.0, 1.0), sym=False)
    body.scale(chin + np.array([0.055, 0.02, -0.06]), (0.035, 0.04, 0.05),
               (0.9 if male else 0.85, 1.0, 1.0))
    if male:
        body.grab(chin, 0.03, (0, -0.006, 0.005), sym=False, bones=head)
    else:
        # A small fine chin and a shorter, softer lower face.
        body.grab(chin + np.array([0, 0.012, 0]), (0.05, 0.035, 0.05), (0, 0.004, 0.0),
                  sym=False, bones=head)
        body.grab(chin, 0.022, (0, 0.0, 0.004), sym=False, bones=head)
        body.scale(chin + np.array([0, 0.03, 0.0]), (0.03, 0.015, 0.03), (0.9, 1.0, 1.0),
                   sym=False, bones=head)
    # High sharp cheekbones over hollow cheeks.
    cheek = eye + np.array([0.024, -0.025, -0.018])
    body.inflate(cheek, (0.03, 0.013, 0.03), 0.007 if male else 0.005, bones=head)
    body.grab(cheek, (0.03, 0.016, 0.03), (0.002, 0.002, 0.0), bones=head)
    body.inflate(eye + np.array([0.017, -0.056, -0.012]), (0.02, 0.018, 0.025),
                 -0.006 if male else -0.003, bones=head)
    # Long thin straight nose.
    body.scale(tip + np.array([0, 0.02, -0.02]), (0.03, 0.045, 0.04),
               (0.72 if male else 0.68, 1.0, 1.0), sym=False, bones=head)
    body.grab(tip, 0.02, (0, -0.004, 0.004), sym=False, bones=head)
    body.grab(np.array([0, eye[1] - 0.004, tip[2] - 0.03]), 0.016, (0, 0, 0.004),
              sym=False, bones=head)
    body.scale(np.array([0, eye[1] - 0.006, tip[2] - 0.03]), (0.02, 0.03, 0.03),
               (0.8, 1.0, 1.0), sym=False, bones=head)
    if not male:
        # Larger eyes, a smaller mouth.
        body.scale(eye + np.array([0.002, 0, -0.004]), (0.022, 0.018, 0.02), (1.08, 1.06, 1.0),
                   bones=head)
        body.scale(np.array([0, tip[1] - 0.035, tip[2] - 0.02]), (0.04, 0.015, 0.04),
                   (0.88, 1.0, 1.0), sym=False, bones=head)
    # Almond eyes slanting up at the outer corners, brows sweeping up.
    body.grab(eye + np.array([0.017, 0.0, -0.009]), 0.014, (0.001, 0.009, -0.001), bones=head)
    body.grab(eye + np.array([-0.014, 0.0, 0.003]), 0.009, (0, -0.0015, 0), bones=head)
    bm = _brow_mask(body, eye)
    brow = eye + np.array([0.0, 0.017 if male else 0.022, 0.006])
    _masked(body, "Brows", bm, brow, (0.05, 0.03, 0.05), factors=(1.0, 0.7 if male else 0.6, 1.0))
    _masked(body, "Brows", bm, eye + np.array([0.028, 0.018, -0.01]), 0.022,
            delta=(0.002, 0.008, -0.001))
    body.smooth(cheek, 0.04, 0.3, 2)

    # Long swept-back ears, rooted inside the skull where the human ear was.
    p = body.parts["Body"]["positions"]
    near = (np.abs(p[:, 1] - ear[1]) < 0.012) & (np.abs(p[:, 2] - ear[2]) < 0.012)
    skull = p[near, 0].max()
    root = np.array([skull - 0.016, ear[1] - 0.004, ear[2]])
    reach = np.array([0.095, 0.075, -0.105]) * (1.0 if male else 0.95)
    for side, nm in ((1, "Extra_EarL"), (-1, "Extra_EarR")):
        _ear(body, nm, root, root + reach, upper=0.016, lower=0.034 if male else 0.031,
             thick=0.009, cup=0.006, face=(0.95, 0.1, 0.3), side=side)

    # Slim, sloping trapezius: a long neck, not a bull neck.
    body.grab(body.at("neck_01", (0.075, -0.005, -0.02)), (0.06, 0.045, 0.07), (0, -0.01, 0),
              bones=["spine_03", "clavicle_l", "clavicle_r", "neck_01"])
    body.smooth(body.at("neck_01", (0.075, -0.005, -0.02)), 0.07, 0.4, 3)

    # The side buns sat right where the long ears go: lift them up onto the
    # crown, clear of the ears.
    d = body.parts["Hair_Buns"]
    ids = _pieces(d)
    for r in np.unique(ids):
        sel = ids == r
        c = d["positions"][sel].mean(0)
        if abs(c[0]) > 0.07:
            d["positions"][sel] += np.array([-0.026 * np.sign(c[0]), 0.066, 0.004])

    # ---- Posture ----
    # Upright and proud: the chest lifted, the head carried back over it.
    body.grab(body.at("spine_03", (0, 0.05, 0.12)), 0.16, (0, 0.008, 0.014), sym=False)
    body.grab(H + np.array([0, 0.09, 0]), (0.3, 0.19, 0.3), (0, 0.002, -0.018), sym=False,
              bones=["Head", "neck_01"])

    body.paint(_eyes)
    if male:
        # A lean dancer's torso, not a bodybuilder's: soften the painted
        # six-pack and pecs.
        lo, hi = body.joint("spine_01")[1] + 0.02, body.joint("spine_03")[1] + 0.12
        _soften(body, lambda p: (p[:, 2] > 0.0) & (np.abs(p[:, 0]) < 0.14)
                & (p[:, 1] > lo) & (p[:, 1] < hi), amount=0.65, radius=7)
        _soften(body, lambda p: (p[:, 2] < -0.02) & (np.abs(p[:, 0]) < 0.17)
                & (p[:, 1] > lo) & (p[:, 1] < hi + 0.03), amount=0.55, radius=7)
