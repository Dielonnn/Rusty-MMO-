# Character models

Rigged, animated people, all from free CC0 (public domain) packs; see the
LICENSE files here:

- **Universal Base Characters [Standard]** by Quaternius (quaternius.com):
  the male and female bodies, eyes, brows and hairstyles.
- **Universal Animation Library 1 and 2 [Standard]** by Quaternius: most of
  the animation clips. They share the base characters' skeleton.
- **KayKit Character Pack: Adventurers 1.0** by Kay Lousberg
  (github.com/KayKit-Game-Assets/KayKit-Character-Pack-Adventures-1.0):
  the weapons, shields and hats, and the moves Quaternius' free packs lack
  (two-handed chops, dual-wield stabs, kicks, bow aiming, long casts),
  retargeted onto the Quaternius skeleton.
- **KayKit Character Pack: Skeletons 1.0** by Kay Lousberg
  (github.com/KayKit-Game-Assets/KayKit-Character-Pack-Skeletons-1.0):
  the four skeleton bodies and their gear, rebuilt on the Quaternius skeleton.

`pack.py` made these files from the packs:

- `rig.glb`: the shared skeleton and every animation clip the game uses.
- `male.glb`, `female.glb`: a body each, with eyes, brows and all the
  hairstyles, cut down to about 5k triangles, on one texture atlas.
- `props.glb`: the KayKit weapons and hats, each tied to a hand or head bone.
- `skeletons.glb`: the skeleton bodies and their helmets and hoods.

To change which clips or props are kept, edit the lists in `pack.py` and run

    python pack.py <Universal Base Characters dir> <UAL1_Standard.glb> \
        <UAL2_Standard.glb> <KayKit Adventurers clone> <KayKit Skeletons clone>

with numpy, Pillow and `bpy` (Blender as a Python module, used to thin the
meshes) installed. The `.glb` files are Quaternius' Unreal-Godot exports.
