//! People as rigged, animated models: players, townsfolk and humanoid
//! mobs. The bodies, props and animations come from the KayKit Adventurers
//! and Skeletons character packs (CC0; see `client/assets/characters/`).
//!
//! Every body shares one skeleton and one set of animation clips (from
//! `rig.glb`). A character is drawn by picking a body and the props it
//! carries for its class or mob type (`dress`), repainting the body's
//! texture with its race's skin, its hair and its class or armor colors
//! (`Paint`), posing the skeleton from what it's doing (`layers`), and
//! drawing the body's meshes bent around that pose.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use super::*;
use crate::gfx::{ModelFile, ModelPart, Picture, Transform, texture};

/// The bodies, each a file in `client/assets/characters/`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Body {
    Knight,
    Barbarian,
    Mage,
    Rogue,
    RogueHooded,
    SkeletonMinion,
    SkeletonWarrior,
    SkeletonRogue,
    SkeletonMage,
}

impl Body {
    const ALL: [Body; 9] = [
        Body::Knight,
        Body::Barbarian,
        Body::Mage,
        Body::Rogue,
        Body::RogueHooded,
        Body::SkeletonMinion,
        Body::SkeletonWarrior,
        Body::SkeletonRogue,
        Body::SkeletonMage,
    ];

    fn file(self) -> &'static [u8] {
        match self {
            Body::Knight => include_bytes!("../../assets/characters/knight.glb"),
            Body::Barbarian => include_bytes!("../../assets/characters/barbarian.glb"),
            Body::Mage => include_bytes!("../../assets/characters/mage.glb"),
            Body::Rogue => include_bytes!("../../assets/characters/rogue.glb"),
            Body::RogueHooded => include_bytes!("../../assets/characters/rogue_hooded.glb"),
            Body::SkeletonMinion => include_bytes!("../../assets/characters/skeleton_minion.glb"),
            Body::SkeletonWarrior => {
                include_bytes!("../../assets/characters/skeleton_warrior.glb")
            }
            Body::SkeletonRogue => include_bytes!("../../assets/characters/skeleton_rogue.glb"),
            Body::SkeletonMage => include_bytes!("../../assets/characters/skeleton_mage.glb"),
        }
    }

    /// Which cells of the body's texture (column, row of its 8 by 4 grid of
    /// color swatches) each repaintable part of it uses.
    fn cells(self) -> &'static [((usize, usize), Role)] {
        use Role::*;
        match self {
            Body::Knight => &[
                ((0, 0), Skin),
                ((1, 0), Hair),
                ((3, 0), Main),
                ((7, 0), Main),
                ((7, 1), Legs),
                ((0, 1), Cape),
            ],
            Body::Barbarian => &[
                ((0, 0), Skin),
                ((1, 0), Hair),
                ((0, 1), Main),
                ((1, 1), Trim),
                ((3, 2), Legs),
                ((7, 0), Cape),
            ],
            Body::Mage => &[
                ((0, 0), Skin),
                ((7, 2), Skin),
                ((1, 0), Hair),
                ((0, 1), Main),
                ((1, 1), Main),
                ((7, 1), Trim),
                ((3, 2), Legs),
                ((2, 1), Cape),
            ],
            Body::Rogue | Body::RogueHooded => &[
                ((0, 0), Skin),
                ((1, 0), Hair),
                ((0, 1), Main),
                ((3, 2), Legs),
                ((1, 1), Cape),
            ],
            Body::SkeletonMinion
            | Body::SkeletonWarrior
            | Body::SkeletonRogue
            | Body::SkeletonMage => &[((1, 1), Skin), ((2, 2), Cape), ((7, 0), Main)],
        }
    }
}

/// The parts of a body that can be repainted.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Role {
    Skin,
    Hair,
    /// Shirt, robe or armor.
    Main,
    /// Sleeves and other cloth.
    Trim,
    Legs,
    /// Cape and tabard.
    Cape,
}

const ROLES: usize = 6;

/// Colors to repaint a body with, by `Role` (`None` keeps the original).
pub(super) type Paint = [Option<[u8; 3]>; ROLES];

fn paint_color(c: Color) -> Option<[u8; 3]> {
    Some([c.r, c.g, c.b].map(texture::byte))
}

