#!/usr/bin/env python3
"""Packs the character art into the small files the game embeds.

Sources (all CC0, see README.md):

- Quaternius Universal Base Characters (Standard): the male and female
  bodies, their eyes and brows, and the hairstyles.
- Quaternius Universal Animation Library 1 and 2 (Standard): animations on
  the same skeleton as the bodies.
- KayKit Character Pack: Adventurers and Skeletons: weapons, shields, hats,
  the skeleton mobs, and the animations the Quaternius libraries lack (two-
  handed swings, dual wielding, kicks, bows, long casts), moved onto the
  Quaternius skeleton ("retargeted").

Writes:

- rig.glb: the skeleton (male proportions) and every animation clip the
  game uses. Clips only turn bones (and move the hips), so they play on any
  body built on the skeleton whatever its proportions.
- <race>_<sex>.glb (human_male.glb, orc_female.glb, ...): a body sculpted
  for its race (see sculpt.py and races/) with its eyes, brows, every
  hairstyle and the race's own modeled parts, skinned to the skeleton, with
  one texture holding the skin, hair, eyes and flat colours.
- props.glb: the KayKit weapons and hats, each hanging off the skeleton
  bone that holds it (a hand or the head), resized to fit.
- skeletons.glb: the four KayKit skeleton bodies and their gear, reshaped
  onto the Quaternius skeleton.

Usage:
  pack.py <Universal Base Characters[Standard] dir> <UAL1_Standard.glb>
          <UAL2_Standard.glb> <KayKit Adventurers clone> <KayKit Skeletons clone>
  pack.py <Universal Base Characters[Standard] dir> --bodies   (bodies only)

Needs Python 3 with numpy, Pillow and Blender's `bpy` module (`pip install
bpy`), which thins out the dense meshes.
"""

import io
import json
import os
import struct
import sys

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))

# Animation clips kept from the Quaternius libraries.
UAL1_CLIPS = [
    "Idle_Loop", "Walk_Loop", "Jog_Fwd_Loop", "Sprint_Loop",
    "Jump_Start", "Jump_Loop", "Jump_Land", "Death01", "Hit_Chest", "Hit_Head",
    "Sword_Idle", "Sword_Attack", "Punch_Jab", "Punch_Cross",
    "Spell_Simple_Enter", "Spell_Simple_Idle_Loop", "Spell_Simple_Shoot",
    "Pistol_Idle_Loop", "Pistol_Aim_Neutral", "Pistol_Shoot",
    "Interact", "Idle_Talking_Loop",
]
UAL2_CLIPS = [
    "Sword_Regular_A", "Sword_Regular_B", "Sword_Regular_C", "Sword_Heavy_Combo",
    "Idle_Shield_Loop", "Shield_OneShot", "Sword_Block", "Melee_Hook",
    "OverhandThrow", "Hit_Knockback", "LayToIdle", "Consume",
    "Idle_FoldArms_Loop", "Zombie_Idle_Loop", "Zombie_Walk_Fwd_Loop", "Zombie_Scratch",
]
# KayKit clips moved onto the Quaternius skeleton.
KAYKIT_CLIPS = [
    "2H_Melee_Idle", "2H_Melee_Attack_Chop", "2H_Melee_Attack_Slice",
    "Dualwield_Melee_Attack_Stab", "Dualwield_Melee_Attack_Slice",
    "Unarmed_Idle", "Unarmed_Melee_Attack_Kick",
    "2H_Ranged_Aiming", "2H_Ranged_Shoot", "Spellcast_Long", "Cheer", "Blocking",
]

HAIRS = [
    "Hair_Buzzed", "Hair_BuzzedFemale", "Hair_SimpleParted", "Hair_Long",
    "Hair_Buns", "Hair_Beard",
]

# Which Quaternius bone each KayKit bone becomes.
BONES = {
    "hips": "pelvis", "spine": "spine_01", "chest": "spine_03", "head": "Head",
    "upperarm.l": "upperarm_l", "lowerarm.l": "lowerarm_l", "wrist.l": "hand_l",
    "hand.l": "hand_l", "handslot.l": "hand_l",
    "upperarm.r": "upperarm_r", "lowerarm.r": "lowerarm_r", "wrist.r": "hand_r",
    "hand.r": "hand_r", "handslot.r": "hand_r",
    "upperleg.l": "thigh_l", "lowerleg.l": "calf_l", "foot.l": "foot_l", "toes.l": "ball_l",
    "upperleg.r": "thigh_r", "lowerleg.r": "calf_r", "foot.r": "foot_r", "toes.r": "ball_r",
}
# Bones whose turns a retargeted clip copies (the rest keep their rest pose,
# and the fingers take a grip).
TURNED = [
    "hips", "spine", "chest", "head",
    "upperarm.l", "lowerarm.l", "hand.l", "upperarm.r", "lowerarm.r", "hand.r",
    "upperleg.l", "lowerleg.l", "foot.l", "toes.l",
    "upperleg.r", "lowerleg.r", "foot.r", "toes.r",
]

