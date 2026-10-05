//! The spell book (Y): every ability your class learns, and dragging them
//! onto the two hotbars.

use macroquad::prelude::*;
use shared::data::*;

use crate::drag::{self, Win};
use crate::game::Game;
use crate::hud::{self, GOLD};

const ROWS: usize = 6;
const ROW_H: f32 = 54.0;
const ICON: f32 = 46.0;
const COL_W: f32 = 250.0;

/// A spell picked up from the book or a hotbar slot.
#[derive(Clone, Copy, Debug)]
pub struct SpellDrag {
    pub ability: AbilityId,
    /// The hotbar slot it came from, or `None` from the book.
    pub from: Option<usize>,
    pub start: Vec2,
    /// Moved far enough to count as a drag rather than a click.
    pub moved: bool,
}

pub struct SpellbookLayout {
    pub window: Rect,
    /// Each entry's icon, in `Class::spellbook` order.
    pub icons: [Rect; SPELLBOOK_SIZE],
}

pub fn layout() -> SpellbookLayout {
    let (w, h) = (screen_width(), screen_height());
    let cols = SPELLBOOK_SIZE.div_ceil(ROWS);
    let (ww, wh) = (
        24.0 + cols as f32 * COL_W,
        44.0 + ROWS as f32 * ROW_H + 30.0,
    );
    let window = drag::place(
        Win::Spellbook,
        Rect::new(
            (w - ww) / 2.0 - 220.0,
            ((h - wh) / 2.0 - 60.0).max(20.0),
            ww,
            wh,
        ),
    );
    let icons = std::array::from_fn(|i| {
        let (col, row) = (i / ROWS, i % ROWS);
        Rect::new(
            window.x + 12.0 + col as f32 * COL_W,
            window.y + 40.0 + row as f32 * ROW_H,
            ICON,
            ICON,
        )
    });
    SpellbookLayout { window, icons }
}

/// The book entry under the mouse: its ability and the level it's learned at.
pub fn entry_at(game: &Game, mouse: Vec2) -> Option<(AbilityId, u8)> {
    let l = layout();
    let book = game.class.spellbook();
    l.icons
        .iter()
        .zip(book)
        .find(|(r, _)| {
            // The name next to the icon counts too.
            Rect::new(r.x, r.y, COL_W - 16.0, r.h).contains(mouse)
        })
        .map(|(_, e)| e)
}

pub fn draw(game: &Game, level: u8) {
    let l = layout();
    hud::window(
        l.window,
        &format!(
            "{} Spell Book ({})",
            game.class.name(),
            crate::keys::key_label(crate::keys::Action::Spellbook)
        ),
    );
    let mouse = vec2(mouse_position().0, mouse_position().1);
    let mut tooltip = None;
    for (r, (id, learned_at)) in l.icons.iter().zip(game.class.spellbook()) {
        let a = ability(id);
        let locked = learned_at > level;
        hud::ability_icon(*r, a, locked);
        let on_bar = game.hotbar.contains(&Some(id));
        let name_color = if locked {
            Color::new(0.6, 0.6, 0.6, 1.0)
        } else {
            WHITE
        };
        hud::text(a.name, r.right() + 10.0, r.y + 20.0, 19.0, name_color);
        let sub = if locked {
            format!("Learned at level {learned_at}")
        } else if on_bar {
            format!("Level {learned_at}  -  on your hotbar")
        } else {
            format!("Level {learned_at}")
        };
        let sub_color = if locked {
            Color::new(1.0, 0.45, 0.35, 1.0)
        } else {
            Color::new(0.75, 0.75, 0.75, 1.0)
        };
        hud::text(&sub, r.right() + 10.0, r.y + 40.0, 15.0, sub_color);
        let row = Rect::new(r.x, r.y, COL_W - 16.0, r.h);
        if row.contains(mouse) && game.dragging.is_none() {
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, GOLD);
            tooltip = Some((a, *r, locked.then_some(learned_at)));
        }
    }
    hud::text(
        "Drag a spell onto a hotbar. Drag one off the bar to remove it.",
        l.window.x + 14.0,
        l.window.bottom() - 12.0,
        16.0,
        Color::new(0.85, 0.8, 0.6, 1.0),
    );
    if let Some((a, r, locked)) = tooltip {
        hud::ability_tooltip(a, r, game.class, locked);
    }
}

/// The spell being dragged, following the mouse.
pub fn draw_dragged(game: &Game) {
    if let Some(d) = game.dragging.filter(|d| d.moved) {
        let (x, y) = mouse_position();
        let r = Rect::new(x - ICON / 2.0, y - ICON / 2.0, ICON, ICON);
        hud::ability_icon(r, ability(d.ability), false);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, GOLD);
    }
}

/// Where a dragged spell ends up when it's let go over hotbar slot `to`
/// (`None`: anywhere else). Returns the new hotbars.
pub fn drop(bar: &Hotbar, d: SpellDrag, to: Option<usize>) -> Hotbar {
    let mut bar = *bar;
    match (d.from, to) {
        // Swap two slots.
        (Some(from), Some(to)) => bar.swap(from, to),
        // Off the bar: take it off.
        (Some(from), None) => bar[from] = None,
        // From the book: it moves here if it was elsewhere on the bars, and
        // whatever was here goes back to the book.
        (None, Some(to)) => {
            for slot in bar.iter_mut() {
                if *slot == Some(d.ability) {
                    *slot = None;
                }
            }
            bar[to] = Some(d.ability);
        }
        (None, None) => {}
    }
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drag(ability: AbilityId, from: Option<usize>) -> SpellDrag {
        SpellDrag {
            ability,
            from,
            start: Vec2::ZERO,
            moved: true,
        }
    }

    #[test]
    fn dropping_moves_swaps_and_removes() {
        let bar = Class::Mage.default_hotbar();
        let fireball = bar[0].unwrap();
        let frostbolt = bar[1].unwrap();

        let swapped = drop(&bar, drag(fireball, Some(0)), Some(1));
        assert_eq!(swapped[0], Some(frostbolt));
        assert_eq!(swapped[1], Some(fireball));

        let removed = drop(&bar, drag(fireball, Some(0)), None);
        assert_eq!(removed[0], None);

        // From the book onto an empty slot: it moves rather than copies.
        let moved = drop(&bar, drag(fireball, None), Some(15));
        assert_eq!(moved[15], Some(fireball));
        assert_eq!(moved[0], None);
        assert_eq!(moved.iter().filter(|s| **s == Some(fireball)).count(), 1);

        assert_eq!(drop(&bar, drag(fireball, None), None), bar);
    }
}
