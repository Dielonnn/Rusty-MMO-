//! Clothes as meshes of their own. Each piece someone wears is a shell over
//! the part of their body it covers: the body's own triangles lifted off the
//! skin by the cloth's thickness, smoothed so muscles don't show through,
//! with a rim where it ends (a hem, a cuff, a boot top). The shell bends
//! with the same bones as the skin under it, so it fits every race's body
//! and moves with every animation, and the covered skin isn't drawn.
//!
//! Where each piece ends is `piece`'s call, the same as for its paint. The
//! body's triangles are cut along those lines first, so hems and cuffs run
//! clean rather than following the triangles' zigzag. Shells are built once
//! per body and set of covered pieces, and painted with the clothes'
//! texture (`repaint`), which uses the body's own texture layout.

use std::rc::Rc;

use super::*;

/// How far a piece stands off the skin, in metres.
fn thickness(id: u8, stuff: Stuff) -> f32 {
    let base = match stuff {
        Stuff::Cloth => 0.006,
        Stuff::Leather => 0.009,
        Stuff::Metal => 0.014,
    };
    match id {
        // Belts sit on top of what's under them, boots are chunky.
        BELT_PIECE => base + 0.007,
        BOOTS_PIECE => base + 0.004,
        _ => base,
    }
}

const BOOTS_PIECE: u8 = 6;
const BELT_PIECE: u8 = 7;

/// How many smoothing passes each stuff gets: plate is smoothest, leather
/// keeps a little of the shape under it.
fn passes(stuff: Stuff) -> usize {
    match stuff {
        Stuff::Cloth => 4,
        Stuff::Leather => 3,
        Stuff::Metal => 6,
    }
}

/// A body's skin mesh, read once: which part of the body each of its
/// bones moves.
pub(super) struct Shape {
    /// Which of the file's parts is the body.
    part: usize,
    /// For each of the body's skin joints, the part of the body it's in.
    joint_region: Vec<Region>,
}

pub(super) fn shape(model: &ModelFile) -> Option<Shape> {
    let index = model.parts.iter().position(|p| p.name == "Body")?;
    let joints = model.parts[index].skin.map(|s| &model.skins[s].joints)?;
    Some(Shape {
        part: index,
        joint_region: joints
            .iter()
            .map(|&j| region_of(&model.nodes[j].name))
            .collect(),
    })
}

/// Joints and weights mixed from several vertices' (each with a share).
fn blend(from: &[([u16; 4], [f32; 4], f32)]) -> ([u16; 4], [f32; 4]) {
    let mut all: Vec<(u16, f32)> = Vec::with_capacity(12);
    for (j, w, share) in from {
        for k in 0..4 {
            if w[k] <= 0.0 {
                continue;
            }
            match all.iter_mut().find(|(o, _)| *o == j[k]) {
                Some((_, sum)) => *sum += w[k] * share,
                None => all.push((j[k], w[k] * share)),
            }
        }
    }
    all.sort_by(|a, b| b.1.total_cmp(&a.1));
    all.truncate(4);
    let total = all.iter().map(|a| a.1).sum::<f32>().max(1e-6);
    let mut joints = [0u16; 4];
    let mut weights = [0f32; 4];
    for (k, (j, w)) in all.into_iter().enumerate() {
        joints[k] = j;
        weights[k] = w / total;
    }
    (joints, weights)
}

impl Shape {
    /// Which part of the body a spot is on, from the bones that move it:
    /// the one that pulls hardest.
    pub(super) fn region(&self, joints: [u16; 4], weights: [f32; 4]) -> Region {
        let k = (0..4)
            .max_by(|&a, &b| weights[a].total_cmp(&weights[b]))
            .unwrap_or(0);
        self.joint_region
            .get(joints[k] as usize)
            .copied()
            .unwrap_or(Region::Chest)
    }

