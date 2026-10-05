"""Race bodies: one body per race and sex, sculpted from the Quaternius base
bodies.

Each race's look lives in `races/<race>.py` as `build(body)`, which reshapes
a `Body` (a base body, its eyes, brows and hairstyles) with the tools here:

- `proportions`: lengthen, thicken or resize bones, moving the skeleton and
  the mesh together (short legs, broad shoulders, a big head).
- brushes (`grab`, `scale`, `inflate`, `smooth`): sculpt the mesh like
  Blender's sculpt brushes, with a soft falloff and mirrored to both sides.
- `loft` and `tube`: model new parts (tusks, horns, beards, braids) skinned
  to the skeleton, coloured from a swatch, the skin or the hair texture.
- `paint`: draw on the body's texture (scars, rot, tattoos).

Coordinates are the body's model space in metres: +X is the character's
left, +Y up, +Z forward (the way the face looks). `body.at("Head", (x, y, z))`
is a point offset from a joint's rest position.

`make(ubc, race, sex)` builds a body; `write(body, path)` saves it as a
game file (thinned to a game-sized mesh with smooth normals);
`preview(bodies, path)` renders a contact sheet with Blender so a sculpt can
be checked by eye. Run a race on its own with

    python sculpt.py <Universal Base Characters dir> <race> [out.png]

which writes previews of both sexes.
"""

import importlib
import os
import sys

import numpy as np
from PIL import Image, ImageDraw

import pack

HERE = os.path.dirname(os.path.abspath(__file__))
RACES = ["human", "orc", "elf", "goblin", "gnome", "dwarf", "undead"]
SEXES = ["Male", "Female"]
# In place of the Quaternius folder: start from the game's human body.
GAME = "game"

# Flat colours for modeled parts, in the spare corner of the texture.
SWATCHES = {
    "ivory": (226, 214, 180), "bone": (206, 198, 170), "horn": (72, 60, 50),
    "nail": (200, 170, 150), "dark": (36, 30, 28), "gold": (214, 170, 64),
    "iron": (118, 120, 126), "bronze": (160, 110, 60), "leather": (110, 72, 44),
    "red": (150, 30, 28), "white": (236, 234, 228), "black": (18, 18, 20),
    "teeth": (232, 226, 206), "gum": (150, 70, 70), "eye_glow": (180, 230, 255),
    "green_glow": (150, 255, 140),
}
ATLAS = 1024
GAME_ATLAS = 512
BODY_RECT = (0, 0, 768, 768)
HAIR_RECTS = {"MI_Hair_1": (768, 0, 256, 256), "MI_Hair_2": (768, 256, 256, 256)}
EYES_RECT = (768, 512, 256, 256)
SWATCH_RECT = (768, 768, 256, 256)

# How much of each mesh the game keeps (the face keeps more; see write()).
THIN = {"Body": 0.55, "Hair_Long": 0.5, "Hair_Buns": 0.5, "Brows": 0.6}


