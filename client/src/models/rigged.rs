//! People as rigged, animated models: players, townsfolk and humanoid
//! mobs. The bodies, hair and most animations come from Quaternius'
//! Universal Base Characters and Universal Animation Library; the weapons,
//! hats, skeleton mobs and the animations Quaternius lacks from the KayKit
//! character packs, moved onto the Quaternius skeleton. All are CC0; see
//! `client/assets/characters/`.
//!
//! Every body shares one skeleton and one set of animation clips (from
//! `rig.glb`); a clip only turns bones, so it plays on the male and female
//! bodies alike. A character is drawn by picking a body, a hairstyle and
//! what it carries (`dress`), painting its clothes onto its skin texture
//! (`Garb`), posing the skeleton from what it's doing (`layers`), and
//! drawing the body's meshes bent around that pose.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use super::*;
use crate::gfx::{ModelFile, Picture, Transform, texture};

/// The model files, in `client/assets/characters/`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum File {
    Male,
    Female,
    /// Weapons, shields and hats, each hanging off the bone that holds it.
    Props,
    /// The skeleton mobs and their gear.
    Skeletons,
}

impl File {
    const ALL: [File; 4] = [File::Male, File::Female, File::Props, File::Skeletons];

    fn bytes(self) -> &'static [u8] {
        match self {
            File::Male => include_bytes!("../../assets/characters/male.glb"),
            File::Female => include_bytes!("../../assets/characters/female.glb"),
            File::Props => include_bytes!("../../assets/characters/props.glb"),
            File::Skeletons => include_bytes!("../../assets/characters/skeletons.glb"),
        }
    }
}

/// Whose body someone has.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Body {
    Male,
    Female,
    SkeletonMinion,
    SkeletonWarrior,
    SkeletonRogue,
    SkeletonMage,
}

impl Body {
    fn file(self) -> File {
        match self {
            Body::Male => File::Male,
            Body::Female => File::Female,
            _ => File::Skeletons,
        }
    }

    /// Whether a skinned part of the body's file belongs to this body.
    fn wears(self, part: &str) -> bool {
        match self {
            Body::Male | Body::Female => matches!(part, "Body" | "Eyes" | "Brows"),
            Body::SkeletonMinion => part.starts_with("Skeleton_Minion_"),
            Body::SkeletonWarrior => part.starts_with("Skeleton_Warrior_"),
            Body::SkeletonRogue => part.starts_with("Skeleton_Rogue_"),
            Body::SkeletonMage => part.starts_with("Skeleton_Mage_"),
        }
    }
}

/// A rigid part shown on someone: a weapon, shield or hat.
pub(super) type Prop = (File, &'static str);

/// What a piece of clothing is made of, which changes how it shines.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Stuff {
    Cloth,
    Leather,
    Metal,
}

/// A piece of clothing's color and stuff, or bare skin (`None`).
pub(super) type Fill = Option<([u8; 3], Stuff)>;

/// Clothes, painted onto the body's skin, and the colors of skin and hair.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) struct Garb {
    pub(super) skin: [u8; 3],
    pub(super) hair: [u8; 3],
    pub(super) chest: Fill,
    /// Upper arms.
    pub(super) sleeves: Fill,
    pub(super) forearms: Fill,
    pub(super) hands: Fill,
    /// Hips and legs down to the boots.
    pub(super) legs: Fill,
    pub(super) boots: Fill,
    pub(super) belt: Fill,
    /// A panel of cloth down the front and back.
    pub(super) tabard: Fill,
}

fn bytes(c: Color) -> [u8; 3] {
    [c.r, c.g, c.b].map(texture::byte)
}

fn cloth(c: Color) -> Fill {
    Some((bytes(c), Stuff::Cloth))
}

fn leather(c: Color) -> Fill {
    Some((bytes(c), Stuff::Leather))
}

fn metal(c: Color) -> Fill {
    Some((bytes(c), Stuff::Metal))
}

/// How to draw one person.
#[derive(Clone, Debug)]
pub(super) struct Dress {
    pub(super) body: Body,
    pub(super) garb: Garb,
    /// Hair meshes worn (from the body's file).
    pub(super) hair: Vec<&'static str>,
    pub(super) props: Vec<Prop>,
    /// Overall height, relative to a human.
    pub(super) scale: f32,
    /// Breadth, relative to height.
    pub(super) width: f32,
    /// Head size.
    pub(super) head: f32,
    /// For ears and tusks.
    pub(super) race: Option<Race>,
    pub(super) mohawk: bool,
    /// Shoulder guards: their color and size.
    pub(super) pauldrons: Option<(Color, f32)>,
    /// A long robe's skirt: its color and its hem's.
    pub(super) robe: Option<(Color, Color)>,
}

/// Which part of the body a spot on the skin texture is on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Region {
    /// Not on the body (eyes, unused texture).
    None,
    Head,
    Chest,
    UpperArm,
    Forearm,
    Hand,
    Hips,
    Thigh,
    Calf,
    Foot,
    Hair,
}

/// Where each spot of a body's texture is, found once from its mesh.
struct Map {
    region: Vec<Region>,
    /// Where on the body it is in the bind pose (as shares of the body's
    /// height; y up from the feet, x to the left, z forward).
    spot: Vec<Vec3>,
    /// Spots painted grey in the original (underwear).
    pale: Vec<bool>,
    /// Average brightness of the skin and the hair, to shade against.
    skin_mean: f32,
    hair_mean: f32,
}

struct Loaded {
    model: ModelFile,
    /// For each of the file's nodes, the skeleton node it is.
    binding: Vec<Option<usize>>,
    /// The file's rest pose and where that puts each skeleton node (in
    /// skeleton node order): a body's own proportions.
    rest: Vec<Transform>,
    rest_world: Vec<Mat4>,
    /// The texture, shrunk to `ATLAS` square.
    picture: Picture,
    map: Option<Map>,
}

struct Library {
    rig: ModelFile,
    /// The skeleton's nodes from the spine up (spine, arms and head), which
    /// an attack can take over while the legs keep running.
    upper: Vec<bool>,
    files: Vec<Loaded>,
}

/// The textures are repainted at this many pixels square.
const ATLAS: usize = 512;

