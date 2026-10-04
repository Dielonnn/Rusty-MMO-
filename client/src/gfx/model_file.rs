//! Loading 3D models from binary glTF files (`.glb`), the format most
//! modeling tools and asset packs export.
//!
//! This loads static meshes, already placed where the file's scene puts
//! them. Skeletons and animation clips come later, with the models that
//! need them.

use gltf::buffer::Source;
use macroquad::prelude::*;

use super::Batch;

/// One mesh part of a model, in the model's own space.
#[derive(Clone, Debug)]
pub struct ModelPart {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    /// Three per triangle.
    pub indices: Vec<u32>,
    /// The part's material color.
    pub color: Color,
}

/// A model loaded from a file: all its parts.
#[derive(Clone, Debug, Default)]
pub struct ModelFile {
    pub parts: Vec<ModelPart>,
}

impl ModelFile {
    /// Reads a `.glb` file's bytes (usually from `include_bytes!`).
    pub fn from_glb(bytes: &[u8]) -> Result<Self, String> {
        let file = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
        let blob = file.blob.as_deref();
        let mut parts = Vec::new();
        let scene = file
            .default_scene()
            .or_else(|| file.scenes().next())
            .ok_or("the file has no scene")?;
        let mut stack: Vec<(gltf::Node, Mat4)> =
            scene.nodes().map(|n| (n, Mat4::IDENTITY)).collect();
        while let Some((node, parent)) = stack.pop() {
            let local = Mat4::from_cols_array_2d(&node.transform().matrix());
            let transform = parent * local;
            if let Some(mesh) = node.mesh() {
                for primitive in mesh.primitives() {
                    let reader = primitive.reader(|buffer| match buffer.source() {
                        Source::Bin => blob,
                        Source::Uri(_) => None,
                    });
                    let Some(positions) = reader.read_positions() else {
                        continue;
                    };
                    let positions: Vec<Vec3> = positions
                        .map(|p| transform.transform_point3(Vec3::from(p)))
                        .collect();
                    let normals = match reader.read_normals() {
                        Some(n) => n
                            .map(|n| {
                                transform
                                    .transform_vector3(Vec3::from(n))
                                    .normalize_or_zero()
                            })
                            .collect(),
                        None => vec![Vec3::Y; positions.len()],
                    };
                    let uvs = match reader.read_tex_coords(0) {
                        Some(t) => t.into_f32().map(Vec2::from).collect(),
                        None => vec![Vec2::ZERO; positions.len()],
                    };
                    let indices = match reader.read_indices() {
                        Some(i) => i.into_u32().collect(),
                        None => (0..positions.len() as u32).collect(),
                    };
                    let [r, g, b, a] = primitive
                        .material()
                        .pbr_metallic_roughness()
                        .base_color_factor();
                    parts.push(ModelPart {
                        positions,
                        normals,
                        uvs,
                        indices,
                        color: Color::new(r, g, b, a),
                    });
                }
            }
            stack.extend(node.children().map(|c| (c, transform)));
        }
        Ok(Self { parts })
    }

    /// Draws the model placed by `transform`, its colors multiplied by `tint`.
    pub fn draw(&self, b: &mut Batch, transform: Mat4, tint: Color) {
        for part in &self.parts {
            let color = Color::new(
                part.color.r * tint.r,
                part.color.g * tint.g,
                part.color.b * tint.b,
                part.color.a * tint.a,
            );
            let vertex = |b: &mut Batch, i: usize| {
                b.vertex_uv(
                    transform.transform_point3(part.positions[i]),
                    part.uvs[i],
                    transform.transform_vector3(part.normals[i]),
                    color,
                )
            };
            // Small parts go in whole, sharing vertices; big ones a
            // triangle at a time, so they fit however full the batch is.
            if part.positions.len() <= 4096 {
                let base = b.begin_shape(part.positions.len(), part.indices.len());
                for i in 0..part.positions.len() {
                    vertex(b, i);
                }
                let indices: Vec<u16> = part.indices.iter().map(|&i| base + i as u16).collect();
                b.index(&indices);
            } else {
                for tri in part.indices.chunks_exact(3) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-triangle `.glb`, built by hand.
    fn triangle_glb() -> Vec<u8> {
        let mut bin = Vec::new();
        for p in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for v in p {
                bin.extend_from_slice(&v.to_le_bytes());
            }
        }
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
        let mut json = json.as_bytes().to_vec();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
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

    #[test]
    fn loads_a_glb_and_places_its_parts() {
        let model = ModelFile::from_glb(&triangle_glb()).unwrap();
        assert_eq!(model.parts.len(), 1);
        let part = &model.parts[0];
        assert_eq!(part.indices, vec![0, 1, 2]);
        // The node's translation is applied.
        assert_eq!(part.positions[2], vec3(0.0, 6.0, 0.0));
        assert_eq!(part.color, Color::new(1.0, 0.5, 0.25, 1.0));

        let mut b = Batch::recording();
        model.draw(&mut b, Mat4::from_translation(Vec3::X), WHITE);
        let meshes = b.finish();
        assert_eq!(meshes[0].vertices.len(), 3);
        assert_eq!(meshes[0].vertices[1].position, vec3(2.0, 5.0, 0.0));
    }

    #[test]
    fn rejects_files_that_are_not_models() {
        assert!(ModelFile::from_glb(b"not a model").is_err());
    }
}
