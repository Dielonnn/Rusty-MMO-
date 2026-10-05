//! People as rigged, animated models: players, townsfolk and humanoid
//! mobs. Each race has its own body, male and female, sculpted from
//! Quaternius' Universal Base Characters (see `client/assets/characters/`
//! and its `sculpt.py`); the animations come from Quaternius' Universal
//! Animation Library, and the weapons, hats, skeleton mobs and the
//! animations Quaternius lacks from the KayKit character packs, moved onto
//! the Quaternius skeleton. All are CC0.
//!
//! Every body shares one skeleton and one set of animation clips (from
//! `rig.glb`); a clip only turns bones, so it plays on every race's body
//! alike. A character is drawn by picking a body, a hairstyle and what it
//! carries (`dress`), dressing the body in what it wears (`clothes`: each
//! piece a shell over the skin it covers, painted by `repaint`), posing the
//! skeleton from what it's doing (`layers`), and drawing the body's meshes
//! bent around that pose. Under their clothes everyone wears plain
//! underwear.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use super::*;
use crate::gfx::{ModelFile, ModelPart, Picture, Transform, texture};
use shared::data::ItemKind;

mod clothes;

/// The model files, in `client/assets/characters/`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum File {
    /// A race's own body, male or female (`true`), with its eyes, brows,
    /// hairstyles and modeled extras (tusks, beards, ...).
    Person(Race, bool),
    /// Weapons, shields and hats, each hanging off the bone that holds it.
    Props,
    /// The skeleton mobs and their gear.
    Skeletons,
}

/// How many files: a body per race and sex, the props and the skeletons.
const FILES: usize = Race::ALL.len() * 2 + 2;

impl File {
    const ALL: [File; FILES] = {
        let mut all = [File::Props; FILES];
        let mut k = 0;
        while k < Race::ALL.len() {
            all[k * 2] = File::Person(Race::ALL[k], false);
            all[k * 2 + 1] = File::Person(Race::ALL[k], true);
            k += 1;
        }
        all[FILES - 1] = File::Skeletons;
        all
    };

    fn index(self) -> usize {
        match self {
            File::Person(race, female) => {
                let k = Race::ALL.iter().position(|&r| r == race).unwrap_or(0);
                k * 2 + female as usize
            }
            File::Props => FILES - 2,
            File::Skeletons => FILES - 1,
        }
    }

    fn bytes(self) -> &'static [u8] {
        macro_rules! file {
            ($name:literal) => {
                include_bytes!(concat!("../../assets/characters/", $name, ".glb"))
            };
        }
        match self {
            File::Person(Race::Orc, false) => file!("orc_male"),
            File::Person(Race::Orc, true) => file!("orc_female"),
            File::Person(Race::Elf, false) => file!("elf_male"),
            File::Person(Race::Elf, true) => file!("elf_female"),
            File::Person(Race::Dwarf, false) => file!("dwarf_male"),
            File::Person(Race::Dwarf, true) => file!("dwarf_female"),
            File::Person(Race::Goblin, false) => file!("goblin_male"),
            File::Person(Race::Goblin, true) => file!("goblin_female"),
            File::Person(Race::Gnome, false) => file!("gnome_male"),
            File::Person(Race::Gnome, true) => file!("gnome_female"),
            File::Person(Race::Undead, false) => file!("undead_male"),
            File::Person(Race::Undead, true) => file!("undead_female"),
            File::Person(Race::Human, false) => file!("human_male"),
            File::Person(Race::Human, true) => file!("human_female"),
            File::Props => file!("props"),
            File::Skeletons => file!("skeletons"),
        }
    }
}

/// Whose body someone has.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Body {
    /// A race's body, male or female (`true`).
    Person(Race, bool),
    SkeletonMinion,
    SkeletonWarrior,
    SkeletonRogue,
    SkeletonMage,
}

impl Body {
    fn file(self) -> File {
        match self {
            Body::Person(race, female) => File::Person(race, female),
            _ => File::Skeletons,
        }
    }

    fn person(self) -> bool {
        matches!(self, Body::Person(..))
    }

