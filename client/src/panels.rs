//! Bigger windows: the world map (M), the talent tree (N) and the sandbox
//! panel. Each has a layout function used both to draw it and to work out
//! what a click hit.

use macroquad::prelude::*;
use shared::data::*;
use shared::dungeon::{self, DungeonId};
use shared::props::{self, PropKind};
use shared::protocol::{Destination, EntityKind, SandboxCmd};
use shared::talents::{self, TALENTS, TIER_RANKS, TIER_REQUIRES};
use shared::world::*;

use crate::drag::{self, Win};
use crate::game::Game;
use crate::hud::{
    BORDER, GOLD, PANEL, button, button_ex, class_color, panel, reaction_color, text,
    text_centered, text_width, wrap,
};
use crate::render;

// ---- Compass ----

/// The four directions in world space (x, z): north is +Z, and since the
/// world is seen from above with north up, east is -X.
pub const COMPASS: [(&str, Vec2); 4] = [
    ("N", Vec2::new(0.0, 1.0)),
    ("E", Vec2::new(-1.0, 0.0)),
    ("S", Vec2::new(0.0, -1.0)),
    ("W", Vec2::new(1.0, 0.0)),
];

// ---- World map ----

const MAP_PIXELS: u16 = 320;

/// A picture of a zone, seen from above with north up.
pub fn map_texture(zone: Zone) -> Texture2D {
    let n = MAP_PIXELS as usize;
    let mut img = Image::gen_image_color(MAP_PIXELS, MAP_PIXELS, BLACK);
    let center = zone.center();
    let to_world = |i: f32, j: f32| {
        // Column 0 is the west edge (+X), row 0 the north edge (+Z).
        let x = center.x + WORLD_HALF_SIZE - (i + 0.5) / n as f32 * 2.0 * WORLD_HALF_SIZE;
        let z = center.y + WORLD_HALF_SIZE - (j + 0.5) / n as f32 * 2.0 * WORLD_HALF_SIZE;
        vec2(x, z)
    };
    for j in 0..n {
        for i in 0..n {
            let w = to_world(i as f32, j as f32);
            img.set_pixel(i as u32, j as u32, render::map_color(zone, w));
        }
    }
    // Buildings and other landmarks.
    let to_pixel = |p: Vec3| {
        let rel = vec2(p.x - center.x, p.z - center.y);
        vec2(
            (WORLD_HALF_SIZE - rel.x) / (2.0 * WORLD_HALF_SIZE) * n as f32,
            (WORLD_HALF_SIZE - rel.y) / (2.0 * WORLD_HALF_SIZE) * n as f32,
        )
    };
    let mut dot = |p: Vec2, r: f32, c: Color| {
        let (x0, x1) = ((p.x - r).floor() as i32, (p.x + r).ceil() as i32);
        let (y0, y1) = ((p.y - r).floor() as i32, (p.y + r).ceil() as i32);
        for y in y0.max(0)..=y1.min(n as i32 - 1) {
            for x in x0.max(0)..=x1.min(n as i32 - 1) {
                if vec2(x as f32 + 0.5, y as f32 + 0.5).distance(p) <= r {
                    img.set_pixel(x as u32, y as u32, c);
                }
            }
        }
    };
    for prop in props::props(zone) {
        let p = to_pixel(prop.pos);
        match prop.kind {
            PropKind::House => dot(p, 2.6, Color::new(0.25, 0.18, 0.14, 1.0)),
            PropKind::Centerpiece | PropKind::Well => dot(p, 1.6, Color::new(0.6, 0.6, 0.65, 1.0)),
            PropKind::Tent => dot(p, 1.5, Color::new(0.45, 0.3, 0.2, 1.0)),
            PropKind::Pillar => dot(p, 1.0, Color::new(0.75, 0.75, 0.78, 1.0)),
            PropKind::Mesa => dot(p, 3.0, Color::new(0.55, 0.32, 0.2, 1.0)),
            PropKind::Tree | PropKind::Conifer | PropKind::DeadTree | PropKind::Cactus => {
                let g = render::mix(render::foliage(zone), BLACK, 0.35);
                dot(p, 0.9, g)
            }
            _ => {}
        }
    }
    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Linear);
    tex
}

/// "Gray Wolf" to "Gray Wolves", "Wild Boar" to "Wild Boars".
fn plural(name: &str) -> String {
    match name.strip_suffix("Wolf") {
        Some(stem) => format!("{stem}Wolves"),
        None => format!("{name}s"),
    }
}

pub fn map_rect() -> Rect {
    let (w, h) = (screen_width(), screen_height());
    let size = (w.min(h) - 120.0).max(300.0);
    Rect::new((w - size) / 2.0, (h - size) / 2.0 + 10.0, size, size)
}

