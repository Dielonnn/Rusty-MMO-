//! The shape of the world: terrain height, bounds and a few geometry helpers.
//!
//! Both sides use the same terrain function, so the server can put mobs on
//! the ground and the client can render the same hills it walks on.
//!
//! Directions: `yaw` 0 faces +Z, and the forward vector is `(sin yaw, 0, cos yaw)`.

use glam::{Vec2, Vec3, vec3};

/// The playable area spans `-WORLD_HALF_SIZE..WORLD_HALF_SIZE` on X and Z.
pub const WORLD_HALF_SIZE: f32 = 220.0;
/// The town around the origin is flat and has no mobs.
pub const TOWN_RADIUS: f32 = 28.0;
/// Where players appear when they log in or release their spirit.
pub const GRAVEYARD: Vec2 = Vec2::new(0.0, -10.0);
/// Lakes fill everything below this height.
pub const WATER_LEVEL: f32 = -4.6;
/// How far away other entities are sent to (and drawn by) a client.
pub const VIEW_DISTANCE: f32 = 110.0;

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Height of the ground at `(x, z)`.
pub fn terrain_height(x: f32, z: f32) -> f32 {
    let d = (x * x + z * z).sqrt();
    let outside_town = smoothstep(TOWN_RADIUS, TOWN_RADIUS + 30.0, d);
    let mut hills = (x * 0.031).sin() * 3.5
        + (z * 0.027).cos() * 3.0
        + ((x + z) * 0.071).sin() * 1.1
        + ((x - 0.6 * z) * 0.013).sin() * 6.0;
    // Shallow valleys: only the deepest hold lakes.
    if hills < 0.0 {
        hills *= 0.45;
    }
    // Mountains rise along the edge of the map.
    let edge = smoothstep(
        WORLD_HALF_SIZE - 30.0,
        WORLD_HALF_SIZE,
        x.abs().max(z.abs()),
    );
    hills * outside_town + edge * 22.0
}

/// The point on the ground under `(x, z)`.
pub fn ground(x: f32, z: f32) -> Vec3 {
    vec3(x, terrain_height(x, z), z)
}

/// Keeps a position inside the map and not below the ground.
pub fn clamp_to_world(pos: Vec3) -> Vec3 {
    let limit = WORLD_HALF_SIZE - 2.0;
    let x = pos.x.clamp(-limit, limit);
    let z = pos.z.clamp(-limit, limit);
    vec3(x, pos.y.max(terrain_height(x, z)), z)
}

pub fn forward(yaw: f32) -> Vec3 {
    vec3(yaw.sin(), 0.0, yaw.cos())
}

/// The yaw that faces from `from` towards `to`.
pub fn yaw_towards(from: Vec3, to: Vec3) -> f32 {
    (to.x - from.x).atan2(to.z - from.z)
}

/// Distance on the XZ plane.
pub fn flat_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

/// Whether something at `from` looking along `yaw` has `to` within the
/// forward 180 degree arc. Things right on top of you always count.
pub fn is_facing(from: Vec3, yaw: f32, to: Vec3) -> bool {
    let dir = vec3(to.x - from.x, 0.0, to.z - from.z);
    if dir.length_squared() < 0.25 {
        return true;
    }
    forward(yaw).dot(dir.normalize()) >= -0.05
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn town_is_flat() {
        for (x, z) in [
            (0.0, 0.0),
            (10.0, -15.0),
            (-20.0, 5.0),
            (GRAVEYARD.x, GRAVEYARD.y),
        ] {
            assert_eq!(terrain_height(x, z), 0.0);
        }
    }

    #[test]
    fn lakes_are_small() {
        let mut wet = 0;
        let n = 100;
        for i in 0..n {
            for j in 0..n {
                let x = -WORLD_HALF_SIZE + 2.0 * WORLD_HALF_SIZE * i as f32 / n as f32;
                let z = -WORLD_HALF_SIZE + 2.0 * WORLD_HALF_SIZE * j as f32 / n as f32;
                if terrain_height(x, z) < WATER_LEVEL {
                    wet += 1;
                }
            }
        }
        assert!(
            wet < n * n / 10,
            "{wet} of {} samples are under water",
            n * n
        );
    }

    #[test]
    fn edges_are_raised() {
        assert!(terrain_height(WORLD_HALF_SIZE, 0.0) > 12.0);
    }

    #[test]
    fn facing() {
        let origin = Vec3::ZERO;
        assert!(is_facing(origin, 0.0, vec3(0.0, 0.0, 5.0)));
        assert!(!is_facing(origin, 0.0, vec3(0.0, 0.0, -5.0)));
        let yaw = yaw_towards(origin, vec3(3.0, 0.0, -4.0));
        assert!(forward(yaw).distance(vec3(0.6, 0.0, -0.8)) < 1e-5);
    }

    #[test]
    fn clamping() {
        let p = clamp_to_world(vec3(1000.0, -50.0, 0.0));
        assert!(p.x < WORLD_HALF_SIZE);
        assert_eq!(p.y, terrain_height(p.x, p.z));
    }
}
