//! The 3D shader: per-pixel lighting from a warm sun and a cool sky, a
//! soft rim light, a faint painted grain on every lit surface, and
//! distance fog.

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation};
use macroquad::prelude::*;

use super::texture;

/// Where light comes from and what color it is.
#[derive(Clone, Copy)]
pub struct Light {
    /// The direction light comes from.
    pub sun_dir: Vec3,
    pub sun: Vec3,
    pub ambient: Vec3,
}

/// Distance fog: things fade into `color` between `near` and `far`.
#[derive(Clone, Copy)]
pub struct Fog {
    pub color: Color,
    pub near: f32,
    pub far: f32,
}

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
attribute vec4 normal;

varying lowp vec2 uv;
varying lowp vec4 color;
varying mediump vec4 surface;
varying highp vec3 world;
varying mediump float depth;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    vec4 p = Model * vec4(position, 1);
    gl_Position = Projection * p;
    // For a perspective camera, clip-space w is the distance along the view.
    depth = gl_Position.w;
    world = p.xyz;
    color = color0 / 255.0;
    uv = texcoord;
    // xyz: the surface normal; w: 1 if lit, 0 if it glows.
    surface = normal;
}"#;

const FRAGMENT: &str = r#"#version 100
precision mediump float;
varying lowp vec4 color;
varying lowp vec2 uv;
varying mediump vec4 surface;
varying highp vec3 world;
varying mediump float depth;

uniform sampler2D Texture;
uniform sampler2D Detail;
uniform vec3 SunDir;
uniform vec3 SunColor;
uniform vec3 Ambient;
uniform vec3 CameraPos;
uniform vec4 FogColor;
uniform float FogNear;
uniform float FogFar;

// How each part of the look is weighted (a classic, painted MMO lean:
// soft wrap-around light, a gentle rim, a faint brush grain).
const float WRAP = 0.3;
const float RIM = 0.22;
const float GRAIN = 0.16;

void main() {
    vec4 c = color * texture2D(Texture, uv);
    vec3 rgb = c.rgb;
    if (surface.w > 0.5) {
        vec3 n = normalize(surface.xyz);
        // Wrapped diffuse: light creeps past the terminator, so shapes
        // shade in soft gradients instead of a hard edge.
        float d = clamp((dot(n, SunDir) + WRAP) / (1.0 + WRAP), 0.0, 1.0);
        d = d * d * (3.0 - 2.0 * d);
        // Sky fill from above (cool), bounce from the ground below (warm, dim).
        float up = 0.5 + 0.5 * n.y;
        vec3 fill = mix(Ambient * vec3(0.66, 0.6, 0.54), Ambient * vec3(0.98, 1.0, 1.06), up);
        vec3 light = SunColor * d * 0.9 + fill;
        // Rim light catching the edges facing away from the camera.
        vec3 view = normalize(CameraPos - world);
        float rim = pow(1.0 - clamp(dot(n, view), 0.0, 1.0), 3.0);
        light += (SunColor * 0.6 + Ambient * 0.4) * rim * RIM;
        // A painted grain, sampled along the two axes the surface faces
        // least, and faded out with distance so it never shimmers.
        vec3 a = abs(n);
        vec2 tc = a.y > max(a.x, a.z) ? world.xz : (a.x > a.z ? world.zy : world.xy);
        float g = texture2D(Detail, tc * 0.31).r * 0.6 + texture2D(Detail, tc * 0.07).r * 0.4;
        float near = 1.0 - clamp((depth - 25.0) / 60.0, 0.0, 1.0);
        rgb *= light * (1.0 + (g - 0.5) * GRAIN * 2.0 * near);
    }
    float f = clamp((depth - FogNear) / (FogFar - FogNear), 0.0, 1.0);
    gl_FragColor = vec4(mix(rgb, FogColor.rgb, f * f * (3.0 - 2.0 * f)), c.a);
}"#;

/// The 3D material and its textures.
pub struct Shading {
    material: Option<Material>,
}

impl Shading {
    pub fn new() -> Self {
        let material = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX,
                fragment: FRAGMENT,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    depth_write: true,
                    depth_test: Comparison::LessOrEqual,
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    )),
                    ..Default::default()
                },
                uniforms: vec![
                    UniformDesc::new("SunDir", UniformType::Float3),
                    UniformDesc::new("SunColor", UniformType::Float3),
                    UniformDesc::new("Ambient", UniformType::Float3),
                    UniformDesc::new("CameraPos", UniformType::Float3),
                    UniformDesc::new("FogColor", UniformType::Float4),
                    UniformDesc::new("FogNear", UniformType::Float1),
                    UniformDesc::new("FogFar", UniformType::Float1),
                ],
                textures: vec!["Detail".to_string()],
            },
        )
        .ok();
        if let Some(m) = &material {
            m.set_texture("Detail", texture::detail());
        }
        Self { material }
    }

    /// Call after `set_camera` for a 3D pass: lights and fogs everything
    /// drawn until `end`.
    pub fn begin(&self, light: &Light, fog: Fog) {
        let Some(m) = &self.material else {
            return;
        };
        m.set_uniform("SunDir", light.sun_dir.normalize_or_zero());
        m.set_uniform("SunColor", light.sun);
        m.set_uniform("Ambient", light.ambient);
        m.set_uniform("CameraPos", camera_position());
        m.set_uniform("FogColor", vec4(fog.color.r, fog.color.g, fog.color.b, 1.0));
        m.set_uniform("FogNear", fog.near);
        m.set_uniform("FogFar", fog.far);
        gl_use_material(m);
    }

    pub fn end(&self) {
        gl_use_default_material();
    }
}

impl Default for Shading {
    fn default() -> Self {
        Self::new()
    }
}

/// Where the current camera is, worked out from its view-projection
/// matrix: the camera is the one point that projects to infinity.
fn camera_position() -> Vec3 {
    // SAFETY: only reads macroquad's current matrix; nothing else is
    // borrowing its context during a frame's 3D setup.
    let m = unsafe { get_internal_gl() }.quad_gl.get_projection_matrix();
    camera_from(m)
}

fn camera_from(view_projection: Mat4) -> Vec3 {
    let h = view_projection.inverse() * vec4(0.0, 0.0, 1.0, 0.0);
    if h.w.abs() < 1e-6 {
        Vec3::ZERO
    } else {
        h.truncate() / h.w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_camera_from_its_matrix() {
        // Built the way macroquad builds a `Camera3D`'s matrix.
        let position = vec3(12.0, 30.0, -7.0);
        let view = Mat4::look_at_rh(position, vec3(0.0, 2.0, 5.0), Vec3::Y);
        let matrix = Mat4::perspective_rh_gl(0.9, 1.6, 0.1, 1500.0) * view;
        let found = camera_from(matrix);
        assert!(found.distance(position) < 0.05, "{found}");
    }
}