# KayKit props kept, with how much to shrink each from the chibi KayKit
# scale to the Quaternius one.
PROPS = {
    "Knight": {"1H_Sword": 0.45, "2H_Sword": 0.5, "Badge_Shield": 0.42,
               "Round_Shield": 0.42, "Knight_Helmet": 0.2},
    "Barbarian": {"1H_Axe": 0.45, "2H_Axe": 0.5, "Barbarian_Round_Shield": 0.42,
                  "Mug": 0.25, "Barbarian_Hat": 0.2},
    "Mage": {"1H_Wand": 0.45, "2H_Staff": 0.52, "Spellbook": 0.32,
             "Spellbook_open": 0.32, "Mage_Hat": 0.2},
    "Rogue": {"Knife": 0.45, "Knife_Offhand": 0.45, "1H_Crossbow": 0.45,
              "2H_Crossbow": 0.5, "Throwable": 0.35},
}
SKELETONS = ["Skeleton_Minion", "Skeleton_Warrior", "Skeleton_Rogue", "Skeleton_Mage"]
# Gear worn by the skeletons, kept with their bodies.
SKELETON_GEAR = {"Skeleton_Warrior_Helmet": 0.24, "Skeleton_Rogue_Hood": 0.24,
                 "Skeleton_Mage_Hat": 0.24}

FPS = 30


# ---- Reading glTF ----

def load(path):
    if path.endswith(".glb"):
        data = open(path, "rb").read()
        n = struct.unpack("<I", data[12:16])[0]
        doc = json.loads(data[20:20 + n])
        off = 20 + n
        bl = struct.unpack("<I", data[off:off + 4])[0]
        doc["_bin"] = data[off + 8:off + 8 + bl]
        doc["_dir"] = None
    else:
        doc = json.load(open(path))
        d = os.path.dirname(path)
        doc["_bin"] = open(os.path.join(d, doc["buffers"][0]["uri"]), "rb").read()
        doc["_dir"] = d
    return doc


TYPES = {5126: np.float32, 5123: np.uint16, 5125: np.uint32, 5121: np.uint8,
         5122: np.int16, 5120: np.int8}
COMPONENTS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


def read(doc, index):
    a = doc["accessors"][index]
    view = doc["bufferViews"][a["bufferView"]]
    dtype = np.dtype(TYPES[a["componentType"]])
    n = COMPONENTS[a["type"]]
    start = view.get("byteOffset", 0) + a.get("byteOffset", 0)
    stride = view.get("byteStride", 0) or dtype.itemsize * n
    raw = np.frombuffer(doc["_bin"], np.uint8, count=stride * (a["count"] - 1) + dtype.itemsize * n,
                        offset=start)
    rows = np.lib.stride_tricks.as_strided(raw, (a["count"], dtype.itemsize * n), (stride, 1))
    out = np.ascontiguousarray(rows).view(dtype).reshape(a["count"], n)
    if a.get("normalized"):
        out = out.astype(np.float32) / np.iinfo(dtype).max
    return out


def image_of(doc, index):
    im = doc["images"][index]
    if "uri" in im:
        return Image.open(os.path.join(doc["_dir"], im["uri"])).convert("RGB")
    view = doc["bufferViews"][im["bufferView"]]
    start = view.get("byteOffset", 0)
    return Image.open(io.BytesIO(doc["_bin"][start:start + view["byteLength"]])).convert("RGB")


# ---- Transforms ----

def quat_to_mat(q):
    x, y, z, w = q
    return np.array([
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ])


def mat_to_quat(m):
    t = np.trace(m)
    if t > 0:
        s = np.sqrt(t + 1) * 2
        q = [(m[2, 1] - m[1, 2]) / s, (m[0, 2] - m[2, 0]) / s, (m[1, 0] - m[0, 1]) / s, 0.25 * s]
    else:
        i = int(np.argmax(np.diag(m)))
        if i == 0:
            s = np.sqrt(1 + m[0, 0] - m[1, 1] - m[2, 2]) * 2
            q = [0.25 * s, (m[0, 1] + m[1, 0]) / s, (m[0, 2] + m[2, 0]) / s, (m[2, 1] - m[1, 2]) / s]
        elif i == 1:
            s = np.sqrt(1 + m[1, 1] - m[0, 0] - m[2, 2]) * 2
            q = [(m[0, 1] + m[1, 0]) / s, 0.25 * s, (m[1, 2] + m[2, 1]) / s, (m[0, 2] - m[2, 0]) / s]
        else:
            s = np.sqrt(1 + m[2, 2] - m[0, 0] - m[1, 1]) * 2
            q = [(m[0, 2] + m[2, 0]) / s, (m[1, 2] + m[2, 1]) / s, 0.25 * s, (m[1, 0] - m[0, 1]) / s]
    q = np.array(q)
    return q / np.linalg.norm(q)


