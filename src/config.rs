use serde::Deserialize;
use std::{error::Error, fs, path::PathBuf, str::FromStr};
// Assuming global_hotkey uses these types. Adjust if necessary based on the actual crate API.
// If global_hotkey doesn't expose Modifiers/Code directly for config, we might need custom deserialization
// or a simpler string representation initially. For now, let's assume direct use is possible or we define placeholders.
// Re-checking global_hotkey docs: It uses `Modifiers` and `Key`. Let's use those.
// Update: Using Code and Modifiers from global_hotkey::hotkey
use global_hotkey::hotkey::{Code, Modifiers};

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct HotkeyMapping {
    #[serde(with = "modifiers_serde")]
    pub modifiers: Modifiers,
    #[serde(with = "code_serde")]
    pub key: Code,
    pub device_name: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    #[serde(default)] // Defaults to false if not present
    pub fuzzy_match: bool,
    #[serde(default)] // Defaults to an empty vec if not present
    pub hotkeys: Vec<HotkeyMapping>,
}

/// Loads configuration from `config.toml` located next to the executable.
pub fn load_config() -> Result<Config, Box<dyn Error>> {
    let mut config_path = get_executable_dir()?;
    config_path.push("config.toml");

    println!("Attempting to load config from: {}", config_path.display()); // Added for debugging

    let config_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config file at {}: {}", config_path.display(), e))?;

    let config: Config = toml::from_str(&config_content)
        .map_err(|e| format!("Failed to parse TOML config: {}", e))?;

    Ok(config)
}

/// Gets the directory containing the executable.
fn get_executable_dir() -> Result<PathBuf, Box<dyn Error>> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {}", e))?;
    let exe_dir = exe_path.parent()
        .ok_or("Failed to get parent directory of the executable")?
        .to_path_buf();
    Ok(exe_dir)
}

// --- Serde helpers for Modifiers and Code ---
// These might need adjustment based on how global_hotkey expects keys/modifiers
// to be represented in text or if it provides its own serde features.
// This is a basic implementation assuming string representations.

mod modifiers_serde {
    use super::Modifiers;
    use serde::{self, Deserializer, Serializer};
    use std::str::FromStr;

    pub fn serialize<S>(modifiers: &Modifiers, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // This serialization might be overly simple. Check global_hotkey for canonical representation.
        let mut s = String::new();
        if modifiers.contains(Modifiers::SHIFT) { s.push_str("Shift+"); }
        if modifiers.contains(Modifiers::CONTROL) { s.push_str("Ctrl+"); }
        if modifiers.contains(Modifiers::ALT) { s.push_str("Alt+"); }
        if modifiers.contains(Modifiers::META) { s.push_str("Meta+"); }
        // Remove trailing '+' if any
        if s.ends_with('+') { s.pop(); }
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Modifiers, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Modifiers::from_str(&s).map_err(serde::de::Error::custom)
    }
}

mod code_serde {
    use super::Code;
    use serde::{self, Deserializer, Serializer};
    use std::str::FromStr;

    pub fn serialize<S>(key: &Code, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Need to convert Code enum to string. This depends heavily on global_hotkey's implementation.
        // Placeholder: Use Debug representation, might not be ideal for config files.
        serializer.serialize_str(&format!("{:?}", key))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Code, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        // This FromStr implementation needs to exist or be created for Code.
        // Assuming global_hotkey provides a way to parse key names (e.g., "KeyA", "Digit1").
        Code::from_str(&s).map_err(serde::de::Error::custom)
    }
}