/// Where boots end and the belt sits, as a share of the body's height.
const BOOT_TOP: f32 = 0.27;
const BELT: (f32, f32) = (0.545, 0.575);
/// A tabard's half width, bottom and top, front and back.
const TABARD: (f32, f32, f32) = (0.05, 0.4, 0.76);

fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let rig = ModelFile::from_glb(include_bytes!("../../assets/characters/rig.glb"))
            .expect("the character skeleton loads");
        let spine = rig.node("spine_01");
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
        let files = File::ALL
            .iter()
            .map(|file| {
                let model = ModelFile::from_glb(file.bytes()).expect("a character file loads");
                let binding = model.bind(&rig);
                let mut rest = rig.rest_pose();
                let own = model.rest_pose();
                for (i, b) in binding.iter().enumerate() {
                    if let Some(r) = b {
                        rest[*r] = own[i];
                    }
                }
                let rest_world = rig.world(&rest);
                let picture = shrink(
                    model
                        .picture
                        .as_ref()
                        .expect("character files have a texture"),
                );
                let map = matches!(file, File::Male | File::Female).then(|| map(&model, &picture));
                Loaded {
                    model,
                    binding,
                    rest,
                    rest_world,
                    picture,
                    map,
                }
            })
            .collect();
        Library { rig, upper, files }
    })
}

/// Box-filters a picture down to `ATLAS` square.
fn shrink(p: &Picture) -> Picture {
    let (sx, sy) = ((p.width / ATLAS).max(1), (p.height / ATLAS).max(1));
    let mut pixels = Vec::with_capacity(ATLAS * ATLAS);
    for y in 0..ATLAS {
        for x in 0..ATLAS {
            let mut sum = [0u32; 4];
            for j in 0..sy {
                for i in 0..sx {
                    let px =
                        p.pixels[((y * sy + j) * p.width + x * sx + i).min(p.pixels.len() - 1)];
                    for k in 0..4 {
                        sum[k] += px[k] as u32;
                    }
                }
            }
            let n = (sx * sy) as u32;
            pixels.push(sum.map(|s| (s / n) as u8));
        }
    }
    Picture {
        width: ATLAS,
        height: ATLAS,
        pixels,
    }
}

fn luma(p: [u8; 4]) -> f32 {
    (p[0] as f32 * 0.3 + p[1] as f32 * 0.59 + p[2] as f32 * 0.11) / 255.0
}

/// Which region a bone's skin belongs to.
fn region_of(bone: &str) -> Region {
    let side = |s: &str| bone.starts_with(s);
    if bone == "Head" || bone == "neck_01" {
        Region::Head
    } else if side("upperarm") {
        Region::UpperArm
    } else if side("lowerarm") {
        Region::Forearm
    } else if side("hand")
        || ["index", "middle", "ring", "pinky", "thumb"]
            .iter()
            .any(|f| bone.starts_with(f))
    {
        Region::Hand
    } else if bone == "pelvis" {
        Region::Hips
    } else if side("thigh") {
        Region::Thigh
    } else if side("calf") {
        Region::Calf
    } else if side("foot") || side("ball") {
        Region::Foot
    } else {
        Region::Chest
    }
}

/// Finds which part of the body each spot of the texture covers, by
/// drawing the body's triangles flat onto the texture.
fn map(model: &ModelFile, picture: &Picture) -> Map {
    let n = ATLAS * ATLAS;
    let mut region = vec![Region::None; n];
    let mut spot = vec![Vec3::ZERO; n];
    let body_height = model
        .parts
        .iter()
        .filter(|p| p.name == "Body")
        .flat_map(|p| p.positions.iter().map(|v| v.y))
        .fold(0.01f32, f32::max);
    for part in &model.parts {
        let fixed = match part.name.as_str() {
            "Body" => None,
            "Eyes" => continue,
            _ => Some(Region::Hair),
        };
        let joints = part.skin.map(|s| &model.skins[s].joints);
        let vertex_region = |v: usize| {
            if let Some(r) = fixed {
                return r;
            }
            let (Some(joints), Some(j), Some(w)) =
                (joints, part.joints.get(v), part.weights.get(v))
            else {
                return Region::Chest;
            };
            let k = (0..4).max_by(|&a, &b| w[a].total_cmp(&w[b])).unwrap_or(0);
            region_of(&model.nodes[joints[j[k] as usize]].name)
        };
        for tri in part.indices.as_chunks::<3>().0 {
            let v = tri.map(|i| i as usize);
            let uv = v.map(|i| part.uvs[i] * ATLAS as f32);
            let regions = v.map(vertex_region);
            let spots = v.map(|i| part.positions[i] / body_height);
            let (lo, hi) = (
                uv[0].min(uv[1]).min(uv[2]).floor().max(Vec2::ZERO),
                uv[0].max(uv[1]).max(uv[2]).ceil(),
            );
            let area = (uv[1] - uv[0]).perp_dot(uv[2] - uv[0]);
            if area.abs() < 1e-6 {
                continue;
            }
            for y in lo.y as usize..(hi.y as usize).min(ATLAS) {
                for x in lo.x as usize..(hi.x as usize).min(ATLAS) {
                    let p = vec2(x as f32 + 0.5, y as f32 + 0.5);
                    let w = [
                        (uv[2] - uv[1]).perp_dot(p - uv[1]) / area,
                        (uv[0] - uv[2]).perp_dot(p - uv[2]) / area,
                        (uv[1] - uv[0]).perp_dot(p - uv[0]) / area,
                    ];
                    if w.iter().any(|&w| w < -0.02) {
                        continue;
                    }
                    let i = y * ATLAS + x;
                    let k = (0..3).max_by(|&a, &b| w[a].total_cmp(&w[b])).unwrap_or(0);
                    region[i] = regions[k];
                    spot[i] = spots[0] * w[0] + spots[1] * w[1] + spots[2] * w[2];
                }
            }
        }
    }
    // Grow each region a few spots, so filtering at seams doesn't pick up
    // what's beside them.
    for _ in 0..3 {
        let before = region.clone();
        let spots = spot.clone();
        for y in 0..ATLAS {
            for x in 0..ATLAS {
                let i = y * ATLAS + x;
                if before[i] != Region::None {
                    continue;
                }
                let around = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)];
                for (dx, dy) in around {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= ATLAS as i32 || ny >= ATLAS as i32 {
                        continue;
                    }
                    let j = ny as usize * ATLAS + nx as usize;
                    if before[j] != Region::None {
                        region[i] = before[j];
                        spot[i] = spots[j];
                        break;
                    }
                }
            }
        }
    }
    let pale: Vec<bool> = picture
        .pixels
        .iter()
        .map(|p| {
            let max = p[0].max(p[1]).max(p[2]) as f32;
            let min = p[0].min(p[1]).min(p[2]) as f32;
            max > 0.0 && (max - min) / max < 0.2
        })
        .collect();
    let mean = |want: &dyn Fn(usize) -> bool| {
        let (sum, count) = (0..n)
            .filter(|&i| want(i))
            .fold((0.0, 0usize), |(s, c), i| {
                (s + luma(picture.pixels[i]), c + 1)
            });
        (sum / count.max(1) as f32).max(0.05)
    };
    let skin_mean = mean(&|i| region[i] == Region::Head && !pale[i]);
    let hair_mean = mean(&|i| region[i] == Region::Hair);
    Map {
        region,
        spot,
        pale,
        skin_mean,
        hair_mean,
    }
}

