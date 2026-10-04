//! Quests on screen: the quest giver's window, the quest log (L), the
//! tracker under the minimap, and the "!" and "?" over quest givers' heads.

use macroquad::prelude::*;
use shared::data::{format_money, item};
use shared::protocol::Stack;
use shared::quests::{Goal, Quest, QuestId, QuestLog, quest, zone_quests};
use shared::world::Zone;

use crate::hud::{
    BORDER, GOLD, PANEL, button, item_icon, panel, quality_color, text, text_centered, text_width,
    wrap,
};

/// Where a quest stands for you.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Status {
    /// Not offered yet: you need this level.
    TooLow(u8),
    Available,
    InProgress(u16, u16),
    /// Done: go back to the quest giver.
    Ready,
    Done,
}

pub fn status(log: &QuestLog, q: &Quest, level: u8, bags: &[Option<Stack>]) -> Status {
    if log.is_done(q.id) {
        return Status::Done;
    }
    match log.progress(q.id) {
        Some(p) => match q.goal {
            Goal::Kill { count, .. } if p >= count => Status::Ready,
            Goal::Kill { count, .. } => Status::InProgress(p, count),
            Goal::TurnIn { item: it } => {
                if bags.iter().flatten().any(|(id, _)| *id == it) {
                    Status::Ready
                } else {
                    Status::InProgress(0, 1)
                }
            }
        },
        None if level < q.min_level => Status::TooLow(q.min_level),
        None => Status::Available,
    }
}

/// What a quest giver shows over their head: "?" when you have a quest to
/// hand in, "!" when they have one for you.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Marker {
    Ready,
    Available,
}

pub fn marker(log: &QuestLog, zone: Zone, level: u8, bags: &[Option<Stack>]) -> Option<Marker> {
    let all: Vec<Status> = zone_quests(zone)
        .iter()
        .map(|q| status(log, q, level, bags))
        .collect();
    if all.contains(&Status::Ready) {
        Some(Marker::Ready)
    } else if all.contains(&Status::Available) {
        Some(Marker::Available)
    } else {
        None
    }
}

