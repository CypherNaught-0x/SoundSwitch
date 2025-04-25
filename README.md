# Sound Switch

A simple Rust application for Windows that allows switching the default audio output device using globally configured hotkeys.

## Features

*   **Global Hotkeys:** Define custom key combinations to switch to specific audio devices.
*   **Configurable Devices:** Map hotkeys to target audio device names in a configuration file.
*   **Fuzzy Matching:** Optionally enable fuzzy matching for device names if the exact name isn't known or contains variable elements.
*   **Background Operation:** Runs silently in the background with a system tray icon.
*   **System Tray Control:** Provides a "Quit" option in the system tray menu to cleanly exit the application.

## Configuration

The application requires a `config.toml` file located in the same directory as the executable (`sound_switch.exe`).

**Example `config.toml`:**

```toml
# Set to true to enable fuzzy matching for device names, false for exact matching.
fuzzy_match = true

# Define your hotkey mappings here.
# 'keys' uses a format like "Modifier+Modifier+Key" (e.g., "Ctrl+Shift+F1", "Alt+1").
# Supported modifiers: Ctrl, Alt, Shift, Win (Super/Meta).
# See the 'global_hotkey' crate documentation for specific key names.
# 'device' is the friendly name of the audio output device as shown in Windows Sound settings.
[[hotkeys]]
keys = "Ctrl+Alt+1"
device = "Speakers (Realtek High Definition Audio)"

[[hotkeys]]
keys = "Ctrl+Alt+2"
device = "Headset (HyperX Cloud II Wireless)"

[[hotkeys]]
keys = "Ctrl+Alt+F4"
device = "DELL U2719DC (NVIDIA High Definition Audio)"
```

**Finding Device Names:**
You can find the exact names of your audio output devices in the Windows Sound settings panel.

## Building

1.  Ensure you have Rust and Cargo installed ([https://rustup.rs/](https://rustup.rs/)).
2.  Clone the repository (if applicable) or navigate to the project directory.
3.  Build the release executable:
    ```bash
    cargo build --release
    ```
    The executable will be located at `target/release/sound_switch.exe`.

## Running

1.  Create the `config.toml` file as described above and place it in the `target/release/` directory alongside `sound_switch.exe`.
2.  Double-click `sound_switch.exe` to run it.
3.  The application will start in the background. Look for its icon in the system tray.
4.  Press your configured hotkeys to switch audio devices.
5.  Right-click the tray icon and select "Quit" to stop the application.

## Dependencies

This project relies on several Rust crates, including:

*   `windows-rs`: For interacting with Windows APIs (Core Audio).
*   `global_hotkey`: For registering and listening to global hotkeys.
*   `tray-item`: For creating the system tray icon and menu.
*   `toml`: For parsing the configuration file.
*   `serde`: For configuration deserialization.
*   `fuzzy-matcher`: For fuzzy string matching of device names.

## Platform

This application is designed specifically for **Windows** due to its reliance on Windows-specific APIs for audio device control and hotkeys.