def compose(t=None, r=None, s=None):
    m = np.eye(4)
    rot = quat_to_mat(r) if r is not None else np.eye(3)
    sc = np.diag(s) if s is not None else np.eye(3)
    m[:3, :3] = rot @ sc
    if t is not None:
        m[:3, 3] = t
    return m


def local_matrix(node):
    if "matrix" in node:
        return np.array(node["matrix"]).reshape(4, 4).T
    return compose(node.get("translation"), node.get("rotation"), node.get("scale"))


def parents(doc):
    out = [None] * len(doc["nodes"])
    for i, n in enumerate(doc["nodes"]):
        for c in n.get("children", []):
            out[c] = i
    return out


def worlds(doc, local=None):
    par = parents(doc)
    local = local if local is not None else [local_matrix(n) for n in doc["nodes"]]
    out = [None] * len(local)

    def at(i):
        if out[i] is None:
            out[i] = local[i] if par[i] is None else at(par[i]) @ local[i]
        return out[i]

    for i in range(len(local)):
        at(i)
    return out


def index(doc):
    return {n.get("name"): i for i, n in enumerate(doc["nodes"])}


def rotation_of(m):
    """The rotation in a matrix, with any scale taken out."""
    r = m[:3, :3]
    return r / np.linalg.norm(r, axis=0)


def slerp(a, b, t):
    d = np.dot(a, b)
    if d < 0:
        b, d = -b, -d
    if d > 0.9995:
        q = a + (b - a) * t
    else:
        th = np.arccos(d)
        q = (np.sin((1 - t) * th) * a + np.sin(t * th) * b) / np.sin(th)
    return q / np.linalg.norm(q)


def sample(times, values, t, rotation):
    if t <= times[0]:
        return values[0]
    if t >= times[-1]:
        return values[-1]
    k = int(np.searchsorted(times, t)) - 1
    f = (t - times[k]) / (times[k + 1] - times[k])
    if rotation:
        return slerp(values[k], values[k + 1], f)
    return values[k] + (values[k + 1] - values[k]) * f


# ---- Writing glTF ----

class Writer:
    def __init__(self):
        self.doc = {"asset": {"version": "2.0", "generator": "Rusty MMO pack.py"},
                    "nodes": [], "accessors": [], "bufferViews": []}
        self.blob = bytearray()

    def view(self, data):
        while len(self.blob) % 4:
            self.blob.append(0)
        self.doc["bufferViews"].append({"buffer": 0, "byteOffset": len(self.blob),
                                        "byteLength": len(data)})
        self.blob += data
        return len(self.doc["bufferViews"]) - 1

    def accessor(self, array, kind, component, minmax=False):
        array = np.ascontiguousarray(array)
        a = {"bufferView": self.view(array.tobytes()), "componentType": component,
             "count": int(array.shape[0]), "type": kind}
        if minmax:
            a["min"] = array.min(0).tolist()
            a["max"] = array.max(0).tolist()
        self.doc["accessors"].append(a)
        return len(self.doc["accessors"]) - 1

    def floats(self, array, kind, minmax=False):
        return self.accessor(np.asarray(array, np.float32), kind, 5126, minmax)

    def skeleton(self, bones):
        """Adds `bones` (name, parent index or None, 4x4 local) as the first
        nodes; returns their node indices."""
        for name, parent, local in bones:
            t = local[:3, 3]
            r = mat_to_quat(rotation_of(local))
            node = {"name": name, "translation": t.tolist(), "rotation": r.tolist()}
            self.doc["nodes"].append(node)
        for i, (_, parent, _) in enumerate(bones):
            if parent is not None:
                self.doc["nodes"][parent].setdefault("children", []).append(i)
        return list(range(len(bones)))

    def texture(self, picture):
        buf = io.BytesIO()
        picture.save(buf, "PNG", optimize=True)
        self.doc["images"] = [{"bufferView": self.view(buf.getvalue()), "mimeType": "image/png"}]
        self.doc["textures"] = [{"source": 0}]
        self.doc["materials"] = [{"pbrMetallicRoughness": {"baseColorTexture": {"index": 0},
                                                           "metallicFactor": 0.0}}]

    def mesh(self, name, parent, positions, normals, uvs, indices, joints=None, weights=None,
             skin=None):
        attributes = {
            "POSITION": self.floats(positions, "VEC3", True),
            "NORMAL": self.floats(normals, "VEC3"),
            "TEXCOORD_0": self.floats(uvs, "VEC2"),
        }
        if joints is not None:
            attributes["JOINTS_0"] = self.accessor(np.asarray(joints, np.uint8), "VEC4", 5121)
            attributes["WEIGHTS_0"] = self.floats(weights, "VEC4")
        prim = {"attributes": attributes,
                "indices": self.accessor(np.asarray(indices, np.uint16).reshape(-1, 1),
                                         "SCALAR", 5123),
                "material": 0}
        self.doc.setdefault("meshes", []).append({"name": name, "primitives": [prim]})
        node = {"name": name, "mesh": len(self.doc["meshes"]) - 1}
        if skin is not None:
            node["skin"] = skin
        self.doc["nodes"].append(node)
        i = len(self.doc["nodes"]) - 1
        if parent is not None:
            self.doc["nodes"][parent].setdefault("children", []).append(i)
        return i

    def skin(self, joints, inverse_binds):
        mats = np.stack([m.T.reshape(16) for m in inverse_binds])
        self.doc.setdefault("skins", []).append(
            {"joints": joints, "inverseBindMatrices": self.floats(mats, "MAT4")})
        return len(self.doc["skins"]) - 1

    def animation(self, name, tracks):
        """`tracks`: (node, path, times, values)."""
        channels, samplers = [], []
        for node, path, times, values in tracks:
            samplers.append({"input": self.floats(np.asarray(times).reshape(-1, 1), "SCALAR", True),
                             "output": self.floats(values, "VEC4" if path == "rotation" else "VEC3"),
                             "interpolation": "LINEAR"})
            channels.append({"sampler": len(samplers) - 1, "target": {"node": node, "path": path}})
        self.doc.setdefault("animations", []).append(
            {"name": name, "channels": channels, "samplers": samplers})

    def save(self, path):
        doc = dict(self.doc)
        roots = set(range(len(doc["nodes"]))) - {c for n in doc["nodes"] for c in n.get("children", [])}
        doc["scenes"] = [{"nodes": sorted(roots)}]
        doc["scene"] = 0
        while len(self.blob) % 4:
            self.blob.append(0)
        doc["buffers"] = [{"byteLength": len(self.blob)}]
        text = json.dumps(doc, separators=(",", ":")).encode()
        while len(text) % 4:
            text += b" "
        total = 12 + 8 + len(text) + 8 + len(self.blob)
        with open(path, "wb") as f:
            f.write(b"glTF" + struct.pack("<II", 2, total))
            f.write(struct.pack("<I", len(text)) + b"JSON" + text)
            f.write(struct.pack("<I", len(self.blob)) + b"BIN\0" + bytes(self.blob))
        print(f"{os.path.basename(path)}: {total // 1024} KB")


