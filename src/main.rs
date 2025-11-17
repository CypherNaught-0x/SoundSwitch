// Only show console window in debug builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use simplelog::*;
use std::error::Error;
use std::fs::File; // For log file creation // Import simplelog macros and types
// use std::collections::HashMap; // Removed unused import
// use std::sync::mpsc::{channel, Receiver as MpscReceiver}; // Keep commented
use crossbeam_channel; // Restore
use log::{error, info, warn};
use std::sync::Arc; // Restore
use std::sync::atomic::{AtomicBool, Ordering}; // Restore
use std::thread;
use std::time::Duration; // Keep for sleep // Import log macros

mod audio_device;
mod config;
mod hotkey_manager;

use audio_device::{AudioDevice, list_output_devices, list_input_devices, set_default_output_device, set_default_input_device};
use config::{Config, load_config}; // Import Config struct
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState}; // Corrected import name
use hotkey_manager::register_hotkeys;
use tray_item::TrayItem;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
};
use windows_core::BOOL; // Use windows_core::BOOL as suggested by compiler // Restore tray item import

// Enum for messages between threads
enum AppMessage {
    HotkeyError(String), // Use String for thread safety
    Quit,
}

// Function to handle hotkey logic in a separate thread with a Win32 message loop
fn hotkey_listener_thread(
    config: Config,
    shutdown_signal: Arc<AtomicBool>,
    error_sender: crossbeam_channel::Sender<AppMessage>,
) {
    info!("Hotkey listener thread started."); // Log info

    // Initialize COM for this thread (required by some system APIs)
    // Revert back to Multi-Threaded Apartment (MTA)
    let hr = unsafe {
        windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        )
    };
    if hr.is_err() {
        // Check HRESULT directly
        let _ = error_sender.send(AppMessage::HotkeyError(format!(
            "Hotkey thread failed to initialize COM (MTA): {:?}",
            hr
        )));
        return;
    }
    info!("Hotkey thread COM initialized."); // Log info

    // 1. Create Hotkey Manager (must live in this thread)
    let manager = match GlobalHotKeyManager::new() {
        Ok(m) => m,
        Err(e) => {
            let _ = error_sender.send(AppMessage::HotkeyError(format!(
                "Failed to create GlobalHotKeyManager: {}",
                e
            )));
            unsafe { windows::Win32::System::Com::CoUninitialize() };
            return;
        }
    };
    info!("Hotkey manager created in thread."); // Log info

    // 2. Register Hotkeys
    let (hotkey_device_map, hotkeys) = match register_hotkeys(&manager, &config) {
        Ok((map, keys)) => {
            info!("Hotkey registration successful in thread."); // Log info
            (map, keys)
        }
        Err(e) => {
            error!("Error registering hotkeys in thread: {}", e); // Log error
            let _ = error_sender.send(AppMessage::HotkeyError(format!(
                "Failed to register hotkeys: {}",
                e
            )));
            unsafe { windows::Win32::System::Com::CoUninitialize() };
            return;
        }
    };

    // 3. Get Hotkey Event Receiver
    let receiver = GlobalHotKeyEvent::receiver();
    info!("Hotkey event listener waiting for events..."); // Log info

    // 4. Get initial list of audio devices (both output and input)
    let available_output_devices = match list_output_devices() {
        Ok(devices) => devices,
        Err(e) => {
            error!(
                "Fatal: Could not list audio output devices in thread: {}. Exiting thread.",
                e
            ); // Log error
            let _ = error_sender.send(AppMessage::HotkeyError(format!(
                "Failed to list audio output devices: {}",
                e
            )));
            unsafe { windows::Win32::System::Com::CoUninitialize() };
            return;
        }
    };
    info!("Found {} audio output devices in thread.", available_output_devices.len()); // Log info

    let available_input_devices = match list_input_devices() {
        Ok(devices) => devices,
        Err(e) => {
            error!(
                "Fatal: Could not list audio input devices in thread: {}. Exiting thread.",
                e
            ); // Log error
            let _ = error_sender.send(AppMessage::HotkeyError(format!(
                "Failed to list audio input devices: {}",
                e
            )));
            unsafe { windows::Win32::System::Com::CoUninitialize() };
            return;
        }
    };
    info!("Found {} audio input devices in thread.", available_input_devices.len()); // Log info

    // 5. Win32 Message Loop combined with Hotkey/Shutdown Check
    let mut msg = MSG::default();
    loop {
        // Check for hotkey events first (non-blocking)
        if let Ok(event) = receiver.try_recv() {
            // println!("--- DEBUG: Received hotkey event: ID={}, State={:?}", event.id, event.state); // Remove debug print
            if event.state == HotKeyState::Pressed {
                let hotkey_id = event.id;
                if let Some(mapping) = hotkey_device_map.get(&hotkey_id) {
                    info!(
                        // Log info
                        "Hotkey ID {} pressed, switching to output: '{}', input: '{:?}'",
                        hotkey_id, mapping.device_name, mapping.input_device_name
                    );
                    
                    // Switch output device
                    match find_and_set_output_device(&mapping.device_name, &available_output_devices, &config) {
                        Ok(name) => info!("Successfully set output device to {}", name), // Log info
                        Err(e) => error!("Failed to set output device: {}", e),          // Log error
                    }
                    
                    // Switch input device if specified
                    if let Some(input_device_name) = &mapping.input_device_name {
                        match find_and_set_input_device(input_device_name, &available_input_devices, &config) {
                            Ok(name) => info!("Successfully set input device to {}", name), // Log info
                            Err(e) => error!("Failed to set input device: {}", e),          // Log error
                        }
                    }
                } else {
                    warn!("Received event for unknown hotkey ID: {}", hotkey_id); // Log warning
                }
            }
        }

        // Process Windows messages (crucial for global-hotkey)
        // Use PeekMessageW for non-blocking check
        let message_handled: BOOL = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) };
        if message_handled.as_bool() {
            // println!("--- DEBUG: Processing Windows message: {:?}", msg.message); // Optional: Very verbose
            unsafe {
                let _ = TranslateMessage(&msg); // Ignore result
                DispatchMessageW(&msg);
            }
        }

        // Check for shutdown signal
        if shutdown_signal.load(Ordering::Relaxed) {
            info!("Shutdown signal received in hotkey thread. Exiting loop."); // Log info
            break;
        }

        // If no messages and no hotkey events, sleep briefly to avoid high CPU usage
        if !message_handled.as_bool() && receiver.is_empty() {
            thread::sleep(Duration::from_millis(10)); // Short sleep
        }
    }

    // Cleanup
    info!("Unregistering all hotkeys..."); // Log info
    if let Err(e) = manager.unregister_all(&hotkeys) {
        error!("Error unregistering hotkeys: {}", e); // Log error
        let _ = error_sender.send(AppMessage::HotkeyError(format!(
            "Failed to unregister hotkeys: {}",
            e
        )));
    } else {
        info!("Hotkeys unregistered successfully."); // Log info
    }

    // Uninitialize COM for this thread
    unsafe { windows::Win32::System::Com::CoUninitialize() };
    info!("Hotkey thread COM uninitialized."); // Log info

    info!("Hotkey listener thread finished."); // Log info
}

