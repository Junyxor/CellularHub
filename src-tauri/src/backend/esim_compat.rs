//! Experimental eSIM compatibility path for Windows machines where the native
//! Windows eSIM/LPA surface is unavailable.
//!
//! This module never emulates an eUICC. It only looks for a *real* eUICC that a
//! modem exposes through standard AT APDU commands and, when the optional
//! `lpac` executable is available, hands provisioning to lpac.

use std::{
    io::{Read, Write},
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

use crate::model::{EsimCompatCandidate, EsimCompatInstallRequest, EsimCompatReport};

pub fn probe() -> EsimCompatReport {
    let lpac = find_lpac();
    let lpac_available = lpac.is_some();
    let candidates = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|port| probe_port(&port.port_name, lpac_available))
        .collect();

    EsimCompatReport {
        windows_lpa_available: windows_lpa_available(),
        lpac_path: lpac.map(|path| path.to_string_lossy().to_string()),
        candidates,
    }
}

pub fn install(request: EsimCompatInstallRequest) -> Result<String, String> {
    let activation = normalize_activation(&request.activation_code)?;
    let lpac = find_lpac().ok_or_else(|| {
        "未找到 lpac。请把官方 lpac.exe 放到 CellularHub.exe 同目录、tools 目录，或加入 PATH。".to_string()
    })?;

    let transport = match request.transport.trim().to_ascii_lowercase().as_str() {
        "at" => "at",
        "at_csim" | "csim" => "at_csim",
        other => return Err(format!("不支持的 eUICC APDU transport: {other}")),
    };

    if request.port.trim().is_empty() {
        return Err("缺少 eUICC/Modem 串口".into());
    }

    let mut command = Command::new(&lpac);
    command
        .env("LPAC_APDU", transport)
        .env("LPAC_APDU_AT_DEVICE", request.port.trim())
        .args(["profile", "download", "-a", activation.as_str()]);

    #[cfg(target_os = "windows")]
    command.env("LPAC_HTTP", "winhttp");

    let output = command
        .output()
        .map_err(|error| format!("无法启动 lpac: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        return Ok(if stdout.is_empty() {
            "lpac 已完成 profile download。请在设备/运营商状态中确认配置文件是否已安装。".into()
        } else {
            trim_output(&stdout)
        });
    }

    let detail = if !stderr.is_empty() { stderr } else { stdout };
    Err(format!(
        "lpac provisioning 失败（{}）。{}",
        output.status,
        if detail.is_empty() {
            "该 modem/eUICC 可能不支持所选 AT APDU 通道，或 AT 后端响应时间不足。".into()
        } else {
            trim_output(&detail)
        }
    ))
}

fn probe_port(port_name: &str, lpac_available: bool) -> Option<EsimCompatCandidate> {
    let mut port = serialport::new(port_name, 115_200)
        .timeout(Duration::from_millis(450))
        .open()
        .ok()?;

    let at = at_command(&mut *port, "AT", Duration::from_millis(900)).ok()?;
    if !command_ok(&at) {
        return None;
    }

    let ccho = at_command(&mut *port, "AT+CCHO=?", Duration::from_millis(850)).unwrap_or_default();
    let csim = at_command(&mut *port, "AT+CSIM=?", Duration::from_millis(850)).unwrap_or_default();
    let qesim = at_command(&mut *port, "AT+QESIM=?", Duration::from_millis(850)).unwrap_or_default();

    let at_apdu = command_supported(&ccho);
    let at_csim = command_supported(&csim);
    let vendor_esim = qesim.to_ascii_uppercase().contains("QESIM") && command_ok(&qesim);

    let eid = if vendor_esim {
        let result = at_command(&mut *port, "AT+QESIM=\"eid\"", Duration::from_millis(1200)).unwrap_or_default();
        extract_eid(&result)
    } else {
        None
    };

    if !(at_apdu || at_csim || vendor_esim || eid.is_some()) {
        return None;
    }

    let install_ready = lpac_available && (at_apdu || at_csim);
    let mut details = Vec::new();
    if at_apdu { details.push("CCHO/CGLA APDU"); }
    if at_csim { details.push("CSIM APDU"); }
    if vendor_esim { details.push("厂商 QESIM"); }
    if eid.is_some() { details.push("EID 可读"); }
    if lpac_available { details.push("lpac 已发现"); } else { details.push("缺少 lpac"); }

    Some(EsimCompatCandidate {
        port: port_name.to_string(),
        at_apdu,
        at_csim,
        vendor_esim,
        eid,
        lpac_available,
        install_ready,
        note: details.join(" · "),
    })
}

