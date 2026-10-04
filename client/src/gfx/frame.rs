//! A local coordinate frame for building models out of parts.

use macroquad::prelude::*;
use shared::world::forward;

use super::{Batch, basis};

/// A local coordinate frame for building a model out of parts. Every part
/// is built along the frame's own axes, so the whole model turns together.
#[derive(Clone, Copy)]
pub struct Frame {
    origin: Vec3,
    r: Vec3,
    u: Vec3,
    f: Vec3,
    scale: f32,
}

impl Frame {
    pub fn upright(pos: Vec3, yaw: f32, scale: f32) -> Self {
        let f = forward(yaw);
        Self {
            origin: pos,
            r: vec3(-f.z, 0.0, f.x),
            u: Vec3::Y,
            f,
            scale,
        }
    }

    /// Fallen over on its back (people) or side (animals).
    pub fn fallen(pos: Vec3, yaw: f32, scale: f32, on_side: bool) -> Self {
        let up = Self::upright(pos, yaw, scale);
        if on_side {
            Self {
                origin: pos + Vec3::Y * 0.25 * scale,
                r: Vec3::Y,
                u: -up.r,
                f: up.f,
                scale,
            }
        } else {
            Self {
                origin: pos + Vec3::Y * 0.2 * scale,
                r: up.r,
                u: -up.f,
                f: Vec3::Y,
                scale,
            }
        }
    }

    /// Upright, but leaning forward by `pitch` (back if negative) from the
    /// feet.
    pub fn leaning(pos: Vec3, yaw: f32, scale: f32, pitch: f32) -> Self {
        let up = Self::upright(pos, yaw, scale);
        let (sn, cs) = pitch.sin_cos();
        Self {
            origin: pos,
            r: up.r,
            u: up.u * cs + up.f * sn,
            f: up.f * cs - up.u * sn,
            scale,
        }
    }

    pub(crate) fn dir(&self, l: Vec3) -> Vec3 {
        self.r * l.x + self.u * l.y + self.f * l.z
    }

    /// A limb from `joint` along local direction `d`. Returns where it ends.
    pub fn segment(
        &self,
        b: &mut Batch,
        joint: Vec3,
        d: Vec3,
        half_width: f32,
        length: f32,
        color: Color,
    ) -> Vec3 {
        let s = self.scale;
        let wd = self.dir(d).normalize_or_zero();
        let (u, v) = basis(wd, self.r);
        b.cuboid(
            self.p(joint + d * length * 0.5),
            [
                u * half_width * s,
                wd * length * 0.5 * s,
                v * half_width * s,
            ],
            color,
        );
        joint + d * length
    }

    pub(crate) fn p(&self, l: Vec3) -> Vec3 {
        self.origin + self.dir(l) * self.scale
    }

    pub fn cube(&self, b: &mut Batch, center: Vec3, half: Vec3, color: Color) {
        let s = self.scale;
        b.cuboid(
            self.p(center),
            [
                self.r * half.x * s,
                self.u * half.y * s,
                self.f * half.z * s,
            ],
            color,
        );
    }

    /// A box tilted forward by `pitch` around the frame's right axis.
    pub fn tilted(&self, b: &mut Batch, center: Vec3, half: Vec3, pitch: f32, color: Color) {
        let s = self.scale;
        let up = vec3(0.0, pitch.cos(), pitch.sin());
        let fwd = vec3(0.0, -pitch.sin(), pitch.cos());
        b.cuboid(
            self.p(center),
            [
                self.r * half.x * s,
                self.dir(up) * half.y * s,
                self.dir(fwd) * half.z * s,
            ],
            color,
        );
    }

    /// A limb hanging from `joint`, swung forward by `angle` radians.
    /// Returns where it ends.
    pub fn limb(
        &self,
        b: &mut Batch,
        joint: Vec3,
        angle: f32,
        half_width: f32,
        length: f32,
        color: Color,
    ) -> Vec3 {
        let d = vec3(0.0, -angle.cos(), angle.sin());
        let p = vec3(0.0, angle.sin(), angle.cos());
        let s = self.scale;
        let center = joint + d * length * 0.5;
        b.cuboid(
            self.p(center),
            [
                self.r * half_width * s,
                self.dir(d) * length * 0.5 * s,
                self.dir(p) * half_width * s,
            ],
            color,
        );
        joint + d * length
    }

    /// A beam between two local points.
    pub fn beam(&self, b: &mut Batch, from: Vec3, to: Vec3, half_width: f32, color: Color) {
        b.beam(
            self.p(from),
            self.p(to),
            half_width * self.scale,
            self.f,
            color,
        );
    }

    pub fn sphere(&self, b: &mut Batch, center: Vec3, radius: f32, color: Color) {
        self.ellipsoid(b, center, Vec3::splat(radius), color);
    }

    pub fn ellipsoid(&self, b: &mut Batch, center: Vec3, radii: Vec3, color: Color) {
        let s = self.scale;
        b.ellipsoid_axes(
            self.p(center),
            [
                self.r * radii.x * s,
                self.u * radii.y * s,
                self.f * radii.z * s,
            ],
            color,
        );
    }

    pub fn glow(&self, b: &mut Batch, center: Vec3, radius: f32, color: Color) {
        b.lit(|b| self.sphere(b, center, radius, color));
    }

    pub fn cone(&self, b: &mut Batch, base: Vec3, height: f32, radius: f32, color: Color) {
        b.cone_ref(
            self.p(base),
            self.u * height * self.scale,
            self.f,
            radius * self.scale,
            0.0,
            10,
            color,
        );
    }

    /// A cone pointing along a local direction.
    pub fn cone_dir(&self, b: &mut Batch, base: Vec3, dir: Vec3, radius: f32, color: Color) {
        b.cone_ref(
            self.p(base),
            self.dir(dir) * self.scale,
            self.f,
            radius * self.scale,
            0.0,
            6,
            color,
        );
    }

    pub fn cylinder(&self, b: &mut Batch, base: Vec3, height: f32, radius: f32, color: Color) {
        let r = radius * self.scale;
        b.cone_ref(
            self.p(base),
            self.u * height * self.scale,
            self.f,
            r,
            r,
            10,
            color,
        );
    }

    /// A cylinder along a local direction.
    pub fn cylinder_dir(&self, b: &mut Batch, base: Vec3, dir: Vec3, radius: f32, color: Color) {
        let r = radius * self.scale;
        b.cone_ref(
            self.p(base),
            self.dir(dir) * self.scale,
            self.u,
            r,
            r,
            10,
            color,
        );
    }
}