// Example of how FromStr might need to be implemented or used if not provided by global_hotkey
// This is illustrative and likely needs adjustment based on the actual `Code` enum.
// NOTE: This implementation needs to be comprehensive based on `global_hotkey::hotkey::Code` variants.
impl FromStr for Code {
    type Err = String; // Simple error type

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // This mapping is incomplete and depends entirely on the variants of the `Code` enum
        // provided by the `global_hotkey` crate. You'll need to consult its documentation
        // or source code to create a complete mapping.
        match s.to_uppercase().as_str() {
            "A" | "KEYA" => Ok(Code::KeyA),
            "B" | "KEYB" => Ok(Code::KeyB),
            "C" | "KEYC" => Ok(Code::KeyC),
            "D" | "KEYD" => Ok(Code::KeyD),
            "E" | "KEYE" => Ok(Code::KeyE),
            "F" | "KEYF" => Ok(Code::KeyF),
            "G" | "KEYG" => Ok(Code::KeyG),
            "H" | "KEYH" => Ok(Code::KeyH),
            "I" | "KEYI" => Ok(Code::KeyI),
            "J" | "KEYJ" => Ok(Code::KeyJ),
            "K" | "KEYK" => Ok(Code::KeyK),
            "L" | "KEYL" => Ok(Code::KeyL),
            "M" | "KEYM" => Ok(Code::KeyM),
            "N" | "KEYN" => Ok(Code::KeyN),
            "O" | "KEYO" => Ok(Code::KeyO),
            "P" | "KEYP" => Ok(Code::KeyP),
            "Q" | "KEYQ" => Ok(Code::KeyQ),
            "R" | "KEYR" => Ok(Code::KeyR),
            "S" | "KEYS" => Ok(Code::KeyS),
            "T" | "KEYT" => Ok(Code::KeyT),
            "U" | "KEYU" => Ok(Code::KeyU),
            "V" | "KEYV" => Ok(Code::KeyV),
            "W" | "KEYW" => Ok(Code::KeyW),
            "X" | "KEYX" => Ok(Code::KeyX),
            "Y" | "KEYY" => Ok(Code::KeyY),
            "Z" | "KEYZ" => Ok(Code::KeyZ),
            "1" | "DIGIT1" => Ok(Code::Digit1),
            "2" | "DIGIT2" => Ok(Code::Digit2),
            "3" | "DIGIT3" => Ok(Code::Digit3),
            "4" | "DIGIT4" => Ok(Code::Digit4),
            "5" | "DIGIT5" => Ok(Code::Digit5),
            "6" | "DIGIT6" => Ok(Code::Digit6),
            "7" | "DIGIT7" => Ok(Code::Digit7),
            "8" | "DIGIT8" => Ok(Code::Digit8),
            "9" | "DIGIT9" => Ok(Code::Digit9),
            "0" | "DIGIT0" => Ok(Code::Digit0),
            "F1" => Ok(Code::F1),
            "F2" => Ok(Code::F2),
            "F3" => Ok(Code::F3),
            "F4" => Ok(Code::F4),
            "F5" => Ok(Code::F5),
            "F6" => Ok(Code::F6),
            "F7" => Ok(Code::F7),
            "F8" => Ok(Code::F8),
            "F9" => Ok(Code::F9),
            "F10" => Ok(Code::F10),
            "F11" => Ok(Code::F11),
            "F12" => Ok(Code::F12),
            // Add mappings for all other keys supported by `global_hotkey::hotkey::Code`
            // E.g., Space, Enter, Escape, Arrow keys, Numpad keys, etc.
            "SPACE" => Ok(Code::Space),
            "ENTER" => Ok(Code::Enter),
            "ESCAPE" => Ok(Code::Escape),
            "BACKSPACE" => Ok(Code::Backspace),
            "TAB" => Ok(Code::Tab),
            "ARROWLEFT" | "LEFT" => Ok(Code::ArrowLeft),
            "ARROWRIGHT" | "RIGHT" => Ok(Code::ArrowRight),
            "ARROWUP" | "UP" => Ok(Code::ArrowUp),
            "ARROWDOWN" | "DOWN" => Ok(Code::ArrowDown),
            // ... add many more ...
            _ => Err(format!("Unknown or unsupported key code: {}", s)),
        }
    }
}

// Similarly, FromStr for Modifiers might be needed if global_hotkey doesn't provide it
// or if its format differs from what we want in the config.
impl FromStr for Modifiers {
     type Err = String;

     fn from_str(s: &str) -> Result<Self, Self::Err> {
         let mut modifiers = Modifiers::empty();
         for part in s.split('+') {
             match part.trim().to_lowercase().as_str() {
                 "shift" => modifiers |= Modifiers::SHIFT,
                 "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
                 "alt" | "option" => modifiers |= Modifiers::ALT, // Alt/Option depending on OS? Check crate docs.
                 "win" | "super" | "meta" => modifiers |= Modifiers::META, // Win/Super/Meta? Check crate docs.
                 "" => {} // Allow empty strings from splitting, e.g., "Shift+"
                 _ => return Err(format!("Unknown modifier: {}", part)),
             }
         }
         Ok(modifiers)
     }
 }