/// A rigid part shown on a body: a prop or hat from one of the body files
/// (all bodies share the skeleton, so any body can carry any of them).
pub(super) type Prop = (Body, &'static str);

/// How to draw one person.
#[derive(Clone, Debug)]
pub(super) struct Dress {
    pub(super) body: Body,
    /// Whose head (and hair) it wears.
    pub(super) head_from: Body,
    pub(super) props: Vec<Prop>,
    pub(super) paint: Paint,
    /// Overall height, relative to a human.
    pub(super) scale: f32,
    /// Breadth, relative to height.
    pub(super) width: f32,
    /// Head size.
    pub(super) head: f32,
    /// For ears and tusks.
    pub(super) race: Option<Race>,
}

struct Loaded {
    model: ModelFile,
    binding: Vec<Option<usize>>,
    /// The texture, shrunk to `ATLAS` pixels square.
    picture: Picture,
}

struct Library {
    rig: ModelFile,
    /// The skeleton's nodes above the hips (spine, arms and head), which an
    /// attack can take over while the legs keep running.
    upper: Vec<bool>,
    bodies: Vec<Loaded>,
}

/// The repainted textures are this many pixels square.
const ATLAS: usize = 256;

fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let rig = ModelFile::from_glb(include_bytes!("../../assets/characters/rig.glb"))
            .expect("the character skeleton loads");
        let spine = rig.node("spine");
        let upper = (0..rig.nodes.len())
            .map(|mut i| {
                loop {
                    if Some(i) == spine {
                        break true;
                    }
                    match rig.nodes[i].parent {
                        Some(p) => i = p,
                        None => break false,
                    }
                }
            })
            .collect();
        let bodies = Body::ALL
            .iter()
            .map(|body| {
                let model = ModelFile::from_glb(body.file()).expect("a character body loads");
                let binding = model.bind(&rig);
                let picture = shrink(model.picture.as_ref().expect("bodies have a texture"));
                Loaded {
                    model,
                    binding,
                    picture,
                }
            })
            .collect();
        Library { rig, upper, bodies }
    })
}

/// Box-filters a picture down to `ATLAS` square.
fn shrink(p: &Picture) -> Picture {
    let (sx, sy) = (p.width / ATLAS, p.height / ATLAS);
    let mut pixels = Vec::with_capacity(ATLAS * ATLAS);
    for y in 0..ATLAS {
        for x in 0..ATLAS {
            let mut sum = [0u32; 4];
            for j in 0..sy {
                for i in 0..sx {
                    let px = p.pixels[(y * sy + j) * p.width + x * sx + i];
                    for k in 0..4 {
                        sum[k] += px[k] as u32;
                    }
                }
            }
            let n = (sx * sy).max(1) as u32;
            pixels.push(sum.map(|s| (s / n) as u8));
        }
    }
    Picture {
        width: ATLAS,
        height: ATLAS,
        pixels,
    }
}

fn luma(p: [f32; 3]) -> f32 {
    p[0] * 0.3 + p[1] * 0.59 + p[2] * 0.11
}

/// Repaints a body's texture: each repainted cell takes its new color,
/// keeping the light and shade painted into it.
fn repaint(body: Body, picture: &Picture, paint: &Paint) -> Vec<[u8; 4]> {
    let mut pixels = picture.pixels.clone();
    let (cw, ch) = (picture.width / 8, picture.height / 4);
    for &((cx, cy), role) in body.cells() {
        let Some(target) = paint[role as usize] else {
            continue;
        };
        let target = target.map(|v| v as f32 / 255.0);
        let cell = |pixels: &[[u8; 4]]| {
            (cy * ch..(cy + 1) * ch)
                .flat_map(move |y| (cx * cw..(cx + 1) * cw).map(move |x| y * picture.width + x))
                .map(|i| pixels[i])
                .collect::<Vec<_>>()
        };
        let rgb = |p: [u8; 4]| [p[0], p[1], p[2]].map(|v| v as f32 / 255.0);
        let original = cell(&picture.pixels);
        let mean = luma(
            original
                .iter()
                .map(|&p| rgb(p))
                .fold([0.0; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]])
                .map(|v| v / original.len() as f32),
        )
        .max(0.02);
        for y in cy * ch..(cy + 1) * ch {
            for x in cx * cw..(cx + 1) * cw {
                let i = y * picture.width + x;
                let shade = luma(rgb(pixels[i])) / mean;
                let a = pixels[i][3];
                let [r, g, b] = target.map(|t| texture::byte(t * shade));
                pixels[i] = [r, g, b, a];
            }
        }
    }
    pixels
}