# ---- The skeleton ----

class Skeleton:
    """The Quaternius skeleton as a file has it: bone names in skin order,
    parents and rest transforms."""

    def __init__(self, doc):
        skin = doc["skins"][0]
        self.nodes = skin["joints"]
        self.names = [doc["nodes"][j]["name"] for j in self.nodes]
        par = parents(doc)
        where = {node: k for k, node in enumerate(self.nodes)}
        self.parent = [where.get(par[j]) for j in self.nodes]
        self.local = [local_matrix(doc["nodes"][j]) for j in self.nodes]
        self.world = []
        for k in range(len(self.nodes)):
            p = self.parent[k]
            self.world.append(self.local[k] if p is None else self.world[p] @ self.local[k])
        self.index = {n: k for k, n in enumerate(self.names)}

    def bones(self):
        return list(zip(self.names, self.parent, self.local))

    def at(self, name):
        return self.world[self.index[name]][:3, 3]


# ---- Animations ----

def quaternius_clips(writer, skel, doc, wanted):
    """Copies clips from a Quaternius library, keeping only bone turns (and
    the hips' movement), so they suit any body on the skeleton."""
    nodes = doc["nodes"]
    lib_names = index(doc)
    lib_pelvis = np.array(nodes[lib_names["pelvis"]]["translation"])
    our_pelvis = skel.local[skel.index["pelvis"]][:3, 3]
    ratio = our_pelvis[2] / lib_pelvis[2]
    clips = {a["name"]: a for a in doc["animations"]}
    for name in wanted:
        anim = clips[name]
        tracks = []
        for ch in anim["channels"]:
            bone = nodes[ch["target"]["node"]]["name"]
            path = ch["target"]["path"]
            if bone not in skel.index:
                continue
            if path == "translation" and bone != "pelvis":
                continue
            if path == "scale":
                continue
            s = anim["samplers"][ch["sampler"]]
            times = read(doc, s["input"])[:, 0]
            values = read(doc, s["output"]).astype(np.float64)
            if s.get("interpolation") == "CUBICSPLINE":
                values = values[1::3]
            if path == "translation":
                values = our_pelvis + (values - lib_pelvis) * ratio
            # Bones that hold still need just one key.
            if np.abs(values - values[0]).max() < 1e-4:
                times, values = times[:1], values[:1]
            tracks.append((skel.index[bone], path, times, values))
        writer.animation(name, tracks)


