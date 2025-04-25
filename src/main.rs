// Only show console window in debug builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver as MpscReceiver}; // Renamed to avoid conflict
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration; // For polling sleep

mod audio_device;
mod config;
mod hotkey_manager;

use audio_device::{list_output_devices, set_default_output_device, AudioDevice};
use config::{load_config, Config}; // Import Config struct
use global_hotkey::{GlobalHotKeyEvent, HotKeyState, HotkeyManager}; // Import HotkeyManager
use hotkey_manager::register_hotkeys;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use tray_item::TrayItem; // Import TrayItem

// Enum for messages between threads
enum AppMessage {
    HotkeyError(Box<dyn Error + Send>),
    Quit,
}

// Function to handle hotkey logic in a separate thread
fn hotkey_listener_thread(
    config: Config,
    shutdown_signal: Arc<AtomicBool>,
    error_sender: std::sync::mpsc::Sender<AppMessage>,
) {
    println!("Hotkey listener thread started.");

    // 1. Register Hotkeys (Manager must live in this thread)
    let manager = match HotkeyManager::new() {
        Ok(m) => m,
        Err(e) => {
            let _ = error_sender.send(AppMessage::HotkeyError(Box::new(e)));
            return;
        }
    };

    let hotkey_device_map = match register_hotkeys(&manager, &config) {
        Ok(map) => {
            println!("Hotkey registration successful in thread.");
            map
        }
        Err(e) => {
            eprintln!("Error registering hotkeys in thread: {}", e);
            // Send error back to main thread to potentially notify user
            let _ = error_sender.send(AppMessage::HotkeyError(e));
            return; // Stop thread if registration fails
        }
    };

    // 2. Setup Event Receiver
    let receiver = GlobalHotKeyEvent::receiver();
    println!("Hotkey event listener waiting for events...");

    // 3. Get initial list of audio devices
    let available_devices = match list_output_devices() {
        Ok(devices) => devices,
        Err(e) => {
            eprintln!("Fatal: Could not list audio output devices in thread: {}. Exiting thread.", e);
            let _ = error_sender.send(AppMessage::HotkeyError(e));
            return;
        }
    };
    println!("Found {} audio devices in thread.", available_devices.len());


    // 4. Event Loop
    loop {
        // Check for shutdown signal periodically using recv_timeout
        match receiver.recv_timeout(Duration::from_millis(250)) {
            Ok(event) => {
                if event.state == HotKeyState::Pressed {
                    let hotkey_id = event.id;
                    if let Some(target_device_name) = hotkey_device_map.get(&hotkey_id) {
                        println!(
                            "Hotkey ID {} pressed, switching to '{}'",
                            hotkey_id, target_device_name
                        );
                        match find_and_set_device(target_device_name, &available_devices, &config) {
                            Ok(name) => println!("Successfully set device to {}", name),
                            Err(e) => eprintln!("Failed to set device: {}", e), // Log error but continue
                        }
                    } else {
                        eprintln!("Warning: Received event for unknown hotkey ID: {}", hotkey_id);
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Timeout is expected, check shutdown signal
                if shutdown_signal.load(Ordering::Relaxed) {
                    println!("Shutdown signal received in hotkey thread. Exiting loop.");
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                eprintln!("Hotkey channel disconnected. Exiting thread.");
                let _ = error_sender.send(AppMessage::HotkeyError(
                    "Hotkey event channel disconnected".into(),
                ));
                break;
            }
        }
         // Check shutdown signal again after processing an event or timeout
         if shutdown_signal.load(Ordering::Relaxed) {
            println!("Shutdown signal received in hotkey thread. Exiting loop.");
            break;
        }
    }

    // Cleanup: Unregister hotkeys before the thread exits
    println!("Unregistering all hotkeys...");
    if let Err(e) = manager.unregister_all() {
        eprintln!("Error unregistering hotkeys: {}", e);
        // Send error back? Maybe not critical if we are quitting anyway.
        let _ = error_sender.send(AppMessage::HotkeyError(Box::new(e)));
    } else {
        println!("Hotkeys unregistered successfully.");
    }

    println!("Hotkey listener thread finished.");
}

// Helper function to find and set the audio device
fn find_and_set_device(
    target_device_name: &str,
    available_devices: &[AudioDevice],
    config: &Config,
) -> Result<String, Box<dyn Error>> {
    let mut found_device_id: Option<String> = None;
    let mut found_device_name: Option<String> = None;

    if config.fuzzy_match {
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
            // println!("  Best fuzzy match: '{}' (Score: {})", device.name, score);
            found_device_id = Some(device.id.clone());
            found_device_name = Some(device.name.clone());
        } else {
            return Err(format!("No fuzzy match found for '{}'", target_device_name).into());
        }
    } else {
        // println!("Exact matching enabled."); // Less verbose logging
        if let Some(device) = available_devices.iter().find(|d| &d.name == target_device_name) {
            // println!("  Exact match found: '{}'", device.name);
            found_device_id = Some(device.id.clone());
            found_device_name = Some(device.name.clone());
        } else {
            return Err(format!("No exact match found for '{}'", target_device_name).into());
        }
    }

    if let Some(id_to_set) = found_device_id {
        set_default_output_device(&id_to_set)?;
        Ok(found_device_name.unwrap_or_else(|| id_to_set)) // Return name if found, else ID
    } else {
        // This case should technically be handled by the Err returns above
        Err("Device ID was determined but somehow lost".into())
    }
}


fn run_tray_app() -> Result<(), Box<dyn Error>> {
    println!("Starting SoundSwitch with Tray Icon...");

    // 1. Load Configuration (needed for the hotkey thread)
    let config = load_config().map_err(|e| {
        eprintln!("Fatal: Error loading configuration: {}. Exiting.", e);
        e
    })?;
    println!("Configuration loaded successfully.");
     if config.hotkeys.is_empty() {
        println!("Warning: No hotkeys defined in the configuration.");
    }


    // 2. Setup communication channels
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let (error_sender, error_receiver): (
        std::sync::mpsc::Sender<AppMessage>,
        MpscReceiver<AppMessage>, // Use renamed type
    ) = channel();

    // 3. Spawn Hotkey Listener Thread
    let shutdown_signal_clone = Arc::clone(&shutdown_signal);
    let error_sender_clone = error_sender.clone(); // Clone sender for the thread
    let config_clone = config.clone(); // Clone config for the thread

    let hotkey_thread_handle = thread::spawn(move || {
        hotkey_listener_thread(config_clone, shutdown_signal_clone, error_sender_clone);
    });
    println!("Hotkey listener thread spawned.");


    // 4. Setup Tray Icon
    // Use a simple placeholder icon name for now.
    // For a real icon, you'd load it from a file (e.g., .ico on Windows)
    // using `tray.set_icon(Icon::from_path("path/to/icon.ico")?)`
    let mut tray = TrayItem::new("SoundSwitch", tray_item::IconSource::Resource("default-icon"))?;
    println!("Tray icon created.");

    // Add Quit menu item
    let quit_sender = error_sender.clone(); // Clone sender for the quit callback
    tray.add_menu_item("Quit", move || {
        println!("Quit menu item selected.");
        // Send a Quit message to the main loop to initiate shutdown
        let _ = quit_sender.send(AppMessage::Quit);
    })?;
    println!("'Quit' menu item added.");


    // 5. Main Event Loop (Handling Tray Events and Messages)
    println!("Main thread entering event loop (polling for messages)...");
    loop {
        // Check for messages from the hotkey thread or quit callback
        match error_receiver.try_recv() {
            Ok(AppMessage::HotkeyError(err)) => {
                // Log the error. Could potentially show a notification.
                eprintln!("Error received from hotkey thread: {}", err);
                // Decide if the app should quit on certain errors. For now, just log.
                // If it was a critical init error, the thread might have already stopped.
            }
            Ok(AppMessage::Quit) => {
                println!("Quit message received. Initiating shutdown...");
                break; // Exit the main loop to start shutdown
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // No message, continue polling
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                eprintln!("Error: Communication channel disconnected unexpectedly. Exiting.");
                // Signal shutdown just in case the hotkey thread is still running somehow
                shutdown_signal.store(true, Ordering::Relaxed);
                break; // Exit loop
            }
        }

        // Add a small sleep to prevent the loop from spinning excessively
        // Note: tray-item doesn't seem to have its own blocking event loop,
        // so we poll. Adjust sleep duration as needed.
        thread::sleep(Duration::from_millis(100));

        // Check if the hotkey thread has panicked or exited unexpectedly
        if hotkey_thread_handle.is_finished() {
             eprintln!("Warning: Hotkey listener thread has finished unexpectedly.");
             // Attempt to join to get potential panic message (might block)
             match hotkey_thread_handle.join() {
                 Ok(_) => eprintln!("Hotkey thread joined cleanly after finishing early."),
                 Err(e) => eprintln!("Hotkey thread panicked: {:?}", e),
             }
             // Decide whether to exit the main app here. Let's exit for safety.
             shutdown_signal.store(true, Ordering::Relaxed); // Ensure signal is set
             break;
        }
    }

    // 6. Shutdown Sequence
    println!("Starting shutdown sequence...");

    // Signal the hotkey thread to stop
    println!("Setting shutdown signal for hotkey thread...");
    shutdown_signal.store(true, Ordering::Relaxed);

    // Wait for the hotkey thread to finish
    println!("Waiting for hotkey thread to join...");
    // Re-acquire handle if it was moved in the is_finished check (it wasn't)
    match hotkey_thread_handle.join() {
        Ok(_) => println!("Hotkey thread joined successfully."),
        Err(e) => eprintln!("Error joining hotkey thread (it might have panicked): {:?}", e),
    }

    println!("SoundSwitch application finished.");
    Ok(())
}


fn main() {
    // Use run_tray_app instead of run_app
    if let Err(e) = run_tray_app() {
        // Using eprintln might not be visible if the console is hidden.
        // Consider logging to a file or using a message box for errors in release.
        eprintln!("Application exited with error: {}", e);
        // For now, just print to stderr, which might go nowhere in release.
        // A message box could be used here for critical errors.
        // Example (requires enabling UI features in windows-rs):
        // use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_OK, MB_ICONERROR};
        // use windows::core::w;
        // unsafe {
        //     MessageBoxW(None, w!("Application exited with error."), w!("SoundSwitch Error"), MB_OK | MB_ICONERROR);
        // }
        std::process::exit(1);
    }
}