/// The objective line: "Gray Wolves slain: 3/8".
pub fn objective(q: &Quest, s: Status) -> String {
    let (have, need) = match s {
        Status::InProgress(p, n) => (p, n),
        Status::Ready | Status::Done => (q.needed(), q.needed()),
        _ => (0, q.needed()),
    };
    match q.goal {
        Goal::Kill { kind, count } => {
            let name = kind.template().name;
            if count == 1 {
                format!("{name} slain: {have}/1")
            } else {
                format!("{name}s slain: {have}/{need}").replace("Wolfs", "Wolves")
            }
        }
        Goal::TurnIn { item: it } => format!("{} crafted: {have}/1", item(it).name),
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
    Accept(QuestId),
    TurnIn(QuestId),
    Abandon(QuestId),
}

pub struct GiverLayout {
    pub window: Rect,
    pub rows: Vec<Rect>,
    pub buttons: Vec<(Rect, Action)>,
    pub close: Rect,
}

pub fn giver_layout(zone: Zone, log: &QuestLog, level: u8, bags: &[Option<Stack>]) -> GiverLayout {
    let (w, h) = (screen_width(), screen_height());
    let window = Rect::new(16.0, ((h - 560.0) / 2.0).max(110.0), 470.0, 560.0);
    let _ = w;
    let mut rows = Vec::new();
    let mut buttons = Vec::new();
    for (i, q) in zone_quests(zone).iter().enumerate() {
        let row = Rect::new(
            window.x + 12.0,
            window.y + 40.0 + i as f32 * 166.0,
            window.w - 24.0,
            158.0,
        );
        let b = Rect::new(row.right() - 110.0, row.bottom() - 36.0, 100.0, 30.0);
        match status(log, q, level, bags) {
            Status::Available => buttons.push((b, Action::Accept(q.id))),
            Status::Ready => buttons.push((b, Action::TurnIn(q.id))),
            _ => {}
        }
        rows.push(row);
    }
    let close = Rect::new(window.right() - 34.0, window.y + 4.0, 28.0, 22.0);
    GiverLayout {
        window,
        rows,
        buttons,
        close,
    }
}

fn status_text(s: Status) -> (String, Color) {
    match s {
        Status::TooLow(l) => (
            format!("Requires level {l}"),
            Color::new(0.9, 0.4, 0.35, 1.0),
        ),
        Status::Available => ("Available".into(), Color::new(1.0, 0.85, 0.2, 1.0)),
        Status::InProgress(..) => ("In progress".into(), Color::new(0.75, 0.75, 0.75, 1.0)),
        Status::Ready => (
            "Complete! Hand it in.".into(),
            Color::new(0.4, 1.0, 0.4, 1.0),
        ),
        Status::Done => ("Done".into(), Color::new(0.55, 0.55, 0.55, 1.0)),
    }
}

fn reward_line(q: &Quest, x: f32, y: f32) -> Option<(Rect, shared::data::ItemId)> {
    text("Reward:", x, y, 15.0, Color::new(0.8, 0.8, 0.8, 1.0));
    let mut cx = x + text_width("Reward:", 15.0) + 8.0;
    let money = format_money(q.money);
    text(&money, cx, y, 15.0, GOLD);
    cx += text_width(&money, 15.0) + 10.0;
    text(
        &format!("{} XP", q.xp),
        cx,
        y,
        15.0,
        Color::new(0.75, 0.6, 1.0, 1.0),
    );
    cx += text_width(&format!("{} XP", q.xp), 15.0) + 10.0;
    q.reward.map(|r| {
        let icon = Rect::new(cx, y - 17.0, 24.0, 24.0);
        item_icon(icon, r, 1);
        text(
            item(r).name,
            icon.right() + 5.0,
            y,
            15.0,
            quality_color(item(r).quality),
        );
        (icon, r)
    })
}

/// The quest giver's window. Returns the reward item under the mouse, for a
/// tooltip.
pub fn draw_giver(
    giver_name: &str,
    zone: Zone,
    log: &QuestLog,
    level: u8,
    bags: &[Option<Stack>],
) -> Option<(Rect, shared::data::ItemId)> {
    let l = giver_layout(zone, log, level, bags);
    let r = l.window;
    panel(r);
    draw_rectangle(
        r.x + 2.0,
        r.y + 2.0,
        r.w - 4.0,
        26.0,
        Color::new(0.3, 0.22, 0.06, 0.95),
    );
    text(giver_name, r.x + 10.0, r.y + 21.0, 20.0, GOLD);
    button(l.close, "x");
    let mouse = vec2(mouse_position().0, mouse_position().1);
    let mut hover = None;
    for (q, row) in zone_quests(zone).iter().zip(&l.rows) {
        let s = status(log, q, level, bags);
        draw_rectangle(
            row.x,
            row.y,
            row.w,
            row.h,
            Color::new(0.16, 0.12, 0.08, 0.8),
        );
        draw_rectangle_lines(row.x, row.y, row.w, row.h, 1.0, BORDER);
        // A "!" or "?" badge.
        let badge = match s {
            Status::Ready => Some("?"),
            Status::Available => Some("!"),
            _ => None,
        };
        if let Some(bdg) = badge {
            text(
                bdg,
                row.x + 8.0,
                row.y + 24.0,
                26.0,
                Color::new(1.0, 0.85, 0.15, 1.0),
            );
        }
        text(q.name, row.x + 26.0, row.y + 22.0, 20.0, GOLD);
        let (st, sc) = status_text(s);
        text(
            &st,
            row.right() - 10.0 - text_width(&st, 15.0),
            row.y + 20.0,
            15.0,
            sc,
        );
        let body = if s == Status::Ready {
            q.done_text
        } else {
            q.text
        };
        for (k, line) in wrap(body, 62).iter().take(4).enumerate() {
            text(
                line,
                row.x + 10.0,
                row.y + 44.0 + k as f32 * 17.0,
                15.0,
                Color::new(0.92, 0.88, 0.78, 1.0),
            );
        }
        text(
            &objective(q, s),
            row.x + 10.0,
            row.y + 118.0,
            15.0,
            Color::new(1.0, 0.95, 0.6, 1.0),
        );
        if let Some((icon, id)) = reward_line(q, row.x + 10.0, row.bottom() - 12.0)
            && icon.contains(mouse)
        {
            hover = Some((icon, id));
        }
    }
    for (b, action) in &l.buttons {
        let label = match action {
            Action::Accept(_) => "Accept",
            Action::TurnIn(_) => "Complete",
            Action::Abandon(_) => "Abandon",
        };
        button(*b, label);
    }
    hover
}

pub struct LogLayout {
    pub window: Rect,
    pub buttons: Vec<(Rect, Action)>,
}

pub fn log_layout(log: &QuestLog) -> LogLayout {
    let (w, h) = (screen_width(), screen_height());
    let rows = log.active.len().max(1) as f32;
    let window = Rect::new(
        (w - 460.0) / 2.0,
        ((h - 400.0) / 2.0).max(60.0),
        460.0,
        60.0 + rows * 92.0,
    );
    let buttons = log
        .active
        .iter()
        .enumerate()
        .map(|(i, (id, _))| {
            let y = window.y + 40.0 + i as f32 * 92.0;
            (
                Rect::new(window.right() - 100.0, y + 50.0, 86.0, 28.0),
                Action::Abandon(*id),
            )
        })
        .collect();
    LogLayout { window, buttons }
}

pub fn draw_log(log: &QuestLog, level: u8, bags: &[Option<Stack>]) {
    let l = log_layout(log);
    let r = l.window;
    panel(r);
    draw_rectangle(
        r.x + 2.0,
        r.y + 2.0,
        r.w - 4.0,
        26.0,
        Color::new(0.3, 0.22, 0.06, 0.95),
    );
    text(
        &format!(
            "Quest Log (L)  -  {}/{}",
            log.active.len(),
            shared::quests::MAX_ACTIVE
        ),
        r.x + 10.0,
        r.y + 21.0,
        20.0,
        GOLD,
    );
    if log.active.is_empty() {
        text_centered(
            "No quests. Look for a \"!\" over the quest giver in town.",
            r.x + r.w / 2.0,
            r.y + 70.0,
            16.0,
            Color::new(0.8, 0.8, 0.8, 1.0),
        );
    }
    for (i, (id, _)) in log.active.iter().enumerate() {
        let q = quest(*id);
        let y = r.y + 40.0 + i as f32 * 92.0;
        let s = status(log, q, level, bags);
        text(q.name, r.x + 14.0, y + 18.0, 19.0, GOLD);
        let (st, sc) = status_text(s);
        text(&st, r.x + 14.0, y + 38.0, 15.0, sc);
        text(
            &objective(q, s),
            r.x + 14.0,
            y + 58.0,
            15.0,
            Color::new(1.0, 0.95, 0.6, 1.0),
        );
        text(
            &format!(
                "From {}, {}",
                shared::quests::quest_giver_name(q.zone),
                q.zone.town_name()
            ),
            r.x + 14.0,
            y + 78.0,
            14.0,
            Color::new(0.7, 0.7, 0.7, 1.0),
        );
    }
    for (b, _) in &l.buttons {
        button(*b, "Abandon");
    }
}

/// The quest tracker on the right side of the screen.
pub fn draw_tracker(log: &QuestLog, level: u8, bags: &[Option<Stack>], top: f32) {
    if log.active.is_empty() {
        return;
    }
    let x = screen_width() - 290.0;
    let mut y = top;
    text("Quests", x, y, 19.0, GOLD);
    y += 6.0;
    for (id, _) in &log.active {
        let q = quest(*id);
        let s = status(log, q, level, bags);
        y += 20.0;
        text(q.name, x, y, 17.0, Color::new(1.0, 0.85, 0.3, 1.0));
        y += 17.0;
        let (line, c) = if s == Status::Ready {
            (
                format!("  Return to {}", shared::quests::quest_giver_name(q.zone)),
                Color::new(0.4, 1.0, 0.4, 1.0),
            )
        } else {
            (
                format!("  - {}", objective(q, s)),
                Color::new(0.92, 0.92, 0.92, 1.0),
            )
        };
        text(&line, x, y, 15.0, c);
    }
    let _ = PANEL;
}

/// A big golden "!" or "?" over a quest giver, as screen text.
pub fn draw_marker(m: Marker, at: Vec2, time: f32) {
    let bob = (time * 2.5).sin() * 4.0;
    let s = if m == Marker::Ready { "?" } else { "!" };
    let c = Color::new(1.0, 0.82, 0.1, 1.0);
    for (dx, dy) in [(-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0)] {
        text_centered(
            s,
            at.x + dx,
            at.y + bob + dy,
            44.0,
            Color::new(0.0, 0.0, 0.0, 0.8),
        );
    }
    text_centered(s, at.x, at.y + bob, 44.0, c);
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::data::items;

    #[test]
    fn statuses_follow_progress_and_bags() {
        let q = &zone_quests(Zone::Amberfall)[0];
        let craft = &zone_quests(Zone::Amberfall)[1];
        let mut log = QuestLog::default();
        assert_eq!(status(&log, q, 1, &[]), Status::Available);
        assert_eq!(status(&log, craft, 1, &[]), Status::TooLow(2));
        assert_eq!(
            marker(&log, Zone::Amberfall, 1, &[]),
            Some(Marker::Available)
        );
        log.active.push((q.id, 3));
        log.active.push((craft.id, 0));
        assert_eq!(status(&log, q, 2, &[]), Status::InProgress(3, 8));
        let bags = [Some((items::LEATHER_VEST, 1))];
        assert_eq!(status(&log, craft, 2, &bags), Status::Ready);
        assert_eq!(marker(&log, Zone::Amberfall, 2, &bags), Some(Marker::Ready));
        assert_eq!(
            objective(q, Status::InProgress(3, 8)),
            "Gray Wolves slain: 3/8"
        );
        log.active.clear();
        log.done = vec![q.id, craft.id, zone_quests(Zone::Amberfall)[2].id];
        assert_eq!(marker(&log, Zone::Amberfall, 10, &[]), None);
    }
}