pub fn draw_map(game: &Game, tex: &Texture2D) {
    let r = map_rect();
    let zone = game.zone;
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.55),
    );
    // A parchment frame with a carved title plate.
    let frame = Rect::new(r.x - 18.0, r.y - 52.0, r.w + 36.0, r.h + 70.0);
    draw_rectangle(
        frame.x,
        frame.y,
        frame.w,
        frame.h,
        Color::new(0.33, 0.22, 0.12, 1.0),
    );
    draw_rectangle(
        frame.x + 5.0,
        frame.y + 5.0,
        frame.w - 10.0,
        frame.h - 10.0,
        Color::new(0.82, 0.7, 0.5, 1.0),
    );
    draw_rectangle_lines(
        frame.x + 5.0,
        frame.y + 5.0,
        frame.w - 10.0,
        frame.h - 10.0,
        2.0,
        Color::new(0.55, 0.4, 0.22, 1.0),
    );
    for (x, y) in [
        (frame.x, frame.y),
        (frame.right(), frame.y),
        (frame.x, frame.bottom()),
        (frame.right(), frame.bottom()),
    ] {
        draw_circle(x, y, 10.0, Color::new(0.85, 0.68, 0.3, 1.0));
        draw_circle(x, y, 6.0, Color::new(0.4, 0.28, 0.14, 1.0));
    }
    let plate = Rect::new(r.x + r.w / 2.0 - 230.0, frame.y + 10.0, 460.0, 32.0);
    draw_rectangle(
        plate.x,
        plate.y,
        plate.w,
        plate.h,
        Color::new(0.3, 0.2, 0.1, 0.95),
    );
    draw_rectangle_lines(
        plate.x,
        plate.y,
        plate.w,
        plate.h,
        2.0,
        Color::new(0.85, 0.68, 0.3, 1.0),
    );
    text_centered(
        &format!("{}  -  {}", zone.name(), zone.subtitle()),
        r.x + r.w / 2.0,
        plate.y + 23.0,
        22.0,
        GOLD,
    );
    // The land, inked onto the parchment.
    draw_texture_ex(
        tex,
        r.x,
        r.y,
        Color::new(1.0, 0.94, 0.82, 1.0),
        DrawTextureParams {
            dest_size: Some(vec2(r.w, r.h)),
            ..Default::default()
        },
    );
    // Faded, ragged edges.
    for k in 0..6 {
        let a = 0.22 - k as f32 * 0.035;
        let w = k as f32 * 4.0;
        let c = Color::new(0.82, 0.7, 0.5, a.max(0.0) * 3.0);
        draw_rectangle(r.x, r.y + w, r.w, 4.0, c);
        draw_rectangle(r.x, r.bottom() - w - 4.0, r.w, 4.0, c);
        draw_rectangle(r.x + w, r.y, 4.0, r.h, c);
        draw_rectangle(r.right() - w - 4.0, r.y, 4.0, r.h, c);
    }
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(0.45, 0.32, 0.18, 1.0));
    let center = zone.center();
    let to_screen = |p: Vec2| {
        let rel = p - center;
        vec2(
            r.x + (WORLD_HALF_SIZE - rel.x) / (2.0 * WORLD_HALF_SIZE) * r.w,
            r.y + (WORLD_HALF_SIZE - rel.y) / (2.0 * WORLD_HALF_SIZE) * r.h,
        )
    };
    // Labels: the town, the camps and the elite.
    let layout = zone.layout();
    let mobs = MobKind::for_zone(zone);
    let town = to_screen(center);
    // Quest objectives: yellow areas where what you hunt lives.
    let hunting: Vec<MobKind> = game
        .me
        .quests
        .active
        .iter()
        .filter_map(|(id, p)| match shared::quests::quest(*id).goal {
            shared::quests::Goal::Kill { kind, count } if *p < count => Some(kind),
            _ => None,
        })
        .collect();
    for site in &layout.sites {
        for sp in &site.spawns {
            if hunting.contains(&mobs[sp.role]) {
                let p = to_screen(zone.to_world(site.center + sp.offset));
                let rad = (sp.radius + 6.0) / (2.0 * WORLD_HALF_SIZE) * r.w;
                draw_circle(p.x, p.y, rad, Color::new(1.0, 0.85, 0.2, 0.25));
                draw_circle_lines(p.x, p.y, rad, 2.0, Color::new(1.0, 0.85, 0.2, 0.8));
            }
        }
    }
    // The town: a shield with its name on a banner.
    draw_circle(town.x, town.y, 10.0, Color::new(0.3, 0.2, 0.1, 1.0));
    draw_circle(town.x, town.y, 7.0, Color::new(0.85, 0.68, 0.3, 1.0));
    let tw = text_width(zone.town_name(), 20.0) + 16.0;
    draw_rectangle(
        town.x - tw / 2.0,
        town.y - 40.0,
        tw,
        24.0,
        Color::new(0.25, 0.17, 0.08, 0.85),
    );
    text_centered(zone.town_name(), town.x, town.y - 22.0, 20.0, GOLD);
    for site in &layout.sites {
        let p = to_screen(zone.to_world(site.center));
        let spawn = site.spawns[0];
        let name = mobs[spawn.role].template().name;
        let (lo, hi) = spawn.levels;
        let levels = if lo == hi {
            format!("{lo}")
        } else {
            format!("{lo}-{hi}")
        };
        let label = match site.kind {
            shared::layout::SiteKind::Ruins => format!("{name} (Elite {levels})"),
            shared::layout::SiteKind::Camp => format!("{name} camp ({levels})"),
            shared::layout::SiteKind::Beasts => format!("{} ({levels})", plural(name)),
        };
        match site.kind {
            shared::layout::SiteKind::Ruins => {
                // A skull for the elite.
                draw_circle(p.x, p.y, 9.0, Color::new(0.15, 0.1, 0.08, 1.0));
                draw_circle(p.x, p.y - 1.0, 6.0, Color::new(0.95, 0.9, 0.8, 1.0));
                draw_circle(p.x - 2.5, p.y - 1.5, 1.6, BLACK);
                draw_circle(p.x + 2.5, p.y - 1.5, 1.6, BLACK);
                draw_rectangle(
                    p.x - 3.0,
                    p.y + 3.0,
                    6.0,
                    3.0,
                    Color::new(0.95, 0.9, 0.8, 1.0),
                );
            }
            shared::layout::SiteKind::Camp => {
                // A tent.
                draw_triangle(
                    vec2(p.x, p.y - 8.0),
                    vec2(p.x - 8.0, p.y + 5.0),
                    vec2(p.x + 8.0, p.y + 5.0),
                    Color::new(0.55, 0.25, 0.15, 1.0),
                );
                draw_triangle_lines(
                    vec2(p.x, p.y - 8.0),
                    vec2(p.x - 8.0, p.y + 5.0),
                    vec2(p.x + 8.0, p.y + 5.0),
                    1.5,
                    BLACK,
                );
            }
            shared::layout::SiteKind::Beasts => {
                // A paw print.
                draw_circle(p.x, p.y + 1.5, 3.5, Color::new(0.35, 0.22, 0.12, 0.9));
                for k in 0..4 {
                    let a = -2.4 + k as f32 * 0.55;
                    draw_circle(
                        p.x + a.cos() * 5.5,
                        p.y + a.sin() * 5.5,
                        1.6,
                        Color::new(0.35, 0.22, 0.12, 0.9),
                    );
                }
            }
        }
        let lw = text_width(&label, 15.0);
        draw_rectangle(
            p.x - lw / 2.0 - 4.0,
            p.y - 25.0,
            lw + 8.0,
            18.0,
            Color::new(0.2, 0.13, 0.06, 0.6),
        );
        text_centered(
            &label,
            p.x,
            p.y - 11.0,
            15.0,
            Color::new(1.0, 0.92, 0.75, 1.0),
        );
    }
    // Everyone you can see.
    for e in game.entities.values() {
        if Some(e.view.id) == game.my_id || e.view.dead {
            continue;
        }
        let p = to_screen(vec2(e.pos.x, e.pos.z));
        if !r.contains(p) {
            continue;
        }
        let c = match e.view.kind {
            EntityKind::Player(class) => class_color(class),
            EntityKind::Merchant(_) => GOLD,
            EntityKind::QuestGiver(_) => {
                if let Some(m) = crate::quests_ui::marker(
                    &game.me.quests,
                    Place::Zone(zone),
                    game.level(),
                    &game.me.bags,
                ) {
                    let s = if m == crate::quests_ui::Marker::Ready {
                        "?"
                    } else {
                        "!"
                    };
                    text_centered(s, p.x + 1.0, p.y + 9.0, 28.0, BLACK);
                    text_centered(s, p.x, p.y + 8.0, 28.0, Color::new(1.0, 0.85, 0.1, 1.0));
                    continue;
                }
                GOLD
            }
            _ => reaction_color(&e.view, game),
        };
        draw_circle(p.x, p.y, 3.0, c);
    }
    // You, as an arrow.
    let me = to_screen(vec2(game.pos.x, game.pos.z));
    let f = forward(game.yaw);
    // On the map, +Z is up and +X is left.
    let dir = vec2(-f.x, -f.z);
    let side = vec2(-dir.y, dir.x);
    draw_triangle(
        me + dir * 11.0,
        me - dir * 7.0 + side * 7.0,
        me - dir * 7.0 - side * 7.0,
        Color::new(1.0, 0.95, 0.4, 1.0),
    );
    draw_triangle_lines(
        me + dir * 11.0,
        me - dir * 7.0 + side * 7.0,
        me - dir * 7.0 - side * 7.0,
        1.5,
        BLACK,
    );
    // Compass.
    let c = vec2(r.right() - 46.0, r.y + 46.0);
    draw_circle(c.x, c.y, 34.0, Color::new(0.0, 0.0, 0.0, 0.55));
    draw_circle_lines(c.x, c.y, 34.0, 2.0, BORDER);
    for (label, d) in COMPASS {
        let p = c + vec2(-d.x, -d.y) * 22.0;
        let color = if label == "N" {
            Color::new(1.0, 0.35, 0.3, 1.0)
        } else {
            WHITE
        };
        text_centered(label, p.x, p.y + 7.0, 20.0, color);
    }
    text_centered(
        "M or Esc to close",
        r.x + r.w / 2.0,
        r.bottom() + 0.0 - 8.0,
        16.0,
        Color::new(0.85, 0.85, 0.85, 0.9),
    );
}

