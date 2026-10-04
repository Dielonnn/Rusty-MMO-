//! What humanoids wear and carry: pauldrons, shields, weapons and armor.

use super::*;

/// Where the hands ended up, and other measurements gear hangs off.
pub(super) struct Hands {
    pub(super) left: Vec3,
    pub(super) right: Vec3,
    pub(super) left_angle: f32,
    pub(super) right_angle: f32,
    pub(super) shoulder_y: f32,
    pub(super) sw: f32,
    pub(super) y: f32,
    pub(super) chest_w: f32,
}

pub(super) fn sword(b: &mut Batch, fr: &Frame, hand: Vec3, angle: f32, length: f32) {
    let guard = fr.limb(b, hand, angle, 0.025, 0.12, WOOD);
    fr.limb(b, guard, angle, 0.13, 0.04, GOLD);
    let tip = fr.limb(b, guard, angle, 0.04, length, STEEL);
    // A bright edge along the blade.
    fr.limb(b, guard, angle, 0.012, length * 0.98, c(0.92, 0.94, 0.98));
    let _ = tip;
}

pub(super) fn shield(b: &mut Batch, fr: &Frame, hand: Vec3, face: Color, trim: Color) {
    fr.cube(
        b,
        hand + vec3(-0.07, 0.05, 0.13),
        vec3(0.04, 0.33, 0.27),
        face,
    );
    fr.cube(
        b,
        hand + vec3(-0.11, 0.05, 0.13),
        vec3(0.015, 0.34, 0.03),
        trim,
    );
    fr.cube(
        b,
        hand + vec3(-0.11, 0.05, 0.13),
        vec3(0.015, 0.03, 0.28),
        trim,
    );
    fr.sphere(b, hand + vec3(-0.12, 0.05, 0.13), 0.06, trim);
}

/// A big pauldron on each shoulder, in the chunky style of the classic
/// fantasy MMOs: a dome, a rim, and (`spikes` > 0) spikes on top.
pub(super) fn pauldron(
    b: &mut Batch,
    fr: &Frame,
    h: &Hands,
    size: f32,
    color: Color,
    rim: Option<Color>,
    spikes: u32,
) {
    for sx in [-1.0f32, 1.0] {
        let at = vec3(h.sw * sx * 1.05, h.shoulder_y + 0.07, 0.0);
        fr.ellipsoid(b, at, vec3(size, size * 0.62, size), color);
        if let Some(rim) = rim {
            fr.ellipsoid(
                b,
                at - vec3(0.0, size * 0.25, 0.0),
                vec3(size * 1.06, size * 0.18, size * 1.06),
                rim,
            );
        }
        for k in 0..spikes {
            let a = (k as f32 - (spikes as f32 - 1.0) / 2.0) * 0.45;
            fr.cone_dir(
                b,
                at + vec3(sx * size * 0.3, size * 0.45, a * size),
                vec3(sx * size * 0.5, size * 1.1, 0.0),
                size * 0.22,
                dark(color, 0.85),
            );
        }
    }
}