/// Paints a body's clothes, skin and hair onto its texture, keeping the
/// light and shade painted into it.
fn repaint(picture: &Picture, map: &Map, garb: &Garb) -> Vec<[u8; 4]> {
    let n = ATLAS * ATLAS;
    let mut pixels = picture.pixels.clone();
    // Which piece of clothing each spot got, to outline their edges.
    let mut piece = vec![0u8; n];
    for i in 0..n {
        let region = map.region[i];
        if region == Region::None {
            continue;
        }
        let at = map.spot[i];
        let h = at.y;
        let pale = map.pale[i];
        let (fill, id): (Fill, u8) = match region {
            Region::None => continue,
            Region::Hair => {
                let shade = luma(pixels[i]) / map.hair_mean;
                let [r, g, b] = garb.hair.map(|c| texture::byte(c as f32 / 255.0 * shade));
                pixels[i] = [r, g, b, 255];
                continue;
            }
            Region::Head => (None, 0),
            Region::Chest | Region::Hips
                if garb.belt.is_some() && (BELT.0..BELT.1).contains(&h) =>
            {
                (garb.belt, 7)
            }
            Region::Chest | Region::Hips | Region::Thigh
                if garb.tabard.is_some()
                    && at.x.abs() < TABARD.0
                    && (TABARD.1..TABARD.2).contains(&h) =>
            {
                (garb.tabard, 8)
            }
            Region::Chest => match garb.chest {
                Some(_) => (garb.chest, 1),
                // A bare chest keeps its underwear, in the trousers' color.
                None if pale => (garb.legs, 5),
                None => (None, 0),
            },
            Region::UpperArm => (garb.sleeves, 2),
            Region::Forearm => (garb.forearms, 3),
            Region::Hand => (garb.hands, 4),
            Region::Hips | Region::Thigh => (garb.legs, 5),
            Region::Calf if h < BOOT_TOP && garb.boots.is_some() => (garb.boots, 6),
            Region::Calf => (garb.legs, 5),
            Region::Foot => (garb.boots, 6),
        };
        let shade = luma(pixels[i]) / map.skin_mean;
        let color = match fill {
            None => {
                piece[i] = 0;
                garb.skin.map(|c| c as f32 / 255.0 * shade)
            }
            Some((c, stuff)) => {
                piece[i] = id;
                // Clothes keep only some of the painted muscle, and a little
                // weave.
                let x = (i % ATLAS) as i32;
                let y = (i / ATLAS) as i32;
                let grain = texture::hash(x, y, id as u32) - 0.5;
                let k = match stuff {
                    Stuff::Cloth => 0.92 + (shade - 1.0) * 0.35 + grain * 0.08,
                    Stuff::Leather => 0.9 + (shade - 1.0) * 0.45 + grain * 0.12,
                    Stuff::Metal => 1.05 + (shade - 1.0) * 0.8 + grain * 0.03,
                };
                // Plate is made of overlapping bands.
                let banded = stuff == Stuff::Metal
                    && matches!(region, Region::Chest | Region::Hips | Region::Thigh)
                    && h < 0.7
                    && (h / 0.034).fract() < 0.14;
                let k = if banded { k * 0.7 } else { k };
                c.map(|c| c as f32 / 255.0 * k)
            }
        };
        let [r, g, b] = color.map(texture::byte);
        pixels[i] = [r, g, b, 255];
    }
    // Dark seams where one piece of clothing meets another, or skin.
    let mut out = pixels.clone();
    for y in 1..ATLAS - 1 {
        for x in 1..ATLAS - 1 {
            let i = y * ATLAS + x;
            if map.region[i] == Region::None || map.region[i] == Region::Hair {
                continue;
            }
            let edge = [i - 1, i + 1, i - ATLAS, i + ATLAS].iter().any(|&j| {
                map.region[j] != Region::None
                    && map.region[j] != Region::Hair
                    && piece[j] != piece[i]
            });
            if edge {
                let p = pixels[i];
                out[i] = [
                    (p[0] as f32 * 0.55) as u8,
                    (p[1] as f32 * 0.55) as u8,
                    (p[2] as f32 * 0.55) as u8,
                    255,
                ];
            }
        }
    }
    out
}

thread_local! {
    static TEXTURES: RefCell<HashMap<(Body, Option<Garb>), Texture2D>> =
        RefCell::new(HashMap::new());
}

/// A body's texture with its garb painted on (or a file's own, for `None`),
/// made once and kept.
fn painted(body: Body, garb: Option<Garb>) -> Texture2D {
    TEXTURES.with(|t| {
        t.borrow_mut()
            .entry((body, garb))
            .or_insert_with(|| {
                let lib = library();
                let file = &lib.files[body.file() as usize];
                let pixels = match (garb, &file.map) {
                    (Some(garb), Some(map)) => repaint(&file.picture, map, &garb),
                    _ => file.picture.pixels.clone(),
                };
                texture::atlas(ATLAS, ATLAS, &pixels)
            })
            .clone()
    })
}

fn file_texture(file: File) -> Texture2D {
    let body = match file {
        File::Male => Body::Male,
        File::Female => Body::Female,
        // Props share no body, so key them by one that isn't theirs.
        File::Props => return props_texture(),
        File::Skeletons => Body::SkeletonMinion,
    };
    painted(body, None)
}