def kaykit_clip(doc, name):
    """Samples a KayKit clip: per frame, every node's local matrix."""
    anim = next(a for a in doc["animations"] if a["name"] == name)
    rest = [(n.get("translation", [0, 0, 0]), n.get("rotation", [0, 0, 0, 1]),
             n.get("scale", [1, 1, 1])) for n in doc["nodes"]]
    tracks = {}
    duration = 0.0
    for ch in anim["channels"]:
        s = anim["samplers"][ch["sampler"]]
        times = read(doc, s["input"])[:, 0]
        values = read(doc, s["output"]).astype(np.float64)
        tracks[(ch["target"]["node"], ch["target"]["path"])] = (times, values)
        duration = max(duration, times[-1])
    frames = []
    count = max(2, int(round(duration * FPS)) + 1)
    times = np.linspace(0, duration, count)
    for t in times:
        local = []
        for i, (rt, rr, rs) in enumerate(rest):
            parts = []
            for path, default in (("translation", rt), ("rotation", rr), ("scale", rs)):
                tr = tracks.get((i, path))
                parts.append(sample(tr[0], tr[1], t, path == "rotation") if tr else
                             np.array(default, dtype=np.float64))
            local.append(compose(*parts))
        frames.append(local)
    return times, frames


def retarget(writer, skel, kk, grip):
    """Moves KayKit clips onto the Quaternius skeleton: each copied bone
    turns, in the model's space, as its KayKit counterpart turns from the
    T-pose both rigs were built in. The hips move as far, scaled to the
    longer legs."""
    kk_names = index(kk)
    kk_rest = worlds(kk)
    kk_hips = kk_rest[kk_names["hips"]][:3, 3]
    ratio = skel.at("pelvis")[1] / kk_hips[1]
    turned = {BONES[b]: kk_names[b] for b in TURNED}
    for name in KAYKIT_CLIPS:
        times, frames = kaykit_clip(kk, name)
        rotations = [[] for _ in skel.names]
        pelvis_moves = []
        for local in frames:
            world = worlds(kk, local)
            out = [None] * len(skel.names)
            for k, bone in enumerate(skel.names):
                p = skel.parent[k]
                parent = np.eye(3) if p is None else out[p]
                if bone in turned:
                    j = turned[bone]
                    w = rotation_of(world[j]) @ rotation_of(kk_rest[j]).T @ rotation_of(skel.world[k])
                    r = parent.T @ w
                elif bone in grip:
                    r = quat_to_mat(grip[bone])
                else:
                    r = rotation_of(skel.local[k])
                out[k] = parent @ r
                rotations[k].append(mat_to_quat(r))
            moved = world[kk_names["hips"]][:3, 3] - kk_hips
            target = skel.at("pelvis") + moved * ratio
            root = skel.world[skel.parent[skel.index["pelvis"]]]
            pelvis_moves.append((np.linalg.inv(root) @ np.append(target, 1.0))[:3])
        tracks = []
        for k, rots in enumerate(rotations):
            rots = np.array(rots)
            # Keep neighbouring keys on the same side, for smooth blending.
            for f in range(1, len(rots)):
                if np.dot(rots[f], rots[f - 1]) < 0:
                    rots[f] = -rots[f]
            if np.abs(rots - rots[0]).max() < 1e-4:
                tracks.append((k, "rotation", times[:1], rots[:1]))
            else:
                tracks.append((k, "rotation", times, rots))
        tracks.append((skel.index["pelvis"], "translation", times, np.array(pelvis_moves)))
        writer.animation(name, tracks)


def grip_pose(skel, doc):
    """Finger turns from the first frame of a clip holding a sword."""
    anim = next(a for a in doc["animations"] if a["name"] == "Sword_Idle")
    nodes = doc["nodes"]
    out = {}
    for ch in anim["channels"]:
        bone = nodes[ch["target"]["node"]]["name"]
        if ch["target"]["path"] != "rotation" or not any(
                f in bone for f in ("index", "middle", "ring", "pinky", "thumb")):
            continue
        out[bone] = read(doc, anim["samplers"][ch["sampler"]]["output"])[0].astype(np.float64)
    return out


# ---- Bodies ----