thread_local! {
    static TEXTURES: RefCell<HashMap<(Body, Paint), Texture2D>> = RefCell::new(HashMap::new());
}

/// A body's texture repainted, made once and kept.
fn painted(body: Body, paint: &Paint) -> Texture2D {
    TEXTURES.with(|t| {
        t.borrow_mut()
            .entry((body, *paint))
            .or_insert_with(|| {
                let lib = library();
                let pixels = repaint(body, &lib.bodies[body as usize].picture, paint);
                texture::atlas(ATLAS, ATLAS, &pixels)
            })
            .clone()
    })
}

/// Picks the body, props, colors and build for someone.
pub(super) fn dress(outfit: Outfit, look: &Look) -> Dress {
    let a = look.appearance;
    let seed = look.seed;
    let worn = |s: Slot| look.gear[s.index()].map(|id| rgb(item(id).color));
    let mut paint: Paint = [None; ROLES];
    let set = |paint: &mut Paint, role: Role, c: Color| paint[role as usize] = paint_color(c);
    use Body::*;
    let (body, props): (Body, Vec<Prop>) = match outfit {
        Outfit::Class(class) => {
            let (torso, legs, sleeves, _boots, cape) = class_colors(class);
            set(&mut paint, Role::Main, worn(Slot::Chest).unwrap_or(torso));
            set(&mut paint, Role::Trim, worn(Slot::Hands).unwrap_or(sleeves));
            set(&mut paint, Role::Legs, worn(Slot::Legs).unwrap_or(legs));
            set(&mut paint, Role::Cape, cape.unwrap_or(dark(torso, 0.8)));
            let helmet = worn(Slot::Head).is_some();
            let mut props: Vec<Prop> = match class {
                Class::Barbarian => vec![(Barbarian, "2H_Axe")],
                Class::Fighter => vec![(Knight, "1H_Sword"), (Knight, "Badge_Shield")],
                Class::Paladin => vec![(Knight, "2H_Sword")],
                Class::Monk => vec![],
                Class::Rogue => vec![(Rogue, "Knife"), (Rogue, "Knife_Offhand")],
                Class::Ranger => vec![(Rogue, "2H_Crossbow")],
                Class::Artificer => vec![(Rogue, "1H_Crossbow")],
                Class::Bard => vec![(Mage, "1H_Wand"), (Mage, "Spellbook_open")],
                Class::Cleric => vec![(Barbarian, "1H_Axe"), (Knight, "Round_Shield")],
                Class::Druid => vec![(Mage, "2H_Staff")],
                Class::Mage => vec![(Mage, "2H_Staff")],
                Class::Sorcerer => vec![(Mage, "1H_Wand"), (Mage, "Spellbook")],
                Class::Warlock => vec![(Mage, "1H_Wand"), (Mage, "Spellbook_open")],
            };
            let body = match class {
                Class::Fighter | Class::Paladin => Knight,
                Class::Barbarian | Class::Monk => Barbarian,
                Class::Rogue | Class::Ranger if helmet => RogueHooded,
                Class::Rogue | Class::Ranger | Class::Artificer | Class::Bard => Rogue,
                Class::Cleric | Class::Druid | Class::Mage | Class::Sorcerer | Class::Warlock => {
                    Mage
                }
            };
            if body.has_cape() && (cape.is_some() || body == Rogue) {
                props.push(body.cape());
            }
            if helmet && let Some(hat) = body.hat() {
                props.push(hat);
            }
            (body, props)
        }
        Outfit::Merchant => {
            set(&mut paint, Role::Main, c(0.5, 0.3, 0.2));
            set(&mut paint, Role::Trim, c(0.85, 0.82, 0.72));
            set(&mut paint, Role::Legs, c(0.35, 0.28, 0.22));
            (Barbarian, vec![(Barbarian, "Mug")])
        }
        Outfit::QuestGiver => {
            set(&mut paint, Role::Main, c(0.62, 0.64, 0.68));
            set(&mut paint, Role::Legs, c(0.25, 0.22, 0.2));
            set(&mut paint, Role::Cape, c(0.22, 0.28, 0.5));
            (Knight, vec![(Knight, "Knight_Cape"), (Mage, "Spellbook")])
        }
        Outfit::Mob(style, colors) => {
            let [primary, secondary, _] = colors;
            set(&mut paint, Role::Main, primary);
            set(&mut paint, Role::Trim, dark(primary, 0.8));
            set(&mut paint, Role::Legs, secondary);
            set(&mut paint, Role::Cape, dark(secondary, 0.9));
            match style {
                HumanoidStyle::Bandit => (
                    RogueHooded,
                    vec![
                        (Rogue, "Knife"),
                        (Rogue, "Knife_Offhand"),
                        (Rogue, "Rogue_Cape"),
                    ],
                ),
                HumanoidStyle::Raider => (
                    Barbarian,
                    vec![
                        (Barbarian, "1H_Axe"),
                        (Barbarian, "Barbarian_Round_Shield"),
                        (Barbarian, "Barbarian_Hat"),
                    ],
                ),
                HumanoidStyle::Mystic => (
                    Mage,
                    vec![
                        (Mage, "1H_Wand"),
                        (Mage, "Spellbook_open"),
                        (Mage, "Mage_Hat"),
                    ],
                ),
                HumanoidStyle::Shaman | HumanoidStyle::TrollShaman | HumanoidStyle::TroggShaman => {
                    (Mage, vec![(Mage, "2H_Staff"), (Mage, "Mage_Cape")])
                }
                HumanoidStyle::Satyr | HumanoidStyle::Trickster => {
                    (Rogue, vec![(Rogue, "Knife"), (Rogue, "Throwable")])
                }
                HumanoidStyle::Trogg => (Barbarian, vec![(Barbarian, "1H_Axe")]),
                HumanoidStyle::Troll => (Barbarian, vec![(Barbarian, "2H_Axe")]),
                HumanoidStyle::Skeleton => match seed % 3 {
                    0 => (
                        SkeletonWarrior,
                        vec![
                            (Knight, "1H_Sword"),
                            (Knight, "Round_Shield"),
                            (SkeletonWarrior, "Skeleton_Warrior_Helmet"),
                        ],
                    ),
                    1 => (
                        SkeletonRogue,
                        vec![
                            (Rogue, "Knife"),
                            (Rogue, "Knife_Offhand"),
                            (SkeletonRogue, "Skeleton_Rogue_Hood"),
                            (SkeletonRogue, "Skeleton_Rogue_Cape"),
                        ],
                    ),
                    _ => (SkeletonMinion, vec![(Barbarian, "1H_Axe")]),
                },
                HumanoidStyle::Necromancer => (
                    SkeletonMage,
                    vec![(Mage, "2H_Staff"), (SkeletonMage, "Skeleton_Mage_Hat")],
                ),
            }
        }
        // Giants are still built from shapes; see `humanoid`.
        Outfit::Giant(..) => (Barbarian, vec![]),
    };

    // Skin and hair.
    let person = matches!(
        outfit,
        Outfit::Class(_) | Outfit::Merchant | Outfit::QuestGiver
    );
    let skeletal = matches!(
        body,
        SkeletonMinion | SkeletonWarrior | SkeletonRogue | SkeletonMage
    );
    let skin = match outfit {
        _ if person => Some(skin_color(a.race, a.skin)),
        Outfit::Mob(style, colors) => match style {
            HumanoidStyle::Bandit | HumanoidStyle::Mystic | HumanoidStyle::Raider => {
                Some(skin_color(Race::Human, (seed % 4) as u8))
            }
            HumanoidStyle::Shaman => Some(skin_color(Race::Orc, (seed % 5) as u8)),
            HumanoidStyle::Skeleton | HumanoidStyle::Necromancer => None,
            _ => Some(colors[0]),
        },
        _ => None,
    };
    if let Some(s) = skin {
        set(&mut paint, Role::Skin, s);
    }
    if !skeletal {
        let hair = if person {
            hair_color(a.hair_color)
        } else {
            hair_color((seed / 3 % 4) as u8)
        };
        set(&mut paint, Role::Hair, hair);
    }
    // Some mobs wear their own skin instead of clothes.
    if let Outfit::Mob(
        HumanoidStyle::Satyr
        | HumanoidStyle::Trickster
        | HumanoidStyle::Trogg
        | HumanoidStyle::Troll,
        colors,
    ) = outfit
    {
        set(&mut paint, Role::Main, dark(colors[0], 0.9));
        set(&mut paint, Role::Trim, colors[0]);
    }

    let (race, mut scale, mut width, head) = if person {
        let shape = race_shape(a.race);
        let head = match a.race {
            Race::Goblin | Race::Gnome => 1.12,
            Race::Undead => 0.95,
            _ => 1.0,
        };
        (
            Some(a.race),
            shape.scale,
            shape.limbs.clamp(0.9, 1.12),
            head,
        )
    } else {
        (None, 1.0, 1.0, 1.0)
    };
    if person && a.body == 1 {
        width *= 0.92;
    }
    match outfit {
        Outfit::Mob(HumanoidStyle::Troll | HumanoidStyle::TrollShaman, _) => {
            scale = 1.15;
            width = 1.05;
        }
        Outfit::Mob(HumanoidStyle::Trogg | HumanoidStyle::TroggShaman, _) => {
            scale = 0.85;
            width = 1.12;
        }
        _ => {}
    }
    // Players pick a hairstyle, which is a head from one of the bodies;
    // anyone else wears their body's own.
    let head_from = match (person, body) {
        (true, Knight | Barbarian | Mage | Rogue) => match a.hair_style % Appearance::HAIR_STYLES {
            0 | 4 => Barbarian,
            1 => Knight,
            2 => Rogue,
            _ => Mage,
        },
        _ => body,
    };
    Dress {
        body,
        head_from,
        props,
        paint,
        scale,
        width,
        head,
        race,
    }
}