    /// Whether a skinned part of the body's file belongs to this body.
    fn wears(self, part: &str) -> bool {
        match self {
            Body::Person(..) => {
                matches!(part, "Body" | "Eyes" | "Brows") || part.starts_with("Extra_")
            }
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

/// What someone wears, and the colors of their skin and hair.
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
    /// Whether the boots are shoes, covering only the feet.
    pub(super) shoes: bool,
    pub(super) belt: Fill,
    /// A panel of cloth down the front and back.
    pub(super) tabard: Fill,
}

impl Garb {
    /// Nothing on but underwear.
    fn bare(self) -> Garb {
        Garb {
            skin: self.skin,
            hair: self.hair,
            chest: None,
            sleeves: None,
            forearms: None,
            hands: None,
            legs: None,
            boots: None,
            shoes: false,
            belt: None,
            tabard: None,
        }
    }
}

/// How a worn item is cut.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Cut {
    /// Chest: plate, with shoulder guards.
    Plate,
    /// Chest: a jerkin, coat or doublet, with sleeves.
    Coat,
    /// Chest: no sleeves.
    Vest,
    /// Chest: a long robe, sleeves to the wrists.
    Robe,
    /// Hands: gloves.
    Gloves,
    /// Hands: gloves up to the elbows.
    Gauntlets,
    /// Hands: just the forearms.
    Bracers,
    Trousers,
    /// Feet: boots up to below the knees.
    Boots,
    /// Feet: sandals and slippers.
    Shoes,
    Helm,
    Hood,
    /// A band around the head, which hides no hair.
    Circlet,
}

/// What a piece of armor is made of and how it's cut, from its slot and
/// name.
pub(super) fn wear_of(id: ItemId) -> (Stuff, Cut) {
    let it = item(id);
    let has = |words: &[&str]| words.iter().any(|w| it.name.contains(w));
    // Leather first: a hide chestguard is leather.
    let stuff = if has(&[
        "Leather",
        "Hide",
        "hide",
        "Jerkin",
        "Coat",
        "Grips",
        "Bracers",
        "Boots",
        "Scout",
        "Ridgerunner",
        "Rimeheart",
    ]) {
        Stuff::Leather
    } else if has(&[
        "Hauberk",
        "Breastplate",
        "Chestguard",
        "Legplates",
        "Gauntlets",
        "Sabatons",
        "Crown",
    ]) {
        Stuff::Metal
    } else {
        Stuff::Cloth
    };
    let slot = match it.kind {
        ItemKind::Armor { slot, .. } => slot,
        _ => Slot::Chest,
    };
    let cut = match slot {
        Slot::Head if has(&["Hood"]) => Cut::Hood,
        Slot::Head if has(&["Circlet", "Crown"]) => Cut::Circlet,
        Slot::Head => Cut::Helm,
        Slot::Chest if has(&["Robe", "Vestment"]) => Cut::Robe,
        Slot::Chest if has(&["Vest", "Wrap"]) => Cut::Vest,
        Slot::Chest if stuff == Stuff::Metal => Cut::Plate,
        Slot::Chest => Cut::Coat,
        Slot::Hands if has(&["Gauntlets", "Wraps"]) => Cut::Gauntlets,
        Slot::Hands if has(&["Bracers"]) => Cut::Bracers,
        Slot::Hands => Cut::Gloves,
        Slot::Legs => Cut::Trousers,
        Slot::Feet if has(&["Sandals", "Slippers"]) => Cut::Shoes,
        Slot::Feet => Cut::Boots,
    };
    (stuff, cut)
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
    /// Height, relative to the body's own.
    pub(super) scale: f32,
    /// Girth of the body and limbs, relative to the body's own (the weight
    /// slider).
    pub(super) girth: f32,
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
    /// Where on the body it is in the bind pose (meters; y up from the
    /// feet, x to the left, z forward).
    spot: Vec<Vec3>,
    /// Spots painted grey in the original: underwear.
    pale: Vec<bool>,
    /// Average brightness of the skin and the hair, to shade against.
    skin_mean: f32,
    hair_mean: f32,
}

/// Where clothes end on a body, in meters up from its feet (and out from
/// its middle), found from its skeleton so they fit every race.
#[derive(Clone, Copy, Debug)]
pub(super) struct Marks {
    /// Where boots end.
    boot_top: f32,
    /// A belt's bottom and top.
    belt: (f32, f32),
    /// How high trousers come up when nothing is worn over them: over the
    /// underwear.
    trousers: f32,
    /// A tabard's half width, bottom and top, front and back.
    tabard: (f32, f32, f32),
}

/// Measures of a body that things drawn on it are sized by.
#[derive(Clone, Copy, Debug)]
struct Build {
    /// How tall it stands (meters).
    height: f32,
    /// How far the top of the head is above the head bone.
    crown: f32,
    /// How thick the upper arms are.
    arms: f32,
    /// A long robe's skirt, from the waist down to the ankles: each ring's
    /// height, half width, and front and back, clear of the legs.
    skirt: [(f32, f32, f32, f32); SKIRT_RINGS],
}

const SKIRT_RINGS: usize = 5;

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
    /// For people: their texture's map, skin, marks and build.
    map: Option<Map>,
    shape: Option<clothes::Shape>,
    marks: Option<Marks>,
    build: Option<Build>,
}

struct Library {
    rig: ModelFile,
    /// The skeleton's nodes from the spine up (spine, arms and head), which
    /// an attack can take over while the legs keep running.
    upper: Vec<bool>,
    files: Vec<Loaded>,
}

impl Library {
    fn file(&self, file: File) -> &Loaded {
        &self.files[file.index()]
    }

    /// The human male's build, which the game's sizes are set by.
    fn standard(&self) -> Build {
        self.file(File::Person(Race::Human, false))
            .build
            .expect("people have a build")
    }
}

/// The textures are repainted at this many pixels square.
const ATLAS: usize = 512;

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
                let person = matches!(file, File::Person(..));
                let joint = |name: &str| {
                    rig.node(name)
                        .map_or(Vec3::ZERO, |i| rest_world[i].transform_point3(Vec3::ZERO))
                };
                let shape = person.then(|| clothes::shape(&model)).flatten();
                let (map, marks, build) = match &shape {
                    Some(shape) => {
                        let map = map(&model, &picture, shape);
                        let marks = marks(&joint, &map);
                        (Some(map), Some(marks), Some(shape.build(&model, &joint)))
                    }
                    None => (None, None, None),
                };
                Loaded {
                    model,
                    binding,
                    rest,
                    rest_world,
                    picture,
                    map,
                    shape,
                    marks,
                    build,
                }
            })
            .collect();
        Library { rig, upper, files }
    })
}

