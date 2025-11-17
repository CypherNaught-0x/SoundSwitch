use log::{error, info};
use std::os::windows::process::CommandExt; // Import the extension trait
use std::process::Command; // Import logging macros
// use windows::core; // Keep commented unless needed elsewhere
// use windows::core::{GUID, PCWSTR}; // Remove unused GUID, PCWSTR
use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
// Import PCWSTR for wide strings
use windows::{
    Win32::{
        Foundation::PROPERTYKEY,
        // Foundation::SysAllocStringLen, // Removed unused import
        Media::Audio::{
            DEVICE_STATE_ACTIVE, // Filter for active devices
            // ERole,               // Removed - No longer needed
            IMMDevice, // Removed unused IMMEndpoint
            IMMDeviceCollection,
            IMMDeviceEnumerator,
            MMDeviceEnumerator, // Device enumerator
            // eCommunications,    // Removed - No longer needed
            // eConsole,           // Removed - No longer needed
            eRender,
            eCapture, // Added for input devices
        },
        System::Com::{
            CLSCTX_ALL,
            COINIT_MULTITHREADED, // COM initialization flags
            // IUnknown, // Moved to windows::core
            CoCreateInstance,
            CoInitializeEx,
            CoUninitialize,
        },
        UI::Shell::PropertiesSystem::IPropertyStore, // For device properties
    },
    core::{PWSTR, Result}, // Keep Result for list_output_devices
}; // For converting &str to wide strings

// Define a structure to hold device information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

// PKEY_Device_FriendlyName
const PKEY_DEVICE_FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
    fmtid: windows::core::GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};

/// Enumerates active audio output (rendering) devices.
pub fn list_output_devices() -> Result<Vec<AudioDevice>> {
    unsafe {
        // Initialize COM for this thread
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED); // Use multithreaded apartment

        let mut devices = Vec::new();

        // Create an instance of the device enumerator
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

        // Get the collection of active rendering devices
        let collection: IMMDeviceCollection =
            enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;

        let count = collection.GetCount()?;

        for i in 0..count {
            let device: IMMDevice = collection.Item(i)?;
            let id_pwstr: PWSTR = device.GetId()?;
            let id = id_pwstr.to_string().unwrap_or_default(); // Convert PWSTR to String
            windows::Win32::System::Com::CoTaskMemFree(Some(id_pwstr.as_ptr() as *mut _)); // Free the memory allocated by GetId

            // Get the property store for the device
            let properties: IPropertyStore =
                device.OpenPropertyStore(windows::Win32::System::Com::STGM_READ)?;

            // Get the friendly name property
            let prop_variant = properties.GetValue(&PKEY_DEVICE_FRIENDLY_NAME)?;

            // Extract the string value (PWSTR) from the PROPVARIANT
            // prop_variant.Anonymous.Anonymous.vt holds the type, should be VT_LPWSTR
            // prop_variant.Anonymous.Anonymous.Anonymous holds the data
            let name = if prop_variant.Anonymous.Anonymous.vt
                == windows::Win32::System::Variant::VT_LPWSTR
            {
                prop_variant
                    .Anonymous
                    .Anonymous
                    .Anonymous
                    .pwszVal
                    .to_string()
                    .unwrap_or_else(|_| "Invalid Name".to_string())
            } else {
                "Unknown Name".to_string()
            };

            // Important: Need to free the PROPVARIANT memory
            // PropVariantClear is often in Com::StructuredStorage or just Com
            PropVariantClear((&prop_variant) as *const _ as *mut _)?;

            if !id.is_empty() && name != "Unknown Name" && name != "Invalid Name" {
                devices.push(AudioDevice { id, name });
            }
        }

        // Uninitialize COM
        CoUninitialize();

        Ok(devices)
    }
}