impl Body {
    /// The cape hanging off its chest, if it has one.
    fn cape(self) -> Prop {
        (
            self,
            match self {
                Body::Knight => "Knight_Cape",
                Body::Barbarian => "Barbarian_Cape",
                Body::Mage => "Mage_Cape",
                _ => "Rogue_Cape",
            },
        )
    }

    fn has_cape(self) -> bool {
        matches!(
            self,
            Body::Knight | Body::Barbarian | Body::Mage | Body::Rogue | Body::RogueHooded
        )
    }

    /// Its helmet or hat, shown when wearing something on the head.
    fn hat(self) -> Option<Prop> {
        match self {
            Body::Knight => Some((self, "Knight_Helmet")),
            Body::Barbarian => Some((self, "Barbarian_Hat")),
            Body::Mage => Some((self, "Mage_Hat")),
            _ => None,
        }
    }
}

/// One animation playing: which clip, how far in (seconds), how strongly,
/// and whether only the upper body follows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Layer {
    pub(super) clip: &'static str,
    pub(super) time: f32,
    pub(super) weight: f32,
    pub(super) upper: bool,
}

fn layer(clip: &'static str, time: f32, weight: f32, upper: bool) -> Layer {
    Layer {
        clip,
        time,
        weight,
        upper,
    }
}