fn props_texture() -> Texture2D {
    thread_local! {
        static PROPS: Texture2D = {
            let lib = library();
            texture::atlas(ATLAS, ATLAS, &lib.files[File::Props as usize].picture.pixels)
        };
    }
    PROPS.with(|t| t.clone())
}

/// The class's usual clothes.
fn class_garb(class: Class) -> (Fill, Fill, Fill, Fill, Fill, Fill, Fill) {
    let (torso, legs, sleeves, boots, _) = class_colors(class);
    let brown = dark(LEATHER, 0.8);
    // chest, sleeves, forearms, hands, legs, boots, belt
    match class {
        Class::Fighter | Class::Paladin => (
            metal(torso),
            metal(sleeves),
            metal(sleeves),
            metal(dark(sleeves, 0.85)),
            metal(legs),
            metal(dark(legs, 0.6)),
            leather(brown),
        ),
        Class::Barbarian => (
            None,
            None,
            leather(sleeves),
            None,
            leather(legs),
            leather(boots),
            leather(dark(torso, 0.7)),
        ),
        Class::Monk => (
            cloth(torso),
            None,
            cloth(c(0.9, 0.86, 0.75)),
            cloth(c(0.9, 0.86, 0.75)),
            cloth(dark(legs, 0.5)),
            None,
            cloth(c(0.9, 0.86, 0.75)),
        ),
        Class::Rogue | Class::Ranger | Class::Artificer | Class::Bard => (
            leather(torso),
            leather(sleeves),
            leather(dark(sleeves, 0.85)),
            leather(boots),
            cloth(legs),
            leather(boots),
            leather(brown),
        ),
        Class::Cleric => (
            cloth(torso),
            cloth(sleeves),
            cloth(sleeves),
            leather(boots),
            cloth(legs),
            leather(boots),
            cloth(c(0.85, 0.7, 0.3)),
        ),
        Class::Druid | Class::Mage | Class::Sorcerer | Class::Warlock => (
            cloth(torso),
            cloth(sleeves),
            cloth(sleeves),
            None,
            cloth(legs),
            leather(boots),
            cloth(dark(torso, 0.6)),
        ),
    }
}