    /// Which part of the body a spot inside one of the body's triangles is
    /// on, `at` its barycentric weights.
    pub(super) fn region_in(&self, body: &ModelPart, tri: [usize; 3], at: [f32; 3]) -> Region {
        let (j, w) = blend(&[0, 1, 2].map(|k| (body.joints[tri[k]], body.weights[tri[k]], at[k])));
        self.region(j, w)
    }

    pub(super) fn body<'a>(&self, model: &'a ModelFile) -> &'a ModelPart {
        &model.parts[self.part]
    }

    /// Measures the body, with its joints where `joint` says.
    pub(super) fn build(&self, model: &ModelFile, joint: &dyn Fn(&str) -> Vec3) -> Build {
        let body = self.body(model);
        let region = |v: usize| self.region(body.joints[v], body.weights[v]);
        let points = || 0..body.positions.len();
        let top = points()
            .filter(|&v| region(v) == Region::Head)
            .map(|v| body.positions[v].y)
            .fold(0.0f32, f32::max);
        // The upper arm's thickness: how far its skin is from the bone, on
        // average.
        let (shoulder, elbow) = (joint("upperarm_l"), joint("lowerarm_l"));
        let bone = (elbow - shoulder).normalize_or_zero();
        let (sum, count) = points()
            .filter(|&v| region(v) == Region::UpperArm && body.positions[v].x > 0.0)
            .map(|v| {
                let d = body.positions[v] - shoulder;
                (d - bone * d.dot(bone)).length()
            })
            .fold((0.0, 0usize), |(s, c), d| (s + d, c + 1));
        // A skirt's rings, around the hips and legs at each height, and
        // flaring out a little more each ring down.
        let hips = joint("pelvis").y;
        let waist = hips + (joint("spine_01").y - hips) * 0.53;
        let ankle = joint("foot_l").y + 0.044;
        let mut skirt = [(0.0, 0.0, 0.0, 0.0); SKIRT_RINGS];
        let mut last = (0.0f32, f32::MIN, f32::MAX);
        for (r, ring) in skirt.iter_mut().enumerate() {
            let t = r as f32 / (SKIRT_RINGS - 1) as f32;
            let y = waist + t * (ankle - waist);
            let (mut wide, mut front, mut back) = (0.1f32, 0.05f32, -0.05f32);
            for v in points() {
                let p = body.positions[v];
                let legs = matches!(
                    region(v),
                    Region::Chest | Region::Hips | Region::Thigh | Region::Calf
                );
                if legs && (p.y - y).abs() < 0.05 {
                    wide = wide.max(p.x.abs());
                    front = front.max(p.z);
                    back = back.min(p.z);
                }
            }
            let flare = 0.06 + t * 0.09;
            let grow = if r == 0 { 0.0 } else { 0.02 };
            wide = (wide + flare).max(last.0 + grow);
            front = (front + flare).max(last.1 + grow * 0.75);
            back = (back - flare).min(last.2 - grow * 0.75);
            last = (wide, front, back);
            *ring = (y, wide, front, back);
        }
        Build {
            skirt,
            height: body.positions.iter().map(|p| p.y).fold(0.01f32, f32::max),
            crown: top - joint("Head").y,
            arms: sum / count.max(1) as f32,
        }
    }
}

/// A body dressed: the skin still showing, and the clothes over the rest.
pub(super) struct Dressed {
    pub(super) skin: ModelPart,
    pub(super) clothes: Option<ModelPart>,
}

/// What decides a dressed body's shape: which stuff each piece is made of.
type Cover = [Option<Stuff>; 9];

fn cover(garb: &Garb) -> Cover {
    let s = |f: Fill| f.map(|f| f.1);
    [
        s(garb.chest),
        s(garb.sleeves),
        s(garb.forearms),
        s(garb.hands),
        s(garb.legs),
        s(garb.boots),
        s(garb.belt),
        s(garb.tabard),
        garb.shoes.then_some(Stuff::Leather),
    ]
}