// Helper function to find and set the audio output device
fn find_and_set_output_device(
    target_device_name: &str,
    available_devices: &[AudioDevice],
    config: &Config,
) -> Result<String, Box<dyn Error>> {
    // println!("--- DEBUG: Entering find_and_set_device for target '{}'", target_device_name); // Remove debug print
    let mut found_device_id: Option<String> = None;
    let mut found_device_name: Option<String> = None;

    if config.fuzzy_match {
        // println!("--- DEBUG: Using fuzzy matching ---"); // Remove debug print
        // println!("Fuzzy matching enabled."); // Less verbose logging
        let matcher = SkimMatcherV2::default();
        let mut best_match: Option<(i64, &AudioDevice)> = None;

        for device in available_devices {
            if let Some(score) = matcher.fuzzy_match(&device.name, target_device_name) {
                if best_match.is_none() || score > best_match.unwrap().0 {
                    best_match = Some((score, device));
                }
            }
        }

        if let Some((_score, device)) = best_match {
            // Don't need score anymore
            // println!("--- DEBUG: Best fuzzy match: '{}' (Score: {}) with ID '{}'", device.name, _score, device.id); // Remove debug print
            found_device_id = Some(device.id.clone());
            found_device_name = Some(device.name.clone());
        } else {
            // println!("--- DEBUG: No fuzzy match found ---"); // Remove debug print
            return Err(format!("No fuzzy match found for '{}'", target_device_name).into());
        }
    } else {
        // println!("--- DEBUG: Using exact matching ---"); // Remove debug print
        if let Some(device) = available_devices
            .iter()
            .find(|d| &d.name == target_device_name)
        {
            // println!("--- DEBUG: Exact match found: '{}' with ID '{}'", device.name, device.id); // Remove debug print
            found_device_id = Some(device.id.clone());
            found_device_name = Some(device.name.clone());
        } else {
            // println!("--- DEBUG: No exact match found ---"); // Remove debug print
            return Err(format!("No exact match found for '{}'", target_device_name).into());
        }
    }

    if let Some(id_to_set) = &found_device_id {
        // println!("--- DEBUG: Attempting to set default device to ID '{}'", id_to_set); // Remove debug print
        set_default_output_device(id_to_set)?;
        // println!("--- DEBUG: set_default_output_device succeeded ---"); // Remove debug print
        Ok(found_device_name.unwrap_or_else(|| id_to_set.clone())) // Clone id_to_set if name is None
    } else {
        // This case should technically be handled by the Err returns above
        // println!("--- DEBUG: Error - found_device_id was None unexpectedly ---"); // Remove debug print
        Err("Device ID was determined but somehow lost".into())
    }
}

