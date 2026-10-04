# Character models

Rigged, animated people, from two free packs by Kay Lousberg
(www.kaylousberg.com), both CC0 (public domain; see the LICENSE files here):

- **KayKit Character Pack: Adventurers 1.0**
  (github.com/KayKit-Game-Assets/KayKit-Character-Pack-Adventures-1.0):
  the Knight, Barbarian, Mage, Rogue and hooded Rogue bodies, their weapons
  and hats, and the animations.
- **KayKit Character Pack: Skeletons 1.0**
  (github.com/KayKit-Game-Assets/KayKit-Character-Pack-Skeletons-1.0):
  the four skeleton bodies.

`pack.py` made these files from the packs' `.glb` models: `rig.glb` holds the
shared skeleton and the animation clips the game uses, and each body file its
meshes, skin, texture and props without animations. To change which clips
are kept, edit the list in `pack.py` and run it again on clones of the packs.