/// Where clothes end on a body with its joints where `joint` says and its
/// texture mapped by `map`. The shares were measured on the human body.
fn marks(joint: &dyn Fn(&str) -> Vec3, map: &Map) -> Marks {
    let (pelvis, waist) = (joint("pelvis").y, joint("spine_01").y);
    let (chest, neck) = (joint("spine_03").y, joint("neck_01").y);
    let (knee, ankle) = (joint("calf_l").y, joint("foot_l").y);
    let hips = (joint("thigh_l").x - joint("thigh_r").x).abs() / 2.0;
    let belt = (
        pelvis + (waist - pelvis) * 0.3,
        pelvis + (waist - pelvis) * 0.75,
    );
    let middle = (belt.0 + belt.1) / 2.0;
    // The top of the underwear's shorts (not a top's, higher up).
    let shorts = (0..map.pale.len())
        .filter(|&i| map.pale[i] && matches!(map.region[i], Region::Chest | Region::Hips))
        .map(|i| map.spot[i].y)
        .filter(|&y| y < middle + 0.12)
        .fold(middle, f32::max);
    Marks {
        boot_top: ankle + (knee - ankle) * 0.88,
        belt,
        trousers: shorts + 0.012,
        tabard: (
            hips * 0.79,
            knee + (pelvis - knee) * 0.45,
            chest + (neck - chest) * 0.31,
        ),
    }
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
fn map(model: &ModelFile, picture: &Picture, shape: &clothes::Shape) -> Map {
    let n = ATLAS * ATLAS;
    let mut region = vec![Region::None; n];
    let mut spot = vec![Vec3::ZERO; n];
    for part in &model.parts {
        let body = match part.name.as_str() {
            "Body" => true,
            // Modeled extras are colored by the swatches their texture
            // points at, or by the skin or hair they borrow.
            name if name == "Eyes" || name.starts_with("Extra_") => continue,
            _ => false,
        };
        for tri in part.indices.as_chunks::<3>().0 {
            let v = tri.map(|i| i as usize);
            let uv = v.map(|i| part.uvs[i] * ATLAS as f32);
            let spots = v.map(|i| part.positions[i]);
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
                    // The body's parts meet where its bones' pulls do, as
                    // the clothes are cut (see `clothes`).
                    region[i] = if body {
                        let w = w.map(|w| w.max(0.0));
                        shape.region_in(part, v, w)
                    } else {
                        Region::Hair
                    };
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

/// Underwear: plain linen.
const UNDERWEAR: [u8; 3] = [214, 204, 182];
const UNDERWEAR_PIECE: u8 = 9;

/// Which piece of clothing covers a spot on the body (`at`, in the bind
/// pose) in `region`, and the piece's number (0 for bare skin).
fn piece(region: Region, at: Vec3, garb: &Garb, marks: &Marks) -> (Fill, u8) {
    let h = at.y;
    let waist = (marks.belt.0 + marks.belt.1) / 2.0;
    let belt = (marks.belt.0..marks.belt.1).contains(&h);
    let tabard = at.x.abs() < marks.tabard.0 && (marks.tabard.1..marks.tabard.2).contains(&h);
    match region {
        Region::None | Region::Head | Region::Hair => (None, 0),
        Region::Chest | Region::Hips | Region::Thigh if garb.belt.is_some() && belt => {
            (garb.belt, 7)
        }
        Region::Chest | Region::Hips | Region::Thigh if garb.tabard.is_some() && tabard => {
            (garb.tabard, 8)
        }
        // Shirts and trousers meet at the waist, whichever bone moves it;
        // without a shirt, trousers come up over the underwear.
        Region::Chest | Region::Hips if h >= waist && garb.chest.is_some() => (garb.chest, 1),
        Region::Chest | Region::Hips
            if h >= waist && (h >= marks.trousers || garb.legs.is_none()) =>
        {
            (None, 0)
        }
        Region::UpperArm => (garb.sleeves, 2),
        Region::Forearm => (garb.forearms, 3),
        Region::Hand => (garb.hands, 4),
        Region::Chest | Region::Hips | Region::Thigh => (garb.legs, 5),
        Region::Calf if h < marks.boot_top && garb.boots.is_some() && !garb.shoes => {
            (garb.boots, 6)
        }
        Region::Calf => (garb.legs, 5),
        Region::Foot => (garb.boots, 6),
    }
}

/// Paints a body's clothes, underwear, skin and hair onto its texture,
/// keeping the light and shade painted into it.
fn repaint(picture: &Picture, map: &Map, marks: &Marks, garb: &Garb) -> Vec<[u8; 4]> {
    let n = ATLAS * ATLAS;
    let mut pixels = picture.pixels.clone();
    // Which piece of clothing each spot got, and its color, to outline
    // where one color meets another.
    let mut pieces = vec![0u8; n];
    let mut colors: Vec<Option<[u8; 3]>> = vec![None; n];
    for i in 0..n {
        let region = map.region[i];
        match region {
            Region::None => continue,
            Region::Hair => {
                let shade = luma(pixels[i]) / map.hair_mean;
                let [r, g, b] = garb.hair.map(|c| texture::byte(c as f32 / 255.0 * shade));
                pixels[i] = [r, g, b, 255];
                continue;
            }
            _ => {}
        }
        let (mut fill, mut id) = piece(region, map.spot[i], garb, marks);
        if fill.is_none() {
            id = 0;
            if map.pale[i] && region != Region::Head {
                fill = Some((UNDERWEAR, Stuff::Cloth));
                id = UNDERWEAR_PIECE;
            }
        }
        pieces[i] = id;
        colors[i] = fill.map(|f| f.0);
        let shade = luma(pixels[i]) / map.skin_mean;
        let color = match fill {
            None => garb.skin.map(|c| c as f32 / 255.0 * shade),
            Some((c, stuff)) => {
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
                let h = map.spot[i].y;
                let banded = stuff == Stuff::Metal
                    && matches!(region, Region::Chest | Region::Hips | Region::Thigh)
                    && h < marks.tabard.2
                    && (h / 0.062).fract() < 0.14;
                let k = if banded { k * 0.7 } else { k };
                c.map(|c| c as f32 / 255.0 * k)
            }
        };
        let [r, g, b] = color.map(texture::byte);
        pixels[i] = [r, g, b, 255];
    }
    // Clothes reach a few spots past where they end, so the edges of their
    // shells, cut finer than the texture's spots, never show skin.
    let bare = |id: u8| id == 0 || id == UNDERWEAR_PIECE;
    let body = |r: Region| r != Region::None && r != Region::Hair;
    for _ in 0..3 {
        let (before, ids, was) = (pixels.clone(), pieces.clone(), colors.clone());
        for y in 1..ATLAS - 1 {
            for x in 1..ATLAS - 1 {
                let i = y * ATLAS + x;
                if !body(map.region[i]) || !bare(ids[i]) {
                    continue;
                }
                let around = [i - 1, i + 1, i - ATLAS, i + ATLAS];
                if let Some(&j) = around
                    .iter()
                    .find(|&&j| body(map.region[j]) && !bare(ids[j]))
                {
                    pixels[i] = before[j];
                    pieces[i] = ids[j];
                    colors[i] = was[j];
                }
            }
        }
    }
    // Dark seams where one color of clothing meets another, or skin.
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
                    && colors[j] != colors[i]
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
    static TEXTURES: RefCell<HashMap<(File, Option<Garb>), Texture2D>> =
        RefCell::new(HashMap::new());
}

/// A file's texture with `garb` painted on (or its own, for `None`), made
/// once and kept.
fn painted(file: File, garb: Option<Garb>) -> Texture2D {
    TEXTURES.with(|t| {
        t.borrow_mut()
            .entry((file, garb))
            .or_insert_with(|| {
                let loaded = library().file(file);
                let pixels = match (garb, &loaded.map, &loaded.marks) {
                    (Some(garb), Some(map), Some(marks)) => {
                        repaint(&loaded.picture, map, marks, &garb)
                    }
                    _ => loaded.picture.pixels.clone(),
                };
                texture::atlas(ATLAS, ATLAS, &pixels)
            })
            .clone()
    })
}

/// Picks the body, clothes, props and build for someone.
pub(super) fn dress(outfit: Outfit, look: &Look) -> Dress {
    let a = look.appearance;
    let seed = look.seed;
    let person = matches!(
        outfit,
        Outfit::Class(_) | Outfit::Merchant | Outfit::QuestGiver
    );
    let female = if person {
        a.body == 1
    } else {
        seed / 7 % 2 == 1
    };
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
        shoes: false,
        belt: None,
        tabard: None,
    };
    use File::Props as P;
    let mut body = Body::Person(if person { a.race } else { Race::Human }, female);
    let mut scale = if person { a.height_scale() } else { 1.0 };
    let girth = if person { a.weight_scale() } else { 1.0 };
    let mut pauldrons = None;
    let mut robe = None;
    let props: Vec<Prop> = match outfit {
        Outfit::Class(class) => {
            // What they wear is the items they have on.
            let worn = |slot: Slot| {
                look.gear[slot.index()].map(|id| {
                    let color = rgb(item(id).color);
                    let (stuff, cut) = wear_of(id);
                    (color, Some((bytes(color), stuff)), cut)
                })
            };
            if let Some((color, fill, cut)) = worn(Slot::Chest) {
                garb.chest = fill;
                match cut {
                    Cut::Plate => {
                        garb.sleeves = fill;
                        pauldrons = Some((dark(color, 0.8), 1.2));
                    }
                    Cut::Coat => {
                        garb.sleeves = fill;
                        if fill.is_some_and(|f| f.1 == Stuff::Leather) {
                            pauldrons = Some((dark(color, 0.85), 0.75));
                        }
                    }
                    Cut::Robe => {
                        garb.sleeves = fill;
                        garb.forearms = fill;
                        let hem = class_colors(class).4.unwrap_or(dark(color, 0.6));
                        robe = Some((color, hem));
                    }
                    _ => {}
                }
                garb.belt = match cut {
                    Cut::Robe | Cut::Vest => cloth(dark(color, 0.6)),
                    _ => leather(dark(LEATHER, 0.8)),
                };
                if matches!(class, Class::Fighter | Class::Paladin | Class::Cleric) {
                    garb.tabard = class_colors(class).4.and_then(cloth);
                }
            }
            if let Some((_, fill, cut)) = worn(Slot::Hands) {
                match cut {
                    Cut::Bracers => garb.forearms = fill,
                    Cut::Gauntlets => {
                        garb.hands = fill;
                        garb.forearms = fill;
                    }
                    _ => garb.hands = fill,
                }
            }
            if let Some((color, fill, _)) = worn(Slot::Legs) {
                // Under a robe, legs that stride out of the skirt show the
                // robe.
                garb.legs = if robe.is_some() { garb.chest } else { fill };
                if garb.belt.is_none() {
                    garb.belt = leather(dark(color, 0.6));
                }
            }
            if let Some((_, fill, cut)) = worn(Slot::Feet) {
                garb.boots = fill;
                garb.shoes = cut == Cut::Shoes;
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
            if let Some(id) = look.gear[Slot::Head.index()] {
                match wear_of(id) {
                    (_, Cut::Circlet) => {}
                    (_, Cut::Hood) => props.push((File::Skeletons, "Skeleton_Rogue_Hood")),
                    (Stuff::Metal, _) => props.push((P, "Knight_Helmet")),
                    (Stuff::Leather, _) => props.push((P, "Barbarian_Hat")),
                    (Stuff::Cloth, _) => props.push((P, "Mage_Hat")),
                }
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
                    body = Body::Person(Race::Human, false);
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
                    body = match style {
                        HumanoidStyle::Shaman => Body::Person(Race::Orc, female),
                        HumanoidStyle::TrollShaman => Body::Person(Race::Orc, false),
                        _ => Body::Person(Race::Dwarf, false),
                    };
                    garb.skin = if style == HumanoidStyle::Shaman {
                        bytes(skin_color(Race::Orc, (seed % 5) as u8))
                    } else {
                        bytes(primary)
                    };
                    garb.chest = cloth(secondary);
                    garb.forearms = leather(dark(secondary, 0.7));
                    garb.legs = cloth(dark(secondary, 0.8));
                    garb.belt = leather(dark(LEATHER, 0.7));
                    robe = Some((secondary, dark(secondary, 0.6)));
                    vec![(P, "2H_Staff")]
                }
                HumanoidStyle::Satyr | HumanoidStyle::Trickster => {
                    body = Body::Person(Race::Elf, female);
                    garb.skin = bytes(primary);
                    garb.legs = leather(dark(secondary, 0.6));
                    garb.boots = leather(dark(secondary, 0.4));
                    vec![(P, "Knife"), (P, "Throwable")]
                }
                HumanoidStyle::Trogg => {
                    body = Body::Person(Race::Dwarf, false);
                    garb.skin = bytes(primary);
                    rags(&mut garb);
                    vec![(P, "1H_Axe")]
                }
                HumanoidStyle::Troll => {
                    body = Body::Person(Race::Orc, false);
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
    match outfit {
        Outfit::Mob(HumanoidStyle::Troll | HumanoidStyle::TrollShaman, _) => scale = 1.15,
        Outfit::Mob(HumanoidStyle::Trogg | HumanoidStyle::TroggShaman, _) => scale = 0.85,
        _ => {}
    }

    // Hair: players pick a style; others get one from their seed.
    let style = if person {
        a.hair_style % Appearance::HAIR_STYLES
    } else {
        (seed / 5 % 4) as u8
    };
    let female = matches!(body, Body::Person(_, true));
    let mut hair: Vec<&'static str> = match (style, female) {
        (0, false) => vec!["Hair_Beard"],
        (0, true) => vec!["Hair_BuzzedFemale"],
        (1, _) => vec!["Hair_SimpleParted"],
        (2, _) => vec!["Hair_Long"],
        (3, _) => vec!["Hair_Buns"],
        (_, false) => vec!["Hair_Buzzed"],
        (_, true) => vec!["Hair_BuzzedFemale"],
    };
    let hatted = props.iter().any(|p| is_hat(p.1));
    if hatted {
        // Only a beard shows under a hat.
        hair.retain(|h| *h == "Hair_Beard");
    }
    if !body.person() {
        hair.clear();
    }
    let mohawk = person && style == 4 && !hatted && body.person();
    // Dwarf men are bearded, whatever their hair.
    if body == Body::Person(Race::Dwarf, false) && !hair.contains(&"Hair_Beard") {
        hair.push("Hair_Beard");
    }
    Dress {
        body,
        garb,
        hair,
        props,
        scale,
        girth,
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
            Style::SwordBoard => "Idle_Shield_Loop",
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

/// Whether a model's node hangs off the right hand.
fn in_right_hand(model: &ModelFile, mut node: usize) -> bool {
    loop {
        if model.nodes[node].name == "hand_r" {
            return true;
        }
        match model.nodes[node].parent {
            Some(p) => node = p,
            None => return false,
        }
    }
}

/// The fingers of a hand, each from the knuckle out.
const FINGERS: [&str; 5] = ["thumb", "index", "middle", "ring", "pinky"];

/// Poses the right arm, hand and fingers for an emote `t` seconds in, over
/// whatever else is playing: waving, pointing, blowing a kiss, wagging a
/// finger or flipping someone off. Directions are in the skeleton's own
/// space: +z is forward, +y up and +x the character's left.
fn gesture(lib: &Library, pose: &mut [Transform], emote: shared::emote::Emote, t: f32) {
    use shared::emote::Emote as E;
    let length = emote.duration().unwrap_or(2.0);
    // Ease into the pose and back out of it at the end.
    let w = ((t / 0.25).min(1.0) * ((length - t) / 0.3).min(1.0)).clamp(0.0, 1.0);
    let w = w * w * (3.0 - 2.0 * w);
    if w <= 0.0 {
        return;
    }
    let wobble = |speed: f32| (t * speed).sin();
    // Upper arm, forearm, hand (the fingers' way) and the hand's thumb
    // side, and which fingers stay straight while the rest make a fist.
    let (upper, fore, hand, side, straight): (Vec3, Vec3, Vec3, Vec3, &[&str]) = match emote {
        E::Wave => (
            vec3(-0.75, 0.55, 0.2),
            vec3(-0.2 + 0.45 * wobble(9.0), 1.0, 0.15),
            vec3(-0.2 + 0.6 * wobble(9.0), 1.0, 0.1),
            vec3(1.0, 0.0, 0.0),
            &FINGERS,
        ),
        E::Point => (
            vec3(-0.25, 0.25, 1.0),
            vec3(-0.1, 0.25, 1.0),
            vec3(-0.05, 0.2, 1.0),
            vec3(0.6, 1.0, 0.0),
            &["index"],
        ),
        E::FlipOff => (
            vec3(-0.55, -0.3, 0.75),
            vec3(-0.1, 1.0, 0.3),
            vec3(0.0, 1.0, 0.15 + 0.15 * wobble(14.0).max(0.0)),
            vec3(-1.0, 0.0, 0.0),
            &["middle"],
        ),
        E::No => (
            vec3(-0.55, -0.45, 0.7),
            vec3(-0.05, 1.0, 0.3),
            vec3(0.55 * wobble(13.0), 1.0, 0.1),
            vec3(1.0, 0.0, 0.0),
            &["index"],
        ),
        E::Kiss => {
            // Fingers to the lips, then the arm sweeps out with the kiss.
            let k = ((t - 0.7) / 0.45).clamp(0.0, 1.0);
            let k = k * k * (3.0 - 2.0 * k);
            (
                vec3(-0.2, -0.6, 0.75).lerp(vec3(-0.35, 0.35, 0.9), k),
                vec3(0.6, 0.7, 0.4).lerp(vec3(-0.25, 0.4, 1.0), k),
                vec3(0.25, 1.0, 0.05).lerp(vec3(-0.2, 0.55, 1.0), k),
                vec3(-1.0, 0.0, 0.0).lerp(vec3(-1.0, 0.3, 0.0), k),
                &FINGERS,
            )
        }
        E::Sit | E::Backflip => return,
    };

    // Curl the fingers into a fist (as in a punch), but for the straight ones.
    if let Some(fist) = lib.rig.clip("Punch_Jab") {
        let mut punch = pose.to_vec();
        fist.apply(fist.duration * 0.3, &mut punch);
        for finger in FINGERS {
            for joint in 1..=3 {
                let Some(i) = lib.rig.node(&format!("{finger}_0{joint}_r")) else {
                    continue;
                };
                let goal = if straight.contains(&finger) {
                    lib.rig.nodes[i].rest.rotation
                } else {
                    punch[i].rotation
                };
                pose[i].rotation = pose[i].rotation.slerp(goal, w);
            }
        }
    }

    let at = |pose: &[Transform], bone: &str| {
        lib.rig
            .node(bone)
            .map_or(Vec3::ZERO, |i| lib.rig.world(pose)[i].w_axis.truncate())
    };
    // Turns a bone by `turn` in the skeleton's space, as much as the emote
    // has eased in.
    let turn = |pose: &mut [Transform], bone: &str, turn: Quat| {
        let Some(i) = lib.rig.node(bone) else {
            return;
        };
        let parent = lib.rig.nodes[i].parent.map_or(Quat::IDENTITY, |p| {
            lib.rig.world(pose)[p].to_scale_rotation_translation().1
        });
        let turn = Quat::IDENTITY.slerp(turn, w);
        pose[i].rotation = (parent.inverse() * turn * parent * pose[i].rotation).normalize();
    };
    // Points a bone the way of `dir`, from it to the next joint.
    let aim = |pose: &mut [Transform], bone: &str, next: &str, dir: Vec3| {
        let now = (at(pose, next) - at(pose, bone)).normalize_or_zero();
        if now != Vec3::ZERO {
            turn(pose, bone, Quat::from_rotation_arc(now, dir.normalize()));
        }
    };
    aim(pose, "upperarm_r", "lowerarm_r", upper);
    aim(pose, "lowerarm_r", "hand_r", fore);
    // The hand both points and turns its thumb side the right way.
    let frame = |dir: Vec3, side: Vec3| {
        let dir = dir.normalize();
        let side = (side - dir * side.dot(dir)).normalize_or_zero();
        Mat3::from_cols(dir, side, dir.cross(side))
    };
    let now = frame(
        at(pose, "middle_01_r") - at(pose, "hand_r"),
        at(pose, "index_01_r") - at(pose, "pinky_01_r"),
    );
    let goal = frame(hand, side);
    turn(
        pose,
        "hand_r",
        Quat::from_mat3(&(goal * now.transpose())).normalize(),
    );
}

/// A long robe's skirt: rings of cloth from the waist to the ankles (see
/// `Build::skirt`), each spot following the hips and the leg on its side.
fn robe_skirt(
    b: &mut Batch,
    frame: &dyn Fn(&str) -> Mat4,
    rings: &[(f32, f32, f32, f32); SKIRT_RINGS],
    girth: f32,
    (color, hem): (Color, Color),
) {
    const SIDES: usize = 18;
    let (pelvis, left, right) = (frame("pelvis"), frame("thigh_l"), frame("thigh_r"));
    let (left_knee, right_knee) = (frame("calf_l"), frame("calf_r"));
    let ring = |r: usize, k: usize| {
        let t = r as f32 / (SKIRT_RINGS - 1) as f32;
        let a = std::f32::consts::TAU * k as f32 / SIDES as f32;
        let (y, wide, front, back) = rings[r];
        let (rx, rz) = (wide * girth, (front - back) / 2.0 * girth);
        let p = vec3(a.sin() * rx, y, (front + back) / 2.0 + a.cos() * rz);
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
    for r in 0..SKIRT_RINGS - 1 {
        let shade = if r == SKIRT_RINGS - 2 { hem } else { color };
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

/// Bends a body thicker or thinner (`girth` of its own) around its bones:
/// for each joint, a stretch across its bone, in the pose the body was
/// bound in. Heads, hands, feet and fingers keep their size.
fn thickened(model: &ModelFile, girth: f32) -> Vec<Mat4> {
    let mut adjust = vec![Mat4::IDENTITY; model.nodes.len()];
    for skin in &model.skins {
        for (&j, inv) in skin.joints.iter().zip(&skin.inverse_bind) {
            let k = match region_of(&model.nodes[j].name) {
                Region::Head | Region::Hand | Region::Foot => continue,
                _ if model.nodes[j].name == "neck_01" => continue,
                _ => girth,
            };
            let bind = inv.inverse();
            let at = bind.transform_point3(Vec3::ZERO);
            let along = bind.transform_vector3(Vec3::Y).normalize_or_zero();
            // Scale by k across the bone and keep its length.
            let across = Mat3::IDENTITY * k
                + Mat3::from_cols(along * along.x, along * along.y, along * along.z) * (1.0 - k);
            adjust[j] =
                Mat4::from_translation(at) * Mat4::from_mat3(across) * Mat4::from_translation(-at);
        }
    }
    adjust
}

/// How tall a race's body stands, as the game draws it (before the height
/// slider).
pub(super) fn stature(race: Race, female: bool) -> f32 {
    let lib = library();
    let file = Body::Person(race, female).file();
    let build = lib.file(file).build.expect("people have a build");
    2.1 * build.height / lib.standard().height
}

pub(super) fn draw(b: &mut Batch, look: &Look, outfit: Outfit, pos: Vec3, yaw: f32, pose: Pose) {
    let lib = library();
    let dress = dress(outfit, look);
    let style = style_of(outfit);
    let id = dress.body.file();
    let file = lib.file(id);
    let mut skeleton = posed(lib, file, &layers(style, pose));
    if let Some((emote, t)) = pose.emote.filter(|_| !pose.dead) {
        gesture(lib, &mut skeleton, emote, t);
    }
    let skeleton = lib.rig.world(&skeleton);
    let standard = lib.standard();
    let size = 2.1 / standard.height * dress.scale;
    let transform = Mat4::from_translation(pos)
        * Mat4::from_rotation_y(yaw)
        * Mat4::from_scale(Vec3::splat(size));
    // A flush of red when hit.
    let hurt = if pose.dead { 0.0 } else { pose.hurt };
    let tint = Color::new(1.0, 1.0 - hurt * 0.35, 1.0 - hurt * 0.4, 1.0);

    let world = file.model.world_on(&skeleton, &file.binding);
    let adjust = (dress.girth != 1.0).then(|| thickened(&file.model, dress.girth));
    let joints = file
        .model
        .joint_matrices(transform, &world, adjust.as_deref());
    // An emote needs the right hand free: what it holds is put away.
    let gesturing = pose.emote.is_some() && !pose.dead;
    let away = |model: &ModelFile, part: &ModelPart| gesturing && in_right_hand(model, part.node);
    let held = |part: &ModelPart| {
        dress
            .props
            .iter()
            .any(|&(from, name)| from == id && name == part.name)
            && !away(&file.model, part)
    };
    let dressed = dress
        .body
        .person()
        .then(|| clothes::dressed(id, file, &dress.garb))
        .flatten();
    match dressed {
        Some(dressed) => {
            // The skin that shows, in its underwear, with the eyes, brows,
            // hair and modeled extras.
            b.textured(&painted(id, Some(dress.garb.bare())), |b| {
                let parts = std::iter::once(&dressed.skin)
                    .chain(file.model.parts.iter().filter(|p| p.name != "Body"));
                file.model
                    .draw_parts(b, transform, &world, &joints, parts, |part| {
                        let shown = if part.skin.is_some() {
                            dress.body.wears(&part.name) || dress.hair.contains(&part.name.as_str())
                        } else {
                            held(part)
                        };
                        shown.then_some(tint)
                    });
            });
            if let Some(clothes) = &dressed.clothes {
                b.textured(&painted(id, Some(dress.garb)), |b| {
                    file.model
                        .draw_parts(b, transform, &world, &joints, [clothes], |_| Some(tint));
                });
            }
        }
        _ => {
            b.textured(&painted(id, None), |b| {
                file.model
                    .draw_parts(b, transform, &world, &joints, &file.model.parts, |part| {
                        let shown = if part.skin.is_some() {
                            dress.body.wears(&part.name)
                        } else {
                            held(part)
                        };
                        shown.then_some(tint)
                    });
            });
        }
    }
    // Props from other files, with their own textures.
    for from in [File::Props, File::Skeletons] {
        if from == id || !dress.props.iter().any(|p| p.0 == from) {
            continue;
        }
        let other = lib.file(from);
        let world = other.model.world_on(&skeleton, &other.binding);
        b.textured(&painted(from, None), |b| {
            other.model.draw_posed(b, transform, &world, |part| {
                (part.skin.is_none()
                    && dress.props.contains(&(from, part.name.as_str()))
                    && !away(&other.model, part))
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
    // Sized to this body: its arms, hips and head against a human's.
    let build = file.build.unwrap_or(standard);
    if let Some((color, k)) = dress.pauldrons {
        let k = k * (build.arms / standard.arms).min(1.25) * dress.girth;
        for side in ["l", "r"] {
            let bone = format!("upperarm_{side}");
            let out = if side == "l" { 1.0 } else { -1.0 };
            let center = rest(&bone) + vec3(0.035 * out, 0.035, 0.0) * k;
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
    if let Some(colors) = dress.robe {
        robe_skirt(b, &frame, &build.skirt, dress.girth, colors);
    }
    // A mohawk is a crest of hair on a shaved head.
    if dress.mohawk {
        let k = build.crown / standard.crown;
        let hair = hair_color(look.appearance.hair_color);
        let center = rest("Head") + vec3(0.0, 0.085, -0.012) * k;
        for i in 0..7 {
            let a = -1.15 + i as f32 * 0.36;
            let dir = vec3(0.0, a.cos(), a.sin());
            let base = at("Head", center + dir * 0.095 * k);
            let up = along("Head", dir * 0.05 * k);
            b.cone(base, up, 0.022 * size * k, 0.0, 6, hair);
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

    const HUMAN: File = File::Person(Race::Human, false);

    #[test]
    fn every_file_loads_on_the_shared_skeleton() {
        let lib = library();
        for (i, (file, loaded)) in File::ALL.iter().zip(&lib.files).enumerate() {
            assert_eq!(file.index(), i);
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
        // Every race's bodies know where the clothes go.
        for race in Race::ALL {
            for female in [false, true] {
                let file = File::Person(race, female);
                let loaded = lib.file(file);
                let map = loaded.map.as_ref().unwrap();
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
                assert!(map.pale.iter().any(|p| *p), "{file:?} has no underwear");
                let marks = loaded.marks.unwrap();
                assert!(marks.boot_top < marks.belt.0 && marks.belt.1 < marks.tabard.2);
                assert!(loaded.shape.is_some() && loaded.build.is_some(), "{file:?}");
            }
        }
    }

    #[test]
    fn sculpted_races_have_bodies_of_their_own() {
        let lib = library();
        let body = |race| {
            &lib.file(File::Person(race, false))
                .model
                .parts
                .iter()
                .find(|p| p.name == "Body")
                .unwrap()
                .positions
        };
        for race in Race::ALL {
            let height = lib.file(File::Person(race, false)).build.unwrap().height;
            assert!((1.0..2.2).contains(&height), "{race:?} is {height} m");
            if race != Race::Human {
                assert!(body(race) != body(Race::Human), "{race:?} is a human");
            }
            assert_eq!(Body::Person(race, true).file(), File::Person(race, true));
        }
        // Orcs and elves stand taller than humans; the small folk well short.
        for race in [Race::Orc, Race::Elf] {
            assert!(stature(race, false) > stature(Race::Human, false));
        }
        for race in [Race::Dwarf, Race::Gnome, Race::Goblin] {
            assert!(stature(race, false) < stature(Race::Human, false) * 0.85);
        }
    }

    #[test]
    fn every_part_and_clip_used_exists() {
        let lib = library();
        for outfit in outfits() {
            for seed in 0..6 {
                let mut l = look(EntityKind::Player(Class::Mage));
                l.seed = seed;
                l.appearance.race = Race::ALL[seed as usize % Race::ALL.len()];
                l.appearance.hair_style = seed as u8;
                l.appearance.body = seed as u8 % 2;
                // With and without a hat of each kind.
                for head in [
                    None,
                    Some(items::LINEN_HOOD),
                    Some(items::LEATHER_CAP),
                    Some(items::WOLFHIDE_HELM),
                    Some(items::SILKWEAVE_CIRCLET),
                    Some(items::CROWN_OF_THE_SUNKEN_KING),
                ] {
                    l.gear[Slot::Head.index()] = head;
                    let d = dress(outfit, &l);
                    for (from, name) in &d.props {
                        let model = &lib.file(*from).model;
                        assert!(
                            model
                                .parts
                                .iter()
                                .any(|p| p.name == *name && p.skin.is_none()),
                            "{from:?} has no {name}"
                        );
                    }
                    let model = &lib.file(d.body.file()).model;
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
        let body = lib.file(HUMAN);
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
        // Posing a body keeps its own bone lengths, however long they are.
        let lib = library();
        let idle = layers(Style::Staff, Pose::default());
        let arm = lib.rig.node("lowerarm_l").unwrap();
        for file in File::ALL {
            let body = lib.file(file);
            let posed = posed(lib, body, &idle)[arm].translation.length();
            assert!((posed - body.rest[arm].translation.length()).abs() < 1e-4);
        }
    }

    #[test]
    fn worn_items_are_cut_from_their_names() {
        use items::*;
        for (id, want) in [
            (SQUIRES_HAUBERK, (Stuff::Metal, Cut::Plate)),
            (TRACKERS_JERKIN, (Stuff::Leather, Cut::Coat)),
            (MINSTRELS_DOUBLET, (Stuff::Cloth, Cut::Coat)),
            (LEATHER_VEST, (Stuff::Leather, Cut::Vest)),
            (INITIATES_WRAP, (Stuff::Cloth, Cut::Vest)),
            (NOVICES_VESTMENTS, (Stuff::Cloth, Cut::Robe)),
            (APPRENTICES_ROBE, (Stuff::Cloth, Cut::Robe)),
            (SQUIRES_GAUNTLETS, (Stuff::Metal, Cut::Gauntlets)),
            (HAND_WRAPS, (Stuff::Cloth, Cut::Gauntlets)),
            (HIDE_BRACERS, (Stuff::Leather, Cut::Bracers)),
            (SPELLWEAVER_GLOVES, (Stuff::Cloth, Cut::Gloves)),
            (SQUIRES_LEGPLATES, (Stuff::Metal, Cut::Trousers)),
            (HIDE_BREECHES, (Stuff::Leather, Cut::Trousers)),
            (FUR_BOOTS, (Stuff::Leather, Cut::Boots)),
            (SQUIRES_SABATONS, (Stuff::Metal, Cut::Boots)),
            (LINEN_SANDALS, (Stuff::Cloth, Cut::Shoes)),
            (WOLFHIDE_HELM, (Stuff::Leather, Cut::Helm)),
            (LINEN_HOOD, (Stuff::Cloth, Cut::Hood)),
            (CROWN_OF_THE_SUNKEN_KING, (Stuff::Metal, Cut::Circlet)),
            (HEARTSTONE_CHESTGUARD, (Stuff::Metal, Cut::Plate)),
            (RIMEHEART_CHESTGUARD, (Stuff::Leather, Cut::Coat)),
            (ASHFIST_GAUNTLETS, (Stuff::Metal, Cut::Gauntlets)),
        ] {
            assert_eq!(wear_of(id), want, "{}", item(id).name);
        }
    }

    #[test]
    fn people_wear_what_they_have_on() {
        let lib = library();
        for class in Class::ALL {
            for race in Race::ALL {
                let mut l = look(EntityKind::Player(class));
                l.appearance.race = race;
                l.gear = class.starter_gear();
                let d = dress(Outfit::Class(class), &l);
                assert_eq!(d.body, Body::Person(race, false));
                assert!(d.garb.legs.is_some(), "{class:?} has no trousers");
                assert_eq!(
                    d.garb.chest.is_some(),
                    l.gear[Slot::Chest.index()].is_some()
                );
                let file = lib.file(d.body.file());
                let dressed = clothes::dressed(d.body.file(), file, &d.garb).unwrap();
                let clothes = dressed.clothes.as_ref().expect("clothes");
                let body = file.model.parts.iter().find(|p| p.name == "Body").unwrap();
                // The clothes stand off the skin they hide.
                assert!(dressed.skin.indices.len() < body.indices.len());
                assert!(!clothes.indices.is_empty());
                assert_eq!(clothes.positions.len(), clothes.uvs.len());
                assert!(
                    clothes
                        .indices
                        .iter()
                        .all(|&i| (i as usize) < clothes.positions.len())
                );
                // Undressed, they're in their underwear.
                let bare = clothes::dressed(d.body.file(), file, &d.garb.bare()).unwrap();
                assert!(bare.clothes.is_none());
                assert_eq!(bare.skin.indices.len(), body.indices.len());
            }
        }
    }

    #[test]
    fn clothes_and_underwear_are_painted_where_they_go() {
        let lib = library();
        let file = lib.file(HUMAN);
        let (map, marks) = (file.map.as_ref().unwrap(), file.marks.as_ref().unwrap());
        let mut garb = dress(
            Outfit::Class(Class::Mage),
            &look(EntityKind::Player(Class::Mage)),
        )
        .garb;
        garb.skin = [60, 160, 60];
        garb.chest = Some(([200, 20, 20], Stuff::Cloth));
        garb.belt = None;
        let find = |pixels: &[[u8; 4]], r: Region, pale: bool| {
            (0..pixels.len())
                .filter(|&i| map.region[i] == r && map.pale[i] == pale)
                .map(|i| pixels[i])
                .nth(200)
                .unwrap()
        };
        let pixels = repaint(&file.picture, map, marks, &garb);
        let face = find(&pixels, Region::Head, false);
        assert!(face[1] > face[0] && face[1] > face[2], "{face:?}");
        let chest = find(&pixels, Region::Chest, false);
        assert!(chest[0] > chest[1] && chest[0] > chest[2], "{chest:?}");
        // Bare, the underwear is linen.
        let pixels = repaint(&file.picture, map, marks, &garb.bare());
        let shorts = find(&pixels, Region::Hips, true);
        assert!(shorts[0] > 120 && shorts[0] >= shorts[2], "{shorts:?}");
    }

    #[test]
    fn heavier_bodies_are_thicker_but_as_long() {
        let lib = library();
        let file = lib.file(HUMAN);
        let adjust = thickened(&file.model, 1.2);
        let arm = file.model.node("upperarm_l").unwrap();
        let hand = file.model.node("hand_l").unwrap();
        assert_eq!(adjust[hand], Mat4::IDENTITY);
        // The upper arm bone runs out along x: a point beside it moves
        // further out, one on it stays.
        let skin = &file.model.skins[0];
        let k = skin.joints.iter().position(|&j| j == arm).unwrap();
        let bind = skin.inverse_bind[k].inverse();
        let (at, along) = (
            bind.transform_point3(Vec3::ZERO),
            bind.transform_vector3(Vec3::Y).normalize(),
        );
        let on = at + along * 0.1;
        assert!(adjust[arm].transform_point3(on).distance(on) < 1e-4);
        let beside = on + along.any_orthonormal_vector() * 0.05;
        let moved = adjust[arm].transform_point3(beside);
        assert!(((moved - on).length() - 0.06).abs() < 1e-4);
    }

    #[test]
    fn races_and_classes_look_different() {
        let mut l = look(EntityKind::Player(Class::Fighter));
        l.gear = Class::Fighter.starter_gear();
        let human = dress(Outfit::Class(Class::Fighter), &l);
        l.appearance.race = Race::Orc;
        let orc = dress(Outfit::Class(Class::Fighter), &l);
        assert_ne!(human.garb.skin, orc.garb.skin);
        assert_ne!(human.body, orc.body);
        l.gear = Class::Mage.starter_gear();
        let mage = dress(Outfit::Class(Class::Mage), &l);
        assert_ne!(mage.garb.chest, orc.garb.chest);
        assert!(mage.robe.is_some() && orc.pauldrons.is_some());
        l.appearance.body = 1;
        assert_eq!(
            dress(Outfit::Class(Class::Mage), &l).body,
            Body::Person(Race::Orc, true)
        );
        // A hat shows when one is worn, and hides the hair; a crown hides
        // none.
        assert!(!orc.props.iter().any(|p| is_hat(p.1)));
        l.appearance.body = 0;
        l.appearance.hair_style = 2;
        l.gear[Slot::Head.index()] = Some(items::WOLFHIDE_HELM);
        let helmed = dress(Outfit::Class(Class::Fighter), &l);
        assert!(helmed.props.iter().any(|p| p.1 == "Barbarian_Hat"));
        assert!(helmed.hair.is_empty());
        l.gear[Slot::Head.index()] = Some(items::CROWN_OF_THE_SUNKEN_KING);
        let crowned = dress(Outfit::Class(Class::Fighter), &l);
        assert!(!crowned.props.iter().any(|p| is_hat(p.1)));
        assert_eq!(crowned.hair, ["Hair_Long"]);
        // Sliders change the size of the person, not the race's body.
        l.appearance.height = Appearance::SLIDER_MAX;
        l.appearance.weight = 0;
        let tall = dress(Outfit::Class(Class::Fighter), &l);
        assert_eq!(tall.body, crowned.body);
        assert!(tall.scale > crowned.scale && tall.girth < crowned.girth);
    }

    #[test]
    fn emotes_raise_the_right_hand_and_free_it() {
        use shared::emote::Emote;
        let lib = library();
        let body = lib.file(HUMAN);
        let still = posed(lib, body, &layers(Style::Staff, Pose::default()));
        let hand = lib.rig.node("hand_r").unwrap();
        let height = |pose: &[Transform]| lib.rig.world(pose)[hand].w_axis.y;
        for emote in [
            Emote::Wave,
            Emote::Point,
            Emote::FlipOff,
            Emote::No,
            Emote::Kiss,
        ] {
            let mut pose = still.clone();
            gesture(lib, &mut pose, emote, 1.0);
            assert!(height(&pose) > height(&still) + 0.2, "{emote:?}");
        }
        // Whole-body emotes are the game's to draw.
        let mut sit = still.clone();
        gesture(lib, &mut sit, Emote::Sit, 1.0);
        assert_eq!(sit, still);
        // A staff hangs off the right hand, a spellbook off the left.
        let props = &lib.file(File::Props).model;
        let part = |name: &str| props.parts.iter().find(|p| p.name == name).unwrap().node;
        assert!(in_right_hand(props, part("2H_Staff")));
        assert!(!in_right_hand(props, part("Spellbook")));
    }
}