fn duration(clip: &str) -> f32 {
    library().rig.clip(clip).map_or(1.0, |c| c.duration)
}

/// Seconds into a looping clip.
fn looped(clip: &str, time: f32) -> f32 {
    time.rem_euclid(duration(clip).max(0.01))
}

/// The animations someone is playing, base first.
pub(super) fn layers(style: Style, pose: Pose) -> Vec<Layer> {
    let t = pose.time;
    if pose.dead {
        return vec![layer("Death_A_Pose", 0.0, 1.0, false)];
    }
    let mut out = Vec::new();
    // What the legs are doing.
    let base = if pose.airborne {
        "Jump_Idle"
    } else if pose.moving {
        "Running_A"
    } else {
        match style {
            Style::Fists => "Unarmed_Idle",
            Style::TwoHander | Style::Giant => "2H_Melee_Idle",
            _ => "Idle",
        }
    };
    let base_time = if pose.moving && !pose.airborne {
        // One run cycle per full stride.
        (pose.walk / std::f32::consts::TAU).rem_euclid(1.0) * duration(base)
    } else {
        looped(base, t)
    };
    out.push(layer(base, base_time, 1.0, false));
    let upper = pose.moving || pose.airborne;

    if pose.casting {
        let clip = match style {
            Style::Bow => "2H_Ranged_Aiming",
            Style::Rifle => "1H_Ranged_Aiming",
            Style::Holy | Style::Hammer | Style::Nature => "Spellcast_Long",
            _ => "Spellcasting",
        };
        out.push(layer(clip, looped(clip, t), 1.0, upper));
    } else if pose.attacking() {
        let clip = match style {
            Style::TwoHander | Style::Giant => "2H_Melee_Attack_Chop",
            Style::SwordBoard | Style::Brute => {
                if pose.combo.is_multiple_of(2) {
                    "1H_Melee_Attack_Slice_Diagonal"
                } else {
                    "1H_Melee_Attack_Chop"
                }
            }
            Style::Hammer => "2H_Melee_Attack_Slice",
            Style::Fists => match pose.combo % 3 {
                0 => "Unarmed_Melee_Attack_Punch_A",
                1 => "Unarmed_Melee_Attack_Punch_B",
                _ => "Unarmed_Melee_Attack_Kick",
            },
            Style::Daggers => {
                if pose.combo.is_multiple_of(2) {
                    "Dualwield_Melee_Attack_Stab"
                } else {
                    "Dualwield_Melee_Attack_Slice"
                }
            }
            Style::Bow => "2H_Ranged_Shoot",
            Style::Rifle => "1H_Ranged_Shoot",
            Style::Merchant => "Interact",
            _ => "Spellcast_Shoot",
        };
        // The game's swings are quick: play the clip's strike and ease out
        // of it before its slow recovery.
        let s = pose.swing;
        let weight = (s / 0.08).min(1.0) * ((1.0 - s) / 0.25).min(1.0);
        let kick = clip == "Unarmed_Melee_Attack_Kick";
        out.push(layer(
            clip,
            s * 0.75 * duration(clip),
            weight,
            upper && !kick,
        ));
    }
    if pose.hurt > 0.0 {
        let k = pose.hurt.min(1.0);
        out.push(layer(
            "Hit_A",
            (1.0 - k) * 0.5 * duration("Hit_A"),
            k * 0.6,
            true,
        ));
    }
    out
}