def falloff(d):
    """Smooth 1 at the centre to 0 at distance 1."""
    t = np.clip(1.0 - d, 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


GRID = 8


def cell_uv(k):
    """Texture coordinate (0..1) of swatch cell `k`'s centre."""
    x0, y0, w, _ = SWATCH_RECT
    cell = w // GRID
    return ((x0 + (k % GRID) * cell + cell / 2) / ATLAS,
            (y0 + (k // GRID) * cell + cell / 2) / ATLAS)


def swatch_uv(name):
    return cell_uv(list(SWATCHES).index(name))


class Body:
    """A base body being sculpted: its skeleton and every mesh part, all in
    model space at the bind pose."""

    def __init__(self, ubc, sex):
        self.sex = sex
        self.race = None
        self.prethinned = ubc == GAME
        if self.prethinned:
            self._from_game()
            return
        folder = os.path.join(ubc, "Base Characters", "Godot - UE")
        textures = os.path.join(ubc, "Base Characters", "Textures")
        hair_dir = os.path.join(ubc, "Hairstyles", "Rigged to Head Bone", "glTF (Godot -Unreal)")
        doc = pack.load(os.path.join(folder, f"Superhero_{sex}_FullBody.gltf"))
        self.skel = pack.Skeleton(doc)
        self.names = self.skel.names
        self.index = self.skel.index
        self.parent = self.skel.parent
        self.local = [m.copy() for m in self.skel.local]
        self.world = [m.copy() for m in self.skel.world]

        # The texture: skin (left 3/4), hair, eyes, and the swatches.
        atlas = Image.new("RGB", (ATLAS, ATLAS), (128, 128, 128))
        body_file = {"Male": "T_Superhero_Male_Ligh.png",
                     "Female": "T_Superhero_Female_Light_BaseColor.png"}[sex]
        sources = {"body": (body_file, BODY_RECT),
                   "MI_Hair_1": ("T_Hair_1_BaseColor.png", HAIR_RECTS["MI_Hair_1"]),
                   "MI_Hair_2": ("T_Hair_2_BaseColor.png", HAIR_RECTS["MI_Hair_2"]),
                   "MI_Eyes": ("T_Eye_Brown.png", EYES_RECT)}
        for file, (x, y, w, h) in sources.values():
            img = Image.open(os.path.join(textures, file)).convert("RGB")
            atlas.paste(img.resize((w, h), Image.LANCZOS), (x, y))
        self.texture = atlas
        self.cells = 0
        for color in SWATCHES.values():
            self.swatch(color)
        rect = lambda r: tuple(v / ATLAS for v in r)

        # Parts: name -> mesh data (positions, normals, uvs in the atlas,
        # indices, joints, weights). "Body", "Eyes", "Brows", the hairstyles,
        # then whatever a race models.
        self.parts = {}
        for m in range(len(doc["meshes"])):
            data = pack.to_skeleton(pack.mesh_data(doc, m), doc, self.skel)
            mat = data["material"]
            if mat.startswith("MI_Superhero"):
                name, r = "Body", BODY_RECT
            elif mat == "MI_Eyes":
                name, r = "Eyes", EYES_RECT
            else:
                name, r = "Brows", HAIR_RECTS[mat]
            self.parts[name] = pack.place_uvs(data, rect(r))
        for hair in pack.HAIRS:
            hdoc = pack.load(os.path.join(hair_dir, hair + ".gltf"))
            data = pack.to_skeleton(pack.mesh_data(hdoc, 0), hdoc, self.skel, pack.Skeleton(hdoc))
            self.parts[hair] = pack.place_uvs(data, rect(HAIR_RECTS[data["material"]]))
        self.base = list(self.parts)
        self._axes()

    def _from_game(self):
        """Starts from the game's own human body (`human_<sex>.glb`) instead
        of the Quaternius files: the same skeleton, meshes and texture,
        already thinned to game size."""
        doc = pack.load(os.path.join(HERE, f"human_{self.sex.lower()}.glb"))
        self.skel = pack.Skeleton(doc)
        self.names = self.skel.names
        self.index = self.skel.index
        self.parent = self.skel.parent
        self.local = [m.copy() for m in self.skel.local]
        self.world = [m.copy() for m in self.skel.world]
        self.texture = pack.image_of(doc, 0).resize((ATLAS, ATLAS), Image.LANCZOS)
        self.cells = 0
        for color in SWATCHES.values():
            self.swatch(color)
        self.parts = {}
        for m, mesh in enumerate(doc["meshes"]):
            if mesh["name"].startswith("Extra_"):
                continue
            data = pack.to_skeleton(pack.mesh_data(doc, m), doc, self.skel)
            self.parts[mesh["name"]] = data
        self.base = list(self.parts)
        self._axes()

    def swatch(self, color):
        """Paints a new flat colour into a free swatch cell and returns its
        texture coordinate, for `add(..., uvs=...)` or `color=` (a name in
        SWATCHES, or the returned (u, v))."""
        k = self.cells
        assert k < GRID * GRID, "out of swatch cells"
        self.cells += 1
        x0, y0, w, _ = SWATCH_RECT
        cell = w // GRID
        x, y = x0 + (k % GRID) * cell, y0 + (k // GRID) * cell
        ImageDraw.Draw(self.texture).rectangle([x, y, x + cell - 1, y + cell - 1],
                                               fill=tuple(int(c) for c in color))
        return cell_uv(k)

    def ground(self):
        """Stands the body on the ground: moves the hips (and everything)
        up or down so the soles are at height 0."""
        low = self.parts["Body"]["positions"][:, 1].min()
        if abs(low) < 1e-5:
            return
        k = self.index["pelvis"]
        self.local[k] = self.local[k].copy()
        self.local[k][1, 3] -= low
        for j in range(len(self.names)):
            if j != self.index["root"]:
                self.world[j] = self.world[j].copy()
                self.world[j][1, 3] -= low
        for d in self.parts.values():
            d["positions"] = d["positions"] - np.array([0.0, low, 0.0])

    # ---- Where things are ----

    def joint(self, name):
        """A joint's rest position now."""
        return self.world[self.index[name]][:3, 3].copy()

    def at(self, name, offset=(0.0, 0.0, 0.0)):
        """A point `offset` metres from a joint's rest position."""
        return self.joint(name) + np.asarray(offset, float)

    def weight_of(self, part, bones):
        """Per vertex of `part`, its total skin weight on `bones` (a name
        or list of names; a name ending in `*` matches by prefix)."""
        if isinstance(bones, str):
            bones = [bones]
        want = set()
        for b in bones:
            if b.endswith("*"):
                want |= {k for k, n in enumerate(self.names) if n.startswith(b[:-1])}
            else:
                want.add(self.index[b])
        d = self.parts[part]
        hit = np.isin(d["joints"], list(want))
        return (d["weights"] * hit).sum(1)

    def height(self):
        return self.parts["Body"]["positions"][:, 1].max()

    def _axes(self):
        """Each bone's direction: toward its farthest child, or its parent's
        direction for bones without children."""
        n = len(self.names)
        self.axis = [None] * n
        kids = [[c for c in range(n) if self.parent[c] == k] for k in range(n)]
        for k in range(n):
            best = None
            for c in kids[k]:
                off = self.world[c][:3, 3] - self.world[k][:3, 3]
                if best is None or np.linalg.norm(off) > np.linalg.norm(best):
                    best = off
            if best is not None and np.linalg.norm(best) > 1e-6:
                self.axis[k] = best / np.linalg.norm(best)
        for k in range(n):
            if self.axis[k] is None:
                p = self.parent[k]
                self.axis[k] = self.axis[p] if p is not None else np.array([0.0, 1.0, 0.0])

    # ---- Proportions ----

    def proportions(self, length=None, girth=None, size=None):
        """Reshapes bones and everything skinned to them.

        `length`: bone -> factor along the bone (it reaches farther, and its
        children move out with it). `girth`: bone -> factor across the bone
        (thicker limbs, a wider chest; children off to the side, like the
        shoulders off the chest, move out too). `size`: bone -> uniform
        factor about the joint (a bigger head, smaller hands). A name ending
        in `_*` applies to both sides (`"thigh_*"`)."""
        def expand(table):
            out = {}
            for name, v in (table or {}).items():
                if name.endswith("_*"):
                    for side in ("_l", "_r"):
                        out[self.index[name[:-2] + side]] = v
                elif name.endswith("*"):
                    for k, n in enumerate(self.names):
                        if n.startswith(name[:-1]):
                            out[k] = v
                else:
                    out[self.index[name]] = v
            return out
        length, girth, size = expand(length), expand(girth), expand(size)
        n = len(self.names)
        A = []
        for k in range(n):
            a = self.axis[k][:, None]
            along = a @ a.T
            m = length.get(k, 1.0) * along + girth.get(k, 1.0) * (np.eye(3) - along)
            A.append(size.get(k, 1.0) * m)
        old = [w.copy() for w in self.world]
        new = [None] * n
        for k in range(n):
            p = self.parent[k]
            if p is None:
                new[k] = old[k].copy()
                continue
            offset = old[k][:3, 3] - old[p][:3, 3]
            new[k] = old[k].copy()
            new[k][:3, 3] = new[p][:3, 3] + A[p] @ offset
        # Locals from the new worlds (rotations are unchanged).
        for k in range(n):
            p = self.parent[k]
            self.local[k] = new[k] if p is None else np.linalg.inv(new[p]) @ new[k]
        self.world = new
        moves = [(old[k][:3, 3], new[k][:3, 3], A[k]) for k in range(n)]
        for d in self.parts.values():
            self._carry(d, moves)
        self._axes()

    def _carry(self, d, moves):
        o = np.stack([m[0] for m in moves])
        t = np.stack([m[1] for m in moves])
        A = np.stack([m[2] for m in moves])
        J, W = d["joints"], d["weights"]
        p = d["positions"]
        out = np.zeros_like(p)
        for slot in range(J.shape[1]):
            j = J[:, slot]
            rel = p - o[j]
            out += W[:, slot, None] * (t[j] + np.einsum("vij,vj->vi", A[j], rel))
        wsum = W.sum(1, keepdims=True)
        d["positions"] = np.where(wsum > 1e-6, out / np.maximum(wsum, 1e-6), p)

    # ---- Brushes ----

    def _parts(self, parts):
        if parts is None:
            return list(self.parts)
        return [parts] if isinstance(parts, str) else list(parts)

    def _strokes(self, center, sym):
        c = np.asarray(center, float)
        yield c, 1.0
        if sym and abs(c[0]) > 1e-4:
            yield c * np.array([-1.0, 1.0, 1.0]), -1.0

    def _weight(self, part, p, center, radius, bones):
        r = np.broadcast_to(np.asarray(radius, float), (3,))
        f = falloff(np.linalg.norm((p - center) / r, axis=1))
        if bones is not None:
            f = f * self.weight_of(part, bones)
        return f

    def grab(self, center, radius, delta, sym=True, parts=None, bones=None):
        """Pulls everything near `center` by `delta` (the centre moves the
        whole way, the edge of `radius` not at all). `radius` may be a
        number or (x, y, z) for an oval brush. With `sym`, the same stroke
        is mirrored onto the other side. `bones` limits it to vertices
        skinned to those bones (weighted)."""
        delta = np.asarray(delta, float)
        for c, side in self._strokes(center, sym):
            dd = delta * np.array([side, 1.0, 1.0])
            for name in self._parts(parts):
                d = self.parts[name]
                f = self._weight(name, d["positions"], c, radius, bones)
                d["positions"] = d["positions"] + f[:, None] * dd

    def scale(self, center, radius, factors, sym=True, parts=None, bones=None):
        """Stretches or squeezes everything near `center` about it, by
        `factors` (a number or (x, y, z)) at the centre fading to none at
        the edge: widen a jaw, flatten a nose, narrow a waist."""
        k = np.broadcast_to(np.asarray(factors, float), (3,))
        for c, _ in self._strokes(center, sym):
            for name in self._parts(parts):
                d = self.parts[name]
                p = d["positions"]
                f = self._weight(name, p, c, radius, bones)
                d["positions"] = p + f[:, None] * (p - c) * (k - 1.0)

    def inflate(self, center, radius, amount, sym=True, parts="Body", bones=None):
        """Pushes the surface out along its normals by `amount` metres at
        the centre (negative sinks it: hollow cheeks, sunken eyes)."""
        for c, _ in self._strokes(center, sym):
            for name in self._parts(parts):
                d = self.parts[name]
                n = self.normals(name)
                f = self._weight(name, d["positions"], c, radius, bones)
                d["positions"] = d["positions"] + f[:, None] * n * amount

    def smooth(self, center, radius, strength=0.5, iterations=3, sym=True, parts="Body"):
        """Relaxes the surface near `center` (evens out bumps left by other
        strokes)."""
        for name in self._parts(parts):
            d = self.parts[name]
            weld, nbrs = self._graph(name)
            for c, _ in self._strokes(center, sym):
                for _ in range(iterations):
                    p = d["positions"]
                    f = self._weight(name, p, c, radius, None) * strength
                    q = self._welded(p, weld)
                    avg = np.zeros_like(q)
                    np.add.at(avg, nbrs[:, 0], q[nbrs[:, 1]])
                    cnt = np.bincount(nbrs[:, 0], minlength=len(q))[:, None]
                    avg = np.where(cnt > 0, avg / np.maximum(cnt, 1), q)
                    d["positions"] = p + f[:, None] * (avg[weld] - p)

    def _graph(self, name):
        d = self.parts[name]
        _, weld = np.unique(np.round(d["positions"], 5), axis=0, return_inverse=True)
        weld = weld.reshape(-1)
        tris = weld[d["indices"].reshape(-1, 3)]
        e = np.concatenate([tris[:, [0, 1]], tris[:, [1, 2]], tris[:, [2, 0]]])
        e = np.concatenate([e, e[:, ::-1]])
        return weld, np.unique(e, axis=0)

    def _welded(self, p, weld):
        q = np.zeros((weld.max() + 1, 3))
        np.add.at(q, weld, p)
        return q / np.bincount(weld)[:, None]

    def normals(self, name):
        """Smooth vertex normals of a part as it is now."""
        d = self.parts[name]
        p = d["positions"]
        _, weld = np.unique(np.round(p, 5), axis=0, return_inverse=True)
        weld = weld.reshape(-1)
        t = d["indices"].reshape(-1, 3)
        fn = np.cross(p[t[:, 1]] - p[t[:, 0]], p[t[:, 2]] - p[t[:, 0]])
        n = np.zeros((weld.max() + 1, 3))
        for c in range(3):
            np.add.at(n, weld[t[:, c]], fn)
        n = n[weld]
        return n / np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-9)

    # ---- New parts ----

    def add(self, name, positions, indices, uvs=None, color=None, bone=None, follow=None):
        """Adds a modeled part. Its colour is `uvs` (per vertex, 0..1 in
        the atlas), or `color`: a swatch name, "skin" (takes the body's
        skin and its tint in game) or "hair" (takes the hair colour). It
        moves with `bone`, or with whatever body vertices are nearest when
        `follow` is a part name such as "Body"."""
        p = np.asarray(positions, float)
        idx = np.asarray(indices, np.int64).reshape(-1)
        if uvs is None:
            uvs = np.repeat([self._color_uv(color or "ivory", p)], len(p), axis=0) \
                if color not in ("skin", "hair") else self._color_uv(color, p)
        uvs = np.asarray(uvs, float).reshape(len(p), 2)
        if follow is not None:
            src = self.parts[follow]
            j, w = [], []
            for q in p:
                k = np.argmin(((src["positions"] - q) ** 2).sum(1))
                j.append(src["joints"][k])
                w.append(src["weights"][k])
            joints, weights = np.array(j), np.array(w)
        else:
            joints = np.zeros((len(p), 4), np.int64)
            joints[:, 0] = self.index[bone or "Head"]
            weights = np.zeros((len(p), 4))
            weights[:, 0] = 1.0
        d = {"positions": p, "normals": np.zeros_like(p), "uvs": uvs, "indices": idx,
             "joints": joints, "weights": weights, "material": name}
        self.parts[name] = d
        d["normals"] = self.normals(name)
        return d

    def _color_uv(self, color, p):
        if color == "skin":
            # A plain stretch of skin: the texel under the nearest body
            # vertex, so the part takes the skin tint.
            src = self.parts["Body"]
            k = [np.argmin(((src["positions"] - q) ** 2).sum(1)) for q in p]
            return src["uvs"][k]
        if color == "hair":
            # Texels the hairstyles use, so the part takes the hair colour.
            src = self.parts["Hair_Long"]["uvs"]
            return src[np.arange(len(p)) * 7 % len(src)]
        if isinstance(color, str):
            return np.array(swatch_uv(color))
        return np.asarray(color, float)

    def loft(self, name, rings, closed=True, cap=(True, True), **kw):
        """Models a part by joining rings of points (each ring the same
        number of points, in order around): a tusk is rings shrinking to a
        point, a beard rings under the jaw growing down. `rings` is a
        (rings, points, 3) array. Other arguments go to `add`."""
        rings = np.asarray(rings, float)
        nr, npt, _ = rings.shape
        pos = rings.reshape(-1, 3)
        centres = rings.mean(1)
        span = npt if closed else npt - 1
        sides = []
        for r in range(nr - 1):
            for i in range(span):
                a, b = r * npt + i, r * npt + (i + 1) % npt
                c, e = a + npt, b + npt
                sides += [(a, c, b), (b, c, e)]
        sides = np.array(sides, np.int64).reshape(-1, 3)
        # Face outward: away from the middle of each ring.
        fn = np.cross(pos[sides[:, 1]] - pos[sides[:, 0]], pos[sides[:, 2]] - pos[sides[:, 0]])
        mid = (centres[sides[:, 0] // npt] + centres[sides[:, 1] // npt]) / 2
        if (np.einsum("ij,ij->i", fn, pos[sides].mean(1) - mid)).sum() < 0:
            sides = sides[:, ::-1]
        tris = [tuple(t) for t in sides]
        pos = pos.tolist()
        for end, step in ((0, 1), (nr - 1, -1)):
            if not (closed and cap[0 if end == 0 else 1]):
                continue
            centre = centres[end]
            if np.linalg.norm(rings[end] - centre, axis=1).max() < 1e-6:
                continue
            out = centres[end] - centres[min(max(end + step, 0), nr - 1)]
            pos.append(centre.tolist())
            ci = len(pos) - 1
            for i in range(npt):
                a, b = end * npt + i, end * npt + (i + 1) % npt
                n = np.cross(np.subtract(pos[a], pos[ci]), np.subtract(pos[b], pos[ci]))
                tris.append((ci, a, b) if np.dot(n, out) >= 0 else (ci, b, a))
        return self.add(name, pos, tris, **kw)

    def tube(self, name, path, radii, sides=10, flat=1.0, up=(0.0, 1.0, 0.0), **kw):
        """A tapering tube along `path` (points) with `radii` (one per
        point; end on 0 for a point): tusks, horns, braids, fingers of a
        claw. `flat` < 1 squashes it (a blade-like horn)."""
        path = np.asarray(path, float)
        radii = np.broadcast_to(np.asarray(radii, float), (len(path),))
        rings = []
        up = np.asarray(up, float)
        for i, c in enumerate(path):
            t = path[min(i + 1, len(path) - 1)] - path[max(i - 1, 0)]
            t /= max(np.linalg.norm(t), 1e-9)
            s = np.cross(t, up)
            if np.linalg.norm(s) < 1e-6:
                s = np.cross(t, [1.0, 0.0, 0.0])
            s /= np.linalg.norm(s)
            u = np.cross(s, t)
            ang = np.linspace(0, 2 * np.pi, sides, endpoint=False)
            rings.append([c + radii[i] * (np.cos(a) * s + flat * np.sin(a) * u) for a in ang])
        return self.loft(name, rings, **kw)

    # ---- Texture ----

    def uv_at(self, point):
        """Where on the texture (pixels) the body surface nearest `point`
        is painted, to draw scars, tattoos or rot in the right place."""
        d = self.parts["Body"]
        k = np.argmin(((d["positions"] - np.asarray(point, float)) ** 2).sum(1))
        u, v = d["uvs"][k]
        return u * ATLAS, v * ATLAS

    def paint(self, fn):
        """Calls `fn(image, draw, body)` to paint on the texture (a PIL
        image and its ImageDraw)."""
        fn(self.texture, ImageDraw.Draw(self.texture), self)


# ---- Building and saving ----

def load_race(race):
    """A race's module from races/ (a race without one yet is the plain
    base body)."""
    folder = os.path.join(HERE, "races")
    if not os.path.exists(os.path.join(folder, race + ".py")):
        return type(sys)("plain")
    if folder not in sys.path:
        sys.path.insert(0, folder)
    module = importlib.import_module(race)
    return importlib.reload(module)


def make(ubc, race, sex):
    body = Body(ubc, sex)
    body.race = race
    module = load_race(race)
    if hasattr(module, "build"):
        module.build(body)
    body.ground()
    return body


def finish(body):
    """The parts as the game gets them: thinned (the face keeps more of its
    triangles) with smooth normals."""
    out = {}
    for name, d in body.parts.items():
        d = {k: (v.copy() if isinstance(v, np.ndarray) else v) for k, v in d.items()}
        keep = None
        if name == "Body":
            keep = np.clip(body.weight_of("Body", ["Head", "neck_01"]) * 1.5, 0, 1)
            keep = np.maximum(keep, body.weight_of("Body", ["hand_*", "index*", "middle*",
                                                             "ring*", "pinky*", "thumb*"]) * 0.5)
        ratio = 1.0 if body.prethinned and name in body.base else THIN.get(name, 1.0)
        out[name] = pack.thin(d, ratio, keep)
    return out


def write(body, path):
    parts = finish(body)
    w = pack.Writer()
    bones = list(zip(body.names, body.parent, body.local))
    w.skeleton(bones)
    skin = w.skin(list(range(len(body.names))), [np.linalg.inv(m) for m in body.world])
    for name, d in parts.items():
        pack.write_part(w, name, d, skin)
    # The game paints textures at 512 square, so that's all it needs.
    w.texture(body.texture.resize((GAME_ATLAS, GAME_ATLAS), Image.LANCZOS))
    w.save(path)
    return parts


# ---- Previews ----

PREVIEW_HAIR = {"Male": "Hair_SimpleParted", "Female": "Hair_Long"}


def preview(bodies, path, hair=None, tint=None, tile=360):
    """Renders each body (a row each) from the front, side and back, and
    its head from the front, three-quarters and side, in Blender, and
    saves the sheet to `path`. `hair` picks one hairstyle (default a
    plain one per sex, "none" for bald); `tint` is the skin colour to
    show (default the texture's own)."""
    import bpy

    rows = []
    for body in bodies:
        parts = finish(body)
        style = hair or PREVIEW_HAIR[body.sex]
        shown = {k: v for k, v in parts.items()
                 if k not in pack.HAIRS or k == style or (k == "Hair_Beard" and style == "beard")}
        tex = body.texture.copy()
        if tint is not None:
            tex = tinted(tex, body, tint)
        rows.append(render_row(bpy, shown, tex, body, tile))
    sheet = Image.new("RGB", (tile * 6, tile * len(rows)), (40, 40, 44))
    for r, row in enumerate(rows):
        for c, img in enumerate(row):
            sheet.paste(img, (c * tile, r * tile))
    sheet.save(path)
    return path


def tinted(tex, body, tint):
    """Recolours the skin part of the texture toward `tint`, keeping its
    shading (a rough stand-in for the game's skin tint)."""
    a = np.asarray(tex).astype(np.float32)
    x, y, w, h = BODY_RECT
    skin = a[y:y + h, x:x + w]
    lum = skin.mean(2, keepdims=True)
    mean = np.median(lum)
    under = lum < mean * 0.45  # underwear is dark: leave it
    out = np.clip(np.asarray(tint, np.float32) * (lum / max(mean, 1)), 0, 255)
    a[y:y + h, x:x + w] = np.where(under, skin, out)
    for hx, hy, hw, hh in HAIR_RECTS.values():
        hair = a[hy:hy + hh, hx:hx + hw]
        a[hy:hy + hh, hx:hx + hw] = hair * (np.array([0.55, 0.38, 0.24]) * 1.2)
    return Image.fromarray(np.clip(a, 0, 255).astype(np.uint8))


def render_row(bpy, parts, tex, body, tile):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    sc = bpy.context.scene
    img_path = f"/tmp/_sculpt_tex_{os.getpid()}.png"
    tex.save(img_path)
    image = bpy.data.images.load(img_path)
    mat = bpy.data.materials.new("m")
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    bsdf = nodes.get("Principled BSDF")
    t = nodes.new("ShaderNodeTexImage")
    t.image = image
    mat.node_tree.links.new(t.outputs["Color"], bsdf.inputs["Base Color"])
    bsdf.inputs["Roughness"].default_value = 0.6
    top = 0.0
    for name, d in parts.items():
        mesh = bpy.data.meshes.new(name)
        tris = d["indices"].reshape(-1, 3)
        # glTF's +Z forward, +Y up -> Blender's -Y forward, +Z up.
        p = d["positions"]
        bp = np.stack([p[:, 0], -p[:, 2], p[:, 1]], 1)
        mesh.from_pydata(bp.tolist(), [], tris.tolist())
        mesh.validate()
        uv = mesh.uv_layers.new(name="uv")
        cuv = d["uvs"][tris.reshape(-1)].copy()
        cuv[:, 1] = 1.0 - cuv[:, 1]
        uv.data.foreach_set("uv", cuv.astype(np.float32).reshape(-1))
        n = d["normals"]
        bn = np.stack([n[:, 0], -n[:, 2], n[:, 1]], 1)
        mesh.shade_smooth()
        mesh.normals_split_custom_set_from_vertices(bn.tolist())
        mesh.materials.append(mat)
        obj = bpy.data.objects.new(name, mesh)
        sc.collection.objects.link(obj)
        top = max(top, p[:, 1].max())
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.5
    sun.rotation_euler = (0.9, 0.0, -0.5)
    sc.collection.objects.link(sun)
    fill = bpy.data.objects.new("fill", bpy.data.lights.new("fill", "SUN"))
    fill.data.energy = 1.2
    fill.rotation_euler = (1.2, 0.0, 2.6)
    sc.collection.objects.link(fill)
    sc.world = bpy.data.worlds.new("w")
    sc.world.color = (0.35, 0.35, 0.38)
    sc.render.engine = "CYCLES"
    sc.cycles.samples = 12
    sc.cycles.device = "CPU"
    sc.render.resolution_x = tile
    sc.render.resolution_y = tile
    sc.render.film_transparent = False
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    sc.collection.objects.link(cam)
    sc.camera = cam
    head = body.joint("Head")
    hz = np.array([head[0], -head[2], head[1] + 0.08])
    views = []
    # Full body: orthographic, 2 m tall frame so races compare by size.
    for yaw in (0.0, 90.0, 180.0):
        views.append(("ortho", yaw, np.array([0.0, 0.0, 0.95]), 2.05))
    for yaw in (0.0, 35.0, 90.0):
        views.append(("ortho", yaw, hz, 0.42))
    out = []
    for kind, yaw, target, size in views:
        cam.data.type = "ORTHO"
        cam.data.ortho_scale = size
        a = np.radians(yaw)
        d = 4.0
        # Camera in front (-Y in Blender) turning toward the character's
        # right side.
        pos = target + np.array([d * np.sin(a), -d * np.cos(a), 0.0])
        cam.location = pos.tolist()
        cam.rotation_euler = (np.pi / 2, 0.0, a)
        view = f"/tmp/_sculpt_view_{os.getpid()}.png"
        sc.render.filepath = view
        bpy.ops.render.render(write_still=True)
        out.append(Image.open(view).convert("RGB").copy())
    return out


def main():
    ubc, race = sys.argv[1], sys.argv[2]
    path = sys.argv[3] if len(sys.argv) > 3 else f"/tmp/{race}.png"
    module = load_race(race)
    bodies = [make(ubc, race, sex) for sex in SEXES]
    preview(bodies, path, tint=getattr(module, "PREVIEW_SKIN", None))
    print(path)


if __name__ == "__main__":
    main()
