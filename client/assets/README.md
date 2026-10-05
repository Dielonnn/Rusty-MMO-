# Art

Textures and models built into the game with `include_bytes!`, so the exe
stays a single file. Each folder belongs to one part of the graphics code:

- `world/`: terrain, foliage and building textures (`client/src/world/`)
- `characters/`: character, creature and gear models (`client/src/models/`);
  see its README for where they come from
- `fx/`: spell effect textures (`client/src/vfx.rs`)

Decode textures with `gfx::texture::load` and models with
`gfx::ModelFile::from_glb`. Check each file's license before adding it, and
note where it came from next to it.