fn at_command(port: &mut dyn serialport::SerialPort, command: &str, timeout: Duration) -> Result<String, String> {
    port.clear(serialport::ClearBuffer::Input).ok();
    port.write_all(format!("{command}\r").as_bytes()).map_err(|e| e.to_string())?;
    port.flush().ok();

    let started = Instant::now();
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 512];
    while started.elapsed() < timeout {
        match port.read(&mut buffer) {
            Ok(n) if n > 0 => {
                bytes.extend_from_slice(&buffer[..n]);
                let text = String::from_utf8_lossy(&bytes);
                if text.contains("\r\nOK\r\n") || text.contains("\r\nERROR\r\n") || text.contains("+CME ERROR") {
                    break;
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn command_ok(response: &str) -> bool {
    let upper = response.to_ascii_uppercase();
    upper.contains("OK") && !upper.contains("ERROR")
}

fn command_supported(response: &str) -> bool {
    !response.trim().is_empty() && command_ok(response)
}

fn extract_eid(response: &str) -> Option<String> {
    response
        .split(|ch: char| !ch.is_ascii_digit())
        .find(|token| token.len() == 32)
        .map(str::to_string)
}

fn normalize_activation(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.to_ascii_lowercase().starts_with("lpa:") {
        Ok(value.to_string())
    } else if value.starts_with("1$") {
        Ok(format!("LPA:{value}"))
    } else {
        Err("请输入 LPA: URI 或二维码中的完整 1$ activation code".into())
    }
}

fn find_lpac() -> Option<PathBuf> {
    let exe_name = if cfg!(target_os = "windows") { "lpac.exe" } else { "lpac" };

    if let Ok(current) = std::env::current_exe() {
        if let Some(parent) = current.parent() {
            for candidate in [parent.join(exe_name), parent.join("tools").join(exe_name)] {
                if candidate.is_file() { return Some(candidate); }
            }
        }
    }

    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            let candidate = directory.join(exe_name);
            if candidate.is_file() { return Some(candidate); }
        }
    }
    None
}

fn windows_lpa_available() -> bool {
    #[cfg(not(target_os = "windows"))]
    { false }

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("reg")
            .args(["query", r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion", "/v", "CurrentBuildNumber"])
            .output();
        let Ok(output) = output else { return false; };
        let text = String::from_utf8_lossy(&output.stdout);
        text.split_whitespace()
            .filter_map(|token| token.parse::<u32>().ok())
            .any(|build| build >= 26_100)
    }
}

fn trim_output(value: &str) -> String {
    const MAX: usize = 1600;
    if value.chars().count() <= MAX { return value.to_string(); }
    let mut text: String = value.chars().take(MAX).collect();
    text.push_str("\n…输出已截断");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_only_full_eid() {
        let line = "+QESIM: \"eid\",0,\"89049032000000000000000000001234\"\r\nOK";
        assert_eq!(extract_eid(line).as_deref(), Some("89049032000000000000000000001234"));
        assert_eq!(extract_eid("+QESIM: \"eid\",8500000A,\"\"\r\nOK"), None);
    }

    #[test]
    fn activation_normalization() {
        assert_eq!(normalize_activation("1$server$id").unwrap(), "LPA:1$server$id");
        assert_eq!(normalize_activation("lpa:1$server$id").unwrap(), "lpa:1$server$id");
    }
}
