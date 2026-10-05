//! Key bindings: every game action you can rebind in Settings > Controls,
//! the key it starts on, and reading the keyboard through them.
//!
//! Enter and / (chat), Esc (close and menu), Backspace and Shift (top
//! hotbar) stay fixed, so they can't be bound to anything else.

use macroquad::prelude::*;

use crate::settings;

/// Something a key does in the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Forward,
    Back,
    StrafeLeft,
    StrafeRight,
    TurnLeft,
    TurnRight,
    Jump,
    /// A hotbar slot, 0 to 7 (Shift for the top bar).
    Slot(u8),
    TargetNext,
    TargetSelf,
    AutoAttack,
    Bags,
    Character,
    Skills,
    Spellbook,
    Talents,
    QuestLog,
    Map,
    Travel,
    Sandbox,
    Help,
}

/// Every action, in the order Settings lists them.
pub const ACTIONS: [Action; 28] = [
    Action::Forward,
    Action::Back,
    Action::StrafeLeft,
    Action::StrafeRight,
    Action::TurnLeft,
    Action::TurnRight,
    Action::Jump,
    Action::TargetNext,
    Action::TargetSelf,
    Action::AutoAttack,
    Action::Slot(0),
    Action::Slot(1),
    Action::Slot(2),
    Action::Slot(3),
    Action::Slot(4),
    Action::Slot(5),
    Action::Slot(6),
    Action::Slot(7),
    Action::Bags,
    Action::Character,
    Action::Skills,
    Action::Spellbook,
    Action::Talents,
    Action::QuestLog,
    Action::Map,
    Action::Travel,
    Action::Sandbox,
    Action::Help,
];

impl Action {
    pub fn index(self) -> usize {
        ACTIONS.iter().position(|a| *a == self).unwrap_or(0)
    }

    pub fn name(self) -> String {
        match self {
            Action::Forward => "Run forward".into(),
            Action::Back => "Run back".into(),
            Action::StrafeLeft => "Strafe left".into(),
            Action::StrafeRight => "Strafe right".into(),
            Action::TurnLeft => "Turn left".into(),
            Action::TurnRight => "Turn right".into(),
            Action::Jump => "Jump".into(),
            Action::Slot(i) => format!("Hotbar {}", i + 1),
            Action::TargetNext => "Target next enemy".into(),
            Action::TargetSelf => "Target yourself".into(),
            Action::AutoAttack => "Attack on/off".into(),
            Action::Bags => "Bags".into(),
            Action::Character => "Character".into(),
            Action::Skills => "Skills".into(),
            Action::Spellbook => "Spell book".into(),
            Action::Talents => "Talents".into(),
            Action::QuestLog => "Quest log".into(),
            Action::Map => "World map".into(),
            Action::Travel => "Use waystone".into(),
            Action::Sandbox => "Sandbox panel".into(),
            Action::Help => "Controls help".into(),
        }
    }

    /// The key it's saved under.
    pub fn save_key(self) -> String {
        match self {
            Action::Slot(i) => format!("key_slot{}", i + 1),
            a => format!("key_{a:?}").to_lowercase(),
        }
    }

    pub fn default_key(self) -> KeyCode {
        match self {
            Action::Forward => KeyCode::W,
            Action::Back => KeyCode::S,
            Action::StrafeLeft => KeyCode::A,
            Action::StrafeRight => KeyCode::D,
            Action::TurnLeft => KeyCode::Left,
            Action::TurnRight => KeyCode::Right,
            Action::Jump => KeyCode::Space,
            Action::Slot(i) => [
                KeyCode::Key1,
                KeyCode::Key2,
                KeyCode::Key3,
                KeyCode::Key4,
                KeyCode::Key5,
                KeyCode::Key6,
                KeyCode::Q,
                KeyCode::E,
            ][i as usize % 8],
            Action::TargetNext => KeyCode::Tab,
            Action::TargetSelf => KeyCode::F1,
            Action::AutoAttack => KeyCode::T,
            Action::Bags => KeyCode::B,
            Action::Character => KeyCode::C,
            Action::Skills => KeyCode::K,
            Action::Spellbook => KeyCode::Y,
            Action::Talents => KeyCode::N,
            Action::QuestLog => KeyCode::L,
            Action::Map => KeyCode::M,
            Action::Travel => KeyCode::F,
            Action::Sandbox => KeyCode::P,
            Action::Help => KeyCode::H,
        }
    }
}