// Helper function to find and set the audio input device
fn find_and_set_input_device(
    target_device_name: &str,
    available_devices: &[AudioDevice],
    config: &Config,
) -> Result<String, Box<dyn Error>> {
    let mut found_device_id: Option<String> = None;
    let mut found_device_name: Option<String> = None;

    if config.fuzzy_match {
        let matcher = SkimMatcherV2::default();
        let mut best_match: Option<(i64, &AudioDevice)> = None;

        for device in available_devices {
            if let Some(score) = matcher.fuzzy_match(&device.name, target_device_name) {
                if best_match.is_none() || score > best_match.unwrap().0 {
                    best_match = Some((score, device));
                }
            }
        }

        if let Some((_score, device)) = best_match {
            found_device_id = Some(device.id.clone());
            found_device_name = Some(device.name.clone());
        } else {
            return Err(format!("No fuzzy match found for input device '{}'", target_device_name).into());
        }
    } else {
        if let Some(device) = available_devices
            .iter()
            .find(|d| &d.name == target_device_name)
        {
            found_device_id = Some(device.id.clone());
            found_device_name = Some(device.name.clone());
        } else {
            return Err(format!("No exact match found for input device '{}'", target_device_name).into());
        }
    }

    if let Some(id_to_set) = &found_device_id {
        set_default_input_device(id_to_set)?;
        Ok(found_device_name.unwrap_or_else(|| id_to_set.clone())) // Clone id_to_set if name is None
    } else {
        // This case should technically be handled by the Err returns above
        Err("Input device ID was determined but somehow lost".into())
    }
}

// Function to validate that configured devices exist on the system
fn validate_configured_devices(config: &Config) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut missing_output_devices = Vec::new();
    let mut missing_input_devices = Vec::new();

    // Get available devices
    let available_output_devices = match list_output_devices() {
        Ok(devices) => devices,
        Err(e) => {
            error!("Failed to list output devices during validation: {}", e);
            return (missing_output_devices, missing_input_devices, Vec::new(), Vec::new());
        }
    };

    let available_input_devices = match list_input_devices() {
        Ok(devices) => devices,
        Err(e) => {
            error!("Failed to list input devices during validation: {}", e);
            return (missing_output_devices, missing_input_devices, Vec::new(), Vec::new());
        }
    };

    // Create lists of available device names for the notification
    let available_output_names: Vec<String> = available_output_devices.iter().map(|d| d.name.clone()).collect();
    let available_input_names: Vec<String> = available_input_devices.iter().map(|d| d.name.clone()).collect();

    // Check each configured hotkey mapping
    for mapping in &config.hotkeys {
        // Check output device
        let output_found = if config.fuzzy_match {
            let matcher = SkimMatcherV2::default();
            available_output_devices.iter().any(|device| {
                matcher.fuzzy_match(&device.name, &mapping.device_name).is_some()
            })
        } else {
            available_output_devices.iter().any(|device| device.name == mapping.device_name)
        };

        if !output_found {
            let entry = format!("{} (hotkey: {})", mapping.device_name, mapping.keys);
            missing_output_devices.push(entry.clone());
            warn!("Output device not found: {}", entry);
        }

        // Check input device if specified
        if let Some(input_device_name) = &mapping.input_device_name {
            let input_found = if config.fuzzy_match {
                let matcher = SkimMatcherV2::default();
                available_input_devices.iter().any(|device| {
                    matcher.fuzzy_match(&device.name, input_device_name).is_some()
                })
            } else {
                available_input_devices.iter().any(|device| device.name == *input_device_name)
            };

            if !input_found {
                let entry = format!("{} (hotkey: {})", input_device_name, mapping.keys);
                missing_input_devices.push(entry.clone());
                warn!("Input device not found: {}", entry);
            }
        }
    }

    (missing_output_devices, missing_input_devices, available_output_names, available_input_names)
}

