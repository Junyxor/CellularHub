//! Receive-only AT-command fallback for Windows USB/COM modems.
//!
//! CellularHub never emits AT+CMGS here. Port discovery is passive; a port is only
//! opened after the user explicitly asks to test/poll it or start a receive session.

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

pub struct AtListenerHandle {
    #[cfg(windows)]
    inner: transport::AtListenerHandle,
}

pub struct AtListenerStarted {
    pub handle: AtListenerHandle,
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

pub fn start_listener<F>(device_id: &str, on_message: F) -> Result<AtListenerStarted, String>
where
    F: Fn(IncomingSms) + Send + 'static,
{
    #[cfg(windows)]
    {
        let started = transport::start_listener(device_id, on_message)?;
        Ok(AtListenerStarted {
            handle: AtListenerHandle { inner: started.handle },
            operator: started.operator,
            signal: started.signal,
        })
    }
    #[cfg(not(windows))]
    {
        let _ = device_id;
        let _ = on_message;
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
            source_key: format!(
                "at:{port_name}:text:{storage_index}:{sender}:{}",
                timestamp.as_deref().unwrap_or("")
            ),
            sender,
            body: body_lines.join("\n"),
            // AT SCTS is not RFC3339; keep it out until timezone decoding is exact.
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
        if looks_like_hex_pdu(pdu) {
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

fn parse_text_cmgr(port_name: &str, storage_index: u32, lines: &[String]) -> Option<IncomingSms> {
    let header_index = lines.iter().position(|line| line.trim().starts_with("+CMGR:"))?;
    let header = lines[header_index].trim().strip_prefix("+CMGR:")?.trim();
    let fields = quoted_fields(header);
    let sender = fields.get(1).cloned().filter(|value| !value.is_empty()).unwrap_or_else(|| "Unknown".into());
    let timestamp = fields
        .iter()
        .rev()
        .find(|value| value.contains('/') && value.contains(':'))
        .cloned();
    let body = lines
        .iter()
        .skip(header_index + 1)
        .take_while(|line| !is_terminal_ok(line) && !is_terminal_error(line))
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join("\n");

    Some(IncomingSms {
        source_key: format!(
            "at:{port_name}:text:{storage_index}:{sender}:{}",
            timestamp.as_deref().unwrap_or("")
        ),
        sender,
        body,
        received_at: None,
        concatenation: None,
    })
}

fn parse_pdu_cmgr(port_name: &str, storage_index: u32, lines: &[String]) -> Option<IncomingSms> {
    let header_index = lines.iter().position(|line| line.trim().starts_with("+CMGR:"))?;
    let pdu = lines
        .iter()
        .skip(header_index + 1)
        .map(|line| line.trim())
        .find(|line| looks_like_hex_pdu(line))?;
    let decoded = decode_deliver_pdu(pdu).ok()?;
    Some(IncomingSms {
        source_key: format!("at:{port_name}:pdu:{storage_index}:{pdu}"),
        sender: decoded.sender,
        body: decoded.body,
        received_at: None,
        concatenation: decoded.concatenation,
    })
}

fn looks_like_hex_pdu(value: &str) -> bool {
    value.len() >= 4 && value.len() % 2 == 0 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

#[cfg(windows)]
mod transport {
    use std::{
        io::{ErrorKind, Read, Write},
        sync::mpsc::{self, Receiver, Sender},
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    };

    use serialport::{SerialPort, SerialPortType};

    use super::*;

    const BAUD_RATES: &[u32] = &[115_200, 57_600, 38_400, 19_200, 9_600];
    const COMMAND_TIMEOUT: Duration = Duration::from_millis(1_500);
    const LISTENER_READ_TIMEOUT: Duration = Duration::from_millis(180);
    const FALLBACK_SCAN_INTERVAL: Duration = Duration::from_secs(30);
    const LISTENER_START_TIMEOUT: Duration = Duration::from_secs(12);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum SmsMode {
        Text,
        Pdu,
    }

    pub struct AtListenerHandle {
        stop: Option<Sender<()>>,
        thread: Option<JoinHandle<()>>,
    }

    impl Drop for AtListenerHandle {
        fn drop(&mut self) {
            if let Some(stop) = self.stop.take() {
                let _ = stop.send(());
            }
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    pub struct ListenerStarted {
        pub handle: AtListenerHandle,
        pub operator: Option<String>,
        pub signal: Option<u8>,
    }

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
                        // Port presence is not proof of SMS support. A successful explicit probe flips this true.
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
        let port_name = port_name_from_id(device_id)?;
        let mut last_error = String::new();
        for &baud in BAUD_RATES {
            match serialport::new(port_name, baud)
                .timeout(LISTENER_READ_TIMEOUT)
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

    pub fn start_listener<F>(device_id: &str, on_message: F) -> Result<ListenerStarted, String>
    where
        F: Fn(IncomingSms) + Send + 'static,
    {
        let port_name = port_name_from_id(device_id)?.to_string();
        let (stop_tx, stop_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread_name = format!("cellularhub-at-{}", port_name.replace(['\\', '/', ':'], "_"));

        let thread = thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                let opened = open_receive_session(&port_name);
                let (mut port, mode, operator, signal) = match opened {
                    Ok(value) => value,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };

                if ready_tx.send(Ok((operator, signal))).is_err() {
                    return;
                }

                if let Ok(messages) = read_unread_snapshot(&port_name, port.as_mut(), mode) {
                    for message in messages {
                        on_message(message);
                    }
                }
                run_listener(&port_name, port.as_mut(), mode, stop_rx, on_message);
            })
            .map_err(|error| format!("failed to start AT listener thread: {error}"))?;

        match ready_rx.recv_timeout(LISTENER_START_TIMEOUT) {
            Ok(Ok((operator, signal))) => Ok(ListenerStarted {
                handle: AtListenerHandle {
                    stop: Some(stop_tx),
                    thread: Some(thread),
                },
                operator,
                signal,
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(error) => {
                let _ = stop_tx.send(());
                let _ = thread.join();
                Err(format!("AT listener did not become ready: {error}"))
            }
        }
    }

    fn port_name_from_id(device_id: &str) -> Result<&str, String> {
        let port_name = device_id
            .strip_prefix("at:")
            .ok_or_else(|| "not an AT provider device id".to_string())?;
        if port_name.trim().is_empty() {
            return Err("AT device id has an empty port name".into());
        }
        Ok(port_name)
    }

    fn poll_open_port(port_name: &str, port: &mut dyn SerialPort) -> Result<AtPoll, String> {
        let (mode, operator, signal) = initialize_receive_session(port)?;
        let messages = read_unread_snapshot(port_name, port, mode)?;
        Ok(AtPoll {
            messages,
            operator,
            signal,
        })
    }

    fn open_receive_session(
        port_name: &str,
    ) -> Result<(Box<dyn SerialPort>, SmsMode, Option<String>, Option<u8>), String> {
        let mut last_error = String::new();
        for &baud in BAUD_RATES {
            match serialport::new(port_name, baud)
                .timeout(LISTENER_READ_TIMEOUT)
                .open()
            {
                Ok(mut port) => match initialize_receive_session(port.as_mut()) {
                    Ok((mode, operator, signal)) => return Ok((port, mode, operator, signal)),
                    Err(error) => last_error = format!("{baud} baud: {error}"),
                },
                Err(error) => last_error = format!("{baud} baud: {error}"),
            }
        }
        Err(format!("could not establish background AT session on {port_name}: {last_error}"))
    }

    fn initialize_receive_session(
        port: &mut dyn SerialPort,
    ) -> Result<(SmsMode, Option<String>, Option<u8>), String> {
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
        let mode = if text_mode.iter().any(|line| is_terminal_ok(line)) {
            SmsMode::Text
        } else {
            let pdu_mode = command(port, "AT+CMGF=0")?;
            require_ok(&pdu_mode, "AT+CMGF=0")?;
            SmsMode::Pdu
        };

        // Some modems reject this CNMI form but still permit CMGL/CMGR polling.
        // Listener mode therefore falls back to a 30-second unread scan.
        let _ = command(port, "AT+CNMI=2,1,0,0,0");
        Ok((mode, operator, signal))
    }

    fn read_unread_snapshot(
        port_name: &str,
        port: &mut dyn SerialPort,
        mode: SmsMode,
    ) -> Result<Vec<IncomingSms>, String> {
        match mode {
            SmsMode::Text => {
                let mut lines = command(port, "AT+CMGL=\"REC UNREAD\"")?;
                if require_ok(&lines, "AT+CMGL=REC UNREAD").is_err() {
                    lines = command(port, "AT+CMGL=\"ALL\"")?;
                    require_ok(&lines, "AT+CMGL=ALL")?;
                }
                Ok(parse_text_cmgl(port_name, &lines))
            }
            SmsMode::Pdu => {
                let lines = command(port, "AT+CMGL=0")?;
                require_ok(&lines, "AT+CMGL=0")?;
                Ok(parse_pdu_cmgl(port_name, &lines))
            }
        }
    }

    fn run_listener<F>(
        port_name: &str,
        port: &mut dyn SerialPort,
        mode: SmsMode,
        stop: Receiver<()>,
        on_message: F,
    ) where
        F: Fn(IncomingSms),
    {
        let _ = port.set_timeout(LISTENER_READ_TIMEOUT);
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 512];
        let mut next_scan = Instant::now() + FALLBACK_SCAN_INTERVAL;

        loop {
            if stop.try_recv().is_ok() {
                break;
            }

            match port.read(&mut chunk) {
                Ok(0) => {}
                Ok(count) => {
                    bytes.extend_from_slice(&chunk[..count]);
                    for line in drain_complete_lines(&mut bytes) {
                        if let Some(notification) = parse_cmti(&line) {
                            if let Ok(Some(message)) = read_index(port_name, port, mode, &notification) {
                                on_message(message);
                            }
                        }
                    }
                }
                Err(error) if error.kind() == ErrorKind::TimedOut => {}
                Err(_) => break,
            }

            if Instant::now() >= next_scan {
                if let Ok(messages) = read_unread_snapshot(port_name, port, mode) {
                    for message in messages {
                        on_message(message);
                    }
                }
                next_scan = Instant::now() + FALLBACK_SCAN_INTERVAL;
            }
        }
    }

    fn read_index(
        port_name: &str,
        port: &mut dyn SerialPort,
        mode: SmsMode,
        notification: &CmtiNotification,
    ) -> Result<Option<IncomingSms>, String> {
        if valid_storage_name(&notification.storage) {
            let _ = command(
                port,
                &format!("AT+CPMS=\"{}\"", notification.storage),
            );
        }
        let lines = command(port, &format!("AT+CMGR={}", notification.index))?;
        require_ok(&lines, "AT+CMGR")?;
        Ok(match mode {
            SmsMode::Text => parse_text_cmgr(port_name, notification.index, &lines),
            SmsMode::Pdu => parse_pdu_cmgr(port_name, notification.index, &lines),
        })
    }

    fn valid_storage_name(storage: &str) -> bool {
        !storage.is_empty()
            && storage.len() <= 8
            && storage.chars().all(|ch| ch.is_ascii_alphanumeric())
    }

    fn drain_complete_lines(bytes: &mut Vec<u8>) -> Vec<String> {
        let mut output = Vec::new();
        loop {
            let Some(index) = bytes.iter().position(|byte| matches!(*byte, b'\r' | b'\n')) else {
                break;
            };
            let raw: Vec<u8> = bytes.drain(..=index).collect();
            let line = String::from_utf8_lossy(&raw)
                .trim_matches(['\r', '\n', ' ', '\t'])
                .to_string();
            if !line.is_empty() {
                output.push(line);
            }
        }
        output
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
        port.set_timeout(Duration::from_millis(120))
            .map_err(|error| error.to_string())?;
        port.write_all(command.as_bytes())
            .map_err(|error| error.to_string())?;
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
                        || text
                            .lines()
                            .rev()
                            .find(|line| !line.trim().is_empty())
                            .is_some_and(is_terminal_ok)
                    {
                        break;
                    }
                }
                Err(error) if error.kind() == ErrorKind::TimedOut => {}
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
    fn parses_text_cmgr_record() {
        let lines = vec![
            r#"+CMGR: "REC UNREAD","10086",,"26/09/10,08:00:00+32""#.into(),
            "【服务】验证码 284193".into(),
            "OK".into(),
        ];
        let message = parse_text_cmgr("COM3", 12, &lines).unwrap();
        assert_eq!(message.sender, "10086");
        assert_eq!(message.body, "【服务】验证码 284193");
        assert!(message.source_key.contains(":12:"));
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
