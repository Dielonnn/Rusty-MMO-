//! Color helpers shared by everything that draws.

use macroquad::prelude::*;

pub const fn c(r: f32, g: f32, b: f32) -> Color {
    Color::new(r, g, b, 1.0)
}

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

pub fn rgb((r, g, b): (f32, f32, f32)) -> Color {
    Color::new(r, g, b, 1.0)
}

/// Darker (or, above 1, lighter) version of a color.
pub fn dark(c: Color, f: f32) -> Color {
    Color::new(
        (c.r * f).min(1.0),
        (c.g * f).min(1.0),
        (c.b * f).min(1.0),
        c.a,
    )
}
