//! Saved accounts and characters, kept in a JSON file.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use shared::data::{Appearance, Class};
use shared::protocol::CharacterSummary;

use crate::character::{Character, validate_name};

/// How many characters one account may have.
pub const MAX_CHARACTERS: usize = 8;

#[derive(Serialize, Deserialize, Default)]
struct SaveFile {
    version: u32,
    /// Account name to password hash. Saves from before v7.2 have none.
    #[serde(default)]
    accounts: BTreeMap<String, String>,
    characters: Vec<Character>,
}

pub struct Store {
    /// `None` keeps everything in memory (tests).
    path: Option<PathBuf>,
    accounts: BTreeMap<String, String>,
    characters: Vec<Character>,
    /// What was last written, to skip writing when nothing changed.
    written: String,
}

/// Account names are case-insensitive.
pub fn normalize_account(raw: &str) -> Result<String, &'static str> {
    let name: String = raw.trim().to_lowercase();
    if name.is_empty() || name.len() > 24 || name.chars().any(|c| c.is_control()) {
        return Err("Account names must be 1 to 24 characters.");
    }
    Ok(name)
}

impl Store {
    pub fn in_memory() -> Self {
        Self {
            path: None,
            accounts: BTreeMap::new(),
            characters: Vec::new(),
            written: String::new(),
        }
    }

    /// Loads the save file at `path`, or starts empty if there isn't one yet.
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let mut store = Self {
            path: Some(path.clone()),
            accounts: BTreeMap::new(),
            characters: Vec::new(),
            written: String::new(),
        };
        match fs::read_to_string(&path) {
            Ok(text) => {
                let file: SaveFile = serde_json::from_str(&text).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{}: {e}", path.display()),
                    )
                })?;
                store.accounts = file.accounts;
                store.characters = file.characters;
                for c in &mut store.characters {
                    c.sanitize();
                }
                store.written = text;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        Ok(store)
    }

    /// The account's password hash, or `None` if it has no password yet
    /// (a new account, or one saved before passwords existed).
    pub fn password_hash(&self, account: &str) -> Option<&str> {
        self.accounts.get(account).map(String::as_str)
    }

    pub fn set_password_hash(&mut self, account: &str, hash: String) {
        self.accounts.insert(account.to_string(), hash);
    }

    /// Whether any character was saved under this account.
    pub fn has_characters(&self, account: &str) -> bool {
        self.characters.iter().any(|c| c.account == account)
    }

    pub fn list(&self, account: &str) -> Vec<CharacterSummary> {
        self.characters
            .iter()
            .filter(|c| c.account == account)
            .map(Character::summary)
            .collect()
    }

    pub fn get(&self, account: &str, name: &str) -> Option<&Character> {
        self.characters
            .iter()
            .find(|c| c.account == account && c.name.eq_ignore_ascii_case(name))
    }

    pub fn create(
        &mut self,
        account: &str,
        name: &str,
        class: Class,
        appearance: Appearance,
    ) -> Result<(), String> {
        let name = validate_name(name)?;
        if self
            .characters
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(&name))
        {
            return Err(format!("The name {name} is taken."));
        }
        if self
            .characters
            .iter()
            .filter(|c| c.account == account)
            .count()
            >= MAX_CHARACTERS
        {
            return Err(format!("You can have at most {MAX_CHARACTERS} characters."));
        }
        self.characters
            .push(Character::new(account, &name, class, appearance));
        Ok(())
    }

    pub fn delete(&mut self, account: &str, name: &str) -> Result<(), String> {
        let before = self.characters.len();
        self.characters
            .retain(|c| !(c.account == account && c.name.eq_ignore_ascii_case(name)));
        if self.characters.len() == before {
            return Err(format!("You have no character named {name}."));
        }
        Ok(())
    }

    /// Stores a character's latest state.
    pub fn update(&mut self, c: Character) {
        match self.characters.iter_mut().find(|old| old.name == c.name) {
            Some(old) => *old = c,
            None => self.characters.push(c),
        }
    }

    /// Writes the save file if anything changed.
    pub fn save(&mut self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let file = SaveFile {
            version: 2,
            accounts: self.accounts.clone(),
            characters: self.characters.clone(),
        };
        let text = serde_json::to_string_pretty(&file).map_err(io::Error::other)?;
        if text == self.written {
            return Ok(());
        }
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir)?;
        }
        // Write to a temporary file first so a crash can't leave half a save.
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, &text)?;
        fs::rename(&tmp, path)?;
        self.written = text;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_list_delete() {
        let mut s = Store::in_memory();
        s.create("ann", "aria", Class::Mage, Appearance::default())
            .unwrap();
        assert!(
            s.create("bob", "ARIA", Class::Rogue, Appearance::default())
                .is_err()
        );
        s.create("bob", "Brom", Class::Barbarian, Appearance::default())
            .unwrap();
        assert_eq!(s.list("ann").len(), 1);
        assert_eq!(s.list("ann")[0].name, "Aria");
        assert!(
            s.get("bob", "aria").is_none(),
            "accounts only see their own characters"
        );
        assert!(s.delete("bob", "Aria").is_err());
        s.delete("ann", "aria").unwrap();
        assert!(s.list("ann").is_empty());
    }

    #[test]
    fn saves_and_loads() {
        let dir = std::env::temp_dir().join(format!("rusty-mmo-test-{}", std::process::id()));
        let path = dir.join("characters.json");
        let _ = fs::remove_file(&path);
        let mut s = Store::open(path.clone()).unwrap();
        s.create(
            "ann",
            "Aria",
            Class::Cleric,
            Appearance {
                hair_style: 3,
                ..Default::default()
            },
        )
        .unwrap();
        let mut c = s.get("ann", "Aria").unwrap().clone();
        c.level = 5;
        c.money = 77;
        s.update(c);
        s.set_password_hash("ann", "$argon2id$hash".into());
        s.save().unwrap();

        let loaded = Store::open(path.clone()).unwrap();
        assert_eq!(loaded.password_hash("ann"), Some("$argon2id$hash"));
        assert_eq!(loaded.password_hash("bob"), None);
        let c = loaded.get("ann", "Aria").unwrap();
        assert_eq!(c.level, 5);
        assert_eq!(c.money, 77);
        assert_eq!(c.appearance.hair_style, 3);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn loads_saves_without_passwords() {
        let dir = std::env::temp_dir().join(format!("rusty-mmo-old-{}", std::process::id()));
        let path = dir.join("characters.json");
        let mut s = Store::open(path.clone()).unwrap();
        s.create("ann", "Aria", Class::Mage, Appearance::default())
            .unwrap();
        s.save().unwrap();
        // Strip the accounts, as a v7.0 server would have written it.
        let text = fs::read_to_string(&path).unwrap();
        let mut json: serde_json::Value = serde_json::from_str(&text).unwrap();
        json.as_object_mut().unwrap().remove("accounts");
        fs::write(&path, json.to_string()).unwrap();

        let loaded = Store::open(path.clone()).unwrap();
        assert!(loaded.has_characters("ann"));
        assert_eq!(loaded.password_hash("ann"), None);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn accounts() {
        assert_eq!(normalize_account("  Ann "), Ok("ann".to_string()));
        assert!(normalize_account("").is_err());
    }
}
