//! People, giants and humanoid mobs: bodies, faces, hair and headwear.

use super::*;

pub(super) fn humanoid(
    b: &mut Batch,
    pos: Vec3,
    yaw: f32,
    outfit: Outfit,
    look: &Look,
    pose: Pose,
) {
    let a = look.appearance;
    let seed = look.seed;
    let person = matches!(
        outfit,
        Outfit::Class(_) | Outfit::Merchant | Outfit::QuestGiver
    );
    let (race, scale) = match outfit {
        Outfit::Class(_) | Outfit::Merchant | Outfit::QuestGiver => {
            (a.race, race_shape(a.race).scale)
        }
        Outfit::Giant(..) => (Race::Human, 2.2),
        Outfit::Mob(..) => (Race::Human, 1.0),
    };
    let shape = race_shape(race);
    let style = style_of(outfit);
    let anim = animate(style, pose);
    let fr = if pose.dead {
        Frame::fallen(pos, yaw, scale, false)
    } else {
        Frame::leaning(pos, yaw, scale, anim.lean)
    };
    let skin = match outfit {
        Outfit::Class(_) | Outfit::Merchant | Outfit::QuestGiver => skin_color(race, a.skin),
        Outfit::Mob(style, colors) => match style {
            HumanoidStyle::Bandit
            | HumanoidStyle::Mystic
            | HumanoidStyle::Raider
            | HumanoidStyle::Necromancer => skin_color(Race::Human, (seed % 4) as u8),
            HumanoidStyle::Shaman => skin_color(Race::Orc, (seed % 5) as u8),
            HumanoidStyle::Skeleton => BONE,
            _ => colors[0],
        },
        Outfit::Giant(_, colors) => colors[1],
    };
    let hair = if person {
        hair_color(a.hair_color)
    } else {
        hair_color((seed / 3 % 4) as u8)
    };
    let slender = a.body == 1 && person;

    // Clothes.
    let (mut torso, mut legs, mut arms, mut boots, cape) = match outfit {
        Outfit::Class(class) => class_colors(class),
        Outfit::QuestGiver => (
            c(0.22, 0.28, 0.5),
            c(0.25, 0.22, 0.2),
            c(0.6, 0.62, 0.66),
            c(0.3, 0.2, 0.14),
            Some(c(0.55, 0.12, 0.1)),
        ),
        Outfit::Merchant => (
            c(0.5, 0.3, 0.2),
            c(0.35, 0.28, 0.22),
            c(0.85, 0.82, 0.72),
            c(0.3, 0.2, 0.14),
            None,
        ),
        Outfit::Mob(style, colors) => match style {
            HumanoidStyle::Skeleton => (BONE, BONE, BONE, BONE, None),
            HumanoidStyle::Satyr | HumanoidStyle::Trickster => {
                (skin, colors[1], skin, c(0.15, 0.12, 0.1), None)
            }
            HumanoidStyle::Trogg | HumanoidStyle::Troll => (skin, colors[1], skin, colors[1], None),
            _ => (colors[0], colors[1], colors[0], dark(colors[1], 0.8), None),
        },
        Outfit::Giant(_, colors) => (colors[0], colors[1], dark(colors[0], 0.95), colors[1], None),
    };
    if let Outfit::Mob(HumanoidStyle::Bandit | HumanoidStyle::Raider, _) = outfit {
        // Fighters go bare-armed.
        arms = skin;
    }
    // Worn armor shows on the body.
    let worn = |s: Slot| look.gear[s.index()].map(|id| rgb(item(id).color));
    let bare_skin = matches!(
        outfit,
        Outfit::Giant(..) | Outfit::Mob(HumanoidStyle::Skeleton, _)
    );
    let mut hands = if bare_skin { legs } else { skin };
    if let Some(cc) = worn(Slot::Chest) {
        torso = cc;
        arms = dark(cc, 0.92);
    }
    if let Some(cc) = worn(Slot::Legs) {
        legs = cc;
    }
    if let Some(cc) = worn(Slot::Feet) {
        boots = cc;
    }
    if let Some(cc) = worn(Slot::Hands) {
        hands = cc;
    }
    let is_robed = match outfit {
        Outfit::Class(class) => robed(class) && worn(Slot::Chest).is_none(),
        Outfit::Mob(style, _) => matches!(
            style,
            HumanoidStyle::Mystic | HumanoidStyle::Necromancer | HumanoidStyle::TrollShaman
        ),
        _ => false,
    };
    let has_plate = matches!(outfit, Outfit::Class(class) if plated(class));
    let goat_legs = matches!(
        outfit,
        Outfit::Mob(HumanoidStyle::Satyr | HumanoidStyle::Trickster, _)
    );
    let skeletal = matches!(
        outfit,
        Outfit::Mob(HumanoidStyle::Skeleton, _) | Outfit::Giant(GiantStyle::Bone, _)
    );
    let limb_w = shape.limbs * if skeletal { 0.55 } else { 1.0 };
    let trim = dark(torso, 0.7);

    // Legs: thigh, shin and a boot (or hoof). The body sinks by `crouch`,
    // bending the knees to keep the feet on the ground.
    let crouch = anim.crouch;
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let l = anim.legs[i];
        let sink = crouch * 2.2;
        let hip = vec3(
            0.13 * side * if slender { 1.1 } else { 1.0 },
            0.95 - crouch,
            0.0,
        );
        let thigh = l.swing + sink * 0.5;
        let knee = fr.segment(
            b,
            hip,
            limb_dir(thigh, l.spread, side),
            0.1 * limb_w,
            0.46,
            legs,
        );
        let hoof = if goat_legs { 0.6 } else { 0.0 };
        let shin = thigh - l.bend - sink + hoof;
        let ankle = fr.segment(
            b,
            knee,
            limb_dir(shin, l.spread * 0.5, side),
            0.085 * limb_w,
            0.45,
            legs,
        );
        if has_plate {
            fr.sphere(b, knee + vec3(0.0, 0.0, 0.06), 0.085, STEEL);
        }
        if goat_legs {
            fr.cube(b, ankle, vec3(0.07, 0.06, 0.08), c(0.12, 0.1, 0.08));
        } else {
            let foot = ankle + vec3(0.0, -0.01, 0.07);
            fr.cube(b, foot, vec3(0.1, 0.07, 0.17), boots);
            fr.cube(
                b,
                foot + vec3(0.0, 0.04, 0.1),
                vec3(0.095, 0.03, 0.07),
                dark(boots, 1.15),
            );
            // Boot cuff and sole.
            fr.cube(
                b,
                ankle + vec3(0.0, 0.1, 0.0),
                vec3(0.095, 0.03, 0.095),
                dark(boots, 0.75),
            );
            fr.cube(
                b,
                foot + vec3(0.0, -0.05, 0.0),
                vec3(0.088, 0.012, 0.155),
                dark(boots, 0.5),
            );
        }
    }
    let y = anim.bob - crouch;
    if is_robed {
        let sway = (anim.legs[0].swing - anim.legs[1].swing) * 0.08;
        fr.tilted(
            b,
            vec3(0.0, 0.6 + y * 0.5, 0.0),
            vec3(0.27, 0.36, 0.2),
            sway,
            legs,
        );
        fr.tilted(
            b,
            vec3(0.0, 0.3 + y * 0.5, 0.0),
            vec3(0.3, 0.08, 0.23),
            sway * 1.5,
            dark(legs, 0.85),
        );
        // Embroidered hem.
        fr.tilted(
            b,
            vec3(0.0, 0.23 + y * 0.5, 0.0),
            vec3(0.305, 0.02, 0.235),
            sway * 1.5,
            GOLD,
        );
    }
    if goat_legs {
        fr.cube(
            b,
            vec3(0.0, 0.85 + y, 0.0),
            vec3(0.28, 0.16, 0.18),
            dark(legs, 0.85),
        );
    }

    // Body.
    let breathe = (pose.time * 1.6).sin() * 0.01;
    let hips_w = if slender { 0.25 } else { 0.24 } * shape.limbs.max(0.9);
    let chest_w = if slender { 0.25 } else { 0.3 } * shape.limbs.max(0.85);
    if skeletal {
        fr.cube(b, vec3(0.0, 1.0 + y, 0.0), vec3(0.2, 0.06, 0.1), BONE);
        fr.cube(b, vec3(0.0, 1.25 + y, -0.05), vec3(0.04, 0.22, 0.04), BONE);
        for k in 0..4 {
            let yy = 1.28 + k as f32 * 0.07;
            fr.cube(
                b,
                vec3(0.0, yy + y, 0.02),
                vec3(0.2 - k as f32 * 0.01, 0.018, 0.12),
                BONE,
            );
        }
    } else {
        fr.cube(b, vec3(0.0, 1.0 + y, 0.0), vec3(hips_w, 0.1, 0.15), legs);
        fr.cube(
            b,
            vec3(0.0, 1.18 + y, 0.0),
            vec3(hips_w - 0.02, 0.1, 0.14),
            torso,
        );
        fr.cube(
            b,
            vec3(0.0, 1.4 + y + breathe, 0.0),
            vec3(chest_w, 0.15, 0.17),
            torso,
        );
        // Collar and a seam down the front.
        fr.cube(
            b,
            vec3(0.0, 1.535 + y, 0.0),
            vec3(chest_w * 0.55, 0.025, 0.14),
            trim,
        );
        fr.cube(b, vec3(0.0, 1.3 + y, 0.172), vec3(0.012, 0.2, 0.004), trim);
        // Belt, buckle and a pouch.
        fr.cube(
            b,
            vec3(0.0, 1.07 + y, 0.0),
            vec3(hips_w + 0.01, 0.04, 0.155),
            c(0.25, 0.17, 0.1),
        );
        fr.cube(b, vec3(0.0, 1.07 + y, 0.16), vec3(0.045, 0.035, 0.01), GOLD);
        fr.cube(
            b,
            vec3(-hips_w + 0.02, 1.0 + y, 0.12),
            vec3(0.06, 0.07, 0.04),
            LEATHER,
        );
    }
    if let Some(cc) = cape.filter(|_| worn(Slot::Chest).is_none()) {
        let flap = 0.08
            + (anim.legs[0].swing.abs() + anim.legs[1].swing.abs()) * 0.12
            + (pose.time * 2.0 + seed as f32).sin() * 0.02;
        fr.tilted(
            b,
            vec3(0.0, 1.13 + y, -0.21),
            vec3(0.25, 0.44, 0.02),
            -flap,
            cc,
        );
        // Clasps at the shoulders.
        for sx in [-1.0, 1.0] {
            fr.sphere(b, vec3(0.16 * sx, 1.5 + y, -0.17), 0.035, GOLD);
        }
    }

    // Neck and head.
    let hs = shape.head;
    fr.cylinder(
        b,
        vec3(0.0, 1.53 + y, 0.0),
        0.1,
        0.075 * limb_w.max(0.8),
        skin,
    );
    let nod = if pose.hurt > 0.0 {
        -0.03 * pose.hurt
    } else {
        0.0
    };
    let head = vec3(0.0, 1.55 + 0.21 * hs + y, 0.01 + nod);
    match outfit {
        Outfit::Giant(style, colors) => giant_head(b, &fr, head, style, colors, pose),
        _ => face(b, &fr, head, hs, race, skin, hair, outfit, skeletal, pose),
    }

    // Hair (players pick a style; mobs vary by seed).
    let hair_style_id = match outfit {
        Outfit::Class(_) | Outfit::Merchant | Outfit::QuestGiver => a.hair_style,
        Outfit::Mob(HumanoidStyle::Bandit | HumanoidStyle::Raider, _) => 1 + (seed % 2) as u8,
        _ => 0,
    };
    let headgear = worn(Slot::Head);
    if headgear.is_none() && !matches!(outfit, Outfit::Giant(..)) {
        hair_style(b, &fr, head, hs, hair_style_id, hair, pose, anim.lean);
        // Gnomes with a broad build grow a beard.
        if person && race == Race::Gnome && a.body == 0 {
            fr.ellipsoid(
                b,
                head + vec3(0.0, -0.17, 0.12) * hs,
                vec3(0.13, 0.12, 0.08) * hs,
                hair,
            );
        }
    }
    match headgear {
        Some(cc) => {
            if look.gear[Slot::Head.index()] == Some(items::LINEN_HOOD) {
                fr.cube(
                    b,
                    head + vec3(0.0, 0.08, -0.03) * hs,
                    vec3(0.23, 0.18, 0.21) * hs,
                    cc,
                );
                fr.cone(
                    b,
                    head + vec3(0.0, 0.2, -0.06) * hs,
                    0.22 * hs,
                    0.16 * hs,
                    cc,
                );
            } else {
                fr.cube(
                    b,
                    head + vec3(0.0, 0.15, 0.0) * hs,
                    vec3(0.215, 0.08, 0.215) * hs,
                    cc,
                );
                fr.cube(
                    b,
                    head + vec3(0.0, 0.09, 0.2) * hs,
                    vec3(0.2, 0.025, 0.04) * hs,
                    dark(cc, 0.8),
                );
            }
        }
        None => headwear(b, &fr, head, hs, outfit, pose),
    }

    // Arms.
    let shoulder_y = 1.53 + y;
    let sw = chest_w + 0.08;
    let mut hand_pos = [Vec3::ZERO; 2];
    let mut forearm = [0.0f32; 2];
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let l = anim.arms[i];
        let shoulder = vec3(sw * side, shoulder_y, 0.0);
        fr.sphere(b, shoulder, 0.1 * limb_w.max(0.7), arms);
        let el = fr.segment(
            b,
            shoulder,
            limb_dir(l.swing, l.spread, side),
            0.08 * limb_w,
            0.34,
            arms,
        );
        let fore = l.swing + l.bend;
        let sleeve = if is_robed { arms } else { dark(arms, 0.95) };
        let wrist = fr.segment(
            b,
            el,
            limb_dir(fore, l.spread * 0.7, side),
            0.07 * limb_w,
            0.32,
            sleeve,
        );
        if has_plate {
            fr.sphere(b, el, 0.075, STEEL);
        }
        if !skeletal && !bare_skin {
            // A bracer at the wrist.
            let d = limb_dir(fore, l.spread * 0.7, side);
            fr.segment(
                b,
                wrist - d * 0.1,
                d,
                0.078 * limb_w,
                0.08,
                dark(sleeve, 0.7),
            );
        }
        fr.sphere(
            b,
            wrist + vec3(0.0, -0.04, 0.0),
            0.088 * limb_w.max(0.8),
            hands,
        );
        hand_pos[i] = wrist + vec3(0.0, -0.05, 0.0);
        forearm[i] = fore;
    }
    let h = Hands {
        left: hand_pos[0],
        right: hand_pos[1],
        left_angle: forearm[0] + 1.55,
        right_angle: forearm[1] + 1.55,
        shoulder_y,
        sw,
        y,
        chest_w,
    };
    gear_and_weapons(b, &fr, outfit, &h, look, pose, legs);
    shoulders(b, &fr, outfit, &h, look, pose);
    if look.gear[Slot::Chest.index()] == Some(items::HEARTSTONE_CHESTGUARD) {
        fr.glow(b, vec3(0.0, 1.42 + y, 0.18), 0.06, c(0.35, 0.9, 1.0));
        for sx in [-1.0, 1.0] {
            fr.ellipsoid(
                b,
                vec3(sw * sx, shoulder_y + 0.06, 0.0),
                vec3(0.15, 0.09, 0.15),
                c(0.45, 0.47, 0.52),
            );
        }
    }
    // Spell light gathering in the hands while casting.
    if pose.casting && !pose.dead {
        let col = cast_color(style);
        let pulse = 0.06 + (pose.time * 8.0).sin().abs() * 0.04;
        for p in hand_pos {
            fr.glow(b, p + vec3(0.0, 0.04, 0.06), pulse, col);
        }
    }
}

