# Spell effect textures

- `spell_fx.png`: the atlas every glowing spell effect is cut from (glow,
  flare, flame, wisp, ring, runes, band, shard). Painted pixel by pixel by
  `make_spell_fx.py` in this folder, so it's our own work, released as CC0.
  Change the script and rerun it (`python3 make_spell_fx.py`, no packages
  needed) rather than editing the picture, and keep the cell order in step
  with `Cell` in `client/src/vfx.rs`.