/// The keys an action can be bound to.
const BINDABLE: &[KeyCode] = &[
    KeyCode::A,
    KeyCode::B,
    KeyCode::C,
    KeyCode::D,
    KeyCode::E,
    KeyCode::F,
    KeyCode::G,
    KeyCode::H,
    KeyCode::I,
    KeyCode::J,
    KeyCode::K,
    KeyCode::L,
    KeyCode::M,
    KeyCode::N,
    KeyCode::O,
    KeyCode::P,
    KeyCode::Q,
    KeyCode::R,
    KeyCode::S,
    KeyCode::T,
    KeyCode::U,
    KeyCode::V,
    KeyCode::W,
    KeyCode::X,
    KeyCode::Y,
    KeyCode::Z,
    KeyCode::Key0,
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::Key5,
    KeyCode::Key6,
    KeyCode::Key7,
    KeyCode::Key8,
    KeyCode::Key9,
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
    KeyCode::F12,
    KeyCode::Space,
    KeyCode::Tab,
    KeyCode::Up,
    KeyCode::Down,
    KeyCode::Left,
    KeyCode::Right,
    KeyCode::Insert,
    KeyCode::Delete,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::Apostrophe,
    KeyCode::Comma,
    KeyCode::Minus,
    KeyCode::Period,
    KeyCode::Semicolon,
    KeyCode::Equal,
    KeyCode::LeftBracket,
    KeyCode::RightBracket,
    KeyCode::Backslash,
    KeyCode::GraveAccent,
    KeyCode::Kp0,
    KeyCode::Kp1,
    KeyCode::Kp2,
    KeyCode::Kp3,
    KeyCode::Kp4,
    KeyCode::Kp5,
    KeyCode::Kp6,
    KeyCode::Kp7,
    KeyCode::Kp8,
    KeyCode::Kp9,
    KeyCode::LeftControl,
    KeyCode::LeftAlt,
    KeyCode::RightControl,
    KeyCode::RightAlt,
];

pub fn bindable(k: KeyCode) -> bool {
    BINDABLE.contains(&k)
}

/// How a key is written in the settings file.
pub fn save_name(k: KeyCode) -> String {
    format!("{k:?}")
}

/// The key a settings file names, if it's one that can be bound.
pub fn from_save_name(name: &str) -> Option<KeyCode> {
    BINDABLE.iter().copied().find(|k| save_name(*k) == name)
}

/// How a key is shown to players: "1" not "Key1", "Num 1" not "Kp1".
pub fn label(k: KeyCode) -> String {
    let name = save_name(k);
    match k {
        KeyCode::Space => "Space".into(),
        KeyCode::LeftControl => "Left Ctrl".into(),
        KeyCode::RightControl => "Right Ctrl".into(),
        KeyCode::LeftAlt => "Left Alt".into(),
        KeyCode::RightAlt => "Right Alt".into(),
        KeyCode::Apostrophe => "'".into(),
        KeyCode::Comma => ",".into(),
        KeyCode::Minus => "-".into(),
        KeyCode::Period => ".".into(),
        KeyCode::Semicolon => ";".into(),
        KeyCode::Equal => "=".into(),
        KeyCode::LeftBracket => "[".into(),
        KeyCode::RightBracket => "]".into(),
        KeyCode::Backslash => "\\".into(),
        KeyCode::GraveAccent => "`".into(),
        _ => {
            if let Some(n) = name.strip_prefix("Kp") {
                format!("Num {n}")
            } else if let Some(n) = name.strip_prefix("Key") {
                n.into()
            } else {
                name
            }
        }
    }
}

/// The key an action is bound to now.
pub fn key(a: Action) -> KeyCode {
    settings::with(|s| s.keys[a.index()])
}

/// The label of the key an action is bound to, for help text and titles.
pub fn key_label(a: Action) -> String {
    label(key(a))
}

pub fn down(a: Action) -> bool {
    is_key_down(key(a))
}

pub fn pressed(a: Action) -> bool {
    is_key_pressed(key(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_bindable_and_distinct() {
        for (i, a) in ACTIONS.iter().enumerate() {
            assert!(bindable(a.default_key()), "{a:?}");
            assert_eq!(a.index(), i);
            for b in &ACTIONS[i + 1..] {
                assert_ne!(a.default_key(), b.default_key(), "{a:?} and {b:?}");
                assert_ne!(a.save_key(), b.save_key());
            }
        }
    }

    #[test]
    fn key_names_round_trip() {
        for &k in BINDABLE {
            assert_eq!(from_save_name(&save_name(k)), Some(k));
        }
        assert_eq!(from_save_name("Escape"), None);
        assert_eq!(label(KeyCode::Key1), "1");
        assert_eq!(label(KeyCode::Kp5), "Num 5");
    }
}