/// The color of the light in someone's hands while they cast.
pub(super) fn cast_color(style: Style) -> Color {
    match style {
        Style::Staff => Color::new(1.0, 0.55, 0.2, 0.85),
        Style::Holy | Style::Hammer => Color::new(1.0, 0.95, 0.6, 0.85),
        Style::Nature => Color::new(0.5, 1.0, 0.4, 0.85),
        Style::Wild => Color::new(0.8, 0.4, 1.0, 0.85),
        Style::Fel => Color::new(0.4, 1.0, 0.3, 0.85),
        Style::Lute => Color::new(0.95, 0.6, 1.0, 0.7),
        Style::Rifle => Color::new(0.5, 0.85, 1.0, 0.7),
        Style::Caster => Color::new(0.7, 0.5, 1.0, 0.85),
        _ => Color::new(0.9, 0.9, 1.0, 0.6),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn face(
    b: &mut Batch,
    fr: &Frame,
    head: Vec3,
    hs: f32,
    race: Race,
    skin: Color,
    hair: Color,
    outfit: Outfit,
    skeletal: bool,
    pose: Pose,
) {
    let person = matches!(
        outfit,
        Outfit::Class(_) | Outfit::Merchant | Outfit::QuestGiver
    );
    let undead = (person && race == Race::Undead) || skeletal;
    if skeletal {
        fr.ellipsoid(b, head, vec3(0.17, 0.2, 0.18) * hs, BONE);
        fr.cube(
            b,
            head + vec3(0.0, -0.15, 0.06) * hs,
            vec3(0.12, 0.05, 0.1) * hs,
            dark(BONE, 0.85),
        );
    } else {
        let jaw = if race == Race::Orc { 1.1 } else { 1.0 };
        fr.ellipsoid(b, head, vec3(0.19 * jaw, 0.21, 0.2) * hs, skin);
        // Chin and cheeks.
        fr.ellipsoid(
            b,
            head + vec3(0.0, -0.13, 0.08) * hs,
            vec3(0.11 * jaw, 0.07, 0.1) * hs,
            skin,
        );
    }
    let eye_glow = match outfit {
        Outfit::Mob(
            HumanoidStyle::Skeleton | HumanoidStyle::Necromancer | HumanoidStyle::Trickster,
            colors,
        ) => Some(colors[2]),
        _ if undead => Some(c(0.95, 0.9, 0.55)),
        _ => None,
    };
    let tusks = (person && race == Race::Orc)
        || matches!(
            outfit,
            Outfit::Mob(
                HumanoidStyle::Troll | HumanoidStyle::TrollShaman | HumanoidStyle::Trogg,
                _
            )
        );
    // Blink every few seconds.
    let blink = (pose.time * 0.5 + head.x).fract() < 0.03;
    for sx in [-1.0, 1.0] {
        let eye = head + vec3(0.07 * sx, 0.03, 0.17) * hs;
        match eye_glow {
            Some(g) => {
                fr.sphere(b, eye, 0.04 * hs, c(0.08, 0.06, 0.08));
                fr.glow(b, eye + vec3(0.0, 0.0, 0.02) * hs, 0.022 * hs, g);
            }
            None if blink => {
                fr.cube(
                    b,
                    eye + vec3(0.0, 0.0, 0.02) * hs,
                    vec3(0.035, 0.006, 0.01) * hs,
                    dark(skin, 0.7),
                );
            }
            None => {
                fr.sphere(b, eye, 0.035 * hs, c(0.95, 0.95, 0.95));
                fr.sphere(
                    b,
                    eye + vec3(0.0, 0.0, 0.025) * hs,
                    0.018 * hs,
                    c(0.12, 0.1, 0.1),
                );
            }
        }
        if !skeletal {
            fr.cube(
                b,
                head + vec3(0.07 * sx, 0.085, 0.18) * hs,
                vec3(0.04, 0.012, 0.012) * hs,
                dark(hair, 0.9),
            );
        }
        let ear = head + vec3(0.18 * sx, 0.02, -0.02) * hs;
        match race {
            _ if !person => {
                if !skeletal {
                    fr.sphere(b, head + vec3(0.19 * sx, 0.0, 0.0) * hs, 0.04 * hs, skin);
                }
            }
            Race::Elf => fr.cone_dir(b, ear, vec3(0.2 * sx, 0.12, -0.08) * hs, 0.045 * hs, skin),
            Race::Goblin => fr.cone_dir(b, ear, vec3(0.3 * sx, 0.06, -0.04) * hs, 0.07 * hs, skin),
            Race::Orc => fr.cone_dir(b, ear, vec3(0.1 * sx, 0.06, -0.03) * hs, 0.05 * hs, skin),
            _ => fr.sphere(b, head + vec3(0.19 * sx, 0.0, 0.0) * hs, 0.04 * hs, skin),
        }
        if tusks {
            fr.cone_dir(
                b,
                head + vec3(0.06 * sx, -0.12, 0.15) * hs,
                vec3(0.01 * sx, 0.09, 0.02) * hs,
                0.02 * hs,
                c(0.96, 0.94, 0.85),
            );
        }
    }
    if skeletal {
        fr.cube(
            b,
            head + vec3(0.0, -0.06, 0.17) * hs,
            vec3(0.02, 0.03, 0.01) * hs,
            c(0.1, 0.08, 0.08),
        );
        return;
    }
    match race {
        Race::Goblin if person => fr.cone_dir(
            b,
            head + vec3(0.0, -0.01, 0.18) * hs,
            vec3(0.0, -0.02, 0.14) * hs,
            0.04 * hs,
            dark(skin, 0.9),
        ),
        Race::Gnome if person => fr.sphere(
            b,
            head + vec3(0.0, -0.02, 0.2) * hs,
            0.05 * hs,
            dark(skin, 0.95),
        ),
        _ => fr.cube(
            b,
            head + vec3(0.0, -0.015, 0.205) * hs,
            vec3(0.022, 0.04, 0.025) * hs,
            dark(skin, 0.9),
        ),
    }
    let mouth = if undead {
        c(0.2, 0.12, 0.15)
    } else {
        c(0.55, 0.28, 0.25)
    };
    let w = if race == Race::Goblin && person {
        0.07
    } else {
        0.045
    };
    // The mouth opens with a shout when hit.
    let open = 0.01 + pose.hurt * 0.025;
    fr.cube(
        b,
        head + vec3(0.0, -0.09, 0.18) * hs,
        vec3(w, open, 0.01) * hs,
        mouth,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn hair_style(
    b: &mut Batch,
    fr: &Frame,
    head: Vec3,
    hs: f32,
    style: u8,
    hair: Color,
    pose: Pose,
    lean: f32,
) {
    let top = head + vec3(0.0, 0.17, -0.01) * hs;
    match style {
        1 => {
            fr.cube(b, top, vec3(0.2, 0.06, 0.2) * hs, hair);
            fr.cube(
                b,
                head + vec3(0.0, 0.05, -0.13) * hs,
                vec3(0.2, 0.13, 0.08) * hs,
                hair,
            );
            // A fringe.
            fr.cube(
                b,
                head + vec3(0.0, 0.14, 0.17) * hs,
                vec3(0.18, 0.03, 0.03) * hs,
                dark(hair, 0.9),
            );
        }
        2 => {
            fr.cube(b, top, vec3(0.21, 0.06, 0.21) * hs, hair);
            fr.cube(
                b,
                head + vec3(0.0, -0.08, -0.14) * hs,
                vec3(0.21, 0.26, 0.07) * hs,
                hair,
            );
            for sx in [-1.0, 1.0] {
                fr.cube(
                    b,
                    head + vec3(0.19 * sx, -0.04, -0.03) * hs,
                    vec3(0.03, 0.2, 0.09) * hs,
                    hair,
                );
            }
        }
        3 => {
            fr.cube(b, top, vec3(0.2, 0.06, 0.2) * hs, hair);
            fr.cube(
                b,
                head + vec3(0.0, 0.05, -0.14) * hs,
                vec3(0.19, 0.12, 0.07) * hs,
                hair,
            );
            let sway = (pose.time * 2.0).sin() * 0.1 - lean * 1.5;
            let tie = head + vec3(0.0, 0.05, -0.21) * hs;
            fr.sphere(b, tie, 0.05 * hs, dark(hair, 0.8));
            fr.limb(b, tie, -0.35 + sway, 0.045 * hs, 0.32 * hs, hair);
        }
        4 => {
            fr.cube(
                b,
                head + vec3(0.0, 0.2, -0.02) * hs,
                vec3(0.035, 0.08, 0.19) * hs,
                hair,
            );
        }
        _ => {}
    }
}

pub(super) fn headwear(b: &mut Batch, fr: &Frame, head: Vec3, hs: f32, outfit: Outfit, pose: Pose) {
    let h = |v: Vec3| head + v * hs;
    match outfit {
        Outfit::Class(class) => match class {
            Class::Barbarian => {
                fr.cube(
                    b,
                    h(vec3(0.0, 0.1, 0.0)),
                    vec3(0.205, 0.025, 0.205) * hs,
                    c(0.45, 0.32, 0.2),
                );
                for sx in [-1.0, 1.0] {
                    fr.cube(
                        b,
                        h(vec3(0.09 * sx, 0.0, 0.19)),
                        vec3(0.015, 0.05, 0.01) * hs,
                        c(0.75, 0.12, 0.1),
                    );
                }
                // A bone charm on the headband.
                fr.cone_dir(
                    b,
                    h(vec3(0.15, 0.1, 0.12)),
                    vec3(0.04, -0.08, 0.02) * hs,
                    0.015 * hs,
                    BONE,
                );
            }
            Class::Fighter => {
                fr.cube(
                    b,
                    h(vec3(0.0, 0.16, 0.0)),
                    vec3(0.22, 0.07, 0.22) * hs,
                    STEEL,
                );
                fr.cube(
                    b,
                    h(vec3(0.0, 0.1, 0.0)),
                    vec3(0.225, 0.02, 0.225) * hs,
                    dark(STEEL, 0.8),
                );
                fr.cube(
                    b,
                    h(vec3(0.0, 0.24, -0.02)),
                    vec3(0.02, 0.04, 0.16) * hs,
                    c(0.18, 0.3, 0.6),
                );
            }
            Class::Paladin | Class::Cleric => {
                fr.cylinder(b, h(vec3(0.0, 0.17, 0.0)), 0.04 * hs, 0.21 * hs, GOLD);
                fr.glow(b, h(vec3(0.0, 0.2, 0.2)), 0.03 * hs, c(1.0, 0.95, 0.6));
            }
            Class::Monk => {
                let red = c(0.85, 0.15, 0.12);
                fr.cube(
                    b,
                    h(vec3(0.0, 0.1, 0.0)),
                    vec3(0.205, 0.03, 0.205) * hs,
                    red,
                );
                fr.limb(
                    b,
                    h(vec3(0.0, 0.1, -0.21)),
                    -0.6 + (pose.time * 3.0).sin() * 0.1,
                    0.03 * hs,
                    0.25 * hs,
                    red,
                );
            }
            Class::Rogue => {
                fr.cube(
                    b,
                    h(vec3(0.0, -0.075, 0.12)),
                    vec3(0.195, 0.065, 0.1) * hs,
                    c(0.12, 0.12, 0.14),
                );
            }
            Class::Ranger => {
                fr.cube(
                    b,
                    h(vec3(0.0, -0.12, -0.17)),
                    vec3(0.2, 0.1, 0.08) * hs,
                    c(0.22, 0.32, 0.18),
                );
            }
            Class::Artificer => {
                fr.cube(
                    b,
                    h(vec3(0.0, 0.1, 0.0)),
                    vec3(0.205, 0.025, 0.205) * hs,
                    LEATHER,
                );
                for sx in [-1.0, 1.0] {
                    fr.cylinder_dir(
                        b,
                        h(vec3(0.07 * sx, 0.1, 0.18)),
                        vec3(0.0, 0.0, 0.05) * hs,
                        0.05 * hs,
                        c(0.7, 0.55, 0.25),
                    );
                    b.lit(|b| {
                        fr.cylinder_dir(
                            b,
                            h(vec3(0.07 * sx, 0.1, 0.23)),
                            vec3(0.0, 0.0, 0.01) * hs,
                            0.04 * hs,
                            c(0.5, 0.85, 1.0),
                        )
                    });
                }
            }
            Class::Bard => {
                let hat = c(0.6, 0.18, 0.45);
                fr.cylinder(b, h(vec3(0.0, 0.14, 0.0)), 0.04 * hs, 0.3 * hs, hat);
                fr.ellipsoid(b, h(vec3(0.0, 0.22, 0.0)), vec3(0.2, 0.08, 0.2) * hs, hat);
                let bob = (pose.time * 3.0).sin() * 0.03;
                fr.beam(
                    b,
                    h(vec3(0.15, 0.2, -0.05)),
                    h(vec3(0.3, 0.55 + bob, -0.25)),
                    0.02 * hs,
                    c(0.95, 0.85, 0.3),
                );
            }
            Class::Druid => {
                let antler = c(0.6, 0.5, 0.35);
                for sx in [-1.0, 1.0] {
                    let base = h(vec3(0.12 * sx, 0.15, 0.0));
                    let tip = h(vec3(0.28 * sx, 0.45, -0.05));
                    fr.beam(b, base, tip, 0.025 * hs, antler);
                    fr.beam(
                        b,
                        base.lerp(tip, 0.5),
                        h(vec3(0.3 * sx, 0.3, 0.1)),
                        0.02 * hs,
                        antler,
                    );
                    fr.sphere(
                        b,
                        h(vec3(0.2 * sx, 0.16, 0.08)),
                        0.03 * hs,
                        c(0.4, 0.65, 0.3),
                    );
                }
                fr.cube(
                    b,
                    h(vec3(0.0, 0.11, 0.0)),
                    vec3(0.205, 0.02, 0.205) * hs,
                    c(0.3, 0.5, 0.2),
                );
            }
            Class::Mage => {
                // The pointed hat is built in the head's frame, so it turns with it.
                let hat = c(0.27, 0.17, 0.6);
                fr.cylinder(b, h(vec3(0.0, 0.14, 0.0)), 0.04 * hs, 0.38 * hs, hat);
                fr.cone(b, h(vec3(0.0, 0.17, 0.0)), 0.65 * hs, 0.23 * hs, hat);
                fr.cylinder(b, h(vec3(0.0, 0.18, 0.0)), 0.05 * hs, 0.235 * hs, GOLD);
                // Stars on the hat.
                fr.glow(b, h(vec3(0.08, 0.38, 0.12)), 0.022 * hs, c(1.0, 0.9, 0.5));
                fr.glow(b, h(vec3(-0.05, 0.5, 0.07)), 0.016 * hs, c(1.0, 0.9, 0.5));
            }
            Class::Sorcerer => {
                fr.cylinder(b, h(vec3(0.0, 0.17, 0.0)), 0.04 * hs, 0.21 * hs, GOLD);
                fr.cone_dir(
                    b,
                    h(vec3(0.0, 0.21, 0.14)),
                    vec3(0.0, 0.12, 0.03) * hs,
                    0.03 * hs,
                    GOLD,
                );
                fr.glow(b, h(vec3(0.0, 0.2, 0.2)), 0.025 * hs, c(0.8, 0.4, 1.0));
            }
            Class::Warlock => {
                fr.cube(
                    b,
                    h(vec3(0.0, 0.07, -0.04)),
                    vec3(0.22, 0.18, 0.2) * hs,
                    c(0.12, 0.08, 0.14),
                );
                for sx in [-1.0, 1.0] {
                    fr.cone_dir(
                        b,
                        h(vec3(0.12 * sx, 0.2, 0.05)),
                        vec3(0.08 * sx, 0.18, -0.08) * hs,
                        0.04 * hs,
                        c(0.2, 0.15, 0.15),
                    );
                }
            }
        },
        Outfit::QuestGiver => {
            // A plumed officer's hat.
            fr.cylinder(
                b,
                h(vec3(0.0, 0.13, 0.0)),
                0.03 * hs,
                0.3 * hs,
                c(0.18, 0.15, 0.2),
            );
            fr.cylinder(
                b,
                h(vec3(0.0, 0.15, 0.0)),
                0.12 * hs,
                0.2 * hs,
                c(0.18, 0.15, 0.2),
            );
            fr.cylinder(b, h(vec3(0.0, 0.16, 0.0)), 0.025 * hs, 0.205 * hs, GOLD);
            for k in 0..3 {
                let x = 0.1 + k as f32 * 0.03;
                fr.beam(
                    b,
                    h(vec3(x, 0.25, -0.05)),
                    h(vec3(x + 0.12, 0.6 - k as f32 * 0.05, -0.25)),
                    0.025 * hs,
                    c(0.95, 0.95, 0.92),
                );
            }
        }
        Outfit::Merchant => {
            fr.cylinder(
                b,
                h(vec3(0.0, 0.15, 0.0)),
                0.07 * hs,
                0.21 * hs,
                c(0.2, 0.42, 0.3),
            );
            fr.beam(
                b,
                h(vec3(0.15, 0.2, 0.0)),
                h(vec3(0.25, 0.42, -0.2)),
                0.02 * hs,
                c(0.9, 0.3, 0.2),
            );
        }
        Outfit::Mob(style, colors) => match style {
            HumanoidStyle::Bandit => {
                fr.cube(
                    b,
                    h(vec3(0.0, -0.06, 0.15)),
                    vec3(0.19, 0.07, 0.07) * hs,
                    colors[2],
                );
            }
            HumanoidStyle::Mystic | HumanoidStyle::Necromancer => {
                let hood = dark(colors[0], 0.7);
                fr.cube(
                    b,
                    h(vec3(0.0, 0.06, -0.04)),
                    vec3(0.23, 0.2, 0.21) * hs,
                    hood,
                );
                fr.cone(b, h(vec3(0.0, 0.22, -0.08)), 0.32 * hs, 0.17 * hs, hood);
            }
            HumanoidStyle::Raider => {
                fr.ellipsoid(
                    b,
                    h(vec3(0.0, 0.15, -0.01)),
                    vec3(0.23, 0.12, 0.23) * hs,
                    c(0.92, 0.88, 0.78),
                );
                fr.cube(
                    b,
                    h(vec3(0.0, -0.07, 0.14)),
                    vec3(0.19, 0.07, 0.07) * hs,
                    colors[2],
                );
            }
            HumanoidStyle::Shaman | HumanoidStyle::TrollShaman | HumanoidStyle::TroggShaman => {
                for k in 0..5 {
                    let a = (k as f32 - 2.0) * 0.35;
                    let col = if k % 2 == 0 {
                        colors[2]
                    } else {
                        c(0.95, 0.92, 0.85)
                    };
                    fr.beam(
                        b,
                        h(vec3(a * 0.3, 0.15, -0.05)),
                        h(vec3(a * 0.6, 0.5, -0.15)),
                        0.025 * hs,
                        col,
                    );
                }
            }
            HumanoidStyle::Satyr | HumanoidStyle::Trickster => {
                let horn = c(0.85, 0.8, 0.7);
                for sx in [-1.0, 1.0] {
                    let bend = h(vec3(0.2 * sx, 0.32, -0.05));
                    fr.beam(b, h(vec3(0.1 * sx, 0.15, 0.03)), bend, 0.035 * hs, horn);
                    fr.beam(b, bend, h(vec3(0.26 * sx, 0.3, -0.2)), 0.025 * hs, horn);
                }
            }
            HumanoidStyle::Troll => {
                fr.cube(
                    b,
                    h(vec3(0.0, 0.2, -0.02)),
                    vec3(0.04, 0.09, 0.18) * hs,
                    c(0.85, 0.3, 0.2),
                );
            }
            HumanoidStyle::Trogg | HumanoidStyle::Skeleton => {}
        },
        Outfit::Giant(..) => {}
    }
}

pub(super) fn giant_head(
    b: &mut Batch,
    fr: &Frame,
    head: Vec3,
    style: GiantStyle,
    colors: [Color; 3],
    pose: Pose,
) {
    let glow_c = Color::new(
        colors[2].r,
        colors[2].g,
        colors[2].b,
        0.7 + 0.3 * (pose.time * 2.0).sin().abs(),
    );
    match style {
        GiantStyle::Yeti => {
            fr.ellipsoid(b, head, vec3(0.24, 0.24, 0.24), colors[0]);
            fr.ellipsoid(
                b,
                head + vec3(0.0, -0.03, 0.16),
                vec3(0.14, 0.12, 0.08),
                c(0.55, 0.65, 0.8),
            );
            for sx in [-1.0, 1.0] {
                fr.cone_dir(
                    b,
                    head + vec3(0.17 * sx, 0.14, 0.0),
                    vec3(0.15 * sx, 0.12, 0.1),
                    0.05,
                    c(0.75, 0.7, 0.62),
                );
                fr.glow(b, head + vec3(0.07 * sx, 0.03, 0.22), 0.035, glow_c);
                fr.cone_dir(
                    b,
                    head + vec3(0.04 * sx, -0.1, 0.2),
                    vec3(0.0, -0.06, 0.0),
                    0.015,
                    c(0.95, 0.95, 0.9),
                );
            }
        }
        GiantStyle::Bone => {
            fr.ellipsoid(b, head, vec3(0.2, 0.22, 0.22), colors[0]);
            fr.cube(
                b,
                head + vec3(0.0, -0.17, 0.06),
                vec3(0.14, 0.05, 0.12),
                dark(colors[0], 0.85),
            );
            for sx in [-1.0, 1.0] {
                fr.sphere(
                    b,
                    head + vec3(0.08 * sx, 0.03, 0.18),
                    0.05,
                    c(0.08, 0.06, 0.08),
                );
                fr.glow(b, head + vec3(0.08 * sx, 0.03, 0.2), 0.03, glow_c);
                fr.cone_dir(
                    b,
                    head + vec3(0.15 * sx, 0.15, 0.0),
                    vec3(0.12 * sx, 0.25, -0.05),
                    0.04,
                    colors[0],
                );
            }
        }
        GiantStyle::Treant => {
            fr.cube(
                b,
                head + vec3(0.0, 0.03, 0.0),
                vec3(0.2, 0.22, 0.19),
                colors[0],
            );
            for k in 0..5 {
                let a = k as f32 * 1.26 + (pose.time * 0.5).sin() * 0.05;
                fr.sphere(
                    b,
                    head + vec3(a.cos() * 0.2, 0.3, a.sin() * 0.2),
                    0.2,
                    colors[2],
                );
            }
            for sx in [-1.0, 1.0] {
                fr.glow(b, head + vec3(0.08 * sx, 0.05, 0.2), 0.04, c(0.6, 1.0, 0.4));
            }
        }
        _ => {
            fr.cube(
                b,
                head + vec3(0.0, 0.03, 0.0),
                vec3(0.2, 0.19, 0.19),
                colors[0],
            );
            fr.cube(
                b,
                head + vec3(0.0, 0.17, -0.02),
                vec3(0.16, 0.05, 0.15),
                dark(colors[0], 0.85),
            );
            for sx in [-1.0, 1.0] {
                fr.glow(b, head + vec3(0.08 * sx, 0.05, 0.19), 0.045, glow_c);
            }
            if style == GiantStyle::Crystal {
                for k in 0..3 {
                    let x = k as f32 - 1.0;
                    fr.cone_dir(
                        b,
                        head + vec3(x * 0.1, 0.2, -0.05),
                        vec3(x * 0.08, 0.3, -0.05),
                        0.05,
                        colors[2],
                    );
                }
            }
        }
    }
}
