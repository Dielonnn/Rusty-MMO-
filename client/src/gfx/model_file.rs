//! Loading 3D models from binary glTF files (`.glb`), the format most
//! modeling tools and asset packs export: their meshes, their skeletons and
//! skins (meshes that bend with the bones under them), their animation
//! clips and their texture.
//!
//! A model's nodes form a tree: bones, and the meshes that hang off them.
//! A pose gives each node its own transform, relative to its parent; the
//! file's own transforms are the rest pose, and an animation clip sets
//! them over time. Models that share a skeleton can share clips: pose the
//! skeleton (from one file) and draw each body (from others) on it, with
//! their bones matched by name (see `ModelFile::bind`).

use gltf::animation::util::ReadOutputs;
use gltf::buffer::Source;
use macroquad::prelude::*;

use super::Batch;

/// One mesh part of a model, in its node's space (or, if skinned, in the
/// space its skin was bound in).
#[derive(Clone, Debug)]
pub struct ModelPart {
    /// The name of the node holding it, to show or hide it by.
    pub name: String,
    /// The node holding it, which places it if it isn't skinned.
    pub node: usize,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    /// Three per triangle.
    pub indices: Vec<u32>,
    /// The part's material color.
    pub color: Color,
    /// Skinned parts: which skin, and for each vertex up to four of its
    /// joints (indices into the skin's joints) and how much each one pulls.
    pub skin: Option<usize>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
}

/// A node's transform relative to its parent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    /// Partway (`t`, 0 to 1) from this transform to `other`.
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            translation: self.translation.lerp(other.translation, t),
            rotation: self.rotation.slerp(other.rotation, t),
            scale: self.scale.lerp(other.scale, t),
        }
    }
}

/// A bone, a mesh's place, or a group: one node of the model's tree.
#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub parent: Option<usize>,
    /// Where the file puts it: its rest pose.
    pub rest: Transform,
}

/// The bones a skinned mesh bends with.
#[derive(Clone, Debug)]
pub struct Skin {
    /// The nodes acting as joints.
    pub joints: Vec<usize>,
    /// For each joint, from the space the mesh was bound in to the joint's.
    pub inverse_bind: Vec<Mat4>,
}

/// What one animation channel moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Channel {
    Translation,
    Rotation,
    Scale,
}

/// One node's keyframes for one property in a clip.
#[derive(Clone, Debug)]
struct Track {
    node: usize,
    channel: Channel,
    times: Vec<f32>,
    /// xyz for translation and scale, a quaternion for rotation.
    values: Vec<Vec4>,
    /// Holds each key until the next instead of blending.
    step: bool,
}

impl Track {
    fn sample(&self, time: f32) -> Vec4 {
        let last = self.times.len() - 1;
        let k = self.times.partition_point(|&t| t <= time);
        if k == 0 {
            return self.values[0];
        }
        if k > last {
            return self.values[last];
        }
        let (a, b) = (k - 1, k);
        let span = self.times[b] - self.times[a];
        let t = if self.step || span <= 0.0 {
            0.0
        } else {
            (time - self.times[a]) / span
        };
        match self.channel {
            Channel::Rotation => {
                let qa = Quat::from_vec4(self.values[a]);
                let qb = Quat::from_vec4(self.values[b]);
                Vec4::from(qa.slerp(qb, t))
            }
            _ => self.values[a].lerp(self.values[b], t),
        }
    }
}

/// An animation: how the model's nodes move over time.
#[derive(Clone, Debug)]
pub struct Clip {
    pub name: String,
    /// Seconds.
    pub duration: f32,
    tracks: Vec<Track>,
}

impl Clip {
    /// Sets the nodes the clip moves in `pose` (one transform per node of
    /// the model it came from) to where they are `time` seconds in. Times
    /// past either end hold the first or last frame.
    pub fn apply(&self, time: f32, pose: &mut [Transform]) {
        for track in &self.tracks {
            let Some(node) = pose.get_mut(track.node) else {
                continue;
            };
            let v = track.sample(time);
            match track.channel {
                Channel::Translation => node.translation = v.truncate(),
                Channel::Rotation => node.rotation = Quat::from_vec4(v).normalize(),
                Channel::Scale => node.scale = v.truncate(),
            }
        }
    }
}