thread_local! {
    static DRESSED: RefCell<HashMap<(File, Cover), Rc<Dressed>>> = RefCell::new(HashMap::new());
}

/// `garb`'s clothes on a body file, made once per body and cover and kept.
pub(super) fn dressed(file: File, loaded: &Loaded, garb: &Garb) -> Option<Rc<Dressed>> {
    let (shape, marks) = (loaded.shape.as_ref()?, loaded.marks.as_ref()?);
    let key = (file, cover(garb));
    Some(DRESSED.with(|d| {
        d.borrow_mut()
            .entry(key)
            .or_insert_with(|| Rc::new(dress_up(&loaded.model, shape, marks, garb)))
            .clone()
    }))
}

/// What covers a spot: the piece's number and stuff.
type Worn = Option<(u8, Stuff)>;

/// Adds a vertex mixed from others of `mesh` (each with a share), and
/// returns its index.
fn mix(mesh: &mut ModelPart, from: &[(usize, f32)]) -> u32 {
    let position = from.iter().map(|&(v, s)| mesh.positions[v] * s).sum();
    let normal = from
        .iter()
        .map(|&(v, s)| mesh.normals[v] * s)
        .sum::<Vec3>()
        .normalize_or_zero();
    let uv = from.iter().map(|&(v, s)| mesh.uvs[v] * s).sum();
    let mixed: Vec<_> = from
        .iter()
        .map(|&(v, s)| (mesh.joints[v], mesh.weights[v], s))
        .collect();
    let (joints, weights) = blend(&mixed);
    mesh.positions.push(position);
    mesh.normals.push(normal);
    mesh.uvs.push(uv);
    mesh.joints.push(joints);
    mesh.weights.push(weights);
    (mesh.positions.len() - 1) as u32
}

/// The body's mesh with its triangles cut along where each piece of
/// clothing ends, and what covers each triangle.
fn cut_up(body: &ModelPart, shape: &Shape, marks: &Marks, garb: &Garb) -> (ModelPart, Vec<Worn>) {
    let worn_at = |p: Vec3, j: [u16; 4], w: [f32; 4]| -> Worn {
        let (fill, id) = piece(shape.region(j, w), p, garb, marks);
        fill.map(|f| (id, f.1))
    };
    let id = |w: Worn| w.map_or(0, |w| w.0);
    let worn: Vec<Worn> = (0..body.positions.len())
        .map(|v| worn_at(body.positions[v], body.joints[v], body.weights[v]))
        .collect();
    let mut mesh = body.clone();
    mesh.indices.clear();
    let mut covers = Vec::new();
    // Where each edge was cut, so the triangles either side share it.
    let mut cuts: HashMap<(u32, u32), u32> = HashMap::new();
    let mut cut = |mesh: &mut ModelPart, a: u32, b: u32| -> u32 {
        let (a, b) = (a.min(b), a.max(b));
        *cuts.entry((a, b)).or_insert_with(|| {
            let (va, vb) = (a as usize, b as usize);
            let from = id(worn[va]);
            // Find where along the edge the piece changes.
            let (mut lo, mut hi) = (0.0f32, 1.0f32);
            for _ in 0..10 {
                let t = (lo + hi) / 2.0;
                let p = mesh.positions[va].lerp(mesh.positions[vb], t);
                let (j, w) = blend(&[
                    (mesh.joints[va], mesh.weights[va], 1.0 - t),
                    (mesh.joints[vb], mesh.weights[vb], t),
                ]);
                if id(worn_at(p, j, w)) == from {
                    lo = t;
                } else {
                    hi = t;
                }
            }
            let t = (lo + hi) / 2.0;
            mix(mesh, &[(va, 1.0 - t), (vb, t)])
        })
    };
    for tri in body.indices.as_chunks::<3>().0 {
        let ids = tri.map(|v| id(worn[v as usize]));
        let of = |v: u32| worn[v as usize];
        if ids[0] == ids[1] && ids[1] == ids[2] {
            mesh.indices.extend(tri);
            covers.push(of(tri[0]));
        } else if ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2] {
            // Three pieces meet: each corner gets its share, out to the
            // middle.
            let [a, b, c] = *tri;
            let middle = mix(&mut mesh, &[a, b, c].map(|v| (v as usize, 1.0 / 3.0)));
            let (ab, bc, ca) = (
                cut(&mut mesh, a, b),
                cut(&mut mesh, b, c),
                cut(&mut mesh, c, a),
            );
            for (corner, next, before) in [(a, ab, ca), (b, bc, ab), (c, ca, bc)] {
                mesh.indices
                    .extend([corner, next, middle, corner, middle, before]);
                covers.extend([of(corner), of(corner)]);
            }
        } else {
            // One corner differs: cut it off.
            let r = if ids[0] == ids[1] {
                0
            } else if ids[1] == ids[2] {
                1
            } else {
                2
            };
            let [a, b, c] = [tri[r], tri[(r + 1) % 3], tri[(r + 2) % 3]];
            let (ca, cb) = (cut(&mut mesh, c, a), cut(&mut mesh, c, b));
            mesh.indices.extend([cb, c, ca, a, b, cb, a, cb, ca]);
            covers.extend([of(c), of(a), of(a)]);
        }
    }
    (mesh, covers)
}

