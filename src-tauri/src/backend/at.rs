use std::{
    io::{Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use chrono::Utc;
use serialport::{SerialPortType, UsbPortInfo};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::{
    backend::pdu::{decode_sms_deliver, DecodedSms, MultipartAssembler},
    manager::CellularManager,
    model::{BackendKind, CapabilitySet, CellularSnapshot, DeviceInfo, SmsMessage},
};

pub fn discover() -> Vec<DeviceInfo> {
    let Ok(ports) = serialport::available_ports() else { return Vec::new(); };
    ports.into_iter().map(|port| {
        let (manufacturer, product, likely_modem) = match port.port_type {
            SerialPortType::UsbPort(UsbPortInfo { manufacturer, product, vid, pid, .. }) => {
                let haystack = format!("{} {}", manufacturer.clone().unwrap_or_default(), product.clone().unwrap_or_default()).to_ascii_lowercase();
                let likely = ["quectel", "fibocom", "sierra", "modem", "mobile", "wwan", "lte", "5g", "huawei", "zte"].iter().any(|needle| haystack.contains(needle));
                (manufacturer, product.or_else(|| Some(format!("USB {:04x}:{:04x}", vid, pid))), likely)
            }
            _ => (None, None, false),
        };
        DeviceInfo {
            id: format!("at:{}", port.port_name),
            name: product.clone().unwrap_or_else(|| if likely_modem { "蜂窝 AT Modem".into() } else { "串口设备（可探测 AT）".into() }),
            backend: BackendKind::At,
            port: Some(port.port_name),
            manufacturer,
            product,
            capabilities: CapabilitySet { data: false, sms_receive: true, sms_send: false, esim: false, signal: true, operator: true },
        }
    }).collect()
}

pub fn spawn_listener(
    app: AppHandle,
    manager: Arc<CellularManager>,
    port_name: String,
    baud_rate: u32,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut probe = serialport::new(&port_name, baud_rate)
        .timeout(Duration::from_millis(700))
        .open()
        .map_err(|e| format!("无法打开 {port_name}: {e}"))?;
    let _ = send_command(&mut *probe, "AT", Duration::from_secs(2))?;
    drop(probe);

    let worker_port = port_name.clone();
    thread::spawn(move || {
        if let Err(error) = run_worker(&app, &manager, &worker_port, baud_rate, &stop) {
            manager.set_error(error);
        }
    });

    manager.set_snapshot(CellularSnapshot {
        backend: BackendKind::At,
        device_name: format!("AT Modem · {port_name}"),
        operator_name: "正在读取运营商".into(),
        network_type: "Cellular".into(),
        signal_percent: 0,
        rssi_dbm: None,
        connected: true,
        sms_ready: true,
        esim_ready: false,
        last_error: None,
    });
    Ok(())
}

fn run_worker(app: &AppHandle, manager: &Arc<CellularManager>, port_name: &str, baud_rate: u32, stop: &Arc<AtomicBool>) -> Result<(), String> {
    let mut port = serialport::new(port_name, baud_rate)
        .timeout(Duration::from_millis(250))
        .open()
        .map_err(|e| format!("AT 后端打开失败: {e}"))?;

    let _ = send_command(&mut *port, "ATE0", Duration::from_secs(1));
    let text_mode = send_command(&mut *port, "AT+CMGF=1", Duration::from_secs(2))
        .map(|response| !response.to_ascii_uppercase().contains("ERROR"))
        .unwrap_or(false);
    if !text_mode {
        send_command(&mut *port, "AT+CMGF=0", Duration::from_secs(2))?;
    }
    let _ = send_command(&mut *port, "AT+CNMI=2,1,0,0,0", Duration::from_secs(2));

    update_radio_metadata(&mut *port, manager);
    let mut multipart = MultipartAssembler::default();

    let list_command = if text_mode { "AT+CMGL=\"ALL\"" } else { "AT+CMGL=4" };
    if let Ok(existing) = send_command(&mut *port, list_command, Duration::from_secs(5)) {
        for parsed in parse_cmgl_any(&existing, text_mode) {
            deliver_decoded(app, manager, &mut multipart, parsed.decoded, parsed.slot);
        }
    }

    let mut rx = String::new();
    let mut last_poll = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        let mut buf = [0u8; 1024];
        match port.read(&mut buf) {
            Ok(n) if n > 0 => {
                rx.push_str(&String::from_utf8_lossy(&buf[..n]));
                while let Some(line_end) = rx.find("\r\n") {
                    let line = rx[..line_end].trim().to_string();
                    rx.drain(..line_end + 2);
                    if let Some(slot) = parse_cmti(&line) {
                        if let Ok(response) = send_command(&mut *port, &format!("AT+CMGR={slot}"), Duration::from_secs(4)) {
                            if let Some(parsed) = parse_cmgr_any(&response, text_mode, Some(slot)) {
                                deliver_decoded(app, manager, &mut multipart, parsed.decoded, parsed.slot);
                            }
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(format!("AT 串口读取失败: {e}")),
        }

        if last_poll.elapsed() > Duration::from_secs(30) {
            update_radio_metadata(&mut *port, manager);
            last_poll = Instant::now();
        }
    }
    Ok(())
}

struct ParsedIncoming {
    decoded: DecodedSms,
    slot: Option<u32>,
}

fn deliver_decoded(app: &AppHandle, manager: &Arc<CellularManager>, multipart: &mut MultipartAssembler, decoded: DecodedSms, slot: Option<u32>) {
    if let Some((sender, body)) = multipart.push(decoded) {
        let mut sms = new_sms(sender, body);
        sms.slot = slot;
        deliver(app, manager, sms);
    }
}

fn deliver(app: &AppHandle, manager: &Arc<CellularManager>, sms: SmsMessage) {
    if manager.push_message(sms.clone()) {
        let _ = app.emit("sms-received", sms.clone());
        crate::notification_center::show_sms(app, &sms);
    }
}

fn update_radio_metadata(port: &mut dyn serialport::SerialPort, manager: &Arc<CellularManager>) {
    let operator = send_command(port, "AT+COPS?", Duration::from_secs(2)).ok().and_then(|r| parse_operator(&r));
    let csq = send_command(port, "AT+CSQ", Duration::from_secs(2)).ok().and_then(|r| parse_csq(&r));
    let mut snapshot = manager.snapshot();
    if let Some(name) = operator { snapshot.operator_name = name; }
    if let Some((percent, dbm)) = csq { snapshot.signal_percent = percent; snapshot.rssi_dbm = dbm; }
    manager.set_snapshot(snapshot);
}

fn send_command(port: &mut dyn serialport::SerialPort, command: &str, timeout: Duration) -> Result<String, String> {
    port.write_all(format!("{command}\r").as_bytes()).map_err(|e| e.to_string())?;
    port.flush().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + timeout;
    let mut output = String::new();
    while Instant::now() < deadline {
        let mut buf = [0u8; 1024];
        match port.read(&mut buf) {
            Ok(n) if n > 0 => {
                output.push_str(&String::from_utf8_lossy(&buf[..n]));
                if output.contains("\r\nOK\r\n") || output.contains("\r\nERROR\r\n") || output.ends_with("OK\r\n") { break; }
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    if output.trim().is_empty() { Err(format!("{command} 无响应")) } else { Ok(output) }
}

fn parse_cmti(line: &str) -> Option<u32> {
    if !line.starts_with("+CMTI:") { return None; }
    line.rsplit(',').next()?.trim().parse().ok()
}

fn parse_cmgr_any(response: &str, text_mode: bool, slot: Option<u32>) -> Option<ParsedIncoming> {
    if text_mode { parse_cmgr_text(response, slot) } else { parse_cmgr_pdu(response, slot) }
}

fn parse_cmgr_text(response: &str, slot: Option<u32>) -> Option<ParsedIncoming> {
    let mut lines = response.lines().map(str::trim).filter(|line| !line.is_empty() && *line != "OK" && *line != "ERROR");
    let header = lines.find(|line| line.starts_with("+CMGR:"))?;
    let fields = csv_fields(header.strip_prefix("+CMGR:")?.trim());
    let sender = fields.get(1).cloned().unwrap_or_else(|| "未知号码".into());
    let body = lines.filter(|line| !line.starts_with("+CMGR:")).collect::<Vec<_>>().join("\n");
    Some(ParsedIncoming { decoded: DecodedSms { sender: decode_possible_ucs2(&sender), body: decode_possible_ucs2(&body), concat: None }, slot })
}

fn parse_cmgr_pdu(response: &str, slot: Option<u32>) -> Option<ParsedIncoming> {
    let lines: Vec<&str> = response.lines().map(str::trim).filter(|line| !line.is_empty() && *line != "OK" && *line != "ERROR").collect();
    let header_index = lines.iter().position(|line| line.starts_with("+CMGR:"))?;
    let pdu_line = lines.get(header_index + 1)?;
    let decoded = decode_sms_deliver(pdu_line).ok()?;
    Some(ParsedIncoming { decoded, slot })
}

fn parse_cmgl_any(response: &str, text_mode: bool) -> Vec<ParsedIncoming> {
    if text_mode { parse_cmgl_text(response) } else { parse_cmgl_pdu(response) }
}

fn parse_cmgl_text(response: &str) -> Vec<ParsedIncoming> {
    let lines: Vec<&str> = response.lines().map(str::trim).filter(|line| !line.is_empty() && *line != "OK").collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].starts_with("+CMGL:") {
            let fields = csv_fields(lines[i].trim_start_matches("+CMGL:").trim());
            let slot = fields.first().and_then(|value| value.parse::<u32>().ok());
            let sender = fields.get(2).cloned().unwrap_or_else(|| "未知号码".into());
            let body = lines.get(i + 1).copied().unwrap_or_default();
            out.push(ParsedIncoming { decoded: DecodedSms { sender: decode_possible_ucs2(&sender), body: decode_possible_ucs2(body), concat: None }, slot });
            i += 2;
        } else { i += 1; }
    }
    out
}

fn parse_cmgl_pdu(response: &str) -> Vec<ParsedIncoming> {
    let lines: Vec<&str> = response.lines().map(str::trim).filter(|line| !line.is_empty() && *line != "OK" && *line != "ERROR").collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].starts_with("+CMGL:") {
            let fields = csv_fields(lines[i].trim_start_matches("+CMGL:").trim());
            let slot = fields.first().and_then(|value| value.parse::<u32>().ok());
            if let Some(pdu_line) = lines.get(i + 1) {
                if let Ok(decoded) = decode_sms_deliver(pdu_line) { out.push(ParsedIncoming { decoded, slot }); }
            }
            i += 2;
        } else { i += 1; }
    }
    out
}

fn new_sms(sender: String, body: String) -> SmsMessage {
    SmsMessage { id: Uuid::new_v4().to_string(), sender, body, received_at: Utc::now().to_rfc3339(), unread: true, archived: false, backend: BackendKind::At, slot: None }
}

fn csv_fields(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for ch in input.chars() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => { result.push(current.trim().to_string()); current.clear(); }
            _ => current.push(ch),
        }
    }
    result.push(current.trim().to_string());
    result
}

fn decode_possible_ucs2(input: &str) -> String {
    let clean = input.trim().trim_matches('"');
    if clean.len() >= 4 && clean.len() % 4 == 0 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
        let units: Option<Vec<u16>> = clean.as_bytes().chunks_exact(4).map(|chunk| std::str::from_utf8(chunk).ok().and_then(|hex| u16::from_str_radix(hex, 16).ok())).collect();
        if let Some(units) = units { if let Ok(decoded) = String::from_utf16(&units) { return decoded; } }
    }
    clean.to_string()
}

fn parse_operator(response: &str) -> Option<String> {
    let line = response.lines().find(|line| line.trim().starts_with("+COPS:"))?;
    let fields = csv_fields(line.split_once(':')?.1.trim());
    fields.get(2).map(|value| decode_possible_ucs2(value)).filter(|value| !value.is_empty())
}

fn parse_csq(response: &str) -> Option<(u8, Option<i32>)> {
    let line = response.lines().find(|line| line.trim().starts_with("+CSQ:"))?;
    let rssi: i32 = line.split_once(':')?.1.split(',').next()?.trim().parse().ok()?;
    if rssi == 99 { return Some((0, None)); }
    let rssi = rssi.clamp(0, 31);
    let dbm = -113 + 2 * rssi;
    let percent = ((rssi as f32 / 31.0) * 100.0).round() as u8;
    Some((percent, Some(dbm)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn parses_cmti() { assert_eq!(parse_cmti("+CMTI: \"SM\",12"), Some(12)); }
    #[test] fn decodes_ucs2() { assert_eq!(decode_possible_ucs2("4F60597D"), "你好"); }
    #[test] fn parses_csq() { assert_eq!(parse_csq("\r\n+CSQ: 20,99\r\nOK\r\n"), Some((65, Some(-73)))); }
}