// ---- Talents ----

pub struct TalentLayout {
    pub window: Rect,
    /// Indexed like `talents::Ranks`.
    pub boxes: [Rect; TALENTS],
    pub reset: Rect,
}

pub fn talent_layout() -> TalentLayout {
    let (w, h) = (screen_width(), screen_height());
    let window = drag::place(
        Win::Talents,
        Rect::new(
            (w - 690.0) / 2.0,
            ((h - 470.0) / 2.0).max(20.0),
            690.0,
            470.0,
        ),
    );
    let boxes = std::array::from_fn(|i| {
        let (branch, tier) = (i / 3, i % 3);
        Rect::new(
            window.x + 20.0 + branch as f32 * 222.0,
            window.y + 96.0 + tier as f32 * 104.0,
            206.0,
            88.0,
        )
    });
    let reset = Rect::new(window.right() - 150.0, window.bottom() - 46.0, 130.0, 32.0);
    TalentLayout {
        window,
        boxes,
        reset,
    }
}

pub fn draw_talents(game: &Game, level: u8) {
    let l = talent_layout();
    let tree = talents::tree(game.class);
    let ranks = game.me.talents;
    let spent = talents::spent(&ranks);
    let points = talents::points(level);
    panel(l.window);
    draw_rectangle(
        l.window.x + 2.0,
        l.window.y + 2.0,
        l.window.w - 4.0,
        26.0,
        Color::new(0.25, 0.17, 0.08, 0.9),
    );
    text(
        &format!(
            "{} Talents ({})",
            game.class.name(),
            crate::keys::key_label(crate::keys::Action::Talents)
        ),
        l.window.x + 10.0,
        l.window.y + 21.0,
        20.0,
        GOLD,
    );
    let left = points.saturating_sub(spent);
    let summary = format!("Points to spend: {left}   (one per level from 2)");
    text(
        &summary,
        l.window.x + 20.0,
        l.window.y + 52.0,
        18.0,
        if left > 0 {
            Color::new(0.4, 1.0, 0.4, 1.0)
        } else {
            WHITE
        },
    );
    for b in 0..3 {
        let x = l.boxes[b * 3].x;
        let in_branch = talents::spent_in_branch(&ranks, b);
        text(
            &format!("{} ({in_branch})", tree.branches[b]),
            x,
            l.window.y + 84.0,
            20.0,
            class_color(game.class),
        );
    }
    let mouse = vec2(mouse_position().0, mouse_position().1);
    let mut hover = None;
    for (i, r) in l.boxes.iter().enumerate() {
        let (branch, tier) = (i / 3, i % 3);
        let talent = &tree.talents[i];
        let rank = ranks[i];
        let max = TIER_RANKS[tier];
        let open = talents::spent_in_branch(&ranks, branch) >= TIER_REQUIRES[tier];
        let learnable = talents::can_learn(&ranks, level, i).is_ok();
        let bg = if rank == max {
            Color::new(0.32, 0.25, 0.08, 0.95)
        } else if rank > 0 {
            Color::new(0.2, 0.25, 0.12, 0.95)
        } else if open {
            Color::new(0.14, 0.14, 0.17, 0.95)
        } else {
            Color::new(0.08, 0.08, 0.09, 0.95)
        };
        draw_rectangle(r.x, r.y, r.w, r.h, bg);
        let edge = if r.contains(mouse) {
            GOLD
        } else if learnable {
            Color::new(0.4, 1.0, 0.4, 1.0)
        } else {
            BORDER
        };
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, edge);
        let name_color = if open {
            WHITE
        } else {
            Color::new(0.5, 0.5, 0.5, 1.0)
        };
        text(talent.name, r.x + 8.0, r.y + 22.0, 18.0, name_color);
        let rank_s = format!("{rank}/{max}");
        text(
            &rank_s,
            r.right() - 8.0 - text_width(&rank_s, 18.0),
            r.bottom() - 8.0,
            18.0,
            if rank > 0 {
                GOLD
            } else {
                Color::new(0.7, 0.7, 0.7, 1.0)
            },
        );
        for (k, line) in wrap(&talent.describe(rank.max(1)), 30)
            .iter()
            .take(3)
            .enumerate()
        {
            text(
                line,
                r.x + 8.0,
                r.y + 40.0 + k as f32 * 15.0,
                14.0,
                Color::new(0.85, 0.82, 0.7, 1.0),
            );
        }
        if r.contains(mouse) {
            hover = Some(i);
        }
    }
    if let Some(i) = hover {
        let tier = i % 3;
        let talent = &tree.talents[i];
        let rank = ranks[i];
        let mut lines = vec![(talent.name.to_string(), WHITE)];
        lines.push((
            format!("Rank {rank} of {}", TIER_RANKS[tier]),
            Color::new(0.8, 0.8, 0.8, 1.0),
        ));
        if rank > 0 {
            lines.push((talent.describe(rank), GOLD));
        }
        if rank < TIER_RANKS[tier] {
            lines.push((
                format!("Next rank: {}", talent.describe(rank + 1)),
                Color::new(0.6, 1.0, 0.6, 1.0),
            ));
        }
        if let Err(why) = talents::can_learn(&ranks, level, i)
            && rank < TIER_RANKS[tier]
        {
            lines.push((why.to_string(), Color::new(1.0, 0.4, 0.35, 1.0)));
        } else if rank < TIER_RANKS[tier] {
            lines.push(("Click to learn".into(), Color::new(0.6, 0.8, 1.0, 1.0)));
        }
        let r = l.boxes[i];
        let w = 330.0;
        let mut all = Vec::new();
        for (line, c) in lines {
            for (k, part) in wrap(&line, 38).into_iter().enumerate() {
                let part = if k > 0 { format!("  {part}") } else { part };
                all.push((part, c));
            }
        }
        let h = 14.0 + all.len() as f32 * 19.0;
        let x = (r.right() + 8.0).min(screen_width() - w - 4.0);
        let y = r.y.min(screen_height() - h - 4.0);
        draw_rectangle(x, y, w, h, PANEL);
        draw_rectangle_lines(x, y, w, h, 2.0, BORDER);
        for (k, (line, c)) in all.iter().enumerate() {
            text(line, x + 10.0, y + 22.0 + k as f32 * 19.0, 16.0, *c);
        }
    }
    button_ex(l.reset, "Reset talents", spent > 0);
}