/// Picks the body, clothes, props and build for someone.
pub(super) fn dress(outfit: Outfit, look: &Look) -> Dress {
    let a = look.appearance;
    let seed = look.seed;
    let worn = |s: Slot| look.gear[s.index()].map(|id| rgb(item(id).color));
    let person = matches!(
        outfit,
        Outfit::Class(_) | Outfit::Merchant | Outfit::QuestGiver
    );
    let sex = if person { a.body } else { (seed / 7 % 2) as u8 };
    let human = if sex == 1 { Body::Female } else { Body::Male };
    let skin = bytes(if person {
        skin_color(a.race, a.skin)
    } else {
        skin_color(Race::Human, (seed % 4) as u8)
    });
    let hair_color = bytes(hair_color(if person {
        a.hair_color
    } else {
        (seed / 3 % 4) as u8
    }));
    let mut garb = Garb {
        skin,
        hair: hair_color,
        chest: None,
        sleeves: None,
        forearms: None,
        hands: None,
        legs: None,
        boots: None,
        belt: None,
        tabard: None,
    };
    use File::Props as P;
    let mut body = human;
    let mut pauldrons = None;
    let mut robe = None;
    let props: Vec<Prop> = match outfit {
        Outfit::Class(class) => {
            let (chest, sleeves, forearms, hands, legs, boots, belt) = class_garb(class);
            let stuff = |f: Fill| f.map_or(Stuff::Cloth, |f| f.1);
            // Worn armor takes its item's color.
            let wear = |f: Fill, slot: Slot, bare: Stuff| match worn(slot) {
                Some(c) => Some((bytes(c), f.map_or(bare, |f| f.1))),
                None => f,
            };
            garb.chest = wear(chest, Slot::Chest, Stuff::Leather);
            garb.sleeves = sleeves.or(garb.chest.filter(|_| worn(Slot::Chest).is_some()));
            garb.forearms = forearms;
            garb.hands = wear(hands, Slot::Hands, Stuff::Leather);
            if worn(Slot::Hands).is_some() {
                garb.forearms = garb.hands;
            }
            garb.legs = wear(legs, Slot::Legs, stuff(legs));
            garb.boots = wear(boots, Slot::Feet, Stuff::Leather);
            garb.belt = belt;
            let (torso, _, sleeve, _, cape) = class_colors(class);
            if matches!(class, Class::Fighter | Class::Paladin | Class::Cleric) {
                garb.tabard = cape.and_then(cloth);
            }
            pauldrons = match class {
                Class::Fighter | Class::Paladin => {
                    Some((dark(worn(Slot::Chest).unwrap_or(sleeve), 0.8), 1.2))
                }
                Class::Rogue | Class::Ranger | Class::Artificer | Class::Bard => {
                    Some((dark(worn(Slot::Chest).unwrap_or(torso), 0.85), 0.75))
                }
                _ => None,
            };
            if robed(class) {
                let color = worn(Slot::Chest).unwrap_or(torso);
                robe = Some((color, cape.unwrap_or(dark(color, 0.6))));
            }
            let mut props: Vec<Prop> = match class {
                Class::Barbarian => vec![(P, "2H_Axe")],
                Class::Fighter => vec![(P, "1H_Sword"), (P, "Badge_Shield")],
                Class::Paladin => vec![(P, "2H_Sword")],
                Class::Monk => vec![],
                Class::Rogue => vec![(P, "Knife"), (P, "Knife_Offhand")],
                Class::Ranger => vec![(P, "2H_Crossbow")],
                Class::Artificer => vec![(P, "1H_Crossbow")],
                Class::Bard => vec![(P, "1H_Wand"), (P, "Spellbook_open")],
                Class::Cleric => vec![(P, "1H_Axe"), (P, "Round_Shield")],
                Class::Druid | Class::Mage => vec![(P, "2H_Staff")],
                Class::Sorcerer | Class::Warlock => vec![(P, "1H_Wand"), (P, "Spellbook")],
            };
            if worn(Slot::Head).is_some() {
                props.push(match class {
                    Class::Fighter | Class::Paladin | Class::Cleric => (P, "Knight_Helmet"),
                    Class::Barbarian | Class::Monk => (P, "Barbarian_Hat"),
                    Class::Rogue | Class::Ranger | Class::Artificer | Class::Bard => {
                        (File::Skeletons, "Skeleton_Rogue_Hood")
                    }
                    _ => (P, "Mage_Hat"),
                });
            }
            props
        }
        Outfit::Merchant => {
            garb.chest = cloth(c(0.5, 0.3, 0.2));
            garb.sleeves = cloth(c(0.85, 0.82, 0.72));
            garb.forearms = garb.sleeves;
            garb.legs = cloth(c(0.35, 0.28, 0.22));
            garb.boots = leather(dark(LEATHER, 0.8));
            garb.belt = leather(c(0.92, 0.9, 0.85));
            vec![(P, "Mug")]
        }
        Outfit::QuestGiver => {
            garb.chest = metal(c(0.62, 0.64, 0.68));
            garb.sleeves = cloth(c(0.22, 0.28, 0.5));
            garb.forearms = metal(c(0.62, 0.64, 0.68));
            garb.hands = leather(LEATHER);
            garb.legs = cloth(c(0.25, 0.22, 0.2));
            garb.boots = leather(dark(LEATHER, 0.7));
            garb.belt = leather(GOLD);
            garb.tabard = cloth(c(0.22, 0.28, 0.5));
            pauldrons = Some((c(0.62, 0.64, 0.68), 0.9));
            vec![(P, "Spellbook")]
        }
        Outfit::Mob(style, colors) => {
            let [primary, secondary, _] = colors;
            let rags = |garb: &mut Garb| {
                garb.legs = leather(dark(primary, 0.55));
                garb.belt = leather(dark(primary, 0.4));
            };
            match style {
                HumanoidStyle::Bandit => {
                    garb.chest = leather(primary);
                    garb.sleeves = leather(dark(primary, 0.8));
                    garb.forearms = leather(dark(primary, 0.7));
                    garb.hands = leather(dark(LEATHER, 0.6));
                    garb.legs = cloth(secondary);
                    garb.boots = leather(dark(LEATHER, 0.6));
                    garb.belt = leather(LEATHER);
                    pauldrons = Some((dark(primary, 0.8), 0.7));
                    vec![
                        (P, "Knife"),
                        (P, "Knife_Offhand"),
                        (File::Skeletons, "Skeleton_Rogue_Hood"),
                    ]
                }
                HumanoidStyle::Raider => {
                    garb.forearms = leather(dark(primary, 0.8));
                    garb.legs = leather(secondary);
                    garb.boots = leather(dark(secondary, 0.7));
                    garb.belt = leather(dark(LEATHER, 0.8));
                    vec![
                        (P, "1H_Axe"),
                        (P, "Barbarian_Round_Shield"),
                        (P, "Barbarian_Hat"),
                    ]
                }
                HumanoidStyle::Mystic => {
                    garb.chest = cloth(primary);
                    garb.sleeves = cloth(primary);
                    garb.forearms = cloth(dark(primary, 0.85));
                    garb.legs = cloth(secondary);
                    garb.boots = leather(dark(LEATHER, 0.6));
                    garb.belt = cloth(dark(secondary, 0.6));
                    robe = Some((primary, secondary));
                    vec![(P, "1H_Wand"), (P, "Spellbook_open"), (P, "Mage_Hat")]
                }
                HumanoidStyle::Shaman | HumanoidStyle::TrollShaman | HumanoidStyle::TroggShaman => {
                    if style == HumanoidStyle::Shaman {
                        garb.skin = bytes(skin_color(Race::Orc, (seed % 5) as u8));
                    } else {
                        garb.skin = bytes(primary);
                    }
                    garb.chest = cloth(secondary);
                    garb.forearms = leather(dark(secondary, 0.7));
                    garb.legs = cloth(dark(secondary, 0.8));
                    garb.belt = leather(dark(LEATHER, 0.7));
                    robe = Some((secondary, dark(secondary, 0.6)));
                    vec![(P, "2H_Staff")]
                }
                HumanoidStyle::Satyr | HumanoidStyle::Trickster => {
                    garb.skin = bytes(primary);
                    garb.legs = leather(dark(secondary, 0.6));
                    garb.boots = leather(dark(secondary, 0.4));
                    vec![(P, "Knife"), (P, "Throwable")]
                }
                HumanoidStyle::Trogg => {
                    garb.skin = bytes(primary);
                    rags(&mut garb);
                    vec![(P, "1H_Axe")]
                }
                HumanoidStyle::Troll => {
                    garb.skin = bytes(primary);
                    rags(&mut garb);
                    garb.forearms = leather(dark(secondary, 0.6));
                    vec![(P, "2H_Axe")]
                }
                HumanoidStyle::Skeleton => match seed % 3 {
                    0 => {
                        body = Body::SkeletonWarrior;
                        vec![
                            (P, "1H_Sword"),
                            (P, "Round_Shield"),
                            (File::Skeletons, "Skeleton_Warrior_Helmet"),
                        ]
                    }
                    1 => {
                        body = Body::SkeletonRogue;
                        vec![
                            (P, "Knife"),
                            (P, "Knife_Offhand"),
                            (File::Skeletons, "Skeleton_Rogue_Hood"),
                        ]
                    }
                    _ => {
                        body = Body::SkeletonMinion;
                        vec![(P, "1H_Axe")]
                    }
                },
                HumanoidStyle::Necromancer => {
                    body = Body::SkeletonMage;
                    vec![(P, "2H_Staff"), (File::Skeletons, "Skeleton_Mage_Hat")]
                }
            }
        }
        // Giants are still built from shapes; see `humanoid`.
        Outfit::Giant(..) => vec![],
    };
    // Bare-skinned brutes are one gender.
    if matches!(
        outfit,
        Outfit::Mob(
            HumanoidStyle::Trogg
                | HumanoidStyle::Troll
                | HumanoidStyle::Raider
                | HumanoidStyle::TrollShaman
                | HumanoidStyle::TroggShaman,
            _
        )
    ) {
        body = Body::Male;
    }

    let (race, mut scale, mut width, mut head) = if person {
        let shape = race_shape(a.race);
        let head = match a.race {
            Race::Goblin | Race::Gnome => 1.18,
            Race::Undead => 0.95,
            _ => 1.0,
        };
        (
            Some(a.race),
            shape.scale,
            shape.limbs.clamp(0.88, 1.15),
            head,
        )
    } else {
        (None, 1.0, 1.0, 1.0)
    };
    match outfit {
        Outfit::Mob(HumanoidStyle::Troll | HumanoidStyle::TrollShaman, _) => {
            scale = 1.15;
            width = 1.08;
        }
        Outfit::Mob(HumanoidStyle::Trogg | HumanoidStyle::TroggShaman, _) => {
            scale = 0.85;
            width = 1.15;
            head = 1.1;
        }
        _ => {}
    }

    // Hair: players pick a style; others get one from their seed.
    let style = if person {
        a.hair_style % Appearance::HAIR_STYLES
    } else {
        (seed / 5 % 4) as u8
    };
    let female = body == Body::Female;
    let mut hair: Vec<&'static str> = match (style, female) {
        (0, false) => vec!["Hair_Beard"],
        (0, true) => vec!["Hair_BuzzedFemale"],
        (1, _) => vec!["Hair_SimpleParted"],
        (2, _) => vec!["Hair_Long"],
        (3, _) => vec!["Hair_Buns"],
        (_, false) => vec!["Hair_Buzzed"],
        (_, true) => vec!["Hair_BuzzedFemale"],
    };
    // Bearded races.
    if !female && matches!(a.race, Race::Gnome) && person && !hair.contains(&"Hair_Beard") {
        hair.push("Hair_Beard");
    }
    let hatted = props.iter().any(|p| is_hat(p.1));
    if hatted {
        // Only a beard shows under a hat.
        hair.retain(|h| *h == "Hair_Beard");
    }
    if !matches!(body, Body::Male | Body::Female) {
        hair.clear();
    }
    let mohawk = person && style == 4 && !hatted && matches!(body, Body::Male | Body::Female);
    Dress {
        body,
        garb,
        hair,
        props,
        scale,
        width,
        head,
        race,
        mohawk,
        pauldrons,
        robe,
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
        return vec![layer("Death01", duration("Death01"), 1.0, false)];
    }
    let mut out = Vec::new();
    // What the legs are doing.
    let base = if pose.airborne {
        "Jump_Loop"
    } else if pose.moving {
        "Jog_Fwd_Loop"
    } else {
        match style {
            Style::Fists => "Unarmed_Idle",
            Style::TwoHander | Style::Giant | Style::Hammer => "2H_Melee_Idle",
            Style::SwordBoard => "Idle_Shield_Loop",
            Style::Daggers | Style::Brute => "Sword_Idle",
            Style::Rifle => "Pistol_Idle_Loop",
            Style::Merchant => "Idle_FoldArms_Loop",
            _ => "Idle_Loop",
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
            Style::Rifle => "Pistol_Aim_Neutral",
            Style::Holy | Style::Hammer | Style::Nature => "Spellcast_Long",
            _ => "Spell_Simple_Idle_Loop",
        };
        out.push(layer(clip, looped(clip, t), 1.0, upper));
    } else if pose.attacking() {
        let (clip, strike) = match style {
            Style::TwoHander | Style::Giant => ("2H_Melee_Attack_Chop", 0.75),
            Style::SwordBoard | Style::Brute => (
                ["Sword_Regular_A", "Sword_Regular_B", "Sword_Regular_C"][pose.combo as usize % 3],
                0.9,
            ),
            Style::Hammer => ("2H_Melee_Attack_Slice", 0.75),
            Style::Fists => match pose.combo % 3 {
                0 => ("Punch_Jab", 0.85),
                1 => ("Punch_Cross", 0.85),
                _ => ("Unarmed_Melee_Attack_Kick", 0.75),
            },
            Style::Daggers => {
                if pose.combo.is_multiple_of(2) {
                    ("Dualwield_Melee_Attack_Stab", 0.75)
                } else {
                    ("Dualwield_Melee_Attack_Slice", 0.75)
                }
            }
            Style::Bow => ("2H_Ranged_Shoot", 0.75),
            Style::Rifle => ("Pistol_Shoot", 0.9),
            Style::Merchant => ("Interact", 0.75),
            _ => ("Spell_Simple_Shoot", 0.9),
        };
        // The game's swings are quick: play the clip's strike and ease out
        // of it before its slow recovery.
        let s = pose.swing;
        let weight = (s / 0.08).min(1.0) * ((1.0 - s) / 0.25).min(1.0);
        let kick = clip == "Unarmed_Melee_Attack_Kick";
        out.push(layer(
            clip,
            s * strike * duration(clip),
            weight,
            upper && !kick,
        ));
    }
    if pose.hurt > 0.0 {
        let k = pose.hurt.min(1.0);
        out.push(layer(
            "Hit_Chest",
            (1.0 - k) * duration("Hit_Chest"),
            k * 0.7,
            true,
        ));
    }
    out
}

