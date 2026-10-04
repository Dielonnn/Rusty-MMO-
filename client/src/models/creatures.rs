//! Beasts: wolves, boars, spiders and scorpions.

use super::*;

/// An animal's frame: fallen when dead, rearing (pitched) when attacking.
pub(super) fn beast_frame(pos: Vec3, yaw: f32, pose: Pose, rear: f32) -> Frame {
    if pose.dead {
        Frame::fallen(pos, yaw, 1.0, true)
    } else {
        Frame::leaning(pos, yaw, 1.0, -rear - pose.hurt * 0.12)
    }
}

pub(super) fn wolf(b: &mut Batch, pos: Vec3, yaw: f32, colors: [Color; 3], seed: u32, pose: Pose) {
    let bite = if pose.attacking() {
        bump(pose.swing)
    } else {
        0.0
    };
    let fr = beast_frame(pos, yaw, pose, bite * 0.12);
    let tint = (seed % 3) as f32 * 0.04;
    let fur = Color::new(colors[0].r + tint, colors[0].g + tint, colors[0].b, 1.0);
    let back = colors[1];
    let belly = mix(colors[0], c(0.85, 0.82, 0.78), 0.5);
    let moving = pose.moving && !pose.dead;
    let gait = if moving { pose.walk } else { 0.0 };
    let lunge = bite * 0.3;
    let breathe = (pose.time * 2.0).sin() * 0.01;
    let y = 0.78
        + if moving {
            (gait * 2.0).sin().abs() * 0.05
        } else {
            breathe
        };
    fr.cube(b, vec3(0.0, y, 0.22), vec3(0.23, 0.24 + breathe, 0.32), fur);
    fr.cube(b, vec3(0.0, y + 0.02, -0.32), vec3(0.19, 0.2, 0.28), fur);
    fr.cube(b, vec3(0.0, y + 0.23, -0.05), vec3(0.17, 0.05, 0.55), back);
    fr.cube(b, vec3(0.0, y - 0.2, 0.15), vec3(0.17, 0.06, 0.3), belly);
    fr.cube(b, vec3(0.0, y + 0.08, 0.5), vec3(0.25, 0.25, 0.1), back);
    // A shaggy mane.
    for k in 0..5 {
        let z = 0.55 - k as f32 * 0.12;
        fr.cone_dir(
            b,
            vec3(0.0, y + 0.25, z),
            vec3(0.0, 0.12, -0.1),
            0.06,
            dark(back, 0.85),
        );
        for sx in [-1.0, 1.0] {
            fr.cone_dir(
                b,
                vec3(0.2 * sx, y + 0.12, z),
                vec3(0.08 * sx, 0.02, -0.1),
                0.05,
                dark(fur, 0.9),
            );
        }
    }
    // Front legs move together at a gallop, the hind legs half a beat behind.
    for (x, z, phase) in [
        (-1.0, 1.0, 0.0),
        (1.0, 1.0, 0.5),
        (-1.0, -1.0, 3.1),
        (1.0, -1.0, 3.6),
    ] {
        let swing = if moving {
            (gait + phase).sin() * 0.6
        } else {
            0.0
        };
        let bend = if moving {
            (gait + phase + 1.0).sin().max(0.0) * 0.7
        } else {
            0.0
        };
        let hip = vec3(x * 0.15, y - 0.12, z * 0.4);
        let knee = fr.limb(b, hip, swing, 0.07, 0.36, fur);
        let ankle = fr.limb(b, knee, swing - bend * z, 0.055, 0.32, back);
        fr.cube(
            b,
            ankle + vec3(0.0, 0.0, 0.04),
            vec3(0.06, 0.035, 0.08),
            back,
        );
        for k in 0..3 {
            let cx = (k as f32 - 1.0) * 0.03;
            fr.cone_dir(
                b,
                ankle + vec3(cx, -0.02, 0.11),
                vec3(0.0, -0.02, 0.04),
                0.01,
                c(0.15, 0.13, 0.12),
            );
        }
    }
    // The head sniffs about when idle.
    let sniff = if moving || pose.dead {
        0.0
    } else {
        ((pose.time * 0.6 + seed as f32).sin() * 3.0).clamp(-1.0, 0.0) * 0.12
    };
    let head = vec3(0.0, y + 0.22 + sniff + bite * 0.04, 0.72 + lunge);
    fr.cube(b, head, vec3(0.16, 0.15, 0.17), fur);
    fr.cube(
        b,
        head + vec3(0.0, -0.03, 0.24),
        vec3(0.08, 0.05, 0.13),
        fur,
    );
    // The jaw opens to bite.
    let jaw = 0.25 + bite * 0.6;
    fr.tilted(
        b,
        head + vec3(0.0, -0.1, 0.2),
        vec3(0.07, 0.025, 0.12),
        jaw,
        belly,
    );
    fr.sphere(b, head + vec3(0.0, 0.0, 0.38), 0.04, c(0.08, 0.07, 0.07));
    if bite > 0.2 {
        fr.cube(
            b,
            head + vec3(0.0, -0.08, 0.26),
            vec3(0.05, 0.02, 0.06),
            c(0.6, 0.2, 0.25),
        );
    }
    for sx in [-1.0, 1.0] {
        fr.glow(b, head + vec3(0.08 * sx, 0.05, 0.16), 0.032, colors[2]);
        let ear_twitch = if (pose.time * 1.3 + sx).sin() > 0.95 {
            0.04
        } else {
            0.0
        };
        fr.cone_dir(
            b,
            head + vec3(0.09 * sx, 0.12, -0.04),
            vec3(0.03 * sx, 0.17, -0.03 - ear_twitch),
            0.06,
            back,
        );
        fr.cone_dir(
            b,
            head + vec3(0.05 * sx, -0.08, 0.33),
            vec3(0.0, -0.06, 0.0),
            0.015,
            c(0.95, 0.94, 0.88),
        );
    }
    let wag = (pose.time * if moving { 8.0 } else { 4.0 }).sin() * 0.25;
    let t1 = fr.limb(b, vec3(0.0, y + 0.12, -0.58), 2.2 + wag, 0.07, 0.25, fur);
    let t2 = fr.limb(b, t1, 2.6 + wag * 1.3, 0.09, 0.25, back);
    fr.sphere(b, t2, 0.07, belly);
}