/// The skeleton posed by `layers`, one transform per skeleton node.
fn posed(lib: &Library, layers: &[Layer]) -> Vec<Transform> {
    let mut pose = lib.rig.rest_pose();
    for l in layers {
        let Some(clip) = lib.rig.clip(l.clip) else {
            continue;
        };
        if l.weight >= 1.0 && !l.upper {
            clip.apply(l.time, &mut pose);
            continue;
        }
        let mut target = pose.clone();
        clip.apply(l.time, &mut target);
        for (i, (p, t)) in pose.iter_mut().zip(&target).enumerate() {
            if !l.upper || lib.upper[i] {
                *p = p.lerp(t, l.weight.clamp(0.0, 1.0));
            }
        }
    }
    pose
}

fn is_hat(name: &str) -> bool {
    name.ends_with("_Hat") || name.ends_with("_Helmet") || name.ends_with("_Hood")
}

/// Whether a part is a body's head (with its face and hair).
fn is_head(part: &ModelPart) -> bool {
    part.skin.is_some() && part.name.ends_with("_Head")
}

/// How tall the bodies are, in their files' units.
const BODY_HEIGHT: f32 = 2.2;

pub(super) fn draw(b: &mut Batch, look: &Look, outfit: Outfit, pos: Vec3, yaw: f32, pose: Pose) {
    let lib = library();
    let dress = dress(outfit, look);
    let style = style_of(outfit);
    let mut skeleton = posed(lib, &layers(style, pose));
    if let Some(head) = lib.rig.node("head") {
        skeleton[head].scale *= dress.head;
    }
    let skeleton = lib.rig.world(&skeleton);
    let size = 2.1 / BODY_HEIGHT * dress.scale;
    let transform = Mat4::from_translation(pos)
        * Mat4::from_rotation_y(yaw)
        * Mat4::from_scale(vec3(size * dress.width, size, size * dress.width));
    // A flush of red when hit.
    let hurt = if pose.dead { 0.0 } else { pose.hurt };
    let tint = Color::new(1.0, 1.0 - hurt * 0.35, 1.0 - hurt * 0.4, 1.0);

    let body = &lib.bodies[dress.body as usize];
    let world = body.model.world_on(&skeleton, &body.binding);
    let texture = painted(dress.body, &dress.paint);
    b.textured(&texture, |b| {
        body.model.draw_posed(b, transform, &world, |part| {
            if is_head(part) && dress.head_from != dress.body {
                return None;
            }
            let shown = part.skin.is_some()
                || dress
                    .props
                    .iter()
                    .any(|&(from, name)| from == dress.body && name == part.name);
            shown.then_some(tint)
        });
    });
    if dress.head_from != dress.body {
        let other = &lib.bodies[dress.head_from as usize];
        let world = other.model.world_on(&skeleton, &other.binding);
        let texture = painted(dress.head_from, &dress.paint);
        b.textured(&texture, |b| {
            other
                .model
                .draw_posed(b, transform, &world, |part| is_head(part).then_some(tint));
        });
    }
    // Props from other bodies' files, with their own textures.
    for &(from, name) in &dress.props {
        if from == dress.body {
            continue;
        }
        let other = &lib.bodies[from as usize];
        let world = other.model.world_on(&skeleton, &other.binding);
        let texture = painted(from, &[None; ROLES]);
        b.textured(&texture, |b| {
            other.model.draw_posed(b, transform, &world, |part| {
                (part.name == name).then_some(tint)
            });
        });
    }

    let at = |name: &str| {
        lib.rig
            .node(name)
            .map(|i| transform * skeleton[i])
            .unwrap_or(transform)
    };
    // Ears and tusks.
    if let Some(race) = dress.race {
        let head = at("head");
        let skin = skin_color(race, look.appearance.skin);
        let ear = |b: &mut Batch, side: f32, dir: Vec3, radius: f32| {
            let base = head.transform_point3(vec3(0.5 * side, 0.45, -0.02));
            let tip = head.transform_vector3(dir * vec3(side, 1.0, 1.0));
            b.cone(base, tip, radius * size * dress.head, 0.0, 8, skin);
        };
        for side in [-1.0, 1.0] {
            match race {
                Race::Elf => ear(b, side, vec3(0.42, 0.26, -0.16), 0.09),
                Race::Goblin => ear(b, side, vec3(0.6, 0.12, -0.08), 0.15),
                Race::Gnome => ear(b, side, vec3(0.16, 0.06, -0.04), 0.1),
                Race::Orc => {
                    ear(b, side, vec3(0.2, 0.1, -0.06), 0.1);
                    let tusk = head.transform_point3(vec3(0.17 * side, 0.12, 0.42));
                    b.cone(
                        tusk,
                        head.transform_vector3(vec3(0.02 * side, 0.16, 0.03)),
                        0.04 * size * 2.0,
                        0.0,
                        6,
                        BONE,
                    );
                }
                _ => {}
            }
        }
    }
    // A mohawk is a crest of hair on a bald head.
    let mohawk = matches!(outfit, Outfit::Class(_)) && look.appearance.hair_style == 4;
    if mohawk && dress.head_from == Body::Barbarian && !dress.props.iter().any(|p| is_hat(p.1)) {
        let head = at("head");
        let hair = hair_color(look.appearance.hair_color);
        for k in 0..6 {
            let a = -1.1 + k as f32 * 0.38;
            let base = head.transform_point3(vec3(0.0, 0.5 + a.cos() * 0.42, a.sin() * 0.47));
            let up = head.transform_vector3(vec3(0.0, a.cos(), a.sin()) * 0.26);
            b.cone(base, up, 0.12 * size * dress.head, 0.0, 6, hair);
        }
    }
    // Spell light gathering in the hands while casting.
    if pose.casting && !pose.dead {
        let color = cast_color(style);
        let pulse = (0.06 + (pose.time * 8.0).sin().abs() * 0.04) * size * 1.4;
        for hand in ["hand.l", "hand.r"] {
            let p = at(hand).transform_point3(vec3(0.0, 0.1, 0.0));
            b.glow_sphere(p, pulse, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(kind: EntityKind) -> Look {
        Look {
            kind,
            appearance: Appearance::default(),
            gear: [None; 5],
            seed: 7,
        }
    }

    #[test]
    fn every_body_loads_on_the_shared_skeleton() {
        let lib = library();
        for (body, loaded) in Body::ALL.iter().zip(&lib.bodies) {
            assert!(!loaded.model.parts.is_empty(), "{body:?}");
            // Every bone the skins bend with is in the skeleton.
            for skin in &loaded.model.skins {
                for &j in &skin.joints {
                    assert!(
                        loaded.binding[j].is_some(),
                        "{body:?}: {}",
                        loaded.model.nodes[j].name
                    );
                }
            }
        }
    }

    #[test]
    fn every_prop_and_clip_used_exists() {
        let lib = library();
        let mut outfits: Vec<Outfit> = Class::ALL.iter().map(|&c| Outfit::Class(c)).collect();
        outfits.extend([Outfit::Merchant, Outfit::QuestGiver]);
        let colors = [WHITE; 3];
        for style in [
            HumanoidStyle::Bandit,
            HumanoidStyle::Mystic,
            HumanoidStyle::Raider,
            HumanoidStyle::Shaman,
            HumanoidStyle::Satyr,
            HumanoidStyle::Trickster,
            HumanoidStyle::Trogg,
            HumanoidStyle::TroggShaman,
            HumanoidStyle::Troll,
            HumanoidStyle::TrollShaman,
            HumanoidStyle::Skeleton,
            HumanoidStyle::Necromancer,
        ] {
            outfits.push(Outfit::Mob(style, colors));
        }
        for outfit in outfits {
            for seed in 0..3 {
                let mut l = look(EntityKind::Player(Class::Mage));
                l.seed = seed;
                // With and without a helmet.
                for head in [None, Some(items::LINEN_HOOD)] {
                    l.gear[Slot::Head.index()] = head;
                    let d = dress(outfit, &l);
                    for (from, name) in &d.props {
                        let model = &lib.bodies[*from as usize].model;
                        assert!(
                            model.parts.iter().any(|p| p.name == *name),
                            "{from:?} has no {name}"
                        );
                    }
                }
            }
            let style = style_of(outfit);
            for pose in [
                Pose::default(),
                Pose {
                    moving: true,
                    ..Default::default()
                },
                Pose {
                    casting: true,
                    ..Default::default()
                },
                Pose {
                    swing: 0.5,
                    hurt: 0.5,
                    ..Default::default()
                },
                Pose {
                    swing: 0.5,
                    combo: 1,
                    ..Default::default()
                },
                Pose {
                    swing: 0.5,
                    combo: 2,
                    ..Default::default()
                },
                Pose {
                    dead: true,
                    ..Default::default()
                },
                Pose {
                    airborne: true,
                    ..Default::default()
                },
            ] {
                for l in layers(style, pose) {
                    assert!(lib.rig.clip(l.clip).is_some(), "no clip {}", l.clip);
                }
            }
        }
    }

    #[test]
    fn attacks_take_over_the_upper_body_while_running() {
        let pose = Pose {
            moving: true,
            swing: 0.4,
            walk: 2.0,
            ..Default::default()
        };
        let l = layers(Style::SwordBoard, pose);
        assert_eq!(l[0].clip, "Running_A");
        assert!(l[1].upper && l[1].weight > 0.9);
        // Standing still, the whole body swings.
        let l = layers(
            Style::SwordBoard,
            Pose {
                moving: false,
                ..pose
            },
        );
        assert!(!l[1].upper);

        let lib = library();
        let running = posed(lib, &layers(Style::SwordBoard, Pose { swing: 0.0, ..pose }));
        let both = posed(lib, &layers(Style::SwordBoard, pose));
        let leg = lib.rig.node("upperleg.l").unwrap();
        let arm = lib.rig.node("upperarm.r").unwrap();
        assert_eq!(running[leg], both[leg]);
        assert_ne!(running[arm], both[arm]);
    }

    #[test]
    fn repainting_keeps_the_shading() {
        let lib = library();
        let mut paint: Paint = [None; ROLES];
        paint[Role::Skin as usize] = Some([60, 160, 60]);
        let picture = &lib.bodies[Body::Knight as usize].picture;
        let pixels = repaint(Body::Knight, picture, &paint);
        // The skin cell turned green, lighter at the top than the bottom.
        let (top, bottom) = (pixels[4 * ATLAS + 8], pixels[56 * ATLAS + 8]);
        assert!(top[1] > top[0] && top[1] > top[2], "{top:?}");
        assert!(top[1] > bottom[1], "{top:?} {bottom:?}");
        // Other cells are untouched.
        let far = 3 * (ATLAS / 4) * ATLAS + 200;
        assert_eq!(pixels[far], picture.pixels[far]);
    }

    #[test]
    fn races_and_classes_look_different() {
        let mut l = look(EntityKind::Player(Class::Fighter));
        let human = dress(Outfit::Class(Class::Fighter), &l);
        l.appearance.race = Race::Orc;
        let orc = dress(Outfit::Class(Class::Fighter), &l);
        assert_ne!(
            human.paint[Role::Skin as usize],
            orc.paint[Role::Skin as usize]
        );
        assert!(orc.width > human.width);
        let mage = dress(Outfit::Class(Class::Mage), &l);
        assert_eq!(mage.body, Body::Mage);
        assert_ne!(
            mage.paint[Role::Main as usize],
            orc.paint[Role::Main as usize]
        );
        // A helmet shows when one is worn.
        assert!(!orc.props.iter().any(|p| p.1 == "Knight_Helmet"));
        l.gear[Slot::Head.index()] = Some(items::LINEN_HOOD);
        let helmed = dress(Outfit::Class(Class::Fighter), &l);
        assert!(helmed.props.iter().any(|p| p.1 == "Knight_Helmet"));
    }
}