fn dress_up(model: &ModelFile, shape: &Shape, marks: &Marks, garb: &Garb) -> Dressed {
    let (mesh, covers) = cut_up(shape.body(model), shape, marks, garb);
    let tris = mesh.indices.as_chunks::<3>().0;
    let mut skin = mesh.clone();
    skin.indices = tris
        .iter()
        .zip(&covers)
        .filter(|(_, c)| c.is_none())
        .flat_map(|(t, _)| *t)
        .collect();
    if covers.iter().all(|c| c.is_none()) {
        return Dressed {
            skin,
            clothes: None,
        };
    }

    // Weld copies that sit on the same spot (split at texture seams).
    let mut ids: HashMap<[i32; 3], usize> = HashMap::new();
    let mut point = Vec::new();
    let weld: Vec<usize> = mesh
        .positions
        .iter()
        .map(|p| {
            let key = (*p * 1e5).round().as_ivec3().to_array();
            *ids.entry(key).or_insert_with(|| {
                point.push(*p);
                point.len() - 1
            })
        })
        .collect();
    let points = point.len();
    let mut normal = vec![Vec3::ZERO; points];
    for (v, n) in mesh.normals.iter().enumerate() {
        normal[weld[v]] += *n;
    }
    for n in &mut normal {
        *n = n.normalize_or_zero();
    }
    // How thick the clothes are at each point, how smooth, and which
    // points are beside it.
    let mut thick = vec![0f32; points];
    let mut smooth = vec![0usize; points];
    let mut neighbors = vec![Vec::new(); points];
    for (t, c) in tris.iter().zip(&covers) {
        let Some((id, stuff)) = *c else { continue };
        let w = t.map(|i| weld[i as usize]);
        for k in 0..3 {
            thick[w[k]] = thick[w[k]].max(thickness(id, stuff));
            smooth[w[k]] = smooth[w[k]].max(passes(stuff));
            let (a, b) = (w[k], w[(k + 1) % 3]);
            if a != b {
                if !neighbors[a].contains(&b) {
                    neighbors[a].push(b);
                }
                if !neighbors[b].contains(&a) {
                    neighbors[b].push(a);
                }
            }
        }
    }

    // Lift the covered points off the skin and smooth them out.
    let mut lifted: Vec<Vec3> = (0..points)
        .map(|w| point[w] + normal[w] * thick[w])
        .collect();
    let most = smooth.iter().copied().max().unwrap_or(0);
    for pass in 0..most {
        let before = lifted.clone();
        for w in 0..points {
            if pass >= smooth[w] || neighbors[w].is_empty() {
                continue;
            }
            let near = &neighbors[w];
            let mean = near.iter().map(|&n| before[n]).sum::<Vec3>() / near.len() as f32;
            let mut q = before[w] + (mean - before[w]) * 0.5;
            // Never sink into the skin, nor drift far from it.
            let t = thick[w];
            let off = (q - point[w]).dot(normal[w]);
            if off < t * 0.8 {
                q += normal[w] * (t * 0.8 - off);
            }
            let away = q - point[w];
            if away.length() > t * 4.0 {
                q = point[w] + away.normalize() * t * 4.0;
            }
            lifted[w] = q;
        }
    }

    // The shell: the covered triangles, at the lifted points.
    let mut out = ModelPart {
        name: "Clothes".into(),
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        indices: Vec::new(),
        joints: Vec::new(),
        weights: Vec::new(),
        ..mesh.clone()
    };
    let add = |out: &mut ModelPart, v: usize, at: Vec3, n: Vec3| {
        out.positions.push(at);
        out.normals.push(n);
        out.uvs.push(mesh.uvs[v]);
        out.joints.push(mesh.joints[v]);
        out.weights.push(mesh.weights[v]);
        (out.positions.len() - 1) as u32
    };
    // Smooth normals over the shell, shared across texture seams, and how
    // many covered triangles use each welded edge, to find the rims.
    let mut shell_normal = vec![Vec3::ZERO; points];
    let mut edges: HashMap<(usize, usize), u32> = HashMap::new();
    for (t, c) in tris.iter().zip(&covers) {
        if c.is_none() {
            continue;
        }
        let w = t.map(|i| weld[i as usize]);
        let p = w.map(|w| lifted[w]);
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        for k in 0..3 {
            shell_normal[w[k]] += n;
            let (a, b) = (w[k], w[(k + 1) % 3]);
            *edges.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let mut slot = vec![u32::MAX; mesh.positions.len()];
    for (t, c) in tris.iter().zip(&covers) {
        if c.is_none() {
            continue;
        }
        for &i in t {
            let v = i as usize;
            if slot[v] == u32::MAX {
                let w = weld[v];
                let n = shell_normal[w].normalize_or_zero();
                let n = if n == Vec3::ZERO { normal[w] } else { n };
                slot[v] = add(&mut out, v, lifted[w], n);
            }
            out.indices.push(slot[v]);
        }
    }
    // Rims: where a piece ends, a strip from its edge back to the skin, so
    // the cloth has some thickness.
    for (t, c) in tris.iter().zip(&covers) {
        if c.is_none() {
            continue;
        }
        for k in 0..3 {
            let (va, vb, vc) = (
                t[k] as usize,
                t[(k + 1) % 3] as usize,
                t[(k + 2) % 3] as usize,
            );
            let (a, b) = (weld[va], weld[vb]);
            if a == b || edges[&(a.min(b), a.max(b))] != 1 {
                continue;
            }
            let along = lifted[b] - lifted[a];
            let up = (normal[a] + normal[b]).normalize_or_zero();
            let mut out_dir = along.cross(up).normalize_or_zero();
            if out_dir.dot(lifted[a] - lifted[weld[vc]]) < 0.0 {
                out_dir = -out_dir;
            }
            let sink = |w: usize| point[w] + normal[w] * 0.001;
            let first = out.positions.len() as u32;
            for (v, at) in [
                (va, lifted[a]),
                (vb, lifted[b]),
                (vb, sink(b)),
                (va, sink(a)),
            ] {
                add(&mut out, v, at, out_dir);
            }
            // Wound to face out, as the shell is.
            out.indices
                .extend([first, first + 2, first + 1, first, first + 3, first + 2]);
        }
    }
    Dressed {
        skin,
        clothes: Some(out),
    }
}