// Function to show a Windows notification for missing devices
fn show_missing_devices_notification(
    missing_output: &[String], 
    missing_input: &[String], 
    available_output: &[String], 
    available_input: &[String]
) {
    if missing_output.is_empty() && missing_input.is_empty() {
        return; // Nothing to show
    }

    let mut message = String::from("SoundSwitch has started but some configured devices were not found:\n\n");

    if !missing_output.is_empty() {
        message.push_str(&format!("Missing Output Device{} ({}):\n", 
            if missing_output.len() > 1 { "s" } else { "" },
            missing_output.len()));
        for device in missing_output {
            message.push_str(&format!("  • {}\n", device));
        }
        message.push('\n');
    }

    if !missing_input.is_empty() {
        message.push_str(&format!("Missing Input Device{} ({}):\n", 
            if missing_input.len() > 1 { "s" } else { "" },
            missing_input.len()));
        for device in missing_input {
            message.push_str(&format!("  • {}\n", device));
        }
        message.push('\n');
    }

    message.push_str("The application will continue to run, but these hotkeys will not work until the devices are available.\n\n");

    // Add available devices list to help with configuration
    if !missing_output.is_empty() && !available_output.is_empty() {
        message.push_str(&format!("Available Output Devices ({}):\n", available_output.len()));
        for device in available_output {
            message.push_str(&format!("  • {}\n", device));
        }
        message.push('\n');
    }

    if !missing_input.is_empty() && !available_input.is_empty() {
        message.push_str(&format!("Available Input Devices ({}):\n", available_input.len()));
        for device in available_input {
            message.push_str(&format!("  • {}\n", device));
        }
        message.push('\n');
    }

    message.push_str("Possible solutions:\n");
    message.push_str("• Check that the devices are connected and enabled in Windows Sound settings\n");
    message.push_str("• Verify the device names in your config.toml file match the available devices above\n");
    message.push_str("• Consider enabling fuzzy matching in your configuration");

    // Show Windows MessageBox
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONWARNING, MB_OK, MessageBoxW};
    use windows::core::HSTRING;

    let title = HSTRING::from("SoundSwitch - Missing Audio Devices");
    let content = HSTRING::from(message);

    unsafe {
        MessageBoxW(
            None,
            &content,
            &title,
            MB_OK | MB_ICONWARNING,
        );
    }
}