/// Which talent (or the reset button, as `TALENTS`) a click hit.
pub fn talent_click(mouse: Vec2) -> Option<usize> {
    let l = talent_layout();
    if l.reset.contains(mouse) {
        return Some(TALENTS);
    }
    l.boxes.iter().position(|r| r.contains(mouse))
}

// ---- Sandbox ----

pub struct SandboxLayout {
    pub window: Rect,
    pub buttons: Vec<(Rect, SandboxAction)>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SandboxAction {
    Command(SandboxCmd),
    LevelDown,
    LevelUp,
    Tab(ItemTab),
}

/// The sandbox's item tabs. Every item is on exactly one of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemTab {
    Weapons,
    Armor(Slot),
    Other,
}

impl ItemTab {
    pub const ALL: [ItemTab; 7] = [
        ItemTab::Weapons,
        ItemTab::Armor(Slot::Head),
        ItemTab::Armor(Slot::Chest),
        ItemTab::Armor(Slot::Hands),
        ItemTab::Armor(Slot::Legs),
        ItemTab::Armor(Slot::Feet),
        ItemTab::Other,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ItemTab::Weapons => "Weapons",
            ItemTab::Armor(slot) => slot.name(),
            ItemTab::Other => "Other",
        }
    }

    /// Which tab an item is listed on: potions, food and materials are Other.
    pub fn of(item: &Item) -> ItemTab {
        match item.kind {
            ItemKind::Weapon { .. } => ItemTab::Weapons,
            ItemKind::Armor { slot, .. } => ItemTab::Armor(slot),
            ItemKind::Material | ItemKind::Potion { .. } | ItemKind::Food { .. } => ItemTab::Other,
        }
    }

    /// The items on this tab, in the game's item order.
    pub fn items(self) -> impl Iterator<Item = ItemId> {
        (0..ITEMS.len())
            .filter(move |&i| ItemTab::of(&ITEMS[i]) == self)
            .map(|i| ItemId(i as u16))
    }
}

