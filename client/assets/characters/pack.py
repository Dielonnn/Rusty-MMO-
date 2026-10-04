#!/usr/bin/env python3
"""Packs the KayKit character models into the small files the game embeds.

The packs ship every character with all 76 animations and every weapon
inside it (about 3.6 MB each). The game shares one set of animations
between all bodies, so this writes:

- rig.glb: the skeleton and the animation clips the game uses, and no
  meshes. Channels that only move the rig's IK helpers are dropped.
- one .glb per body: its meshes, skin, texture and attached props
  (weapons, hats), and no animations.

Usage: pack.py <KayKit Adventurers pack dir> <KayKit Skeletons pack dir>
(clones of github.com/KayKit-Game-Assets/KayKit-Character-Pack-Adventures-1.0
and KayKit-Character-Pack-Skeletons-1.0). Uses only the standard library.
"""

import json
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))

CLIPS = [
    "Idle", "Unarmed_Idle", "2H_Melee_Idle", "Blocking",
    "Walking_A", "Running_A", "Jump_Idle",
    "Death_A", "Death_A_Pose", "Hit_A",
    "1H_Melee_Attack_Chop", "1H_Melee_Attack_Slice_Diagonal",
    "1H_Melee_Attack_Stab", "2H_Melee_Attack_Chop", "2H_Melee_Attack_Slice",
    "Dualwield_Melee_Attack_Stab", "Dualwield_Melee_Attack_Slice",
    "Unarmed_Melee_Attack_Punch_A", "Unarmed_Melee_Attack_Punch_B",
    "Unarmed_Melee_Attack_Kick",
    "1H_Ranged_Aiming", "1H_Ranged_Shoot", "2H_Ranged_Aiming", "2H_Ranged_Shoot",
    "Spellcasting", "Spellcast_Shoot", "Spellcast_Raise", "Spellcast_Long",
    "Throw", "Cheer", "Interact",
]

BODIES = {
    "knight": "adventurers/Knight.glb",
    "barbarian": "adventurers/Barbarian.glb",
    "mage": "adventurers/Mage.glb",
    "rogue": "adventurers/Rogue.glb",
    "rogue_hooded": "adventurers/Rogue_Hooded.glb",
    "skeleton_minion": "skeletons/Skeleton_Minion.glb",
    "skeleton_warrior": "skeletons/Skeleton_Warrior.glb",
    "skeleton_rogue": "skeletons/Skeleton_Rogue.glb",
    "skeleton_mage": "skeletons/Skeleton_Mage.glb",
}


def read_glb(path):
    data = open(path, "rb").read()
    json_len = struct.unpack("<I", data[12:16])[0]
    doc = json.loads(data[20 : 20 + json_len])
    bin_len = struct.unpack("<I", data[20 + json_len : 24 + json_len])[0]
    blob = data[28 + json_len : 28 + json_len + bin_len]
    return doc, blob


def write_glb(path, doc, blob):
    """Writes `doc`, keeping only the buffer views something still uses."""
    views = doc["bufferViews"]
    used = sorted(
        {a["bufferView"] for a in doc.get("accessors", [])}
        | {i["bufferView"] for i in doc.get("images", [])}
    )
    remap = {old: new for new, old in enumerate(used)}
    out = bytearray()
    new_views = []
    for old in used:
        v = dict(views[old])
        start = v.get("byteOffset", 0)
        chunk = blob[start : start + v["byteLength"]]
        while len(out) % 4:
            out.append(0)
        v["byteOffset"] = len(out)
        out += chunk
        new_views.append(v)
    for a in doc.get("accessors", []):
        a["bufferView"] = remap[a["bufferView"]]
    for i in doc.get("images", []):
        i["bufferView"] = remap[i["bufferView"]]
    doc["bufferViews"] = new_views
    while len(out) % 4:
        out.append(0)
    doc["buffers"] = [{"byteLength": len(out)}]
    text = json.dumps(doc, separators=(",", ":")).encode()
    while len(text) % 4:
        text += b" "
    total = 12 + 8 + len(text) + 8 + len(out)
    with open(path, "wb") as f:
        f.write(b"glTF" + struct.pack("<II", 2, total))
        f.write(struct.pack("<I", len(text)) + b"JSON" + text)
        f.write(struct.pack("<I", len(out)) + b"BIN\0" + bytes(out))


def drop_unused_accessors(doc):
    """Removes accessors nothing points at any more, renumbering the rest."""
    used = set()
    for m in doc.get("meshes", []):
        for p in m["primitives"]:
            used.update(p["attributes"].values())
            if "indices" in p:
                used.add(p["indices"])
    for s in doc.get("skins", []):
        if "inverseBindMatrices" in s:
            used.add(s["inverseBindMatrices"])
    for a in doc.get("animations", []):
        for s in a["samplers"]:
            used.update([s["input"], s["output"]])
    order = sorted(used)
    remap = {old: new for new, old in enumerate(order)}
    doc["accessors"] = [doc["accessors"][i] for i in order]
    for m in doc.get("meshes", []):
        for p in m["primitives"]:
            p["attributes"] = {k: remap[v] for k, v in p["attributes"].items()}
            if "indices" in p:
                p["indices"] = remap[p["indices"]]
    for s in doc.get("skins", []):
        if "inverseBindMatrices" in s:
            s["inverseBindMatrices"] = remap[s["inverseBindMatrices"]]
    for a in doc.get("animations", []):
        for s in a["samplers"]:
            s["input"] = remap[s["input"]]
            s["output"] = remap[s["output"]]


def pack_rig(src, dst):
    doc, blob = read_glb(src)
    nodes = doc["nodes"]
    helper = lambda i: "IK" in nodes[i]["name"] or "control" in nodes[i]["name"]
    clips = {a["name"]: a for a in doc["animations"]}
    missing = [c for c in CLIPS if c not in clips]
    assert not missing, missing
    animations = []
    for name in CLIPS:
        a = clips[name]
        channels = [c for c in a["channels"] if not helper(c["target"]["node"])]
        samplers = []
        for c in channels:
            samplers.append(a["samplers"][c["sampler"]])
            c["sampler"] = len(samplers) - 1
        animations.append({"name": name, "channels": channels, "samplers": samplers})
    doc["animations"] = animations
    for n in nodes:
        n.pop("mesh", None)
        n.pop("skin", None)
    for key in ["meshes", "skins", "materials", "textures", "images", "samplers"]:
        doc.pop(key, None)
    drop_unused_accessors(doc)
    write_glb(dst, doc, blob)


def pack_body(src, dst):
    doc, blob = read_glb(src)
    doc.pop("animations", None)
    drop_unused_accessors(doc)
    write_glb(dst, doc, blob)


def main():
    adventurers, skeletons = sys.argv[1:3]
    roots = {
        "adventurers": os.path.join(
            adventurers, "addons/kaykit_character_pack_adventures/Characters/gltf"
        ),
        "skeletons": os.path.join(
            skeletons, "addons/kaykit_character_pack_skeletons/Characters/gltf"
        ),
    }
    path = lambda p: os.path.join(roots[p.split("/")[0]], p.split("/")[1])
    pack_rig(path(BODIES["knight"]), os.path.join(HERE, "rig.glb"))
    for name, src in BODIES.items():
        pack_body(path(src), os.path.join(HERE, name + ".glb"))


if __name__ == "__main__":
    main()
