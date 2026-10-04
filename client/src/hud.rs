//! The 2D interface drawn over the world: unit frames, action bar, cast bar,
//! nameplates, chat, minimap, floating combat text and the bag, character
//! crafting and merchant windows.

use macroquad::prelude::*;
use shared::data::*;
use shared::protocol::*;
use shared::world::Zone;

use crate::game::{Game, Windows, wrap_angle};
use crate::render;

pub const PANEL: Color = Color::new(0.07, 0.06, 0.08, 0.86);
pub const BORDER: Color = Color::new(0.6, 0.48, 0.3, 1.0);
pub const GOLD: Color = Color::new(1.0, 0.82, 0.25, 1.0);

const SLOT: f32 = 44.0;

/// Where everything goes on screen this frame.
pub struct Layout {
    pub player_frame: Rect,
    pub target_frame: Rect,
    pub tot_frame: Rect,
    pub hotbar: [Rect; ACTION_BAR_SLOTS],
    pub castbar: Rect,
    pub xpbar: Rect,
    pub minimap: (Vec2, f32),
    pub release_button: Rect,
    pub menu_buttons: [Rect; 3],
    pub bags: Option<Rect>,
    pub bag_slots: Vec<Rect>,
    pub character: Option<Rect>,
    pub gear_slots: Vec<Rect>,
    pub crafting: Option<Rect>,
    pub craft_buttons: Vec<Rect>,
    pub vendor: Option<Rect>,
    pub vendor_buttons: Vec<Rect>,
    pub talents: Option<Rect>,
    pub sandbox: Option<Rect>,
    pub map: bool,
    pub quest_window: Option<Rect>,
    pub party: PartyLayout,
}

/// Party frames down the left, and the buttons that go with parties.
#[derive(Default)]
pub struct PartyLayout {
    /// The other members' frames.
    pub frames: Vec<(Rect, EntityId)>,
    /// The leader's remove buttons on those frames.
    pub kicks: Vec<(Rect, String)>,
    pub leave: Option<Rect>,
    /// The invite popup, with its Accept and Decline buttons.
    pub popup: Option<(Rect, Rect, Rect)>,
    /// Invite whoever you've targeted.
    pub invite_target: Option<(Rect, String)>,
}

impl PartyLayout {
    pub fn new(game: &Game, target_frame: Rect) -> Self {
        let mut l = Self::default();
        let me = game.my_id;
        let party = game.me.party.as_ref();
        let leader = party.is_none_or(|p| Some(p.leader) == me);
        if let Some(p) = party {
            let mut y = PARTY_TOP;
            for m in p.members.iter().filter(|m| Some(m.id) != me) {
                let r = Rect::new(16.0, y, 200.0, 44.0);
                l.frames.push((r, m.id));
                if leader {
                    l.kicks.push((
                        Rect::new(r.right() - 20.0, r.y + 4.0, 16.0, 14.0),
                        m.name.clone(),
                    ));
                }
                y += 50.0;
            }
            l.leave = Some(Rect::new(16.0, y, 110.0, 24.0));
        }
        if game.me.invite.is_some() {
            let (w, h) = (340.0, 100.0);
            let r = Rect::new((screen_width() - w) / 2.0, screen_height() * 0.42, w, h);
            l.popup = Some((
                r,
                Rect::new(r.x + 30.0, r.y + 52.0, 125.0, 34.0),
                Rect::new(r.right() - 155.0, r.y + 52.0, 125.0, 34.0),
            ));
        }
        let full = party.is_some_and(|p| p.members.len() >= MAX_PARTY_SIZE);
        if let Some(t) = game.target_ent()
            && t.view.kind.is_player()
            && Some(t.view.id) != me
            && leader
            && !full
            && party.is_none_or(|p| p.members.iter().all(|m| m.id != t.view.id))
        {
            l.invite_target = Some((
                Rect::new(
                    target_frame.right() + 16.0,
                    target_frame.bottom() - 4.0,
                    120.0,
                    26.0,
                ),
                t.view.name.clone(),
            ));
        }
        l
    }

    fn rects(&self) -> impl Iterator<Item = Rect> + '_ {
        self.frames
            .iter()
            .map(|(r, _)| *r)
            .chain(self.leave)
            .chain(self.popup.map(|p| p.0))
            .chain(self.invite_target.as_ref().map(|(r, _)| *r))
    }
}

/// Where the first party frame goes, below your own frame and auras.
const PARTY_TOP: f32 = 144.0;

