//! Emotes: slash commands that make your character do something
//! (`/wave`, `/sit`, ...) and tell the players around you.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Emote {
    Kiss,
    Sit,
    Backflip,
    Wave,
    FlipOff,
    /// Wagging a finger: "no".
    No,
    Point,
}

impl Emote {
    pub const ALL: [Emote; 7] = [
        Emote::Kiss,
        Emote::Sit,
        Emote::Backflip,
        Emote::Wave,
        Emote::FlipOff,
        Emote::No,
        Emote::Point,
    ];

    /// The emote a slash command (without the `/`) stands for.
    pub fn from_command(cmd: &str) -> Option<Emote> {
        Some(match cmd.to_ascii_lowercase().as_str() {
            "kiss" => Emote::Kiss,
            "sit" => Emote::Sit,
            "backflip" | "flip" => Emote::Backflip,
            "wave" | "hi" | "bye" => Emote::Wave,
            "flipoff" | "rude" | "bird" => Emote::FlipOff,
            "no" | "wag" => Emote::No,
            "point" => Emote::Point,
            _ => return None,
        })
    }

    /// The main command, as listed in help.
    pub fn command(self) -> &'static str {
        match self {
            Emote::Kiss => "/kiss",
            Emote::Sit => "/sit",
            Emote::Backflip => "/backflip",
            Emote::Wave => "/wave",
            Emote::FlipOff => "/flipoff",
            Emote::No => "/no",
            Emote::Point => "/point",
        }
    }

    /// Seconds the emote plays for. `None`: until you move or act (sitting).
    pub fn duration(self) -> Option<f32> {
        match self {
            Emote::Sit => None,
            Emote::Backflip => Some(1.1),
            Emote::Kiss | Emote::No => Some(2.0),
            Emote::Wave | Emote::FlipOff | Emote::Point => Some(2.5),
        }
    }

    /// What the players around see in chat, with `target` if you have one.
    pub fn text(self, who: &str, target: Option<&str>) -> String {
        match (self, target) {
            (Emote::Kiss, Some(t)) => format!("{who} blows a kiss to {t}."),
            (Emote::Kiss, None) => format!("{who} blows a kiss."),
            (Emote::Sit, _) => format!("{who} sits down."),
            (Emote::Backflip, _) => format!("{who} does a backflip!"),
            (Emote::Wave, Some(t)) => format!("{who} waves at {t}."),
            (Emote::Wave, None) => format!("{who} waves."),
            (Emote::FlipOff, Some(t)) => format!("{who} flips {t} off."),
            (Emote::FlipOff, None) => format!("{who} flips everyone off."),
            (Emote::No, Some(t)) => format!("{who} wags a finger at {t}. No!"),
            (Emote::No, None) => format!("{who} wags a finger. No!"),
            (Emote::Point, Some(t)) => format!("{who} points at {t}."),
            (Emote::Point, None) => format!("{who} points over there."),
        }
    }

    /// A short label shown over the character while the emote plays.
    pub fn label(self) -> &'static str {
        match self {
            Emote::Kiss => "*blows a kiss*",
            Emote::Sit => "*sitting*",
            Emote::Backflip => "*backflip*",
            Emote::Wave => "*waves*",
            Emote::FlipOff => "*flips you off*",
            Emote::No => "*wags a finger*",
            Emote::Point => "*points*",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_emote_has_its_own_command() {
        for e in Emote::ALL {
            assert_eq!(Emote::from_command(&e.command()[1..]), Some(e));
        }
        assert_eq!(Emote::from_command("WAVE"), Some(Emote::Wave));
        assert_eq!(Emote::from_command("dance"), None);
    }
}
