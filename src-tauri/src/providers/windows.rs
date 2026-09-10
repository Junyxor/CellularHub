use std::{env, path::PathBuf, process::Command};

use crate::model::{CellularDevice, RuntimeCapabilities};

pub fn enumerate_devices() -> Vec<CellularDevice> {
    #[cfg(windows)]
    {
        native::enumerate_devices().unwrap_or_default()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

pub fn detect_runtime_capabilities() -> RuntimeCapabilities {
    RuntimeCapabilities {
        native_lpa_available: native_lpa_handler_available(),
        // The bridge stays false until a real eUICC and usable APDU transport are proven.
        euicc_bridge_available: false,
        lpac_found: find_lpac().is_some(),
        platform: if cfg!(windows) {
            "windows".into()
        } else {
            env::consts::OS.into()
        },
    }
}

pub fn launch_native_lpa(code: &str) -> Result<String, String> {
    if !cfg!(windows) {
        return Err("Windows LPA is only available on Windows".into());
    }
    if !native_lpa_handler_available() {
        return Err("Windows does not expose the lpa: URI handler; Windows 11 24H2+ is required for this path".into());
    }

    let normalized = normalize_activation_code(code)?;
    Command::new("explorer.exe")
        .arg(format!("lpa:{normalized}"))
        .spawn()
        .map_err(|e| format!("failed to open Windows LPA handler: {e}"))?;
    Ok("Activation code was handed to the Windows LPA handler".into())
}

fn native_lpa_handler_available() -> bool {
    #[cfg(windows)]
    {
        Command::new("reg.exe")
            .args(["query", r"HKCR\lpa"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn normalize_activation_code(code: &str) -> Result<String, String> {
    let trimmed = code.trim();
    if trimmed.is_empty() {
        return Err("activation code is empty".into());
    }
    if let Some(rest) = trimmed.strip_prefix("LPA:") {
        return Ok(rest.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("lpa:") {
        return Ok(rest.to_string());
    }
    if trimmed.starts_with("1$") {
        return Ok(trimmed.to_string());
    }
    Err("expected an LPA: URI or 1$ activation code".into())
}

fn find_lpac() -> Option<PathBuf> {
    let candidates = [PathBuf::from("lpac.exe"), PathBuf::from("tools/lpac.exe")];
    if let Some(found) = candidates.into_iter().find(|path| path.exists()) {
        return Some(found);
    }
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|path| path.join("lpac.exe"))
            .find(|candidate| candidate.exists())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_lpa_codes_without_shell_interpolation() {
        assert_eq!(normalize_activation_code("LPA:1$example.test$ABC").unwrap(), "1$example.test$ABC");
        assert_eq!(normalize_activation_code("1$example.test$ABC").unwrap(), "1$example.test$ABC");
        assert!(normalize_activation_code("https://example.test").is_err());
    }
}

#[cfg(windows)]
mod native {
    use std::{ffi::c_void, mem::ManuallyDrop, ptr::null_mut};

    use windows::{
        core::{BSTR, Interface, IUnknown},
        Win32::{
            Foundation::RPC_E_CHANGED_MODE,
            NetworkManagement::MobileBroadband::{
                IMbnInterface, IMbnInterfaceManager, IMbnRegistration, IMbnSignal,
                MbnInterfaceManager, MBN_INTERFACE_CAPS, MBN_SMS_CAPS_PDU_RECEIVE,
            },
            System::{
                Com::{
                    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL,
                    COINIT_APARTMENTTHREADED, SAFEARRAY,
                },
                Ole::{SafeArrayDestroy, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound},
            },
        },
    };

    use crate::model::{CellularDevice, DeviceCapabilities};

    struct ComApartment {
        must_uninitialize: bool,
    }

    impl ComApartment {
        fn enter() -> windows::core::Result<Self> {
            let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            if hr.is_ok() {
                Ok(Self {
                    must_uninitialize: true,
                })
            } else if hr == RPC_E_CHANGED_MODE {
                // Tauri/WebView may already have selected a different apartment model.
                // COM is still initialized on this thread; do not balance it with CoUninitialize.
                Ok(Self {
                    must_uninitialize: false,
                })
            } else {
                Err(hr.into())
            }
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            if self.must_uninitialize {
                unsafe { CoUninitialize() };
            }
        }
    }

    struct SafeArrayGuard(*mut SAFEARRAY);

    impl Drop for SafeArrayGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                let _ = unsafe { SafeArrayDestroy(self.0) };
            }
        }
    }

    struct InterfaceCaps(MBN_INTERFACE_CAPS);

    impl InterfaceCaps {
        fn load(interface: &IMbnInterface) -> windows::core::Result<Self> {
            let mut caps = Self(MBN_INTERFACE_CAPS::default());
            unsafe { interface.GetInterfaceCapability(&mut caps.0)? };
            Ok(caps)
        }
    }

    impl Drop for InterfaceCaps {
        fn drop(&mut self) {
            unsafe {
                ManuallyDrop::drop(&mut self.0.customDataClass);
                ManuallyDrop::drop(&mut self.0.customBandClass);
                ManuallyDrop::drop(&mut self.0.deviceID);
                ManuallyDrop::drop(&mut self.0.manufacturer);
                ManuallyDrop::drop(&mut self.0.model);
                ManuallyDrop::drop(&mut self.0.firmwareInfo);
            }
        }
    }

    pub fn enumerate_devices() -> windows::core::Result<Vec<CellularDevice>> {
        let _apartment = ComApartment::enter()?;
        let manager: IMbnInterfaceManager =
            unsafe { CoCreateInstance(&MbnInterfaceManager, None, CLSCTX_ALL)? };
        let raw_array = unsafe { manager.GetInterfaces()? };
        if raw_array.is_null() {
            return Ok(Vec::new());
        }
        let array = SafeArrayGuard(raw_array);
        let lower = unsafe { SafeArrayGetLBound(array.0, 1)? };
        let upper = unsafe { SafeArrayGetUBound(array.0, 1)? };
        if upper < lower {
            return Ok(Vec::new());
        }

        let mut devices = Vec::with_capacity((upper - lower + 1) as usize);
        for index in lower..=upper {
            let mut raw: *mut c_void = null_mut();
            unsafe {
                SafeArrayGetElement(
                    array.0,
                    &index,
                    (&mut raw as *mut *mut c_void).cast::<c_void>(),
                )?;
            }
            if raw.is_null() {
                continue;
            }

            let unknown = unsafe { IUnknown::from_raw(raw) };
            let interface: IMbnInterface = unknown.cast()?;
            if let Ok(device) = describe_interface(&interface) {
                devices.push(device);
            }
        }
        Ok(devices)
    }

    fn describe_interface(interface: &IMbnInterface) -> windows::core::Result<CellularDevice> {
        let caps = InterfaceCaps::load(interface)?;
        let id = unsafe { interface.InterfaceID()? };
        let id = bstr_text(&id).unwrap_or_else(|| "unknown-mbn-interface".into());
        let manufacturer = bstr_text(&caps.0.manufacturer);
        let model = bstr_text(&caps.0.model);
        let label = match (&manufacturer, &model) {
            (Some(manufacturer), Some(model)) => format!("{manufacturer} {model}"),
            (Some(manufacturer), None) => manufacturer.clone(),
            (None, Some(model)) => model.clone(),
            (None, None) => "Windows Mobile Broadband".into(),
        };

        let registration = interface.cast::<IMbnRegistration>().ok();
        let operator = registration
            .as_ref()
            .and_then(|value| unsafe { value.GetProviderName().ok() })
            .and_then(|value| bstr_text(&value));
        let network_class = registration
            .as_ref()
            .and_then(|value| unsafe { value.GetCurrentDataClass().ok() })
            .and_then(data_class_label);
        let signal = interface
            .cast::<IMbnSignal>()
            .ok()
            .and_then(|value| unsafe { value.GetSignalStrength().ok() })
            .and_then(signal_percent);

        let status = registration
            .as_ref()
            .and_then(|value| unsafe { value.GetRegisterState().ok() })
            .map(|state| registration_status(state.0))
            .unwrap_or_else(|| {
                let ready = unsafe { interface.GetReadyState().ok() }.map(|state| state.0);
                match ready {
                    Some(1) => "available",
                    _ => "unavailable",
                }
            })
            .to_string();

        Ok(CellularDevice {
            id: format!("mbn:{id}"),
            label,
            kind: "windows-mbn".into(),
            model,
            operator,
            signal,
            network_class,
            status,
            capabilities: DeviceCapabilities {
                sms_receive: caps.0.smsCaps & (MBN_SMS_CAPS_PDU_RECEIVE.0 as u32) != 0,
                // v1.0 deliberately keeps outbound SMS disabled even if the modem advertises it.
                sms_send: false,
                // A Windows MBN interface is not proof that this particular device has an eUICC.
                native_esim: false,
                euicc_bridge: false,
            },
        })
    }

    fn bstr_text(value: &BSTR) -> Option<String> {
        String::try_from(value)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn signal_percent(raw: u32) -> Option<u8> {
        (raw <= 31).then(|| ((raw * 100 + 15) / 31) as u8)
    }

    fn registration_status(raw: i32) -> &'static str {
        match raw {
            // HOME, ROAMING, PARTNER
            3 | 4 | 5 => "connected",
            // DEREGISTERED, SEARCHING
            1 | 2 => "available",
            _ => "unavailable",
        }
    }

    fn data_class_label(mask: u32) -> Option<String> {
        let label = if mask & 0x20 != 0 {
            "LTE"
        } else if mask & 0x10 != 0 {
            "HSUPA"
        } else if mask & 0x08 != 0 {
            "HSDPA"
        } else if mask & 0x04 != 0 {
            "UMTS"
        } else if mask & 0x02 != 0 {
            "EDGE"
        } else if mask & 0x01 != 0 {
            "GPRS"
        } else if mask != 0 {
            return Some(format!("0x{mask:X}"));
        } else {
            return None;
        };
        Some(label.into())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn signal_strength_mapping_matches_mbn_range() {
            assert_eq!(signal_percent(0), Some(0));
            assert_eq!(signal_percent(20), Some(65));
            assert_eq!(signal_percent(31), Some(100));
            assert_eq!(signal_percent(99), None);
        }

        #[test]
        fn registration_states_are_mapped_conservatively() {
            assert_eq!(registration_status(3), "connected");
            assert_eq!(registration_status(4), "connected");
            assert_eq!(registration_status(2), "available");
            assert_eq!(registration_status(6), "unavailable");
        }
    }
}