pub(super) fn boar(b: &mut Batch, pos: Vec3, yaw: f32, colors: [Color; 3], seed: u32, pose: Pose) {
    let toss = if pose.attacking() {
        bump(pose.swing)
    } else {
        0.0
    };
    let fr = beast_frame(pos, yaw, pose, toss * 0.1);
    let tint = (seed % 3) as f32 * 0.03;
    let hide = Color::new(colors[0].r + tint, colors[0].g, colors[0].b, 1.0);
    let dark_hide = colors[1];
    let snout = mix(hide, c(0.85, 0.55, 0.5), 0.6);
    let tusk = colors[2];
    let moving = pose.moving && !pose.dead;
    let gait = if moving { pose.walk * 1.3 } else { 0.0 };
    let lunge = toss * 0.3;
    let breathe = (pose.time * 1.8).sin() * 0.012;
    let y = 0.68
        + if moving {
            (gait * 2.0).sin().abs() * 0.03
        } else {
            breathe
        };
    // The body is built along the boar's own axes, so it turns with the boar.
    fr.ellipsoid(b, vec3(0.0, y, 0.0), vec3(0.34, 0.33 + breathe, 0.62), hide);
    fr.ellipsoid(b, vec3(0.0, y + 0.05, 0.38), vec3(0.32, 0.33, 0.3), hide);
    // Darker patches.
    fr.ellipsoid(
        b,
        vec3(0.12, y + 0.1, -0.15),
        vec3(0.24, 0.22, 0.3),
        dark(hide, 0.85),
    );
    for i in 0..8 {
        let z = 0.5 - i as f32 * 0.13;
        let top = y + 0.3 - (i as f32 - 2.5).abs() * 0.015;
        fr.cone_dir(
            b,
            vec3(0.0, top, z),
            vec3(0.0, 0.16, -0.07),
            0.055,
            dark_hide,
        );
    }
    for (x, z, phase) in [
        (-1.0, 1.0, 0.0),
        (1.0, 1.0, 3.1),
        (-1.0, -1.0, 3.1),
        (1.0, -1.0, 0.0),
    ] {
        let swing = if moving {
            (gait + phase).sin() * 0.5
        } else {
            0.0
        };
        let hip = vec3(x * 0.2, y - 0.18, z * 0.38);
        let ankle = fr.limb(b, hip, swing, 0.08, 0.36, hide);
        // Split hooves.
        for sx in [-1.0, 1.0] {
            fr.cube(
                b,
                ankle + vec3(0.03 * sx, 0.0, 0.01),
                vec3(0.03, 0.04, 0.06),
                c(0.15, 0.12, 0.1),
            );
        }
    }
    // Rooting about with the snout when idle.
    let root = if moving || pose.dead {
        0.0
    } else {
        (((pose.time * 0.5 + seed as f32 * 0.3).sin() * 2.0).clamp(-1.0, 0.0)) * 0.15
    };
    let head = vec3(0.0, y - 0.02 + root + toss * 0.15, 0.72 + lunge);
    fr.cube(b, head, vec3(0.19, 0.18, 0.2), hide);
    fr.cylinder_dir(
        b,
        head + vec3(0.0, -0.04, 0.2),
        vec3(0.0, 0.0, 0.1),
        0.09,
        snout,
    );
    for sx in [-1.0, 1.0] {
        fr.sphere(
            b,
            head + vec3(0.035 * sx, -0.04, 0.3),
            0.02,
            c(0.2, 0.1, 0.1),
        );
        fr.sphere(
            b,
            head + vec3(0.12 * sx, 0.07, 0.17),
            0.03,
            c(0.08, 0.06, 0.05),
        );
        fr.cone_dir(
            b,
            head + vec3(0.13 * sx, 0.15, -0.05),
            vec3(0.06 * sx, 0.12, -0.05),
            0.06,
            dark_hide,
        );
        fr.cone_dir(
            b,
            head + vec3(0.12 * sx, -0.12, 0.2),
            vec3(0.05 * sx, 0.1 + toss * 0.05, 0.1),
            0.03,
            tusk,
        );
    }
    let t = fr.limb(b, vec3(0.0, y + 0.15, -0.6), 2.5, 0.025, 0.12, dark_hide);
    fr.limb(
        b,
        t,
        1.3 + (pose.time * 5.0).sin() * 0.3,
        0.025,
        0.1,
        dark_hide,
    );
}

