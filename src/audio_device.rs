use windows::{
    core::{Result, PWSTR, HSTRING},
    Win32::{
        Foundation::SysAllocStringLen,
        Media::Audio::{
            eRender, eConsole, eCommunications, // Specify rendering devices and roles
            IMMDeviceEnumerator, MMDeviceEnumerator, // Device enumerator
            IMMDeviceCollection, IMMDevice, IMMEndpoint, // Device interfaces
            DEVICE_STATE_ACTIVE, // Filter for active devices
            ERole, // Enum for device roles
        },
        System::Com::{
            CoInitializeEx, CoUninitialize, CoCreateInstance,
            CLSCTX_ALL, COINIT_MULTITHREADED, // COM initialization flags
            IUnknown, // Base COM interface
        },
        UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY}, // For device properties
    },
};
use windows::core::{Interface, GUID, HRESULT, PCWSTR}; // Import PCWSTR for wide strings
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt; // For converting &str to wide strings

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
        CoInitializeEx(None, COINIT_MULTITHREADED)?; // Use multithreaded apartment

        let mut devices = Vec::new();

        // Create an instance of the device enumerator
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

        // Get the collection of active rendering devices
        let collection: IMMDeviceCollection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;

        let count = collection.GetCount()?;

        for i in 0..count {
            let device: IMMDevice = collection.Item(i)?;
            let id_pwstr: PWSTR = device.GetId()?;
            let id = id_pwstr.to_string().unwrap_or_default(); // Convert PWSTR to String
            windows::Win32::System::Com::CoTaskMemFree(Some(id_pwstr.as_ptr() as *mut _)); // Free the memory allocated by GetId

            // Get the property store for the device
            let properties: IPropertyStore = device.OpenPropertyStore(windows::Win32::System::Com::STGM_READ)?;

            // Get the friendly name property
            let prop_variant = properties.GetValue(&PKEY_DEVICE_FRIENDLY_NAME)?;

            // Extract the string value (PWSTR) from the PROPVARIANT
            // prop_variant.Anonymous.Anonymous.vt holds the type, should be VT_LPWSTR
            // prop_variant.Anonymous.Anonymous.Anonymous holds the data
            let name = if prop_variant.Anonymous.Anonymous.vt == windows::Win32::System::Variant::VT_LPWSTR {
                 prop_variant.Anonymous.Anonymous.Anonymous.pwszVal.to_string().unwrap_or_else(|_| "Invalid Name".to_string())
            } else {
                "Unknown Name".to_string()
            };

            // Important: Need to free the PROPVARIANT memory
            windows::Win32::System::Variant::PropVariantClear(&prop_variant)?;

            if !id.is_empty() && name != "Unknown Name" && name != "Invalid Name" {
                devices.push(AudioDevice { id, name });
            }
        }

        // Uninitialize COM
        CoUninitialize();

        Ok(devices)
    }
}


// --- Undocumented COM Interface: IPolicyConfigVista ---
// Found via reverse engineering / online resources. Use with caution.
#[repr(C)]
struct IPolicyConfigVistaVtbl {
    parent: windows::core::IUnknown_Vtbl,
    get_mixing_format: usize, // Placeholder, not used
    get_device_format: usize, // Placeholder, not used
    reset_device_format: usize, // Placeholder, not used
    set_device_format: usize, // Placeholder, not used
    get_processing_period: usize, // Placeholder, not used
    set_processing_period: usize, // Placeholder, not used
    get_share_mode: usize, // Placeholder, not used
    set_share_mode: usize, // Placeholder, not used
    get_property_value: usize, // Placeholder, not used
    set_property_value: usize, // Placeholder, not used
    set_default_endpoint: unsafe extern "system" fn(
        this: *mut IPolicyConfigVista,
        device_id: PCWSTR,
        role: ERole,
    ) -> HRESULT,
    set_endpoint_visibility: usize, // Placeholder, not used
}

#[windows::core::interface("568b9108-44bf-40b4-9006-86afe5b5a680")] // IID_IPolicyConfigVista
unsafe trait IPolicyConfigVista: IUnknown {
    unsafe fn set_default_endpoint(&self, device_id: PCWSTR, role: ERole) -> Result<()>;
}

// Implement the trait method using the VTable pointer
impl IPolicyConfigVista {
    unsafe fn set_default_endpoint(&self, device_id: PCWSTR, role: ERole) -> Result<()> {
        let this = self as *const *const IPolicyConfigVistaVtbl;
        let this = *this; // Dereference to get the VTable pointer
        let vtbl = &*(*this); // Dereference to get the VTable struct
        (vtbl.set_default_endpoint)(this as *mut _, device_id, role).ok()
    }
}

// CLSID_PolicyConfigClient
const POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);


/// Sets the default audio output device for Console (Multimedia) and Communications roles.
///
/// # Arguments
/// * `device_id` - The unique ID string of the device to set as default.
///
/// # Safety
/// This function uses undocumented Windows COM interfaces (`IPolicyConfigVista`).
/// It might break in future Windows updates.
pub fn set_default_output_device(device_id: &str) -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)?;

        // Convert the device ID string to a wide string (PCWSTR)
        let wide_device_id: Vec<u16> = OsStr::new(device_id)
            .encode_wide()
            .chain(std::iter::once(0)) // Null-terminate
            .collect();
        let pcwstr_device_id = PCWSTR::from_raw(wide_device_id.as_ptr());

        // Create an instance of the PolicyConfigClient
        // We expect an IPolicyConfigVista interface pointer back
        let policy_config: IPolicyConfigVista = CoCreateInstance(
            &POLICY_CONFIG_CLIENT,
            None,
            CLSCTX_ALL, // Request an in-process server
        )?;

        // Set the default device for both Console and Communications roles
        policy_config.set_default_endpoint(pcwstr_device_id, eConsole)?;
        policy_config.set_default_endpoint(pcwstr_device_id, eCommunications)?;

        CoUninitialize();
        Ok(())
    }
}