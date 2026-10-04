//! The 2D interface drawn over the world: unit frames, action bar, cast bar,
//! nameplates, chat, minimap and floating combat text.

use macroquad::prelude::*;
use shared::data::*;
use shared::protocol::*;

use crate::game::{Game, wrap_angle};
use crate::render;

pub const PANEL: Color = Color::new(0.06, 0.06, 0.09, 0.82);
pub const BORDER: Color = Color::new(0.55, 0.5, 0.38, 1.0);
const GOLD: Color = Color::new(1.0, 0.82, 0.25, 1.0);

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
}

impl Layout {
    pub fn new() -> Self {
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
        Self {
            player_frame: Rect::new(16.0, 16.0, 250.0, 66.0),
            target_frame: Rect::new(282.0, 16.0, 250.0, 66.0),
            tot_frame: Rect::new(548.0, 30.0, 140.0, 36.0),
            hotbar,
            castbar: Rect::new((w - 320.0) / 2.0, bar_y - 46.0, 320.0, 20.0),
            xpbar: Rect::new(bar_x - 120.0, h - 16.0, bar_w + 240.0, 10.0),
            minimap: (vec2(w - 96.0, 96.0), 80.0),
            release_button: Rect::new((w - 200.0) / 2.0, h * 0.3 + 46.0, 200.0, 40.0),
            menu_buttons: std::array::from_fn(|i| {
                Rect::new(menu_x, h * 0.35 + 50.0 + i as f32 * 52.0, menu_w, 40.0)
            }),
        }
    }

    /// Whether the mouse is over something that should eat world clicks.
    pub fn blocks(&self, mouse: Vec2, has_target: bool, dead: bool) -> bool {
        self.hotbar.iter().any(|r| r.contains(mouse))
            || self.player_frame.contains(mouse)
            || (has_target && self.target_frame.contains(mouse))
            || (dead && self.release_button.contains(mouse))
            || self.minimap.0.distance(mouse) < self.minimap.1
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
        Class::Warrior => Color::new(0.78, 0.61, 0.43, 1.0),
        Class::Mage => Color::new(0.41, 0.8, 0.94, 1.0),
        Class::Cleric => Color::new(0.95, 0.95, 0.95, 1.0),
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
}

pub fn bar(r: Rect, frac: f32, fill: Color, label: &str) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(r.x, r.y, r.w * frac.clamp(0.0, 1.0), r.h, fill);
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
    let hover = r.contains(vec2(mouse_position().0, mouse_position().1));
    let bg = if hover {
        Color::new(0.35, 0.27, 0.12, 0.95)
    } else {
        Color::new(0.22, 0.17, 0.08, 0.95)
    };
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if hover { GOLD } else { BORDER });
    text_centered(label, r.x + r.w / 2.0, r.y + r.h / 2.0 + 7.0, 22.0, WHITE);
}

