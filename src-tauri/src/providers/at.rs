//! AT-command SMS fallback primitives.
//!
//! This module deliberately stops at protocol parsing and initialization plans.
//! Serial-port ownership will be added in the next hardware slice so CellularHub
//! never claims an AT modem is usable before the transport has actually opened.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmtiNotification {
    pub storage: String,
    pub index: u32,
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