thread_local! {
    /// The sandbox item tab that's open. Kept until the game closes.
    static SANDBOX_TAB: std::cell::Cell<ItemTab> = const { std::cell::Cell::new(ItemTab::Weapons) };
}

pub fn sandbox_tab() -> ItemTab {
    SANDBOX_TAB.with(|t| t.get())
}

/// What a click on the sandbox panel did.
pub enum SandboxClick {
    /// Ask the server for this.
    Send(SandboxCmd),
    /// Handled here (an item tab was picked).
    Handled,
}

pub fn sandbox_layout(zone: Zone) -> SandboxLayout {
    let (w, _) = (screen_width(), screen_height());
    let window = drag::place(
        Win::Sandbox,
        Rect::new(w - 16.0 - 300.0 - 280.0, 110.0, 280.0, 560.0),
    );
    let mut buttons = Vec::new();
    let x = window.x + 12.0;
    let iw = window.w - 24.0;
    let half = (iw - 8.0) / 2.0;
    let third = (iw - 16.0) / 3.0;
    let mut y = window.y + 60.0;
    buttons.push((Rect::new(x, y, 40.0, 30.0), SandboxAction::LevelDown));
    buttons.push((
        Rect::new(x + iw - 40.0, y, 40.0, 30.0),
        SandboxAction::LevelUp,
    ));
    y += 40.0;
    buttons.push((
        Rect::new(x, y, half, 30.0),
        SandboxAction::Command(SandboxCmd::AddMoney(100)),
    ));
    buttons.push((
        Rect::new(x + half + 8.0, y, half, 30.0),
        SandboxAction::Command(SandboxCmd::AddMoney(10_000)),
    ));
    y += 38.0;
    buttons.push((
        Rect::new(x, y, half, 30.0),
        SandboxAction::Command(SandboxCmd::ToggleGod),
    ));
    buttons.push((
        Rect::new(x + half + 8.0, y, half, 30.0),
        SandboxAction::Command(SandboxCmd::Refresh),
    ));
    y += 62.0;
    for (i, z) in Zone::ALL.into_iter().enumerate() {
        buttons.push((
            Rect::new(
                x + (i % 3) as f32 * (third + 8.0),
                y + (i / 3) as f32 * 36.0,
                third,
                30.0,
            ),
            SandboxAction::Command(SandboxCmd::Teleport(z)),
        ));
    }
    y += 98.0;
    // The zone's mobs (and its water mob), then a button to clear them.
    let spawns = MobKind::for_zone(zone)
        .into_iter()
        .chain(MobKind::water(zone))
        .map(|kind| SandboxCmd::SpawnMob { kind, level: 0 })
        .chain([SandboxCmd::ClearSpawns]);
    let mut cells = 0usize;
    for (i, cmd) in spawns.enumerate() {
        buttons.push((
            Rect::new(
                x + (i % 2) as f32 * (half + 8.0),
                y + (i / 2) as f32 * 36.0,
                half,
                30.0,
            ),
            SandboxAction::Command(cmd),
        ));
        cells += 1;
    }
    y += cells.div_ceil(2) as f32 * 36.0 + 26.0;
    // Item tabs, four to a row.
    let tab_w = (iw - 3.0 * 4.0) / 4.0;
    for (i, tab) in ItemTab::ALL.into_iter().enumerate() {
        buttons.push((
            Rect::new(
                x + (i % 4) as f32 * (tab_w + 4.0),
                y + (i / 4) as f32 * 28.0,
                tab_w,
                24.0,
            ),
            SandboxAction::Tab(tab),
        ));
    }
    y += ItemTab::ALL.len().div_ceil(4) as f32 * 28.0 + 6.0;
    let per_row = 5;
    let size = (iw - (per_row - 1) as f32 * 6.0) / per_row as f32;
    for (i, id) in sandbox_tab().items().enumerate() {
        buttons.push((
            Rect::new(
                x + (i % per_row) as f32 * (size + 6.0),
                y + (i / per_row) as f32 * (size + 6.0),
                size,
                size,
            ),
            SandboxAction::Command(SandboxCmd::GiveItem(id)),
        ));
    }
    // Tall enough for the fullest tab, so the window doesn't jump around
    // when you switch tabs.
    let most = ItemTab::ALL
        .iter()
        .map(|t| t.items().count())
        .max()
        .unwrap_or(0);
    let rows = most.div_ceil(per_row).max(1);
    let bottom = y + rows as f32 * (size + 6.0) + 10.0;
    SandboxLayout {
        window: Rect::new(window.x, window.y, window.w, bottom - window.y),
        buttons,
    }
}