/// A decoded texture image: `width` by `height` RGBA pixels.
#[derive(Clone, Debug)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[u8; 4]>,
}

/// A model loaded from a file: its node tree, mesh parts, skins, clips and
/// texture.
#[derive(Clone, Debug, Default)]
pub struct ModelFile {
    pub nodes: Vec<Node>,
    /// Nodes in an order where every parent comes before its children.
    order: Vec<usize>,
    pub parts: Vec<ModelPart>,
    pub skins: Vec<Skin>,
    pub clips: Vec<Clip>,
    /// The first texture in the file, if it has one.
    pub picture: Option<Picture>,
}

impl ModelFile {
    /// Reads a `.glb` file's bytes (usually from `include_bytes!`).
    pub fn from_glb(bytes: &[u8]) -> Result<Self, String> {
        let file = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
        let blob = file.blob.as_deref();
        let data = |buffer: gltf::Buffer| match buffer.source() {
            Source::Bin => blob,
            Source::Uri(_) => None,
        };

        let mut nodes: Vec<Node> = file
            .nodes()
            .map(|n| {
                let (translation, rotation, scale) = n.transform().decomposed();
                Node {
                    name: n.name().unwrap_or_default().to_string(),
                    parent: None,
                    rest: Transform {
                        translation: Vec3::from(translation),
                        rotation: Quat::from_array(rotation),
                        scale: Vec3::from(scale),
                    },
                }
            })
            .collect();
        for n in file.nodes() {
            for child in n.children() {
                nodes[child.index()].parent = Some(n.index());
            }
        }
        let order = parents_first(&nodes);

        let mut parts = Vec::new();
        for node in file.nodes() {
            let Some(mesh) = node.mesh() else {
                continue;
            };
            let skin = node.skin().map(|s| s.index());
            for primitive in mesh.primitives() {
                let reader = primitive.reader(data);
                let Some(positions) = reader.read_positions() else {
                    continue;
                };
                let positions: Vec<Vec3> = positions.map(Vec3::from).collect();
                let count = positions.len();
                let normals = match reader.read_normals() {
                    Some(n) => n.map(Vec3::from).collect(),
                    None => vec![Vec3::Y; count],
                };
                let uvs = match reader.read_tex_coords(0) {
                    Some(t) => t.into_f32().map(Vec2::from).collect(),
                    None => vec![Vec2::ZERO; count],
                };
                let indices = match reader.read_indices() {
                    Some(i) => i.into_u32().collect(),
                    None => (0..count as u32).collect(),
                };
                let (joints, weights) = match (skin, reader.read_joints(0), reader.read_weights(0))
                {
                    (Some(_), Some(j), Some(w)) => (j.into_u16().collect(), w.into_f32().collect()),
                    _ => (Vec::new(), Vec::new()),
                };
                let [r, g, b, a] = primitive
                    .material()
                    .pbr_metallic_roughness()
                    .base_color_factor();
                parts.push(ModelPart {
                    name: node.name().unwrap_or_default().to_string(),
                    node: node.index(),
                    positions,
                    normals,
                    uvs,
                    indices,
                    color: Color::new(r, g, b, a),
                    skin: skin.filter(|_| !joints.is_empty()),
                    joints,
                    weights,
                });
            }
        }

        let skins = file
            .skins()
            .map(|s| {
                let joints: Vec<usize> = s.joints().map(|j| j.index()).collect();
                let inverse_bind = match s.reader(data).read_inverse_bind_matrices() {
                    Some(m) => m.map(|m| Mat4::from_cols_array_2d(&m)).collect(),
                    None => vec![Mat4::IDENTITY; joints.len()],
                };
                Skin {
                    joints,
                    inverse_bind,
                }
            })
            .collect();

        let mut clips = Vec::new();
        for animation in file.animations() {
            let mut tracks = Vec::new();
            for channel in animation.channels() {
                let reader = channel.reader(data);
                let (Some(times), Some(outputs)) = (reader.read_inputs(), reader.read_outputs())
                else {
                    continue;
                };
                let times: Vec<f32> = times.collect();
                let (channel_kind, values): (Channel, Vec<Vec4>) = match outputs {
                    ReadOutputs::Translations(t) => (
                        Channel::Translation,
                        t.map(|v| Vec3::from(v).extend(0.0)).collect(),
                    ),
                    ReadOutputs::Rotations(r) => {
                        (Channel::Rotation, r.into_f32().map(Vec4::from).collect())
                    }
                    ReadOutputs::Scales(s) => (
                        Channel::Scale,
                        s.map(|v| Vec3::from(v).extend(0.0)).collect(),
                    ),
                    ReadOutputs::MorphTargetWeights(_) => continue,
                };
                let sampler = channel.sampler();
                let cubic = sampler.interpolation() == gltf::animation::Interpolation::CubicSpline;
                // Cubic splines store an in-tangent, the value and an
                // out-tangent per key; keep just the values.
                let values = if cubic {
                    values.chunks(3).map(|c| c[1]).collect()
                } else {
                    values
                };
                if times.is_empty() || values.len() < times.len() {
                    continue;
                }
                tracks.push(Track {
                    node: channel.target().node().index(),
                    channel: channel_kind,
                    times,
                    values,
                    step: sampler.interpolation() == gltf::animation::Interpolation::Step,
                });
            }
            let duration = tracks
                .iter()
                .filter_map(|t| t.times.last().copied())
                .fold(0.0, f32::max);
            clips.push(Clip {
                name: animation.name().unwrap_or_default().to_string(),
                duration,
                tracks,
            });
        }

        let picture = file.images().find_map(|image| match image.source() {
            gltf::image::Source::View { view, .. } => {
                let bytes = data(view.buffer())?;
                let bytes = bytes.get(view.offset()..view.offset() + view.length())?;
                let image = Image::from_file_with_format(bytes, None).ok()?;
                Some(Picture {
                    width: image.width as usize,
                    height: image.height as usize,
                    pixels: image.bytes.as_chunks::<4>().0.to_vec(),
                })
            }
            gltf::image::Source::Uri { .. } => None,
        });

        Ok(Self {
            nodes,
            order,
            parts,
            skins,
            clips,
            picture,
        })
    }

