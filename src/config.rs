use log::info;
use serde::Deserialize;
use std::{error::Error, fs};
// Assuming global_hotkey uses these types. Adjust if necessary based on the actual crate API.
// If global_hotkey doesn't expose Modifiers/Code directly for config,
// or a simpler string representation initially. For now, let's assume direct use is possible or we define placeholders.
// Re-checking global_hotkey docs: It uses `Modifiers` and `Key`. Let's use those.
// Update: Using Code and Modifiers from global_hotkey::hotkey
// use global_hotkey::hotkey::{Code, Modifiers}; // Removed unused imports

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct HotkeyMapping {
    // Deserialize the hotkey combination as a single string first
    pub keys: String,
    // Modifiers and Code will be parsed later in hotkey_manager
    // pub modifiers: Modifiers, // Removed
    // pub key: Code, // Removed
    pub device_name: String,
    // Optional input device to switch to when switching output
    pub input_device_name: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    #[serde(default)] // Defaults to false if not present
    pub fuzzy_match: bool,
    #[serde(default)] // Defaults to an empty vec if not present
    pub hotkeys: Vec<HotkeyMapping>,
}

/// Loads configuration from `config.toml`.
/// It first looks next to the executable, then falls back to the current working directory.
pub fn load_config() -> Result<Config, Box<dyn Error>> {
    let exe_dir = std::env::current_exe()?
        .parent()
        .ok_or("Failed to get parent directory of executable")?
        .to_path_buf();

    let mut config_path_exe = exe_dir.clone();
    config_path_exe.push("config.toml");

    let mut config_path_cwd = std::env::current_dir()?;
    config_path_cwd.push("config.toml");

    let config_path_to_use = if config_path_exe.exists() {
        config_path_exe
    } else if config_path_cwd.exists() {
        // Fallback for running with `cargo run` where cwd is project root
        config_path_cwd
    } else {
        // Neither exists, return error with helpful guidance
        return Err(format!(
            "Config file 'config.toml' not found!\n\n\
            Searched in:\n\
            1. Next to executable: {}\n\
            2. Current working directory: {}\n\n\
            Please create a config.toml file in one of these locations.\n\
            Use config.toml.example as a template if available.",
            config_path_exe.display(),
            config_path_cwd.display()
        )
        .into());
    };

    info!(
        "Attempting to load config from: {}",
        config_path_to_use.display()
    );

    let config_content = fs::read_to_string(&config_path_to_use) // Use the correct variable here
        .map_err(|e| {
            format!(
                "Failed to read config file at {}: {}",
                config_path_to_use.display(),
                e
            )
        })?;

    let config: Config = toml::from_str(&config_content)
        .map_err(|e| format!("Failed to parse TOML config: {}", e))?;

    Ok(config)
}

// Removed unused function get_executable_dir

// --- Removed serde helpers and FromStr implementations ---
// Parsing logic moved to hotkey_manager.rs
