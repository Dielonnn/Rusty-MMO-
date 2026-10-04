//! The 3D shaders: per-pixel lighting from a warm sun and a cool sky, a
//! soft rim light, a faint painted grain on every lit surface, and
//! distance fog; cut-out textures for foliage; blended, painted ground;
//! and rippling water.

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation};
use macroquad::models::{Mesh, draw_mesh};
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

varying mediump vec2 uv;
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

/// What every fragment shader starts with: its inputs, the light and fog
/// uniforms, and the lighting and fog shared by all of them.
const COMMON: &str = r#"#version 100
precision mediump float;
varying lowp vec4 color;
varying mediump vec2 uv;
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

// The light falling on a surface facing `n`.
vec3 lighting(vec3 n) {
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
    return light + (SunColor * 0.6 + Ambient * 0.4) * rim * RIM;
}

vec3 fogged(vec3 rgb) {
    float f = clamp((depth - FogNear) / (FogFar - FogNear), 0.0, 1.0);
    return mix(rgb, FogColor.rgb, f * f * (3.0 - 2.0 * f));
}
"#;

/// Everything but the ground and water. A texture's alpha cuts the shape
/// out (leaves, grass) rather than fading it; the vertex alpha fades it.
const FRAGMENT: &str = r#"
void main() {
    vec4 t = texture2D(Texture, uv);
    if (t.a < 0.5) {
        discard;
    }
    vec3 rgb = color.rgb * t.rgb;
    if (surface.w > 0.5) {
        vec3 n = normalize(surface.xyz);
        // A painted grain, sampled along the two axes the surface faces
        // least, and faded out with distance so it never shimmers.
        vec3 a = abs(n);
        vec2 tc = a.y > max(a.x, a.z) ? world.xz : (a.x > a.z ? world.zy : world.xy);
        float g = texture2D(Detail, tc * 0.31).r * 0.6 + texture2D(Detail, tc * 0.07).r * 0.4;
        float near = 1.0 - clamp((depth - 25.0) / 60.0, 0.0, 1.0);
        rgb *= lighting(n) * (1.0 + (g - 0.5) * GRAIN * 2.0 * near);
    }
    gl_FragColor = vec4(fogged(rgb), color.a);
}"#;

/// The ground: grass (or sand, snow...), dirt and rock textures blended by
/// the weights each vertex carries in its texture coordinates (x: rock,
/// y: dirt and road), over the zone's ground colors.
const TERRAIN: &str = r#"
uniform sampler2D Ground;
uniform vec3 GroundShadow;
uniform vec3 GroundLight;

void main() {
    vec3 n = normalize(surface.xyz);
    vec4 g = texture2D(Ground, world.xz * 0.21);
    // A much larger, offset copy breaks up the repeats.
    float broad = texture2D(Ground, world.xz * 0.033 + vec2(0.31, 0.67)).a;
    // Rock is projected from the side the slope faces most, so cliffs
    // aren't smeared.
    vec3 a = abs(n);
    vec2 rp = a.y > max(a.x, a.z) ? world.xz : (a.x > a.z ? world.zy : world.xy);
    float stone = texture2D(Ground, rp * 0.12).b;
    // The patterns push the edges between materials about, so they meet
    // in ragged, painted borders instead of smooth fades.
    float dirt = clamp((uv.y - 0.5) * 2.5 + 0.5 + (g.g - 0.5) * 1.2, 0.0, 1.0);
    float rock = clamp((uv.x - 0.5) * 2.5 + 0.5 + (stone - 0.5) * 1.2, 0.0, 1.0);
    vec3 top = mix(GroundShadow, GroundLight, g.r);
    vec3 m = mix(top, vec3(0.68 + 0.62 * g.g), dirt);
    m = mix(m, vec3(0.6 + 0.75 * stone) * vec3(1.0, 0.99, 1.02), rock);
    m *= 0.86 + 0.28 * broad;
    // Far away the fine pattern averages out, which hides its repeats.
    float near = 1.0 - clamp((depth - 70.0) / 120.0, 0.0, 1.0);
    m = mix(vec3(0.97 + (broad - 0.5) * 0.2), m, 0.35 + 0.65 * near);
    gl_FragColor = vec4(fogged(color.rgb * m * lighting(n)), 1.0);
}"#;

/// Water: rippling, deeper and darker away from the shore, reflecting the
/// sky at low angles, with a sun glint and foam lapping at the shoreline.
/// Each vertex's texture x is the water's depth there; y is 1 for ice.
const WATER: &str = r#"
uniform float Time;
uniform vec3 Sky;