    /// Every node where the file puts it.
    pub fn rest_pose(&self) -> Vec<Transform> {
        self.nodes.iter().map(|n| n.rest).collect()
    }

    pub fn clip(&self, name: &str) -> Option<&Clip> {
        self.clips.iter().find(|c| c.name == name)
    }

    /// The index of the node called `name`.
    pub fn node(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == name)
    }

    /// Where each node ends up in the model's space, given each one's
    /// transform relative to its parent.
    pub fn world(&self, pose: &[Transform]) -> Vec<Mat4> {
        let mut world = vec![Mat4::IDENTITY; self.nodes.len()];
        for &i in &self.order {
            let local = pose.get(i).unwrap_or(&self.nodes[i].rest).matrix();
            world[i] = match self.nodes[i].parent {
                Some(p) => world[p] * local,
                None => local,
            };
        }
        world
    }

    /// Matches this model's nodes to `skeleton`'s by name, so it can be
    /// drawn on a pose of `skeleton` (see `draw_on`).
    pub fn bind(&self, skeleton: &ModelFile) -> Vec<Option<usize>> {
        self.nodes.iter().map(|n| skeleton.node(&n.name)).collect()
    }

    /// Where this model's nodes end up when drawn on a skeleton posed so its
    /// nodes are at `skeleton_world`: those the skeleton has follow it, and
    /// the rest (weapons, hats) hang off their parents as the file puts them.
    pub fn world_on(&self, skeleton_world: &[Mat4], binding: &[Option<usize>]) -> Vec<Mat4> {
        let mut world = vec![Mat4::IDENTITY; self.nodes.len()];
        for &i in &self.order {
            world[i] = match binding.get(i).copied().flatten() {
                Some(s) => skeleton_world[s],
                None => {
                    let local = self.nodes[i].rest.matrix();
                    match self.nodes[i].parent {
                        Some(p) => world[p] * local,
                        None => local,
                    }
                }
            };
        }
        world
    }

    /// Draws the model in its rest pose, placed by `transform`, its colors
    /// multiplied by `tint`. (For static models, such as the buildings and
    /// props to come.)
    #[allow(dead_code)]
    pub fn draw(&self, b: &mut Batch, transform: Mat4, tint: Color) {
        let world = self.world(&self.rest_pose());
        self.draw_posed(b, transform, &world, |_| Some(tint));
    }

    /// Draws the model with its nodes at `world` (from `world` or
    /// `world_on`), placed by `transform`. `paint` says, for each part,
    /// what to multiply its colors by, or `None` to leave it out.
    pub fn draw_posed(
        &self,
        b: &mut Batch,
        transform: Mat4,
        world: &[Mat4],
        mut paint: impl FnMut(&ModelPart) -> Option<Color>,
    ) {
        let joint_matrices: Vec<Vec<Mat4>> = self
            .skins
            .iter()
            .map(|s| {
                s.joints
                    .iter()
                    .zip(&s.inverse_bind)
                    .map(|(&j, inv)| transform * world[j] * *inv)
                    .collect()
            })
            .collect();
        for part in &self.parts {
            let Some(tint) = paint(part) else {
                continue;
            };
            let color = Color::new(
                part.color.r * tint.r,
                part.color.g * tint.g,
                part.color.b * tint.b,
                part.color.a * tint.a,
            );
            let place = transform * world[part.node];
            let skin = part.skin.map(|s| &joint_matrices[s]);
            // Each vertex moved once, however many triangles share it.
            let moved: Vec<(Vec3, Vec3)> = (0..part.positions.len())
                .map(|i| match skin {
                    Some(mats) => {
                        let mut m = Mat4::ZERO;
                        for (&j, &w) in part.joints[i].iter().zip(&part.weights[i]) {
                            if w > 0.0 {
                                m += mats[j as usize] * w;
                            }
                        }
                        (
                            m.transform_point3(part.positions[i]),
                            m.transform_vector3(part.normals[i]),
                        )
                    }
                    None => (
                        place.transform_point3(part.positions[i]),
                        place.transform_vector3(part.normals[i]),
                    ),
                })
                .collect();
            let vertex = |b: &mut Batch, i: usize| {
                let (p, n) = moved[i];
                b.vertex_uv(p, part.uvs[i], n, color)
            };
            // Most parts go in whole, sharing vertices; huge ones a
            // triangle at a time, so they fit however full the batch is.
            if part.positions.len() <= 16384 {
                let base = b.begin_shape(part.positions.len(), part.indices.len());
                for i in 0..part.positions.len() {
                    vertex(b, i);
                }
                let indices: Vec<u16> = part.indices.iter().map(|&i| base + i as u16).collect();
                b.index(&indices);
            } else {
                for tri in part.indices.as_chunks::<3>().0 {
                    let base = b.begin_shape(3, 3);
                    for &i in tri {
                        vertex(b, i as usize);
                    }
                    b.index(&[base, base + 1, base + 2]);
                }
            }
        }
    }
}