pub fn draw_sandbox(game: &Game, level: u8) {
    let l = sandbox_layout(game.zone);
    let r = l.window;
    panel(r);
    draw_rectangle(
        r.x + 2.0,
        r.y + 2.0,
        r.w - 4.0,
        26.0,
        Color::new(0.1, 0.25, 0.3, 0.9),
    );
    text(
        &format!(
            "Sandbox ({})",
            crate::keys::key_label(crate::keys::Action::Sandbox)
        ),
        r.x + 10.0,
        r.y + 21.0,
        20.0,
        GOLD,
    );
    let x = r.x + 12.0;
    let label = |s: &str, y: f32| text(s, x, y, 16.0, Color::new(0.75, 0.85, 0.9, 1.0));
    label("Level", r.y + 52.0);
    text_centered(
        &format!("Level {level}"),
        r.x + r.w / 2.0,
        r.y + 82.0,
        22.0,
        WHITE,
    );
    let mouse = vec2(mouse_position().0, mouse_position().1);
    let mut item_tip = None;
    for (b, action) in &l.buttons {
        match action {
            SandboxAction::LevelDown => button_ex(*b, "-", level > 1),
            SandboxAction::LevelUp => button_ex(*b, "+", level < MAX_LEVEL),
            SandboxAction::Tab(tab) => {
                let open = *tab == sandbox_tab();
                let bg = if open {
                    Color::new(0.2, 0.42, 0.5, 1.0)
                } else if b.contains(mouse) {
                    Color::new(0.16, 0.22, 0.28, 1.0)
                } else {
                    Color::new(0.1, 0.13, 0.17, 1.0)
                };
                draw_rectangle(b.x, b.y, b.w, b.h, bg);
                draw_rectangle_lines(b.x, b.y, b.w, b.h, 1.5, if open { GOLD } else { BORDER });
                text_centered(
                    tab.name(),
                    b.x + b.w / 2.0,
                    b.y + 17.0,
                    15.0,
                    if open { GOLD } else { WHITE },
                );
            }
            SandboxAction::Command(cmd) => match cmd {
                SandboxCmd::AddMoney(c) => button(*b, &format!("+{}", format_money(*c))),
                SandboxCmd::ToggleGod => {
                    button(*b, if game.me.god { "God: ON" } else { "God: off" });
                    if game.me.god {
                        draw_rectangle_lines(
                            b.x,
                            b.y,
                            b.w,
                            b.h,
                            2.0,
                            Color::new(0.4, 1.0, 0.4, 1.0),
                        );
                    }
                }
                SandboxCmd::Refresh => button(*b, "Refresh"),
                SandboxCmd::Teleport(z) => {
                    let short = z.name().split(' ').next().unwrap_or("");
                    button(*b, short);
                }
                SandboxCmd::SpawnMob { kind, .. } => button(*b, kind.template().name),
                SandboxCmd::ClearSpawns => button(*b, "Clear spawns"),
                SandboxCmd::GiveItem(id) => {
                    crate::hud::item_icon(*b, *id, 1);
                    if b.contains(mouse) {
                        draw_rectangle_lines(b.x, b.y, b.w, b.h, 2.0, GOLD);
                        item_tip = Some(item(*id).name);
                    }
                }
                SandboxCmd::SetLevel(_) => {}
            },
        }
    }
    let first =
        |f: fn(&SandboxAction) -> bool| l.buttons.iter().find(|(_, a)| f(a)).map(|(r, _)| r.y);
    if let Some(y) = first(|a| matches!(a, SandboxAction::Command(SandboxCmd::Teleport(_)))) {
        label("Teleport to a town", y - 8.0);
    }
    if let Some(y) = first(|a| matches!(a, SandboxAction::Command(SandboxCmd::SpawnMob { .. }))) {
        label("Summon (at your level)", y - 8.0);
    }
    if let Some(y) = first(|a| matches!(a, SandboxAction::Tab(_))) {
        label("Give yourself an item", y - 8.0);
    }
    if let Some(name) = item_tip {
        text_centered(name, mouse.x, mouse.y - 14.0, 18.0, WHITE);
    }
}