void main() {
    float deep = uv.x;
    float still = 1.0 - clamp(uv.y, 0.0, 1.0);
    vec2 p = world.xz;
    // A few waves crossing at different angles and speeds.
    vec2 slope = vec2(0.0);
    vec2 d1 = vec2(0.8, 0.6);
    vec2 d2 = vec2(-0.45, 0.9);
    vec2 d3 = vec2(0.97, -0.25);
    slope += d1 * cos(dot(p, d1) * 0.9 + Time * 1.3) * 0.5;
    slope += d2 * cos(dot(p, d2) * 1.7 + Time * 1.9) * 0.3;
    slope += d3 * cos(dot(p, d3) * 3.1 + Time * 2.6) * 0.18;
    float fine = texture2D(Detail, p * 0.12 + vec2(Time * 0.02, Time * 0.013)).r - 0.5;
    slope += vec2(fine, -fine) * 0.6;
    vec3 n = normalize(vec3(-slope.x * 0.16 * still, 1.0, -slope.y * 0.16 * still));
    vec3 view = normalize(CameraPos - world);
    float facing = clamp(dot(n, view), 0.0, 1.0);
    float fresnel = 0.05 + 0.55 * pow(1.0 - facing, 5.0);
    float far = clamp(deep / 2.5, 0.0, 1.0);
    vec3 body = mix(mix(color.rgb, vec3(1.0), 0.08), color.rgb * 0.62, far);
    body *= Ambient * 0.85 + SunColor * 0.5;
    // The sky it reflects is tinted by the water itself.
    vec3 rgb = mix(body, Sky * mix(vec3(1.0), color.rgb * 1.5, 0.35), fresnel);
    vec3 h = normalize(SunDir + view);
    rgb += SunColor * pow(clamp(dot(n, h), 0.0, 1.0), 120.0) * 1.2;
    // Foam where it's shallowest, lapping in and out.
    float lap = 0.45 + 0.15 * sin(Time * 1.4 + p.x * 0.27 + p.y * 0.21);
    float band = 1.0 - smoothstep(0.0, lap, deep);
    float froth = texture2D(Detail, p * 0.45 + vec2(0.0, Time * 0.03)).r;
    float foam = band * smoothstep(0.35, 0.55, froth + band * 0.45) * still;
    rgb = mix(rgb, (Ambient + SunColor * 0.7) * 0.95, foam * 0.85);
    float alpha = mix(color.a * 0.8, color.a, far);
    alpha = clamp(max(alpha + fresnel * 0.25, foam * 0.9), 0.0, 1.0);
    gl_FragColor = vec4(fogged(rgb), alpha);
}"#;

/// A zone's painted ground: the texture (its channels: red the top layer,
/// such as grass, sand or snow; green dirt; blue rock; alpha broad
/// variation) and the tints the top layer's dark and light parts take.
pub struct GroundPaint {
    pub texture: Texture2D,
    pub shadow: Vec3,
    pub light: Vec3,
}

/// The 3D materials and their textures.
pub struct Shading {
    material: Option<Material>,
    terrain: Option<Material>,
    water: Option<Material>,
}

/// The uniforms every material has.
fn common_uniforms() -> Vec<UniformDesc> {
    vec![
        UniformDesc::new("SunDir", UniformType::Float3),
        UniformDesc::new("SunColor", UniformType::Float3),
        UniformDesc::new("Ambient", UniformType::Float3),
        UniformDesc::new("CameraPos", UniformType::Float3),
        UniformDesc::new("FogColor", UniformType::Float4),
        UniformDesc::new("FogNear", UniformType::Float1),
        UniformDesc::new("FogFar", UniformType::Float1),
    ]
}

fn material(
    fragment: &str,
    extra: Vec<UniformDesc>,
    textures: &[&str],
    detail: &Texture2D,
) -> Option<Material> {
    let fragment = format!("{COMMON}{fragment}");
    let mut uniforms = common_uniforms();
    uniforms.extend(extra);
    let mut names = vec!["Detail".to_string()];
    names.extend(textures.iter().map(|t| t.to_string()));
    let m = load_material(
        ShaderSource::Glsl {
            vertex: VERTEX,
            fragment: &fragment,
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
            uniforms,
            textures: names,
        },
    )
    .ok()?;
    m.set_texture("Detail", detail.clone());
    Some(m)
}

impl Shading {
    pub fn new() -> Self {
        let detail = texture::detail();
        Self {
            material: material(FRAGMENT, vec![], &[], &detail),
            terrain: material(
                TERRAIN,
                vec![
                    UniformDesc::new("GroundShadow", UniformType::Float3),
                    UniformDesc::new("GroundLight", UniformType::Float3),
                ],
                &["Ground"],
                &detail,
            ),
            water: material(
                WATER,
                vec![
                    UniformDesc::new("Time", UniformType::Float1),
                    UniformDesc::new("Sky", UniformType::Float3),
                ],
                &[],
                &detail,
            ),
        }
    }

    fn all(&self) -> impl Iterator<Item = &Material> {
        [&self.material, &self.terrain, &self.water]
            .into_iter()
            .flatten()
    }

    /// Call after `set_camera` for a 3D pass: lights and fogs everything
    /// drawn until `end`. `sky` is the color water reflects.
    pub fn begin(&self, light: &Light, fog: Fog, sky: Color) {
        let camera = camera_position();
        for m in self.all() {
            m.set_uniform("SunDir", light.sun_dir.normalize_or_zero());
            m.set_uniform("SunColor", light.sun);
            m.set_uniform("Ambient", light.ambient);
            m.set_uniform("CameraPos", camera);
            m.set_uniform("FogColor", vec4(fog.color.r, fog.color.g, fog.color.b, 1.0));
            m.set_uniform("FogNear", fog.near);
            m.set_uniform("FogFar", fog.far);
        }
        if let Some(w) = &self.water {
            w.set_uniform("Sky", vec3(sky.r, sky.g, sky.b));
        }
        self.restore();
    }

    fn restore(&self) {
        if let Some(m) = &self.material {
            gl_use_material(m);
        }
    }

    /// Draws ground meshes with the painted ground, then goes back to the
    /// ordinary material.
    pub fn draw_terrain(&self, meshes: &[Mesh], paint: &GroundPaint) {
        if let Some(t) = &self.terrain {
            t.set_texture("Ground", paint.texture.clone());
            t.set_uniform("GroundShadow", paint.shadow);
            t.set_uniform("GroundLight", paint.light);
            gl_use_material(t);
        }
        for m in meshes {
            draw_mesh(m);
        }
        self.restore();
    }

    /// Draws water meshes, rippling as `time` goes by, then goes back to
    /// the ordinary material.
    pub fn draw_water(&self, meshes: &[Mesh], time: f32) {
        if let Some(w) = &self.water {
            w.set_uniform("Time", time);
            gl_use_material(w);
        }
        for m in meshes {
            draw_mesh(m);
        }
        self.restore();
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