fn run_tray_app() -> Result<(), Box<dyn Error>> {
    info!("Starting SoundSwitch with Tray Icon..."); // Log info

    // 1. Load Configuration (needed for the hotkey thread)
    let config = match load_config() {
        Ok(cfg) => {
            info!("Configuration loaded successfully."); // Log info
            if cfg.hotkeys.is_empty() {
                warn!("No hotkeys defined in the configuration."); // Log warning
            }
            cfg // Return the loaded config
        }
        Err(e) => {
            // Print the specific config error and return it to exit run_tray_app
            error!("!!! Fatal: Error loading configuration: {} !!!", e); // Log error
            return Err(e); // Propagate the error
        }
    };
    // If we reach here, config loaded successfully.

    // 1.5. Validate configured devices and show notification if any are missing
    info!("Validating configured devices..."); // Log info
    let (missing_output, missing_input, available_output, available_input) = validate_configured_devices(&config);
    if !missing_output.is_empty() || !missing_input.is_empty() {
        warn!(
            "Missing devices found - Output: {:?}, Input: {:?}",
            missing_output, missing_input
        ); // Log warning
        show_missing_devices_notification(&missing_output, &missing_input, &available_output, &available_input);
    } else {
        info!("All configured devices found."); // Log info
    }

    // 2. Setup communication channels (Restore)
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let (error_sender, error_receiver) = crossbeam_channel::unbounded::<AppMessage>();

    // 3. Spawn Hotkey Listener Thread (Restore)
    let shutdown_signal_clone = Arc::clone(&shutdown_signal);
    let error_sender_clone = error_sender.clone(); // Clone sender for the thread
    let config_clone = config.clone(); // Clone config for the thread

    let hotkey_thread_handle = thread::spawn(move || {
        hotkey_listener_thread(config_clone, shutdown_signal_clone, error_sender_clone)
    });
    info!("Hotkey listener thread spawned."); // Log info

    // 4. Setup Tray Icon (Restore)
    // Use a simple placeholder icon name for now.
    // For a real icon, you'd load it from a file (e.g., .ico on Windows)
    // using `tray.set_icon(Icon::from_path("path/to/icon.ico")?)`
    let mut tray = TrayItem::new(
        "SoundSwitch",
        tray_item::IconSource::Resource("default-icon"),
    )
    .map_err(|e| format!("Failed to create tray icon: {}", e))?;
    info!("Tray icon created."); // Log info

    // Add Quit menu item
    // Use the error_sender (renamed quit_sender) for the Quit message
    let quit_sender = error_sender.clone();
    tray.add_menu_item("Quit", move || {
        info!("Quit menu item selected."); // Log info
        // Send a Quit message to the main loop to initiate shutdown
        let _ = quit_sender.send(AppMessage::Quit);
    })
    .map_err(|e| format!("Failed to add 'Quit' menu item: {}", e))?;
    info!("'Quit' menu item added."); // Log info

    // 5. Main Event Loop (Handling Tray Events and Messages from hotkey thread)
    info!("Main thread entering event loop (polling for messages)..."); // Log info
    loop {
        // Check for messages from the hotkey thread or quit callback
        match error_receiver.try_recv() {
            Ok(AppMessage::HotkeyError(err)) => {
                // Log the error string. Could potentially show a notification.
                error!("Error received from hotkey thread: {}", err); // Log error
                // Decide if the app should quit on certain errors. For now, just log.
            }
            Ok(AppMessage::Quit) => {
                info!("Quit message received. Initiating shutdown..."); // Log info
                break; // Exit the main loop to start shutdown
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {
                // No message, continue polling
            }
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                error!("Communication channel disconnected unexpectedly. Exiting."); // Log error
                // Signal shutdown just in case the hotkey thread is still running somehow
                shutdown_signal.store(true, Ordering::Relaxed);
                break; // Exit loop
            }
        }

        // Add a small sleep to prevent the loop from spinning excessively
        thread::sleep(Duration::from_millis(100));

        // Check if shutdown was requested via Quit menu
        // This check is technically redundant now as the Quit match arm breaks the loop,
        // but keep it for clarity or if other shutdown mechanisms are added.
        if shutdown_signal.load(Ordering::Relaxed) {
            warn!("Shutdown signal detected in main loop."); // Log warning (Should not happen if Quit breaks loop)
            break;
        }
    }

    // 6. Shutdown Sequence (Restore original logic)
    info!("Starting shutdown sequence..."); // Log info

    // Signal the hotkey thread to stop
    info!("Setting shutdown signal for hotkey thread..."); // Log info
    shutdown_signal.store(true, Ordering::Relaxed);

    // Wait for the hotkey thread to finish
    info!("Waiting for hotkey thread to join..."); // Log info
    match hotkey_thread_handle.join() {
        Ok(_) => info!("Hotkey thread joined successfully."), // Log info
        Err(e) => error!(
            "Error joining hotkey thread (it might have panicked): {:?}",
            e
        ), // Log error
    }

    info!("SoundSwitch application finished."); // Log info
    // println!("--- EXITING run_tray_app (Ok) ---"); // Removed debug print
    Ok(())
}

fn main() {
    let _logger = WriteLogger::init(
        LevelFilter::Info,
        ConfigBuilder::new().build(),
        File::create("sound_switch.log").unwrap(), // Create log file
    )
    .unwrap();
    // Use run_tray_app instead of run_app
    if let Err(e) = run_tray_app() {
        // Using eprintln might not be visible if the console is hidden.
        // Consider logging to a file or using a message box for errors in release.
        eprintln!("Application exited with error: {}", e);
        // For now, just print to stderr, which might go nowhere in release.
        // A message box could be used here for critical errors.
        // Example (requires enabling UI features in windows-rs):
        use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        use windows::core::w;
        unsafe {
            MessageBoxW(
                None,
                w!("Application exited with error."),
                w!("SoundSwitch Error"),
                MB_OK | MB_ICONERROR,
            );
        }
        std::process::exit(1);
    }
}