def thin(data, ratio, keep=None):
    """Collapses a skinned mesh's edges until `ratio` of its triangles are
    left (with Blender), keeping texture seams and bone weights, and gives
    it smooth normals. `keep` (0..1 per vertex) protects detail: the face
    keeps more of its triangles than the rest. A ratio of 1 only smooths."""
    import bpy

    pos = data["positions"]
    # Weld the copies a texture seam splits a vertex into, so the mesh is
    # one surface; each corner keeps its own texture coordinate.
    key = np.round(pos, 5)
    _, weld, = np.unique(key, axis=0, return_inverse=True)[:2]
    weld = weld.reshape(-1)
    count = weld.max() + 1
    first = np.zeros(count, np.int64)
    first[weld[::-1]] = np.arange(len(weld))[::-1]
    mesh = bpy.data.meshes.new("thin")
    tris = data["indices"].reshape(-1, 3)
    # Drop triangles that welding squashed (two corners on one point).
    w = weld[tris]
    tris = tris[(w[:, 0] != w[:, 1]) & (w[:, 1] != w[:, 2]) & (w[:, 0] != w[:, 2])]
    mesh.from_pydata(pos[first].tolist(), [], weld[tris].tolist())
    uv = mesh.uv_layers.new(name="uv")
    corner_uv = data["uvs"][tris.reshape(-1)]
    uv.data.foreach_set("uv", corner_uv.astype(np.float32).reshape(-1))
    obj = bpy.data.objects.new("thin", mesh)
    bpy.context.scene.collection.objects.link(obj)
    groups = [obj.vertex_groups.new(name=str(j)) for j in range(int(data["joints"].max()) + 1)]
    for v in range(count):
        src = first[v]
        for j, w in zip(data["joints"][src], data["weights"][src]):
            if w > 0:
                groups[j].add([v], float(w), "ADD")
    if ratio < 1.0:
        mod = obj.modifiers.new("decimate", "DECIMATE")
        mod.ratio = ratio
        if keep is not None:
            group = obj.vertex_groups.new(name="keep")
            for v in range(count):
                group.add([v], float(1.0 - np.clip(keep[first[v]], 0.0, 1.0)), "REPLACE")
            mod.vertex_group = group.name
            mod.vertex_group_factor = 4.0
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.modifier_apply(modifier=mod.name)
    mesh = obj.data
    mesh.shade_smooth()
    mesh.calc_loop_triangles()
    nv = len(mesh.vertices)
    co = np.zeros(nv * 3, np.float32)
    mesh.vertices.foreach_get("co", co)
    co = co.reshape(-1, 3)
    loops = np.zeros(len(mesh.loops), np.int64)
    mesh.loops.foreach_get("vertex_index", loops)
    uvs = np.zeros(len(mesh.loops) * 2, np.float32)
    mesh.uv_layers[0].data.foreach_get("uv", uvs)
    uvs = uvs.reshape(-1, 2)
    normals = np.zeros(len(mesh.loops) * 3, np.float32)
    mesh.corner_normals.foreach_get("vector", normals)
    normals = normals.reshape(-1, 3)
    tri_loops = np.zeros(len(mesh.loop_triangles) * 3, np.int64)
    mesh.loop_triangles.foreach_get("loops", tri_loops)
    # Weights: the four strongest bones of each vertex.
    joints = np.zeros((nv, 4), np.int64)
    weights = np.zeros((nv, 4))
    for v in mesh.vertices:
        gs = sorted(((g.weight, g.group) for g in v.groups
                     if obj.vertex_groups[g.group].name != "keep"), reverse=True)[:4]
        for k, (w, g) in enumerate(gs):
            joints[v.index, k] = int(obj.vertex_groups[g].name)
            weights[v.index, k] = w
    weights /= np.maximum(weights.sum(1, keepdims=True), 1e-6)
    # Split vertices again where corners disagree on texture coordinates.
    corner_key = np.concatenate([loops[tri_loops][:, None], np.round(uvs[tri_loops] * 4096)], 1)
    unique, index_of = np.unique(corner_key, axis=0, return_inverse=True)
    index_of = index_of.reshape(-1)
    pick = np.zeros(len(unique), np.int64)
    pick[index_of] = tri_loops
    verts = loops[pick]
    out = {
        "positions": co[verts].astype(np.float64),
        "normals": normals[pick].astype(np.float64),
        "uvs": uvs[pick].astype(np.float64),
        "indices": index_of,
        "joints": joints[verts],
        "weights": weights[verts],
        "material": data["material"],
    }
    bpy.data.objects.remove(obj)
    bpy.data.meshes.remove(mesh)
    return out


def mesh_data(doc, mesh_index):
    p = doc["meshes"][mesh_index]["primitives"][0]
    a = p["attributes"]
    return {
        "positions": read(doc, a["POSITION"]).astype(np.float64),
        "normals": read(doc, a["NORMAL"]).astype(np.float64),
        "uvs": read(doc, a["TEXCOORD_0"]).astype(np.float64),
        "indices": read(doc, p["indices"])[:, 0].astype(np.int64),
        "joints": read(doc, a["JOINTS_0"]).astype(np.int64) if "JOINTS_0" in a else None,
        "weights": read(doc, a["WEIGHTS_0"]).astype(np.float64) if "WEIGHTS_0" in a else None,
        "material": doc["materials"][p["material"]].get("name", "") if "material" in p else None,
    }