pub(super) fn spider(
    b: &mut Batch,
    pos: Vec3,
    yaw: f32,
    colors: [Color; 3],
    seed: u32,
    pose: Pose,
) {
    let rear = if pose.attacking() {
        bump(pose.swing)
    } else {
        0.0
    };
    let fr = beast_frame(pos, yaw, pose, rear * 0.35);
    let moving = pose.moving && !pose.dead;
    let lunge = rear * 0.2;
    let breathe = (pose.time * 2.2).sin() * 0.015;
    let y = 0.6
        + if moving {
            (pose.walk * 3.0).sin().abs() * 0.03
        } else {
            0.0
        };
    fr.ellipsoid(
        b,
        vec3(0.0, y, 0.15 + lunge),
        vec3(0.3, 0.22, 0.32),
        colors[0],
    );
    fr.ellipsoid(
        b,
        vec3(0.0, y + 0.15, -0.5),
        vec3(0.45, 0.38 + breathe, 0.55),
        colors[1],
    );
    // Markings and bristles on the abdomen.
    fr.ellipsoid(
        b,
        vec3(0.0, y + 0.5, -0.5),
        vec3(0.12, 0.04, 0.2),
        colors[2],
    );
    for k in 0..6 {
        let a = k as f32 * 1.05;
        fr.cone_dir(
            b,
            vec3(a.cos() * 0.3, y + 0.35, -0.5 + a.sin() * 0.35),
            vec3(a.cos() * 0.05, 0.1, a.sin() * 0.05),
            0.02,
            dark(colors[1], 0.6),
        );
    }
    for k in 0..4 {
        for sx in [-1.0f32, 1.0] {
            let phase = k as f32 * 1.6 + if sx > 0.0 { 3.1 } else { 0.0 };
            let lift = if moving {
                (pose.walk * 1.5 + phase).sin().max(0.0) * 0.2
            } else {
                // An idle twitch now and then.
                ((pose.time * 1.7 + phase + seed as f32).sin() - 0.9).max(0.0) * 1.2
            };
            let z = 0.35 - k as f32 * 0.22;
            // The front legs rise to strike.
            let raise = if k == 0 { rear * 0.6 } else { 0.0 };
            let hip = vec3(0.22 * sx, y, z + lunge);
            let knee = vec3(
                0.75 * sx,
                y + 0.45 + lift + raise,
                z * 1.4 + 0.05 + raise * 0.3,
            );
            let foot = vec3(1.05 * sx, raise * 1.2, z * 1.7 + 0.1 + raise * 0.6);
            fr.beam(b, hip, knee, 0.045, colors[0]);
            fr.beam(b, knee, foot, 0.035, colors[1]);
            fr.sphere(b, knee, 0.05, dark(colors[0], 0.8));
        }
    }
    let chew = (pose.time * 6.0).sin() * 0.02 + rear * 0.04;
    for sx in [-1.0, 1.0] {
        fr.glow(b, vec3(0.08 * sx, y + 0.12, 0.45 + lunge), 0.04, colors[2]);
        fr.glow(b, vec3(0.16 * sx, y + 0.08, 0.4 + lunge), 0.03, colors[2]);
        fr.glow(b, vec3(0.05 * sx, y + 0.18, 0.42 + lunge), 0.022, colors[2]);
        fr.cone_dir(
            b,
            vec3((0.07 + chew) * sx, y - 0.05, 0.45 + lunge),
            vec3(-chew * sx, -0.15, 0.08),
            0.03,
            c(0.1, 0.08, 0.08),
        );
    }
}

