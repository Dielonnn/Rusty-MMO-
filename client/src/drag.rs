//! Windows you can move around by dragging their title bars. Each window
//! keeps where you left it until the game closes.

use std::cell::RefCell;
use std::collections::HashMap;

use macroquad::prelude::*;

/// Every window that can be dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Win {
    Bags,
    Character,
    Skills,
    Vendor,
    Talents,
    Sandbox,
    Travel,
    Spellbook,
    QuestGiver,
    QuestLog,
}

/// The strip along a window's top you grab to drag it.
pub const TITLE_BAR: f32 = 30.0;

thread_local! {
    /// How far each window has been moved from where it opens by default.
    static OFFSETS: RefCell<HashMap<Win, Vec2>> = RefCell::new(HashMap::new());
    /// The window being dragged, and where the mouse grabbed it.
    static HELD: RefCell<Option<(Win, Vec2)>> = const { RefCell::new(None) };
}

/// Where a window is, given where it would be if it had never been moved.
/// Its title bar always stays on screen.
pub fn place(win: Win, default: Rect) -> Rect {
    let offset = OFFSETS.with(|o| o.borrow().get(&win).copied().unwrap_or(Vec2::ZERO));
    let (w, h) = (screen_width(), screen_height());
    let x = (default.x + offset.x).clamp(60.0 - default.w, w - 60.0);
    let y = (default.y + offset.y).clamp(0.0, h - TITLE_BAR);
    Rect::new(x, y, default.w, default.h)
}

/// Puts every window back where it opens by default.
pub fn reset() {
    OFFSETS.with(|o| o.borrow_mut().clear());
}

/// The grabbable part of a window's title bar: all but the right end, where
/// close buttons sit.
pub fn title_bar(r: Rect) -> Rect {
    Rect::new(r.x, r.y, r.w - 40.0, TITLE_BAR)
}

/// Starts dragging a window, grabbed at `mouse`.
pub fn grab(win: Win, mouse: Vec2) {
    HELD.with(|h| *h.borrow_mut() = Some((win, mouse)));
}

/// Moves the held window with the mouse while the button is down, and lets
/// go when it's released. True while something is being dragged.
pub fn update(mouse: Vec2, held_down: bool) -> bool {
    HELD.with(|h| {
        let mut held = h.borrow_mut();
        let Some((win, last)) = *held else {
            return false;
        };
        if !held_down {
            *held = None;
            return false;
        }
        OFFSETS.with(|o| *o.borrow_mut().entry(win).or_insert(Vec2::ZERO) += mouse - last);
        *held = Some((win, mouse));
        true
    })
}