/// What a click on the sandbox panel asks the server for.
pub fn sandbox_click(zone: Zone, level: u8, mouse: Vec2) -> Option<SandboxClick> {
    let l = sandbox_layout(zone);
    let (_, action) = l.buttons.iter().find(|(r, _)| r.contains(mouse))?;
    Some(SandboxClick::Send(match *action {
        SandboxAction::Tab(tab) => {
            SANDBOX_TAB.with(|t| t.set(tab));
            return Some(SandboxClick::Handled);
        }
        SandboxAction::LevelDown => SandboxCmd::SetLevel(level.saturating_sub(1).max(1)),
        SandboxAction::LevelUp => SandboxCmd::SetLevel((level + 1).min(MAX_LEVEL)),
        SandboxAction::Command(SandboxCmd::SpawnMob { kind, .. }) => {
            SandboxCmd::SpawnMob { kind, level }
        }
        SandboxAction::Command(cmd) => cmd,
    }))
}

/// The compass letters around the minimap, which turns with the camera.
pub fn minimap_compass(center: Vec2, radius: f32, cam_yaw: f32) {
    let f = vec2(cam_yaw.sin(), cam_yaw.cos());
    let r = vec2(-f.y, f.x);
    for (label, d) in COMPASS {
        let m = vec2(d.dot(r), -d.dot(f)).normalize_or_zero() * (radius - 9.0);
        let p = center + m;
        draw_circle(p.x, p.y, 9.0, Color::new(0.0, 0.0, 0.0, 0.6));
        let color = if label == "N" {
            Color::new(1.0, 0.35, 0.3, 1.0)
        } else {
            WHITE
        };
        text_centered(label, p.x, p.y + 5.0, 16.0, color);
    }
}

/// The land under the minimap: the zone's map picture, cut to a circle and
/// turned with the camera.
pub fn minimap_terrain(
    tex: &Texture2D,
    zone: Zone,
    center: Vec2,
    radius: f32,
    me: Vec3,
    cam_yaw: f32,
    range: f32,
) {
    use macroquad::models::{Mesh, Vertex, draw_mesh};
    let f = vec2(cam_yaw.sin(), cam_yaw.cos());
    let r = vec2(-f.y, f.x);
    let zc = zone.center();
    let scale = range / radius;
    let uv = |m: Vec2| {
        // Inverse of the minimap's projection: screen offset to world.
        let rel = (r * m.x - f * m.y) * scale;
        let w = vec2(me.x, me.z) + rel;
        vec2(
            (WORLD_HALF_SIZE - (w.x - zc.x)) / (2.0 * WORLD_HALF_SIZE),
            (WORLD_HALF_SIZE - (w.y - zc.y)) / (2.0 * WORLD_HALF_SIZE),
        )
    };
    let sides = 48;
    let mut vertices = Vec::with_capacity(sides + 1);
    let vertex = |m: Vec2| Vertex {
        position: vec3(center.x + m.x, center.y + m.y, 0.0),
        uv: uv(m),
        color: [255, 255, 255, 240],
        normal: Vec4::ZERO,
    };
    vertices.push(vertex(Vec2::ZERO));
    for i in 0..sides {
        let a = i as f32 / sides as f32 * std::f32::consts::TAU;
        vertices.push(vertex(vec2(a.cos(), a.sin()) * radius));
    }
    let mut indices = Vec::with_capacity(sides * 3);
    for i in 0..sides as u16 {
        indices.extend_from_slice(&[0, 1 + i, 1 + (i + 1) % sides as u16]);
    }
    draw_mesh(&Mesh {
        vertices,
        indices,
        texture: Some(tex.clone()),
    });
}

// ---- Waystones ----

pub struct TravelLayout {
    pub window: Rect,
    pub close: Rect,
    /// Each destination's button, and whether you can go there.
    pub buttons: Vec<(Rect, Destination, bool)>,
}

/// Where the waystone window sits.
pub fn travel_window() -> Rect {
    let rows = Zone::ALL.len() + DungeonId::ALL.len();
    let h = 92.0 + rows as f32 * 40.0 + 30.0;
    drag::place(
        Win::Travel,
        Rect::new(screen_width() / 2.0 - 190.0, 140.0, 380.0, h),
    )
}

pub fn travel_layout(place: Place, level: u8) -> TravelLayout {
    let window = travel_window();
    let x = window.x + 16.0;
    let w = window.w - 32.0;
    let mut buttons = Vec::new();
    let mut y = window.y + 62.0;
    for z in Zone::ALL {
        buttons.push((
            Rect::new(x, y, w, 32.0),
            Destination::Town(z),
            place != Place::Zone(z),
        ));
        y += 40.0;
    }
    y += 26.0;
    for d in DungeonId::ALL {
        let ok = matches!(place, Place::Zone(_)) && level >= d.get().min_level;
        buttons.push((Rect::new(x, y, w, 32.0), Destination::Dungeon(d), ok));
        y += 40.0;
    }
    TravelLayout {
        window,
        close: Rect::new(window.right() - 30.0, window.y + 6.0, 22.0, 22.0),
        buttons,
    }
}