fn power_color(view: &EntityView) -> Color {
    match view.kind {
        EntityKind::Player(c) if c.power_kind() == PowerKind::Rage => {
            Color::new(0.8, 0.15, 0.15, 1.0)
        }
        _ => Color::new(0.2, 0.4, 0.95, 1.0),
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
    let my_level = game.my_view().map_or(1, |v| v.level);
    let lvl = level_label(view);
    text(
        &lvl,
        r.x + r.w - 8.0 - text_width(&lvl, 18.0),
        r.y + 18.0,
        18.0,
        level_color(view, my_level),
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
    aura_row(
        layout.player_frame.x,
        layout.player_frame.bottom() + 6.0,
        me,
    );
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
                layout.target_frame.bottom() + 38.0,
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
            let y = layout.target_frame.bottom() + if t.view.cast.is_some() { 72.0 } else { 50.0 };
            text(
                "Attacking",
                layout.target_frame.x + 8.0,
                y,
                16.0,
                Color::new(1.0, 0.5, 0.3, 1.0),
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
    if let Some((msg, t)) = &game.banner {
        let alpha = (3.0 - t).min(1.0);
        text_centered(
            msg,
            screen_width() / 2.0,
            screen_height() * 0.3,
            54.0,
            Color::new(1.0, 0.85, 0.3, alpha),
        );
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
}

fn action_bar(game: &Game, layout: &Layout, me: &EntityView) {
    let mouse = vec2(mouse_position().0, mouse_position().1);
    let target = game.target_ent();
    let mut tooltip = None;
    for (i, (r, id)) in layout.hotbar.iter().zip(game.class.abilities()).enumerate() {
        let a = ability(id);
        let c = school_color(a.school);
        draw_rectangle(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0, PANEL);
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

        let out_of_range = a.targeting == Targeting::Enemy
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
        text(
            &(i + 1).to_string(),
            r.x + 4.0,
            r.y + 14.0,
            15.0,
            Color::new(0.9, 0.9, 0.9, 1.0),
        );
        if a.cost > 0.0 && me.power < a.cost {
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
        let hover = r.contains(mouse);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if hover { GOLD } else { BORDER });
        if hover {
            tooltip = Some((a, *r));
        }
    }
    if let Some((a, r)) = tooltip {
        ability_tooltip(a, r, game.class);
    }
}

fn ability_tooltip(a: &Ability, slot: Rect, class: Class) {
    let mut lines: Vec<(String, Color)> = vec![(a.name.to_string(), WHITE)];
    let mut stats = Vec::new();
    if a.cost > 0.0 {
        stats.push(format!("{} {}", a.cost, class.power_kind().name()));
    }
    match a.targeting {
        Targeting::Enemy | Targeting::Friendly => stats.push(format!("{} yd range", a.range)),
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
    let w = 300.0;
    let h = 12.0 + lines.len() as f32 * 20.0;
    let x = (slot.x + slot.w / 2.0 - w / 2.0).clamp(4.0, screen_width() - w - 4.0);
    let y = slot.y - h - 10.0;
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
            let head = e.pos + Vec3::Y * (render::model_height(e.view.kind) + 0.45);
            project(cam, head).map(|p| (e.pos.distance(cam.position), p, e))
        })
        .collect();
    // Far ones first so near ones draw on top.
    plates.sort_by(|a, b| b.0.total_cmp(&a.0));
    let my_level = game.my_view().map_or(1, |v| v.level);
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
        let height = game
            .entities
            .get(&f.entity)
            .map_or(2.0, |e| render::model_height(e.view.kind));
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
    draw_circle(c.x, c.y, radius + 3.0, BORDER);
    draw_circle(c.x, c.y, radius, Color::new(0.18, 0.3, 0.16, 0.92));
    // Map up is where the camera faces.
    let f = vec2(game.cam_yaw.sin(), game.cam_yaw.cos());
    let r = vec2(-f.y, f.x);
    let to_map = |p: Vec3| {
        let rel = vec2(p.x - game.pos.x, p.z - game.pos.z);
        vec2(rel.dot(r), -rel.dot(f)) * (radius / range)
    };
    let town = to_map(Vec3::ZERO);
    if town.length() < radius + 30.0 {
        let tr = shared::world::TOWN_RADIUS * radius / range;
        draw_circle(
            c.x + town.x,
            c.y + town.y,
            tr,
            Color::new(0.55, 0.52, 0.45, 0.6),
        );
    }
    for e in game.entities.values() {
        if Some(e.view.id) == game.my_id {
            continue;
        }
        let m = to_map(e.pos);
        if m.length() > radius - 3.0 {
            continue;
        }
        let color = if e.view.dead {
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
    let label = format!("{:.0}, {:.0}", game.pos.x, game.pos.z);
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
    let start = game.chat.len().saturating_sub(shown);
    let mut y = bottom - line_h * (game.chat.len() - start) as f32;
    for line in &game.chat[start..] {
        let mut s = line.text.clone();
        while text_width(&s, 17.0) > w - 10.0 && !s.is_empty() {
            s.pop();
        }
        text(&s, x, y, 17.0, line.color);
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
            Color::new(1.0, 1.0, 1.0, 1.0),
        );
    }
}

fn help() {
    let lines = [
        ("W A S D / arrows", "Run and turn (Q E strafe)"),
        ("Space", "Jump"),
        ("Left drag", "Look around"),
        ("Right drag", "Steer with the mouse"),
        ("Both buttons", "Run forward"),
        ("Mouse wheel", "Zoom"),
        ("Tab", "Target the next enemy"),
        ("Left click", "Target"),
        ("Right click", "Target and attack"),
        ("F1", "Target yourself"),
        ("1 - 6", "Use abilities"),
        ("T", "Start / stop attacking"),
        ("Esc", "Clear target / menu"),
        ("Enter", "Chat (/who lists players)"),
        ("H", "Hide this help"),
    ];
    let w = 380.0;
    let h = 40.0 + lines.len() as f32 * 20.0;
    let x = screen_width() - w - 16.0;
    let y = 220.0;
    panel(Rect::new(x, y, w, h));
    text("Controls", x + 12.0, y + 26.0, 22.0, GOLD);
    for (i, (key, what)) in lines.iter().enumerate() {
        let ly = y + 50.0 + i as f32 * 20.0;
        text(key, x + 12.0, ly, 17.0, Color::new(1.0, 0.9, 0.6, 1.0));
        text(what, x + 150.0, ly, 17.0, WHITE);
    }
}