/// Shoulders for everyone who doesn't already wear something there.
pub(super) fn shoulders(
    b: &mut Batch,
    fr: &Frame,
    outfit: Outfit,
    h: &Hands,
    look: &Look,
    pose: Pose,
) {
    // Worn chest armor colors the lighter pauldrons.
    let chest = look.gear[Slot::Chest.index()].map(|id| rgb(item(id).color));
    match outfit {
        Outfit::Class(class) => match class {
            Class::Fighter => pauldron(b, fr, h, 0.21, STEEL, Some(c(0.18, 0.3, 0.6)), 0),
            Class::Paladin => pauldron(b, fr, h, 0.24, c(0.9, 0.9, 0.94), Some(GOLD), 0),
            Class::Rogue => {
                let col = chest.unwrap_or(c(0.18, 0.18, 0.2));
                pauldron(b, fr, h, 0.14, col, Some(LEATHER), 0)
            }
            Class::Ranger => {
                let col = chest.unwrap_or(c(0.36, 0.26, 0.16));
                pauldron(b, fr, h, 0.16, col, Some(c(0.25, 0.35, 0.2)), 0);
                // A feather on the right shoulder.
                fr.beam(
                    b,
                    vec3(h.sw * 1.1, h.shoulder_y + 0.15, -0.05),
                    vec3(h.sw * 1.3, h.shoulder_y + 0.45, -0.15),
                    0.02,
                    c(0.85, 0.3, 0.2),
                );
            }
            Class::Artificer => pauldron(
                b,
                fr,
                h,
                0.17,
                c(0.7, 0.55, 0.25),
                Some(c(0.35, 0.33, 0.32)),
                0,
            ),
            Class::Mage => pauldron(b, fr, h, 0.15, c(0.3, 0.2, 0.65), Some(GOLD), 0),
            Class::Cleric => pauldron(b, fr, h, 0.15, c(0.95, 0.93, 0.86), Some(GOLD), 0),
            Class::Sorcerer => {
                pauldron(b, fr, h, 0.15, c(0.55, 0.1, 0.14), Some(GOLD), 0);
                for sx in [-1.0f32, 1.0] {
                    let pulse = 0.04 + (pose.time * 3.0 + sx).sin().abs() * 0.015;
                    let at = vec3(h.sw * sx * 1.05, h.shoulder_y + 0.17, 0.0);
                    fr.glow(b, at, pulse, c(1.0, 0.45, 0.3));
                }
            }
            Class::Warlock => {
                pauldron(
                    b,
                    fr,
                    h,
                    0.17,
                    c(0.14, 0.1, 0.16),
                    Some(c(0.35, 0.1, 0.4)),
                    2,
                );
                for sx in [-1.0f32, 1.0] {
                    fr.sphere(
                        b,
                        vec3(h.sw * sx * 1.05, h.shoulder_y + 0.05, 0.16),
                        0.06,
                        BONE,
                    );
                }
            }
            Class::Druid => {
                for sx in [-1.0f32, 1.0] {
                    for k in 0..3 {
                        let a = k as f32 * 0.9 - 0.9;
                        fr.ellipsoid(
                            b,
                            vec3(
                                h.sw * sx * 1.05 + a.sin() * 0.06,
                                h.shoulder_y + 0.1,
                                a.cos() * 0.08 - 0.02,
                            ),
                            vec3(0.12, 0.03, 0.08),
                            c(0.35 + k as f32 * 0.05, 0.6, 0.28),
                        );
                    }
                }
            }
            Class::Bard => {
                // Puffed, slashed sleeves.
                for sx in [-1.0f32, 1.0] {
                    let at = vec3(h.sw * sx * 1.05, h.shoulder_y, 0.0);
                    fr.ellipsoid(b, at, vec3(0.14, 0.12, 0.14), c(0.15, 0.55, 0.6));
                    fr.cube(
                        b,
                        at + vec3(sx * 0.08, 0.0, 0.0),
                        vec3(0.01, 0.1, 0.06),
                        c(0.6, 0.18, 0.45),
                    );
                }
            }
            // Barbarians wear fur, and monks go bare-shouldered.
            Class::Barbarian | Class::Monk => {}
        },
        Outfit::Mob(style, colors) => match style {
            HumanoidStyle::Bandit | HumanoidStyle::Raider => {
                pauldron(b, fr, h, 0.14, LEATHER, Some(dark(colors[0], 0.8)), 0)
            }
            HumanoidStyle::Troll | HumanoidStyle::TrollShaman => {
                pauldron(b, fr, h, 0.17, BONE, None, 2)
            }
            HumanoidStyle::Skeleton => pauldron(
                b,
                fr,
                h,
                0.15,
                c(0.45, 0.32, 0.22),
                Some(c(0.35, 0.3, 0.28)),
                1,
            ),
            HumanoidStyle::Trogg | HumanoidStyle::TroggShaman => {
                pauldron(b, fr, h, 0.16, c(0.35, 0.33, 0.35), None, 3)
            }
            HumanoidStyle::Necromancer => {
                pauldron(b, fr, h, 0.15, c(0.15, 0.12, 0.16), Some(BONE), 2)
            }
            _ => {}
        },
        Outfit::Giant(style, colors) => {
            // Great slabs of armor chained across the chest.
            if !matches!(style, GiantStyle::Yeti | GiantStyle::Treant) {
                let plate = dark(colors[0], 0.8);
                pauldron(b, fr, h, 0.24, plate, Some(dark(plate, 0.7)), 2);
                fr.beam(
                    b,
                    vec3(-h.sw, h.shoulder_y - 0.05, 0.18),
                    vec3(h.sw * 0.6, 1.05 + h.y, 0.18),
                    0.025,
                    c(0.4, 0.4, 0.42),
                );
            }
        }
        Outfit::Merchant | Outfit::QuestGiver => {}
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn gear_and_weapons(
    b: &mut Batch,
    fr: &Frame,
    outfit: Outfit,
    h: &Hands,
    look: &Look,
    pose: Pose,
    legs: Color,
) {
    let (rhand, lhand) = (h.right, h.left);
    let (wa, la) = (h.right_angle, h.left_angle);
    let y = h.y;
    match outfit {
        Outfit::Class(class) => match class {
            Class::Barbarian => {
                // A fur mantle and a great two-handed axe.
                for sx in [-1.0, 1.0] {
                    fr.ellipsoid(
                        b,
                        vec3(h.sw * sx, h.shoulder_y + 0.05, -0.02),
                        vec3(0.17, 0.1, 0.17),
                        c(0.55, 0.45, 0.35),
                    );
                    fr.ellipsoid(
                        b,
                        vec3(h.sw * sx * 0.9, h.shoulder_y + 0.1, -0.05),
                        vec3(0.12, 0.06, 0.12),
                        c(0.65, 0.55, 0.45),
                    );
                }
                fr.cube(
                    b,
                    vec3(0.0, 1.4 + y, 0.175),
                    vec3(0.03, 0.15, 0.01),
                    LEATHER,
                );
                let haft = fr.limb(b, rhand, wa, 0.035, 1.2, WOOD);
                // A double-bitted head.
                fr.limb(
                    b,
                    haft - vec3(0.0, 0.0, 0.0),
                    wa,
                    0.04,
                    0.08,
                    dark(STEEL, 0.7),
                );
                fr.tilted(
                    b,
                    haft + vec3(0.0, 0.0, 0.0),
                    vec3(0.025, 0.2, 0.15),
                    wa - 1.57,
                    STEEL,
                );
                fr.tilted(b, haft, vec3(0.03, 0.06, 0.16), wa - 1.57, dark(STEEL, 0.8));
            }
            Class::Fighter => {
                for sx in [-1.0, 1.0] {
                    fr.ellipsoid(
                        b,
                        vec3(h.sw * sx, h.shoulder_y + 0.06, 0.0),
                        vec3(0.15, 0.08, 0.15),
                        STEEL,
                    );
                }
                fr.cube(
                    b,
                    vec3(0.0, 1.25 + y, 0.175),
                    vec3(0.14, 0.32, 0.01),
                    c(0.18, 0.3, 0.6),
                );
                fr.cube(b, vec3(0.0, 1.38 + y, 0.18), vec3(0.06, 0.06, 0.005), GOLD);
                sword(b, fr, rhand, wa, 0.85);
                shield(b, fr, lhand, c(0.18, 0.3, 0.6), STEEL);
            }
            Class::Paladin => {
                for sx in [-1.0, 1.0] {
                    fr.ellipsoid(
                        b,
                        vec3(h.sw * sx, h.shoulder_y + 0.06, 0.0),
                        vec3(0.16, 0.09, 0.16),
                        c(0.88, 0.88, 0.92),
                    );
                    fr.ellipsoid(
                        b,
                        vec3(h.sw * sx, h.shoulder_y + 0.04, 0.0),
                        vec3(0.17, 0.04, 0.17),
                        GOLD,
                    );
                }
                fr.cube(b, vec3(0.0, 1.3 + y, 0.175), vec3(0.04, 0.2, 0.01), GOLD);
                fr.cube(b, vec3(0.0, 1.38 + y, 0.18), vec3(0.12, 0.035, 0.01), GOLD);
                let head = fr.limb(b, rhand, wa, 0.03, 0.7, WOOD);
                fr.cube(b, head, vec3(0.1, 0.1, 0.16), STEEL);
                fr.cube(b, head, vec3(0.105, 0.03, 0.165), GOLD);
                if pose.casting {
                    fr.glow(b, head, 0.18, Color::new(1.0, 0.95, 0.6, 0.6));
                }
                shield(b, fr, lhand, c(0.9, 0.88, 0.8), GOLD);
            }
            Class::Monk => {
                let red = c(0.85, 0.15, 0.12);
                fr.cube(
                    b,
                    vec3(0.0, 1.07 + y, 0.0),
                    vec3(h.chest_w - 0.04, 0.05, 0.16),
                    red,
                );
                let flutter = (pose.time * 4.0).sin() * 0.08;
                fr.tilted(
                    b,
                    vec3(0.12, 0.9 + y, 0.15),
                    vec3(0.04, 0.15, 0.01),
                    0.1 + flutter,
                    red,
                );
                fr.sphere(b, rhand, 0.085, c(0.9, 0.88, 0.8));
                fr.sphere(b, lhand, 0.085, c(0.9, 0.88, 0.8));
                // Prayer beads.
                for k in 0..7 {
                    let a = k as f32 * 0.45 - 1.35;
                    fr.sphere(
                        b,
                        vec3(a.sin() * 0.16, 1.45 + y - a.cos() * 0.06, 0.17),
                        0.022,
                        c(0.45, 0.28, 0.15),
                    );
                }
            }
            Class::Rogue => {
                fr.cube(
                    b,
                    vec3(0.0, 1.36 + y, 0.0),
                    vec3(h.chest_w + 0.01, 0.025, 0.175),
                    LEATHER,
                );
                // Throwing knives on the bandolier.
                for k in 0..3 {
                    fr.cube(
                        b,
                        vec3(-0.12 + k as f32 * 0.08, 1.36 + y, 0.18),
                        vec3(0.012, 0.05, 0.006),
                        STEEL,
                    );
                }
                for (hand, angle) in [(rhand, wa), (lhand, la)] {
                    let hilt = fr.limb(b, hand, angle, 0.025, 0.1, c(0.2, 0.12, 0.08));
                    fr.limb(b, hilt, angle, 0.08, 0.03, STEEL);
                    fr.limb(b, hilt, angle, 0.03, 0.42, c(0.82, 0.84, 0.88));
                }
            }
            Class::Ranger => {
                // A longbow in the left hand and a quiver on the back. The
                // string pulls back to the right hand while drawing.
                let top = lhand + vec3(-0.02, 0.75, 0.05);
                let bottom = lhand + vec3(-0.02, -0.75, 0.05);
                let bend = lhand + vec3(-0.02, 0.0, 0.22);
                fr.beam(b, top, bend, 0.025, WOOD);
                fr.beam(b, bend, bottom, 0.025, WOOD);
                let drawing = pose.casting || (pose.swing > 0.0 && pose.swing < 0.6);
                let string = if drawing {
                    rhand
                } else {
                    lhand + vec3(-0.02, 0.0, 0.0)
                };
                fr.beam(b, top, string, 0.006, c(0.9, 0.9, 0.85));
                fr.beam(b, string, bottom, 0.006, c(0.9, 0.9, 0.85));
                if drawing {
                    let tip = rhand + (lhand - rhand).normalize_or_zero() * 0.9;
                    fr.beam(b, rhand, tip, 0.012, WOOD);
                    fr.cone_dir(
                        b,
                        tip,
                        (lhand - rhand).normalize_or_zero() * 0.08,
                        0.025,
                        STEEL,
                    );
                }
                fr.tilted(
                    b,
                    vec3(0.12, 1.35 + y, -0.22),
                    vec3(0.07, 0.25, 0.06),
                    -0.35,
                    LEATHER,
                );
                for k in 0..3 {
                    let x = 0.08 + k as f32 * 0.04;
                    fr.tilted(
                        b,
                        vec3(x, 1.65 + y, -0.32),
                        vec3(0.012, 0.08, 0.012),
                        -0.35,
                        c(0.85, 0.2, 0.15),
                    );
                }
            }
            Class::Artificer => {
                fr.cube(
                    b,
                    vec3(0.0, 1.32 + y, -0.27),
                    vec3(0.2, 0.22, 0.1),
                    c(0.45, 0.35, 0.22),
                );
                for sx in [-1.0, 1.0] {
                    fr.cylinder(
                        b,
                        vec3(0.12 * sx, 1.5 + y, -0.3),
                        0.3,
                        0.04,
                        c(0.7, 0.55, 0.25),
                    );
                    // Puffs of steam from the pipes.
                    let t = (pose.time * 0.8 + sx).fract();
                    fr.glow(
                        b,
                        vec3(0.12 * sx, 1.85 + y + t * 0.4, -0.3),
                        0.04 + t * 0.06,
                        Color::new(0.9, 0.9, 0.9, 0.5 * (1.0 - t)),
                    );
                }
                fr.glow(b, vec3(0.0, 1.35 + y, -0.38), 0.06, c(0.5, 0.85, 1.0));
                let stock = fr.limb(b, rhand, wa, 0.05, 0.25, WOOD);
                let muzzle = fr.limb(b, stock, wa, 0.03, 0.55, c(0.35, 0.35, 0.38));
                fr.limb(b, stock, wa, 0.045, 0.12, c(0.7, 0.55, 0.25));
                if pose.swing > 0.3 && pose.swing < 0.45 {
                    fr.glow(b, muzzle, 0.12, c(0.6, 0.9, 1.0));
                }
            }
            Class::Bard => {
                // A lute held across the body.
                let neck_end = lhand + vec3(0.25, 0.6, 0.15);
                fr.ellipsoid(
                    b,
                    rhand + vec3(-0.08, 0.05, 0.08),
                    vec3(0.16, 0.2, 0.06),
                    c(0.6, 0.38, 0.18),
                );
                fr.sphere(b, rhand + vec3(-0.08, 0.05, 0.14), 0.05, c(0.2, 0.12, 0.08));
                fr.beam(
                    b,
                    rhand + vec3(-0.08, 0.2, 0.1),
                    neck_end,
                    0.025,
                    c(0.35, 0.22, 0.12),
                );
                fr.cube(b, vec3(0.0, 1.3 + y, 0.175), vec3(0.04, 0.2, 0.01), GOLD);
                // Notes drifting up while playing.
                if pose.casting || (pose.swing > 0.0 && pose.swing < 1.0) {
                    for k in 0..3 {
                        let t = (pose.time * 0.7 + k as f32 / 3.0).fract();
                        let p = rhand + vec3((t * 9.0).sin() * 0.2, 0.3 + t * 1.2, 0.2);
                        fr.glow(b, p, 0.04, Color::new(0.95, 0.6, 1.0, 1.0 - t));
                    }
                }
            }
            Class::Cleric => {
                fr.cube(b, vec3(0.0, 1.27 + y, 0.175), vec3(0.07, 0.3, 0.01), GOLD);
                fr.cube(b, vec3(0.0, 1.38 + y, 0.18), vec3(0.14, 0.035, 0.01), GOLD);
                let head = fr.limb(b, rhand, wa, 0.03, 0.6, WOOD);
                fr.sphere(b, head, 0.11, c(0.82, 0.82, 0.85));
                for k in 0..4 {
                    let a = k as f32 * 1.57;
                    fr.cone_dir(
                        b,
                        head,
                        vec3(a.cos() * 0.15, 0.0, a.sin() * 0.15),
                        0.03,
                        c(0.82, 0.82, 0.85),
                    );
                }
            }
            Class::Druid => {
                fr.cube(
                    b,
                    rhand + vec3(0.0, 0.25, 0.0),
                    vec3(0.035, 0.95, 0.035),
                    c(0.45, 0.32, 0.2),
                );
                for k in 0..4 {
                    let a = k as f32 * 1.57 + pose.time * 0.3;
                    fr.sphere(
                        b,
                        rhand + vec3(a.cos() * 0.08, 1.2, a.sin() * 0.08),
                        0.07,
                        c(0.35, 0.65, 0.25),
                    );
                }
                fr.glow(b, rhand + vec3(0.0, 1.25, 0.0), 0.04, c(0.8, 1.0, 0.6));
                // Leaves swirling while casting.
                if pose.casting {
                    for k in 0..6 {
                        let a = pose.time * 2.5 + k as f32 * 1.05;
                        let p = vec3(
                            a.cos() * 0.7,
                            0.8 + y + (a * 0.7).sin() * 0.5,
                            a.sin() * 0.7,
                        );
                        fr.cube(b, p, vec3(0.05, 0.01, 0.03), c(0.4, 0.75, 0.3));
                    }
                }
            }
            Class::Mage => {
                fr.cube(b, vec3(0.0, 1.27 + y, 0.175), vec3(0.04, 0.27, 0.01), GOLD);
                fr.cube(b, rhand + vec3(0.0, 0.3, 0.0), vec3(0.03, 0.95, 0.03), WOOD);
                fr.cube(
                    b,
                    rhand + vec3(0.0, 1.18, 0.0),
                    vec3(0.06, 0.04, 0.06),
                    GOLD,
                );
                let orb = if pose.casting { 0.14 } else { 0.1 };
                fr.glow(b, rhand + vec3(0.0, 1.32, 0.0), orb, c(0.55, 0.85, 1.0));
            }
            Class::Sorcerer => {
                fr.cube(b, vec3(0.0, 1.27 + y, 0.175), vec3(0.04, 0.27, 0.01), GOLD);
                // Orbs of wild magic circling, faster while casting.
                let speed = if pose.casting { 6.0 } else { 2.0 };
                for k in 0..3 {
                    let a = pose.time * speed + k as f32 * 2.09;
                    let p = vec3(
                        a.cos() * 0.55,
                        1.5 + y + (a * 2.0).sin() * 0.1,
                        a.sin() * 0.55,
                    );
                    let col = [c(1.0, 0.4, 0.2), c(0.6, 0.4, 1.0), c(0.3, 0.9, 1.0)][k];
                    fr.glow(b, p, 0.07, col);
                }
            }
            Class::Warlock => {
                fr.cube(b, vec3(0.18, 1.0 + y, 0.13), vec3(0.06, 0.07, 0.06), BONE);
                // A grimoire on a chain at the hip.
                fr.cube(
                    b,
                    vec3(-0.22, 0.98 + y, 0.05),
                    vec3(0.03, 0.1, 0.08),
                    c(0.3, 0.1, 0.25),
                );
                let flame = Color::new(0.4, 1.0, 0.3, 0.9);
                let flicker = (pose.time * 7.0).sin().abs();
                fr.glow(
                    b,
                    rhand + vec3(0.0, 0.05, 0.1),
                    0.08 + flicker * 0.02,
                    flame,
                );
                fr.glow(
                    b,
                    rhand + vec3(0.0, 0.17 + flicker * 0.05, 0.1),
                    0.04,
                    flame,
                );
                fr.glow(b, lhand + vec3(0.0, 0.05, 0.1), 0.06, flame);
            }
        },
        Outfit::QuestGiver => {
            // A tabard with the zone's emblem, steel pauldrons, and a scroll.
            fr.cube(
                b,
                vec3(0.0, 1.15 + y, 0.175),
                vec3(0.16, 0.38, 0.01),
                c(0.22, 0.28, 0.5),
            );
            fr.cube(b, vec3(0.0, 1.32 + y, 0.18), vec3(0.07, 0.07, 0.006), GOLD);
            fr.cube(b, vec3(0.0, 0.8 + y, 0.18), vec3(0.165, 0.02, 0.008), GOLD);
            pauldron(b, fr, h, 0.2, STEEL, Some(GOLD), 0);
            fr.cylinder_dir(
                b,
                lhand + vec3(-0.15, 0.02, 0.08),
                vec3(0.3, 0.0, 0.0),
                0.05,
                c(0.92, 0.88, 0.72),
            );
            for sx in [-1.0, 1.0] {
                fr.sphere(
                    b,
                    lhand + vec3(sx * 0.16, 0.02, 0.08),
                    0.06,
                    c(0.55, 0.35, 0.2),
                );
            }
            fr.cube(
                b,
                vec3(-0.22, 0.98 + y, 0.06),
                vec3(0.03, 0.1, 0.08),
                c(0.45, 0.15, 0.12),
            );
        }
        Outfit::Merchant => {
            fr.cube(
                b,
                vec3(0.0, 1.05 + y, 0.17),
                vec3(0.2, 0.3, 0.01),
                c(0.88, 0.85, 0.75),
            );
            fr.sphere(b, vec3(0.2, 1.0 + y, 0.12), 0.07, c(0.55, 0.4, 0.2));
            fr.cube(
                b,
                vec3(-0.12, 1.12 + y, 0.18),
                vec3(0.04, 0.05, 0.005),
                c(0.6, 0.6, 0.6),
            );
        }
        Outfit::Mob(style, colors) => match style {
            HumanoidStyle::Bandit | HumanoidStyle::Raider | HumanoidStyle::Trogg => {
                let hilt = fr.limb(b, rhand, wa, 0.025, 0.1, WOOD);
                if style == HumanoidStyle::Trogg || look.seed.is_multiple_of(2) {
                    let end = fr.limb(b, hilt, wa, 0.07, 0.6, c(0.35, 0.24, 0.14));
                    fr.sphere(b, end, 0.08, c(0.3, 0.2, 0.12));
                } else {
                    fr.limb(b, hilt, wa, 0.03, 0.5, STEEL);
                }
            }
            HumanoidStyle::Satyr => {
                let hilt = fr.limb(b, rhand, wa, 0.025, 0.1, WOOD);
                fr.limb(b, hilt, wa, 0.03, 0.55, STEEL);
                fr.limb(b, lhand, la, 0.025, 0.4, STEEL);
            }
            HumanoidStyle::Troll => {
                let haft = fr.limb(b, rhand, wa, 0.03, 0.9, WOOD);
                fr.cube(b, haft, vec3(0.03, 0.15, 0.12), c(0.75, 0.85, 0.95));
            }
            HumanoidStyle::Skeleton => {
                let hilt = fr.limb(b, rhand, wa, 0.025, 0.1, WOOD);
                fr.limb(b, hilt, wa, 0.035, 0.6, c(0.55, 0.42, 0.3));
                shield(b, fr, lhand, c(0.35, 0.3, 0.28), c(0.55, 0.42, 0.3));
            }
            HumanoidStyle::Mystic
            | HumanoidStyle::Trickster
            | HumanoidStyle::Necromancer
            | HumanoidStyle::Shaman
            | HumanoidStyle::TrollShaman
            | HumanoidStyle::TroggShaman => {
                let pulse = 0.08 + (pose.time * 3.0).sin().abs() * 0.03;
                let g = colors[2];
                fr.glow(b, rhand + vec3(0.0, 0.05, 0.12), pulse, g);
                if matches!(
                    style,
                    HumanoidStyle::Necromancer | HumanoidStyle::Shaman | HumanoidStyle::TrollShaman
                ) {
                    fr.cube(b, lhand + vec3(0.0, 0.3, 0.0), vec3(0.03, 0.95, 0.03), WOOD);
                    let top = if style == HumanoidStyle::Necromancer {
                        BONE
                    } else {
                        g
                    };
                    fr.sphere(b, lhand + vec3(0.0, 1.3, 0.0), 0.1, top);
                } else {
                    fr.glow(b, lhand + vec3(0.0, 0.05, 0.12), pulse * 0.7, g);
                }
            }
        },
        Outfit::Giant(style, colors) => {
            fr.cube(b, lhand, vec3(0.16, 0.16, 0.16), legs);
            fr.cube(b, rhand, vec3(0.16, 0.16, 0.16), legs);
            for sx in [-1.0, 1.0] {
                fr.cube(
                    b,
                    vec3(h.sw * sx, h.shoulder_y + 0.08, 0.0),
                    vec3(0.16, 0.12, 0.16),
                    dark(legs, 0.9),
                );
            }
            let accent = colors[2];
            match style {
                GiantStyle::Stone | GiantStyle::Sandstone => {
                    let moss = if style == GiantStyle::Stone {
                        c(0.32, 0.45, 0.2)
                    } else {
                        dark(colors[0], 0.8)
                    };
                    fr.cube(
                        b,
                        vec3(-0.15, h.shoulder_y + 0.21, -0.05),
                        vec3(0.12, 0.03, 0.1),
                        moss,
                    );
                    let rune = Color::new(
                        accent.r,
                        accent.g,
                        accent.b,
                        0.6 + 0.4 * (pose.time * 2.0).sin().abs(),
                    );
                    fr.glow(b, vec3(0.0, 1.4 + y, 0.18), 0.07, rune);
                    for (x, yy) in [(-0.15, 1.25), (0.15, 1.25), (0.0, 1.17)] {
                        b.lit(|b| {
                            fr.cube(b, vec3(x, yy + y, 0.172), vec3(0.03, 0.03, 0.005), rune)
                        });
                    }
                    // Cracks.
                    fr.cube(
                        b,
                        vec3(0.1, 1.05 + y, 0.155),
                        vec3(0.008, 0.08, 0.004),
                        dark(colors[0], 0.5),
                    );
                }
                GiantStyle::Treant => {
                    for sx in [-1.0, 1.0] {
                        let from = vec3(h.sw * sx, h.shoulder_y, 0.0);
                        let to = vec3(h.sw * sx * 1.8, h.shoulder_y + 0.5, -0.1);
                        fr.beam(b, from, to, 0.04, colors[0]);
                        fr.sphere(b, to + vec3(0.03 * sx, 0.1, 0.0), 0.15, accent);
                    }
                }
                GiantStyle::Crystal => {
                    b.lit(|b| {
                        for k in 0..5 {
                            let x = (k as f32 - 2.0) * 0.12;
                            fr.cone_dir(
                                b,
                                vec3(x, 1.45 + y, -0.15),
                                vec3(x * 0.5, 0.35, -0.2),
                                0.06,
                                accent,
                            );
                        }
                        for sx in [-1.0, 1.0] {
                            fr.cone_dir(
                                b,
                                vec3(h.sw * sx, h.shoulder_y + 0.15, 0.0),
                                vec3(0.1 * sx, 0.3, 0.0),
                                0.07,
                                accent,
                            );
                        }
                    });
                }
                GiantStyle::Yeti => {
                    fr.ellipsoid(
                        b,
                        vec3(0.0, 1.35 + y, 0.0),
                        vec3(0.36, 0.3, 0.24),
                        colors[0],
                    );
                    for sx in [-1.0, 1.0] {
                        fr.ellipsoid(
                            b,
                            vec3(h.sw * sx, h.shoulder_y + 0.05, 0.0),
                            vec3(0.2, 0.15, 0.2),
                            colors[0],
                        );
                    }
                    // Icicles in the fur.
                    for k in 0..4 {
                        fr.cone_dir(
                            b,
                            vec3(-0.15 + k as f32 * 0.1, 1.1 + y, 0.2),
                            vec3(0.0, -0.12, 0.0),
                            0.02,
                            c(0.8, 0.9, 1.0),
                        );
                    }
                }
                GiantStyle::Bone => {
                    for k in 0..4 {
                        fr.cone_dir(
                            b,
                            vec3(0.0, 1.55 - k as f32 * 0.12 + y, -0.1),
                            vec3(0.0, 0.05, -0.2),
                            0.04,
                            colors[0],
                        );
                    }
                    fr.glow(b, vec3(0.0, 1.3 + y, 0.05), 0.08, accent);
                }
            }
            // A slam shakes up dust.
            if pose.swing > 0.4 && pose.swing < 0.75 {
                let t = (pose.swing - 0.4) / 0.35;
                for k in 0..6 {
                    let a = k as f32 * 1.05;
                    let p = vec3(a.cos(), 0.1, a.sin() + 0.8) * (0.3 + t * 0.6);
                    fr.glow(
                        b,
                        p,
                        0.1 + t * 0.1,
                        Color::new(0.7, 0.65, 0.55, 0.6 * (1.0 - t)),
                    );
                }
            }
        }
    }
}