pub fn draw_travel(game: &Game) {
    let l = travel_layout(game.place, game.level());
    let r = l.window;
    panel(r);
    text(
        &format!(
            "Waystone ({})",
            crate::keys::key_label(crate::keys::Action::Travel)
        ),
        r.x + 12.0,
        r.y + 26.0,
        22.0,
        GOLD,
    );
    button(l.close, "x");
    for (b, to, ok) in &l.buttons {
        let label = match to {
            Destination::Town(z) if game.place == Place::Zone(*z) => {
                format!("{}  (you are here)", z.town_name())
            }
            Destination::Town(z) => format!("{}  -  {}", z.town_name(), z.name()),
            Destination::Dungeon(d) if game.dungeon().is_some_and(|i| dungeon::of(i).id == *d) => {
                format!("{}  (you are here)", d.get().name)
            }
            Destination::Dungeon(d) => {
                format!("{}  (level {}+, party)", d.get().name, d.get().min_level)
            }
        };
        button_ex(*b, &label, *ok);
    }
    if let Some((b, _, _)) = l.buttons.get(Zone::ALL.len()) {
        text(
            "Dungeons",
            b.x,
            b.y - 8.0,
            16.0,
            Color::new(0.75, 0.85, 0.9, 1.0),
        );
    }
    if let Some((b, _, _)) = l.buttons.first() {
        text(
            "Towns",
            b.x,
            b.y - 8.0,
            16.0,
            Color::new(0.75, 0.85, 0.9, 1.0),
        );
    }
}

// ---- Dungeon maps ----

/// The dungeon's floor under the minimap, around you and turned with the
/// camera.
pub fn minimap_halls(center: Vec2, radius: f32, me: Vec3, cam_yaw: f32, range: f32) {
    let f = vec2(cam_yaw.sin(), cam_yaw.cos());
    let r = vec2(-f.y, f.x);
    let scale = radius / range;
    let step = 2.0;
    let floor = Color::new(0.42, 0.5, 0.5, 0.9);
    let n = (range / step) as i32;
    for i in -n..=n {
        for j in -n..=n {
            let rel = vec2(i as f32, j as f32) * step;
            let m = vec2(rel.dot(r), -rel.dot(f)) * scale;
            if m.length() > radius - 2.0 {
                continue;
            }
            let p = me + vec3(rel.x, 0.0, rel.y);
            if dungeon::at(me).is_some_and(|d| d.open(dungeon::to_local(p))) {
                let s = step * scale + 0.6;
                draw_rectangle(
                    center.x + m.x - s / 2.0,
                    center.y + m.y - s / 2.0,
                    s,
                    s,
                    floor,
                );
            }
        }
    }
}

/// The dungeon's plan, with its rooms named and you on it.
pub fn draw_dungeon_map(game: &Game) {
    let Some(d) = dungeon::at(game.pos) else {
        return;
    };
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.55),
    );
    let r = map_rect();
    panel(Rect::new(r.x - 18.0, r.y - 52.0, r.w + 36.0, r.h + 70.0));
    text_centered(d.name, r.x + r.w / 2.0, r.y - 20.0, 24.0, GOLD);
    // Fit every hall in the square, entrance at the bottom.
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for h in d.halls {
        lo = lo.min(h.center - h.half);
        hi = hi.max(h.center + h.half);
    }
    let span = (hi - lo).max_element() + 12.0;
    let mid = (lo + hi) / 2.0;
    let scale = r.w / span;
    // North (+Z) is up; +X is to the left, as on the world map.
    let to_screen = |p: Vec2| {
        vec2(
            r.x + r.w / 2.0 - (p.x - mid.x) * scale,
            r.y + r.h / 2.0 - (p.y - mid.y) * scale,
        )
    };
    let floor = Color::new(0.35, 0.42, 0.42, 1.0);
    for h in d.halls {
        let a = to_screen(h.center + h.half);
        let b = to_screen(h.center - h.half);
        draw_rectangle(a.x, a.y, b.x - a.x, b.y - a.y, floor);
    }
    for h in d.halls.iter().filter(|h| !h.name.is_empty()) {
        let p = to_screen(h.center);
        text_centered(h.name, p.x, p.y + 6.0, 18.0, WHITE);
    }
    let stone = to_screen(dungeon::EXIT_STONE);
    draw_circle(stone.x, stone.y, 5.0, Color::new(0.45, 0.9, 1.0, 1.0));
    // Your party, then you.
    for e in game.entities.values() {
        if e.view.kind.is_player() && Some(e.view.id) != game.my_id {
            let p = to_screen(dungeon::to_local(e.pos));
            draw_circle(p.x, p.y, 4.0, Color::new(0.3, 0.6, 1.0, 1.0));
        }
    }
    let me = to_screen(dungeon::to_local(game.pos));
    let d = game.yaw;
    let dir = vec2(-d.sin(), -d.cos());
    let side = vec2(-dir.y, dir.x);
    draw_triangle(
        me + dir * 9.0,
        me - dir * 6.0 + side * 6.0,
        me - dir * 6.0 - side * 6.0,
        Color::new(1.0, 0.95, 0.4, 1.0),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_is_on_one_sandbox_tab() {
        let listed: usize = ItemTab::ALL.iter().map(|t| t.items().count()).sum();
        assert_eq!(listed, ITEMS.len());
        for tab in ItemTab::ALL {
            assert!(tab.items().next().is_some(), "{tab:?} is empty");
        }
    }
}