/// The skeleton posed by `layers` on a body's proportions, one transform
/// per skeleton node.
fn posed(lib: &Library, body: &Loaded, layers: &[Layer]) -> Vec<Transform> {
    let mut pose = body.rest.clone();
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
    // The clips move the hips for the skeleton's own legs: scale that to
    // this body's.
    if let Some(hips) = lib.rig.node("pelvis") {
        let ours = body.rest[hips].translation;
        let theirs = lib.rig.nodes[hips].rest.translation;
        let moved = pose[hips].translation - theirs;
        pose[hips].translation = ours + moved * (ours.length() / theirs.length().max(1e-4));
    }
    pose
}

/// A long robe's skirt: rings of cloth from the hips to the ankles, each
/// spot following the hips and the leg on its side.
fn robe_skirt(b: &mut Batch, frame: &dyn Fn(&str) -> Mat4, hips: Vec3, color: Color, hem: Color) {
    const SIDES: usize = 18;
    const RINGS: usize = 5;
    let (pelvis, left, right) = (frame("pelvis"), frame("thigh_l"), frame("thigh_r"));
    let (left_knee, right_knee) = (frame("calf_l"), frame("calf_r"));
    let ring = |r: usize, k: usize| {
        let t = r as f32 / (RINGS - 1) as f32;
        let a = std::f32::consts::TAU * k as f32 / SIDES as f32;
        let (rx, rz) = (0.165 + t * 0.11, 0.125 + t * 0.12);
        let y = hips.y + 0.065 - t * (hips.y - 0.13);
        let p = vec3(hips.x + a.sin() * rx, y, hips.z + 0.01 + a.cos() * rz);
        let n = vec3(a.sin() / rx, 0.25, a.cos() / rz).normalize();
        // Left of the body is +x: each side follows its own leg, the front
        // and back both; lower down, more of the leg and less of the hips.
        let lw = (0.5 + 0.5 * a.sin()).clamp(0.0, 1.0);
        let legs = (left * lw + right * (1.0 - lw)) * (1.0 - t * 0.35)
            + (left_knee * lw + right_knee * (1.0 - lw)) * (t * 0.35);
        let k = (t * 1.3).min(0.85);
        let m = pelvis * (1.0 - k) + legs * k;
        (m.transform_point3(p), m.transform_vector3(n))
    };
    for r in 0..RINGS - 1 {
        let shade = if r == RINGS - 2 { hem } else { color };
        for k in 0..SIDES {
            let (a, na) = ring(r, k);
            let (b2, nb) = ring(r, (k + 1) % SIDES);
            let (c2, nc) = ring(r + 1, (k + 1) % SIDES);
            let (d, nd) = ring(r + 1, k);
            b.quad_normals([a, b2, c2, d], [na, nb, nc, nd], shade);
        }
    }
}

