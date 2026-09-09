use std::{env, path::PathBuf, process::Command};

use crate::model::{CellularDevice, RuntimeCapabilities};

pub fn enumerate_devices() -> Vec<CellularDevice> {
    // Real MBN/MBIM enumeration is isolated behind this provider. The 1.0 bootstrap
    // intentionally returns an empty list unless a native provider is implemented,
    // rather than fabricating hardware capabilities.
    Vec::new()
}

pub fn detect_runtime_capabilities() -> RuntimeCapabilities {
    let lpac_found = find_lpac().is_some();
    RuntimeCapabilities {
        native_lpa_available: cfg!(windows),
        euicc_bridge_available: false,
        lpac_found,
        platform: if cfg!(windows) { "windows".into() } else { env::consts::OS.into() },
    }
}

pub fn launch_native_lpa(code: &str) -> Result<String, String> {
    if !cfg!(windows) {
        return Err("Windows LPA is only available on Windows".into());
    }
    let normalized = normalize_activation_code(code)?;
    Command::new("cmd")
        .args(["/C", "start", "", &format!("lpa:{normalized}")])
        .spawn()
        .map_err(|e| format!("failed to open Windows LPA handler: {e}"))?;
    Ok("Activation code was handed to the Windows LPA handler".into())
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
    if let Some(found) = candidates.into_iter().find(|p| p.exists()) {
        return Some(found);
    }
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|path| path.join("lpac.exe"))
            .find(|candidate| candidate.exists())
    })
}