def to_skeleton(data, doc, skel, fit=None):
    """Renumbers a skinned mesh's joints to `skel`'s. With `fit` (the
    mesh's own skeleton), moves the mesh from where that skeleton's bones
    were bound to where `skel`'s are."""
    names = [doc["nodes"][j]["name"] for j in doc["skins"][0]["joints"]]
    remap = np.array([skel.index[n] for n in names])
    data["joints"] = remap[data["joints"]]
    if fit is not None:
        carry = np.stack([skel.world[k] @ np.linalg.inv(fit.world[fit.index[skel.names[k]]])
                          for k in range(len(skel.names))])
        m = np.einsum("vk,vkij->vij", data["weights"], carry[data["joints"]])
        data["positions"] = np.einsum("vij,vj->vi", m[:, :3, :3], data["positions"]) + m[:, :3, 3]
        n = np.einsum("vij,vj->vi", m[:, :3, :3], data["normals"])
        data["normals"] = n / np.linalg.norm(n, axis=1, keepdims=True)
    return data


def place_uvs(data, rect):
    """Squeezes a mesh's texture coordinates into `rect` (x, y, w, h, in
    0..1) of the shared texture."""
    x, y, w, h = rect
    uv = np.clip(data["uvs"], 0.0, 1.0)
    data["uvs"] = np.stack([x + uv[:, 0] * w, y + uv[:, 1] * h], axis=1)
    return data


def write_part(writer, name, data, skin, parent=None):
    writer.mesh(name, parent, data["positions"], data["normals"], data["uvs"], data["indices"],
                data["joints"], data["weights"], skin)


# ---- KayKit gear and skeletons ----

def palm(skel, side):
    """Where a hand holds things, in the skeleton's rest pose."""
    hand = skel.at("hand_" + side)
    knuckle = skel.at("middle_01_" + side)
    return hand + (knuckle - hand) * 0.75 + np.array([0.0, -0.025, 0.0])


def kaykit_carry(kk, skel, thick, head):
    """For each KayKit node, a matrix carrying points from the KayKit
    T-pose to the Quaternius one around its bone: from the KayKit joint to
    the Quaternius joint, stretched along the bone to its new length and
    across it by `thick` (the head by `head`)."""
    names = index(kk)
    rest = worlds(kk)
    pos = lambda n: rest[names[n]][:3, 3]
    out = {}
    children = {"hips": "spine", "spine": "chest", "chest": "head",
                "upperarm.l": "lowerarm.l", "lowerarm.l": "wrist.l",
                "upperarm.r": "lowerarm.r", "lowerarm.r": "wrist.r",
                "upperleg.l": "lowerleg.l", "lowerleg.l": "foot.l",
                "upperleg.r": "lowerleg.r", "lowerleg.r": "foot.r"}
    for kname, qname in BONES.items():
        start, end = pos(kname), skel.at(qname)
        if kname in children:
            child = children[kname]
            length = np.linalg.norm(skel.at(BONES[child]) - end) / np.linalg.norm(pos(child) - start)
            axis = int(np.argmax(np.abs(pos(child) - start)))
            s = np.full(3, thick)
            s[axis] = length
        elif kname == "head":
            s = np.full(3, head)
        else:
            s = np.full(3, thick)
        m = np.eye(4)
        m[:3, :3] = np.diag(s)
        m[:3, 3] = end - s * start
        out[kname] = m
    return out


def kaykit_texture(doc, cell):
    img = image_of(doc, 0)
    return img.resize((cell, cell), Image.LANCZOS)


def rigid_part(kk, node, skel, bone, scale, anchor):
    """A KayKit prop, placed relative to the Quaternius bone that holds it:
    the KayKit attachment point moves to `anchor`, shrunk by `scale`."""
    rest = worlds(kk)
    par = parents(kk)
    data = mesh_data(kk, kk["nodes"][node]["mesh"])
    holder = rest[par[node]][:3, 3]
    m = np.eye(4)
    m[:3, :3] *= scale
    m[:3, 3] = anchor - scale * holder
    m = np.linalg.inv(skel.world[skel.index[bone]]) @ m @ rest[node]
    data["positions"] = data["positions"] @ m[:3, :3].T + m[:3, 3]
    n = data["normals"] @ np.linalg.inv(m[:3, :3])
    data["normals"] = n / np.linalg.norm(n, axis=1, keepdims=True)
    data["joints"] = data["weights"] = None
    return data


def anchor_for(skel, holder, part_scale):
    if holder == "handslot.r":
        return "hand_r", palm(skel, "r")
    if holder == "handslot.l":
        return "hand_l", palm(skel, "l")
    if holder == "head":
        return "Head", skel.at("Head")
    raise ValueError(holder)