/// The nodes' indices, each parent before its children.
fn parents_first(nodes: &[Node]) -> Vec<usize> {
    let depth = |mut i: usize| {
        let mut d = 0;
        while let Some(p) = nodes[i].parent {
            i = p;
            d += 1;
            if d > nodes.len() {
                break;
            }
        }
        d
    };
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|&i| depth(i));
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Packs a glTF JSON document and its binary data into a `.glb`.
    fn glb(json: &str, bin: &[u8]) -> Vec<u8> {
        let mut json = json.as_bytes().to_vec();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let mut bin = bin.to_vec();
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let total = 12 + 8 + json.len() + 8 + bin.len();
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }

    fn floats(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    /// A one-triangle `.glb`, built by hand.
    fn triangle_glb() -> Vec<u8> {
        let bin = floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        let json = r#"{
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"mesh": 0, "translation": [0, 5, 0]}],
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "material": 0}]}],
            "materials": [{"pbrMetallicRoughness": {"baseColorFactor": [1, 0.5, 0.25, 1]}}],
            "buffers": [{"byteLength": 36}],
            "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 36}],
            "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3,
                "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0]}]
        }"#;
        glb(json, &bin)
    }

    /// A two-bone arm: a root at the origin and an elbow one unit up, a
    /// skinned quad from the root to two units up whose top two vertices
    /// follow the elbow, and a clip that turns the elbow 90 degrees about Z
    /// over one second.
    fn arm_glb() -> Vec<u8> {
        let mut bin = Vec::new();
        // 0: positions (4 x vec3, 48 bytes)
        bin.extend(floats(&[
            0.0, 0.0, 0.0, 0.1, 0.0, 0.0, 0.1, 2.0, 0.0, 0.0, 2.0, 0.0,
        ]));
        // 48: joints (4 x u8x4, 16 bytes)
        bin.extend([0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
        // 64: weights (4 x vec4, 64 bytes)
        for _ in 0..4 {
            bin.extend(floats(&[1.0, 0.0, 0.0, 0.0]));
        }
        // 128: inverse binds (2 x mat4, 128 bytes): identity, and back down one.
        bin.extend(floats(&Mat4::IDENTITY.to_cols_array()));
        bin.extend(floats(&Mat4::from_translation(-Vec3::Y).to_cols_array()));
        // 256: key times (2 floats)
        bin.extend(floats(&[0.0, 1.0]));
        // 264: rotations (2 x vec4)
        let turned = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        bin.extend(floats(&[0.0, 0.0, 0.0, 1.0]));
        bin.extend(floats(&turned.to_array()));
        let json = r#"{
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [0, 3]}],
            "nodes": [
                {"name": "root", "children": [1]},
                {"name": "elbow", "translation": [0, 1, 0], "children": [2]},
                {"name": "sword", "translation": [0, 0.5, 0], "mesh": 1},
                {"name": "skin", "mesh": 0, "skin": 0}
            ],
            "skins": [{"joints": [0, 1], "inverseBindMatrices": 3}],
            "meshes": [
                {"primitives": [{"attributes": {"POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2}}]},
                {"primitives": [{"attributes": {"POSITION": 0}}]}
            ],
            "animations": [{"name": "bend",
                "channels": [{"sampler": 0, "target": {"node": 1, "path": "rotation"}}],
                "samplers": [{"input": 4, "output": 5}]}],
            "buffers": [{"byteLength": 296}],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": 48},
                {"buffer": 0, "byteOffset": 48, "byteLength": 16},
                {"buffer": 0, "byteOffset": 64, "byteLength": 64},
                {"buffer": 0, "byteOffset": 128, "byteLength": 128},
                {"buffer": 0, "byteOffset": 256, "byteLength": 8},
                {"buffer": 0, "byteOffset": 264, "byteLength": 32}
            ],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3",
                    "min": [0, 0, 0], "max": [0.1, 2, 0]},
                {"bufferView": 1, "componentType": 5121, "count": 4, "type": "VEC4"},
                {"bufferView": 2, "componentType": 5126, "count": 4, "type": "VEC4"},
                {"bufferView": 3, "componentType": 5126, "count": 2, "type": "MAT4"},
                {"bufferView": 4, "componentType": 5126, "count": 2, "type": "SCALAR",
                    "min": [0], "max": [1]},
                {"bufferView": 5, "componentType": 5126, "count": 2, "type": "VEC4"}
            ]
        }"#;
        glb(json, &bin)
    }

    fn positions(b: Batch) -> Vec<Vec3> {
        b.finish()
            .iter()
            .flat_map(|m| m.vertices.iter().map(|v| v.position))
            .collect()
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        a.distance(b) < 1e-4
    }

    #[test]
    fn loads_a_glb_and_places_its_parts() {
        let model = ModelFile::from_glb(&triangle_glb()).unwrap();
        assert_eq!(model.parts.len(), 1);
        let part = &model.parts[0];
        assert_eq!(part.indices, vec![0, 1, 2]);
        assert_eq!(part.color, Color::new(1.0, 0.5, 0.25, 1.0));

        let mut b = Batch::recording();
        model.draw(&mut b, Mat4::from_translation(Vec3::X), WHITE);
        let p = positions(b);
        assert_eq!(p.len(), 3);
        // The node's translation is applied, then the drawing transform.
        assert!(close(p[1], vec3(2.0, 5.0, 0.0)));
        assert!(close(p[2], vec3(1.0, 6.0, 0.0)));
    }

    #[test]
    fn rejects_files_that_are_not_models() {
        assert!(ModelFile::from_glb(b"not a model").is_err());
    }

    #[test]
    fn skinned_meshes_bend_with_their_bones() {
        let model = ModelFile::from_glb(&arm_glb()).unwrap();
        assert_eq!(model.skins.len(), 1);
        assert_eq!(model.clips.len(), 1);
        let clip = model.clip("bend").unwrap();
        assert!((clip.duration - 1.0).abs() < 1e-6);
        let only_skin = |p: &ModelPart| (p.name == "skin").then_some(WHITE);

        // At rest the quad stands straight up.
        let world = model.world(&model.rest_pose());
        let mut b = Batch::recording();
        model.draw_posed(&mut b, Mat4::IDENTITY, &world, only_skin);
        let p = positions(b);
        assert!(close(p[2], vec3(0.1, 2.0, 0.0)), "{p:?}");

        // A second in, the elbow has turned a quarter around Z, so the top
        // of the quad points along -X from the elbow at (0, 1, 0).
        let mut pose = model.rest_pose();
        clip.apply(1.0, &mut pose);
        let world = model.world(&pose);
        let mut b = Batch::recording();
        model.draw_posed(&mut b, Mat4::IDENTITY, &world, only_skin);
        let p = positions(b);
        assert!(close(p[0], Vec3::ZERO), "{p:?}");
        assert!(close(p[3], vec3(-1.0, 1.0, 0.0)), "{p:?}");
        assert!(close(p[2], vec3(-1.0, 1.1, 0.0)), "{p:?}");

        // Halfway, it's turned an eighth.
        let mut pose = model.rest_pose();
        clip.apply(0.5, &mut pose);
        let elbow = model.node("elbow").unwrap();
        let angle = pose[elbow].rotation.to_euler(EulerRot::XYZ).2;
        assert!((angle - std::f32::consts::FRAC_PI_4).abs() < 1e-4);
    }

    #[test]
    fn bodies_follow_a_shared_skeleton_by_bone_name() {
        let skeleton = ModelFile::from_glb(&arm_glb()).unwrap();
        let body = ModelFile::from_glb(&arm_glb()).unwrap();
        let binding = body.bind(&skeleton);
        assert!(binding.iter().all(|b| b.is_some()));
        let mut pose = skeleton.rest_pose();
        skeleton.clip("bend").unwrap().apply(1.0, &mut pose);
        let world = body.world_on(&skeleton.world(&pose), &binding);
        // The sword hangs half a unit along the turned elbow.
        let sword = body.node("sword").unwrap();
        assert!(close(
            world[sword].transform_point3(Vec3::ZERO),
            vec3(-0.5, 1.0, 0.0)
        ));
    }
}