pub(super) fn scorpion(b: &mut Batch, pos: Vec3, yaw: f32, colors: [Color; 3], pose: Pose) {
    let strike = if pose.attacking() {
        bump(pose.swing)
    } else {
        0.0
    };
    let fr = beast_frame(pos, yaw, pose, 0.0);
    let moving = pose.moving && !pose.dead;
    let y = 0.45;
    fr.ellipsoid(b, vec3(0.0, y, 0.0), vec3(0.35, 0.16, 0.55), colors[0]);
    for k in 0..4 {
        let kf = k as f32;
        fr.cube(
            b,
            vec3(0.0, y + 0.12, 0.36 - kf * 0.24),
            vec3(0.3 - kf * 0.03, 0.04, 0.09),
            colors[1],
        );
    }
    for k in 0..3 {
        for sx in [-1.0f32, 1.0] {
            let phase = k as f32 * 1.9 + if sx > 0.0 { 3.1 } else { 0.0 };
            let lift = if moving {
                (pose.walk * 1.5 + phase).sin().max(0.0) * 0.15
            } else {
                0.0
            };
            let z = 0.25 - k as f32 * 0.25;
            let knee = vec3(0.65 * sx, y + 0.2 + lift, z);
            fr.beam(b, vec3(0.3 * sx, y, z), knee, 0.035, colors[0]);
            fr.beam(b, knee, vec3(0.85 * sx, 0.0, z - 0.05), 0.03, colors[1]);
        }
    }
    // Pincers snap open and shut.
    let snap = ((pose.time * 2.0).sin() * 0.5 + 0.5) * 0.06 + strike * 0.08;
    for sx in [-1.0f32, 1.0] {
        let elbow = vec3(0.4 * sx, y + 0.1, 0.65);
        let claw = vec3(0.3 * sx, y + 0.1 + strike * 0.1, 1.0 + strike * 0.25);
        fr.beam(b, vec3(0.25 * sx, y, 0.45), elbow, 0.05, colors[0]);
        fr.beam(b, elbow, claw, 0.05, colors[0]);
        fr.ellipsoid(
            b,
            claw + vec3(0.0, 0.0, 0.1),
            vec3(0.1, 0.07, 0.16),
            colors[1],
        );
        fr.cone_dir(
            b,
            claw + vec3((0.04 + snap) * sx, 0.0, 0.22),
            vec3(-0.04 * sx, 0.0, 0.15),
            0.04,
            colors[1],
        );
        fr.cone_dir(
            b,
            claw + vec3((0.0 - snap) * sx, 0.0, 0.22),
            vec3(0.03 * sx, 0.0, 0.13),
            0.03,
            dark(colors[1], 0.8),
        );
    }
    // A tail curling over the back, swaying, then striking forward.
    let sway = if moving {
        0.0
    } else {
        (pose.time * 1.5).sin() * 0.08
    };
    let mut p = vec3(0.0, y + 0.05, -0.5);
    for k in 0..6 {
        let a = 0.2 + k as f32 * (0.42 + strike * 0.12) + sway;
        let next = p + vec3(sway * 0.1 * k as f32, a.sin() * 0.22, -a.cos() * 0.22);
        let col = if k % 2 == 0 { colors[0] } else { colors[1] };
        fr.sphere(b, next, 0.11 - k as f32 * 0.008, col);
        p = next;
    }
    fr.sphere(b, p + vec3(0.0, 0.02, 0.06), 0.08, colors[1]);
    fr.cone_dir(
        b,
        p + vec3(0.0, 0.0, 0.08),
        vec3(0.0, -0.08, 0.25),
        0.05,
        colors[2],
    );
    fr.glow(
        b,
        p + vec3(0.0, -0.06, 0.3),
        0.025,
        Color::new(0.6, 1.0, 0.3, 0.8),
    );
    for sx in [-1.0, 1.0] {
        fr.sphere(b, vec3(0.08 * sx, y + 0.12, 0.5), 0.03, c(0.05, 0.05, 0.05));
    }
}