/// Enumerates active audio input (capture) devices.
pub fn list_input_devices() -> Result<Vec<AudioDevice>> {
    unsafe {
        // Initialize COM for this thread
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED); // Use multithreaded apartment

        let mut devices = Vec::new();

        // Create an instance of the device enumerator
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

        // Get the collection of active capture devices
        let collection: IMMDeviceCollection =
            enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)?;

        let count = collection.GetCount()?;

        for i in 0..count {
            let device: IMMDevice = collection.Item(i)?;
            let id_pwstr: PWSTR = device.GetId()?;
            let id = id_pwstr.to_string().unwrap_or_default(); // Convert PWSTR to String
            windows::Win32::System::Com::CoTaskMemFree(Some(id_pwstr.as_ptr() as *mut _)); // Free the memory allocated by GetId

            // Get the property store for the device
            let properties: IPropertyStore =
                device.OpenPropertyStore(windows::Win32::System::Com::STGM_READ)?;

            // Get the friendly name property
            let prop_variant = properties.GetValue(&PKEY_DEVICE_FRIENDLY_NAME)?;

            // Extract the string value (PWSTR) from the PROPVARIANT
            let name = if prop_variant.Anonymous.Anonymous.vt
                == windows::Win32::System::Variant::VT_LPWSTR
            {
                prop_variant
                    .Anonymous
                    .Anonymous
                    .Anonymous
                    .pwszVal
                    .to_string()
                    .unwrap_or_else(|_| "Invalid Name".to_string())
            } else {
                "Unknown Name".to_string()
            };

            // Important: Need to free the PROPVARIANT memory
            PropVariantClear((&prop_variant) as *const _ as *mut _)?;

            if !id.is_empty() && name != "Unknown Name" && name != "Invalid Name" {
                devices.push(AudioDevice { id, name });
            }
        }

        // Uninitialize COM
        CoUninitialize();

        Ok(devices)
    }
}

// --- Undocumented COM Interface Definitions Removed ---

/// Sets the default audio output device using PowerShell's Set-AudioDevice cmdlet.
///
/// # Arguments
/// * `device_id` - The unique ID string of the device to set as default.
///
/// # Notes
/// - Requires PowerShell 5.1 or later.
/// - May require the user to install the `AudioDeviceCmdlets` module:
///   `Install-Module -Name AudioDeviceCmdlets -Scope CurrentUser`
/// - Hides the PowerShell window during execution.
// Use standard library Result and Box<dyn Error> for flexibility
pub fn set_default_output_device(
    device_id: &str,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let escaped_device_id = device_id.replace('\'', "''");

    // --- Get path to bundled module manifest ---
    let mut module_manifest_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;
    module_manifest_path.pop(); // Remove executable name
    module_manifest_path.push("modules");
    module_manifest_path.push("AudioDeviceCmdlets");
    module_manifest_path.push("AudioDeviceCmdlets.psd1"); // Directly point to the manifest

    // Check if the constructed path actually exists before proceeding
    if !module_manifest_path.exists() {
        return Err(format!("Bundled module manifest not found at expected path: {}", module_manifest_path.display()).into());
    }

    let module_path_str = module_manifest_path.to_str()
        .ok_or("Failed to convert module path to string")?;
    // Escape path for PowerShell command
    let escaped_module_path = module_path_str.replace('\'', "''");
    // --- End get path ---


    // Construct the PowerShell command: Import using full path, then run Set-AudioDevice
    let command_str = format!(
        // Use single quotes around the path in PowerShell
        "Import-Module -Name '{}' -ErrorAction Stop; Set-AudioDevice -ID '{}'",
        escaped_module_path,
        escaped_device_id
    );

    info!("Executing PowerShell: {}", command_str); // Log info

    // Execute the command using powershell.exe
    const CREATE_NO_WINDOW: u32 = 0x08000000; // Define flag to hide window
    let output = Command::new("powershell.exe")
        .creation_flags(CREATE_NO_WINDOW) // Set the flag to prevent window creation
        // Arguments to hide window and run command
        .args(&[
            "-NoProfile",      // Don't load user profile
            "-NonInteractive", // Don't require user interaction
            "-WindowStyle", "Hidden", // Hide the window
            "-Command", &command_str, // Use the new command string
        ])
        .output() // Capture stdout/stderr/status
        .map_err(|e| format!("Failed to execute PowerShell command: {}", e))?; // This ? now works with Box<dyn Error>

    // Check the exit status
    if output.status.success() {
        info!("PowerShell command succeeded."); // Log info
        Ok(())
    } else {
        // Combine stdout and stderr for error message
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let err_msg = format!(
            "PowerShell command failed with status: {}. Stdout: '{}'. Stderr: '{}'",
            output.status,
            stdout.trim(),
            stderr.trim()
        );
        error!("{}", err_msg); // Log error
        Err(err_msg.into()) // This .into() correctly converts String to Box<dyn Error>
    }
}