fn is_hat(name: &str) -> bool {
    name.ends_with("_Hat") || name.ends_with("_Helmet") || name.ends_with("_Hood")
}

/// How tall the male body is, in its file's units (meters).
const BODY_HEIGHT: f32 = 1.81;

pub(super) fn draw(b: &mut Batch, look: &Look, outfit: Outfit, pos: Vec3, yaw: f32, pose: Pose) {
    let lib = library();
    let dress = dress(outfit, look);
    let style = style_of(outfit);
    let file = &lib.files[dress.body.file() as usize];
    let mut skeleton = posed(lib, file, &layers(style, pose));
    if let Some(head) = lib.rig.node("Head") {
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

    let world = file.model.world_on(&skeleton, &file.binding);
    let garb = matches!(dress.body, Body::Male | Body::Female).then_some(dress.garb);
    let texture = painted(dress.body, garb);
    b.textured(&texture, |b| {
        file.model.draw_posed(b, transform, &world, |part| {
            let shown = if part.skin.is_some() {
                dress.body.wears(&part.name) || dress.hair.contains(&part.name.as_str())
            } else {
                dress
                    .props
                    .iter()
                    .any(|&(from, name)| from == dress.body.file() && name == part.name)
            };
            shown.then_some(tint)
        });
    });
    // Props from other files, with their own textures.
    for from in [File::Props, File::Skeletons] {
        if from == dress.body.file() || !dress.props.iter().any(|p| p.0 == from) {
            continue;
        }
        let other = &lib.files[from as usize];
        let world = other.model.world_on(&skeleton, &other.binding);
        b.textured(&file_texture(from), |b| {
            other.model.draw_posed(b, transform, &world, |part| {
                (part.skin.is_none() && dress.props.contains(&(from, part.name.as_str())))
                    .then_some(tint)
            });
        });
    }

    // How a bone carries the points of the body's rest pose to where they
    // are now.
    let frame = |bone: &str| {
        lib.rig
            .node(bone)
            .map(|i| transform * skeleton[i] * file.rest_world[i].inverse())
            .unwrap_or(transform)
    };
    let at = |bone: &str, point: Vec3| frame(bone).transform_point3(point);
    let along = |bone: &str, v: Vec3| frame(bone).transform_vector3(v);
    let rest = |bone: &str| {
        lib.rig.node(bone).map_or(Vec3::ZERO, |i| {
            file.rest_world[i].transform_point3(Vec3::ZERO)
        })
    };
    if let Some((color, k)) = dress.pauldrons {
        for side in ["l", "r"] {
            let bone = format!("upperarm_{side}");
            let out = if side == "l" { 1.0 } else { -1.0 };
            let center = rest(&bone) + vec3(0.035 * out, 0.035, 0.0);
            let m = frame(&bone);
            b.ellipsoid_axes(
                m.transform_point3(center),
                [
                    m.transform_vector3(Vec3::X * 0.09 * k),
                    m.transform_vector3(Vec3::Y * 0.042 * k),
                    m.transform_vector3(Vec3::Z * 0.095 * k),
                ],
                color,
            );
        }
    }
    if let Some((color, hem)) = dress.robe {
        robe_skirt(b, &frame, rest("pelvis"), color, hem);
    }
    let head = rest("Head");
    let grown = size * dress.head;
    // Ears and tusks.
    if let Some(race) = dress.race {
        let skin = skin_color(race, look.appearance.skin);
        let ear = |b: &mut Batch, side: f32, dir: Vec3, radius: f32| {
            let base = at("Head", head + vec3(0.074 * side, 0.068, -0.012));
            let tip = along("Head", dir * vec3(side, 1.0, 1.0));
            b.cone(base, tip, radius * grown, 0.0, 8, skin);
        };
        for side in [-1.0, 1.0] {
            match race {
                Race::Elf => ear(b, side, vec3(0.075, 0.05, -0.03), 0.018),
                Race::Goblin => ear(b, side, vec3(0.11, 0.025, -0.015), 0.028),
                Race::Gnome => ear(b, side, vec3(0.03, 0.012, -0.008), 0.02),
                Race::Orc => {
                    ear(b, side, vec3(0.035, 0.02, -0.012), 0.02);
                    let tusk = at("Head", head + vec3(0.022 * side, 0.005, 0.088));
                    b.cone(
                        tusk,
                        along("Head", vec3(0.004 * side, 0.03, 0.006)),
                        0.007 * grown,
                        0.0,
                        6,
                        BONE,
                    );
                }
                _ => {}
            }
        }
    }
    // A mohawk is a crest of hair on a shaved head.
    if dress.mohawk {
        let hair = hair_color(look.appearance.hair_color);
        let center = head + vec3(0.0, 0.085, -0.012);
        for k in 0..7 {
            let a = -1.15 + k as f32 * 0.36;
            let dir = vec3(0.0, a.cos(), a.sin());
            let base = at("Head", center + dir * 0.095);
            let up = along("Head", dir * 0.05);
            b.cone(base, up, 0.022 * grown, 0.0, 6, hair);
        }
    }
    // Spell light gathering in the hands while casting.
    if pose.casting && !pose.dead {
        let color = cast_color(style);
        let pulse = (0.035 + (pose.time * 8.0).sin().abs() * 0.025) * size;
        for side in ["l", "r"] {
            let p = at(
                &format!("hand_{side}"),
                rest(&format!("middle_01_{side}")) + vec3(0.0, -0.03, 0.0),
            );
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

    fn outfits() -> Vec<Outfit> {
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
        outfits
    }

    #[test]
    fn every_file_loads_on_the_shared_skeleton() {
        let lib = library();
        for (file, loaded) in File::ALL.iter().zip(&lib.files) {
            assert!(!loaded.model.parts.is_empty(), "{file:?}");
            // Every bone the skins bend with is in the skeleton.
            for skin in &loaded.model.skins {
                for &j in &skin.joints {
                    assert!(
                        loaded.binding[j].is_some(),
                        "{file:?}: {}",
                        loaded.model.nodes[j].name
                    );
                }
            }
        }
        // The bodies' textures know where the clothes go.
        for file in [File::Male, File::Female] {
            let map = lib.files[file as usize].map.as_ref().unwrap();
            for region in [
                Region::Head,
                Region::Chest,
                Region::Hand,
                Region::Thigh,
                Region::Foot,
                Region::Hair,
            ] {
                assert!(map.region.contains(&region), "{file:?} {region:?}");
            }
        }
    }

    #[test]
    fn every_part_and_clip_used_exists() {
        let lib = library();
        for outfit in outfits() {
            for seed in 0..6 {
                let mut l = look(EntityKind::Player(Class::Mage));
                l.seed = seed;
                l.appearance.hair_style = seed as u8;
                l.appearance.body = seed as u8 % 2;
                // With and without a helmet.
                for head in [None, Some(items::LINEN_HOOD)] {
                    l.gear[Slot::Head.index()] = head;
                    let d = dress(outfit, &l);
                    for (from, name) in &d.props {
                        let model = &lib.files[*from as usize].model;
                        assert!(
                            model
                                .parts
                                .iter()
                                .any(|p| p.name == *name && p.skin.is_none()),
                            "{from:?} has no {name}"
                        );
                    }
                    let model = &lib.files[d.body.file() as usize].model;
                    for hair in &d.hair {
                        assert!(model.parts.iter().any(|p| p.name == *hair), "no {hair}");
                    }
                    assert!(model.parts.iter().any(|p| d.body.wears(&p.name)));
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
        assert_eq!(l[0].clip, "Jog_Fwd_Loop");
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
        let body = &lib.files[File::Male as usize];
        let running = posed(
            lib,
            body,
            &layers(Style::SwordBoard, Pose { swing: 0.0, ..pose }),
        );
        let both = posed(lib, body, &layers(Style::SwordBoard, pose));
        let leg = lib.rig.node("thigh_l").unwrap();
        let arm = lib.rig.node("upperarm_r").unwrap();
        assert_eq!(running[leg], both[leg]);
        assert_ne!(running[arm], both[arm]);
    }

    #[test]
    fn bodies_keep_their_own_proportions() {
        // The female skeleton is smaller; posing it keeps her bone lengths.
        let lib = library();
        let (male, female) = (
            &lib.files[File::Male as usize],
            &lib.files[File::Female as usize],
        );
        let idle = layers(Style::Staff, Pose::default());
        let arm = lib.rig.node("lowerarm_l").unwrap();
        let m = posed(lib, male, &idle)[arm].translation.length();
        let f = posed(lib, female, &idle)[arm].translation.length();
        assert!((m - male.rest[arm].translation.length()).abs() < 1e-4);
        assert!((f - female.rest[arm].translation.length()).abs() < 1e-4);
        assert!(f < m);
    }

    #[test]
    fn clothes_are_painted_where_they_go() {
        let lib = library();
        let file = &lib.files[File::Male as usize];
        let map = file.map.as_ref().unwrap();
        let mut garb = dress(
            Outfit::Class(Class::Mage),
            &look(EntityKind::Player(Class::Mage)),
        )
        .garb;
        garb.skin = [60, 160, 60];
        garb.chest = Some(([200, 20, 20], Stuff::Cloth));
        garb.belt = None;
        let pixels = repaint(&file.picture, map, &garb);
        let find = |r: Region| {
            (0..pixels.len())
                .filter(|&i| map.region[i] == r && !map.pale[i])
                .map(|i| pixels[i])
                .nth(200)
                .unwrap()
        };
        let face = find(Region::Head);
        assert!(face[1] > face[0] && face[1] > face[2], "{face:?}");
        let chest = find(Region::Chest);
        assert!(chest[0] > chest[1] && chest[0] > chest[2], "{chest:?}");
    }

    #[test]
    fn races_and_classes_look_different() {
        let mut l = look(EntityKind::Player(Class::Fighter));
        let human = dress(Outfit::Class(Class::Fighter), &l);
        l.appearance.race = Race::Orc;
        let orc = dress(Outfit::Class(Class::Fighter), &l);
        assert_ne!(human.garb.skin, orc.garb.skin);
        assert!(orc.width > human.width);
        let mage = dress(Outfit::Class(Class::Mage), &l);
        assert_ne!(mage.garb.chest, orc.garb.chest);
        l.appearance.body = 1;
        assert_eq!(dress(Outfit::Class(Class::Mage), &l).body, Body::Female);
        // A helmet shows when one is worn, and hides the hair.
        assert!(!orc.props.iter().any(|p| p.1 == "Knight_Helmet"));
        l.appearance.body = 0;
        l.appearance.hair_style = 2;
        l.gear[Slot::Head.index()] = Some(items::LINEN_HOOD);
        let helmed = dress(Outfit::Class(Class::Fighter), &l);
        assert!(helmed.props.iter().any(|p| p.1 == "Knight_Helmet"));
        assert!(helmed.hair.is_empty());
    }
}
