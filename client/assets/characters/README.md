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

A race can have its own body, male and female, sculpted from the
Quaternius base bodies by `sculpt.py` and its file in `races/` (`orc.py`, `elf.py`, `dwarf.py`,
`goblin.py`, `gnome.py`, `undead.py`; `human.py` keeps the base body): the bones and the mesh are reshaped together (longer legs,
broader shoulders, a bigger head), the flesh is moulded with brushes (a
jutting brow, a hunched back, long pointed ears), and race parts such as
tusks and beards are modeled as `Extra_` meshes on the same skeleton and
texture. Everyone stays in the base models' underwear; clothes are made in
the game from what a character wears (`client/src/models/rigged/clothes.rs`).

`pack.py` made these files from the packs:

- `rig.glb`: the shared skeleton and every animation clip the game uses.
- `<race>_male.glb`, `<race>_female.glb` (`human_male.glb`,
  `orc_female.glb`, ...): a race's body, with its eyes, brows, all the
  hairstyles and its `Extra_` parts, thinned to a game-sized mesh, on one
  texture atlas.
- `props.glb`: the KayKit weapons and hats, each tied to a hand or head bone.
- `skeletons.glb`: the skeleton bodies and their helmets and hoods.

To change which clips or props are kept, edit the lists in `pack.py` and run

    python pack.py <Universal Base Characters dir> <UAL1_Standard.glb> \
        <UAL2_Standard.glb> <KayKit Adventurers clone> <KayKit Skeletons clone>

To change a race, edit its file in `races/`, look at it with

    python sculpt.py <Universal Base Characters dir> <race> preview.png

(front, side and back views and three close-ups of the head, male above
female), and rebuild the bodies with

    python pack.py <Universal Base Characters dir> --bodies

In place of the Universal Base Characters folder, both can take `game`:
the race is then sculpted from the game's own `human_male.glb` and
`human_female.glb` (already thinned, so they aren't thinned again). The
dwarf, goblin, gnome and undead bodies were made that way:

    python sculpt.py game goblin preview.png

Both need numpy, Pillow and `bpy` (Blender as a Python module, used to
thin the meshes and render the previews) installed. The `.glb` files are
Quaternius' Unreal-Godot exports.