/// Sets the default audio input device using PowerShell's Set-AudioDevice cmdlet.
///
/// # Arguments
/// * `device_id` - The unique ID string of the device to set as default input.
///
/// # Notes
/// - Requires PowerShell 5.1 or later.
/// - May require the user to install the `AudioDeviceCmdlets` module:
///   `Install-Module -Name AudioDeviceCmdlets -Scope CurrentUser`
/// - Hides the PowerShell window during execution.
pub fn set_default_input_device(
    device_id: &str,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let escaped_device_id = device_id.replace('\'', "''");

    // --- Get path to bundled module manifest ---
    let mut module_manifest_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;
    module_manifest_path.pop(); // Remove executable name
    module_manifest_path.push("modules");
    module_manifest_path.push("AudioDeviceCmdlets");
    module_manifest_path.push("AudioDeviceCmdlets.psd1"); // Directly point to the manifest

    // Check if the constructed path actually exists before proceeding
    if !module_manifest_path.exists() {
        return Err(format!("Bundled module manifest not found at expected path: {}", module_manifest_path.display()).into());
    }

    let module_path_str = module_manifest_path.to_str()
        .ok_or("Failed to convert module path to string")?;
    // Escape path for PowerShell command
    let escaped_module_path = module_path_str.replace('\'', "''");
    // --- End get path ---

    // Construct the PowerShell command: Import using full path, then run Set-AudioDevice with -RecordingDevice flag
    let command_str = format!(
        // Use single quotes around the path in PowerShell
        "Import-Module -Name '{}' -ErrorAction Stop; Set-AudioDevice -ID '{}' -RecordingDevice",
        escaped_module_path,
        escaped_device_id
    );

    info!("Executing PowerShell for input device: {}", command_str); // Log info

    // Execute the command using powershell.exe
    const CREATE_NO_WINDOW: u32 = 0x08000000; // Define flag to hide window
    let output = Command::new("powershell.exe")
        .creation_flags(CREATE_NO_WINDOW) // Set the flag to prevent window creation
        // Arguments to hide window and run command
        .args(&[
            "-NoProfile",      // Don't load user profile
            "-NonInteractive", // Don't require user interaction
            "-WindowStyle", "Hidden", // Hide the window
            "-Command", &command_str, // Use the new command string
        ])
        .output() // Capture stdout/stderr/status
        .map_err(|e| format!("Failed to execute PowerShell command for input device: {}", e))?;

    // Check the exit status
    if output.status.success() {
        info!("PowerShell command for input device succeeded."); // Log info
        Ok(())
    } else {
        // Combine stdout and stderr for error message
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let err_msg = format!(
            "PowerShell command for input device failed with status: {}. Stdout: '{}'. Stderr: '{}'",
            output.status,
            stdout.trim(),
            stderr.trim()
        );
        error!("{}", err_msg); // Log error
        Err(err_msg.into()) // This .into() correctly converts String to Box<dyn Error>
    }
}

// Removed unused helper function find_module_manifest
