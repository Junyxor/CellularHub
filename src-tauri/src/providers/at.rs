//! Receive-only AT-command fallback for Windows USB/COM modems.
//!
//! CellularHub never emits AT+CMGS here. Port discovery is passive; a port is only
//! opened after the user explicitly asks to poll that device.

use crate::{
    model::{CellularDevice, DeviceCapabilities},
    sms::{inbox::IncomingSms, pdu::decode_deliver_pdu},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmtiNotification {
    pub storage: String,
    pub index: u32,
}

#[derive(Debug, Clone, Default)]
pub struct AtPoll {
    pub messages: Vec<IncomingSms>,
    pub operator: Option<String>,
    pub signal: Option<u8>,
}

pub const TEXT_MODE_INIT: &[&str] = &[
    "AT",
    "ATE0",
    "AT+CMGF=1",
    "AT+CNMI=2,1,0,0,0",
];

pub const PDU_MODE_INIT: &[&str] = &[
    "AT",
    "ATE0",
    "AT+CMGF=0",
    "AT+CNMI=2,1,0,0,0",
];

pub fn parse_cmti(line: &str) -> Option<CmtiNotification> {
    let payload = line.trim().strip_prefix("+CMTI:")?.trim();
    let (storage, index) = payload.split_once(',')?;
    let storage = storage.trim().trim_matches('"').trim();
    if storage.is_empty() {
        return None;
    }
    Some(CmtiNotification {
        storage: storage.to_string(),
        index: index.trim().parse().ok()?,
    })
}

pub fn parse_csq_percent(line: &str) -> Option<u8> {
    let payload = line.trim().strip_prefix("+CSQ:")?.trim();
    let raw: u32 = payload.split(',').next()?.trim().parse().ok()?;
    if raw > 31 {
        return None;
    }
    Some(((raw * 100 + 15) / 31) as u8)
}

pub fn parse_cops_operator(line: &str) -> Option<String> {
    let payload = line.trim().strip_prefix("+COPS:")?.trim();
    let start = payload.find('"')? + 1;
    let end = payload[start..].find('"')? + start;
    let operator = payload[start..end].trim();
    (!operator.is_empty()).then(|| operator.to_string())
}

pub fn is_terminal_ok(line: &str) -> bool {
    line.trim().eq_ignore_ascii_case("OK")
}

pub fn is_terminal_error(line: &str) -> bool {
    let line = line.trim();
    line.eq_ignore_ascii_case("ERROR")
        || line.starts_with("+CME ERROR:")
        || line.starts_with("+CMS ERROR:")
}

pub fn enumerate_devices() -> Vec<CellularDevice> {
    #[cfg(windows)]
    {
        transport::enumerate_devices()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

pub fn poll(device_id: &str) -> Result<AtPoll, String> {
    #[cfg(windows)]
    {
        transport::poll(device_id)
    }
    #[cfg(not(windows))]
    {
        let _ = device_id;
        Err("AT fallback is only available on Windows in CellularHub 1.0".into())
    }
}

fn quoted_fields(input: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for ch in input.chars() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                output.push(current.trim().trim_matches('"').to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    output.push(current.trim().trim_matches('"').to_string());
    output
}

fn parse_text_cmgl(port_name: &str, lines: &[String]) -> Vec<IncomingSms> {
    let mut messages = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index].trim();
        let Some(header) = line.strip_prefix("+CMGL:") else {
            index += 1;
            continue;
        };
        let fields = quoted_fields(header.trim());
        let storage_index = fields.first().cloned().unwrap_or_else(|| "?".into());
        let sender = fields.get(2).cloned().unwrap_or_else(|| "Unknown".into());
        let timestamp = fields
            .iter()
            .rev()
            .find(|value| value.contains('/') && value.contains(':'))
            .cloned();

        index += 1;
        let mut body_lines = Vec::new();
        while index < lines.len() && !lines[index].trim().starts_with("+CMGL:") {
            let candidate = lines[index].trim();
            if is_terminal_ok(candidate) || is_terminal_error(candidate) {
                break;
            }
            if !candidate.is_empty() {
                body_lines.push(candidate.to_string());
            }
            index += 1;
        }
        messages.push(IncomingSms {
            source_key: format!("at:{port_name}:text:{storage_index}:{sender}:{}", timestamp.as_deref().unwrap_or("")),
            sender,
            body: body_lines.join("\n"),
            // AT SCTS is not RFC3339; keep it out of the public model until timezone decoding is exact.
            received_at: None,
            concatenation: None,
        });
    }
    messages
}

fn parse_pdu_cmgl(port_name: &str, lines: &[String]) -> Vec<IncomingSms> {
    let mut messages = Vec::new();
    let mut index = 0usize;
    while index + 1 < lines.len() {
        let line = lines[index].trim();
        let Some(header) = line.strip_prefix("+CMGL:") else {
            index += 1;
            continue;
        };
        let storage_index = header.split(',').next().unwrap_or("?").trim();
        let pdu = lines[index + 1].trim();
        if pdu.len() >= 4 && pdu.len() % 2 == 0 && pdu.chars().all(|ch| ch.is_ascii_hexdigit()) {
            if let Ok(decoded) = decode_deliver_pdu(pdu) {
                messages.push(IncomingSms {
                    source_key: format!("at:{port_name}:pdu:{storage_index}:{pdu}"),
                    sender: decoded.sender,
                    body: decoded.body,
                    received_at: None,
                    concatenation: decoded.concatenation,
                });
            }
        }
        index += 2;
    }
    messages
}

#[cfg(windows)]
mod transport {
    use std::{
        io::{ErrorKind, Read, Write},
        time::{Duration, Instant},
    };

    use serialport::{SerialPort, SerialPortType};

    use super::*;

    const BAUD_RATES: &[u32] = &[115_200, 57_600, 38_400, 19_200, 9_600];
    const COMMAND_TIMEOUT: Duration = Duration::from_millis(900);

    pub fn enumerate_devices() -> Vec<CellularDevice> {
        serialport::available_ports()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|port| {
                let (label, model) = match &port.port_type {
                    SerialPortType::UsbPort(info) => {
                        let product = info.product.clone().filter(|value| !value.trim().is_empty());
                        let manufacturer = info.manufacturer.clone().filter(|value| !value.trim().is_empty());
                        let label = match (&manufacturer, &product) {
                            (Some(manufacturer), Some(product)) => format!("{manufacturer} {product}"),
                            (Some(manufacturer), None) => manufacturer.clone(),
                            (None, Some(product)) => product.clone(),
                            (None, None) => format!("USB modem {}", port.port_name),
                        };
                        (label, product)
                    }
                    // Unknown COM devices are still surfaced as candidates. We do not open them automatically.
                    SerialPortType::Unknown => (format!("Serial modem candidate {}", port.port_name), None),
                    _ => return None,
                };
                Some(CellularDevice {
                    id: format!("at:{}", port.port_name),
                    label,
                    kind: "at".into(),
                    model,
                    operator: None,
                    signal: None,
                    network_class: None,
                    status: "available".into(),
                    capabilities: DeviceCapabilities {
                        // Port presence is not proof of SMS support. A successful explicit poll flips this true.
                        sms_receive: false,
                        sms_send: false,
                        native_esim: false,
                        euicc_bridge: false,
                    },
                })
            })
            .collect()
    }

    pub fn poll(device_id: &str) -> Result<AtPoll, String> {
        let port_name = device_id
            .strip_prefix("at:")
            .ok_or_else(|| "not an AT provider device id".to_string())?;
        if port_name.trim().is_empty() {
            return Err("AT device id has an empty port name".into());
        }

        let mut last_error = String::new();
        for &baud in BAUD_RATES {
            match serialport::new(port_name, baud)
                .timeout(Duration::from_millis(180))
                .open()
            {
                Ok(mut port) => match poll_open_port(port_name, port.as_mut()) {
                    Ok(result) => return Ok(result),
                    Err(error) => last_error = format!("{baud} baud: {error}"),
                },
                Err(error) => last_error = format!("{baud} baud: {error}"),
            }
        }
        Err(format!("could not establish receive-only AT session on {port_name}: {last_error}"))
    }

    fn poll_open_port(port_name: &str, port: &mut dyn SerialPort) -> Result<AtPoll, String> {
        let at = command(port, "AT")?;
        require_ok(&at, "AT")?;
        let _ = command(port, "ATE0");

        let operator = command(port, "AT+COPS?")
            .ok()
            .and_then(|lines| lines.iter().find_map(|line| parse_cops_operator(line)));
        let signal = command(port, "AT+CSQ")
            .ok()
            .and_then(|lines| lines.iter().find_map(|line| parse_csq_percent(line)));

        let text_mode = command(port, "AT+CMGF=1")?;
        let messages = if text_mode.iter().any(|line| is_terminal_ok(line)) {
            let _ = command(port, "AT+CNMI=2,1,0,0,0");
            let mut lines = command(port, "AT+CMGL=\"REC UNREAD\"")?;
            if require_ok(&lines, "AT+CMGL=REC UNREAD").is_err() {
                lines = command(port, "AT+CMGL=\"ALL\"")?;
                require_ok(&lines, "AT+CMGL=ALL")?;
            }
            parse_text_cmgl(port_name, &lines)
        } else {
            let pdu_mode = command(port, "AT+CMGF=0")?;
            require_ok(&pdu_mode, "AT+CMGF=0")?;
            let _ = command(port, "AT+CNMI=2,1,0,0,0");
            let lines = command(port, "AT+CMGL=0")?;
            require_ok(&lines, "AT+CMGL=0")?;
            parse_pdu_cmgl(port_name, &lines)
        };

        Ok(AtPoll {
            messages,
            operator,
            signal,
        })
    }

    fn require_ok(lines: &[String], command_name: &str) -> Result<(), String> {
        if lines.iter().any(|line| is_terminal_ok(line)) {
            Ok(())
        } else {
            let diagnostic = lines.last().cloned().unwrap_or_else(|| "no response".into());
            Err(format!("{command_name} failed: {diagnostic}"))
        }
    }

    fn command(port: &mut dyn SerialPort, command: &str) -> Result<Vec<String>, String> {
        port.set_timeout(Duration::from_millis(120)).map_err(|error| error.to_string())?;
        port.write_all(command.as_bytes()).map_err(|error| error.to_string())?;
        port.write_all(b"\r").map_err(|error| error.to_string())?;
        port.flush().map_err(|error| error.to_string())?;

        let deadline = Instant::now() + COMMAND_TIMEOUT;
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 512];
        loop {
            match port.read(&mut chunk) {
                Ok(0) => {}
                Ok(count) => {
                    bytes.extend_from_slice(&chunk[..count]);
                    let text = String::from_utf8_lossy(&bytes);
                    if text.lines().any(is_terminal_error)
                        || text.lines().rev().find(|line| !line.trim().is_empty()).is_some_and(is_terminal_ok)
                    {
                        break;
                    }
                }
                Err(error) if error.kind() == ErrorKind::TimedOut => {
                    if !bytes.is_empty() && Instant::now() >= deadline {
                        break;
                    }
                }
                Err(error) => return Err(error.to_string()),
            }
            if Instant::now() >= deadline {
                break;
            }
        }

        Ok(String::from_utf8_lossy(&bytes)
            .replace('\r', "\n")
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && *line != command)
            .map(ToOwned::to_owned)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cmti_notification() {
        assert_eq!(
            parse_cmti(r#"+CMTI: "SM",12"#),
            Some(CmtiNotification {
                storage: "SM".into(),
                index: 12,
            })
        );
    }

    #[test]
    fn converts_csq_to_percentage_and_rejects_unknown() {
        assert_eq!(parse_csq_percent("+CSQ: 20,99"), Some(65));
        assert_eq!(parse_csq_percent("+CSQ: 31,99"), Some(100));
        assert_eq!(parse_csq_percent("+CSQ: 99,99"), None);
    }

    #[test]
    fn parses_operator_name() {
        assert_eq!(
            parse_cops_operator(r#"+COPS: 0,0,"China Mobile",7"#),
            Some("China Mobile".into())
        );
    }

    #[test]
    fn parses_text_cmgl_records_without_dropping_repeated_body_lines() {
        let lines = vec![
            r#"+CMGL: 12,"REC UNREAD","10086",,"26/09/10,08:00:00+32""#.into(),
            "验证码 284193".into(),
            "验证码 284193".into(),
            "OK".into(),
        ];
        let messages = parse_text_cmgl("COM3", &lines);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender, "10086");
        assert_eq!(messages[0].body, "验证码 284193\n验证码 284193");
    }

    #[test]
    fn recognizes_terminal_responses() {
        assert!(is_terminal_ok(" OK\r\n"));
        assert!(is_terminal_error("+CMS ERROR: 500"));
        assert!(is_terminal_error("ERROR"));
    }

    #[test]
    fn init_sequences_keep_send_disabled() {
        assert!(TEXT_MODE_INIT.contains(&"AT+CMGF=1"));
        assert!(PDU_MODE_INIT.contains(&"AT+CMGF=0"));
        assert!(!TEXT_MODE_INIT.iter().any(|command| command.contains("CMGS")));
        assert!(!PDU_MODE_INIT.iter().any(|command| command.contains("CMGS")));
    }
}