def pack_props(adventurers, skel):
    w = Writer()
    w.skeleton(skel.bones())
    cell = 512
    atlas = Image.new("RGB", (1024, 1024))
    for k, (body, props) in enumerate(PROPS.items()):
        kk = load(os.path.join(adventurers, body + ".glb"))
        x, y = (k % 2) * cell, (k // 2) * cell
        atlas.paste(kaykit_texture(kk, cell), (x, y))
        names = index(kk)
        par = parents(kk)
        for prop, scale in props.items():
            node = names[prop]
            holder = kk["nodes"][par[node]]["name"]
            bone, anchor = anchor_for(skel, holder, scale)
            data = rigid_part(kk, node, skel, bone, scale, anchor)
            place_uvs(data, (x / 1024, y / 1024, cell / 1024, cell / 1024))
            write_part(w, prop, data, None, skel.index[bone])
    w.texture(atlas)
    w.save(os.path.join(HERE, "props.glb"))


def pack_skeletons(skeletons, skel):
    w = Writer()
    w.skeleton(skel.bones())
    skin = w.skin(list(range(len(skel.names))), [np.linalg.inv(m) for m in skel.world])
    texture = None
    for body in SKELETONS:
        kk = load(os.path.join(skeletons, body + ".glb"))
        texture = texture or kaykit_texture(kk, 512)
        carry = kaykit_carry(kk, skel, thick=0.48, head=0.27)
        joint_names = [kk["nodes"][j]["name"] for j in kk["skins"][0]["joints"]]
        par = parents(kk)
        for i, node in enumerate(kk["nodes"]):
            if "mesh" not in node:
                continue
            if "skin" in node:
                data = mesh_data(kk, node["mesh"])
                mats = np.stack([carry[n] if n in carry else carry[nearest(kk, n, carry)]
                                 for n in joint_names])
                m = np.einsum("vk,vkij->vij", data["weights"], mats[data["joints"]])
                data["positions"] = (np.einsum("vij,vj->vi", m[:, :3, :3], data["positions"])
                                     + m[:, :3, 3])
                nm = np.einsum("vij,vj->vi", np.linalg.inv(m[:, :3, :3]).transpose(0, 2, 1),
                               data["normals"])
                data["normals"] = nm / np.linalg.norm(nm, axis=1, keepdims=True)
                remap = np.array([skel.index[BONES.get(n, BONES[nearest(kk, n, carry)])]
                                  for n in joint_names])
                data["joints"] = remap[data["joints"]]
                write_part(w, node["name"], data, skin)
            elif node["name"] in SKELETON_GEAR:
                holder = kk["nodes"][par[i]]["name"]
                bone, anchor = anchor_for(skel, holder, 0)
                data = rigid_part(kk, i, skel, bone, SKELETON_GEAR[node["name"]], anchor)
                write_part(w, node["name"], data, None, skel.index[bone])
    w.texture(texture)
    w.save(os.path.join(HERE, "skeletons.glb"))


def nearest(kk, name, known):
    """The closest ancestor of a KayKit node that has a Quaternius bone."""
    names = index(kk)
    par = parents(kk)
    i = names[name]
    while i is not None and kk["nodes"][i]["name"] not in known:
        i = par[i]
    return kk["nodes"][i]["name"] if i is not None else "hips"


def pack_races(ubc):
    """One body file per race and sex (see sculpt.py and races/)."""
    import sculpt
    for race in sculpt.RACES:
        # Races not sculpted yet use the human body (see rigged.rs).
        if not os.path.exists(os.path.join(HERE, "races", race + ".py")):
            continue
        for sex in sculpt.SEXES:
            body = sculpt.make(ubc, race, sex)
            sculpt.write(body, os.path.join(HERE, f"{race}_{sex.lower()}.glb"))


def main():
    if len(sys.argv) == 3 and sys.argv[2] == "--bodies":
        pack_races(sys.argv[1])
        return
    ubc, ual1, ual2, adventurers, skeletons = sys.argv[1:6]
    adventurers = os.path.join(adventurers, "addons/kaykit_character_pack_adventures/Characters/gltf")
    skeletons = os.path.join(skeletons, "addons/kaykit_character_pack_skeletons/Characters/gltf")
    folder = os.path.join(ubc, "Base Characters", "Godot - UE")
    male_doc = load(os.path.join(folder, "Superhero_Male_FullBody.gltf"))
    male = Skeleton(male_doc)

    lib1, lib2 = load(ual1), load(ual2)
    w = Writer()
    w.skeleton(male.bones())
    quaternius_clips(w, male, lib1, UAL1_CLIPS)
    quaternius_clips(w, male, lib2, UAL2_CLIPS)
    retarget(w, male, load(os.path.join(adventurers, "Knight.glb")), grip_pose(male, lib1))
    w.save(os.path.join(HERE, "rig.glb"))

    pack_races(ubc)
    pack_props(adventurers, male)
    pack_skeletons(skeletons, male)


if __name__ == "__main__":
    main()