impl Layout {
    pub fn new(windows: &Windows) -> Self {
        let (w, h) = (screen_width(), screen_height());
        let slot = 54.0;
        let gap = 6.0;
        let bar_w = ACTION_BAR_SLOTS as f32 * (slot + gap) - gap;
        let bar_x = (w - bar_w) / 2.0;
        let bar_y = h - slot - 26.0;
        let hotbar =
            std::array::from_fn(|i| Rect::new(bar_x + i as f32 * (slot + gap), bar_y, slot, slot));
        let menu_w = 240.0;
        let menu_x = (w - menu_w) / 2.0;

        let bags_rect = Rect::new(
            w - 16.0 - 5.0 * (SLOT + 4.0) - 20.0,
            h - 330.0,
            5.0 * (SLOT + 4.0) + 20.0,
            4.0 * (SLOT + 4.0) + 76.0,
        );
        let bag_slots = if windows.bags {
            (0..BAG_SLOTS)
                .map(|i| {
                    let (col, row) = (i % 5, i / 5);
                    Rect::new(
                        bags_rect.x + 12.0 + col as f32 * (SLOT + 4.0),
                        bags_rect.y + 36.0 + row as f32 * (SLOT + 4.0),
                        SLOT,
                        SLOT,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        let char_rect = Rect::new(16.0, 150.0, 280.0, 330.0);
        let gear_slots = if windows.character {
            (0..5)
                .map(|i| {
                    Rect::new(
                        char_rect.x + 12.0,
                        char_rect.y + 40.0 + i as f32 * (SLOT + 6.0),
                        char_rect.w - 24.0,
                        SLOT,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        let craft_x = if windows.character { 312.0 } else { 16.0 };
        let craft_rect = Rect::new(craft_x, 150.0, 400.0, 46.0 + RECIPES.len() as f32 * 34.0);
        let craft_buttons = if windows.crafting {
            (0..RECIPES.len())
                .map(|i| {
                    Rect::new(
                        craft_rect.x + craft_rect.w - 82.0,
                        craft_rect.y + 38.0 + i as f32 * 34.0,
                        70.0,
                        28.0,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        let vendor_x = 16.0
            + if windows.character { 296.0 } else { 0.0 }
            + if windows.crafting {
                craft_rect.w + 16.0
            } else {
                0.0
            };
        let vendor_rect = Rect::new(
            vendor_x,
            150.0,
            340.0,
            70.0 + MERCHANT_GOODS.len() as f32 * 40.0,
        );
        let vendor_buttons = if windows.vendor.is_some() {
            (0..MERCHANT_GOODS.len())
                .map(|i| {
                    Rect::new(
                        vendor_rect.right() - 82.0,
                        vendor_rect.y + 40.0 + i as f32 * 40.0,
                        70.0,
                        30.0,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        Self {
            player_frame: Rect::new(16.0, 16.0, 250.0, 66.0),
            target_frame: Rect::new(282.0, 16.0, 250.0, 66.0),
            tot_frame: Rect::new(548.0, 30.0, 140.0, 36.0),
            hotbar,
            castbar: Rect::new((w - 320.0) / 2.0, bar_y - 46.0, 320.0, 20.0),
            xpbar: Rect::new(bar_x - 120.0, h - 16.0, bar_w + 240.0, 10.0),
            minimap: (vec2(w - 96.0, 104.0), 80.0),
            release_button: Rect::new((w - 200.0) / 2.0, h * 0.3 + 46.0, 200.0, 40.0),
            menu_buttons: std::array::from_fn(|i| {
                Rect::new(menu_x, h * 0.35 + 50.0 + i as f32 * 52.0, menu_w, 40.0)
            }),
            bags: windows.bags.then_some(bags_rect),
            bag_slots,
            character: windows.character.then_some(char_rect),
            gear_slots,
            crafting: windows.crafting.then_some(craft_rect),
            craft_buttons,
            vendor: windows.vendor.is_some().then_some(vendor_rect),
            vendor_buttons,
            talents: windows
                .talents
                .then(|| crate::panels::talent_layout().window),
            sandbox: windows
                .sandbox
                .then(|| crate::panels::sandbox_layout(Zone::Amberfall).window),
            map: windows.map,
            quest_window: None,
            party: PartyLayout::default(),
        }
    }

    /// Whether the mouse is over an open window.
    pub fn over_window(&self, mouse: Vec2) -> bool {
        self.map
            || [
                self.bags,
                self.character,
                self.crafting,
                self.vendor,
                self.talents,
                self.sandbox,
                self.quest_window,
            ]
            .iter()
            .flatten()
            .any(|r| r.contains(mouse))
    }

    /// Whether the mouse is over something that should eat world clicks.
    pub fn blocks(&self, mouse: Vec2, has_target: bool, dead: bool) -> bool {
        self.party.rects().any(|r| r.contains(mouse))
            || self.hotbar.iter().any(|r| r.contains(mouse))
            || self.player_frame.contains(mouse)
            || (has_target && self.target_frame.contains(mouse))
            || (dead && self.release_button.contains(mouse))
            || self.minimap.0.distance(mouse) < self.minimap.1
            || self.map
            || [
                self.bags,
                self.character,
                self.crafting,
                self.vendor,
                self.talents,
                self.sandbox,
                self.quest_window,
            ]
            .iter()
            .flatten()
            .any(|r| r.contains(mouse))
    }
}

/// Screen position of a world point, or `None` if it's behind the camera.
pub fn project(cam: &Camera3D, p: Vec3) -> Option<Vec2> {
    let clip = cam.matrix() * p.extend(1.0);
    if clip.w <= 0.05 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some(vec2(
        (ndc.x + 1.0) * 0.5 * screen_width(),
        (1.0 - ndc.y) * 0.5 * screen_height(),
    ))
}

pub fn school_color(school: School) -> Color {
    match school {
        School::Physical => Color::new(0.75, 0.62, 0.48, 1.0),
        School::Fire => Color::new(1.0, 0.45, 0.12, 1.0),
        School::Frost => Color::new(0.45, 0.8, 1.0, 1.0),
        School::Arcane => Color::new(0.8, 0.5, 1.0, 1.0),
        School::Holy => Color::new(1.0, 0.92, 0.55, 1.0),
        School::Shadow => Color::new(0.6, 0.3, 0.8, 1.0),
        School::Nature => Color::new(0.45, 0.9, 0.35, 1.0),
    }
}

pub fn quality_color(q: Quality) -> Color {
    match q {
        Quality::Common => Color::new(0.95, 0.95, 0.95, 1.0),
        Quality::Uncommon => Color::new(0.3, 1.0, 0.25, 1.0),
        Quality::Rare => Color::new(0.25, 0.55, 1.0, 1.0),
    }
}

/// Name color: red for enemies that attack on sight, yellow for ones that
/// don't, green for friends, grey for the dead.
pub fn reaction_color(view: &EntityView, my_class: Class) -> Color {
    if view.dead {
        return Color::new(0.6, 0.6, 0.6, 1.0);
    }
    match view.kind {
        EntityKind::Mob { aggressive, .. }
            if view.kind.hostile_to(EntityKind::Player(my_class)) =>
        {
            if aggressive || view.in_combat {
                Color::new(1.0, 0.3, 0.25, 1.0)
            } else {
                Color::new(1.0, 0.9, 0.3, 1.0)
            }
        }
        _ => Color::new(0.35, 0.95, 0.4, 1.0),
    }
}

pub fn class_color(class: Class) -> Color {
    match class {
        Class::Barbarian => Color::new(0.78, 0.61, 0.43, 1.0),
        Class::Fighter => Color::new(0.68, 0.7, 0.78, 1.0),
        Class::Paladin => Color::new(0.96, 0.55, 0.73, 1.0),
        Class::Monk => Color::new(0.0, 1.0, 0.6, 1.0),
        Class::Rogue => Color::new(1.0, 0.96, 0.41, 1.0),
        Class::Ranger => Color::new(0.67, 0.83, 0.45, 1.0),
        Class::Artificer => Color::new(0.85, 0.65, 0.3, 1.0),
        Class::Bard => Color::new(0.95, 0.45, 0.85, 1.0),
        Class::Cleric => Color::new(0.95, 0.95, 0.95, 1.0),
        Class::Druid => Color::new(1.0, 0.49, 0.04, 1.0),
        Class::Mage => Color::new(0.41, 0.8, 0.94, 1.0),
        Class::Sorcerer => Color::new(0.95, 0.3, 0.3, 1.0),
        Class::Warlock => Color::new(0.58, 0.51, 0.79, 1.0),
    }
}

/// Text with a dark drop shadow, so it reads on any background.
pub fn text(s: &str, x: f32, y: f32, size: f32, color: Color) {
    let shadow = Color::new(0.0, 0.0, 0.0, color.a * 0.85);
    draw_text(s, x + 1.0, y + 1.0, size, shadow);
    draw_text(s, x, y, size, color);
}

pub fn text_width(s: &str, size: f32) -> f32 {
    measure_text(s, None, size as u16, 1.0).width
}

pub fn text_centered(s: &str, cx: f32, y: f32, size: f32, color: Color) {
    text(s, cx - text_width(s, size) / 2.0, y, size, color);
}

pub fn panel(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, BORDER);
    draw_rectangle_lines(
        r.x + 3.0,
        r.y + 3.0,
        r.w - 6.0,
        r.h - 6.0,
        1.0,
        Color::new(BORDER.r, BORDER.g, BORDER.b, 0.35),
    );
}

fn window(r: Rect, title: &str) {
    panel(r);
    draw_rectangle(
        r.x + 2.0,
        r.y + 2.0,
        r.w - 4.0,
        26.0,
        Color::new(0.25, 0.17, 0.08, 0.9),
    );
    text(title, r.x + 10.0, r.y + 21.0, 20.0, GOLD);
}

pub fn bar(r: Rect, frac: f32, fill: Color, label: &str) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(r.x, r.y, r.w * frac.clamp(0.0, 1.0), r.h, fill);
    // A little shine along the top.
    draw_rectangle(
        r.x,
        r.y,
        r.w * frac.clamp(0.0, 1.0),
        r.h * 0.35,
        Color::new(1.0, 1.0, 1.0, 0.12),
    );
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, Color::new(0.0, 0.0, 0.0, 0.8));
    if !label.is_empty() {
        let size = (r.h + 2.0).min(18.0);
        text_centered(
            label,
            r.x + r.w / 2.0,
            r.y + r.h * 0.5 + size * 0.3,
            size,
            WHITE,
        );
    }
}

pub fn button(r: Rect, label: &str) {
    button_ex(r, label, true);
}

pub fn button_ex(r: Rect, label: &str, enabled: bool) {
    let hover = enabled && r.contains(vec2(mouse_position().0, mouse_position().1));
    let bg = if !enabled {
        Color::new(0.15, 0.13, 0.1, 0.9)
    } else if hover {
        Color::new(0.38, 0.27, 0.11, 0.95)
    } else {
        Color::new(0.24, 0.17, 0.08, 0.95)
    };
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if hover { GOLD } else { BORDER });
    let size = if r.h < 34.0 { 18.0 } else { 22.0 };
    let color = if enabled {
        WHITE
    } else {
        Color::new(0.5, 0.5, 0.5, 1.0)
    };
    text_centered(
        label,
        r.x + r.w / 2.0,
        r.y + r.h / 2.0 + size * 0.32,
        size,
        color,
    );
}

fn power_color(view: &EntityView) -> Color {
    match view.kind {
        EntityKind::Player(c) => class_power_color(c),
        _ => Color::new(0.2, 0.4, 0.95, 1.0),
    }
}

fn class_power_color(class: Class) -> Color {
    match class.power_kind() {
        PowerKind::Rage => Color::new(0.8, 0.15, 0.15, 1.0),
        PowerKind::Energy => Color::new(0.95, 0.85, 0.2, 1.0),
        PowerKind::Mana => Color::new(0.2, 0.4, 0.95, 1.0),
    }
}

/// The other party members: click one to target them. Members too far away
/// to share kills are dimmed.
fn party_frames(game: &Game, layout: &Layout) {
    let Some(party) = &game.me.party else { return };
    for (r, id) in &layout.party.frames {
        let Some(m) = party.members.iter().find(|m| m.id == *id) else {
            continue;
        };
        panel(*r);
        if game.target == Some(m.id) {
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, GOLD);
        }
        let alpha = if m.near { 1.0 } else { 0.45 };
        let mut name_x = r.x + 7.0;
        if m.id == party.leader {
            draw_poly(r.x + 11.0, r.y + 11.0, 4, 5.0, 45.0, GOLD);
            name_x += 11.0;
        }
        let mut color = class_color(m.class);
        color.a = alpha;
        text(&m.name, name_x, r.y + 16.0, 17.0, color);
        let lvl = m.level.to_string();
        let lvl_right = if layout.party.kicks.is_empty() {
            r.right() - 7.0
        } else {
            r.right() - 26.0
        };
        text(
            &lvl,
            lvl_right - text_width(&lvl, 16.0),
            r.y + 16.0,
            16.0,
            Color::new(1.0, 1.0, 1.0, alpha),
        );
        let frac = if m.dead {
            0.0
        } else {
            m.hp / m.max_hp.max(1.0)
        };
        let mut hp = hp_color(frac);
        hp.a = alpha;
        let label = if m.dead { "Dead" } else { "" };
        bar(
            Rect::new(r.x + 7.0, r.y + 21.0, r.w - 14.0, 12.0),
            frac,
            hp,
            label,
        );
        if m.max_power > 0.0 {
            let mut c = class_power_color(m.class);
            c.a = alpha;
            bar(
                Rect::new(r.x + 7.0, r.y + 35.0, r.w - 14.0, 5.0),
                m.power / m.max_power,
                c,
                "",
            );
        }
    }
    let mouse = vec2(mouse_position().0, mouse_position().1);
    for (r, name) in &layout.party.kicks {
        let hover = r.contains(mouse);
        let c = if hover {
            Color::new(1.0, 0.35, 0.3, 1.0)
        } else {
            Color::new(0.7, 0.6, 0.55, 1.0)
        };
        text_centered("x", r.x + r.w / 2.0, r.y + 11.0, 18.0, c);
        if hover {
            tooltip_box(
                &[(format!("Remove {name} from the party"), WHITE)],
                *r,
                false,
            );
        }
    }
    if let Some(r) = layout.party.leave {
        button(r, "Leave party");
    }
}

fn hp_color(frac: f32) -> Color {
    if frac > 0.5 {
        Color::new(0.15, 0.75, 0.2, 1.0)
    } else if frac > 0.2 {
        Color::new(0.85, 0.7, 0.1, 1.0)
    } else {
        Color::new(0.85, 0.2, 0.15, 1.0)
    }
}

fn level_label(view: &EntityView) -> String {
    match view.kind {
        EntityKind::Mob { elite: true, .. } => format!("{} Elite", view.level),
        _ => view.level.to_string(),
    }
}

/// How hard a mob is compared to you, as a color for its level.
fn level_color(view: &EntityView, my_level: u8) -> Color {
    if view.kind.is_player() {
        return WHITE;
    }
    let diff = view.level as i32 - my_level as i32;
    match diff {
        d if d >= 5 => Color::new(1.0, 0.1, 0.1, 1.0),
        3..=4 => Color::new(1.0, 0.5, 0.1, 1.0),
        -2..=2 => Color::new(1.0, 1.0, 0.1, 1.0),
        d if d <= -5 => Color::new(0.6, 0.6, 0.6, 1.0),
        _ => Color::new(0.25, 0.85, 0.25, 1.0),
    }
}

fn unit_frame(r: Rect, view: &EntityView, game: &Game) {
    panel(r);
    let name_color = match view.kind {
        EntityKind::Player(c) => class_color(c),
        _ => reaction_color(view, game.class),
    };
    text(&view.name, r.x + 8.0, r.y + 19.0, 20.0, name_color);
    let lvl = level_label(view);
    text(
        &lvl,
        r.x + r.w - 8.0 - text_width(&lvl, 18.0),
        r.y + 18.0,
        18.0,
        level_color(view, game.level()),
    );
    if view.in_combat {
        draw_circle(
            r.x + r.w - 14.0 - text_width(&lvl, 18.0),
            r.y + 12.0,
            4.0,
            Color::new(1.0, 0.25, 0.2, 1.0),
        );
    }
    let hp_frac = view.hp / view.max_hp.max(1.0);
    let hp_label = if view.dead {
        "Dead".to_string()
    } else {
        format!("{} / {}", view.hp.ceil(), view.max_hp.ceil())
    };
    bar(
        Rect::new(r.x + 8.0, r.y + 26.0, r.w - 16.0, 16.0),
        hp_frac,
        hp_color(hp_frac),
        &hp_label,
    );
    if view.max_power > 0.0 {
        let label = format!("{} / {}", view.power.floor(), view.max_power.floor());
        bar(
            Rect::new(r.x + 8.0, r.y + 46.0, r.w - 16.0, 12.0),
            view.power / view.max_power,
            power_color(view),
            &label,
        );
    }
}

fn aura_row(x: f32, y: f32, view: &EntityView) {
    for (i, aura) in view.auras.iter().enumerate() {
        let a = ability(aura.ability);
        let r = Rect::new(x + i as f32 * 30.0, y, 26.0, 26.0);
        draw_rectangle(r.x, r.y, r.w, r.h, school_color(a.school));
        let border = if aura.harmful {
            Color::new(0.9, 0.1, 0.1, 1.0)
        } else {
            Color::new(0.1, 0.8, 0.2, 1.0)
        };
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, border);
        let frac = aura.remaining / aura.duration.max(0.01);
        draw_rectangle(
            r.x,
            r.y,
            r.w,
            r.h * (1.0 - frac),
            Color::new(0.0, 0.0, 0.0, 0.45),
        );
        text_centered(&abbreviation(a.name), r.x + 13.0, r.y + 13.0, 13.0, WHITE);
        text_centered(
            &format!("{:.0}", aura.remaining.ceil()),
            r.x + 13.0,
            r.y + 24.0,
            13.0,
            WHITE,
        );
        if aura.stacks > 1 {
            text(
                &aura.stacks.to_string(),
                r.x + r.w - 7.0,
                r.y + r.h + 4.0,
                16.0,
                GOLD,
            );
        }
    }
}

/// "Heroic Strike" -> "HS", "Fireball" -> "Fireb".
pub fn abbreviation(name: &str) -> String {
    let words: Vec<&str> = name.split([' ', ':']).filter(|w| !w.is_empty()).collect();
    if words.len() > 1 {
        words.iter().filter_map(|w| w.chars().next()).collect()
    } else {
        name.chars().take(5).collect()
    }
}

fn cast_bar(r: Rect, cast: &CastView) {
    let a = ability(cast.ability);
    panel(Rect::new(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0));
    bar(
        r,
        cast.elapsed / cast.total,
        Color::new(0.95, 0.7, 0.15, 1.0),
        "",
    );
    text(a.name, r.x + 6.0, r.y + r.h * 0.5 + 5.0, 16.0, WHITE);
    let left = format!("{:.1}", (cast.total - cast.elapsed).max(0.0));
    text(
        &left,
        r.x + r.w - 6.0 - text_width(&left, 16.0),
        r.y + r.h * 0.5 + 5.0,
        16.0,
        WHITE,
    );
}

pub fn draw(game: &Game, layout: &Layout, cam: &Camera3D) {
    let Some(me) = game.my_view() else {
        text_centered(
            "Entering the world...",
            screen_width() / 2.0,
            screen_height() / 2.0,
            32.0,
            WHITE,
        );
        return;
    };
    nameplates(game, cam);
    floating_text(game, cam);

    // Unit frames.
    unit_frame(layout.player_frame, me, game);
    let mut below = layout.player_frame.bottom() + 6.0;
    if game.class.uses_combo_points() {
        for i in 0..MAX_COMBO_POINTS {
            let lit = i < game.me.combo_points;
            let c = vec2(layout.player_frame.x + 16.0 + i as f32 * 20.0, below + 8.0);
            draw_circle(c.x, c.y, 7.0, Color::new(0.0, 0.0, 0.0, 0.7));
            draw_circle(
                c.x,
                c.y,
                5.0,
                if lit {
                    Color::new(1.0, 0.3, 0.15, 1.0)
                } else {
                    Color::new(0.25, 0.2, 0.2, 1.0)
                },
            );
        }
        below += 20.0;
    }
    aura_row(layout.player_frame.x, below, me);
    party_frames(game, layout);
    if let Some(t) = game.target_ent() {
        unit_frame(layout.target_frame, &t.view, game);
        aura_row(
            layout.target_frame.x,
            layout.target_frame.bottom() + 6.0,
            &t.view,
        );
        if let Some(cast) = &t.view.cast {
            let r = Rect::new(
                layout.target_frame.x + 8.0,
                layout.target_frame.bottom() + 40.0,
                layout.target_frame.w - 16.0,
                14.0,
            );
            cast_bar(r, cast);
        }
        if let Some(tot) = t.view.target.and_then(|id| game.entities.get(&id)) {
            let r = layout.tot_frame;
            panel(r);
            let color = match tot.view.kind {
                EntityKind::Player(c) => class_color(c),
                _ => reaction_color(&tot.view, game.class),
            };
            text(&tot.view.name, r.x + 6.0, r.y + 15.0, 15.0, color);
            let frac = tot.view.hp / tot.view.max_hp.max(1.0);
            bar(
                Rect::new(r.x + 6.0, r.y + 21.0, r.w - 12.0, 9.0),
                frac,
                hp_color(frac),
                "",
            );
        }
        if game.me.auto_attacking {
            let y = layout.target_frame.bottom() + if t.view.cast.is_some() { 74.0 } else { 52.0 };
            text(
                "Attacking",
                layout.target_frame.x + 8.0,
                y,
                16.0,
                Color::new(1.0, 0.5, 0.3, 1.0),
            );
        }
        if let Some((r, _)) = &layout.party.invite_target {
            button(*r, "Invite");
        }
        if t.view.lootable {
            text(
                "Right-click to loot",
                layout.target_frame.x + 120.0,
                layout.target_frame.bottom() + 52.0,
                16.0,
                GOLD,
            );
        }
    }

    action_bar(game, layout, me);

    // Our cast bar.
    if let Some(cast) = &me.cast {
        cast_bar(layout.castbar, cast);
    } else if let Some((msg, t)) = &game.cast_flash {
        let r = layout.castbar;
        let alpha = 1.0 - t;
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.7, 0.1, 0.1, alpha * 0.9));
        text_centered(
            msg,
            r.x + r.w / 2.0,
            r.y + 15.0,
            18.0,
            Color::new(1.0, 1.0, 1.0, alpha),
        );
    }

    // Experience.
    let xp = layout.xpbar;
    let label = if game.me.xp_next == 0 {
        format!("Level {} (max)", me.level)
    } else {
        format!(
            "Level {}  -  {} / {} XP",
            me.level, game.me.xp, game.me.xp_next
        )
    };
    let frac = if game.me.xp_next == 0 {
        1.0
    } else {
        game.me.xp as f32 / game.me.xp_next as f32
    };
    bar(xp, frac, Color::new(0.55, 0.3, 0.85, 1.0), "");
    text_centered(
        &label,
        xp.x + xp.w / 2.0,
        xp.y - 3.0,
        15.0,
        Color::new(0.9, 0.85, 1.0, 1.0),
    );

    minimap(game, layout);
    chat(game);
    if let Some(r) = layout.bags {
        bags(game, layout, r);
    }
    if let Some(r) = layout.character {
        character(game, layout, r, me);
    }
    if let Some(r) = layout.crafting {
        crafting(game, layout, r);
    }
    if let Some(r) = layout.vendor {
        vendor(game, layout, r);
    }
    if layout.sandbox.is_some() {
        crate::panels::draw_sandbox(game, me.level);
    }
    if layout.talents.is_some() {
        crate::panels::draw_talents(game, me.level);
    }

    // Errors and banners.
    for (i, (msg, t)) in game.errors.iter().enumerate() {
        let alpha = (2.5 - t).min(1.0);
        text_centered(
            msg,
            screen_width() / 2.0,
            screen_height() * 0.16 + i as f32 * 24.0,
            22.0,
            Color::new(1.0, 0.2, 0.15, alpha),
        );
    }
    if let Some((title, sub, t)) = &game.banner {
        let alpha = (5.0 - t).min(1.0).min(t * 2.0);
        let y = screen_height() * 0.27;
        text_centered(
            title,
            screen_width() / 2.0,
            y,
            58.0,
            Color::new(1.0, 0.85, 0.45, alpha),
        );
        if !sub.is_empty() {
            text_centered(
                sub,
                screen_width() / 2.0,
                y + 34.0,
                24.0,
                Color::new(0.95, 0.9, 0.8, alpha),
            );
        }
    }

    if me.dead {
        let r = Rect::new(
            (screen_width() - 300.0) / 2.0,
            screen_height() * 0.3,
            300.0,
            100.0,
        );
        panel(r);
        text_centered("You have died.", r.x + r.w / 2.0, r.y + 30.0, 26.0, WHITE);
        button(layout.release_button, "Release Spirit");
    }
    if game.show_help {
        help();
    }
    let level = me.level;
    let tracker_top = if game.show_help { 700.0 } else { 240.0 };
    crate::quests_ui::draw_tracker(&game.me.quests, level, &game.me.bags, tracker_top);
    if let Some((msg, t)) = &game.quest_flash {
        let alpha = (3.0 - t).min(1.0);
        text_centered(
            msg,
            screen_width() / 2.0,
            screen_height() * 0.22,
            24.0,
            Color::new(1.0, 0.85, 0.2, alpha),
        );
    }
    if game.windows.quest_log {
        crate::quests_ui::draw_log(&game.me.quests, level, &game.me.bags);
    }
    if let Some(giver) = game
        .windows
        .quest_giver
        .and_then(|id| game.entities.get(&id))
        && let Some((icon, id)) = crate::quests_ui::draw_giver(
            &giver.view.name,
            game.zone,
            &game.me.quests,
            level,
            &game.me.bags,
        )
    {
        tooltip_box(&item_lines(id), icon, true);
    }
    if let Some((r, accept, decline)) = layout.party.popup
        && let Some(from) = &game.me.invite
    {
        panel(r);
        text_centered(
            &format!("{from} invites you to a party."),
            r.x + r.w / 2.0,
            r.y + 32.0,
            20.0,
            WHITE,
        );
        button(accept, "Accept");
        button(decline, "Decline");
    }
    if game.menu_open {
        let w = 280.0;
        let r = Rect::new((screen_width() - w) / 2.0, screen_height() * 0.35, w, 220.0);
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            Color::new(0.0, 0.0, 0.0, 0.35),
        );
        panel(r);
        text_centered("Game Menu", r.x + r.w / 2.0, r.y + 32.0, 26.0, GOLD);
        for (b, label) in layout
            .menu_buttons
            .iter()
            .zip(["Return to Game", "Log Out", "Quit Game"])
        {
            button(*b, label);
        }
    }
    item_tooltips(game, layout);
    if layout.map
        && let Some((_, tex)) = &game.map_texture
    {
        crate::panels::draw_map(game, tex);
    }
}

fn action_bar(game: &Game, layout: &Layout, me: &EntityView) {
    let mouse = vec2(mouse_position().0, mouse_position().1);
    let target = game.target_ent();
    let mut tooltip = None;
    for (i, (r, id)) in layout.hotbar.iter().zip(game.class.abilities()).enumerate() {
        let a = ability(id);
        let c = school_color(a.school);
        let locked = UNLOCK_LEVELS[i] > me.level;
        draw_rectangle(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0, PANEL);
        if locked {
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.12, 0.12, 0.13, 1.0));
            // A little padlock.
            let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0 - 4.0);
            draw_circle_lines(cx, cy - 4.0, 7.0, 3.0, Color::new(0.55, 0.55, 0.58, 1.0));
            draw_rectangle(
                cx - 10.0,
                cy - 2.0,
                20.0,
                14.0,
                Color::new(0.55, 0.55, 0.58, 1.0),
            );
            text_centered(
                &format!("Lv {}", UNLOCK_LEVELS[i]),
                cx,
                r.y + r.h - 4.0,
                15.0,
                Color::new(0.8, 0.8, 0.8, 1.0),
            );
        } else {
            draw_rectangle(
                r.x,
                r.y,
                r.w,
                r.h,
                Color::new(c.r * 0.45, c.g * 0.45, c.b * 0.45, 1.0),
            );
            draw_rectangle(
                r.x + 4.0,
                r.y + 4.0,
                r.w - 8.0,
                r.h - 8.0,
                Color::new(c.r * 0.7, c.g * 0.7, c.b * 0.7, 1.0),
            );
            let out_of_range = a.targeting.needs_enemy()
                && target.is_some_and(|t| {
                    game.is_hostile(&t.view)
                        && t.pos.distance(game.pos)
                            > a.range + 1.0 + render::model_radius(t.view.kind) * 0.5
                });
            let label_color = if out_of_range {
                Color::new(1.0, 0.25, 0.2, 1.0)
            } else {
                WHITE
            };
            text_centered(
                &abbreviation(a.name),
                r.x + r.w / 2.0,
                r.y + r.h / 2.0 + 7.0,
                20.0,
                label_color,
            );
            let unusable = (a.cost > 0.0 && me.power < a.cost)
                || (a.needs_combo_points() && game.me.combo_points == 0);
            if unusable {
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.1, 0.2, 0.8, 0.45));
            }
            // Cooldown, or the global cooldown, as a shrinking shade.
            let cd = game
                .me
                .cooldowns
                .iter()
                .find(|(a, _, _)| *a == id)
                .map(|(_, rem, total)| (*rem, *total));
            let (rem, total) = match cd {
                Some((rem, total)) if rem > game.me.gcd => (rem, total),
                _ => (game.me.gcd, GCD),
            };
            if rem > 0.0 {
                let frac = (rem / total).clamp(0.0, 1.0);
                draw_rectangle(
                    r.x,
                    r.y + r.h * (1.0 - frac),
                    r.w,
                    r.h * frac,
                    Color::new(0.0, 0.0, 0.0, 0.6),
                );
                if rem > 1.6 {
                    text_centered(
                        &format!("{:.0}", rem.ceil()),
                        r.x + r.w / 2.0,
                        r.y + r.h - 6.0,
                        18.0,
                        GOLD,
                    );
                }
            }
        }
        let key = if i == E_SLOT {
            "E".to_string()
        } else {
            (i + 1).to_string()
        };
        text(
            &key,
            r.x + 4.0,
            r.y + 14.0,
            15.0,
            Color::new(0.9, 0.9, 0.9, 1.0),
        );
        let hover = r.contains(mouse);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if hover { GOLD } else { BORDER });
        if hover {
            tooltip = Some((a, *r, UNLOCK_LEVELS[i], locked));
        }
    }
    if let Some((a, r, level, locked)) = tooltip {
        ability_tooltip(a, r, game.class, locked.then_some(level));
    }
}

pub fn tooltip_box(lines: &[(String, Color)], anchor: Rect, above: bool) {
    let w = 300.0;
    let h = 12.0 + lines.len() as f32 * 20.0;
    let x = (anchor.x + anchor.w / 2.0 - w / 2.0).clamp(4.0, screen_width() - w - 4.0);
    let y = if above {
        anchor.y - h - 10.0
    } else {
        anchor.bottom() + 8.0
    };
    let y = y.clamp(4.0, screen_height() - h - 4.0);
    panel(Rect::new(x, y, w, h));
    for (i, (line, color)) in lines.iter().enumerate() {
        text(
            line,
            x + 10.0,
            y + 24.0 + i as f32 * 20.0,
            if i == 0 { 20.0 } else { 17.0 },
            *color,
        );
    }
}

fn ability_tooltip(a: &Ability, slot: Rect, class: Class, locked_until: Option<u8>) {
    let mut lines: Vec<(String, Color)> = vec![(a.name.to_string(), WHITE)];
    let mut stats = Vec::new();
    if a.cost > 0.0 {
        stats.push(format!("{} {}", a.cost, class.power_kind().name()));
    }
    match a.targeting {
        Targeting::Enemy | Targeting::Friendly => stats.push(format!("{} yd range", a.range)),
        Targeting::AroundTarget(r) => stats.push(format!("{} yd range, {r} yd radius", a.range)),
        Targeting::AroundCaster(r) => stats.push(format!("{r} yd radius")),
        Targeting::Caster => {}
    }
    lines.push((stats.join("   "), Color::new(0.8, 0.8, 0.8, 1.0)));
    let mut timing = vec![if a.cast_time > 0.0 {
        format!("{} sec cast", a.cast_time)
    } else {
        "Instant".into()
    }];
    if a.cooldown > 0.0 {
        timing.push(format!("{} sec cooldown", a.cooldown));
    }
    lines.push((timing.join("   "), Color::new(0.8, 0.8, 0.8, 1.0)));
    for line in wrap(a.description, 34) {
        lines.push((line, GOLD));
    }
    if let Some(level) = locked_until {
        lines.push((
            format!("Learned at level {level}"),
            Color::new(1.0, 0.35, 0.3, 1.0),
        ));
    }
    tooltip_box(&lines, slot, true);
}

pub fn item_lines(id: ItemId) -> Vec<(String, Color)> {
    let it = item(id);
    let grey = Color::new(0.8, 0.8, 0.8, 1.0);
    let green = Color::new(0.3, 1.0, 0.25, 1.0);
    let mut lines = vec![(it.name.to_string(), quality_color(it.quality))];
    match it.kind {
        ItemKind::Material => lines.push(("Crafting material".into(), grey)),
        ItemKind::Potion { health, power } => {
            let mut what = Vec::new();
            if health > 0.0 {
                what.push(format!("{health:.0} health"));
            }
            if power > 0.0 {
                what.push(format!("{power:.0} mana or energy"));
            }
            lines.push((format!("Use: restores {}", what.join(" and ")), green));
        }
        ItemKind::Armor {
            slot,
            armor,
            stamina,
            power,
        } => {
            lines.push((slot.name().into(), grey));
            lines.push((format!("{armor} Armor"), WHITE));
            if stamina > 0.0 {
                lines.push((format!("+{stamina} Stamina"), green));
            }
            if power > 0.0 {
                lines.push((format!("+{power} Power"), green));
            }
        }
    }
    for line in wrap(it.description, 34) {
        lines.push((line, GOLD));
    }
    lines
}

fn item_tooltips(game: &Game, layout: &Layout) {
    let mouse = vec2(mouse_position().0, mouse_position().1);
    for (i, r) in layout.bag_slots.iter().enumerate() {
        if r.contains(mouse)
            && let Some(Some((id, _))) = game.me.bags.get(i)
        {
            let mut lines = item_lines(*id);
            let hint = Color::new(0.6, 0.8, 1.0, 1.0);
            if game.windows.vendor.is_some() {
                let n = game.me.bags[i].map_or(1, |(_, n)| n) as u32;
                lines.push((
                    format!(
                        "Right-click to sell for {}",
                        format_money(item(*id).sell_price() * n)
                    ),
                    hint,
                ));
            } else if matches!(item(*id).kind, ItemKind::Armor { .. }) {
                lines.push(("Click to wear".into(), hint));
            } else if matches!(item(*id).kind, ItemKind::Potion { .. }) {
                lines.push(("Right-click to drink".into(), hint));
            }
            tooltip_box(&lines, *r, true);
        }
    }
    for (r, id) in layout.vendor_buttons.iter().zip(MERCHANT_GOODS) {
        let row = Rect::new(r.x - 250.0, r.y, r.w + 250.0, r.h);
        if row.contains(mouse) {
            tooltip_box(&item_lines(id), *r, false);
        }
    }
    let Some(me) = game.my_view() else { return };
    for (i, r) in layout.gear_slots.iter().enumerate() {
        if r.contains(mouse)
            && let Some(id) = me.gear[i]
        {
            let mut lines = item_lines(id);
            lines.push(("Click to take off".into(), Color::new(0.6, 0.8, 1.0, 1.0)));
            tooltip_box(&lines, *r, false);
        }
    }
}

pub fn item_icon(r: Rect, id: ItemId, count: u16) {
    let it = item(id);
    let c = Color::new(it.color.0, it.color.1, it.color.2, 1.0);
    draw_rectangle(r.x + 3.0, r.y + 3.0, r.w - 6.0, r.h - 6.0, c);
    draw_rectangle(
        r.x + 3.0,
        r.y + 3.0,
        r.w - 6.0,
        (r.h - 6.0) * 0.35,
        Color::new(1.0, 1.0, 1.0, 0.18),
    );
    let q = quality_color(it.quality);
    draw_rectangle_lines(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, 2.0, q);
    let short: String = it
        .name
        .split(' ')
        .filter_map(|w| w.chars().next())
        .collect();
    text_centered(
        &short,
        r.x + r.w / 2.0,
        r.y + r.h / 2.0 + 6.0,
        18.0,
        Color::new(0.1, 0.08, 0.06, 0.9),
    );
    if count > 1 {
        let s = count.to_string();
        text(
            &s,
            r.x + r.w - 4.0 - text_width(&s, 16.0),
            r.y + r.h - 4.0,
            16.0,
            WHITE,
        );
    }
}

fn bags(game: &Game, layout: &Layout, r: Rect) {
    window(r, "Backpack (B)");
    for (i, slot) in layout.bag_slots.iter().enumerate() {
        draw_rectangle(
            slot.x,
            slot.y,
            slot.w,
            slot.h,
            Color::new(0.0, 0.0, 0.0, 0.5),
        );
        draw_rectangle_lines(
            slot.x,
            slot.y,
            slot.w,
            slot.h,
            1.0,
            Color::new(0.4, 0.33, 0.22, 1.0),
        );
        if let Some(Some((id, n))) = game.me.bags.get(i) {
            item_icon(*slot, *id, *n);
        }
    }
    let money = format_money(game.me.money);
    text(
        &money,
        r.x + r.w - 12.0 - text_width(&money, 20.0),
        r.bottom() - 12.0,
        20.0,
        GOLD,
    );
}

fn character(game: &Game, layout: &Layout, r: Rect, me: &EntityView) {
    window(
        r,
        &format!("{} - Level {} {} (C)", me.name, me.level, game.class.name()),
    );
    for (i, slot) in layout.gear_slots.iter().enumerate() {
        draw_rectangle(
            slot.x,
            slot.y,
            slot.w,
            slot.h,
            Color::new(0.0, 0.0, 0.0, 0.45),
        );
        let icon = Rect::new(slot.x, slot.y, SLOT, SLOT);
        draw_rectangle_lines(
            icon.x,
            icon.y,
            icon.w,
            icon.h,
            1.0,
            Color::new(0.4, 0.33, 0.22, 1.0),
        );
        let name = Slot::ALL[i].name();
        match me.gear[i] {
            Some(id) => {
                item_icon(icon, id, 1);
                text(
                    item(id).name,
                    slot.x + SLOT + 8.0,
                    slot.y + 20.0,
                    18.0,
                    quality_color(item(id).quality),
                );
                text(
                    name,
                    slot.x + SLOT + 8.0,
                    slot.y + 38.0,
                    15.0,
                    Color::new(0.65, 0.65, 0.65, 1.0),
                );
            }
            None => {
                text(
                    name,
                    slot.x + SLOT + 8.0,
                    slot.y + 28.0,
                    18.0,
                    Color::new(0.5, 0.5, 0.5, 1.0),
                );
            }
        }
    }
    let s = game.me.stats;
    let y = r.y + 40.0 + 5.0 * (SLOT + 6.0) + 8.0;
    text(
        &format!("Health {}    Armor {}", me.max_hp, s.armor),
        r.x + 14.0,
        y,
        17.0,
        WHITE,
    );
    text(
        &format!("Stamina +{}    Power +{}%", s.stamina, s.power),
        r.x + 14.0,
        y + 20.0,
        17.0,
        WHITE,
    );
}

fn crafting(game: &Game, layout: &Layout, r: Rect) {
    window(r, "Crafting (K)");
    for (i, (recipe, b)) in RECIPES.iter().zip(&layout.craft_buttons).enumerate() {
        let y = r.y + 38.0 + i as f32 * 34.0;
        let result = item(recipe.result);
        text(
            result.name,
            r.x + 12.0,
            y + 14.0,
            17.0,
            quality_color(result.quality),
        );
        let mut have_all = true;
        let mats: Vec<String> = recipe
            .materials
            .iter()
            .map(|(m, n)| {
                let have: u32 = game
                    .me
                    .bags
                    .iter()
                    .flatten()
                    .filter(|(id, _)| id == m)
                    .map(|(_, c)| *c as u32)
                    .sum();
                have_all &= have >= *n as u32;
                format!("{} {} ({have})", n, item(*m).name)
            })
            .collect();
        let color = if have_all {
            Color::new(0.75, 0.75, 0.75, 1.0)
        } else {
            Color::new(0.9, 0.4, 0.35, 1.0)
        };
        text(&mats.join(", "), r.x + 12.0, y + 29.0, 14.0, color);
        button_ex(*b, "Craft", have_all);
        let _ = i;
    }
}

fn vendor(game: &Game, layout: &Layout, r: Rect) {
    let name = game
        .windows
        .vendor
        .and_then(|id| game.entities.get(&id))
        .map_or("Merchant".to_string(), |m| m.view.name.clone());
    window(r, &name);
    for (b, id) in layout.vendor_buttons.iter().zip(MERCHANT_GOODS) {
        let it = item(id);
        let icon = Rect::new(r.x + 12.0, b.y - 3.0, 36.0, 36.0);
        item_icon(icon, id, 1);
        text(
            it.name,
            icon.right() + 8.0,
            b.y + 13.0,
            17.0,
            quality_color(it.quality),
        );
        let affordable = game.me.money >= it.price;
        let price_color = if affordable {
            GOLD
        } else {
            Color::new(0.9, 0.4, 0.35, 1.0)
        };
        text(
            &format_money(it.price),
            icon.right() + 8.0,
            b.y + 30.0,
            15.0,
            price_color,
        );
        button_ex(*b, "Buy", affordable);
    }
    text(
        "Right-click items in your bags to sell them.",
        r.x + 12.0,
        r.bottom() - 12.0,
        15.0,
        Color::new(0.75, 0.75, 0.75, 1.0),
    );
}

pub fn wrap(s: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in s.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + word.len() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn nameplates(game: &Game, cam: &Camera3D) {
    let mut plates: Vec<_> = game
        .entities
        .values()
        .filter(|e| Some(e.view.id) != game.my_id && e.pos.distance(game.pos) < 60.0)
        .filter_map(|e| {
            let h = if e.view.dead {
                0.9
            } else {
                render::model_height(e.view.kind, e.view.appearance)
            };
            let head = e.pos + Vec3::Y * (h + 0.45);
            project(cam, head).map(|p| (e.pos.distance(cam.position), p, e))
        })
        .collect();
    // Far ones first so near ones draw on top.
    plates.sort_by(|a, b| b.0.total_cmp(&a.0));
    let my_level = game.level();
    for (dist, p, e) in plates {
        let v = &e.view;
        let targeted = game.target == Some(v.id);
        let size = if targeted {
            19.0
        } else {
            (17.0 - dist * 0.08).max(12.0)
        };
        let color = match v.kind {
            EntityKind::Player(c) if !v.dead => class_color(c),
            _ => reaction_color(v, game.class),
        };
        let label = if v.kind.is_player() {
            v.name.clone()
        } else {
            format!("{} ({})", v.name, level_label(v))
        };
        if v.kind.is_npc() {
            let title = if matches!(v.kind, EntityKind::QuestGiver(_)) {
                "<Quests>"
            } else {
                "<Merchant>"
            };
            text_centered(&v.name, p.x, p.y - 8.0, size, color);
            text_centered(title, p.x, p.y + 8.0, size * 0.85, color);
            if matches!(v.kind, EntityKind::QuestGiver(_))
                && let Some(m) = crate::quests_ui::marker(
                    &game.me.quests,
                    Zone::at(e.pos),
                    my_level,
                    &game.me.bags,
                )
            {
                crate::quests_ui::draw_marker(m, vec2(p.x, p.y - 30.0), game.time);
            }
            continue;
        }
        text_centered(&label, p.x, p.y - 8.0, size, color);
        if !v.kind.is_player() && !v.dead && (targeted || v.hp < v.max_hp || v.in_combat) {
            let w = 70.0;
            let frac = v.hp / v.max_hp.max(1.0);
            bar(
                Rect::new(p.x - w / 2.0, p.y - 2.0, w, 6.0),
                frac,
                hp_color(frac),
                "",
            );
            if let Some(cast) = &v.cast {
                bar(
                    Rect::new(p.x - w / 2.0, p.y + 6.0, w, 4.0),
                    cast.elapsed / cast.total,
                    Color::new(0.95, 0.7, 0.15, 1.0),
                    "",
                );
            }
        }
        if targeted && !v.kind.is_player() {
            let lvl = level_color(v, my_level);
            draw_triangle(
                vec2(p.x - 7.0, p.y - 34.0),
                vec2(p.x + 7.0, p.y - 34.0),
                vec2(p.x, p.y - 25.0),
                lvl,
            );
        }
    }
}

fn floating_text(game: &Game, cam: &Camera3D) {
    for f in &game.floats {
        let height = game.entities.get(&f.entity).map_or(2.0, |e| {
            render::model_height(e.view.kind, e.view.appearance)
        });
        let p = f.anchor + Vec3::Y * (height + 0.5 + f.age * 1.1);
        let Some(s) = project(cam, p) else { continue };
        let pop = if f.age < 0.12 {
            1.0 + (0.12 - f.age) * 4.0
        } else {
            1.0
        };
        let size = if f.big { 34.0 } else { 24.0 } * pop;
        let mut color = f.color;
        color.a = (1.4 - f.age).min(1.0);
        text_centered(&f.text, s.x, s.y, size, color);
    }
}

fn minimap(game: &Game, layout: &Layout) {
    let (c, radius) = layout.minimap;
    let range = 90.0;
    let zone = Zone::at(game.pos);
    let place = if zone.in_town(game.pos) {
        zone.town_name()
    } else {
        zone.name()
    };
    text_centered(place, c.x, c.y - radius - 8.0, 18.0, GOLD);
    let t = render::theme(zone);
    let ground = render::mix(t.fog, Color::new(0.1, 0.1, 0.1, 1.0), 0.45);
    // A gold-trimmed frame, like the classic MMO minimaps.
    draw_circle(c.x, c.y, radius + 7.0, Color::new(0.2, 0.15, 0.08, 1.0));
    draw_circle(c.x, c.y, radius + 5.0, Color::new(0.85, 0.68, 0.3, 1.0));
    draw_circle(c.x, c.y, radius + 2.0, Color::new(0.25, 0.18, 0.08, 1.0));
    draw_circle(
        c.x,
        c.y,
        radius,
        Color::new(ground.r, ground.g, ground.b, 0.92),
    );
    if let Some((z, tex)) = &game.map_texture
        && *z == zone
    {
        crate::panels::minimap_terrain(tex, zone, c, radius, game.pos, game.cam_yaw, range);
    }
    // Map up is where the camera faces.
    let f = vec2(game.cam_yaw.sin(), game.cam_yaw.cos());
    let r = vec2(-f.y, f.x);
    let to_map = |p: Vec3| {
        let rel = vec2(p.x - game.pos.x, p.z - game.pos.z);
        vec2(rel.dot(r), -rel.dot(f)) * (radius / range)
    };

    for e in game.entities.values() {
        if Some(e.view.id) == game.my_id {
            continue;
        }
        let m = to_map(e.pos);
        if m.length() > radius - 3.0 {
            continue;
        }
        if matches!(e.view.kind, EntityKind::QuestGiver(_))
            && let Some(mk) =
                crate::quests_ui::marker(&game.me.quests, zone, game.level(), &game.me.bags)
        {
            let s = if mk == crate::quests_ui::Marker::Ready {
                "?"
            } else {
                "!"
            };
            text_centered(s, c.x + m.x + 1.0, c.y + m.y + 7.0, 22.0, BLACK);
            text_centered(
                s,
                c.x + m.x,
                c.y + m.y + 6.0,
                22.0,
                Color::new(1.0, 0.85, 0.1, 1.0),
            );
            continue;
        }
        let color = if e.view.lootable {
            GOLD
        } else if matches!(e.view.kind, EntityKind::Merchant(_)) {
            Color::new(1.0, 0.75, 0.2, 1.0)
        } else if e.view.dead {
            Color::new(0.5, 0.5, 0.5, 1.0)
        } else if e.view.kind.is_player() {
            Color::new(0.3, 0.6, 1.0, 1.0)
        } else {
            reaction_color(&e.view, game.class)
        };
        let size = if game.target == Some(e.view.id) {
            4.0
        } else {
            2.5
        };
        draw_circle(c.x + m.x, c.y + m.y, size, color);
    }
    // Us, as an arrow pointing where we face.
    let d = wrap_angle(game.yaw - game.cam_yaw);
    let dir = vec2(-d.sin(), -d.cos());
    let side = vec2(-dir.y, dir.x);
    draw_triangle(
        c + dir * 8.0,
        c - dir * 5.0 + side * 5.0,
        c - dir * 5.0 - side * 5.0,
        Color::new(1.0, 0.95, 0.4, 1.0),
    );
    let local = zone.to_local(vec2(game.pos.x, game.pos.z));
    crate::panels::minimap_compass(c, radius, game.cam_yaw);
    let label = format!("{:.0}, {:.0}", local.x, local.y);
    text_centered(
        &label,
        c.x,
        c.y + radius + 18.0,
        15.0,
        Color::new(0.9, 0.9, 0.9, 1.0),
    );
}

fn chat(game: &Game) {
    let x = 16.0;
    let line_h = 19.0;
    let bottom = screen_height() - 40.0;
    let shown = 9;
    let typing = game.chat_input.is_some();
    let w = 440.0;
    if typing {
        draw_rectangle(
            x - 6.0,
            bottom - line_h * (shown as f32 + 1.0) - 6.0,
            w,
            line_h * (shown as f32 + 1.0) + 16.0,
            Color::new(0.0, 0.0, 0.0, 0.4),
        );
    }
    // Wrap long lines, then show the newest ones.
    let mut lines: Vec<(String, Color)> = Vec::new();
    for line in &game.chat[game.chat.len().saturating_sub(shown)..] {
        let mut current = String::new();
        for word in line.text.split(' ') {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if text_width(&candidate, 17.0) > w - 10.0 && !current.is_empty() {
                lines.push((std::mem::take(&mut current), line.color));
                current = format!("  {word}");
            } else {
                current = candidate;
            }
        }
        lines.push((current, line.color));
    }
    let start = lines.len().saturating_sub(shown);
    let mut y = bottom - line_h * (lines.len() - start) as f32;
    for (s, color) in &lines[start..] {
        text(s, x, y, 17.0, *color);
        y += line_h;
    }
    if let Some(input) = &game.chat_input {
        let cursor = if (game.time * 2.0) as i32 % 2 == 0 {
            "_"
        } else {
            ""
        };
        text(
            &format!("Say: {input}{cursor}"),
            x,
            bottom + 4.0,
            18.0,
            WHITE,
        );
    }
}

fn help() {
    let lines = [
        ("W S / arrows", "Run forward and back"),
        ("A D", "Strafe left and right"),
        ("Left / Right", "Turn"),
        ("Space", "Jump"),
        ("Left drag", "Look around"),
        ("Right drag", "Steer (cursor locks)"),
        ("Both buttons", "Run forward"),
        ("Mouse wheel", "Zoom"),
        ("Tab", "Target the next enemy"),
        ("Left click", "Target"),
        ("Right click", "Attack, loot, or trade"),
        ("1 - 6, E", "Use abilities"),
        ("T  /  F1", "Toggle attack / target self"),
        ("B  C  K", "Bags, character, crafting"),
        ("N  M", "Talents, world map"),
        ("P", "Sandbox panel (sandbox mode)"),
        ("Esc", "Close / clear target / menu"),
        ("Enter", "Chat (/help lists commands)"),
        ("/invite NAME", "Party up (or target, Invite)"),
        ("/p MESSAGE", "Talk to your party"),
        ("H", "Hide this help"),
    ];
    let w = 380.0;
    let h = 40.0 + lines.len() as f32 * 20.0;
    let x = screen_width() - w - 16.0;
    let y = 236.0;
    panel(Rect::new(x, y, w, h));
    text("Controls", x + 12.0, y + 26.0, 22.0, GOLD);
    for (i, (key, what)) in lines.iter().enumerate() {
        let ly = y + 50.0 + i as f32 * 20.0;
        text(key, x + 12.0, ly, 17.0, Color::new(1.0, 0.9, 0.6, 1.0));
        text(what, x + 150.0, ly, 17.0, WHITE);
    }
}
